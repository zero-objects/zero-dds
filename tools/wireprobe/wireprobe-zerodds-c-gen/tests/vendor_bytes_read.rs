// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The same vendor bytes, read back by the C column.
//!
//! `vendor_bytes.rs` asserts that the compiled C *writes* the byte
//! strings `crates/ast-layout/tests/fixtures/vendor_bytes.md` records
//! Cyclone DDS, RTI Connext 7.7 and Fast DDS producing. This asserts
//! that it *reads* them: the hex goes into the probe's standard input
//! and the values that come out are compared, in canonical form,
//! against the sample the vendor was given.
//!
//! Against the sample and not against the C encoder's own output. A
//! decoder wrong in the same way its encoder is wrong passes a round
//! trip through itself every time — `docs/specs/zerodds-interop-suite-1.0.md`
//! §4, P2 — and the whole reason these rows exist is that nothing in
//! this project produced them.
//!
//! The test compiles C. It needs a working `cc` and is skipped, loudly,
//! when there is none.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The probe script, and the generator it should use.
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

fn have_compiler() -> bool {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    Command::new(cc)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn skip() -> bool {
    let (script, _) = probe();
    if !Path::new(&script).exists() {
        eprintln!("wireprobe-zerodds-c is missing; the C column is not measured here");
        return true;
    }
    if !have_compiler() {
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

fn bytes_of(hex: &str) -> Vec<u8> {
    let digits: Vec<char> = hex.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(digits.len() % 2 == 0, "an odd number of hex digits");
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(&pair.iter().collect::<String>(), 16).expect("a hex byte"))
        .collect()
}

/// One read through the C probe, as the JSON it printed.
fn read(idl: &str, type_name: &str, hex: &str, flags: &[&str]) -> String {
    let (script, generator) = probe();
    // A counter and not a clock, for the reason `vendor_bytes.rs`
    // gives: `cargo test` runs these in parallel threads of one
    // process, and two that took the same nanosecond shared a
    // directory.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-c-read-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl_path = dir.join("type.idl");
    let bytes_path = dir.join("payload.bin");
    std::fs::write(&idl_path, idl).expect("the idl");
    std::fs::write(&bytes_path, bytes_of(hex)).expect("the payload");

    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .arg("--idl")
        .arg(&idl_path)
        .arg("--type")
        .arg(type_name)
        .arg("--in")
        .arg(&bytes_path)
        .args(flags)
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the C probe did not read {type_name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The vendor's bytes, read, against the sample the vendor was given.
///
/// Both sides go through `canonicalise`, so a difference in how a
/// `double` was printed is not reported as a difference in what was
/// read. `crates/wire-probe/src/canonical.rs` is where the form is
/// defined and `contract.rs` is where the rules behind it live.
fn assert_reads(idl: &str, type_name: &str, hex: &str, flags: &[&str], sample: &str) {
    let printed = read(idl, type_name, hex, flags);
    let ast = zerodds_wire_probe::encode::model_from_idl(idl, "vendor.idl").expect("the IDL");
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
            type_name
                .split("::")
                .filter(|segment| !segment.is_empty())
                .map(std::string::ToString::to_string)
                .collect(),
        ))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type is in the model");

    let expected = zerodds_wire_probe::canonical::canonicalise(
        &ast,
        type_,
        &serde_json::from_str::<serde_json::Value>(sample).expect("the sample parses"),
        type_name,
    )
    .expect("the sample is a value of this type");
    let found = zerodds_wire_probe::canonical::canonicalise(
        &ast,
        type_,
        &serde_json::from_str::<serde_json::Value>(&printed)
            .unwrap_or_else(|error| panic!("the probe printed `{printed}`: {error}")),
        type_name,
    )
    .unwrap_or_else(|error| panic!("the probe printed `{printed}`: {error}"));

    if let Some(first) = zerodds_wire_probe::canonical::difference(&expected, &found, type_name) {
        panic!("{type_name}: {first}");
    }
}

// -- all three implementations agree -------------------------------------

#[test]
fn point_reads_back_from_what_cyclone_rti_and_fastdds_wrote() {
    guard!();
    let idl = "@final struct Point { long x; double y; };";
    let sample = r#"{ "x": 1, "y": 2.0 }"#;
    assert_reads(
        idl,
        "Point",
        "010000000000000000000040",
        &["--encoding", "xcdr2"],
        sample,
    );
    assert_reads(
        idl,
        "Point",
        "01000000000000000000000000000040",
        &["--encoding", "xcdr1"],
        sample,
    );
}

#[test]
fn the_mixed_primitive_struct_reads_back() {
    guard!();
    let idl = "\
@final struct Mixed {
    boolean b; octet o; short s; unsigned short us;
    long l; unsigned long ul; long long ll; unsigned long long ull;
    float f; double d;
};";
    let sample = r#"{
        "b": true, "o": 171, "s": -12345, "us": 54321,
        "l": -1234567, "ul": 2345678,
        "ll": "-987654321", "ull": "123456789",
        "f": 2.5, "d": 3.14159
    }"#;
    assert_reads(
        idl,
        "Mixed",
        "01abc7cf31d40000 7929edff ceca2300 4f9721c5ffffffff \
         15cd5b0700000000 00002040 00000000 6e861bf0f9210940",
        &["--encoding", "xcdr1"],
        sample,
    );
    assert_reads(
        idl,
        "Mixed",
        "01abc7cf31d40000 7929edff ceca2300 4f9721c5ffffffff \
         15cd5b0700000000 00002040 6e861bf0f9210940",
        &["--encoding", "xcdr2"],
        sample,
    );
}

#[test]
fn a_structure_with_no_trailing_padding_reads_back() {
    guard!();
    // Six bytes and not eight. The trailing-byte check has to accept
    // the two that are not there rather than demanding them.
    let idl = "@final struct Odd { long a; short b; };";
    let sample = r#"{ "a": 1, "b": 2 }"#;
    assert_reads(idl, "Odd", "010000000200", &["--encoding", "xcdr2"], sample);
    assert_reads(idl, "Odd", "010000000200", &["--encoding", "xcdr1"], sample);
}

#[test]
fn a_string_and_a_sequence_read_back() {
    guard!();
    assert_reads(
        "@final struct WithString { string s; long n; };",
        "WithString",
        "04000000 61626300 07000000",
        &["--encoding", "xcdr2"],
        r#"{ "s": "abc", "n": 7 }"#,
    );
    assert_reads(
        "@final struct Ids { sequence<long> values; };",
        "Ids",
        "03000000 01000000 02000000 03000000",
        &["--encoding", "xcdr2"],
        r#"{ "values": [1, 2, 3] }"#,
    );
}

#[test]
fn the_three_extensibilities_read_back() {
    guard!();
    let members = "long a; double b; short c;";
    let sample = r#"{ "a": 7, "b": 1.5, "c": -3 }"#;
    assert_reads(
        &format!("@final struct Ext {{ {members} }};"),
        "Ext",
        "07000000 000000000000f83f fdff",
        &["--encoding", "xcdr2"],
        sample,
    );
    assert_reads(
        &format!("@appendable struct Ext {{ {members} }};"),
        "Ext",
        "0e000000 07000000 000000000000f83f fdff",
        &["--encoding", "xcdr2"],
        sample,
    );
}

// -- where the three part ------------------------------------------------

#[test]
fn each_vendors_parameter_list_reads_back_under_its_own_dialect() {
    guard!();
    // The `ext_mutable` XCDR1 rows, big endian as the fixture records
    // them. The three differ in the header form, in what the length
    // field counts and in the must-understand bit of the marker and the
    // sentinel — every one of which the reader has to get right, and
    // two of which it had wrong: the id was compared against the whole
    // word, so Cyclone's `7f01` marker and `7f02` sentinel were
    // unrecognised, and a parameter value was aligned from the stream
    // rather than from its own start, which put four bytes of padding
    // in front of `double b` that no vendor wrote.
    let idl = "@mutable struct Ext { long a; double b; short c; };";
    let sample = r#"{ "a": 7, "b": 1.5, "c": -3 }"#;

    assert_reads(
        idl,
        "Ext",
        concat!(
            "7f0100080000000000000004000000077f0100080000000100000008",
            "3ff80000000000007f0100080000000200000004fffd00007f020000"
        ),
        &[
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "cyclone",
        ],
        sample,
    );
    assert_reads(
        idl,
        "Ext",
        "0000000400000007000100083ff800000000000000020004fffd00007f020000",
        &[
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "rti",
        ],
        sample,
    );
    assert_reads(
        idl,
        "Ext",
        "0000000400000007000100083ff800000000000000020002fffd00003f020000",
        &[
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "fastdds",
        ],
        sample,
    );
}

#[test]
fn the_mutable_emheaders_read_back() {
    guard!();
    let idl = "@mutable struct Ext { long a; double b; short c; };";
    let sample = r#"{ "a": 7, "b": 1.5, "c": -3 }"#;
    assert_reads(
        idl,
        "Ext",
        "0000001a2000000000000007300000013ff800000000000010000002fffd",
        &["--encoding", "pl-cdr2", "--endian", "be"],
        sample,
    );
}

#[test]
fn a_collection_of_a_struct_reads_back_under_both_readings() {
    guard!();
    // Cyclone and Fast DDS delimit an array of a fixed-size `@final`
    // struct under XCDR2; RTI does not. The reader takes the dialect
    // from the same key the writer does, so each stream reads under the
    // setting that produced it and neither is sniffed.
    let idl = "\
@final struct Cell { long a; long b; };
@final struct ArrN { Cell items[2]; };";
    let sample = r#"{ "items": [ { "a": 1, "b": 2 }, { "a": 3, "b": 4 } ] }"#;
    assert_reads(
        idl,
        "ArrN",
        "10000000 01000000 02000000 03000000 04000000",
        &["--encoding", "xcdr2", "--dialect", "cyclone"],
        sample,
    );
    assert_reads(
        idl,
        "ArrN",
        "01000000 02000000 03000000 04000000",
        &["--encoding", "xcdr2", "--dialect", "rti"],
        sample,
    );
}

#[test]
fn an_enumeration_collection_reads_back_under_both_readings() {
    guard!();
    let idl = "\
enum Colour { RED, GREEN, BLUE };
@final struct Cs { Colour colours[2]; };";
    let sample = r#"{ "colours": ["GREEN", "BLUE"] }"#;
    assert_reads(
        idl,
        "Cs",
        "08000000 01000000 02000000",
        &["--encoding", "xcdr2", "--dialect", "cyclone"],
        sample,
    );
    assert_reads(
        idl,
        "Cs",
        "01000000 02000000",
        &["--encoding", "xcdr2", "--dialect", "fastdds"],
        sample,
    );
}

#[test]
fn an_absent_optional_member_comes_back_absent() {
    guard!();
    // `mut_optional`: in a `@mutable` type the parameter list is what
    // says which members are there, so an absent one leaves no trace and
    // has to come back as a key that is not there. A decoder that
    // produced a zero in its place would return a sample of the right
    // shape with a value nobody sent. Outside a `@mutable` type the
    // absence *is* written — §7.4.3.5.2 rule (20), §7.4.1.2.
    let idl = "@mutable struct MutOpt { long a; @optional long b; };";
    let sample = r#"{ "a": 7 }"#;
    assert_reads(
        idl,
        "MutOpt",
        "08000000 00000020 07000000",
        &["--encoding", "xcdr2", "--endian", "le"],
        sample,
    );
    assert_reads(
        idl,
        "MutOpt",
        "000000082000000000000007",
        &["--encoding", "xcdr2", "--endian", "be"],
        sample,
    );
    assert_reads(
        idl,
        "MutOpt",
        "00000004000000077f020000",
        &[
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "rti",
        ],
        sample,
    );
    assert_reads(
        idl,
        "MutOpt",
        "00000004000000073f020000",
        &[
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "fastdds",
        ],
        sample,
    );
    assert_reads(
        idl,
        "MutOpt",
        "7f0100080000000000000004000000077f020000",
        &[
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "cyclone",
        ],
        sample,
    );
}

// -- what the read direction refuses -------------------------------------

#[test]
fn a_payload_cut_short_is_refused_rather_than_read_as_a_shorter_sample() {
    guard!();
    // The silent-drop shape this suite exists to find: the sample would
    // come back looking plausible with its last members missing.
    let (script, generator) = probe();
    let dir = std::env::temp_dir().join(format!("zd-c-short-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl = dir.join("type.idl");
    let payload = dir.join("payload.bin");
    std::fs::write(&idl, "@final struct Point { long x; double y; };").expect("the idl");
    // Four bytes of a twelve-byte payload.
    std::fs::write(&payload, bytes_of("01000000")).expect("the payload");
    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .arg("--idl")
        .arg(&idl)
        .args(["--type", "Point", "--encoding", "xcdr2", "--in"])
        .arg(&payload)
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !output.status.success(),
        "a truncated payload read as a sample: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let complaint = String::from_utf8_lossy(&output.stderr);
    // Named and offset-bearing: a status with no offset sends its
    // reader back through the payload by hand.
    assert!(
        complaint.contains("fewer bytes remain") && complaint.contains("at "),
        "the refusal says neither what nor where: {complaint}"
    );
}

#[test]
fn a_stream_framed_for_another_peer_names_the_setting_it_fits() {
    guard!();
    // Fast DDS's exact parameter length read under RTI's rounded-up
    // convention. Well formed, and the wrong setting — which is a
    // different thing from a malformed stream and says so.
    let (script, generator) = probe();
    let dir = std::env::temp_dir().join(format!("zd-c-framing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl = dir.join("type.idl");
    let payload = dir.join("payload.bin");
    std::fs::write(&idl, "@mutable struct Ext { short c; };").expect("the idl");
    std::fs::write(&payload, bytes_of("00000002fffd00007f020000")).expect("the payload");
    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .arg("--idl")
        .arg(&idl)
        .args([
            "--type",
            "Ext",
            "--encoding",
            "pl-cdr1",
            "--endian",
            "be",
            "--dialect",
            "rti",
            "--in",
        ])
        .arg(&payload)
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !output.status.success(),
        "the wrong framing was accepted: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(
        complaint.contains("--pl-cdr1-headers short-true-length"),
        "the refusal does not name the setting the bytes fit: {complaint}"
    );
}
