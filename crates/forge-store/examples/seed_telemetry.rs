//! Semeia um evento real de telemetria num `.forge/telemetry.db`, usando o
//! mesmo `forge_store::Telemetry` que o restante do produto usa — não é um
//! hack de SQL cru. Existe para testes de integração cross-process (ex.:
//! o e2e de `web/` que sobe um `forge dashboard` real e confirma que a
//! tela de Telemetria reflete um evento gravado por fora).
//!
//! Uso: cargo run -p forge-store --example seed_telemetry -- <db_path> <nome> <session_id> [props_json] [ts]

use forge_store::Telemetry;

fn main() {
    let mut args = std::env::args().skip(1);
    let db_path = args
        .next()
        .expect("uso: seed_telemetry <db_path> <nome> <session_id> [props_json] [ts]");
    let name = args.next().expect("faltou <nome>");
    let session_id = args.next().expect("faltou <session_id>");
    let props_json = args.next().unwrap_or_else(|| "{}".to_string());
    let ts = args
        .next()
        .unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string());

    let props: serde_json::Value = serde_json::from_str(&props_json).expect("props_json inválido");

    let telemetry = Telemetry::open(&db_path).expect("falha ao abrir telemetry.db");
    telemetry.record(&name, &session_id, props, &ts);

    println!("evento '{name}' gravado em {db_path}");
}
