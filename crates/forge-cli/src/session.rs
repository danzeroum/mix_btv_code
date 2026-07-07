//! Sessão do CLI: registra os eventos do loop no ledger append-only
//! (`.forge/forge.db` na raiz do workspace).

use forge_core::LoopEvent;
use forge_schemas::ledger::{LedgerEntry, OverrideMark};
use forge_store::LedgerStore;
use serde_json::{json, Value};
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub struct Session {
    store: LedgerStore,
    id: String,
}

pub(crate) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

impl Session {
    pub fn open(root: &Path, task: &str, model: &str) -> anyhow::Result<Self> {
        let dir = root.join(".forge");
        std::fs::create_dir_all(&dir)?;
        let store = LedgerStore::open(dir.join("forge.db").to_str().unwrap_or(".forge/forge.db"))?;
        let id = format!(
            "s{:x}",
            OffsetDateTime::now_utc().unix_timestamp_nanos() & 0xffff_ffff_ffff
        );
        let mut session = Self { store, id };
        session.append("session.start", json!({"task": task, "model": model}))?;
        Ok(session)
    }

    fn append(&mut self, kind: &str, payload: Value) -> anyhow::Result<()> {
        self.store.append(LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: kind.into(),
            actor: format!("forge-cli:{}", self.id),
            payload,
            r#override: None,
            fake_marker: None,
            ts: now_rfc3339(),
        })?;
        Ok(())
    }

    /// Registra um evento do loop. Falhas de ledger não derrubam a sessão —
    /// são reportadas no stderr (o turno do usuário vale mais que o registro).
    pub fn record(&mut self, event: &LoopEvent) {
        let entry = match event {
            LoopEvent::TextDelta(_) => None, // granularidade de turno, não de delta
            LoopEvent::TurnCompleted {
                provider,
                input_tokens,
                output_tokens,
            } => Some((
                "llm.turn",
                json!({"provider": provider, "input_tokens": input_tokens, "output_tokens": output_tokens}),
            )),
            LoopEvent::ToolStarted { name, scope } => {
                Some(("tool.run", json!({"tool": name, "scope": scope})))
            }
            LoopEvent::ToolFinished {
                name, ok, summary, ..
            } => Some((
                "tool.result",
                json!({"tool": name, "ok": ok, "summary": summary}),
            )),
            LoopEvent::ToolDenied { name, scope } => {
                Some(("tool.denied", json!({"tool": name, "scope": scope})))
            }
        };
        if let Some((kind, payload)) = entry {
            if let Err(e) = self.append(kind, payload) {
                eprintln!("  [ledger] falha ao registrar {kind}: {e}");
            }
        }
    }

    /// Registra um evento avulso (ex.: `user.turn` no chat). Falhas são
    /// reportadas no stderr, sem derrubar a sessão.
    pub fn note(&mut self, kind: &str, payload: Value) {
        if let Err(e) = self.append(kind, payload) {
            eprintln!("  [ledger] falha ao registrar {kind}: {e}");
        }
    }

    pub fn finish(&mut self, success: bool, steps: usize) -> anyhow::Result<()> {
        self.append("session.end", json!({"success": success, "steps": steps}))
    }

    /// Verifica a integridade da cadeia e retorna o total de entradas.
    pub fn verify(&self) -> anyhow::Result<u64> {
        Ok(self.store.verify_chain()?)
    }
}

/// Registra uma entrada avulsa no MESMO ledger (`.forge/forge.db`), fora do
/// ciclo de vida de uma `Session` de tarefa — usado por mutações de
/// configuração (matriz de permissão, Fase 7 Onda 2) que não têm
/// `session.start`/`session.end` próprios. Sempre marcada como `override`:
/// afrouxar/restringir permissão pelo navegador é a mutação mais sensível
/// deste plano e nunca deve passar em silêncio pelo ledger.
pub fn append_override_entry(
    root: &Path,
    actor: &str,
    kind: &str,
    payload: Value,
) -> anyhow::Result<()> {
    let dir = root.join(".forge");
    std::fs::create_dir_all(&dir)?;
    let mut store = LedgerStore::open(dir.join("forge.db").to_str().unwrap_or(".forge/forge.db"))?;
    store.append(LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: kind.into(),
        actor: actor.into(),
        payload,
        r#override: Some(OverrideMark {
            marked: true,
            reason: None,
        }),
        fake_marker: None,
        ts: now_rfc3339(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessao_registra_eventos_no_ledger_com_cadeia_integra() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::open(dir.path(), "tarefa", "claude-sonnet-5").unwrap();
        s.record(&LoopEvent::TurnCompleted {
            provider: "anthropic".into(),
            input_tokens: 10,
            output_tokens: 5,
        });
        s.record(&LoopEvent::ToolStarted {
            name: "read".into(),
            scope: "f.txt".into(),
        });
        s.record(&LoopEvent::TextDelta("ignorado")); // deltas não vão ao ledger
        s.finish(true, 2).unwrap();
        // start + turn + tool + end = 4 entradas, cadeia íntegra
        assert_eq!(s.verify().unwrap(), 4);
    }
}
