//! Fast binary file detector inspecting leading byte sequences.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Inspects up to 1024 bytes for binary markers (null bytes or invalid UTF-8).
#[must_use]
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    if bytes.contains(&b'\0') {
        return true;
    }
    match std::str::from_utf8(bytes) {
        Ok(_) => false,
        Err(err) => err.error_len().is_some(),
    }
}

/// Reads up to 1024 bytes from a file to detect if it contains binary content.
#[must_use]
pub fn is_binary_file(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 1024];
    let Ok(n) = file.read(&mut buf) else {
        return true;
    };
    is_binary_bytes(&buf[..n])
}
