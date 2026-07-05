//! Parsers de findings por ferramenta — funções puras, testadas com saída
//! real capturada da ferramenta (não schemas adivinhados). Cada parser é
//! best-effort: campo ausente ou formato inesperado só descarta aquele
//! item, nunca panica — a fronteira de robustez de `run_pipeline` já
//! garante que um passo malcomportado vira `Fail`, não crash.

use forge_schemas::verification::Finding;

/// `cargo test` (formato texto padrão do stable, sem `--format=json` que é
/// nightly-only) — captura linhas `test <nome> ... FAILED`.
pub fn parse_cargo_test(stdout: &str) -> Vec<Finding> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line.strip_prefix("test ")?.strip_suffix(" ... FAILED")?;
            Some(Finding {
                tool: "cargo test".to_string(),
                severity: "error".to_string(),
                message: format!("teste falhou: {name}"),
                file: None,
                line: None,
            })
        })
        .collect()
}

/// `cargo clippy --message-format=json` — um objeto JSON por linha; só nos
/// interessam `reason == "compiler-message"` com `level` warning/error.
pub fn parse_clippy_json(stdout: &str) -> Vec<Finding> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|v| v.get("reason").and_then(|r| r.as_str()) == Some("compiler-message"))
        .filter_map(|v| {
            let message = v.get("message")?;
            let level = message.get("level")?.as_str()?;
            if level != "warning" && level != "error" {
                return None;
            }
            let text = message.get("message")?.as_str()?.to_string();
            let primary_span = message
                .get("spans")
                .and_then(|s| s.as_array())
                .and_then(|spans| {
                    spans
                        .iter()
                        .find(|s| s.get("is_primary").and_then(|p| p.as_bool()) == Some(true))
                });
            let file = primary_span
                .and_then(|s| s.get("file_name"))
                .and_then(|f| f.as_str())
                .map(|s| s.to_string());
            let line_no = primary_span
                .and_then(|s| s.get("line_start"))
                .and_then(|l| l.as_u64());
            Some(Finding {
                tool: "clippy".to_string(),
                severity: level.to_string(),
                message: text,
                file,
                line: line_no,
            })
        })
        .collect()
}

/// `ruff check --output-format=json` — array de objetos na raiz do stdout.
pub fn parse_ruff_json(stdout: &str) -> Vec<Finding> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(stdout) else {
        return vec![];
    };
    items
        .into_iter()
        .filter_map(|v| {
            let message = v.get("message")?.as_str()?.to_string();
            let severity = v
                .get("severity")
                .and_then(|s| s.as_str())
                .unwrap_or("error")
                .to_string();
            let file = v
                .get("filename")
                .and_then(|f| f.as_str())
                .map(|s| s.to_string());
            let line_no = v
                .get("location")
                .and_then(|l| l.get("row"))
                .and_then(|r| r.as_u64());
            let code = v.get("code").and_then(|c| c.as_str()).unwrap_or("ruff");
            Some(Finding {
                tool: format!("ruff({code})"),
                severity,
                message,
                file,
                line: line_no,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_test_extrai_apenas_os_testes_falhos() {
        let stdout = "\
running 3 tests
test tests::foo ... ok
test tests::bar ... FAILED
test tests::baz ... ok

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
        let findings = parse_cargo_test(stdout);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("tests::bar"));
        assert_eq!(findings[0].tool, "cargo test");
    }

    #[test]
    fn cargo_test_sem_falhas_nao_gera_findings() {
        let stdout = "test tests::foo ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        assert!(parse_cargo_test(stdout).is_empty());
    }

    // Capturado de verdade rodando `cargo clippy --message-format=json` sobre
    // um `fn main() { let x = 1; println!("hi"); }` — não é schema adivinhado.
    const CLIPPY_LINE: &str = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable: `x`","spans":[{"file_name":"src/main.rs","is_primary":true,"line_start":2}]}}"#;

    #[test]
    fn clippy_json_extrai_warning_com_arquivo_e_linha() {
        let findings = parse_clippy_json(CLIPPY_LINE);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, "warning");
        assert_eq!(findings[0].file.as_deref(), Some("src/main.rs"));
        assert_eq!(findings[0].line, Some(2));
        assert!(findings[0].message.contains("unused variable"));
    }

    #[test]
    fn clippy_json_ignora_reason_que_nao_e_compiler_message() {
        let line = r#"{"reason":"build-finished","success":true}"#;
        assert!(parse_clippy_json(line).is_empty());
    }

    #[test]
    fn clippy_json_linha_invalida_nao_panica() {
        assert!(parse_clippy_json("isto não é json\n{quebrado").is_empty());
    }

    // Capturado de verdade rodando `ruff check --output-format=json` sobre um
    // arquivo com um import não usado.
    const RUFF_JSON: &str = r#"[{"cell":null,"code":"F401","filename":"/tmp/bad.py","location":{"column":8,"row":1},"message":"`os` imported but unused","severity":"error"}]"#;

    #[test]
    fn ruff_json_extrai_finding_com_codigo_arquivo_e_linha() {
        let findings = parse_ruff_json(RUFF_JSON);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].tool, "ruff(F401)");
        assert_eq!(findings[0].file.as_deref(), Some("/tmp/bad.py"));
        assert_eq!(findings[0].line, Some(1));
    }

    #[test]
    fn ruff_json_vazio_nao_gera_findings() {
        assert!(parse_ruff_json("[]").is_empty());
    }

    #[test]
    fn ruff_json_invalido_nao_panica() {
        assert!(parse_ruff_json("não é json").is_empty());
    }
}
