//! Gateway LLM: transporte HTTP com streaming, keys só neste processo,
//! cadeia de fallback entre providers (princípio do proxy do prompte).

use crate::anthropic;
use crate::chat::{AssistantTurn, GenerateRequest};
use crate::openai;
use crate::provider::ProviderId;
use crate::sse::SseParser;
use futures_util::StreamExt;

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("nenhum provider configurado — defina ANTHROPIC_API_KEY, DEEPSEEK_API_KEY ou OPENAI_API_KEY")]
    NoProvider,
    #[error("todos os providers falharam: {0}")]
    AllFailed(String),
    #[error("limite de requisições excedido: {0}")]
    RateLimited(String),
}

/// Contrato de geração consumido pelo loop de agente (forge-core). O loop é
/// genérico sobre este trait, então testes usam um gerador roteirizado.
pub trait Generator {
    fn generate(
        &self,
        req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> impl std::future::Future<Output = Result<AssistantTurn, GatewayError>> + Send;
}

#[derive(Debug, Clone)]
struct ProviderConfig {
    id: ProviderId,
    api_key: String,
    base_url: String,
}

pub struct Gateway {
    client: reqwest::Client,
    providers: Vec<ProviderConfig>,
}

impl Gateway {
    /// Detecta providers pelas variáveis de ambiente, na ordem da cadeia de
    /// fallback padrão: Anthropic → DeepSeek → OpenAI.
    pub fn from_env() -> Self {
        let candidates = [
            (
                ProviderId::Anthropic,
                "ANTHROPIC_API_KEY",
                anthropic::DEFAULT_BASE_URL,
            ),
            (
                ProviderId::Deepseek,
                "DEEPSEEK_API_KEY",
                openai::DEEPSEEK_BASE_URL,
            ),
            (
                ProviderId::Openai,
                "OPENAI_API_KEY",
                openai::OPENAI_BASE_URL,
            ),
        ];
        let providers = candidates
            .into_iter()
            .filter_map(|(id, env, base)| {
                std::env::var(env)
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|api_key| ProviderConfig {
                        id,
                        api_key,
                        base_url: base.to_string(),
                    })
            })
            .collect();
        Self {
            client: reqwest::Client::new(),
            providers,
        }
    }

    /// Nomes dos providers disponíveis (para o CLI reportar).
    pub fn available(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|p| provider_name(&p.id).to_string())
            .collect()
    }

    async fn call_provider(
        &self,
        cfg: &ProviderConfig,
        req: &GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, String> {
        let (url, request) = match cfg.id {
            ProviderId::Anthropic => (
                format!("{}/v1/messages", cfg.base_url),
                self.client
                    .post(format!("{}/v1/messages", cfg.base_url))
                    .header("x-api-key", &cfg.api_key)
                    .header("anthropic-version", anthropic::API_VERSION)
                    .json(&anthropic::build_request_body(req)),
            ),
            _ => (
                format!("{}/v1/chat/completions", cfg.base_url),
                self.client
                    .post(format!("{}/v1/chat/completions", cfg.base_url))
                    .bearer_auth(&cfg.api_key)
                    .json(&openai::build_request_body(req)),
            ),
        };

        let resp = request.send().await.map_err(|e| format!("{url}: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url}: HTTP {status}: {}",
                body.chars().take(300).collect::<String>()
            ));
        }

        let mut parser = SseParser::new();
        let mut stream = resp.bytes_stream();
        match cfg.id {
            ProviderId::Anthropic => {
                let mut agg = anthropic::TurnAggregator::new();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| format!("stream: {e}"))?;
                    for event in parser.push(&chunk) {
                        if let Some(delta) = agg.handle(&event.data) {
                            on_delta(&delta);
                        }
                    }
                }
                Ok(agg.finish())
            }
            _ => {
                let mut agg = openai::TurnAggregator::new(provider_name(&cfg.id));
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| format!("stream: {e}"))?;
                    for event in parser.push(&chunk) {
                        if let Some(delta) = agg.handle(&event.data) {
                            on_delta(&delta);
                        }
                    }
                }
                Ok(agg.finish())
            }
        }
    }
}

impl Generator for Gateway {
    async fn generate(
        &self,
        req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, GatewayError> {
        if self.providers.is_empty() {
            return Err(GatewayError::NoProvider);
        }
        let mut failures = Vec::new();
        for cfg in &self.providers {
            match self.call_provider(cfg, &req, on_delta).await {
                Ok(turn) => return Ok(turn),
                Err(e) => failures.push(format!("{}: {e}", provider_name(&cfg.id))),
            }
        }
        Err(GatewayError::AllFailed(failures.join(" | ")))
    }
}

fn provider_name(id: &ProviderId) -> &'static str {
    match id {
        ProviderId::Anthropic => "anthropic",
        ProviderId::Deepseek => "deepseek",
        ProviderId::Openai => "openai",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_keys_nao_ha_providers() {
        // from_env depende do ambiente; aqui garantimos só a construção
        // vazia → NoProvider (o CI não tem keys definidas).
        let gw = Gateway {
            client: reqwest::Client::new(),
            providers: vec![],
        };
        let err = futures_util::future::FutureExt::now_or_never(gw.generate(
            GenerateRequest {
                model: "x".into(),
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_tokens: 16,
                temperature: None,
            },
            &mut |_| {},
        ))
        .expect("resolve imediatamente")
        .unwrap_err();
        assert!(matches!(err, GatewayError::NoProvider));
    }
}
