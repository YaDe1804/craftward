//! Percent-encoding for cache file names and URL path segments.
//!
//! Letters, digits, `-`, `_`, and `.` stay as they are. Every other byte
//! becomes `%` and two uppercase hex digits. OSV cache names and vulnerability
//! URLs rely on `.` staying visible, for example in a version such as `1.2.3`.
//! End-of-life product file names also turn `.` into `_`, so a cycle such as
//! `9.3` stays one path segment on disk.

use std::fmt::Write as _;

pub(crate) fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// Like [`encode`], then replaces `.` with `_` for a single file-name segment.
pub(crate) fn encode_file_name(text: &str) -> String {
    encode(text).replace('.', "_")
}
