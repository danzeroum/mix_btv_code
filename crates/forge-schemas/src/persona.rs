//! Persona de squad como **conteúdo** (`persona.v1`, Fase 2 BuildToValue).
//!
//! Uma persona é um item de galeria: identidade + modelos mentais (referências
//! canônicas da área — "Clean Architecture" para um arquiteto, "Bluebook" para
//! um paralegal), princípios, escada de autonomia (DESCRITIVA — rótulo
//! consultável, não um loop automático; ADR 0021), gatilhos de ativação e
//! contratos de comunicação (handoff). "Squads são conteúdo, não código": o
//! admin publica uma persona e ela aparece na galeria — nenhuma recompilação.
//!
//! O contrato canônico vive em `schemas/json/persona.v1.schema.json`; este tipo
//! deve permanecer compatível (teste em `tests/schema_fixtures.rs`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Nível de autonomia DESCRITIVO (rótulo). Não dispara promoção/rebaixamento
/// automático — é metadado que o humano consulta/sobrepõe (ADR 0021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AutonomyLabel {
    L1,
    L2,
    L3,
    L4,
    L5,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MentalModel {
    /// Referência canônica da área (livro/autor/norma).
    pub reference: String,
    /// Quando aplicar essa referência.
    pub apply_when: String,
}

/// Severidade de um princípio quando violado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrincipleSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CorePrinciple {
    pub id: String,
    pub description: String,
    /// Como validar (ex.: "manual", "static_analysis", "code_review").
    pub validation: String,
    pub severity: PrincipleSeverity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Autonomy {
    pub level: AutonomyLabel,
    #[serde(default)]
    pub can_decide_alone: Vec<String>,
    #[serde(default)]
    pub requires_approval: Vec<String>,
    /// Se o papel pode vetar uma entrega (ex.: um Auditor/Segurança).
    pub can_veto: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActivationTriggers {
    #[serde(default)]
    pub semantic_patterns: Vec<String>,
    #[serde(default)]
    pub context_keywords: Vec<String>,
    /// Limiar de confiança [0,1] para a persona "acender" numa tarefa.
    pub confidence_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Communication {
    #[serde(default)]
    pub receives_from: Vec<String>,
    #[serde(default)]
    pub delivers_to: Vec<String>,
    /// Descrição do contrato de handoff (o que entrega e com que critério).
    pub handoff_contract: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Persona {
    pub id: String,
    pub display_name: String,
    /// Domínio/profissão (ex.: "editorial", "juridico", "musica", "software").
    pub domain: String,
    #[serde(default)]
    pub mental_models: Vec<MentalModel>,
    #[serde(default)]
    pub core_principles: Vec<CorePrinciple>,
    pub autonomy: Autonomy,
    pub activation_triggers: ActivationTriggers,
    pub communication: Communication,
    /// Formatos exportáveis que esta persona entrega (DOCX, MusicXML, PDF…).
    #[serde(default)]
    pub delivery_formats: Vec<String>,
}

impl Persona {
    /// Checagem semântica além do schema: o limiar de confiança tem que estar
    /// em [0,1] (um limiar fora disso nunca acenderia ou sempre acenderia — um
    /// erro silencioso de galeria). Erro claro em vez de persona quebrada.
    pub fn validate(&self) -> Result<(), String> {
        let t = self.activation_triggers.confidence_threshold;
        if !(0.0..=1.0).contains(&t) {
            return Err(format!(
                "confidence_threshold fora de [0,1]: {t} (persona {})",
                self.id
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona_min() -> Persona {
        Persona {
            id: "revisor-de-estilo".into(),
            display_name: "Revisor de estilo".into(),
            domain: "editorial".into(),
            mental_models: vec![],
            core_principles: vec![],
            autonomy: Autonomy {
                level: AutonomyLabel::L3,
                can_decide_alone: vec![],
                requires_approval: vec![],
                can_veto: false,
            },
            activation_triggers: ActivationTriggers {
                semantic_patterns: vec![],
                context_keywords: vec![],
                confidence_threshold: 0.6,
            },
            communication: Communication {
                receives_from: vec![],
                delivers_to: vec![],
                handoff_contract: "artigo.md + notas".into(),
            },
            delivery_formats: vec![],
        }
    }

    #[test]
    fn valida_limiar_de_confianca() {
        assert!(persona_min().validate().is_ok());
        let mut p = persona_min();
        p.activation_triggers.confidence_threshold = 1.5;
        assert!(p.validate().is_err());
    }
}
