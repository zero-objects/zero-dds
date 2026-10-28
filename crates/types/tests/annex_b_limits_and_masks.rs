// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The length limits and the minimal masks of DDS-XTypes 1.3 Annex B.
//!
//! - p. 279: `MEMBER_NAME_MAX_LENGTH = 256` for `MemberName`,
//!   `TYPE_NAME_MAX_LENGTH = 256` for `QualifiedTypeName`, both
//!   `string<N>`.
//! - p. 286: `ANNOTATION_STR_VALUE_MAX_LEN = 128` for the `string` and
//!   `wstring` cases of `AnnotationParameterValue`,
//!   `ANNOTATION_OCTETSEC_VALUE_MAX_LEN = 128`.
//! - p. 280 and 281: `MemberFlagMinimalMask = 0x003f` and
//!   `TypeFlagMinimalMask = 0x0007`, "mask used to remove the flags that
//!   do no affect assignability".

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::Ast;
use zerodds_types::TypeIdentifier;
use zerodds_types::from_ast::{complete_type_object, minimal_type_object};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::PrimitiveKind;
use zerodds_types::type_object::common::{
    ANNOTATION_OCTETSEC_VALUE_MAX_LEN, ANNOTATION_STR_VALUE_MAX_LEN, AnnotationParameterValue,
    CommonStructMember, CompleteMemberDetail, CompleteTypeDetail, MEMBER_NAME_MAX_LENGTH,
    OptionalAppliedAnnotationSeq, TYPE_NAME_MAX_LENGTH,
};
use zerodds_types::type_object::complete::{
    CompleteStructHeader, CompleteStructMember, CompleteStructType,
};
use zerodds_types::type_object::flags::{
    MEMBER_FLAG_MINIMAL_MASK, StructMemberFlag, StructTypeFlag, TYPE_FLAG_MINIMAL_MASK,
};
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};
use zerodds_wire::Writer;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "limits.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn member_detail(name: &str) -> CompleteMemberDetail {
    CompleteMemberDetail {
        name: name.to_string(),
        ann_builtin: Default::default(),
        ann_custom: OptionalAppliedAnnotationSeq(None),
    }
}

fn type_detail(name: &str) -> CompleteTypeDetail {
    CompleteTypeDetail {
        ann_builtin: Default::default(),
        ann_custom: OptionalAppliedAnnotationSeq(None),
        type_name: name.to_string(),
    }
}

#[test]
fn the_limits_are_the_constants_of_annex_b() {
    assert_eq!(MEMBER_NAME_MAX_LENGTH, 256);
    assert_eq!(TYPE_NAME_MAX_LENGTH, 256);
    assert_eq!(ANNOTATION_STR_VALUE_MAX_LEN, 128);
    assert_eq!(ANNOTATION_OCTETSEC_VALUE_MAX_LEN, 128);
}

/// `string<MEMBER_NAME_MAX_LENGTH> MemberName`: 256 octets are written,
/// 257 are refused.
#[test]
fn a_member_name_holds_at_most_256_octets() {
    let mut w = zerodds_types::framing::xcdr2_writer();
    member_detail(&"m".repeat(256))
        .encode_into(&mut w)
        .expect("256 is the bound");
    let mut w = zerodds_types::framing::xcdr2_writer();
    member_detail(&"m".repeat(257))
        .encode_into(&mut w)
        .expect_err("257 exceeds string<256>");
}

#[test]
fn a_member_name_over_256_octets_is_refused_on_reading() {
    let mut w = zerodds_types::framing::xcdr2_writer();
    w.write_string(&"m".repeat(257));
    w.write_u8(0);
    w.write_u8(0);
    let bytes = w.finish();
    CompleteMemberDetail::decode_from(&mut zerodds_types::framing::xcdr2_reader(&bytes))
        .expect_err("257 exceeds string<256>");

    let mut w = zerodds_types::framing::xcdr2_writer();
    w.write_string(&"m".repeat(256));
    w.write_u8(0);
    w.write_u8(0);
    let bytes = w.finish();
    CompleteMemberDetail::decode_from(&mut zerodds_types::framing::xcdr2_reader(&bytes))
        .expect("256 is the bound");
}

#[test]
fn a_type_name_holds_at_most_256_octets() {
    let mut w = zerodds_types::framing::xcdr2_writer();
    type_detail(&"T".repeat(256))
        .encode_into(&mut w)
        .expect("256 is the bound");
    let mut w = zerodds_types::framing::xcdr2_writer();
    type_detail(&"T".repeat(257))
        .encode_into(&mut w)
        .expect_err("257 exceeds string<256>");

    // The same refusal reading: the name is the last member, so a
    // detail written with a one-character name is cut off in front of
    // it and followed by a 257-character string at the same offset.
    let mut w = zerodds_types::framing::xcdr2_writer();
    type_detail("T").encode_into(&mut w).unwrap();
    let short = w.finish();
    let mut w = zerodds_types::framing::xcdr2_writer();
    w.write_bytes(&short[..short.len() - 6]);
    w.write_string(&"T".repeat(257));
    let bytes = w.finish();
    CompleteTypeDetail::decode_from(&mut zerodds_types::framing::xcdr2_reader(&bytes))
        .expect_err("257 exceeds string<256>");
}

/// A whole object is refused, not written with a cut name.
#[test]
fn an_object_with_a_long_type_name_does_not_encode() {
    let object = CompleteTypeObject::Struct(CompleteStructType {
        struct_flags: StructTypeFlag(StructTypeFlag::IS_FINAL),
        header: CompleteStructHeader {
            base_type: TypeIdentifier::None,
            detail: type_detail(&"T".repeat(257)),
        },
        member_seq: Vec::new(),
    });
    TypeObject::Complete(object)
        .to_bytes_le()
        .expect_err("the type name exceeds TYPE_NAME_MAX_LENGTH");
}

/// `string<ANNOTATION_STR_VALUE_MAX_LEN>`: 128 octets pass, 129 do not.
#[test]
fn an_annotation_string_value_holds_at_most_128_octets() {
    AnnotationParameterValue::String8("s".repeat(128))
        .to_bytes()
        .expect("128 is the bound");
    AnnotationParameterValue::String8("s".repeat(129))
        .to_bytes()
        .expect_err("129 exceeds string<128>");

    let mut w = Writer::with_max_align(zerodds_wire::Endianness::Little, 4);
    w.write_u8(0x20);
    w.write_string(&"s".repeat(129));
    let bytes = w.finish();
    AnnotationParameterValue::from_bytes(&bytes).expect_err("129 exceeds string<128> on reading");
}

/// `wstring<ANNOTATION_STR_VALUE_MAX_LEN>` counts UTF-16 code units.
#[test]
fn an_annotation_wstring_value_holds_at_most_128_code_units() {
    AnnotationParameterValue::String16("s".repeat(128))
        .to_bytes()
        .expect("128 is the bound");
    AnnotationParameterValue::String16("s".repeat(129))
        .to_bytes()
        .expect_err("129 exceeds wstring<128>");
    // 64 characters outside the BMP are 128 code units; 65 are 130.
    AnnotationParameterValue::String16("\u{1D11E}".repeat(64))
        .to_bytes()
        .expect("128 code units");
    AnnotationParameterValue::String16("\u{1D11E}".repeat(65))
        .to_bytes()
        .expect_err("130 code units");

    let mut bytes = vec![0x21, 0, 0, 0];
    bytes.extend_from_slice(&(129u32 * 2).to_le_bytes());
    for _ in 0..129 {
        bytes.extend_from_slice(&[b's', 0]);
    }
    AnnotationParameterValue::from_bytes(&bytes).expect_err("129 exceeds wstring<128> on reading");
}

#[test]
fn the_masks_are_the_constants_of_annex_b() {
    assert_eq!(MEMBER_FLAG_MINIMAL_MASK, 0x003f);
    assert_eq!(TYPE_FLAG_MINIMAL_MASK, 0x0007);
}

/// `TypeFlagMinimalMask` selects `M, A, F`: `@nested` and
/// `@autoid(HASH)` are in the complete object and not in the minimal one.
#[test]
fn a_minimal_struct_carries_only_the_extensibility_flags() {
    let ast = model(
        r#"
@final @nested @autoid(HASH) struct Inner { long a; };
"#,
    );
    let id = ast
        .types()
        .find(|(_, node)| node.name == "Inner")
        .map(|(id, _)| id)
        .unwrap();
    let CompleteTypeObject::Struct(complete) = complete_type_object(&ast, id).unwrap() else {
        panic!("not a struct");
    };
    assert!(complete.struct_flags.has(StructTypeFlag::IS_FINAL));
    assert!(complete.struct_flags.has(StructTypeFlag::IS_NESTED));
    assert!(complete.struct_flags.has(StructTypeFlag::IS_AUTOID_HASH));

    let MinimalTypeObject::Struct(minimal) = minimal_type_object(&ast, id).unwrap() else {
        panic!("not a struct");
    };
    assert_eq!(minimal.struct_flags.0, StructTypeFlag::IS_FINAL);
}

/// The projection of a complete object applies the same masks: a flag
/// word with bits above `IS_KEY` loses them in the minimal form, the
/// flags below stay.
#[test]
fn the_projection_masks_member_and_type_flags() {
    let member = CompleteStructMember {
        common: CommonStructMember {
            member_id: 0,
            member_flags: StructMemberFlag(
                StructMemberFlag::IS_KEY | StructMemberFlag::TRY_CONSTRUCT1 | 0x0080 | 0x4000,
            ),
            member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
        },
        detail: member_detail("a"),
    };
    let object = CompleteTypeObject::Struct(CompleteStructType {
        struct_flags: StructTypeFlag(
            StructTypeFlag::IS_APPENDABLE
                | StructTypeFlag::IS_NESTED
                | StructTypeFlag::IS_AUTOID_HASH,
        ),
        header: CompleteStructHeader {
            base_type: TypeIdentifier::None,
            detail: type_detail("S"),
        },
        member_seq: vec![member],
    });
    let MinimalTypeObject::Struct(minimal) = object.to_minimal(&TypeRegistry::new()).unwrap()
    else {
        panic!("not a struct");
    };
    assert_eq!(minimal.struct_flags.0, StructTypeFlag::IS_APPENDABLE);
    assert_eq!(
        minimal.member_seq[0].common.member_flags.0,
        StructMemberFlag::IS_KEY | StructMemberFlag::TRY_CONSTRUCT1
    );
}
