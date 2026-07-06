//! Ferramentas determinísticas da plataforma Forge.
//!
//! Princípio (fork do opencode): "o LLM orquestra; ferramentas
//! determinísticas verificam". Fase 1: read, grep, edit e bash reais sob o
//! motor de permissões; LSP/MCP/webfetch/sandbox chegam nas Fases 2–6.

pub mod bash;
pub mod diff;
pub mod edit;
pub mod grep;
pub mod read;
pub mod registry;
pub mod sandbox;
pub mod skill;

pub use diff::{format_diff, line_diff, DiffLine};
pub use registry::ToolRegistry;
pub use sandbox::{Sandbox, SandboxError, SandboxOutput};
pub use skill::SkillTool;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("argumentos inválidos: {0}")]
    InvalidArgs(String),
    #[error("falha de execução: {0}")]
    Execution(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    /// Quando o output excede o limite, ele é truncado e o restante vai
    /// para um arquivo gerenciado (Managed Tool Output File).
    pub truncated: bool,
    /// Caminho (relativo à raiz do workspace) do output completo, quando
    /// truncado e persistido por [`bound_output_managed`].
    pub overflow_path: Option<String>,
    /// Diff de linhas, quando a ferramenta alterou um arquivo texto
    /// (hoje: `edit`) — consumido pela TUI para o bloco colorido.
    pub diff: Option<Vec<diff::DiffLine>>,
}

/// Contrato de ferramenta: identidade estável, schema para o modelo,
/// escopo para o motor de permissões e execução com args JSON.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema dos argumentos, anunciado ao modelo.
    fn input_schema(&self) -> Value;
    /// Escopo avaliado pelo motor de permissões (caminho, comando...).
    fn scope(&self, args: &Value) -> String;
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError>;
}

/// Limite padrão de bytes devolvidos inline ao contexto do modelo.
pub const DEFAULT_OUTPUT_LIMIT: usize = 32 * 1024;

/// Trunca o output em uma fronteira de char válida, sinalizando truncamento
/// — sem persistir o restante (uso interno/teste; ferramentas com acesso
/// ao workspace devem preferir [`bound_output_managed`]).
pub fn bound_output(content: String, limit: usize) -> ToolOutput {
    if content.len() <= limit {
        return ToolOutput {
            content,
            truncated: false,
            overflow_path: None,
            diff: None,
        };
    }
    let mut cut = limit;
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    ToolOutput {
        content: content[..cut].to_string(),
        truncated: true,
        overflow_path: None,
        diff: None,
    }
}

/// Como [`bound_output`], mas quando o conteúdo excede o limite grava o
/// texto completo em `<root>/.forge/tool-outputs/<id>.txt` (Managed Tool
/// Output File) e devolve o caminho relativo — o modelo recebe o cabeçalho
/// inline mais a indicação de onde ler o resto (ex.: via `read`).
pub fn bound_output_managed(
    root: &Path,
    content: String,
    limit: usize,
) -> Result<ToolOutput, ToolError> {
    if content.len() <= limit {
        return Ok(ToolOutput {
            content,
            truncated: false,
            overflow_path: None,
            diff: None,
        });
    }
    let mut cut = limit;
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    let rel_dir = Path::new(".forge").join("tool-outputs");
    let dir = root.join(&rel_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| ToolError::Execution(format!("overflow dir: {e}")))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let filename = format!("{nanos:024x}.txt");
    std::fs::write(dir.join(&filename), &content)
        .map_err(|e| ToolError::Execution(format!("overflow write: {e}")))?;
    Ok(ToolOutput {
        content: content[..cut].to_string(),
        truncated: true,
        overflow_path: Some(rel_dir.join(&filename).to_string_lossy().replace('\\', "/")),
        diff: None,
    })
}

pub(crate) fn required_str<'a>(args: &'a Value, field: &str) -> Result<&'a str, ToolError> {
    args.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArgs(format!("campo '{field}' obrigatório")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncamento_respeita_fronteira_utf8() {
        let out = bound_output("aça".repeat(10), 5);
        assert!(out.truncated);
        assert!(out.content.len() <= 5);
        assert!(std::str::from_utf8(out.content.as_bytes()).is_ok());
        assert!(out.overflow_path.is_none());
    }

    #[test]
    fn managed_persiste_o_conteudo_completo() {
        let dir = tempfile::tempdir().unwrap();
        let full = "x".repeat(100);
        let out = bound_output_managed(dir.path(), full.clone(), 10).unwrap();
        assert!(out.truncated);
        assert_eq!(out.content.len(), 10);
        let rel = out.overflow_path.expect("overflow gravado");
        assert!(rel.starts_with(".forge/tool-outputs/"));
        let persisted = std::fs::read_to_string(dir.path().join(&rel)).unwrap();
        assert_eq!(persisted, full);
    }

    #[test]
    fn managed_sem_overflow_quando_cabe_no_limite() {
        let dir = tempfile::tempdir().unwrap();
        let out = bound_output_managed(dir.path(), "curto".into(), 100).unwrap();
        assert!(!out.truncated);
        assert!(out.overflow_path.is_none());
        assert!(!dir.path().join(".forge").exists());
    }
}
