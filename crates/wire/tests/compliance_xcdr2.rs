// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The XCDR2 golden vectors in `tests/compliance/xcdr2/`.
//!
//! This test lived in `crates/cdr` until the IDL rebuild removed that
//! crate. The vectors stayed — `tests/compliance/xcdr2/int32_le.hex` is
//! still there and still committed — and nothing read them for as long
//! as the CI job that names the target has been failing on a missing
//! test target rather than on a wrong byte.
//!
//! It sits in `zerodds-wire` because that is where `BufferReader` and
//! `BufferWriter` went: `Reader` and `Writer` are the runtime a
//! generated codec calls, and a golden vector is exactly a statement
//! about what that runtime puts on the wire.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::path::PathBuf;

use zerodds_wire::{Endianness, Reader, Writer};

/// `tests/compliance/` from this crate's manifest directory.
fn compliance_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("the repository root")
        .join("tests")
        .join("compliance")
}

/// The octets a `.hex` vector names, with `#` starting a comment.
///
/// The vectors are written for a person to read — grouped, spaced and
/// annotated with the clause they come from — so the loader takes any
/// whitespace and an optional `0x`, and stops at a `#`.
fn load_hex(path: &std::path::Path) -> Vec<u8> {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut out = Vec::new();
    for line in text.lines() {
        let stripped = line.split('#').next().unwrap_or("");
        for token in stripped.split_whitespace() {
            let t = token.strip_prefix("0x").unwrap_or(token);
            for chunk in t.as_bytes().chunks(2) {
                let s = std::str::from_utf8(chunk).expect("hex is ascii");
                out.push(u8::from_str_radix(s, 16).expect("two hex digits"));
            }
        }
    }
    out
}

/// XTypes 1.3 Annex A: an `int32` is four octets, least significant
/// first under little-endian.
///
/// Both directions, because one alone proves less than it looks: a
/// reader that agrees with a writer that is wrong reads the vector
/// correctly and writes something else. The re-encode has to be
/// byte-identical to the file.
#[test]
fn int32_le_golden_vector_decodes_and_re_encodes() {
    let path = compliance_root().join("xcdr2").join("int32_le.hex");
    let bytes = load_hex(&path);
    assert_eq!(bytes.len(), 4, "the vector is one int32");

    let mut r = Reader::new(&bytes, Endianness::Little);
    let value = r.read_u32().expect("four octets are there") as i32;
    assert_eq!(value, 0x1234_5678, "the value the vector names");

    let mut w = Writer::new(Endianness::Little);
    w.write_u32(0x1234_5678);
    assert_eq!(
        w.finish(),
        bytes,
        "the re-encode is the file, octet for octet"
    );
}
