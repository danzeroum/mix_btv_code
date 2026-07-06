//! Endpoint de carga (Fase 6 Onda 8b): um servidor HTTP mínimo que embrulha o
//! `ScriptedGenerator` (sem provider, **sem API key**) e expõe `POST /generate`.
//! É o **alvo do load-test do k6** — que martela este endpoint e valida o P95 do
//! caminho do gateway sob concorrência (a régua "k6 valida o P95", critério nº 3
//! da Fase 6). NÃO é produto: é o análogo do fixture MCP, mas para medir latência.
//!
//! Mede o overhead do NOSSO lado (serialização/agregação/streaming) isolado da
//! latência de rede do provider — que é justamente o que se quer garantir sob
//! carga. Escuta só em `127.0.0.1` (local-first, como o dashboard).

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use forge_llm::chat::GenerateRequest;
use forge_llm::{Generator, ScriptedGenerator};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    generator: Arc<ScriptedGenerator>,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("FORGE_LOADGEN_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7900);

    let state = AppState {
        generator: Arc::new(ScriptedGenerator::echo("resposta de carga, sem key real")),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/generate", post(generate))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind loadgen");
    eprintln!("forge-loadgen — http://{addr} (POST /generate, GET /health)");
    axum::serve(listener, app).await.expect("serve loadgen");
}

async fn health() -> &'static str {
    "ok"
}

/// Chama o gerador roteirizado e devolve o texto. O corpo do request é ignorado
/// de propósito (a resposta é canned) — o que se mede é o caminho, não o input.
async fn generate(State(state): State<AppState>) -> Json<Value> {
    let req = GenerateRequest {
        model: "scripted".into(),
        system: String::new(),
        messages: vec![],
        tools: vec![],
        max_tokens: 64,
        temperature: None,
    };
    let mut sink = |_: &str| {};
    let turn = state
        .generator
        .generate(req, &mut sink)
        .await
        .expect("scripted generate não falha");
    Json(json!({ "text": turn.text() }))
}
