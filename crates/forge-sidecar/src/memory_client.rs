//! Cliente e supervisor do sidecar Python `forge_squad.memory_server`
//! (Fase 7 Onda 8, ADR 0022): `MemoryService.Recall`/`List` sobre o corpus
//! episódico de `AgentMemorySystem` — o Python continua dono do dado (não
//! precisa de `--core-socket`: ao contrário do `forge_squad.server`, este
//! sidecar não chama de volta o Rust, só lê o que o orquestrador já gravou
//! em disco).

use crate::client::{socket_ready, SidecarError};
use forge_proto::memory::memory_service_client::MemoryServiceClient;
use forge_proto::memory::{
    HealthRequest, ListRequest, ListResponse, RecallRequest, RecallResponse,
};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tonic::transport::{Channel, Endpoint, Uri};
use tonic::Request;

#[derive(Clone, Debug)]
pub struct MemoryClient {
    inner: MemoryServiceClient<Channel>,
}

impl MemoryClient {
    pub async fn connect(path: impl Into<PathBuf>) -> Result<Self, SidecarError> {
        let path = path.into();
        let channel = Endpoint::try_from("http://memory.invalid")
            .expect("URI placeholder válida")
            .connect_with_connector(tower::service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    let stream = tokio::net::UnixStream::connect(path).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| SidecarError::Unavailable(e.to_string()))?;
        Ok(Self {
            inner: MemoryServiceClient::new(channel),
        })
    }

    pub async fn health(&mut self) -> Result<(bool, String), SidecarError> {
        let resp = self
            .inner
            .health(Request::new(HealthRequest {}))
            .await?
            .into_inner();
        Ok((resp.ready, resp.version))
    }

    pub async fn recall(&mut self, query: &str, k: u32) -> Result<RecallResponse, SidecarError> {
        let resp = self
            .inner
            .recall(Request::new(RecallRequest {
                query: query.to_string(),
                k,
            }))
            .await?;
        Ok(resp.into_inner())
    }

    pub async fn list(
        &mut self,
        agent: Option<String>,
        limit: u32,
    ) -> Result<ListResponse, SidecarError> {
        let resp = self
            .inner
            .list(Request::new(ListRequest { agent, limit }))
            .await?;
        Ok(resp.into_inner())
    }
}

pub struct MemorySupervisor {
    child: Child,
    socket_path: PathBuf,
}

impl MemorySupervisor {
    /// Spawna `uv run python -m forge_squad.memory_server --socket <path>
    /// [--memory-dir <dir>]` no diretório do workspace Python.
    ///
    /// `memory_dir: None` é a escolha certa em produção: `forge_squad.
    /// server`'s `SquadServicer` (que é quem de fato ESCREVE a memória, via
    /// `remember_decision`) nunca recebe `--memory-dir` hoje — cai no
    /// default de `AgentMemorySystem()` (`.forge/squad-memory` relativo ao
    /// `current_dir` do processo, que é o MESMO `python_workspace_dir`
    /// passado aqui). Sem essa simetria, este serviço leria um corpus
    /// diferente do que o squad real escreve. `Some(dir)` existe para
    /// testes apontarem a um corpus isolado e determinístico.
    pub fn spawn(
        python_workspace_dir: &Path,
        socket_path: PathBuf,
        memory_dir: Option<&Path>,
    ) -> Result<Self, SidecarError> {
        // Mesmo achado já corrigido em `SidecarSupervisor`/`SquadSupervisor`:
        // o diretório do socket é responsabilidade de quem sobe o processo.
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&socket_path);
        let mut cmd = Command::new("uv");
        cmd.args([
            "run",
            "python",
            "-m",
            "forge_squad.memory_server",
            "--socket",
        ])
        .arg(&socket_path);
        if let Some(dir) = memory_dir {
            cmd.arg("--memory-dir").arg(dir);
        }
        cmd.current_dir(python_workspace_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Mesmo motivo de `SidecarSupervisor`/`SquadSupervisor`: `uv run`
        // reforka o Python como filho — sem isto, matar só o `uv` deixaria
        // o servidor de memória órfão.
        #[cfg(unix)]
        cmd.process_group(0);
        let child = cmd
            .spawn()
            .map_err(|e| SidecarError::Unavailable(format!("spawn do memory sidecar: {e}")))?;
        Ok(Self { child, socket_path })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn kill(&mut self) -> std::io::Result<()> {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        self.child.kill().await
    }

    pub async fn wait_ready(&mut self, timeout: Duration) -> Result<MemoryClient, SidecarError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|e| SidecarError::Unavailable(e.to_string()))?
            {
                let mut stderr = String::new();
                if let Some(mut pipe) = self.child.stderr.take() {
                    use tokio::io::AsyncReadExt;
                    let _ = pipe.read_to_string(&mut stderr).await;
                }
                return Err(SidecarError::Unavailable(format!(
                    "memory sidecar encerrou antes de ficar pronto (status: {status}); stderr: {stderr}"
                )));
            }
            if socket_ready(&self.socket_path) {
                if let Ok(mut client) = MemoryClient::connect(self.socket_path.clone()).await {
                    if let Ok((true, _)) = client.health().await {
                        return Ok(client);
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(SidecarError::Unavailable(format!(
                    "timeout de {timeout:?} esperando o memory sidecar iniciar"
                )));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Drop for MemorySupervisor {
    /// Mesmo achado (Onda 4/5): `kill_on_drop` só sinaliza o `uv` imediato;
    /// `process_group(0)` no `spawn` + `kill()` de grupo aqui evita órfão.
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn processo_inexistente_falha_rapido_com_unavailable() {
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
        let mut sup = MemorySupervisor::spawn(dir.path(), sock, None).unwrap();
        let err = sup.wait_ready(Duration::from_secs(5)).await.unwrap_err();
        assert!(matches!(err, SidecarError::Unavailable(_)));
    }
}
