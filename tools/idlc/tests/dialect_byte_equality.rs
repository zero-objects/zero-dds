// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-idlc generate --lang rust --dialect <d>` produces Rust that,
//! compiled and run, writes the same bytes `wireprobe-zerodds --dialect
//! <d>` does.
//!
//! Item 2 and item 1 of task D3 both change what a generated file
//! contains — the `LayoutKey` the codec is planned under, and (for the
//! axes a `Writer` reads at construction rather than at a call site) the
//! framing the caller has to build the `Writer` with. Neither is
//! trustworthy from the text alone: `crates/lang-rules/tests/
//! rust_corpus_compiles.rs` already showed that a file can look right
//! and not compile, and a file that compiles can still choose the wrong
//! constant. This runs the generated code and reads the octets back.
//!
//! ## The cases
//!
//! [`crates/wire-probe/tests/dialect_roundtrip.rs`]'s seven, mirrored
//! here rather than shared: that file is a `[[test]]` binary of its own
//! in a different crate, and its `Case`/`cases()` are private to it.
//! Copying seven short cases is smaller and safer than making two
//! crates share test-only code across a crate boundary for the first
//! time. Each one keeps the dialect `dialect_roundtrip.rs`'s own doc
//! comment names as the one whose preset departs from the specification
//! on that case's axis — so this is drawing on that file's measurement,
//! not re-measuring it.
//!
//! ## Two tests, two costs
//!
//! [`the_fast_cases_are_byte_identical`] compiles each of the seven
//! cases once, under the one dialect that exercises it, and runs the
//! result under both byte orders — eight `rustc` invocations, not
//! eighty, so it runs in every `cargo test`. It also covers one axis
//! flag overriding a dialect's own answer (`--dialect fastdds
//! --length-codes compact`), which `crates/ast-layout/src/axes.rs`'s
//! `dialect_first_then_override` already checks in isolation; this
//! checks it end to end, through `rustc`.
//!
//! [`every_case_under_every_dialect_is_byte_identical`] is the literal
//! reading of the task: all seven cases, all eleven dialects, both byte
//! orders. `#[ignore]`, on the `rustc_corpus_compiles.rs` precedent —
//! seventy-seven `rustc` invocations belong behind `--include-ignored`, not in
//! the default gate every `cargo test -p zerodds-idlc` runs.
//!
//! `print_stderr` is allowed for the reason `rust_corpus_compiles.rs`
//! gives: a skipped compiler check has to be said, on the one channel a
//! passing test does not otherwise use.
//!
//! ## RTI's per-sample PL_CDR1 header
//!
//! The last two tests are the step `--dialect rti` adds: a nested
//! `@mutable` type that is small on its own is framed with the short
//! parameter header inside a sample that stays within the step and with
//! `PID_EXTENDED` inside one past it. The generated Rust resolves that
//! per sample — its `encode` entry asks `LayoutKey::for_sample` for the
//! sample whose root the type is and hands the answer to the body, which
//! threads it to every type it calls — and `zerodds_wire_codec`, the
//! comparison target, resolves the same crossing at its own root
//! (`crates/wire-codec/src/encode.rs`'s `for_sample`). A generated codec
//! that baked the step into the nested type instead would write the
//! short form in both samples.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use serde_json::{Value, json};
use zerodds_ast_layout::{AxisFlag, Dialect, Encoding, Endianness};
use zerodds_wire_probe::encode::{hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// One case: the IDL, the Rust literal for the same sample
/// `dialect_roundtrip.rs`'s `values` closure builds, and the dialect
/// whose preset that file documents as departing from the specification
/// on this axis.
#[derive(Clone, Copy)]
struct Case {
    /// What the case is for, in a failure message.
    what: &'static str,
    idl: &'static str,
    /// The generated file's top-level Rust type.
    type_name: &'static str,
    encoding: Encoding,
    values: fn() -> Value,
    /// A Rust expression of type `type_name`, hand-built from the same
    /// values `values()` returns. `the_literals_match_their_json` reads
    /// each one back through the wire-probe canonicaliser and checks it
    /// against `values()`, so a literal that drifted from its case
    /// fails there rather than being trusted.
    rust_literal: &'static str,
    dialect: Dialect,
    /// `--<axis> <value>` overrides applied after the dialect, empty for
    /// six of the seven.
    axis_overrides: &'static [(AxisFlag, &'static str)],
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            what: "a mutable struct under XCDR1 (PL_CDR1 headers and PID flags)",
            idl: "@mutable struct Ext { long a; double b; short c; };",
            type_name: "Ext",
            encoding: Encoding::PlCdr1,
            values: || json!({ "a": 7, "b": 1.5, "c": -3 }),
            rust_literal: "Ext { a: 7, b: 1.5, c: -3 }",
            dialect: Dialect::OpenDds,
            axis_overrides: &[],
        },
        Case {
            what: "an unannotated aggregate under XCDR2 (default extensibility)",
            idl: "\
struct Location { double x; double y; double z; };
struct Robot { Location spot; long tail; };",
            type_name: "Robot",
            encoding: Encoding::DelimitedCdr,
            values: || json!({ "spot": { "x": 1.0, "y": 2.0, "z": 3.0 }, "tail": -1 }),
            rust_literal: "Robot { spot: Location { x: 1.0, y: 2.0, z: 3.0 }, tail: -1 }",
            dialect: Dialect::Cyclone,
            axis_overrides: &[],
        },
        Case {
            what: "a sequence of enumerations under XCDR2 (collection delimiters)",
            idl: "\
enum Colour { RED, GREEN, BLUE };
@final struct SeqE { sequence<Colour> cs; long tail; };",
            type_name: "SeqE",
            encoding: Encoding::PlainCdr2,
            values: || json!({ "cs": ["BLUE", "RED"], "tail": -1 }),
            rust_literal: "SeqE { cs: vec![Colour::BLUE, Colour::RED], tail: -1 }",
            dialect: Dialect::Cyclone,
            axis_overrides: &[],
        },
        Case {
            what: "a wstring under XCDR1 (wstring length)",
            idl: "@final struct WStr { wstring w; octet tail; };",
            type_name: "WStr",
            encoding: Encoding::PlainCdr1,
            values: || json!({ "w": "ab", "tail": 171 }),
            rust_literal: "WStr { w: String::from(\"ab\"), tail: 171 }",
            dialect: Dialect::Rti,
            axis_overrides: &[],
        },
        Case {
            what: "a wchar under XCDR1 (wchar width)",
            idl: "@final struct WCh { octet head; wchar c; long n; };",
            type_name: "WCh",
            encoding: Encoding::PlainCdr1,
            values: || json!({ "head": 1, "c": "a", "n": 7 }),
            rust_literal: "WCh { head: 1, c: 'a', n: 7 }",
            dialect: Dialect::Rti,
            axis_overrides: &[],
        },
        Case {
            what: "the length codes of a mutable struct under XCDR2 (length codes)",
            idl: "\
@final struct Cell { long a; long b; };
@appendable struct ACell { long a; long b; };
@mutable struct LenNest { ACell app; long ap[2]; Cell nested; long tail; };",
            type_name: "LenNest",
            encoding: Encoding::PlCdr2,
            values: || {
                json!({
                    "app": { "a": 1, "b": 2 }, "ap": [7, 8],
                    "nested": { "a": 9, "b": 10 }, "tail": 11
                })
            },
            rust_literal: "LenNest { app: ACell { a: 1, b: 2 }, ap: [7, 8], \
                            nested: Cell { a: 9, b: 10 }, tail: 11 }",
            dialect: Dialect::FastDds,
            axis_overrides: &[],
        },
        Case {
            what: "a must-understand member under XCDR2 (must-understand)",
            idl: "@mutable struct MU { @id(1) @must_understand long a; @id(2) long b; };",
            type_name: "MU",
            encoding: Encoding::PlCdr2,
            values: || json!({ "a": 7, "b": -2 }),
            rust_literal: "MU { a: 7, b: -2 }",
            dialect: Dialect::FastDds,
            axis_overrides: &[],
        },
        // The eighth case: one axis flag overriding the dialect it
        // rides on. FastDDS's own preset is `short-where-it-fits`;
        // `--length-codes compact` has to win, the same claim
        // `axes.rs::dialect_first_then_override` makes for the key
        // alone, checked here through `rustc` instead.
        Case {
            what: "the length codes of a mutable struct, FastDDS overridden to compact",
            idl: "\
@final struct Cell { long a; long b; };
@appendable struct ACell { long a; long b; };
@mutable struct LenNest { ACell app; long ap[2]; Cell nested; long tail; };",
            type_name: "LenNest",
            encoding: Encoding::PlCdr2,
            values: || {
                json!({
                    "app": { "a": 1, "b": 2 }, "ap": [7, 8],
                    "nested": { "a": 9, "b": 10 }, "tail": 11
                })
            },
            rust_literal: "LenNest { app: ACell { a: 1, b: 2 }, ap: [7, 8], \
                            nested: Cell { a: 9, b: 10 }, tail: 11 }",
            dialect: Dialect::FastDds,
            axis_overrides: &[(AxisFlag::LengthCodes, "compact")],
        },
    ]
}

/// The bytes the wire-probe reflective encoder gives for `case` under
/// `dialect`/`case.axis_overrides` and `endianness` — the answer
/// `wireprobe-zerodds --dialect <d> [--<axis> <value>]` gives, computed
/// in process instead of through a second binary.
fn reflective_bytes(case: &Case, endianness: Endianness) -> Vec<u8> {
    let ast = model_from_idl_in_dialect(case.idl, "case.idl", case.dialect).expect("the model");
    let overrides: Vec<(AxisFlag, &str)> = case.axis_overrides.to_vec();
    let settings = zerodds_ast_layout::dialect_settings_from_overrides(
        case.dialect,
        case.encoding,
        endianness,
        zerodds_ast_layout::Purpose::Data,
        &overrides,
    )
    .expect("the axis overrides parse");
    let map_length = if settings.map_length_with_delimiter {
        zerodds_wire_probe::value::MapLengthPolicy::SequenceEquivalent
    } else {
        zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits
    };
    let request = ProbeRequest::from_key(case.type_name, settings.key, map_length);
    let sample = (case.values)();
    zerodds_wire_probe::encode::encode(&ast, &request, &sample).expect("the reflective encode")
}

// -- rustc, and what every generated file is compiled behind ------------

/// Whether `rustc` is usable at all.
fn rustc_available() -> bool {
    Command::new("rustc")
        .current_dir(repository_root())
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn work() -> PathBuf {
    std::env::temp_dir().join(format!("zd-idlc-dialect-bytes-{}", std::process::id()))
}

/// A scratch directory unique to the calling test thread, so that
/// [`the_fast_cases_are_byte_identical`] and
/// [`every_case_under_every_dialect_is_byte_identical`] — which the
/// default test harness may run on two threads at once — never write
/// the same `case.idl` or the same compiled binary out from under each
/// other. `work()` alone is shared on purpose (the dependency rlibs);
/// this is where the two tests' own scratch files must not be.
fn thread_work() -> PathBuf {
    work().join(format!("{:?}", std::thread::current().id()))
}

/// `zerodds-float128` and `zerodds-wire`, compiled once for the whole
/// test binary — `rustc` against an rlib another toolchain or another
/// concurrent test run wrote is `E0514`/`failed to open object file`,
/// which is a message about the harness and not about the generated
/// code. The same reasoning `rust_corpus_compiles.rs`'s `DEPENDENCY`
/// gives, extended to the runtime the codec calls and not only the
/// value type of `long double`.
static DEPENDENCIES: LazyLock<Result<(PathBuf, PathBuf), String>> = LazyLock::new(build_deps);

fn deps() -> Result<(PathBuf, PathBuf), String> {
    DEPENDENCIES.clone()
}

fn build_deps() -> Result<(PathBuf, PathBuf), String> {
    let directory = work().join("deps");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let float128 = directory.join("libzerodds_float128.rlib");
    let run = |crate_name: &str, source: &Path, out: &Path, externs: &[(&str, &Path)]| {
        let mut command = Command::new("rustc");
        command
            .current_dir(repository_root())
            .args(["--edition", "2024", "--crate-type=lib", "--crate-name"])
            .arg(crate_name)
            .arg("--out-dir")
            .arg(&directory)
            // Needed for `zerodds_wire`'s own build: it depends on
            // `zerodds_float128`, and an `--extern` naming one rlib's
            // path is not enough for rustc to resolve a transitive
            // dependency the *next* compile also needs by name — it
            // still asks its ordinary search path, which `-L` is.
            .arg("-L")
            .arg(&directory);
        for (name, path) in externs {
            command
                .arg("--extern")
                .arg(format!("{name}={}", path.display()));
        }
        command.arg(source);
        let output = command.output().map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(out.to_path_buf())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    };
    run(
        "zerodds_float128",
        &repository_root().join("crates/float128/src/lib.rs"),
        &float128,
        &[],
    )?;
    let wire = directory.join("libzerodds_wire.rlib");
    run(
        "zerodds_wire",
        &repository_root().join("crates/wire/src/lib.rs"),
        &wire,
        &[("zerodds_float128", &float128)],
    )?;
    Ok((float128, wire))
}

/// `Ext { a: 7, .. }.encode(&mut w)`'s `Writer`, as the `rustc` literal
/// that builds it — every axis a `Writer` reads at construction, not
/// baked into the generated call site. Built from the same
/// `zerodds_ast_layout` settings [`reflective_bytes`] used, so the
/// harness and the comparison target cannot drift apart from a second,
/// independent transcription of a dialect's numbers.
fn writer_prelude(case: &Case, endianness_expr: &str, max_align: u8) -> String {
    let overrides: Vec<(AxisFlag, &str)> = case.axis_overrides.to_vec();
    let settings = zerodds_ast_layout::dialect_settings_from_overrides(
        case.dialect,
        case.encoding,
        Endianness::Little, // the framing does not depend on byte order
        zerodds_ast_layout::Purpose::Data,
        &overrides,
    )
    .expect("the axis overrides parse");
    let map_length = if settings.map_length_with_delimiter {
        zerodds_wire_probe::value::MapLengthPolicy::SequenceEquivalent
    } else {
        zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits
    };
    let request = ProbeRequest::from_key(case.type_name, settings.key, map_length);
    let framing = request.pl_cdr1_framing();
    format!(
        "let mut w = zerodds_wire::Writer::with_max_align({endianness_expr}, {max_align})\n\
         \x20\x20\x20\x20.with_pl_cdr1_framing(zerodds_wire::PlCdr1Framing {{\n\
         \x20\x20\x20\x20\x20\x20\x20\x20pid_flags: zerodds_wire::PidFlagPolicy::{:?},\n\
         \x20\x20\x20\x20\x20\x20\x20\x20highest_short_id: {},\n\
         \x20\x20\x20\x20\x20\x20\x20\x20pad_declared_length: {},\n\
         \x20\x20\x20\x20\x20\x20\x20\x20pad_declared_optional_length: {},\n\
         \x20\x20\x20\x20\x20\x20\x20\x20member_must_understand: {},\n\
         \x20\x20\x20\x20\x20\x20\x20\x20require_extended: {},\n\
         \x20\x20\x20\x20\x20\x20\x20\x20pad_after_value: {},\n\
         \x20\x20\x20\x20}})\n\
         \x20\x20\x20\x20.with_wchar_width(zerodds_wire::WcharWidth::{:?})\n\
         \x20\x20\x20\x20.with_wchar_framing(zerodds_wire::WcharFraming::{:?})\n\
         \x20\x20\x20\x20.with_wstring_unit_order(zerodds_wire::WstringUnitOrder::{:?})\n\
         \x20\x20\x20\x20.with_short_length_codes({});\n",
        framing.pid_flags,
        framing.highest_short_id,
        framing.pad_declared_length,
        framing.pad_declared_optional_length,
        framing.member_must_understand,
        framing.require_extended,
        framing.pad_after_value,
        request.wchar_width(),
        request.wire_wchar_framing(),
        request.wire_wstring_unit_order(),
        request.length_codes.short_code_where_it_fits(),
    )
}

/// `zerodds-idlc generate --lang rust`, for `case` under its dialect and
/// axis overrides.
fn generate_rust(case: &Case, endianness: Endianness) -> String {
    let dir = thread_work().join("gen").join(case.type_name).join(
        case.axis_overrides
            .iter()
            .map(|(a, v)| format!("{}-{v}", a.flag_name()))
            .collect::<String>(),
    );
    std::fs::create_dir_all(&dir).expect("the generate scratch directory");
    let idl = dir.join("case.idl");
    std::fs::write(&idl, case.idl).expect("the source is written");
    let out = dir.join("out");
    std::fs::create_dir_all(&out).expect("the output directory");

    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"));
    command.args([
        "generate",
        "--lang",
        "rust",
        "--dialect",
        case.dialect.as_str(),
    ]);
    for (axis, value) in case.axis_overrides {
        command.arg(axis.flag_name()).arg(value);
    }
    command.args([
        "--encoding",
        encoding_flag(case.encoding),
        "--endian",
        if endianness == Endianness::Big {
            "be"
        } else {
            "le"
        },
        "--out",
    ]);
    command.arg(&out);
    command.arg(&idl);
    let output = command.output().expect("the binary runs");
    assert!(
        output.status.success(),
        "generate {} under {:?}: exited {:?}\n{}",
        case.what,
        case.dialect,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(out.join("case.rs")).expect("the emitted file")
}

fn encoding_flag(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::PlainCdr1 => "plain-cdr1",
        Encoding::PlCdr1 => "pl-cdr1",
        Encoding::PlainCdr2 => "plain-cdr2",
        Encoding::DelimitedCdr => "delimited-cdr",
        Encoding::PlCdr2 => "pl-cdr2",
    }
}

/// Compile `case`'s generated Rust plus a harness that encodes
/// `case.rust_literal` under both byte orders, and run it. Returns the
/// two hex strings, little endian first.
fn compile_and_run(case: &Case, generated: &str, tag: &str) -> (String, String) {
    let dir = thread_work().join("run").join(tag);
    std::fs::create_dir_all(&dir).expect("the run scratch directory");
    let (float128, wire) = deps().clone().expect("the dependencies compile");

    let le_prelude = writer_prelude(
        case,
        "zerodds_wire::Endianness::Little",
        case.encoding.max_alignment(),
    );
    let be_prelude = writer_prelude(
        case,
        "zerodds_wire::Endianness::Big",
        case.encoding.max_alignment(),
    );
    let harness = format!(
        "#![allow(dead_code, unused_imports, unused_mut)]\n\
         {generated}\n\
         fn hex(bytes: &[u8]) -> String {{\n\
         \x20\x20\x20\x20use std::fmt::Write as _;\n\
         \x20\x20\x20\x20let mut out = String::new();\n\
         \x20\x20\x20\x20for byte in bytes {{ let _ = write!(out, \"{{byte:02x}}\"); }}\n\
         \x20\x20\x20\x20out\n\
         }}\n\
         fn encode_le() -> Vec<u8> {{\n\
         \x20\x20\x20\x20{le_prelude}\n\
         \x20\x20\x20\x20let sample = {rust_literal};\n\
         \x20\x20\x20\x20sample.encode(&mut w).expect(\"encode\");\n\
         \x20\x20\x20\x20w.finish()\n\
         }}\n\
         fn encode_be() -> Vec<u8> {{\n\
         \x20\x20\x20\x20{be_prelude}\n\
         \x20\x20\x20\x20let sample = {rust_literal};\n\
         \x20\x20\x20\x20sample.encode(&mut w).expect(\"encode\");\n\
         \x20\x20\x20\x20w.finish()\n\
         }}\n\
         fn main() {{\n\
         \x20\x20\x20\x20println!(\"{{}}\", hex(&encode_le()));\n\
         \x20\x20\x20\x20println!(\"{{}}\", hex(&encode_be()));\n\
         }}\n",
        rust_literal = case.rust_literal,
    );
    let source = dir.join("main.rs");
    std::fs::write(&source, &harness).expect("the harness is written");
    let binary = dir.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let deps_dir = wire.parent().expect("the deps directory");
    let output = Command::new("rustc")
        .current_dir(repository_root())
        .args(["--edition", "2024", "--crate-type=bin"])
        .arg("-L")
        .arg(deps_dir)
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("-o")
        .arg(&binary)
        .arg(&source)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "{} ({tag}) did not compile:\n{}\n---\n{harness}",
        case.what,
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary).output().expect("the binary runs");
    assert!(
        run.status.success(),
        "{} ({tag}) did not run: {:?}\n{}",
        case.what,
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    let mut lines = stdout.lines();
    let le = lines.next().expect("the LE line").to_string();
    let be = lines.next().expect("the BE line").to_string();
    let _ = std::fs::remove_dir_all(&dir);
    (le, be)
}

/// Every hand-written [`Case::rust_literal`] is the same sample
/// [`Case::values`] is, read back through the wire-probe canonicaliser
/// — so a literal that drifted from its case is caught here rather than
/// trusted because it compiles.
#[test]
fn the_literals_match_their_json() {
    for case in cases() {
        let ast = model_from_idl_in_dialect(case.idl, "case.idl", case.dialect).expect("model");
        let type_id = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                case.type_name.to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        let sample = (case.values)();
        let canonical =
            zerodds_wire_probe::canonical::canonicalise(&ast, type_id, &sample, case.type_name)
                .expect("the sample canonicalises");
        // The literal is checked by what it produces on the wire, not by
        // parsing Rust: encode it and decode the bytes back, and compare
        // the decoded canonical form against the JSON sample's. A
        // literal with a field transposed or a wrong value still
        // compiles; it does not still round-trip to the same JSON.
        let endianness = Endianness::Little;
        let overrides: Vec<(AxisFlag, &str)> = case.axis_overrides.to_vec();
        let settings = zerodds_ast_layout::dialect_settings_from_overrides(
            case.dialect,
            case.encoding,
            endianness,
            zerodds_ast_layout::Purpose::Data,
            &overrides,
        )
        .expect("overrides parse");
        let map_length = if settings.map_length_with_delimiter {
            zerodds_wire_probe::value::MapLengthPolicy::SequenceEquivalent
        } else {
            zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits
        };
        let request = ProbeRequest::from_key(case.type_name, settings.key, map_length);
        let bytes = zerodds_wire_probe::encode::encode(&ast, &request, &sample).expect("encode");
        let decoded = zerodds_wire_probe::decode::decode(&ast, &request, &bytes).expect("decode");
        assert_eq!(
            zerodds_wire_probe::canonical::text(&decoded),
            zerodds_wire_probe::canonical::text(&canonical),
            "{}: the round trip through the bytes this case's own values() \
             produces does not match values() itself — the fixture, not \
             rust_literal, is what this asserts",
            case.what
        );
    }
}

/// The eight cases (seven axes, one axis flag overriding a dialect),
/// each under the one dialect that exercises it, both byte orders, one
/// `rustc` compile each.
#[test]
fn the_fast_cases_are_byte_identical() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    if let Err(detail) = deps() {
        eprintln!("SKIPPED: the runtime dependencies did not compile:\n{detail}");
        return;
    }
    for case in cases() {
        // `--endian` only reaches `ZD_ENDIANNESS`, the preamble comment
        // and `Default`'s hard-coded floats' literal spelling — never
        // the codec, which takes its order from the `Writer`/`Reader`
        // the caller builds, not from a constant the file names. The
        // harness below builds its own of both orders, so one
        // `--endian le` generation covers both.
        let generated = generate_rust(&case, Endianness::Little);
        let tag = format!(
            "{}{}",
            case.type_name,
            case.axis_overrides
                .iter()
                .map(|(a, v)| format!("-{}-{v}", a.flag_name()))
                .collect::<String>()
        );
        let (le_hex, be_hex) = compile_and_run(&case, &generated, &tag);
        let expected_le = hex(&reflective_bytes(&case, Endianness::Little));
        let expected_be = hex(&reflective_bytes(&case, Endianness::Big));
        assert_eq!(
            le_hex, expected_le,
            "{} under {:?}, little endian: generated Rust disagrees with the \
             reflective encoder",
            case.what, case.dialect
        );
        assert_eq!(
            be_hex, expected_be,
            "{} under {:?}, big endian: generated Rust disagrees with the \
             reflective encoder",
            case.what, case.dialect
        );
    }
}

/// The literal reading of the task: every one of the seven axis cases,
/// every one of the eleven dialects, both byte orders — seventy-seven `rustc`
/// invocations. `#[ignore]`: `cargo test -p zerodds-idlc --include-ignored`
/// runs it; the default `cargo test` does not, on the
/// `rust_corpus_compiles.rs` precedent for the same reason.
#[test]
#[ignore = "seventy-seven rustc invocations; run with --include-ignored"]
fn every_case_under_every_dialect_is_byte_identical() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    if let Err(detail) = deps() {
        eprintln!("SKIPPED: the runtime dependencies did not compile:\n{detail}");
        return;
    }
    // The seven axis cases only — the eighth (the axis-flag override) is
    // `the_fast_cases_are_byte_identical`'s alone: it is one dialect and
    // one flag, not a claim about every dialect.
    for case in cases()
        .into_iter()
        .filter(|case| case.axis_overrides.is_empty())
    {
        for dialect in Dialect::ALL {
            let under_dialect = Case {
                dialect,
                axis_overrides: &[],
                ..case
            };
            let generated = generate_rust(&under_dialect, Endianness::Little);
            let tag = format!("{}-{}", under_dialect.type_name, dialect.as_str());
            let (le_hex, be_hex) = compile_and_run(&under_dialect, &generated, &tag);
            let expected_le = hex(&reflective_bytes(&under_dialect, Endianness::Little));
            let expected_be = hex(&reflective_bytes(&under_dialect, Endianness::Big));
            assert_eq!(
                le_hex, expected_le,
                "{} under {:?}, little endian",
                under_dialect.what, dialect
            );
            assert_eq!(
                be_hex, expected_be,
                "{} under {:?}, big endian",
                under_dialect.what, dialect
            );
        }
    }
}

/// A nested `@mutable` type that is small on its own, in two samples:
/// `Small`'s type stays within RTI's short-header step and `Large`'s
/// passes it.
///
/// The same IDL as `dialect_c_byte_equality.rs`'s `RTI_STEP_IDL`, so the
/// C and the Rust columns are compared on one shape and one pair of
/// samples. The nested `Inner.s` is framed short in one and with
/// `PID_EXTENDED` in the other, and `id` (a `long`) keeps the short
/// header in both.
///
/// One [`Case`] per root, so the file's own helpers —
/// [`generate_rust`], [`compile_and_run`], [`writer_prelude`] and
/// [`reflective_bytes`] — carry the generation, the compile, the
/// `Writer`'s framing and the expected octets, with no second
/// transcription of RTI's numbers here.
fn rti_step_cases() -> Vec<Case> {
    let idl = "\
module m {
  @mutable struct Inner { long a; sequence<long, 100> s; };
  @mutable struct Small { long id; Inner part; sequence<octet, 100> big; };
  @mutable struct Large { long id; Inner part; sequence<octet, 65532> big; };
};";
    vec![
        Case {
            what: "the nested type itself, small on its own, as the sample's root",
            idl,
            type_name: "m::Inner",
            encoding: Encoding::PlCdr1,
            values: || json!({ "a": 2, "s": [3] }),
            rust_literal: "m::Inner { a: 2, s: vec![3] }",
            dialect: Dialect::Rti,
            axis_overrides: &[],
        },
        Case {
            what: "a mutable struct below RTI's short-header step, XCDR1",
            idl,
            type_name: "m::Small",
            encoding: Encoding::PlCdr1,
            values: || json!({ "id": 1, "part": { "a": 2, "s": [3] }, "big": [4] }),
            rust_literal: "m::Small { id: 1, part: m::Inner { a: 2, s: vec![3] }, big: vec![4] }",
            dialect: Dialect::Rti,
            axis_overrides: &[],
        },
        Case {
            what: "the same struct past RTI's short-header step, XCDR1",
            idl,
            type_name: "m::Large",
            encoding: Encoding::PlCdr1,
            values: || json!({ "id": 1, "part": { "a": 2, "s": [3] }, "big": [4] }),
            rust_literal: "m::Large { id: 1, part: m::Inner { a: 2, s: vec![3] }, big: vec![4] }",
            dialect: Dialect::Rti,
            axis_overrides: &[],
        },
    ]
}

/// One `rustc` compile per root, both byte orders, and the two hex
/// strings the harness printed, little endian first.
///
/// `--endian` only reaches the preamble comment and `Default`'s floats,
/// never the codec — the order is the `Writer`'s — so one generation
/// covers both, on `the_fast_cases_are_byte_identical`'s reasoning.
fn rti_step_run(case: &Case) -> (String, String) {
    let generated = generate_rust(case, Endianness::Little);
    let tag = format!("rti-step-{}", case.type_name.replace("::", "-"));
    compile_and_run(case, &generated, &tag)
}

/// Compile `case`'s generated Rust plus a harness that encodes
/// `case.rust_literal` and prints the hex, and return it.
///
/// [`compile_and_run`]'s shape without its second byte order: the
/// question here is whether the reflective decoder can read what the
/// generated writer wrote, and one order answers that.
fn compile_and_run_once(case: &Case, generated: &str, tag: &str) -> String {
    let dir = thread_work().join("run").join(tag);
    std::fs::create_dir_all(&dir).expect("the run scratch directory");
    let (float128, wire) = deps().clone().expect("the dependencies compile");

    let le_prelude = writer_prelude(
        case,
        "zerodds_wire::Endianness::Little",
        case.encoding.max_alignment(),
    );
    let harness = format!(
        "#![allow(dead_code, unused_imports, unused_mut)]\n\
         {generated}\n\
         fn hex(bytes: &[u8]) -> String {{\n\
         \x20\x20\x20\x20use std::fmt::Write as _;\n\
         \x20\x20\x20\x20let mut out = String::new();\n\
         \x20\x20\x20\x20for byte in bytes {{ let _ = write!(out, \"{{byte:02x}}\"); }}\n\
         \x20\x20\x20\x20out\n\
         }}\n\
         fn encode_le() -> Vec<u8> {{\n\
         \x20\x20\x20\x20{le_prelude}\n\
         \x20\x20\x20\x20let sample = {rust_literal};\n\
         \x20\x20\x20\x20sample.encode(&mut w).expect(\"encode\");\n\
         \x20\x20\x20\x20w.finish()\n\
         }}\n\
         fn main() {{\n\
         \x20\x20\x20\x20println!(\"{{}}\", hex(&encode_le()));\n\
         }}\n",
        rust_literal = case.rust_literal,
    );
    let source = dir.join("main.rs");
    std::fs::write(&source, &harness).expect("the harness is written");
    let binary = dir.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let deps_dir = wire.parent().expect("the deps directory");
    let output = Command::new("rustc")
        .current_dir(repository_root())
        .args(["--edition", "2024", "--crate-type=bin"])
        .arg("-L")
        .arg(deps_dir)
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("-o")
        .arg(&binary)
        .arg(&source)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "{} ({tag}) did not compile:\n{}\n---\n{harness}",
        case.what,
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary).output().expect("the binary runs");
    assert!(
        run.status.success(),
        "{} ({tag}) did not run: {:?}\n{}",
        case.what,
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    let got = stdout.lines().next().expect("the hex line").to_string();
    let _ = std::fs::remove_dir_all(&dir);
    got
}

/// RTI decides the PL_CDR1 header form for the **sample's** type, so
/// `Small` and `Large` write different octets for the *same* nested
/// `Inner`, and the generated Rust has to differ with them.
///
/// A codec that planned the header once per type would frame `Inner`
/// short in both — the answer for a sample below the step — and `Large`'s
/// parameter for the nested type would be four octets where the
/// reflective encoder writes twelve.
#[test]
fn generated_rust_under_rti_frames_a_nested_type_by_its_sample() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    if let Err(detail) = deps() {
        eprintln!("SKIPPED: the runtime dependencies did not compile:\n{detail}");
        return;
    }
    let cases = rti_step_cases();
    let mut small = String::new();
    let mut large = String::new();
    for case in &cases {
        let (le_hex, be_hex) = rti_step_run(case);
        for (endianness, got) in [(Endianness::Little, &le_hex), (Endianness::Big, &be_hex)] {
            assert_eq!(
                got,
                &hex(&reflective_bytes(case, endianness)),
                "{} under {:?}, {} endian: generated Rust disagrees with the reflective encoder",
                case.what,
                case.dialect,
                if endianness == Endianness::Little {
                    "little"
                } else {
                    "big"
                }
            );
        }
        match case.type_name {
            "m::Small" => small = le_hex,
            "m::Large" => large = le_hex,
            _ => {}
        }
    }
    assert!(
        !small.is_empty() && !large.is_empty(),
        "both samples ran: small={small:?} large={large:?}"
    );
    assert_ne!(
        small, large,
        "Small and Large wrote the same octets; RTI's short-header step did not reach the nested \
         encoder"
    );
}

/// The other end of the split: the nested type reached as a **root**
/// keeps the short header, and the reflective decoder reads both samples
/// back.
///
/// The generated `decode` needs no per-sample knowledge at all —
/// `Reader::read_pid_as` takes the header form off the wire
/// (`crates/wire/src/reader.rs`) — and that is a property worth pinning
/// rather than assuming: a decoder that took the form from the plan, as
/// the encoder used to, would read a `Large` sample's nested parameter
/// as four octets and take everything after it from the wrong offset.
/// Each sample's generated octets are decoded here by
/// `zerodds_wire_probe`, and the values it reads back are compared with
/// the case's own `values()`.
#[test]
fn generated_rust_under_rti_is_read_back_by_the_reflective_decoder() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    if let Err(detail) = deps() {
        eprintln!("SKIPPED: the runtime dependencies did not compile:\n{detail}");
        return;
    }
    for case in rti_step_cases() {
        let generated = generate_rust(&case, Endianness::Little);
        let tag = format!("rti-read-{}", case.type_name.replace("::", "-"));
        let hex_line = compile_and_run_once(&case, &generated, &tag);

        let ast = model_from_idl_in_dialect(case.idl, "case.idl", case.dialect).expect("the model");
        let overrides: Vec<(AxisFlag, &str)> = case.axis_overrides.to_vec();
        let settings = zerodds_ast_layout::dialect_settings_from_overrides(
            case.dialect,
            case.encoding,
            Endianness::Little,
            zerodds_ast_layout::Purpose::Data,
            &overrides,
        )
        .expect("the axis overrides parse");
        let map_length = if settings.map_length_with_delimiter {
            zerodds_wire_probe::value::MapLengthPolicy::SequenceEquivalent
        } else {
            zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits
        };
        let request = ProbeRequest::from_key(case.type_name, settings.key, map_length);
        let bytes = (0..hex_line.len() / 2)
            .map(|i| u8::from_str_radix(&hex_line[i * 2..i * 2 + 2], 16).expect("a hex octet"))
            .collect::<Vec<u8>>();
        let decoded =
            zerodds_wire_probe::decode::decode(&ast, &request, &bytes).unwrap_or_else(|error| {
                panic!(
                    "{}: the reflective decoder refused {hex_line}: {error}",
                    case.what
                )
            });
        // Split the scoped name the way `zerodds_wire_codec`'s own
        // `find_type` does, so the two ask the index the same question:
        // the cases here are in a module, and `m::Small` is two segments.
        let segments = case
            .type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let type_id = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        let sample = (case.values)();
        let canonical =
            zerodds_wire_probe::canonical::canonicalise(&ast, type_id, &sample, case.type_name)
                .expect("the sample canonicalises");
        assert_eq!(
            zerodds_wire_probe::canonical::text(&decoded),
            zerodds_wire_probe::canonical::text(&canonical),
            "{}: the generated Rust's octets do not read back as the sample that produced them",
            case.what
        );
    }
}

// -- the three HDDS axes, against what HDDS wrote ------------------------

/// Five corpus cases, one per shape the three axes `Dialect::Hdds` adds
/// to a generated codec reach: an array of `@final` structs (each
/// element starts its own alignment block), a sequence of `@appendable`
/// structs (a length before every element) and `wstring` as a member, a
/// sequence element and an array element (UTF-8 octets). The expected
/// octets are not this tree's: they are the files HDDS's own serialiser
/// wrote, `tests/interop-results/<case>/hdds/<encoding>.le.bin`.
fn hdds_cases() -> Vec<(&'static str, Case)> {
    let case = |what, idl, type_name, encoding, rust_literal| Case {
        what,
        idl,
        type_name,
        encoding,
        // Not read: the octets come from HDDS, not from the reflective
        // encoder, so no JSON sample is needed for the comparison.
        values: || Value::Null,
        rust_literal,
        dialect: Dialect::Hdds,
        axis_overrides: &[],
    };
    let mut out = Vec::new();
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        out.push((
            "array-1d-struct-final",
            case(
                "an array of final structs under HDDS",
                include_str!("../../../tests/interop-corpus/array-1d-struct-final/type.idl"),
                "Sample",
                encoding,
                "Sample { items: [Cell { a: 1, b: 2 }, Cell { a: 3, b: 4 }], tail: 119 }",
            ),
        ));
        out.push((
            "sequence-struct-appendable",
            case(
                "a sequence of appendable structs under HDDS",
                include_str!("../../../tests/interop-corpus/sequence-struct-appendable/type.idl"),
                "Sample",
                encoding,
                "Sample { none: vec![], one: vec![Cell { a: -1234567, b: -2 }], \
                 many: vec![Cell { a: 1, b: 2 }, Cell { a: -1, b: 32767 }, \
                 Cell { a: 2147483647, b: -32768 }], tail: -1 }",
            ),
        ));
        out.push((
            "wstring-member",
            case(
                "a wstring member under HDDS",
                include_str!("../../../tests/interop-corpus/wstring-member/type.idl"),
                "m::S",
                encoding,
                "m::S { w: \"abc\".to_string() }",
            ),
        ));
        out.push((
            "sequence-wstring",
            case(
                "sequences of wstring under HDDS",
                include_str!("../../../tests/interop-corpus/sequence-wstring/type.idl"),
                "Sample",
                encoding,
                "Sample { none: vec![], one: vec![\"\u{e4}Z\".to_string()], \
                 many: vec![String::new(), \"\u{e4}Z\".to_string(), \"\u{1234}\".to_string()], \
                 bounded: vec![String::new(), \"\u{e4}Z\".to_string()], tail: -1 }",
            ),
        ));
        out.push((
            "array-1d-wstring",
            case(
                "an array of wstring under HDDS",
                include_str!("../../../tests/interop-corpus/array-1d-wstring/type.idl"),
                "Sample",
                encoding,
                "Sample { items: [String::new(), \"\u{e4}Z\".to_string(), \"\u{1234}\".to_string()], \
                 tail: -1 }",
            ),
        ));
    }
    out
}

/// The recorded HDDS file for one case and encoding, as hex.
fn hdds_recorded(corpus_case: &str, encoding: Encoding) -> String {
    let path = repository_root()
        .join("tests/interop-results")
        .join(corpus_case)
        .join("hdds")
        .join(format!("{}.le.bin", encoding_flag(encoding)));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    hex(&bytes)
}

/// The generated decoder over HDDS's octets: it has to give back the
/// sample the literal names and consume every octet.
fn compile_and_decode(case: &Case, generated: &str, tag: &str, recorded: &str) -> String {
    let dir = thread_work().join("run").join(tag);
    std::fs::create_dir_all(&dir).expect("the run scratch directory");
    let (float128, wire) = deps().clone().expect("the dependencies compile");
    let ProbeRequestParts {
        wchar_width,
        wchar_framing,
        unit_order,
    } = request_parts(case);
    let octets = (0..recorded.len() / 2)
        .map(|i| format!("0x{}", &recorded[i * 2..i * 2 + 2]))
        .collect::<Vec<_>>()
        .join(", ");
    let harness = format!(
        "#![allow(dead_code, unused_imports, unused_mut)]\n\
         {generated}\n\
         fn main() {{\n\
         \x20\x20\x20\x20let bytes: &[u8] = &[{octets}];\n\
         \x20\x20\x20\x20let mut r = zerodds_wire::Reader::with_max_align(bytes, zerodds_wire::Endianness::Little, {max_align})\n\
         \x20\x20\x20\x20\x20\x20\x20\x20.with_wchar_width(zerodds_wire::WcharWidth::{wchar_width:?})\n\
         \x20\x20\x20\x20\x20\x20\x20\x20.with_wchar_framing(zerodds_wire::WcharFraming::{wchar_framing:?})\n\
         \x20\x20\x20\x20\x20\x20\x20\x20.with_wstring_unit_order(zerodds_wire::WstringUnitOrder::{unit_order:?});\n\
         \x20\x20\x20\x20let got = {type_name}::decode(&mut r).expect(\"decode\");\n\
         \x20\x20\x20\x20let want = {rust_literal};\n\
         \x20\x20\x20\x20println!(\"{{}} {{}}\", if got == want {{ \"same\" }} else {{ \"differs\" }}, r.remaining());\n\
         \x20\x20\x20\x20if got != want {{ eprintln!(\"{{got:?}}\"); }}\n\
         }}\n",
        max_align = case.encoding.max_alignment(),
        type_name = case.type_name,
        rust_literal = case.rust_literal,
    );
    let source = dir.join("main.rs");
    std::fs::write(&source, &harness).expect("the harness is written");
    let binary = dir.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let deps_dir = wire.parent().expect("the deps directory");
    let output = Command::new("rustc")
        .current_dir(repository_root())
        .args(["--edition", "2024", "--crate-type=bin"])
        .arg("-L")
        .arg(deps_dir)
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("-o")
        .arg(&binary)
        .arg(&source)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "{} ({tag}) did not compile:\n{}\n---\n{harness}",
        case.what,
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary).output().expect("the binary runs");
    assert!(
        run.status.success(),
        "{} ({tag}) did not run: {:?}\n{}",
        case.what,
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let line = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout).trim(),
        String::from_utf8_lossy(&run.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    line
}

struct ProbeRequestParts {
    wchar_width: zerodds_wire::WcharWidth,
    wchar_framing: zerodds_wire::WcharFraming,
    unit_order: zerodds_wire::WstringUnitOrder,
}

/// The `wchar`/`wstring` settings `writer_prelude` gives the `Writer`,
/// for the `Reader` of the same case.
fn request_parts(case: &Case) -> ProbeRequestParts {
    let settings = zerodds_ast_layout::dialect_settings_from_overrides(
        case.dialect,
        case.encoding,
        Endianness::Little,
        zerodds_ast_layout::Purpose::Data,
        &[],
    )
    .expect("the dialect's settings");
    let request = ProbeRequest::from_key(
        case.type_name,
        settings.key,
        zerodds_wire_probe::value::MapLengthPolicy::Rule15Omits,
    );
    ProbeRequestParts {
        wchar_width: request.wchar_width(),
        wchar_framing: request.wire_wchar_framing(),
        unit_order: request.wire_wstring_unit_order(),
    }
}

/// Generated Rust under `--dialect hdds` writes the octets HDDS wrote for
/// the five cases, under PLAIN_CDR and PLAIN_CDR2, and reads them back as
/// the same sample with nothing left over.
#[test]
fn generated_rust_under_hdds_writes_and_reads_what_hdds_wrote() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    if let Err(detail) = deps() {
        eprintln!("SKIPPED: the runtime dependencies did not compile:\n{detail}");
        return;
    }
    for (corpus_case, case) in hdds_cases() {
        let recorded = hdds_recorded(corpus_case, case.encoding);
        let generated = generate_rust(&case, Endianness::Little);
        let tag = format!("hdds-{corpus_case}-{}", encoding_flag(case.encoding));
        let (le, _be) = compile_and_run(&case, &generated, &tag);
        assert_eq!(
            le,
            recorded,
            "{corpus_case} ({}): generated Rust under hdds does not write \
             tests/interop-results/{corpus_case}/hdds/{}.le.bin",
            encoding_flag(case.encoding),
            encoding_flag(case.encoding)
        );
        let read = compile_and_decode(&case, &generated, &format!("{tag}-read"), &recorded);
        assert_eq!(
            read,
            "same 0",
            "{corpus_case} ({}): generated Rust under hdds does not read HDDS's octets back \
             as the sample",
            encoding_flag(case.encoding)
        );
    }
}
