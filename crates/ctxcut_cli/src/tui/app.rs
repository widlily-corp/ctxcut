//! Application state machine and business logic for the TUI dashboard.

use arboard::Clipboard;
use ctxcut_core::{
    ContextSlicer, ExecutionTracer, ImpactAnalyzer, ImpactCallerItem, OverviewOptions,
    SliceOptions, SliceResult, TelemetryLogger, TelemetrySummary, TraceResult,
    WorkspaceOverviewGenerator,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

/// Active pane with keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    /// Workspace symbol navigator list.
    Navigator,
    /// Live AST slice code preview.
    Preview,
    /// Reverse caller and execution impact graph.
    Impact,
    /// Lifetime token and cost savings telemetry.
    Telemetry,
}

impl ActivePane {
    /// Returns next pane in circular focus order.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Navigator => Self::Preview,
            Self::Preview => Self::Impact,
            Self::Impact => Self::Telemetry,
            Self::Telemetry => Self::Navigator,
        }
    }

    /// Returns previous pane in circular focus order.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Navigator => Self::Telemetry,
            Self::Preview => Self::Navigator,
            Self::Impact => Self::Preview,
            Self::Telemetry => Self::Impact,
        }
    }
}

/// A workspace symbol record displayed in the navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    /// Source file path.
    pub file_path: PathBuf,
    /// Symbol name.
    pub symbol_name: String,
    /// Symbol category (function, struct, class, etc.).
    pub kind: String,
    /// Signature summary.
    pub signature: String,
    /// 1-based start line.
    pub line: usize,
}

/// Global interactive TUI application state.
pub struct AppState {
    /// Currently focused pane.
    pub active_pane: ActivePane,
    /// Workspace root directory.
    pub workspace_root: PathBuf,
    /// All discovered symbols in workspace.
    pub symbols: Vec<SymbolEntry>,
    /// Indices into `symbols` matching the search filter.
    pub filtered_symbols: Vec<usize>,
    /// Selected index within `filtered_symbols`.
    pub selected_symbol_idx: usize,
    /// Active search filter string.
    pub search_query: String,
    /// Whether user is currently typing in search input.
    pub is_searching: bool,
    /// Whether background workspace scanning/indexing is in progress.
    pub is_loading: bool,
    /// Background symbol scan channel receiver.
    pub scan_rx: Option<Receiver<Vec<SymbolEntry>>>,
    /// Currently generated AST slice result.
    pub current_slice: Option<SliceResult>,
    /// Currently generated caller impact items.
    pub current_impact: Option<Vec<ImpactCallerItem>>,
    /// Currently generated execution trace result.
    pub current_trace: Option<TraceResult>,
    /// Cached telemetry summary statistics.
    pub telemetry_summary: TelemetrySummary,
    /// Vertical scroll offset for preview panel.
    pub preview_scroll: u16,
    /// Vertical scroll offset for impact panel.
    pub impact_scroll: u16,
    /// Vertical scroll offset for telemetry panel.
    pub telemetry_scroll: u16,
    /// Temporary user feedback status banner message.
    pub status_message: String,
    /// Whether application should exit.
    pub should_quit: bool,
}

impl AppState {
    /// Discovers symbols across the workspace via SQLite index or AST generator.
    pub fn discover_symbols(workspace_root: &Path) -> Vec<SymbolEntry> {
        // 1. Fast-path: query persistent SQLite index (.ctxcut/index.db) if available
        if let Ok(mut engine) = ctxcut_core::IndexEngine::open_or_create(workspace_root) {
            let _ = engine.sync_incremental(&ctxcut_core::IndexOptions::default());

            let query_sql = r#"
                SELECT f.path, s.name, s.kind, s.signature, s.start_line 
                FROM symbols s 
                JOIN files f ON s.file_id = f.id 
                ORDER BY s.name ASC
            "#;

            if let Ok(mut stmt) = engine.connection().prepare(query_sql) {
                let symbol_rows = stmt.query_map([], |row| {
                    let rel_path: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let kind: String = row.get(2)?;
                    let signature: Option<String> = row.get(3)?;
                    let start_line: i64 = row.get(4)?;

                    Ok(SymbolEntry {
                        file_path: workspace_root.join(rel_path),
                        symbol_name: name,
                        kind,
                        signature: signature.unwrap_or_default(),
                        line: start_line as usize,
                    })
                });

                if let Ok(rows) = symbol_rows {
                    let entries: Vec<SymbolEntry> = rows.filter_map(Result::ok).collect();
                    if !entries.is_empty() {
                        return entries;
                    }
                }
            }
        }

        // 2. Fallback: on-the-fly AST overview generation
        let opts = OverviewOptions::default();
        if let Ok(report) = WorkspaceOverviewGenerator::generate(workspace_root, &opts) {
            let mut entries = Vec::new();
            for file in report.files {
                let file_path = workspace_root.join(&file.path);
                for sym in file.symbols {
                    entries.push(SymbolEntry {
                        file_path: file_path.clone(),
                        symbol_name: sym.name,
                        kind: sym.kind,
                        signature: sym.signature.unwrap_or_default(),
                        line: sym.start_line,
                    });
                }
            }
            return entries;
        }

        Vec::new()
    }

    /// Initializes application state for the given workspace directory with instant non-blocking startup.
    pub fn new(workspace_root: PathBuf) -> Self {
        let telemetry_summary = TelemetryLogger::load_summary().unwrap_or_default();

        let mut app = Self {
            active_pane: ActivePane::Navigator,
            workspace_root,
            symbols: Vec::new(),
            filtered_symbols: Vec::new(),
            selected_symbol_idx: 0,
            search_query: String::new(),
            is_searching: false,
            is_loading: true,
            scan_rx: None,
            current_slice: None,
            current_impact: None,
            current_trace: None,
            telemetry_summary,
            preview_scroll: 0,
            impact_scroll: 0,
            telemetry_scroll: 0,
            status_message: "Scanning workspace symbols...".to_string(),
            should_quit: false,
        };

        app.scan_symbols();
        app
    }

    /// Initializes application state with pre-populated symbols (for deterministic testing / instant ready state).
    pub fn new_with_symbols(workspace_root: PathBuf, symbols: Vec<SymbolEntry>) -> Self {
        let telemetry_summary = TelemetryLogger::load_summary().unwrap_or_default();
        let mut app = Self {
            active_pane: ActivePane::Navigator,
            workspace_root,
            symbols,
            filtered_symbols: Vec::new(),
            selected_symbol_idx: 0,
            search_query: String::new(),
            is_searching: false,
            is_loading: false,
            scan_rx: None,
            current_slice: None,
            current_impact: None,
            current_trace: None,
            telemetry_summary,
            preview_scroll: 0,
            impact_scroll: 0,
            telemetry_scroll: 0,
            status_message: String::new(),
            should_quit: false,
        };

        app.apply_filter();
        if !app.symbols.is_empty() {
            app.trigger_slice();
        }
        app
    }

    /// Polls background asynchronous tasks such as symbol discovery.
    pub fn tick(&mut self) {
        if let Some(ref rx) = self.scan_rx {
            match rx.try_recv() {
                Ok(entries) => {
                    self.symbols = entries;
                    self.apply_filter();
                    self.is_loading = false;
                    self.scan_rx = None;
                    if !self.symbols.is_empty() && self.current_slice.is_none() {
                        self.trigger_slice();
                    }
                    if self.symbols.is_empty() {
                        self.status_message = "No symbols found in workspace".to_string();
                    } else {
                        self.status_message =
                            format!("Discovered {} symbols in workspace", self.symbols.len());
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_loading = false;
                    self.scan_rx = None;
                    if self.symbols.is_empty() {
                        self.status_message = "No symbols found in workspace".to_string();
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    /// Starts an asynchronous background workspace scan.
    pub fn scan_symbols(&mut self) {
        self.is_loading = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let ws_root = self.workspace_root.clone();
        std::thread::spawn(move || {
            let entries = Self::discover_symbols(&ws_root);
            let _ = tx.send(entries);
        });
        self.scan_rx = Some(rx);
    }

    /// Synchronously scans the workspace directory.
    pub fn scan_symbols_sync(&mut self) {
        self.symbols = Self::discover_symbols(&self.workspace_root);
        self.is_loading = false;
        self.scan_rx = None;
        self.apply_filter();
        if !self.symbols.is_empty() && self.current_slice.is_none() {
            self.trigger_slice();
        }
    }

    /// Updates `filtered_symbols` based on `search_query`.
    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_symbols = (0..self.symbols.len()).collect();
        } else {
            let query_lower = self.search_query.to_lowercase();
            let ws_root_str = self.workspace_root.to_string_lossy();
            let clean_ws_root = ws_root_str.strip_prefix(r"\\?\").unwrap_or(&ws_root_str);
            let ws_norm = clean_ws_root.replace('\\', "/");
            let ws_norm_trimmed = ws_norm.trim_end_matches('/');

            self.filtered_symbols = self
                .symbols
                .iter()
                .enumerate()
                .filter(|(_, sym)| {
                    let sym_path_str = sym.file_path.to_string_lossy();
                    let clean_sym_path =
                        sym_path_str.strip_prefix(r"\\?\").unwrap_or(&sym_path_str);
                    let sym_norm = clean_sym_path.replace('\\', "/");

                    let rel_path = if sym_norm
                        .to_lowercase()
                        .starts_with(&ws_norm_trimmed.to_lowercase())
                    {
                        sym_norm[ws_norm_trimmed.len()..].trim_start_matches('/')
                    } else {
                        &sym_norm
                    };

                    sym.symbol_name.to_lowercase().contains(&query_lower)
                        || rel_path.to_lowercase().contains(&query_lower)
                        || sym.kind.to_lowercase().contains(&query_lower)
                })
                .map(|(idx, _)| idx)
                .collect();
        }

        if self.selected_symbol_idx >= self.filtered_symbols.len() {
            self.selected_symbol_idx = self.filtered_symbols.len().saturating_sub(1);
        }
    }

    /// Moves cursor down in the current pane.
    pub fn select_next(&mut self) {
        match self.active_pane {
            ActivePane::Navigator => {
                if !self.filtered_symbols.is_empty()
                    && self.selected_symbol_idx + 1 < self.filtered_symbols.len()
                {
                    self.selected_symbol_idx += 1;
                }
            }
            ActivePane::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
            ActivePane::Impact => {
                self.impact_scroll = self.impact_scroll.saturating_add(1);
            }
            ActivePane::Telemetry => {
                self.telemetry_scroll = self.telemetry_scroll.saturating_add(1);
            }
        }
    }

    /// Moves cursor up in the current pane.
    pub fn select_prev(&mut self) {
        match self.active_pane {
            ActivePane::Navigator => {
                self.selected_symbol_idx = self.selected_symbol_idx.saturating_sub(1);
            }
            ActivePane::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            ActivePane::Impact => {
                self.impact_scroll = self.impact_scroll.saturating_sub(1);
            }
            ActivePane::Telemetry => {
                self.telemetry_scroll = self.telemetry_scroll.saturating_sub(1);
            }
        }
    }

    /// Moves cursor down by a page in the current pane.
    pub fn select_page_down(&mut self) {
        match self.active_pane {
            ActivePane::Navigator => {
                if !self.filtered_symbols.is_empty() {
                    self.selected_symbol_idx =
                        (self.selected_symbol_idx + 10).min(self.filtered_symbols.len() - 1);
                }
            }
            ActivePane::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_add(10);
            }
            ActivePane::Impact => {
                self.impact_scroll = self.impact_scroll.saturating_add(10);
            }
            ActivePane::Telemetry => {
                self.telemetry_scroll = self.telemetry_scroll.saturating_add(10);
            }
        }
    }

    /// Moves cursor up by a page in the current pane.
    pub fn select_page_up(&mut self) {
        match self.active_pane {
            ActivePane::Navigator => {
                self.selected_symbol_idx = self.selected_symbol_idx.saturating_sub(10);
            }
            ActivePane::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_sub(10);
            }
            ActivePane::Impact => {
                self.impact_scroll = self.impact_scroll.saturating_sub(10);
            }
            ActivePane::Telemetry => {
                self.telemetry_scroll = self.telemetry_scroll.saturating_sub(10);
            }
        }
    }

    /// Jumps to beginning of current pane.
    pub fn select_first(&mut self) {
        match self.active_pane {
            ActivePane::Navigator => {
                self.selected_symbol_idx = 0;
            }
            ActivePane::Preview => {
                self.preview_scroll = 0;
            }
            ActivePane::Impact => {
                self.impact_scroll = 0;
            }
            ActivePane::Telemetry => {
                self.telemetry_scroll = 0;
            }
        }
    }

    /// Jumps to end of current pane.
    pub fn select_last(&mut self) {
        match self.active_pane {
            ActivePane::Navigator => {
                if !self.filtered_symbols.is_empty() {
                    self.selected_symbol_idx = self.filtered_symbols.len() - 1;
                }
            }
            ActivePane::Preview => {
                self.preview_scroll = u16::MAX;
            }
            ActivePane::Impact => {
                self.impact_scroll = u16::MAX;
            }
            ActivePane::Telemetry => {
                self.telemetry_scroll = u16::MAX;
            }
        }
    }

    /// Triggers AST slicing on the currently selected symbol.
    pub fn trigger_slice(&mut self) {
        let Some(&sym_idx) = self.filtered_symbols.get(self.selected_symbol_idx) else {
            return;
        };
        let sym = &self.symbols[sym_idx];

        let slicer = ContextSlicer::new();
        let opts = SliceOptions::default();

        let base_name = sym
            .symbol_name
            .rsplit("::")
            .next()
            .and_then(|s| s.rsplit('.').next())
            .unwrap_or(&sym.symbol_name);

        let slice_res = slicer
            .slice_symbol(&sym.file_path, &sym.symbol_name, &opts)
            .or_else(|_| slicer.slice_symbol(&sym.file_path, base_name, &opts));

        if let Ok(slice) = slice_res {
            TelemetryLogger::record_slice(&slice, "tui_slice", None);
            self.current_slice = Some(slice);
            self.preview_scroll = 0;
            self.status_message = format!("Generated slice for `{}`", sym.symbol_name);
            self.telemetry_summary = TelemetryLogger::load_summary().unwrap_or_default();
        } else {
            self.status_message = format!("Failed to slice symbol `{}`", sym.symbol_name);
        }
    }

    /// Triggers upstream caller discovery on selected symbol.
    pub fn trigger_impact(&mut self) {
        let Some(&sym_idx) = self.filtered_symbols.get(self.selected_symbol_idx) else {
            return;
        };
        let sym = &self.symbols[sym_idx];
        let base_name = sym
            .symbol_name
            .rsplit("::")
            .next()
            .and_then(|s| s.rsplit('.').next())
            .unwrap_or(&sym.symbol_name);

        let target_file = Some(sym.file_path.as_path());
        let opts = SliceOptions::default();

        let impact_res = ImpactAnalyzer::find_callers(
            &self.workspace_root,
            &sym.symbol_name,
            target_file,
            &opts,
        )
        .or_else(|_| {
            ImpactAnalyzer::find_callers(&self.workspace_root, base_name, target_file, &opts)
        });

        if let Ok(result) = impact_res {
            self.status_message = format!(
                "Found {} upstream callers for `{}`",
                result.callers.len(),
                sym.symbol_name
            );
            self.current_impact = Some(result.callers);
            self.current_trace = None;
            self.active_pane = ActivePane::Impact;
            self.impact_scroll = 0;
        } else {
            self.status_message = format!("No callers found for `{}`", sym.symbol_name);
        }
    }

    /// Triggers execution trace from selected symbol.
    pub fn trigger_trace(&mut self) {
        let Some(&sym_idx) = self.filtered_symbols.get(self.selected_symbol_idx) else {
            return;
        };
        let sym = &self.symbols[sym_idx];
        let base_name = sym
            .symbol_name
            .rsplit("::")
            .next()
            .and_then(|s| s.rsplit('.').next())
            .unwrap_or(&sym.symbol_name);

        let opts = SliceOptions::default();

        let trace_res = ExecutionTracer::trace(&self.workspace_root, &sym.symbol_name, &opts)
            .or_else(|_| ExecutionTracer::trace(&self.workspace_root, base_name, &opts));

        if let Ok(trace) = trace_res {
            self.status_message = format!(
                "Traced {} hops from `{}`",
                trace.steps.len(),
                sym.symbol_name
            );
            self.current_trace = Some(trace);
            self.current_impact = None;
            self.active_pane = ActivePane::Impact;
            self.impact_scroll = 0;
        } else {
            self.status_message = format!("Trace failed for `{}`", sym.symbol_name);
        }
    }

    /// Copies rendered slice Markdown to system clipboard.
    pub fn copy_slice_clipboard(&mut self) {
        let Some(ref slice) = self.current_slice else {
            self.status_message = "No active slice to copy".to_string();
            return;
        };

        let md = slice.to_markdown();
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&md).is_ok() {
                self.status_message = "✔ Sliced Markdown copied to clipboard!".to_string();
                return;
            }
        }
        self.status_message = "Failed to access system clipboard".to_string();
    }

    /// Refreshes workspace symbols and reload telemetry.
    pub fn refresh(&mut self) {
        self.scan_symbols();
        self.telemetry_summary = TelemetryLogger::load_summary().unwrap_or_default();
        self.status_message = "✔ Workspace refreshed".to_string();
    }
}
