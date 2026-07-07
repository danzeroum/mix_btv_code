//! Fase 7 Onda 1 — fundação web: DTO owned+`Serialize` espelhando
//! `forge_core::LoopEvent`, rota SSE genérica por sessão, ponte de permissão
//! HTTP (mesmo desenho do `TuiResolver`, rodando em `spawn_blocking`), guarda
//! de `Origin`/`Host` contra CSRF/DNS-rebinding, e contrato de erro
//! `{error, code}`.
//!
//! Mora em `forge-cli` (não em `forge-core`, que continua UI-agnóstico, nem
//! em `forge-server`, que continua sem depender de `forge-core`/`forge-tools`
//! — o router daqui é `.merge()`ado ao `forge_server::router()` existente).

use axum::extract::{Path, Request, State};
use axum::http::{header, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use forge_core::{
    AgentLoop, Decision, LoopEvent, PermissionEngine, PermissionResolver, Rule, BUILD, PLAN,
};
use forge_llm::gateway::Generator;
use forge_llm::scripted::ScriptedGenerator;
use forge_tools::{DiffLine, ToolRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

/// Eventos observáveis de uma sessão web — espelha `LoopEvent` (ver `From`
/// abaixo) mais os eventos que só existem no servidor: pedido de permissão
/// pendente, fim, erro. `#[serde(tag = "type")]` dá um contrato estável por
/// nome de evento (ADR 0016).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    TextDelta {
        text: String,
    },
    TurnCompleted {
        provider: String,
        input_tokens: u64,
        output_tokens: u64,
    },
    ToolStarted {
        name: String,
        scope: String,
    },
    ToolFinished {
        name: String,
        ok: bool,
        summary: String,
        diff: Option<Vec<DiffLine>>,
    },
    ToolDenied {
        name: String,
        scope: String,
    },
    /// Pedido de permissão pendente — o front resolve via
    /// `POST /api/session/:id/permission`.
    PermissionRequested {
        request_id: String,
        tool: String,
        scope: String,
    },
    /// Fim do turno — a task de `spawn_blocking` terminou sem erro.
    /// `ledger_verified` é a contagem real de `Session::verify()` (a mesma
    /// cadeia de hash que `.forge/forge.db` guarda) — nunca um número
    /// fabricado, mesmo no modo roteirizado (`FORGE_SCRIPTED=1`).
    Done {
        ledger_verified: u64,
    },
    Error {
        message: String,
    },
}

impl From<LoopEvent<'_>> for SessionEvent {
    fn from(e: LoopEvent<'_>) -> Self {
        match e {
            LoopEvent::TextDelta(s) => SessionEvent::TextDelta {
                text: s.to_string(),
            },
            LoopEvent::TurnCompleted {
                provider,
                input_tokens,
                output_tokens,
            } => SessionEvent::TurnCompleted {
                provider,
                input_tokens,
                output_tokens,
            },
            LoopEvent::ToolStarted { name, scope } => SessionEvent::ToolStarted { name, scope },
            LoopEvent::ToolFinished {
                name,
                ok,
                summary,
                diff,
            } => SessionEvent::ToolFinished {
                name,
                ok,
                summary,
                diff,
            },
            LoopEvent::ToolDenied { name, scope } => SessionEvent::ToolDenied { name, scope },
        }
    }
}

/// Contrato de erro único `{error, code}` para toda rota nova desta fase.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
    pub code: String,
}

impl ErrorBody {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            code: code.to_string(),
        }
    }
}

struct PendingPermission {
    request_id: String,
    responder: std::sync::mpsc::Sender<bool>,
}

struct SessionState {
    log: Vec<SessionEvent>,
    tx: tokio::sync::broadcast::Sender<SessionEvent>,
    pending: Option<PendingPermission>,
    /// Sessão = ator único (ADR 0018): uma mensagem por vez. Uma segunda
    /// tentativa concorrente recebe `409`, nunca corrompe o histórico.
    busy: bool,
}

impl SessionState {
    fn new() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(256);
        Self {
            log: Vec::new(),
            tx,
            pending: None,
            busy: false,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HubError {
    #[error("limite de sessões simultâneas atingido ({0})")]
    TooManySessions(usize),
}

/// Estado compartilhado de todas as sessões web vivas — publica eventos,
/// mantém o pedido de permissão pendente (sobrevive a navegador fechado,
/// reemitido via snapshot a quem conectar depois — ADR 0016), e aplica o
/// teto de sessões vivas simultâneas (ADR 0020). `Clone` barato (`Arc` por
/// dentro).
#[derive(Clone)]
pub struct SessionHub {
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    max_sessions: usize,
    permission_timeout: Duration,
    next_request_id: Arc<AtomicU64>,
}

impl SessionHub {
    pub fn new(max_sessions: usize, permission_timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            max_sessions,
            permission_timeout,
            next_request_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Garante que a sessão existe (cria na primeira vez); aplica o teto de
    /// sessões simultâneas — `Err` vira `429` no handler chamador.
    pub fn ensure_session(&self, session_id: &str) -> Result<(), HubError> {
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        if sessions.contains_key(session_id) {
            return Ok(());
        }
        if sessions.len() >= self.max_sessions {
            return Err(HubError::TooManySessions(self.max_sessions));
        }
        sessions.insert(session_id.to_string(), SessionState::new());
        Ok(())
    }

    pub fn publish(&self, session_id: &str, event: SessionEvent) {
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        if let Some(state) = sessions.get_mut(session_id) {
            state.log.push(event.clone());
            let _ = state.tx.send(event);
        }
    }

    /// Snapshot do que já aconteceu (inclui pedido de permissão pendente, se
    /// houver — via o próprio log) + assinatura para eventos ao vivo daí em
    /// diante. Reconectar depois que um pedido já existe ainda o vê (prova a
    /// semântica de reconexão do ADR 0016 — não é só o caminho feliz).
    fn subscribe(
        &self,
        session_id: &str,
    ) -> (
        Vec<SessionEvent>,
        tokio::sync::broadcast::Receiver<SessionEvent>,
    ) {
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        let state = sessions
            .entry(session_id.to_string())
            .or_insert_with(SessionState::new);
        (state.log.clone(), state.tx.subscribe())
    }

    /// Chamado pelo `WebPermissionResolver` — publica o pedido, bloqueia até
    /// a resposta ou o timeout (fail-closed: `Deny` ao expirar, ADR 0017).
    /// Só é seguro porque o chamador SEMPRE roda em `spawn_blocking` (nunca
    /// `tokio::spawn` comum) — decisão explícita da Onda 1 (ADR 0020).
    fn request_permission(&self, session_id: &str, tool: &str, scope: &str) -> bool {
        let request_id = format!(
            "perm-{}",
            self.next_request_id.fetch_add(1, Ordering::Relaxed)
        );
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        {
            let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
            let Some(state) = sessions.get_mut(session_id) else {
                return false;
            };
            state.pending = Some(PendingPermission {
                request_id: request_id.clone(),
                responder: tx,
            });
        }
        self.publish(
            session_id,
            SessionEvent::PermissionRequested {
                request_id,
                tool: tool.to_string(),
                scope: scope.to_string(),
            },
        );
        let allowed = rx.recv_timeout(self.permission_timeout).unwrap_or(false);
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        if let Some(state) = sessions.get_mut(session_id) {
            state.pending = None;
        }
        allowed
    }

    /// Marca a sessão como ocupada — `Err` se já houver uma mensagem em
    /// processamento (a segunda tentativa concorrente vira `409`, não
    /// corrompe o histórico, ADR 0018).
    pub fn try_start(&self, session_id: &str) -> Result<(), ()> {
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        let Some(state) = sessions.get_mut(session_id) else {
            return Err(());
        };
        if state.busy {
            return Err(());
        }
        state.busy = true;
        Ok(())
    }

    /// Libera a sessão para a próxima mensagem — chamado sempre ao fim da
    /// tarefa (sucesso ou erro).
    pub fn finish_busy(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        if let Some(state) = sessions.get_mut(session_id) {
            state.busy = false;
        }
    }

    /// Resolve um pedido pendente — `Err` se não houver pedido pendente ou o
    /// `request_id` não bater (evita resolver o pedido errado numa corrida
    /// entre um timeout e uma resposta tardia).
    pub fn resolve_permission(
        &self,
        session_id: &str,
        request_id: &str,
        allow: bool,
    ) -> Result<(), ()> {
        let mut sessions = self.sessions.lock().expect("session hub mutex poisoned");
        let Some(state) = sessions.get_mut(session_id) else {
            return Err(());
        };
        let matches = state
            .pending
            .as_ref()
            .map(|p| p.request_id == request_id)
            .unwrap_or(false);
        if !matches {
            return Err(());
        }
        let pending = state.pending.take().expect("checked above");
        let _ = pending.responder.send(allow);
        Ok(())
    }
}

/// Ponte permissão↔HTTP: mesmo desenho do `TuiResolver` (publica o pedido,
/// bloqueia até responderem pelo canal pareado) — aqui o canal é o
/// `SessionHub` em vez de um `mpsc` direto, e quem "responde" é um handler
/// HTTP em vez de um modal de terminal.
pub struct WebPermissionResolver {
    pub hub: SessionHub,
    pub session_id: String,
}

impl PermissionResolver for WebPermissionResolver {
    fn resolve(&mut self, tool: &str, scope: &str) -> bool {
        self.hub.request_permission(&self.session_id, tool, scope)
    }
}

/// Parâmetros de uma tarefa agrupados à parte (não achatados nos argumentos
/// de `spawn_session_task`) só para não estourar o teto de aridade do
/// clippy — sem significado próprio além disso.
pub struct SessionTaskSpec {
    pub tools: ToolRegistry,
    pub permissions: PermissionEngine,
    pub model: String,
    pub system: String,
    pub task: String,
    /// Raiz do workspace — abre o MESMO ledger (`.forge/forge.db`) que o CLI
    /// usa, para que `Done.ledger_verified` seja uma contagem real, mesmo no
    /// modo roteirizado.
    pub root: std::path::PathBuf,
}

/// Publica `Done` com a contagem real do ledger (`Session::verify()`) e
/// libera a sessão para a próxima mensagem — chamado só no caminho de
/// sucesso; erro é publicado inline no chamador (que tem o contexto do `e`).
fn finish_task_ok(
    hub: &SessionHub,
    session_id: &str,
    ledger_session: &mut crate::session::Session,
    steps: usize,
) {
    let _ = ledger_session.finish(true, steps);
    let verified = ledger_session.verify().unwrap_or(0);
    hub.publish(
        session_id,
        SessionEvent::Done {
            ledger_verified: verified,
        },
    );
    hub.finish_busy(session_id);
}

/// Roda uma tarefa do agent loop numa sessão, publicando `SessionEvent`s no
/// hub e resolvendo `Ask` via `WebPermissionResolver`. SEMPRE em
/// `spawn_blocking` (nunca `tokio::spawn` comum, ADR 0020) — o resolver
/// bloqueia a thread até a UI responder ou o timeout expirar; um
/// `tokio::spawn` comum esgotaria uma worker-thread do reactor sob N sessões.
/// Grava no MESMO ledger que `forge run` usa — inclusive no modo
/// roteirizado, para que a fronteira "ledger íntegro: N" nunca seja de
/// fachada.
pub fn spawn_session_task<G>(
    hub: SessionHub,
    session_id: String,
    generator: Arc<G>,
    spec: SessionTaskSpec,
) where
    G: Generator + Send + Sync + 'static,
{
    let SessionTaskSpec {
        tools,
        permissions,
        model,
        system,
        task,
        root,
    } = spec;
    tokio::task::spawn_blocking(move || {
        let rt_handle = tokio::runtime::Handle::current();
        let outcome: anyhow::Result<(usize, crate::session::Session)> = (|| {
            let mut ledger_session = crate::session::Session::open(&root, &task, &model)?;
            let agent_loop = AgentLoop {
                generator: generator.as_ref(),
                tools: &tools,
                permissions,
                model,
                system,
                max_steps: 30,
                max_tokens: 4096,
            };
            let mut resolver = WebPermissionResolver {
                hub: hub.clone(),
                session_id: session_id.clone(),
            };
            let hub_events = hub.clone();
            let session_id_events = session_id.clone();
            // Sem `move`: `ledger_session` é só emprestada (capturada por
            // `&mut`) — precisamos dela de volta depois do `run` para
            // `finish`/`verify` (ver comentário no tipo de retorno acima).
            let mut on_event = |e: LoopEvent| {
                ledger_session.record(&e);
                hub_events.publish(&session_id_events, SessionEvent::from(e));
            };
            let result = rt_handle.block_on(agent_loop.run(&task, &mut resolver, &mut on_event));
            Ok((result?.steps, ledger_session))
        })();
        match outcome {
            Ok((steps, mut ledger_session)) => {
                finish_task_ok(&hub, &session_id, &mut ledger_session, steps)
            }
            Err(e) => {
                // Falha antes/durante a abertura do ledger — sem sessão para
                // registrar `finish`, mas ainda libera a sessão e avisa.
                hub.publish(
                    &session_id,
                    SessionEvent::Error {
                        message: e.to_string(),
                    },
                );
                hub.finish_busy(&session_id);
            }
        }
    });
}

/// Onda 2: replica a receita `prepare`/`build_loop`/`open_durable`/
/// `continue_run`/`persist_new` do CLI (`forge run`) para uma mensagem web —
/// mesma dupla persistência (ledger + `DurableSession`), mesmo `on_event`
/// registrando os dois. SEMPRE em `spawn_blocking` (ver `spawn_session_task`).
fn spawn_message_task(
    hub: SessionHub,
    session_id: String,
    generator: Arc<crate::CliGenerator>,
    opts: crate::RunOpts,
    root: std::path::PathBuf,
    message: String,
    overrides: Vec<Rule>,
) {
    tokio::task::spawn_blocking(move || {
        let rt_handle = tokio::runtime::Handle::current();
        let outcome: anyhow::Result<(usize, crate::session::Session)> = (|| {
            let tools = crate::skills::build_registry(&root);
            let mut agent_loop = crate::build_loop(generator.as_ref(), &opts, &tools)?;
            // Onda 2: as regras persistidas (matriz build/plan×tool + "sempre"
            // da ponte de permissão) sempre vencem o default do perfil —
            // nunca o contrário, senão afrouxar a matriz na UI não teria
            // efeito nenhum na sessão real.
            agent_loop.permissions = agent_loop.permissions.overlay(&overrides);
            let mut ledger_session = crate::session::Session::open(&root, &message, &opts.model)?;
            let mut durable = crate::open_durable(&root, &opts, &message)?;

            rt_handle.block_on(crate::maybe_compact(
                generator.as_ref(),
                &opts,
                &mut durable,
                &mut ledger_session,
                false,
            ))?;

            durable
                .messages
                .push(forge_llm::chat::ChatMessage::user_text(&message));
            let mut resolver = WebPermissionResolver {
                hub: hub.clone(),
                session_id: session_id.clone(),
            };
            let hub_events = hub.clone();
            let session_id_events = session_id.clone();
            // Sem `move` — mesma razão do `spawn_session_task`.
            let mut on_event = |event: LoopEvent| {
                ledger_session.record(&event);
                hub_events.publish(&session_id_events, SessionEvent::from(event));
            };
            let result = rt_handle.block_on(agent_loop.continue_run(
                &mut durable.messages,
                &mut resolver,
                &mut on_event,
            ));
            let _persisted = durable.persist_new().unwrap_or(0);
            Ok((result?.steps, ledger_session))
        })();
        match outcome {
            Ok((steps, mut ledger_session)) => {
                finish_task_ok(&hub, &session_id, &mut ledger_session, steps)
            }
            Err(e) => {
                hub.publish(
                    &session_id,
                    SessionEvent::Error {
                        message: e.to_string(),
                    },
                );
                hub.finish_busy(&session_id);
            }
        }
    });
}

#[derive(Deserialize)]
struct SendMessageBody {
    message: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
}

async fn send_message_handler(
    State(state): State<WebAgentState>,
    Path(session_id): Path<String>,
    Json(body): Json<SendMessageBody>,
) -> Response {
    if let Err(HubError::TooManySessions(max)) = state.hub.ensure_session(&session_id) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorBody::new(
                "too_many_sessions",
                format!("limite de {max} sessões simultâneas atingido"),
            )),
        )
            .into_response();
    }
    if state.hub.try_start(&session_id).is_err() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorBody::new(
                "session_busy",
                "sessão já está processando uma mensagem — aguarde terminar",
            )),
        )
            .into_response();
    }

    let opts = crate::RunOpts {
        model: body.model.unwrap_or_else(|| "claude-sonnet-5".into()),
        agent: body.agent.unwrap_or_else(|| "build".into()),
        yes: false,
        no_cache: false,
        session: Some(session_id.clone()),
        context_window: 200_000,
    };
    let root = match std::env::current_dir() {
        Ok(r) => r,
        Err(e) => {
            state.hub.finish_busy(&session_id);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody::new("cwd_error", e.to_string())),
            )
                .into_response();
        }
    };
    // Overrides persistidos (matriz + "sempre") do perfil desta mensagem —
    // sempre carregado, mesmo no modo roteirizado, para que a fronteira
    // "override afeta sessão real" nunca seja de fachada.
    let overrides = load_rule_overrides(&root, &opts.agent);

    // Modo roteirizado (e2e sem API key, mesmo truque do `loadgen`/k6/squad
    // e2e): pede um `bash` real (exercita a ponte de permissão de verdade) e
    // encerra — deliberadamente ignora o texto da mensagem para o roteiro,
    // só o ecoa dentro do comando.
    if std::env::var_os("FORGE_SCRIPTED").is_some() {
        let tools = crate::skills::build_registry(&root);
        let generator = Arc::new(ScriptedGenerator::from_sequence(scripted_turns_for(
            &body.message,
        )));
        spawn_session_task(
            state.hub.clone(),
            session_id,
            generator,
            SessionTaskSpec {
                tools,
                permissions: PermissionEngine::default().overlay(&overrides),
                model: opts.model.clone(),
                system: "modo roteirizado (FORGE_SCRIPTED=1) — sem provider real".into(),
                task: body.message,
                root,
            },
        );
        return StatusCode::ACCEPTED.into_response();
    }

    let generator = match crate::prepare(&opts) {
        Ok((generator, _root)) => generator,
        Err(e) => {
            state.hub.finish_busy(&session_id);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorBody::new("no_provider", e.to_string())),
            )
                .into_response();
        }
    };
    spawn_message_task(
        state.hub.clone(),
        session_id,
        Arc::new(generator),
        opts,
        root,
        body.message,
        overrides,
    );
    StatusCode::ACCEPTED.into_response()
}

/// Caminho do storage de regras persistidas — mesma raiz do workspace que o
/// ledger (`.forge/`), arquivo próprio (`rules.db`).
fn rules_db_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".forge").join("rules.db")
}

fn open_rule_store(root: &std::path::Path) -> anyhow::Result<forge_store::RuleStore> {
    let path = rules_db_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(forge_store::RuleStore::open(
        path.to_str().unwrap_or(".forge/rules.db"),
    )?)
}

fn rule_record_to_core(record: forge_store::RuleRecord) -> Rule {
    Rule {
        tool: record.tool,
        scope_prefix: record.scope_prefix,
        decision: match record.decision {
            forge_store::RuleDecision::Allow => Decision::Allow,
            forge_store::RuleDecision::Ask => Decision::Ask,
            forge_store::RuleDecision::Deny => Decision::Deny,
        },
    }
}

/// Carrega os overrides persistidos de um perfil — fail-open (vazio) se o
/// storage não puder ser aberto, mesmo padrão de outros stores opcionais
/// (telemetria): uma `rules.db` corrompida nunca deve impedir uma sessão de
/// rodar, só faz o perfil se comportar como se não tivesse override nenhum.
/// `pub(crate)`: reusado pelo console MCP (Onda 7) para o preview de
/// política real — os perfis const não têm regra `mcp__*`, então sem isso o
/// preview seria sempre "ask".
pub(crate) fn load_rule_overrides(root: &std::path::Path, profile: &str) -> Vec<Rule> {
    let Ok(store) = open_rule_store(root) else {
        return Vec::new();
    };
    store
        .list_for_profile(profile)
        .map(|records| records.into_iter().map(rule_record_to_core).collect())
        .unwrap_or_default()
}

fn decision_from_wire(s: &str) -> Option<forge_store::RuleDecision> {
    match s {
        "allow" => Some(forge_store::RuleDecision::Allow),
        "ask" => Some(forge_store::RuleDecision::Ask),
        "deny" => Some(forge_store::RuleDecision::Deny),
        _ => None,
    }
}

#[derive(Deserialize)]
struct SetRuleBody {
    profile: String,
    tool: String,
    #[serde(default)]
    scope_prefix: Option<String>,
    decision: String,
}

/// `POST /api/permissions/rules` — grava um override (matriz ou "sempre") e
/// deixa rastro no ledger (ADR: mutação de política de permissão nunca passa
/// em silêncio). Mutação mais sensível deste plano — por isso sempre
/// auditada, nunca um clique único e opaco.
async fn set_rule_handler(Json(body): Json<SetRuleBody>) -> Response {
    let Some(decision) = decision_from_wire(&body.decision) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new(
                "invalid_decision",
                format!("decisão inválida: {} (use allow/ask/deny)", body.decision),
            )),
        )
            .into_response();
    };
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
    let result: anyhow::Result<forge_store::RuleRecord> = (|| {
        let mut store = open_rule_store(&root)?;
        let record = store.set(
            &body.profile,
            &body.tool,
            body.scope_prefix.as_deref(),
            decision,
            &crate::session::now_rfc3339(),
        )?;
        crate::session::append_override_entry(
            &root,
            "web:permissions",
            "permission_rule.set",
            serde_json::json!({
                "profile": record.profile,
                "tool": record.tool,
                "scope_prefix": record.scope_prefix,
                "decision": body.decision,
            }),
        )?;
        Ok(record)
    })();
    match result {
        Ok(record) => Json(record).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("storage_error", e.to_string())),
        )
            .into_response(),
    }
}

/// `GET /api/permissions/rules` — lista todos os overrides ativos, para a UI
/// mostrar (e permitir revogar) o que está persistido.
async fn list_rules_handler() -> Response {
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
    match open_rule_store(&root).and_then(|s| s.list_all().map_err(Into::into)) {
        Ok(rules) => Json(rules).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("storage_error", e.to_string())),
        )
            .into_response(),
    }
}

/// `DELETE /api/permissions/rules/:id` — revoga um override; a partir daí o
/// perfil volta a valer sem ele. Também auditado no ledger.
async fn revoke_rule_handler(Path(id): Path<i64>) -> Response {
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
    let result: anyhow::Result<bool> = (|| {
        let mut store = open_rule_store(&root)?;
        let existing = store.get(id)?;
        let removed = store.remove(id)?;
        if removed {
            if let Some(rec) = existing {
                crate::session::append_override_entry(
                    &root,
                    "web:permissions",
                    "permission_rule.revoke",
                    serde_json::json!({
                        "id": rec.id,
                        "profile": rec.profile,
                        "tool": rec.tool,
                        "scope_prefix": rec.scope_prefix,
                    }),
                )?;
            }
        }
        Ok(removed)
    })();
    match result {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("rule_not_found", "regra não encontrada")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("storage_error", e.to_string())),
        )
            .into_response(),
    }
}

/// Tools cobertas pela matriz build/plan — mesmo conjunto que
/// `web/src/api/session.ts`'s `TOOL_POLICIES` já mostrava como mock.
const MATRIX_TOOLS: [&str; 5] = ["read", "grep", "edit", "bash", "webfetch"];

#[derive(Serialize)]
struct MatrixRow {
    tool: String,
    build: Decision,
    plan: Decision,
}

/// `GET /api/permissions/matrix` — decisão EFETIVA (default do perfil +
/// overrides persistidos) para cada tool × {build,plan}; fonte única de
/// verdade é `forge_core::{BUILD,PLAN}`, nunca uma cópia fabricada em TS.
async fn get_matrix_handler() -> Response {
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
    let build_engine = (BUILD.permissions)().overlay(&load_rule_overrides(&root, "build"));
    let plan_engine = (PLAN.permissions)().overlay(&load_rule_overrides(&root, "plan"));
    let rows: Vec<MatrixRow> = MATRIX_TOOLS
        .iter()
        .map(|tool| MatrixRow {
            tool: tool.to_string(),
            build: build_engine.evaluate(tool, ""),
            plan: plan_engine.evaluate(tool, ""),
        })
        .collect();
    Json(rows).into_response()
}

fn scripted_turns_for(message: &str) -> Vec<forge_llm::chat::AssistantTurn> {
    use forge_llm::chat::{AssistantTurn, ContentBlock, StopReason, Usage};
    vec![
        AssistantTurn {
            content: vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": format!("echo {message:?}")}),
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        },
        AssistantTurn {
            content: vec![ContentBlock::Text {
                text: "pronto".into(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        },
    ]
}

#[derive(Clone)]
struct WebAgentState {
    hub: SessionHub,
}

/// Router aditivo do agente web (Ondas 1-2) — pensado para ser `.merge()`ado
/// ao `forge_server::router()` existente. Ver `merged_router` para a
/// composição completa (com a guarda de `Origin`/`Host`).
pub fn router(hub: SessionHub) -> Router {
    Router::new()
        .route("/api/session/{id}/events", get(sse_handler))
        .route("/api/session/{id}/message", post(send_message_handler))
        .route(
            "/api/session/{id}/permission",
            post(resolve_permission_handler),
        )
        .route("/api/permissions/matrix", get(get_matrix_handler))
        .route(
            "/api/permissions/rules",
            get(list_rules_handler).post(set_rule_handler),
        )
        .route(
            "/api/permissions/rules/{id}",
            axum::routing::delete(revoke_rule_handler),
        )
        .with_state(WebAgentState { hub })
}

/// Compõe o router do agente web com o `forge_server::router()` existente,
/// um router aditivo (`extra` — squad (Onda 4) / prompt-render (Onda 5) /
/// o que mais precisar de `forge-sidecar`/`forge-tools`/`forge-core`,
/// indisponíveis em `forge-server`) e a guarda de `Origin`/`Host` —
/// `forge-server` continua sem ganhar dependência nenhuma dos três.
pub fn merged_router(hub: SessionHub, dashboard: Router, extra: Router) -> Router {
    dashboard
        .merge(router(hub))
        .merge(extra)
        .layer(middleware::from_fn(require_local_origin))
}

/// Sobe o dashboard com o agente web habilitado — padrão desde a Onda 15
/// (fecho); `--no-web-agent` volta ao dashboard só-leitura. Mesma
/// SPA/telemetria do dashboard padrão, mais as rotas desta onda e as de
/// `extra` (squad, Onda 4; prompt-render, Onda 5) por trás da guarda de
/// `Origin`/`Host`. `forge-server` em si segue intocado (zero dependência
/// nova) — a composição mora aqui.
// 8 argumentos = os handles/config que `main.rs` já mantém abertos (um por
// storage) + o que a composição de routers pede — função só encaminha, não
// teria o que uma struct de agrupamento ganhasse em clareza.
#[allow(clippy::too_many_arguments)]
pub async fn serve_with_agent(
    telemetry: forge_store::Telemetry,
    prompt_library: std::sync::Arc<std::sync::Mutex<forge_store::PromptLibrary>>,
    ledger: std::sync::Arc<std::sync::Mutex<forge_store::LedgerStore>>,
    root: impl AsRef<std::path::Path>,
    addr: std::net::SocketAddr,
    web_dir: impl AsRef<std::path::Path>,
    hub: SessionHub,
    extra: Router,
) -> std::io::Result<()> {
    let dashboard = forge_server::router(telemetry, prompt_library, ledger, root, web_dir);
    let app = merged_router(hub, dashboard, extra);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

/// Teto default de sessões vivas simultâneas e prazo default de permissão
/// pendente sem resposta (ADR 0017/0020) — configuráveis via env var para
/// quem precisar de outro valor; testes usam `SessionHub::new` direto com
/// prazos encurtados.
pub fn default_hub() -> SessionHub {
    let max_sessions = std::env::var("FORGE_MAX_SESSIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let timeout_secs = std::env::var("FORGE_PERMISSION_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    SessionHub::new(max_sessions, Duration::from_secs(timeout_secs))
}

async fn sse_handler(
    State(state): State<WebAgentState>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let (snapshot, rx) = state.hub.subscribe(&session_id);
    let snapshot_stream = tokio_stream::iter(snapshot);
    let live_stream = BroadcastStream::new(rx).filter_map(|r| r.ok());
    let combined = snapshot_stream.chain(live_stream).map(to_sse_event);
    Sse::new(combined).keep_alive(KeepAlive::default())
}

fn to_sse_event(e: SessionEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(&e)
        .unwrap_or_else(|_| Event::default().data("erro de serialização")))
}

#[derive(Deserialize)]
struct ResolvePermissionBody {
    request_id: String,
    allow: bool,
}

async fn resolve_permission_handler(
    State(state): State<WebAgentState>,
    Path(session_id): Path<String>,
    Json(body): Json<ResolvePermissionBody>,
) -> Response {
    match state
        .hub
        .resolve_permission(&session_id, &body.request_id, body.allow)
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(()) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "permission_not_found",
                "pedido de permissão não encontrado ou expirado",
            )),
        )
            .into_response(),
    }
}

/// Guarda de CSRF/DNS-rebinding (ADR 0015): valida `Origin` contra localhost
/// em todo método ≠ `GET`. Sem `Origin` (curl/CLI) passa — só o navegador
/// manda esse header em requisição cross-origin; é a rota que literalmente
/// aprova execução de `bash`, então qualquer site aberto no mesmo navegador
/// não pode alcançá-la.
pub async fn require_local_origin(req: Request, next: Next) -> Response {
    if req.method() != Method::GET {
        if let Some(origin) = req.headers().get(header::ORIGIN) {
            let origin_str = origin.to_str().unwrap_or("");
            if !is_local_origin(origin_str) {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ErrorBody::new("forbidden_origin", "origin não permitida")),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}

fn is_local_origin(origin: &str) -> bool {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"));
    let Some(rest) = rest else {
        return false;
    };
    let host_port = rest.split('/').next().unwrap_or("");
    let host = host_port
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(host_port);
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `std::env::current_dir`/`set_current_dir` são estado global do
    /// processo — testes que trocam o diretório atual (para controlar onde
    /// `send_message_handler`/os handlers de `/api/permissions/*` procuram
    /// `.forge/`) precisam rodar mutuamente exclusivos entre si, incluindo
    /// contra os testes de `squad_agent` (mesmo binário de teste) — por
    /// isso o lock é compartilhado via `crate::test_support`, não local.
    use crate::test_support::lock_cwd;

    #[test]
    fn origin_localhost_variantes_sao_aceitas() {
        assert!(is_local_origin("http://127.0.0.1:7878"));
        assert!(is_local_origin("http://localhost:5173"));
        assert!(is_local_origin("https://127.0.0.1"));
        assert!(is_local_origin("http://[::1]:7878"));
    }

    #[test]
    fn origin_externa_e_rejeitada() {
        assert!(!is_local_origin("https://evil.example"));
        // Ataque de sufixo: não pode bastar "127.0.0.1" aparecer na string.
        assert!(!is_local_origin("http://127.0.0.1.evil.example"));
        assert!(!is_local_origin("http://evil.example/?u=127.0.0.1"));
        assert!(!is_local_origin(""));
    }

    #[test]
    fn hub_publica_e_snapshot_replays_para_quem_conecta_depois() {
        let hub = SessionHub::new(8, Duration::from_millis(200));
        hub.ensure_session("s1").unwrap();
        hub.publish(
            "s1",
            SessionEvent::TextDelta {
                text: "olá".into()
            },
        );
        // Conecta DEPOIS do evento já ter sido publicado — snapshot-then-live.
        let (snapshot, _rx) = hub.subscribe("s1");
        assert_eq!(snapshot.len(), 1);
        matches!(snapshot[0], SessionEvent::TextDelta { .. });
    }

    #[test]
    fn permissao_pendente_sobrevive_e_e_vista_por_quem_conecta_depois() {
        let hub = SessionHub::new(8, Duration::from_millis(500));
        hub.ensure_session("s1").unwrap();
        let hub2 = hub.clone();
        let handle = std::thread::spawn(move || hub2.request_permission("s1", "bash", "$ ls"));
        // Dá tempo do pedido ser publicado antes de "conectar".
        std::thread::sleep(Duration::from_millis(50));
        let (snapshot, _rx) = hub.subscribe("s1");
        assert!(snapshot.iter().any(
            |e| matches!(e, SessionEvent::PermissionRequested { tool, .. } if tool == "bash")
        ));
        // Resolve com o request_id certo, extraído do snapshot.
        let request_id = snapshot
            .iter()
            .find_map(|e| match e {
                SessionEvent::PermissionRequested { request_id, .. } => Some(request_id.clone()),
                _ => None,
            })
            .unwrap();
        hub.resolve_permission("s1", &request_id, true).unwrap();
        assert!(handle.join().unwrap());
    }

    #[test]
    fn permissao_sem_resposta_expira_em_deny() {
        let hub = SessionHub::new(8, Duration::from_millis(50));
        hub.ensure_session("s1").unwrap();
        // Ninguém chama resolve_permission — deve expirar em `false` (Deny).
        assert!(!hub.request_permission("s1", "bash", "$ rm -rf /"));
    }

    #[test]
    fn resolver_com_request_id_errado_nao_afeta_o_pedido_pendente() {
        let hub = SessionHub::new(8, Duration::from_millis(500));
        hub.ensure_session("s1").unwrap();
        let hub2 = hub.clone();
        let handle = std::thread::spawn(move || hub2.request_permission("s1", "bash", "$ ls"));
        std::thread::sleep(Duration::from_millis(30));
        assert!(hub
            .resolve_permission("s1", "perm-nao-existe", true)
            .is_err());
        // O pedido real ainda está pendente e ainda pode ser resolvido.
        let (snapshot, _rx) = hub.subscribe("s1");
        let request_id = snapshot
            .iter()
            .find_map(|e| match e {
                SessionEvent::PermissionRequested { request_id, .. } => Some(request_id.clone()),
                _ => None,
            })
            .unwrap();
        hub.resolve_permission("s1", &request_id, false).unwrap();
        assert!(!handle.join().unwrap());
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

    /// Fronteira da Onda 1, ponta a ponta: servidor axum real em porta
    /// efêmera, generator sequenciado (`tool_use` de `bash` → `Ask` → depois
    /// `end_turn`), cliente HTTP real (reqwest + `bytes_stream`) recebe o SSE
    /// e vê o pedido de permissão pendente, um `POST` real resolve, e o
    /// stream termina em `done` com o resultado real da ferramenta.
    #[tokio::test(flavor = "multi_thread")]
    async fn fluxo_completo_sse_mais_permissao_via_http_real() {
        use forge_llm::chat::{AssistantTurn, ContentBlock, StopReason, Usage};
        use forge_llm::scripted::ScriptedGenerator;

        let dir = tempfile::tempdir().unwrap();
        let tools = ToolRegistry::default_set(dir.path());
        let turn1 = AssistantTurn {
            content: vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "echo oi"}),
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        };
        let turn2 = AssistantTurn {
            content: vec![ContentBlock::Text {
                text: "pronto".into(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        };
        let generator = Arc::new(ScriptedGenerator::from_sequence(vec![turn1, turn2]));

        let hub = SessionHub::new(8, Duration::from_secs(5));
        hub.ensure_session("s1").unwrap();
        let app = router(hub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let sse_resp = client
            .get(format!("http://{addr}/api/session/s1/events"))
            .send()
            .await
            .unwrap();
        assert!(sse_resp.status().is_success());

        spawn_session_task(
            hub.clone(),
            "s1".into(),
            generator,
            SessionTaskSpec {
                tools,
                permissions: PermissionEngine::default(),
                model: "scripted-model".into(),
                system: "sistema de teste".into(),
                task: "faça algo".into(),
                root: dir.path().to_path_buf(),
            },
        );

        let mut stream = sse_resp.bytes_stream();
        let mut buf = String::new();
        let request_id = loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            let found = extract_events(&buf).into_iter().find_map(|e| {
                if e.get("type")? == "permission_requested" {
                    Some(e.get("request_id")?.as_str()?.to_string())
                } else {
                    None
                }
            });
            if let Some(id) = found {
                break id;
            }
        };

        let resp = client
            .post(format!("http://{addr}/api/session/s1/permission"))
            .json(&serde_json::json!({ "request_id": request_id, "allow": true }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            if extract_events(&buf)
                .iter()
                .any(|e| e.get("type").map(|t| t == "done").unwrap_or(false))
            {
                break;
            }
        }
        let events = extract_events(&buf);
        assert!(events
            .iter()
            .any(|e| e.get("type").map(|t| t == "tool_started").unwrap_or(false)));
        assert!(events.iter().any(|e| {
            e.get("type").map(|t| t == "tool_finished").unwrap_or(false)
                && e.get("ok").map(|ok| ok == true).unwrap_or(false)
        }));
    }

    /// Segundo teste da fronteira: sem resposta ao pedido de permissão, o
    /// resolver expira sozinho em `Deny` — não trava o loop para sempre.
    #[tokio::test(flavor = "multi_thread")]
    async fn sse_sem_resposta_expira_em_deny_e_a_ferramenta_e_negada() {
        use forge_llm::chat::{AssistantTurn, ContentBlock, StopReason, Usage};
        use forge_llm::scripted::ScriptedGenerator;

        let dir = tempfile::tempdir().unwrap();
        let tools = ToolRegistry::default_set(dir.path());
        let turn1 = AssistantTurn {
            content: vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "echo oi"}),
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        };
        let turn2 = AssistantTurn {
            content: vec![ContentBlock::Text {
                text: "pronto".into(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        };
        let generator = Arc::new(ScriptedGenerator::from_sequence(vec![turn1, turn2]));

        // Prazo bem curto — ninguém vai responder, o teste prova a expiração.
        let hub = SessionHub::new(8, Duration::from_millis(200));
        hub.ensure_session("s1").unwrap();
        let app = router(hub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let sse_resp = client
            .get(format!("http://{addr}/api/session/s1/events"))
            .send()
            .await
            .unwrap();

        spawn_session_task(
            hub.clone(),
            "s1".into(),
            generator,
            SessionTaskSpec {
                tools,
                permissions: PermissionEngine::default(),
                model: "scripted-model".into(),
                system: "sistema de teste".into(),
                task: "faça algo".into(),
                root: dir.path().to_path_buf(),
            },
        );

        let mut stream = sse_resp.bytes_stream();
        let mut buf = String::new();
        loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            if extract_events(&buf)
                .iter()
                .any(|e| e.get("type").map(|t| t == "done").unwrap_or(false))
            {
                break;
            }
        }
        let events = extract_events(&buf);
        assert!(events
            .iter()
            .any(|e| e.get("type").map(|t| t == "tool_denied").unwrap_or(false)));
    }

    /// Quarto teste da fronteira: conectar o SSE **depois** do pedido de
    /// permissão já existir — prova a semântica de reconexão (snapshot do
    /// estado atual + eventos ao vivo daí em diante, ADR 0016), não só o
    /// caminho feliz de "já estava conectado" do primeiro teste.
    #[tokio::test(flavor = "multi_thread")]
    async fn conectar_depois_do_pedido_pendente_ainda_mostra_o_pedido() {
        use forge_llm::chat::{AssistantTurn, ContentBlock, StopReason, Usage};
        use forge_llm::scripted::ScriptedGenerator;

        let dir = tempfile::tempdir().unwrap();
        let tools = ToolRegistry::default_set(dir.path());
        let turn1 = AssistantTurn {
            content: vec![ContentBlock::ToolUse {
                id: "tu1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "echo oi"}),
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        };
        let turn2 = AssistantTurn {
            content: vec![ContentBlock::Text {
                text: "pronto".into(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 5,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        };
        let generator = Arc::new(ScriptedGenerator::from_sequence(vec![turn1, turn2]));

        let hub = SessionHub::new(8, Duration::from_secs(5));
        hub.ensure_session("s1").unwrap();
        let app = router(hub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Dispara a tarefa SEM ninguém conectado ao SSE ainda — o pedido de
        // permissão fica pendente "no vazio" (navegador fechado/nunca aberto).
        spawn_session_task(
            hub.clone(),
            "s1".into(),
            generator,
            SessionTaskSpec {
                tools,
                permissions: PermissionEngine::default(),
                model: "scripted-model".into(),
                system: "sistema de teste".into(),
                task: "faça algo".into(),
                root: dir.path().to_path_buf(),
            },
        );
        // Espera o pedido ser publicado no hub antes de "abrir o navegador".
        for _ in 0..50 {
            if hub
                .sessions
                .lock()
                .unwrap()
                .get("s1")
                .map(|s| s.pending.is_some())
                .unwrap_or(false)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // SÓ AGORA conecta — via snapshot, deve ver o pedido já pendente.
        let client = reqwest::Client::new();
        let sse_resp = client
            .get(format!("http://{addr}/api/session/s1/events"))
            .send()
            .await
            .unwrap();
        let mut stream = sse_resp.bytes_stream();
        let mut buf = String::new();
        let request_id = loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            let found = extract_events(&buf).into_iter().find_map(|e| {
                if e.get("type")? == "permission_requested" {
                    Some(e.get("request_id")?.as_str()?.to_string())
                } else {
                    None
                }
            });
            if let Some(id) = found {
                break id;
            }
        };

        // Resolve para não deixar a task de fundo pendurada além do teste.
        hub.resolve_permission("s1", &request_id, true).unwrap();
    }

    #[tokio::test]
    async fn origin_cruzada_recebe_403_e_sem_origin_passa() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let hub = SessionHub::new(8, Duration::from_secs(5));
        hub.ensure_session("s1").unwrap();
        let app = router(hub).layer(middleware::from_fn(require_local_origin));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/session/s1/permission")
                    .header(header::ORIGIN, "https://evil.example")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"request_id":"x","allow":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/session/s1/permission")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"request_id":"x","allow":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Sem Origin, passa a guarda — chega ao handler (404: pedido
        // inexistente, não 403 de CSRF).
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// Onda 2, ponta a ponta pela rota HTTP real (não `spawn_session_task`
    /// direto): `POST /api/session/:id/message` em modo roteirizado
    /// (`FORGE_SCRIPTED=1`, sem API key) dispara o mesmo `bash` real, a
    /// permissão é resolvida via HTTP, e o stream termina em `done`.
    #[tokio::test(flavor = "multi_thread")]
    async fn post_message_real_dispara_bash_via_modo_roteirizado() {
        let _guard = lock_cwd().await;
        std::env::set_var("FORGE_SCRIPTED", "1");
        let dir = tempfile::tempdir().unwrap();
        let hub = SessionHub::new(8, Duration::from_secs(5));
        let app = router(hub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let client = reqwest::Client::new();
        let sse_resp = client
            .get(format!("http://{addr}/api/session/s1/events"))
            .send()
            .await
            .unwrap();
        let mut stream = sse_resp.bytes_stream();

        let post_resp = client
            .post(format!("http://{addr}/api/session/s1/message"))
            .json(&serde_json::json!({"message": "diga oi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(post_resp.status(), reqwest::StatusCode::ACCEPTED);

        // Uma segunda mensagem concorrente na MESMA sessão é recusada (409) —
        // ator único, ADR 0018.
        let conflict_resp = client
            .post(format!("http://{addr}/api/session/s1/message"))
            .json(&serde_json::json!({"message": "outra"}))
            .send()
            .await
            .unwrap();
        assert_eq!(conflict_resp.status(), reqwest::StatusCode::CONFLICT);

        let mut buf = String::new();
        let request_id = loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            let found = extract_events(&buf).into_iter().find_map(|e| {
                if e.get("type")? == "permission_requested" {
                    Some(e.get("request_id")?.as_str()?.to_string())
                } else {
                    None
                }
            });
            if let Some(id) = found {
                break id;
            }
        };
        let resp = client
            .post(format!("http://{addr}/api/session/s1/permission"))
            .json(&serde_json::json!({"request_id": request_id, "allow": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            if extract_events(&buf)
                .iter()
                .any(|e| e.get("type").map(|t| t == "done").unwrap_or(false))
            {
                break;
            }
        }
        let events = extract_events(&buf);
        assert!(events.iter().any(|e| {
            e.get("type").map(|t| t == "tool_finished").unwrap_or(false)
                && e.get("ok").map(|ok| ok == true).unwrap_or(false)
        }));

        // `Done.ledger_verified` bate, por igualdade, com uma leitura
        // independente do MESMO `.forge/forge.db` — nunca um número
        // fabricado, nem no modo roteirizado.
        let reported = events
            .iter()
            .find_map(|e| {
                if e.get("type")? == "done" {
                    e.get("ledger_verified")?.as_u64()
                } else {
                    None
                }
            })
            .expect("evento done com ledger_verified");
        let ledger = forge_store::LedgerStore::open(
            dir.path().join(".forge").join("forge.db").to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(reported, ledger.verify_chain().unwrap());
        assert!(reported > 0);

        // A sessão não fica presa em "busy" depois do `done` — uma segunda
        // mensagem na mesma sessão é aceita normalmente.
        let second_resp = client
            .post(format!("http://{addr}/api/session/s1/message"))
            .json(&serde_json::json!({"message": "de novo"}))
            .send()
            .await
            .unwrap();
        assert_eq!(second_resp.status(), reqwest::StatusCode::ACCEPTED);

        std::env::set_current_dir(orig_cwd).unwrap();
    }

    /// Onda 13 (Modelo & Onboarding): `model`/`agent` já existiam em
    /// `SendMessageBody` desde a Onda 1, mas o frontend nunca os populava —
    /// `unwrap_or_else` sempre caía no default. Esta onda liga o frontend;
    /// este teste prova que o campo `agent` do corpo HTTP produz
    /// comportamento OBSERVÁVEL diferente via `load_rule_overrides(&root,
    /// &opts.agent)` — não só "o campo viajou": um override real (mesmo
    /// mecanismo persistido da matriz de permissão, Onda 2) para
    /// `plan`+`bash` = deny faz a MESMA mensagem roteirizada terminar direto
    /// em `tool_denied` quando `agent: "plan"` é enviado, e pedir
    /// confirmação (`permission_requested`, o default real de `build`, sem
    /// override) quando `agent` nem é enviado.
    #[tokio::test(flavor = "multi_thread")]
    async fn post_message_respeita_o_agent_do_corpo_via_override_persistido() {
        let _guard = lock_cwd().await;
        std::env::set_var("FORGE_SCRIPTED", "1");
        let dir = tempfile::tempdir().unwrap();
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        open_rule_store(dir.path())
            .unwrap()
            .set(
                "plan",
                "bash",
                None,
                forge_store::RuleDecision::Deny,
                &crate::session::now_rfc3339(),
            )
            .unwrap();

        let hub = SessionHub::new(8, Duration::from_secs(5));
        let app = router(hub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::new();

        // agent="plan": override deny já decide, nunca pede confirmação.
        let sse_plan = client
            .get(format!("http://{addr}/api/session/plan1/events"))
            .send()
            .await
            .unwrap();
        let mut stream_plan = sse_plan.bytes_stream();
        client
            .post(format!("http://{addr}/api/session/plan1/message"))
            .json(&serde_json::json!({"message": "diga oi", "agent": "plan"}))
            .send()
            .await
            .unwrap();
        let mut buf_plan = String::new();
        loop {
            let chunk = stream_plan.next().await.unwrap().unwrap();
            buf_plan.push_str(std::str::from_utf8(&chunk).unwrap());
            if extract_events(&buf_plan)
                .iter()
                .any(|e| e.get("type").map(|t| t == "done").unwrap_or(false))
            {
                break;
            }
        }
        let events_plan = extract_events(&buf_plan);
        assert!(
            events_plan
                .iter()
                .any(|e| e.get("type").map(|t| t == "tool_denied").unwrap_or(false)),
            "agent=plan com override deny deveria negar bash sem perguntar: {events_plan:?}"
        );
        assert!(
            !events_plan.iter().any(|e| e
                .get("type")
                .map(|t| t == "permission_requested")
                .unwrap_or(false)),
            "não deveria pedir confirmação — o override já decide sozinho"
        );

        // Sem `agent` no corpo: cai no default "build" (sem override) — bash
        // pede confirmação de verdade, comportamento diferente do caso acima.
        let sse_build = client
            .get(format!("http://{addr}/api/session/build1/events"))
            .send()
            .await
            .unwrap();
        let mut stream_build = sse_build.bytes_stream();
        client
            .post(format!("http://{addr}/api/session/build1/message"))
            .json(&serde_json::json!({"message": "diga oi"}))
            .send()
            .await
            .unwrap();
        let mut buf_build = String::new();
        let request_id = loop {
            let chunk = stream_build.next().await.unwrap().unwrap();
            buf_build.push_str(std::str::from_utf8(&chunk).unwrap());
            let found = extract_events(&buf_build).into_iter().find_map(|e| {
                if e.get("type")? == "permission_requested" {
                    Some(e.get("request_id")?.as_str()?.to_string())
                } else {
                    None
                }
            });
            if let Some(id) = found {
                break id;
            }
        };
        client
            .post(format!("http://{addr}/api/session/build1/permission"))
            .json(&serde_json::json!({"request_id": request_id, "allow": true}))
            .send()
            .await
            .unwrap();

        std::env::set_current_dir(orig_cwd).unwrap();
    }

    #[tokio::test]
    async fn post_message_alem_do_teto_de_sessoes_recebe_429() {
        let hub = SessionHub::new(1, Duration::from_secs(5));
        hub.ensure_session("ja-existe").unwrap();
        let app = router(hub);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://{addr}/api/session/nova/message"))
            .json(&serde_json::json!({"message": "oi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn teto_de_sessoes_simultaneas_e_respeitado() {
        let hub = SessionHub::new(2, Duration::from_millis(100));
        hub.ensure_session("s1").unwrap();
        hub.ensure_session("s2").unwrap();
        assert_eq!(hub.ensure_session("s3"), Err(HubError::TooManySessions(2)));
        // Sessão já existente não conta de novo — sempre `Ok`.
        assert!(hub.ensure_session("s1").is_ok());
    }

    async fn spawn_permissions_app() -> std::net::SocketAddr {
        let hub = SessionHub::new(8, Duration::from_secs(5));
        let app = router(hub);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    /// Onda 2 (remanescente): sem nenhum override persistido, a matriz
    /// devolvida é EXATAMENTE o default dos perfis reais
    /// (`forge_core::{BUILD,PLAN}`) — nunca os valores fabricados que o mock
    /// TS antigo inventava (ex.: `plan`+`bash` era "deny" no mock; o perfil
    /// real é "ask").
    #[tokio::test(flavor = "multi_thread")]
    async fn matriz_reflete_defaults_reais_do_perfil_sem_overrides() {
        let _guard = lock_cwd().await;
        let dir = tempfile::tempdir().unwrap();
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let addr = spawn_permissions_app().await;
        let client = reqwest::Client::new();
        let rows: Vec<serde_json::Value> = client
            .get(format!("http://{addr}/api/permissions/matrix"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        let row = |tool: &str| rows.iter().find(|r| r["tool"] == tool).unwrap().clone();
        assert_eq!(row("read")["build"], "allow");
        assert_eq!(row("edit")["build"], "ask");
        assert_eq!(row("edit")["plan"], "deny");
        // read_only(): bash é "ask", não "deny" — precisão sobre o mock antigo.
        assert_eq!(row("bash")["plan"], "ask");
        // Nenhum perfil tem regra para webfetch — Ask por ausência de regra.
        assert_eq!(row("webfetch")["build"], "ask");
        assert_eq!(row("webfetch")["plan"], "ask");

        std::env::set_current_dir(orig_cwd).unwrap();
    }

    /// Gravar uma regra persiste, a matriz passa a refletir o override (só
    /// no perfil gravado — o outro perfil continua no default), e a
    /// mutação deixa uma entrada auditada no MESMO ledger que o resto da
    /// plataforma usa.
    #[tokio::test(flavor = "multi_thread")]
    async fn post_rule_persiste_matriz_reflete_e_ledger_audita() {
        let _guard = lock_cwd().await;
        let dir = tempfile::tempdir().unwrap();
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let addr = spawn_permissions_app().await;
        let client = reqwest::Client::new();
        let set_resp = client
            .post(format!("http://{addr}/api/permissions/rules"))
            .json(&serde_json::json!({"profile": "build", "tool": "edit", "decision": "allow"}))
            .send()
            .await
            .unwrap();
        assert_eq!(set_resp.status(), reqwest::StatusCode::OK);
        let record: serde_json::Value = set_resp.json().await.unwrap();
        assert_eq!(record["tool"], "edit");
        assert_eq!(record["decision"], "allow");

        let rows: Vec<serde_json::Value> = client
            .get(format!("http://{addr}/api/permissions/matrix"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let edit = rows.iter().find(|r| r["tool"] == "edit").unwrap();
        assert_eq!(edit["build"], "allow"); // override venceu o default "ask"
        assert_eq!(edit["plan"], "deny"); // perfil "plan" não foi tocado

        let ledger = forge_store::LedgerStore::open(
            dir.path().join(".forge").join("forge.db").to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(ledger.verify_chain().unwrap(), 1);

        std::env::set_current_dir(orig_cwd).unwrap();
    }

    /// A UI "lista as rules ativas com botão de revogar": listar mostra o
    /// que foi gravado, revogar remove da lista (e do efeito), repetir a
    /// revogação é idempotente (404, não 500), e a revogação também deixa
    /// rastro no ledger — 2 entradas ao todo (set + revoke).
    #[tokio::test(flavor = "multi_thread")]
    async fn revoke_rule_remove_da_lista_e_ledger_audita_revogacao() {
        let _guard = lock_cwd().await;
        let dir = tempfile::tempdir().unwrap();
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let addr = spawn_permissions_app().await;
        let client = reqwest::Client::new();
        let record: serde_json::Value = client
            .post(format!("http://{addr}/api/permissions/rules"))
            .json(&serde_json::json!({"profile": "plan", "tool": "bash", "decision": "allow"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = record["id"].as_i64().unwrap();

        let rules_before: Vec<serde_json::Value> = client
            .get(format!("http://{addr}/api/permissions/rules"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(rules_before.len(), 1);

        let del_resp = client
            .delete(format!("http://{addr}/api/permissions/rules/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(del_resp.status(), reqwest::StatusCode::NO_CONTENT);

        let rules_after: Vec<serde_json::Value> = client
            .get(format!("http://{addr}/api/permissions/rules"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(rules_after.is_empty());

        // Revogar de novo é idempotente: 404, não um 204/500 confuso.
        let del_again = client
            .delete(format!("http://{addr}/api/permissions/rules/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(del_again.status(), reqwest::StatusCode::NOT_FOUND);

        let ledger = forge_store::LedgerStore::open(
            dir.path().join(".forge").join("forge.db").to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(ledger.verify_chain().unwrap(), 2);

        std::env::set_current_dir(orig_cwd).unwrap();
    }

    /// A prova que fecha a decisão "não read-only" da Onda 2: um override
    /// persistido de verdade (não só cosmético na matriz) muda o
    /// comportamento de uma sessão REAL — com `build`+`bash` sempre
    /// permitido, o loop nunca publica `permission_requested` (pula
    /// `resolver.resolve`, `Decision::Allow` de `evaluate` já resolve) e a
    /// ferramenta roda de verdade.
    #[tokio::test(flavor = "multi_thread")]
    async fn override_persistido_faz_sessao_real_pular_pedido_de_permissao() {
        let _guard = lock_cwd().await;
        std::env::set_var("FORGE_SCRIPTED", "1");
        let dir = tempfile::tempdir().unwrap();
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let addr = spawn_permissions_app().await;
        let client = reqwest::Client::new();
        let set_resp = client
            .post(format!("http://{addr}/api/permissions/rules"))
            .json(&serde_json::json!({"profile": "build", "tool": "bash", "decision": "allow"}))
            .send()
            .await
            .unwrap();
        assert_eq!(set_resp.status(), reqwest::StatusCode::OK);

        let sse_resp = client
            .get(format!("http://{addr}/api/session/s1/events"))
            .send()
            .await
            .unwrap();
        let mut stream = sse_resp.bytes_stream();

        let post_resp = client
            .post(format!("http://{addr}/api/session/s1/message"))
            .json(&serde_json::json!({"message": "diga oi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(post_resp.status(), reqwest::StatusCode::ACCEPTED);

        let mut buf = String::new();
        loop {
            let chunk = stream.next().await.unwrap().unwrap();
            buf.push_str(std::str::from_utf8(&chunk).unwrap());
            if extract_events(&buf)
                .iter()
                .any(|e| e.get("type").map(|t| t == "done").unwrap_or(false))
            {
                break;
            }
        }
        let events = extract_events(&buf);
        assert!(
            !events
                .iter()
                .any(|e| e.get("type").map(|t| t == "permission_requested").unwrap_or(false)),
            "bash deveria ter sido auto-aprovado pelo override persistido, sem pedir permissão: {events:?}"
        );
        assert!(
            events.iter().any(|e| {
                e.get("type").map(|t| t == "tool_finished").unwrap_or(false)
                    && e.get("ok").map(|ok| ok == true).unwrap_or(false)
            }),
            "a ferramenta bash deveria ter rodado de verdade (allow), não só sido pulada: {events:?}"
        );

        std::env::set_current_dir(orig_cwd).unwrap();
    }
}
