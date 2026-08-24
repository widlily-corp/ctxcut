//! TUI view components.

pub mod header;
pub mod impact;
pub mod navigator;
pub mod preview;
pub mod telemetry;

pub use header::render_header;
pub use impact::render_impact;
pub use navigator::render_navigator;
pub use preview::render_preview;
pub use telemetry::render_telemetry;

/// Truncates a string to at most `max_chars` Unicode characters safely at a char boundary.
#[must_use]
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    s.char_indices()
        .nth(max_chars)
        .map(|(idx, _)| &s[..idx])
        .unwrap_or(s)
}
