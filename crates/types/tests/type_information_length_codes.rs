// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `PID_TYPE_INFORMATION` as the four vendors actually write it.
//!
//! `TypeInformation` is an `@mutable` aggregate (XTypes 1.3 §7.6.3.2.1),
//! so each of its two members is introduced by an `EMHEADER` and
//! §7.4.3.4.2 leaves the length code to the sender. Cyclone DDS, Fast
//! DDS, OpenDDS and ZeroDDS all write **LC=4** — a separately serialized
//! `NEXTINT`. RTI Connext writes **LC=5**, where the `NEXTINT` is
//! *reused* as the member's own leading length word and is not
//! serialized a second time. Both are conformant, and the difference is
//! four octets per member.
//!
//! The octets below are RTI Connext 7.7.0's, read off the SEDP
//! announcement of topic `RichF` on the Linux test host; they are quoted
//! in `docs/OPEN-ITEMS.md` beside ZeroDDS's own for the same type.
//! Whatever a decoder does with them, it must arrive at the two
//! `TypeIdentifier`s and the two sizes they carry.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_types::TypeIdentifier;
use zerodds_types::type_information::TypeInformation;
use zerodds_wire::Endianness;

/// One `TypeIdentifierWithDependencies` body: the `TypeIdentifierWithSize`
/// of `hash`, and no dependencies.
///
/// 36 octets — which is the `0x24` both vendors state — laid out as
/// §7.6.3.2.1 does: a DHEADER of 20 over the `TypeIdentifierWithSize`
/// (an `EquivalenceKind` octet, fourteen hash octets, one octet of
/// padding, the `uint32` size), then the dependent count and its empty
/// sequence.
fn with_dependencies(kind: u8, hash: [u8; 14], size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&20u32.to_le_bytes()); // DHEADER, TypeIdentifierWithSize
    out.push(kind);
    out.extend_from_slice(&hash);
    out.push(0); // padding to the uint32
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // dependent_typeid_count
    out.extend_from_slice(&4u32.to_le_bytes()); // DHEADER, the empty sequence
    out.extend_from_slice(&0u32.to_le_bytes()); // its length
    assert_eq!(out.len(), 36);
    out
}

/// The two members of a `TypeInformation`, each introduced with the
/// given length code.
///
/// * `4` — a `NEXTINT` of its own, then the body. What Cyclone, Fast
///   DDS, OpenDDS and ZeroDDS write.
/// * `5` — no separate `NEXTINT`: the body's own leading DHEADER is it.
///   What RTI Connext writes.
fn type_information_bytes(length_code: u8) -> Vec<u8> {
    const MEMBER_ID_MINIMAL: u32 = 0x1001;
    const MEMBER_ID_COMPLETE: u32 = 0x1002;

    let member = |id: u32, body: &[u8]| {
        let mut out = Vec::new();
        let word = (u32::from(length_code) << 28) | id;
        out.extend_from_slice(&word.to_le_bytes());
        if length_code == 4 {
            // The body's length, serialized separately.
            #[allow(clippy::cast_possible_truncation)]
            out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        }
        // The body's own DHEADER — which under LC=5 is also the NEXTINT.
        #[allow(clippy::cast_possible_truncation)]
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        out
    };

    let minimal = with_dependencies(
        0xf1,
        [
            0xd1, 0xb2, 0xad, 0x6d, 0x55, 0x70, 0x68, 0x10, 0x07, 0xd0, 0x30, 0x5d, 0x5a, 0x21,
        ],
        77,
    );
    let complete = with_dependencies(
        0xf2,
        [
            0xc1, 0x64, 0xbf, 0xd8, 0xe6, 0x94, 0x4d, 0xc9, 0xee, 0x5e, 0x2e, 0x5f, 0x5c, 0x20,
        ],
        111,
    );

    let mut members = member(MEMBER_ID_MINIMAL, &minimal);
    members.extend_from_slice(&member(MEMBER_ID_COMPLETE, &complete));

    let mut out = Vec::new();
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(members.len() as u32).to_le_bytes()); // the @appendable DHEADER
    out.extend_from_slice(&members);
    out
}

fn expect_the_two_identifiers(bytes: &[u8], what: &str) {
    let ti = TypeInformation::from_bytes(bytes, Endianness::Little)
        .unwrap_or_else(|e| panic!("{what}: decode failed: {e:?}"));

    let minimal = ti.minimal.typeid_with_size.type_id.clone();
    let complete = ti.complete.typeid_with_size.type_id.clone();

    assert_eq!(
        minimal,
        TypeIdentifier::EquivalenceHashMinimal(zerodds_types::EquivalenceHash([
            0xd1, 0xb2, 0xad, 0x6d, 0x55, 0x70, 0x68, 0x10, 0x07, 0xd0, 0x30, 0x5d, 0x5a, 0x21,
        ])),
        "{what}: the MINIMAL identifier"
    );
    assert_eq!(
        complete,
        TypeIdentifier::EquivalenceHashComplete(zerodds_types::EquivalenceHash([
            0xc1, 0x64, 0xbf, 0xd8, 0xe6, 0x94, 0x4d, 0xc9, 0xee, 0x5e, 0x2e, 0x5f, 0x5c, 0x20,
        ])),
        "{what}: the COMPLETE identifier"
    );
    assert_eq!(
        ti.minimal.typeid_with_size.typeobject_serialized_size, 77,
        "{what}: the MINIMAL size"
    );
    assert_eq!(
        ti.complete.typeid_with_size.typeobject_serialized_size, 111,
        "{what}: the COMPLETE size"
    );
}

/// The length code the open-source stacks write, and the one this
/// decoder was measured against until RTI joined the matrix.
#[test]
fn a_type_information_whose_members_use_length_code_4_decodes() {
    expect_the_two_identifiers(&type_information_bytes(4), "LC=4");
}

/// RTI Connext's. Before the fix this produced
/// `TypeIdentifier::Primitive(Boolean)` — the low octet of the second
/// member's EMHEADER, read as a discriminator — and every endpoint pair
/// with RTI was refused with `kinds do not match`.
#[test]
fn a_type_information_whose_members_use_length_code_5_decodes() {
    expect_the_two_identifiers(&type_information_bytes(5), "LC=5");
}
