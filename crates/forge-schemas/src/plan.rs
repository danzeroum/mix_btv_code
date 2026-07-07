//! Manifesto de plano/entrega da esteira (`plan.v1`, Fase 2 BuildToValue).
//!
//! O plano é o "ticket de trabalho" declarativo que a esteira executa: fases →
//! entregas (arquivos exportáveis: DOCX/XLSX/PDF/MusicXML) → quality gates por
//! fase → critérios de sucesso → orçamento → rollback. É a ponte entre o grafo
//! de squad (`squad.workflow.v1`, a fiação) e os artefatos exportáveis (o
//! produto). Contrato canônico em `schemas/json/plan.v1.schema.json`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Prerequisites {
    #[serde(default)]
    pub contracts: Vec<String>,
    #[serde(default)]
    pub approvals: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlanPhase {
    /// Ordem na esteira (1-based, única e sequencial — ver `Plan::validate`).
    pub order: u32,
    /// Papel/persona responsável pela fase.
    pub primary_role: String,
    #[serde(default)]
    pub support_roles: Vec<String>,
    /// Artefatos exportáveis produzidos nesta fase (ex.: "pauta.md",
    /// "partitura.musicxml").
    #[serde(default)]
    pub deliverables: Vec<String>,
    /// Se a fase abre um gate humano (o membro humano aprova antes de seguir).
    pub approval_required: bool,
    /// Confiança estimada [0,1] da fase.
    pub estimated_confidence: f64,
    /// Gates de qualidade da fase (ex.: "revisao:pass", "test-coverage:85%").
    #[serde(default)]
    pub quality_gates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct SuccessCriteria {
    #[serde(default)]
    pub functional: Vec<String>,
    #[serde(default)]
    pub non_functional: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Budget {
    /// Custo estimado (na moeda/telemetria da plataforma).
    pub estimated_cost: f64,
    /// Teto de chamadas de LLM para o plano.
    pub max_llm_calls: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct RollbackStrategy {
    /// Se um kill-switch aborta a esteira (Fase 3 amarra ao `operational_status`).
    pub kill_switch: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    #[serde(default)]
    pub prerequisites: Prerequisites,
    pub execution_sequence: Vec<PlanPhase>,
    #[serde(default)]
    pub success_criteria: SuccessCriteria,
    pub budget: Budget,
    #[serde(default)]
    pub rollback_strategy: RollbackStrategy,
}

impl Plan {
    /// Checagens semânticas além do schema:
    /// - a sequência não é vazia (um plano sem fases não produz entrega);
    /// - `order` é 1..=N única e sequencial (esteira sem buracos/duplicatas);
    /// - cada `estimated_confidence` está em [0,1].
    ///
    /// Erro claro (um 422) em vez de um plano silenciosamente quebrado.
    pub fn validate(&self) -> Result<(), String> {
        if self.execution_sequence.is_empty() {
            return Err("execution_sequence vazia — um plano precisa de ao menos uma fase".into());
        }
        let mut orders: Vec<u32> = self.execution_sequence.iter().map(|p| p.order).collect();
        orders.sort_unstable();
        for (i, order) in orders.iter().enumerate() {
            let expected = (i as u32) + 1;
            if *order != expected {
                return Err(format!(
                    "orders da esteira devem ser 1..={} únicas e sequenciais; achei {order} onde esperava {expected}",
                    self.execution_sequence.len()
                ));
            }
        }
        for phase in &self.execution_sequence {
            if !(0.0..=1.0).contains(&phase.estimated_confidence) {
                return Err(format!(
                    "estimated_confidence fora de [0,1] na fase {}: {}",
                    phase.order, phase.estimated_confidence
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase(order: u32) -> PlanPhase {
        PlanPhase {
            order,
            primary_role: "pauteiro".into(),
            support_roles: vec![],
            deliverables: vec!["pauta.md".into()],
            approval_required: true,
            estimated_confidence: 0.8,
            quality_gates: vec!["revisao:pass".into()],
        }
    }

    fn plan_of(orders: &[u32]) -> Plan {
        Plan {
            prerequisites: Prerequisites::default(),
            execution_sequence: orders.iter().map(|o| phase(*o)).collect(),
            success_criteria: SuccessCriteria::default(),
            budget: Budget {
                estimated_cost: 0.0,
                max_llm_calls: 20,
            },
            rollback_strategy: RollbackStrategy { kill_switch: true },
        }
    }

    #[test]
    fn aceita_esteira_sequencial() {
        assert!(plan_of(&[1, 2, 3]).validate().is_ok());
    }

    #[test]
    fn rejeita_esteira_vazia_e_com_buraco() {
        assert!(plan_of(&[]).validate().is_err());
        assert!(plan_of(&[1, 3]).validate().is_err());
        assert!(plan_of(&[1, 1]).validate().is_err());
    }

    #[test]
    fn rejeita_confianca_fora_de_intervalo() {
        let mut p = plan_of(&[1]);
        p.execution_sequence[0].estimated_confidence = 2.0;
        assert!(p.validate().is_err());
    }
}
