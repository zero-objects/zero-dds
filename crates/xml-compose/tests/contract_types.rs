// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xml-compose` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`):
//! - `resolve::TypeRegistry::new` and `from_ast::register_all` in
//!   `compose_all_sources`: the skipped types become
//!   `Composed::without_type_objects`, an `Err` becomes an error diagnostic.
//! - `dynamic::DynamicType::find` in `type_of_composed`, behind
//!   `create_type_w_document` and `create_type_w_uri` (XTypes 1.3
//!   §7.5.2.2.13, §7.5.2.2.14): the type of a scoped name, or an error whose
//!   `Display` text is returned.
//!
//! The model is built here from IDL with `zerodds-idl-parse` and
//! `zerodds-idl-lower` (dependencies of this crate); `register_all` and
//! `DynamicType::find` read the model and not the syntax it came from.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B, with the MINIMAL and
//! COMPLETE hashes CycloneDDS 11.0.1, Fast DDS 3.6.1 and RTI Connext 7.7.0
//! report for `to::Plain` (`crates/types/tests/typeobject_vendor_dheader.rs`);
//! §7.3.4.2 Table 29 (no TypeIdentifier kind for `native`); §7.5.2.2.14
//! ("If an error occurs, this method shall return a nil value") and
//! §7.5.2.8 Table 54 (`name`, `kind` of a DynamicType).
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use zerodds_types::EquivalenceHash;
use zerodds_types::dynamic::{DynamicType, TypeKind};
use zerodds_types::from_ast::{FromAstError, register_all};
use zerodds_types::resolve::TypeRegistry;

fn model(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl").ast;
    let _ = ast.resolve_all_by_name();
    ast
}

const PLAIN: &str = "module to { @final struct Plain { long a; short b; }; };";

#[test]
fn xtypes_7_3_4_5_register_all_files_the_type_under_the_vendors_hashes() {
    let ast = model(PLAIN);
    let mut registry = TypeRegistry::new();
    let skipped = register_all(&ast, &mut registry).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    assert!(
        registry
            .get_minimal(&EquivalenceHash([
                0xde, 0xb9, 0x22, 0xf0, 0x40, 0xf7, 0xc2, 0x28, 0xb2, 0x57, 0x28, 0xbe, 0x51, 0xf6,
            ]))
            .is_some()
    );
    assert!(
        registry
            .get_complete(&EquivalenceHash([
                0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7,
            ]))
            .is_some()
    );
}

#[test]
fn xtypes_7_3_4_2_register_all_lists_a_skipped_type_and_errs_on_an_unbuildable_one() {
    let mut registry = TypeRegistry::new();
    let skipped = register_all(
        &model("module m { native Handle; struct A { long x; }; };"),
        &mut registry,
    )
    .unwrap();
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(skipped[0].0.ends_with("Handle"), "{skipped:?}");
    assert!(matches!(skipped[0].1, FromAstError::Unrepresentable { .. }));

    // An unresolved member type is an error, which this crate reports as
    // a diagnostic rather than as an empty list.
    let mut registry = TypeRegistry::new();
    let result = register_all(
        &model("module m { struct S { Missing value; }; };"),
        &mut registry,
    );
    assert!(
        matches!(result, Err(FromAstError::Unresolved { .. })),
        "{result:?}"
    );
}

#[test]
fn xtypes_7_5_2_8_find_takes_a_scoped_name_and_refuses_an_unknown_one() {
    let ast = Arc::new(model(PLAIN));
    for spelling in ["to::Plain", "::to::Plain"] {
        let found = DynamicType::find(&ast, spelling).unwrap();
        assert_eq!(found.name(), "Plain");
        assert_eq!(found.qualified_name(), "to::Plain");
        assert_eq!(found.kind(), TypeKind::Structure);
        assert_eq!(found.member_count(), 2);
    }
    // §7.5.2.2.14: a document without the type is an error; the text names
    // what was asked for.
    let missing = DynamicType::find(&ast, "to::Absent").unwrap_err();
    assert!(missing.to_string().contains("to::Absent"), "{missing}");
    // A module is not a type.
    assert!(DynamicType::find(&ast, "to").is_err());
    // §7.5.2.2.14: "trivially the case if the name is nil or the empty
    // string".
    assert!(DynamicType::find(&ast, "").is_err());
}
