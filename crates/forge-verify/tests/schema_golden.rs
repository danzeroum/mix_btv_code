//! Fixture golden: uma `VerificationEvidence` real produzida por
//! `run_pipeline` (com findings preenchidos de verdade, não vazios) precisa
//! validar contra `schemas/json/verification-evidence.v1.schema.json` —
//! fecha o risco de drift de contrato citado no PLANO agora que `findings`
//! deixou de ser sempre `[]`.

use forge_verify::{run_pipeline, Parser, StepSpec};
use jsonschema::validator_for;
use serde_json::Value;
use std::time::Duration;

fn schema() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/json/verification-evidence.v1.schema.json"
    );
    let raw = std::fs::read_to_string(path).expect("schema existe");
    serde_json::from_str(&raw).expect("schema é JSON válido")
}

#[test]
fn evidencia_real_com_findings_valida_contra_o_schema() {
    let evidence = run_pipeline(
        "run-golden",
        "deadbeef",
        "2026-07-05T00:00:00Z",
        &[
            StepSpec::new("test", "false", vec![]).with_timeout(Duration::from_secs(5)),
            StepSpec::new(
                "lint",
                "printf",
                vec![
                    "%s\\n".to_string(),
                    r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable: `x`","spans":[{"file_name":"src/main.rs","is_primary":true,"line_start":2}]}}"#
                        .to_string(),
                ],
            )
            .with_parser(Parser::ClippyJson),
        ],
    );

    let instance = serde_json::to_value(&evidence).expect("evidência serializa");
    let validator = validator_for(&schema()).expect("schema compila");
    let errors: Vec<_> = validator.iter_errors(&instance).collect();
    assert!(
        errors.is_empty(),
        "evidência com findings reais não bateu o schema: {errors:?}"
    );
}

#[test]
fn evidencia_sem_passos_tambem_valida() {
    let evidence = run_pipeline("run-vazio", "cafebabe", "2026-07-05T00:00:00Z", &[]);
    let instance = serde_json::to_value(&evidence).expect("evidência serializa");
    let validator = validator_for(&schema()).expect("schema compila");
    assert!(validator.is_valid(&instance));
}

#[test]
fn documento_com_campo_obrigatorio_ausente_falha_o_schema() {
    // Prova que o teste acima não é um "sempre passa" — um documento
    // deliberadamente quebrado (sem `verdict`) precisa reprovar.
    let broken = serde_json::json!({
        "run_id": "x",
        "git_sha": "y",
        "steps": [],
        "produced_at": "2026-07-05T00:00:00Z"
    });
    let validator = validator_for(&schema()).expect("schema compila");
    assert!(!validator.is_valid(&broken));
}
