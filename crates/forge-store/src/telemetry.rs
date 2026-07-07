//! Telemetria offline-first (origem: prompte): eventos gravados localmente
//! em SQLite (`telemetry-event.v1`), agregados sob demanda pelo dashboard
//! (`forge-server`). Nada sai da máquina do usuário.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub name: String,
    pub session_id: String,
    pub props: Value,
    pub ts: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetrySummary {
    pub total_events: u64,
    pub by_name: HashMap<String, u64>,
    /// `cache.hit / (cache.hit + cache.miss)`, ou `None` sem nenhuma chamada.
    pub cache_hit_rate: Option<f64>,
}

/// Contagens por modelo (Fase 7 Onda 7, A5) — `model` vem de `props.model`,
/// gravado por `RateLimitedGenerator`/`CachedGenerator` em todo `llm.call`/
/// `cache.hit`/`cache.miss` real. Tier é derivado por quem consome isto
/// (`forge_llm::model_tier::tier_from_id`) — `forge-store` não depende de
/// `forge-llm`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub calls: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

pub struct TelemetryStore {
    conn: Connection,
}

impl TelemetryStore {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS telemetry_event (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT NOT NULL,
                session_id TEXT NOT NULL,
                props      TEXT NOT NULL,
                ts         TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn record(
        &self,
        name: &str,
        session_id: &str,
        props: &Value,
        ts: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO telemetry_event (name, session_id, props, ts) VALUES (?1, ?2, ?3, ?4)",
            params![name, session_id, props.to_string(), ts],
        )?;
        Ok(())
    }

    pub fn recent(&self, limit: u32) -> rusqlite::Result<Vec<TelemetryRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, session_id, props, ts FROM telemetry_event ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            let props_text: String = row.get(2)?;
            Ok(TelemetryRecord {
                name: row.get(0)?,
                session_id: row.get(1)?,
                props: serde_json::from_str(&props_text).unwrap_or(Value::Null),
                ts: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn summary(&self) -> rusqlite::Result<TelemetrySummary> {
        let mut by_name = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT name, COUNT(*) FROM telemetry_event GROUP BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;
        let mut total = 0u64;
        for row in rows {
            let (name, count) = row?;
            total += count;
            by_name.insert(name, count);
        }
        let hits = *by_name.get("cache.hit").unwrap_or(&0);
        let misses = *by_name.get("cache.miss").unwrap_or(&0);
        let cache_hit_rate = if hits + misses > 0 {
            Some(hits as f64 / (hits + misses) as f64)
        } else {
            None
        };
        Ok(TelemetrySummary {
            total_events: total,
            by_name,
            cache_hit_rate,
        })
    }

    /// Agrega os eventos de um experimento A/B por variante (Fase 6 Onda 7):
    /// devolve `(variante, n, sucessos)` por variante. Um evento pertence ao
    /// experimento se `props.experiment` bate; é atribuído por `props.variant`;
    /// conta como sucesso se `props.success` é verdadeiro (JSON `true`/`1`).
    /// Usa a extensão JSON1 do SQLite (bundled) — `summary` só agrupa por nome,
    /// isto é a consulta nova que o A/B exige.
    pub fn experiment_variants(
        &self,
        experiment: &str,
    ) -> rusqlite::Result<Vec<(String, u64, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT json_extract(props, '$.variant') AS variant,
                    COUNT(*) AS n,
                    SUM(CASE WHEN json_extract(props, '$.success') = 1 THEN 1 ELSE 0 END) AS successes
             FROM telemetry_event
             WHERE json_extract(props, '$.experiment') = ?1
               AND json_extract(props, '$.variant') IS NOT NULL
             GROUP BY variant
             ORDER BY variant",
        )?;
        let rows = stmt.query_map(params![experiment], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, u64>(2)?,
            ))
        })?;
        rows.collect()
    }

    /// Agrega `llm.call`/`cache.hit`/`cache.miss` por `props.model` (Fase 7
    /// Onda 7, A5) — mesmo padrão de `experiment_variants`: um `CASE WHEN`
    /// por coluna na mesma consulta, não três `SELECT`s separados.
    pub fn model_usage(&self) -> rusqlite::Result<Vec<ModelUsage>> {
        let mut stmt = self.conn.prepare(
            "SELECT json_extract(props, '$.model') AS model,
                    SUM(CASE WHEN name = 'llm.call' THEN 1 ELSE 0 END) AS calls,
                    SUM(CASE WHEN name = 'cache.hit' THEN 1 ELSE 0 END) AS cache_hits,
                    SUM(CASE WHEN name = 'cache.miss' THEN 1 ELSE 0 END) AS cache_misses
             FROM telemetry_event
             WHERE json_extract(props, '$.model') IS NOT NULL
             GROUP BY model
             ORDER BY model",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ModelUsage {
                model: row.get(0)?,
                calls: row.get(1)?,
                cache_hits: row.get(2)?,
                cache_misses: row.get(3)?,
            })
        })?;
        rows.collect()
    }
}

/// Handle cloneável e thread-safe sobre um [`TelemetryStore`] — decoradores
/// do gateway (cache, rate limit) e o dashboard compartilham a mesma
/// conexão sem precisar saber de SQLite.
#[derive(Clone)]
pub struct Telemetry(Arc<Mutex<TelemetryStore>>);

impl Telemetry {
    pub fn new(store: TelemetryStore) -> Self {
        Self(Arc::new(Mutex::new(store)))
    }

    pub fn open(path: &str) -> rusqlite::Result<Self> {
        Ok(Self::new(TelemetryStore::open(path)?))
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Ok(Self::new(TelemetryStore::open_in_memory()?))
    }

    /// Falhas de telemetria nunca devem quebrar o caminho principal —
    /// registradas em stderr e descartadas.
    pub fn record(&self, name: &str, session_id: &str, props: Value, ts: &str) {
        if let Err(e) = self
            .0
            .lock()
            .expect("telemetry mutex poisoned")
            .record(name, session_id, &props, ts)
        {
            eprintln!("  [telemetria] falha ao registrar {name}: {e}");
        }
    }

    pub fn recent(&self, limit: u32) -> Vec<TelemetryRecord> {
        self.0
            .lock()
            .expect("telemetry mutex poisoned")
            .recent(limit)
            .unwrap_or_default()
    }

    pub fn summary(&self) -> TelemetrySummary {
        self.0
            .lock()
            .expect("telemetry mutex poisoned")
            .summary()
            .unwrap_or_default()
    }

    /// `(variante, n, sucessos)` por variante do experimento (Fase 6 Onda 7).
    /// Vazio em falha — como o resto do handle, telemetria não quebra o caminho.
    pub fn experiment_variants(&self, experiment: &str) -> Vec<(String, u64, u64)> {
        self.0
            .lock()
            .expect("telemetry mutex poisoned")
            .experiment_variants(experiment)
            .unwrap_or_default()
    }

    /// Contagens por modelo (Fase 7 Onda 7, A5). Vazio em falha, mesmo padrão
    /// do resto do handle.
    pub fn model_usage(&self) -> Vec<ModelUsage> {
        self.0
            .lock()
            .expect("telemetry mutex poisoned")
            .model_usage()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn record_e_recent_preservam_ordem_mais_recente_primeiro() {
        let store = TelemetryStore::open_in_memory().unwrap();
        store
            .record("llm.call", "s1", &json!({"provider": "anthropic"}), "t1")
            .unwrap();
        store.record("cache.hit", "s1", &json!({}), "t2").unwrap();
        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "cache.hit");
        assert_eq!(recent[1].name, "llm.call");
    }

    #[test]
    fn summary_agrega_por_nome_e_calcula_cache_hit_rate() {
        let store = TelemetryStore::open_in_memory().unwrap();
        for _ in 0..3 {
            store.record("cache.hit", "s1", &json!({}), "t").unwrap();
        }
        store.record("cache.miss", "s1", &json!({}), "t").unwrap();
        store.record("llm.call", "s1", &json!({}), "t").unwrap();

        let summary = store.summary().unwrap();
        assert_eq!(summary.total_events, 5);
        assert_eq!(summary.by_name["cache.hit"], 3);
        assert_eq!(summary.cache_hit_rate, Some(0.75));
    }

    #[test]
    fn summary_sem_chamadas_de_cache_nao_calcula_taxa() {
        let store = TelemetryStore::open_in_memory().unwrap();
        store.record("tool.run", "s1", &json!({}), "t").unwrap();
        assert_eq!(store.summary().unwrap().cache_hit_rate, None);
    }

    #[test]
    fn experiment_variants_agrega_por_variante_e_conta_sucessos() {
        let store = TelemetryStore::open_in_memory().unwrap();
        // Experimento "x": variante A com 2 sucessos em 3; B com 0 em 2.
        for success in [true, true, false] {
            store
                .record(
                    "llm.call",
                    "s",
                    &json!({"experiment": "x", "variant": "A", "success": success}),
                    "t",
                )
                .unwrap();
        }
        for _ in 0..2 {
            store
                .record(
                    "llm.call",
                    "s",
                    &json!({"experiment": "x", "variant": "B", "success": false}),
                    "t",
                )
                .unwrap();
        }
        // Ruído: outro experimento e um evento sem variante — não devem contar.
        store
            .record(
                "llm.call",
                "s",
                &json!({"experiment": "outro", "variant": "A", "success": true}),
                "t",
            )
            .unwrap();
        store.record("cache.hit", "s", &json!({}), "t").unwrap();

        let variants = store.experiment_variants("x").unwrap();
        assert_eq!(
            variants,
            vec![("A".to_string(), 3, 2), ("B".to_string(), 2, 0),]
        );
    }

    #[test]
    fn handle_telemetry_e_clonavel_e_compartilha_o_mesmo_store() {
        let telemetry = Telemetry::open_in_memory().unwrap();
        let clone = telemetry.clone();
        telemetry.record("llm.call", "s1", json!({}), "t");
        clone.record("cache.hit", "s1", json!({}), "t");
        assert_eq!(telemetry.summary().total_events, 2);
    }

    /// Fase 7 Onda 7 (A5): `model_usage` agrega os 3 nomes reais que
    /// `RateLimitedGenerator`/`CachedGenerator` gravam com `props.model`,
    /// por modelo — inclui um evento sem `model` (ex.: `cache.hit` de outro
    /// caminho) e um evento de nome irrelevante, nenhum dos dois deve poluir
    /// a agregação.
    #[test]
    fn model_usage_agrega_llm_call_e_cache_hit_miss_por_modelo() {
        let store = TelemetryStore::open_in_memory().unwrap();
        for _ in 0..3 {
            store
                .record("llm.call", "s", &json!({"model": "claude-sonnet-5"}), "t")
                .unwrap();
        }
        store
            .record("cache.hit", "s", &json!({"model": "claude-sonnet-5"}), "t")
            .unwrap();
        store
            .record("cache.miss", "s", &json!({"model": "claude-sonnet-5"}), "t")
            .unwrap();
        store
            .record("llm.call", "s", &json!({"model": "claude-haiku-4-5"}), "t")
            .unwrap();
        // Ruído: sem `model` e um nome de evento não contado — não devem aparecer.
        store.record("cache.hit", "s", &json!({}), "t").unwrap();
        store
            .record(
                "session.start",
                "s",
                &json!({"model": "claude-sonnet-5"}),
                "t",
            )
            .unwrap();

        let usage = store.model_usage().unwrap();
        assert_eq!(
            usage,
            vec![
                ModelUsage {
                    model: "claude-haiku-4-5".into(),
                    calls: 1,
                    cache_hits: 0,
                    cache_misses: 0,
                },
                ModelUsage {
                    model: "claude-sonnet-5".into(),
                    calls: 3,
                    cache_hits: 1,
                    cache_misses: 1,
                },
            ]
        );
    }
}
