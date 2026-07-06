//! Cliente e supervisor do sidecar Python `forge_promptforge` (Fase 3):
//! primeira ativação do canal gRPC Rust↔Python sobre Unix Domain Socket.
//!
//! `SidecarSupervisor` spawna `uv run python -m forge_promptforge.server`
//! e espera o health check; `SidecarClient` fala `PromptForgeService`
//! (lint/render/list_generators). O núcleo Rust nunca chama provedores
//! LLM a partir daqui — este serviço só cobre a camada de prompts.

pub mod client;
pub mod core_server;
pub mod memory_client;
pub mod service;
pub mod squad_client;
pub mod supervisor;

pub use client::{SidecarClient, SidecarError};
pub use core_server::{serve_core, CoreBackend, CoreServer};
pub use memory_client::{MemoryClient, MemorySupervisor};
pub use service::{MemoryService, SidecarService, SquadLease, SquadPool};
pub use squad_client::{drain_stream, SquadClient, SquadRun, SquadSupervisor};
pub use supervisor::SidecarSupervisor;
