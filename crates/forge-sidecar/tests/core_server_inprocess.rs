//! Isola o `CoreService` server: cliente Rust ↔ servidor Rust sobre UDS,
//! sem Python. Se isto passar e o e2e com Python falhar, o problema é
//! interop; se falhar aqui, é a implementação do servidor.

use forge_core::{Decision, PermissionEngine, Rule};
use forge_proto::core::core_service_client::CoreServiceClient;
use forge_proto::core::{PermissionRequest, ToolCall, ToolResult};
use forge_proto::llm::{llm_chunk, LlmRequest, Usage};
use forge_sidecar::{serve_core, CoreBackend};
use forge_tools::ToolRegistry;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::{Endpoint, Uri};
use tonic::Request;

struct Backend;

#[tonic::async_trait]
impl CoreBackend for Backend {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        Ok((
            format!("resposta para {}", req.requester),
            Usage {
                input_tokens: 3,
                output_tokens: 4,
                cache_hit: false,
                provider: "test".into(),
            },
        ))
    }
    async fn request_permission(&self, _req: &PermissionRequest) -> bool {
        true
    }
    async fn run_tool(&self, _call: &ToolCall) -> ToolResult {
        ToolResult {
            content: "Backend não executa ferramentas".into(),
            truncated: false,
            exit_code: 1,
        }
    }
}

/// Backend com `ToolRegistry`/`PermissionEngine` reais — prova `RunTool`
/// isoladamente do squad Python (cliente Rust ↔ servidor Rust sobre UDS).
struct BackendWithTools {
    tools: Arc<ToolRegistry>,
    permissions: PermissionEngine,
}

#[tonic::async_trait]
impl CoreBackend for BackendWithTools {
    async fn generate(&self, _req: &LlmRequest) -> Result<(String, Usage), String> {
        Err("BackendWithTools não gera texto".into())
    }
    async fn request_permission(&self, _req: &PermissionRequest) -> bool {
        true
    }
    async fn run_tool(&self, call: &ToolCall) -> ToolResult {
        // Mesma lógica de `forge-cli::squad::core_run_tool`, sem depender de
        // `forge-cli` (evitaria um ciclo de dependência de teste) — isolado
        // o bastante para provar o contrato do lado do servidor `CoreService`.
        let args: serde_json::Value = match serde_json::from_str(&call.args_json) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult {
                    content: format!("args_json inválido: {e}"),
                    truncated: false,
                    exit_code: 1,
                }
            }
        };
        let Some(tool) = self.tools.get(&call.tool) else {
            return ToolResult {
                content: format!("ferramenta desconhecida: {}", call.tool),
                truncated: false,
                exit_code: 1,
            };
        };
        let scope = tool.scope(&args);
        let allowed = match self.permissions.evaluate(&call.tool, &scope) {
            Decision::Allow => true,
            Decision::Deny => false,
            Decision::Ask => {
                self.request_permission(&PermissionRequest {
                    tool: call.tool.clone(),
                    scope: scope.clone(),
                    reason: String::new(),
                    confidence: 0.0,
                })
                .await
            }
        };
        if !allowed {
            return ToolResult {
                content: format!("permissão negada para {} em {scope:?}", call.tool),
                truncated: false,
                exit_code: -1,
            };
        }
        match tool.run(&args) {
            Ok(out) => ToolResult {
                content: out.content,
                truncated: out.truncated,
                exit_code: 0,
            },
            Err(e) => ToolResult {
                content: e.to_string(),
                truncated: false,
                exit_code: 1,
            },
        }
    }
}

async fn connect_client(sock: &std::path::Path) -> CoreServiceClient<tonic::transport::Channel> {
    let sock_c = sock.to_path_buf();
    let channel = Endpoint::try_from("http://core.invalid")
        .unwrap()
        .connect_with_connector(tower::service_fn(move |_: Uri| {
            let p = sock_c.clone();
            async move {
                let s = tokio::net::UnixStream::connect(p).await?;
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(s))
            }
        }))
        .await
        .expect("conectar ao core");
    CoreServiceClient::new(channel)
}

async fn wait_for_socket(sock: &std::path::Path) {
    for _ in 0..100 {
        if sock.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn core_service_responde_generate_em_processo() {
    let sock = std::env::temp_dir().join(format!("forge-core-inproc-{}.sock", std::process::id()));
    let core_task = tokio::spawn(serve_core(Backend, sock.clone()));
    wait_for_socket(&sock).await;
    let mut client = connect_client(&sock).await;

    let mut stream = client
        .generate(Request::new(LlmRequest {
            model: "m".into(),
            messages_json: "[]".into(),
            temperature: None,
            max_tokens: None,
            requester: "architect".into(),
        }))
        .await
        .expect("Generate deveria abrir o stream")
        .into_inner();

    let mut text = String::new();
    let mut usage_seen = false;
    while let Some(chunk) = stream.message().await.expect("stream de LlmChunk") {
        match chunk.payload {
            Some(llm_chunk::Payload::TextDelta(t)) => text.push_str(&t),
            Some(llm_chunk::Payload::Usage(u)) => {
                usage_seen = true;
                assert_eq!(u.provider, "test");
            }
            Some(llm_chunk::Payload::Error(e)) => panic!("erro inesperado: {e}"),
            None => {}
        }
    }
    core_task.abort();

    assert_eq!(text, "resposta para architect");
    assert!(usage_seen, "deveria ter recebido um chunk de usage");
}

/// Onda 1 — boundary test do executor: um `ToolCall` de verdade sobre UDS
/// puro (sem Python nenhum) produz um arquivo real no disco. Prova
/// `RunTool` isolado da implementação do servidor, não do squad inteiro.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_tool_executa_de_verdade_e_arquivo_aparece_no_disco() {
    let dir = tempfile::tempdir().unwrap();
    let backend = BackendWithTools {
        tools: Arc::new(ToolRegistry::default_set(dir.path())),
        permissions: PermissionEngine::default(),
    };
    let sock =
        std::env::temp_dir().join(format!("forge-core-runtool-ok-{}.sock", std::process::id()));
    let core_task = tokio::spawn(serve_core(backend, sock.clone()));
    wait_for_socket(&sock).await;
    let mut client = connect_client(&sock).await;

    let result = client
        .run_tool(Request::new(ToolCall {
            tool: "bash".into(),
            args_json: serde_json::json!({"command": "echo conteudo > out.txt"}).to_string(),
            scope: String::new(),
        }))
        .await
        .expect("RunTool deveria responder")
        .into_inner();
    core_task.abort();

    assert_eq!(result.exit_code, 0, "conteúdo: {}", result.content);
    let written = std::fs::read_to_string(dir.path().join("out.txt"))
        .expect("out.txt deveria existir no workspace");
    assert_eq!(written.trim(), "conteudo");
}

/// Onda 1 — negação do motor de permissões não executa nada: nem o
/// `exit_code` de sucesso, nem o arquivo aparecem.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_tool_negado_pela_permissao_nao_executa() {
    let dir = tempfile::tempdir().unwrap();
    let backend = BackendWithTools {
        tools: Arc::new(ToolRegistry::default_set(dir.path())),
        permissions: PermissionEngine {
            rules: vec![Rule {
                tool: "bash".into(),
                scope_prefix: None,
                decision: Decision::Deny,
            }],
        },
    };
    let sock = std::env::temp_dir().join(format!(
        "forge-core-runtool-denied-{}.sock",
        std::process::id()
    ));
    let core_task = tokio::spawn(serve_core(backend, sock.clone()));
    wait_for_socket(&sock).await;
    let mut client = connect_client(&sock).await;

    let result = client
        .run_tool(Request::new(ToolCall {
            tool: "bash".into(),
            args_json: serde_json::json!({"command": "echo conteudo > negado.txt"}).to_string(),
            scope: String::new(),
        }))
        .await
        .expect("RunTool deveria responder mesmo negando")
        .into_inner();
    core_task.abort();

    assert_eq!(result.exit_code, -1);
    assert!(
        !dir.path().join("negado.txt").exists(),
        "negado não deveria ter criado o arquivo"
    );
}
