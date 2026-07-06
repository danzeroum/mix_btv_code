//! Comando `forge tui`: chat com a interface ratatui.
//!
//! Arquitetura: o loop de agente roda numa task tokio e conversa com a UI
//! por canais — eventos do loop viram [`TuiMsg`]s aplicados ao
//! [`TuiState`]; pedidos de permissão bloqueiam o resolver até o usuário
//! responder no modal (s/n). A UI roda na thread principal (crossterm),
//! com o terminal restaurado por guard mesmo em erro.
//!
//! Seletor de modelo/agente: `Ctrl+M` percorre uma lista curada de modelos
//! (cobrindo os tiers) e `Ctrl+G` alterna build/plan. A troca é um
//! `UiCommand` consumido pela task do agente, que reconstrói o
//! `AgentLoop` (barato — sem I/O) antes do próximo turno.

use crate::{maybe_compact, open_durable, session::Session, RunOpts};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use forge_core::{LoopEvent, PermissionResolver};
use forge_llm::chat::ChatMessage;
use forge_llm::Generator;
use forge_tools::DiffLine;
use forge_tui::{DiffKind, Item, PermissionPrompt, TuiState};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Modelos curados para o seletor — um por tier (small/medium/large),
/// cobrindo os três providers do gateway.
const MODEL_CHOICES: &[&str] = &[
    "claude-sonnet-5",
    "claude-opus-4-8",
    "claude-haiku-4-5",
    "deepseek-chat",
    "gpt-4o",
    "gpt-4o-mini",
];

/// Mensagens do loop de agente para a UI (donas dos dados — atravessam threads).
#[derive(Debug)]
pub enum TuiMsg {
    Delta(String),
    TurnDone,
    Tool {
        name: String,
        detail: String,
        ok: bool,
        diff: Option<Vec<DiffLine>>,
    },
    Permission {
        tool: String,
        scope: String,
    },
    Notice(String),
    Status(String),
    Idle,
    Fatal(String),
}

/// Comando da UI para a task do agente.
pub enum UiCommand {
    Send(String),
    SetModel(String),
    SetAgent(String),
}

fn diff_kind(line: &DiffLine) -> (DiffKind, &str) {
    match line {
        DiffLine::Context(s) => (DiffKind::Context, s.as_str()),
        DiffLine::Removed(s) => (DiffKind::Removed, s.as_str()),
        DiffLine::Added(s) => (DiffKind::Added, s.as_str()),
    }
}

/// Aplica uma mensagem ao estado da UI (puro — testável).
pub fn apply(state: &mut TuiState, msg: TuiMsg) {
    match msg {
        TuiMsg::Delta(d) => state.streaming.push_str(&d),
        TuiMsg::TurnDone => state.finish_turn(),
        TuiMsg::Tool {
            name,
            detail,
            ok,
            diff,
        } => {
            state.items.push(Item::Tool { name, detail, ok });
            if let Some(diff) = diff.filter(|d| !d.is_empty()) {
                state.items.push(Item::Diff(
                    diff.iter()
                        .map(|l| {
                            let (k, s) = diff_kind(l);
                            (k, s.to_string())
                        })
                        .collect(),
                ));
            }
        }
        TuiMsg::Permission { tool, scope } => {
            state.permission = Some(PermissionPrompt { tool, scope })
        }
        TuiMsg::Notice(text) => state.items.push(Item::Notice(text)),
        TuiMsg::Status(text) => state.status = text,
        TuiMsg::Idle => state.busy = false,
        TuiMsg::Fatal(text) => {
            state
                .items
                .push(Item::Notice(format!("erro fatal: {text}")));
            state.busy = false;
        }
    }
}

/// Resolver de permissões da TUI: publica o pedido e bloqueia a task do
/// agente até a resposta do modal.
struct TuiResolver {
    evt_tx: mpsc::UnboundedSender<TuiMsg>,
    resp_rx: std_mpsc::Receiver<bool>,
}

impl PermissionResolver for TuiResolver {
    fn resolve(&mut self, tool: &str, scope: &str) -> bool {
        if self
            .evt_tx
            .send(TuiMsg::Permission {
                tool: tool.to_string(),
                scope: scope.to_string(),
            })
            .is_err()
        {
            return false;
        }
        self.resp_rx.recv().unwrap_or(false)
    }
}

/// Restaura o terminal ao sair (inclusive por erro).
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

fn status_line(opts: &RunOpts) -> String {
    format!(
        "modelo {} · agente {} · Ctrl+M troca modelo · Ctrl+G troca agente · Esc sai",
        opts.model, opts.agent
    )
}

pub async fn run_tui<G: Generator + Send + Sync + 'static>(
    generator: std::sync::Arc<G>,
    opts: RunOpts,
    root: std::path::PathBuf,
) -> Result<()> {
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<TuiMsg>();
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<UiCommand>();
    let (resp_tx, resp_rx) = std_mpsc::channel::<bool>();

    // Sidecar opcional (Fase 3): mantido vivo pelo escopo de run_tui inteiro;
    // None se indisponível (lint fica desativado, sem afetar o restante).
    let sidecar_session = crate::sidecar::try_start().await;
    if sidecar_session.is_none() {
        let _ = evt_tx.send(TuiMsg::Notice(
            "sidecar PromptForge indisponível — aviso de lint desativado".into(),
        ));
    }
    let sidecar_client = sidecar_session.as_ref().map(|(_, client)| client.clone());

    // Task do agente: consome comandos da UI e emite TuiMsgs.
    let agent_evt_tx = evt_tx.clone();
    let mut agent_opts = opts.clone();
    let agent_root = root.clone();
    let agent = tokio::spawn(async move {
        let run = async {
            let tools = crate::skills::build_registry(&agent_root);
            let mut ledger = Session::open(&agent_root, "<tui>", &agent_opts.model)?;
            let mut durable = open_durable(&agent_root, &agent_opts, "<tui>")?;
            if durable.resumed_messages() > 0 {
                let _ = agent_evt_tx.send(TuiMsg::Notice(format!(
                    "sessão {} retomada ({} mensagens)",
                    durable.session_id,
                    durable.resumed_messages()
                )));
            }
            let mut resolver = TuiResolver {
                evt_tx: agent_evt_tx.clone(),
                resp_rx,
            };

            while let Some(cmd) = input_rx.recv().await {
                let input = match cmd {
                    UiCommand::SetModel(m) => {
                        agent_opts.model = m;
                        let _ = agent_evt_tx.send(TuiMsg::Status(status_line(&agent_opts)));
                        continue;
                    }
                    UiCommand::SetAgent(a) => {
                        agent_opts.agent = a;
                        let _ = agent_evt_tx.send(TuiMsg::Status(status_line(&agent_opts)));
                        continue;
                    }
                    UiCommand::Send(text) => text,
                };

                if let Some(client) = &sidecar_client {
                    if let Ok(report) = client.clone().lint(&input).await {
                        if let Some(notice) = crate::sidecar::advisory(&report) {
                            let _ = agent_evt_tx.send(TuiMsg::Notice(notice));
                        }
                    }
                }

                // Reconstrói o loop a cada turno: barato (sem I/O) e sempre
                // reflete o modelo/agente correntes escolhidos na UI.
                let agent_loop = crate::build_loop(generator.as_ref(), &agent_opts, &tools)?;

                if maybe_compact(
                    generator.as_ref(),
                    &agent_opts,
                    &mut durable,
                    &mut ledger,
                    false,
                )
                .await?
                {
                    let _ = agent_evt_tx.send(TuiMsg::Notice(format!(
                        "contexto compactado — época {}",
                        durable.epoch()
                    )));
                }
                ledger.note("user.turn", serde_json::json!({"chars": input.len()}));
                durable.messages.push(ChatMessage::user_text(&input));

                let result = {
                    let evt = agent_evt_tx.clone();
                    let ledger = &mut ledger;
                    let mut on_event = move |event: LoopEvent| {
                        ledger.record(&event);
                        let msg = match event {
                            LoopEvent::TextDelta(d) => Some(TuiMsg::Delta(d.to_string())),
                            LoopEvent::TurnCompleted { .. } => Some(TuiMsg::TurnDone),
                            LoopEvent::ToolStarted { .. } => None,
                            LoopEvent::ToolFinished {
                                name,
                                ok,
                                summary,
                                diff,
                            } => Some(TuiMsg::Tool {
                                name,
                                detail: summary,
                                ok,
                                diff,
                            }),
                            LoopEvent::ToolDenied { name, scope } => Some(TuiMsg::Tool {
                                name,
                                detail: format!("{scope:?} negado"),
                                ok: false,
                                diff: None,
                            }),
                        };
                        if let Some(msg) = msg {
                            let _ = evt.send(msg);
                        }
                    };
                    agent_loop
                        .continue_run(&mut durable.messages, &mut resolver, &mut on_event)
                        .await
                };
                if let Err(e) = durable.persist_new() {
                    let _ = agent_evt_tx.send(TuiMsg::Notice(format!("falha ao persistir: {e}")));
                }
                if let Err(e) = result {
                    let _ = agent_evt_tx.send(TuiMsg::Notice(format!("erro: {e}")));
                }
                let _ = agent_evt_tx.send(TuiMsg::Idle);
            }
            anyhow::Ok(())
        };
        if let Err(e) = run.await {
            let _ = evt_tx.send(TuiMsg::Fatal(e.to_string()));
        }
    });

    // UI na thread principal.
    enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;

    let mut ui_opts = opts.clone();
    let mut state = TuiState {
        status: status_line(&ui_opts),
        ..Default::default()
    };

    let quit = tokio::task::block_in_place(|| -> Result<()> {
        loop {
            while let Ok(msg) = evt_rx.try_recv() {
                apply(&mut state, msg);
            }
            terminal.draw(|f| forge_tui::render(f, &state))?;

            if !event::poll(Duration::from_millis(50))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Modal de permissão captura o teclado.
            if state.permission.is_some() {
                match key.code {
                    KeyCode::Char('s') | KeyCode::Char('y') => {
                        state.permission = None;
                        let _ = resp_tx.send(true);
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        state.permission = None;
                        let _ = resp_tx.send(false);
                    }
                    _ => {}
                }
                continue;
            }

            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('c') => break,
                    KeyCode::Char('m') => {
                        let idx = MODEL_CHOICES
                            .iter()
                            .position(|m| *m == ui_opts.model)
                            .map(|i| (i + 1) % MODEL_CHOICES.len())
                            .unwrap_or(0);
                        ui_opts.model = MODEL_CHOICES[idx].to_string();
                        state.status = status_line(&ui_opts);
                        if input_tx
                            .send(UiCommand::SetModel(ui_opts.model.clone()))
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                    KeyCode::Char('g') => {
                        ui_opts.agent = if ui_opts.agent == "build" {
                            "plan"
                        } else {
                            "build"
                        }
                        .to_string();
                        state.status = status_line(&ui_opts);
                        if input_tx
                            .send(UiCommand::SetAgent(ui_opts.agent.clone()))
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
            }

            match key.code {
                KeyCode::Esc => break,
                KeyCode::Enter => {
                    let text = state.input.trim().to_string();
                    if !text.is_empty() && !state.busy {
                        state.items.push(Item::User(text.clone()));
                        state.input.clear();
                        state.busy = true;
                        if input_tx.send(UiCommand::Send(text)).is_err() {
                            break;
                        }
                    }
                }
                KeyCode::Backspace => {
                    state.input.pop();
                }
                KeyCode::Char(c) => state.input.push(c),
                _ => {}
            }
        }
        Ok(())
    });

    drop(input_tx);
    agent.abort();
    quit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_conduz_o_ciclo_de_um_turno() {
        let mut state = TuiState {
            busy: true,
            ..Default::default()
        };

        apply(&mut state, TuiMsg::Delta("Vou ".into()));
        apply(&mut state, TuiMsg::Delta("verificar.".into()));
        assert_eq!(state.streaming, "Vou verificar.");

        apply(&mut state, TuiMsg::TurnDone);
        assert!(state.streaming.is_empty());
        assert!(matches!(&state.items[0], Item::Assistant(t) if t == "Vou verificar."));

        apply(
            &mut state,
            TuiMsg::Tool {
                name: "edit".into(),
                detail: "src/lib.rs".into(),
                ok: true,
                diff: Some(vec![
                    DiffLine::Removed("let x = 1;".into()),
                    DiffLine::Added("let x = 2;".into()),
                ]),
            },
        );
        assert!(matches!(&state.items[1], Item::Tool { name, .. } if name == "edit"));
        assert!(matches!(&state.items[2], Item::Diff(lines) if lines.len() == 2));

        apply(
            &mut state,
            TuiMsg::Permission {
                tool: "bash".into(),
                scope: "cargo test".into(),
            },
        );
        assert!(state.permission.is_some());

        apply(&mut state, TuiMsg::Idle);
        assert!(!state.busy);
    }

    #[test]
    fn tool_sem_diff_nao_cria_item_de_diff() {
        let mut state = TuiState::default();
        apply(
            &mut state,
            TuiMsg::Tool {
                name: "read".into(),
                detail: "f.txt".into(),
                ok: true,
                diff: None,
            },
        );
        assert_eq!(state.items.len(), 1);
    }

    #[test]
    fn status_muda_com_o_modelo() {
        let mut state = TuiState::default();
        apply(&mut state, TuiMsg::Status("modelo x".into()));
        assert_eq!(state.status, "modelo x");
    }
}
