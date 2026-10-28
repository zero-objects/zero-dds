// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The read direction under the settings the corpus run never reaches.
//!
//! `tools/wireprobe/wireprobe-zerodds-c-corpus --roundtrip` walks every
//! case in five encodings and both byte orders, and it walks all of them
//! under the **default** dialect. Four wire questions therefore never
//! get their other answer in that run: how a delimited `map` states its
//! entry count, which of the three `wstring` length conventions is on
//! the wire, whether the payload carries its encapsulation header, and
//! which peer's PL_CDR1 framing the parameters take.
//!
//! Each of those is a branch in `crates/lang-rules/src/emit/decode.rs` or
//! in `runtime/c`, and a branch nothing runs is a branch that is wrong
//! whenever somebody finally does run it. `rule-15-omits` is the
//! sharpest of them: a delimited map with no count has to be read
//! twice, once to count the entries and once into storage sized from
//! that count, and the counting pass has to leave the reader — cursor,
//! origin, status and heap — exactly where it found it.
//!
//! The bytes come from the Rust encoder through the library rather than
//! from a recorded string: what is under test is the C reader against
//! the setting, and a hand-copied payload would only add a place to
//! mistype one.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::value::ProbeRequest;

fn probe() -> (PathBuf, PathBuf) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = manifest
        .parent()
        .expect("tools/wireprobe")
        .join("wireprobe-zerodds-c");
    (
        script,
        PathBuf::from(env!("CARGO_BIN_EXE_wireprobe-zerodds-c-gen")),
    )
}

fn skip() -> bool {
    let (script, _) = probe();
    if !Path::new(&script).exists() {
        eprintln!("wireprobe-zerodds-c is missing; the C column is not measured here");
        return true;
    }
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    if !Command::new(cc)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        eprintln!("no C compiler; the C column is not measured here");
        return true;
    }
    false
}

macro_rules! guard {
    () => {
        if skip() {
            return;
        }
    };
}

/// The Rust probe writes under these settings, the C probe reads, and
/// the values are compared against the sample that went in.
fn assert_round_trip(
    idl: &str,
    type_name: &str,
    sample: &str,
    request: &ProbeRequest,
    flags: &[&str],
) {
    let ast = zerodds_wire_probe::encode::model_from_idl(idl, "policy.idl").expect("the IDL");
    let value: serde_json::Value = serde_json::from_str(sample).expect("the sample parses");
    let bytes = zerodds_wire_probe::encode::encode(&ast, request, &value)
        .unwrap_or_else(|error| panic!("{type_name}: the sample did not serialise: {error}"));

    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-c-policy-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl_path = dir.join("type.idl");
    let payload = dir.join("payload.bin");
    std::fs::write(&idl_path, idl).expect("the idl");
    std::fs::write(&payload, &bytes).expect("the payload");

    let (script, generator) = probe();
    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .arg("--idl")
        .arg(&idl_path)
        .arg("--type")
        .arg(type_name)
        .arg("--in")
        .arg(&payload)
        .args(flags)
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "{type_name} under {flags:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            type_name.to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type is in the model");
    let expected = zerodds_wire_probe::canonical::canonicalise(&ast, type_, &value, type_name)
        .expect("the sample is a value of this type");
    let found = zerodds_wire_probe::canonical::canonicalise(
        &ast,
        type_,
        &serde_json::from_str(&printed)
            .unwrap_or_else(|error| panic!("the probe printed `{printed}`: {error}")),
        type_name,
    )
    .unwrap_or_else(|error| panic!("the probe printed `{printed}`: {error}"));
    if let Some(first) = zerodds_wire_probe::canonical::difference(&expected, &found, type_name) {
        panic!("{type_name} under {flags:?}: {first}");
    }
}

#[test]
fn a_delimited_map_with_no_entry_count_reads_back() {
    guard!();
    // Rule (15) of §7.4.3.5.3 writes the DHEADER and the pairs with no
    // count, where §7.4.1.1.4's equivalence to a sequence gives it both.
    // Neither can be ruled out from the stream alone, so the caller
    // chooses — and the count-less reading is the one that costs the
    // decoder a second pass.
    let idl = "@final struct WithMap { map<long, double> table; long tail; };";
    let sample = r#"{ "table": [[7, 1.5], [-3, -0.25], [2147483647, 2.0]], "tail": -1 }"#;
    for endianness in [Endianness::Little, Endianness::Big] {
        for encoding in [Encoding::DelimitedCdr, Encoding::PlCdr2] {
            let mut request = ProbeRequest::new("WithMap", encoding, endianness);
            request.map_length = zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits;
            assert_round_trip(
                idl,
                "WithMap",
                sample,
                &request,
                &[
                    "--encoding",
                    match encoding {
                        Encoding::DelimitedCdr => "delimited-cdr",
                        _ => "pl-cdr2",
                    },
                    "--endian",
                    match endianness {
                        Endianness::Little => "le",
                        Endianness::Big => "be",
                    },
                    "--map-length",
                    "rule-15-omits",
                ],
            );
        }
    }
}

#[test]
fn a_map_whose_entries_hold_strings_reads_back_with_no_count() {
    guard!();
    // The counting pass allocates out of the same heap the second pass
    // uses. A rewind that put the cursor back and left the blocks
    // behind would run the heap down by one map's worth per pass, and
    // the second pass would not fit the payload the first one did.
    let idl = "@final struct WithMap { map<long, string> table; long tail; };";
    let sample = r#"{ "table": [[1, "über"], [2, ""], [3, "abc"]], "tail": 7 }"#;
    let mut request = ProbeRequest::new("WithMap", Encoding::DelimitedCdr, Endianness::Little);
    request.map_length = zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits;
    assert_round_trip(
        idl,
        "WithMap",
        sample,
        &request,
        &[
            "--encoding",
            "delimited-cdr",
            "--endian",
            "le",
            "--map-length",
            "rule-15-omits",
        ],
    );
}

#[test]
fn each_wstring_length_convention_reads_back() {
    guard!();
    // Two of the three measured implementations write something other
    // than §7.4.1.1.2's byte count, and the corpus run only ever uses
    // the default. A supplementary code point is left out of the
    // character-count sample on purpose: that convention cannot carry
    // one and the writer refuses it rather than truncating.
    let idl = "@final struct W { wstring text; long tail; };";
    for (policy, sample) in [
        ("specification", r#"{ "text": "äZ𝄞", "tail": -1 }"#),
        ("character-count", r#"{ "text": "äZ", "tail": -1 }"#),
        (
            "character-count-terminated",
            r#"{ "text": "äZ𝄞", "tail": -1 }"#,
        ),
    ] {
        for endianness in [Endianness::Little, Endianness::Big] {
            let mut request = ProbeRequest::new("W", Encoding::PlainCdr1, endianness);
            request.wstring_length =
                zerodds_ast_layout::WstringLengthPolicy::parse(policy).expect("a policy");
            assert_round_trip(
                idl,
                "W",
                sample,
                &request,
                &[
                    "--encoding",
                    "xcdr1",
                    "--endian",
                    match endianness {
                        Endianness::Little => "le",
                        Endianness::Big => "be",
                    },
                    "--wstring-length",
                    policy,
                ],
            );
        }
    }
}

#[test]
fn a_payload_behind_its_encapsulation_header_reads_back() {
    guard!();
    // The four bytes are outside the CDR stream, so alignment inside
    // the payload still counts from zero. A reader that took the header
    // for part of the stream would misalign every eight-byte member.
    let idl = "@final struct P { long x; double y; };";
    let sample = r#"{ "x": -1234567, "y": 3.5 }"#;
    let mut request = ProbeRequest::new("P", Encoding::PlainCdr1, Endianness::Big);
    request.emit = zerodds_wire_probe::value::Emit::PayloadWithEncapsulation;
    assert_round_trip(
        idl,
        "P",
        sample,
        &request,
        &[
            "--encoding",
            "xcdr1",
            "--endian",
            "be",
            "--emit",
            "payload+encapsulation",
        ],
    );
}

#[test]
fn each_vendors_pl_cdr1_framing_reads_back_under_its_own_dialect() {
    guard!();
    // The three differ in the header form, in what the length field
    // counts and in the must-understand bit of the marker and the
    // sentinel. The corpus run uses the default — Cyclone's — for all
    // of them, so the other two branches of `zdw_read_pid` and of
    // `check_declared_length` are only reached here.
    let idl = "@mutable struct Ext { long a; double b; short c; string s; };";
    let sample = r#"{ "a": 7, "b": 1.5, "c": -3, "s": "über" }"#;
    for dialect in ["cyclone", "rti", "fastdds", "specification"] {
        for endianness in [Endianness::Little, Endianness::Big] {
            let request = ProbeRequest::in_dialect(
                "Ext",
                Encoding::PlCdr1,
                endianness,
                zerodds_ast_layout::Dialect::parse(dialect).expect("a dialect"),
            );
            assert_round_trip(
                idl,
                "Ext",
                sample,
                &request,
                &[
                    "--encoding",
                    "pl-cdr1",
                    "--endian",
                    match endianness {
                        Endianness::Little => "le",
                        Endianness::Big => "be",
                    },
                    "--dialect",
                    dialect,
                ],
            );
        }
    }
}
