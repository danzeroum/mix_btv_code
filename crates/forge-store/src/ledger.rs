//! Ledger append-only com hash-chain (governança BuildToValue).
//!
//! Cada entrada referencia o hash da anterior; `verify_chain` detecta
//! qualquer adulteração retroativa. Não há UPDATE nem DELETE — apenas
//! INSERT (overrides são novas entradas marcadas).

use forge_schemas::ledger::LedgerEntry;
use rusqlite::{params, Connection};

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("erro de serialização: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("cadeia corrompida na seq {seq}: esperado {expected}, encontrado {found}")]
    BrokenChain {
        seq: u64,
        expected: String,
        found: String,
    },
}

pub struct LedgerStore {
    conn: Connection,
}

impl LedgerStore {
    pub fn open(path: &str) -> Result<Self, LedgerError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, LedgerError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, LedgerError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS ledger (
                seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                prev_hash  TEXT NOT NULL,
                entry_hash TEXT NOT NULL,
                body       TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    /// Anexa uma entrada, calculando `seq`, `prev_hash` e `entry_hash`.
    pub fn append(&mut self, mut entry: LedgerEntry) -> Result<LedgerEntry, LedgerError> {
        let tx = self.conn.transaction()?;
        let prev_hash: String = tx
            .query_row(
                "SELECT entry_hash FROM ledger ORDER BY seq DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        entry.prev_hash = prev_hash.clone();
        entry.entry_hash = entry.chain_hash(&prev_hash);
        tx.execute(
            "INSERT INTO ledger (prev_hash, entry_hash, body) VALUES (?1, ?2, ?3)",
            params![
                entry.prev_hash,
                entry.entry_hash,
                serde_json::to_string(&entry)?
            ],
        )?;
        entry.seq = tx.last_insert_rowid() as u64;
        tx.commit()?;
        Ok(entry)
    }

    /// Percorre a cadeia inteira validando os hashes encadeados.
    pub fn verify_chain(&self) -> Result<u64, LedgerError> {
        let mut stmt = self
            .conn
            .prepare("SELECT seq, prev_hash, entry_hash, body FROM ledger ORDER BY seq")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut expected_prev = String::new();
        let mut count = 0u64;
        for row in rows {
            let (seq, prev_hash, entry_hash, body) = row?;
            let entry: LedgerEntry = serde_json::from_str(&body)?;
            let recomputed = entry.chain_hash(&expected_prev);
            if prev_hash != expected_prev || entry_hash != recomputed {
                return Err(LedgerError::BrokenChain {
                    seq,
                    expected: recomputed,
                    found: entry_hash,
                });
            }
            expected_prev = entry_hash;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(kind: &str) -> LedgerEntry {
        LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: kind.into(),
            actor: "test".into(),
            payload: json!({"n": 1}),
            r#override: None,
            fake_marker: None,
            ts: "2026-07-05T00:00:00Z".into(),
        }
    }

    #[test]
    fn append_encadeia_e_verifica() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let first = store.append(entry("session.start")).unwrap();
        let second = store.append(entry("tool.run")).unwrap();
        assert_eq!(first.seq, 1);
        assert_eq!(second.prev_hash, first.entry_hash);
        assert_eq!(store.verify_chain().unwrap(), 2);
    }

    #[test]
    fn adulteracao_e_detectada() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        store.append(entry("session.start")).unwrap();
        store.append(entry("tool.run")).unwrap();
        store
            .conn
            .execute(
                "UPDATE ledger SET body = replace(body, '\"n\":1', '\"n\":2') WHERE seq = 1",
                [],
            )
            .unwrap();
        assert!(matches!(
            store.verify_chain(),
            Err(LedgerError::BrokenChain { seq: 1, .. })
        ));
    }

    /// Fase 5 Onda 4: a certificação é PRODUZIDA em Python
    /// (`forge_review.certification.certify`) e REGISTRADA aqui — o ledger
    /// já suporta qualquer `kind`/`payload` livre, então nenhuma mudança de
    /// produção é necessária; este teste prova a capacidade com um payload
    /// no formato real que `Certification.model_dump()` produziria.
    #[test]
    fn certificacao_registra_no_ledger_com_cadeia_integra() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        store.append(entry("session.start")).unwrap();

        let certification_payload = json!({
            "run_id": "run-1",
            "git_sha": "deadbeef",
            "verdict": {
                "approved": true,
                "value_score": 0.86,
                "reason": "aprovado por média ponderada",
                "gate_triggered": null,
            },
            "evidence_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "steps_summary": ["test: ok", "lint: ok"],
            "produced_at": "2026-07-05T00:00:00Z",
        });
        let cert_entry = LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: "certification".into(),
            actor: "forge_review".into(),
            payload: certification_payload.clone(),
            r#override: None,
            fake_marker: None,
            ts: "2026-07-05T00:00:01Z".into(),
        };

        let registered = store.append(cert_entry).unwrap();
        assert_eq!(registered.seq, 2);
        assert_eq!(registered.kind, "certification");
        assert_eq!(
            registered.payload["evidence_hash"],
            certification_payload["evidence_hash"]
        );
        assert_eq!(store.verify_chain().unwrap(), 2);
    }
}
