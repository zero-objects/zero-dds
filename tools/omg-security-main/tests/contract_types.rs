// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-security-main` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-types`: the `DdsType` impl `zerodds-build`
//! generates for `ShapeType` from `interop/shapes/omg-dds-security.idl`
//! (`src/shape_type.rs`, `include!`): a constant `TYPE_IDENTIFIER:
//! TypeIdentifier = TypeIdentifier::EquivalenceHashComplete(
//! EquivalenceHash([..]))` and a `type_object()` that is
//! `type_object::TypeObject::from_bytes_le(&[..]).ok()` over the octets of
//! the COMPLETE object. `src/main.rs`, `src/cli.rs` and `src/security.rs`
//! do not name `zerodds-types`. The tool is a binary without a library, so
//! the contract builds what the generated code holds from the specification
//! and calls `zerodds-types` with it.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B, the COMPLETE object of
//! `ShapeType { @key string<64> color; long x; long y; long shapesize; }`
//! (APPENDABLE; `//@key` is the annotation by Table 24's slash-slash-at
//! form), written out by hand with the layout rules that reproduce
//! CycloneDDS 11.0.1's octets for `to::Plain`; "First 14 bytes of MD5 of
//! the serialized TypeObject", the MD5 computed outside this tree.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::hash::compute_complete_hash;
use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// `ShapeType` of `omg-dds-security.idl`, COMPLETE, XCDR2 little endian:
/// DHEADER 128, `EK_COMPLETE`, `TK_STRUCTURE`, `IS_APPENDABLE`; the header
/// with the name "ShapeType"; four members, the first `color` with flags
/// `TRY_CONSTRUCT1 | IS_MUST_UNDERSTAND | IS_KEY` and the identifier
/// `70 40` (`TI_STRING8_SMALL`, bound 64).
const SHAPE_TYPE_COMPLETE: &str = "80000000f251020012000000000000000a000000536861706554797065000000600000000400000014000000000000003100704006000000636f6c6f72000000100000000100000001000400020000007800000010000000020000000100040002000000790000001800000003000000010004000a000000736861706573697a65000000";
/// MD5 of those 132 octets, first 14.
const SHAPE_TYPE_HASH: [u8; 14] = [
    0x2f, 0x77, 0xf7, 0xcc, 0x1f, 0xc9, 0x68, 0xd2, 0xab, 0x28, 0x28, 0xf7, 0x40, 0x10,
];

/// What the generated impl writes, spelled the way it spells it.
const TYPE_IDENTIFIER: TypeIdentifier =
    TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(SHAPE_TYPE_HASH));

#[test]
fn annex_b_the_generated_object_parses_and_its_hash_is_the_announced_identifier() {
    let bytes: Vec<u8> = (0..SHAPE_TYPE_COMPLETE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&SHAPE_TYPE_COMPLETE[i..i + 2], 16).unwrap())
        .collect();
    let object = TypeObject::from_bytes_le(&bytes).ok();
    let Some(TypeObject::Complete(complete)) = &object else {
        panic!("the generated `type_object()` would answer None");
    };
    let CompleteTypeObject::Struct(shape) = complete else {
        panic!("a structure");
    };
    assert_eq!(shape.header.detail.type_name, "ShapeType");
    assert_eq!(
        shape.struct_flags,
        StructTypeFlag(StructTypeFlag::IS_APPENDABLE)
    );
    assert_eq!(shape.member_seq.len(), 4);
    let color = &shape.member_seq[0];
    assert_eq!(color.detail.name, "color");
    assert!(color.common.member_flags.has(StructMemberFlag::IS_KEY));
    assert_eq!(
        color.common.member_type_id,
        TypeIdentifier::String8Small { bound: 64 }
    );
    // The identifier a peer resolves this object by.
    assert_eq!(
        TypeIdentifier::EquivalenceHashComplete(compute_complete_hash(complete).unwrap()),
        TYPE_IDENTIFIER
    );
}
