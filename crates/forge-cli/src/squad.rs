//! Comando `forge squad`: delega a tarefa ao squad multi-agente Python
//! (Onda 4d). Fecha o laço bidirecional com o `Gateway` real como
//! `CoreBackend` — as API keys ficam só aqui (ADR 0001), o Python só
//! conhece o UDS. Fallback progressivo de 3 níveis: squad → agente-único
//! → safe-mode read-only.

use crate::session::{now_rfc3339, Session};
use crate::{run_once, RunOpts};
use anyhow::Result;
use forge_llm::chat::{ChatMessage, GenerateRequest};
use forge_llm::Generator;
use forge_proto::core::PermissionRequest;
use forge_proto::llm::{LlmRequest, Usage};
use forge_proto::squad::{squad_event, SquadTask};
use forge_sidecar::{serve_core, CoreBackend, SquadRun, SquadSupervisor};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// `CoreBackend` real: `Generate` passa pelo `Gateway` (streaming agregado),
/// `RequestPermission` resolve HITL no terminal (ou auto-aprova com `--yes`).
struct GatewayCoreBackend<G: Generator> {
    generator: Arc<G>,
    auto_yes: bool,
}

#[derive(serde::Deserialize)]
struct WireMsg {
    role: String,
    content: String,
}

#[tonic::async_trait]
impl<G: Generator + Send + Sync + 'static> CoreBackend for GatewayCoreBackend<G> {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        let msgs: Vec<WireMsg> = serde_json::from_str(&req.messages_json)
            .map_err(|e| format!("messages_json inválido: {e}"))?;
        let mut system = String::new();
        let mut chat = Vec::new();
        for m in msgs {
            if m.role == "system" {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&m.content);
            } else {
                chat.push(ChatMessage::user_text(&m.content));
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
        let turn = self
            .generator
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
}

/// Localiza o workspace Python do sidecar: `FORGE_PYTHON_DIR`, senão um
/// `python/pyproject.toml` subindo a partir do binário ou do cwd.
fn locate_python_dir() -> Option<PathBuf> {
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
