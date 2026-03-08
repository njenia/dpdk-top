//! Sparkline chart widget using Unicode block characters.
//! Renders a rolling time-series in a given Rect, with optional Y-axis label.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct Sparkline<'a> {
    data: &'a [f64],
    style: Style,
    max: Option<f64>,
    label: Option<&'a str>,
    baseline_style: Style,
}

impl<'a> Sparkline<'a> {
    pub fn new(data: &'a [f64]) -> Self {
        Self {
            data,
            style: Style::default().fg(Color::Cyan),
            max: None,
            label: None,
            baseline_style: Style::default().fg(Color::DarkGray),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    #[allow(dead_code)]
    pub fn baseline_style(mut self, style: Style) -> Self {
        self.baseline_style = style;
        self
    }
}

impl Widget for Sparkline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height == 0 || self.data.is_empty() {
            return;
        }

        let label_width: u16 = if let Some(label) = self.label {
            let lw = label.len() as u16 + 1;
            if lw < area.width.saturating_sub(4) {
                buf.set_string(area.x, area.y, label, self.baseline_style);
                lw
            } else {
                0
            }
        } else {
            0
        };

        let chart_x = area.x + label_width;
        let chart_w = (area.width - label_width) as usize;
        let chart_h = area.height as usize;

        if chart_w == 0 || chart_h == 0 {
            return;
        }

        let max_val = self
            .max
            .filter(|&m| m > 0.0)
            .unwrap_or_else(|| self.data.iter().cloned().fold(0.0_f64, f64::max));
        let max_val = if max_val > 0.0 { max_val } else { 1.0 };

        let data_len = self.data.len();
        let visible = data_len.min(chart_w);
        let start = data_len.saturating_sub(visible);

        let levels = chart_h * 8;

        for (i, &v) in self.data[start..].iter().enumerate().take(visible) {
            let normalized = (v / max_val).clamp(0.0, 1.0);
            let bar_units = (normalized * levels as f64) as usize;

            let full_rows = bar_units / 8;
            let frac = bar_units % 8;

            for row in 0..chart_h {
                let y = area.y + (chart_h - 1 - row) as u16;
                let x = chart_x + i as u16;
                if x >= area.x + area.width || y >= area.y + area.height {
                    continue;
                }
                let ch = if row < full_rows {
                    BLOCKS[8]
                } else if row == full_rows && frac > 0 {
                    BLOCKS[frac]
                } else {
                    continue;
                };
                buf.set_string(x, y, ch.to_string(), self.style);
            }
        }

        // Draw baseline dots for empty columns
        let bottom_y = area.y + area.height - 1;
        for i in 0..chart_w {
            let x = chart_x + i as u16;
            if x < area.x + area.width {
                let cell = buf.cell((x, bottom_y));
                if let Some(cell) = cell {
                    if cell.symbol() == " " {
                        buf.set_string(x, bottom_y, "·", self.baseline_style);
                    }
                }
            }
        }
    }
}
