//! Comando `forge squad`: delega a tarefa ao squad multi-agente Python
//! (Onda 4d). Fecha o laço bidirecional com o `Gateway` real como
//! `CoreBackend` — as API keys ficam só aqui (ADR 0001), o Python só
//! conhece o UDS. Fallback progressivo de 3 níveis: squad → agente-único
//! → safe-mode read-only.

use crate::session::{now_rfc3339, Session};
use crate::{run_once, RunOpts};
use anyhow::Result;
use forge_core::{Decision, PermissionEngine};
use forge_llm::chat::{ChatMessage, ContentBlock, GenerateRequest, Role};
use forge_llm::Generator;
use forge_proto::core::{PermissionRequest, ToolCall, ToolResult};
use forge_proto::llm::{LlmRequest, Usage};
use forge_proto::squad::{squad_event, SquadTask};
use forge_sidecar::{serve_core, CoreBackend, SquadRun, SquadSupervisor};
use forge_tools::ToolRegistry;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// `ToolResult.exit_code`: convenção compartilhada pelos três `CoreBackend`
/// de produção (ver `core_run_tool`). `0` = sucesso; `1` = erro de
/// execução/args inválidos/ferramenta desconhecida (nunca rodou, ou rodou e
/// falhou — vale tentar de novo com outra entrada); `-1` = negado pelo
/// motor de permissões ou por um humano (nunca chegou a executar — não
/// adianta repetir a mesma ação).
pub(crate) const TOOL_EXIT_OK: i32 = 0;
pub(crate) const TOOL_EXIT_ERROR: i32 = 1;
pub(crate) const TOOL_EXIT_DENIED: i32 = -1;

/// `CoreBackend::run_tool` de verdade, compartilhado pelos três backends de
/// produção (CLI, web, scripted). Recalcula o escopo a partir de
/// `args_json` via `Tool::scope` — o `ToolCall.scope` vindo da rede NUNCA é
/// usado na decisão de permissão (só o Rust decide escopo; um Python
/// bugado/comprometido não pode declarar um escopo mais permissivo que o
/// real). Avalia via `PermissionEngine`; no caso `Ask`, delega a decisão a
/// `ask` (o mesmo bridge de HITL que o backend já usa para
/// `request_permission` — stdin no CLI, `SquadHub::request_hitl` na web).
/// A execução síncrona (`tool.run`) roda dentro de `spawn_blocking`; a
/// checagem de permissão (incluindo o `Ask` assíncrono) fica fora — não
/// bloqueia uma worker-thread do reactor esperando um clique humano.
/// Registra cada chamada no ledger de `root` (best-effort — falha de
/// ledger nunca derruba a execução da ferramenta).
pub(crate) async fn core_run_tool<F, Fut>(
    tools: &Arc<ToolRegistry>,
    permissions: &PermissionEngine,
    call: &ToolCall,
    root: &Path,
    ask: F,
) -> ToolResult
where
    F: FnOnce(PermissionRequest) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let args: serde_json::Value = match serde_json::from_str(&call.args_json) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                content: format!("args_json inválido: {e}"),
                truncated: false,
                exit_code: TOOL_EXIT_ERROR,
            }
        }
    };
    if tools.get(&call.tool).is_none() {
        return ToolResult {
            content: format!("ferramenta desconhecida: {}", call.tool),
            truncated: false,
            exit_code: TOOL_EXIT_ERROR,
        };
    }
    let scope = tools.get(&call.tool).expect("validado acima").scope(&args);

    let allowed = match permissions.evaluate(&call.tool, &scope) {
        Decision::Allow => true,
        Decision::Deny => false,
        Decision::Ask => {
            ask(PermissionRequest {
                tool: call.tool.clone(),
                scope: scope.clone(),
                reason: format!("squad pede '{}' em {scope:?}", call.tool),
                confidence: 0.0,
            })
            .await
        }
    };
    if !allowed {
        let result = ToolResult {
            content: format!("permissão negada para {} em {scope:?}", call.tool),
            truncated: false,
            exit_code: TOOL_EXIT_DENIED,
        };
        log_tool_run(root, call, &scope, &result);
        return result;
    }

    let tools_for_blocking = Arc::clone(tools);
    let tool_name = call.tool.clone();
    let run_result = tokio::task::spawn_blocking(move || {
        let tool = tools_for_blocking
            .get(&tool_name)
            .expect("validado antes do spawn_blocking");
        tool.run(&args)
    })
    .await;

    let result = match run_result {
        Ok(Ok(out)) => {
            let mut content = out.content;
            if out.truncated {
                match &out.overflow_path {
                    Some(path) => content.push_str(&format!(
                        "\n[output truncado; completo em {path} — use read para consultar]"
                    )),
                    None => content.push_str("\n[output truncado]"),
                }
            }
            ToolResult {
                content,
                truncated: out.truncated,
                exit_code: TOOL_EXIT_OK,
            }
        }
        Ok(Err(e)) => ToolResult {
            content: e.to_string(),
            truncated: false,
            exit_code: TOOL_EXIT_ERROR,
        },
        Err(e) => ToolResult {
            content: format!("falha interna ao rodar ferramenta: {e}"),
            truncated: false,
            exit_code: TOOL_EXIT_ERROR,
        },
    };
    log_tool_run(root, call, &scope, &result);
    result
}

/// Best-effort — nunca deixa uma falha de ledger derrubar a resposta do
/// `RunTool` (mesma postura de `Session::note`).
fn log_tool_run(root: &Path, call: &ToolCall, scope: &str, result: &ToolResult) {
    if let Err(e) = crate::session::append_entry(
        root,
        "forge-cli:squad-tool",
        "squad.tool_run",
        json!({
            "tool": call.tool,
            "scope": scope,
            "exit_code": result.exit_code,
            "truncated": result.truncated,
        }),
    ) {
        eprintln!("  [ledger] falha ao registrar squad.tool_run: {e}");
    }
}

/// `CoreBackend` real: `Generate` passa pelo `Gateway` (streaming agregado),
/// `RequestPermission` resolve HITL no terminal (ou auto-aprova com `--yes`),
/// `RunTool` executa de verdade sob `ToolRegistry`/`PermissionEngine`
/// ("tool execution architecture" — squad como executor).
struct GatewayCoreBackend<G: Generator> {
    generator: Arc<G>,
    auto_yes: bool,
    root: PathBuf,
    tools: Arc<ToolRegistry>,
    tool_permissions: PermissionEngine,
}

#[derive(serde::Deserialize)]
pub(crate) struct WireMsg {
    role: String,
    content: String,
}

/// `CoreBackend::generate` de verdade: desempacota `messages_json`, chama o
/// `Generator` real (mesmo Gateway/rate-limit/cache do resto da CLI) e
/// agrega a resposta. Compartilhado entre o backend do `forge squad` (CLI,
/// HITL via stdin) e o do agente web (Onda 4, HITL via HTTP) — a única
/// diferença entre os dois é `request_permission`.
pub(crate) async fn core_generate<G: Generator>(
    generator: &G,
    req: &LlmRequest,
) -> Result<(String, Usage), String> {
    let msgs: Vec<WireMsg> = serde_json::from_str(&req.messages_json)
        .map_err(|e| format!("messages_json inválido: {e}"))?;
    let mut system = String::new();
    let mut chat = Vec::new();
    for m in msgs {
        match m.role.as_str() {
            "system" => {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&m.content);
            }
            // Loop ReAct do squad (Onda 2) manda histórico multi-turno de
            // verdade — sem isto, um "assistant" cairia em `Role::User` e a
            // API da Anthropic (que exige alternância estrita user/
            // assistant) recusaria/malformaria a conversa. Todo caller
            // anterior mandava só 1 system + 1 user, então este ramo nunca
            // foi exercitado antes do loop ReAct existir.
            "assistant" => chat.push(ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::Text { text: m.content }],
            }),
            _ => chat.push(ChatMessage::user_text(&m.content)),
        }
    }
    let gen_req = GenerateRequest {
        model: req.model.clone(),
        system,
        messages: chat,
        tools: vec![],
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature,
    };
    let mut sink = |_: &str| {};
    let turn = generator
        .generate(gen_req, &mut sink)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        turn.text(),
        Usage {
            input_tokens: turn.usage.input_tokens,
            output_tokens: turn.usage.output_tokens,
            cache_hit: turn.provider.contains("+cache"),
            provider: turn.provider,
        },
    ))
}

#[tonic::async_trait]
impl<G: Generator + Send + Sync + 'static> CoreBackend for GatewayCoreBackend<G> {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        core_generate(self.generator.as_ref(), req).await
    }

    async fn request_permission(&self, req: &PermissionRequest) -> bool {
        if self.auto_yes {
            return true;
        }
        let prompt = format!(
            "\n  [HITL] o squad pede aprovação para '{}' (confiança {:.2}) — {}? [s/N] ",
            req.tool, req.confidence, req.reason
        );
        tokio::task::spawn_blocking(move || {
            use std::io::Write;
            eprint!("{prompt}");
            let _ = std::io::stderr().flush();
            let mut answer = String::new();
            if std::io::stdin().read_line(&mut answer).is_err() {
                return false;
            }
            matches!(
                answer.trim().to_lowercase().as_str(),
                "s" | "sim" | "y" | "yes"
            )
        })
        .await
        .unwrap_or(false)
    }

    async fn run_tool(&self, call: &ToolCall) -> ToolResult {
        core_run_tool(
            &self.tools,
            &self.tool_permissions,
            call,
            &self.root,
            |req| async move { self.request_permission(&req).await },
        )
        .await
    }
}

/// Localiza o workspace Python do sidecar: `FORGE_PYTHON_DIR`, senão um
/// `python/pyproject.toml` subindo a partir do binário ou do cwd.
/// `pub(crate)` — reusado por `squad_agent.rs` (Onda 4) e `prompt_render.rs`
/// (Onda 5) para achar o mesmo workspace Python dos dois sidecares.
pub(crate) fn locate_python_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("FORGE_PYTHON_DIR") {
        let p = PathBuf::from(dir);
        if p.join("pyproject.toml").exists() {
            return Some(p);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            candidates.push(ancestor.join("python"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("python"));
    }
    candidates
        .into_iter()
        .find(|p| p.join("pyproject.toml").exists())
}

/// Ponto de entrada do `forge squad`. Constrói o `CoreService` real e
/// tenta o squad; degrada em 3 níveis se necessário.
pub async fn run_squad<G: Generator + Send + Sync + 'static>(
    generator: G,
    opts: &RunOpts,
    root: &Path,
    task: String,
) -> Result<()> {
    let generator = Arc::new(generator);
    let forge_dir = root.join(".forge");
    std::fs::create_dir_all(&forge_dir)?;
    let pid = std::process::id();
    let core_sock = forge_dir.join(format!("squad-core-{pid}.sock"));
    let squad_sock = forge_dir.join(format!("squad-{pid}.sock"));

    let backend = GatewayCoreBackend {
        generator: generator.clone(),
        auto_yes: opts.yes,
        root: root.to_path_buf(),
        tools: Arc::new(ToolRegistry::default_set(root)),
        tool_permissions: (forge_core::BUILD.permissions)(),
    };
    let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
    for _ in 0..100 {
        if core_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Nível 1: squad multi-agente.
    let squad_result = try_squad(&core_sock, &squad_sock, opts, root, &task).await;
    core_task.abort();

    match squad_result {
        Ok(()) => Ok(()),
        Err(reason) => {
            eprintln!("\n  ⚠ squad indisponível ({reason}) — fallback nível 2: agente-único");
            // Nível 2: agente-único (Rust puro, sem Python).
            match run_once(generator.as_ref(), opts, root, task.clone()).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Nível 3: safe-mode read-only.
                    eprintln!(
                        "\n  ⚠ agente-único falhou ({e}) — fallback nível 3: safe-mode read-only"
                    );
                    safe_mode(&task);
                    Ok(())
                }
            }
        }
    }
}

/// Sobe o squad e drena o stream, registrando o consenso no ledger.
/// Devolve `Err(motivo)` se qualquer etapa falhar (dispara o fallback).
async fn try_squad(
    core_sock: &Path,
    squad_sock: &Path,
    opts: &RunOpts,
    root: &Path,
    task: &str,
) -> std::result::Result<(), String> {
    let py_dir = locate_python_dir()
        .ok_or_else(|| "workspace Python não encontrado (defina FORGE_PYTHON_DIR)".to_string())?;
    let mut supervisor =
        SquadSupervisor::spawn(&py_dir, squad_sock.to_path_buf(), core_sock, &opts.model)
            .map_err(|e| e.to_string())?;
    let mut client = supervisor
        .wait_ready(Duration::from_secs(30))
        .await
        .map_err(|e| e.to_string())?;

    let mut session = Session::open(root, task, &opts.model).map_err(|e| e.to_string())?;

    // Fase 5 Onda 3: roda o /verify sobre o workspace atual ANTES do squad e
    // anexa a evidência ao SquadTask — é isso que tira o auditor do vácuo
    // (julgar código no ar) e o coloca julgando sobre fatos determinísticos.
    // Evidência ausente/inválida do outro lado (server.py) é fail-closed, não
    // "sem evidência = ok" — por isso preferimos propagar um erro aqui a
    // enviar uma string vazia silenciosamente se o pipeline falhar ao rodar.
    eprintln!("  ⏱ rodando /verify sobre o workspace antes do squad (evidência para o auditor)…");
    let evidence = crate::run_verify_pipeline(root, None)
        .map_err(|e| format!("falha ao rodar /verify antes do squad: {e}"))?;
    eprintln!(
        "  ✓ /verify concluído — veredito {:?} ({} passo(s))",
        evidence.verdict,
        evidence.steps.len()
    );
    let verification_evidence_json = serde_json::to_string(&evidence)
        .map_err(|e| format!("falha ao serializar evidência: {e}"))?;

    // `max_autonomy_level` hardcoded (não uma flag de CLI, nem lido do
    // request web): confirmado nesta onda que o campo é ignorado
    // ponta-a-ponta hoje — `forge_squad/server.py::ExecuteTask` nunca lê
    // `request.max_autonomy_level`; a autonomia real vem de
    // `ProgressiveAutonomyManager`/`agent_trust_scores` (`hitl.py`),
    // desconectado deste campo do proto. Wire-lo até a UI seria só "o campo
    // viajou" sem efeito nenhum — descope explícito (ADR 0021), não
    // esquecimento. Ver `pendencias.md` (Onda 13).
    let stream = client
        .execute_task(SquadTask {
            task_id: format!("s{pid:x}", pid = std::process::id()),
            description: task.to_string(),
            decision_type: "architecture".into(),
            max_autonomy_level: 3,
            verification_evidence_json,
        })
        .await
        .map_err(|e| e.to_string())?;

    eprintln!("forge squad — {task:?}\n");
    // Drena manualmente para renderizar ao vivo e registrar o consenso.
    let outcome = render_and_record(stream, &mut session).await;
    match outcome {
        SquadRun::Completed(_) => {
            let _ = session.finish(true, 1);
            Ok(())
        }
        SquadRun::Failed { reason, .. } => {
            let _ = session.finish(false, 0);
            Err(reason)
        }
    }
}

async fn render_and_record(
    stream: tonic::Streaming<forge_proto::squad::SquadEvent>,
    session: &mut Session,
) -> SquadRun {
    // Reutiliza drain_stream mas com efeito colateral de render+ledger: como
    // drain_stream consome o stream, aqui replicamos o laço para poder
    // imprimir e registrar cada evento conforme chega.
    let mut inner = stream;
    let mut events = Vec::new();
    loop {
        match inner.message().await {
            Ok(Some(ev)) => {
                render_event(&ev, session);
                if let Some(squad_event::Payload::Error(reason)) = &ev.payload {
                    let reason = reason.clone();
                    events.push(ev);
                    return SquadRun::Failed { events, reason };
                }
                events.push(ev);
            }
            Ok(None) => return SquadRun::Completed(events),
            Err(status) => {
                return SquadRun::Failed {
                    events,
                    reason: status.to_string(),
                }
            }
        }
    }
}

fn render_event(ev: &forge_proto::squad::SquadEvent, session: &mut Session) {
    match &ev.payload {
        Some(squad_event::Payload::Proposal(p)) => {
            eprintln!("  · proposta {} (conf {:.2})", p.agent, p.confidence);
        }
        Some(squad_event::Payload::Consensus(c)) => {
            eprintln!(
                "  ⚖ consenso: {} (força {:.2}){}",
                c.decision_maker,
                c.strength,
                if c.requires_human {
                    " — pede HITL"
                } else {
                    ""
                }
            );
            // Critério da Fase 4: consenso registrado no ledger.
            session.note(
                "squad.consensus",
                json!({
                    "decision_maker": c.decision_maker,
                    "strength": c.strength,
                    "requires_human": c.requires_human,
                    "ts": now_rfc3339(),
                }),
            );
        }
        Some(squad_event::Payload::Handoff(h)) => {
            eprintln!(
                "  → handoff {}→{} (fase {})",
                h.from_agent, h.to_agent, h.phase
            );
        }
        Some(squad_event::Payload::Hitl(h)) => {
            eprintln!(
                "  ⏸ escalonamento HITL: {} (conf {:.2})",
                h.reason, h.confidence
            );
        }
        Some(squad_event::Payload::Step(s)) => {
            eprintln!(
                "  {} step {}: {}",
                if s.success { "✓" } else { "✗" },
                s.step_id,
                s.summary
            );
        }
        Some(squad_event::Payload::Error(e)) => eprintln!("  ✗ erro do squad: {e}"),
        Some(squad_event::Payload::Chat(c)) => eprintln!("  💬 {}: {}", c.author, c.text),
        None => {}
    }
}

fn safe_mode(task: &str) {
    eprintln!(
        "  safe-mode read-only: nenhum motor de agente disponível para {task:?}.\n  \
         nenhuma ação de escrita foi tomada. Configure um provider (ANTHROPIC_API_KEY etc.) \
         ou o sidecar Python para reativar squad/agente-único."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_llm::chat::{AssistantTurn, StopReason, Usage as ChatUsage};
    use forge_llm::gateway::GatewayError;
    use std::sync::Mutex;

    /// Gerador de teste que só registra as `messages` recebidas — usado
    /// para provar o mapeamento de papel de `core_generate` (Onda 2), sem
    /// precisar de um provider real.
    struct RecordingGenerator {
        received: Mutex<Vec<Vec<ChatMessage>>>,
    }

    impl Generator for RecordingGenerator {
        async fn generate(
            &self,
            req: GenerateRequest,
            _on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, GatewayError> {
            self.received.lock().unwrap().push(req.messages);
            Ok(AssistantTurn {
                content: vec![ContentBlock::Text { text: "ok".into() }],
                stop_reason: StopReason::EndTurn,
                usage: ChatUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
                provider: "recording".into(),
            })
        }
    }

    #[tokio::test]
    async fn core_generate_mapeia_papel_assistant_para_role_assistant() {
        let generator = RecordingGenerator {
            received: Mutex::new(Vec::new()),
        };
        let messages_json = serde_json::to_string(&serde_json::json!([
            {"role": "system", "content": "prompt de sistema"},
            {"role": "user", "content": "tarefa"},
            {"role": "assistant", "content": "{\"action\":\"tool_call\"}"},
            {"role": "user", "content": "observação"},
        ]))
        .unwrap();
        let req = LlmRequest {
            model: "m".into(),
            messages_json,
            temperature: None,
            max_tokens: None,
            requester: "developer".into(),
        };

        core_generate(&generator, &req).await.expect("generate ok");

        let received = generator.received.lock().unwrap();
        let messages = &received[0];
        // system não entra em `messages` (vira `GenerateRequest.system`) —
        // só as duas mensagens de chat + a de assistant sobram, na ordem.
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[2].role, Role::User);
    }
}
