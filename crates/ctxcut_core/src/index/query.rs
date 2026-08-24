//! Accelerated query execution powered by the persistent SQLite index.

use super::sqlite::IndexEngine;
use crate::error::{CoreError, Result};
use crate::model::{
    ExtractedImplementor, ExtractedSymbol, FileOverviewItem, ImpactCallerItem, OverviewOptions,
    SymbolOverviewItem, WorkspaceOverviewReport,
};
use crate::overview::format_overview_markdown;
use crate::tokenizer::{calculate_savings_percentage, count_tokens};
use crate::traversal::LanguageStatItem;
use rusqlite::params;
use std::collections::HashMap;

impl IndexEngine {
    /// Accelerated symbol lookup by relative file path and symbol name.
    pub fn find_symbol(
        &self,
        file_path: &str,
        symbol_query: &str,
    ) -> Result<Option<ExtractedSymbol>> {
        let norm_path = file_path.replace('\\', "/");
        let mut stmt = self.conn.prepare(
            r#"
            SELECT s.name, s.kind, f.path, s.start_line, s.end_line, s.doc_comment, s.signature, s.body, f.language
            FROM symbols s
            JOIN files f ON s.file_id = f.id
            WHERE (f.path = ?1 OR f.path LIKE ?2) AND (s.name = ?3 OR s.name LIKE ?4)
            LIMIT 1
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare find_symbol: {e}")))?;

        let pattern_path = format!("%{norm_path}");
        let pattern_sym = format!("%{symbol_query}%");

        let mut rows = stmt
            .query(params![norm_path, pattern_path, symbol_query, pattern_sym])
            .map_err(|e| CoreError::DatabaseError(format!("Failed to query find_symbol: {e}")))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| CoreError::DatabaseError(e.to_string()))?
        {
            let name: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let path_str: String = row.get(2)?;
            let start_line: usize = row.get::<_, i64>(3)? as usize;
            let end_line: usize = row.get::<_, i64>(4)? as usize;
            let doc_comment: Option<String> = row.get(5)?;
            let signature: String = row.get(6)?;
            let body: String = row.get(7)?;
            let lang_str: String = row.get(8)?;
            let language = lang_str;

            Ok(Some(ExtractedSymbol {
                name,
                kind,
                file_path: path_str,
                start_line,
                end_line,
                doc_comment,
                signature,
                body,
                language,
            }))
        } else {
            Ok(None)
        }
    }

    /// Accelerated symbol lookup by name across all files in workspace.
    pub fn find_symbols_by_name(&self, symbol_name: &str) -> Result<Vec<ExtractedSymbol>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT s.name, s.kind, f.path, s.start_line, s.end_line, s.doc_comment, s.signature, s.body, f.language
            FROM symbols s
            JOIN files f ON s.file_id = f.id
            WHERE s.name = ?1 OR s.name LIKE ?2
            ORDER BY s.id ASC
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare find_symbols_by_name: {e}")))?;

        let pattern = format!("%{symbol_name}%");
        let rows = stmt
            .query_map(params![symbol_name, pattern], |row| {
                let name: String = row.get(0)?;
                let kind: String = row.get(1)?;
                let path_str: String = row.get(2)?;
                let start_line: usize = row.get::<_, i64>(3)? as usize;
                let end_line: usize = row.get::<_, i64>(4)? as usize;
                let doc_comment: Option<String> = row.get(5)?;
                let signature: String = row.get(6)?;
                let body: String = row.get(7)?;
                let lang_str: String = row.get(8)?;
                let language = lang_str;

                Ok(ExtractedSymbol {
                    name,
                    kind,
                    file_path: path_str,
                    start_line,
                    end_line,
                    doc_comment,
                    signature,
                    body,
                    language,
                })
            })
            .map_err(|e| CoreError::DatabaseError(format!("Failed to execute query: {e}")))?;

        let mut results = Vec::new();
        for item in rows.flatten() {
            results.push(item);
        }
        Ok(results)
    }

    /// Accelerated upstream caller discovery for a target symbol.
    pub fn find_callers(
        &self,
        target_symbol: &str,
        _target_file: Option<&str>,
    ) -> Result<Vec<ImpactCallerItem>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT c.caller_symbol_name, c.caller_kind, f.path, c.call_line, c.call_snippet, c.caller_signature
            FROM callers c
            JOIN files f ON c.caller_file_id = f.id
            WHERE c.target_symbol_name = ?1 OR c.target_symbol_name LIKE ?2
            ORDER BY f.path ASC, c.call_line ASC
            "#,
        ).map_err(|e| CoreError::DatabaseError(format!("Failed to prepare find_callers: {e}")))?;

        let pattern = format!("%{target_symbol}%");
        let rows = stmt
            .query_map(params![target_symbol, pattern], |row| {
                let caller_symbol: String = row.get(0)?;
                let caller_kind: String = row.get(1)?;
                let path_str: String = row.get(2)?;
                let line_number: usize = row.get::<_, i64>(3)? as usize;
                let call_snippet: String = row.get(4)?;
                let caller_signature: Option<String> = row.get(5)?;

                Ok(ImpactCallerItem {
                    caller_symbol,
                    caller_kind,
                    file_path: path_str,
                    line_number,
                    call_snippet,
                    caller_signature,
                })
            })
            .map_err(|e| CoreError::DatabaseError(format!("Failed to query callers: {e}")))?;

        let mut callers = Vec::new();
        for item in rows.flatten() {
            callers.push(item);
        }
        Ok(callers)
    }

    /// Accelerated interface/trait implementor lookup.
    pub fn find_implementors(&self, interface_name: &str) -> Result<Vec<ExtractedImplementor>> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
            SELECT i.interface_name, i.implementor_name, f.path, i.kind, i.definition
            FROM implementors i
            JOIN files f ON i.file_id = f.id
            WHERE i.interface_name = ?1 OR i.interface_name LIKE ?2
            ORDER BY i.implementor_name ASC
            "#,
            )
            .map_err(|e| {
                CoreError::DatabaseError(format!("Failed to prepare find_implementors: {e}"))
            })?;

        let pattern = format!("%{interface_name}%");
        let rows = stmt
            .query_map(params![interface_name, pattern], |row| {
                let iface: String = row.get(0)?;
                let impl_name: String = row.get(1)?;
                let path_str: String = row.get(2)?;
                let kind: String = row.get(3)?;
                let definition: String = row.get(4)?;

                Ok(ExtractedImplementor {
                    interface_name: iface,
                    implementor_name: impl_name,
                    kind,
                    file_path: path_str,
                    definition,
                })
            })
            .map_err(|e| CoreError::DatabaseError(format!("Failed to query implementors: {e}")))?;

        let mut implementors = Vec::new();
        for item in rows.flatten() {
            implementors.push(item);
        }
        Ok(implementors)
    }

    /// Accelerated workspace overview generation from persistent SQLite index in <5ms.
    pub fn get_workspace_overview(
        &self,
        opts: &OverviewOptions,
    ) -> Result<WorkspaceOverviewReport> {
        // 1. Fetch all files
        struct RawFile {
            id: i64,
            path: String,
            language: String,
            total_lines: usize,
            total_tokens: usize,
        }

        let mut stmt_files = self
            .conn
            .prepare(
                "SELECT id, path, language, total_lines, total_tokens FROM files ORDER BY path ASC",
            )
            .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        let file_rows = stmt_files
            .query_map([], |r| {
                Ok(RawFile {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    language: r.get(2)?,
                    total_lines: r.get::<_, i64>(3)? as usize,
                    total_tokens: r.get::<_, i64>(4)? as usize,
                })
            })
            .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        let mut raw_files = Vec::new();
        let mut total_lines = 0;
        let mut total_raw_tokens = 0;
        let mut lang_counts: HashMap<String, (usize, usize, usize)> = HashMap::new();

        for f in file_rows.flatten() {
            total_lines += f.total_lines;
            total_raw_tokens += f.total_tokens;
            let entry = lang_counts.entry(f.language.clone()).or_insert((0, 0, 0));
            entry.0 += 1;
            entry.1 += f.total_lines;
            entry.2 += f.total_tokens;
            raw_files.push(f);
        }

        // 2. Fetch all symbols
        let mut stmt_syms = self
            .conn
            .prepare(
                r#"
            SELECT file_id, name, kind, start_line, end_line, signature, doc_comment
            FROM symbols
            ORDER BY file_id ASC, start_line ASC
            "#,
            )
            .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        let mut file_symbols_map: HashMap<i64, Vec<SymbolOverviewItem>> = HashMap::new();
        let sym_rows = stmt_syms
            .query_map([], |r| {
                let file_id: i64 = r.get(0)?;
                let name: String = r.get(1)?;
                let kind: String = r.get(2)?;
                let start_line: usize = r.get::<_, i64>(3)? as usize;
                let end_line: usize = r.get::<_, i64>(4)? as usize;
                let signature: Option<String> = r.get(5)?;
                let doc_summary: Option<String> = r.get(6)?;

                Ok((
                    file_id,
                    SymbolOverviewItem {
                        name,
                        kind,
                        start_line,
                        end_line,
                        signature,
                        doc_summary,
                    },
                ))
            })
            .map_err(|e| CoreError::DatabaseError(e.to_string()))?;

        let mut total_symbols = 0;
        for item in sym_rows.flatten() {
            total_symbols += 1;
            file_symbols_map.entry(item.0).or_default().push(item.1);
        }

        let mut files = Vec::new();
        for rf in raw_files {
            let symbols = file_symbols_map.remove(&rf.id).unwrap_or_default();
            files.push(FileOverviewItem {
                path: rf.path,
                language: rf.language,
                total_lines: rf.total_lines,
                total_tokens: rf.total_tokens,
                symbols,
            });
        }

        let mut language_breakdown = Vec::new();
        for (lang, (file_count, lines, tokens)) in lang_counts {
            language_breakdown.push(LanguageStatItem {
                language: lang,
                file_count,
                total_lines: lines,
                estimated_tokens: tokens,
            });
        }
        language_breakdown.sort_by_key(|b| std::cmp::Reverse(b.estimated_tokens));

        let mut report = WorkspaceOverviewReport {
            root_path: self.workspace_root.to_string_lossy().to_string(),
            total_files: files.len(),
            total_lines,
            total_raw_tokens,
            total_overview_tokens: 0,
            token_savings_percentage: 0.0,
            total_symbols,
            language_breakdown,
            files,
        };

        let rendered_md = format_overview_markdown(&report);
        let overview_tokens = count_tokens(&rendered_md);
        report.total_overview_tokens = overview_tokens;
        report.token_savings_percentage =
            calculate_savings_percentage(total_raw_tokens, overview_tokens);

        // Budget enforcement
        if let Some(budget) = opts.budget {
            if report.total_overview_tokens > budget {
                for f in &mut report.files {
                    for s in &mut f.symbols {
                        s.doc_summary = None;
                    }
                }
                let pass1_md = format_overview_markdown(&report);
                let pass1_tokens = count_tokens(&pass1_md);
                report.total_overview_tokens = pass1_tokens;
                report.token_savings_percentage =
                    calculate_savings_percentage(total_raw_tokens, pass1_tokens);

                if report.total_overview_tokens > budget {
                    for f in &mut report.files {
                        for s in &mut f.symbols {
                            s.signature = None;
                        }
                    }
                    let pass2_md = format_overview_markdown(&report);
                    let pass2_tokens = count_tokens(&pass2_md);
                    report.total_overview_tokens = pass2_tokens;
                    report.token_savings_percentage =
                        calculate_savings_percentage(total_raw_tokens, pass2_tokens);
                }
            }
        }

        Ok(report)
    }
}
