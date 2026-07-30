//! Broad-audit P0-4 — cross-backend extensibility parity for the
//! `@extensibility(FINAL|APPENDABLE|MUTABLE)` LONG form.
//!
//! The effective extensibility is normalized ONCE in the semantic layer
//! (`zerodds_idl::semantics::extensibility_of`), which honors both the short
//! forms (`@final`/`@appendable`/`@mutable`) and the long form
//! `@extensibility(X)` (XTypes 1.3 §7.3.3). This test proves — through the REAL
//! emit paths — that C++, C and TS read the long form identically to Rust:
//! `@extensibility(MUTABLE)` → PL_CDR / EMHEADER wire, `@extensibility(FINAL)`
//! → plain (no DHEADER/EMHEADER). Before the fix C/C++/TS scanned only the
//! short forms, silently treating the long form as the APPENDABLE default and
//! drifting the wire from Rust/Java/Python.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::ast::types::Specification;
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, CppGenOptions, generate_c_header, generate_cpp_header};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};
use zerodds_idl_ts::generate_ts_source;

fn parse(src: &str) -> Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse")
}

fn cpp(src: &str) -> String {
    generate_cpp_header(&parse(src), &CppGenOptions::default()).expect("cpp gen")
}
fn c(src: &str) -> String {
    generate_c_header(&parse(src), &CGenOptions::default()).expect("c gen")
}
fn ts(src: &str) -> String {
    generate_ts_source(&parse(src)).expect("ts gen")
}
fn rust(src: &str) -> String {
    generate_rust_module(&parse(src), &RustGenOptions::default()).expect("rust gen")
}

/// `@extensibility(MUTABLE)` long form → every backend must emit the MUTABLE
/// wire form (PL_CDR / EMHEADER), not the APPENDABLE default.
#[test]
fn long_form_mutable_is_mutable_in_all_backends() {
    const SRC: &str = "@extensibility(MUTABLE) struct M { long a; };";

    // C++: extensibility marker MUTABLE + a PL_CDR/EMHEADER wire body.
    let cpp = cpp(SRC);
    assert!(
        cpp.contains("DataRepresentationKind::MUTABLE"),
        "C++ downgraded @extensibility(MUTABLE) long form"
    );
    assert!(
        !cpp.contains("DataRepresentationKind::APPENDABLE"),
        "C++ treated the long form as the APPENDABLE default"
    );
    assert!(
        cpp.contains("emheader") || cpp.contains("pl_cdr1_member_begin"),
        "C++ MUTABLE struct must carry EMHEADER/PL_CDR wire members"
    );

    // C: `.extensibility = 2` (MUTABLE) + PL member writer.
    let c = c(SRC);
    assert!(
        c.contains(".extensibility = 2"),
        "C downgraded @extensibility(MUTABLE) long form (expected 2)"
    );
    assert!(
        c.contains("pl1_write_member") || c.contains("emheader"),
        "C MUTABLE struct must carry the PL/EMHEADER wire path"
    );

    // TS: descriptor extensibility "mutable".
    let ts = ts(SRC);
    assert!(
        ts.contains("extensibility: \"mutable\""),
        "TS downgraded @extensibility(MUTABLE) long form"
    );

    // Rust reference: Extensibility::Mutable.
    let rust = rust(SRC);
    assert!(
        rust.contains("Extensibility::Mutable"),
        "Rust reference lost the MUTABLE long form"
    );
}

/// `@extensibility(FINAL)` long form → every backend must emit the plain wire
/// form (no DHEADER/EMHEADER).
#[test]
fn long_form_final_is_final_in_all_backends() {
    const SRC: &str = "@extensibility(FINAL) struct F { long a; };";

    // C++: FINAL marker + NO EMHEADER/PL_CDR frame.
    let cpp = cpp(SRC);
    assert!(
        cpp.contains("DataRepresentationKind::FINAL"),
        "C++ lost the @extensibility(FINAL) long form"
    );
    assert!(
        !cpp.contains("emheader") && !cpp.contains("pl_cdr1_member_begin"),
        "C++ FINAL struct must be plain (no EMHEADER/PL_CDR frame)"
    );

    // C: `.extensibility = 0` (FINAL) + no PL member writer.
    let c = c(SRC);
    assert!(
        c.contains(".extensibility = 0"),
        "C lost the @extensibility(FINAL) long form (expected 0)"
    );
    assert!(
        !c.contains("pl1_write_member") && !c.contains("emheader"),
        "C FINAL struct must be plain (no PL/EMHEADER wire path)"
    );

    // TS: descriptor extensibility "final".
    let ts = ts(SRC);
    assert!(
        ts.contains("extensibility: \"final\""),
        "TS lost the @extensibility(FINAL) long form"
    );

    // Rust reference: Extensibility::Final.
    let rust = rust(SRC);
    assert!(
        rust.contains("Extensibility::Final"),
        "Rust reference lost the FINAL long form"
    );
}

/// The long form and the short form MUST produce identical output — the whole
/// point of P0-4. `@extensibility(MUTABLE)` and `@mutable` on the same shape
/// must yield byte-identical backend output in C++, C, TS and Rust.
#[test]
fn long_form_and_short_form_are_identical() {
    const LONG: &str = "@extensibility(MUTABLE) struct S { long a; };";
    const SHORT: &str = "@mutable struct S { long a; };";

    assert_eq!(cpp(LONG), cpp(SHORT), "C++ long vs short form drift");
    assert_eq!(c(LONG), c(SHORT), "C long vs short form drift");
    assert_eq!(ts(LONG), ts(SHORT), "TS long vs short form drift");
    assert_eq!(rust(LONG), rust(SHORT), "Rust long vs short form drift");
}
