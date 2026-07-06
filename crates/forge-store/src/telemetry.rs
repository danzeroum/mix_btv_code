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
}
