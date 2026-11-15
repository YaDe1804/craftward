//! Percent-encoding for cache file names and URL path segments.
//!
//! Letters, digits, `-`, `_`, and `.` stay as they are, except a name that is
//! only dots or that starts with `.`: those leading dots become `%2E` so a
//! cache path never uses the segments `.` or `..` and never creates a hidden
//! file. Every other byte becomes `%` and two uppercase hex digits. OSV
//! cache names and vulnerability URLs rely on `.` staying visible in the
//! middle of a version such as `1.2.3`. End-of-life product file names also
//! turn remaining `.` into `_`, so a cycle such as `9.3` stays one path
//! segment on disk.

use std::fmt::Write as _;

pub(crate) fn encode(text: &str) -> String {
    let mut out = String::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    if text.starts_with('.') {
        while index < bytes.len() && bytes[index] == b'.' {
            let _ = write!(out, "%2E");
            index += 1;
        }
    }
    for &byte in &bytes[index..] {
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

#[cfg(test)]
mod tests {
    use super::{encode, encode_file_name};

    #[test]
    fn leading_and_dot_only_names_cannot_form_dot_segments() {
        assert_eq!(encode("."), "%2E");
        assert_eq!(encode(".."), "%2E%2E");
        assert_eq!(encode(".hidden"), "%2Ehidden");
        assert_eq!(encode_file_name("."), "%2E");
        assert_eq!(encode_file_name(".."), "%2E%2E");
        assert_eq!(encode_file_name(".hidden"), "%2Ehidden");
        assert_eq!(encode("1.2.3"), "1.2.3");
        assert_eq!(encode_file_name("9.3"), "9_3");
    }
}
