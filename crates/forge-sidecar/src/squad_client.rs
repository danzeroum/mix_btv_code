//! Cliente e supervisor do sidecar Python `forge_squad.server` (Onda 4d):
//! `SquadService.ExecuteTask` devolve um stream de `SquadEvent` que o
//! `forge squad` renderiza ao vivo. O supervisor spawna o processo com
//! dois sockets — o seu (`--socket`) e o do `CoreService` Rust
//! (`--core-socket`), fechando o laço bidirecional.

use crate::client::{socket_ready, SidecarError};
use forge_proto::squad::squad_service_client::SquadServiceClient;
use forge_proto::squad::{squad_event, HealthRequest, SquadEvent, SquadTask};
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

/// Resultado de drenar o stream do squad — a base do fallback progressivo.
#[derive(Debug)]
pub enum SquadRun {
    /// O stream completou normalmente; eventos coletados.
    Completed(Vec<SquadEvent>),
    /// O stream quebrou (ex.: sidecar morto com `kill -9`) ou o squad
    /// emitiu um `SquadEvent::error` — quem chama deve degradar para o
    /// nível seguinte (agente-único → safe-mode).
    Failed {
        events: Vec<SquadEvent>,
        reason: String,
    },
}

/// Drena o stream do squad até o fim ou até a primeira falha. Uma quebra
/// de transporte (o processo Python morto no meio da execução) vira
/// `Failed` via `Err(Status)`; um `error` no stream vira `Failed` pelo
/// conteúdo — os dois disparam o fallback.
pub async fn drain_stream(mut stream: Streaming<SquadEvent>) -> SquadRun {
    let mut events = Vec::new();
    loop {
        match stream.message().await {
            Ok(Some(ev)) => {
                if let Some(squad_event::Payload::Error(reason)) = &ev.payload {
                    let reason = reason.clone();
                    events.push(ev);
                    return SquadRun::Failed { events, reason };
                }
                events.push(ev);
            }
            Ok(None) => return SquadRun::Completed(events),
            Err(status) => {
                return SquadRun::Failed {
                    events,
                    reason: status.to_string(),
                }
            }
        }
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
        // O diretório do socket é responsabilidade de quem sobe o processo,
        // não de quem chama `spawn` — um `SquadPool` com N slots, por
        // exemplo, não deveria precisar saber que precisa criar o diretório
        // de antemão (achado real: faltava isso, o bind gRPC do lado Python
        // falhava com "No such file or directory" antes desta linha existir).
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&socket_path);
        let mut cmd = Command::new("uv");
        cmd.args(["run", "python", "-m", "forge_squad.server", "--socket"])
            .arg(&socket_path)
            .arg("--core-socket")
            .arg(core_socket)
            .arg("--model")
            .arg(model)
            .current_dir(python_workspace_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // `uv run` spawna o Python como filho: matar só o processo `uv`
        // deixaria o servidor Python órfão (rodando). Colocando o `uv` como
        // líder do próprio grupo de processos, `kill()` sinaliza o grupo
        // inteiro (uv + python).
        #[cfg(unix)]
        cmd.process_group(0);
        let child = cmd
            .spawn()
            .map_err(|e| SidecarError::Unavailable(format!("spawn do squad: {e}")))?;
        Ok(Self { child, socket_path })
    }

    /// PID do processo supervisionado — usado pelo `SquadPool` (Onda 3)
    /// para provar estabilidade entre chamadas e detectar troca após um
    /// restart.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Mata o squad (SIGKILL). Em Unix, sinaliza o **grupo** de processos
    /// (uv + python), não só o wrapper `uv` — do contrário o servidor
    /// Python ficaria órfão. Usado para injeção de falha nos testes de
    /// fallback e para encerramento explícito.
    pub async fn kill(&mut self) -> std::io::Result<()> {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            // Grupo == pid do líder (process_group(0) no spawn). Sinal
            // negativo = grupo inteiro.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        self.child.kill().await
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

impl Drop for SquadSupervisor {
    /// Achado real (Onda 4, `squad_agent`'s e2e): `kill_on_drop(true)` do
    /// tokio só sinaliza o processo IMEDIATO (`uv`) — como `uv run` reforka
    /// o Python como filho, isso deixava `forge_squad.server` **órfão
    /// rodando para sempre** toda vez que um `SquadSupervisor` era dropado
    /// sem `.kill()` explícito antes (o caso comum: fim de teste, slot do
    /// `SquadPool` sendo substituído após detectar queda). `process_group(0)`
    /// no `spawn` já deixava `uv` líder do próprio grupo — faltava usar
    /// isso também no `Drop`, não só no `kill()` explícito. `Drop` não pode
    /// ser `async`, então o sinal é enviado aqui de forma síncrona
    /// (`libc::kill` não bloqueia) antes do reaper do tokio agir sobre o
    /// processo imediato.
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}
