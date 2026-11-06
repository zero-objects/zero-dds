// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-test-vendor-reader` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe` (`src/lib.rs`,
//! `keyed_point_a_type_object`): `encode::model_from_idl` to lower
//! `KEYED_POINT_A_IDL` into the model `zerodds-types` builds the COMPLETE
//! `TypeObject` from. It relies on the model holding the type under its
//! declared name with its members in declaration order and the `@key`
//! annotation on `id`, and on an IDL with a diagnostic being refused rather
//! than lowered in part.
//!
//! Oracles: OMG IDL 4.2 §7.4.1.4.4.4.1 (members in declaration order),
//! §7.5.2 (a name that resolves to no declaration is an error); DDS-XTypes
//! 1.3 §7.2.2.4.4.4.8 (`@key` marks a key member).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_test_vendor_reader::KEYED_POINT_A_IDL;
use zerodds_wire_probe::encode::model_from_idl;

#[test]
fn xtypes_7_2_2_4_4_4_8_the_model_holds_keyed_point_a_with_its_key() {
    let ast = model_from_idl(KEYED_POINT_A_IDL, "keyed-point-a.idl").unwrap();
    let (id, _) = ast
        .types()
        .find(|(_, node)| node.name == "KeyedPointA")
        .expect("KeyedPointA is a type of the model");
    let members: Vec<(String, bool)> = ast
        .members_of(id)
        .iter()
        .map(|member| {
            let node = ast.member(*member).unwrap();
            (node.name.clone(), node.facets.is_key())
        })
        .collect();
    assert_eq!(
        members,
        [("id".to_string(), true), ("value".to_string(), false)]
    );
}

#[test]
fn idl_7_5_2_an_idl_with_a_diagnostic_is_refused() {
    let error = model_from_idl(
        "@final struct KeyedPointA { @key long id; Missing value; };",
        "missing.idl",
    )
    .err()
    .expect("a member of an undeclared type is no model");
    assert!(format!("{error}").contains("Missing"), "{error}");
}
