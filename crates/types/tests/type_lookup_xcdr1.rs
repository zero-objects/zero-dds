// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeLookup service samples in the encoding version 1, `CDR_LE`.
//!
//! The expected octets below are written out by hand from the clauses, not
//! captured from another implementation: no measured vendor sends this form
//! (see `docs/spec-coverage/ddsi-rtps-2.5.md`, §10.4). They are this
//! project's reading of XTypes 1.3 §7.4.1 and not an oracle; what they pin
//! is that the codec writes that reading and reads it back.
//!
//! - §7.4.1.1: final and appendable aggregates are plain CDR, no `DHEADER`;
//!   §7.4.1.1.1 aligns a primitive to its own size.
//! - §7.4.1.1.5.2 and §7.4.1.2: an `@optional` member of a final or
//!   appendable structure has a parameter header with the member id and
//!   the exact length, zero when absent, and no `PID_LIST_END` after it.
//! - §7.4.1.2: a mutable aggregate is a parameter list; each value is its
//!   own CDR block (alignment counts from its start), the length is exact,
//!   the next header starts four-aligned, and `PID_LIST_END` closes it.
//! - §7.4.1.2.2 and Table 34: member ids above `0x3F00` take
//!   `PID_EXTENDED`, `0x7F01` with the must-understand bit; `PID_LIST_END`
//!   is `0x7F02`; the `eMemberHeader` has `FLAG_2` only for a
//!   must-understand member.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_types::framing::{xcdr1_reader, xcdr1_writer};
use zerodds_types::type_identifier::{EquivalenceHash, PrimitiveKind, TypeIdentifier};
use zerodds_types::type_lookup::{
    GetTypesReply, GetTypesRequest, GetTypesResult, ReplyHeader, RequestHeader, TypeIdentifierPair,
    TypeIdentifierTypeObjectPair, TypeLookupCall, TypeLookupEncapsulation, TypeLookupEncoding,
    TypeLookupReplyMessage, TypeLookupRequest, TypeLookupReturn,
};
use zerodds_types::type_object::TypeObject;
use zerodds_types::type_object::common::{
    AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CommonStructMember,
    CompleteMemberDetail, CompleteTypeDetail, NameHash, OptionalAppliedAnnotationSeq,
};
use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
use zerodds_types::type_object::minimal::{
    MinimalStructHeader, MinimalStructMember, MinimalStructType,
};
use zerodds_types::{MinimalTypeObject, framing::EncodingVersion};
use zerodds_wire::Endianness;

fn hex(text: &str) -> Vec<u8> {
    let digits: String = text.chars().filter(char::is_ascii_hexdigit).collect();
    (0..digits.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn a_get_types_request_is_cdr_le_with_its_parameters_as_a_parameter_list() {
    let request = TypeLookupRequest {
        header: RequestHeader {
            writer_guid: [0x5a; 16],
            sequence_number: 0x0102_0304_0506_0708,
            instance_name: "dds.builtin.TOS.x".into(),
        },
        data: TypeLookupCall::GetTypes(GetTypesRequest {
            type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                [0x11; 14],
            ))],
        }),
    };
    let expected = hex(concat!(
        "0001 0000",                                         // CDR_LE, options 0
        "5a5a5a5a 5a5a5a5a 5a5a5a5a 5a5a5a5a",               // requestId.writer_guid
        "04030201 08070605",                                 // sequence_number high, low
        "12000000 6464732e 6275696c 74696e2e 544f532e 7800", // instanceName
        "0000",                                              // to the union's long
        "d3528201",                                          // TypeLookup_getTypes_HashId
        "017f0800 6560530c 13000000",                        // PID_EXTENDED, type_ids, 19
        "01000000 f1 1111111111111111111111111111",          // one EK_MINIMAL identifier
        "00",                                                // to the next header
        "027f0000",                                          // PID_LIST_END
    ));
    let payload = request.to_payload(TypeLookupEncoding::Xcdr1).unwrap();
    assert_eq!(payload, expected);

    let encapsulation = TypeLookupEncapsulation::read(&payload).unwrap();
    assert_eq!(encapsulation.version, EncodingVersion::Xcdr1);
    let mut reader = encapsulation.reader(&payload[4..]);
    assert_eq!(
        TypeLookupRequest::decode_from(&mut reader).unwrap(),
        request
    );
    assert_eq!(reader.remaining(), 0);
}

fn minimal_struct() -> TypeObject {
    TypeObject::Minimal(MinimalTypeObject::Struct(MinimalStructType {
        struct_flags: StructTypeFlag(0x0002),
        header: MinimalStructHeader {
            base_type: TypeIdentifier::None,
        },
        member_seq: vec![MinimalStructMember {
            common: CommonStructMember {
                member_id: 0,
                member_flags: StructMemberFlag(0x0001),
                member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
            },
            detail: NameHash([0x0c, 0xc1, 0x75, 0xb9]),
        }],
    }))
}

#[test]
fn a_get_types_reply_carries_its_object_in_the_encoding_version_1() {
    let reply = TypeLookupReplyMessage {
        header: ReplyHeader {
            writer_guid: [0x5a; 16],
            sequence_number: 0x0102_0304_0506_0708,
            remote_ex: 0,
        },
        ret: TypeLookupReturn::GetTypes(GetTypesResult::Ok(GetTypesReply {
            types: vec![TypeIdentifierTypeObjectPair {
                type_identifier: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                    [0x22; 14],
                )),
                type_object: minimal_struct(),
            }],
            complete_to_minimal: Vec::<TypeIdentifierPair>::new(),
        })),
    };
    let expected = hex(concat!(
        "0001 0000",
        "5a5a5a5a 5a5a5a5a 5a5a5a5a 5a5a5a5a", // relatedRequestId.writer_guid
        "04030201 08070605",                   // sequence_number
        "00000000",                            // remoteEx
        "d3528201",                            // TypeLookup_Return: getTypes
        "00000000",                            // _Result: DDS_RETCODE_OK
        "017f0800 d14a8002 2b000000",          // PID_EXTENDED, types, 43
        "01000000",                            // one pair
        "f1 2222222222222222222222222222",     // its identifier
        "f1",                                  // TypeObject: EK_MINIMAL, no DHEADER
        "51",                                  // MinimalTypeObject: TK_STRUCTURE
        "00 0200",                             // to two; struct_flags
        "00",                                  // header.base_type: TK_NONE
        "000000 01000000",                     // to four; one member, no DHEADER
        "00000000 0100 04",                    // member_id, flags, TK_INT32
        "0cc175b9",                            // name_hash of "a"
        "00",                                  // to the next header
        "017f0800 77658e0b 04000000 00000000", // complete_to_minimal, empty
        "027f0000",                            // PID_LIST_END
    ));
    let payload = reply.to_payload(TypeLookupEncoding::Xcdr1).unwrap();
    assert_eq!(payload, expected);

    let mut reader = xcdr1_reader(&payload[4..], Endianness::Little);
    assert_eq!(
        TypeLookupReplyMessage::decode_from(&mut reader).unwrap(),
        reply
    );
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn absent_optional_members_are_parameter_headers_of_length_zero() {
    let detail = CompleteTypeDetail {
        ann_builtin: AppliedBuiltinTypeAnnotations::default(),
        ann_custom: OptionalAppliedAnnotationSeq::default(),
        type_name: "::X".into(),
    };
    let mut w = xcdr1_writer(Endianness::Little);
    detail.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(
        bytes,
        hex(concat!(
            "00000000", // ann_builtin: member 0, length 0
            "01000000", // ann_custom: member 1, length 0
            "04000000 3a3a5800",
        ))
    );
    let mut r = xcdr1_reader(&bytes, Endianness::Little);
    assert_eq!(CompleteTypeDetail::decode_from(&mut r).unwrap(), detail);
}

#[test]
fn a_present_optional_record_nests_its_own_optional_members() {
    let detail = CompleteMemberDetail {
        name: "a".into(),
        ann_builtin: AppliedBuiltinMemberAnnotations {
            unit: Some("m".into()),
            ..AppliedBuiltinMemberAnnotations::default()
        },
        ann_custom: OptionalAppliedAnnotationSeq::default(),
    };
    let mut w = xcdr1_writer(Endianness::Little);
    detail.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(
        bytes,
        hex(concat!(
            "02000000 6100", // name
            "0000",          // to four
            "01001800",      // ann_builtin: member 1, 24 octets
            "00000600",      //   unit: member 0, 6 octets
            "02000000 6d00", //   "m"
            "0000",          //   to the next header
            "01000000",      //   min: member 1, absent
            "02000000",      //   max: member 2, absent
            "03000000",      //   hash_id: member 3, absent
            "02000000",      // ann_custom: member 2, absent
        ))
    );
    let mut r = xcdr1_reader(&bytes, Endianness::Little);
    assert_eq!(CompleteMemberDetail::decode_from(&mut r).unwrap(), detail);
}

#[test]
fn an_optional_header_naming_another_member_is_refused() {
    // ann_builtin of CompleteTypeDetail is member 0; this header says 5.
    let bytes = hex("05000000 01000000 04000000 3a3a5800");
    let mut r = xcdr1_reader(&bytes, Endianness::Little);
    assert!(CompleteTypeDetail::decode_from(&mut r).is_err());
}

#[test]
fn the_encapsulation_names_the_encoding_version() {
    let read = |id: [u8; 2]| TypeLookupEncapsulation::read(&[id[0], id[1], 0, 0]);
    assert_eq!(read([0, 0]).unwrap().version, EncodingVersion::Xcdr1);
    assert_eq!(read([0, 0]).unwrap().endianness, Endianness::Big);
    assert_eq!(read([0, 1]).unwrap().version, EncodingVersion::Xcdr1);
    assert_eq!(read([0, 6]).unwrap().version, EncodingVersion::Xcdr2);
    assert_eq!(read([0, 7]).unwrap().version, EncodingVersion::Xcdr2);
    assert_eq!(read([0, 7]).unwrap().endianness, Endianness::Little);
    // PL_CDR_LE would say the outermost type is mutable, and it is not.
    assert_eq!(read([0, 3]), Err(Some([0, 3])));
    assert_eq!(TypeLookupEncapsulation::read(&[0, 1]), Err(None));
}

#[test]
fn both_encodings_carry_the_same_request() {
    let request = TypeLookupRequest {
        header: RequestHeader {
            writer_guid: [7; 16],
            sequence_number: 9,
            instance_name: "dds.builtin.TOS.y".into(),
        },
        data: TypeLookupCall::GetTypes(GetTypesRequest {
            type_ids: vec![TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(
                [3; 14],
            ))],
        }),
    };
    for encoding in [TypeLookupEncoding::Xcdr1, TypeLookupEncoding::Xcdr2] {
        let payload = request.to_payload(encoding).unwrap();
        assert_eq!(payload[..2], encoding.representation_identifier());
        // XTypes 1.3 §7.6.3.1.2: the sample ends four-aligned and the
        // options count the zero octets that took it there.
        assert_eq!(payload.len() % 4, 0, "{encoding:?}");
        let padding = usize::from(payload[3] & 0x03);
        assert!(payload[payload.len() - padding..].iter().all(|&b| b == 0));
        let encapsulation = TypeLookupEncapsulation::read(&payload).unwrap();
        let mut reader = encapsulation.reader(&payload[4..payload.len() - padding]);
        assert_eq!(
            TypeLookupRequest::decode_from(&mut reader).unwrap(),
            request,
            "{encoding:?}"
        );
        assert_eq!(reader.remaining(), 0, "{encoding:?}");
    }
    // The XCDR2 form of this request ends three octets past a four-octet
    // offset: one identifier is fifteen octets.
    assert_eq!(
        request.to_payload(TypeLookupEncoding::Xcdr2).unwrap()[2..4],
        [0, 1]
    );
    assert_eq!(TypeLookupEncoding::default(), TypeLookupEncoding::Xcdr2);
    assert_eq!(
        TypeLookupEncoding::parse("xcdr1"),
        Some(TypeLookupEncoding::Xcdr1)
    );
    assert_eq!(TypeLookupEncoding::parse("cdr2"), None);
}

/// The `default:` arm of `TypeIdentifier`: the discriminator, then
/// `ExtendedTypeDefn`, a mutable structure with no member in XTypes 1.3,
/// which under XCDR1 is a parameter list of `PID_LIST_END` alone
/// (§7.4.1.2), four-aligned after the octet.
#[test]
fn an_extended_identifier_is_its_discriminator_and_an_empty_parameter_list() {
    let extended = TypeIdentifier::Extended {
        discriminator: 0x7e,
        body: Vec::new(),
    };
    let mut w = xcdr1_writer(Endianness::Little);
    extended.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, hex("7e000000 027f0000"));
    let mut r = xcdr1_reader(&bytes, Endianness::Little);
    assert_eq!(TypeIdentifier::decode_from(&mut r).unwrap(), extended);
    assert_eq!(r.remaining(), 0);
}

/// A member of a later revision in the XCDR1 list is one this revision
/// does not have: stepped over when it may be (§7.4.1.2.3), and the value
/// refused when it is must-understand (§7.4.1.2.1).
#[test]
fn an_unknown_member_of_an_extended_arm_is_skipped_unless_it_must_be_understood() {
    let read = |text: &str| {
        let bytes = hex(text);
        let mut r = xcdr1_reader(&bytes, Endianness::Little);
        TypeIdentifier::decode_from(&mut r)
    };
    assert_eq!(
        read("7e000000 05000400 01020304 027f0000").unwrap(),
        TypeIdentifier::Extended {
            discriminator: 0x7e,
            body: Vec::new(),
        }
    );
    assert!(read("7e000000 05400400 01020304 027f0000").is_err());
}

/// Members read under XCDR2 for a type of a later revision cannot be framed
/// again as XCDR1 parameters without that type.
#[test]
fn extended_members_read_under_xcdr2_have_no_xcdr1_form() {
    let extended = TypeIdentifier::Extended {
        discriminator: 0x7e,
        body: vec![1, 2, 3, 4],
    };
    let mut w = xcdr1_writer(Endianness::Little);
    assert!(extended.encode_into(&mut w).is_err());
}

/// `MinimalExtendedType` and `CompleteExtendedType`, the `default:` arms of
/// the two TypeObject unions, under XCDR1.
#[test]
fn an_extended_type_object_round_trips_under_xcdr1() {
    use zerodds_types::CompleteTypeObject;
    for object in [
        TypeObject::Minimal(MinimalTypeObject::Extended {
            type_kind: 0x7d,
            body: Vec::new(),
        }),
        TypeObject::Complete(CompleteTypeObject::Extended {
            type_kind: 0x7d,
            body: Vec::new(),
        }),
    ] {
        let mut w = xcdr1_writer(Endianness::Little);
        object.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        assert_eq!(bytes[1..2], [0x7d]);
        assert_eq!(bytes[4..], hex("027f0000"));
        let mut r = xcdr1_reader(&bytes, Endianness::Little);
        assert_eq!(TypeObject::decode_from(&mut r).unwrap(), object);
    }
}

/// `ExtendedAnnotationParameterValue`, the `default:` arm of
/// `AnnotationParameterValue`, as the `@min` of a member under XCDR1.
#[test]
fn an_extended_annotation_value_round_trips_under_xcdr1() {
    use zerodds_types::type_object::common::AnnotationParameterValue;
    let detail = CompleteMemberDetail {
        name: "a".into(),
        ann_builtin: AppliedBuiltinMemberAnnotations {
            min: Some(AnnotationParameterValue::Extended {
                kind: 0x7c,
                body: Vec::new(),
            }),
            ..AppliedBuiltinMemberAnnotations::default()
        },
        ann_custom: OptionalAppliedAnnotationSeq::default(),
    };
    let mut w = xcdr1_writer(Endianness::Little);
    detail.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    let mut r = xcdr1_reader(&bytes, Endianness::Little);
    assert_eq!(CompleteMemberDetail::decode_from(&mut r).unwrap(), detail);
}
