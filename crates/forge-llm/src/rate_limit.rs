//! Rate limiting por janela deslizante, tier-gated (origem: prompte —
//! limites diferentes para anônimo (15) vs autenticado (60) por 10 min).
//!
//! O Forge não tem usuários/autenticação, então o eixo de tier vira o
//! `ModelTier` do próprio request: modelos small (baratos, rápidos) têm
//! um teto mais generoso; modelos large (caros) são mais conservadores —
//! uma salvaguarda de custo, não uma defesa contra abuso multiusuário.

use crate::model_tier::ModelTier;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, thiserror::Error)]
#[error(
    "limite de {max_requests} chamadas por {window:?} excedido; esperaria mais de {max_wait:?}"
)]
pub struct RateLimitError {
    pub max_requests: usize,
    pub window: Duration,
    pub max_wait: Duration,
}

/// Limitador de janela deslizante: no máximo `max_requests` chamadas a
/// cada `window`. Quando o teto é atingido, `acquire` espera até liberar
/// uma vaga, mas nunca mais que `max_wait` — depois disso, erro (rede de
/// segurança contra travar o CLI indefinidamente).
pub struct RateLimiter {
    max_requests: usize,
    window: Duration,
    max_wait: Duration,
    timestamps: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration, max_wait: Duration) -> Self {
        Self {
            max_requests,
            window,
            max_wait,
            timestamps: Mutex::new(VecDeque::new()),
        }
    }

    /// Limites default por tier — mais generoso para modelos small
    /// (baratos), mais conservador para large (caros).
    pub fn for_tier(tier: ModelTier) -> Self {
        let (max_requests, window) = match tier {
            ModelTier::Small => (60, Duration::from_secs(600)),
            ModelTier::Medium => (30, Duration::from_secs(600)),
            ModelTier::Large => (15, Duration::from_secs(600)),
        };
        Self::new(max_requests, window, window)
    }

    /// Remove timestamps fora da janela e devolve quanto falta esperar
    /// para haver vaga (`None` se já há vaga agora).
    fn poll(&self) -> Result<Option<Duration>, RateLimitError> {
        let now = Instant::now();
        let mut ts = self.timestamps.lock().expect("rate limiter mutex poisoned");
        while let Some(&front) = ts.front() {
            if now.duration_since(front) >= self.window {
                ts.pop_front();
            } else {
                break;
            }
        }
        if ts.len() < self.max_requests {
            ts.push_back(now);
            return Ok(None);
        }
        let wait = self.window - now.duration_since(*ts.front().expect("cheio implica não vazio"));
        if wait > self.max_wait {
            return Err(RateLimitError {
                max_requests: self.max_requests,
                window: self.window,
                max_wait: self.max_wait,
            });
        }
        Ok(Some(wait))
    }

    /// Espera até haver uma vaga (ou erra se a espera excederia `max_wait`).
    pub async fn acquire(&self) -> Result<(), RateLimitError> {
        loop {
            match self.poll()? {
                None => return Ok(()),
                Some(wait) => tokio::time::sleep(wait).await,
            }
        }
    }

    /// Teto de chamadas por janela (Fase 7 Onda 10, A4) — leitura pura da
    /// configuração, não do estado. Getter, não campo público: `timestamps`
    /// continua encapsulado.
    pub fn max_requests(&self) -> usize {
        self.max_requests
    }

    /// Duração da janela (Fase 7 Onda 10, A4).
    pub fn window(&self) -> Duration {
        self.window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dentro_do_limite_nao_espera() {
        let limiter = RateLimiter::new(2, Duration::from_secs(10), Duration::from_secs(10));
        limiter.acquire().await.unwrap();
        limiter.acquire().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn acima_do_limite_espera_a_janela_liberar() {
        let limiter = RateLimiter::new(1, Duration::from_millis(100), Duration::from_secs(5));
        limiter.acquire().await.unwrap();

        let start = Instant::now();
        limiter.acquire().await.unwrap(); // deve esperar ~100ms (tempo pausado, avança sozinho)
        assert!(Instant::now().duration_since(start) >= Duration::from_millis(100));
    }

    #[tokio::test(start_paused = true)]
    async fn espera_maior_que_max_wait_e_erro() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60), Duration::from_millis(10));
        limiter.acquire().await.unwrap();
        let err = limiter.acquire().await.unwrap_err();
        assert_eq!(err.max_requests, 1);
    }

    #[test]
    fn defaults_por_tier_sao_mais_conservadores_para_modelos_caros() {
        let small = RateLimiter::for_tier(ModelTier::Small);
        let large = RateLimiter::for_tier(ModelTier::Large);
        assert!(small.max_requests > large.max_requests);
    }

    #[test]
    fn getters_expoem_a_config_sem_expor_o_estado() {
        let limiter = RateLimiter::new(42, Duration::from_secs(99), Duration::from_secs(99));
        assert_eq!(limiter.max_requests(), 42);
        assert_eq!(limiter.window(), Duration::from_secs(99));
    }
}
