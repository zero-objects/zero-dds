//! WP 1.10 T5 — QoS-PID-Wire-Golden-Vectors.

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
fn durability_transient_local_golden_vector() {
    let p = compliance_root()
        .join("qos_pid")
        .join("durability_transient_local_le.hex");
    let bytes = load_hex(&p);
    assert_eq!(bytes, [0x01, 0x00, 0x00, 0x00]);

    let mut r = zerodds_cdr::BufferReader::new(&bytes, zerodds_cdr::Endianness::Little);
    let p = zerodds_qos::DurabilityQosPolicy::decode_from(&mut r).unwrap();
    assert_eq!(p.kind, zerodds_qos::DurabilityKind::TransientLocal);

    // Roundtrip.
    let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little);
    p.encode_into(&mut w).unwrap();
    assert_eq!(w.into_bytes(), bytes);
}
