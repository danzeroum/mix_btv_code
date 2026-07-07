//! Gerador roteirizado (Fase 6 Onda 8): implementa o `Generator` **real** sem
//! chamar provider nenhum — devolve um turno canned, **sem API key**. É o
//! "generator scripted, sem key real" que o PLANO pede para o load-test (k6) e
//! os benches criterion do caminho do gateway.
//!
//! Antes isto só existia como test double dentro de `#[cfg(test)]` (ver
//! `forge-core/src/agent_loop.rs`); promovido a tipo público reusável para que
//! um binário/endpoint fora de teste possa construí-lo. Determinístico e
//! thread-safe, então serve a carga concorrente sem esgotar estado.
//!
//! Fase 7 Onda 1: `from_sequence` acrescenta sequenciamento (turno N na
//! N-ésima chamada) — o que faltava para roteirizar um cenário
//! `tool_use → Ask → end_turn` sem inventar um novo test double. `echo`/
//! `from_turn` continuam devolvendo sempre o mesmo turno (sequência de 1
//! elemento, índice sempre grampeado em 0) — comportamento inalterado para
//! quem já os usa (benches, `loadgen`, k6).

use crate::chat::{AssistantTurn, ContentBlock, GenerateRequest, StopReason, Usage};
use crate::gateway::{GatewayError, Generator};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Um gerador que devolve turnos pré-definidos em sequência — ideal para
/// carga/bench com 1 turno só (resposta determinística, sem rede, sem key) e
/// para roteirizar um cenário de vários passos em teste. Ao esgotar a
/// sequência, repete o último turno para sempre (nunca entra em pânico sob
/// chamadas além do roteiro).
pub struct ScriptedGenerator {
    turns: Mutex<Vec<AssistantTurn>>,
    index: AtomicUsize,
}

impl ScriptedGenerator {
    /// Gerador que ecoa um texto fixo como turno do assistente.
    pub fn echo(text: impl Into<String>) -> Self {
        Self::from_turn(AssistantTurn {
            content: vec![ContentBlock::Text { text: text.into() }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
            provider: "scripted".into(),
        })
    }

    /// Gerador que devolve um turno arbitrário (ex.: com tool_use), para cenários
    /// além do eco.
    pub fn from_turn(turn: AssistantTurn) -> Self {
        Self::from_sequence(vec![turn])
    }

    /// Gerador que devolve os turnos em sequência (turno N na N-ésima
    /// chamada); ao esgotar, repete o último turno para sempre.
    pub fn from_sequence(turns: Vec<AssistantTurn>) -> Self {
        assert!(
            !turns.is_empty(),
            "ScriptedGenerator precisa de ao menos 1 turno"
        );
        Self {
            turns: Mutex::new(turns),
            index: AtomicUsize::new(0),
        }
    }
}

impl Generator for ScriptedGenerator {
    async fn generate(
        &self,
        _req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, GatewayError> {
        // Espelha o caminho real: emite o texto pelo callback de streaming e
        // devolve o turno agregado.
        let turns = self
            .turns
            .lock()
            .expect("scripted generator mutex poisoned");
        let i = self
            .index
            .fetch_add(1, Ordering::Relaxed)
            .min(turns.len() - 1);
        let turn = turns[i].clone();
        on_delta(&turn.text());
        Ok(turn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_req() -> GenerateRequest {
        GenerateRequest {
            model: "scripted".into(),
            system: String::new(),
            messages: vec![],
            tools: vec![],
            max_tokens: 64,
            temperature: None,
        }
    }

    #[test]
    fn echo_devolve_o_texto_e_emite_delta() {
        let gen = ScriptedGenerator::echo("olá mundo");
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut deltas = String::new();
        let turn = rt.block_on(async {
            let mut sink = |d: &str| deltas.push_str(d);
            gen.generate(dummy_req(), &mut sink).await.unwrap()
        });
        assert_eq!(turn.text(), "olá mundo");
        assert_eq!(deltas, "olá mundo");
        assert_eq!(turn.provider, "scripted");
    }

    #[test]
    fn echo_e_reusavel_sem_esgotar() {
        // Mesmo gerador chamado várias vezes devolve sempre o mesmo turno — o
        // que a carga concorrente exige (não consome uma fila).
        let gen = ScriptedGenerator::echo("x");
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut noop = |_: &str| {};
            for _ in 0..3 {
                assert_eq!(
                    gen.generate(dummy_req(), &mut noop).await.unwrap().text(),
                    "x"
                );
            }
        });
    }
}
