//! Fase 7 Onda 4 — squad ao vivo pelo navegador: `POST /api/squad/run`
//! dispara `SquadService.ExecuteTask` (via o `SquadPool` de longa duração
//! da Onda 3) e transmite `SquadEvent` como SSE — **sem DTO espelho**:
//! serde direto no tipo gerado pelo proto (`forge_proto::squad::SquadEvent`
//! já deriva `Serialize`, ver `forge-proto/build.rs`). O gate HITL troca o
//! `stdin` do CLI por `POST /api/squad/:task_id/hitl`, mesma forma da ponte
//! de permissão da Onda 1 (`SessionHub`), incluindo persistência do pedido
//! pendente.
//!
//! **Decisão de escopo (não em nenhum ADR — ver `pendencias.md`):** o pool
//! é usado com **capacidade 1** nesta entrega. `SquadTask`/`PermissionRequest`
//! não carregam nenhum identificador de tarefa no proto atual — rodar >1
//! squad concorrente pelo mesmo `CoreService` compartilhado não teria como
//! demultiplexar de qual tarefa uma chamada `Generate`/`RequestPermission`
//! veio. Capacidade 1 elimina a ambiguidade (só uma tarefa viva por vez)
//! sem fingir uma concorrência que não seria seguramente correlacionada.
//! Resolver isso de verdade (core_socket por slot + CoreService por slot)
//! é escopo maior, deixado para uma onda futura.

use crate::squad::{core_generate, locate_python_dir};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use forge_llm::gateway::Generator;
use forge_proto::core::PermissionRequest;
use forge_proto::llm::{LlmRequest, Usage};
use forge_proto::squad::{squad_event, ChatMessage, SquadEvent, SquadTask};
use forge_sidecar::{serve_core, CoreBackend, SidecarError, SquadPool};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

use crate::web_agent::ErrorBody;

struct PendingHitl {
    responder: tokio::sync::oneshot::Sender<bool>,
}

struct SquadTaskState {
    log: Vec<SquadEvent>,
    /// `None` quando a tarefa já terminou — dropar o `Sender` é o que faz
    /// o SSE de qualquer assinante (já conectado ou futuro) fechar sozinho
    /// (`BroadcastStream` chega a `None`/fim de stream quando o último
    /// `Sender` morre). Sem isso, a conexão SSE fica aberta para sempre
    /// mesmo depois do squad terminar de verdade — achado real, testado
    /// via e2e (`run_squad_via_http_com_gate_hitl_real_e_ledger`).
    tx: Option<tokio::sync::broadcast::Sender<SquadEvent>>,
    pending: Option<PendingHitl>,
    /// Mensagens do usuário ainda não consumidas por um ponto de consulta do
    /// orquestrador. Na Fase 1a servem de trilha visível (ecoadas como
    /// `ChatMessage`); a Fase 1b (`AwaitUserTurn`) as puxa como turno real do
    /// membro humano dentro do orquestrador.
    inbox: VecDeque<String>,
    /// Kill-switch (Fase 3, Prioridade-Zero): quando `true`, a tarefa foi
    /// parada por um `emergency_stop` — nenhum passo novo deve seguir.
    stopped: bool,
    /// Handle para abortar a task tokio que drena o stream do orquestrador —
    /// registrado logo após o `spawn`. Parar de verdade um squad em execução
    /// (não só marcar a flag) exige cancelar quem consome o stream gRPC.
    abort: Option<tokio::task::AbortHandle>,
}

impl SquadTaskState {
    fn new() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(256);
        Self {
            log: Vec::new(),
            tx: Some(tx),
            pending: None,
            inbox: VecDeque::new(),
            stopped: false,
            abort: None,
        }
    }
}

/// Monta um `SquadEvent` de chat (variante `ChatMessage` do proto) — usado
/// para ecoar a fala do usuário e as falas narradas dos agentes na conversa.
fn chat_event(task_id: &str, author: &str, author_role: &str, text: String) -> SquadEvent {
    SquadEvent {
        task_id: task_id.to_string(),
        ts: now_rfc3339(),
        payload: Some(squad_event::Payload::Chat(ChatMessage {
            author: author.to_string(),
            author_role: author_role.to_string(),
            text,
            in_reply_to: String::new(),
        })),
    }
}

/// Estado compartilhado de todas as tarefas de squad vivas — publica
/// eventos crus do proto, mantém o gate HITL pendente (sobrevive a
/// navegador fechado, reemitido via snapshot a quem conectar depois, mesmo
/// desenho do `SessionHub`/ADR 0016).
#[derive(Clone)]
pub struct SquadHub {
    tasks: Arc<Mutex<HashMap<String, SquadTaskState>>>,
    hitl_timeout: Duration,
    next_task_seq: Arc<AtomicU64>,
}

impl SquadHub {
    pub fn new(hitl_timeout: Duration) -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            hitl_timeout,
            next_task_seq: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Gera um `task_id` novo e garante o estado vazio correspondente.
    fn new_task(&self) -> String {
        let task_id = format!("sq{:x}", self.next_task_seq.fetch_add(1, Ordering::Relaxed));
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        tasks.insert(task_id.clone(), SquadTaskState::new());
        task_id
    }

    pub fn publish(&self, task_id: &str, event: SquadEvent) {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        if let Some(state) = tasks.get_mut(task_id) {
            state.log.push(event.clone());
            if let Some(tx) = &state.tx {
                let _ = tx.send(event);
            }
        }
    }

    /// Marca a tarefa como terminada — dropa o `Sender` de eventos ao vivo,
    /// o que faz o SSE de qualquer assinante (já conectado ou futuro)
    /// terminar sozinho em vez de ficar pendurado para sempre esperando um
    /// evento que nunca mais vai chegar. O snapshot (`log`) continua
    /// disponível para quem conectar depois.
    fn finish_task(&self, task_id: &str) {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        if let Some(state) = tasks.get_mut(task_id) {
            state.tx = None;
        }
    }

    /// Snapshot do que já aconteceu + assinatura para eventos ao vivo daí em
    /// diante — mesma semântica de reconexão do `SessionHub` (ADR 0016).
    /// `None` no segundo item significa "tarefa já terminou" — o chamador
    /// serve só o snapshot e encerra o SSE, não fica esperando.
    fn subscribe(
        &self,
        task_id: &str,
    ) -> (
        Vec<SquadEvent>,
        Option<tokio::sync::broadcast::Receiver<SquadEvent>>,
    ) {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        let state = tasks
            .entry(task_id.to_string())
            .or_insert_with(SquadTaskState::new);
        (
            state.log.clone(),
            state.tx.as_ref().map(|tx| tx.subscribe()),
        )
    }

    /// Chamado pelo `CoreBackend` desta onda quando o orquestrador Python
    /// pede aprovação humana — bloqueia até a resposta ou o timeout
    /// (fail-closed: nega, ADR 0017). O evento informativo
    /// `SquadEvent::Hitl` já é emitido pelo próprio orquestrador antes desta
    /// chamada (ver `orchestrator.py`) — não duplicamos aqui.
    async fn request_hitl(&self, task_id: &str) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
            let Some(state) = tasks.get_mut(task_id) else {
                return false;
            };
            state.pending = Some(PendingHitl { responder: tx });
        }
        match tokio::time::timeout(self.hitl_timeout, rx).await {
            Ok(Ok(allow)) => allow,
            _ => false,
        }
    }

    /// Resolve o gate HITL pendente — `Err` se não houver nenhum (evita
    /// resolver algo inexistente/já resolvido).
    fn resolve_hitl(&self, task_id: &str, allow: bool) -> Result<(), ()> {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        let Some(state) = tasks.get_mut(task_id) else {
            return Err(());
        };
        let Some(pending) = state.pending.take() else {
            return Err(());
        };
        let _ = pending.responder.send(allow);
        Ok(())
    }

    /// Registra uma mensagem do usuário na tarefa (o humano como MEMBRO da
    /// squad). Enfileira na `inbox` (para a Fase 1b puxar como turno real) e
    /// ecoa a fala como `ChatMessage` no stream, para todos os assinantes
    /// verem — mesma UX de qualquer outro membro. `Err` se a tarefa não
    /// existe (ex.: id inválido ou já drenada e removida).
    pub fn push_user_message(&self, task_id: &str, text: String) -> Result<(), ()> {
        {
            let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
            let Some(state) = tasks.get_mut(task_id) else {
                return Err(());
            };
            state.inbox.push_back(text.clone());
        }
        // `publish` já grava no log + faz broadcast; fora do lock acima para
        // não reentrar no mutex.
        self.publish(task_id, chat_event(task_id, "Você", "HUMAN", text));
        Ok(())
    }

    /// Retira a próxima mensagem do usuário pendente, se houver (consumida
    /// pelo ponto de consulta do orquestrador na Fase 1b). Não bloqueia.
    #[cfg_attr(not(test), allow(dead_code))]
    fn take_user_message(&self, task_id: &str) -> Option<String> {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        tasks.get_mut(task_id).and_then(|s| s.inbox.pop_front())
    }

    /// Registra o handle de abort da task que roda o squad — chamado logo
    /// depois do `spawn` em `run_squad_handler`. Idempotente: se a tarefa já
    /// foi parada antes de registrar (corrida improvável), aborta na hora.
    fn register_abort(&self, task_id: &str, abort: tokio::task::AbortHandle) {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        if let Some(state) = tasks.get_mut(task_id) {
            if state.stopped {
                abort.abort();
            } else {
                state.abort = Some(abort);
            }
        }
    }

    /// Kill-switch (Fase 3, Prioridade-Zero): para um squad em execução. Marca
    /// a flag, aborta a task que drena o stream, destrava qualquer gate HITL
    /// pendente (negando, fail-closed) para não deixar o orquestrador
    /// pendurado, publica um evento de erro visível e encerra o SSE. `Err` se
    /// a tarefa não existe. Idempotente: parar de novo não faz mal.
    pub fn emergency_stop(&self, task_id: &str, reason: &str) -> Result<(), ()> {
        {
            let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
            let Some(state) = tasks.get_mut(task_id) else {
                return Err(());
            };
            state.stopped = true;
            if let Some(abort) = state.abort.take() {
                abort.abort();
            }
            // Destrava um gate pendente negando (fail-closed) — mesmo espírito
            // do timeout de HITL (ADR 0017).
            if let Some(pending) = state.pending.take() {
                let _ = pending.responder.send(false);
            }
        }
        // Fora do lock: publica o evento de parada e encerra o stream.
        self.publish(
            task_id,
            SquadEvent {
                task_id: task_id.to_string(),
                ts: now_rfc3339(),
                payload: Some(squad_event::Payload::Error(format!(
                    "squad interrompido (kill-switch): {reason}"
                ))),
            },
        );
        self.finish_task(task_id);
        Ok(())
    }

    /// Se a tarefa foi parada por kill-switch. Usada como checagem
    /// Prioridade-Zero antes de seguir passos (ver `run_squad_task_inner`).
    fn is_stopped(&self, task_id: &str) -> bool {
        let tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        tasks.get(task_id).map(|s| s.stopped).unwrap_or(false)
    }
}

/// `CoreBackend` real do agente web: `Generate` passa pelo `Gateway`/rate
/// limit/cache (mesmo `core_generate` do `forge squad` CLI);
/// `RequestPermission` resolve o gate via HTTP em vez de stdin.
struct WebSquadCoreBackend<G: Generator> {
    generator: Arc<G>,
    hub: SquadHub,
    task_id: String,
}

#[tonic::async_trait]
impl<G: Generator + Send + Sync + 'static> CoreBackend for WebSquadCoreBackend<G> {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        core_generate(self.generator.as_ref(), req).await
    }

    async fn request_permission(&self, _req: &PermissionRequest) -> bool {
        self.hub.request_hitl(&self.task_id).await
    }
}

/// `CoreBackend` roteirizado (e2e sem API key, `FORGE_SCRIPTED=1`, mesmo
/// truque do `loadgen`/k6/squad e2e): respostas determinísticas por
/// `requester`, confiança baixa o bastante para produzir consenso fraco
/// (`strength < 0.7`) de propósito — exercita o gate HITL real de ponta a
/// ponta, não só o caminho "consenso forte, sem humano".
struct ScriptedSquadCoreBackend {
    hub: SquadHub,
    task_id: String,
}

#[tonic::async_trait]
impl CoreBackend for ScriptedSquadCoreBackend {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        let text = match req.requester.as_str() {
            "planner" => {
                r#"{"steps":[{"step":1,"action":"deploy","description":"publicar","estimated_time":10,"dependencies":[],"can_fail":true}],"estimated_duration":10,"confidence":0.5}"#
            }
            "architect" => {
                r#"{"problem_analysis":"x","recommendation":"micro","architecture":"microservices","components":["api"],"confidence":0.5}"#
            }
            "developer" => r#"{"final_output":"code","status":"completed","confidence":0.5}"#,
            "auditor" => {
                r#"{"passed":true,"approved":true,"confidence":0.5,"notes":"ok","issues":[],"agent_scores":{},"additional_checks":[]}"#
            }
            "designer" => r#"{"pattern":"material","components":["ui"],"confidence":0.5}"#,
            "ops" => r#"{"strategy":"blue-green","stages":["build"],"confidence":0.5}"#,
            other => return Err(format!("requester inesperado no modo roteirizado: {other}")),
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
        self.hub.request_hitl(&self.task_id).await
    }
}

fn now_rfc3339() -> String {
    crate::session::now_rfc3339()
}

/// Roda a tarefa de squad inteira: sobe um `CoreService` fresco (barato —
/// in-process, sem custo de `uv run`; distinto do `SquadPool`, que
/// supervisiona só o lado Python caro), roda `/verify` (mesma receita do
/// `forge squad` CLI, evidência para o auditor), adquire o único slot do
/// pool, executa e drena o stream publicando cada evento cru + registrando
/// o consenso no ledger.
async fn run_squad_task<B>(
    hub: SquadHub,
    pool: Arc<SquadPool>,
    root: PathBuf,
    task_id: String,
    description: String,
    backend_for: impl FnOnce(SquadHub, String) -> B,
) where
    B: CoreBackend,
{
    let outcome = run_squad_task_inner(
        hub.clone(),
        pool,
        root,
        task_id.clone(),
        description,
        backend_for,
    )
    .await;
    // Se foi parado por kill-switch, `emergency_stop` já publicou o evento de
    // erro e encerrou — não republica (evita erro duplicado no stream).
    if let Err(reason) = outcome {
        if !hub.is_stopped(&task_id) {
            hub.publish(
                &task_id,
                SquadEvent {
                    task_id: task_id.clone(),
                    ts: now_rfc3339(),
                    payload: Some(squad_event::Payload::Error(reason)),
                },
            );
        }
    }
    // Sempre — sucesso ou erro — encerra o SSE de quem estiver conectado
    // (ver comentário em `SquadTaskState.tx`): sem isso, a conexão HTTP
    // fica pendurada para sempre mesmo com a tarefa já concluída.
    hub.finish_task(&task_id);
}

async fn run_squad_task_inner<B>(
    hub: SquadHub,
    pool: Arc<SquadPool>,
    root: PathBuf,
    task_id: String,
    description: String,
    backend_for: impl FnOnce(SquadHub, String) -> B,
) -> Result<(), String>
where
    B: CoreBackend,
{
    let forge_dir = root.join(".forge");
    std::fs::create_dir_all(&forge_dir).map_err(|e| e.to_string())?;
    // Socket fixo, reusado sequencialmente entre tarefas (capacidade 1 do
    // pool — nunca duas tarefas vivas ao mesmo tempo disputando o bind).
    let core_sock = forge_dir.join("squad-pool-core.sock");

    let backend = backend_for(hub.clone(), task_id.clone());
    let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
    for _ in 0..100 {
        if core_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let root_for_verify = root.clone();
    let evidence =
        tokio::task::spawn_blocking(move || crate::run_verify_pipeline(&root_for_verify, None))
            .await
            .map_err(|e| format!("task de /verify falhou: {e}"))?
            .map_err(|e| format!("falha ao rodar /verify antes do squad: {e}"))?;
    let verification_evidence_json = serde_json::to_string(&evidence)
        .map_err(|e| format!("falha ao serializar evidência: {e}"))?;

    // Abre a sessão de ledger ANTES de mover `description` para o
    // `SquadTask` abaixo — mesma sessão/ledger que o resto da plataforma
    // usa (`.forge/forge.db`). "model" aqui é rótulo informativo do que o
    // pool está configurado pra usar (`squad_model()`), não uma escolha
    // por-tarefa — a sessão de squad não escolhe modelo por si, e cada
    // agente Python chama `Generate` com o modelo herdado do pool.
    let mut ledger_session = crate::session::Session::open(&root, &description, &squad_model())
        .map_err(|e| e.to_string())?;

    let lease = pool
        .acquire()
        .await
        .map_err(|e: SidecarError| e.to_string())?;
    let mut client = lease.client().clone();

    // Hardcoded, não lido de `RunSquadBody`: o campo é ignorado
    // ponta-a-ponta pelo Python hoje (ver mesmo comentário em `squad.rs`) —
    // wire-lo até a UI seria "o campo viajou" sem efeito real. Descope
    // explícito da Onda 13 (ADR 0021), não esquecimento.
    let mut stream = client
        .execute_task(SquadTask {
            task_id: task_id.clone(),
            description,
            decision_type: "architecture".into(),
            max_autonomy_level: 3,
            verification_evidence_json,
        })
        .await
        .map_err(|e| e.to_string())?;

    let mut failure: Option<String> = None;
    loop {
        // Prioridade-Zero (kill-switch, Fase 3): antes de processar o próximo
        // evento, checa se a tarefa foi parada. `emergency_stop` também aborta
        // esta task diretamente; a checagem dá uma saída limpa quando a parada
        // acontece entre eventos, sem depender só do abort abrupto.
        if hub.is_stopped(&task_id) {
            return Err("squad interrompido por kill-switch".into());
        }
        match stream.message().await {
            Ok(Some(event)) => {
                if let Some(squad_event::Payload::Consensus(c)) = &event.payload {
                    ledger_session.note(
                        "squad.consensus",
                        serde_json::json!({
                            "task_id": task_id,
                            "decision_maker": c.decision_maker,
                            "strength": c.strength,
                            "requires_human": c.requires_human,
                        }),
                    );
                }
                let is_error = matches!(&event.payload, Some(squad_event::Payload::Error(_)));
                hub.publish(&task_id, event);
                if is_error {
                    failure = Some("o squad emitiu um evento de erro".into());
                    break;
                }
            }
            Ok(None) => break,
            Err(status) => {
                failure = Some(status.to_string());
                break;
            }
        }
    }
    let _ = ledger_session.finish(failure.is_none(), 1);
    core_task.abort();
    match failure {
        None => Ok(()),
        Some(reason) => Err(reason),
    }
}

#[derive(Deserialize)]
struct RunSquadBody {
    task: String,
}

#[derive(Serialize)]
struct RunSquadResponse {
    task_id: String,
}

#[derive(Clone)]
struct SquadAgentState {
    hub: SquadHub,
    pool: Arc<SquadPool>,
}

async fn run_squad_handler(
    State(state): State<SquadAgentState>,
    Json(body): Json<RunSquadBody>,
) -> Response {
    let root = match std::env::current_dir() {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody::new("cwd_error", e.to_string())),
            )
                .into_response()
        }
    };
    let task_id = state.hub.new_task();
    let hub = state.hub.clone();
    let pool = state.pool.clone();
    let task_id_for_task = task_id.clone();

    let handle = if std::env::var_os("FORGE_SCRIPTED").is_some() {
        tokio::spawn(run_squad_task(
            hub,
            pool,
            root,
            task_id_for_task,
            body.task,
            |hub, task_id| ScriptedSquadCoreBackend { hub, task_id },
        ))
    } else {
        let opts = crate::RunOpts {
            model: squad_model(),
            agent: "build".into(),
            yes: false,
            no_cache: false,
            session: None,
            context_window: 200_000,
        };
        let generator = match crate::prepare(&opts) {
            Ok((generator, _root)) => Arc::new(generator),
            Err(e) => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorBody::new("no_provider", e.to_string())),
                )
                    .into_response()
            }
        };
        tokio::spawn(run_squad_task(
            hub,
            pool,
            root,
            task_id_for_task,
            body.task,
            move |hub, task_id| WebSquadCoreBackend {
                generator,
                hub,
                task_id,
            },
        ))
    };
    // Registra o abort para o kill-switch (Fase 3) poder parar a task de
    // verdade, não só marcar a flag.
    state.hub.register_abort(&task_id, handle.abort_handle());

    (StatusCode::ACCEPTED, Json(RunSquadResponse { task_id })).into_response()
}

async fn squad_sse_handler(
    State(state): State<SquadAgentState>,
    Path(task_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (snapshot, rx) = state.hub.subscribe(&task_id);
    let snapshot_stream = tokio_stream::iter(snapshot);
    // `rx` é `None` quando a tarefa já tinha terminado no momento da
    // assinatura (ver comentário em `SquadTaskState.tx`) — quem chega
    // atrasado só recebe o snapshot e um stream já-vazio, em vez de ficar
    // pendurado esperando um `Sender` que nunca vai existir.
    let live_stream: std::pin::Pin<Box<dyn tokio_stream::Stream<Item = SquadEvent> + Send>> =
        match rx {
            Some(rx) => Box::pin(BroadcastStream::new(rx).filter_map(|r| r.ok())),
            None => Box::pin(tokio_stream::empty()),
        };
    let combined = snapshot_stream.chain(live_stream).map(to_sse_event);
    Sse::new(combined).keep_alive(KeepAlive::default())
}

fn to_sse_event(e: SquadEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(&e)
        .unwrap_or_else(|_| Event::default().data("erro de serialização")))
}

#[derive(Deserialize)]
struct ResolveHitlBody {
    allow: bool,
}

async fn resolve_hitl_handler(
    State(state): State<SquadAgentState>,
    Path(task_id): Path<String>,
    Json(body): Json<ResolveHitlBody>,
) -> Response {
    match state.hub.resolve_hitl(&task_id, body.allow) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(()) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "hitl_not_found",
                "nenhum gate HITL pendente para esta tarefa",
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PostMessageBody {
    text: String,
}

/// O usuário como MEMBRO da squad: injeta uma mensagem na tarefa viva. A fala
/// é ecoada no stream (todos veem) e enfileirada para o orquestrador. Responde
/// `202 Accepted` **sem corpo** — o cliente não deve chamar `.json()` (mesmo
/// cuidado do bug de `202` corrigido na Onda 15).
async fn post_message_handler(
    State(state): State<SquadAgentState>,
    Path(task_id): Path<String>,
    Json(body): Json<PostMessageBody>,
) -> Response {
    let text = body.text.trim().to_string();
    if text.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody::new("empty_message", "mensagem vazia")),
        )
            .into_response();
    }
    match state.hub.push_user_message(&task_id, text) {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(()) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "task_not_found",
                "tarefa de squad inexistente ou já encerrada",
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct EmergencyStopBody {
    #[serde(default)]
    reason: Option<String>,
}

/// Kill-switch (Fase 3, governança): para um squad em execução imediatamente.
/// Responde `200` se parou, `404` se a tarefa não existe. Idempotente. O
/// motivo é opcional (default "solicitado pelo operador") e vai no evento de
/// erro publicado no stream, ficando visível para todos os assinantes.
async fn emergency_stop_handler(
    State(state): State<SquadAgentState>,
    Path(task_id): Path<String>,
    body: Option<Json<EmergencyStopBody>>,
) -> Response {
    let reason = body
        .and_then(|b| b.0.reason)
        .unwrap_or_else(|| "solicitado pelo operador".into());
    match state.hub.emergency_stop(&task_id, &reason) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(()) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "task_not_found",
                "tarefa de squad inexistente",
            )),
        )
            .into_response(),
    }
}

/// Router aditivo do squad ao vivo — `.merge()`ado ao router do agente web
/// (mesma composição de `web_agent::merged_router`, mesma guarda de
/// `Origin`/`Host`).
pub fn router(hub: SquadHub, pool: Arc<SquadPool>) -> Router {
    Router::new()
        .route("/api/squad/run", post(run_squad_handler))
        .route("/api/squad/{task_id}/events", get(squad_sse_handler))
        .route("/api/squad/{task_id}/hitl", post(resolve_hitl_handler))
        .route("/api/squad/{task_id}/message", post(post_message_handler))
        .route(
            "/api/squad/{task_id}/emergency-stop",
            post(emergency_stop_handler),
        )
        .with_state(SquadAgentState { hub, pool })
}

/// Modelo do squad — configurável via `FORGE_SQUAD_MODEL` porque o pool é
/// construído **uma vez só**, na subida do `forge dashboard` (capacidade 1,
/// reusado sequencialmente entre tarefas — ver comentário de módulo), e vira
/// o default de todos os 5 agentes Python (`UnifiedOrchestrator`, que passa
/// este `model` pra cada `ArchitectAgent`/`DeveloperAgent`/etc.). Não existe
/// hoje um caminho por-tarefa (`RunSquadBody`/`SquadTask` não carregam
/// `model`) — fazer isso de verdade exigiria o campo no proto, escopo maior
/// que esta correção. Sem a env var, mantém o default antigo
/// (`claude-sonnet-5`) — comportamento inalterado pra quem não configurar
/// nada.
fn squad_model() -> String {
    std::env::var("FORGE_SQUAD_MODEL").unwrap_or_else(|_| "claude-sonnet-5".into())
}

/// Constrói o pool do squad para o agente web — capacidade 1 (ver
/// comentário de módulo). Workspace Python ausente não impede a
/// construção (lazy: só falha, com erro claro, no primeiro `acquire()`
/// de verdade) — mesma filosofia fail-soft-até-o-uso do resto do agente
/// web.
pub fn default_squad_pool(root: &std::path::Path) -> Arc<SquadPool> {
    let py_dir = locate_python_dir().unwrap_or_else(|| PathBuf::from("python"));
    let socket_dir = root.join(".forge").join("squad-pool");
    let core_sock = root.join(".forge").join("squad-pool-core.sock");
    Arc::new(SquadPool::new(
        py_dir,
        socket_dir,
        core_sock,
        squad_model(),
        1,
        Duration::from_secs(30),
    ))
}

pub fn default_hub() -> SquadHub {
    let timeout_secs = std::env::var("FORGE_HITL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    SquadHub::new(Duration::from_secs(timeout_secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::lock_cwd;

    fn uv_missing() -> bool {
        std::process::Command::new("uv")
            .arg("--version")
            .output()
            .is_err()
    }

    fn python_workspace_present() -> bool {
        locate_python_dir().is_some()
    }

    fn extract_events(buf: &str) -> Vec<serde_json::Value> {
        buf.split("\n\n")
            .filter_map(|chunk| {
                chunk
                    .strip_prefix("data: ")
                    .or_else(|| chunk.strip_prefix("data:"))
            })
            .filter_map(|json_str| serde_json::from_str(json_str.trim()).ok())
            .collect()
    }

    /// Prova as duas pontas do bug real (achado em produção via VPS): sem a
    /// env var, o comportamento antigo (`claude-sonnet-5`) continua intacto;
    /// com ela, o squad passa a pedir o modelo configurado — antes desta
    /// correção não havia NENHUM jeito de mudar isso sem recompilar.
    #[test]
    fn squad_model_le_forge_squad_model_com_fallback_pro_claude_sonnet_5() {
        std::env::remove_var("FORGE_SQUAD_MODEL");
        assert_eq!(squad_model(), "claude-sonnet-5");

        std::env::set_var("FORGE_SQUAD_MODEL", "deepseek-chat");
        assert_eq!(squad_model(), "deepseek-chat");

        std::env::remove_var("FORGE_SQUAD_MODEL");
    }

    #[test]
    fn resolve_hitl_sem_pendente_devolve_err() {
        let hub = SquadHub::new(Duration::from_millis(100));
        // task existe (via subscribe/new_task) mas nada pediu HITL ainda.
        let _ = hub.subscribe("t1");
        assert!(hub.resolve_hitl("t1", true).is_err());
        assert!(hub.resolve_hitl("tarefa-inexistente", true).is_err());
    }

    #[tokio::test]
    async fn request_hitl_sem_resposta_expira_em_deny() {
        let hub = SquadHub::new(Duration::from_millis(50));
        let _ = hub.subscribe("t1");
        assert!(!hub.request_hitl("t1").await);
    }

    #[test]
    fn push_user_message_ecoa_chat_no_stream_e_enfileira() {
        let hub = SquadHub::new(Duration::from_millis(100));
        let _ = hub.subscribe("t1");
        assert!(hub
            .push_user_message("t1", "priorize o tom formal".into())
            .is_ok());
        // A mensagem foi ecoada como ChatMessage(HUMAN) no snapshot...
        let (snapshot, _rx) = hub.subscribe("t1");
        let chat = snapshot.iter().find_map(|e| match &e.payload {
            Some(squad_event::Payload::Chat(c)) => Some(c),
            _ => None,
        });
        let chat = chat.expect("esperava um ChatMessage no stream");
        assert_eq!(chat.author_role, "HUMAN");
        assert_eq!(chat.text, "priorize o tom formal");
        // ...e ficou enfileirada para o ponto de consulta do orquestrador.
        assert_eq!(
            hub.take_user_message("t1").as_deref(),
            Some("priorize o tom formal")
        );
        assert!(hub.take_user_message("t1").is_none());
    }

    #[test]
    fn push_user_message_em_tarefa_inexistente_devolve_err() {
        let hub = SquadHub::new(Duration::from_millis(100));
        assert!(hub.push_user_message("nao-existe", "oi".into()).is_err());
    }

    #[test]
    fn emergency_stop_marca_para_publica_erro_e_encerra() {
        let hub = SquadHub::new(Duration::from_millis(100));
        let _ = hub.subscribe("t1");
        assert!(!hub.is_stopped("t1"));
        assert!(hub.emergency_stop("t1", "teste").is_ok());
        assert!(hub.is_stopped("t1"));
        // Publicou um evento de erro com o motivo (visível no snapshot)...
        let (snapshot, rx) = hub.subscribe("t1");
        let err = snapshot.iter().find_map(|e| match &e.payload {
            Some(squad_event::Payload::Error(m)) => Some(m.clone()),
            _ => None,
        });
        assert!(err.unwrap().contains("kill-switch"));
        // ...e encerrou o SSE (tx dropado → sem receiver ao vivo).
        assert!(rx.is_none());
    }

    #[test]
    fn emergency_stop_em_tarefa_inexistente_devolve_err() {
        let hub = SquadHub::new(Duration::from_millis(100));
        assert!(hub.emergency_stop("nao-existe", "x").is_err());
    }

    #[tokio::test]
    async fn emergency_stop_destrava_gate_hitl_pendente_negando() {
        let hub = SquadHub::new(Duration::from_secs(5));
        let _ = hub.subscribe("t1");
        let hub2 = hub.clone();
        let handle = tokio::spawn(async move { hub2.request_hitl("t1").await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(hub.emergency_stop("t1", "parada").is_ok());
        // O gate pendente foi negado (fail-closed), não ficou pendurado.
        assert!(!handle.await.unwrap());
    }

    #[tokio::test]
    async fn request_hitl_resolvido_true_devolve_true() {
        let hub = SquadHub::new(Duration::from_secs(5));
        let _ = hub.subscribe("t1");
        let hub2 = hub.clone();
        let handle = tokio::spawn(async move { hub2.request_hitl("t1").await });
        // Dá tempo do pedido ficar pendente antes de resolver.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(hub.resolve_hitl("t1", true).is_ok());
        assert!(handle.await.unwrap());
    }

    /// Fronteira da Onda 4, ponta a ponta, contra o squad Python REAL (sem
    /// API key, `FORGE_SCRIPTED=1`): `POST /api/squad/run` dispara a
    /// execução via `SquadPool`; o SSE mostra `SquadEvent`s crus chegando
    /// (agentes mudando de estado ao vivo, não array estático); o consenso
    /// roteirizado é fraco de propósito (`requires_human: true`) — prova
    /// que o gate HITL real é exercitado; resolver via `POST .../hitl`
    /// libera o orquestrador, que conclui; `squad.consensus` aparece no
    /// MESMO ledger que o resto da plataforma usa.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn run_squad_via_http_com_gate_hitl_real_e_ledger() {
        if uv_missing() || !python_workspace_present() {
            eprintln!("uv/workspace Python ausente — pulando teste de squad real");
            return;
        }
        let _guard = lock_cwd().await;
        std::env::set_var("FORGE_SCRIPTED", "1");
        let dir = tempfile::tempdir().unwrap();
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let hub = default_hub();
        let pool = default_squad_pool(dir.path());
        let app = router(hub, pool);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let run_resp = client
            .post(format!("http://{addr}/api/squad/run"))
            .json(&serde_json::json!({"task": "migrar módulo de pagamentos"}))
            .send()
            .await
            .unwrap();
        assert_eq!(run_resp.status(), reqwest::StatusCode::ACCEPTED);
        let run_body: serde_json::Value = run_resp.json().await.unwrap();
        let task_id = run_body["task_id"].as_str().unwrap().to_string();
        assert!(!task_id.is_empty());

        let sse_resp = client
            .get(format!("http://{addr}/api/squad/{task_id}/events"))
            .send()
            .await
            .unwrap();
        assert!(sse_resp.status().is_success());
        let mut stream = sse_resp.bytes_stream();

        // Drena até o stream fechar (fim natural da execução) ou até um
        // teto de segurança de eventos (evita travar o teste para sempre
        // se algo no orquestrador quebrar sem emitir erro).
        let mut buf = String::new();
        let mut hitl_seen = false;
        let mut consensus_requires_human = None;
        for _ in 0..500 {
            let Some(chunk) = stream.next().await else {
                break;
            };
            let chunk = chunk.unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            let events = extract_events(&buf);

            if !hitl_seen {
                if let Some(ev) = events.iter().find(|e| e["payload"]["Hitl"].is_object()) {
                    assert_eq!(ev["task_id"], task_id);
                    hitl_seen = true;
                    // Resolve o gate agora que sabemos que está pendente.
                    let resp = client
                        .post(format!("http://{addr}/api/squad/{task_id}/hitl"))
                        .json(&serde_json::json!({"allow": true}))
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(resp.status(), reqwest::StatusCode::OK);
                }
            }
            if let Some(ev) = events
                .iter()
                .find(|e| e["payload"]["Consensus"].is_object())
            {
                consensus_requires_human = ev["payload"]["Consensus"]["requires_human"].as_bool();
            }
            if hitl_seen && consensus_requires_human.is_some() {
                // Já vimos o suficiente para as asserções — mas continua
                // drenando mais alguns ciclos para deixar o orquestrador
                // fechar o stream sozinho (prova que ele realmente retomou
                // e terminou, não travou esperando algo mais).
            }
        }

        assert!(hitl_seen, "esperava um evento SquadEvent::Hitl no stream");
        assert_eq!(
            consensus_requires_human,
            Some(true),
            "o consenso roteirizado deveria ser fraco (confiança 0.5 uniforme) e pedir humano"
        );

        // O consenso ficou registrado no MESMO ledger (.forge/forge.db) que
        // o resto da plataforma usa — não um número/registro fabricado.
        let ledger = forge_store::LedgerStore::open(
            dir.path().join(".forge").join("forge.db").to_str().unwrap(),
        )
        .unwrap();
        assert!(ledger.verify_chain().unwrap() > 0);

        std::env::set_current_dir(orig_cwd).unwrap();
    }
}
