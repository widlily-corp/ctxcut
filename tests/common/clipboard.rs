//! Clipboard Mock utility for ctxcut headless testing.
//!
//! Provides an in-memory thread-safe mock for clipboard interactions when
//! running tests in headless CI environments without a display server.

use std::sync::{Arc, Mutex};

/// Thread-safe in-memory clipboard mock for test environments.
#[derive(Debug, Clone, Default)]
pub struct ClipboardMock {
    content: Arc<Mutex<Option<String>>>,
}

impl ClipboardMock {
    /// Creates a new empty `ClipboardMock`.
    pub fn new() -> Self {
        Self {
            content: Arc::new(Mutex::new(None)),
        }
    }

    /// Retrieves current text from mock clipboard.
    pub fn get_text(&self) -> Option<String> {
        let guard = self.content.lock().expect("Lock poisoned in ClipboardMock");
        guard.clone()
    }

    /// Sets text into mock clipboard.
    pub fn set_text(&self, text: impl Into<String>) {
        let mut guard = self.content.lock().expect("Lock poisoned in ClipboardMock");
        *guard = Some(text.into());
    }

    /// Clears mock clipboard contents.
    pub fn clear(&self) {
        let mut guard = self.content.lock().expect("Lock poisoned in ClipboardMock");
        *guard = None;
    }

    /// Checks whether mock clipboard is empty or unpopulated.
    pub fn is_empty(&self) -> bool {
        let guard = self.content.lock().expect("Lock poisoned in ClipboardMock");
        guard.as_ref().map_or(true, |s| s.is_empty())
    }

    /// Checks whether mock clipboard text contains the given substring.
    pub fn contains(&self, substr: &str) -> bool {
        let guard = self.content.lock().expect("Lock poisoned in ClipboardMock");
        guard.as_ref().map_or(false, |s| s.contains(substr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_mock_lifecycle() {
        let clip = ClipboardMock::new();
        assert!(clip.is_empty());
        assert_eq!(clip.get_text(), None);

        clip.set_text("# Context Slice\nfn hello() {}");
        assert!(!clip.is_empty());
        assert_eq!(clip.get_text().as_deref(), Some("# Context Slice\nfn hello() {}"));
        assert!(clip.contains("Context Slice"));
        assert!(clip.contains("fn hello()"));
        assert!(!clip.contains("non_existent"));

        clip.clear();
        assert!(clip.is_empty());
        assert_eq!(clip.get_text(), None);
    }
}
