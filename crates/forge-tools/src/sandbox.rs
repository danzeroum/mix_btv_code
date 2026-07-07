//! Sandbox Docker real (Fase 6 Onda 2), em Rust via `bollard`.
//!
//! O confinamento que os terceiros (Onda 3) vão exigir: rodar um comando num
//! contêiner com limites duros — filesystem (rootfs read-only + um único mount
//! de trabalho gravável em `/work`), rede (desabilitada por padrão), tempo e
//! memória, sem privilégios (`cap-drop ALL`, `no-new-privileges`). Traduz para
//! opções reais do bollard os quatro limites que o stub Python
//! (`forge_squad/sandbox.py`) já nomeava (timeout 30s, mem 512 MB, cpu 0.5,
//! rede off).
//!
//! **Fail-closed:** sem daemon Docker, `run` devolve
//! `SandboxError::DaemonUnavailable` — nunca um panic, nunca um "rodou"
//! silencioso. A Onda 3 decide a partir disso (terceiro não roda; built-in
//! confiável segue pelo caminho não-containerizado da Onda 1).
//!
//! **Contrato de verificação (ver testes):** os quatro vetores de contenção
//! (escrita fora do mount, rede proibida, timeout do contêiner, limite de
//! memória) exigem um daemon e vivem no CI; onde não há daemon eles **pulam com
//! log audível** — um teste de contenção que passa sem daemon seria um falso
//! positivo catastrófico. O caminho gracioso (daemon ausente → erro claro) roda
//! em qualquer lugar.

use std::path::PathBuf;
use std::time::Duration;

/// Perfil de confinamento + parâmetros de execução. O default (`new`) é a
/// superfície de segurança da fase: rede off, mem/cpu/tempo limitados, rootfs
/// read-only, sem privilégios.
#[derive(Debug, Clone)]
pub struct Sandbox {
    pub image: String,
    /// Diretório do host montado (gravável) em `/work` — o único ponto
    /// gravável; o resto do rootfs é read-only.
    pub mount: PathBuf,
    pub network_disabled: bool,
    pub mem_limit_mb: u64,
    pub cpu_quota: f64,
    pub timeout: Duration,
}

impl Sandbox {
    /// Perfil confinado padrão (os quatro limites herdados do stub Python).
    pub fn new(mount: PathBuf) -> Self {
        Self {
            image: "python:3.11-slim".to_string(),
            mount,
            network_disabled: true,
            mem_limit_mb: 512,
            cpu_quota: 0.5,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    pub fn with_network(mut self, enabled: bool) -> Self {
        self.network_disabled = !enabled;
        self
    }
    pub fn with_mem_limit_mb(mut self, mb: u64) -> Self {
        self.mem_limit_mb = mb;
        self
    }
}

/// Saída de uma execução confinada.
#[derive(Debug, Clone)]
pub struct SandboxOutput {
    pub stdout: String,
    pub exit_code: i64,
    pub timed_out: bool,
}

/// Distingue "daemon indisponível" (fail-closed — a Onda 3 precisa saber que
/// **não** rodou) de "erro de execução". A distinção é o contrato que a Onda 3
/// consome para o fail-closed de terceiros.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("daemon Docker indisponível: {0}")]
    DaemonUnavailable(String),
    #[error("execução no sandbox falhou: {0}")]
    Execution(String),
}

impl Sandbox {
    /// Só verifica se o daemon Docker responde — sem criar/subir nenhum
    /// contêiner (diferente de `run`/`run_with`, Fase 7 Onda 10, A4). Fail-
    /// closed: qualquer erro (conexão ou ping) vira `false`, nunca panic.
    /// Função associada (não `&self`): reachability do daemon não depende de
    /// nenhum campo do perfil (image/mount/limites).
    pub async fn ping() -> bool {
        match bollard::Docker::connect_with_local_defaults() {
            Ok(docker) => Self::ping_with(&docker).await,
            Err(_) => false,
        }
    }

    /// Como `ping`, mas contra um cliente já configurado — testável contra um
    /// endpoint deterministicamente morto, mesmo padrão de `run_with`.
    pub async fn ping_with(docker: &bollard::Docker) -> bool {
        docker.ping().await.is_ok()
    }

    /// Roda `cmd` (com `env`) no contêiner confinado. Async (bollard). Conecta
    /// ao daemon local; se ele não responder, devolve `DaemonUnavailable`.
    pub async fn run(
        &self,
        cmd: &[String],
        env: &[(String, String)],
    ) -> Result<SandboxOutput, SandboxError> {
        let docker = bollard::Docker::connect_with_local_defaults()
            .map_err(|e| SandboxError::DaemonUnavailable(e.to_string()))?;
        self.run_with(&docker, cmd, env).await
    }

    /// Garante que a imagem está local, puxando-a **só se ausente** (via
    /// `inspect_image`). bollard não faz pull no `create` — um sandbox de
    /// terceiro precisa disto de verdade, e é o que faltava para os testes de
    /// contenção rodarem no runner de CI (onde a imagem não vem pré-baixada).
    async fn ensure_image(&self, docker: &bollard::Docker) -> Result<(), SandboxError> {
        use bollard::image::CreateImageOptions;
        use futures_util::StreamExt;

        if docker.inspect_image(&self.image).await.is_ok() {
            return Ok(());
        }
        let (from_image, tag) = match self.image.rsplit_once(':') {
            Some((name, tag)) => (name.to_string(), tag.to_string()),
            None => (self.image.clone(), "latest".to_string()),
        };
        let mut pull = docker.create_image(
            Some(CreateImageOptions::<String> {
                from_image,
                tag,
                ..Default::default()
            }),
            None,
            None,
        );
        while let Some(item) = pull.next().await {
            item.map_err(|e| {
                SandboxError::Execution(format!("pull da imagem {}: {e}", self.image))
            })?;
        }
        Ok(())
    }

    /// Como `run`, mas contra um cliente já configurado — permite testar tanto
    /// a contenção (cliente com daemon real, no CI) quanto o caminho gracioso
    /// (cliente apontado a um socket inexistente).
    pub async fn run_with(
        &self,
        docker: &bollard::Docker,
        cmd: &[String],
        env: &[(String, String)],
    ) -> Result<SandboxOutput, SandboxError> {
        use bollard::container::{Config, KillContainerOptions, LogsOptions};
        use bollard::models::HostConfig;
        use futures_util::StreamExt;

        // Confirma que o daemon responde antes de qualquer coisa (fail-closed).
        docker
            .ping()
            .await
            .map_err(|e| SandboxError::DaemonUnavailable(e.to_string()))?;

        // bollard não faz pull no `create` — garante a imagem localmente antes.
        self.ensure_image(docker).await?;

        let env_vec: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let host_config = HostConfig {
            memory: Some((self.mem_limit_mb as i64) * 1024 * 1024),
            nano_cpus: Some((self.cpu_quota * 1_000_000_000.0) as i64),
            network_mode: Some(
                if self.network_disabled {
                    "none"
                } else {
                    "bridge"
                }
                .to_string(),
            ),
            binds: Some(vec![format!("{}:/work", self.mount.display())]),
            cap_drop: Some(vec!["ALL".to_string()]),
            security_opt: Some(vec!["no-new-privileges".to_string()]),
            readonly_rootfs: Some(true),
            ..Default::default()
        };
        let config = Config::<String> {
            image: Some(self.image.clone()),
            cmd: Some(cmd.to_vec()),
            working_dir: Some("/work".to_string()),
            env: Some(env_vec),
            // Roda como o dono do mount: com `cap_drop ALL` o root do contêiner
            // não tem CAP_DAC_OVERRIDE, então só o dono escreve em /work. Rodar
            // não-root é também melhor postura de contenção.
            user: mount_user(&self.mount),
            host_config: Some(host_config),
            ..Default::default()
        };

        let created = docker
            .create_container(
                None::<bollard::container::CreateContainerOptions<String>>,
                config,
            )
            .await
            .map_err(|e| SandboxError::Execution(format!("create: {e}")))?;
        let id = created.id;

        if let Err(e) = docker
            .start_container(
                &id,
                None::<bollard::container::StartContainerOptions<String>>,
            )
            .await
        {
            remove_quiet(docker, &id).await;
            return Err(SandboxError::Execution(format!("start: {e}")));
        }

        // Espera com o timeout do CONTÊINER (mecanismo do bollard, não o do
        // host — não confundir com `run_with_timeout`).
        let mut timed_out = false;
        let exit_code = {
            let wait = async {
                let mut stream = docker.wait_container(
                    &id,
                    None::<bollard::container::WaitContainerOptions<String>>,
                );
                stream.next().await
            };
            match tokio::time::timeout(self.timeout, wait).await {
                Ok(Some(Ok(resp))) => resp.status_code,
                // Saída != 0 vem como erro no bollard, com o código dentro.
                Ok(Some(Err(bollard::errors::Error::DockerContainerWaitError {
                    code, ..
                }))) => code,
                Ok(Some(Err(e))) => {
                    remove_quiet(docker, &id).await;
                    return Err(SandboxError::Execution(format!("wait: {e}")));
                }
                Ok(None) => -1,
                Err(_) => {
                    // Timeout: mata o contêiner.
                    timed_out = true;
                    let _ = docker
                        .kill_container(&id, None::<KillContainerOptions<String>>)
                        .await;
                    -1
                }
            }
        };

        // Colhe stdout+stderr antes de remover o contêiner.
        let mut stdout = String::new();
        {
            let mut logs = docker.logs(
                &id,
                Some(LogsOptions::<String> {
                    stdout: true,
                    stderr: true,
                    tail: "all".to_string(),
                    ..Default::default()
                }),
            );
            while let Some(chunk) = logs.next().await {
                if let Ok(out) = chunk {
                    stdout.push_str(&String::from_utf8_lossy(&out.into_bytes()));
                }
            }
        }

        remove_quiet(docker, &id).await;

        Ok(SandboxOutput {
            stdout,
            exit_code,
            timed_out,
        })
    }
}

async fn remove_quiet(docker: &bollard::Docker, id: &str) {
    use bollard::container::RemoveContainerOptions;
    let _ = docker
        .remove_container(
            id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

/// `uid:gid` do dono do mount, para o processo confinado poder escrever em
/// `/work` mesmo sob `cap_drop ALL` (sem CAP_DAC_OVERRIDE o root do contêiner
/// não escreve num mount de outro dono). `None` fora do Unix → dono default.
#[cfg(unix)]
fn mount_user(mount: &std::path::Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(mount)
        .ok()
        .map(|m| format!("{}:{}", m.uid(), m.gid()))
}

#[cfg(not(unix))]
fn mount_user(_mount: &std::path::Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roda em QUALQUER lugar (CI e local, sem daemon): um endpoint
    /// deterministicamente morto (porta 1) deve virar `DaemonUnavailable` —
    /// erro claro, nunca panic. É o caminho gracioso do fail-closed.
    #[tokio::test]
    async fn daemon_inalcancavel_vira_erro_claro_nao_panic() {
        let docker = bollard::Docker::connect_with_http(
            "http://127.0.0.1:1",
            5,
            bollard::API_DEFAULT_VERSION,
        )
        .expect("connect_with_http só configura o cliente");
        let sb = Sandbox::new(std::env::temp_dir());
        let err = sb
            .run_with(&docker, &["echo".into(), "oi".into()], &[])
            .await
            .unwrap_err();
        assert!(
            matches!(err, SandboxError::DaemonUnavailable(_)),
            "esperava DaemonUnavailable, veio {err:?}"
        );
    }

    /// `ping_with` contra um endpoint deterministicamente morto — a mesma
    /// prova de `daemon_inalcancavel_vira_erro_claro_nao_panic`, mas para o
    /// caminho de leitura (Fase 7 Onda 10, A4): honesto `false`, nunca panic,
    /// sem depender de Docker estar instalado neste ambiente.
    #[tokio::test]
    async fn ping_com_daemon_inalcancavel_e_false() {
        let docker = bollard::Docker::connect_with_http(
            "http://127.0.0.1:1",
            5,
            bollard::API_DEFAULT_VERSION,
        )
        .expect("connect_with_http só configura o cliente");
        assert!(!Sandbox::ping_with(&docker).await);
    }

    /// Os testes de contenção são `#[ignore]`: no `cargo test` local aparecem
    /// como "ignored" (visíveis — NUNCA verdes por engano). O CI os roda com
    /// `--include-ignored` num runner COM daemon. Se rodarem sem daemon, este
    /// helper FALHA — nunca "passam" sem ter containerizado nada (o guard-rail
    /// da onda de segurança: um teste de contenção verde sem daemon é um falso
    /// positivo catastrófico).
    async fn daemon_obrigatorio() -> bollard::Docker {
        let d = bollard::Docker::connect_with_local_defaults().expect(
            "daemon Docker é obrigatório para a contenção (rode no CI com --include-ignored)",
        );
        d.ping()
            .await
            .expect("daemon Docker não respondeu — a contenção não pode ser verificada sem ele");
        d
    }

    /// Contenção nº 1: escrever no mount (`/work`) funciona; escrever fora dele
    /// (rootfs read-only) falha. O mesmo comando fora do sandbox teria sucesso.
    #[tokio::test]
    #[ignore = "contenção exige daemon Docker; roda no CI: cargo test -- --include-ignored"]
    async fn contencao_escrita_fora_do_mount_e_bloqueada() {
        let docker = daemon_obrigatorio().await;
        let dir = tempfile::tempdir().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf());
        let dentro = sb
            .run_with(
                &docker,
                &[
                    "sh".into(),
                    "-c".into(),
                    "echo ok > /work/f && cat /work/f".into(),
                ],
                &[],
            )
            .await
            .unwrap();
        assert_eq!(dentro.exit_code, 0, "escrever no mount deveria funcionar");
        assert!(dentro.stdout.contains("ok"));

        let fora = sb
            .run_with(
                &docker,
                &["sh".into(), "-c".into(), "echo x > /forbidden".into()],
                &[],
            )
            .await
            .unwrap();
        assert_ne!(
            fora.exit_code, 0,
            "escrever no rootfs read-only deveria ser bloqueado"
        );
    }

    /// Contenção nº 2: rede desabilitada por padrão — resolver/conectar falha.
    #[tokio::test]
    #[ignore = "contenção exige daemon Docker; roda no CI: cargo test -- --include-ignored"]
    async fn contencao_rede_proibida() {
        let docker = daemon_obrigatorio().await;
        let dir = tempfile::tempdir().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf());
        let r = sb
            .run_with(
                &docker,
                &[
                    "sh".into(),
                    "-c".into(),
                    "getent hosts example.com || exit 42".into(),
                ],
                &[],
            )
            .await
            .unwrap();
        assert_ne!(r.exit_code, 0, "rede off: a resolução deveria falhar");
    }

    /// Contenção nº 3: o timeout do contêiner mata um comando que dorme além do
    /// limite (mecanismo do bollard, não do host).
    #[tokio::test]
    #[ignore = "contenção exige daemon Docker; roda no CI: cargo test -- --include-ignored"]
    async fn contencao_timeout_do_conteiner() {
        let docker = daemon_obrigatorio().await;
        let dir = tempfile::tempdir().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf()).with_timeout(Duration::from_secs(2));
        let r = sb
            .run_with(&docker, &["sh".into(), "-c".into(), "sleep 30".into()], &[])
            .await
            .unwrap();
        assert!(
            r.timed_out,
            "o contêiner deveria ter sido morto pelo timeout"
        );
    }

    /// Contenção nº 4: estourar o limite de memória é morto pelo cgroup.
    #[tokio::test]
    #[ignore = "contenção exige daemon Docker; roda no CI: cargo test -- --include-ignored"]
    async fn contencao_limite_de_memoria() {
        let docker = daemon_obrigatorio().await;
        let dir = tempfile::tempdir().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf()).with_mem_limit_mb(64);
        // Aloca ~512 MB com o limite em 64 MB → OOM-killed pelo cgroup.
        let r = sb
            .run_with(
                &docker,
                &[
                    "python3".into(),
                    "-c".into(),
                    "bytearray(512*1024*1024)".into(),
                ],
                &[],
            )
            .await
            .unwrap();
        assert_ne!(
            r.exit_code, 0,
            "estourar a memória deveria ser morto pelo cgroup"
        );
    }
}
