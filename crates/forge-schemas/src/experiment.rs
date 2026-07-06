//! Relatório de A/B testing sobre a telemetria (`experiment.v1`, Fase 6 Onda 7).
//!
//! Um experimento atribui eventos de telemetria a variantes (pelas chaves
//! `props.experiment`, `props.variant` e `props.success`); este módulo compara a
//! **taxa de sucesso** de duas variantes com um **teste z de duas proporções** e
//! deriva o veredito
//! **dos dados**: `Significant` (com vencedor) só quando p < α; senão
//! `Inconclusive` ("sem significância" — nunca inventa vencedor); e
//! `InsufficientData` quando a amostra é pequena demais para o teste ser
//! confiável. Mesma postura do `verification::derive_verdict` (veredito honesto
//! derivado, não fabricado) e da régua "Nada Fake" aplicada a estatística.
//!
//! Sem crate de estatística no workspace — o teste é hand-rolled em Rust puro
//! (CDF normal via aproximação de `erf` de Abramowitz-Stegun 7.1.26, |erro| ≤
//! 1.5e-7), suficiente para um p-valor de decisão.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Nível de significância (5%) e amostra mínima por variante abaixo da qual o
/// teste z (aproximação normal) não é confiável e o veredito é `InsufficientData`.
pub const ALPHA: f64 = 0.05;
pub const MIN_SAMPLES: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentVerdict {
    /// Diferença estatisticamente significativa (p < α) — há vencedor.
    Significant,
    /// Amostra suficiente, mas sem diferença significativa — **sem vencedor**.
    Inconclusive,
    /// Amostra pequena demais para o teste ser confiável — não se conclui.
    InsufficientData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VariantStats {
    pub variant: String,
    /// Tamanho da amostra (eventos atribuídos à variante).
    pub n: u64,
    /// Quantos foram sucesso.
    pub successes: u64,
    /// `successes / n` (0 quando `n == 0`).
    pub rate: f64,
}

impl VariantStats {
    pub fn new(variant: impl Into<String>, n: u64, successes: u64) -> Self {
        let rate = if n > 0 {
            successes as f64 / n as f64
        } else {
            0.0
        };
        Self {
            variant: variant.into(),
            n,
            successes,
            rate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExperimentReport {
    pub experiment: String,
    /// A métrica comparada (hoje: `success_rate`).
    pub metric: String,
    pub variants: Vec<VariantStats>,
    pub verdict: ExperimentVerdict,
    /// A variante vencedora — `Some` **apenas** quando `verdict == Significant`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner: Option<String>,
    /// p-valor bicaudal do teste z de duas proporções.
    pub p_value: f64,
    pub produced_at: String,
}

impl ExperimentReport {
    /// Constrói o relatório a partir de exatamente duas variantes. Veredito
    /// derivado dos dados: `InsufficientData` se alguma variante tem menos que
    /// [`MIN_SAMPLES`]; senão o teste z decide `Significant` (vencedor = maior
    /// taxa) ou `Inconclusive`. **Nunca** devolve vencedor sem significância.
    pub fn from_two_variants(
        experiment: impl Into<String>,
        metric: impl Into<String>,
        a: VariantStats,
        b: VariantStats,
        produced_at: impl Into<String>,
    ) -> Self {
        let (verdict, winner, p_value) = if a.n < MIN_SAMPLES || b.n < MIN_SAMPLES {
            (ExperimentVerdict::InsufficientData, None, 1.0)
        } else {
            let p = two_proportion_p_value(a.successes, a.n, b.successes, b.n);
            if p < ALPHA {
                let winner = if a.rate >= b.rate {
                    a.variant.clone()
                } else {
                    b.variant.clone()
                };
                (ExperimentVerdict::Significant, Some(winner), p)
            } else {
                (ExperimentVerdict::Inconclusive, None, p)
            }
        };
        Self {
            experiment: experiment.into(),
            metric: metric.into(),
            variants: vec![a, b],
            verdict,
            winner,
            p_value,
            produced_at: produced_at.into(),
        }
    }
}

/// p-valor bicaudal do teste z de duas proporções (variância *pooled*).
/// Devolve `1.0` (sem evidência de diferença) quando não há amostra ou não há
/// variância (todas as observações iguais).
pub fn two_proportion_p_value(x1: u64, n1: u64, x2: u64, n2: u64) -> f64 {
    if n1 == 0 || n2 == 0 {
        return 1.0;
    }
    let (x1, n1, x2, n2) = (x1 as f64, n1 as f64, x2 as f64, n2 as f64);
    let p1 = x1 / n1;
    let p2 = x2 / n2;
    let p_pool = (x1 + x2) / (n1 + n2);
    let se = (p_pool * (1.0 - p_pool) * (1.0 / n1 + 1.0 / n2)).sqrt();
    if se == 0.0 {
        return 1.0;
    }
    let z = (p1 - p2) / se;
    (2.0 * (1.0 - normal_cdf(z.abs()))).clamp(0.0, 1.0)
}

/// CDF da normal padrão via `erf`.
fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

/// `erf` por Abramowitz-Stegun 7.1.26 (|erro| ≤ 1.5e-7) — evita puxar um crate
/// de estatística só para isto.
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erf_bate_valores_conhecidos() {
        assert!((erf(0.0)).abs() < 1e-6);
        assert!((erf(1.0) - 0.8427007).abs() < 1e-5);
        assert!((erf(-1.0) + 0.8427007).abs() < 1e-5);
        // normal_cdf(0) = 0.5; normal_cdf(1.96) ≈ 0.975
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-3);
    }

    #[test]
    fn diferenca_grande_e_significativa() {
        // 90/100 vs 50/100 — diferença enorme, p-valor minúsculo.
        let p = two_proportion_p_value(90, 100, 50, 100);
        assert!(p < 0.001, "esperava p muito pequeno, veio {p}");
    }

    #[test]
    fn diferenca_minima_nao_e_significativa() {
        // 50/100 vs 52/100 — ruído; p-valor alto.
        let p = two_proportion_p_value(50, 100, 52, 100);
        assert!(p > 0.05, "esperava p alto (sem significância), veio {p}");
    }

    #[test]
    fn proporcoes_iguais_dao_p_um() {
        // Taxas iguais → z = 0 → p ≈ 1. A aproximação de erf (A&S 7.1.26) tem
        // erro ≤ 1.5e-7, então erf(0) ≈ 1e-9 (não exato) — folga de 1e-6.
        assert!((two_proportion_p_value(10, 20, 10, 20) - 1.0).abs() < 1e-6);
        // Sem variância (todos sucesso) → early-return exato 1.0, não NaN.
        assert_eq!(two_proportion_p_value(20, 20, 20, 20), 1.0);
    }

    #[test]
    fn relatorio_significativo_tem_vencedor() {
        let a = VariantStats::new("A", 100, 90);
        let b = VariantStats::new("B", 100, 50);
        let r = ExperimentReport::from_two_variants("exp", "success_rate", a, b, "t");
        assert_eq!(r.verdict, ExperimentVerdict::Significant);
        assert_eq!(r.winner.as_deref(), Some("A"));
        assert!(r.p_value < 0.001);
    }

    #[test]
    fn relatorio_sem_diferenca_e_inconclusive_sem_vencedor() {
        // A régua Nada Fake: variantes empatadas não inventam vencedor.
        let a = VariantStats::new("A", 100, 50);
        let b = VariantStats::new("B", 100, 52);
        let r = ExperimentReport::from_two_variants("exp", "success_rate", a, b, "t");
        assert_eq!(r.verdict, ExperimentVerdict::Inconclusive);
        assert!(r.winner.is_none(), "sem significância, sem vencedor");
    }

    #[test]
    fn amostra_pequena_e_insuficiente() {
        let a = VariantStats::new("A", 5, 5);
        let b = VariantStats::new("B", 5, 0);
        let r = ExperimentReport::from_two_variants("exp", "success_rate", a, b, "t");
        assert_eq!(r.verdict, ExperimentVerdict::InsufficientData);
        assert!(r.winner.is_none());
    }

    #[test]
    fn variant_stats_calcula_taxa() {
        assert_eq!(VariantStats::new("A", 4, 1).rate, 0.25);
        assert_eq!(VariantStats::new("A", 0, 0).rate, 0.0);
    }
}
