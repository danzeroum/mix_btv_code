//! `forge` — CLI da plataforma unificada (BuildToValue + opencode + prompte).
//!
//! Fase 1: `run` executa o loop de agente real e `chat` abre o REPL
//! multi-turno — gateway LLM com streaming e cache por hash, ferramentas
//! sob permissão interativa e ledger em `.forge/forge.db`.
//! `squad` ativa o sidecar Python na Fase 4; `verify` completa na Fase 5.

mod cache;
mod session;
mod tui_app;

use anyhow::{bail, Context, Result};
use cache::CachedGenerator;
use clap::{Parser, Subcommand};
use forge_core::{
    AgentLoop, CompactionPolicy, DurableSession, LoopEvent, PermissionResolver, BUILD, PLAN,
};
use forge_llm::chat::ChatMessage;
use forge_llm::{tier_from_id, Gateway, Generator, ModelTier};
use forge_store::{EventStore, PromptCache};
use forge_tools::ToolRegistry;
use std::io::{BufRead, Write};
use std::path::PathBuf;

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
    /// Roda o pipeline de verificação determinística.
    Verify,
    /// Delega a tarefa ao squad multi-agente (requer sidecar Python).
    Squad { task: String },
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
        Commands::Verify => {
            println!("forge verify — pipeline em implementação (Fase 5 do roadmap)");
            Ok(())
        }
        Commands::Squad { task } => {
            println!("forge squad — tarefa: {task:?}");
            println!("(sidecar forge-squadd em implementação — Fase 4 do roadmap)");
            Ok(())
        }
    }
}

/// Monta o gerador concreto (gateway + cache, salvo --no-cache) e valida
/// que há providers configurados.
fn prepare(opts: &RunOpts) -> Result<(CachedGenerator<Gateway>, PathBuf)> {
    let gateway = Gateway::from_env();
    let available = gateway.available();
    if available.is_empty() {
        bail!(
            "nenhum provider configurado — defina ANTHROPIC_API_KEY, DEEPSEEK_API_KEY ou OPENAI_API_KEY"
        );
    }
    let root = std::env::current_dir().context("diretório atual")?;
    eprintln!(
        "forge — modelo {} ({}) · agente {} · providers: {} · cache: {}",
        opts.model,
        tier_name(tier_from_id(&opts.model)),
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
    Ok((CachedGenerator::new(gateway, cache), root))
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

async fn run_once<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    root: &std::path::Path,
    task: String,
) -> Result<()> {
    let tools = ToolRegistry::default_set(root);
    let agent_loop = build_loop(generator, opts, &tools)?;
    let mut session = session::Session::open(root, &task, &opts.model)?;
    let mut durable = open_durable(root, opts, &task)?;
    let mut resolver = CliResolver { auto_yes: opts.yes };

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
    let tools = ToolRegistry::default_set(root);
    let agent_loop = build_loop(generator, opts, &tools)?;
    let mut session = session::Session::open(root, "<chat>", &opts.model)?;
    let mut resolver = CliResolver { auto_yes: opts.yes };

    let mut durable = open_durable(root, opts, "<chat>")?;
    eprintln!("forge chat — digite a mensagem (vazio, \"sair\" ou Ctrl-D encerra; /compact força nova época)\n");
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
        maybe_compact(generator, opts, &mut durable, &mut session, false).await?;

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

fn print_event(event: &LoopEvent) {
    match event {
        LoopEvent::TextDelta(d) => {
            print!("{d}");
            let _ = std::io::stdout().flush();
        }
        LoopEvent::TurnCompleted { .. } => println!(),
        LoopEvent::ToolStarted { name, scope } => eprintln!("  ⚒ {name} {scope:?}"),
        LoopEvent::ToolFinished { name, ok, summary } => {
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
