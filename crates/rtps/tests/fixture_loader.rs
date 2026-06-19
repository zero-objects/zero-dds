//! Fixture loader for compliance vectors (WP 1.10 T1).
//!
//! Hex-Files im Format:
//! ```text
//! # Kommentar
//! DE AD BE EF 01 02  # inline-Kommentar
//! # ...
//! ```
//!
//! Wird via `load_hex()` zu `Vec<u8>` geparst.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::path::{Path, PathBuf};

/// Workspace-Root-Fixtures unter `tests/compliance/`.
pub fn compliance_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("compliance")
}

/// Parses a hex fixture file. Whitespace + `#...` comments are
/// ignoriert; `0x`-Prefix optional; 16 MiB Sanity-Cap.
pub fn load_hex(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    let text = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let stripped = line.split('#').next().unwrap_or("");
        for token in stripped.split_whitespace() {
            let t = token.strip_prefix("0x").unwrap_or(token);
            if t.len() % 2 != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("odd-length hex token '{token}'"),
                ));
            }
            for chunk in t.as_bytes().chunks(2) {
                let s = std::str::from_utf8(chunk).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "non-utf8 hex")
                })?;
                let b = u8::from_str_radix(s, 16).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad hex '{s}'"))
                })?;
                out.push(b);
            }
        }
        if out.len() > 16 * 1024 * 1024 {
            return Err(std::io::Error::other("fixture exceeds 16 MiB"));
        }
    }
    Ok(out)
}

#[test]
fn loader_smoke_test() {
    // Minimal-Fixture inline pruefen.
    let tmp = std::env::temp_dir().join("zerodds-loader-smoke.hex");
    std::fs::write(&tmp, "# hdr\nDE AD BE EF # tail\n0x01 02\n").unwrap();
    let b = load_hex(&tmp).unwrap();
    assert_eq!(b, vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02]);
}

#[test]
fn compliance_root_exists() {
    assert!(compliance_root().is_dir(), "tests/compliance/ must exist");
}
