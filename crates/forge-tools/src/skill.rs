//! `SkillTool`: expõe uma skill (diretório + manifest) como `dyn Tool` no
//! `ToolRegistry`. Fase 6 Onda 1 — o runtime que torna uma skill vetada
//! invocável pelo agente.
//!
//! Built-ins rodam como subprocesso direto (são do repo, confiáveis, e ainda
//! assim passam pelo vetter no loader). O confinamento em sandbox de código de
//! terceiro é a Onda 2 (executor) + Onda 3 (amarração). O `run` aqui espelha o
//! `bash.rs` (o tool de referência), mas isola o subprocesso no próprio grupo
//! e mata o GRUPO no timeout — a lição do órfão da Fase 4d, para que uma skill
//! que trava não trave o loop.

use crate::{bound_output, Tool, ToolError, ToolOutput, DEFAULT_OUTPUT_LIMIT};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Timeout padrão de execução de uma skill built-in.
const DEFAULT_SKILL_TIMEOUT_MS: u64 = 30_000;

/// Uma skill carregada: identidade + entrypoint a rodar. Diferente dos tools
/// built-in (nome `&'static`), a identidade vem de `String`s do manifest — é
/// por isso que o trait `Tool` passou a devolver `&str` (lifetime de `&self`)
/// nesta onda: um implementador dinâmico não teria um `&'static` a devolver.
pub struct SkillTool {
    name: String,
    description: String,
    /// Corpo shell a executar (o `entrypoint` do `skill.toml`), rodado via
    /// `sh -c` com o valor de `input` disponível como `$1`.
    entrypoint: String,
    /// Diretório da skill — vira o cwd do subprocesso.
    dir: PathBuf,
    timeout: Duration,
}

impl SkillTool {
    /// Constrói a partir dos campos já extraídos do manifest. O parse do
    /// `skill.toml` e o vetting acontecem no loader (`forge-cli`); aqui a
    /// skill já é confiável o suficiente para virar tool — só executamos.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        entrypoint: impl Into<String>,
        dir: PathBuf,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            entrypoint: entrypoint.into(),
            dir,
            timeout: Duration::from_millis(DEFAULT_SKILL_TIMEOUT_MS),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        // Onda 1: schema genérico. O manifest ainda não declara schema de
        // entrada; um schema estruturado é aditivo (quando uma skill real
        // exigir) — não inventar agora.
        json!({
            "type": "object",
            "properties": {
                "input": {"type": "string", "description": "texto passado à skill como $1"}
            },
            "required": ["input"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        // Informativo para o motor de permissões: a skill + um resumo da
        // entrada (como o bash entrega o comando).
        let input = args.get("input").and_then(Value::as_str).unwrap_or("");
        let preview: String = input.chars().take(60).collect();
        format!("skill:{} {}", self.name, preview)
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let input = args.get("input").and_then(Value::as_str).unwrap_or("");
        let output = run_entrypoint(&self.dir, &self.entrypoint, input, self.timeout)?;
        Ok(bound_output(output, DEFAULT_OUTPUT_LIMIT))
    }
}

/// Roda o entrypoint via `sh -c` com `input` como `$1`, cwd no diretório da
/// skill, matando o grupo de processos inteiro se estourar o timeout.
fn run_entrypoint(
    dir: &Path,
    entrypoint: &str,
    input: &str,
    timeout: Duration,
) -> Result<String, ToolError> {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(entrypoint) // corpo do script; referencia $1
        .arg("forge-skill") // $0
        .arg(input) // $1
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0); // pgid == pid: o kill de -pid pega netos também

    let mut child = command
        .spawn()
        .map_err(|e| ToolError::Execution(format!("spawn skill: {e}")))?;
    let pid = child.id();

    let start = Instant::now();
    let status = loop {
        match child
            .try_wait()
            .map_err(|e| ToolError::Execution(e.to_string()))?
        {
            Some(status) => break status,
            None if start.elapsed() > timeout => {
                kill_process_group(pid);
                let _ = child.wait();
                return Err(ToolError::Execution(format!(
                    "skill excedeu o timeout de {}ms",
                    timeout.as_millis()
                )));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };

    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        use std::io::Read;
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let mut s = String::new();
        let _ = e.read_to_string(&mut s);
        out.push_str(&s);
    }
    if !status.success() {
        out.push_str(&format!(
            "\n[skill exit code: {}]",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(out)
}

#[cfg(unix)]
fn kill_process_group(pid: u32) {
    // `process_group(0)` fez o filho virar líder do próprio grupo (pgid == pid);
    // `-pid` mata o grupo inteiro. Mesma técnica de `forge-verify::exec`.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_pid: u32) {
    // Fora do Unix o `Child` ainda é derrubado no drop; sem garantia de netos.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(entrypoint: &str) -> (tempfile::TempDir, SkillTool) {
        let dir = tempfile::tempdir().unwrap();
        let tool = SkillTool::new("echo-test", "desc", entrypoint, dir.path().to_path_buf());
        (dir, tool)
    }

    #[test]
    fn executa_entrypoint_e_recebe_input_como_arg() {
        let (_d, tool) = skill(r#"printf 'RECEBIDO:%s' "$1""#);
        let out = tool.run(&json!({"input": "ola mundo"})).unwrap();
        assert_eq!(out.content, "RECEBIDO:ola mundo");
    }

    #[test]
    fn roda_no_diretorio_da_skill() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marca.txt"), "x").unwrap();
        let tool = SkillTool::new("ls-skill", "d", "ls", dir.path().to_path_buf());
        let out = tool.run(&json!({"input": ""})).unwrap();
        assert!(out.content.contains("marca.txt"));
    }

    #[test]
    fn timeout_mata_skill_travada() {
        let (_d, tool) = skill("sleep 5");
        let tool = tool.with_timeout(Duration::from_millis(120));
        let start = Instant::now();
        let err = tool.run(&json!({"input": ""})).unwrap_err();
        assert!(err.to_string().contains("timeout"));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "deveria ter matado bem antes dos 5s"
        );
    }

    #[test]
    fn exit_code_de_falha_aparece() {
        let (_d, tool) = skill("exit 7");
        let out = tool.run(&json!({"input": ""})).unwrap();
        assert!(out.content.contains("[skill exit code: 7]"));
    }

    #[test]
    fn identidade_dinamica_satisfaz_o_trait() {
        // O ponto do trait `&str`: nome/descrição vindos de `String` (manifest)
        // implementam o trait — um `&'static str` não daria.
        let dir = tempfile::tempdir().unwrap();
        let tool = SkillTool::new(
            String::from("dyn-name"),
            String::from("dyn-desc"),
            "true",
            dir.path().to_path_buf(),
        );
        assert_eq!(tool.name(), "dyn-name");
        assert_eq!(tool.description(), "dyn-desc");
        assert_eq!(tool.scope(&json!({"input": "abc"})), "skill:dyn-name abc");
    }
}
