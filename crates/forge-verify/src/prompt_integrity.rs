//! Validador de integridade de contrato de prompt (Fase 3 BuildToValue).
//!
//! Complementar ao `prompt-cache-key.v1` (`forge-schemas::canonical`): o
//! cache-key responde "é o MESMO prompt?" (determinismo/dedup); este validador
//! responde "esse contrato é **seguro e completo** para rodar?" (campos
//! obrigatórios, regras de ética, piso de qualidade, padrão perigoso). São
//! ortogonais — a ideia é rodar isto no cache-miss, antes de executar.
//!
//! Porte do `PromptIntegrityValidator` do BuildToValueIDE, adaptado ao estilo
//! determinístico deste crate (zero-dep, sem regex — casamento de substring
//! minúsculo é suficiente e honesto para um pré-filtro). A severidade depende
//! do **modo** (tier de tenant): em `Vitrine` um padrão perigoso é aviso; em
//! `Enterprise` é erro que reprova. Fail-closed: contrato inválido não roda.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Modo/tier que decide a severidade de um padrão perigoso (mesmo eixo do
/// BuildToValueIDE): vitrine (demonstração) tolera com aviso; enterprise barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptMode {
    Vitrine,
    Enterprise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegrityIssue {
    /// Código estável para o frontend/telemetria (ex.: "missing_field").
    pub code: String,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// `true` só quando não há nenhum `Error` — o contrato pode rodar.
    pub valid: bool,
    /// Placar heurístico [0,1]: 1.0 − 0.1·(erros+avisos), com piso em 0.
    pub score: f64,
    pub issues: Vec<IntegrityIssue>,
}

/// Campos que todo contrato de prompt deve declarar (ausência = erro).
const REQUIRED_FIELDS: [&str; 4] = ["name", "version", "ethics_check", "quality_gates"];

/// Padrões perigosos (casamento de substring, minúsculo). Não é um controle de
/// segurança forte — é um pré-filtro barato contra injeção/comando destrutivo,
/// igual ao do BuildToValueIDE. A severidade final depende do modo.
const DANGEROUS_PATTERNS: [&str; 7] = [
    "rm -rf",
    "drop table",
    "eval(",
    "exec(",
    "__import__",
    "os.system",
    "subprocess",
];

/// Piso de qualidade abaixo do qual emitimos aviso (mesmo default do IDE).
const MIN_QUALITY_FLOOR: f64 = 0.7;

/// Valida um contrato de prompt (JSON) e produz um relatório com veredito.
pub fn validate_contract(contract: &Value, mode: PromptMode) -> IntegrityReport {
    let mut issues = Vec::new();

    // 1) Campos obrigatórios.
    for field in REQUIRED_FIELDS {
        if contract.get(field).is_none() {
            issues.push(IntegrityIssue {
                code: "missing_field".into(),
                message: format!("campo obrigatório ausente: {field}"),
                severity: Severity::Error,
            });
        }
    }

    // 2) Política de ética: regras no_pii/no_bias e ethics habilitada.
    if let Some(ethics) = contract.get("ethics_check") {
        if ethics.get("enabled") == Some(&Value::Bool(false)) {
            issues.push(IntegrityIssue {
                code: "ethics_disabled".into(),
                message: "ethics_check.enabled é false".into(),
                severity: Severity::Warning,
            });
        }
        let rules: Vec<String> = ethics
            .get("rules")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        for expected in ["no_pii", "no_bias"] {
            if !rules.iter().any(|r| r == expected) {
                issues.push(IntegrityIssue {
                    code: "ethics_rule_missing".into(),
                    message: format!("regra de ética recomendada ausente: {expected}"),
                    severity: Severity::Warning,
                });
            }
        }
    }

    // 3) Piso de qualidade.
    if let Some(min_score) = contract
        .get("quality_gates")
        .and_then(|q| q.get("min_score"))
        .and_then(|v| v.as_f64())
    {
        if min_score < MIN_QUALITY_FLOOR {
            issues.push(IntegrityIssue {
                code: "quality_floor".into(),
                message: format!(
                    "quality_gates.min_score {min_score} abaixo do piso {MIN_QUALITY_FLOOR}"
                ),
                severity: Severity::Warning,
            });
        }
    }

    // 4) Padrão perigoso no contrato inteiro serializado (minúsculo). No modo
    // enterprise vira erro (reprova); no vitrine, aviso.
    let haystack = contract.to_string().to_lowercase();
    let danger_sev = match mode {
        PromptMode::Enterprise => Severity::Error,
        PromptMode::Vitrine => Severity::Warning,
    };
    for pat in DANGEROUS_PATTERNS {
        if haystack.contains(pat) {
            issues.push(IntegrityIssue {
                code: "dangerous_pattern".into(),
                message: format!("padrão perigoso detectado: {pat:?}"),
                severity: danger_sev,
            });
        }
    }

    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let score = (1.0 - 0.1 * (issues.len() as f64)).max(0.0);
    IntegrityReport {
        valid: errors == 0,
        score,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn contrato_ok() -> Value {
        json!({
            "name": "resumo-editorial",
            "version": "1.0.0",
            "ethics_check": { "enabled": true, "rules": ["no_pii", "no_bias"] },
            "quality_gates": { "min_score": 0.85 }
        })
    }

    #[test]
    fn contrato_completo_e_seguro_e_valido() {
        let r = validate_contract(&contrato_ok(), PromptMode::Enterprise);
        assert!(r.valid);
        assert!(r.issues.is_empty());
        assert_eq!(r.score, 1.0);
    }

    #[test]
    fn campo_obrigatorio_ausente_reprova() {
        let mut c = contrato_ok();
        c.as_object_mut().unwrap().remove("version");
        let r = validate_contract(&c, PromptMode::Vitrine);
        assert!(!r.valid);
        assert!(r.issues.iter().any(|i| i.code == "missing_field"));
    }

    #[test]
    fn piso_de_qualidade_baixo_e_etica_incompleta_sao_avisos_nao_reprovam() {
        let c = json!({
            "name": "x", "version": "1", "quality_gates": { "min_score": 0.5 },
            "ethics_check": { "enabled": true, "rules": ["no_pii"] }
        });
        let r = validate_contract(&c, PromptMode::Vitrine);
        // avisos (quality_floor + no_bias ausente) mas nenhum erro → válido.
        assert!(r.valid);
        assert!(r.issues.iter().any(|i| i.code == "quality_floor"));
        assert!(r.issues.iter().any(|i| i.code == "ethics_rule_missing"));
        assert!(r.score < 1.0);
    }

    #[test]
    fn padrao_perigoso_reprova_em_enterprise_mas_so_avisa_em_vitrine() {
        let c = json!({
            "name": "x", "version": "1",
            "ethics_check": { "enabled": true, "rules": ["no_pii", "no_bias"] },
            "quality_gates": { "min_score": 0.9 },
            "template": "faça DROP TABLE users; e rode os.system('rm -rf /')"
        });
        let ent = validate_contract(&c, PromptMode::Enterprise);
        assert!(!ent.valid, "enterprise deve reprovar padrão perigoso");
        assert!(ent.issues.iter().any(|i| i.code == "dangerous_pattern"));

        let vit = validate_contract(&c, PromptMode::Vitrine);
        assert!(vit.valid, "vitrine tolera com aviso, não reprova");
        assert!(vit
            .issues
            .iter()
            .any(|i| i.code == "dangerous_pattern" && i.severity == Severity::Warning));
    }

    #[test]
    fn score_nunca_negativo() {
        let c =
            json!({ "template": "rm -rf drop table eval( exec( __import__ os.system subprocess" });
        let r = validate_contract(&c, PromptMode::Enterprise);
        assert!(r.score >= 0.0);
        assert!(!r.valid);
    }
}
