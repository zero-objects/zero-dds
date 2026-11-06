// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-build` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-idl-compose` (`src/lib.rs`,
//! `Config::compile` and `compile_bundle`; `src/dds_type.rs`): `PathLoader`
//! over the configured include directories, whose `opened()` becomes the
//! build script's `cargo:rerun-if-changed` lines; `compose_with` for one unit
//! and `compose_all` for a bundle, with `ComposeOptions` carrying the
//! default extensibility, the `-D` defines, the `@hashid` counter axis and
//! the refusal of `@key` on a union discriminator; `Composed::failed()` and
//! its error diagnostics, which fail the build; `Composed::ast`, which the
//! generator walks.
//!
//! Oracle: IDL 4.2 §7.3 (the preprocessor is the C one: a quoted
//! `#include` is searched beside the including file first, ISO C §6.10.2;
//! `#ifdef` sees a macro defined before the first line) and §7.5.2 (a name
//! declared in one unit of a bundle resolves in another, the scope rules
//! are the unit's); DDS-XTypes 1.3 §7.3.1.2.1.8 (an aggregate without an
//! extensibility annotation takes the configured one, an annotated one keeps
//! its own), §7.3.1.2.1.1 (`@hashid`: the first four octets of MD5 of the
//! name, little endian, masked with 0x0FFFFFFF; the clause's example `color`
//! is 0x0FA5DD70; `x` is 0x01E4D49D, computed outside this tree) and
//! Table 21 (`@key` on a union discriminator).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use std::path::Path;

use zerodds_ast::Extensibility;
use zerodds_idl_compose::{
    ComposeOptions, HashedIdCounter, KeyOnUnionDiscriminator, PathLoader, compose_all, compose_with,
};
use zerodds_types::type_object::complete::CompleteTypeObject;

fn write(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_owned()
}

fn member_ids(composed: &zerodds_idl_compose::Composed, type_name: &str) -> Vec<(String, u32)> {
    composed
        .registry
        .iter_completes()
        .find_map(|(_, object)| match object {
            CompleteTypeObject::Struct(s) if s.header.detail.type_name == type_name => Some(
                s.member_seq
                    .iter()
                    .map(|m| (m.detail.name.clone(), m.common.member_id))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no COMPLETE object for {type_name}"))
}

#[test]
fn idl_7_3_a_quoted_include_beside_the_unit_is_read_and_reported_as_opened() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    write(dir.path(), "common.idl", "struct Near { long a; };");
    write(elsewhere.path(), "common.idl", "struct Far { long a; };");
    let main = write(
        dir.path(),
        "main.idl",
        "#include \"common.idl\"\nstruct Main { Near n; };",
    );
    let loader = PathLoader::new(vec![elsewhere.path().to_path_buf()]);
    let composed = compose_with(&main, &loader, ComposeOptions::default()).unwrap();
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let names: Vec<&str> = composed.ast.types().map(|(_, n)| n.name.as_str()).collect();
    assert!(
        names.contains(&"Near") && !names.contains(&"Far"),
        "{names:?}"
    );
    assert!(
        loader.opened().iter().any(|p| p.ends_with("common.idl")),
        "the build reruns when the include changes: {:?}",
        loader.opened()
    );
}

#[test]
fn xtypes_7_3_1_2_1_8_the_configured_extensibility_fills_only_what_the_source_left_open() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(
        dir.path(),
        "x.idl",
        "struct Silent { long a; }; @appendable struct Said { long a; };",
    );
    let composed = compose_with(
        &main,
        &PathLoader::bare(),
        ComposeOptions {
            default_extensibility: Some(Extensibility::Final),
            ..ComposeOptions::default()
        },
    )
    .unwrap();
    let of = |name: &str| {
        let (id, _) = composed.ast.types().find(|(_, n)| n.name == name).unwrap();
        composed.ast.effective_extensibility(id)
    };
    assert_eq!(of("Silent"), Extensibility::Final);
    assert_eq!(of("Said"), Extensibility::Appendable);
}

#[test]
fn idl_7_3_a_define_is_in_place_before_the_first_line() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(
        dir.path(),
        "d.idl",
        "#ifdef WITH_EXTRA\nstruct Extra { long a; };\n#endif\nstruct Always { long b; };",
    );
    let with = compose_with(
        &main,
        &PathLoader::bare(),
        ComposeOptions {
            defines: vec![("WITH_EXTRA".into(), String::new())],
            ..ComposeOptions::default()
        },
    )
    .unwrap();
    assert!(with.ast.types().any(|(_, n)| n.name == "Extra"));
    let without = compose_with(&main, &PathLoader::bare(), ComposeOptions::default()).unwrap();
    assert!(!without.ast.types().any(|(_, n)| n.name == "Extra"));
}

#[test]
fn xtypes_7_3_1_2_1_1_a_hashed_id_is_the_md5_of_the_name_and_the_axis_decides_what_follows_it() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(
        dir.path(),
        "h.idl",
        "@mutable struct H { @hashid long color; long next; @hashid long x; };",
    );
    let compose = |counter| {
        compose_with(
            &main,
            &PathLoader::bare(),
            ComposeOptions {
                hashed_id_advances_the_counter: counter,
                ..ComposeOptions::default()
            },
        )
        .unwrap()
    };
    let advances = member_ids(&compose(HashedIdCounter::Advances), "H");
    assert_eq!(
        advances,
        [
            ("color".into(), 0x0FA5_DD70),
            ("next".into(), 0x0FA5_DD71),
            ("x".into(), 0x01E4_D49D)
        ]
    );
    let skipped = member_ids(&compose(HashedIdCounter::Skipped), "H");
    assert_eq!(skipped[0].1, 0x0FA5_DD70);
    assert_eq!(skipped[2].1, 0x01E4_D49D);
    assert_ne!(
        skipped[1].1, 0x0FA5_DD71,
        "the counter did not move past the hash"
    );
}

#[test]
fn xtypes_table_21_a_key_on_a_union_discriminator_is_an_error_when_the_build_refuses_it() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(
        dir.path(),
        "u.idl",
        "union U switch (@key long) { case 1: long a; };",
    );
    let refused = compose_with(
        &main,
        &PathLoader::bare(),
        ComposeOptions {
            key_on_union_discriminator: KeyOnUnionDiscriminator::Refused,
            ..ComposeOptions::default()
        },
    )
    .unwrap();
    assert!(refused.failed(), "the build stops at the declaration");
    let carried = compose_with(&main, &PathLoader::bare(), ComposeOptions::default()).unwrap();
    assert!(!carried.failed(), "{:?}", carried.diagnostics);
}

#[test]
fn idl_7_5_2_a_bundle_resolves_a_name_across_its_units_and_a_broken_unit_fails_it() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.idl");
    let b = dir.path().join("b.idl");
    std::fs::write(&a, "module m { struct Inner { long v; }; };").unwrap();
    std::fs::write(&b, "module n { struct Outer { m::Inner i; }; };").unwrap();
    let bundle = compose_all(&[&a, &b], &PathLoader::bare(), ComposeOptions::default()).unwrap();
    assert!(!bundle.failed(), "{:?}", bundle.diagnostics);
    std::fs::write(&b, "module n { struct Outer { m::Missing i; }; };").unwrap();
    let broken = compose_all(&[&a, &b], &PathLoader::bare(), ComposeOptions::default()).unwrap();
    assert!(broken.failed(), "an unresolved name fails the bundle");
}
