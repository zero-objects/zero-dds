//! WP 1.10 T3 — XCDR2 Golden-Vector Tests.

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

use std::path::PathBuf;

fn compliance_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("compliance")
}

fn load_hex(path: &std::path::Path) -> Vec<u8> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    for line in text.lines() {
        let stripped = line.split('#').next().unwrap_or("");
        for tok in stripped.split_whitespace() {
            let t = tok.strip_prefix("0x").unwrap_or(tok);
            for chunk in t.as_bytes().chunks(2) {
                let s = std::str::from_utf8(chunk).unwrap();
                out.push(u8::from_str_radix(s, 16).unwrap());
            }
        }
    }
    out
}

#[test]
fn int32_le_golden_vector_decodes() {
    let p = compliance_root().join("xcdr2").join("int32_le.hex");
    let bytes = load_hex(&p);
    assert_eq!(bytes.len(), 4);

    let mut r = zerodds_cdr::BufferReader::new(&bytes, zerodds_cdr::Endianness::Little);
    let v = r.read_u32().unwrap() as i32;
    assert_eq!(v, 0x1234_5678);

    // Roundtrip: re-encode must be byte-identical.
    let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little);
    w.write_u32(0x1234_5678).unwrap();
    assert_eq!(w.into_bytes(), bytes);
}
