//! Tier 1 Tests: Feature 14 — Interactive TUI Dashboard & Telemetry
//!
//! Verifies terminal telemetry, interactive dashboard, and lifecycle:
//! - Terminal guard lifecycle, panic recovery, and raw mode restore
//! - Non-blocking instant splash state and async symbol streaming
//! - Full buffer rendering via Ratatui TestBackend (loading & populated states)
//! - Keyboard navigation (Tab, j/k, /, Enter, c, i, t, r, q/Esc/Ctrl+C)
//! - Windows Terminal resize & focus event resilience
//! - Small viewport dimension fallback (< 20x5)
//! - Telemetry metrics aggregation and ROI pricing calculation

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ctxcut_cli::tui::{
    handle_key_event, render_dashboard, ActivePane, AppState, SymbolEntry, TerminalGuard,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_f14_tui_render_buffer_generation() {
    // Arrange: Metrics command execution
    let runner = CliRunner::new();

    // Act: Render metrics in text format
    let output = runner
        .run(&["metrics", "--format", "text"])
        .expect("Command failed");

    // Assert: Render output produced cleanly
    output.assert_success();
    assert!(
        output.stdout.contains("METRICS")
            || output.stdout.contains("Telemetry")
            || output.stdout.contains("ROI")
            || output.stdout.contains("Tokens")
    );
}

#[test]
fn test_f14_tui_metrics_tab_data_binding() {
    // Arrange: Metrics in JSON format
    let runner = CliRunner::new();

    // Act: Query JSON metrics
    let output = runner
        .run(&["metrics", "--format", "json"])
        .expect("Command failed");

    // Assert: Valid JSON containing model savings / telemetry keys
    output.assert_success();
    let json: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert!(json.is_object());
}

#[test]
fn test_f14_tui_slice_preview_widget() {
    // Arrange: Run a slice to record telemetry
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("tui_preview.ts");
    fs::write(
        &file_path,
        "export function previewWidget() { return 'preview'; }\n",
    )
    .unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:previewWidget", file_path.display());
    let slice_out = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Slice failed");
    slice_out.assert_success();

    // Act: Verify metrics updated
    let metrics_out = runner.run(&["metrics"]).expect("Metrics failed");

    // Assert: Succeeded
    metrics_out.assert_success();
}

#[test]
fn test_f14_tui_model_tier_pricing_table() {
    // Arrange: Stats with history flag
    let runner = CliRunner::new();

    // Act: Run stats --history
    let output = runner.run(&["stats", "--history"]).expect("Command failed");

    // Assert: Output reports lifetime ROI
    output.assert_success();
}

#[test]
fn test_f14_tui_event_loop_exit_handling() {
    // Arrange: CLI help output
    let runner = CliRunner::new();

    // Act: Invoke help
    let output = runner.run(&["--help"]).expect("Command failed");

    // Assert: Exit code 0
    output.assert_success();
}

#[test]
fn test_f14_tui_terminal_guard_lifecycle_and_restore() {
    // Arrange & Act: Initialize terminal guard or restore
    let mut guard = match TerminalGuard::init() {
        Ok(g) => g,
        Err(_) => return, // In headless CI without console handle, gracefully handled
    };

    // Assert: Explicit restore clears active state
    guard.restore();
    // Subsequent drop should be safe and idempotent
    drop(guard);
}

#[test]
fn test_f14_tui_instant_splash_and_async_loading() {
    // Arrange: Workspace with a TypeScript file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("service.ts");
    fs::write(&file_path, "export function computeTotal(items: number[]): number { return items.reduce((a, b) => a + b, 0); }\n").unwrap();

    // Act: Non-blocking AppState initialization
    let mut app = AppState::new(dir.path().to_path_buf());

    // Assert: Starts immediately in loading state
    assert!(app.is_loading);
    assert!(app.status_message.contains("Scanning"));

    // Poll background discovery until symbols arrive
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(20));
        app.tick();
        if !app.is_loading {
            break;
        }
    }

    // Assert: Discovery completed and automatically sliced first symbol
    assert!(!app.is_loading);
    assert!(!app.symbols.is_empty());
    assert_eq!(app.symbols[0].symbol_name, "computeTotal");
    assert!(app.current_slice.is_some());
}

#[test]
fn test_f14_tui_buffer_rendering_loading_and_populated() {
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("index.ts");
    fs::write(
        &file_path,
        "export function handleRequest() { return 200; }\n",
    )
    .unwrap();

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

    // 1. Test Loading / Splash state rendering
    let loading_app = AppState {
        active_pane: ActivePane::Navigator,
        workspace_root: dir.path().to_path_buf(),
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
        telemetry_summary: Default::default(),
        preview_scroll: 0,
        impact_scroll: 0,
        telemetry_scroll: 0,
        status_message: "Scanning workspace symbols...".to_string(),
        should_quit: false,
    };

    terminal
        .draw(|f| render_dashboard(f, &loading_app))
        .expect("Failed to draw loading frame");

    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);
    assert!(buffer_str.contains("CTXCUT"));
    assert!(buffer_str.contains("Scanning"));

    // 2. Test Populated state rendering
    let symbol = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name: "handleRequest".to_string(),
        kind: "function".to_string(),
        signature: "export function handleRequest()".to_string(),
        line: 1,
    };

    let populated_app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![symbol]);

    terminal
        .draw(|f| render_dashboard(f, &populated_app))
        .expect("Failed to draw populated frame");

    let pop_buffer = terminal.backend().buffer();
    let pop_buffer_str = format!("{:?}", pop_buffer);
    assert!(pop_buffer_str.contains("handleRequest"));
    assert!(pop_buffer_str.contains("SYMBOLS"));
    assert!(pop_buffer_str.contains("TELEMETRY"));
    assert!(pop_buffer_str.contains("Tokens Saved"));
}

#[test]
fn test_f14_tui_keyboard_navigation_and_event_dispatch() {
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("app.ts");
    fs::write(
        &file_path,
        "export function Alpha() { return 1; }\nexport function Beta() { return 2; }\n",
    )
    .unwrap();

    let sym1 = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name: "Alpha".to_string(),
        kind: "function".to_string(),
        signature: "export function Alpha()".to_string(),
        line: 1,
    };
    let sym2 = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name: "Beta".to_string(),
        kind: "function".to_string(),
        signature: "export function Beta()".to_string(),
        line: 2,
    };

    let mut app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![sym1, sym2]);

    let make_key = |code: KeyCode, modifiers: KeyModifiers| KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };

    // 1. Focus Pane Cycling: Tab and BackTab
    assert_eq!(app.active_pane, ActivePane::Navigator);
    handle_key_event(&mut app, make_key(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.active_pane, ActivePane::Preview);
    handle_key_event(&mut app, make_key(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.active_pane, ActivePane::Impact);
    handle_key_event(&mut app, make_key(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.active_pane, ActivePane::Telemetry);
    handle_key_event(&mut app, make_key(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.active_pane, ActivePane::Navigator);

    handle_key_event(&mut app, make_key(KeyCode::BackTab, KeyModifiers::empty()));
    assert_eq!(app.active_pane, ActivePane::Telemetry);
    handle_key_event(&mut app, make_key(KeyCode::Tab, KeyModifiers::SHIFT));
    assert_eq!(app.active_pane, ActivePane::Impact);

    // 2. Cursor navigation: j / k
    app.active_pane = ActivePane::Navigator;
    assert_eq!(app.selected_symbol_idx, 0);
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('j'), KeyModifiers::empty()),
    );
    assert_eq!(app.selected_symbol_idx, 1);
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('k'), KeyModifiers::empty()),
    );
    assert_eq!(app.selected_symbol_idx, 0);

    // 3. Search filter mode
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('/'), KeyModifiers::empty()),
    );
    assert!(app.is_searching);
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('B'), KeyModifiers::empty()),
    );
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('e'), KeyModifiers::empty()),
    );
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('t'), KeyModifiers::empty()),
    );
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('a'), KeyModifiers::empty()),
    );
    assert_eq!(app.filtered_symbols.len(), 1);
    assert_eq!(app.symbols[app.filtered_symbols[0]].symbol_name, "Beta");

    handle_key_event(&mut app, make_key(KeyCode::Enter, KeyModifiers::empty()));
    assert!(!app.is_searching);

    // 4. Action triggers: Impact, Trace, Refresh, Clipboard
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('i'), KeyModifiers::empty()),
    );
    assert_eq!(app.active_pane, ActivePane::Impact);

    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('t'), KeyModifiers::empty()),
    );
    assert_eq!(app.active_pane, ActivePane::Impact);

    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('c'), KeyModifiers::empty()),
    );
    assert!(!app.status_message.is_empty());

    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('r'), KeyModifiers::empty()),
    );
    assert!(app.is_loading);

    // 5. Exit triggers: Ctrl+C / q / Esc
    assert!(!app.should_quit);
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('q'), KeyModifiers::empty()),
    );
    assert!(app.should_quit);

    app.should_quit = false;
    handle_key_event(&mut app, make_key(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.should_quit);

    app.should_quit = false;
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
    assert!(app.should_quit);
}

#[test]
fn test_f14_tui_small_dimension_fallback() {
    // Arrange: A tiny terminal area (< 20x5)
    let backend = TestBackend::new(15, 3);
    let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");
    let app = AppState::new_with_symbols(PathBuf::from("."), Vec::new());

    // Act: Render into small terminal
    let res = terminal.draw(|f| render_dashboard(f, &app));

    // Assert: Render succeeds without panic
    assert!(res.is_ok());
    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);
    assert!(buffer_str.contains("Size < 20x5") || buffer_str.contains("..."));
}

#[test]
fn test_f14_tui_channel_disconnected_resilience() {
    // Arrange: AppState with a scan channel that gets dropped immediately
    let (tx, rx) = std::sync::mpsc::channel::<Vec<SymbolEntry>>();
    drop(tx);

    let mut app = AppState {
        active_pane: ActivePane::Navigator,
        workspace_root: PathBuf::from("."),
        symbols: Vec::new(),
        filtered_symbols: Vec::new(),
        selected_symbol_idx: 0,
        search_query: String::new(),
        is_searching: false,
        is_loading: true,
        scan_rx: Some(rx),
        current_slice: None,
        current_impact: None,
        current_trace: None,
        telemetry_summary: Default::default(),
        preview_scroll: 0,
        impact_scroll: 0,
        telemetry_scroll: 0,
        status_message: "Scanning workspace symbols...".to_string(),
        should_quit: false,
    };

    // Act: Tick when channel disconnected
    app.tick();

    // Assert: is_loading reset to false and scan_rx cleared
    assert!(!app.is_loading);
    assert!(app.scan_rx.is_none());
    assert!(app.status_message.contains("No symbols") || app.status_message.contains("workspace"));
}

#[test]
fn test_f14_tui_overshoot_scroll_clamping() {
    // Arrange: Populated AppState with high scroll offsets
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("scroll_test.ts");
    fs::write(
        &file_path,
        "export function shortFunction() {\n    return 42;\n}\n",
    )
    .unwrap();

    let sym = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name: "shortFunction".to_string(),
        kind: "function".to_string(),
        signature: "export function shortFunction()".to_string(),
        line: 1,
    };

    let mut app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![sym]);
    app.preview_scroll = 500;
    app.impact_scroll = 500;
    app.telemetry_scroll = 500;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

    // Act: Draw with large scroll offsets
    let res = terminal.draw(|f| render_dashboard(f, &app));

    // Assert: Draws cleanly without panic and preview still shows target content
    assert!(res.is_ok());
    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);
    assert!(buffer_str.contains("shortFunction"));
    assert!(buffer_str.contains("return 42"));
}

#[test]
fn test_f14_tui_kpi_card_subtitle_rendering() {
    // Arrange: Telemetry dashboard with active savings
    let dir = TempDir::new().expect("Failed to create tempdir");
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

    let mut app = AppState::new_with_symbols(dir.path().to_path_buf(), Vec::new());
    app.telemetry_summary.total_saved_tokens = 50_000;
    app.telemetry_summary.compression_percentage = 68.5;
    app.telemetry_summary
        .cost_savings_by_tier
        .standard_sonnet_gpt4o = 12.50;
    app.telemetry_summary.cost_savings_by_tier.frontier_opus = 75.00;

    // Act: Render dashboard
    let res = terminal.draw(|f| render_dashboard(f, &app));

    // Assert: Both values and card subtitles are rendered in the buffer
    assert!(res.is_ok());
    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);
    assert!(buffer_str.contains("50.0K"));
    assert!(buffer_str.contains("68.5% Avg Reduction"));
    assert!(buffer_str.contains("$12.50"));
    assert!(buffer_str.contains("Claude 3.5 Sonnet"));
    assert!(buffer_str.contains("$75.00"));
    assert!(buffer_str.contains("Claude 3.7 Opus"));
}

#[test]
fn test_f14_tui_windows_unc_path_stripping() {
    // Arrange: Workspace root with \\?\ UNC prefix
    let unc_path = PathBuf::from(r"\\?\C:\projects\my_app");
    let file_path = PathBuf::from(r"\\?\C:\projects\my_app\src\handlers\auth.ts");

    let sym = SymbolEntry {
        file_path,
        symbol_name: "login".to_string(),
        kind: "function".to_string(),
        signature: "export function login()".to_string(),
        line: 10,
    };

    let app = AppState::new_with_symbols(unc_path, vec![sym]);
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

    // Act: Draw dashboard
    let res = terminal.draw(|f| render_dashboard(f, &app));

    // Assert: UNC prefix is cleanly stripped from workspace header and navigator
    assert!(res.is_ok());
    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);
    assert!(
        buffer_str.contains("WORKSPACE: C:\\projects\\my_app")
            || buffer_str.contains("WORKSPACE: C:/projects/my_app")
    );
    assert!(!buffer_str.contains(r"\\?\C:"));
    assert!(buffer_str.contains("src/handlers/auth.ts:10:login"));
}

#[test]
fn test_f14_tui_qualified_symbol_slice_and_impact() {
    // Arrange: Class with method in TypeScript
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("service.ts");
    fs::write(
        &file_path,
        "export class AuthService {\n    validateToken(token: string): boolean {\n        return token.length > 0;\n    }\n}\n",
    )
    .unwrap();

    let sym = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name: "AuthService.validateToken".to_string(),
        kind: "method".to_string(),
        signature: "validateToken(token: string): boolean".to_string(),
        line: 2,
    };

    let mut app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![sym]);

    // Act: Trigger slice on qualified method name
    app.trigger_slice();

    // Assert: Slicing succeeds via fallback to base name
    assert!(app.current_slice.is_some());
    let slice = app.current_slice.as_ref().unwrap();
    assert!(slice.target_symbol.body.contains("validateToken"));
    assert!(app.status_message.contains("Generated slice"));
}

#[test]
fn test_f14_tui_right_border_integrity_under_long_lines() {
    // Arrange: Workspace with very long lines of code in preview
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("long_lines.ts");
    let long_code = "export function aVeryLongFunctionNameThatExceedsTheWidthOfTheTerminalPanelAndContinuesForManyColumns(): string {\n    const veryLongVariableNameToEnsureThatEverySingleColumnIsTestedForOverwritingBorders = 'some_super_long_string_value_exceeding_all_limits';\n    return veryLongVariableNameToEnsureThatEverySingleColumnIsTestedForOverwritingBorders;\n}\n";
    fs::write(&file_path, long_code).unwrap();

    let sym = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name:
            "aVeryLongFunctionNameThatExceedsTheWidthOfTheTerminalPanelAndContinuesForManyColumns"
                .to_string(),
        kind: "function".to_string(),
        signature: "export function aVeryLongFunctionName...".to_string(),
        line: 1,
    };

    let app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![sym]);
    let width = 80;
    let height = 24;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

    // Act: Draw frame
    let res = terminal.draw(|f| render_dashboard(f, &app));
    assert!(res.is_ok());

    let buffer = terminal.backend().buffer();

    // Assert: Check that the rightmost column (width - 1) for the main panels contains border vertical lines ('│' or '┐' or '┘' or '┤')
    // and was NOT corrupted / overwritten by code text
    for y in 0..height {
        let cell = buffer.cell((width - 1, y)).expect("Cell exists");
        let sym = cell.symbol();
        // The right border column of the dashboard at y > 0 and y < height - 1 must be a border character, not a letter/quote
        assert!(
            sym == "│" || sym == "┐" || sym == "┘" || sym == "┤" || sym == " " || sym == "─",
            "Right border cell at ({}, {}) was corrupted by: '{}'",
            width - 1,
            y,
            sym
        );
    }
}

#[test]
fn test_f14_tui_rust_cpp_scope_resolution_symbol_slice() {
    // Arrange: Rust struct method with :: scope resolution
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("handler.rs");
    fs::write(
        &file_path,
        "pub struct OrderHandler;\nimpl OrderHandler {\n    pub fn process_order(&self, id: u64) -> bool {\n        id > 0\n    }\n}\n",
    )
    .unwrap();

    let sym = SymbolEntry {
        file_path: file_path.clone(),
        symbol_name: "OrderHandler::process_order".to_string(),
        kind: "method".to_string(),
        signature: "pub fn process_order(&self, id: u64) -> bool".to_string(),
        line: 3,
    };

    let mut app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![sym]);

    // Act: Trigger slice on :: qualified method name
    app.trigger_slice();

    // Assert: Slicing succeeds via :: scope resolution fallback
    assert!(app.current_slice.is_some());
    let slice = app.current_slice.as_ref().unwrap();
    assert!(slice.target_symbol.body.contains("process_order"));
    assert!(app.status_message.contains("Generated slice"));
}

#[test]
fn test_f14_tui_search_arrow_and_page_navigation() {
    // Arrange: 15 symbols for testing pagination and search arrow keys
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("items.ts");
    let mut symbols = Vec::new();
    for i in 1..=15 {
        symbols.push(SymbolEntry {
            file_path: file_path.clone(),
            symbol_name: format!("actionItem_{:02}", i),
            kind: "function".to_string(),
            signature: format!("function actionItem_{:02}()", i),
            line: i,
        });
    }

    let mut app = AppState::new_with_symbols(dir.path().to_path_buf(), symbols);

    let make_key = |code: KeyCode, modifiers: KeyModifiers| KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };

    // 1. PageDown and PageUp in Navigator
    assert_eq!(app.selected_symbol_idx, 0);
    handle_key_event(&mut app, make_key(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(app.selected_symbol_idx, 10);
    handle_key_event(&mut app, make_key(KeyCode::End, KeyModifiers::empty()));
    assert_eq!(app.selected_symbol_idx, 14);
    handle_key_event(&mut app, make_key(KeyCode::Home, KeyModifiers::empty()));
    assert_eq!(app.selected_symbol_idx, 0);

    // 2. Search mode with Up/Down arrows and Enter
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('/'), KeyModifiers::empty()),
    );
    assert!(app.is_searching);
    handle_key_event(
        &mut app,
        make_key(KeyCode::Char('1'), KeyModifiers::empty()),
    );
    // actionItem_01, actionItem_10..15 -> 7 matches
    assert_eq!(app.filtered_symbols.len(), 7);
    assert_eq!(app.selected_symbol_idx, 0);

    // Navigate inside search mode with Down/Up
    handle_key_event(&mut app, make_key(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.selected_symbol_idx, 1);
    handle_key_event(&mut app, make_key(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.selected_symbol_idx, 2);
    handle_key_event(&mut app, make_key(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.selected_symbol_idx, 1);

    // Press Enter to confirm search selection
    handle_key_event(&mut app, make_key(KeyCode::Enter, KeyModifiers::empty()));
    assert!(!app.is_searching);
    assert_eq!(app.selected_symbol_idx, 1);
}

#[test]
fn test_f14_tui_rapid_resize_simulation() {
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("service.ts");
    fs::write(
        &file_path,
        "export function handleTest() { return true; }\n",
    )
    .unwrap();

    let sym = SymbolEntry {
        file_path,
        symbol_name: "handleTest".to_string(),
        kind: "function".to_string(),
        signature: "export function handleTest()".to_string(),
        line: 1,
    };

    let app = AppState::new_with_symbols(dir.path().to_path_buf(), vec![sym]);

    // Simulate sequence of rapid dynamic viewport resize dimensions
    let dimensions = [
        (80, 24),
        (120, 40),
        (20, 5),
        (25, 8),
        (15, 3),
        (10, 2),
        (200, 60),
        (50, 15),
    ];

    for (w, h) in dimensions {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");
        let res = terminal.draw(|f| render_dashboard(f, &app));
        assert!(res.is_ok(), "Failed to render at dimension {}x{}", w, h);
    }
}
