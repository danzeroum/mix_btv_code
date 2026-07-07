//! Fase 7 Onda 10 (A6): perfil do sandbox Docker + saúde do daemon. Mora aqui
//! (não em `forge-server`) porque precisa de `forge-tools::sandbox`.
//!
//! Tela **read-only**: o protótipo do handoff de design não tem nenhum
//! handler de instalar/vetar/habilitar/remover skill de terceiro — gestão de
//! ciclo de vida via web fica fora desta fase. A lista de skills de terceiro
//! em si já é real via `GET /api/skills` (Onda 3/`source` da Onda 10) — esta
//! rota não a duplica, só devolve o perfil de confinamento + o resultado real
//! de `Sandbox::ping()`.

use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use forge_tools::sandbox::Sandbox;
use serde::Serialize;
use std::path::PathBuf;

/// As constantes hardcoded de `Sandbox::run_with` (rootfs read-only,
/// cap-drop ALL, no-new-privileges) não são campos do struct `Sandbox` — são
/// literais dentro do `HostConfig` construído ali. Documentadas aqui como
/// constantes explícitas, não como se fossem configuráveis.
#[derive(Serialize)]
struct SandboxProfileView {
    image: String,
    network_disabled: bool,
    mem_limit_mb: u64,
    cpu_quota: f64,
    timeout_secs: u64,
    rootfs_readonly: bool,
    cap_drop_all: bool,
    no_new_privileges: bool,
}

#[derive(Serialize)]
struct SandboxView {
    profile: SandboxProfileView,
    /// Resultado real de `Sandbox::ping()` — `false` fail-closed sem daemon,
    /// nunca um "rodou" fabricado. Depende do ambiente onde o dashboard roda;
    /// zero probe adicional além deste ping (não sobe container nenhum).
    ping: bool,
}

/// `GET /api/sandbox` — perfil de confinamento (de `Sandbox::new` + as
/// constantes hardcoded de `run_with`) e o resultado real de `ping()`.
async fn get_sandbox() -> impl IntoResponse {
    let profile = Sandbox::new(PathBuf::new());
    let view = SandboxView {
        profile: SandboxProfileView {
            image: profile.image.clone(),
            network_disabled: profile.network_disabled,
            mem_limit_mb: profile.mem_limit_mb,
            cpu_quota: profile.cpu_quota,
            timeout_secs: profile.timeout.as_secs(),
            rootfs_readonly: true,
            cap_drop_all: true,
            no_new_privileges: true,
        },
        ping: Sandbox::ping().await,
    };
    Json(view)
}

/// Router aditivo do console de sandbox — `.merge()`ado ao router do agente
/// web, mesma composição de `mcp_console::router`/`memory_console::router`.
pub fn router() -> Router {
    Router::new().route("/api/sandbox", get(get_sandbox))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Fronteira estrutural: o perfil devolvido bate por igualdade com os
    /// defaults reais de `Sandbox::new` + as constantes hardcoded de
    /// `run_with`. **Não** afirma um valor fixo para `ping`: se há ou não um
    /// daemon Docker alcançável varia por ambiente (dev local vs. runner de
    /// CI) — a propriedade fail-closed determinística já está provada em
    /// `forge_tools::sandbox`'s `ping_com_daemon_inalcancavel_e_false`
    /// (que aponta pra um endpoint deliberadamente morto).
    #[tokio::test]
    async fn sandbox_devolve_o_perfil_real_e_um_ping_bem_formado() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/sandbox")
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

        let expected = Sandbox::new(PathBuf::new());
        let profile = &json["profile"];
        assert_eq!(profile["image"], expected.image);
        assert_eq!(profile["network_disabled"], expected.network_disabled);
        assert_eq!(profile["mem_limit_mb"], expected.mem_limit_mb);
        assert_eq!(profile["cpu_quota"], expected.cpu_quota);
        assert_eq!(profile["timeout_secs"], expected.timeout.as_secs());
        assert_eq!(profile["rootfs_readonly"], true);
        assert_eq!(profile["cap_drop_all"], true);
        assert_eq!(profile["no_new_privileges"], true);

        assert!(json["ping"].is_boolean(), "ping deve ser um bool real");
    }
}
