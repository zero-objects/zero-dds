// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wire-probe` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-idl-compose` (`src/encode.rs`,
//! `model_from_idl_with`): `compose_source_with` with `PathLoader::bare()`,
//! the lowering axes of the dialect it reproduces (default extensibility,
//! the `@hashid` counter and spelling, the enclosing-scope name),
//! `@key` on a union discriminator carried, every vendor's extensions
//! admitted; a model is the probe's only when `Composed::diagnostics` is
//! empty. The probe needs the model, not the type objects: a type the
//! registration cannot represent is listed in `without_type_objects` and
//! is no diagnostic (`crates/idl-compose/CHANGELOG.md`, Fixed).
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5.3.2 (a union case label goes into the
//! TypeObject as a `long`; the corpus case `union-discriminator-longlong`,
//! which Fast DDS and omniORB write, has a label outside it), Table 21 and
//! §7.2.2.4.4.4.8 (`@key` on a union discriminator) and §7.3.1.2.1.8 (an
//! unannotated aggregate is APPENDABLE unless told).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast::Extensibility;
use zerodds_idl_compose::{
    ComposeOptions, KeyOnUnionDiscriminator, PathLoader, compose_source_with,
};

fn probe_options() -> ComposeOptions {
    ComposeOptions {
        key_on_union_discriminator: KeyOnUnionDiscriminator::Carried,
        ..ComposeOptions::default()
    }
}

#[test]
fn xtypes_7_3_4_5_3_2_a_label_outside_long_is_no_diagnostic_and_the_types_are_listed() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/interop-corpus/union-discriminator-longlong/type.idl"
    ))
    .unwrap();
    let composed = compose_source_with("type.idl", &source, &PathLoader::bare(), probe_options());
    assert!(
        composed.diagnostics.is_empty(),
        "{:?}",
        composed.diagnostics
    );
    let listed: Vec<&str> = composed
        .without_type_objects
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        listed.contains(&"Choice") && listed.contains(&"Sample"),
        "{listed:?}"
    );
}

#[test]
fn xtypes_7_2_2_4_4_4_8_a_keyed_discriminator_is_carried_without_a_diagnostic() {
    let composed = compose_source_with(
        "k.idl",
        "enum Kind { A, B }; union U switch (@key Kind) { case A: long a; case B: long b; };",
        &PathLoader::bare(),
        probe_options(),
    );
    assert!(
        composed.diagnostics.is_empty(),
        "{:?}",
        composed.diagnostics
    );
}

#[test]
fn xtypes_7_3_1_2_1_8_silence_is_appendable_and_a_dialects_default_fills_it() {
    let source = "struct Silent { long a; };";
    let of = |options: ComposeOptions| {
        let composed = compose_source_with("s.idl", source, &PathLoader::bare(), options);
        let (id, _) = composed
            .ast
            .types()
            .find(|(_, n)| n.name == "Silent")
            .unwrap();
        composed.ast.effective_extensibility(id)
    };
    assert_eq!(of(probe_options()), Extensibility::Appendable);
    assert_eq!(
        of(ComposeOptions {
            default_extensibility: Some(Extensibility::Final),
            ..probe_options()
        }),
        Extensibility::Final
    );
}
