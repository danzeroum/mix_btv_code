//! Comando `forge tui`: chat com a interface ratatui.
//!
//! Arquitetura: o loop de agente roda numa task tokio e conversa com a UI
//! por canais — eventos do loop viram [`TuiMsg`]s aplicados ao
//! [`TuiState`]; pedidos de permissão bloqueiam o resolver até o usuário
//! responder no modal (s/n). A UI roda na thread principal (crossterm),
//! com o terminal restaurado por guard mesmo em erro.

use crate::{maybe_compact, open_durable, session::Session, RunOpts};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use forge_core::{LoopEvent, PermissionResolver};
use forge_llm::chat::ChatMessage;
use forge_llm::Generator;
use forge_tools::ToolRegistry;
use forge_tui::{Item, PermissionPrompt, TuiState};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Mensagens do loop de agente para a UI (donas dos dados — atravessam threads).
#[derive(Debug)]
pub enum TuiMsg {
    Delta(String),
    TurnDone,
    Tool {
        name: String,
        detail: String,
        ok: bool,
    },
    Permission {
        tool: String,
        scope: String,
    },
    Notice(String),
    Idle,
    Fatal(String),
}

/// Aplica uma mensagem ao estado da UI (puro — testável).
pub fn apply(state: &mut TuiState, msg: TuiMsg) {
    match msg {
        TuiMsg::Delta(d) => state.streaming.push_str(&d),
        TuiMsg::TurnDone => state.finish_turn(),
        TuiMsg::Tool { name, detail, ok } => state.items.push(Item::Tool { name, detail, ok }),
        TuiMsg::Permission { tool, scope } => {
            state.permission = Some(PermissionPrompt { tool, scope })
        }
        TuiMsg::Notice(text) => state.items.push(Item::Notice(text)),
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

pub async fn run_tui<G: Generator + Send + Sync + 'static>(
    generator: std::sync::Arc<G>,
    opts: RunOpts,
    root: std::path::PathBuf,
) -> Result<()> {
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<TuiMsg>();
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<String>();
    let (resp_tx, resp_rx) = std_mpsc::channel::<bool>();

    // Task do agente: consome entradas do usuário e emite TuiMsgs.
    let agent_evt_tx = evt_tx.clone();
    let agent_opts = opts.clone();
    let agent_root = root.clone();
    let agent = tokio::spawn(async move {
        let run = async {
            let tools = ToolRegistry::default_set(&agent_root);
            let mut ledger = Session::open(&agent_root, "<tui>", &agent_opts.model)?;
            let mut durable = open_durable(&agent_root, &agent_opts, "<tui>")?;
            if durable.resumed_messages() > 0 {
                let _ = agent_evt_tx.send(TuiMsg::Notice(format!(
                    "sessão {} retomada ({} mensagens)",
                    durable.session_id,
                    durable.resumed_messages()
                )));
            }
            let agent_loop = crate::build_loop(generator.as_ref(), &agent_opts, &tools)?;
            let mut resolver = TuiResolver {
                evt_tx: agent_evt_tx.clone(),
                resp_rx,
            };

            while let Some(input) = input_rx.recv().await {
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
                            LoopEvent::ToolFinished { name, ok, summary } => Some(TuiMsg::Tool {
                                name,
                                detail: summary,
                                ok,
                            }),
                            LoopEvent::ToolDenied { name, scope } => Some(TuiMsg::Tool {
                                name,
                                detail: format!("{scope:?} negado"),
                                ok: false,
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

    let mut state = TuiState {
        status: format!("modelo {} · Esc sai", opts.model),
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

            match key.code {
                KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Enter => {
                    let text = state.input.trim().to_string();
                    if !text.is_empty() && !state.busy {
                        state.items.push(Item::User(text.clone()));
                        state.input.clear();
                        state.busy = true;
                        if input_tx.send(text).is_err() {
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
            },
        );
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
}
