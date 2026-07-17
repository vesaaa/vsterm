//! Helpers for text that must stay POSIX-safe on remote Linux hosts.
//!
//! Windows checkouts with `core.autocrlf` can bake CR into Rust string literals
//! and embedded assets. Remote `/bin/sh` / bash then fail with `$'\r'` errors.

/// Strip CR so remote shells never see Windows CRLF line endings.
pub(crate) fn normalize_unix_newlines(value: &str) -> String {
    value.replace('\r', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_crlf_and_lone_cr() {
        assert_eq!(normalize_unix_newlines("a\r\nb\r\nc\n"), "a\nb\nc\n");
        assert_eq!(normalize_unix_newlines("do\r\n"), "do\n");
        assert_eq!(normalize_unix_newlines("alone\r"), "alone");
    }
}
