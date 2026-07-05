//! Runtime de sessão da plataforma Forge.
//!
//! Fase 1: motor de permissões, perfis de agente e o loop de agente
//! (mensagens → gateway → ferramentas → repete). O System Context completo
//! (Context Sources tipados, Epochs, compaction em Safe Provider-Turn
//! Boundaries — spec: `opencode/CONTEXT.md`) chega na Fase 2.

pub mod agent;
pub mod agent_loop;
pub mod compaction;
pub mod permission;
pub mod session;

pub use agent::{AgentProfile, BUILD, GENERAL, PLAN};
pub use agent_loop::{
    AgentLoop, DenyAll, LoopError, LoopEvent, LoopOutcome, PermissionResolver, TurnSummary,
};
pub use compaction::{estimate_tokens, CompactionPolicy};
pub use permission::{Decision, PermissionEngine, Rule};
pub use session::{DurableSession, SessionError};
