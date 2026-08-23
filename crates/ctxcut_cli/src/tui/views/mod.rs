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
