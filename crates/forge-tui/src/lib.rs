//! TUI da plataforma Forge (Fase 2).
//!
//! O modelo de estado e o render são puros (testáveis com
//! `ratatui::backend::TestBackend`); o event loop de terminal
//! (crossterm) vive no `forge-cli`, que traduz os `LoopEvent`s do agente
//! para mutações de [`TuiState`]. O painel do squad chega na Fase 4.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

/// Um item do transcript, com a origem visual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    User(String),
    Assistant(String),
    Tool {
        name: String,
        detail: String,
        ok: bool,
    },
    Notice(String),
}

/// Pedido de permissão aguardando resposta do usuário (modal s/n).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPrompt {
    pub tool: String,
    pub scope: String,
}

/// Estado completo da interface — mutado pelo event loop, lido pelo render.
#[derive(Debug, Default)]
pub struct TuiState {
    pub items: Vec<Item>,
    /// Texto do assistente em streaming (turno corrente, ainda aberto).
    pub streaming: String,
    pub input: String,
    pub status: String,
    pub permission: Option<PermissionPrompt>,
    pub busy: bool,
}

impl TuiState {
    /// Fecha o turno em streaming, movendo-o para o transcript.
    pub fn finish_turn(&mut self) {
        if !self.streaming.is_empty() {
            let text = std::mem::take(&mut self.streaming);
            self.items.push(Item::Assistant(text));
        }
    }
}

/// Desenha a interface: transcript, barra de status e linha de entrada,
/// com o modal de permissão por cima quando pendente.
pub fn render(frame: &mut Frame, state: &TuiState) {
    let [transcript_area, status_area, input_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .areas(frame.area());

    // Transcript (as últimas linhas que couberem).
    let mut lines: Vec<Line> = Vec::new();
    for item in &state.items {
        match item {
            Item::User(text) => lines.push(Line::from(vec![
                Span::styled(
                    "você ▸ ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(text.clone()),
            ])),
            Item::Assistant(text) => {
                for (i, part) in text.lines().enumerate() {
                    let prefix = if i == 0 { "forge ▸ " } else { "        " };
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(Color::Yellow)),
                        Span::raw(part.to_string()),
                    ]));
                }
            }
            Item::Tool { name, detail, ok } => lines.push(Line::from(Span::styled(
                format!("  {} {name}: {detail}", if *ok { "⚒" } else { "✗" }),
                Style::default().fg(if *ok { Color::Green } else { Color::Red }),
            ))),
            Item::Notice(text) => lines.push(Line::from(Span::styled(
                format!("· {text}"),
                Style::default().fg(Color::DarkGray),
            ))),
        }
    }
    if !state.streaming.is_empty() {
        for (i, part) in state.streaming.lines().enumerate() {
            let prefix = if i == 0 { "forge ▸ " } else { "        " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::raw(part.to_string()),
            ]));
        }
    }
    let visible = transcript_area.height.saturating_sub(2) as usize;
    let skip = lines.len().saturating_sub(visible);
    let transcript = Paragraph::new(lines.into_iter().skip(skip).collect::<Vec<_>>())
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(" forge "));
    frame.render_widget(transcript, transcript_area);

    // Status.
    let status = Paragraph::new(Line::from(Span::styled(
        if state.busy {
            format!("⋯ {}", state.status)
        } else {
            state.status.clone()
        },
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(status, status_area);

    // Entrada.
    let input = Paragraph::new(state.input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" mensagem (Enter envia · Esc sai) "),
    );
    frame.render_widget(input, input_area);

    // Modal de permissão.
    if let Some(prompt) = &state.permission {
        let area = centered(frame.area(), 60, 5);
        frame.render_widget(Clear, area);
        let modal = Paragraph::new(vec![
            Line::from(format!("permitir {} em {:?}?", prompt.tool, prompt.scope)),
            Line::from(""),
            Line::from(Span::styled(
                "[s] permitir    [n] negar",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" permissão ")
                .style(Style::default().fg(Color::Magenta)),
        );
        frame.render_widget(modal, area);
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw(state: &TuiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, 16)).unwrap();
        terminal.draw(|f| render(f, state)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn transcript_input_e_status_aparecem() {
        let mut state = TuiState {
            input: "próxima pergunta".into(),
            status: "modelo claude-sonnet-5 · sessão s1".into(),
            ..Default::default()
        };
        state.items.push(Item::User("corrija o teste".into()));
        state.items.push(Item::Tool {
            name: "edit".into(),
            detail: "src/lib.rs".into(),
            ok: true,
        });
        state
            .items
            .push(Item::Assistant("Pronto, teste corrigido.".into()));

        let screen = draw(&state);
        assert!(screen.contains("você ▸ corrija o teste"));
        assert!(screen.contains("⚒ edit: src/lib.rs"));
        assert!(screen.contains("forge ▸ Pronto, teste corrigido."));
        assert!(screen.contains("próxima pergunta"));
        assert!(screen.contains("modelo claude-sonnet-5"));
    }

    #[test]
    fn streaming_aparece_e_finish_turn_move_para_o_transcript() {
        let mut state = TuiState {
            streaming: "Analisando".into(),
            ..Default::default()
        };
        assert!(draw(&state).contains("forge ▸ Analisando"));

        state.streaming.push_str(" o arquivo...");
        state.finish_turn();
        assert!(state.streaming.is_empty());
        assert_eq!(state.items.len(), 1);
        assert!(draw(&state).contains("forge ▸ Analisando o arquivo..."));
    }

    #[test]
    fn modal_de_permissao_cobre_a_tela() {
        let state = TuiState {
            permission: Some(PermissionPrompt {
                tool: "bash".into(),
                scope: "cargo test".into(),
            }),
            ..Default::default()
        };
        let screen = draw(&state);
        assert!(screen.contains("permitir bash em \"cargo test\"?"));
        assert!(screen.contains("[s] permitir"));
    }

    #[test]
    fn transcript_longo_mostra_o_final() {
        let mut state = TuiState::default();
        for i in 0..50 {
            state.items.push(Item::Notice(format!("linha {i}")));
        }
        let screen = draw(&state);
        assert!(screen.contains("linha 49"), "últimas linhas visíveis");
        assert!(!screen.contains("linha 0 "), "linhas antigas saem da tela");
    }
}
