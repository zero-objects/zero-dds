// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Config::rust_type` and `Config::rust_type_with` (GitHub #33): what the
//! build refuses, and what a mapping leaves as the IDL says.
//!
//! The generated code is compiled and run by the `#[ignore]`d tests of
//! `dds_type.rs` (integration-codegen-build); these assert what is decided
//! before any code is compiled.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;

use zerodds_build::{Config, Error};

const UNIT: &str = "\
module app {
    typedef string<12> Dtg;
    typedef uint32 ItemId;
    typedef wstring Wide;
    typedef sequence<long> Longs;
    @final struct Track { @key uint32 id; Dtg stamp; ItemId item; Longs values; };
    @final union Mark switch (long) { case 1: Dtg at; case 2: long n; };
};
";

/// Generate `UNIT` with the given configuration and answer the module, or
/// the error.
fn generate(configure: impl Fn(&mut Config)) -> Result<String, Error> {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("app.idl");
    fs::write(&idl, UNIT).expect("write");
    let mut config = Config::new();
    config
        .out_dir(dir.path())
        .emit_cargo_directives(false)
        .dds_type_impls(true)
        .type_identifier_constants(true);
    configure(&mut config);
    config.compile(&[&idl])?;
    Ok(fs::read_to_string(dir.path().join("app.rs")).expect("the module"))
}

/// What a `rust_type` call names has to be a typedef, a struct member or
/// a union branch of the compiled units, with an IDL type a Rust type can
/// stand for — a `string` or a primitive other than `wchar` and `long
/// double` (`rc8-rust-newtype.md`: "an unknown path, or a Rust type whose
/// `Idl` does not match the member's IDL type, is a build error"). Each
/// refusal names the path and why.
#[test]
fn issue_33_a_path_no_rust_type_can_stand_for_fails_the_build() {
    let cases: [(&str, &str, &str); 9] = [
        ("app::Nothing", "crate::X", "names no typedef"),
        (
            "app::Track.missing",
            "crate::X",
            "declares no member `missing`",
        ),
        ("app::Track", "crate::X", "is a struct"),
        ("app::Wide", "crate::X", "`wstring`"),
        ("app::Longs", "crate::X", "`sequence`"),
        ("app::Track.values", "crate::X", "`sequence`"),
        ("app::ItemId", "&str", "no type path"),
        ("app::ItemId", "ItemId", "would name itself"),
        ("other::Dtg", "crate::X", "names no typedef"),
    ];
    for (path, rust_type, says) in cases {
        let refused = generate(|config| {
            config.rust_type(path, rust_type);
        })
        .expect_err(path);
        match &refused {
            Error::RustType { idl_path, detail } => {
                assert_eq!(idl_path, path);
                assert!(detail.contains(says), "{path}: {detail}");
            }
            other => panic!("{path}: {other}"),
        }
    }
    let refused = generate(|config| {
        config.rust_type_with("app::Dtg", "crate::X", "a::B<C>");
    })
    .expect_err("an adapter with generic arguments");
    assert!(refused.to_string().contains("adapter"), "{refused}");
}

/// The mapping changes what the Rust holds and nothing the peer sees:
/// the `TypeObject`s, the `TypeIdentifier`s (XTypes 1.3 §7.3.4) and the
/// type names are the ones the IDL alone gives, octet for octet.
#[test]
fn xtypes_7_3_4_issue_33_the_type_objects_and_identifiers_are_the_idls() {
    let idl_only = generate(|_| {}).expect("plain");
    let mapped = generate(|config| {
        config
            .rust_type("app::Dtg", "crate::Dtg")
            .rust_type("app::Track.id", "core::num::NonZeroU32")
            .rust_type("app::ItemId", "core::num::NonZeroU32")
            .rust_type("app::Mark.n", "core::num::NonZeroI32");
    })
    .expect("mapped");
    let identity = |module: &str| -> Vec<String> {
        module
            .lines()
            .filter(|line| {
                line.contains("&[0x")
                    || line.contains("EquivalenceHash(")
                    || line.contains("const TYPE_NAME")
                    || line.contains("Extensibility::")
                    || line.contains("KEY_HOLDER_MAX_SIZE")
            })
            .map(|line| line.trim().to_string())
            .collect()
    };
    let expected = identity(&idl_only);
    assert!(expected.len() > 6, "{idl_only}");
    assert_eq!(identity(&mapped), expected);
    // And the Rust does change: the member and the typedef hold the types
    // the calls named, and each mapping is stated for the compiler.
    assert!(mapped.contains("pub id: core::num::NonZeroU32"), "{mapped}");
    assert!(mapped.contains("pub type Dtg = crate::Dtg;"), "{mapped}");
    assert!(
        mapped.contains("zerodds_wire::repr::idl_type_is::<crate::Dtg, str>()"),
        "{mapped}"
    );
    assert!(
        mapped.contains("zerodds_wire::repr::idl_type_is::<core::num::NonZeroI32, i32>()"),
        "{mapped}"
    );
}
