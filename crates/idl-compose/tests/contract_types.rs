// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-idl-compose` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`, `compose_source_with`
//! and `compose_all`): `resolve::TypeRegistry::new` and
//! `from_ast::register_all`, which fills the registry of `Composed` with the
//! MINIMAL and COMPLETE object of every type of the model and returns the
//! types it skipped with the reason (`Composed::without_type_objects`); the
//! `TypeRegistry` is a public field of `Composed`.
//!
//! The model is built here with `zerodds-idl-parse` and `zerodds-idl-lower`,
//! the two front-end crates `compose_source_with` calls before
//! `register_all`.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B ("First 14 bytes of MD5 of the
//! serialized TypeObject using XCDR version 2 with Little Endian encoding"),
//! with the MINIMAL hashes CycloneDDS 11.0.1 and Fast DDS 3.6.1 agree on and
//! the COMPLETE hashes CycloneDDS and RTI Connext 7.7.0 report for the corpus
//! below (`crates/types/tests/typeobject_vendor_dheader.rs`); §7.3.4.1 and
//! §7.3.4.2 Table 29 (no TypeIdentifier kind for an IDL `native`).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::EquivalenceHash;
use zerodds_types::from_ast::{FromAstError, register_all};
use zerodds_types::resolve::TypeRegistry;

/// The corpus of `zerodds-examples/idl-conformance/proofs/typeobject`.
const CORPUS: &str = "\
module to {
  @final
  struct Plain {
    long  a;
    short b;
  };
  @appendable
  struct Appendable {
    long  a;
    short b;
  };
  @mutable
  struct Mutable {
    @id(10) long  a;
    @id(20) short b;
  };
  @final
  struct Inner {
    long  x;
    long  y;
  };
};";

/// Name, MINIMAL hash, COMPLETE hash.
const HASHES: [(&str, &str, &str); 4] = [
    (
        "Plain",
        "deb922f040f7c228b25728be51f6",
        "7c0c6fab7d066a0c56b8e93c0ea7",
    ),
    (
        "Appendable",
        "3c9161318d323a5141b44ee3e2b6",
        "27cb33d5ad3f7388a5627a1b48fd",
    ),
    (
        "Mutable",
        "81e76ce14253653ea25be4145831",
        "447863b1f0043ce0e42323a74866",
    ),
    (
        "Inner",
        "0c380bda28dab0250db24ae23c16",
        "f00388e0e0c6619e4175c68e8af8",
    ),
];

fn hash(hex: &str) -> EquivalenceHash {
    let mut out = [0u8; 14];
    for (i, octet) in out.iter_mut().enumerate() {
        *octet = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    EquivalenceHash(out)
}

fn model(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl").ast;
    // A reference left broken stays in the model; `register_all` is the
    // one that answers for it.
    let _ = ast.resolve_all_by_name();
    ast
}

#[test]
fn xtypes_7_3_4_5_register_all_files_both_forms_under_the_hashes_vendors_compute() {
    let ast = model(CORPUS);
    let mut registry = TypeRegistry::new();
    let skipped = register_all(&ast, &mut registry).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    for (name, minimal, complete) in HASHES {
        assert!(
            registry.get_minimal(&hash(minimal)).is_some(),
            "{name}: no MINIMAL object under the vendors' hash"
        );
        assert!(
            registry.get_complete(&hash(complete)).is_some(),
            "{name}: no COMPLETE object under the vendors' hash"
        );
    }
    // Four types, one object of each form each; `long` and `short` are
    // fully descriptive and have no object (§7.3.4.1).
    assert_eq!(registry.len(), (4, 4));
}

#[test]
fn xtypes_7_3_4_2_a_type_without_a_type_identifier_is_listed_and_the_rest_registered() {
    let ast = model(
        "module m {
           native Handle;
           struct A { long x; };
         };",
    );
    let mut registry = TypeRegistry::new();
    let skipped = register_all(&ast, &mut registry).unwrap();
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    let (name, why) = &skipped[0];
    assert!(name.ends_with("Handle"), "{name}");
    assert!(
        matches!(why, FromAstError::Unrepresentable { .. }),
        "{why:?}"
    );
    assert_eq!(registry.len(), (1, 1), "`A` is registered all the same");
}

#[test]
fn register_all_refuses_a_type_it_cannot_build_with_an_error_not_a_skip() {
    // A representable type whose member type does not resolve: not a
    // construct XTypes lacks, so not a skip. `register_all` returns the
    // error and stops; `without_type_objects` cannot report it, and a
    // caller has to report the error itself.
    let ast = model(
        "module m {
           struct S { Missing value; };
         };",
    );
    let mut registry = TypeRegistry::new();
    let result = register_all(&ast, &mut registry);
    assert!(
        matches!(result, Err(FromAstError::Unresolved { .. })),
        "{result:?}"
    );
}
