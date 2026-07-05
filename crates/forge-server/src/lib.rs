//! API local + dashboard de métricas (origem: prompte) — Fase 3.
//!
//! Serve a telemetria offline-first gravada por `forge-store::Telemetry`
//! (`.forge/telemetry.db`) numa página HTML autocontida e duas rotas JSON.
//! Nada sai da máquina do usuário — o servidor escuta só em `127.0.0.1`.

use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use forge_store::Telemetry;
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Clone)]
struct AppState {
    telemetry: Telemetry,
}

/// Monta o router do dashboard sobre um handle de telemetria já aberto.
pub fn router(telemetry: Telemetry) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/summary", get(summary))
        .route("/api/events", get(events))
        .with_state(AppState { telemetry })
}

/// Sobe o dashboard em `addr` (bloqueia até o processo ser encerrado).
pub async fn serve(telemetry: Telemetry, addr: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(telemetry)).await
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

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="pt-br">
<head>
<meta charset="utf-8">
<title>forge — dashboard</title>
<style>
  body { font-family: system-ui, sans-serif; margin: 2rem; background: #0f1115; color: #e6e6e6; }
  h1 { font-size: 1.25rem; }
  table { border-collapse: collapse; width: 100%; margin-top: 1rem; }
  th, td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #2a2d34; font-size: 0.9rem; }
  .cards { display: flex; gap: 1rem; flex-wrap: wrap; }
  .card { background: #1a1d24; border-radius: 8px; padding: 1rem 1.5rem; min-width: 10rem; }
  .card b { display: block; font-size: 1.5rem; }
  code { color: #9ecbff; }
</style>
</head>
<body>
<h1>forge — dashboard de telemetria</h1>
<div class="cards" id="cards"></div>
<h2>eventos recentes</h2>
<table>
  <thead><tr><th>ts</th><th>nome</th><th>sessão</th><th>props</th></tr></thead>
  <tbody id="events"></tbody>
</table>
<script>
async function refresh() {
  const [summary, events] = await Promise.all([
    fetch('/api/summary').then(r => r.json()),
    fetch('/api/events?limit=50').then(r => r.json()),
  ]);
  const rate = summary.cache_hit_rate == null ? "n/a" : (summary.cache_hit_rate * 100).toFixed(1) + "%";
  document.getElementById('cards').innerHTML = `
    <div class="card"><b>${summary.total_events}</b>eventos totais</div>
    <div class="card"><b>${rate}</b>cache hit rate</div>
    <div class="card"><b>${summary.by_name['llm.call'] || 0}</b>chamadas llm</div>
    <div class="card"><b>${summary.by_name['tool.result'] || 0}</b>execuções de ferramenta</div>
  `;
  document.getElementById('events').innerHTML = events.map(e => `
    <tr><td><code>${e.ts}</code></td><td>${e.name}</td><td>${e.session_id}</td><td><code>${JSON.stringify(e.props)}</code></td></tr>
  `).join('');
}
refresh();
setInterval(refresh, 5000);
</script>
</body>
</html>
"#;

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

    #[tokio::test]
    async fn summary_devolve_json_com_total_events() {
        let app = router(telemetry_com_um_evento());
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
        let app = router(telemetry);
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
        let app = router(Telemetry::open_in_memory().unwrap());
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
