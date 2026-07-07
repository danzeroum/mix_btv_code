//! Motor de permissões por ferramenta e escopo (origem: opencode).
//!
//! Superfície de segurança: as decisões vivem no processo Rust e não são
//! contornáveis pelo sidecar Python — o squad pede permissão via
//! `CoreService.RequestPermission` e recebe a decisão pronta.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

/// Regra: decisão para uma ferramenta, opcionalmente restrita a um prefixo
/// de escopo (caminho de arquivo, comando, URL...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_prefix: Option<String>,
    pub decision: Decision,
}

/// Avalia regras na ordem: a primeira compatível vence; sem regra → `Ask`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionEngine {
    pub rules: Vec<Rule>,
}

impl PermissionEngine {
    pub fn evaluate(&self, tool: &str, scope: &str) -> Decision {
        for rule in &self.rules {
            if rule.tool != tool {
                continue;
            }
            match &rule.scope_prefix {
                Some(prefix) if !scope.starts_with(prefix.as_str()) => continue,
                _ => return rule.decision,
            }
        }
        Decision::Ask
    }

    /// Combina `overrides` (checadas primeiro, na ordem dada) com as regras
    /// desta engine — usado para que uma `Rule` persistida pelo usuário
    /// (matriz build/plan×tool ou "sempre" da ponte de permissão, Fase 7
    /// Onda 2) sempre vença o default do perfil, sem duplicar `evaluate`.
    pub fn overlay(&self, overrides: &[Rule]) -> Self {
        let mut rules = overrides.to_vec();
        rules.extend(self.rules.iter().cloned());
        Self { rules }
    }

    /// Perfil somente leitura (safe mode / agente `plan`): edits e bash
    /// negados ou sob pergunta, leitura liberada.
    pub fn read_only() -> Self {
        Self {
            rules: vec![
                Rule {
                    tool: "read".into(),
                    scope_prefix: None,
                    decision: Decision::Allow,
                },
                Rule {
                    tool: "grep".into(),
                    scope_prefix: None,
                    decision: Decision::Allow,
                },
                Rule {
                    tool: "edit".into(),
                    scope_prefix: None,
                    decision: Decision::Deny,
                },
                Rule {
                    tool: "bash".into(),
                    scope_prefix: None,
                    decision: Decision::Ask,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_regra_pergunta() {
        assert_eq!(
            PermissionEngine::default().evaluate("edit", "src/main.rs"),
            Decision::Ask
        );
    }

    #[test]
    fn escopo_por_prefixo() {
        let engine = PermissionEngine {
            rules: vec![
                Rule {
                    tool: "edit".into(),
                    scope_prefix: Some("src/".into()),
                    decision: Decision::Allow,
                },
                Rule {
                    tool: "edit".into(),
                    scope_prefix: None,
                    decision: Decision::Deny,
                },
            ],
        };
        assert_eq!(engine.evaluate("edit", "src/lib.rs"), Decision::Allow);
        assert_eq!(engine.evaluate("edit", "/etc/passwd"), Decision::Deny);
    }

    #[test]
    fn read_only_nega_edits() {
        let engine = PermissionEngine::read_only();
        assert_eq!(engine.evaluate("edit", "src/lib.rs"), Decision::Deny);
        assert_eq!(engine.evaluate("read", "src/lib.rs"), Decision::Allow);
        assert_eq!(engine.evaluate("bash", "cargo test"), Decision::Ask);
    }

    #[test]
    fn overlay_override_vence_default_do_perfil() {
        let base = PermissionEngine {
            rules: vec![Rule {
                tool: "bash".into(),
                scope_prefix: None,
                decision: Decision::Ask,
            }],
        };
        let overridden = base.overlay(&[Rule {
            tool: "bash".into(),
            scope_prefix: None,
            decision: Decision::Allow,
        }]);
        assert_eq!(overridden.evaluate("bash", "ls"), Decision::Allow);
        // Sem override para "edit", o default do perfil ainda vale.
        let base_edit = PermissionEngine {
            rules: vec![Rule {
                tool: "edit".into(),
                scope_prefix: None,
                decision: Decision::Deny,
            }],
        };
        assert_eq!(base_edit.overlay(&[]).evaluate("edit", "x"), Decision::Deny);
    }

    #[test]
    fn overlay_escopo_especifico_pode_conviver_com_default_generico() {
        // Override específico ("sempre" para um comando exato) não deve
        // vazar para outros escopos do mesmo tool — só o default genérico
        // (scope_prefix: None) do perfil se aplica fora do prefixo coberto.
        let base = PermissionEngine {
            rules: vec![Rule {
                tool: "bash".into(),
                scope_prefix: None,
                decision: Decision::Ask,
            }],
        };
        let overridden = base.overlay(&[Rule {
            tool: "bash".into(),
            scope_prefix: Some("npm test".into()),
            decision: Decision::Allow,
        }]);
        assert_eq!(
            overridden.evaluate("bash", "npm test --watch"),
            Decision::Allow
        );
        assert_eq!(overridden.evaluate("bash", "rm -rf /"), Decision::Ask);
    }
}
