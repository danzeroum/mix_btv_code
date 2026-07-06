//! Sobe e supervisiona o sidecar Python (`forge_promptforge.server`) e
//! espera o health check antes de devolver um cliente pronto para uso.
//!
//! Fallback progressivo (ADR 0001/0002): se o sidecar não subir a tempo,
//! quem chama recebe `SidecarError::Unavailable` e decide degradar (pular
//! lint/geradores) em vez de falhar a tarefa do usuário.

use crate::client::{socket_ready, SidecarClient, SidecarError};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};

pub struct SidecarSupervisor {
    child: Child,
    socket_path: PathBuf,
}

impl SidecarSupervisor {
    /// Spawna `uv run python -m forge_promptforge.server --socket <path>`
    /// no diretório do workspace Python. Não bloqueia — use
    /// [`Self::wait_ready`] para esperar o sidecar responder.
    pub fn spawn(python_workspace_dir: &Path, socket_path: PathBuf) -> Result<Self, SidecarError> {
        let _ = std::fs::remove_file(&socket_path); // socket de uma execução anterior
        let mut cmd = Command::new("uv");
        cmd.args([
            "run",
            "python",
            "-m",
            "forge_promptforge.server",
            "--socket",
        ])
        .arg(&socket_path)
        .current_dir(python_workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
        // `uv run` spawna o Python como filho: matar só o processo `uv`
        // deixaria o servidor Python órfão (mesmo achado do `SquadSupervisor`,
        // squad_client.rs). Líder do próprio grupo → `kill()` sinaliza os dois.
        #[cfg(unix)]
        cmd.process_group(0);
        let child = cmd
            .spawn()
            .map_err(|e| SidecarError::Unavailable(format!("spawn do sidecar: {e}")))?;
        Ok(Self { child, socket_path })
    }

    /// PID do processo supervisionado — usado pelo serviço de longa duração
    /// (Onda 3) para provar estabilidade entre chamadas (mesmo PID) e
    /// detectar troca após um restart (PID novo).
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Mata o processo supervisionado (SIGKILL no grupo inteiro em Unix —
    /// ver comentário em `spawn`). Usado para reinício sob demanda e para
    /// injeção de falha em teste (mesmo padrão de `SquadSupervisor::kill`).
    pub async fn kill(&mut self) -> std::io::Result<()> {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        self.child.kill().await
    }

    /// Faz poll do socket + health check até `timeout`. Devolve um cliente
    /// já validado (ready == true) ou `Unavailable` se o prazo esgotar —
    /// inclusive se o processo morreu antes (stderr é incluído no erro).
    pub async fn wait_ready(&mut self, timeout: Duration) -> Result<SidecarClient, SidecarError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|e| SidecarError::Unavailable(e.to_string()))?
            {
                return Err(SidecarError::Unavailable(format!(
                    "sidecar encerrou antes de ficar pronto (status: {status})"
                )));
            }
            if socket_ready(&self.socket_path) {
                if let Ok(mut client) = SidecarClient::connect(self.socket_path.clone()).await {
                    if let Ok((true, _)) = client.health().await {
                        return Ok(client);
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(SidecarError::Unavailable(format!(
                    "timeout de {timeout:?} esperando o sidecar iniciar"
                )));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn processo_inexistente_falha_rapido_com_unavailable() {
        // Mesmo padrão de pulo gracioso de `python_sidecar.rs`/`squad_e2e.rs`:
        // sem "uv" no ambiente, `spawn()` já falha no `Command::spawn()` (erro
        // de processo inexistente, não o que este teste quer exercitar), então
        // pular é mais correto que um unwrap ambíguo.
        if std::process::Command::new("uv")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("uv ausente no ambiente — pulando teste");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("s.sock");
        // "uv" existe no ambiente de dev, mas sem o pacote forge_promptforge
        // instalado num diretório vazio o processo Python vai falhar/sair —
        // exercita o caminho de "sidecar encerrou antes de ficar pronto".
        let mut sup = SidecarSupervisor::spawn(dir.path(), sock).unwrap();
        let err = sup.wait_ready(Duration::from_secs(5)).await.unwrap_err();
        assert!(matches!(err, SidecarError::Unavailable(_)));
    }
}
