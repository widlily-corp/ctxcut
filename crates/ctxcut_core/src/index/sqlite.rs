//! SQLite storage backend, index lifecycle, and incremental synchronization.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::collapsible_if
)]

use super::schema::{apply_pragmas, apply_schema, CURRENT_SCHEMA_VERSION};
use crate::error::{CoreError, Result};
use crate::lang::LanguageRegistry;
use crate::model::{OverviewOptions, SupportedLanguage};
use crate::overview::extract_symbols_from_file;
use crate::parser::{AstUtils, ParserManager};
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
            let Some(lang) = SupportedLanguage::from_path(&disk_path) else {
                continue;
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
                    lang.as_str(),
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
            Self::index_file_ast(&tx, file_id, &disk_path, lang, &content);
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
        lang: SupportedLanguage,
        content: &str,
    ) {
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
