//! Fase 7 Onda 7 (A1): console MCP — enumera `.forge/mcp.toml`, sonda cada
//! servidor (`forge_tools::mcp::list_tools_blocking`, com timeout curto) e
//! calcula o preview de política real via `PermissionEngine::evaluate`
//! combinado com o MESMO store de `Rule` persistida da Onda 2
//! (`web_agent::load_rule_overrides`) — os perfis const (`BUILD`/`PLAN`) não
//! têm regra nenhuma para `mcp__*`, então sem consultar o override o preview
//! seria sempre "ask", nunca refletindo uma decisão real do usuário.
//!
//! Mora aqui (não em `forge-server`) porque precisa de `forge-tools`+
//! `forge-core` — regra de posicionamento de rota da fase.

use axum::extract::State;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use forge_core::{Decision, BUILD, PLAN};
use forge_tools::mcp::list_tools_blocking;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

/// Prazo para um servidor MCP responder ao probe do console — curto de
/// propósito: isto é uma tela de status, não uma chamada de ferramenta real;
/// um servidor lento/travado não deve travar o dashboard junto.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct McpConsoleState {
    root: PathBuf,
}

#[derive(Serialize)]
struct ToolPolicyPreview {
    build: Decision,
    plan: Decision,
}

#[derive(Serialize)]
struct McpToolView {
    /// Namespaced (`mcp__<server>__<tool>`) — o mesmo nome que o
    /// `ToolRegistry` real usaria se o servidor fosse carregado por
    /// `skills::load_mcp_servers`.
    name: String,
    description: String,
    policy: ToolPolicyPreview,
}

#[derive(Serialize)]
struct McpServerView {
    id: String,
    command: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    tools: Vec<McpToolView>,
}

/// `GET /api/mcp` — status real de cada servidor declarado + preview de
/// política por tool. Nunca registra nada no `ToolRegistry` (isso só
/// acontece quando uma sessão real carrega — `skills::build_registry`); isto
/// é somente leitura, para exibição.
async fn list_mcp(State(state): State<McpConsoleState>) -> Response {
    let configs = crate::skills::read_mcp_server_configs(&state.root);
    let build_engine =
        (BUILD.permissions)().overlay(&crate::web_agent::load_rule_overrides(&state.root, "build"));
    let plan_engine =
        (PLAN.permissions)().overlay(&crate::web_agent::load_rule_overrides(&state.root, "plan"));

    let mut servers = Vec::with_capacity(configs.len());
    for config in configs {
        let probe_config = config.clone();
        let probe = tokio::task::spawn_blocking(move || list_tools_blocking(&probe_config));
        let (status, error, tools) = match tokio::time::timeout(PROBE_TIMEOUT, probe).await {
            Ok(Ok(Ok(metas))) => {
                let tools = metas
                    .into_iter()
                    .map(|m| {
                        let full_name = format!("mcp__{}__{}", config.id, m.name);
                        let scope = format!("mcp:{}/{}", config.id, m.name);
                        McpToolView {
                            policy: ToolPolicyPreview {
                                build: build_engine.evaluate(&full_name, &scope),
                                plan: plan_engine.evaluate(&full_name, &scope),
                            },
                            description: m.description,
                            name: full_name,
                        }
                    })
                    .collect();
                ("online", None, tools)
            }
            Ok(Ok(Err(e))) => ("offline", Some(e), Vec::new()),
            Ok(Err(_join_error)) => (
                "offline",
                Some("thread do probe MCP entrou em pânico".to_string()),
                Vec::new(),
            ),
            Err(_elapsed) => (
                "offline",
                Some(format!("sem resposta em {}s", PROBE_TIMEOUT.as_secs())),
                Vec::new(),
            ),
        };
        servers.push(McpServerView {
            id: config.id,
            command: config.command,
            status,
            error,
            tools,
        });
    }
    Json(servers).into_response()
}

/// Router aditivo do console MCP — `.merge()`ado ao router do agente web,
/// mesma composição de `squad_agent::router`/`prompt_render::router`.
pub fn router(root: PathBuf) -> Router {
    Router::new()
        .route("/api/mcp", get(list_mcp))
        .with_state(McpConsoleState { root })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// `forge_mcp_fixture` (Fase 6 Onda 4) é um bin de `forge-tools`, não
    /// deste crate — `CARGO_BIN_EXE_*` só é exposto pelo cargo dentro do
    /// PRÓPRIO pacote que declara o bin. Buildamos explicitamente (barato:
    /// incremental, já compilado pelos testes de `forge-tools`) e apontamos
    /// para o binário no `target/` compartilhado do workspace.
    fn mcp_fixture_bin() -> std::path::PathBuf {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let status = std::process::Command::new("cargo")
            .args(["build", "-p", "forge-tools", "--bin", "forge_mcp_fixture"])
            .current_dir(&repo_root)
            .status()
            .expect("cargo build do forge_mcp_fixture");
        assert!(status.success(), "build do forge_mcp_fixture falhou");
        repo_root
            .join("target")
            .join("debug")
            .join("forge_mcp_fixture")
    }

    fn write_mcp_toml(root: &std::path::Path, fixture_bin: &std::path::Path) {
        std::fs::create_dir_all(root.join(".forge")).unwrap();
        std::fs::write(
            root.join(".forge").join("mcp.toml"),
            format!(
                "[[server]]\nid = \"vivo\"\ncommand = \"{}\"\nargs = []\n\n\
                 [[server]]\nid = \"morto\"\ncommand = \"/caminho/que/nao/existe/forge-mcp-x\"\nargs = []\n",
                fixture_bin.display()
            ),
        )
        .unwrap();
    }

    /// Fronteira da Onda 7 (A1): 2 servidores declarados (1 respondendo de
    /// verdade via subprocess MCP real, 1 apontando pra um comando
    /// inexistente) + 1 override persistido (`Rule` da Onda 2) para uma tool
    /// específica — a tela mostra status real por servidor e a política do
    /// tool com override como `allow` (não "ask" constante), enquanto o
    /// resto permanece "ask" (nenhuma regra, perfis const não cobrem
    /// `mcp__*`).
    #[tokio::test]
    async fn console_mcp_mostra_status_real_e_preview_de_politica_com_override() {
        let fixture_bin = mcp_fixture_bin();
        let dir = tempfile::tempdir().unwrap();
        write_mcp_toml(dir.path(), &fixture_bin);

        // Override real: `mcp__vivo__echo` sempre "allow" para o perfil build.
        let mut rule_store = forge_store::RuleStore::open(
            dir.path().join(".forge").join("rules.db").to_str().unwrap(),
        )
        .unwrap();
        rule_store
            .set(
                "build",
                "mcp__vivo__echo",
                None,
                forge_store::RuleDecision::Allow,
                "2026-01-01T00:00:00Z",
            )
            .unwrap();

        let app = router(dir.path().to_path_buf());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let servers = json.as_array().unwrap();
        assert_eq!(servers.len(), 2);

        let vivo = servers.iter().find(|s| s["id"] == "vivo").unwrap();
        assert_eq!(vivo["status"], "online");
        assert!(vivo["error"].is_null());
        let tools = vivo["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1, "o fixture só anuncia a tool echo");
        let echo = &tools[0];
        assert_eq!(echo["name"], "mcp__vivo__echo");
        // Override real vence: build vira allow, não o "ask" default do perfil.
        assert_eq!(echo["policy"]["build"], "allow");
        // Sem override para o perfil plan: cai no default real do PermissionEngine (ask).
        assert_eq!(echo["policy"]["plan"], "ask");

        let morto = servers.iter().find(|s| s["id"] == "morto").unwrap();
        assert_eq!(morto["status"], "offline");
        assert!(morto["error"].as_str().is_some());
        assert_eq!(morto["tools"].as_array().unwrap().len(), 0);
    }
}
