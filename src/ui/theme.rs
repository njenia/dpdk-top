//! Colors and styles for the TUI.

use ratatui::style::{Color, Modifier, Style};

pub fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn selected_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::REVERSED)
}

pub fn warning_style() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn critical_style() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub fn link_up_style() -> Style {
    Style::default().fg(Color::Green)
}

pub fn link_down_style() -> Style {
    Style::default().fg(Color::Red)
}

pub fn footer_style() -> Style {
    Style::default().fg(Color::DarkGray)
}
