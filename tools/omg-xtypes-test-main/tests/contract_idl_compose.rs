// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-test-main` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-idl-compose` (`src/model.rs`, `compose`):
//! `compose_source_with` over one of the suite's IDL files with
//! `PathLoader::bare()` (the suite's files include nothing) and the
//! `key_on_union_discriminator` axis the suite's command line chooses; a
//! file is the suite's model only when `Composed::diagnostics` is empty,
//! and the model is `Composed::ast`.
//!
//! Oracle: DDS-XTypes 1.3 Table 21 and §7.2.2.4.4.4.8 (`@key` applies to a
//! union discriminator, `union MyCommand switch (@key CommandKind)`),
//! §7.3.4.5 with Annex B (the discriminator's member flags carry `IS_KEY` at
//! bit 5, as a struct member's do); IDL 4.2 §7.3 (an `#include` the loader
//! cannot open is an error of the unit).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_idl_compose::{
    ComposeOptions, KeyOnUnionDiscriminator, PathLoader, compose_source_with,
};
use zerodds_types::type_object::complete::CompleteTypeObject;
use zerodds_types::type_object::flags::UnionDiscriminatorFlag;

const KEYED_UNION: &str = "enum CommandKind { START, STOP };
union MyCommand switch (@key CommandKind) {
    case START: long start;
    case STOP: long stop;
};";

fn compose(source: &str, axis: KeyOnUnionDiscriminator) -> zerodds_idl_compose::Composed {
    compose_source_with(
        "suite.idl",
        source,
        &PathLoader::bare(),
        ComposeOptions {
            key_on_union_discriminator: axis,
            ..ComposeOptions::default()
        },
    )
}

#[test]
fn xtypes_7_2_2_4_4_4_8_the_clauses_keyed_discriminator_is_carried_into_the_type_object() {
    let composed = compose(KEYED_UNION, KeyOnUnionDiscriminator::Carried);
    assert!(
        composed.diagnostics.is_empty(),
        "{:?}",
        composed.diagnostics
    );
    let flags = composed
        .registry
        .iter_completes()
        .find_map(|(_, object)| match object {
            CompleteTypeObject::Union(u) if u.header.detail.type_name == "MyCommand" => {
                Some(u.discriminator.common.member_flags)
            }
            _ => None,
        })
        .expect("the COMPLETE object of MyCommand");
    assert!(flags.has(UnionDiscriminatorFlag::IS_KEY));
}

#[test]
fn xtypes_table_21_a_refused_key_on_a_discriminator_is_a_diagnostic() {
    let composed = compose(KEYED_UNION, KeyOnUnionDiscriminator::Refused);
    assert!(
        !composed.diagnostics.is_empty(),
        "the suite's command line asked for the refusal"
    );
}

#[test]
fn a_file_of_the_suite_composes_without_any_diagnostic() {
    let composed = compose(
        "@appendable struct Sample { @key long id; string<16> name; };",
        KeyOnUnionDiscriminator::Carried,
    );
    assert!(
        composed.diagnostics.is_empty(),
        "{:?}",
        composed.diagnostics
    );
    assert!(composed.ast.types().any(|(_, node)| node.name == "Sample"));
}

#[test]
fn idl_7_3_an_include_the_bare_loader_cannot_open_is_a_diagnostic() {
    let composed = compose(
        "#include \"not_in_the_suite.idl\"\nstruct S { long a; };",
        KeyOnUnionDiscriminator::Carried,
    );
    assert!(!composed.diagnostics.is_empty());
}
