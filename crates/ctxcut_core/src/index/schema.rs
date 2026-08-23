//! SQLite database schema, PRAGMA configuration, and table migrations for persistent indexing.

use crate::error::{CoreError, Result};
use rusqlite::Connection;

/// Current schema migration version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Sets recommended high-throughput and concurrency PRAGMA configurations on a SQLite connection.
pub fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA cache_size = -8000;
        PRAGMA temp_store = MEMORY;
        "#,
    )
    .map_err(|e| CoreError::DatabaseError(format!("Failed to configure SQLite pragmas: {e}")))?;
    Ok(())
}

/// Applies all schema creation and migration DDL statements.
pub fn apply_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- 1. Index Metadata
        CREATE TABLE IF NOT EXISTS index_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- 2. Indexed Files
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            language TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            mtime_secs INTEGER NOT NULL,
            mtime_nanos INTEGER NOT NULL,
            sha256_hash TEXT NOT NULL,
            total_lines INTEGER NOT NULL,
            total_tokens INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
        CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);

        -- 3. Extracted Symbols
        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            container_name TEXT,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            signature TEXT NOT NULL,
            doc_comment TEXT,
            body TEXT NOT NULL,
            is_exported BOOLEAN NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_file_id ON symbols(file_id);
        CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
        CREATE INDEX IF NOT EXISTS idx_symbols_container ON symbols(container_name);
        CREATE INDEX IF NOT EXISTS idx_symbols_name_file ON symbols(name, file_id);

        -- 4. Reverse Call Sites (Impact Analysis)
        CREATE TABLE IF NOT EXISTS callers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target_symbol_name TEXT NOT NULL,
            target_container TEXT,
            caller_file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            caller_symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
            caller_symbol_name TEXT NOT NULL,
            caller_kind TEXT NOT NULL,
            call_line INTEGER NOT NULL,
            call_snippet TEXT NOT NULL,
            caller_signature TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_callers_target ON callers(target_symbol_name);
        CREATE INDEX IF NOT EXISTS idx_callers_caller_sym ON callers(caller_symbol_id);
        CREATE INDEX IF NOT EXISTS idx_callers_target_file ON callers(target_symbol_name, caller_file_id);

        -- 5. Concrete Interface/Trait Implementors
        CREATE TABLE IF NOT EXISTS implementors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            interface_name TEXT NOT NULL,
            implementor_name TEXT NOT NULL,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            definition TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_implementors_iface ON implementors(interface_name);
        CREATE INDEX IF NOT EXISTS idx_implementors_impl ON implementors(implementor_name);
        CREATE INDEX IF NOT EXISTS idx_implementors_file ON implementors(file_id);

        -- 6. File & Symbol Dependencies
        CREATE TABLE IF NOT EXISTS dependencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            target_file_path TEXT NOT NULL,
            imported_symbol TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_deps_source ON dependencies(source_file_id);
        CREATE INDEX IF NOT EXISTS idx_deps_target ON dependencies(target_file_path);

        -- 7. Symbol References
        CREATE TABLE IF NOT EXISTS symbol_references (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            symbol_name TEXT NOT NULL,
            line INTEGER NOT NULL,
            snippet TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_refs_sym ON symbol_references(symbol_name);
        CREATE INDEX IF NOT EXISTS idx_refs_source ON symbol_references(source_file_id);
        "#,
    )
    .map_err(|e| CoreError::DatabaseError(format!("Failed to create SQLite schema tables: {e}")))?;

    // Set schema version in index_meta
    conn.execute(
        "INSERT OR REPLACE INTO index_meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![CURRENT_SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| CoreError::DatabaseError(format!("Failed to write schema version to index_meta: {e}")))?;

    Ok(())
}
