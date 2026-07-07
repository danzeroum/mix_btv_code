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

use forge_schemas::experiment::ExperimentReport;
use forge_schemas::handoff::HandoffEvent;
use forge_schemas::ledger::LedgerEntry;
use forge_schemas::persona::Persona;
use forge_schemas::plan::Plan;
use forge_schemas::telemetry::TelemetryEvent;
use forge_schemas::workflow::SquadWorkflow;
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

#[test]
fn experiment_fixture_valida_e_desserializa() {
    let schema = schema("experiment");
    let doc = fixture("experiment");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: ExperimentReport =
        serde_json::from_value(doc["valid"].clone()).expect("desserializa em ExperimentReport");
    assert_eq!(parsed.experiment, "prompt-tone");
    assert_eq!(parsed.winner.as_deref(), Some("A"));

    assert!(
        !validator.is_valid(&doc["invalid_missing_verdict"]),
        "documento sem 'verdict' deveria reprovar o schema"
    );
}

/// A checagem semântica (aresta referencia nó inexistente) não é
/// expressável em JSON Schema puro — fica em `SquadWorkflow::validate_edges`
/// (testada isoladamente em `workflow.rs`). Aqui só a FORMA: campo
/// obrigatório ausente (`removable`) deve reprovar o schema.
#[test]
fn squad_workflow_fixture_valida_e_desserializa() {
    let schema = schema("squad-workflow");
    let doc = fixture("squad-workflow");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: SquadWorkflow =
        serde_json::from_value(doc["valid"].clone()).expect("desserializa em SquadWorkflow");
    assert_eq!(parsed.nodes.len(), 2);
    assert!(parsed.validate_edges().is_ok());

    assert!(
        !validator.is_valid(&doc["invalid_missing_removable"]),
        "documento sem 'removable' deveria reprovar o schema"
    );
}

#[test]
fn persona_fixture_valida_e_desserializa() {
    let schema = schema("persona");
    let doc = fixture("persona");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: Persona =
        serde_json::from_value(doc["valid"].clone()).expect("desserializa em Persona");
    assert_eq!(parsed.id, "revisor-de-estilo");
    assert!(parsed.validate().is_ok());

    // O limiar fora de [0,1] passa pelo JSON Schema? Não — o schema também
    // limita (minimum/maximum). Cobre as duas camadas (schema + validate()).
    assert!(
        !validator.is_valid(&doc["invalid_threshold_fora_de_intervalo"]),
        "confidence_threshold=1.5 deveria reprovar o schema"
    );
    let parsed_invalid: Persona =
        serde_json::from_value(doc["invalid_threshold_fora_de_intervalo"].clone())
            .expect("desserializa mesmo com valor fora de intervalo (schema é quem barra)");
    assert!(parsed_invalid.validate().is_err());
}

#[test]
fn plan_fixture_valida_e_desserializa() {
    let schema = schema("plan");
    let doc = fixture("plan");
    let validator = validator_for(&schema).expect("schema compila");

    assert!(
        validator.is_valid(&doc["valid"]),
        "fixture válida não bateu o schema: {:?}",
        validator.iter_errors(&doc["valid"]).collect::<Vec<_>>()
    );
    let parsed: Plan = serde_json::from_value(doc["valid"].clone()).expect("desserializa em Plan");
    assert_eq!(parsed.execution_sequence.len(), 2);
    assert!(parsed.validate().is_ok());

    assert!(
        !validator.is_valid(&doc["invalid_sequence_vazia"]),
        "execution_sequence vazia deveria reprovar o schema (minItems)"
    );
}

/// A galeria semeada de personas (`schemas/personas/**/*.json`) é CONTEÚDO:
/// cada arquivo tem que bater o `persona.v1` e passar a validação semântica.
/// Uma persona quebrada na galeria é um bug de conteúdo — pega aqui, não em
/// produção.
#[test]
fn galeria_de_personas_valida_contra_persona_v1() {
    let schema = schema("persona");
    let validator = validator_for(&schema).expect("schema compila");
    let dir = format!("{}/../../schemas/personas", env!("CARGO_MANIFEST_DIR"));

    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect(std::path::Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "esperava personas semeadas na galeria");

    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
        let doc: Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{file:?}: {e}"));
        assert!(
            validator.is_valid(&doc),
            "persona {file:?} não bateu o schema: {:?}",
            validator.iter_errors(&doc).collect::<Vec<_>>()
        );
        let persona: Persona = serde_json::from_value(doc)
            .unwrap_or_else(|e| panic!("{file:?} não desserializa: {e}"));
        persona
            .validate()
            .unwrap_or_else(|e| panic!("{file:?} inválida: {e}"));
    }
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
