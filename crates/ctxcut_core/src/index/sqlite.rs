//! SQLite storage backend, index lifecycle, and incremental synchronization.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::collapsible_if
)]

use super::schema::{apply_pragmas, apply_schema, CURRENT_SCHEMA_VERSION};
use crate::error::{CoreError, Result};
use crate::framework::extract_server_routes;
use crate::fullstack::client_detect::ClientDetector;
use crate::fullstack::model::{ClientApiCall, ServerRouteEndpoint};
use crate::lang::LanguageRegistry;
use crate::model::{
    CallSignatureStub, ExtractedSymbol, ExtractedType, OverviewOptions, SupportedLanguage,
    TokenStats,
};
use crate::overview::extract_symbols_from_file;
use crate::parser::{AstUtils, ParserManager};
use crate::schema::extract_schema_entities;
use crate::swarm::{
    derive_cluster_name, BoundaryStubGenerator, CommunityClusterer, MockContractGenerator,
    SwarmAgentPack, SwarmBudgetEngine, SwarmPartitionManifest, WorkspaceGraphBuilder,
};
use crate::tokenizer::{count_lines, count_tokens};
use crate::traversal::{ProjectWalker, TraversalConfig};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tree_sitter::Node;

/// Configuration options for index synchronization.
#[derive(Debug, Clone, Default)]
pub struct IndexOptions {
    /// Force full rebuild from scratch, wiping existing database records.
    pub rebuild: bool,
    /// Maximum directory traversal depth.
    pub max_depth: Option<usize>,
}

/// Synchronization result summary metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSyncResult {
    /// Count of newly discovered and indexed source files.
    pub files_added: usize,
    /// Count of modified source files updated in database.
    pub files_updated: usize,
    /// Count of deleted source files pruned from database.
    pub files_deleted: usize,
    /// Count of unchanged files bypassed via fast tier-1/tier-2 cache.
    pub files_unchanged: usize,
    /// Total symbols now present in database index.
    pub total_symbols: usize,
    /// Time taken to synchronize in milliseconds.
    pub duration_ms: u64,
}

/// SQLite index status and health statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    /// Absolute path to the SQLite database file on disk.
    pub db_path: PathBuf,
    /// Database file size in bytes (0 if in-memory).
    pub db_size_bytes: u64,
    /// Schema version integer stored in metadata.
    pub schema_version: i64,
    /// Total indexed source files.
    pub total_files: usize,
    /// Total indexed declarations (functions, structs, classes, etc.).
    pub total_symbols: usize,
    /// Total indexed call sites.
    pub total_callers: usize,
    /// Total indexed interface implementors.
    pub total_implementors: usize,
    /// ISO-8601 / RFC-3339 timestamp of last successful index synchronization.
    pub last_indexed_at: Option<String>,
    /// Whether SQLite Write-Ahead Logging (WAL) mode is active.
    pub is_wal_mode: bool,
    /// Whether database is running in in-memory fallback mode.
    pub in_memory: bool,
}

/// Internal file metadata record cached in SQLite.
struct DbFileInfo {
    id: i64,
    size_bytes: u64,
    mtime_secs: u64,
    mtime_nanos: u32,
    sha256_hash: String,
}

/// Persistent AST index engine backed by SQLite.
pub struct IndexEngine {
    pub(crate) workspace_root: PathBuf,
    pub(crate) db_path: PathBuf,
    pub(crate) conn: Connection,
    pub(crate) in_memory: bool,
}

impl IndexEngine {
    /// Opens an existing SQLite index database at `<workspace_root>/.ctxcut/index.db` or creates it.
    pub fn open_or_create(workspace_root: &Path) -> Result<Self> {
        let ws_root = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        let ctxcut_dir = ws_root.join(".ctxcut");
        let db_path = ctxcut_dir.join("index.db");

        let in_memory_fallback = || {
            let conn = Connection::open_in_memory()
                .map_err(|e| CoreError::DatabaseError(format!("In-memory SQLite failed: {e}")))?;
            let mut engine = Self {
                workspace_root: ws_root.clone(),
                db_path: PathBuf::from(":memory:"),
                conn,
                in_memory: true,
            };
            engine.init_database()?;
            Ok(engine)
        };

        if !ctxcut_dir.exists() {
            if fs::create_dir_all(&ctxcut_dir).is_err() {
                return in_memory_fallback();
            }
        }

        let conn_res = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        );

        let conn = match conn_res {
            Ok(c) => c,
            Err(e) => {
                // Check if corrupted database
                if e.to_string().contains("corrupt") || e.to_string().contains("not a database") {
                    Self::recover_corrupted_db(&db_path)?;
                    Connection::open(&db_path).map_err(|e2| {
                        CoreError::DatabaseError(format!("Failed to reopen recovered DB: {e2}"))
                    })?
                } else {
                    return in_memory_fallback();
                }
            }
        };

        let mut engine = Self {
            workspace_root: ws_root.clone(),
            db_path,
            conn,
            in_memory: false,
        };

        // Test database integrity
        if let Err(e) = engine.init_database() {
            if e.to_string().contains("corrupt") || e.to_string().contains("not a database") {
                Self::recover_corrupted_db(&engine.db_path)?;
                engine.conn = Connection::open(&engine.db_path).map_err(|e2| {
                    CoreError::DatabaseError(format!("Failed to reopen recovered DB: {e2}"))
                })?;
                engine.init_database()?;
            } else {
                return in_memory_fallback();
            }
        }

        Ok(engine)
    }

    /// Opens an in-memory SQLite database index (useful for ephemeral unit tests).
    pub fn open_in_memory(workspace_root: &Path) -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| CoreError::DatabaseError(format!("In-memory SQLite failed: {e}")))?;
        let mut engine = Self {
            workspace_root: workspace_root.to_path_buf(),
            db_path: PathBuf::from(":memory:"),
            conn,
            in_memory: true,
        };
        engine.init_database()?;
        Ok(engine)
    }

    /// Recovers a corrupted database by deleting it and any associated WAL/SHM journal files.
    pub fn recover_corrupted_db(db_path: &Path) -> Result<()> {
        if db_path.exists() {
            let _ = fs::remove_file(db_path);
        }
        let wal_path = db_path.with_extension("db-wal");
        if wal_path.exists() {
            let _ = fs::remove_file(wal_path);
        }
        let shm_path = db_path.with_extension("db-shm");
        if shm_path.exists() {
            let _ = fs::remove_file(shm_path);
        }
        Ok(())
    }

    /// Initializes PRAGMA parameters and applies DDL schema migrations.
    fn init_database(&mut self) -> Result<()> {
        apply_pragmas(&self.conn)?;
        apply_schema(&self.conn)?;
        Ok(())
    }

    /// Returns direct reference to underlying rusqlite Connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Returns mutable reference to underlying rusqlite Connection.
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Returns workspace root directory.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Returns database file path.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Cleans and removes database files from disk for the workspace.
    pub fn clean(workspace_root: &Path) -> Result<()> {
        let ws_root = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        let db_path = ws_root.join(".ctxcut").join("index.db");
        Self::recover_corrupted_db(&db_path)
    }

    /// Computes current database health and statistical status.
    pub fn status(&self) -> Result<IndexStatus> {
        let db_size_bytes = if self.in_memory || !self.db_path.exists() {
            0
        } else {
            fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0)
        };

        let schema_version: i64 = self
            .conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = 'schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(CURRENT_SCHEMA_VERSION as i64);

        let total_files: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
            .unwrap_or(0);

        let total_symbols: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap_or(0);

        let total_callers: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM callers", [], |r| r.get(0))
            .unwrap_or(0);

        let total_implementors: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM implementors", [], |r| r.get(0))
            .unwrap_or(0);

        let last_indexed_at: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = 'last_indexed_at'",
                [],
                |r| r.get(0),
            )
            .ok();

        let journal_mode: String = self
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap_or_default();

        let is_wal_mode = journal_mode.eq_ignore_ascii_case("wal");

        Ok(IndexStatus {
            db_path: self.db_path.clone(),
            db_size_bytes,
            schema_version,
            total_files,
            total_symbols,
            total_callers,
            total_implementors,
            last_indexed_at,
            is_wal_mode,
            in_memory: self.in_memory,
        })
    }

    /// Completely rebuilds the index from scratch.
    pub fn rebuild(&mut self) -> Result<IndexSyncResult> {
        let opts = IndexOptions {
            rebuild: true,
            ..Default::default()
        };
        self.sync_incremental(&opts)
    }

    /// Performs incremental synchronization with two-tier change detection.
    pub fn sync_incremental(&mut self, options: &IndexOptions) -> Result<IndexSyncResult> {
        let start_time = Instant::now();

        if options.rebuild {
            self.conn
                .execute_batch(
                    r#"
                    DELETE FROM bm25_postings;
                    DELETE FROM bm25_terms;
                    DELETE FROM bm25_doc_stats;
                    DELETE FROM routes;
                    DELETE FROM client_endpoints;
                    DELETE FROM schema_entities;
                    DELETE FROM symbols;
                    DELETE FROM callers;
                    DELETE FROM implementors;
                    DELETE FROM dependencies;
                    DELETE FROM symbol_references;
                    DELETE FROM files;
                    "#,
                )
                .map_err(|e| CoreError::DatabaseError(format!("Failed to wipe index: {e}")))?;
        }

        // 1. Collect all valid candidate source files on disk
        let traversal_config = TraversalConfig::default();
        if let Some(depth) = options.max_depth {
            let _ = depth;
        }

        let on_disk_paths = ProjectWalker::collect_files(&self.workspace_root, &traversal_config);
        let mut on_disk_set: HashSet<String> = HashSet::new();

        let mut db_files: HashMap<String, DbFileInfo> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT id, path, size_bytes, mtime_secs, mtime_nanos, sha256_hash FROM files",
                )
                .map_err(|e| {
                    CoreError::DatabaseError(format!("Failed to prepare file query: {e}"))
                })?;

            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        DbFileInfo {
                            id: row.get(0)?,
                            size_bytes: row.get::<_, i64>(2)? as u64,
                            mtime_secs: row.get::<_, i64>(3)? as u64,
                            mtime_nanos: row.get::<_, i64>(4)? as u32,
                            sha256_hash: row.get(5)?,
                        },
                    ))
                })
                .map_err(|e| CoreError::DatabaseError(format!("Failed to query files: {e}")))?;

            for row in rows.flatten() {
                db_files.insert(row.0, row.1);
            }
        }

        let mut files_added = 0;
        let mut files_updated = 0;
        let mut files_unchanged = 0;

        let tx = self.conn.transaction().map_err(|e| {
            CoreError::DatabaseError(format!("Failed to begin SQLite transaction: {e}"))
        })?;

        // 2. Process on-disk files
        for disk_path in on_disk_paths {
            let (lang_opt, lang_str) = if let Some(lang) = SupportedLanguage::from_path(&disk_path) {
                (Some(lang), lang.as_str().to_string())
            } else {
                let ext = disk_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if matches!(ext.as_str(), "sql" | "prisma" | "graphql" | "gql" | "proto") {
                    (None, ext)
                } else {
                    continue;
                }
            };

            let Ok(metadata) = fs::metadata(&disk_path) else {
                continue;
            };

            let rel_path = disk_path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&disk_path)
                .to_string_lossy()
                .replace('\\', "/");

            on_disk_set.insert(rel_path.clone());

            let size_bytes = metadata.len();
            let (mtime_secs, mtime_nanos) = match metadata.modified() {
                Ok(t) => {
                    let dur = t.duration_since(UNIX_EPOCH).unwrap_or_default();
                    (dur.as_secs(), dur.subsec_nanos())
                }
                Err(_) => (0, 0),
            };

            // Tier 1 Check: Fast (mtime, size) match
            if let Some(existing) = db_files.get(&rel_path) {
                if existing.size_bytes == size_bytes
                    && existing.mtime_secs == mtime_secs
                    && existing.mtime_nanos == mtime_nanos
                {
                    files_unchanged += 1;
                    continue;
                }
            }

            // File might have changed or is new: read content
            let Ok(content) = fs::read_to_string(&disk_path) else {
                continue;
            };

            // Tier 2 Check: SHA-256 Content Hash
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let sha256_hash = format!("{:x}", hasher.finalize());

            if let Some(existing) = db_files.get(&rel_path) {
                if existing.sha256_hash == sha256_hash {
                    // Content is identical (e.g. touch or checkout). Update mtime only.
                    let _ = tx.execute(
                        "UPDATE files SET mtime_secs = ?1, mtime_nanos = ?2, size_bytes = ?3 WHERE id = ?4",
                        params![mtime_secs as i64, mtime_nanos as i64, size_bytes as i64, existing.id],
                    );
                    files_unchanged += 1;
                    continue;
                }

                // Truly modified: remove old records for this file (cascade deletes symbols/callers)
                let _ = tx.execute("DELETE FROM files WHERE id = ?1", params![existing.id]);
                files_updated += 1;
            } else {
                files_added += 1;
            }

            // Parse AST and extract symbols
            let total_lines = count_lines(&content);
            let total_tokens = count_tokens(&content);
            let now_ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            tx.execute(
                r#"
                INSERT INTO files (path, language, size_bytes, mtime_secs, mtime_nanos, sha256_hash, total_lines, total_tokens, indexed_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    rel_path,
                    lang_str,
                    size_bytes as i64,
                    mtime_secs as i64,
                    mtime_nanos as i64,
                    sha256_hash,
                    total_lines as i64,
                    total_tokens as i64,
                    now_ts
                ],
            ).map_err(|e| CoreError::DatabaseError(format!("Failed to insert file record: {e}")))?;

            let file_id = tx.last_insert_rowid();

            // Extract declarations & callers
            Self::index_file_ast(&tx, file_id, &disk_path, lang_opt, &content);
        }

        // 3. Reconcile deleted files (present in DB, absent on disk)
        let mut files_deleted = 0;
        for (db_path_str, db_info) in &db_files {
            if !on_disk_set.contains(db_path_str) {
                let _ = tx.execute("DELETE FROM files WHERE id = ?1", params![db_info.id]);
                files_deleted += 1;
            }
        }

        // 4. Update index_meta last_indexed_at
        let now_rfc3339 = current_rfc3339_timestamp();
        let _ = tx.execute(
            "INSERT OR REPLACE INTO index_meta (key, value) VALUES ('last_indexed_at', ?1)",
            params![now_rfc3339],
        );

        tx.commit().map_err(|e| {
            CoreError::DatabaseError(format!("Failed to commit index transaction: {e}"))
        })?;

        let total_symbols: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap_or(0);

        // 5. Pre-compute and persist Swarm community clusters and graph edges in SQLite
        let cluster_count: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM clusters", [], |r| r.get(0))
            .unwrap_or(0);
        if cluster_count == 0 || files_added > 0 || files_updated > 0 || files_deleted > 0 || options.rebuild {
            let _ = self.compute_and_persist_swarm_clusters(2);
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(IndexSyncResult {
            files_added,
            files_updated,
            files_deleted,
            files_unchanged,
            total_symbols,
            duration_ms,
        })
    }

    fn index_file_ast(
        tx: &rusqlite::Transaction<'_>,
        file_id: i64,
        file_path: &Path,
        lang_opt: Option<SupportedLanguage>,
        content: &str,
    ) {
        if let Some(lang) = lang_opt {
            let overview_opts = OverviewOptions {
                include_routes: true,
                ..Default::default()
            };

            let symbols = extract_symbols_from_file(file_path, lang, content, &overview_opts);
            let lines: Vec<&str> = content.lines().collect();

            for sym in symbols {
                let container_name = if sym.name.contains('.') {
                    sym.name.split('.').next().map(String::from)
                } else if sym.name.contains("::") {
                    sym.name.split("::").next().map(String::from)
                } else {
                    None
                };

                let start_line = sym.start_line;
                let end_line = sym.end_line;
                let body_snippet = if start_line > 0 && start_line <= lines.len() {
                    let end = end_line.min(lines.len());
                    lines[start_line - 1..end].join("\n")
                } else {
                    String::new()
                };

                let is_exported = sym.name.starts_with("pub ")
                    || sym.kind == "route"
                    || sym.signature.as_deref().unwrap_or("").contains("export ");

                let sym_insert = tx.execute(
                    r#"
                    INSERT INTO symbols (file_id, name, container_name, kind, start_line, end_line, start_byte, end_byte, signature, doc_comment, body, is_exported)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                    "#,
                    params![
                        file_id,
                        sym.name,
                        container_name,
                        sym.kind,
                        start_line as i64,
                        end_line as i64,
                        0i64,
                        0i64,
                        sym.signature,
                        sym.doc_summary,
                        body_snippet,
                        is_exported,
                    ],
                );

                if sym_insert.is_ok() {
                    let symbol_id = tx.last_insert_rowid();

                    let sym_doc = crate::intent::tokenizer::extract_symbol_tokens(
                        &sym.name,
                        sym.signature.as_deref().unwrap_or(""),
                        sym.doc_summary.as_deref(),
                        &file_path.to_string_lossy().replace('\\', "/"),
                        &body_snippet,
                    );

                    let _ = tx.execute(
                        "INSERT OR REPLACE INTO bm25_doc_stats (symbol_id, file_id, total_terms) VALUES (?1, ?2, ?3)",
                        params![symbol_id, file_id, sym_doc.total_terms as i64],
                    );

                    let mut unique_terms = HashSet::new();
                    for term_map in sym_doc.field_term_freqs.values() {
                        for term in term_map.keys() {
                            unique_terms.insert(term.clone());
                        }
                    }

                    for term in &unique_terms {
                        let _ = tx.execute(
                            "INSERT INTO bm25_terms (term, doc_freq, idf) VALUES (?1, 1, 0.0) ON CONFLICT(term) DO UPDATE SET doc_freq = doc_freq + 1",
                            params![term],
                        );
                    }

                    for (&field, term_map) in &sym_doc.field_term_freqs {
                        let field_len = sym_doc.field_lengths.get(&field).copied().unwrap_or(0);
                        for (term, &freq) in term_map {
                            let term_id: i64 = tx.query_row(
                                "SELECT id FROM bm25_terms WHERE term = ?1",
                                params![term],
                                |r| r.get(0),
                            ).unwrap_or(0);

                            if term_id > 0 {
                                let _ = tx.execute(
                                    r#"
                                    INSERT INTO bm25_postings (term_id, symbol_id, file_id, field, term_freq, field_length)
                                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                                    "#,
                                    params![
                                        term_id,
                                        symbol_id,
                                        file_id,
                                        field.as_str(),
                                        freq as i64,
                                        field_len as i64,
                                    ],
                                );
                            }
                        }
                    }

                    if let Some(ref container) = container_name {
                        let _ = tx.execute(
                            r#"
                            INSERT INTO implementors (interface_name, implementor_name, file_id, symbol_id, kind, definition)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                            "#,
                            params![
                                container,
                                sym.name,
                                file_id,
                                symbol_id,
                                sym.kind,
                                sym.signature,
                            ],
                        );
                    }
                }
            }

            // Call extraction
            if let Ok(adapter) = LanguageRegistry::for_path(file_path) {
                let ts_lang = adapter.tree_sitter_language(file_path);
                if let Ok(tree) = ParserManager::parse_source(content, &ts_lang, file_path) {
                    let root = tree.root_node();
                    let mut call_nodes = Vec::new();
                    collect_call_nodes(root, &mut call_nodes);

                    for call_n in call_nodes {
                        let line = call_n.start_position().row + 1;
                        let snippet = AstUtils::node_text(call_n, content);
                        let callee_name = extract_callee_name(call_n, content);

                        if !callee_name.is_empty() {
                            let enclosing_func = find_enclosing_function_name(call_n, content)
                                .unwrap_or_else(|| "anonymous".to_string());

                            let _ = tx.execute(
                                r#"
                                INSERT INTO callers (target_symbol_name, target_container, caller_file_id, caller_symbol_id, caller_symbol_name, caller_kind, call_line, call_snippet, caller_signature)
                                VALUES (?1, NULL, ?2, NULL, ?3, 'function', ?4, ?5, NULL)
                                "#,
                                params![
                                    callee_name,
                                    file_id,
                                    enclosing_func,
                                    line as i64,
                                    snippet,
                                ],
                            );
                        }
                    }
                }
            }
        }

        // 1. Server route extraction
        let routes = extract_server_routes(file_path, content);
        for route in routes {
            let req_dto_str = route.request_dto_type.as_ref().map(|d| d.definition.clone());
            let res_dto_str = route.response_dto_type.as_ref().map(|d| d.definition.clone());
            let _ = tx.execute(
                r#"
                INSERT INTO routes (file_id, framework, http_method, route_path, handler_symbol, handler_signature, request_dto, response_dto, start_line, end_line)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 1)
                "#,
                params![
                    file_id,
                    route.framework,
                    route.http_method,
                    route.route_path,
                    route.handler_symbol,
                    route.handler_signature,
                    req_dto_str,
                    res_dto_str,
                ],
            );
        }

        // 2. Client endpoint extraction
        let client_calls = ClientDetector::new().detect_in_file(file_path, content);
        for call in client_calls {
            let _ = tx.execute(
                r#"
                INSERT INTO client_endpoints (file_id, client_kind, http_method, endpoint_url, rpc_procedure, line_number, call_snippet, request_dto, response_dto)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    file_id,
                    call.client_kind,
                    call.http_method,
                    call.endpoint_url,
                    call.rpc_procedure,
                    call.line_number as i64,
                    call.call_snippet,
                    call.request_dto,
                    call.response_dto,
                ],
            );
        }

        // 3. Schema entity extraction
        let schema_entities = extract_schema_entities(file_path, content);
        for ent in schema_entities {
            let _ = tx.execute(
                r#"
                INSERT INTO schema_entities (file_id, schema_kind, entity_name, table_name, definition, start_line, end_line)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    file_id,
                    ent.schema_kind,
                    ent.entity_name,
                    ent.table_name,
                    ent.definition,
                    ent.start_line as i64,
                    ent.end_line as i64,
                ],
            );
        }
    }

    /// Finds all server route endpoints matching a route path in the SQLite index with sub-5ms lookup latency.
    pub fn find_routes_by_path(&self, route_path: &str) -> Result<Vec<ServerRouteEndpoint>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT r.framework, r.http_method, r.route_path, f.path, r.handler_symbol, r.handler_signature, r.request_dto, r.response_dto
            FROM routes r
            JOIN files f ON r.file_id = f.id
            WHERE r.route_path = ?1 OR r.route_path LIKE ?2
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare route query: {e}")))?;

        let pattern = format!("%{route_path}%");
        let rows = stmt.query_map(params![route_path, pattern], |row| {
            let req_dto: Option<String> = row.get(6)?;
            let res_dto: Option<String> = row.get(7)?;
            let file_path: String = row.get(3)?;

            let req_dto_type = req_dto.map(|d| ExtractedType {
                name: d.clone(),
                kind: "dto".to_string(),
                file_path: file_path.clone(),
                definition: d,
            });
            let response_dto_type = res_dto.map(|d| ExtractedType {
                name: d.clone(),
                kind: "dto".to_string(),
                file_path: file_path.clone(),
                definition: d,
            });

            Ok(ServerRouteEndpoint {
                framework: row.get(0)?,
                http_method: row.get(1)?,
                route_path: row.get(2)?,
                handler_file: file_path,
                handler_symbol: row.get(4)?,
                handler_signature: row.get(5)?,
                request_dto_type: req_dto_type,
                response_dto_type,
            })
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query routes: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    /// Finds all client API calls matching an endpoint URL or RPC procedure in the SQLite index with sub-5ms lookup latency.
    pub fn find_client_endpoints_by_url_or_proc(&self, query: &str) -> Result<Vec<ClientApiCall>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT c.client_kind, c.http_method, c.endpoint_url, c.rpc_procedure, f.path, c.line_number, c.call_snippet, c.request_dto, c.response_dto
            FROM client_endpoints c
            JOIN files f ON c.file_id = f.id
            WHERE c.endpoint_url = ?1 OR c.endpoint_url LIKE ?2 OR c.rpc_procedure = ?1 OR c.rpc_procedure LIKE ?2
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare client endpoint query: {e}")))?;

        let pattern = format!("%{query}%");
        let rows = stmt.query_map(params![query, pattern], |row| {
            Ok(ClientApiCall {
                client_kind: row.get(0)?,
                http_method: row.get(1)?,
                endpoint_url: row.get(2)?,
                rpc_procedure: row.get(3)?,
                file_path: row.get(4)?,
                line_number: row.get::<_, i64>(5)? as usize,
                call_snippet: row.get(6)?,
                request_dto: row.get(7)?,
                response_dto: row.get(8)?,
            })
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query client endpoints: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    /// Finds schema entities by entity name or table name in the SQLite index with sub-5ms lookup latency.
    pub fn find_schema_entities(&self, entity_or_table: &str) -> Result<Vec<ExtractedType>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT s.entity_name, s.schema_kind, f.path, s.definition
            FROM schema_entities s
            JOIN files f ON s.file_id = f.id
            WHERE s.entity_name = ?1 OR s.table_name = ?1 OR s.entity_name LIKE ?2
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare schema query: {e}")))?;

        let pattern = format!("%{entity_or_table}%");
        let rows = stmt.query_map(params![entity_or_table, pattern], |row| {
            Ok(ExtractedType {
                name: row.get(0)?,
                kind: row.get(1)?,
                file_path: row.get(2)?,
                definition: row.get(3)?,
            })
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query schema entities: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    /// Returns all indexed server route endpoints across the workspace.
    pub fn get_all_routes(&self) -> Result<Vec<ServerRouteEndpoint>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT r.framework, r.http_method, r.route_path, f.path, r.handler_symbol, r.handler_signature, r.request_dto, r.response_dto
            FROM routes r
            JOIN files f ON r.file_id = f.id
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare get_all_routes query: {e}")))?;

        let rows = stmt.query_map([], |row| {
            let req_dto: Option<String> = row.get(6)?;
            let res_dto: Option<String> = row.get(7)?;
            let file_path: String = row.get(3)?;

            let req_dto_type = req_dto.map(|d| ExtractedType {
                name: d.clone(),
                kind: "dto".to_string(),
                file_path: file_path.clone(),
                definition: d,
            });
            let response_dto_type = res_dto.map(|d| ExtractedType {
                name: d.clone(),
                kind: "dto".to_string(),
                file_path: file_path.clone(),
                definition: d,
            });

            Ok(ServerRouteEndpoint {
                framework: row.get(0)?,
                http_method: row.get(1)?,
                route_path: row.get(2)?,
                handler_file: file_path,
                handler_symbol: row.get(4)?,
                handler_signature: row.get(5)?,
                request_dto_type: req_dto_type,
                response_dto_type,
            })
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query all routes: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    /// Returns all indexed client API calls across the workspace.
    pub fn get_all_client_endpoints(&self) -> Result<Vec<ClientApiCall>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT c.client_kind, c.http_method, c.endpoint_url, c.rpc_procedure, f.path, c.line_number, c.call_snippet, c.request_dto, c.response_dto
            FROM client_endpoints c
            JOIN files f ON c.file_id = f.id
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare get_all_client_endpoints query: {e}")))?;

        let rows = stmt.query_map([], |row| {
            Ok(ClientApiCall {
                client_kind: row.get(0)?,
                http_method: row.get(1)?,
                endpoint_url: row.get(2)?,
                rpc_procedure: row.get(3)?,
                file_path: row.get(4)?,
                line_number: row.get::<_, i64>(5)? as usize,
                call_snippet: row.get(6)?,
                request_dto: row.get(7)?,
                response_dto: row.get(8)?,
            })
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query all client endpoints: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    /// Performs sub-5ms BM25 ranking across persistent indexed symbols in SQLite.
    pub fn bm25_search_symbols(&self, query: &str, limit: usize) -> Result<Vec<(ExtractedSymbol, f64)>> {
        let keywords = crate::intent::extract_query_keywords(query);
        if keywords.is_empty() {
            return Ok(Vec::new());
        }

        let total_symbols: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap_or(0);

        if total_symbols == 0 {
            return Ok(Vec::new());
        }

        // Compute average field lengths from SQLite
        let mut avg_field_lengths: HashMap<String, f64> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT field, AVG(field_length) FROM bm25_postings GROUP BY field"
            ).map_err(|e| CoreError::DatabaseError(format!("Failed to query avg field lengths: {e}")))?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            }).map_err(|e| CoreError::DatabaseError(format!("Failed to read avg field lengths: {e}")))?;
            for r in rows.flatten() {
                avg_field_lengths.insert(r.0, r.1.max(1.0));
            }
        }

        let mut symbol_scores: HashMap<i64, f64> = HashMap::new();
        let k1 = 1.2f64;
        let b = 0.75f64;

        for term in &keywords {
            let doc_freq: usize = self
                .conn
                .query_row(
                    "SELECT doc_freq FROM bm25_terms WHERE term = ?1",
                    params![term],
                    |r| r.get::<_, i64>(0).map(|v| v as usize),
                )
                .unwrap_or(0);

            if doc_freq == 0 {
                continue;
            }

            let idf = crate::intent::compute_idf(total_symbols, doc_freq);

            let mut stmt = self.conn.prepare(
                r#"
                SELECT p.symbol_id, p.field, p.term_freq, p.field_length
                FROM bm25_postings p
                JOIN bm25_terms t ON p.term_id = t.id
                WHERE t.term = ?1
                "#
            ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare BM25 query: {e}")))?;

            let rows = stmt.query_map(params![term], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as f64,
                    row.get::<_, i64>(3)? as f64,
                ))
            }).map_err(|e| CoreError::DatabaseError(format!("Failed to execute BM25 query: {e}")))?;

            for row in rows.flatten() {
                let (sym_id, field, tf, field_len) = row;
                let avg_len = avg_field_lengths.get(&field).copied().unwrap_or(1.0);
                let field_weight = crate::intent::FieldKind::from_field_str(&field).map(|f| f.weight()).unwrap_or(1.0);
                let norm = 1.0 - b + b * (field_len / avg_len);
                let tf_norm = if norm > 0.0 { tf / norm } else { tf };
                let weighted_tf = field_weight * tf_norm;
                if weighted_tf > 0.0 {
                    let score = idf * ((weighted_tf * (k1 + 1.0)) / (weighted_tf + k1));
                    *symbol_scores.entry(sym_id).or_insert(0.0) += score;
                }
            }
        }

        let mut scored_sym_ids: Vec<(i64, f64)> = symbol_scores.into_iter().collect();
        scored_sym_ids.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_sym_ids.truncate(limit);

        let mut results = Vec::with_capacity(scored_sym_ids.len());
        for (sym_id, score) in scored_sym_ids {
            let sym_res = self.conn.query_row(
                r#"
                SELECT s.name, s.kind, f.path, s.start_line, s.end_line, s.signature, s.doc_comment, s.body, f.language
                FROM symbols s
                JOIN files f ON s.file_id = f.id
                WHERE s.id = ?1
                "#,
                params![sym_id],
                |row| {
                    Ok(ExtractedSymbol {
                        name: row.get(0)?,
                        kind: row.get(1)?,
                        file_path: row.get(2)?,
                        start_line: row.get::<_, i64>(3)? as usize,
                        end_line: row.get::<_, i64>(4)? as usize,
                        signature: row.get(5)?,
                        doc_comment: row.get(6)?,
                        body: row.get(7)?,
                        language: row.get(8)?,
                    })
                }
            );
            if let Ok(sym) = sym_res {
                results.push((sym, score));
            }
        }

        Ok(results)
    }

    /// Computes BM25 corpus statistics: (total_documents, avg_doc_length).
    pub fn get_bm25_stats(&self) -> Result<(usize, f64)> {
        let total_docs: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM bm25_doc_stats", [], |r| r.get(0))
            .unwrap_or(0);
        let avg_len: f64 = self
            .conn
            .query_row("SELECT AVG(total_terms) FROM bm25_doc_stats", [], |r| r.get(0))
            .unwrap_or(0.0);
        Ok((total_docs, avg_len))
    }

    /// Computes and persists Swarm Community Clusters and boundary contracts into SQLite.
    pub fn compute_and_persist_swarm_clusters(&mut self, default_agents: usize) -> Result<()> {
        let graph = WorkspaceGraphBuilder::build(&self.workspace_root)?;
        if graph.nodes.is_empty() {
            return Ok(());
        }

        // Store graph edges cache
        let _ = self.conn.execute("DELETE FROM graph_edges", []);
        {
            let mut edge_stmt = self.conn.prepare(
                "INSERT INTO graph_edges (from_node, to_node, weight, kind) VALUES (?1, ?2, ?3, ?4)",
            ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare graph_edges stmt: {e}")))?;

            for edge in &graph.edges {
                let kind_str = match edge.kind {
                    crate::swarm::EdgeKind::Call => "call",
                    crate::swarm::EdgeKind::TypeRef => "typeref",
                    crate::swarm::EdgeKind::Import => "import",
                    crate::swarm::EdgeKind::CoLocated => "colocated",
                };
                let _ = edge_stmt.execute(params![
                    edge.from,
                    edge.to,
                    edge.weight,
                    kind_str,
                ]);
            }
        }

        // Precompute for default agent counts: 2, 3, 4
        let agent_counts_to_precompute = if default_agents == 2 {
            vec![2, 3, 4]
        } else {
            vec![default_agents]
        };

        for target_agents in agent_counts_to_precompute {
            let clusters = CommunityClusterer::cluster(&graph, target_agents, &[]);
            if clusters.is_empty() {
                continue;
            }

            // Build node -> agent map
            let mut node_to_agent: HashMap<String, String> = HashMap::new();
            for (idx, cluster) in clusters.iter().enumerate() {
                let agent_id = format!("agent_{idx}");
                for node_id in cluster {
                    node_to_agent.insert(node_id.clone(), agent_id.clone());
                }
            }

            // Clean previous clusters for this total_agents
            let _ = self.conn.execute(
                "DELETE FROM clusters WHERE total_agents = ?1",
                params![target_agents as i64],
            );

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            for (idx, cluster_node_ids) in clusters.iter().enumerate() {
                let agent_id = format!("agent_{idx}");
                let cluster_name = derive_cluster_name(&graph, cluster_node_ids, idx);

                let mut internal_symbols = Vec::new();
                for node_id in cluster_node_ids {
                    if let Some(node) = graph.nodes.get(node_id) {
                        internal_symbols.push(node.symbol.clone());
                    }
                }

                let (boundary_stubs, boundary_types) = BoundaryStubGenerator::synthesize_boundaries(
                    &graph,
                    cluster_node_ids,
                    &node_to_agent,
                );

                let primary_lang = internal_symbols
                    .first()
                    .and_then(|s| SupportedLanguage::from_str_loose(&s.language))
                    .unwrap_or(SupportedLanguage::TypeScript);

                let mock_contracts = MockContractGenerator::generate_mocks(
                    &agent_id,
                    &boundary_stubs,
                    &boundary_types,
                    primary_lang,
                );

                let mut pack = SwarmAgentPack {
                    agent_id: agent_id.clone(),
                    cluster_name: cluster_name.clone(),
                    internal_symbols: internal_symbols.clone(),
                    boundary_stubs: boundary_stubs.clone(),
                    boundary_types: boundary_types.clone(),
                    mock_contracts: mock_contracts.clone(),
                    token_stats: TokenStats::calculate(0, 0, 0, 0),
                };

                SwarmBudgetEngine::compute_and_apply_budget(&mut pack, &graph, None);

                let symbol_count = internal_symbols.len() as i64;
                let token_count = pack.token_stats.sliced_tokens as i64;

                let mut cluster_stmt = self.conn.prepare(
                    r#"
                    INSERT INTO clusters (
                        total_agents, cluster_index, agent_id, cluster_name, primary_language,
                        symbol_count, token_count, mock_contracts, raw_tokens, sliced_tokens,
                        raw_lines, sliced_lines, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                    "#,
                ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare cluster insert: {e}")))?;

                cluster_stmt.execute(params![
                    target_agents as i64,
                    idx as i64,
                    agent_id,
                    cluster_name,
                    primary_lang.as_str(),
                    symbol_count,
                    token_count,
                    mock_contracts,
                    pack.token_stats.raw_file_tokens as i64,
                    pack.token_stats.sliced_tokens as i64,
                    pack.token_stats.raw_lines as i64,
                    pack.token_stats.sliced_lines as i64,
                    now,
                ])?;

                let cluster_id = self.conn.last_insert_rowid();

                // Insert cluster symbols
                let mut sym_stmt = self.conn.prepare(
                    r#"
                    INSERT INTO cluster_symbols (
                        cluster_id, symbol_id, symbol_name, file_path, language, signature,
                        doc_comment, body, start_line, end_line, token_count, line_count, is_seed
                    ) VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)
                    "#,
                ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare cluster_symbols insert: {e}")))?;

                for sym in &internal_symbols {
                    let sym_toks = (count_tokens(&sym.signature) + count_tokens(&sym.body)) as i64;
                    let sym_lines = if sym.end_line >= sym.start_line {
                        (sym.end_line - sym.start_line + 1) as i64
                    } else {
                        1
                    };

                    sym_stmt.execute(params![
                        cluster_id,
                        sym.name,
                        sym.file_path,
                        sym.language,
                        sym.signature,
                        sym.doc_comment,
                        sym.body,
                        sym.start_line as i64,
                        sym.end_line as i64,
                        sym_toks,
                        sym_lines,
                    ])?;
                }

                // Insert boundary stubs
                let mut boundary_stmt = self.conn.prepare(
                    r#"
                    INSERT INTO cluster_boundaries (
                        cluster_id, kind, symbol_name, target_agent_id, content, file_path,
                        start_line, end_line, language, signature, doc_comment
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                    "#,
                ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare cluster_boundaries insert: {e}")))?;

                for stub in &boundary_stubs {
                    boundary_stmt.execute(params![
                        cluster_id,
                        "stub",
                        stub.name,
                        "",
                        stub.signature,
                        stub.file_path.as_deref().unwrap_or(""),
                        1i64,
                        1i64,
                        "typescript",
                        Some(stub.signature.clone()),
                        None::<String>,
                    ])?;
                }

                for ty in &boundary_types {
                    boundary_stmt.execute(params![
                        cluster_id,
                        "type",
                        ty.name,
                        "",
                        ty.definition,
                        ty.file_path,
                        1i64,
                        1i64,
                        "typescript",
                        None::<String>,
                        None::<String>,
                    ])?;
                }
            }
        }

        Ok(())
    }

    /// Retrieves pre-computed swarm community partition manifest from SQLite in O(1) time (<10ms).
    pub fn get_precomputed_swarm_manifest(
        &self,
        agents_count: usize,
        seed_symbols: &[String],
        budget_per_agent: Option<usize>,
    ) -> Result<Option<SwarmPartitionManifest>> {
        if !seed_symbols.is_empty() {
            // Seed-constrained clustering requires dynamic partition calculation
            return Ok(None);
        }

        let target_agents = agents_count.max(1);

        let mut cluster_stmt = self.conn.prepare(
            r#"
            SELECT id, cluster_index, agent_id, cluster_name, primary_language,
                   symbol_count, token_count, mock_contracts, raw_tokens, sliced_tokens,
                   raw_lines, sliced_lines
            FROM clusters
            WHERE total_agents = ?1
            ORDER BY cluster_index ASC
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare cluster query: {e}")))?;

        struct ClusterRow {
            id: i64,
            _cluster_index: i64,
            agent_id: String,
            cluster_name: String,
            _primary_language: String,
            _symbol_count: i64,
            _token_count: i64,
            mock_contracts: Option<String>,
            raw_tokens: i64,
            sliced_tokens: i64,
            raw_lines: i64,
            sliced_lines: i64,
        }

        let cluster_rows: Vec<ClusterRow> = cluster_stmt
            .query_map(params![target_agents as i64], |row| {
                Ok(ClusterRow {
                    id: row.get(0)?,
                    _cluster_index: row.get(1)?,
                    agent_id: row.get(2)?,
                    cluster_name: row.get(3)?,
                    _primary_language: row.get(4)?,
                    _symbol_count: row.get(5)?,
                    _token_count: row.get(6)?,
                    mock_contracts: row.get(7)?,
                    raw_tokens: row.get(8)?,
                    sliced_tokens: row.get(9)?,
                    raw_lines: row.get(10)?,
                    sliced_lines: row.get(11)?,
                })
            })
            .map_err(|e| CoreError::DatabaseError(format!("Failed to query clusters: {e}")))?
            .flatten()
            .collect();

        if cluster_rows.is_empty() {
            return Ok(None);
        }

        let mut sym_stmt = self.conn.prepare(
            r#"
            SELECT symbol_name, file_path, language, signature, doc_comment, body, start_line, end_line
            FROM cluster_symbols
            WHERE cluster_id = ?1
            ORDER BY id ASC
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare cluster_symbols query: {e}")))?;

        let mut boundary_stmt = self.conn.prepare(
            r#"
            SELECT kind, symbol_name, target_agent_id, content, file_path, start_line, language, signature, doc_comment
            FROM cluster_boundaries
            WHERE cluster_id = ?1
            ORDER BY id ASC
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare cluster_boundaries query: {e}")))?;

        let mut packs = Vec::with_capacity(cluster_rows.len());
        let mut total_boundary_contracts = 0;

        for crow in cluster_rows {
            // Load internal symbols
            let internal_symbols: Vec<ExtractedSymbol> = sym_stmt
                .query_map(params![crow.id], |r| {
                    let name: String = r.get(0)?;
                    let file_path: String = r.get(1)?;
                    let language: String = r.get(2)?;
                    let signature: String = r.get(3)?;
                    let doc_comment: Option<String> = r.get(4)?;
                    let body: String = r.get(5)?;
                    let start_line: i64 = r.get(6)?;
                    let end_line: i64 = r.get(7)?;

                    Ok(ExtractedSymbol {
                        name,
                        kind: "symbol".to_string(),
                        file_path,
                        language,
                        signature,
                        doc_comment,
                        body,
                        start_line: start_line as usize,
                        end_line: end_line as usize,
                    })
                })
                .map_err(|e| CoreError::DatabaseError(format!("Failed to query cluster symbols: {e}")))?
                .flatten()
                .collect();

            // Load boundaries
            let mut boundary_stubs = Vec::new();
            let mut boundary_types = Vec::new();

            let b_rows = boundary_stmt
                .query_map(params![crow.id], |r| {
                    let kind: String = r.get(0)?;
                    let symbol_name: String = r.get(1)?;
                    let _target_agent: String = r.get(2)?;
                    let content: String = r.get(3)?;
                    let file_path: String = r.get(4)?;
                    let _start_line: i64 = r.get(5)?;
                    let _language: String = r.get(6)?;
                    let signature: Option<String> = r.get(7)?;
                    let _doc_comment: Option<String> = r.get(8)?;

                    Ok((kind, symbol_name, content, file_path, signature))
                })
                .map_err(|e| CoreError::DatabaseError(format!("Failed to query boundaries: {e}")))?;

            for item in b_rows.flatten() {
                let (kind, sym_name, content, file_path, signature) = item;
                if kind == "stub" {
                    let sig_str = signature.unwrap_or(content);
                    let receiver = if sym_name.contains('.') {
                        sym_name.split('.').next().map(|s| s.to_string())
                    } else {
                        None
                    };
                    boundary_stubs.push(CallSignatureStub {
                        name: sym_name,
                        receiver,
                        file_path: if file_path.is_empty() { None } else { Some(file_path) },
                        signature: sig_str,
                    });
                } else {
                    boundary_types.push(ExtractedType {
                        name: sym_name,
                        kind: "type".to_string(),
                        file_path,
                        definition: content,
                    });
                }
            }

            total_boundary_contracts += boundary_stubs.len() + boundary_types.len();

            let token_stats = TokenStats::calculate(
                crow.raw_tokens as usize,
                crow.sliced_tokens as usize,
                crow.raw_lines as usize,
                crow.sliced_lines as usize,
            );

            let mut pack = SwarmAgentPack {
                agent_id: crow.agent_id,
                cluster_name: crow.cluster_name,
                internal_symbols,
                boundary_stubs,
                boundary_types,
                mock_contracts: crow.mock_contracts.unwrap_or_default(),
                token_stats,
            };

            // If budget per agent is specified, apply budget limit
            if let Some(budget) = budget_per_agent {
                if pack.token_stats.sliced_tokens > budget {
                    while pack.token_stats.sliced_tokens > budget && pack.boundary_stubs.len() > 1 {
                        pack.boundary_stubs.pop();
                        let sliced_t: usize = pack.internal_symbols.iter().map(|s| count_tokens(&s.body)).sum::<usize>()
                            + pack.boundary_stubs.iter().map(|s| count_tokens(&s.signature)).sum::<usize>()
                            + pack.boundary_types.iter().map(|t| count_tokens(&t.definition)).sum::<usize>();
                        pack.token_stats = TokenStats::calculate(pack.token_stats.raw_file_tokens, sliced_t, pack.token_stats.raw_lines, pack.token_stats.sliced_lines);
                    }
                }
            }

            packs.push(pack);
        }

        let total_symbols = packs.iter().map(|p| p.internal_symbols.len()).sum();
        let total_agents = packs.len();

        Ok(Some(SwarmPartitionManifest {
            total_agents,
            total_symbols,
            boundary_contracts_count: total_boundary_contracts,
            packs,
        }))
    }

    /// Returns all caller records where `target_symbol_name` matches.
    pub fn find_callers_of_symbol(&self, target_symbol_name: &str) -> Result<Vec<(String, String, usize, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT f.path, c.caller_symbol_name, c.call_line, c.call_snippet
            FROM callers c
            JOIN files f ON c.caller_file_id = f.id
            WHERE c.target_symbol_name = ?1 OR c.target_symbol_name LIKE ?2
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare callers query: {e}")))?;

        let pattern = format!("%{target_symbol_name}%");
        let rows = stmt.query_map(params![target_symbol_name, pattern], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, i64>(2)? as usize,
                row.get(3)?,
            ))
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query callers: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    /// Returns all indexed database and API schema entities.
    pub fn get_all_schema_entities(&self) -> Result<Vec<ExtractedType>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT s.entity_name, s.schema_kind, f.path, s.definition
            FROM schema_entities s
            JOIN files f ON s.file_id = f.id
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare get_all_schema_entities query: {e}")))?;

        let rows = stmt.query_map([], |row| {
            Ok(ExtractedType {
                name: row.get(0)?,
                kind: row.get(1)?,
                file_path: row.get(2)?,
                definition: row.get(3)?,
            })
        }).map_err(|e| CoreError::DatabaseError(format!("Failed to query all schema entities: {e}")))?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }
}

fn collect_call_nodes<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    if matches!(
        node.kind(),
        "call_expression" | "method_invocation" | "function_call" | "invocation_expression"
    ) {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_call_nodes(child, out);
    }
}

fn extract_callee_name(call_node: Node<'_>, source: &str) -> String {
    if let Some(func) = call_node
        .child_by_field_name("function")
        .or_else(|| call_node.child_by_field_name("name"))
        .or_else(|| call_node.named_child(0))
    {
        let raw = AstUtils::node_text(func, source);
        raw.split('(').next().unwrap_or(raw).trim().to_string()
    } else {
        String::new()
    }
}

fn find_enclosing_function_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if matches!(
            n.kind(),
            "function_declaration"
                | "function_item"
                | "function_definition"
                | "method_declaration"
                | "method_definition"
        ) {
            if let Some(name_n) = n.child_by_field_name("name") {
                return Some(AstUtils::node_text(name_n, source).to_string());
            }
        }
        curr = n.parent();
    }
    None
}

fn current_rfc3339_timestamp() -> String {
    let now = SystemTime::now();
    let dur = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    format!("{secs}")
}
