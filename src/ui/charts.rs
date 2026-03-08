//! Charts view: rolling sparkline charts for key metrics.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use std::sync::Arc;

use crate::model::state::AppState;
use crate::ui::format::format_rate;
use crate::ui::widgets::sparkline::Sparkline;

/// How many data points to show (newest on the right).
const CHART_WINDOW: usize = 120;

/// Render rolling sparkline charts for the selected port.
pub fn render_charts(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let ports = state.ports.read().unwrap();
    let selected_id = *state.selected_port_id.read().unwrap();
    let port_history = state.port_history.read().unwrap();
    let mempool_history = state.mempool_history.read().unwrap();
    let mempools = state.mempools.read().unwrap();

    let port_idx = ports.iter().position(|p| p.id == selected_id);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    if let Some(pi) = port_idx {
        if pi < port_history.len() {
            let history = &port_history[pi];
            let len = history.len();

            let rx_pps: Vec<f64> = history.iter().map(|r| r.rx_pps).collect();
            let tx_pps: Vec<f64> = history.iter().map(|r| r.tx_pps).collect();
            let rx_mbps: Vec<f64> = history.iter().map(|r| r.rx_bps / 1_000_000.0).collect();
            let tx_mbps: Vec<f64> = history.iter().map(|r| r.tx_bps / 1_000_000.0).collect();
            let errors: Vec<f64> = history
                .iter()
                .map(|r| r.rx_missed_pps + r.rx_nombuf_pps + r.ierrors_pps + r.oerrors_pps)
                .collect();

            let window = CHART_WINDOW.min(len);

            render_dual_chart(
                frame,
                chunks[0],
                &format!(" Port {} — Packets/sec ({} samples) ", selected_id, len),
                "RX pps",
                &tail(&rx_pps, window),
                Color::Green,
                "TX pps",
                &tail(&tx_pps, window),
                Color::Blue,
            );

            render_dual_chart(
                frame,
                chunks[1],
                &format!(" Port {} — Throughput (Mbps) ", selected_id),
                "RX Mbps",
                &tail(&rx_mbps, window),
                Color::Cyan,
                "TX Mbps",
                &tail(&tx_mbps, window),
                Color::Magenta,
            );

            let error_window = tail(&errors, window);
            render_single_chart(
                frame,
                chunks[2],
                &format!(" Port {} — Errors/sec ", selected_id),
                "Errors",
                &error_window,
                Color::Red,
            );
        } else {
            render_empty(frame, chunks[0], " Packets/sec ", "Waiting for data...");
            render_empty(frame, chunks[1], " Throughput ", "Waiting for data...");
            render_empty(frame, chunks[2], " Errors ", "Waiting for data...");
        }
    } else {
        render_empty(frame, chunks[0], " Packets/sec ", "No port selected");
        render_empty(frame, chunks[1], " Throughput ", "No port selected");
        render_empty(frame, chunks[2], " Errors ", "No port selected");
    }

    // Mempool utilization chart
    if !mempool_history.is_empty() && !mempools.is_empty() {
        let mp_data: Vec<f64> = mempool_history[0].iter().copied().collect();
        let window = tail(&mp_data, CHART_WINDOW);
        render_single_chart(
            frame,
            chunks[3],
            &format!(" Mempool — {} utilization % ", mempools[0].name),
            "Util %",
            &window,
            Color::Yellow,
        );
    } else {
        render_empty(frame, chunks[3], " Mempool ", "Waiting for data...");
    }
}

fn tail(data: &[f64], n: usize) -> Vec<f64> {
    let start = data.len().saturating_sub(n);
    data[start..].to_vec()
}

#[allow(clippy::too_many_arguments)]
fn render_dual_chart(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    label1: &str,
    data1: &[f64],
    color1: Color,
    label2: &str,
    data2: &[f64],
    color2: Color,
) {
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    render_chart_cell(frame, halves[0], label1, data1, color1);
    render_chart_cell(frame, halves[1], label2, data2, color2);
}

fn render_single_chart(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    label: &str,
    data: &[f64],
    color: Color,
) {
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    render_chart_cell(frame, inner, label, data, color);
}

fn render_chart_cell(frame: &mut Frame, area: Rect, label: &str, data: &[f64], color: Color) {
    if area.height < 2 {
        return;
    }

    let current = data.last().copied().unwrap_or(0.0);
    let max_val = data.iter().cloned().fold(0.0_f64, f64::max);
    let current_str = format_rate(current);
    let max_str = format_rate(max_val);

    let label_area = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", label),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(current_str.to_string(), Style::default().fg(Color::White)),
            Span::styled(
                format!("  peak {}", max_str),
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        label_area,
    );

    let chart_area = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(1),
    );
    if chart_area.height > 0 && chart_area.width > 0 {
        let sparkline = Sparkline::new(data)
            .style(Style::default().fg(color))
            .max(max_val);
        frame.render_widget(sparkline, chart_area);
    }
}

fn render_empty(frame: &mut Frame, area: Rect, title: &str, msg: &str) {
    frame.render_widget(
        Paragraph::new(format!("  {}", msg))
            .block(Block::default().title(title).borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
