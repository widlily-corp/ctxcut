//! Status bar widget with keybinding legend and status breadcrumbs.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

/// Application status and keybinding bar widget.
pub struct StatusBar<'a> {
    status_message: &'a str,
    is_searching: bool,
    search_query: &'a str,
}

impl<'a> StatusBar<'a> {
    /// Creates a new status bar widget.
    pub fn new(status_message: &'a str, is_searching: bool, search_query: &'a str) -> Self {
        Self {
            status_message,
            is_searching,
            search_query,
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Fill background
        let bg_style = Style::default().bg(Color::Rgb(20, 24, 30)).fg(Color::White);
        buf.set_style(area, bg_style);

        if self.is_searching {
            let prompt = format!(" / Search: {}_ ", self.search_query);
            buf.set_string(
                area.x + 1,
                area.y,
                &prompt,
                Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD),
            );
        } else if !self.status_message.is_empty() {
            let msg = format!(" ℹ {} ", self.status_message);
            buf.set_string(
                area.x + 1,
                area.y,
                &msg,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
        } else {
            let legend = " [j/k] Navigate  [/] Search  [Tab] Pane  [Enter] Slice  [c] Clip  [i] Impact  [r] Refresh  [q] Quit ";
            buf.set_string(
                area.x + 1,
                area.y,
                legend,
                Style::default().fg(Color::LightCyan),
            );
        }
    }
}
