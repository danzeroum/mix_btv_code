//! Fase 7 Onda 5 (metade `render`): `POST /api/prompt/render` + `GET
//! /api/prompt/generators` sobre o sidecar PromptForge — precisa de
//! `forge-sidecar`, que `forge-server` não tem (regra de posicionamento de
//! rota da fase), por isso mora aqui e é `.merge()`ado no router do agente
//! web (`web_agent::merged_router`'s `extra`), não em `forge-server`.
//!
//! Usa [`SidecarService`] (Onda 3, ADR 0019: instância única compartilhada,
//! sidecar stateless — serializar um render por vez é aceitável) em vez de
//! subir um processo por chamada.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use forge_sidecar::{SidecarError, SidecarService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::web_agent::ErrorBody;

#[derive(Clone)]
struct PromptAgentState {
    service: Arc<SidecarService>,
}

fn sidecar_error_response(e: SidecarError) -> Response {
    match e {
        SidecarError::Unavailable(msg) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody::new("sidecar_unavailable", msg)),
        )
            .into_response(),
        SidecarError::Rpc(status) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorBody::new(
                "sidecar_rpc_error",
                status.message().to_string(),
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct RenderBody {
    generator: String,
    #[serde(default)]
    fields: HashMap<String, String>,
}

#[derive(Serialize)]
struct RenderResponseBody {
    prompt: String,
}

/// `POST /api/prompt/render` — mesma chamada `SidecarClient::render` que o
/// `/prompt save|<generator>` do CLI já usa (`main.rs::handle_prompt_command`),
/// só que sobre o `SidecarService` de longa duração em vez de um processo
/// novo por invocação.
async fn render_handler(
    State(state): State<PromptAgentState>,
    Json(body): Json<RenderBody>,
) -> Response {
    let mut client = match state.service.client().await {
        Ok(c) => c,
        Err(e) => return sidecar_error_response(e),
    };
    match client.render(&body.generator, body.fields).await {
        Ok(prompt) => Json(RenderResponseBody { prompt }).into_response(),
        Err(e) => sidecar_error_response(e),
    }
}

/// `GET /api/prompt/generators` — serde direto no tipo gerado pelo proto
/// (`forge_proto::promptforge::GeneratorInfo` ganha `Serialize` via
/// `forge-proto/build.rs`, mesma técnica do `SquadEvent` da Onda 4) — sem
/// DTO espelho.
async fn generators_handler(State(state): State<PromptAgentState>) -> Response {
    let mut client = match state.service.client().await {
        Ok(c) => c,
        Err(e) => return sidecar_error_response(e),
    };
    match client.list_generators().await {
        Ok(generators) => Json(generators).into_response(),
        Err(e) => sidecar_error_response(e),
    }
}

/// Router aditivo do render de prompts — `.merge()`ado ao router do agente
/// web (mesma composição de `squad_agent::router`, mesma guarda de
/// `Origin`/`Host` herdada de `merged_router`).
pub fn router(service: Arc<SidecarService>) -> Router {
    Router::new()
        .route("/api/prompt/render", post(render_handler))
        .route("/api/prompt/generators", get(generators_handler))
        .with_state(PromptAgentState { service })
}

/// Constrói o `SidecarService` do PromptForge para o agente web. Workspace
/// Python ausente não impede a construção (lazy: só falha, com erro claro,
/// no primeiro `client()` de verdade) — mesma filosofia fail-soft-até-o-uso
/// do resto do agente web.
pub fn default_sidecar_service(root: &std::path::Path) -> Arc<SidecarService> {
    let py_dir = crate::squad::locate_python_dir().unwrap_or_else(|| PathBuf::from("python"));
    let socket = root.join(".forge").join("promptforge.sock");
    Arc::new(SidecarService::new(py_dir, socket, Duration::from_secs(30)))
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

    /// Fronteira da Onda 5 (render), ponta a ponta contra o sidecar
    /// PromptForge REAL: `POST /api/prompt/render` devolve o MESMO texto que
    /// uma chamada gRPC direta ao mesmo processo (`SidecarClient::render`) —
    /// paridade, não só "200 OK". Também confere `GET /api/prompt/generators`
    /// contra a mesma lista devolvida por uma chamada gRPC direta.
    #[tokio::test]
    async fn render_via_http_bate_com_chamada_grpc_direta_ao_mesmo_sidecar() {
        if uv_missing() || !python_workspace_present() {
            eprintln!("uv/workspace Python ausente — pulando teste de render real");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let service = default_sidecar_service(dir.path());
        let app = router(Arc::clone(&service));

        let mut fields = HashMap::new();
        fields.insert("language".to_string(), "rust".to_string());
        fields.insert("context".to_string(), "gateway LLM".to_string());
        fields.insert("code".to_string(), "fn main() {}".to_string());
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/prompt/render")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"generator": "code-review", "fields": fields})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let via_http = json["prompt"].as_str().unwrap().to_string();

        // Mesma chamada, agora direto por gRPC contra o MESMO processo (o
        // `SidecarService` já subiu e ficou vivo da chamada HTTP acima).
        let mut direct_fields = HashMap::new();
        direct_fields.insert("language".to_string(), "rust".to_string());
        direct_fields.insert("context".to_string(), "gateway LLM".to_string());
        direct_fields.insert("code".to_string(), "fn main() {}".to_string());
        let mut direct_client = service.client().await.unwrap();
        let via_grpc = direct_client
            .render("code-review", direct_fields)
            .await
            .unwrap();
        assert_eq!(
            via_http, via_grpc,
            "paridade entre a rota HTTP e o gRPC direto"
        );
        assert!(!via_http.is_empty());

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/prompt/generators")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let names: Vec<String> = json
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["name"].as_str().unwrap().to_string())
            .collect();
        let mut direct_generators = service.client().await.unwrap();
        let direct_names: Vec<String> = direct_generators
            .list_generators()
            .await
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(
            names, direct_names,
            "mesma lista via HTTP e via gRPC direto"
        );
        assert!(names.contains(&"code-review".to_string()));
    }

    /// Sidecar inatingível (workspace Python inexistente) devolve `503`
    /// explícito — fail-soft honesto, não um erro genérico 500 nem um
    /// timeout silencioso.
    #[tokio::test]
    async fn sidecar_indisponivel_devolve_503_explicito() {
        let dir = tempfile::tempdir().unwrap();
        // Diretório vazio: sem `pyproject.toml`, `uv run` falha rápido (ou o
        // spawn falha de saída se `uv` nem existir no ambiente) — os dois
        // casos são `SidecarError::Unavailable`.
        let service = Arc::new(forge_sidecar::SidecarService::new(
            dir.path().to_path_buf(),
            dir.path().join("promptforge.sock"),
            Duration::from_secs(2),
        ));
        let app = router(service);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/prompt/generators")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
