//! Storage durável da plataforma Forge (SQLite via rusqlite).
//!
//! Fase 1–3: ledger append-only com hash-chain verificável, cache de
//! prompts por hash, event store para sessões duráveis, telemetria
//! offline-first e biblioteca de prompts.

pub mod events;
pub mod ledger;
pub mod prompt_cache;
pub mod prompt_library;
pub mod rule_store;
pub mod telemetry;

pub use events::{EventError, EventInput, EventStore, StoredEvent};
pub use ledger::LedgerStore;
pub use prompt_cache::PromptCache;
pub use prompt_library::{PromptLibrary, SavedPrompt};
pub use rule_store::{RuleDecision, RuleRecord, RuleStore, RuleStoreError};
pub use telemetry::{Telemetry, TelemetryRecord, TelemetryStore, TelemetrySummary};
