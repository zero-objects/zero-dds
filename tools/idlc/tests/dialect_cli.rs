// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-idlc generate --dialect <name>` and its axis flags, at the
//! command line: every one of the eleven dialects is accepted, an unknown
//! dialect and an unknown axis value are refused with exit 2 and the
//! list of valid names, and the default — no `--dialect`, no axis flag
//! — still writes what `generate` always wrote.
//!
//! `default_extensibility.rs` is the model: spawn the real binary
//! rather than call `parse_args` in process, because exit code 2 is
//! `main`'s own contract (`std::process::exit(2)`) and not
//! `parse_args`'s return type.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};

const SOURCE: &str = "@mutable struct Sample {\n  @id(1) long a;\n  @id(2) double b;\n};\n";

/// A directory of this test's own, under the target directory so that a
/// `cargo clean` takes it with everything else — `default_extensibility.rs`'s
/// own shape.
fn tempdir(leaf: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("idlc-dialect-cli")
        .join(leaf);
    std::fs::create_dir_all(&dir).expect("the temp directory");
    dir
}

/// `generate --lang rust --dialect <dialect> [axis flags]`, over
/// [`SOURCE`]. Returns the process output; the caller reads
/// `status`/`stdout`/`stderr` rather than this function deciding what
/// counts as success, since the refusal tests want a failure and the
/// acceptance tests want the emitted file.
fn generate(leaf: &str, extra: &[&str]) -> (Output, PathBuf) {
    generate_with_source(leaf, SOURCE, extra)
}

fn generate_with_source(leaf: &str, source: &str, extra: &[&str]) -> (Output, PathBuf) {
    let dir = tempdir(leaf);
    let idl = dir.join("sample.idl");
    std::fs::write(&idl, source).expect("the source is written");
    let out = dir.join("out");
    std::fs::create_dir_all(&out).expect("the output directory");
    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"));
    command.args(["generate", "--lang", "rust", "--out"]);
    command.arg(&out);
    command.args(extra);
    command.arg(&idl);
    let output = command.output().expect("the binary runs");
    (output, out.join("sample.rs"))
}

/// Every one of the eleven dialects `zerodds_ast_layout::Dialect::ALL`
/// names is accepted: `generate` exits 0 and writes a file.
///
/// Not a claim about what each dialect changes in the bytes —
/// `tests/dialect_byte_equality.rs` and
/// `crates/wire-probe/tests/dialect_roundtrip.rs` make that claim, over
/// cases chosen because a dialect's preset actually differs there. This
/// is the flag's own contract: the name is spelled the way
/// `Dialect::as_str` spells it, and `generate` does not refuse a name it
/// should accept.
///
/// A dialect whose preset selects an axis the Rust codec does not carry
/// is accepted as a name and then refused by the generator with the axes
/// named — not exit 2, which is an unknown name. The Rust codec carries
/// every HDDS axis (`zerodds_lang_rules::encode::axes_the_rust_codec_carries`),
/// so this test asks that target-aware half rather than the non-Rust one.
#[test]
fn every_dialect_name_is_accepted() {
    for dialect in Dialect::ALL {
        let (output, file) = generate(
            &format!("accept-{}", dialect.as_str()),
            &["--dialect", dialect.as_str()],
        );
        let key = LayoutKey::in_dialect(
            dialect,
            Encoding::PlainCdr2,
            Endianness::Little,
            Purpose::Data,
        );
        if zerodds_lang_rules::encode::axes_the_rust_codec_carries(key).is_err() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.code() != Some(2) && stderr.contains("do not carry"),
                "--dialect {}: exited {:?}; the name has to be accepted and the codec \
                 refused with its axes named\n{stderr}",
                dialect.as_str(),
                output.status.code(),
            );
            continue;
        }
        assert!(
            output.status.success(),
            "--dialect {}: exited {:?}\n{}",
            dialect.as_str(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let text = std::fs::read_to_string(&file).expect("the emitted file");
        assert!(
            text.contains(&format!("dialect {}", dialect.as_str())) || dialect.is_default(),
            "--dialect {}: the preamble does not name it:\n{text}",
            dialect.as_str()
        );
    }
}

/// An unknown dialect name is refused with exit 2 and the list of valid
/// ones — the same contract `--lang` already has.
#[test]
fn an_unknown_dialect_is_refused_with_exit_2_and_the_valid_names() {
    let (output, _) = generate("unknown-dialect", &["--dialect", "not-a-real-dialect"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-real-dialect"), "{stderr}");
    for dialect in Dialect::ALL {
        assert!(
            stderr.contains(dialect.as_str()),
            "the refusal does not name {}:\n{stderr}",
            dialect.as_str()
        );
    }
}

/// An unknown value for a known axis flag is refused with exit 2 and the
/// value named, the same contract every other malformed flag has.
#[test]
fn an_unknown_axis_value_is_refused_with_exit_2() {
    let (output, _) = generate(
        "unknown-axis-value",
        &["--length-codes", "not-a-real-policy"],
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-real-policy"), "{stderr}");
}

/// An axis flag `--lang`/`--out`/etc. does not recognise stays refused
/// as an unknown option, and the axis-flag path does not swallow it.
#[test]
fn an_axis_flag_under_a_name_nothing_recognises_is_unknown() {
    let (output, _) = generate("unknown-flag", &["--not-an-axis-or-anything-else", "x"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--not-an-axis-or-anything-else"),
        "{stderr}"
    );
}

/// No `--dialect` and no axis flag: the preamble names neither, which is
/// the same text `generate` always wrote before this task started — the
/// one-line version of the before/after diff this task's own report
/// keeps in full over `tests/interop-corpus/`.
#[test]
fn the_default_names_no_dialect_and_no_axis_in_the_preamble() {
    let (output, file) = generate("default", &[]);
    assert!(output.status.success(), "{:?}", output.status);
    let text = std::fs::read_to_string(&file).expect("the emitted file");
    let header = text.lines().next().expect("a header line");
    assert!(!header.contains("dialect "), "{header}");
    for dialect in Dialect::ALL {
        if !dialect.is_default() {
            assert!(!header.contains(dialect.as_str()), "{header}");
        }
    }
}

/// A dialect whose own default extensibility departs from the
/// specification's (Cyclone reads an unannotated aggregate `@final`,
/// `Dialect::default_extensibility`) must not leave the decoder still
/// reading it as `@appendable`.
///
/// The regression this guards: `--dialect cyclone` alone used to move
/// only the `LayoutKey` the codec plans under — correctly dropping the
/// `DHEADER` — while `ComposeOptions::default_extensibility`, which
/// `Ast::effective_extensibility` answers from and the generated
/// decoder's evolution-tolerant `if r.remaining() == 0 { … }` pattern
/// reads, stayed unresolved and answered the specification's
/// `@appendable`. A stream this codec's own encoder writes — no
/// `DHEADER`, no room to detect a truncation — would then be decoded as
/// if a short read meant an older writer's smaller type, defaulting the
/// rest of the members instead of refusing the stream.
#[test]
fn a_dialects_own_default_extensibility_reaches_the_decoder_too() {
    // Unannotated on purpose: a source that *did* say `@appendable` or
    // `@final` still wins over the dialect, so the case this bug needs
    // is a struct whose extensibility is nobody's business but the
    // dialect's.
    let unannotated = "struct Sample {\n  long a;\n  double b;\n};\n";
    let (output, file) =
        generate_with_source("cyclone-final", unannotated, &["--dialect", "cyclone"]);
    assert!(output.status.success(), "{:?}", output.status);
    let text = std::fs::read_to_string(&file).expect("the emitted file");
    assert!(!text.contains("write_dheader"), "{text}");
    assert!(
        !text.contains("r.remaining() == 0"),
        "an @final decode must read unconditionally: {text}"
    );
}
