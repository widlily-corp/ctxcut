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

        -- 8. Server Routes
        CREATE TABLE IF NOT EXISTS routes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            framework TEXT NOT NULL,
            http_method TEXT NOT NULL,
            route_path TEXT NOT NULL,
            handler_symbol TEXT NOT NULL,
            handler_signature TEXT NOT NULL,
            request_dto TEXT,
            response_dto TEXT,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_routes_path ON routes(route_path);
        CREATE INDEX IF NOT EXISTS idx_routes_method_path ON routes(http_method, route_path);
        CREATE INDEX IF NOT EXISTS idx_routes_symbol ON routes(handler_symbol);
        CREATE INDEX IF NOT EXISTS idx_routes_file_id ON routes(file_id);

        -- 9. Client API Call Endpoints
        CREATE TABLE IF NOT EXISTS client_endpoints (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            client_kind TEXT NOT NULL,
            http_method TEXT,
            endpoint_url TEXT,
            rpc_procedure TEXT,
            line_number INTEGER NOT NULL,
            call_snippet TEXT NOT NULL,
            request_dto TEXT,
            response_dto TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_client_endpoints_url ON client_endpoints(endpoint_url);
        CREATE INDEX IF NOT EXISTS idx_client_endpoints_proc ON client_endpoints(rpc_procedure);
        CREATE INDEX IF NOT EXISTS idx_client_endpoints_file_id ON client_endpoints(file_id);

        -- 10. Database and API Schema Entities
        CREATE TABLE IF NOT EXISTS schema_entities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            schema_kind TEXT NOT NULL,
            entity_name TEXT NOT NULL,
            table_name TEXT,
            definition TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_schema_entities_name ON schema_entities(entity_name);
        CREATE INDEX IF NOT EXISTS idx_schema_entities_table ON schema_entities(table_name);
        CREATE INDEX IF NOT EXISTS idx_schema_entities_kind ON schema_entities(schema_kind);
        CREATE INDEX IF NOT EXISTS idx_schema_entities_file_id ON schema_entities(file_id);

        -- 11. BM25 Lexical-Structural Inverted Index
        CREATE TABLE IF NOT EXISTS bm25_terms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            term TEXT NOT NULL UNIQUE,
            doc_freq INTEGER NOT NULL DEFAULT 0,
            idf REAL NOT NULL DEFAULT 0.0
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_bm25_terms_term ON bm25_terms(term);

        CREATE TABLE IF NOT EXISTS bm25_postings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            term_id INTEGER NOT NULL REFERENCES bm25_terms(id) ON DELETE CASCADE,
            symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            field TEXT NOT NULL,
            term_freq INTEGER NOT NULL,
            field_length INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_bm25_postings_term ON bm25_postings(term_id);
        CREATE INDEX IF NOT EXISTS idx_bm25_postings_symbol ON bm25_postings(symbol_id);
        CREATE INDEX IF NOT EXISTS idx_bm25_postings_file ON bm25_postings(file_id);
        CREATE INDEX IF NOT EXISTS idx_bm25_postings_field ON bm25_postings(field);

        CREATE TABLE IF NOT EXISTS bm25_doc_stats (
            symbol_id INTEGER PRIMARY KEY REFERENCES symbols(id) ON DELETE CASCADE,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            total_terms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_bm25_doc_stats_file ON bm25_doc_stats(file_id);

        -- 12. Pre-computed Swarm Clusters
        CREATE TABLE IF NOT EXISTS clusters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            total_agents INTEGER NOT NULL DEFAULT 2,
            cluster_index INTEGER NOT NULL,
            agent_id TEXT NOT NULL,
            cluster_name TEXT NOT NULL,
            primary_language TEXT NOT NULL,
            symbol_count INTEGER NOT NULL,
            token_count INTEGER NOT NULL,
            mock_contracts TEXT,
            raw_tokens INTEGER NOT NULL DEFAULT 0,
            sliced_tokens INTEGER NOT NULL DEFAULT 0,
            raw_lines INTEGER NOT NULL DEFAULT 0,
            sliced_lines INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_clusters_total_agents ON clusters(total_agents);
        CREATE INDEX IF NOT EXISTS idx_clusters_agent ON clusters(agent_id);
        CREATE INDEX IF NOT EXISTS idx_clusters_name ON clusters(cluster_name);

        -- 13. Swarm Cluster Symbol Mappings
        CREATE TABLE IF NOT EXISTS cluster_symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            cluster_id INTEGER NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
            symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
            symbol_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            language TEXT NOT NULL,
            signature TEXT NOT NULL,
            doc_comment TEXT,
            body TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            token_count INTEGER NOT NULL DEFAULT 0,
            line_count INTEGER NOT NULL DEFAULT 0,
            is_seed BOOLEAN NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_cluster_symbols_cluster ON cluster_symbols(cluster_id);
        CREATE INDEX IF NOT EXISTS idx_cluster_symbols_symbol ON cluster_symbols(symbol_id);
        CREATE INDEX IF NOT EXISTS idx_cluster_symbols_name ON cluster_symbols(symbol_name);

        -- 14. Swarm Cluster Boundary Contracts (Stubs & Hoisted Types)
        CREATE TABLE IF NOT EXISTS cluster_boundaries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            cluster_id INTEGER NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            symbol_name TEXT NOT NULL,
            target_agent_id TEXT NOT NULL,
            content TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL DEFAULT 1,
            end_line INTEGER NOT NULL DEFAULT 1,
            language TEXT NOT NULL DEFAULT 'typescript',
            signature TEXT,
            doc_comment TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_cluster_boundaries_cluster ON cluster_boundaries(cluster_id);
        CREATE INDEX IF NOT EXISTS idx_cluster_boundaries_kind ON cluster_boundaries(kind);

        -- 15. Workspace Graph Edges Cache
        CREATE TABLE IF NOT EXISTS graph_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node TEXT NOT NULL,
            to_node TEXT NOT NULL,
            weight REAL NOT NULL,
            kind TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_graph_edges_from ON graph_edges(from_node);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_to ON graph_edges(to_node);
        "#,
    )
    .map_err(|e| CoreError::DatabaseError(format!("Failed to create SQLite schema tables: {e}")))?;

    // Set schema version in index_meta
    conn.execute(
        "INSERT OR REPLACE INTO index_meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![CURRENT_SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| {
        CoreError::DatabaseError(format!("Failed to write schema version to index_meta: {e}"))
    })?;

    Ok(())
}
