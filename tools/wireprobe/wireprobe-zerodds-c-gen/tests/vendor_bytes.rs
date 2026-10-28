// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C column against bytes three foreign implementations produced.
//!
//! Every expectation is a hex string copied out of
//! `crates/ast-layout/tests/fixtures/vendor_bytes.md`, which records
//! what Cyclone DDS, RTI Connext 7.7 and Fast DDS wrote for that IDL
//! and that sample. Nothing in this project produced any of them.
//!
//! `crates/wire-probe/tests/vendor_bytes.rs` asserts the same rows for
//! the Rust probe. This one goes through the whole C path — generate,
//! compile, run — so a row that passes here passes because `runtime/c`
//! wrote those bytes, not because the layout plan says it should have.
//!
//! The test compiles C. It needs a working `cc` and is skipped, loudly,
//! when there is none: a test that passed silently on a machine with no
//! compiler would be a green C column on a machine that cannot build
//! one.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The probe script, and the generator it should use.
///
/// The generator comes from this crate's own build so the test never
/// runs `cargo` inside `cargo`, which deadlocks on the build lock.
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

/// One run of the C probe, as lowercase hex.
fn run(idl: &str, type_name: &str, values: &str, flags: &[&str]) -> String {
    let (script, generator) = probe();
    // A counter and not a clock: `cargo test` runs these in parallel
    // threads of one process, and two that took the same nanosecond
    // shared a directory — one overwrote the other's IDL and the probe
    // reported a type that was not in the model.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-c-vendor-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl_path = dir.join("type.idl");
    let values_path = dir.join("values.json");
    std::fs::write(&idl_path, idl).expect("the idl");
    std::fs::write(&values_path, values).expect("the values");

    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .arg("--idl")
        .arg(&idl_path)
        .arg("--type")
        .arg(type_name)
        .arg("--values")
        .arg(&values_path)
        .args(flags)
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the C probe failed for {type_name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Whether the environment can run these at all.
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

fn hex_of(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

// -- all three implementations agree -------------------------------------

#[test]
fn point_matches_cyclone_rti_and_fastdds() {
    guard!();
    let idl = "@final struct Point { long x; double y; };";
    let values = r#"{ "x": 1, "y": 2.0 }"#;
    assert_eq!(
        run(idl, "Point", values, &["--encoding", "xcdr2"]),
        "010000000000000000000040",
        "XCDR2 caps alignment at four"
    );
    assert_eq!(
        run(idl, "Point", values, &["--encoding", "xcdr1"]),
        "01000000000000000000000000000040",
        "XCDR1 aligns the double to eight"
    );
}

#[test]
fn the_mixed_primitive_struct_matches_all_three() {
    guard!();
    let idl = "\
@final struct Mixed {
    boolean b; octet o; short s; unsigned short us;
    long l; unsigned long ul; long long ll; unsigned long long ull;
    float f; double d;
};";
    let values = r#"{
        "b": true, "o": 171, "s": -12345, "us": 54321,
        "l": -1234567, "ul": 2345678,
        "ll": "-987654321", "ull": "123456789",
        "f": 2.5, "d": 3.14159
    }"#;
    assert_eq!(
        run(idl, "Mixed", values, &["--encoding", "xcdr1"]),
        hex_of(
            "01abc7cf31d40000 7929edff ceca2300 4f9721c5ffffffff \
             15cd5b0700000000 00002040 00000000 6e861bf0f9210940"
        )
    );
    assert_eq!(
        run(idl, "Mixed", values, &["--encoding", "xcdr2"]),
        hex_of(
            "01abc7cf31d40000 7929edff ceca2300 4f9721c5ffffffff \
             15cd5b0700000000 00002040 6e861bf0f9210940"
        )
    );
}

#[test]
fn a_structure_carries_no_trailing_padding() {
    guard!();
    // Six bytes, not eight. The type aligns to four and does not pad out
    // to it — all three, both versions.
    let idl = "@final struct Odd { long a; short b; };";
    let values = r#"{ "a": 1, "b": 2 }"#;
    assert_eq!(
        run(idl, "Odd", values, &["--encoding", "xcdr2"]),
        "010000000200"
    );
    assert_eq!(
        run(idl, "Odd", values, &["--encoding", "xcdr1"]),
        "010000000200"
    );
}

#[test]
fn array_elements_pad_between_themselves_and_not_after_the_last() {
    guard!();
    let idl = "\
@final struct Odd { long a; short b; };
@final struct ArrOdd { Odd items[2]; octet tail; };";
    let values = r#"{ "items": [ { "a": 1, "b": 2 }, { "a": 3, "b": 4 } ], "tail": 119 }"#;
    // All three under XCDR1: the first element pads to the element's
    // four-byte alignment, the second does not, and the octet lands at
    // offset 14.
    assert_eq!(
        run(idl, "ArrOdd", values, &["--encoding", "xcdr1"]),
        hex_of("01000000 0200 0000 03000000 0400 77")
    );
}

#[test]
fn a_string_member_matches_all_three() {
    guard!();
    let idl = "@final struct WithString { string s; long n; };";
    let values = r#"{ "s": "abc", "n": 7 }"#;
    assert_eq!(
        run(idl, "WithString", values, &["--encoding", "xcdr2"]),
        hex_of("04000000 61626300 07000000")
    );
}

#[test]
fn a_sequence_of_a_primitive_matches_all_three() {
    guard!();
    // No delimiter: §7.4.3.5 exempts a collection whose element type is
    // primitive under both versions.
    let idl = "@final struct Ids { sequence<long> values; };";
    let values = r#"{ "values": [1, 2, 3] }"#;
    assert_eq!(
        run(idl, "Ids", values, &["--encoding", "xcdr2"]),
        hex_of("03000000 01000000 02000000 03000000")
    );
}

#[test]
fn the_three_extensibilities_match_all_three() {
    guard!();
    let members = "long a; double b; short c;";
    let values = r#"{ "a": 7, "b": 1.5, "c": -3 }"#;
    // @final: no header at all.
    assert_eq!(
        run(
            &format!("@final struct Ext {{ {members} }};"),
            "Ext",
            values,
            &["--encoding", "xcdr2"]
        ),
        hex_of("07000000 000000000000f83f fdff")
    );
    // @appendable under XCDR2: a four-byte DHEADER of 14.
    assert_eq!(
        run(
            &format!("@appendable struct Ext {{ {members} }};"),
            "Ext",
            values,
            &["--encoding", "xcdr2"]
        ),
        hex_of("0e000000 07000000 000000000000f83f fdff")
    );
    // XCDR1 has no delimited encoding, so @appendable and @final are
    // the same bytes.
    assert_eq!(
        run(
            &format!("@appendable struct Ext {{ {members} }};"),
            "Ext",
            values,
            &["--encoding", "xcdr1"]
        ),
        run(
            &format!("@final struct Ext {{ {members} }};"),
            "Ext",
            values,
            &["--encoding", "xcdr1"]
        )
    );
}

// -- where the three part ------------------------------------------------

#[test]
fn the_pl_cdr1_parameter_list_reproduces_each_vendor_under_its_dialect() {
    guard!();
    // The `ext_mutable` XCDR1 rows, which the fixture records in big
    // endian for all three. Asserted in that byte order rather than
    // transcribed into little: the three streams differ in the header
    // form, in what the length field counts and in the must-understand
    // bit of the marker and the sentinel, and hand-swapping two-byte
    // fields across three axes is how a transcription goes wrong.
    let idl = "@mutable struct Ext { long a; double b; short c; };";
    let values = r#"{ "a": 7, "b": 1.5, "c": -3 }"#;
    let of = |dialect: &str| {
        run(
            idl,
            "Ext",
            values,
            &[
                "--encoding",
                "pl-cdr1",
                "--endian",
                "be",
                "--dialect",
                dialect,
            ],
        )
    };

    assert_eq!(
        of("cyclone"),
        concat!(
            "7f0100080000000000000004000000077f0100080000000100000008",
            "3ff80000000000007f0100080000000200000004fffd00007f020000"
        ),
        "Cyclone: PID_EXTENDED throughout, must-understand set, 56 bytes"
    );
    assert_eq!(
        of("rti"),
        "0000000400000007000100083ff800000000000000020004fffd00007f020000",
        "RTI: the short form, `short c` declared as four, 32 bytes"
    );
    assert_eq!(
        of("fastdds"),
        "0000000400000007000100083ff800000000000000020002fffd00003f020000",
        "Fast DDS: the short form, `short c` declared as two, and a \
         sentinel without the must-understand bit"
    );
    // Which of the three the clause itself implies is
    // `zerodds_ast_layout::Dialect`'s question and is answered there,
    // with the reasoning. What is asserted here is only that each named
    // dialect reproduces the stream that vendor was measured writing.
    assert_eq!(
        run(
            idl,
            "Ext",
            values,
            &["--encoding", "pl-cdr1", "--endian", "be"]
        ),
        of("specification"),
        "and the default dialect is the clause's — it was Cyclone's, whose 56-byte \
         extended-header row is the first assertion above"
    );
}

#[test]
fn the_mutable_emheaders_are_the_ones_all_three_wrote() {
    guard!();
    // `ext_mutable` XCDR2, where the three agree: the length codes are
    // 2, 3 and 1 — the sizes of the members — and not 4 with an
    // explicit NEXTINT for each.
    let idl = "@mutable struct Ext { long a; double b; short c; };";
    let values = r#"{ "a": 7, "b": 1.5, "c": -3 }"#;
    assert_eq!(
        run(
            idl,
            "Ext",
            values,
            &["--encoding", "pl-cdr2", "--endian", "be"]
        ),
        "0000001a2000000000000007300000013ff800000000000010000002fffd"
    );
}

#[test]
fn a_collection_of_a_struct_splits_the_three() {
    guard!();
    // Cyclone and Fast DDS delimit an array of a fixed-size `@final`
    // struct under XCDR2; RTI does not.
    let idl = "\
@final struct Cell { long a; long b; };
@final struct ArrN { Cell items[2]; };";
    let values = r#"{ "items": [ { "a": 1, "b": 2 }, { "a": 3, "b": 4 } ] }"#;
    let body = "01000000 02000000 03000000 04000000";
    assert_eq!(
        run(
            idl,
            "ArrN",
            values,
            &["--encoding", "xcdr2", "--dialect", "cyclone"]
        ),
        hex_of(&format!("10000000 {body}")),
        "Cyclone: twenty bytes with a leading delimiter"
    );
    assert_eq!(
        run(
            idl,
            "ArrN",
            values,
            &["--encoding", "xcdr2", "--dialect", "rti"]
        ),
        hex_of(body),
        "RTI: sixteen, no delimiter"
    );
}

#[test]
fn an_enumeration_collection_splits_cyclone_from_the_other_two() {
    guard!();
    // §7.2.2.4.1 makes an enumeration a constructed type, so §7.4.3.5
    // delimits a collection of one. Fast DDS and RTI do not.
    let idl = "\
enum Colour { RED, GREEN, BLUE };
@final struct Cs { Colour colours[2]; };";
    let values = r#"{ "colours": ["GREEN", "BLUE"] }"#;
    assert_eq!(
        run(
            idl,
            "Cs",
            values,
            &["--encoding", "xcdr2", "--dialect", "cyclone"]
        ),
        hex_of("08000000 01000000 02000000"),
        "Cyclone: twelve bytes"
    );
    assert_eq!(
        run(
            idl,
            "Cs",
            values,
            &["--encoding", "xcdr2", "--dialect", "fastdds"]
        ),
        hex_of("01000000 02000000"),
        "Fast DDS: eight"
    );
}

#[test]
fn an_absent_optional_member_is_not_written() {
    guard!();
    // `mut_optional`: in a `@mutable` type an absent member leaves no
    // trace, because the parameter list is what says which members are
    // there. The fixture has this one in both byte orders, so both are
    // asserted.
    let idl = "@mutable struct MutOpt { long a; @optional long b; };";
    let values = r#"{ "a": 7 }"#;
    assert_eq!(
        run(
            idl,
            "MutOpt",
            values,
            &["--encoding", "xcdr2", "--endian", "le"]
        ),
        hex_of("08000000 00000020 07000000"),
        "twelve bytes: the DHEADER, one EMHEADER, the value"
    );
    assert_eq!(
        run(
            idl,
            "MutOpt",
            values,
            &["--encoding", "xcdr2", "--endian", "be"]
        ),
        "000000082000000000000007"
    );
    assert_eq!(
        run(
            idl,
            "MutOpt",
            values,
            &[
                "--encoding",
                "pl-cdr1",
                "--endian",
                "be",
                "--dialect",
                "rti"
            ]
        ),
        "00000004000000077f020000",
        "RTI's XCDR1 form, twelve bytes"
    );
    assert_eq!(
        run(
            idl,
            "MutOpt",
            values,
            &[
                "--encoding",
                "pl-cdr1",
                "--endian",
                "be",
                "--dialect",
                "fastdds"
            ]
        ),
        "00000004000000073f020000",
        "Fast DDS, differing only in the sentinel's must-understand bit"
    );
    assert_eq!(
        run(
            idl,
            "MutOpt",
            values,
            &[
                "--encoding",
                "pl-cdr1",
                "--endian",
                "be",
                "--dialect",
                "cyclone"
            ]
        ),
        "7f0100080000000000000004000000077f020000",
        "Cyclone, twenty bytes"
    );
}

// -- the encapsulation options -------------------------------------------

#[test]
fn the_encapsulation_options_carry_the_padding_count() {
    guard!();
    // XTypes 1.3 §7.4.1.1 pads the serialized data to the next
    // four-byte boundary with zeros, and §7.6.3.1.2 puts the count in
    // the low two bits of the second options byte, which a receiver
    // "shall" use to find where the data ended.
    //
    // Measured on the loopback wire on 2026-08-09 with `tshark`:
    // `@final struct One { octet a; }` with `a = 0xAB` goes out from
    // Cyclone, RTI and Fast DDS alike as identifier `0001`, options
    // `0003`, serializedData `ab000000`. §7.6.3.1.2's own `TypeB`
    // through Cyclone: options `0001`, `23006200`.
    let one = "@final struct One { octet a; };";
    assert_eq!(
        run(
            one,
            "One",
            r#"{ "a": 171 }"#,
            &["--encoding", "xcdr1", "--emit", "payload+encapsulation"]
        ),
        "00010003ab000000",
        "a one-byte payload needs three bytes of padding and has to say so"
    );

    let type_b = "@final struct TypeB { short a; char b; };";
    assert_eq!(
        run(
            type_b,
            "TypeB",
            r#"{ "a": 35, "b": "b" }"#,
            &["--encoding", "xcdr1", "--emit", "payload+encapsulation"]
        ),
        "0001000123006200"
    );

    // A payload already on the boundary pads by nothing and says zero,
    // which is why leaving the field at zero looked right for so long.
    let point = "@final struct Point { long x; double y; };";
    assert_eq!(
        run(
            point,
            "Point",
            r#"{ "x": 1, "y": 2.0 }"#,
            &["--encoding", "xcdr2", "--emit", "payload+encapsulation"]
        ),
        "00070000010000000000000000000040"
    );
}

// -- the KeyHash ---------------------------------------------------------

#[test]
fn the_keyhash_reproduces_the_rtps_example() {
    guard!();
    // DDSI-RTPS 2.5 §9.6.4.8 Example 1: the four key bytes, big-endian,
    // zero-padded to sixteen. Cyclone and Fast DDS both produce it.
    let idl = "@final struct Foo { @key long id; long x; long y; };";
    let values = r#"{ "id": 305419896, "x": 1, "y": 2 }"#;
    assert_eq!(
        run(idl, "Foo", values, &["--emit", "keyhash"]),
        "12345678000000000000000000000000"
    );
}

#[test]
fn a_keyless_type_has_no_keyhash() {
    guard!();
    // A KeyHash over nothing is not a KeyHash: a keyless topic has one
    // instance and no handle to tell apart. The probe refuses rather
    // than producing sixteen zero bytes.
    let (script, generator) = probe();
    let dir = std::env::temp_dir().join(format!("zd-c-keyless-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl = dir.join("type.idl");
    let values = dir.join("values.json");
    std::fs::write(&idl, "@final struct Plain { long x; };").expect("the idl");
    std::fs::write(&values, r#"{ "x": 1 }"#).expect("the values");
    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .args(["--idl"])
        .arg(&idl)
        .args(["--type", "Plain", "--values"])
        .arg(&values)
        .args(["--emit", "keyhash"])
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !output.status.success(),
        "a keyless type produced a KeyHash: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
