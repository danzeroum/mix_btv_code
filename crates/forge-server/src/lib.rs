//! API local + dashboard de métricas (origem: prompte) — Fase 3.
//!
//! Serve a telemetria offline-first gravada por `forge-store::Telemetry`
//! (`.forge/telemetry.db`) para a SPA em `web/dist` (React/TS, ver `web/`)
//! e duas rotas JSON. Nada sai da máquina do usuário — o servidor escuta
//! só em `127.0.0.1`.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use forge_store::Telemetry;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct AppState {
    telemetry: Telemetry,
}

/// Monta o router do dashboard sobre um handle de telemetria já aberto,
/// servindo os assets estáticos da SPA a partir de `web_dir` (build de
/// `web/`, tipicamente `web/dist`). Path relativo é resolvido contra o
/// diretório de trabalho do processo — ver `forge-cli`'s `run_dashboard`
/// para a resolução por `FORGE_WEB_DIR`/padrão.
pub fn router(telemetry: Telemetry, web_dir: impl AsRef<Path>) -> Router {
    let web_dir = web_dir.as_ref();
    let index_html = web_dir.join("index.html");
    // `fallback` (não `not_found_service`) preserva o status 200 de `index.html`
    // para rotas client-side desconhecidas do servidor (padrão SPA).
    let serve_dir = ServeDir::new(web_dir).fallback(ServeFile::new(index_html));

    Router::new()
        .route("/api/summary", get(summary))
        .route("/api/events", get(events))
        .fallback_service(serve_dir)
        .with_state(AppState { telemetry })
}

/// Sobe o dashboard em `addr` (bloqueia até o processo ser encerrado).
pub async fn serve(
    telemetry: Telemetry,
    addr: SocketAddr,
    web_dir: impl AsRef<Path>,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(telemetry, web_dir)).await
}

/// Resolve o diretório da SPA por precedência: `FORGE_WEB_DIR` → `web/dist`
/// (assumindo execução a partir da raiz do repo). Evita hardcodar a
/// suposição de CWD dentro do router em si.
pub fn default_web_dir() -> PathBuf {
    std::env::var_os("FORGE_WEB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/dist"))
}

async fn summary(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.telemetry.summary())
}

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<u32>,
}

async fn events(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> impl IntoResponse {
    Json(state.telemetry.recent(q.limit.unwrap_or(50)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn telemetry_com_um_evento() -> Telemetry {
        let telemetry = Telemetry::open_in_memory().unwrap();
        telemetry.record(
            "llm.call",
            "s1",
            serde_json::json!({"provider": "anthropic"}),
            "2026-07-05T00:00:00Z",
        );
        telemetry
    }

    /// Fixture de `web/dist` com estrutura aninhada (não só um `index.html`
    /// solto) — exercita o `ServeDir` real: subpasta `assets/` com JS/CSS e
    /// um `favicon.svg` na raiz, para pegar bugs de content-type e de
    /// arquivo-real-vence-fallback que uma fixture trivial não pegaria.
    fn fixture_web_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("index.html"),
            "<html><body>forge</body></html>",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(
            dir.path().join("assets").join("app-abc123.js"),
            "console.log('forge')",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("assets").join("app-abc123.css"),
            "body { color: red; }",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("favicon.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        )
        .unwrap();
        dir
    }

    #[tokio::test]
    async fn summary_devolve_json_com_total_events() {
        let web_dir = fixture_web_dir();
        let app = router(telemetry_com_um_evento(), web_dir.path());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/summary")
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
        assert_eq!(json["total_events"], 1);
    }

    #[tokio::test]
    async fn events_respeita_o_limite() {
        let telemetry = telemetry_com_um_evento();
        telemetry.record("cache.hit", "s1", serde_json::json!({}), "t2");
        let web_dir = fixture_web_dir();
        let app = router(telemetry, web_dir.path());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/events?limit=1")
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
        assert_eq!(json.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn index_devolve_html() {
        let web_dir = fixture_web_dir();
        let app = router(Telemetry::open_in_memory().unwrap(), web_dir.path());
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rota_desconhecida_cai_no_index_html_spa_fallback() {
        let web_dir = fixture_web_dir();
        let app = router(Telemetry::open_in_memory().unwrap(), web_dir.path());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/designer")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn asset_aninhado_e_servido_com_content_type_correto() {
        let web_dir = fixture_web_dir();
        let app = router(Telemetry::open_in_memory().unwrap(), web_dir.path());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/assets/app-abc123.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let content_type = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            content_type.contains("javascript"),
            "esperava content-type de JS, veio {content_type}"
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"console.log('forge')");
    }

    #[tokio::test]
    async fn favicon_real_na_raiz_nao_e_engolido_pelo_fallback_da_spa() {
        let web_dir = fixture_web_dir();
        let app = router(Telemetry::open_in_memory().unwrap(), web_dir.path());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/favicon.svg")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let content_type = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            content_type.contains("svg"),
            "esperava content-type de SVG (arquivo real), veio {content_type} — indício de ter caído no fallback de index.html"
        );
    }
}
