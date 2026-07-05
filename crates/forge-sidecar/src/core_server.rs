//! Servidor `CoreService` (`schemas/proto/core.proto`) — o lado Rust do
//! laço bidirecional (Onda 4d). O sidecar Python do squad chama de volta
//! `Generate` (LLM) e `RequestPermission` (HITL) enquanto executa.
//!
//! `Generate`/`RequestPermission` são atendidos por um [`CoreBackend`]
//! injetável: em produção, o `Gateway` real (forge-cli) + um resolver de
//! permissão; em teste, um backend roteirizado. Os demais RPCs do contrato
//! (`RunTool`/`AppendLedger`/`Recall`/`Remember`) devolvem `Unimplemented`
//! honestamente — o orquestrador atual não os chama (os agentes fazem uma
//! única chamada de LLM e a memória é local ao Python).

use forge_proto::core::core_service_server::{CoreService, CoreServiceServer};
use forge_proto::core::{
    permission_decision, LedgerAck, LedgerAppend, PermissionDecision, PermissionRequest,
    RecallRequest, RecallResponse, RememberAck, RememberRequest, ToolCall, ToolResult,
};
use forge_proto::llm::{llm_chunk, LlmChunk, LlmRequest, Usage};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
use tonic::{Request, Response, Status};

/// Backend que atende os RPCs realmente usados pelo squad. Recebe os tipos
/// **proto** (com `requester`, para telemetria/roteamento em teste).
#[tonic::async_trait]
pub trait CoreBackend: Send + Sync + 'static {
    /// Gera texto para um request; devolve `(texto_agregado, usage)` ou uma
    /// mensagem de erro (que vira um `LlmChunk::error` no stream).
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String>;
    /// Decide uma permissão (true = ALLOW). Fail-closed é responsabilidade
    /// de quem implementa.
    async fn request_permission(&self, req: &PermissionRequest) -> bool;
}

pub struct CoreServer<B: CoreBackend> {
    backend: Arc<B>,
}

impl<B: CoreBackend> CoreServer<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    pub fn into_service(self) -> CoreServiceServer<Self> {
        CoreServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl<B: CoreBackend> CoreService for CoreServer<B> {
    type GenerateStream = ReceiverStream<Result<LlmChunk, Status>>;

    async fn generate(
        &self,
        request: Request<LlmRequest>,
    ) -> Result<Response<Self::GenerateStream>, Status> {
        let req = request.into_inner();
        let backend = self.backend.clone();
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            // Item do stream é `Result<LlmChunk, Status>` (exigido pelo
            // tonic); construímos os `Ok(..)` inline em vez de num helper
            // para não disparar `result_large_err` sobre o `Status`.
            let send = |payload| LlmChunk {
                payload: Some(payload),
            };
            match backend.generate(&req).await {
                Ok((text, usage)) => {
                    let _ = tx.send(Ok(send(llm_chunk::Payload::TextDelta(text)))).await;
                    let _ = tx.send(Ok(send(llm_chunk::Payload::Usage(usage)))).await;
                }
                Err(e) => {
                    let _ = tx.send(Ok(send(llm_chunk::Payload::Error(e)))).await;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn request_permission(
        &self,
        request: Request<PermissionRequest>,
    ) -> Result<Response<PermissionDecision>, Status> {
        let approved = self.backend.request_permission(&request.into_inner()).await;
        let decision = if approved {
            permission_decision::Decision::Allow
        } else {
            permission_decision::Decision::Deny
        };
        Ok(Response::new(PermissionDecision {
            decision: decision as i32,
            operator_note: None,
        }))
    }

    async fn run_tool(&self, _: Request<ToolCall>) -> Result<Response<ToolResult>, Status> {
        Err(Status::unimplemented(
            "RunTool não usado pelo orquestrador atual (agentes fazem uma chamada de LLM)",
        ))
    }

    async fn append_ledger(&self, _: Request<LedgerAppend>) -> Result<Response<LedgerAck>, Status> {
        Err(Status::unimplemented(
            "AppendLedger não usado pelo orquestrador atual",
        ))
    }

    async fn recall(&self, _: Request<RecallRequest>) -> Result<Response<RecallResponse>, Status> {
        Err(Status::unimplemented(
            "Recall não usado — memória é local ao Python no orquestrador atual",
        ))
    }

    async fn remember(&self, _: Request<RememberRequest>) -> Result<Response<RememberAck>, Status> {
        Err(Status::unimplemented(
            "Remember não usado — memória é local ao Python no orquestrador atual",
        ))
    }
}

/// Sobe o `CoreService` num Unix Domain Socket. Bloqueia até o servidor
/// terminar — normalmente rodado numa task e abortado quando o squad
/// encerra.
pub async fn serve_core<B: CoreBackend>(
    backend: B,
    socket_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = std::fs::remove_file(&socket_path);
    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    let incoming = UnixListenerStream::new(listener);
    tonic::transport::Server::builder()
        .add_service(CoreServer::new(backend).into_service())
        .serve_with_incoming(incoming)
        .await?;
    Ok(())
}
