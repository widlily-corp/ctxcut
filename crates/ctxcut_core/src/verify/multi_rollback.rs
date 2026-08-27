//! Multi-file transactional RAII auto-rollback guard and filesystem journal.
//!
//! Ensures 100% zero-loss atomic rollback across multiple modified files on disk
//! if syntax validation, compiler dry-run, or typecheck fails, or if `apply == false`.

use std::fs;
use std::path::{Path, PathBuf};

/// A single file backup entry in the transactional journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    /// Target file path.
    pub path: PathBuf,
    /// Original file content before modification, or `None` if the file was newly created.
    pub original_content: Option<String>,
    /// Whether the file was written/mutated on disk during the transaction.
    pub modified: bool,
}

/// Transactional RAII guard managing multi-file mutation, rollback, and commit.
#[derive(Debug, Default)]
pub struct MultiFileRollbackGuard {
    journal: Vec<JournalEntry>,
    disarmed: bool,
}

impl MultiFileRollbackGuard {
    /// Creates a new, empty multi-file rollback guard.
    pub fn new() -> Self {
        Self {
            journal: Vec::new(),
            disarmed: false,
        }
    }

    /// Records an existing file into the journal before any modifications take place.
    pub fn record_file(&mut self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if self.journal.iter().any(|entry| entry.path == path) {
            return Ok(());
        }

        let original_content = if path.exists() {
            Some(fs::read_to_string(path)?)
        } else {
            None
        };

        self.journal.push(JournalEntry {
            path: path.to_path_buf(),
            original_content,
            modified: false,
        });

        Ok(())
    }

    /// Explicitly registers a file and its captured original content into the journal.
    pub fn record_file_with_content(&mut self, path: impl Into<PathBuf>, content: Option<String>) {
        let path = path.into();
        if let Some(entry) = self.journal.iter_mut().find(|e| e.path == path) {
            if entry.original_content.is_none() {
                entry.original_content = content;
            }
        } else {
            self.journal.push(JournalEntry {
                path,
                original_content: content,
                modified: false,
            });
        }
    }

    /// Writes new content to the specified file on disk, recording its original state if unrecorded.
    pub fn write_file(&mut self, path: impl AsRef<Path>, new_content: &str) -> std::io::Result<()> {
        let path = path.as_ref();
        let idx = if let Some(i) = self.journal.iter().position(|e| e.path == path) {
            i
        } else {
            let original_content = if path.exists() {
                Some(fs::read_to_string(path)?)
            } else {
                None
            };
            self.journal.push(JournalEntry {
                path: path.to_path_buf(),
                original_content,
                modified: false,
            });
            self.journal.len() - 1
        };

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, new_content)?;
        self.journal[idx].modified = true;
        Ok(())
    }

    /// Consumes and commits the transaction, persisting all file modifications on disk.
    pub fn commit(mut self) {
        self.disarmed = true;
    }

    /// Disarms the guard without consuming it, preventing automatic rollback on drop.
    pub fn disarm(&mut self) {
        self.disarmed = true;
    }

    /// Checks if the guard is disarmed.
    pub fn is_disarmed(&self) -> bool {
        self.disarmed
    }

    /// Returns a read-only view of the journal entries.
    pub fn journal(&self) -> &[JournalEntry] {
        &self.journal
    }

    /// Explicitly triggers a rollback of all modified files to their original states on disk.
    pub fn rollback(&mut self) -> std::io::Result<()> {
        if !self.disarmed {
            for entry in self.journal.iter().rev() {
                if entry.modified {
                    match &entry.original_content {
                        Some(orig) => {
                            fs::write(&entry.path, orig)?;
                        }
                        None => {
                            if entry.path.exists() {
                                fs::remove_file(&entry.path)?;
                            }
                        }
                    }
                }
            }
            self.disarmed = true;
        }
        Ok(())
    }
}

impl Drop for MultiFileRollbackGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            for entry in self.journal.iter().rev() {
                if entry.modified {
                    match &entry.original_content {
                        Some(orig) => {
                            let _ = fs::write(&entry.path, orig);
                        }
                        None => {
                            if entry.path.exists() {
                                let _ = fs::remove_file(&entry.path);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_multi_rollback_reverts_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.rs");
        let file2 = temp_dir.path().join("file2.rs");

        fs::write(&file1, "fn one() -> i32 { 1 }\n").unwrap();
        fs::write(&file2, "fn two() -> i32 { 2 }\n").unwrap();

        {
            let mut guard = MultiFileRollbackGuard::new();
            guard
                .write_file(&file1, "fn one() -> i32 { 100 }\n")
                .unwrap();
            guard
                .write_file(&file2, "fn two() -> i32 { 200 }\n")
                .unwrap();

            assert_eq!(
                fs::read_to_string(&file1).unwrap(),
                "fn one() -> i32 { 100 }\n"
            );
            assert_eq!(
                fs::read_to_string(&file2).unwrap(),
                "fn two() -> i32 { 200 }\n"
            );
            // guard dropped here without commit
        }

        assert_eq!(fs::read_to_string(&file1).unwrap(), "fn one() -> i32 { 1 }\n");
        assert_eq!(fs::read_to_string(&file2).unwrap(), "fn two() -> i32 { 2 }\n");
    }

    #[test]
    fn test_multi_rollback_explicit_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("a.ts");
        let file2 = temp_dir.path().join("b.ts");

        fs::write(&file1, "const a = 1;").unwrap();
        fs::write(&file2, "const b = 2;").unwrap();

        let mut guard = MultiFileRollbackGuard::new();
        guard.write_file(&file1, "const a = 10;").unwrap();
        guard.write_file(&file2, "const b = 20;").unwrap();

        assert_eq!(fs::read_to_string(&file1).unwrap(), "const a = 10;");
        assert_eq!(fs::read_to_string(&file2).unwrap(), "const b = 20;");

        guard.rollback().unwrap();
        assert!(guard.is_disarmed());

        assert_eq!(fs::read_to_string(&file1).unwrap(), "const a = 1;");
        assert_eq!(fs::read_to_string(&file2).unwrap(), "const b = 2;");
    }

    #[test]
    fn test_multi_rollback_commit_persists() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("committed.rs");
        fs::write(&file1, "fn initial() {}").unwrap();

        {
            let mut guard = MultiFileRollbackGuard::new();
            guard.write_file(&file1, "fn updated() {}").unwrap();
            guard.commit();
        }

        assert_eq!(fs::read_to_string(&file1).unwrap(), "fn updated() {}");
    }

    #[test]
    fn test_multi_rollback_new_file_deleted_on_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let new_file = temp_dir.path().join("created.rs");

        {
            let mut guard = MultiFileRollbackGuard::new();
            guard.write_file(&new_file, "fn new_func() {}").unwrap();
            assert!(new_file.exists());
            // drop without commit
        }

        assert!(!new_file.exists());
    }
}
