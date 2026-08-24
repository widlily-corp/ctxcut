//! Bordered KPI card widget for telemetry dashboard.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

/// Bordered KPI metric card.
pub struct KpiCard<'a> {
    title: &'a str,
    value: &'a str,
    subtitle: &'a str,
    accent_color: Color,
}

impl<'a> KpiCard<'a> {
    /// Creates a new KPI card widget.
    pub fn new(title: &'a str, value: &'a str, subtitle: &'a str, accent_color: Color) -> Self {
        Self {
            title,
            value,
            subtitle,
            accent_color,
        }
    }
}

impl Widget for KpiCard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(format!(" {} ", self.title))
            .title_style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        let inner_area = block.inner(area);
        block.render(area, buf);

        if inner_area.width < 2 || inner_area.height == 0 {
            return;
        }

        if inner_area.height >= 1 && inner_area.y < buf.area().bottom() {
            let max_w = inner_area.width.saturating_sub(2) as usize;
            let val_str = super::truncate_chars(self.value, max_w);
            buf.set_string(
                inner_area.x + 1,
                inner_area.y,
                val_str,
                Style::default()
                    .fg(self.accent_color)
                    .add_modifier(Modifier::BOLD),
            );
        }

        if inner_area.height >= 2 && (inner_area.y + 1) < buf.area().bottom() {
            let max_w = inner_area.width.saturating_sub(2) as usize;
            let sub_str = super::truncate_chars(self.subtitle, max_w);
            buf.set_string(
                inner_area.x + 1,
                inner_area.y + 1,
                sub_str,
                Style::default().fg(Color::Gray),
            );
        }
    }
}
