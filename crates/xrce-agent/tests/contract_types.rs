// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xrce-agent` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier"). Built with the
//! `daemon` feature (`required-features` in `Cargo.toml`): only the daemon
//! links `zerodds-types` (features `alloc`).
//!
//! What the daemon uses of `zerodds-types`:
//! - `src/daemon/entity_repr.rs`: a binary type representation is read with
//!   `framing::xcdr2_reader(bytes)` and `TypeObject::decode_from`, refused
//!   when `remaining()` is not zero or the object is `TypeObject::Minimal`;
//!   the name of a `TypeObject::Complete` is
//!   `header.detail.type_name` of `CompleteTypeObject::Struct`, `Union`,
//!   `Enumerated`, `Alias`, and `detail.type_name` of `Bitmask`, `Bitset`.
//! - `src/daemon/binary_forms.rs`: the `type_identifier` of an
//!   `OBJK_Topic_Binary` is `TypeIdentifier::decode_from` on the blob's
//!   XCDR2 reader, in the message's byte order.
//! - `src/daemon/session.rs`: a topic named by identifier is matched with
//!   `hash::to_hashed_type_identifier(&TypeObject::Complete(def))` equal to
//!   that identifier; `CompleteTypeObject` is stored per type name
//!   (`profiles.rs`, `server.rs`, `dds_port.rs`).
//!
//! Oracle: DDS-XRCE 1.0 §7.7.3.1.3 and §7.7.3.7.3 (the binary type
//! representation is a `TypeObject` in XCDR version 2; a topic may name its
//! type by `TypeIdentifier`); DDS-XTypes 1.3 §7.3.4.2 Table 29 (`EK_MINIMAL`
//! 0xF1, `EK_COMPLETE` 0xF2), §7.3.4.5 (the `TypeObject` serialization),
//! §7.3.4.6.2 (a hash identifier is the first 14 octets of the MD5 of the
//! serialized object); and CycloneDDS 11.0.1 `idlc`'s type objects for
//! `@final struct to::Plain { long a; short b; }`, recorded in
//! `crates/types/tests/typeobject_vendor_dheader.rs`. The hashes are MD5
//! prefixes of those octets computed with Python's `hashlib`.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::from_ast::complete_type_object;
use zerodds_types::hash::to_hashed_type_identifier;
use zerodds_types::{CompleteTypeObject, EquivalenceHash, TypeIdentifier, TypeObject};
use zerodds_wire::{Endianness, Reader};

const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const PLAIN_MINIMAL: &str = "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe";
const PLAIN_COMPLETE_HASH: [u8; 14] = [
    0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7,
];

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn xtypes_7_3_4_5_a_complete_type_object_reads_to_the_end_of_the_blob() {
    let bytes = unhex(PLAIN_COMPLETE);
    let mut r = zerodds_types::framing::xcdr2_reader(&bytes);
    let object = TypeObject::decode_from(&mut r).unwrap();
    assert_eq!(r.remaining(), 0);
    let TypeObject::Complete(CompleteTypeObject::Struct(st)) = &object else {
        panic!("a complete struct: {object:?}");
    };
    assert_eq!(st.header.detail.type_name, "to::Plain");

    // One octet behind the object is left in the reader for the agent to
    // refuse.
    let mut longer = bytes.clone();
    longer.push(0);
    let mut r = zerodds_types::framing::xcdr2_reader(&longer);
    TypeObject::decode_from(&mut r).unwrap();
    assert_eq!(r.remaining(), 1);
}

#[test]
fn xtypes_7_3_4_5_a_minimal_type_object_decodes_as_minimal() {
    let bytes = unhex(PLAIN_MINIMAL);
    let mut r = zerodds_types::framing::xcdr2_reader(&bytes);
    assert!(matches!(
        TypeObject::decode_from(&mut r).unwrap(),
        TypeObject::Minimal(_)
    ));
}

#[test]
fn xtypes_7_3_4_6_2_the_hashed_identifier_of_a_complete_object_is_ek_complete_and_md5() {
    let bytes = unhex(PLAIN_COMPLETE);
    let object = TypeObject::from_bytes_le(&bytes).unwrap();
    assert_eq!(
        to_hashed_type_identifier(&object).unwrap(),
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(PLAIN_COMPLETE_HASH))
    );
}

#[test]
fn xrce_7_7_3_7_3_a_type_identifier_in_a_topic_blob_reads_in_either_byte_order() {
    // `topic_name` "T", no `type_reference`, a `type_identifier`: EK_COMPLETE
    // and the 14 octets, which no byte order changes.
    for (endianness, length) in [
        (Endianness::Little, [0x02, 0x00, 0x00, 0x00]),
        (Endianness::Big, [0x00, 0x00, 0x00, 0x02]),
    ] {
        let mut blob = length.to_vec();
        blob.extend_from_slice(&[b'T', 0x00, 0x00, 0x01, 0xf2]);
        blob.extend_from_slice(&PLAIN_COMPLETE_HASH);
        let mut r = Reader::with_max_align(&blob, endianness, 4);
        assert_eq!(r.read_string().unwrap(), "T");
        assert!(!r.read_bool().unwrap());
        assert!(r.read_bool().unwrap());
        assert_eq!(
            TypeIdentifier::decode_from(&mut r).unwrap(),
            TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(PLAIN_COMPLETE_HASH))
        );
        assert_eq!(r.remaining(), 0);
    }
}

#[test]
fn xtypes_7_3_4_3_every_named_complete_kind_carries_its_qualified_name() {
    let ast = zerodds_wire_probe::encode::model_from_idl(
        "module m {
           struct S { long a; };
           union U switch (long) { case 1: long x; };
           enum E { A, B };
           typedef long L;
           bitmask BM { F0, F1 };
           bitset BS { bitfield<3> x; };
         };",
        "contract-types.idl",
    )
    .unwrap();
    let complete = |name: &str| {
        let id = ast
            .types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no type `{name}`"));
        complete_type_object(&ast, id).unwrap()
    };
    let named = |object: &CompleteTypeObject| -> Option<String> {
        match object {
            CompleteTypeObject::Struct(s) => Some(s.header.detail.type_name.clone()),
            CompleteTypeObject::Union(u) => Some(u.header.detail.type_name.clone()),
            CompleteTypeObject::Enumerated(e) => Some(e.header.detail.type_name.clone()),
            CompleteTypeObject::Alias(a) => Some(a.header.detail.type_name.clone()),
            CompleteTypeObject::Bitmask(b) => Some(b.detail.type_name.clone()),
            CompleteTypeObject::Bitset(b) => Some(b.detail.type_name.clone()),
            _ => None,
        }
    };
    for (name, qualified) in [
        ("S", "m::S"),
        ("U", "m::U"),
        ("E", "m::E"),
        ("L", "m::L"),
        ("BM", "m::BM"),
        ("BS", "m::BS"),
    ] {
        assert_eq!(named(&complete(name)).as_deref(), Some(qualified));
    }
}
