//! Reusable TUI widgets.

pub mod kpi_card;
pub mod status_bar;

pub use kpi_card::KpiCard;
pub use status_bar::StatusBar;

/// Truncates a string to at most `max_chars` Unicode characters safely at a char boundary.
#[must_use]
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    s.char_indices()
        .nth(max_chars)
        .map(|(idx, _)| &s[..idx])
        .unwrap_or(s)
}
