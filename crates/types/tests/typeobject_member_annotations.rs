// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A member's `@min`, `@max` and `@unit` in a `CompleteTypeObject`, measured
//! against Cyclone DDS and RTI Connext rather than read off this tree.
//!
//! DDS-XTypes 1.3, Annex B, declares
//! `@extensibility(APPENDABLE) struct AppliedBuiltinMemberAnnotations` with
//! four `@optional` members (`unit`, `min`, `max`, `hash_id`) and
//! `@extensibility(FINAL) union AnnotationParameterValue switch (octet)`.
//! Under XCDR2 the first is a `DHEADER` and a presence octet per member,
//! the second a kind octet and the value, nothing in between.
//!
//! This tree used to write neither: no `DHEADER`, every value behind a
//! 4-octet length of its own, and `@default` as a fifth string. Absent
//! annotations collapse to a single `0` and matched the vendors, so only
//! a member that carried one of these annotations showed it — its
//! `COMPLETE` hash was nobody else's.
//!
//! # Where the octets come from
//!
//! Measured on the Linux test host on 2026-10-04 for the IDL in [`IDL`]:
//!
//! - Cyclone DDS 11.0.1: `idlc -l c`, then
//!   `zerodds-examples/idl-conformance/proofs/typeobject/extractors/
//!   cyclone_extract.py`, which checks that the MD5 of each object is the
//!   hash Cyclone put beside it. Cyclone's C generator does not use
//!   `@default` ("Explicit defaults are not supported yet in the C
//!   generator") and writes no trace of it.
//! - RTI Connext 7.7.0: `rtiddsgen -language C`, then
//!   `DDS_TypeObjectV2_create_from_typecode` and `_serialize` as in
//!   `rti_extract.c` beside it; the object starts at offset 24 of RTI's
//!   blob. RTI appends a fifth member to `AppliedBuiltinMemberAnnotations`,
//!   `@default` as an `AnnotationParameterValue`, inside the `DHEADER`.
//!   Fast DDS 3.6.1's `AppliedBuiltinMemberAnnotations` has the four
//!   members of the IDL (`dds_xtypes_typeobject.hpp`).
//!
//! RTI refuses the type unless `a`'s `@default` lies within `@min` and
//! `@max`, hence `@default(5)`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_types::from_ast::{complete_type_object, minimal_type_object};
use zerodds_types::type_object::TypeObject;
use zerodds_types::type_object::common::AnnotationParameterValue;
use zerodds_types::type_object::complete::CompleteTypeObject;

const IDL: &str = "\
module ann {
  @final
  struct Ranged {
    @min(1) @max(9) @default(5) long a;
    @unit(\"m\") double b;
    @default(7) short c;
  };
};";

/// Cyclone DDS 11.0.1, `COMPLETE`, hash `534f584ed520f7560c3a2e731b1e`.
const CYCLONE_COMPLETE: &str = "8c000000f251010014000000000000000c000000616e6e3a3a52616e676564006c000000030000002600000000000000010004000200000061000100110000000001040001000000010400000900000000000000220000000100000001000a0002000000620001000d00000001000000020000006d000000000000001000000002000000010003000200000063000000";

/// Cyclone DDS 11.0.1, `MINIMAL`, hash `40e4ec22fdced5cbb1f78b298b3e`.
const CYCLONE_MINIMAL: &str = "43000000f1510100010000000000000033000000030000000b000000000000000100040cc175b9000b0000000100000001000a92eb5ffe000b000000020000000100034a8a08f0";

/// RTI Connext 7.7.0, `COMPLETE`, from offset 24 of its serialized blob;
/// hash `433503699fef38bdc87c045e9c8e`.
const RTI_COMPLETE: &str = "a7000000f251010014000000000000000c000000616e6e3a3a52616e6765640087000000030000002f000000000000000100040002000000610001001a00000000010400010000000104000009000000000104000500000000000000250000000100000001000a0002000000620001001000000001000000020000006d00000000000000000000001f000000020000000100030002000000630001000a0000000000000001030700000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn ranged() -> (
    zerodds_ast::Ast,
    zerodds_ast_core::Id<zerodds_ast::TypeNode>,
) {
    let parsed = zerodds_idl_parse::parse(IDL);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "annotated.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);
    let id = ast
        .types()
        .find(|(_, node)| node.name == "Ranged")
        .map(|(id, _)| id)
        .expect("Ranged");
    (ast, id)
}

#[test]
fn the_complete_object_is_cyclones_octet_for_octet() {
    let (ast, id) = ranged();
    let complete = TypeObject::Complete(complete_type_object(&ast, id).expect("complete"));
    assert_eq!(
        complete.to_bytes_le().expect("encode"),
        unhex(CYCLONE_COMPLETE)
    );
}

#[test]
fn the_minimal_object_is_cyclones_octet_for_octet() {
    let (ast, id) = ranged();
    let minimal = TypeObject::Minimal(minimal_type_object(&ast, id).expect("minimal"));
    assert_eq!(
        minimal.to_bytes_le().expect("encode"),
        unhex(CYCLONE_MINIMAL)
    );
}

#[test]
fn rtis_object_decodes_and_its_default_slot_is_skipped() {
    let rti = TypeObject::from_bytes_le(&unhex(RTI_COMPLETE)).expect("decode RTI's object");
    let TypeObject::Complete(CompleteTypeObject::Struct(st)) = &rti else {
        panic!("not a complete struct: {rti:?}");
    };
    let a = &st.member_seq[0].detail.ann_builtin;
    assert_eq!(a.min, Some(AnnotationParameterValue::Int32(1)));
    assert_eq!(a.max, Some(AnnotationParameterValue::Int32(9)));
    assert_eq!(a.default_value, None, "RTI's fifth member is not read");
    let b = &st.member_seq[1].detail.ann_builtin;
    assert_eq!(b.unit.as_deref(), Some("m"));

    // Without RTI's `@default` slot what is left is Cyclone's object.
    assert_eq!(rti.to_bytes_le().expect("encode"), unhex(CYCLONE_COMPLETE));
}
