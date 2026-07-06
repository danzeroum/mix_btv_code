//! Fase 7 Onda 8 (A3): `GET /api/memory` + `POST /api/memory/recall` sobre
//! o `MemoryService` (`forge-sidecar`, ADR 0022) — precisa de
//! `forge-sidecar`, que `forge-server` não tem (regra de posicionamento de
//! rota da fase), por isso mora aqui e é `.merge()`ado no router do agente
//! web, igual a `prompt_render`/`squad_agent`/`mcp_console`.
//!
//! Rótulo/nav dizem "RAG"; a tela e esta rota carregam a mesma tensão
//! honesta do resto da plataforma: a recuperação é léxica (TF-IDF,
//! `recall.py`, ADR 0013), não semântica — não finge ser embedding neural.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use forge_sidecar::{MemoryService, SidecarError};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::web_agent::ErrorBody;

#[derive(Clone)]
struct MemoryAgentState {
    service: Arc<MemoryService>,
}

fn sidecar_error_response(e: SidecarError) -> Response {
    match e {
        SidecarError::Unavailable(msg) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody::new("memory_sidecar_unavailable", msg)),
        )
            .into_response(),
        SidecarError::Rpc(status) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorBody::new(
                "memory_sidecar_rpc_error",
                status.message().to_string(),
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    agent: Option<String>,
    limit: Option<u32>,
}

/// `GET /api/memory?agent=&limit=` — mapa de memória agrupado por agente
/// (contagem real, decisão mais recente/de maior confiança reais — sem
/// coluna de tendência de esquecimento, que nada no código computa).
async fn list_handler(
    State(state): State<MemoryAgentState>,
    Query(q): Query<ListQuery>,
) -> Response {
    let mut client = match state.service.client().await {
        Ok(c) => c,
        Err(e) => return sidecar_error_response(e),
    };
    match client.list(q.agent, q.limit.unwrap_or(50)).await {
        Ok(resp) => Json(resp.agents).into_response(),
        Err(e) => sidecar_error_response(e),
    }
}

#[derive(Deserialize)]
struct RecallBody {
    query: String,
    #[serde(default)]
    k: Option<u32>,
}

/// `POST /api/memory/recall {query,k}` — busca léxica (TF-IDF) sobre o
/// corpus episódico.
async fn recall_handler(
    State(state): State<MemoryAgentState>,
    Json(body): Json<RecallBody>,
) -> Response {
    let mut client = match state.service.client().await {
        Ok(c) => c,
        Err(e) => return sidecar_error_response(e),
    };
    match client.recall(&body.query, body.k.unwrap_or(5)).await {
        Ok(resp) => Json(resp.matches).into_response(),
        Err(e) => sidecar_error_response(e),
    }
}

/// Router aditivo do mapa de memória — `.merge()`ado ao router do agente
/// web, mesma composição de `squad_agent`/`prompt_render`/`mcp_console`.
pub fn router(service: Arc<MemoryService>) -> Router {
    Router::new()
        .route("/api/memory", get(list_handler))
        .route("/api/memory/recall", post(recall_handler))
        .with_state(MemoryAgentState { service })
}

/// Constrói o `MemoryService` para o agente web. `memory_dir: None` —
/// mesma resolução relativa que `forge_squad.server`'s `SquadServicer` já
/// usa hoje (nunca recebe `--memory-dir`), então os dois processos
/// concordam sobre onde o corpus mora (ver doc de `MemorySupervisor::spawn`).
pub fn default_memory_service(root: &std::path::Path) -> Arc<MemoryService> {
    let py_dir = crate::squad::locate_python_dir().unwrap_or_else(|| root.join("python"));
    let socket = root.join(".forge").join("memory.sock");
    Arc::new(MemoryService::new(
        py_dir,
        socket,
        None,
        Duration::from_secs(30),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn uv_missing() -> bool {
        std::process::Command::new("uv")
            .arg("--version")
            .output()
            .is_err()
    }

    fn python_workspace_present() -> bool {
        crate::squad::locate_python_dir().is_some()
    }

    fn write_corpus(memory_dir: &std::path::Path) {
        std::fs::create_dir_all(memory_dir).unwrap();
        std::fs::write(
            memory_dir.join("agent_memories.jsonl"),
            concat!(
                r#"{"timestamp":"2026-01-01T00:00:00Z","agent":"architect","decision":{"summary":"plano de arquitetura do gateway aprovado"},"confidence":0.9}"#,
                "\n",
            ),
        )
        .unwrap();
    }

    /// Fronteira da Onda 8: `POST /api/memory/recall` sobre um sidecar de
    /// memória REAL recupera a memória semeada por fora (ground truth de
    /// vocabulário), e `GET /api/memory` mostra o mapa agrupado por agente
    /// com contagem/decisão reais — de ponta a ponta pelo HTTP.
    #[tokio::test]
    async fn recall_e_list_via_http_batem_com_o_corpus_semeado_por_fora() {
        if uv_missing() || !python_workspace_present() {
            eprintln!("uv/workspace Python ausente — pulando teste de memória real");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let memory_dir = dir.path().join("squad-memory");
        write_corpus(&memory_dir);

        let py_dir = crate::squad::locate_python_dir().unwrap();
        let service = Arc::new(forge_sidecar::MemoryService::new(
            py_dir,
            dir.path().join("memory.sock"),
            Some(memory_dir),
            std::time::Duration::from_secs(30),
        ));
        let app = router(service);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/recall")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "plano de arquitetura", "k": 3}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let matches: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = matches.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["agent"], "architect");
        assert!(arr[0]["decision_json"]
            .as_str()
            .unwrap()
            .contains("arquitetura"));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/memory")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let agents: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = agents.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["agent"], "architect");
        assert_eq!(arr[0]["count"], 1);
    }

    /// Sidecar inatingível (workspace Python inexistente) devolve `503`
    /// explícito — mesmo padrão de degradação do PromptForge.
    #[tokio::test]
    async fn sidecar_indisponivel_devolve_503_explicito() {
        let dir = tempfile::tempdir().unwrap();
        let service = Arc::new(MemoryService::new(
            dir.path().to_path_buf(),
            dir.path().join("memory.sock"),
            None,
            Duration::from_secs(2),
        ));
        let app = router(service);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/memory")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
