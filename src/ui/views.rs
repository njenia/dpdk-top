//! Individual view renderers for each TUI tab.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use std::sync::Arc;

use crate::model::state::AppState;
use crate::ui::format::{format_bps, format_int, format_rate};

/// Port detail view: full info + counters + rates for the selected port.
pub fn render_port_detail(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let ports = state.ports.read().unwrap();
    let selected_id = *state.selected_port_id.read().unwrap();

    let Some(port) = ports.iter().find(|p| p.id == selected_id) else {
        frame.render_widget(
            Paragraph::new(" No port selected. Press ↑/↓ to select a port.").block(
                Block::default()
                    .title(" Port Detail ")
                    .borders(Borders::ALL),
            ),
            area,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Min(4),
        ])
        .split(area);

    // Port info
    let info = &port.info;
    let link_str = match info.link_status {
        crate::model::port::LinkStatus::Up => {
            if info.link_speed_mbps > 0 {
                format!("{}G UP", info.link_speed_mbps / 1000)
            } else {
                "UP".into()
            }
        }
        crate::model::port::LinkStatus::Down => "DOWN".into(),
        crate::model::port::LinkStatus::Unknown => "Unknown".into(),
    };
    let info_lines = vec![
        Line::from(format!("  Name:       {}", info.pci)),
        Line::from(format!(
            "  Driver:     {}",
            if info.driver.is_empty() {
                "—"
            } else {
                &info.driver
            }
        )),
        Line::from(format!(
            "  MAC:        {}",
            if info.mac.is_empty() {
                "—"
            } else {
                &info.mac
            }
        )),
        Line::from(format!(
            "  MTU:        {}",
            if info.mtu == 0 {
                "—".to_string()
            } else {
                info.mtu.to_string()
            }
        )),
        Line::from(format!("  Link:       {}", link_str)),
        Line::from(format!("  RX Queues:  {}", info.nb_rx_queues)),
        Line::from(format!("  TX Queues:  {}", info.nb_tx_queues)),
    ];
    frame.render_widget(
        Paragraph::new(info_lines).block(
            Block::default()
                .title(format!(" Port {} — Info ", selected_id))
                .borders(Borders::ALL),
        ),
        chunks[0],
    );

    // Counters + rates
    let s = &port.stats_current;
    let r = &port.rates;
    let counter_lines = vec![
        Line::from(vec![
            Span::raw("              "),
            Span::styled(
                format!("{:>14}", "Packets"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                format!("{:>14}", "Bytes"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                format!("{:>12}", "pps"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                format!("{:>10}", "Mbps"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(format!(
            "  RX          {:>14}    {:>14}    {:>12}    {:>10}",
            format_int(s.ipackets),
            format_int(s.ibytes),
            format_rate(r.rx_pps),
            format_bps(r.rx_bps)
        )),
        Line::from(format!(
            "  TX          {:>14}    {:>14}    {:>12}    {:>10}",
            format_int(s.opackets),
            format_int(s.obytes),
            format_rate(r.tx_pps),
            format_bps(r.tx_bps)
        )),
        Line::from(""),
        Line::from(format!(
            "  RX missed   {:>14}    {:>12}/s",
            format_int(s.imissed),
            format_rate(r.rx_missed_pps)
        )),
        Line::from(format!(
            "  RX errors   {:>14}    {:>12}/s",
            format_int(s.ierrors),
            format_rate(r.ierrors_pps)
        )),
        Line::from(format!(
            "  TX errors   {:>14}    {:>12}/s",
            format_int(s.oerrors),
            format_rate(r.oerrors_pps)
        )),
        Line::from(format!(
            "  RX no-mbuf  {:>14}    {:>12}/s",
            format_int(s.rx_nombuf),
            format_rate(r.rx_nombuf_pps)
        )),
    ];
    frame.render_widget(
        Paragraph::new(counter_lines).block(
            Block::default()
                .title(" Counters & Rates ")
                .borders(Borders::ALL),
        ),
        chunks[1],
    );

    // Queue distribution
    let mut q_lines = vec![];
    if port.queue_stats.is_empty() {
        q_lines.push(Line::from("  (no per-queue stats available)"));
    } else {
        q_lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("RX pps", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("                              "),
            Span::styled("TX pps", Style::default().add_modifier(Modifier::BOLD)),
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
        for (i, q) in port.queue_stats.iter().enumerate() {
            let rx_bar_len = (q.rx_pps / max_rx * 15.0) as usize;
            let tx_bar_len = (q.tx_pps / max_tx * 15.0) as usize;
            let rx_bar = "█".repeat(rx_bar_len) + &"░".repeat(15 - rx_bar_len);
            let tx_bar = "█".repeat(tx_bar_len) + &"░".repeat(15 - tx_bar_len);
            q_lines.push(Line::from(format!(
                "  Q{:<2} {} {:>10}    {} {:>10}",
                i,
                rx_bar,
                format_rate(q.rx_pps),
                tx_bar,
                format_rate(q.tx_pps)
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(q_lines).block(
            Block::default()
                .title(" Queue Distribution ")
                .borders(Borders::ALL),
        ),
        chunks[2],
    );
}

/// Mempools view: expanded mempool info with utilization bars.
pub fn render_mempools(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let mempools = state.mempools.read().unwrap();

    if mempools.is_empty() {
        frame.render_widget(
            Paragraph::new("  No mempools discovered.")
                .block(Block::default().title(" Mempools ").borders(Borders::ALL)),
            area,
        );
        return;
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        format!(
            "  {:<24} {:>12} {:>12} {:>12} {:>8} {:>6}  {:>22}",
            "Name", "Size", "In Use", "Free", "Util %", "Elem", "Utilization"
        ),
        Style::default().add_modifier(Modifier::BOLD),
    )])];

    for mp in mempools.iter() {
        let bar_width = 20;
        let filled = ((mp.utilization_pct / 100.0) * bar_width as f64) as usize;
        let bar = "█".repeat(filled) + &"░".repeat(bar_width - filled);

        let util_color = if mp.utilization_pct > 90.0 {
            Color::Red
        } else if mp.utilization_pct > 70.0 {
            Color::Yellow
        } else {
            Color::Green
        };

        lines.push(Line::from(vec![
            Span::raw(format!(
                "  {:<24} {:>12} {:>12} {:>12} ",
                mp.name,
                format_int(mp.size),
                format_int(mp.in_use),
                format_int(mp.free_count),
            )),
            Span::styled(
                format!("{:>7.1}%", mp.utilization_pct),
                Style::default().fg(util_color),
            ),
            Span::raw(format!(" {:>6}", mp.element_size)),
            Span::raw("  "),
            Span::styled(bar, Style::default().fg(util_color)),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" Mempools ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// Xstats view: extended stats table for the selected port, with scrolling.
pub fn render_xstats(frame: &mut Frame, state: &Arc<AppState>, area: Rect, scroll_offset: u16) {
    let ports = state.ports.read().unwrap();
    let selected_id = *state.selected_port_id.read().unwrap();

    let Some(port) = ports.iter().find(|p| p.id == selected_id) else {
        frame.render_widget(
            Paragraph::new(" No port selected.")
                .block(Block::default().title(" Xstats ").borders(Borders::ALL)),
            area,
        );
        return;
    };

    if port.xstats.is_empty() {
        frame.render_widget(
            Paragraph::new("  No extended stats available for this port.").block(
                Block::default()
                    .title(format!(" Port {} — Xstats ", selected_id))
                    .borders(Borders::ALL),
            ),
            area,
        );
        return;
    }

    let mut sorted_xstats: Vec<_> = port.xstats.iter().collect();
    sorted_xstats.sort_by_key(|(name, _)| (*name).clone());

    let mut lines = vec![Line::from(vec![Span::styled(
        format!("  {:<44} {:>18}", "Counter", "Value"),
        Style::default().add_modifier(Modifier::BOLD),
    )])];

    for (name, (value, _rate)) in &sorted_xstats {
        let style = if *value > 0 {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  {:<44} {:>18}", name, format_int(*value)),
            style,
        )]));
    }

    let title = format!(
        " Port {} — Xstats ({} counters)  ↑↓ scroll ",
        selected_id,
        sorted_xstats.len()
    );
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(title).borders(Borders::ALL))
            .scroll((scroll_offset, 0)),
        area,
    );
}

/// Help overlay.
pub fn render_help(frame: &mut Frame, area: Rect) {
    let help_width = 56u16;
    let help_height = 24u16;
    let x = area.x + area.width.saturating_sub(help_width) / 2;
    let y = area.y + area.height.saturating_sub(help_height) / 2;
    let help_area = Rect::new(
        x,
        y,
        help_width.min(area.width),
        help_height.min(area.height),
    );

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  dpdk-top",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" — DPDK telemetry monitor"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Views",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )]),
        Line::from(""),
        Line::from("  d          Dashboard (overview)"),
        Line::from("  p          Port detail"),
        Line::from("  g          Graphs (rolling charts)"),
        Line::from("  m          Mempools"),
        Line::from("  x          Xstats (extended counters)"),
        Line::from("  Tab        Cycle through views"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Controls",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )]),
        Line::from(""),
        Line::from("  ↑ / k      Select previous port"),
        Line::from("  ↓ / j      Select next port"),
        Line::from("  [ / ]      Switch DPDK instance"),
        Line::from("  1-9        Jump to DPDK instance N"),
        Line::from("  q / Esc    Quit"),
        Line::from("  ?          Toggle this help"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().bg(Color::Black)),
        help_area,
    );
}
