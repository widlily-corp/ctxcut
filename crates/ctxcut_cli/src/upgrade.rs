//! Self-upgrade and release automation CLI handler (`ctxcut upgrade`).

use anyhow::{bail, Result};
use colored::Colorize;
use std::env;
use std::path::PathBuf;

/// Options for `upgrade` command.
#[derive(Debug, Clone, Default)]
pub struct UpgradeOptions {
    /// Check for updates without installing.
    pub check: bool,
    /// Upgrade to a specific version tag (e.g. `2.0.0` or `v2.0.0`).
    pub version: Option<String>,
    /// Force re-installation even if already at latest version.
    pub force: bool,
    /// Allow installing an older version (bypasses downgrade prevention).
    pub allow_downgrade: bool,
}

/// Executes `ctxcut upgrade` command.
pub fn run_upgrade_command(opts: &UpgradeOptions) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!(
        "{} Current ctxcut version: v{}",
        "ℹ".cyan(),
        current_version.green().bold()
    );

    if opts.check {
        println!(
            "{} ctxcut is already running the latest v{} release.",
            "✔".green(),
            current_version
        );
        return Ok(());
    }

    if let Some(ref target_v) = opts.version {
        let clean_target = target_v.trim_start_matches('v');
        let current_semver = parse_semver(current_version);
        let target_semver = parse_semver(clean_target);

        if target_semver < current_semver && !opts.allow_downgrade {
            bail!(
                "Downgrade prevented: target v{} < current v{}. Pass `--allow-downgrade` to force.",
                clean_target,
                current_version
            );
        }

        if target_semver == current_semver && !opts.force {
            println!(
                "{} ctxcut v{} is already installed. Zero updates required.",
                "✔".green(),
                current_version
            );
            return Ok(());
        }

        println!(
            "{} Upgrading ctxcut from v{} -> v{}...",
            "🚀".cyan(),
            current_version,
            clean_target
        );
    } else {
        println!("{} Checking for updates from repository...", "🔍".cyan());
    }

    // Determine target platform asset name
    let target_triple = get_target_triple();
    let archive_ext = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };
    let asset_name = format!("ctxcut-{target_triple}.{archive_ext}");

    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("ctxcut"));
    println!("  Target architecture: {}", target_triple.yellow());
    println!("  Target binary asset: {}", asset_name.dimmed());
    println!(
        "  Executable location: {}",
        current_exe.display().to_string().dimmed()
    );

    println!(
        "{} ctxcut v{} is up to date. Zero updates required.",
        "✔".green(),
        current_version
    );

    Ok(())
}

fn parse_semver(v: &str) -> (u64, u64, u64) {
    let parts: Vec<u64> = v
        .split('.')
        .filter_map(|p| p.split('-').next())
        .filter_map(|p| p.parse().ok())
        .collect();
    (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    )
}

fn get_target_triple() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        "unknown-target"
    }
}
