//! Persistência das regras de permissão editadas pelo usuário (matriz
//! build/plan×tool e overrides "sempre" da ponte de permissão — Fase 7 Onda
//! 2) — puramente armazenamento. A avaliação em si continua em
//! `forge_core::permission::PermissionEngine`; este crate não depende de
//! `forge-core` (que já depende de `forge-store`) para não inverter a
//! aresta de dependência existente, daí `RuleDecision` ser uma cópia local
//! e não o `Decision` de `forge-core`.

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum RuleStoreError {
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDecision {
    Allow,
    Ask,
    Deny,
}

impl RuleDecision {
    fn as_str(self) -> &'static str {
        match self {
            RuleDecision::Allow => "allow",
            RuleDecision::Ask => "ask",
            RuleDecision::Deny => "deny",
        }
    }
}

impl std::str::FromStr for RuleDecision {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allow" => Ok(RuleDecision::Allow),
            "ask" => Ok(RuleDecision::Ask),
            "deny" => Ok(RuleDecision::Deny),
            _ => Err(()),
        }
    }
}

/// Um override persistido: decisão para `tool` sob `profile`, opcionalmente
/// restrita a `scope_prefix` (`None` = regra de matriz, vale para qualquer
/// escopo; `Some` = regra "sempre" de um pedido específico).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RuleRecord {
    pub id: i64,
    pub profile: String,
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_prefix: Option<String>,
    pub decision: RuleDecision,
    pub created_at: String,
}

pub struct RuleStore {
    conn: Connection,
}

impl RuleStore {
    pub fn open(path: &str) -> Result<Self, RuleStoreError> {
        let conn = Connection::open(path)?;
        // Regras podem ser lidas (avaliação de permissão) e escritas (matriz
        // editada pelo navegador) de requisições concorrentes — WAL desde a
        // criação, ao contrário do `LedgerStore` legado (bug conhecido,
        // fechado só na Onda 6).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, RuleStoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, RuleStoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS permission_rules (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                profile      TEXT NOT NULL,
                tool         TEXT NOT NULL,
                scope_prefix TEXT,
                decision     TEXT NOT NULL,
                created_at   TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    /// Grava um override, substituindo qualquer regra existente com a MESMA
    /// chave (`profile`+`tool`+`scope_prefix`) — não acumula duplicatas ao
    /// reeditar a mesma célula da matriz ou repetir "sempre" no mesmo escopo.
    pub fn set(
        &mut self,
        profile: &str,
        tool: &str,
        scope_prefix: Option<&str>,
        decision: RuleDecision,
        created_at: &str,
    ) -> Result<RuleRecord, RuleStoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM permission_rules WHERE profile = ?1 AND tool = ?2 AND scope_prefix IS ?3",
            params![profile, tool, scope_prefix],
        )?;
        tx.execute(
            "INSERT INTO permission_rules (profile, tool, scope_prefix, decision, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![profile, tool, scope_prefix, decision.as_str(), created_at],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(RuleRecord {
            id,
            profile: profile.to_string(),
            tool: tool.to_string(),
            scope_prefix: scope_prefix.map(str::to_string),
            decision,
            created_at: created_at.to_string(),
        })
    }

    /// Busca uma regra pelo id (usado para logar o payload de auditoria
    /// antes de remover).
    pub fn get(&self, id: i64) -> Result<Option<RuleRecord>, RuleStoreError> {
        self.conn
            .query_row(
                "SELECT id, profile, tool, scope_prefix, decision, created_at
                 FROM permission_rules WHERE id = ?1",
                params![id],
                Self::row_to_record,
            )
            .optional()
            .map_err(Into::into)
    }

    /// Remove um override pelo id — `false` se não existia (idempotente).
    pub fn remove(&mut self, id: i64) -> Result<bool, RuleStoreError> {
        let n = self
            .conn
            .execute("DELETE FROM permission_rules WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// Todas as regras persistidas, mais recentes primeiro — alimenta a
    /// lista "rules ativas" da UI (com botão de revogar).
    pub fn list_all(&self) -> Result<Vec<RuleRecord>, RuleStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, profile, tool, scope_prefix, decision, created_at
             FROM permission_rules ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Regras de um perfil, para alimentar `PermissionEngine::overlay`:
    /// regras com escopo específico primeiro (mais específico deve vencer o
    /// default do perfil), e dentro do mesmo nível, a mais recente primeiro.
    pub fn list_for_profile(&self, profile: &str) -> Result<Vec<RuleRecord>, RuleStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, profile, tool, scope_prefix, decision, created_at
             FROM permission_rules
             WHERE profile = ?1
             ORDER BY (scope_prefix IS NULL) ASC, id DESC",
        )?;
        let rows = stmt.query_map(params![profile], Self::row_to_record)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<RuleRecord> {
        let decision_str: String = row.get(4)?;
        let decision = decision_str
            .parse::<RuleDecision>()
            .unwrap_or(RuleDecision::Ask);
        Ok(RuleRecord {
            id: row.get(0)?,
            profile: row.get(1)?,
            tool: row.get(2)?,
            scope_prefix: row.get(3)?,
            decision,
            created_at: row.get(5)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_e_list_all_reflete_gravacao() {
        let mut store = RuleStore::open_in_memory().unwrap();
        let rec = store
            .set(
                "build",
                "bash",
                None,
                RuleDecision::Allow,
                "2026-07-06T00:00:00Z",
            )
            .unwrap();
        assert_eq!(rec.tool, "bash");
        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].decision, RuleDecision::Allow);
    }

    #[test]
    fn set_repetido_na_mesma_chave_substitui_nao_acumula() {
        let mut store = RuleStore::open_in_memory().unwrap();
        store
            .set("build", "bash", None, RuleDecision::Allow, "t1")
            .unwrap();
        store
            .set("build", "bash", None, RuleDecision::Deny, "t2")
            .unwrap();
        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].decision, RuleDecision::Deny);
    }

    #[test]
    fn scope_prefix_none_e_some_sao_chaves_distintas() {
        let mut store = RuleStore::open_in_memory().unwrap();
        store
            .set("build", "bash", None, RuleDecision::Ask, "t1")
            .unwrap();
        store
            .set("build", "bash", Some("npm test"), RuleDecision::Allow, "t2")
            .unwrap();
        assert_eq!(store.list_all().unwrap().len(), 2);
    }

    #[test]
    fn list_for_profile_ordena_escopo_especifico_primeiro() {
        let mut store = RuleStore::open_in_memory().unwrap();
        store
            .set("build", "bash", None, RuleDecision::Ask, "t1")
            .unwrap();
        store
            .set("build", "bash", Some("npm test"), RuleDecision::Allow, "t2")
            .unwrap();
        let rules = store.list_for_profile("build").unwrap();
        assert_eq!(rules[0].scope_prefix.as_deref(), Some("npm test"));
        assert_eq!(rules[1].scope_prefix, None);
    }

    #[test]
    fn list_for_profile_filtra_por_perfil() {
        let mut store = RuleStore::open_in_memory().unwrap();
        store
            .set("build", "bash", None, RuleDecision::Allow, "t1")
            .unwrap();
        store
            .set("plan", "bash", None, RuleDecision::Deny, "t1")
            .unwrap();
        let build_rules = store.list_for_profile("build").unwrap();
        assert_eq!(build_rules.len(), 1);
        assert_eq!(build_rules[0].decision, RuleDecision::Allow);
    }

    #[test]
    fn remove_e_idempotente_e_get_reflete_ausencia() {
        let mut store = RuleStore::open_in_memory().unwrap();
        let rec = store
            .set("build", "bash", None, RuleDecision::Allow, "t1")
            .unwrap();
        assert!(store.remove(rec.id).unwrap());
        assert!(!store.remove(rec.id).unwrap());
        assert!(store.get(rec.id).unwrap().is_none());
    }
}
