//! Ferramenta `edit`: substituição exata e única de trecho num arquivo.

use crate::diff::{format_diff, line_diff};
use crate::{required_str, Tool, ToolError, ToolOutput};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct EditTool {
    pub root: PathBuf,
}

impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Substitui uma ocorrência exata de old_string por new_string num arquivo. old_string deve ser única no arquivo."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "caminho relativo à raiz do workspace"},
                "old_string": {"type": "string", "description": "trecho exato a substituir (único no arquivo, salvo replace_all)"},
                "new_string": {"type": "string", "description": "novo trecho"},
                "replace_all": {"type": "boolean", "description": "substitui todas as ocorrências", "default": false}
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        args["path"].as_str().unwrap_or("").to_string()
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let path = required_str(args, "path")?;
        let old = required_str(args, "old_string")?;
        let new = required_str(args, "new_string")?;
        if old == new {
            return Err(ToolError::InvalidArgs(
                "old_string e new_string são iguais".into(),
            ));
        }
        let replace_all = args["replace_all"].as_bool().unwrap_or(false);
        let full = self.root.join(path);
        let content = std::fs::read_to_string(&full)
            .map_err(|e| ToolError::Execution(format!("{}: {e}", full.display())))?;
        // Semântica do opencode-tools (rust-migration): sem replace_all, a
        // ocorrência deve ser única — edits ambíguos são rejeitados.
        let occurrences = content.matches(old).count();
        match occurrences {
            0 => Err(ToolError::Execution(format!(
                "old_string não encontrada em {path}"
            ))),
            1 => {
                let updated = content.replacen(old, new, 1);
                std::fs::write(&full, &updated)
                    .map_err(|e| ToolError::Execution(format!("{}: {e}", full.display())))?;
                let diff = line_diff(&content, &updated);
                Ok(ToolOutput {
                    content: format!("editado: {path}\n{}", format_diff(&diff)),
                    truncated: false,
                    overflow_path: None,
                    diff: Some(diff),
                })
            }
            n if replace_all => {
                let updated = content.replace(old, new);
                std::fs::write(&full, &updated)
                    .map_err(|e| ToolError::Execution(format!("{}: {e}", full.display())))?;
                let diff = line_diff(&content, &updated);
                Ok(ToolOutput {
                    content: format!("editado: {path} ({n} ocorrências)\n{}", format_diff(&diff)),
                    truncated: false,
                    overflow_path: None,
                    diff: Some(diff),
                })
            }
            n => Err(ToolError::Execution(format!(
                "old_string aparece {n} vezes em {path}; forneça um trecho único ou passe replace_all"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(content: &str) -> (tempfile::TempDir, EditTool) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), content).unwrap();
        let tool = EditTool {
            root: dir.path().to_path_buf(),
        };
        (dir, tool)
    }

    #[test]
    fn substitui_ocorrencia_unica() {
        let (dir, tool) = setup("let x = 1;\nlet y = 2;\n");
        tool.run(&json!({"path": "f.rs", "old_string": "let x = 1;", "new_string": "let x = 10;"}))
            .unwrap();
        let content = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert_eq!(content, "let x = 10;\nlet y = 2;\n");
    }

    #[test]
    fn recusa_ocorrencia_ambigua() {
        let (_dir, tool) = setup("a\na\n");
        let err = tool
            .run(&json!({"path": "f.rs", "old_string": "a", "new_string": "b"}))
            .unwrap_err();
        assert!(err.to_string().contains("2 vezes"));
    }

    #[test]
    fn replace_all_substitui_todas() {
        let (dir, tool) = setup("a\na\na\n");
        let out = tool
            .run(
                &json!({"path": "f.rs", "old_string": "a", "new_string": "b", "replace_all": true}),
            )
            .unwrap();
        assert!(out.content.contains("3 ocorrências"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.rs")).unwrap(),
            "b\nb\nb\n"
        );
    }

    #[test]
    fn substituicao_unica_anexa_diff_estruturado() {
        let (_dir, tool) = setup("let x = 1;\nlet y = 2;\n");
        let out = tool
            .run(&json!({"path": "f.rs", "old_string": "let x = 1;", "new_string": "let x = 10;"}))
            .unwrap();
        let diff = out.diff.expect("diff calculado");
        assert!(diff.contains(&crate::DiffLine::Removed("let x = 1;".into())));
        assert!(diff.contains(&crate::DiffLine::Added("let x = 10;".into())));
        assert!(out.content.contains("- let x = 1;"));
        assert!(out.content.contains("+ let x = 10;"));
    }

    #[test]
    fn recusa_trecho_inexistente() {
        let (_dir, tool) = setup("abc\n");
        assert!(tool
            .run(&json!({"path": "f.rs", "old_string": "xyz", "new_string": "w"}))
            .is_err());
    }
}
