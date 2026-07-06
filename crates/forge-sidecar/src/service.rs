//! Camada de "serviço de longa duração" para os sidecars Python — Fase 7
//! Onda 3. Distinta da camada CLI existente (`supervisor.rs`/
//! `squad_client::SquadSupervisor`, spawn por invocação com
//! `kill_on_drop`, sempre recriada): aqui o processo sobe uma vez e fica
//! vivo entre requisições, com health-check + restart-on-crash — pensada
//! para o processo de longa duração do `forge dashboard`. O CLI de
//! invocação única continua usando `SidecarSupervisor::spawn`/
//! `SquadSupervisor::spawn` diretamente, intocado.
//!
//! Dois desenhos distintos, por design (ADR 0019):
//! - [`SidecarService`] (PromptForge): instância ÚNICA compartilhada — o
//!   sidecar é stateless (lint/render/list_generators não têm estado entre
//!   chamadas), então serializar um `render` por vez é aceitável.
//! - [`SquadPool`]: pool pequeno com limite — squad é execução longa
//!   (múltiplos agentes, múltiplas chamadas de LLM); um processo só
//!   serializaria squads concorrentes à toa.

use crate::client::{SidecarClient, SidecarError};
use crate::memory_client::{MemoryClient, MemorySupervisor};
use crate::squad_client::{SquadClient, SquadSupervisor};
use crate::supervisor::SidecarSupervisor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

struct SidecarState {
    supervisor: SidecarSupervisor,
    client: SidecarClient,
}

/// Serviço de longa duração para o sidecar PromptForge: sobe o processo na
/// primeira chamada e o mantém vivo entre requisições subsequentes.
///
/// `client()` é serializado por um `tokio::sync::Mutex` — não é um detalhe
/// de implementação incidental, é a política declarada (stateless,
/// serializar um `render` por vez é aceitável) em vez de uma pool.
pub struct SidecarService {
    python_workspace_dir: PathBuf,
    socket_path: PathBuf,
    ready_timeout: Duration,
    state: Mutex<Option<SidecarState>>,
}

impl SidecarService {
    pub fn new(
        python_workspace_dir: PathBuf,
        socket_path: PathBuf,
        ready_timeout: Duration,
    ) -> Self {
        Self {
            python_workspace_dir,
            socket_path,
            ready_timeout,
            state: Mutex::new(None),
        }
    }

    /// Devolve um cliente pronto para uso — reusa o processo vivo se o
    /// health-check passar; sobe um processo novo (PID diferente) se for a
    /// primeira chamada ou se o anterior tiver morrido (health-check falha
    /// porque o socket não responde mais — o mesmo sinal que `wait_ready`
    /// já usa para detectar prontidão).
    pub async fn client(&self) -> Result<SidecarClient, SidecarError> {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.as_mut() {
            if let Ok((true, _)) = state.client.health().await {
                return Ok(state.client.clone());
            }
        }
        let mut supervisor =
            SidecarSupervisor::spawn(&self.python_workspace_dir, self.socket_path.clone())?;
        let client = supervisor.wait_ready(self.ready_timeout).await?;
        let ready = client.clone();
        *guard = Some(SidecarState { supervisor, client });
        Ok(ready)
    }

    /// PID do processo atualmente supervisionado, se algum já subiu.
    pub async fn current_pid(&self) -> Option<u32> {
        self.state
            .lock()
            .await
            .as_ref()
            .and_then(|s| s.supervisor.pid())
    }

    /// Mata o processo supervisionado agora, se algum estiver de pé — a
    /// próxima chamada a `client()` sobe um processo novo. Usado para
    /// reinício sob demanda e para testes injetarem uma queda.
    pub async fn kill_current(&self) -> std::io::Result<()> {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.as_mut() {
            state.supervisor.kill().await?;
        }
        Ok(())
    }
}

struct MemoryState {
    supervisor: MemorySupervisor,
    client: MemoryClient,
}

/// Serviço de longa duração para o sidecar de memória do squad (Fase 7
/// Onda 8, ADR 0022) — instância ÚNICA compartilhada, mesmo desenho de
/// [`SidecarService`]: `Recall`/`List` são leituras stateless sobre o
/// corpus episódico, então serializar uma consulta por vez é aceitável (e
/// deliberadamente NÃO usa o `SquadPool` — misturaria disputa de recurso
/// entre leitura de memória e execução real de squad à toa).
pub struct MemoryService {
    python_workspace_dir: PathBuf,
    socket_path: PathBuf,
    /// `None` em produção (mesma resolução relativa que `SquadServicer`
    /// já usa — ver doc de `MemorySupervisor::spawn`); `Some` em testes,
    /// para um corpus isolado e determinístico.
    memory_dir: Option<PathBuf>,
    ready_timeout: Duration,
    state: Mutex<Option<MemoryState>>,
}

impl MemoryService {
    pub fn new(
        python_workspace_dir: PathBuf,
        socket_path: PathBuf,
        memory_dir: Option<PathBuf>,
        ready_timeout: Duration,
    ) -> Self {
        Self {
            python_workspace_dir,
            socket_path,
            memory_dir,
            ready_timeout,
            state: Mutex::new(None),
        }
    }

    pub async fn client(&self) -> Result<MemoryClient, SidecarError> {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.as_mut() {
            if let Ok((true, _)) = state.client.health().await {
                return Ok(state.client.clone());
            }
        }
        let mut supervisor = MemorySupervisor::spawn(
            &self.python_workspace_dir,
            self.socket_path.clone(),
            self.memory_dir.as_deref(),
        )?;
        let client = supervisor.wait_ready(self.ready_timeout).await?;
        let ready = client.clone();
        *guard = Some(MemoryState { supervisor, client });
        Ok(ready)
    }

    pub async fn current_pid(&self) -> Option<u32> {
        self.state
            .lock()
            .await
            .as_ref()
            .and_then(|s| s.supervisor.pid())
    }

    pub async fn kill_current(&self) -> std::io::Result<()> {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.as_mut() {
            state.supervisor.kill().await?;
        }
        Ok(())
    }
}

struct SquadSlotState {
    supervisor: SquadSupervisor,
    client: SquadClient,
}

/// Pool pequeno de processos `forge_squad.server` — cada slot é um
/// processo próprio, gated por um `Semaphore` de capacidade fixa:
/// `acquire()` nunca sobe mais que `capacity` processos concorrentes, e
/// espera (sem falhar) se todos estiverem ocupados.
pub struct SquadPool {
    python_workspace_dir: PathBuf,
    core_socket: PathBuf,
    model: String,
    ready_timeout: Duration,
    slot_sockets: Vec<PathBuf>,
    slot_states: Vec<Mutex<Option<SquadSlotState>>>,
    semaphore: Arc<Semaphore>,
    /// Índices de slot livres — sincronizado 1:1 com os permits do
    /// semáforo (um permit obtido sempre corresponde a um índice
    /// disponível aqui; devolvido junto no `Drop` do lease).
    free: std::sync::Mutex<Vec<usize>>,
}

impl SquadPool {
    pub fn new(
        python_workspace_dir: PathBuf,
        socket_dir: PathBuf,
        core_socket: PathBuf,
        model: String,
        capacity: usize,
        ready_timeout: Duration,
    ) -> Self {
        assert!(capacity > 0, "pool de squad precisa de ao menos 1 slot");
        let slot_sockets = (0..capacity)
            .map(|i| socket_dir.join(format!("squad-slot-{i}.sock")))
            .collect();
        let slot_states = (0..capacity).map(|_| Mutex::new(None)).collect();
        Self {
            python_workspace_dir,
            core_socket,
            model,
            ready_timeout,
            slot_sockets,
            slot_states,
            semaphore: Arc::new(Semaphore::new(capacity)),
            free: std::sync::Mutex::new((0..capacity).collect()),
        }
    }

    pub fn capacity(&self) -> usize {
        self.slot_sockets.len()
    }

    /// Espera até haver um slot livre, garante que o processo daquele slot
    /// está vivo (sobe um novo, PID diferente, se tiver morrido) e devolve
    /// um lease — o slot volta para a lista de livres quando o lease sai de
    /// escopo. `self: &Arc<Self>` (não `&self`) para o lease poder ser
    /// `'static` (movível para dentro de uma task spawnada, ex.: a duração
    /// inteira de uma execução de squad).
    pub async fn acquire(self: &Arc<Self>) -> Result<SquadLease, SidecarError> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .expect("semaphore do SquadPool nunca é fechado");
        let slot = {
            let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
            free.pop()
                .expect("permit obtido implica slot livre disponível (invariante 1:1)")
        };

        let mut state = self.slot_states[slot].lock().await;
        if let Some(s) = state.as_mut() {
            if let Ok((true, _)) = s.client.health().await {
                let client = s.client.clone();
                drop(state);
                return Ok(SquadLease {
                    pool: Arc::clone(self),
                    slot,
                    client,
                    _permit: permit,
                });
            }
        }
        let mut supervisor = SquadSupervisor::spawn(
            &self.python_workspace_dir,
            self.slot_sockets[slot].clone(),
            &self.core_socket,
            &self.model,
        )?;
        let client = supervisor.wait_ready(self.ready_timeout).await?;
        let ready = client.clone();
        *state = Some(SquadSlotState { supervisor, client });
        drop(state);
        Ok(SquadLease {
            pool: Arc::clone(self),
            slot,
            client: ready,
            _permit: permit,
        })
    }

    /// PID do processo do slot, se algum já subiu — observabilidade e teste.
    pub async fn pid_of(&self, slot: usize) -> Option<u32> {
        self.slot_states[slot]
            .lock()
            .await
            .as_ref()
            .and_then(|s| s.supervisor.pid())
    }

    /// Mata o processo do slot agora, se algum estiver de pé — o próximo
    /// `acquire()` que caia nesse slot sobe um processo novo. Usado para
    /// reinício sob demanda e para testes injetarem uma queda.
    pub async fn kill_slot(&self, slot: usize) -> std::io::Result<()> {
        let mut state = self.slot_states[slot].lock().await;
        if let Some(s) = state.as_mut() {
            s.supervisor.kill().await?;
        }
        Ok(())
    }
}

/// Posse temporária de um slot do [`SquadPool`] — devolve o slot à lista de
/// livres quando dropado (sucesso, erro ou panic do chamador).
pub struct SquadLease {
    pool: Arc<SquadPool>,
    slot: usize,
    client: SquadClient,
    _permit: OwnedSemaphorePermit,
}

impl SquadLease {
    pub fn client(&self) -> &SquadClient {
        &self.client
    }

    pub fn client_mut(&mut self) -> &mut SquadClient {
        &mut self.client
    }

    pub fn slot(&self) -> usize {
        self.slot
    }
}

impl Drop for SquadLease {
    fn drop(&mut self) {
        let mut free = self.pool.free.lock().unwrap_or_else(|e| e.into_inner());
        free.push(self.slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python_workspace_dir() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../python"))
    }

    fn uv_missing() -> bool {
        std::process::Command::new("uv")
            .arg("--version")
            .output()
            .is_err()
    }

    /// Fronteira da Onda 3 (PromptForge): 3 requisições sequenciais não
    /// reabrem o processo (PID estável); `kill_current` simula uma queda
    /// (mesmo padrão de `squad_e2e.rs`); a próxima chamada detecta via
    /// health-check e sobe um processo novo (PID diferente) sem que o
    /// servidor Rust em si precise reiniciar.
    #[tokio::test]
    async fn sidecar_service_reusa_processo_e_se_recupera_de_kill() {
        let dir = python_workspace_dir();
        if uv_missing() || !dir.join("pyproject.toml").exists() {
            eprintln!("uv/workspace Python ausente — pulando teste de serviço real");
            return;
        }
        let socket =
            std::env::temp_dir().join(format!("forge-sidecar-service-{}.sock", std::process::id()));
        let service = SidecarService::new(dir, socket, Duration::from_secs(30));

        let mut client1 = service
            .client()
            .await
            .expect("primeira chamada sobe o processo");
        let pid1 = service.current_pid().await.expect("pid após subir");
        assert!(client1.health().await.unwrap().0);

        let mut client2 = service.client().await.expect("segunda chamada reusa");
        assert_eq!(
            service.current_pid().await,
            Some(pid1),
            "PID deveria ser estável"
        );
        assert!(client2.health().await.unwrap().0);

        let mut client3 = service.client().await.expect("terceira chamada reusa");
        assert_eq!(
            service.current_pid().await,
            Some(pid1),
            "PID ainda estável na 3ª chamada"
        );
        assert!(client3.health().await.unwrap().0);

        service
            .kill_current()
            .await
            .expect("kill deveria funcionar");
        // Pequena espera para o SO processar a morte antes do próximo health-check.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut client4 = service
            .client()
            .await
            .expect("quarta chamada sobe um processo novo");
        let pid2 = service.current_pid().await.expect("pid após restart");
        assert_ne!(pid1, pid2, "PID deveria mudar após o restart");
        assert!(
            client4.health().await.unwrap().0,
            "o processo novo deveria responder"
        );
    }

    /// Fronteira da Onda 8 (`MemoryService`, ADR 0022): mesmo contrato do
    /// `SidecarService` (singleton, PID estável entre chamadas, restart
    /// após `kill_current`), agora provado sobre um sidecar de memória REAL
    /// — e, além da estabilidade do processo, que `Recall`/`List` batem com
    /// o que foi escrito diretamente no JSONL episódico (mesmo formato que
    /// `AgentMemorySystem.remember_decision` grava), fechando o laço
    /// gRPC-Rust ↔ corpus-Python de ponta a ponta.
    #[tokio::test]
    async fn memory_service_reusa_processo_e_recall_list_batem_com_o_corpus_real() {
        let dir = python_workspace_dir();
        if uv_missing() || !dir.join("pyproject.toml").exists() {
            eprintln!("uv/workspace Python ausente — pulando teste de memory service real");
            return;
        }
        let memory_dir = std::env::temp_dir().join(format!(
            "forge-memory-service-{}-{}",
            std::process::id(),
            "corpus"
        ));
        std::fs::create_dir_all(&memory_dir).unwrap();
        let episodic_path = memory_dir.join("agent_memories.jsonl");
        std::fs::write(
            &episodic_path,
            concat!(
                r#"{"timestamp":"2026-01-01T00:00:00Z","agent":"architect","decision":{"summary":"corrigir login e senha do usuário"},"confidence":0.9}"#,
                "\n",
                r#"{"timestamp":"2026-01-01T00:00:01Z","agent":"architect","decision":{"summary":"isolar o contêiner docker sem rede"},"confidence":0.4}"#,
                "\n",
            ),
        )
        .unwrap();

        let socket =
            std::env::temp_dir().join(format!("forge-memory-service-{}.sock", std::process::id()));
        // `Some(memory_dir)` aponta o `--memory-dir` do sidecar Python pro
        // mesmo diretório do JSONL semeado acima — o processo abre o
        // corpus real, não recria vazio (produção usa `None`, ver doc de
        // `MemoryService`).
        let service = MemoryService::new(dir, socket, Some(memory_dir), Duration::from_secs(30));
        let mut client1 = service
            .client()
            .await
            .expect("primeira chamada sobe o processo");
        let pid1 = service.current_pid().await.expect("pid após subir");
        assert!(client1.health().await.unwrap().0);

        let recall = client1
            .recall("problema de login e senha", 3)
            .await
            .expect("recall deveria funcionar");
        assert_eq!(recall.matches.len(), 1, "só a memória de login é relevante");
        assert!(recall.matches[0].decision_json.contains("login"));
        assert_eq!(recall.matches[0].agent, "architect");

        let list = client1
            .list(None, 50)
            .await
            .expect("list deveria funcionar");
        assert_eq!(list.agents.len(), 1);
        assert_eq!(list.agents[0].agent, "architect");
        assert_eq!(list.agents[0].count, 2);

        let mut client2 = service.client().await.expect("segunda chamada reusa");
        assert_eq!(
            service.current_pid().await,
            Some(pid1),
            "PID deveria ser estável"
        );
        assert!(client2.health().await.unwrap().0);

        service
            .kill_current()
            .await
            .expect("kill deveria funcionar");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut client3 = service
            .client()
            .await
            .expect("terceira chamada sobe um processo novo");
        let pid2 = service.current_pid().await.expect("pid após restart");
        assert_ne!(pid1, pid2, "PID deveria mudar após o restart");
        assert!(client3.health().await.unwrap().0);
    }

    async fn spawn_test_core(prefix: &str) -> (tokio::task::JoinHandle<()>, PathBuf) {
        let core_sock =
            std::env::temp_dir().join(format!("forge-{prefix}-core-{}.sock", std::process::id()));
        let sock_for_task = core_sock.clone();
        let core_task = tokio::spawn(async move {
            let _ = crate::serve_core(NoopCore, sock_for_task).await;
        });
        for _ in 0..100 {
            if core_sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        (core_task, core_sock)
    }

    /// Fronteira da Onda 3 (Squad, capacidade 2): duas leases concorrentes
    /// sobem 2 processos distintos (2 PIDs diferentes), cada uma com seu
    /// próprio slot — nunca serializa squads concorrentes até o teto.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn squad_pool_capacidade_dois_usa_dois_processos_distintos_concorrentes() {
        let dir = python_workspace_dir();
        if uv_missing() || !dir.join("pyproject.toml").exists() {
            eprintln!("uv/workspace Python ausente — pulando teste de pool real");
            return;
        }
        let (core_task, core_sock) = spawn_test_core("pool2").await;
        let socket_dir = std::env::temp_dir().join(format!("forge-pool2-{}", std::process::id()));
        std::fs::create_dir_all(&socket_dir).unwrap();

        let pool = Arc::new(SquadPool::new(
            dir,
            socket_dir,
            core_sock,
            "claude-sonnet-5".into(),
            2,
            Duration::from_secs(30),
        ));
        assert_eq!(pool.capacity(), 2);

        let lease_a = pool.acquire().await.expect("slot A deveria subir");
        let lease_b = pool
            .acquire()
            .await
            .expect("slot B deveria subir (capacidade 2)");
        assert_ne!(lease_a.slot(), lease_b.slot());
        let pid_a = pool.pid_of(lease_a.slot()).await.expect("pid do slot A");
        let pid_b = pool.pid_of(lease_b.slot()).await.expect("pid do slot B");
        assert_ne!(
            pid_a, pid_b,
            "capacidade 2 deveria usar 2 processos distintos"
        );

        core_task.abort();
    }

    /// Fronteira da Onda 3 (Squad, capacidade 1 — isola o mecanismo de
    /// restart-on-crash de qualquer ambiguidade de qual slot é reusado):
    /// adquirir, devolver, matar o processo do slot e adquirir de novo sobe
    /// um processo novo (PID diferente) no MESMO (único) slot, sem que o
    /// pool em si precise ser recriado.
    #[tokio::test]
    async fn squad_pool_capacidade_um_detecta_queda_e_sobe_processo_novo() {
        let dir = python_workspace_dir();
        if uv_missing() || !dir.join("pyproject.toml").exists() {
            eprintln!("uv/workspace Python ausente — pulando teste de pool real");
            return;
        }
        let (core_task, core_sock) = spawn_test_core("pool1").await;
        let socket_dir = std::env::temp_dir().join(format!("forge-pool1-{}", std::process::id()));
        std::fs::create_dir_all(&socket_dir).unwrap();

        let pool = Arc::new(SquadPool::new(
            dir,
            socket_dir,
            core_sock,
            "claude-sonnet-5".into(),
            1,
            Duration::from_secs(30),
        ));

        let lease1 = pool
            .acquire()
            .await
            .expect("única leases deveria subir o slot 0");
        assert_eq!(lease1.slot(), 0);
        let pid1 = pool.pid_of(0).await.expect("pid do único slot");
        drop(lease1);

        pool.kill_slot(0)
            .await
            .expect("kill do slot deveria funcionar");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let lease2 = pool
            .acquire()
            .await
            .expect("reaquisição deveria subir processo novo");
        assert_eq!(lease2.slot(), 0);
        let pid2 = pool.pid_of(0).await.expect("pid depois do restart");
        assert_ne!(pid1, pid2, "PID deveria mudar após o restart");
        assert!(
            lease2.client().clone().health().await.unwrap().0,
            "o processo novo deveria responder ao health-check"
        );

        core_task.abort();
    }

    struct NoopCore;

    #[tonic::async_trait]
    impl crate::CoreBackend for NoopCore {
        async fn generate(
            &self,
            _req: &forge_proto::llm::LlmRequest,
        ) -> Result<(String, forge_proto::llm::Usage), String> {
            Ok((
                "{}".into(),
                forge_proto::llm::Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_hit: false,
                    provider: "noop".into(),
                },
            ))
        }
        async fn request_permission(&self, _req: &forge_proto::core::PermissionRequest) -> bool {
            true
        }
    }
}
