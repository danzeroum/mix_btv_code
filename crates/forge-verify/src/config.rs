//! Configuração de passos do `/verify` por projeto (`forge.toml` na raiz).
//! Ausência do arquivo cai no [`default_steps`] sensato (typecheck+test+lint
//! do próprio workspace Rust — o que o self-hosting da Fase 5 precisa).

use crate::{Parser, StepSpec};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct VerifyConfig {
    #[serde(rename = "step", default)]
    pub steps: Vec<StepConfig>,
}

#[derive(Debug, Deserialize)]
pub struct StepConfig {
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout_ms: Option<u64>,
    /// Um de: "cargo_test" | "clippy_json" | "ruff_json"; qualquer outro
    /// valor (incluindo ausente) roda sem parser — o passo ainda conta pro
    /// veredito via exit_code, só não produz findings estruturados.
    pub parser: Option<String>,
}

impl StepConfig {
    fn parser(&self) -> Option<Parser> {
        match self.parser.as_deref() {
            Some("cargo_test") => Some(Parser::CargoTest),
            Some("clippy_json") => Some(Parser::ClippyJson),
            Some("ruff_json") => Some(Parser::RuffJson),
            _ => None,
        }
    }
}

impl From<&StepConfig> for StepSpec {
    fn from(c: &StepConfig) -> Self {
        let mut spec = StepSpec::new(c.name.clone(), c.program.clone(), c.args.clone());
        if let Some(ms) = c.timeout_ms {
            spec = spec.with_timeout(Duration::from_millis(ms));
        }
        if let Some(p) = c.parser() {
            spec = spec.with_parser(p);
        }
        spec
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("falha ao ler o arquivo de config: {0}")]
    Io(std::io::Error),
    #[error("TOML inválido: {0}")]
    Parse(#[from] toml::de::Error),
}

/// `Ok(None)` se o arquivo não existir (o chamador decide o default).
/// `Err` se existir mas for TOML inválido — falhar alto é melhor do que
/// rodar silenciosamente um pipeline errado.
pub fn load_config(path: &Path) -> Result<Option<VerifyConfig>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
    let config: VerifyConfig = toml::from_str(&raw)?;
    Ok(Some(config))
}

/// Passos default quando não há `forge.toml`: espelha o job `rust` do CI
/// (`.github/workflows/ci.yml`) — mesmos comandos, incluindo `-D warnings`.
pub fn default_steps() -> Vec<StepSpec> {
    vec![
        StepSpec::new("test", "cargo", vec!["test".into(), "--workspace".into()])
            .with_timeout(Duration::from_secs(300)),
        StepSpec::new(
            "lint",
            "cargo",
            vec![
                "clippy".into(),
                "--workspace".into(),
                "--message-format=json".into(),
                "--".into(),
                "-D".into(),
                "warnings".into(),
            ],
        )
        .with_timeout(Duration::from_secs(180))
        .with_parser(Parser::ClippyJson),
        StepSpec::new(
            "fmt",
            "cargo",
            vec!["fmt".into(), "--all".into(), "--check".into()],
        )
        .with_timeout(Duration::from_secs(30)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arquivo_ausente_retorna_none() {
        let result = load_config(Path::new("/tmp/este-arquivo-forge-toml-nao-existe.toml"));
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn toml_invalido_retorna_erro() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("forge.toml");
        std::fs::write(&path, "isto não é toml válido [[[").unwrap();
        assert!(load_config(&path).is_err());
    }

    #[test]
    fn parseia_passos_e_converte_para_stepspec() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("forge.toml");
        std::fs::write(
            &path,
            r#"
[[step]]
name = "lint"
program = "cargo"
args = ["clippy", "--message-format=json"]
timeout_ms = 60000
parser = "clippy_json"

[[step]]
name = "test"
program = "cargo"
args = ["test"]
"#,
        )
        .unwrap();

        let config = load_config(&path).unwrap().expect("config presente");
        assert_eq!(config.steps.len(), 2);

        let specs: Vec<StepSpec> = config.steps.iter().map(StepSpec::from).collect();
        assert_eq!(specs[0].name, "lint");
        assert_eq!(specs[0].timeout, Some(Duration::from_millis(60000)));
        assert_eq!(specs[0].parser, Some(Parser::ClippyJson));
        assert_eq!(specs[1].name, "test");
        assert_eq!(specs[1].timeout, None);
        assert_eq!(specs[1].parser, None);
    }

    #[test]
    fn default_steps_espelha_o_ci_com_deny_warnings() {
        let steps = default_steps();
        let lint = steps
            .iter()
            .find(|s| s.name == "lint")
            .expect("passo lint existe");
        assert!(lint.args.contains(&"-D".to_string()));
        assert!(lint.args.contains(&"warnings".to_string()));
        assert_eq!(lint.parser, Some(Parser::ClippyJson));
    }
}
