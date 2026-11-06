// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/model.rs`, `build`):
//! `from_ast::type_identifier(.., Form::Complete)`, kept when it is
//! `TypeIdentifier::EquivalenceHashComplete` or a
//! `StronglyConnectedComponent` of `EK_COMPLETE`, else `TypeIdentifier::None`;
//! `from_ast::complete_type_object` and `from_ast::dependency_type_objects`,
//! each kept only if `TypeObject::to_bytes_le` and `TypeObject::from_bytes_le`
//! round-trip it; `from_ast::type_component` for a component identifier.
//! The model is the IDL the derive's description stands for, composed with
//! `zerodds-idl-compose`.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B, the COMPLETE octets
//! CycloneDDS 11.0.1 writes for `to::Plain` and `to::Inner` and the
//! COMPLETE hash CycloneDDS and RTI Connext 7.7.0 report for `to::Plain`
//! (the vectors of `crates/types/tests/typeobject_vendor_dheader.rs`);
//! §7.3.4.6: the equivalence hash is the first 14 octets of the MD5 of the
//! object's octets.

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_codec::DdsType;
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier};

const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const INNER_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";

#[derive(DdsType)]
#[dds(name = "to::Plain", extensibility = "final")]
struct Plain {
    a: i32,
    b: i16,
}

#[derive(DdsType)]
#[dds(name = "to::Inner", extensibility = "final")]
struct Inner {
    x: i32,
    y: i32,
}

#[derive(DdsType)]
#[dds(name = "to::Outer", extensibility = "final")]
struct Outer {
    i: Inner,
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn xtypes_7_3_4_5_the_object_of_a_derived_type_is_the_vendors() {
    let model = zerodds_codec::model::model::<Plain>().unwrap();
    let octets = model.type_object.as_ref().unwrap().to_bytes_le().unwrap();
    assert_eq!(octets, unhex(PLAIN_COMPLETE));
    assert_eq!(
        model.type_identifier,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
            0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7,
        ]))
    );
    let digest = zerodds_wire::md5(&octets);
    assert_eq!(
        model.type_identifier,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(digest[..14].try_into().unwrap()))
    );
    assert!(TypeObject::from_bytes_le(&octets).is_ok());
}

#[test]
fn xtypes_7_3_4_6_3_the_dependencies_are_the_closure_by_hash() {
    let dependencies: Vec<Vec<u8>> = <Outer as zerodds_dcps::DdsType>::type_object_dependencies()
        .into_iter()
        .map(|object| object.to_bytes_le().unwrap())
        .collect();
    assert_eq!(dependencies, vec![unhex(INNER_COMPLETE)]);
    assert_eq!(
        <Inner as zerodds_dcps::DdsType>::type_object()
            .unwrap()
            .to_bytes_le()
            .unwrap(),
        unhex(INNER_COMPLETE)
    );
    assert!(<Plain as zerodds_dcps::DdsType>::type_object_dependencies().is_empty());
    assert_eq!(<Plain as zerodds_dcps::DdsType>::type_component(), None);
}

#[test]
fn xtypes_7_3_4_9_a_type_that_reaches_itself_is_in_a_component() {
    #[derive(DdsType)]
    #[dds(name = "to::Node")]
    struct Node {
        v: i32,
        kids: Vec<Node>,
    }
    match <Node as zerodds_dcps::DdsType>::type_identifier() {
        TypeIdentifier::StronglyConnectedComponent(scc) => {
            assert_eq!(scc.scc_length, 1);
            assert_eq!(scc.scc_index, 1);
        }
        other => panic!("a component identifier, got {other:?}"),
    }
    let component = <Node as zerodds_dcps::DdsType>::type_component().unwrap();
    assert_eq!(component.complete.len(), 1);
    assert_eq!(component.minimal.len(), 1);
}
