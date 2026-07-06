//! Ferramenta `read`: lê um arquivo do workspace com números de linha.

use crate::{
    bound_output_managed, required_str, Tool, ToolError, ToolOutput, DEFAULT_OUTPUT_LIMIT,
};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ReadTool {
    pub root: PathBuf,
}

const DEFAULT_LINE_LIMIT: usize = 2000;

impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Lê um arquivo texto do workspace, com números de linha. Use offset/limit para arquivos grandes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "caminho relativo à raiz do workspace"},
                "offset": {"type": "integer", "description": "linha inicial (1-based)"},
                "limit": {"type": "integer", "description": "máximo de linhas"}
            },
            "required": ["path"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        args["path"].as_str().unwrap_or("").to_string()
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let path = required_str(args, "path")?;
        let full = self.root.join(path);
        let content = std::fs::read_to_string(&full)
            .map_err(|e| ToolError::Execution(format!("{}: {e}", full.display())))?;
        let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = args["limit"].as_u64().unwrap_or(DEFAULT_LINE_LIMIT as u64) as usize;
        let out: String = content
            .lines()
            .enumerate()
            .skip(offset - 1)
            .take(limit)
            .map(|(i, line)| format!("{}\t{line}\n", i + 1))
            .collect();
        bound_output_managed(&self.root, out, DEFAULT_OUTPUT_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_com_numeros_de_linha_e_offset() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "um\ndois\ntres\n").unwrap();
        let tool = ReadTool {
            root: dir.path().to_path_buf(),
        };
        let out = tool.run(&json!({"path": "a.txt", "offset": 2})).unwrap();
        assert_eq!(out.content, "2\tdois\n3\ttres\n");
    }

    #[test]
    fn arquivo_inexistente_da_erro() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ReadTool {
            root: dir.path().to_path_buf(),
        };
        assert!(tool.run(&json!({"path": "nao-existe"})).is_err());
    }
}
