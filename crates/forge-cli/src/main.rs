//! `forge` — CLI da plataforma unificada (BuildToValue + opencode + prompte).
//!
//! Fase 1: `run` executa o loop de agente real e `chat` abre o REPL
//! multi-turno — gateway LLM com streaming e cache por hash, ferramentas
//! sob permissão interativa e ledger em `.forge/forge.db`.
//! `squad` ativa o sidecar Python na Fase 4; `verify` completa na Fase 5.

mod cache;
mod mcp_console;
mod prompt_render;
mod rate_limit_gen;
mod session;
mod sidecar;
mod skills;
mod squad;
mod squad_agent;
#[cfg(test)]
mod test_support;
mod tui_app;
mod web_agent;

use anyhow::{bail, Context, Result};
use cache::CachedGenerator;
use clap::{Parser, Subcommand};
use forge_core::{
    AgentLoop, CompactionPolicy, DurableSession, LoopEvent, PermissionResolver, BUILD, PLAN,
};
use forge_llm::chat::ChatMessage;
use forge_llm::{tier_from_id, Gateway, Generator, ModelTier, RateLimiter};
use forge_schemas::experiment::{ExperimentReport, VariantStats};
use forge_store::{EventStore, PromptCache, Telemetry};
use forge_tools::ToolRegistry;
use rate_limit_gen::RateLimitedGenerator;
use serde_json::Value;
use session::now_rfc3339;
use std::io::{BufRead, Write};
use std::path::PathBuf;

type CliGenerator = CachedGenerator<RateLimitedGenerator<Gateway>>;

#[derive(Parser)]
#[command(
    name = "forge",
    version,
    about = "Coding agent unificado (Rust + Python)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Args, Clone)]
struct RunOpts {
    /// Modelo a usar (define o ModelTier).
    #[arg(long, default_value = "claude-sonnet-5")]
    model: String,
    /// Perfil de agente: build (edita) ou plan (somente leitura).
    #[arg(long, default_value = "build")]
    agent: String,
    /// Aprova automaticamente pedidos de permissão (use com cautela).
    #[arg(long)]
    yes: bool,
    /// Desliga o cache de prompts por hash.
    #[arg(long)]
    no_cache: bool,
    /// Retoma (ou nomeia) uma sessão durável; sem valor, cria uma nova.
    #[arg(long)]
    session: Option<String>,
    /// Janela de contexto do modelo, em tokens (para a compaction).
    #[arg(long, default_value_t = 200_000)]
    context_window: usize,
}

#[derive(Subcommand)]
enum Commands {
    /// Executa uma tarefa única com o agente ativo.
    Run {
        /// Descrição da tarefa.
        task: String,
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Abre o REPL de conversa multi-turno.
    Chat {
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Abre a interface de terminal (ratatui).
    Tui {
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Roda o pipeline de verificação determinística (typecheck/test/lint/
    /// SAST) e grava a evidência `verification-evidence.v1`. Não confundir
    /// com a integridade do ledger — aquilo é `session.verify()` (a cadeia
    /// de hash, checada ao fim de `run`/`chat`); isto verifica código.
    Verify {
        /// Caminho do forge.toml (default: ./forge.toml; se ausente, roda o
        /// pipeline default espelhando o job `rust` do CI).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Onde gravar a evidência (default: .forge/evidence/<run_id>.json).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Formato do resumo impresso no stdout.
        #[arg(long, value_enum, default_value = "human")]
        format: VerifyFormat,
    },
    /// Delega a tarefa ao squad multi-agente (sidecar Python + gateway
    /// Rust). Degrada para agente-único → safe-mode se o squad falhar.
    Squad {
        /// Descrição da tarefa.
        task: String,
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Sobe o dashboard de telemetria (`.forge/telemetry.db`) em localhost.
    Dashboard {
        /// Porta local do dashboard.
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Fase 7 Onda 1 (opt-in até o fecho da fase): habilita as rotas do
        /// agente web (sessão/permissão via SSE) por trás da guarda de
        /// Origin/Host — sem isso, o dashboard segue só leitura como hoje.
        #[arg(long)]
        web_agent: bool,
    },
    /// Gera o relatório de A/B testing de um experimento a partir da telemetria
    /// local: compara a taxa de sucesso das duas variantes com teste de
    /// significância. Sem diferença real → "sem significância", nunca um
    /// vencedor fabricado (a régua Nada Fake aplicada a estatística).
    Experiment {
        /// Nome do experimento (`props.experiment` na telemetria).
        experiment: String,
        /// Banco de telemetria (default: `.forge/telemetry.db`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Formato da saída.
        #[arg(long, value_enum, default_value = "human")]
        format: VerifyFormat,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum VerifyFormat {
    /// Resumo legível por passo + veredito + caminho do artefato.
    Human,
    /// A própria evidência JSON no stdout (além do arquivo gravado).
    Json,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { task, opts } => {
            let (generator, root) = prepare(&opts)?;
            run_once(&generator, &opts, &root, task).await
        }
        Commands::Chat { opts } => {
            let (generator, root) = prepare(&opts)?;
            chat_repl(&generator, &opts, &root).await
        }
        Commands::Tui { opts } => {
            let (generator, root) = prepare(&opts)?;
            tui_app::run_tui(std::sync::Arc::new(generator), opts, root).await
        }
        Commands::Verify {
            config,
            out,
            format,
        } => run_verify(config, out, format),
        Commands::Squad { task, opts } => {
            let (generator, root) = prepare(&opts)?;
            squad::run_squad(generator, &opts, &root, task).await
        }
        Commands::Dashboard { port, web_agent } => run_dashboard(port, web_agent).await,
        Commands::Experiment {
            experiment,
            db,
            format,
        } => run_experiment(experiment, db, format),
    }
}

/// Gera o relatório de A/B de um experimento a partir da telemetria local
/// (`.forge/telemetry.db`). Exige exatamente 2 variantes (o A/B é entre duas);
/// o veredito de significância é derivado dos dados — nunca inventa vencedor.
fn run_experiment(experiment: String, db: Option<PathBuf>, format: VerifyFormat) -> Result<()> {
    let root = std::env::current_dir().context("diretório atual")?;
    let db_path = db.unwrap_or_else(|| root.join(".forge").join("telemetry.db"));
    let telemetry = Telemetry::open(db_path.to_str().unwrap_or(".forge/telemetry.db"))?;

    let variants = telemetry.experiment_variants(&experiment);
    if variants.len() != 2 {
        bail!(
            "A/B exige exatamente 2 variantes; o experimento '{experiment}' tem {} \
             (procuro eventos com props.experiment='{experiment}' e props.variant na telemetria)",
            variants.len()
        );
    }
    let a = VariantStats::new(variants[0].0.clone(), variants[0].1, variants[0].2);
    let b = VariantStats::new(variants[1].0.clone(), variants[1].1, variants[1].2);
    let report =
        ExperimentReport::from_two_variants(experiment, "success_rate", a, b, now_rfc3339());

    match format {
        VerifyFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        VerifyFormat::Human => print_experiment_human(&report),
    }
    Ok(())
}

fn print_experiment_human(report: &ExperimentReport) {
    use forge_schemas::experiment::{ExperimentVerdict, MIN_SAMPLES};
    println!(
        "Experimento: {}  (métrica: {})",
        report.experiment, report.metric
    );
    for v in &report.variants {
        println!(
            "  {}: {}/{} sucessos = {:.1}%",
            v.variant,
            v.successes,
            v.n,
            v.rate * 100.0
        );
    }
    match report.verdict {
        ExperimentVerdict::Significant => println!(
            "Veredito: VENCEDOR {} (p = {:.4} < {ALPHA}) — diferença significativa",
            report.winner.as_deref().unwrap_or("?"),
            report.p_value,
            ALPHA = forge_schemas::experiment::ALPHA,
        ),
        ExperimentVerdict::Inconclusive => println!(
            "Veredito: SEM SIGNIFICÂNCIA (p = {:.4}) — sem vencedor",
            report.p_value
        ),
        ExperimentVerdict::InsufficientData => {
            println!("Veredito: DADOS INSUFICIENTES — mínimo de {MIN_SAMPLES} eventos por variante")
        }
    }
}

/// Sobe o dashboard de telemetria lendo `.forge/telemetry.db` do diretório
/// atual (criado, se ausente, por `run`/`chat`).
async fn run_dashboard(port: u16, web_agent: bool) -> Result<()> {
    let root = std::env::current_dir().context("diretório atual")?;
    let forge_dir = root.join(".forge");
    std::fs::create_dir_all(&forge_dir)?;
    let telemetry = Telemetry::open(
        forge_dir
            .join("telemetry.db")
            .to_str()
            .unwrap_or(".forge/telemetry.db"),
    )?;
    // Mesmo arquivo (`.forge/prompt_library.db`) que `/prompt save|library|...`
    // do chat REPL já usa — não uma segunda biblioteca de prompts. Aberta uma
    // vez aqui (não por requisição) e compartilhada via Arc<Mutex<_>>, mesmo
    // motivo de `Telemetry` já ser um handle compartilhável (Fase 7 Onda 5).
    let prompt_library =
        std::sync::Arc::new(std::sync::Mutex::new(forge_store::PromptLibrary::open(
            forge_dir
                .join("prompt_library.db")
                .to_str()
                .unwrap_or(".forge/prompt_library.db"),
        )?));
    // Mesmo ledger (`.forge/forge.db`) que `session.rs`/`squad_agent.rs` já
    // gravam — a tela só lê o que a CLI/squad já registraram, não uma
    // segunda cadeia (Fase 7 Onda 6).
    let ledger = std::sync::Arc::new(std::sync::Mutex::new(forge_store::LedgerStore::open(
        forge_dir
            .join("forge.db")
            .to_str()
            .unwrap_or(".forge/forge.db"),
    )?));
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let web_dir = forge_server::default_web_dir();
    if web_agent {
        eprintln!(
            "forge dashboard (--web-agent) — http://{addr} (assets: {})",
            web_dir.display()
        );
        let hub = web_agent::default_hub();
        let squad_hub = squad_agent::default_hub();
        let squad_pool = squad_agent::default_squad_pool(&root);
        let squad_router = squad_agent::router(squad_hub, squad_pool);
        let sidecar_service = prompt_render::default_sidecar_service(&root);
        let prompt_router = prompt_render::router(sidecar_service);
        let mcp_router = mcp_console::router(root.clone());
        let extra_router = squad_router.merge(prompt_router).merge(mcp_router);
        web_agent::serve_with_agent(
            telemetry,
            prompt_library,
            ledger,
            &root,
            addr,
            web_dir,
            hub,
            extra_router,
        )
        .await?;
    } else {
        eprintln!(
            "forge dashboard — http://{addr} (assets: {})",
            web_dir.display()
        );
        forge_server::serve(telemetry, prompt_library, ledger, &root, addr, web_dir).await?;
    }
    Ok(())
}

/// Carrega `forge.toml` (`root/forge.toml` se `config` for `None`) ou cai no
/// default que espelha o job `rust` do CI, e roda o pipeline determinístico.
/// Compartilhado entre `forge verify` e `forge squad` (Fase 5 Onda 3: o squad
/// roda o mesmo `/verify` antes de disparar a tarefa, anexando a evidência
/// ao `SquadTask`) — evita duplicar a lógica de carregar config + rodar.
pub(crate) fn run_verify_pipeline(
    root: &std::path::Path,
    config: Option<&std::path::Path>,
) -> Result<forge_schemas::verification::VerificationEvidence> {
    let config_path = config
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.join("forge.toml"));
    let steps = match forge_verify::config::load_config(&config_path)
        .with_context(|| format!("lendo {}", config_path.display()))?
    {
        Some(cfg) => cfg.to_step_specs(),
        None => forge_verify::config::default_steps(),
    };

    let run_id = format!("run-{:x}", nanos_now() & 0xffff_ffff_ffff);
    let sha = git_sha().unwrap_or_else(|| "unknown".to_string());
    let produced_at = now_rfc3339();
    Ok(forge_verify::run_pipeline(
        &run_id,
        &sha,
        &produced_at,
        &steps,
    ))
}

/// Roda `/verify`: carrega `forge.toml` (ou cai no default, que espelha o
/// job `rust` do CI) na raiz do diretório atual, executa o pipeline
/// determinístico e grava `verification-evidence.v1` em disco.
///
/// Sai com código ≠ 0 quando o veredito é `Fail` — é o gate que a Onda 6
/// (CI) vai cobrar para o self-hosting. Isso é resultado legítimo do
/// verify, não um crash: por isso usa `process::exit` **depois** de gravar
/// o artefato e imprimir o resumo, em vez de `anyhow::bail!` (que
/// imprimiria como se fosse erro inesperado, com o prefixo "Error:").
///
/// Não recebe `root` via `prepare()` como `run`/`chat`/`squad` — verify é
/// determinístico e offline (sem provider de LLM), então resolve
/// `current_dir()` por conta própria, igual a `run_dashboard`.
fn run_verify(config: Option<PathBuf>, out: Option<PathBuf>, format: VerifyFormat) -> Result<()> {
    let root = std::env::current_dir().context("diretório atual")?;
    let forge_dir = root.join(".forge");
    std::fs::create_dir_all(&forge_dir)?;

    let evidence = run_verify_pipeline(&root, config.as_deref())?;
    let run_id = evidence.run_id.clone();

    let out_path = out.unwrap_or_else(|| forge_dir.join("evidence").join(format!("{run_id}.json")));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&evidence).context("serializando evidência")?;
    std::fs::write(&out_path, &json).with_context(|| format!("gravando {}", out_path.display()))?;

    match format {
        VerifyFormat::Json => println!("{json}"),
        VerifyFormat::Human => {
            println!("forge verify — run {run_id} ({})", evidence.git_sha);
            for step in &evidence.steps {
                let mark = if step.exit_code == 0 { "✓" } else { "✗" };
                println!(
                    "  {mark} {} ({}ms) — {} finding(s)",
                    step.name,
                    step.duration_ms,
                    step.findings.len()
                );
                for finding in &step.findings {
                    let loc = match (&finding.file, finding.line) {
                        (Some(f), Some(l)) => format!(" [{f}:{l}]"),
                        (Some(f), None) => format!(" [{f}]"),
                        _ => String::new(),
                    };
                    println!("      {} {}{loc}", finding.severity, finding.message);
                }
            }
            println!("veredito: {:?}", evidence.verdict);
            println!("evidência: {}", out_path.display());
        }
    }

    if matches!(evidence.verdict, forge_schemas::verification::Verdict::Fail) {
        std::process::exit(1);
    }
    Ok(())
}

/// `git rev-parse HEAD` best-effort — git ausente/repo fora de um worktree
/// não deve abortar o verify, só perder a rastreabilidade do sha.
fn git_sha() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

fn nanos_now() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Monta o gerador concreto (gateway + rate limit + cache, salvo
/// --no-cache) e valida que há providers configurados. Telemetria
/// (`.forge/telemetry.db`) registra `llm.call`/`cache.hit`/`cache.miss`
/// sem nunca derrubar o caminho principal.
fn prepare(opts: &RunOpts) -> Result<(CliGenerator, PathBuf)> {
    let gateway = Gateway::from_env();
    let available = gateway.available();
    if available.is_empty() {
        bail!(
            "nenhum provider configurado — defina ANTHROPIC_API_KEY, DEEPSEEK_API_KEY ou OPENAI_API_KEY"
        );
    }
    let root = std::env::current_dir().context("diretório atual")?;
    let tier = tier_from_id(&opts.model);
    eprintln!(
        "forge — modelo {} ({}) · agente {} · providers: {} · cache: {}",
        opts.model,
        tier_name(tier),
        opts.agent,
        available.join(", "),
        if opts.no_cache { "off" } else { "on" }
    );

    let forge_dir = root.join(".forge");
    std::fs::create_dir_all(&forge_dir)?;
    let cache = if opts.no_cache {
        // Cache em memória: satisfaz o tipo sem persistir nada.
        PromptCache::open_in_memory()?
    } else {
        PromptCache::open(
            forge_dir
                .join("cache.db")
                .to_str()
                .unwrap_or(".forge/cache.db"),
        )?
    };
    let telemetry = Telemetry::open(
        forge_dir
            .join("telemetry.db")
            .to_str()
            .unwrap_or(".forge/telemetry.db"),
    )
    .ok();
    let limited =
        RateLimitedGenerator::new(gateway, RateLimiter::for_tier(tier), telemetry.clone());
    Ok((CachedGenerator::new(limited, cache, telemetry), root))
}

fn build_loop<'a, G: Generator>(
    generator: &'a G,
    opts: &RunOpts,
    tools: &'a ToolRegistry,
) -> Result<AgentLoop<'a, G>> {
    let profile = match opts.agent.as_str() {
        "build" => &BUILD,
        "plan" => &PLAN,
        other => bail!("agente desconhecido: {other} (use build ou plan)"),
    };
    let tier = tier_from_id(&opts.model);
    Ok(AgentLoop {
        generator,
        tools,
        permissions: (profile.permissions)(),
        model: opts.model.clone(),
        system: system_prompt(tier),
        max_steps: 20,
        max_tokens: 4096,
    })
}

/// Abre a sessão durável (nova ou retomada) em `.forge/sessions.db`.
fn open_durable(root: &std::path::Path, opts: &RunOpts, task_hint: &str) -> Result<DurableSession> {
    let store = EventStore::open(
        root.join(".forge")
            .join("sessions.db")
            .to_str()
            .unwrap_or(".forge/sessions.db"),
    )?;
    let session_id = opts.session.clone().unwrap_or_else(|| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("s{:x}", nanos & 0xffff_ffff_ffff)
    });
    let durable = DurableSession::open(store, &session_id, task_hint, &opts.model)?;
    if durable.resumed_messages() > 0 {
        eprintln!(
            "sessão {session_id} retomada — {} mensagem(ns) no histórico",
            durable.resumed_messages()
        );
    } else {
        eprintln!("sessão {session_id} (retome com --session {session_id})");
    }
    Ok(durable)
}

/// Compacta a sessão se a política mandar e a fronteira for segura.
/// Retorna true se uma nova época começou.
async fn maybe_compact<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    durable: &mut DurableSession,
    session: &mut session::Session,
    force: bool,
) -> Result<bool> {
    let policy = CompactionPolicy::for_tier(tier_from_id(&opts.model), opts.context_window);
    if !(force || policy.needs_compaction(&durable.messages)) {
        return Ok(false);
    }
    if !CompactionPolicy::is_safe_boundary(&durable.messages) {
        if force {
            eprintln!("  compaction adiada: fronteira insegura (turno incompleto)");
        }
        return Ok(false);
    }
    let summary = policy
        .summarize(generator, &opts.model, &durable.messages)
        .await
        .map_err(|e| anyhow::anyhow!("compaction: {e}"))?;
    durable.compact(&summary)?;
    session.note(
        "compaction.applied",
        serde_json::json!({"epoch": durable.epoch(), "summary_chars": summary.len()}),
    );
    eprintln!(
        "  ⟲ contexto compactado — época {} ({} chars de resumo)",
        durable.epoch(),
        summary.len()
    );
    Ok(true)
}

/// Registra no ledger o veredito do vetter para cada skill (built-in +
/// terceiro), como auditoria append-only (Fase 6 Onda 3). Reusa
/// `list_skill_statuses` (o mesmo que alimenta `/api/skills`).
fn record_skill_vetting(root: &std::path::Path, session: &mut session::Session) {
    use forge_verify::vetter::list_skill_statuses;
    let mut statuses = list_skill_statuses(&root.join("skills"), "builtin");
    statuses.extend(list_skill_statuses(
        &root.join(".forge").join("skills"),
        "third-party",
    ));
    for s in statuses {
        session.note(
            "skill.vetting",
            serde_json::json!({"id": s.id, "status": s.status, "detail": s.detail}),
        );
    }
}

async fn run_once<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    root: &std::path::Path,
    task: String,
) -> Result<()> {
    let tools = crate::skills::build_registry(root);
    let agent_loop = build_loop(generator, opts, &tools)?;
    let mut session = session::Session::open(root, &task, &opts.model)?;
    // Fase 6 Onda 3: audita no ledger (append-only) o veredito do vetter para
    // cada skill carregada. A execução de skill já entra no ledger pelos
    // LoopEvents; isto registra a decisão de vetting em si.
    record_skill_vetting(root, &mut session);
    let mut durable = open_durable(root, opts, &task)?;
    let mut resolver = CliResolver { auto_yes: opts.yes };

    // Sidecar opcional (Fase 3): lint consultivo, nunca bloqueante.
    if let Some((_supervisor, mut client)) = sidecar::try_start().await {
        if let Ok(report) = client.lint(&task).await {
            if let Some(notice) = sidecar::advisory(&report) {
                eprintln!("{notice}");
            }
        }
    }

    maybe_compact(generator, opts, &mut durable, &mut session, false).await?;
    durable.messages.push(ChatMessage::user_text(&task));
    let result = {
        let mut on_event = |event: LoopEvent| {
            print_event(&event);
            session.record(&event);
        };
        agent_loop
            .continue_run(&mut durable.messages, &mut resolver, &mut on_event)
            .await
    };
    let persisted = durable.persist_new().unwrap_or_else(|e| {
        eprintln!("  [sessão] falha ao persistir: {e}");
        0
    });
    match result {
        Ok(summary) => {
            session.finish(true, summary.steps)?;
            eprintln!(
                "\nconcluído em {} passo(s) · {} mensagem(ns) persistida(s) · ledger íntegro: {} entrada(s)",
                summary.steps,
                persisted,
                session.verify()?
            );
            Ok(())
        }
        Err(e) => {
            session.finish(false, 0)?;
            bail!("{e}");
        }
    }
}

async fn chat_repl<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    root: &std::path::Path,
) -> Result<()> {
    let tools = crate::skills::build_registry(root);
    let agent_loop = build_loop(generator, opts, &tools)?;
    let mut session = session::Session::open(root, "<chat>", &opts.model)?;
    let mut resolver = CliResolver { auto_yes: opts.yes };

    let mut durable = open_durable(root, opts, "<chat>")?;
    // Sidecar opcional (Fase 3): mantido vivo durante todo o chat para
    // lint consultivo e o comando /prompt; None se indisponível (degrada).
    let sidecar_session = sidecar::try_start().await;
    if sidecar_session.is_none() {
        eprintln!("  (sidecar PromptForge indisponível — render de geradores fica desativado; biblioteca continua ativa)");
    }
    let library = forge_store::PromptLibrary::open(
        root.join(".forge")
            .join("prompt_library.db")
            .to_str()
            .unwrap_or(".forge/prompt_library.db"),
    )?;
    eprintln!("forge chat — digite a mensagem (vazio, \"sair\" ou Ctrl-D encerra; /compact força nova época; /prompt lista geradores; /prompt save|library|use|fav|rm gerencia a biblioteca)\n");
    let stdin = std::io::stdin();
    let mut turns = 0usize;

    loop {
        eprint!("> ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // EOF
        }
        let input = line.trim();
        if input.is_empty() || matches!(input, "sair" | "exit" | "quit") {
            break;
        }
        if input == "/compact" {
            if !maybe_compact(generator, opts, &mut durable, &mut session, true).await? {
                eprintln!("  nada a compactar");
            }
            continue;
        }
        if let Some(rest) = input.strip_prefix("/prompt") {
            let sidecar_client = sidecar_session.as_ref().map(|(_, c)| c.clone());
            handle_prompt_command(sidecar_client, &library, rest.trim()).await;
            continue;
        }
        maybe_compact(generator, opts, &mut durable, &mut session, false).await?;

        if let Some((_, client)) = &sidecar_session {
            if let Ok(report) = client.clone().lint(input).await {
                if let Some(notice) = sidecar::advisory(&report) {
                    eprintln!("{notice}");
                }
            }
        }

        session.note("user.turn", serde_json::json!({"chars": input.len()}));
        durable.messages.push(ChatMessage::user_text(input));
        let result = {
            let mut on_event = |event: LoopEvent| {
                print_event(&event);
                session.record(&event);
            };
            agent_loop
                .continue_run(&mut durable.messages, &mut resolver, &mut on_event)
                .await
        };
        if let Err(e) = durable.persist_new() {
            eprintln!("  [sessão] falha ao persistir: {e}");
        }
        match result {
            Ok(_) => turns += 1,
            Err(e) => {
                eprintln!("\nerro: {e}");
                break;
            }
        }
        println!();
    }

    session.finish(true, turns)?;
    eprintln!(
        "\nchat encerrado após {turns} turno(s) · ledger íntegro: {} entrada(s)",
        session.verify()?
    );
    Ok(())
}

/// Trata `/prompt` no chat. Sem argumentos (ou `list`) lista os geradores
/// do sidecar; `<gerador> chave=valor ...` renderiza e imprime o prompt;
/// `save <nome> [tags=a,b] <gerador> chave=valor ...` renderiza e grava na
/// biblioteca (origem: prompte `library.js`); `library [tag]` lista os
/// prompts salvos; `use <id>` reimprime um prompt salvo; `fav <id>`
/// inverte o favorito; `rm <id>` remove. A biblioteca funciona mesmo sem
/// sidecar — só `save` e o render bruto exigem o gerador Python ativo.
async fn handle_prompt_command(
    mut sidecar: Option<forge_sidecar::SidecarClient>,
    library: &forge_store::PromptLibrary,
    rest: &str,
) {
    if rest.is_empty() || rest == "list" {
        let Some(client) = sidecar.as_mut() else {
            eprintln!("  sidecar indisponível — geradores desativados");
            return;
        };
        match client.list_generators().await {
            Ok(generators) => {
                eprintln!("  geradores disponíveis:");
                for g in generators {
                    let fields: Vec<String> = g.fields.iter().map(|f| f.name.clone()).collect();
                    eprintln!(
                        "    {} [{}] — campos: {}",
                        g.name,
                        g.category,
                        fields.join(", ")
                    );
                }
            }
            Err(e) => eprintln!("  falha ao listar geradores: {e}"),
        }
        return;
    }

    let first_token = rest.split_whitespace().next().unwrap_or("");
    let command_arg = rest[first_token.len()..].trim();

    if first_token == "library" {
        let tag = if command_arg.is_empty() {
            None
        } else {
            Some(command_arg)
        };
        match library.list(tag) {
            Ok(prompts) if prompts.is_empty() => eprintln!("  biblioteca vazia"),
            Ok(prompts) => {
                eprintln!("  prompts salvos:");
                for p in prompts {
                    eprintln!(
                        "    #{} {}{} [{}] — tags: {}",
                        p.id,
                        p.name,
                        if p.favorite { " ★" } else { "" },
                        p.generator,
                        p.tags.join(", ")
                    );
                }
            }
            Err(e) => eprintln!("  falha ao listar biblioteca: {e}"),
        }
        return;
    }

    if first_token == "use" {
        let Ok(id) = command_arg.parse::<i64>() else {
            eprintln!("  uso: /prompt use <id>");
            return;
        };
        match library.get(id) {
            Ok(Some(p)) => eprintln!(
                "  --- {} ({}) ---\n{}\n  ---------------------",
                p.name, p.generator, p.rendered
            ),
            Ok(None) => eprintln!("  prompt #{id} não encontrado"),
            Err(e) => eprintln!("  falha ao buscar prompt #{id}: {e}"),
        }
        return;
    }

    if first_token == "fav" {
        let Ok(id) = command_arg.parse::<i64>() else {
            eprintln!("  uso: /prompt fav <id>");
            return;
        };
        match library.toggle_favorite(id) {
            Ok(Some(state)) => eprintln!("  prompt #{id} favorito: {state}"),
            Ok(None) => eprintln!("  prompt #{id} não encontrado"),
            Err(e) => eprintln!("  falha ao favoritar #{id}: {e}"),
        }
        return;
    }

    if first_token == "rm" {
        let Ok(id) = command_arg.parse::<i64>() else {
            eprintln!("  uso: /prompt rm <id>");
            return;
        };
        match library.delete(id) {
            Ok(true) => eprintln!("  prompt #{id} removido"),
            Ok(false) => eprintln!("  prompt #{id} não encontrado"),
            Err(e) => eprintln!("  falha ao remover #{id}: {e}"),
        }
        return;
    }

    if first_token == "save" {
        let Some(client) = sidecar.as_mut() else {
            eprintln!("  sidecar indisponível — save exige o gerador Python ativo");
            return;
        };
        let mut parts = command_arg.split_whitespace();
        let Some(prompt_name) = parts.next() else {
            eprintln!("  uso: /prompt save <nome> [tags=a,b] <gerador> chave=valor ...");
            return;
        };
        let mut tags = Vec::new();
        let mut generator_name = None;
        let mut fields = std::collections::HashMap::new();
        for token in parts {
            if let Some(list) = token.strip_prefix("tags=") {
                tags = list
                    .split(',')
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
                    .collect();
            } else if generator_name.is_none() {
                generator_name = Some(token.to_string());
            } else if let Some((k, v)) = token.split_once('=') {
                fields.insert(k.to_string(), v.to_string());
            }
        }
        let Some(generator_name) = generator_name else {
            eprintln!("  uso: /prompt save <nome> [tags=a,b] <gerador> chave=valor ...");
            return;
        };
        let fields_json: Value = fields
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect::<serde_json::Map<_, _>>()
            .into();
        match client.render(&generator_name, fields).await {
            Ok(rendered) => {
                match library.save(
                    prompt_name,
                    &generator_name,
                    &fields_json,
                    &rendered,
                    &tags,
                    &now_rfc3339(),
                ) {
                    Ok(id) => eprintln!("  prompt #{id} salvo na biblioteca"),
                    Err(e) => eprintln!("  falha ao salvar na biblioteca: {e}"),
                }
            }
            Err(e) => eprintln!("  falha ao renderizar {generator_name}: {e}"),
        }
        return;
    }

    let Some(client) = sidecar.as_mut() else {
        eprintln!("  sidecar indisponível — render de geradores desativado");
        return;
    };
    let mut parts = rest.split_whitespace();
    let Some(name) = parts.next() else { return };
    let mut fields = std::collections::HashMap::new();
    for pair in parts {
        if let Some((k, v)) = pair.split_once('=') {
            fields.insert(k.to_string(), v.to_string());
        }
    }
    match client.render(name, fields).await {
        Ok(prompt) => eprintln!("  --- prompt gerado ---\n{prompt}\n  ---------------------"),
        Err(e) => eprintln!("  falha ao renderizar {name}: {e}"),
    }
}

fn print_event(event: &LoopEvent) {
    match event {
        LoopEvent::TextDelta(d) => {
            print!("{d}");
            let _ = std::io::stdout().flush();
        }
        LoopEvent::TurnCompleted { .. } => println!(),
        LoopEvent::ToolStarted { name, scope } => eprintln!("  ⚒ {name} {scope:?}"),
        LoopEvent::ToolFinished {
            name, ok, summary, ..
        } => {
            eprintln!("  {} {name}: {summary}", if *ok { "✓" } else { "✗" })
        }
        LoopEvent::ToolDenied { name, scope } => eprintln!("  ⛔ {name} {scope:?} negado"),
    }
}

/// Pergunta ao usuário no terminal quando a política devolve `Ask`.
struct CliResolver {
    auto_yes: bool,
}

impl PermissionResolver for CliResolver {
    fn resolve(&mut self, tool: &str, scope: &str) -> bool {
        if self.auto_yes {
            return true;
        }
        eprint!("\n  permitir {tool} em {scope:?}? [s/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(
            answer.trim().to_lowercase().as_str(),
            "s" | "sim" | "y" | "yes"
        )
    }
}

fn system_prompt(tier: ModelTier) -> String {
    let base = "Você é o forge, um coding agent de terminal. Trabalhe no diretório atual \
usando as ferramentas disponíveis (read, grep, edit, bash). Leia antes de editar; edits \
exigem old_string única. Verifique seu trabalho com as ferramentas (testes, build) antes \
de concluir. Seja direto e objetivo nas respostas.";
    match tier {
        // Disciplina de passos para modelos small (fork do opencode).
        ModelTier::Small => format!("{base} Faça UMA ação por vez e reavalie após cada resultado."),
        _ => base.to_string(),
    }
}

fn tier_name(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Small => "small",
        ModelTier::Medium => "medium",
        ModelTier::Large => "large",
    }
}
