//! Cliente e supervisor do sidecar Python `forge_squad.server` (Onda 4d):
//! `SquadService.ExecuteTask` devolve um stream de `SquadEvent` que o
//! `forge squad` renderiza ao vivo. O supervisor spawna o processo com
//! dois sockets — o seu (`--socket`) e o do `CoreService` Rust
//! (`--core-socket`), fechando o laço bidirecional.

use crate::client::{socket_ready, SidecarError};
use forge_proto::squad::squad_service_client::SquadServiceClient;
use forge_proto::squad::{HealthRequest, SquadEvent, SquadTask};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tonic::transport::{Channel, Endpoint, Uri};
use tonic::{Request, Streaming};

#[derive(Clone, Debug)]
pub struct SquadClient {
    inner: SquadServiceClient<Channel>,
}

impl SquadClient {
    pub async fn connect(path: impl Into<PathBuf>) -> Result<Self, SidecarError> {
        let path = path.into();
        let channel = Endpoint::try_from("http://squad.invalid")
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
            inner: SquadServiceClient::new(channel),
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

    /// Inicia uma tarefa no squad e devolve o stream de `SquadEvent`.
    pub async fn execute_task(
        &mut self,
        task: SquadTask,
    ) -> Result<Streaming<SquadEvent>, SidecarError> {
        let resp = self.inner.execute_task(Request::new(task)).await?;
        Ok(resp.into_inner())
    }
}

pub struct SquadSupervisor {
    child: Child,
    socket_path: PathBuf,
}

impl SquadSupervisor {
    /// Spawna `uv run python -m forge_squad.server --socket <squad> --core-socket <core>`
    /// no diretório do workspace Python.
    pub fn spawn(
        python_workspace_dir: &Path,
        socket_path: PathBuf,
        core_socket: &Path,
        model: &str,
    ) -> Result<Self, SidecarError> {
        let _ = std::fs::remove_file(&socket_path);
        let child = Command::new("uv")
            .args(["run", "python", "-m", "forge_squad.server", "--socket"])
            .arg(&socket_path)
            .arg("--core-socket")
            .arg(core_socket)
            .arg("--model")
            .arg(model)
            .current_dir(python_workspace_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| SidecarError::Unavailable(format!("spawn do squad: {e}")))?;
        Ok(Self { child, socket_path })
    }

    /// Poll do socket + health check até `timeout`; devolve um cliente
    /// pronto ou `Unavailable` (inclusive se o processo morreu antes).
    pub async fn wait_ready(&mut self, timeout: Duration) -> Result<SquadClient, SidecarError> {
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
                    "squad encerrou antes de ficar pronto (status: {status}); stderr: {stderr}"
                )));
            }
            if socket_ready(&self.socket_path) {
                if let Ok(mut client) = SquadClient::connect(self.socket_path.clone()).await {
                    if let Ok((true, _)) = client.health().await {
                        return Ok(client);
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(SidecarError::Unavailable(format!(
                    "timeout de {timeout:?} esperando o squad iniciar"
                )));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
