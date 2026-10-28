// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Config::dialect` and `Config::axis` reach the generated codec, the
//! same way `zerodds-idlc generate --dialect`/`--<axis>` and
//! `wireprobe-zerodds --dialect`/`--<axis>` do — read from the same
//! table, `crates/ast-layout/src/axes.rs`.
//!
//! The case is default extensibility, not PL_CDR1 framing or length
//! codes: those two are runtime-level (`zerodds_wire::Writer`'s own
//! constructor arguments, per `tools/idlc/tests/dialect_byte_equality.rs`'s
//! module doc), so a Rust caller who builds the `Writer` decides them
//! and the generated text does not change. Default extensibility is a
//! `LayoutKey` axis the codec's own text depends on — a `DHEADER`
//! `write_dheader`/`finish_dheader` pair for `@appendable`/`@mutable`,
//! none for `@final` — which is what makes it visible here as a text
//! difference and not only as a byte difference.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;

use zerodds_ast_layout::{AxisFlag, Dialect};
use zerodds_build::{Config, Error};

const SOURCE: &str = "struct Sample { long id; double reading; };";

fn generate(configure: impl FnOnce(&mut Config) -> &mut Config) -> Result<String, Error> {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Sample.idl");
    fs::write(&idl, SOURCE).expect("write idl");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("out dir");
    let mut config = Config::new();
    config.out_dir(&out_dir).emit_cargo_directives(false);
    configure(&mut config);
    config.compile(&[&idl])?;
    Ok(fs::read_to_string(out_dir.join("Sample.rs")).expect("read generated"))
}

/// No `dialect()` call: the specification's own default, `@appendable`
/// for a source that says nothing — XTypes 1.3 §7.3.1.2.1.8 — so a
/// `DHEADER` is written. The same claim `compile.rs`'s other tests make
/// implicitly; asserted here as the baseline the two tests below are
/// differences from.
#[test]
fn no_dialect_call_is_the_specifications_default() {
    let generated = generate(|config| config).expect("compile");
    assert!(
        generated.contains("write_dheader") && generated.contains("finish_dheader"),
        "the default is @appendable, which is a DHEADER: {generated}"
    );
}

/// `Dialect::Cyclone` reads an unannotated aggregate as `@final` —
/// `crates/ast-layout/src/key.rs`'s `Dialect::default_extensibility` —
/// so the same source loses its `DHEADER`, on both the encoder and the
/// decoder.
///
/// The decode half is the regression this test exists for: `dialect()`
/// alone, with no `axis()`/`default_extensibility()` call, used to move
/// `LayoutKey::default_extensibility` (so the codec correctly stopped
/// writing a `DHEADER`) without moving `ComposeOptions::default_extensibility`,
/// which stayed unresolved and left `Ast::effective_extensibility`
/// answering `@appendable`. The decoder that generates from — the
/// evolution-tolerant `if r.remaining() == 0 { <default> } else { <read> }`
/// pattern XTypes 1.3 §7.2.4.4's evolution rules give `@appendable` —
/// would then default a truncated member of an `@final`-framed stream
/// instead of refusing it, silently.
#[test]
fn dialect_reaches_default_extensibility() {
    let generated =
        generate(|config| config.dialect(Dialect::Cyclone)).expect("compile under Cyclone");
    assert!(
        !generated.contains("write_dheader") && !generated.contains("finish_dheader"),
        "Cyclone's default is @final, which has no DHEADER: {generated}"
    );
    assert!(
        !generated.contains("r.remaining() == 0"),
        "an @final decode must read unconditionally, not default a member the \
         way an @appendable evolution reader may: {generated}"
    );
}

/// `axis(AxisFlag::DefaultExtensibility, "final")` reaches the same
/// setting `--default-extensibility final` and `Config::default_extensibility`
/// already did, through the shared table instead of the axis's own
/// dedicated method — the same answer, a second path to it.
#[test]
fn the_axis_flag_reaches_the_same_setting_as_the_dedicated_method() {
    let via_axis = generate(|config| config.axis(AxisFlag::DefaultExtensibility, "final"))
        .expect("compile with the axis override");
    let via_method =
        generate(|config| config.default_extensibility(zerodds_build::DefaultExt::Final))
            .expect("compile with the dedicated method");
    assert_eq!(via_axis, via_method);
    assert!(!via_axis.contains("write_dheader"), "{via_axis}");
}

/// A dialect first, then the axis flag overriding one of its answers —
/// OpenDDS's own default extensibility is `@appendable` (like the
/// specification's), so `final` here is visibly the flag's doing and
/// not the dialect's.
#[test]
fn the_axis_flag_overrides_the_dialect_in_declaration_order() {
    let generated = generate(|config| {
        config
            .dialect(Dialect::OpenDds)
            .axis(AxisFlag::DefaultExtensibility, "final")
    })
    .expect("compile under OpenDDS, overridden to final");
    assert!(
        !generated.contains("write_dheader"),
        "the axis flag must win over the dialect's own appendable: {generated}"
    );
}

/// An axis value the axis does not recognise is `Error::Axis`, naming
/// the flag and the value — there is no command line to refuse it at
/// here, so `compile()`'s `Result` is where the typo surfaces.
#[test]
fn an_unknown_axis_value_is_refused_by_name() {
    let error = generate(|config| config.axis(AxisFlag::DefaultExtensibility, "not-a-real-value"))
        .expect_err("an unknown axis value must not compile");
    let Error::Axis {
        flag,
        value,
        detail,
    } = error
    else {
        panic!("expected Error::Axis, got {error:?}");
    };
    assert_eq!(flag, "--default-extensibility");
    assert_eq!(value, "not-a-real-value");
    assert!(detail.contains("not-a-real-value"), "{detail}");
}
