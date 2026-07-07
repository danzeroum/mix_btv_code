//! Cliente LSP (Fase 6 Onda 5): sobe o language server do projeto
//! (rust-analyzer/pyright conforme o workspace), fala o protocolo LSP **real**
//! (JSON-RPC com framing `Content-Length` sobre stdio) e expõe as consultas
//! semânticas — definição, referências, diagnósticos — como `dyn Tool` no
//! `ToolRegistry`, **sob o mesmo motor de permissões** que qualquer tool.
//!
//! O framing LSP é simples o bastante para não puxar dependência nenhuma: só
//! `serde_json` (que já é dep). Isso mantém o `cargo deny` leve e nos dá controle
//! total — provado por um probe contra o rust-analyzer de verdade (ver o teste de
//! integração real, `lsp_integration.rs`).
//!
//! **Sessão persistente (≠ MCP):** o language server é caro de subir (o
//! rust-analyzer indexa o workspace, ~segundos). Diferente do MCP (connect por
//! chamada), aqui a sessão é **preguiçosa e reusada**: sobe uma vez no primeiro
//! uso e as consultas seguintes reaproveitam o processo já indexado. O processo é
//! morto no `Drop` (lição do process-group da Fase 4 — nada de órfão).

use crate::{bound_output, Tool, ToolError, ToolOutput, ToolRegistry, DEFAULT_OUTPUT_LIMIT};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Quanto esperar o server indexar antes de desistir de uma consulta (o
/// rust-analyzer devolve resultado vazio enquanto indexa; a gente re-tenta).
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Orçamento para os diagnósticos assentarem (são empurrados de forma assíncrona
/// pelo server após o `didOpen`; não há sinal claro de "acabou").
const DIAG_BUDGET: Duration = Duration::from_secs(12);

/// Um language server declarado pelo usuário: o comando que o sobe via stdio, e
/// a raiz do workspace que ele deve analisar.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub root: PathBuf,
}

/// As consultas que expomos como tool. Fixas (o LSP oferece um conjunto
/// conhecido), diferente do MCP onde as tools são anunciadas dinamicamente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspQuery {
    Definition,
    References,
    Diagnostics,
}

impl LspQuery {
    fn as_str(self) -> &'static str {
        match self {
            LspQuery::Definition => "definition",
            LspQuery::References => "references",
            LspQuery::Diagnostics => "diagnostics",
        }
    }
}

/// Um processo de language server vivo, com os canais e o estado de sessão.
struct LspProc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    /// URIs já abertas (`didOpen` só uma vez por documento — reabrir viola o
    /// protocolo).
    opened: HashSet<String>,
    /// Últimos diagnósticos empurrados por URI (`textDocument/publishDiagnostics`).
    diagnostics: HashMap<String, Value>,
}

impl Drop for LspProc {
    fn drop(&mut self) {
        // Mata o server — sem shutdown/exit handshake (pode travar); o kill é
        // o robusto. Nada de rust-analyzer órfão comendo CPU.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A sessão LSP: preguiçosa (sobe no primeiro uso) e compartilhada pelas três
/// tools do mesmo server (`Arc<LspSession>`), para não subir três processos.
pub struct LspSession {
    config: LspServerConfig,
    proc: Mutex<Option<LspProc>>,
}

impl LspSession {
    pub fn new(config: LspServerConfig) -> Self {
        Self {
            config,
            proc: Mutex::new(None),
        }
    }

    /// URI `file://` canônica do arquivo (bate com o que o server devolve).
    fn uri_for(&self, file: &str) -> String {
        let p = PathBuf::from(file);
        let abs = if p.is_absolute() {
            p
        } else {
            self.config.root.join(p)
        };
        let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
        format!("file://{}", abs.display())
    }

    fn read_file(&self, file: &str) -> Result<String, String> {
        let p = PathBuf::from(file);
        let abs = if p.is_absolute() {
            p
        } else {
            self.config.root.join(p)
        };
        std::fs::read_to_string(&abs).map_err(|e| format!("ler {}: {e}", abs.display()))
    }

    /// Sobe o server e faz o handshake (`initialize`/`initialized`) se ainda não
    /// estiver de pé. Chamado sob o lock.
    fn ensure_started(slot: &mut Option<LspProc>, config: &LspServerConfig) -> Result<(), String> {
        if slot.is_some() {
            return Ok(());
        }
        let root = std::fs::canonicalize(&config.root).unwrap_or_else(|_| config.root.clone());
        let root_uri = format!("file://{}", root.display());

        let mut child = Command::new(&config.command)
            .args(&config.args)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("subir LSP '{}': {e}", config.command))?;
        let mut stdin = child.stdin.take().ok_or("LSP sem stdin")?;
        let mut stdout = BufReader::new(child.stdout.take().ok_or("LSP sem stdout")?);

        write_msg(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                    "processId": null,
                    "rootUri": root_uri,
                    "capabilities": {},
                    "workspaceFolders": [{ "uri": root_uri, "name": "forge" }]
                }
            }),
        )?;
        // Aguarda a resposta do initialize, respondendo requests do servidor.
        loop {
            let m = read_msg(&mut stdout)?;
            if m.get("method").is_none() && m.get("id").and_then(Value::as_i64) == Some(1) {
                break;
            }
            respond_server_request(&mut stdin, &m)?;
        }
        write_msg(
            &mut stdin,
            &json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
        )?;

        *slot = Some(LspProc {
            child,
            stdin,
            stdout,
            next_id: 1,
            opened: HashSet::new(),
            diagnostics: HashMap::new(),
        });
        Ok(())
    }

    fn ensure_open(proc: &mut LspProc, uri: &str, text: &str) -> Result<(), String> {
        if proc.opened.contains(uri) {
            return Ok(());
        }
        let lang = language_id(uri);
        write_msg(
            &mut proc.stdin,
            &json!({
                "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                    "textDocument": { "uri": uri, "languageId": lang, "version": 1, "text": text }
                }
            }),
        )?;
        proc.opened.insert(uri.to_string());
        Ok(())
    }

    /// Envia um request e lê até a resposta com o `id`, drenando notificações
    /// (guardando `publishDiagnostics`) e respondendo requests do servidor no
    /// caminho — assim o pipe não entope entre consultas.
    fn request(proc: &mut LspProc, method: &str, params: Value) -> Result<Value, String> {
        proc.next_id += 1;
        let id = proc.next_id;
        write_msg(
            &mut proc.stdin,
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
        )?;
        loop {
            let m = read_msg(&mut proc.stdout)?;
            let has_method = m.get("method").is_some();
            if !has_method {
                // resposta a algum request nosso
                if m.get("id").and_then(Value::as_i64) == Some(id) {
                    if let Some(err) = m.get("error") {
                        return Err(format!("LSP {method}: {err}"));
                    }
                    return Ok(m.get("result").cloned().unwrap_or(Value::Null));
                }
                continue; // resposta velha, ignora
            }
            if m.get("id").is_some() {
                // request do servidor → responde para não travar
                respond_server_request(&mut proc.stdin, &m)?;
                continue;
            }
            // notificação — guarda diagnósticos, ignora o resto
            if m.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
                if let Some(params) = m.get("params") {
                    if let Some(uri) = params.get("uri").and_then(Value::as_str) {
                        proc.diagnostics.insert(
                            uri.to_string(),
                            params.get("diagnostics").cloned().unwrap_or(json!([])),
                        );
                    }
                }
            }
        }
    }

    /// `textDocument/definition` na posição (0-indexed, convenção LSP), com
    /// retry enquanto o server indexa. Devolve o JSON cru do resultado.
    pub fn definition(&self, file: &str, line: u64, character: u64) -> Result<Value, String> {
        self.position_query("textDocument/definition", file, line, character, false)
    }

    /// `textDocument/references` na posição (0-indexed), incluindo a declaração.
    pub fn references(&self, file: &str, line: u64, character: u64) -> Result<Value, String> {
        self.position_query("textDocument/references", file, line, character, true)
    }

    fn position_query(
        &self,
        method: &str,
        file: &str,
        line: u64,
        character: u64,
        include_declaration: bool,
    ) -> Result<Value, String> {
        let uri = self.uri_for(file);
        let text = self.read_file(file)?;
        let mut guard = self.proc.lock().map_err(|_| "lock LSP envenenado")?;
        Self::ensure_started(&mut guard, &self.config)?;
        let proc = guard.as_mut().expect("proc iniciado");
        Self::ensure_open(proc, &uri, &text)?;

        let mut params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });
        if include_declaration {
            params["context"] = json!({ "includeDeclaration": true });
        }

        // Enquanto o rust-analyzer indexa, a resposta vem vazia; re-tenta.
        let start = Instant::now();
        loop {
            let res = Self::request(proc, method, params.clone())?;
            if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
                return Ok(res);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Diagnósticos do arquivo. São empurrados de forma assíncrona pelo server
    /// após o `didOpen`; a gente bombeia round-trips baratos (`documentSymbol`)
    /// para drenar as notificações até assentarem ou estourar o orçamento.
    pub fn diagnostics(&self, file: &str) -> Result<Value, String> {
        let uri = self.uri_for(file);
        let text = self.read_file(file)?;
        let mut guard = self.proc.lock().map_err(|_| "lock LSP envenenado")?;
        Self::ensure_started(&mut guard, &self.config)?;
        let proc = guard.as_mut().expect("proc iniciado");
        Self::ensure_open(proc, &uri, &text)?;

        let start = Instant::now();
        let mut first_seen: Option<Instant> = None;
        loop {
            // Bombeia um round-trip: drena publishDiagnostics para o stash.
            let _ = Self::request(
                proc,
                "textDocument/documentSymbol",
                json!({ "textDocument": { "uri": uri } }),
            );
            let stashed = proc.diagnostics.get(&uri).cloned();
            match &stashed {
                Some(v) if !is_empty(v) => return Ok(stashed.unwrap()),
                Some(_) => {
                    // URI já reportada (talvez vazia = arquivo limpo). Dá um
                    // tempo curto para um diagnóstico tardio, senão devolve vazio.
                    let seen = first_seen.get_or_insert_with(Instant::now);
                    if seen.elapsed() > Duration::from_secs(3) {
                        return Ok(stashed.unwrap());
                    }
                }
                None => {}
            }
            if start.elapsed() > DIAG_BUDGET {
                return Ok(stashed.unwrap_or_else(|| json!([])));
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

/// Uma consulta LSP exposta como `dyn Tool`. As três (definição/referências/
/// diagnósticos) de um mesmo server compartilham a `Arc<LspSession>`.
pub struct LspTool {
    full_name: String,
    kind: LspQuery,
    server_id: String,
    session: Arc<LspSession>,
}

impl Tool for LspTool {
    fn name(&self) -> &str {
        &self.full_name
    }
    fn description(&self) -> &str {
        match self.kind {
            LspQuery::Definition => {
                "LSP: definição do símbolo na posição (file, line, character — 0-indexed)"
            }
            LspQuery::References => {
                "LSP: referências do símbolo na posição (file, line, character — 0-indexed)"
            }
            LspQuery::Diagnostics => "LSP: diagnósticos (erros/avisos) do arquivo",
        }
    }
    fn input_schema(&self) -> Value {
        match self.kind {
            LspQuery::Diagnostics => json!({
                "type": "object",
                "properties": { "file": { "type": "string" } },
                "required": ["file"]
            }),
            _ => json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "line": { "type": "integer", "minimum": 0 },
                    "character": { "type": "integer", "minimum": 0 }
                },
                "required": ["file", "line", "character"]
            }),
        }
    }
    fn scope(&self, args: &Value) -> String {
        let file = args.get("file").and_then(Value::as_str).unwrap_or("");
        format!("lsp:{}/{} {}", self.server_id, self.kind.as_str(), file)
    }
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let file = args
            .get("file")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArgs("campo 'file' obrigatório".into()))?;

        let out = match self.kind {
            LspQuery::Diagnostics => {
                let res = self
                    .session
                    .diagnostics(file)
                    .map_err(ToolError::Execution)?;
                render_diagnostics(file, &res)
            }
            _ => {
                let line = args.get("line").and_then(Value::as_u64).ok_or_else(|| {
                    ToolError::InvalidArgs("campo 'line' obrigatório (0-indexed)".into())
                })?;
                let character = args
                    .get("character")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        ToolError::InvalidArgs("campo 'character' obrigatório (0-indexed)".into())
                    })?;
                let res = if self.kind == LspQuery::Definition {
                    self.session.definition(file, line, character)
                } else {
                    self.session.references(file, line, character)
                }
                .map_err(ToolError::Execution)?;
                render_locations(&res)
            }
        };
        Ok(bound_output(out, DEFAULT_OUTPUT_LIMIT))
    }
}

/// Registra as três tools (`lsp__<id>__{definition,references,diagnostics}`) de
/// um server no registry, compartilhando uma sessão preguiçosa. **Não sobe** o
/// server — isso é feito no primeiro uso (o server é caro). Guarda de colisão
/// como os loaders de skill/MCP. Devolve quantas foram registradas.
pub fn register_lsp_server(registry: &mut ToolRegistry, config: &LspServerConfig) -> usize {
    let session = Arc::new(LspSession::new(config.clone()));
    let mut n = 0;
    for kind in [
        LspQuery::Definition,
        LspQuery::References,
        LspQuery::Diagnostics,
    ] {
        let full_name = format!("lsp__{}__{}", config.id, kind.as_str());
        if registry.get(&full_name).is_some() {
            eprintln!("  lsp tool '{full_name}' colide com um tool já registrado — pulada");
            continue;
        }
        registry.register(Box::new(LspTool {
            full_name,
            kind,
            server_id: config.id.clone(),
            session: session.clone(),
        }));
        n += 1;
    }
    n
}

// --- helpers de protocolo (framing Content-Length, sem dependências) ---

fn write_msg(w: &mut impl Write, v: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    w.write_all(&body).map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())
}

fn read_msg(r: &mut impl BufRead) -> Result<Value, String> {
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("EOF do servidor LSP".into());
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(rest) = t.strip_prefix("Content-Length:") {
            len = rest
                .trim()
                .parse()
                .map_err(|_| "Content-Length inválido".to_string())?;
        }
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(|e| e.to_string())?;
    serde_json::from_slice(&buf).map_err(|e| e.to_string())
}

/// Responde a um request do servidor (tem `id` e `method`) para não travar o
/// handshake. `workspace/configuration` espera um array (um item por pedido);
/// o resto aceita `null`.
fn respond_server_request(w: &mut impl Write, m: &Value) -> Result<(), String> {
    let (Some(id), Some(method)) = (m.get("id"), m.get("method").and_then(Value::as_str)) else {
        return Ok(());
    };
    let result = if method == "workspace/configuration" {
        let n = m
            .get("params")
            .and_then(|p| p.get("items"))
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(1);
        Value::Array(vec![Value::Null; n])
    } else {
        Value::Null
    };
    write_msg(w, &json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn is_empty(v: &Value) -> bool {
    v.is_null() || v.as_array().map(|a| a.is_empty()).unwrap_or(false)
}

fn language_id(uri: &str) -> &'static str {
    if uri.ends_with(".rs") {
        "rust"
    } else if uri.ends_with(".py") {
        "python"
    } else {
        "plaintext"
    }
}

/// Converte `file:///a/b.rs` no caminho `/a/b.rs` para exibição.
fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

/// Renderiza `Location | Location[] | LocationLink[]` como linhas
/// `caminho:line:character` (0-indexed, convenção LSP). Robusto ao shape.
fn render_locations(v: &Value) -> String {
    let items: Vec<Value> = match v {
        Value::Array(a) => a.clone(),
        Value::Null => vec![],
        other => vec![other.clone()],
    };
    let mut lines = Vec::new();
    for it in items {
        // Location: {uri, range}; LocationLink: {targetUri, targetSelectionRange|targetRange}
        let uri = it
            .get("uri")
            .or_else(|| it.get("targetUri"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let range = it
            .get("range")
            .or_else(|| it.get("targetSelectionRange"))
            .or_else(|| it.get("targetRange"));
        let (line, ch) = range
            .and_then(|r| r.get("start"))
            .map(|s| {
                (
                    s.get("line").and_then(Value::as_u64).unwrap_or(0),
                    s.get("character").and_then(Value::as_u64).unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));
        lines.push(format!("{}:{}:{}", uri_to_path(uri), line, ch));
    }
    if lines.is_empty() {
        "(nenhum resultado)".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_diagnostics(file: &str, v: &Value) -> String {
    let Some(arr) = v.as_array() else {
        return "(sem diagnósticos)".to_string();
    };
    if arr.is_empty() {
        return format!("{file}: sem diagnósticos");
    }
    let mut lines = Vec::new();
    for d in arr {
        let (line, ch) = d
            .get("range")
            .and_then(|r| r.get("start"))
            .map(|s| {
                (
                    s.get("line").and_then(Value::as_u64).unwrap_or(0),
                    s.get("character").and_then(Value::as_u64).unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));
        let sev = match d.get("severity").and_then(Value::as_u64) {
            Some(1) => "error",
            Some(2) => "warning",
            Some(3) => "info",
            Some(4) => "hint",
            _ => "diag",
        };
        let msg = d.get("message").and_then(Value::as_str).unwrap_or("");
        lines.push(format!("{file}:{line}:{ch}: {sev}: {msg}"));
    }
    lines.join("\n")
}

// Os testes vivem em `crates/forge-tools/tests/lsp_integration.rs`: um hermético
// (server fixture, sempre roda) e um contra o rust-analyzer REAL (ignored; roda
// no CI com a componente instalada).
