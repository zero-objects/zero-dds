// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The collection TypeObjects of DDS-XTypes 1.3 Annex B against the
//! serializer of Cyclone DDS 11.0.1.
//!
//! The expected octets are the output of
//! `internal/release/evidence/rc7-typeobject-collections/cyclone-collection-oracle.c`:
//! `dds_stream_write_sampleLE` with Cyclone's installed
//! `DDS_XTypes_TypeObject` descriptor, encoding version 2 for all eight
//! objects and version 1 for the three minimal ones. Cyclone refuses to
//! write a complete collection under version 1 (the call returns false), so
//! the complete objects are held to their own XCDR1 round trip only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use zerodds_types::framing::{xcdr1_reader, xcdr1_writer, xcdr2_reader, xcdr2_writer};
use zerodds_types::type_object::TypeObject;
use zerodds_types::type_object::common::{
    AppliedBuiltinMemberAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
};
use zerodds_types::type_object::complete::{
    CompleteArrayType, CompleteCollectionElement, CompleteMapType, CompleteSequenceType,
};
use zerodds_types::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
use zerodds_types::type_object::minimal::{
    CommonCollectionElement, MinimalArrayType, MinimalCollectionElement, MinimalMapType,
    MinimalSequenceType,
};
use zerodds_types::{CompleteTypeObject, MinimalTypeObject, PrimitiveKind, TypeIdentifier};
use zerodds_wire::Endianness;

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn common(kind: PrimitiveKind) -> CommonCollectionElement {
    CommonCollectionElement {
        // TRY_CONSTRUCT1, as the oracle sets it.
        element_flags: CollectionElementFlag(1),
        type_id: TypeIdentifier::Primitive(kind),
    }
}

fn element(kind: PrimitiveKind) -> CompleteCollectionElement {
    CompleteCollectionElement {
        common: common(kind),
        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
        ann_custom: OptionalAppliedAnnotationSeq::default(),
    }
}

fn detail(name: &str) -> CompleteTypeDetail {
    CompleteTypeDetail {
        ann_builtin: Default::default(),
        ann_custom: OptionalAppliedAnnotationSeq::default(),
        type_name: name.into(),
    }
}

fn sequence(detail_name: Option<&str>) -> CompleteSequenceType {
    CompleteSequenceType {
        collection_flag: CollectionTypeFlag(0),
        bound: 10,
        detail: detail_name.map(detail),
        element: element(PrimitiveKind::Int32),
    }
}

fn xcdr2(object: &TypeObject) -> Vec<u8> {
    let mut w = xcdr2_writer();
    object.encode_into(&mut w).unwrap();
    w.finish()
}

fn xcdr1(object: &TypeObject) -> Vec<u8> {
    let mut w = xcdr1_writer(Endianness::Little);
    object.encode_into(&mut w).unwrap();
    w.finish()
}

/// Encode under XCDR2 to Cyclone's octets and read them back.
fn against_cyclone_xcdr2(object: TypeObject, cyclone: &str) {
    let bytes = hex(cyclone);
    assert_eq!(xcdr2(&object), bytes, "XCDR2 octets of {object:?}");
    let mut r = xcdr2_reader(&bytes);
    assert_eq!(TypeObject::decode_from(&mut r).unwrap(), object);
    assert_eq!(r.remaining(), 0);
}

fn xcdr1_round_trip(object: &TypeObject) -> Vec<u8> {
    let bytes = xcdr1(object);
    let mut r = xcdr1_reader(&bytes, Endianness::Little);
    assert_eq!(&TypeObject::decode_from(&mut r).unwrap(), object);
    assert_eq!(r.remaining(), 0);
    bytes
}

#[test]
fn a_named_complete_sequence_is_cyclones() {
    let object = TypeObject::Complete(CompleteTypeObject::Sequence(sequence(Some("::S"))));
    against_cyclone_xcdr2(
        object.clone(),
        "21000000f2600000100000000a00000001000000040000003a3a5300050000000100040000",
    );
    xcdr1_round_trip(&object);
}

#[test]
fn an_anonymous_complete_sequence_has_no_detail() {
    let object = TypeObject::Complete(CompleteTypeObject::Sequence(sequence(None)));
    against_cyclone_xcdr2(
        object.clone(),
        "19000000f2600000050000000a00000000000000050000000100040000",
    );
    xcdr1_round_trip(&object);
}

#[test]
fn an_annotated_element_carries_its_builtin_annotations() {
    let mut s = sequence(Some("::S"));
    s.element.ann_builtin.unit = Some("m".into());
    let object = TypeObject::Complete(CompleteTypeObject::Sequence(s));
    against_cyclone_xcdr2(
        object.clone(),
        concat!(
            "32000000f2600000100000000a00000001000000040000003a3a5300",
            "16000000010004010d00000001000000020000006d0000000000"
        ),
    );
    xcdr1_round_trip(&object);
}

#[test]
fn a_complete_array_is_appendable_around_its_header_and_element() {
    let object = TypeObject::Complete(CompleteTypeObject::Array(CompleteArrayType {
        collection_flag: CollectionTypeFlag(0),
        bound_seq: vec![2, 3],
        detail: detail("::A"),
        element: element(PrimitiveKind::Int16),
    }));
    against_cyclone_xcdr2(
        object.clone(),
        concat!(
            "31000000f2610000290000000000000018000000020000000200000003000000",
            "00000000040000003a3a4100050000000100030000"
        ),
    );
    xcdr1_round_trip(&object);
}

#[test]
fn a_complete_map_is_cyclones() {
    let object = TypeObject::Complete(CompleteTypeObject::Map(CompleteMapType {
        collection_flag: CollectionTypeFlag(0),
        bound: 5,
        detail: Some(detail("::M")),
        key: element(PrimitiveKind::Int32),
        element: element(PrimitiveKind::Int64),
    }));
    against_cyclone_xcdr2(
        object.clone(),
        concat!(
            "2d000000f2620000100000000500000001000000040000003a3a4d00",
            "050000000100040000000000050000000100050000"
        ),
    );
    xcdr1_round_trip(&object);
}

#[test]
fn the_minimal_collections_are_cyclones_under_both_encodings() {
    let cases = [
        (
            TypeObject::Minimal(MinimalTypeObject::Sequence(MinimalSequenceType {
                collection_flag: CollectionTypeFlag(0),
                bound: 10,
                element: MinimalCollectionElement {
                    common: common(PrimitiveKind::Int32),
                },
            })),
            "13000000f1600000040000000a00000003000000010004",
            "f16000000a000000010004",
        ),
        (
            TypeObject::Minimal(MinimalTypeObject::Array(MinimalArrayType {
                collection_flag: CollectionTypeFlag(0),
                bound_seq: vec![2, 3],
                element: MinimalCollectionElement {
                    common: common(PrimitiveKind::Int16),
                },
            })),
            "1b000000f16100000c00000002000000020000000300000003000000010003",
            "f1610000020000000200000003000000010003",
        ),
        (
            TypeObject::Minimal(MinimalTypeObject::Map(MinimalMapType {
                collection_flag: CollectionTypeFlag(0),
                bound: 5,
                key: MinimalCollectionElement {
                    common: common(PrimitiveKind::Int32),
                },
                element: MinimalCollectionElement {
                    common: common(PrimitiveKind::Int64),
                },
            })),
            "1b000000f16200000400000005000000030000000100040003000000010005",
            "f16200000500000001000400010005",
        ),
    ];
    for (object, cyclone_xcdr2, cyclone_xcdr1) in cases {
        against_cyclone_xcdr2(object.clone(), cyclone_xcdr2);
        assert_eq!(
            xcdr1_round_trip(&object),
            hex(cyclone_xcdr1),
            "XCDR1 octets of {object:?}"
        );
    }
}
