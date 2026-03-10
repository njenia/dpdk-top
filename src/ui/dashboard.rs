//! Dashboard view: overview of all ports + queues + mempools.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::port::LinkStatus;
use crate::ui::format::{format_bps, format_int, format_rate};
use crate::ui::theme::*;

use std::sync::Arc;

use crate::model::state::AppState;

pub fn render_dashboard(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let ports = state.ports.read().unwrap();
    let mempools = state.mempools.read().unwrap();

    let selected_id = *state.selected_port_id.read().unwrap();

    // Ports: header + 1 row per port + 2 borders. Always at least 6 rows (room for ~3 ports).
    let ports_height = (ports.len() as u16 + 3).max(6);

    // Queues: 1 row per queue + 1 title row + 2 borders, capped at 30% of viewport.
    let queue_count = ports
        .iter()
        .find(|p| p.id == selected_id)
        .map(|p| p.info.nb_rx_queues.max(p.queue_stats.len() as u16))
        .unwrap_or(0);
    let queues_max = (area.height * 30 / 100).max(4);
    let queues_height = (queue_count + 3).min(queues_max).max(4);

    // Mempools: 1 row per pool + 2 borders.
    let mp_height = (mempools.len().max(1) as u16 + 2).max(3);

    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(ports_height),
            Constraint::Length(queues_height),
            Constraint::Min(mp_height),
        ])
        .split(area);

    // Ports table
    if !ports.is_empty() {
        let mut rows = vec![Line::from(vec![
            Span::raw(" ID  "),
            Span::raw("Name/PCI        "),
            Span::raw("Link    "),
            Span::raw("RX pps     "),
            Span::raw("TX pps     "),
            Span::raw("RX Mbps "),
            Span::raw("TX Mbps "),
        ])];
        for port in ports.iter() {
            let link_str = match port.info.link_status {
                LinkStatus::Up => {
                    if port.info.link_speed_mbps > 0 {
                        format!("{}G UP", port.info.link_speed_mbps / 1000)
                    } else {
                        "UP".to_string()
                    }
                }
                LinkStatus::Down => "DOWN".to_string(),
                LinkStatus::Unknown => "--".to_string(),
            };
            let rx_pps = format_rate(port.rates.rx_pps);
            let tx_pps = format_rate(port.rates.tx_pps);
            let rx_mbps = format_bps(port.rates.rx_bps)
                .replace(" Mbps", "")
                .replace(" Gbps", "");
            let tx_mbps = format_bps(port.rates.tx_bps)
                .replace(" Mbps", "")
                .replace(" Gbps", "");
            let sel = if port.id == selected_id { "▶" } else { " " };
            let link_style = if port.info.link_status == LinkStatus::Up {
                link_up_style()
            } else {
                link_down_style()
            };
            let row_style = if port.id == selected_id {
                selected_style()
            } else {
                Style::default()
            };
            rows.push(Line::from(vec![
                Span::styled(format!("{} {}", sel, port.id), row_style),
                Span::raw(" "),
                Span::styled(
                    format!(
                        "{:16}",
                        port.info.pci.as_str().chars().take(16).collect::<String>()
                    ),
                    row_style,
                ),
                Span::styled(format!("{:8} ", link_str), link_style),
                Span::styled(format!("{:>10} ", rx_pps), row_style),
                Span::styled(format!("{:>10} ", tx_pps), row_style),
                Span::styled(format!("{:>8} ", rx_mbps), row_style),
                Span::styled(format!("{:>8} ", tx_mbps), row_style),
            ]));
        }
        frame.render_widget(
            Paragraph::new(rows)
                .block(
                    Block::default()
                        .title(format!(" Ports ({}) ", ports.len()))
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: false }),
            body_chunks[0],
        );
    }

    // Queue distribution for selected port
    if let Some(port) = ports.iter().find(|p| p.id == selected_id) {
        let mut q_lines = vec![];
        if port.queue_stats.is_empty() {
            q_lines.push(Line::from(" (no queue stats) "));
        } else {
            q_lines.push(Line::from(vec![
                Span::raw("        "),
                Span::styled("RX pps", Style::from(selected_style())),
                Span::raw("                              "),
                Span::styled("TX pps", Style::from(selected_style())),
            ]));
            let max_rx = port
                .queue_stats
                .iter()
                .map(|q| q.rx_pps)
                .fold(1.0_f64, f64::max);
            let max_tx = port
                .queue_stats
                .iter()
                .map(|q| q.tx_pps)
                .fold(1.0_f64, f64::max);
            for (i, q) in port.queue_stats.iter().take(8).enumerate() {
                let rx_bar_len = (q.rx_pps / max_rx * 15.0) as usize;
                let tx_bar_len = (q.tx_pps / max_tx * 15.0) as usize;
                let rx_bar = "█".repeat(rx_bar_len) + &"░".repeat(15 - rx_bar_len);
                let tx_bar = "█".repeat(tx_bar_len) + &"░".repeat(15 - tx_bar_len);
                q_lines.push(Line::from(format!(
                    " Q{:<2} {} {:>10}    {} {:>10}",
                    i,
                    rx_bar,
                    format_rate(q.rx_pps),
                    tx_bar,
                    format_rate(q.tx_pps)
                )));
            }
        }
        let total_queues = port.info.nb_rx_queues.max(port.info.nb_tx_queues);
        let queue_title = if total_queues > 0 {
            format!(" Queues ({}) ", total_queues)
        } else {
            " Queues ".to_string()
        };
        frame.render_widget(
            Paragraph::new(q_lines)
                .block(Block::default().title(queue_title).borders(Borders::ALL)),
            body_chunks[1],
        );
    } else if !ports.is_empty() {
        frame.render_widget(
            Paragraph::new(" Select a port to see queue distribution ")
                .block(Block::default().title(" Queues ").borders(Borders::ALL)),
            body_chunks[1],
        );
    }

    // Mempools summary
    if !mempools.is_empty() {
        let mut mp_lines = vec![];
        for mp in mempools.iter() {
            mp_lines.push(Line::from(format!(
                " {}   {} / {}  ({:.1}%)",
                mp.name,
                format_int(mp.in_use),
                format_int(mp.size),
                mp.utilization_pct
            )));
        }
        frame.render_widget(
            Paragraph::new(mp_lines).block(
                Block::default()
                    .title(format!(" Mempools ({}) ", mempools.len()))
                    .borders(Borders::ALL),
            ),
            body_chunks[2],
        );
    } else {
        frame.render_widget(
            Paragraph::new(" No mempools ")
                .block(Block::default().title(" Mempools ").borders(Borders::ALL)),
            body_chunks[2],
        );
    }
}
