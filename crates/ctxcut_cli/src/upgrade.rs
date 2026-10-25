//! Self-upgrade and release automation CLI handler (`ctxcut upgrade`).

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Options for `upgrade` command.
#[derive(Debug, Clone, Default)]
pub struct UpgradeOptions {
    /// Check for updates without installing.
    pub check: bool,
    /// Upgrade to a specific version tag (e.g. `2.1.0` or `v2.1.0`).
    pub version: Option<String>,
    /// Force re-installation even if already at latest version.
    pub force: bool,
    /// Allow installing an older version (bypasses downgrade prevention).
    pub allow_downgrade: bool,
}

#[derive(serde::Deserialize, Debug)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(serde::Deserialize, Debug)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Executes `ctxcut upgrade` command.
pub fn run_upgrade_command(opts: &UpgradeOptions) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!(
        "{} Current ctxcut version: v{}",
        "ℹ".cyan(),
        current_version.green().bold()
    );

    let target_triple = get_target_triple();
    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("ctxcut"));

    println!("  Target architecture: {}", target_triple.yellow());
    println!(
        "  Executable location: {}",
        current_exe.display().to_string().dimmed()
    );

    // Fetch release information from GitHub API
    let release = fetch_release_info(opts.version.as_deref())?;
    let target_version = release.tag_name.trim_start_matches('v');

    let current_semver = parse_semver(current_version);
    let target_semver = parse_semver(target_version);

    if opts.check {
        if target_semver > current_semver {
            println!(
                "{} An update is available: v{} -> v{}",
                "ℹ".cyan(),
                current_version.yellow(),
                target_version.green().bold()
            );
            println!("  Run `ctxcut upgrade` to install the update.");
        } else {
            println!(
                "{} ctxcut is already running the latest release (v{}).",
                "✔".green(),
                current_version
            );
        }
        return Ok(());
    }

    if target_semver < current_semver && !opts.allow_downgrade {
        bail!(
            "Downgrade prevented: target v{} < current v{}. Pass `--allow-downgrade` to force.",
            target_version,
            current_version
        );
    }

    if target_semver == current_semver && !opts.force {
        println!(
            "{} ctxcut v{} is up to date. Zero updates required. (Pass `--force` to reinstall).",
            "✔".green(),
            current_version
        );
        return Ok(());
    }

    println!(
        "{} Upgrading ctxcut from v{} -> v{}...",
        "🚀".cyan(),
        current_version,
        target_version.green().bold()
    );

    // Locate matching archive asset for current platform
    let archive_ext = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };

    let matching_asset = release.assets.iter().find(|asset| {
        let name = &asset.name;
        name.contains(target_triple)
            && (name.ends_with(archive_ext)
                || (archive_ext == "tar.gz" && name.ends_with(".tar.gz")))
    });

    let Some(asset) = matching_asset else {
        let available: Vec<_> = release.assets.iter().map(|a| a.name.as_str()).collect();
        bail!(
            "Could not find release binary asset for target '{}' in release v{}.\nAvailable assets: {:?}",
            target_triple,
            target_version,
            available
        );
    };

    println!("{} Downloading {}...", "⬇".cyan(), asset.name.yellow());
    let archive_bytes = download_asset(&asset.browser_download_url)?;

    println!(
        "{} Download complete ({} bytes). Extracting binary...",
        "📦".cyan(),
        archive_bytes.len()
    );

    // Extract binary into a temporary file
    let mut temp_file = tempfile::Builder::new()
        .prefix("ctxcut_update_")
        .suffix(if cfg!(target_os = "windows") {
            ".exe"
        } else {
            ""
        })
        .tempfile()
        .context("Failed to create temporary file for update payload")?;

    extract_executable(&archive_bytes, archive_ext, &mut temp_file)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(temp_file.path())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(temp_file.path(), perms)?;
    }

    // Flush temporary file before self_replace
    temp_file.flush()?;

    println!("{} Replacing executable...", "🔄".cyan());
    self_replace::self_replace(temp_file.path()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to replace executable {}: {e}. If installed in a protected directory, rerun with elevated privileges.",
            current_exe.display()
        )
    })?;

    println!(
        "{} Successfully upgraded ctxcut to v{}!",
        "✔".green().bold(),
        target_version.green().bold()
    );

    Ok(())
}

fn fetch_release_info(specific_version: Option<&str>) -> Result<GithubRelease> {
    let api_url = if let Some(v) = specific_version {
        let tag = if v.starts_with('v') {
            v.to_string()
        } else {
            format!("v{v}")
        };
        format!("https://api.github.com/repos/widlily-corp/ctxcut/releases/tags/{tag}")
    } else {
        "https://api.github.com/repos/widlily-corp/ctxcut/releases/latest".to_string()
    };

    println!("{} Querying release from GitHub...", "🔍".cyan());

    let resp = ureq::get(&api_url)
        .set("User-Agent", "ctxcut-upgrade")
        .set("Accept", "application/vnd.github.v3+json")
        .call();

    match resp {
        Ok(r) => {
            let release: GithubRelease = r
                .into_json()
                .context("Failed to parse GitHub release JSON")?;
            Ok(release)
        }
        Err(ureq::Error::Status(404, _)) => {
            bail!("Release not found on GitHub repository widlily-corp/ctxcut.");
        }
        Err(ureq::Error::Status(code, _)) => {
            bail!("GitHub API returned HTTP status {code}. Rate limit may have been reached.");
        }
        Err(e) => {
            bail!("Failed to connect to GitHub releases API: {e}");
        }
    }
}

fn download_asset(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
        .set("User-Agent", "ctxcut-upgrade")
        .call()
        .map_err(|e| anyhow::anyhow!("Failed to download asset from {url}: {e}"))?;

    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .context("Failed while reading asset download stream")?;
    Ok(bytes)
}

fn extract_executable(
    archive_bytes: &[u8],
    archive_ext: &str,
    dest: &mut tempfile::NamedTempFile,
) -> Result<()> {
    if archive_ext == "zip" {
        let cursor = std::io::Cursor::new(archive_bytes);
        let mut zip_archive = zip::ZipArchive::new(cursor)
            .context("Failed to read zip archive from downloaded bytes")?;

        let mut found = false;
        for i in 0..zip_archive.len() {
            let mut file = zip_archive.by_index(i)?;
            let name = file.name().to_string();
            let is_target_bin = name == "ctxcut.exe"
                || name.ends_with("/ctxcut.exe")
                || name.ends_with("\\ctxcut.exe")
                || name == "ctxcut"
                || name.ends_with("/ctxcut")
                || name.ends_with("\\ctxcut");

            if is_target_bin {
                std::io::copy(&mut file, dest)
                    .context("Failed to write extracted binary to temp file")?;
                found = true;
                break;
            }
        }

        if !found {
            bail!("Target executable not found inside zip archive.");
        }
    } else {
        let cursor = std::io::Cursor::new(archive_bytes);
        let gz = flate2::read::GzDecoder::new(cursor);
        let mut tar_archive = tar::Archive::new(gz);

        let mut found = false;
        for entry_res in tar_archive.entries()? {
            let mut entry = entry_res?;
            let path = entry.path()?;
            let is_target_bin = path.file_name().and_then(|n| n.to_str()) == Some("ctxcut");

            if is_target_bin {
                std::io::copy(&mut entry, dest)
                    .context("Failed to write extracted tar binary to temp file")?;
                found = true;
                break;
            }
        }

        if !found {
            bail!("Target executable 'ctxcut' not found inside tar archive.");
        }
    }

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
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "aarch64-pc-windows-msvc"
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
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        "unknown-target"
    }
}
