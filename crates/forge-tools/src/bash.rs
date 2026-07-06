//! Ferramenta `bash`: executa um comando shell no workspace, com timeout.

use crate::{
    bound_output_managed, required_str, Tool, ToolError, ToolOutput, DEFAULT_OUTPUT_LIMIT,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct BashTool {
    pub root: PathBuf,
}

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Executa um comando shell (sh -c) na raiz do workspace e retorna stdout+stderr. Timeout padrão de 120s."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "comando a executar"},
                "timeout_ms": {"type": "integer", "description": "timeout em milissegundos (máx 600000)"}
            },
            "required": ["command"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        args["command"].as_str().unwrap_or("").to_string()
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let command = required_str(args, "command")?;
        let timeout = Duration::from_millis(
            args["timeout_ms"]
                .as_u64()
                .unwrap_or(DEFAULT_TIMEOUT_MS)
                .min(MAX_TIMEOUT_MS),
        );

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ToolError::Execution(format!("spawn: {e}")))?;

        let start = Instant::now();
        let status = loop {
            match child
                .try_wait()
                .map_err(|e| ToolError::Execution(e.to_string()))?
            {
                Some(status) => break status,
                None if start.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ToolError::Execution(format!(
                        "timeout de {}ms excedido",
                        timeout.as_millis()
                    )));
                }
                None => std::thread::sleep(Duration::from_millis(25)),
            }
        };

        let mut output = String::new();
        if let Some(mut out) = child.stdout.take() {
            use std::io::Read;
            let _ = out.read_to_string(&mut output);
        }
        if let Some(mut err) = child.stderr.take() {
            use std::io::Read;
            let mut e = String::new();
            let _ = err.read_to_string(&mut e);
            output.push_str(&e);
        }
        if !status.success() {
            output.push_str(&format!("\n[exit code: {}]", status.code().unwrap_or(-1)));
        }
        bound_output_managed(&self.root, output, DEFAULT_OUTPUT_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> (tempfile::TempDir, BashTool) {
        let dir = tempfile::tempdir().unwrap();
        let tool = BashTool {
            root: dir.path().to_path_buf(),
        };
        (dir, tool)
    }

    #[test]
    fn executa_no_diretorio_do_workspace() {
        let (dir, tool) = tool();
        std::fs::write(dir.path().join("x.txt"), "").unwrap();
        let out = tool.run(&json!({"command": "ls"})).unwrap();
        assert!(out.content.contains("x.txt"));
    }

    #[test]
    fn falha_inclui_exit_code() {
        let (_dir, tool) = tool();
        let out = tool.run(&json!({"command": "exit 3"})).unwrap();
        assert!(out.content.contains("[exit code: 3]"));
    }

    #[test]
    fn timeout_mata_o_processo() {
        let (_dir, tool) = tool();
        let err = tool
            .run(&json!({"command": "sleep 5", "timeout_ms": 100}))
            .unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }
}
