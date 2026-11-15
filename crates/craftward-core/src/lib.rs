//! Domain logic shared by the Craftward tools.
//!
//! The command-line interface and the desktop application both depend on this
//! crate.

/// Returns the version of the Craftward core library.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_matches_package_metadata() {
        let value = version();
        assert_ne!(value, "");
        assert_eq!(value, env!("CARGO_PKG_VERSION"));
    }
}
