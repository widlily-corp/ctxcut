//! Persistent SQLite Indexing Engine (`.ctxcut/index.db`).
//!
//! Provides sub-5ms repository queries, two-tier incremental change detection,
//! WAL concurrency, automated schema migrations, and crash recovery.

pub mod query;
pub mod schema;
pub mod sqlite;

pub use schema::CURRENT_SCHEMA_VERSION;
pub use sqlite::{IndexEngine, IndexOptions, IndexStatus, IndexSyncResult};
