//! Utilization bar (e.g. mempool %, queue distribution).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

pub struct Bar {
    ratio: f64,
    width: u16,
    style_fill: Style,
    style_empty: Style,
}

impl Bar {
    pub fn new(ratio: f64, width: u16) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
            width,
            style_fill: Style::default().fg(Color::Green),
            style_empty: Style::default().fg(Color::DarkGray),
        }
    }

    pub fn style_fill(mut self, s: Style) -> Self {
        self.style_fill = s;
        self
    }

    pub fn style_empty(mut self, s: Style) -> Self {
        self.style_empty = s;
        self
    }
}

impl Widget for Bar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let fill_len = (self.ratio * self.width as f64).round() as u16;
        let fill_len = fill_len.min(area.width).min(self.width);
        for x in area.x..(area.x + fill_len) {
            if x < area.x + area.width {
                buf.set_string(x, area.y, "█", self.style_fill);
            }
        }
        for x in (area.x + fill_len)..(area.x + area.width.min(self.width)) {
            buf.set_string(x, area.y, "░", self.style_empty);
        }
    }
}
