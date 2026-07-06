//! Classificação de modelos em tiers (porta de
//! `opencode/packages/opencode/src/provider/model-tier.ts`, fork danzeroum).
//!
//! Modelos "small" recebem comportamento adaptado (prompt enxuto, menos
//! ferramentas, compaction antecipada, lembretes de step-discipline).
//! A lista é conservadora por filosofia: classificar um modelo capaz como
//! "small" degrada mais do que deixar um pequeno passar como "medium".
//! Um override manual por configuração cobre erros de classificação.

use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Small,
    Medium,
    Large,
}

struct TierRules {
    small: Vec<Regex>,
    /// Padrão "large" + exclusão opcional (substitui os lookaheads do
    /// original TS, que o crate `regex` não suporta).
    large: Vec<(Regex, Option<Regex>)>,
}

fn rules() -> &'static TierRules {
    static RULES: OnceLock<TierRules> = OnceLock::new();
    RULES.get_or_init(|| TierRules {
        small: vec![
            Regex::new(r"haiku").unwrap(),
            Regex::new(r"mini").unwrap(),
            Regex::new(r"flash").unwrap(),
            Regex::new(r"nano").unwrap(),
            Regex::new(r"lite").unwrap(),
            Regex::new(r"small").unwrap(),
            Regex::new(r"\b\d+b\b").unwrap(),
            Regex::new(r"3\.5-turbo").unwrap(),
        ],
        large: vec![
            (Regex::new(r"opus").unwrap(), None),
            (Regex::new(r"sonnet").unwrap(), None),
            (Regex::new(r"gpt-4\.1").unwrap(), None),
            (
                Regex::new(r"gpt-4o").unwrap(),
                Some(Regex::new(r"gpt-4o-mini").unwrap()),
            ),
            (
                Regex::new(r"gpt-5").unwrap(),
                Some(Regex::new(r"gpt-5-(?:mini|nano)").unwrap()),
            ),
            (Regex::new(r"gemini-[\d.]+-pro").unwrap(), None),
            (Regex::new(r"-(?:70|72|123|235|405)b\b").unwrap(), None),
        ],
    })
}

/// Classifica um id de modelo (case-insensitive). "large" é checado primeiro
/// para que, por exemplo, `gemini-2.5-pro` não seja lido como small por
/// conter "mini".
pub fn tier_from_id(model_id: &str) -> ModelTier {
    let id = model_id.to_lowercase();
    let rules = rules();
    for (pattern, exclusion) in &rules.large {
        if pattern.is_match(&id) && !exclusion.as_ref().is_some_and(|ex| ex.is_match(&id)) {
            return ModelTier::Large;
        }
    }
    if rules.small.iter().any(|re| re.is_match(&id)) {
        return ModelTier::Small;
    }
    ModelTier::Medium
}

impl ModelTier {
    /// Fração da janela de contexto em que a compaction dispara: modelos
    /// small compactam antecipadamente (~75%), demais no padrão (~90%).
    pub fn compaction_threshold(self) -> f64 {
        match self {
            ModelTier::Small => 0.75,
            _ => 0.90,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifica_smalls() {
        for id in [
            "claude-haiku-4-5",
            "gpt-4o-mini",
            "gemini-2.0-flash",
            "gpt-5-nano",
            "mistral-7b",
            "gpt-3.5-turbo",
        ] {
            assert_eq!(tier_from_id(id), ModelTier::Small, "{id}");
        }
    }

    #[test]
    fn classifica_larges() {
        for id in [
            "claude-opus-4-8",
            "claude-sonnet-5",
            "gpt-4.1",
            "gpt-4o",
            "gpt-5",
            "gemini-2.5-pro",
            "llama-3.1-405b",
        ] {
            assert_eq!(tier_from_id(id), ModelTier::Large, "{id}");
        }
    }

    #[test]
    fn desconhecidos_sao_medium() {
        for id in ["deepseek-chat", "qwen-max", "grok-4"] {
            assert_eq!(tier_from_id(id), ModelTier::Medium, "{id}");
        }
    }

    #[test]
    fn gemini_pro_nao_e_small_apesar_do_mini_no_nome() {
        assert_eq!(tier_from_id("gemini-2.5-pro"), ModelTier::Large);
    }

    #[test]
    fn small_compacta_antecipadamente() {
        assert_eq!(
            tier_from_id("claude-haiku-4-5").compaction_threshold(),
            0.75
        );
        assert_eq!(tier_from_id("deepseek-chat").compaction_threshold(), 0.90);
    }
}
