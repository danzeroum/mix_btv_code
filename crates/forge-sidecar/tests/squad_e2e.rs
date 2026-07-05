//! Teste cross-process real do laço bidirecional completo (Onda 4d):
//!
//!   Rust CoreService (scripted)  ⇄  Python forge_squad.server (real)
//!
//! O Rust sobe o `CoreService`, spawna o servidor Python do squad de
//! verdade (`uv run python -m forge_squad.server`) apontado para o socket
//! do Core, chama `ExecuteTask` e coleta o stream de `SquadEvent`. Prova
//! o laço inteiro: o orquestrador Python chama de volta o `Generate` do
//! Rust para cada agente e streama os eventos de volta. Pulado (sem
//! falhar) se `uv`/workspace Python ausentes — como `python_sidecar.rs`.

use forge_proto::core::PermissionRequest;
use forge_proto::llm::{LlmRequest, Usage};
use forge_proto::squad::{handoff, squad_event, SquadEvent, SquadTask};
use forge_sidecar::{drain_stream, serve_core, CoreBackend, SquadRun, SquadSupervisor};
use std::path::PathBuf;
use std::time::Duration;

fn python_workspace_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../python"))
}

/// Backend do Core roteirizado por `requester` — o papel que o Gateway
/// real cumpre em produção, mas determinístico (sem tocar em nenhuma API).
struct ScriptedCore;

#[tonic::async_trait]
impl CoreBackend for ScriptedCore {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        // Consenso forte (arch 0.9 / dev 0.2 / aud 0.2) → requires_human false.
        let text = match req.requester.as_str() {
            "planner" => {
                r#"{"steps":[{"step":1,"action":"deploy","description":"publicar","estimated_time":10,"dependencies":[],"can_fail":true}],"estimated_duration":10,"confidence":0.8}"#
            }
            "architect" => {
                r#"{"problem_analysis":"x","recommendation":"micro","architecture":"microservices","components":["api"],"confidence":0.9}"#
            }
            "developer" => r#"{"final_output":"code","status":"completed","confidence":0.2}"#,
            "auditor" => {
                r#"{"passed":true,"approved":true,"confidence":0.2,"notes":"ok","issues":[],"agent_scores":{},"additional_checks":[]}"#
            }
            "designer" => r#"{"pattern":"material","components":["ui"],"confidence":0.8}"#,
            "ops" => r#"{"strategy":"blue-green","stages":["build"],"confidence":0.9}"#,
            other => return Err(format!("requester inesperado: {other}")),
        };
        Ok((
            text.to_string(),
            Usage {
                input_tokens: 1,
                output_tokens: 2,
                cache_hit: false,
                provider: "scripted".into(),
            },
        ))
    }

    async fn request_permission(&self, _req: &PermissionRequest) -> bool {
        true
    }
}

// Runtime multi-thread: o CoreService (respondendo os callbacks do Python)
// e a task que drena o stream do squad precisam progredir em paralelo — é
// também a configuração real do `forge squad`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn squad_python_real_streama_eventos_pelo_laco_bidirecional() {
    let dir = python_workspace_dir();
    if !dir.join("pyproject.toml").exists() {
        eprintln!("workspace Python ausente em {dir:?} — pulando e2e do squad");
        return;
    }

    let pid = std::process::id();
    let core_sock = std::env::temp_dir().join(format!("forge-squad-core-{pid}.sock"));
    let squad_sock = std::env::temp_dir().join(format!("forge-squad-{pid}.sock"));

    // Sobe o CoreService numa task e espera o socket existir antes de
    // spawnar o Python (evita corrida de conexão).
    let core_task = tokio::spawn(serve_core(ScriptedCore, core_sock.clone()));
    for _ in 0..100 {
        if core_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let mut supervisor =
        match SquadSupervisor::spawn(&dir, squad_sock, &core_sock, "claude-sonnet-5") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("não foi possível spawnar o squad ({e}) — pulando e2e");
                core_task.abort();
                return;
            }
        };

    let mut client = supervisor
        .wait_ready(Duration::from_secs(30))
        .await
        .expect("squad Python real deveria ficar pronto");

    let mut stream = client
        .execute_task(SquadTask {
            task_id: "t1".into(),
            description: "publicar serviço de pagamentos".into(),
            decision_type: "architecture".into(),
            max_autonomy_level: 3,
            // Sem evidência (Fase 5 Onda 3) — este teste é do laço 4d, não
            // exercita validate_results; o auditor cai em fail-closed sem
            // afetar as asserções deste teste (proposals/consensus/steps).
            verification_evidence_json: String::new(),
        })
        .await
        .expect("ExecuteTask deveria abrir o stream");

    let mut events: Vec<SquadEvent> = Vec::new();
    while let Some(ev) = stream.message().await.expect("stream de SquadEvent") {
        events.push(ev);
    }
    core_task.abort();

    assert!(!events.is_empty(), "o squad deveria ter emitido eventos");

    // Propostas dos 3 agentes que votam.
    let proposers: Vec<String> = events
        .iter()
        .filter_map(|e| match &e.payload {
            Some(squad_event::Payload::Proposal(p)) => Some(p.agent.clone()),
            _ => None,
        })
        .collect();
    assert!(
        proposers.contains(&"architect".to_string()),
        "proposers: {proposers:?}"
    );
    assert!(
        proposers.contains(&"developer".to_string()),
        "proposers: {proposers:?}"
    );
    assert!(
        proposers.contains(&"auditor".to_string()),
        "proposers: {proposers:?}"
    );

    // Consenso: requires_human preservado (false aqui) e decision_maker real.
    let consensus = events
        .iter()
        .find_map(|e| match &e.payload {
            Some(squad_event::Payload::Consensus(c)) => Some(c),
            _ => None,
        })
        .expect("deveria haver um evento de consenso");
    assert!(
        !consensus.requires_human,
        "consenso forte não deveria exigir humano"
    );
    assert_eq!(consensus.decision_maker, "architect");

    // Pelo menos um step e os handoffs start/complete.
    let has_step = events
        .iter()
        .any(|e| matches!(&e.payload, Some(squad_event::Payload::Step(_))));
    assert!(has_step, "deveria haver ao menos um StepResult");

    let phases: Vec<i32> = events
        .iter()
        .filter_map(|e| match &e.payload {
            Some(squad_event::Payload::Handoff(h)) => Some(h.phase),
            _ => None,
        })
        .collect();
    assert!(
        phases.contains(&(handoff::Phase::Start as i32)),
        "handoff phases: {phases:?}"
    );
    assert!(
        phases.contains(&(handoff::Phase::Complete as i32)),
        "handoff phases: {phases:?}"
    );
}

/// Backend do Core que demora — mantém o squad bloqueado na primeira
/// chamada `Generate` (o planner) tempo suficiente para o `kill -9`.
struct SlowCore;

#[tonic::async_trait]
impl CoreBackend for SlowCore {
    async fn generate(&self, _req: &LlmRequest) -> Result<(String, Usage), String> {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok((
            "{}".into(),
            Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_hit: false,
                provider: "slow".into(),
            },
        ))
    }
    async fn request_permission(&self, _req: &PermissionRequest) -> bool {
        true
    }
}

/// Critério de aceite da Fase 4: `kill -9` no sidecar dispara o fallback.
/// Com o squad bloqueado esperando o `Generate` (lento), matamos o processo
/// Python; a quebra do stream vira `SquadRun::Failed` — o sinal que o CLI
/// usa para degradar (squad → agente-único → safe-mode).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_do_sidecar_dispara_fallback() {
    let dir = python_workspace_dir();
    if !dir.join("pyproject.toml").exists() {
        eprintln!("workspace Python ausente em {dir:?} — pulando teste de kill/fallback");
        return;
    }

    let pid = std::process::id();
    let core_sock = std::env::temp_dir().join(format!("forge-kill-core-{pid}.sock"));
    let squad_sock = std::env::temp_dir().join(format!("forge-kill-squad-{pid}.sock"));

    let core_task = tokio::spawn(serve_core(SlowCore, core_sock.clone()));
    for _ in 0..100 {
        if core_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let mut supervisor =
        match SquadSupervisor::spawn(&dir, squad_sock, &core_sock, "claude-sonnet-5") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("não foi possível spawnar o squad ({e}) — pulando");
                core_task.abort();
                return;
            }
        };
    let mut client = supervisor
        .wait_ready(Duration::from_secs(30))
        .await
        .expect("squad deveria ficar pronto");

    let stream = client
        .execute_task(SquadTask {
            task_id: "t-kill".into(),
            description: "tarefa interrompida".into(),
            decision_type: "architecture".into(),
            max_autonomy_level: 3,
            // Sem evidência (Fase 5 Onda 3) — este teste é do laço 4d, não
            // exercita validate_results; o auditor cai em fail-closed sem
            // afetar as asserções deste teste (proposals/consensus/steps).
            verification_evidence_json: String::new(),
        })
        .await
        .expect("ExecuteTask deveria abrir o stream");

    // Squad agora está bloqueado no Generate lento do planner. kill -9.
    tokio::time::sleep(Duration::from_millis(400)).await;
    supervisor.kill().await.expect("kill deveria funcionar");

    let outcome = drain_stream(stream).await;
    core_task.abort();

    match outcome {
        SquadRun::Failed { reason, .. } => {
            assert!(!reason.is_empty(), "a falha deveria ter um motivo");
        }
        SquadRun::Completed(events) => {
            panic!(
                "esperava Failed após kill -9, veio Completed com {} eventos",
                events.len()
            );
        }
    }
}
