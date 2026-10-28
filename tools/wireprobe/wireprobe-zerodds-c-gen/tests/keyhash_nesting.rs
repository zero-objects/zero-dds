// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! How far below a keyless aggregate the key holder reaches.
//!
//! §7.6.8 Step 1 removes a nested aggregate's non-key members only "if
//! there are any key members". One dialect reads the consequence
//! differently: `zerodds_ast_layout::KeyHolderNesting` records that RTI
//! under XCDR1 stops filtering altogether below the first keyless
//! aggregate and takes the subtree as the data has it, where everyone
//! else keeps applying Step 1 at every level.
//!
//! The C column reproduces the RTPS §9.6.4.8 example already — one key
//! member, no nesting — which is exactly the shape that cannot tell the
//! two readings apart. This is the shape that can: a keyed struct
//! *below* a keyless one, so the two policies disagree about whether
//! the keyless struct's grandchild contributes its non-key members.
//!
//! Measured against the Rust probe rather than against a recorded
//! string, because what is under test is that the two columns answer
//! the same way under the same dialect. The C column had this wrong: it
//! named every nested member's encoder by the enclosing purpose instead
//! of asking `keyhash::member_key`, so it produced the recursive answer
//! under every dialect and the two hashes parted on the RTI/XCDR1 row.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::value::{Emit, ProbeRequest};

/// A keyed struct two levels down, under a keyless one.
///
/// `Middle` declares no `@key`, so Step 1 leaves all of its members in.
/// What the policies disagree about is `Deep`: recursively, only its
/// `@key long k` is in the holder; under RTI's XCDR1 reading, `junk`
/// comes too because the filtering stopped at `Middle`.
const IDL: &str = "\
@final struct Deep { @key long k; long junk; };
@final struct Middle { Deep d; long m; };
@final struct S { @key Middle mid; long tail; };";

const SAMPLE: &str = r#"{ "mid": { "d": { "k": 7, "junk": 99 }, "m": 5 }, "tail": -1 }"#;

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

/// The KeyHash the compiled C produces, as lowercase hex.
fn c_keyhash(dialect: &str, encoding: &str) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-c-nesting-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the work directory");
    let idl = dir.join("type.idl");
    let values = dir.join("values.json");
    std::fs::write(&idl, IDL).expect("the idl");
    std::fs::write(&values, SAMPLE).expect("the values");

    let (script, generator) = probe();
    let output = Command::new(&script)
        .env("WIREPROBE_C_GEN", &generator)
        .arg("--idl")
        .arg(&idl)
        .args(["--type", "S", "--values"])
        .arg(&values)
        .args([
            "--emit",
            "keyhash",
            "--encoding",
            encoding,
            "--dialect",
            dialect,
        ])
        .output()
        .expect("the probe ran");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the C probe failed under --dialect {dialect} --encoding {encoding}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The same, through the Rust probe's library.
fn rust_keyhash(dialect: Dialect, encoding: Encoding) -> String {
    let ast = zerodds_wire_probe::encode::model_from_idl(IDL, "nesting.idl").expect("the IDL");
    let value: serde_json::Value = serde_json::from_str(SAMPLE).expect("the sample parses");
    let mut request = ProbeRequest::in_dialect("S", encoding, Endianness::Little, dialect);
    request.emit = Emit::KeyHash;
    zerodds_wire_probe::encode::hex(
        &zerodds_wire_probe::encode::encode(&ast, &request, &value).expect("the KeyHash"),
    )
}

#[test]
fn the_two_columns_agree_about_how_far_the_holder_nests() {
    if skip() {
        return;
    }
    for (name, dialect) in [
        ("cyclone", Dialect::Cyclone),
        ("rti", Dialect::Rti),
        ("fastdds", Dialect::FastDds),
        ("specification", Dialect::Specification),
    ] {
        for (flag, encoding) in [
            ("xcdr1", Encoding::PlainCdr1),
            ("xcdr2", Encoding::PlainCdr2),
        ] {
            assert_eq!(
                c_keyhash(name, flag),
                rust_keyhash(dialect, encoding),
                "--dialect {name} --encoding {flag}"
            );
        }
    }
}

/// And the case is one that can tell the two readings apart.
///
/// A test that passed because every dialect produced the same hash
/// would prove nothing about the rule. `junk` is 99 — `0x63` — and it
/// is in exactly one of the eight.
#[test]
fn the_case_separates_the_two_readings() {
    let recursive = rust_keyhash(Dialect::Cyclone, Encoding::PlainCdr1);
    let whole = rust_keyhash(Dialect::Rti, Encoding::PlainCdr1);
    assert_ne!(
        recursive, whole,
        "the fixture cannot tell KeyHolderNesting apart"
    );
    assert!(
        whole.contains("00000063") && !recursive.contains("00000063"),
        "recursive {recursive}, keyless-nested-whole {whole}"
    );
}
