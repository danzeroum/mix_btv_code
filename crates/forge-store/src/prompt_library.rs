//! Biblioteca de prompts salvos (origem: prompte `library.js`/`savedPrompts.js`):
//! salvar, favoritar, organizar por tags e reusar prompts renderizados
//! pelo PromptForge.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPrompt {
    pub id: i64,
    pub name: String,
    pub generator: String,
    pub fields: Value,
    pub rendered: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub created_at: String,
}

pub struct PromptLibrary {
    conn: Connection,
}

impl PromptLibrary {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS prompt_library (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT NOT NULL,
                generator  TEXT NOT NULL,
                fields     TEXT NOT NULL,
                rendered   TEXT NOT NULL,
                tags       TEXT NOT NULL,
                favorite   INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn save(
        &self,
        name: &str,
        generator: &str,
        fields: &Value,
        rendered: &str,
        tags: &[String],
        created_at: &str,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO prompt_library (name, generator, fields, rendered, tags, favorite, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![
                name,
                generator,
                fields.to_string(),
                rendered,
                serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string()),
                created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Lista os prompts salvos (mais recentes primeiro), opcionalmente
    /// filtrados por uma tag exata.
    pub fn list(&self, tag: Option<&str>) -> rusqlite::Result<Vec<SavedPrompt>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, generator, fields, rendered, tags, favorite, created_at
             FROM prompt_library ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_prompt)?;
        let mut out = Vec::new();
        for row in rows {
            let prompt = row?;
            if tag.is_none_or(|t| prompt.tags.iter().any(|pt| pt == t)) {
                out.push(prompt);
            }
        }
        Ok(out)
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Option<SavedPrompt>> {
        self.conn
            .query_row(
                "SELECT id, name, generator, fields, rendered, tags, favorite, created_at
                 FROM prompt_library WHERE id = ?1",
                params![id],
                Self::row_to_prompt,
            )
            .optional()
    }

    /// Inverte o favorito e devolve o novo estado, ou `None` se o id não existe.
    pub fn toggle_favorite(&self, id: i64) -> rusqlite::Result<Option<bool>> {
        let Some(current) = self.get(id)? else {
            return Ok(None);
        };
        let new_state = !current.favorite;
        self.conn.execute(
            "UPDATE prompt_library SET favorite = ?1 WHERE id = ?2",
            params![new_state as i64, id],
        )?;
        Ok(Some(new_state))
    }

    /// Remove o prompt; devolve `true` se algo foi apagado.
    pub fn delete(&self, id: i64) -> rusqlite::Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM prompt_library WHERE id = ?1", params![id])?
            > 0)
    }

    fn row_to_prompt(row: &rusqlite::Row) -> rusqlite::Result<SavedPrompt> {
        let fields_text: String = row.get(3)?;
        let tags_text: String = row.get(5)?;
        Ok(SavedPrompt {
            id: row.get(0)?,
            name: row.get(1)?,
            generator: row.get(2)?,
            fields: serde_json::from_str(&fields_text).unwrap_or(Value::Null),
            rendered: row.get(4)?,
            tags: serde_json::from_str(&tags_text).unwrap_or_default(),
            favorite: row.get::<_, i64>(6)? != 0,
            created_at: row.get(7)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lib_with_one_entry() -> (PromptLibrary, i64) {
        let lib = PromptLibrary::open_in_memory().unwrap();
        let id = lib
            .save(
                "revisão do pagamento",
                "code-review",
                &json!({"language": "rust"}),
                "prompt renderizado",
                &["financeiro".to_string(), "revisão".to_string()],
                "2026-07-05T00:00:00Z",
            )
            .unwrap();
        (lib, id)
    }

    #[test]
    fn salva_e_recupera_por_id() {
        let (lib, id) = lib_with_one_entry();
        let saved = lib.get(id).unwrap().unwrap();
        assert_eq!(saved.name, "revisão do pagamento");
        assert_eq!(saved.generator, "code-review");
        assert_eq!(saved.tags, vec!["financeiro", "revisão"]);
        assert!(!saved.favorite);
    }

    #[test]
    fn lista_filtra_por_tag() {
        let (lib, _id) = lib_with_one_entry();
        lib.save(
            "outro",
            "bug-fix",
            &json!({}),
            "x",
            &["bugs".to_string()],
            "t",
        )
        .unwrap();

        assert_eq!(lib.list(None).unwrap().len(), 2);
        assert_eq!(lib.list(Some("financeiro")).unwrap().len(), 1);
        assert_eq!(lib.list(Some("inexistente")).unwrap().len(), 0);
    }

    #[test]
    fn toggle_favorite_inverte_e_persiste() {
        let (lib, id) = lib_with_one_entry();
        assert_eq!(lib.toggle_favorite(id).unwrap(), Some(true));
        assert!(lib.get(id).unwrap().unwrap().favorite);
        assert_eq!(lib.toggle_favorite(id).unwrap(), Some(false));
        assert!(!lib.get(id).unwrap().unwrap().favorite);
    }

    #[test]
    fn toggle_e_delete_de_id_inexistente_sao_no_ops_seguros() {
        let (lib, _id) = lib_with_one_entry();
        assert_eq!(lib.toggle_favorite(999).unwrap(), None);
        assert!(!lib.delete(999).unwrap());
    }

    #[test]
    fn delete_remove_o_prompt() {
        let (lib, id) = lib_with_one_entry();
        assert!(lib.delete(id).unwrap());
        assert!(lib.get(id).unwrap().is_none());
    }
}
