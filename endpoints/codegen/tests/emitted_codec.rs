// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the generator emits for the three IDL fixtures.
//!
//! The byte-level oracle is outside this file: `test/test_gen.c` compares
//! the emitted C codec against the Rust golden vectors in
//! `endpoints/swift/testdata/`, and `test/test_nested_gen.c` and
//! `test/test_mutable_gen.c` do the same for the other two. What is
//! checked here is the part that decides those bytes and that used to be
//! re-derived from annotations in this crate rather than read off the
//! model: the effective extensibility and the effective member ids.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("idl")
}

/// Run the generator over one fixture and return the emitted text.
fn generate(fixture: &str, lang: &str, extension: &str) -> String {
    let out = tempdir();
    let status = Command::new(env!("CARGO_BIN_EXE_zerodds-endpoint-codegen"))
        .arg(fixtures().join(fixture))
        .arg(&out)
        .arg(lang)
        .status()
        .expect("run the generator");
    assert!(status.success(), "{fixture}: the generator failed");
    let stem = Path::new(fixture).file_stem().expect("stem");
    let path = out.join(format!("{}_gen.{extension}", stem.to_string_lossy()));
    let text = std::fs::read_to_string(&path).expect("read the emitted file");
    let _ = std::fs::remove_dir_all(&out);
    text
}

fn tempdir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "zerodds-codegen-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&path).expect("create the output directory");
    path
}

#[test]
fn a_final_struct_carries_no_dheader() {
    // `@final` — XTypes 1.3 §7.4.3.5.3: no delimiter.
    let emitted = generate("sensor.idl", "c", "c");
    assert!(
        !emitted.contains("zdw_dheader_begin"),
        "a final struct gained a DHEADER:\n{emitted}"
    );
    assert!(emitted.contains("int sensor_reading_encode"));
}

#[test]
fn an_appendable_struct_is_delimited() {
    let emitted = generate("nested.idl", "c", "c");
    assert!(
        emitted.contains("zdw_dheader_begin"),
        "an appendable struct lost its DHEADER"
    );
    assert!(
        !emitted.contains("zdw_emheader_begin"),
        "an appendable struct gained member headers"
    );
}

#[test]
fn a_mutable_struct_carries_the_member_ids_the_source_declared() {
    // `@id(10)`, `@id(20)` and `@id(30)` decide the EMHEADERs. The
    // effective ids come from the frontend's completion pass; this
    // crate used to parse the annotations a second time and had its own
    // answer for anything the clause left implicit.
    let emitted = generate("mutable.idl", "c", "c");
    for id in [10, 20, 30] {
        assert!(
            emitted.contains(&format!("zdw_emheader_begin(w, {id}uL, 0)")),
            "member id {id} is not in the emitted encoder:\n{emitted}"
        );
    }
    assert!(emitted.contains("_id == 10uL"), "the decoder lost id 10");
    assert!(emitted.contains("_id == 30uL"), "the decoder lost id 30");
}

#[test]
fn a_nested_struct_is_emitted_before_the_one_that_holds_it() {
    // The C89 output has no forward declarations, so declaration order
    // is load-bearing.
    let emitted = generate("nested.idl", "c", "h");
    let inner = emitted.find("} Inner;").expect("Inner");
    let outer = emitted.find("} Outer;").expect("Outer");
    assert!(inner < outer, "Outer is declared before Inner");
}

#[test]
fn the_python_and_java_columns_emit_the_same_member_ids() {
    let python = generate("mutable.idl", "python", "py");
    let java = generate("mutable.idl", "java", "java");
    for id in [10, 20, 30] {
        assert!(
            python.contains(&format!("w.emheader_begin({id}, 0)")),
            "python lost member id {id}"
        );
        assert!(
            java.contains(&format!("w.emheader({id}, false")),
            "java lost member id {id}"
        );
    }
}

#[test]
fn the_c_column_names_every_capacity_it_uses() {
    let header = generate("sensor.idl", "c", "h");
    assert!(header.contains("#define SENSOR_READING_LABEL_CAP"));
    assert!(header.contains("#define SENSOR_READING_RAW_CAP"));
    assert!(header.contains("zdw_u64_t stamp;"));
}
