//! Gateway LLM da plataforma Forge.
//!
//! Providers HTTP (Anthropic / OpenAI / DeepSeek) com streaming SSE,
//! cadeia de fallback, classificação `ModelTier` e chave de cache por hash.
//! As API keys vivem exclusivamente neste processo — o sidecar Python só
//! conhece o socket gRPC (Fase 3+).

pub mod anthropic;
pub mod chat;
pub mod gateway;
pub mod model_tier;
pub mod openai;
pub mod provider;
pub mod rate_limit;
pub mod sse;

pub use chat::{
    AssistantTurn, ChatMessage, ContentBlock, GenerateRequest, StopReason, ToolSpec, Usage,
};
pub use gateway::{Gateway, GatewayError, Generator};
pub use model_tier::{tier_from_id, ModelTier};
pub use provider::{FallbackChain, LlmRequest, ProviderId};
pub use rate_limit::{RateLimitError, RateLimiter};
