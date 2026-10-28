// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What `TypeIdentifierWithSize::typeobject_serialized_size` counts.
//!
//! XTypes 1.3 §7.6.3.2.1 asks `minimal.typeid_with_size` and
//! `complete.typeid_with_size` for "the ... Hash TypeIdentifier of the
//! TopicType and the serialized size of the associated TypeObject". The
//! Annex-B comment on `EquivalenceHash` builds that identifier from the
//! "First 14 bytes of MD5 of the serialized TypeObject using XCDR version 2
//! with Little Endian encoding". Both sentences name one serialization, so
//! the announced size is the length of the stream the hash is taken over —
//! and Annex B declares `union TypeObject` `@extensibility(APPENDABLE)`, so
//! that stream opens with a DHEADER which counts.
//!
//! ZeroDDS used to announce the discriminator-plus-body length instead, four
//! octets short. RTI Connext 7.7.0 checks the number against the record it
//! already holds under the same hash and refuses the endpoint at discovery:
//! `PRESTypeObjectV2Table_assertTypeObjectRecord: BAD PARAMETER FAILURE |
//! TypeObjectV2 serializedSize mismatch (current: 84, new: 80)`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::hash::{
    complete_hash_input, complete_serialized_size, minimal_hash_input, minimal_serialized_size,
};
use zerodds_types::type_object::CompleteTypeObject;
use zerodds_types::{
    EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier, TypeInformation, TypeObject,
};

/// `@extensibility(FINAL) struct Robot { uint32 id; uint32 label; }` —
/// `tests/interop/ci/fastdds/robot.idl`, the type the `ci-rti`,
/// `ci-cyclone` and `ci-fastdds` gate cells exchange.
fn robot() -> (MinimalTypeObject, CompleteTypeObject) {
    let b = TypeObjectBuilder::struct_type("Robot")
        .extensibility(Extensibility::Final)
        .member(
            "id",
            TypeIdentifier::Primitive(PrimitiveKind::UInt32),
            |m| m,
        )
        .member(
            "label",
            TypeIdentifier::Primitive(PrimitiveKind::UInt32),
            |m| m,
        );
    (
        MinimalTypeObject::Struct(b.build_minimal()),
        CompleteTypeObject::Struct(b.build_complete()),
    )
}

/// The size ZeroDDS announces for `Robot` is the size three other stacks
/// announce for the same type and the fourth counts for it.
///
/// Measured, all on the Linux test host, all over the IDL above:
///
/// - **Cyclone DDS 11.0.1**, `idlc robot.idl`, the `TYPE_INFO_CDR_Robot`
///   blob of the generated `robot.c`: `37 00 00 00` = 55 for MINIMAL,
///   `54 00 00 00` = 84 for COMPLETE. Its `TYPE_MAP_CDR_Robot` carries the
///   TypeObjects themselves, opening `33 00 00 00 f1` and `50 00 00 00 f2`
///   — DHEADER 51 resp. 80, then the EquivalenceKind, 55 resp. 84 in total.
/// - **Fast DDS 3.6.1**, `TypeObjectRegistry::get_type_information` after
///   `register_Robot_type_identifier`: 55 and 84. Serializing the same
///   TypeObjects with `fastcdr` XCDRv2 gives 55 and 84 octets, opening
///   `33 00 00 00 f1` and `50 00 00 00 f2`.
/// - **OpenDDS 3.34.0**, `TypeSupportImpl::to_type_info` on the
///   `opendds_idl -Gxtypes-complete` type support: 55 and 84, and
///   `serialized_size(get_typeobject_encoding(), to)` agrees.
/// - **RTI Connext 7.7.0**: counted 84 for the COMPLETE TypeObject of this
///   type and rejected the ZeroDDS endpoint that announced 80.
///
/// The hashes are asserted alongside the sizes because a size golden alone
/// would still pass if the builder started producing a different `Robot`.
/// They are Cyclone's, read off the same `TYPE_INFO_CDR_Robot` blob
/// (`f1 62 54 …` and `f2 aa d3 …`, EquivalenceKind then 14 hash octets), so
/// the assertion is that ZeroDDS builds this type byte-identically.
#[test]
fn robot_is_announced_at_the_size_four_stacks_agree_on() {
    let (minimal, complete) = robot();
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();

    assert_eq!(
        ti.minimal.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([
            0x62, 0x54, 0x36, 0xec, 0xee, 0x22, 0xcc, 0x29, 0x43, 0xeb, 0xc5, 0x98, 0xc4, 0x3e,
        ])),
        "the MINIMAL TypeObject is no longer Cyclone's, so the size golden below means nothing"
    );
    assert_eq!(
        ti.complete.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
            0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
        ])),
        "the COMPLETE TypeObject is no longer Cyclone's and RTI's, \
         so the size golden below means nothing"
    );

    assert_eq!(ti.minimal.typeid_with_size.typeobject_serialized_size, 55);
    assert_eq!(ti.complete.typeid_with_size.typeobject_serialized_size, 84);
}

/// The DHEADER is in the announced number, and it is the same four octets
/// the hash is taken over.
#[test]
fn the_announced_size_opens_with_the_dheader_of_the_appendable_union() {
    let (minimal, complete) = robot();

    let min = minimal_hash_input(&minimal).unwrap();
    assert_eq!(&min[..5], &[0x33, 0x00, 0x00, 0x00, 0xf1]);
    assert_eq!(min.len(), 55);

    let com = complete_hash_input(&complete).unwrap();
    assert_eq!(&com[..5], &[0x50, 0x00, 0x00, 0x00, 0xf2]);
    assert_eq!(com.len(), 84);
}

/// The invariant behind the golden, over a spread of type kinds: whatever
/// a TypeObject is, the number announced next to its hash is the length
/// of the bytes that hash was taken over, and those bytes open with the
/// DHEADER of the appendable union Annex B declares.
///
/// The undelimited body used to be four octets shorter and this asserted
/// the difference. `TypeObject::encode_into` writes the DHEADER itself
/// now and `minimal_hash_input` calls that encoder and adds nothing —
/// one path and not two — so the body *is* the hashed stream. What is
/// left to hold is that the delimiter is there and states the rest of
/// the length, which is what a peer counting the same object counts.
#[test]
fn every_announced_size_is_the_length_of_the_hashed_stream() {
    let simple = TypeObjectBuilder::struct_type("::probe::Simple")
        .extensibility(Extensibility::Final)
        .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m);
    let wide = TypeObjectBuilder::struct_type("::probe::Wide")
        .extensibility(Extensibility::Mutable)
        .member("k", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
            m.key()
        })
        .member("s", TypeIdentifier::String8Small { bound: 255 }, |m| m)
        .member(
            "o",
            TypeIdentifier::Primitive(PrimitiveKind::Float64),
            |m| m.optional(),
        );
    let appendable = TypeObjectBuilder::struct_type("::probe::Appendable")
        .extensibility(Extensibility::Appendable)
        .member("x", TypeIdentifier::Primitive(PrimitiveKind::UInt16), |m| m);

    for b in [&simple, &wide, &appendable] {
        let minimal = MinimalTypeObject::Struct(b.build_minimal());
        let complete = CompleteTypeObject::Struct(b.build_complete());
        let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();

        let min_hashed = minimal_hash_input(&minimal).unwrap();
        let com_hashed = complete_hash_input(&complete).unwrap();
        assert_eq!(
            ti.minimal.typeid_with_size.typeobject_serialized_size as usize,
            min_hashed.len()
        );
        assert_eq!(
            ti.complete.typeid_with_size.typeobject_serialized_size as usize,
            com_hashed.len()
        );

        // One encoder, so the announced number and the encoded object
        // cannot differ by a delimiter one of them wrote and the other
        // did not.
        let min_body = TypeObject::Minimal(minimal.clone()).to_bytes_le().unwrap();
        let com_body = TypeObject::Complete(complete.clone())
            .to_bytes_le()
            .unwrap();
        assert_eq!(min_hashed, min_body);
        assert_eq!(com_hashed, com_body);

        // And the delimiter is there, stating the length of everything
        // after it. A stream that lost it would still be self-consistent
        // above; this is what tells the two forms apart.
        for body in [&min_body, &com_body] {
            let dheader =
                u32::from_le_bytes(body[..4].try_into().expect("a DHEADER is four octets"))
                    as usize;
            assert_eq!(
                dheader,
                body.len() - 4,
                "the DHEADER does not state the rest of the object"
            );
        }
    }
}

/// The two size helpers are the single source the announcement draws on, so
/// they have to agree with the hash input they are derived from.
#[test]
fn the_size_helpers_report_the_hashed_length() {
    let (minimal, complete) = robot();
    assert_eq!(
        minimal_serialized_size(&minimal).unwrap() as usize,
        minimal_hash_input(&minimal).unwrap().len()
    );
    assert_eq!(
        complete_serialized_size(&complete).unwrap() as usize,
        complete_hash_input(&complete).unwrap().len()
    );
}
