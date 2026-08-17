//! Integration tests for ctxcut_cli stats calculation and fast token estimation.

use ctxcut_cli::stats::{
    calculate_deep_stats, calculate_fast_stats, calculate_stats, format_stats_text,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_calculate_stats_fast_and_deep_modes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let ts_file = root.join("auth.ts");
    fs::write(
        &ts_file,
        r#"
export interface LoginRequest {
  username: string;
  passwordHash: string;
  clientId: string;
  redirectUri: string;
  scope: string[];
}

export interface AuthToken {
  token: string;
  refreshToken: string;
  expiresIn: number;
  tokenType: string;
}

export interface UserSession {
  sessionId: string;
  userId: string;
  createdAt: number;
  lastActive: number;
  ipAddress: string;
}

export class AuthService {
  private activeTokens: Map<string, UserSession> = new Map();
  private blacklist: Set<string> = new Set();

  public async login(req: LoginRequest): Promise<AuthToken> {
    if (this.blacklist.has(req.username)) {
      throw new Error("User account is suspended");
    }
    const token = "mock_jwt_" + req.username + "_" + Date.now();
    const session: UserSession = {
      sessionId: "sess_" + Date.now(),
      userId: req.username,
      createdAt: Date.now(),
      lastActive: Date.now(),
      ipAddress: "127.0.0.1",
    };
    this.activeTokens.set(token, session);
    return {
      token,
      refreshToken: "refresh_" + token,
      expiresIn: 3600,
      tokenType: "Bearer",
    };
  }

  public async validateToken(token: string): Promise<boolean> {
    const session = this.activeTokens.get(token);
    if (!session) {
      return false;
    }
    session.lastActive = Date.now();
    return true;
  }

  public async logout(token: string): Promise<boolean> {
    return this.activeTokens.delete(token);
  }

  public async revokeAllSessions(userId: string): Promise<number> {
    let revoked = 0;
    for (const [t, s] of this.activeTokens.entries()) {
      if (s.userId === userId) {
        this.activeTokens.delete(t);
        revoked++;
      }
    }
    return revoked;
  }
}
"#,
    )
    .unwrap();

    // 1. Test fast estimation
    let fast_report = calculate_stats(root, true).unwrap();
    assert_eq!(fast_report.total_files, 1);
    assert!(fast_report.total_raw_tokens > 150);
    assert!(fast_report.total_sliced_tokens > 0);
    assert!(fast_report.total_sliced_tokens < fast_report.total_raw_tokens);
    assert!(fast_report.savings_percentage > 0.0);

    // 2. Test deep AST slicing
    let deep_report = calculate_stats(root, false).unwrap();
    assert_eq!(deep_report.total_files, 1);
    assert_eq!(deep_report.total_raw_tokens, fast_report.total_raw_tokens);
    assert!(deep_report.total_sliced_tokens > 0);
    assert!(deep_report.savings_percentage >= 0.0);

    // 3. Test explicit fast/deep functions
    let fast_explicit = calculate_fast_stats(root).unwrap();
    assert_eq!(fast_explicit.total_files, 1);

    let deep_explicit = calculate_deep_stats(root).unwrap();
    assert_eq!(deep_explicit.total_files, 1);
}

#[test]
fn test_stats_single_file_mode() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let py_file = root.join("calc.py");
    fs::write(
        &py_file,
        "def add(a: int, b: int) -> int:\n    return a + b\n\ndef sub(a: int, b: int) -> int:\n    return a - b\n",
    )
    .unwrap();

    let report = calculate_stats(&py_file, true).unwrap();
    assert_eq!(report.total_files, 1);
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].path, py_file.to_string_lossy().to_string());
}

#[test]
fn test_format_stats_text_rendering() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let file = root.join("app.ts");
    fs::write(&file, "export const app = 42;\n").unwrap();

    let report = calculate_stats(root, true).unwrap();
    let formatted = format_stats_text(&report);

    assert!(formatted.contains("ctxcut Token Optimization & Context Statistics"));
    assert!(formatted.contains("Total Files Analyzed:"));
    assert!(formatted.contains("Estimated Savings:"));
}

#[test]
fn test_stats_ignores_blacklisted_directories() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let node_modules = root.join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();
    fs::write(node_modules.join("vendor.ts"), "export const vendor = 1;").unwrap();

    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("built.rs"), "pub const B: u32 = 2;").unwrap();

    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.ts"), "export const main = 3;").unwrap();

    let report = calculate_stats(root, true).unwrap();
    assert_eq!(report.total_files, 1);
    assert!(report.files[0].path.ends_with("main.ts"));
}
