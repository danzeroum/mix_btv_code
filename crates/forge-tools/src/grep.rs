//! Ferramenta `grep`: busca por regex com as bibliotecas do ripgrep
//! (`grep` + `ignore`), respeitando .gitignore.
//!
//! Porte do `opencode-tools` da branch `rust-migration` (ADR 0002): o
//! `Searcher` do ripgrep dá a mesma semântica de matching da ferramenta
//! `grep` clássica do opencode, com custo linear em memória por linha.

use crate::{
    bound_output_managed, required_str, Tool, ToolError, ToolOutput, DEFAULT_OUTPUT_LIMIT,
};
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::Searcher;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct GrepTool {
    pub root: PathBuf,
}

const MAX_MATCHES: usize = 200;

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Busca um padrão (regex) nos arquivos do workspace, respeitando .gitignore. Retorna caminho:linha:conteúdo."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "expressão regular"},
                "path": {"type": "string", "description": "subdiretório ou arquivo (opcional)"}
            },
            "required": ["pattern"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        args["path"].as_str().unwrap_or(".").to_string()
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let pattern = required_str(args, "pattern")?;
        let matcher = RegexMatcher::new(pattern)
            .map_err(|e| ToolError::InvalidArgs(format!("regex: {e}")))?;
        let base = match args["path"].as_str() {
            Some(p) => self.root.join(p),
            None => self.root.clone(),
        };

        let mut matches = Vec::new();
        let walker = ignore::WalkBuilder::new(&base)
            .hidden(true)
            .require_git(false) // .gitignore vale mesmo fora de um repo git
            .build();
        'outer: for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let rel = path.strip_prefix(&self.root).unwrap_or(path).to_path_buf();
            let mut searcher = Searcher::new();
            let mut full = false;
            // Erros por arquivo (binário/não-UTF8) são pulados, não abortam.
            let _ = searcher.search_path(
                &matcher,
                path,
                UTF8(|line_number, line| {
                    matches.push(format!(
                        "{}:{}:{}",
                        rel.display(),
                        line_number,
                        line.trim_end()
                    ));
                    if matches.len() >= MAX_MATCHES {
                        full = true;
                        return Ok(false); // para este arquivo
                    }
                    Ok(true)
                }),
            );
            if full {
                matches.push(format!(
                    "... (limite de {MAX_MATCHES} ocorrências atingido)"
                ));
                break 'outer;
            }
        }
        if matches.is_empty() {
            return Ok(ToolOutput {
                content: "nenhuma ocorrência".into(),
                truncated: false,
                overflow_path: None,
                diff: None,
            });
        }
        bound_output_managed(&self.root, matches.join("\n"), DEFAULT_OUTPUT_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encontra_ocorrencias_e_respeita_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn alvo() {}\n").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/b.rs"), "fn alvo() {}\n").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        let tool = GrepTool {
            root: dir.path().to_path_buf(),
        };
        let out = tool.run(&json!({"pattern": "alvo"})).unwrap();
        assert!(out.content.contains("a.rs:1:"));
        assert!(!out.content.contains("target/"));
    }

    #[test]
    fn regex_invalida_da_erro_de_args() {
        let dir = tempfile::tempdir().unwrap();
        let tool = GrepTool {
            root: dir.path().to_path_buf(),
        };
        assert!(matches!(
            tool.run(&json!({"pattern": "("})),
            Err(ToolError::InvalidArgs(_))
        ));
    }

    #[test]
    fn arquivo_binario_e_pulado_sem_erro() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bin.dat"), [0u8, 159, 146, 150]).unwrap();
        std::fs::write(dir.path().join("ok.txt"), "alvo\n").unwrap();
        let tool = GrepTool {
            root: dir.path().to_path_buf(),
        };
        let out = tool.run(&json!({"pattern": "alvo"})).unwrap();
        assert!(out.content.contains("ok.txt:1:alvo"));
    }
}
