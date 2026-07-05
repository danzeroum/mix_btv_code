//! Decorator de rate limiting do gateway, gated por `ModelTier`
//! (`forge_llm::RateLimiter`) — a ideia dos tiers anon/auth do prompte
//! aplicada ao custo dos modelos. Fica por baixo do `CachedGenerator`:
//! um hit de cache nunca consome uma vaga do limitador.

use crate::session::now_rfc3339;
use forge_llm::chat::{AssistantTurn, GenerateRequest};
use forge_llm::gateway::{GatewayError, Generator};
use forge_llm::RateLimiter;
use forge_store::Telemetry;

pub struct RateLimitedGenerator<G: Generator> {
    inner: G,
    limiter: RateLimiter,
    telemetry: Option<Telemetry>,
}

impl<G: Generator> RateLimitedGenerator<G> {
    pub fn new(inner: G, limiter: RateLimiter, telemetry: Option<Telemetry>) -> Self {
        Self {
            inner,
            limiter,
            telemetry,
        }
    }
}

impl<G: Generator + Sync> Generator for RateLimitedGenerator<G> {
    async fn generate(
        &self,
        req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, GatewayError> {
        self.limiter
            .acquire()
            .await
            .map_err(|e| GatewayError::RateLimited(e.to_string()))?;
        if let Some(t) = &self.telemetry {
            t.record(
                "llm.call",
                "cli",
                serde_json::json!({"model": req.model}),
                &now_rfc3339(),
            );
        }
        self.inner.generate(req, on_delta).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_llm::model_tier::ModelTier;

    struct EchoGen;
    impl Generator for EchoGen {
        async fn generate(
            &self,
            _req: GenerateRequest,
            _on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, GatewayError> {
            Ok(AssistantTurn {
                provider: "echo".into(),
                content: vec![],
                stop_reason: forge_llm::chat::StopReason::EndTurn,
                usage: forge_llm::chat::Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            })
        }
    }

    #[tokio::test]
    async fn registra_telemetria_de_chamada() {
        let telemetry = Telemetry::open_in_memory().unwrap();
        let gen = RateLimitedGenerator::new(
            EchoGen,
            RateLimiter::for_tier(ModelTier::Large),
            Some(telemetry.clone()),
        );
        gen.generate(
            GenerateRequest {
                model: "x".into(),
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_tokens: 16,
                temperature: None,
            },
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(telemetry.summary().by_name.get("llm.call"), Some(&1));
    }
}
