//! Main TUI app loop and event handling.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::model::state::AppState;
use crate::ui::charts;
use crate::ui::dashboard::render_dashboard;
use crate::ui::views;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Dashboard,
    Port,
    Mempools,
    Xstats,
    Graphs,
}

pub fn run_tui(
    instances: Vec<Arc<AppState>>,
    shutdown: Arc<AtomicBool>,
    _no_color: bool,
) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut current_view = View::Dashboard;
    let mut show_help = false;
    let mut xstats_scroll: u16 = 0;
    let mut instance_idx: usize = 0;

    loop {
        let state = &instances[instance_idx];
        let n_instances = instances.len();
        let view = current_view;

        terminal.draw(|f| {
            let area = f.area();
            let body_area = layout_with_footer(f, state, area, view, instance_idx, n_instances);
            match view {
                View::Dashboard => render_dashboard(f, state, body_area),
                View::Port => views::render_port_detail(f, state, body_area),
                View::Mempools => views::render_mempools(f, state, body_area),
                View::Xstats => views::render_xstats(f, state, body_area, xstats_scroll),
                View::Graphs => charts::render_charts(f, state, body_area),
            }
            if show_help {
                views::render_help(f, area);
            }
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if show_help {
                    show_help = false;
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }

                    // Instance switching: [ / ] or number keys 1–9
                    KeyCode::Char('[') => {
                        if instance_idx > 0 {
                            instance_idx -= 1;
                            xstats_scroll = 0;
                        }
                    }
                    KeyCode::Char(']') => {
                        if instance_idx + 1 < instances.len() {
                            instance_idx += 1;
                            xstats_scroll = 0;
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                        let idx = (c as usize) - ('1' as usize);
                        if idx < instances.len() {
                            instance_idx = idx;
                            xstats_scroll = 0;
                        }
                    }

                    KeyCode::Char('d') => {
                        current_view = View::Dashboard;
                    }
                    KeyCode::Char('p') => {
                        current_view = View::Port;
                    }
                    KeyCode::Char('m') => {
                        current_view = View::Mempools;
                    }
                    KeyCode::Char('x') => {
                        current_view = View::Xstats;
                        xstats_scroll = 0;
                    }
                    KeyCode::Char('g') => {
                        current_view = View::Graphs;
                    }
                    KeyCode::Char('?') => {
                        show_help = !show_help;
                    }

                    KeyCode::Up | KeyCode::Char('k') => {
                        if current_view == View::Xstats {
                            xstats_scroll = xstats_scroll.saturating_sub(1);
                        } else {
                            move_port_selection(state, -1);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if current_view == View::Xstats {
                            xstats_scroll = xstats_scroll.saturating_add(1);
                        } else {
                            move_port_selection(state, 1);
                        }
                    }
                    KeyCode::PageUp => {
                        if current_view == View::Xstats {
                            xstats_scroll = xstats_scroll.saturating_sub(20);
                        }
                    }
                    KeyCode::PageDown => {
                        if current_view == View::Xstats {
                            xstats_scroll = xstats_scroll.saturating_add(20);
                        }
                    }

                    KeyCode::Tab => {
                        current_view = match current_view {
                            View::Dashboard => View::Port,
                            View::Port => View::Graphs,
                            View::Graphs => View::Mempools,
                            View::Mempools => View::Xstats,
                            View::Xstats => View::Dashboard,
                        };
                        xstats_scroll = 0;
                    }
                    KeyCode::BackTab => {
                        current_view = match current_view {
                            View::Dashboard => View::Xstats,
                            View::Xstats => View::Mempools,
                            View::Mempools => View::Graphs,
                            View::Graphs => View::Port,
                            View::Port => View::Dashboard,
                        };
                        xstats_scroll = 0;
                    }

                    _ => {}
                }
            }
        }
    }

    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    crossterm::terminal::disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

fn move_port_selection(state: &Arc<AppState>, direction: i32) {
    let ids: Vec<u16> = state.ports.read().unwrap().iter().map(|p| p.id).collect();
    if ids.is_empty() {
        return;
    }
    let current = *state.selected_port_id.read().unwrap();
    let pos = ids.iter().position(|&id| id == current).unwrap_or(0);
    let new_pos = if direction < 0 {
        pos.saturating_sub(1)
    } else {
        (pos + 1).min(ids.len().saturating_sub(1))
    };
    *state.selected_port_id.write().unwrap() = ids[new_pos];
}

/// Render header + footer, return the body area in between.
fn layout_with_footer(
    frame: &mut ratatui::Frame,
    state: &Arc<AppState>,
    area: ratatui::layout::Rect,
    current_view: View,
    instance_idx: usize,
    n_instances: usize,
) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph};

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .split(area);

    let connected = *state.connected.read().unwrap();

    // Build the instance indicator (e.g. "[2/3]") when multiple instances exist
    let instance_label = if n_instances > 1 {
        format!("  [{}/{}]  ", instance_idx + 1, n_instances)
    } else {
        String::new()
    };

    let socket_name = state
        .socket_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let header = format!(
        " dpdk-top v0.1.0{}  {}  poll: {}s   {} ",
        instance_label,
        if socket_name.is_empty() {
            state.socket_path.display().to_string()
        } else {
            socket_name
        },
        state.poll_interval_secs,
        if connected {
            "● connected"
        } else {
            "○ disconnected"
        }
    );
    frame.render_widget(
        Paragraph::new(header)
            .block(Block::default().borders(Borders::BOTTOM))
            .style(crate::ui::theme::header_style()),
        chunks[0],
    );

    let active = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::REVERSED);
    let inactive = Style::default().fg(Color::DarkGray);

    let tab = |label: &str, key: char, view: View| -> Span {
        let style = if current_view == view {
            active
        } else {
            inactive
        };
        Span::styled(format!(" [{}]{} ", key, label), style)
    };

    let mut footer_spans = vec![
        tab("dashboard", 'd', View::Dashboard),
        Span::raw(" "),
        tab("port", 'p', View::Port),
        Span::raw(" "),
        tab("graphs", 'g', View::Graphs),
        Span::raw(" "),
        tab("mempools", 'm', View::Mempools),
        Span::raw(" "),
        tab("xstats", 'x', View::Xstats),
        Span::raw("    "),
        Span::styled(" [?]help ", inactive),
        Span::raw("  "),
        Span::styled(" q:quit ", inactive),
    ];

    // Show instance navigation hint when multiple instances are present
    if n_instances > 1 {
        footer_spans.push(Span::raw("  "));
        // Show [ and ] as distinct bracketed keys
        footer_spans.push(Span::styled(
            " instance: ",
            Style::default().fg(Color::Yellow),
        ));
        footer_spans.push(Span::styled(
            "[ ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        ));
        footer_spans.push(Span::styled(
            format!(" {}/{} ", instance_idx + 1, n_instances),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        footer_spans.push(Span::styled(
            " ]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(footer_spans)).block(Block::default().borders(Borders::TOP)),
        chunks[2],
    );

    chunks[1]
}

pub fn run_watch_mode(_socket_path: &std::path::Path, _interval: f64, _xstat: &str) -> Result<()> {
    anyhow::bail!("Watch mode not yet implemented; use --json or TUI")
}
