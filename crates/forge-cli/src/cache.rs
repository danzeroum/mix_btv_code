//! Decorator de cache do gateway (`prompt-cache-key.v1`).
//!
//! Requests idênticos (modelo + system + ferramentas + histórico +
//! temperatura) devolvem o turno gravado sem tocar a rede — a ideia de
//! cache por hash do prompte aplicada ao coding agent. O hash usa
//! `forge_schemas::request_hash`, o mesmo contrato com paridade
//! Rust×Python garantida pelas fixtures.

use crate::session::now_rfc3339;
use forge_llm::chat::{AssistantTurn, GenerateRequest};
use forge_llm::gateway::{GatewayError, Generator};
use forge_store::{PromptCache, Telemetry};
use serde_json::{json, Value};
use std::sync::Mutex;

pub struct CachedGenerator<G: Generator> {
    inner: G,
    cache: Mutex<PromptCache>,
    telemetry: Option<Telemetry>,
}

impl<G: Generator> CachedGenerator<G> {
    pub fn new(inner: G, cache: PromptCache, telemetry: Option<Telemetry>) -> Self {
        Self {
            inner,
            cache: Mutex::new(cache),
            telemetry,
        }
    }

    fn cache_key(req: &GenerateRequest) -> String {
        // O "messages" do contrato v1 é o envelope canônico completo do
        // request — inclui modelo/system/tools para evitar colisões.
        let envelope = json!({
            "model": req.model,
            "system": req.system,
            "tools": req.tools,
            "chat": req.messages,
        });
        let temperature = req.temperature.map(|t| json!(t)).unwrap_or(Value::Null);
        forge_schemas::request_hash(&envelope, &temperature)
    }
}

impl<G: Generator + Sync> Generator for CachedGenerator<G> {
    async fn generate(
        &self,
        req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, GatewayError> {
        let key = Self::cache_key(&req);

        let hit = { self.cache.lock().unwrap().get(&key).ok().flatten() };
        if let Some(stored) = hit {
            if let Ok(mut turn) = serde_json::from_str::<AssistantTurn>(&stored) {
                let text = turn.text();
                if !text.is_empty() {
                    on_delta(&text);
                }
                turn.provider = format!("{}+cache", turn.provider);
                if let Some(t) = &self.telemetry {
                    t.record(
                        "cache.hit",
                        "cli",
                        json!({"model": req.model}),
                        &now_rfc3339(),
                    );
                }
                return Ok(turn);
            }
        }

        if let Some(t) = &self.telemetry {
            t.record(
                "cache.miss",
                "cli",
                json!({"model": req.model}),
                &now_rfc3339(),
            );
        }
        let turn = self.inner.generate(req, on_delta).await?;
        if let Ok(serialized) = serde_json::to_string(&turn) {
            let ts = now_rfc3339();
            let _ = self.cache.lock().unwrap().put(&key, &serialized, &ts);
        }
        Ok(turn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_llm::chat::{ChatMessage, ContentBlock, StopReason, Usage};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Gerador que conta chamadas e devolve sempre o mesmo turno.
    struct CountingGen {
        calls: AtomicUsize,
    }

    impl Generator for CountingGen {
        async fn generate(
            &self,
            _req: GenerateRequest,
            on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, GatewayError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            on_delta("resposta");
            Ok(AssistantTurn {
                content: vec![ContentBlock::Text {
                    text: "resposta".into(),
                }],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
                provider: "fake".into(),
            })
        }
    }

    fn req(task: &str) -> GenerateRequest {
        GenerateRequest {
            model: "m".into(),
            system: "s".into(),
            messages: vec![ChatMessage::user_text(task)],
            tools: vec![],
            max_tokens: 64,
            temperature: Some(0.3),
        }
    }

    #[tokio::test]
    async fn segunda_chamada_identica_vem_do_cache() {
        let gen = CachedGenerator::new(
            CountingGen {
                calls: AtomicUsize::new(0),
            },
            PromptCache::open_in_memory().unwrap(),
            None,
        );

        let mut deltas = String::new();
        let t1 = gen
            .generate(req("oi"), &mut |d| deltas.push_str(d))
            .await
            .unwrap();
        let t2 = gen
            .generate(req("oi"), &mut |d| deltas.push_str(d))
            .await
            .unwrap();
        let t3 = gen
            .generate(req("outra"), &mut |d| deltas.push_str(d))
            .await
            .unwrap();

        assert_eq!(gen.inner.calls.load(Ordering::SeqCst), 2); // hit no meio
        assert_eq!(t1.provider, "fake");
        assert_eq!(t2.provider, "fake+cache");
        assert_eq!(t3.provider, "fake");
        assert_eq!(t2.text(), "resposta");
        assert_eq!(deltas, "resposta".repeat(3)); // hit também emite o texto
    }
}
