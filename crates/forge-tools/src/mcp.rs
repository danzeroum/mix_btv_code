//! Cliente MCP (Fase 6 Onda 4): conecta a servidores MCP externos (via `rmcp`,
//! transporte child-process/stdio), lista suas tools e as expõe como `dyn Tool`
//! no `ToolRegistry` — **sob o mesmo motor de permissões** (tool MCP = tool como
//! qualquer outra: pede permissão, entra no ledger). Nomes são namespaced
//! (`mcp__<server>__<tool>`) para não colidir com built-ins/skills.
//!
//! A chamada do rmcp é async; o `Tool::run` é sync. Atravessamos com a mesma
//! ponte do sandbox (thread dedicada + runtime próprio) — `run_on_thread`.
//! Conexão por chamada (connect→call→encerra): simples e sem estado
//! compartilhado; a sessão persistente é uma otimização registrada em
//! pendencias.md.

use crate::{bound_output, Tool, ToolError, ToolOutput, ToolRegistry, DEFAULT_OUTPUT_LIMIT};
use serde::Serialize;
use serde_json::Value;
use std::future::Future;

/// Um servidor MCP declarado pelo usuário: o comando que o sobe via stdio.
#[derive(Debug, Clone, Serialize)]
pub struct McpServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Metadados de uma tool anunciada por um servidor MCP.
#[derive(Serialize)]
pub struct McpToolMeta {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Uma tool MCP exposta como `dyn Tool`. Guarda o servidor e o nome real da
/// tool; `run` reconecta e chama. Nome público é namespaced.
pub struct McpTool {
    full_name: String,
    description: String,
    input_schema: Value,
    server: McpServerConfig,
    tool: String,
}

impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.full_name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }
    fn scope(&self, args: &Value) -> String {
        // Informativo para o permission-engine; permite regras por servidor/tool.
        let preview: String = args.to_string().chars().take(60).collect();
        format!("mcp:{}/{} {}", self.server.id, self.tool, preview)
    }
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let out = call_tool_blocking(&self.server, &self.tool, args.clone())
            .map_err(ToolError::Execution)?;
        Ok(bound_output(out, DEFAULT_OUTPUT_LIMIT))
    }
}

/// Conecta, lista as tools e devolve os metadados (sync, via ponte).
pub fn list_tools_blocking(config: &McpServerConfig) -> Result<Vec<McpToolMeta>, String> {
    let config = config.clone();
    run_on_thread(move || async move {
        let client = connect(&config).await?;
        let tools = client.list_all_tools().await.map_err(|e| e.to_string())?;
        let _ = client.cancel().await;
        Ok(tools
            .into_iter()
            .map(|t| McpToolMeta {
                name: t.name.to_string(),
                description: t.description.map(|d| d.to_string()).unwrap_or_default(),
                input_schema: Value::Object((*t.input_schema).clone()),
            })
            .collect())
    })
}

/// Chama uma tool MCP e devolve o texto do resultado (sync, via ponte).
fn call_tool_blocking(config: &McpServerConfig, tool: &str, args: Value) -> Result<String, String> {
    let config = config.clone();
    let tool = tool.to_string();
    run_on_thread(move || async move {
        let client = connect(&config).await?;
        let mut params = rmcp::model::CallToolRequestParams::new(tool);
        if let Value::Object(m) = args {
            params = params.with_arguments(m);
        }
        let result = client.call_tool(params).await.map_err(|e| e.to_string())?;
        let _ = client.cancel().await;
        Ok(render_content(&result))
    })
}

/// Conecta a um servidor MCP via transporte child-process (stdio).
async fn connect(
    config: &McpServerConfig,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, String> {
    use rmcp::transport::TokioChildProcess;
    use rmcp::ServiceExt;
    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(&config.args);
    let transport = TokioChildProcess::new(cmd).map_err(|e| format!("spawn MCP: {e}"))?;
    ().serve(transport)
        .await
        .map_err(|e| format!("handshake MCP: {e}"))
}

/// Extrai o texto dos blocos de conteúdo do resultado (robusto ao shape exato:
/// serializa cada bloco e puxa o campo `text`, ignorando não-texto).
fn render_content(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| {
            serde_json::to_value(c)
                .ok()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(String::from))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ponte sync→async: roda o future numa thread dedicada com runtime próprio
/// (mesma técnica do sandbox — não dá para aninhar `block_on` no worker do loop).
fn run_on_thread<F, Fut, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, String>>,
    T: Send + 'static,
{
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("runtime MCP: {e}"))?;
        rt.block_on(f())
    })
    .join()
    .map_err(|_| "thread MCP entrou em pânico".to_string())?
}

/// Conecta ao servidor, lista as tools e registra cada uma como `McpTool`
/// namespaced no registry. Guarda de colisão como o loader de skills. Devolve
/// quantas foram registradas.
pub fn register_mcp_server(
    registry: &mut ToolRegistry,
    config: &McpServerConfig,
) -> Result<usize, String> {
    let metas = list_tools_blocking(config)?;
    let mut n = 0;
    for m in metas {
        let full_name = format!("mcp__{}__{}", config.id, m.name);
        if registry.get(&full_name).is_some() {
            eprintln!("  mcp tool '{full_name}' colide com um tool já registrado — pulada");
            continue;
        }
        registry.register(Box::new(McpTool {
            full_name,
            description: m.description,
            input_schema: m.input_schema,
            server: config.clone(),
            tool: m.name,
        }));
        n += 1;
    }
    Ok(n)
}

// Os testes de integração cross-process (precisam de
// `CARGO_BIN_EXE_forge_mcp_fixture`, exposto pelo cargo só a integration tests)
// vivem em `crates/forge-tools/tests/mcp_integration.rs`.
