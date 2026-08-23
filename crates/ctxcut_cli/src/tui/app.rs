//! Application state machine and business logic for the TUI dashboard.

use arboard::Clipboard;
use ctxcut_core::{
    ContextSlicer, ExecutionTracer, ImpactAnalyzer, ImpactCallerItem,
    OverviewOptions, SliceOptions, SliceResult,
    TelemetryLogger, TelemetrySummary, TraceResult, WorkspaceOverviewGenerator,
};
use std::path::PathBuf;

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
    /// Initializes application state for the given workspace directory.
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

        app.scan_symbols();
        if !app.symbols.is_empty() {
            app.trigger_slice();
        }
        app
    }

    /// Scans workspace directory and extracts all top-level symbols.
    pub fn scan_symbols(&mut self) {
        let opts = OverviewOptions::default();
        if let Ok(report) = WorkspaceOverviewGenerator::generate(&self.workspace_root, &opts) {
            let mut entries = Vec::new();
            for file in report.files {
                let file_path = self.workspace_root.join(&file.path);
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
            self.symbols = entries;
        }

        self.apply_filter();
    }

    /// Updates `filtered_symbols` based on `search_query`.
    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_symbols = (0..self.symbols.len()).collect();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_symbols = self
                .symbols
                .iter()
                .enumerate()
                .filter(|(_, sym)| {
                    sym.symbol_name.to_lowercase().contains(&query_lower)
                        || sym
                            .file_path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&query_lower)
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

    /// Triggers AST slicing on the currently selected symbol.
    pub fn trigger_slice(&mut self) {
        let Some(&sym_idx) = self.filtered_symbols.get(self.selected_symbol_idx) else {
            return;
        };
        let sym = &self.symbols[sym_idx];

        let slicer = ContextSlicer::new();
        let opts = SliceOptions::default();

        let base_name = sym.symbol_name.split('.').next_back().unwrap_or(&sym.symbol_name);

        if let Ok(slice) = slicer.slice_symbol(&sym.file_path, base_name, &opts) {
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
        let base_name = sym.symbol_name.split('.').next_back().unwrap_or(&sym.symbol_name);

        let target_file = Some(sym.file_path.as_path());
        let opts = SliceOptions::default();
        if let Ok(result) = ImpactAnalyzer::find_callers(&self.workspace_root, base_name, target_file, &opts) {
            self.status_message = format!("Found {} upstream callers for `{}`", result.callers.len(), base_name);
            self.current_impact = Some(result.callers);
            self.current_trace = None;
            self.active_pane = ActivePane::Impact;
            self.impact_scroll = 0;
        } else {
            self.status_message = format!("No callers found for `{}`", base_name);
        }
    }

    /// Triggers execution trace from selected symbol.
    pub fn trigger_trace(&mut self) {
        let Some(&sym_idx) = self.filtered_symbols.get(self.selected_symbol_idx) else {
            return;
        };
        let sym = &self.symbols[sym_idx];
        let base_name = sym.symbol_name.split('.').next_back().unwrap_or(&sym.symbol_name);

        let opts = SliceOptions::default();

        if let Ok(trace) = ExecutionTracer::trace(&self.workspace_root, base_name, &opts) {
            self.status_message = format!("Traced {} hops from `{}`", trace.steps.len(), base_name);
            self.current_trace = Some(trace);
            self.current_impact = None;
            self.active_pane = ActivePane::Impact;
            self.impact_scroll = 0;
        } else {
            self.status_message = format!("Trace failed for `{}`", base_name);
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
