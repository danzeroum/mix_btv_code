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

use crate::{
    bound_output, Sandbox, SandboxError, SandboxOutput, Tool, ToolError, ToolOutput,
    DEFAULT_OUTPUT_LIMIT,
};
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
    /// Terceiro (untrusted): `run` executa confinado no sandbox (Onda 2), com
    /// fail-closed se o daemon estiver ausente. Built-in (trusted) roda direto.
    /// É a régua de segurança da Onda 3 — código de fora só roda dentro da cela.
    sandboxed: bool,
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
            sandboxed: false,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Marca a skill como de terceiro: `run` a executa **confinada no sandbox**
    /// (Onda 2), fail-closed se não houver daemon Docker. O built-in confiável
    /// (default) roda direto.
    pub fn sandboxed(mut self) -> Self {
        self.sandboxed = true;
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
        let output = if self.sandboxed {
            run_in_sandbox(&self.dir, &self.entrypoint, input, self.timeout)?
        } else {
            run_entrypoint(&self.dir, &self.entrypoint, input, self.timeout)?
        };
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

/// Roda o entrypoint da skill DENTRO do sandbox Docker (Onda 2), como código de
/// terceiro. **Fail-closed:** se o daemon está ausente, devolve erro e a skill
/// NÃO roda — o `run_entrypoint` direto jamais é usado como fallback (seria
/// rodar código alheio fora da cela, a falha catastrófica da Onda 3).
fn run_in_sandbox(
    dir: &Path,
    entrypoint: &str,
    input: &str,
    timeout: Duration,
) -> Result<String, ToolError> {
    let sandbox = Sandbox::new(dir.to_path_buf()).with_timeout(timeout);
    // Mesmo contrato do caminho direto (`sh -c <entrypoint> forge-skill <input>`,
    // cwd em /work), só que confinado.
    let cmd = vec![
        "sh".to_string(),
        "-c".to_string(),
        entrypoint.to_string(),
        "forge-skill".to_string(),
        input.to_string(),
    ];
    match run_sandbox_blocking(sandbox, cmd) {
        Ok(out) => {
            let mut s = out.stdout;
            if out.timed_out {
                s.push_str("\n[skill: timeout no sandbox]");
            } else if out.exit_code != 0 {
                s.push_str(&format!("\n[skill exit code: {}]", out.exit_code));
            }
            Ok(s)
        }
        Err(SandboxError::DaemonUnavailable(m)) => Err(ToolError::Execution(format!(
            "sandbox Docker indisponível — skill de terceiro não roda (fail-closed): {m}"
        ))),
        Err(SandboxError::Execution(m)) => Err(ToolError::Execution(format!("sandbox: {m}"))),
    }
}

/// Ponte sync→async: o `Tool::run` é síncrono, mas o `Sandbox` (bollard) é
/// async. Roda o await numa thread dedicada com runtime próprio — seguro de
/// chamar de dentro OU fora de um runtime tokio (não dá para aninhar `block_on`
/// no worker do loop do agente).
fn run_sandbox_blocking(sandbox: Sandbox, cmd: Vec<String>) -> Result<SandboxOutput, SandboxError> {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| SandboxError::Execution(format!("runtime do sandbox: {e}")))?;
        rt.block_on(sandbox.run(&cmd, &[]))
    })
    .join()
    .map_err(|_| SandboxError::Execution("thread do sandbox entrou em pânico".into()))?
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

    /// Fase 6 Onda 3, fail-closed (o guard-rail catastrófico): uma skill de
    /// terceiro (`.sandboxed()`) sem daemon Docker **não roda** — devolve erro e
    /// o entrypoint não executa (provado por um arquivo que ele criaria). Roda
    /// onde não há daemon (aqui); com daemon (CI) a skill roda confinada e o
    /// teste apenas registra isso (o caminho daemon-ausente não é exercitável).
    #[test]
    fn terceiro_sem_daemon_nao_roda_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let marca = dir.path().join("EXECUTOU");
        let entry = format!("touch \"{}\"", marca.display());
        let tool = SkillTool::new("terceiro", "d", entry, dir.path().to_path_buf()).sandboxed();
        match tool.run(&json!({"input": ""})) {
            Err(e) => {
                assert!(
                    e.to_string().contains("fail-closed") || e.to_string().contains("sandbox"),
                    "erro inesperado: {e}"
                );
                assert!(
                    !marca.exists(),
                    "sem sandbox, a skill de terceiro NÃO pode ter executado"
                );
            }
            Ok(_) => eprintln!(
                "[skill] há daemon Docker; o caminho fail-closed sem daemon não é exercitável aqui"
            ),
        }
    }

    /// Fase 6 Onda 3, o marco (critério nº 1): uma skill de terceiro roda
    /// CONFINADA e devolve seu output. Exige daemon → `#[ignore]`, roda no CI com
    /// `--include-ignored` (a lição da Onda 2).
    #[test]
    #[ignore = "execução confinada exige daemon Docker; roda no CI: cargo test -- --include-ignored"]
    fn terceiro_roda_confinado_e_devolve_output() {
        let dir = tempfile::tempdir().unwrap();
        let tool = SkillTool::new(
            "greet",
            "d",
            r#"printf 'CONFINADO:%s' "$1""#,
            dir.path().to_path_buf(),
        )
        .sandboxed();
        let out = tool.run(&json!({"input": "mundo"})).unwrap();
        assert!(
            out.content.contains("CONFINADO:mundo"),
            "output confinado: {}",
            out.content
        );
    }

    /// Fase 6 Onda 3, contenção (fronteira nº 3): uma skill que passaria o vetter
    /// estático mas em runtime tenta escrever fora do mount é CONTIDA pelo sandbox
    /// (rootfs read-only) — a 2ª camada pega o que a 1ª (estática) não vê. CI.
    #[test]
    #[ignore = "contenção exige daemon Docker; roda no CI: cargo test -- --include-ignored"]
    fn terceiro_que_abusa_e_contido_pelo_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let tool = SkillTool::new(
            "abusa",
            "d",
            "echo x > /etc/forge-escape || echo BLOQUEADO",
            dir.path().to_path_buf(),
        )
        .sandboxed();
        let out = tool.run(&json!({"input": ""})).unwrap();
        assert!(
            out.content.contains("BLOQUEADO"),
            "escrever fora do mount deveria ser bloqueado pelo rootfs read-only: {}",
            out.content
        );
    }
}
