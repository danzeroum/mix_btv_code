//! Pipeline de verificação determinística (`/verify`).
//!
//! Porta do `script/verify.ts` do fork do opencode: roda passos
//! configuráveis (typecheck → test → lint → SAST) como subprocessos com
//! timeout e produz `verification-evidence.v1` — evidência estruturada que
//! o Auditor do squad consome no lugar de opinião de LLM.
//!
//! Fase 5: timeouts com kill de grupo de processos (ver `exec`), findings
//! estruturados por ferramenta (ver `parsers`), o comando `forge verify`
//! (crates/forge-cli) escrevendo a evidência em disco, e o skill-vetter
//! (ver `vetter`) que aponta esta mesma máquina para o diretório de uma
//! skill e decide vet/block.

pub mod config;
pub mod exec;
pub mod parsers;
pub mod prompt_integrity;
pub mod vetter;

use forge_schemas::verification::{Finding, VerificationEvidence, VerificationStep};
use std::time::Duration;

/// Qual parser aplicar à stdout do passo para extrair findings estruturados.
/// `None` = sem parser — o passo ainda conta para o veredito via exit_code,
/// só não produz findings individuais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parser {
    CargoTest,
    ClippyJson,
    RuffJson,
}

impl Parser {
    fn apply(self, stdout: &str) -> Vec<Finding> {
        match self {
            Parser::CargoTest => parsers::parse_cargo_test(stdout),
            Parser::ClippyJson => parsers::parse_clippy_json(stdout),
            Parser::RuffJson => parsers::parse_ruff_json(stdout),
        }
    }
}

/// Um passo declarado do pipeline (comando + args + timeout + parser opcional).
#[derive(Debug, Clone)]
pub struct StepSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    /// `None` roda sem limite — aceitável em uso local, desaconselhado em CI.
    pub timeout: Option<Duration>,
    pub parser: Option<Parser>,
}

impl StepSpec {
    pub fn new(name: impl Into<String>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            program: program.into(),
            args,
            timeout: None,
            parser: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_parser(mut self, parser: Parser) -> Self {
        self.parser = Some(parser);
        self
    }
}

/// Executa os passos em ordem e monta a evidência. Passos após uma falha
/// ainda rodam — a evidência registra todos os resultados.
pub fn run_pipeline(
    run_id: &str,
    git_sha: &str,
    produced_at: &str,
    steps: &[StepSpec],
) -> VerificationEvidence {
    run_pipeline_with_progress(run_id, git_sha, produced_at, steps, |_, _, _| {})
}

/// Como `run_pipeline`, mas invoca `on_step(passo_concluido, total, &step)`
/// depois de cada passo — usado pelo job em background do dashboard (Fase 7
/// Onda 11) para reportar progresso via polling sem esperar o pipeline
/// inteiro terminar. `run_pipeline` é este mesmo laço com um callback vazio.
pub fn run_pipeline_with_progress(
    run_id: &str,
    git_sha: &str,
    produced_at: &str,
    steps: &[StepSpec],
    mut on_step: impl FnMut(usize, usize, &VerificationStep),
) -> VerificationEvidence {
    let total = steps.len();
    let mut executed = Vec::with_capacity(total);
    for (i, spec) in steps.iter().enumerate() {
        let step = run_step(spec);
        on_step(i + 1, total, &step);
        executed.push(step);
    }
    let verdict = VerificationEvidence::derive_verdict(&executed);
    VerificationEvidence {
        run_id: run_id.to_string(),
        git_sha: git_sha.to_string(),
        steps: executed,
        verdict,
        produced_at: produced_at.to_string(),
    }
}

/// Sentinela de exit code para passo que estourou o timeout — convenção do
/// utilitário `timeout` do coreutils, distinta de qualquer exit code real.
const TIMEOUT_EXIT_CODE: i32 = 124;

fn run_step(spec: &StepSpec) -> VerificationStep {
    let result = exec::run_with_timeout(&spec.program, &spec.args, spec.timeout);
    let tool = format!("{} {}", spec.program, spec.args.join(" "))
        .trim()
        .to_string();

    match result.output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut findings = spec.parser.map(|p| p.apply(&stdout)).unwrap_or_default();
            let exit_code = if result.timed_out {
                TIMEOUT_EXIT_CODE
            } else {
                output.status.code().unwrap_or(-1)
            };
            if result.timed_out {
                findings.push(Finding {
                    tool: spec.program.clone(),
                    severity: "error".to_string(),
                    message: format!(
                        "passo '{}' excedeu o timeout e foi encerrado (grupo de processos morto)",
                        spec.name
                    ),
                    file: None,
                    line: None,
                });
            }
            VerificationStep {
                name: spec.name.clone(),
                tool,
                exit_code,
                duration_ms: result.duration_ms,
                findings,
            }
        }
        Err(e) => VerificationStep {
            name: spec.name.clone(),
            tool,
            exit_code: -1,
            duration_ms: result.duration_ms,
            findings: vec![Finding {
                tool: spec.program.clone(),
                severity: "error".to_string(),
                message: format!("falha ao executar: {e}"),
                file: None,
                line: None,
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_schemas::verification::Verdict;

    #[test]
    fn passo_verdadeiro_passa_e_falso_falha() {
        let evidence = run_pipeline(
            "run-1",
            "deadbeef",
            "2026-07-05T00:00:00Z",
            &[
                StepSpec::new("ok", "true", vec![]),
                StepSpec::new("falha", "false", vec![]),
            ],
        );
        assert!(matches!(evidence.verdict, Verdict::Fail));
        assert_eq!(evidence.steps.len(), 2);
        assert_eq!(evidence.steps[0].exit_code, 0);
        assert_ne!(evidence.steps[1].exit_code, 0);
    }

    #[test]
    fn evidencia_serializa_para_json() {
        let evidence = run_pipeline("run-2", "cafebabe", "2026-07-05T00:00:00Z", &[]);
        let json = serde_json::to_value(&evidence).unwrap();
        assert_eq!(json["run_id"], "run-2");
        assert_eq!(json["verdict"], "pass");
    }

    #[test]
    fn passo_com_timeout_estourado_falha_com_finding_e_exit_code_sentinela() {
        let evidence = run_pipeline(
            "run-3",
            "sha",
            "ts",
            &[StepSpec::new("dorme", "sleep", vec!["5".to_string()])
                .with_timeout(Duration::from_millis(150))],
        );
        assert!(matches!(evidence.verdict, Verdict::Fail));
        assert_eq!(evidence.steps[0].exit_code, TIMEOUT_EXIT_CODE);
        assert_eq!(evidence.steps[0].findings.len(), 1);
        assert!(evidence.steps[0].findings[0].message.contains("timeout"));
    }

    #[test]
    fn passo_com_parser_preenche_findings_reais() {
        let evidence = run_pipeline(
            "run-4",
            "sha",
            "ts",
            &[StepSpec::new("echo-clippy", "printf", vec![
                "%s\\n".to_string(),
                r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable: `x`","spans":[{"file_name":"src/main.rs","is_primary":true,"line_start":2}]}}"#.to_string(),
            ])
            .with_parser(Parser::ClippyJson)],
        );
        assert_eq!(evidence.steps[0].findings.len(), 1);
        assert_eq!(
            evidence.steps[0].findings[0].file.as_deref(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn run_pipeline_with_progress_reporta_cada_passo_em_ordem() {
        let mut seen = Vec::new();
        let evidence = run_pipeline_with_progress(
            "run-progress",
            "sha",
            "ts",
            &[
                StepSpec::new("um", "true", vec![]),
                StepSpec::new("dois", "true", vec![]),
                StepSpec::new("tres", "false", vec![]),
            ],
            |step, total, s| seen.push((step, total, s.name.clone())),
        );
        assert_eq!(
            seen,
            vec![
                (1, 3, "um".to_string()),
                (2, 3, "dois".to_string()),
                (3, 3, "tres".to_string()),
            ]
        );
        assert_eq!(evidence.steps.len(), 3);
        assert!(matches!(evidence.verdict, Verdict::Fail));
    }

    #[test]
    fn programa_inexistente_falha_sem_panicar() {
        let evidence = run_pipeline(
            "run-5",
            "sha",
            "ts",
            &[StepSpec::new(
                "inexistente",
                "este-programa-nao-existe-xyz",
                vec![],
            )],
        );
        assert!(matches!(evidence.verdict, Verdict::Fail));
        assert_eq!(evidence.steps[0].exit_code, -1);
        assert_eq!(evidence.steps[0].findings.len(), 1);
    }
}
