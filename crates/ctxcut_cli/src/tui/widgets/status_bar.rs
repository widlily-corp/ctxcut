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
        if area.width == 0 || area.height == 0 || area.y >= buf.area().bottom() {
            return;
        }

        // Fill background
        let bg_style = Style::default().bg(Color::Rgb(20, 24, 30)).fg(Color::White);
        buf.set_style(area, bg_style);

        let max_w = area.width.saturating_sub(2) as usize;
        if max_w == 0 {
            return;
        }

        if self.is_searching {
            let prompt = format!(" / Search: {}_ ", self.search_query);
            let truncated = super::truncate_chars(&prompt, max_w);
            buf.set_string(
                area.x + 1,
                area.y,
                truncated,
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );
        } else if !self.status_message.is_empty() {
            let msg = format!(" ℹ {} ", self.status_message);
            let truncated = super::truncate_chars(&msg, max_w);
            buf.set_string(
                area.x + 1,
                area.y,
                truncated,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        } else {
            let legend = " [j/k] Navigate  [/] Search  [Tab] Pane  [Enter] Slice  [c] Clip  [i] Impact  [t] Trace  [r] Refresh  [q] Quit ";
            let truncated = super::truncate_chars(legend, max_w);
            buf.set_string(
                area.x + 1,
                area.y,
                truncated,
                Style::default().fg(Color::LightCyan),
            );
        }
    }
}
