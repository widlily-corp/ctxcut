//! Git Sandbox module for ctxcut E2E test suite.
//!
//! Provides isolated, automated temporary git repository management for testing
//! `ctxcut diff` and `--staged` operations, tracking file modifications, branches,
//! and diff outputs.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Isolated temporary Git repository fixture for testing diff operations.
pub struct GitSandbox {
    dir: TempDir,
}

impl std::fmt::Debug for GitSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitSandbox")
            .field("path", &self.path())
            .finish()
    }
}

impl GitSandbox {
    /// Creates and initializes a new isolated Git repository in a temporary directory.
    ///
    /// Configures `user.name`, `user.email`, `init.defaultBranch = main`, and `commit.gpgsign = false`.
    pub fn new() -> io::Result<Self> {
        let dir = TempDir::new()?;
        let sandbox = Self { dir };

        sandbox.git(&["init", "-b", "main"])?;
        sandbox.git(&["config", "user.name", "ctxcut-test-agent"])?;
        sandbox.git(&["config", "user.email", "test-agent@ctxcut.dev"])?;
        sandbox.git(&["config", "commit.gpgsign", "false"])?;
        sandbox.git(&["config", "core.autocrlf", "false"])?;

        Ok(sandbox)
    }

    /// Returns the root filesystem path of the temporary git sandbox repository.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Resolves a relative path to an absolute path inside the sandbox directory.
    pub fn resolve_path(&self, rel_path: impl AsRef<Path>) -> PathBuf {
        self.dir.path().join(rel_path)
    }

    /// Executes an arbitrary `git` command inside the sandbox directory.
    pub fn git(&self, args: &[&str]) -> io::Result<Output> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.dir.path())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "git command failed: git {}\nStdout: {}\nStderr: {}",
                    args.join(" "),
                    stdout,
                    stderr
                ),
            ));
        }

        Ok(output)
    }

    /// Writes content to a file at `rel_path`, creating any necessary parent directories.
    pub fn write_file(&self, rel_path: impl AsRef<Path>, content: &str) -> io::Result<PathBuf> {
        let full_path = self.resolve_path(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
        Ok(full_path)
    }

    /// Reads the entire contents of a file inside the sandbox.
    pub fn read_file(&self, rel_path: impl AsRef<Path>) -> io::Result<String> {
        let full_path = self.resolve_path(rel_path);
        fs::read_to_string(full_path)
    }

    /// Overwrites the content of an existing file.
    pub fn modify_file(&self, rel_path: impl AsRef<Path>, new_content: &str) -> io::Result<()> {
        let full_path = self.resolve_path(rel_path);
        fs::write(full_path, new_content)
    }

    /// Appends text to an existing file.
    pub fn append_file(&self, rel_path: impl AsRef<Path>, extra_content: &str) -> io::Result<()> {
        let full_path = self.resolve_path(rel_path);
        let mut existing = fs::read_to_string(&full_path)?;
        existing.push_str(extra_content);
        fs::write(full_path, existing)
    }

    /// Deletes a file inside the sandbox.
    pub fn delete_file(&self, rel_path: impl AsRef<Path>) -> io::Result<()> {
        let full_path = self.resolve_path(rel_path);
        fs::remove_file(full_path)
    }

    /// Renames a file using `git mv` or filesystem rename.
    pub fn rename_file(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        let from_str = from.as_ref().to_string_lossy();
        let to_str = to.as_ref().to_string_lossy();
        self.git(&["mv", &from_str, &to_str])?;
        Ok(())
    }

    /// Stages a specific file using `git add <rel_path>`.
    pub fn stage_file(&self, rel_path: impl AsRef<Path>) -> io::Result<()> {
        let path_str = rel_path.as_ref().to_string_lossy();
        self.git(&["add", &path_str])?;
        Ok(())
    }

    /// Stages all modified and new files using `git add -A`.
    pub fn stage_all(&self) -> io::Result<()> {
        self.git(&["add", "-A"])?;
        Ok(())
    }

    /// Unstages all staged changes using `git restore --staged .` or `git reset`.
    pub fn unstage_all(&self) -> io::Result<()> {
        let _ = self.git(&["restore", "--staged", "."])
            .or_else(|_| self.git(&["reset", "HEAD"]));
        Ok(())
    }

    /// Creates a commit with the specified message. Returns commit stdout.
    pub fn commit(&self, message: &str) -> io::Result<String> {
        let output = self.git(&["commit", "-m", message])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Creates and switches to a new Git branch.
    pub fn create_branch(&self, branch_name: &str) -> io::Result<()> {
        self.git(&["checkout", "-b", branch_name])?;
        Ok(())
    }

    /// Switches to an existing Git branch or commit hash.
    pub fn checkout(&self, target: &str) -> io::Result<()> {
        self.git(&["checkout", target])?;
        Ok(())
    }

    /// Returns git diff output as a string.
    ///
    /// If `staged` is true, executes `git diff --staged` (or `--cached`).
    /// Otherwise executes `git diff`.
    pub fn get_diff(&self, staged: bool) -> io::Result<String> {
        let args = if staged {
            vec!["diff", "--staged"]
        } else {
            vec!["diff"]
        };
        let output = self.git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Copies a directory tree from source into the sandbox at `rel_dest`.
    pub fn copy_tree(&self, src_dir: &Path, rel_dest: impl AsRef<Path>) -> io::Result<()> {
        let dest_root = self.resolve_path(rel_dest);
        fs::create_dir_all(&dest_root)?;

        for entry in walk_dir_recursive(src_dir)? {
            let relative = entry.strip_prefix(src_dir).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, format!("Path strip prefix error: {}", e))
            })?;
            let target_path = dest_root.join(relative);

            if entry.is_dir() {
                fs::create_dir_all(&target_path)?;
            } else {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&entry, &target_path)?;
            }
        }

        Ok(())
    }
}

fn walk_dir_recursive(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.push(path.clone());
                files.extend(walk_dir_recursive(&path)?);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_sandbox_lifecycle() -> io::Result<()> {
        let sandbox = GitSandbox::new()?;
        assert!(sandbox.path().exists());

        // Write and stage initial file
        sandbox.write_file("src/math.ts", "export function add(a: number, b: number): number {\n    return a + b;\n}\n")?;
        sandbox.stage_all()?;
        sandbox.commit("Initial commit")?;

        // Modify file unstaged
        sandbox.modify_file("src/math.ts", "export function add(a: number, b: number): number {\n    // modified\n    return a + b;\n}\n")?;

        let unstaged_diff = sandbox.get_diff(false)?;
        assert!(unstaged_diff.contains("+    // modified"));

        let staged_diff = sandbox.get_diff(true)?;
        assert!(staged_diff.is_empty());

        // Stage modification
        sandbox.stage_file("src/math.ts")?;
        let staged_diff_after = sandbox.get_diff(true)?;
        assert!(staged_diff_after.contains("+    // modified"));

        // Commit change
        sandbox.commit("Update math.ts")?;
        let final_diff = sandbox.get_diff(false)?;
        assert!(final_diff.is_empty());

        Ok(())
    }
}
