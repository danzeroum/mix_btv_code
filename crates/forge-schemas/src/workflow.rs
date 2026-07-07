//! Grafo do Squad Designer (`squad.workflow.v1`, Fase 7 Onda 14).
//!
//! Salvar valida a forma (schema + integridade de arestas) e grava no
//! ledger — **não aplica** ao orquestrador real: o `UnifiedOrchestrator`
//! continua com os 5 agentes fixos (`forge_squad`), sem reescrita nesta
//! fase. "Salvar honesto": o servidor confirma que o grafo foi validado e
//! persistido, nunca que o squad passou a usá-lo.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowNodeKind {
    Card,
    Pill,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowNodeParam {
    pub k: String,
    pub v: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub kind: WorkflowNodeKind,
    pub name: String,
    pub role: String,
    pub color: String,
    pub icon: String,
    pub sub: String,
    pub params: Vec<WorkflowNodeParam>,
    pub removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SquadWorkflow {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

impl SquadWorkflow {
    /// Única checagem semântica além do schema (campos/tipos obrigatórios já
    /// cobertos por serde + JSON Schema): toda aresta referencia um nó que
    /// existe. Erro aponta o lado (`from`/`to`) e o id que falhou — um 422
    /// claro, não um 500 genérico nem um grafo salvo silenciosamente
    /// quebrado.
    pub fn validate_edges(&self) -> Result<(), String> {
        let ids: std::collections::HashSet<&str> =
            self.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in &self.edges {
            if !ids.contains(edge.from.as_str()) {
                return Err(format!(
                    "aresta referencia nó inexistente em 'from': {}",
                    edge.from
                ));
            }
            if !ids.contains(edge.to.as_str()) {
                return Err(format!(
                    "aresta referencia nó inexistente em 'to': {}",
                    edge.to
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> WorkflowNode {
        WorkflowNode {
            id: id.into(),
            x: 0.0,
            y: 0.0,
            kind: WorkflowNodeKind::Card,
            name: id.into(),
            role: "agente".into(),
            color: "var(--rust)".into(),
            icon: "◆".into(),
            sub: "".into(),
            params: vec![],
            removable: true,
        }
    }

    #[test]
    fn grafo_com_arestas_validas_passa() {
        let wf = SquadWorkflow {
            nodes: vec![node("a"), node("b")],
            edges: vec![WorkflowEdge {
                from: "a".into(),
                to: "b".into(),
                label: None,
            }],
        };
        assert!(wf.validate_edges().is_ok());
    }

    #[test]
    fn aresta_para_no_inexistente_e_rejeitada_com_erro_claro() {
        let wf = SquadWorkflow {
            nodes: vec![node("a")],
            edges: vec![WorkflowEdge {
                from: "a".into(),
                to: "fantasma".into(),
                label: None,
            }],
        };
        let err = wf.validate_edges().unwrap_err();
        assert!(err.contains("fantasma"), "erro deveria citar o id: {err}");
    }
}
