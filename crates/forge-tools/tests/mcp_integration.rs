//! Teste de integração cross-process do cliente MCP (Fase 6 Onda 4): sobe o
//! servidor fixture (`forge_mcp_fixture`, compilado pelo cargo) como **processo
//! separado**, lista suas tools no `ToolRegistry` (namespaced) e faz uma chamada
//! real ida-e-volta — a fronteira literal da onda. Auto-contido, roda em
//! qualquer lugar (não depende de node/npx nem de servidor externo).

use forge_tools::mcp::{register_mcp_server, McpServerConfig};
use forge_tools::ToolRegistry;

fn fixture_config() -> McpServerConfig {
    McpServerConfig {
        id: "fixture".to_string(),
        command: env!("CARGO_BIN_EXE_forge_mcp_fixture").to_string(),
        args: vec![],
    }
}

#[test]
fn mcp_server_fixture_lista_e_chama_via_registry() {
    let mut registry = ToolRegistry::default_set(std::path::Path::new("."));
    let n = register_mcp_server(&mut registry, &fixture_config()).expect("registra o fixture");
    assert!(n >= 1, "esperava >=1 tool do fixture, veio {n}");

    let tool = registry
        .get("mcp__fixture__echo")
        .expect("a tool echo do fixture deve estar registrada, namespaced");

    // A chamada real atravessa o processo MCP separado e volta.
    let out = tool
        .run(&serde_json::json!({"input": "mundo"}))
        .expect("a chamada MCP deve retornar");
    assert!(
        out.content.contains("ECHO:mundo"),
        "o echo do fixture deveria voltar; veio: {}",
        out.content
    );
}

#[test]
fn mcp_nomes_namespaced_nao_colidem() {
    let mut registry = ToolRegistry::default_set(std::path::Path::new("."));
    register_mcp_server(&mut registry, &fixture_config()).unwrap();
    // o nome namespaced (mcp__fixture__echo) não sombreia um built-in
    assert!(registry.get("bash").is_some());
    // registrar o mesmo servidor de novo: a colisão é pulada (não duplica)
    let n2 = register_mcp_server(&mut registry, &fixture_config()).unwrap();
    assert_eq!(n2, 0, "segundo registro do mesmo servidor não duplica");
}
