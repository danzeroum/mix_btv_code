//! Servidor MCP **fixture** (Fase 6 Onda 4): expõe uma tool `echo` via stdio,
//! para o teste de integração cross-process do cliente MCP (`forge-tools::mcp`).
//! Não é produto — é o "servidor de terceiro" que o teste sobe como processo
//! separado. Cargo o compila junto dos testes de `forge-tools`.

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServiceExt};
use std::sync::Arc;

#[derive(Clone)]
struct Fixture;

impl ServerHandler for Fixture {
    #[allow(clippy::field_reassign_with_default)]
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

    async fn list_tools(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "input": { "type": "string" } }
        });
        let schema_obj = schema.as_object().cloned().unwrap_or_default();
        let tool = Tool::new(
            "echo",
            "Ecoa o input com prefixo ECHO:",
            Arc::new(schema_obj),
        );
        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let input = req
            .arguments
            .as_ref()
            .and_then(|a| a.get("input"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "ECHO:{input}"
        ))]))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = Fixture
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}
