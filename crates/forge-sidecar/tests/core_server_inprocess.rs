//! Isola o `CoreService` server: cliente Rust ↔ servidor Rust sobre UDS,
//! sem Python. Se isto passar e o e2e com Python falhar, o problema é
//! interop; se falhar aqui, é a implementação do servidor.

use forge_proto::core::core_service_client::CoreServiceClient;
use forge_proto::core::PermissionRequest;
use forge_proto::llm::{llm_chunk, LlmRequest, Usage};
use forge_sidecar::{serve_core, CoreBackend};
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn core_service_responde_generate_em_processo() {
    let sock = std::env::temp_dir().join(format!("forge-core-inproc-{}.sock", std::process::id()));
    let core_task = tokio::spawn(serve_core(Backend, sock.clone()));
    for _ in 0..100 {
        if sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let sock_c = sock.clone();
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
    let mut client = CoreServiceClient::new(channel);

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
