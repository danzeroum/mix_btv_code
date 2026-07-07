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

use forge_proto::core::{PermissionRequest, ToolCall, ToolResult};
use forge_proto::llm::{LlmRequest, Usage};
use forge_proto::squad::{handoff, squad_event, SquadEvent, SquadTask};
use forge_sidecar::{drain_stream, serve_core, CoreBackend, SquadRun, SquadSupervisor};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

    // Estes dois testes não roteirizam nenhuma ação de ferramenta (o plano
    // roteirizado do "planner" nunca gera um passo "implement" com
    // tool_call) — RunTool nunca é chamado aqui. `ScriptedCoreWithTools`
    // (Onda 3, no fechamento) é quem exercita RunTool de verdade.
    async fn run_tool(&self, _call: &ToolCall) -> ToolResult {
        ToolResult {
            content: "ScriptedCore não executa ferramentas".into(),
            truncated: false,
            exit_code: 1,
        }
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

    async fn run_tool(&self, _call: &ToolCall) -> ToolResult {
        ToolResult {
            content: "SlowCore não executa ferramentas".into(),
            truncated: false,
            exit_code: 1,
        }
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

/// Backend do Core roteirizado, com `ToolRegistry`/`PermissionEngine`
/// REAIS (Onda 1) — prova o fechamento da "tool execution architecture":
/// `forge squad "crie X.html..."` produz `X.html` de verdade no
/// workspace, registrado no ledger, com o auditor julgando sobre
/// evidência real (não texto que ele "acha" que foi produzido).
///
/// `generate` sequencia respostas por chamada (não por `requester` fixo,
/// como `ScriptedCore`) para "developer"/"auditor": o developer precisa de
/// 3 respostas em ordem (proposta → tool_call → final_answer) e o auditor
/// de 2 (proposta → validate_results, esta última só aprovada se o
/// payload que chegou até aqui carregar evidência real de tool_call —
/// falha o teste alto e claro se não carregar, em vez de aprovar calado).
struct ScriptedCoreWithTools {
    tools: Arc<forge_tools::ToolRegistry>,
    permissions: forge_core::PermissionEngine,
    root: PathBuf,
    filename: String,
    developer_call: AtomicUsize,
    auditor_call: AtomicUsize,
}

impl ScriptedCoreWithTools {
    /// Mesma lógica de `forge-cli::squad::core_run_tool` — duplicada aqui
    /// porque `forge-sidecar` (onde este teste mora) não pode depender de
    /// `forge-cli` (a direção de dependência é a oposta). Isolado o
    /// bastante para provar o contrato do lado do servidor `CoreService`,
    /// como `core_server_inprocess.rs::BackendWithTools` já faz.
    async fn run_tool_real(&self, call: &ToolCall) -> ToolResult {
        let args: serde_json::Value = match serde_json::from_str(&call.args_json) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult {
                    content: format!("args_json inválido: {e}"),
                    truncated: false,
                    exit_code: 1,
                }
            }
        };
        let Some(tool) = self.tools.get(&call.tool) else {
            return ToolResult {
                content: format!("ferramenta desconhecida: {}", call.tool),
                truncated: false,
                exit_code: 1,
            };
        };
        let scope = tool.scope(&args);
        let allowed = match self.permissions.evaluate(&call.tool, &scope) {
            forge_core::Decision::Allow => true,
            forge_core::Decision::Deny => false,
            forge_core::Decision::Ask => true, // request_permission sempre aprova neste teste
        };
        let result = if !allowed {
            ToolResult {
                content: format!("permissão negada para {} em {scope:?}", call.tool),
                truncated: false,
                exit_code: -1,
            }
        } else {
            match tool.run(&args) {
                Ok(out) => ToolResult {
                    content: out.content,
                    truncated: out.truncated,
                    exit_code: 0,
                },
                Err(e) => ToolResult {
                    content: e.to_string(),
                    truncated: false,
                    exit_code: 1,
                },
            }
        };
        self.log_tool_run(call, &scope, &result);
        result
    }

    /// Mesma forma de `session.rs::append_entry` (kind/actor/payload, sem
    /// override) — duplicado pelo mesmo motivo de `run_tool_real`.
    fn log_tool_run(&self, call: &ToolCall, scope: &str, result: &ToolResult) {
        let dir = self.root.join(".forge");
        let _ = std::fs::create_dir_all(&dir);
        let Ok(mut store) = forge_store::LedgerStore::open(dir.join("forge.db").to_str().unwrap())
        else {
            return;
        };
        let _ = store.append(forge_schemas::ledger::LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: "squad.tool_run".into(),
            actor: "forge-cli:squad-tool".into(),
            payload: serde_json::json!({
                "tool": call.tool,
                "scope": scope,
                "exit_code": result.exit_code,
                "truncated": result.truncated,
            }),
            r#override: None,
            fake_marker: None,
            ts: "2026-01-01T00:00:00Z".into(),
        });
    }
}

#[tonic::async_trait]
impl CoreBackend for ScriptedCoreWithTools {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        let text = match req.requester.as_str() {
            "planner" => format!(
                r#"{{"steps":[{{"step":1,"action":"implement","description":"criar {}","estimated_time":10,"dependencies":["seed"],"can_fail":true}}],"estimated_duration":10,"confidence":0.8}}"#,
                self.filename
            ),
            "architect" => {
                r#"{"problem_analysis":"x","recommendation":"micro","architecture":"microservices","components":["api"],"confidence":0.9}"#.to_string()
            }
            "developer" => {
                let call = self.developer_call.fetch_add(1, Ordering::SeqCst);
                match call {
                    // Proposta inicial (_get_squad_proposals — caminho antigo,
                    // sem "action", nunca usa ferramenta).
                    0 => r#"{"final_output":"vou criar o arquivo","status":"completed","confidence":0.2}"#.to_string(),
                    // 1ª iteração do loop ReAct: cria o arquivo de verdade via
                    // bash (só ele cria; edit exige que o arquivo já exista)
                    // e roda sha256sum — a evidência que o auditor vai ver.
                    1 => serde_json::json!({
                        "action": "tool_call",
                        "tool": "bash",
                        "args": {
                            "command": format!(
                                "printf '<html><body>Calculadora: soma(a,b)</body></html>' > {} && sha256sum {}",
                                self.filename, self.filename
                            )
                        },
                        "reasoning": "criar o arquivo pedido e gerar evidência de verificação"
                    })
                    .to_string(),
                    // 2ª iteração: já tem a observação da tool_call anterior
                    // (sha256sum) no histórico — encerra com final_answer.
                    _ => serde_json::json!({
                        "action": "final_answer",
                        "final_output": format!("{} criado e verificado com sha256sum", self.filename),
                        "status": "completed",
                        "confidence": 0.9,
                        "notes": "arquivo criado via bash; sha256sum confirma o conteúdo"
                    })
                    .to_string(),
                }
            }
            "auditor" => {
                let call = self.auditor_call.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    // Proposta inicial — sem prior_results, sem evidência
                    // esperada.
                    r#"{"passed":true,"approved":true,"confidence":0.2,"notes":"ok","issues":[],"agent_scores":{},"additional_checks":[]}"#.to_string()
                } else {
                    // validate_results final — só aprova se o payload que
                    // chegou até aqui (via gRPC, do lado Python) carregar
                    // evidência real de tool_call. Falha ALTO E CLARO (não
                    // aprova calado) se a evidência não estiver lá — é
                    // exatamente isto que prova que o auditor não está
                    // julgando no vácuo.
                    assert!(
                        req.messages_json.contains(&self.filename),
                        "payload do auditor não menciona o arquivo — mensagens: {}",
                        req.messages_json
                    );
                    assert!(
                        req.messages_json.contains("tool_calls"),
                        "payload do auditor não carrega tool_calls — mensagens: {}",
                        req.messages_json
                    );
                    r#"{"approved":true,"confidence":0.9,"issues":[],"agent_scores":{}}"#.to_string()
                }
            }
            other => return Err(format!("requester inesperado no fechamento: {other}")),
        };
        Ok((
            text,
            Usage {
                input_tokens: 1,
                output_tokens: 2,
                cache_hit: false,
                provider: "scripted-with-tools".into(),
            },
        ))
    }

    async fn request_permission(&self, _req: &PermissionRequest) -> bool {
        true
    }

    async fn run_tool(&self, call: &ToolCall) -> ToolResult {
        self.run_tool_real(call).await
    }
}

/// O teste de fechamento da "tool execution architecture": prova, ponta a
/// ponta e com o processo Python REAL, a definição de pronto do parecer
/// original — `forge squad "crie X.html..."` produzindo `X.html` real no
/// workspace, registrado no ledger, com o auditor julgando sobre o
/// arquivo que existe de fato (não alegando às cegas).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn squad_cria_arquivo_real_via_run_tool_ledger_e_auditor_veem_evidencia() {
    let dir = python_workspace_dir();
    if !dir.join("pyproject.toml").exists() {
        eprintln!("workspace Python ausente em {dir:?} — pulando e2e de fechamento");
        return;
    }

    let workspace_root = tempfile::tempdir().expect("tempdir do workspace");
    let filename = "scientific-calculator.html";

    let backend = ScriptedCoreWithTools {
        tools: Arc::new(forge_tools::ToolRegistry::default_set(
            workspace_root.path(),
        )),
        permissions: forge_core::PermissionEngine::default(),
        root: workspace_root.path().to_path_buf(),
        filename: filename.to_string(),
        developer_call: AtomicUsize::new(0),
        auditor_call: AtomicUsize::new(0),
    };

    let pid = std::process::id();
    let core_sock = std::env::temp_dir().join(format!("forge-squad-close-core-{pid}.sock"));
    let squad_sock = std::env::temp_dir().join(format!("forge-squad-close-{pid}.sock"));

    let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
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
                eprintln!("não foi possível spawnar o squad ({e}) — pulando e2e de fechamento");
                core_task.abort();
                return;
            }
        };
    let mut client = supervisor
        .wait_ready(Duration::from_secs(30))
        .await
        .expect("squad Python real deveria ficar pronto");

    let evidence_json = serde_json::json!({
        "run_id": "e2e-fechamento",
        "git_sha": "e2e",
        "steps": [],
        "verdict": "pass",
        "produced_at": "2026-01-01T00:00:00Z",
    })
    .to_string();

    let stream = client
        .execute_task(SquadTask {
            task_id: "t-fechamento".into(),
            description: format!("crie {filename} com uma função de soma"),
            decision_type: "architecture".into(),
            max_autonomy_level: 3,
            // Evidência PRESENTE (Fase 5 Onda 3) — ao contrário dos outros
            // dois testes deste arquivo, este PRECISA exercitar
            // validate_results() de verdade (é o veredito final que a
            // definição de pronto exige observar).
            verification_evidence_json: evidence_json,
        })
        .await
        .expect("ExecuteTask deveria abrir o stream");

    let mut events: Vec<SquadEvent> = Vec::new();
    let mut stream = stream;
    while let Some(ev) = stream.message().await.expect("stream de SquadEvent") {
        events.push(ev);
    }
    core_task.abort();

    // 1. O critério literal: o arquivo existe de verdade no workspace.
    let written = std::fs::read_to_string(workspace_root.path().join(filename))
        .unwrap_or_else(|e| panic!("{filename} deveria existir no workspace: {e}"));
    assert!(written.contains("Calculadora"), "conteúdo: {written}");

    // 2. O ledger tem a entrada da escrita.
    let ledger = forge_store::LedgerStore::open(
        workspace_root
            .path()
            .join(".forge")
            .join("forge.db")
            .to_str()
            .unwrap(),
    )
    .expect("ledger deveria abrir");
    let entries = ledger
        .recent(50, Some("forge-cli:squad-tool"))
        .expect("recent não deveria falhar");
    assert!(
        entries.iter().any(|e| e.kind == "squad.tool_run"),
        "esperava uma entrada squad.tool_run no ledger, achei: {entries:?}"
    );

    // 3. Consenso real (mesmo padrão dos outros dois testes deste arquivo).
    let consensus = events
        .iter()
        .find_map(|e| match &e.payload {
            Some(squad_event::Payload::Consensus(c)) => Some(c),
            _ => None,
        })
        .expect("deveria haver um evento de consenso");
    assert!(!consensus.requires_human);

    // 4. O veredito final é observável fora do retorno Python descartado
    // (server.py) e reflete aprovação — sobre evidência real (o assert
    // dentro de ScriptedCoreWithTools::generate já teria derrubado o
    // teste se a evidência não tivesse chegado ao auditor).
    let final_validation = events.iter().find_map(|e| match &e.payload {
        Some(squad_event::Payload::Step(s)) if s.step_id == "final_validation" => Some(s),
        _ => None,
    });
    let final_validation = final_validation.expect("deveria haver um StepResult final_validation");
    assert!(
        final_validation.success,
        "summary: {}",
        final_validation.summary
    );
    let summary: serde_json::Value =
        serde_json::from_str(&final_validation.summary).expect("summary deveria ser JSON");
    assert_eq!(summary["approved"], serde_json::json!(true));
}
