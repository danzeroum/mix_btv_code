//! Fixtures golden dos contratos que ainda não tinham round-trip contra o
//! JSON Schema (Onda 6 — só `prompt-cache-key.v1` e `verification-evidence.v1`
//! tinham fixture antes desta onda; ver `parity.rs` e
//! `forge-verify/tests/schema_golden.rs`).
//!
//! Para `handoff-event`, `ledger-entry` e `telemetry-event` existe tipo Rust
//! (`schemars`-derivado) e nenhum consumidor Python que os parseie como
//! objeto estruturado — a fixture prova schema↔struct, não paridade
//! cross-language (não há o que comparar do lado Python). Para
//! `prompt-template` **não existe tipo em nenhum dos dois lados** ainda
//! (é a parte serializável do `generators.js` do prompte, não portada nesta
//! fase) — a fixture aqui só protege o arquivo de schema contra drift de
//! sintaxe, e isso é dito explicitamente no `$comment` da fixture, não
//! escondido.
//!
//! Cada schema tem um caso `valid` (deve bater o schema e, quando há tipo,
//! desserializar sem erro) e ao menos um `invalid_*` (deve reprovar o
//! schema) — sem o negativo, o positivo pode estar passando por schema
//! vazio/permissivo demais (a lição da Onda 1).

use forge_schemas::handoff::HandoffEvent;
use forge_schemas::ledger::LedgerEntry;
use forge_schemas::telemetry::TelemetryEvent;
use jsonschema::validator_for;
use serde_json::Value;

fn schema(name: &str) -> Value {
    let path = format!(
        "{}/../../schemas/json/{name}.v1.schema.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&raw).expect("schema é JSON válido")
}

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/../../schemas/fixtures/{name}.v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&raw).expect("fixture é JSON válido")
}

#[test]
fn handoff_event_fixture_valida_e_desserializa() {
    let schema = schema("handoff-event");
    let doc = fixture("handoff-event");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: HandoffEvent =
        serde_json::from_value(doc["valid"].clone()).expect("desserializa em HandoffEvent");
    assert_eq!(parsed.task_id, "task-1");

    assert!(
        !validator.is_valid(&doc["invalid_missing_ts"]),
        "documento sem 'ts' deveria reprovar o schema"
    );
}

#[test]
fn ledger_entry_fixture_valida_e_desserializa() {
    let schema = schema("ledger-entry");
    let doc = fixture("ledger-entry");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: LedgerEntry =
        serde_json::from_value(doc["valid"].clone()).expect("desserializa em LedgerEntry");
    assert_eq!(parsed.kind, "session.start");

    assert!(
        !validator.is_valid(&doc["invalid_missing_entry_hash"]),
        "documento sem 'entry_hash' deveria reprovar o schema"
    );
}

#[test]
fn telemetry_event_fixture_valida_e_desserializa() {
    let schema = schema("telemetry-event");
    let doc = fixture("telemetry-event");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: TelemetryEvent =
        serde_json::from_value(doc["valid"].clone()).expect("desserializa em TelemetryEvent");
    assert_eq!(parsed.name, "llm.call");

    assert!(
        !validator.is_valid(&doc["invalid_missing_name"]),
        "documento sem 'name' deveria reprovar o schema"
    );
}

/// Sem tipo Rust/Python — só protege o schema em si (sintaxe/drift), não
/// uma paridade de tipo. Ver nota no topo do arquivo e no `$comment` da
/// fixture.
#[test]
fn prompt_template_fixture_valida_contra_o_schema_sem_tipo_associado() {
    let schema = schema("prompt-template");
    let doc = fixture("prompt-template");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    assert!(
        !validator.is_valid(&doc["invalid_missing_fields"]),
        "documento sem 'fields' deveria reprovar o schema"
    );
}
