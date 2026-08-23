//! RAII auto-rollback guard for safe transactional file modification during verification.

use std::fs;
use std::path::{Path, PathBuf};

/// RAII Guard that ensures the target file is reverted to its original content on drop
/// unless explicitly committed or disarmed.
#[derive(Debug)]
pub struct RollbackGuard {
    path: PathBuf,
    original_content: String,
    disarmed: bool,
}

impl RollbackGuard {
    /// Creates a new rollback guard capturing the original file content.
    pub fn new(path: impl Into<PathBuf>, original_content: String) -> Self {
        Self {
            path: path.into(),
            original_content,
            disarmed: false,
        }
    }

    /// Disarms the guard, allowing changes to remain on disk.
    pub fn disarm(&mut self) {
        self.disarmed = true;
    }

    /// Consumes and commits the guard.
    pub fn commit(mut self) {
        self.disarmed = true;
    }

    /// Checks if the guard has been disarmed.
    pub fn is_disarmed(&self) -> bool {
        self.disarmed
    }

    /// Explicitly triggers rollback and marks guard as disarmed.
    pub fn rollback(&mut self) -> std::io::Result<()> {
        if !self.disarmed {
            fs::write(&self.path, &self.original_content)?;
            self.disarmed = true;
        }
        Ok(())
    }

    /// Returns a reference to the original file content.
    pub fn original_content(&self) -> &str {
        &self.original_content
    }

    /// Returns the target file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RollbackGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = fs::write(&self.path, &self.original_content);
        }
    }
}
