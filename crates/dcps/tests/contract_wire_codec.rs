// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec`, from its non-test sources:
//! - `value::EncapsulationIds` (`Table60`, `Table39`, `parse`, `identifier`,
//!   `xml_identifier`) and `value::encapsulation_form` for
//!   `wire.encapsulation_ids` and the header a writer puts in front of a
//!   sample (`runtime.rs::encap_in_table`, the config validation).
//! - `decode::padding_present` for where the serialized data of a received
//!   payload ends (`runtime.rs`: `user_encap_body`, the XML path, TypeLookup
//!   requests); the caller refuses a count the body cannot hold.
//! - `ProbeRequest::in_dialect` with `PlainCdr1`/`PlainCdr2`, its
//!   `key_holder`, `key_holder_order`, `emit`, `pl_cdr1_headers` and
//!   `pl_cdr1_reading` fields, `Emit::{KeyHash, PayloadWithEncapsulation}`;
//!   `encode`, `decode`, `decode_prefix`, `encode::key_holder`,
//!   `encode::serialized_key`, `decode::serialized_key`,
//!   `decode::key_holder`, `decode::key_holder_prefix`,
//!   `decode::validate_key_holder`, all over `zerodds_types::dynamic::
//!   DynamicData` (`serialized_type.rs`); `ProbeError::Discarded` told apart
//!   as `DecodeError::Discarded`; `Source::kind` for a refusal
//!   (`xml_data.rs`).
//!
//! Oracles: DDS-XTypes 1.3 §7.4.3.4 Table 39 and §7.6.3.1.2 Table 60 (the
//! encapsulation identifiers), §7.6.3.1.2 (the padding count in the options,
//! its example `TypeB` with options `0x0001`), §7.4.1 and §7.4.3.4.2 (XCDR1
//! and XCDR2 octets, the DHEADER of an `@appendable` type), §7.6.8 (the key
//! holder, zero-padded up to 16 octets, MD5 beyond), §7.2.2.7 and §7.2.4.1
//! (DISCARD is the default); DDSI-RTPS 2.5 §9.4.2.11 (the `serializedKey`)
//! with the Cyclone 11.0.1 measurement `000900000400000007000000` quoted in
//! `serialized_type.rs`; RFC 1321 (MD5, computed outside this code). The
//! model is read from IDL through `zerodds-wire-probe`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::{Ast, PrimitiveKind};
use zerodds_ast_layout::{Dialect, Encoding, Endianness, KeyHolderExtensibility, KeyHolderOrder};
use zerodds_types::dynamic::DynamicData;
use zerodds_wire_codec::value::{EncapsulationIds, encapsulation_form};
use zerodds_wire_codec::{Emit, ProbeError, ProbeRequest, Source, decode, decode_prefix, encode};

const IDL: &str = r"
@appendable struct A { long x; @key long id; long y; };
@final struct F { @key string color; long n; };
@final struct Narrow { string<3> s; };
";

fn model() -> Ast {
    zerodds_wire_probe::encode::model_from_idl(IDL, "contract.idl").expect("the model")
}

/// `SerializedTypeSupport::request`: representation 0 is XCDR1, 2 XCDR2.
fn request(type_name: &str, xcdr2: bool, big_endian: bool) -> ProbeRequest {
    ProbeRequest::in_dialect(
        type_name,
        if xcdr2 {
            Encoding::PlainCdr2
        } else {
            Encoding::PlainCdr1
        },
        if big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        },
        Dialect::default(),
    )
}

fn long(value: i128) -> DynamicData {
    DynamicData::integer(PrimitiveKind::Int32, value)
}

fn sample_a(x: i128, id: i128, y: i128) -> DynamicData {
    DynamicData::Aggregate(vec![
        ("x".into(), long(x)),
        ("id".into(), long(id)),
        ("y".into(), long(y)),
    ])
}

// ---------------------------------------------------------------------
// The encapsulation header
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_1_2_and_7_4_3_4_the_identifiers_of_table_60_and_table_39() {
    use Encoding::{DelimitedCdr, PlCdr1, PlCdr2, PlainCdr1, PlainCdr2};
    use Endianness::{Big, Little};
    // (encoding, order, Table 60, Table 39); version 1 is the same in both.
    let rows = [
        (PlainCdr1, Big, 0x00, 0x00),
        (PlainCdr1, Little, 0x01, 0x01),
        (PlCdr1, Big, 0x02, 0x02),
        (PlCdr1, Little, 0x03, 0x03),
        (PlainCdr2, Big, 0x06, 0x10),
        (PlainCdr2, Little, 0x07, 0x11),
        (DelimitedCdr, Big, 0x08, 0x14),
        (DelimitedCdr, Little, 0x09, 0x15),
        (PlCdr2, Big, 0x0a, 0x12),
        (PlCdr2, Little, 0x0b, 0x13),
    ];
    for (encoding, order, t60, t39) in rows {
        assert_eq!(
            EncapsulationIds::Table60.identifier(encoding, order),
            [0x00, t60]
        );
        assert_eq!(
            EncapsulationIds::Table39.identifier(encoding, order),
            [0x00, t39]
        );
        // A reader takes either table.
        assert_eq!(encapsulation_form([0x00, t60]), Some((encoding, order)));
        assert_eq!(encapsulation_form([0x00, t39]), Some((encoding, order)));
    }
    // XML: {0x00, 0x04} in Table 60, {0x01, 0x00} in Table 39; neither is
    // an encoding of the binary kind.
    assert_eq!(EncapsulationIds::Table60.xml_identifier(), [0x00, 0x04]);
    assert_eq!(EncapsulationIds::Table39.xml_identifier(), [0x01, 0x00]);
    assert_eq!(encapsulation_form([0x00, 0x04]), None);
    assert_eq!(encapsulation_form([0x01, 0x00]), None);

    // `wire.encapsulation_ids`: Table 60 by default, an unknown value refused.
    assert_eq!(EncapsulationIds::default(), EncapsulationIds::Table60);
    assert_eq!(
        EncapsulationIds::parse("table-60"),
        Some(EncapsulationIds::Table60)
    );
    assert_eq!(
        EncapsulationIds::parse("table-39"),
        Some(EncapsulationIds::Table39)
    );
    assert_eq!(EncapsulationIds::parse("Table60"), None);
}

#[test]
fn xtypes_7_6_3_1_2_the_padding_count_ends_the_data_where_it_can_be_padding() {
    use zerodds_wire_codec::decode::padding_present;
    // No count, no padding.
    assert_eq!(padding_present(8, 0x00), 0);
    // The clause's `TypeB { short a; char b; }`: 4 header + 3 data + 1 pad.
    assert_eq!(padding_present(8, 0x01), 1);
    assert_eq!(padding_present(12, 0x03), 3);
    // Only the low two bits of the second options octet are the count.
    assert_eq!(padding_present(8, 0xFF), 3);
    // A payload whose length is no multiple of four carries no padding,
    // whatever the count says (OpenDDS 3.34: count 2 on 70 octets).
    assert_eq!(padding_present(70, 0x02), 0);
    // A count the body cannot hold is answered as is: 5 - 3 leaves less
    // than the header, which the runtime refuses.
    let pad = padding_present(5, 0x03);
    assert_eq!(pad, 3);
    assert!(5_usize.checked_sub(pad).filter(|end| *end >= 4).is_none());
}

// ---------------------------------------------------------------------
// Samples
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_4_3_4_2_a_sample_in_xcdr1_and_xcdr2_both_byte_orders() {
    let ast = model();
    let a = sample_a(1, 7, 2);
    // XCDR2: the @appendable type opens with a DHEADER of 12; XCDR1 has
    // none. The encapsulation header is not part of `Emit::Payload`.
    let cases: [(bool, bool, &[u8]); 4] = [
        (
            true,
            false,
            &[12, 0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0, 2, 0, 0, 0],
        ),
        (
            true,
            true,
            &[0, 0, 0, 12, 0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0, 2],
        ),
        (false, false, &[1, 0, 0, 0, 7, 0, 0, 0, 2, 0, 0, 0]),
        (false, true, &[0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0, 2]),
    ];
    for (xcdr2, big, octets) in cases {
        let request = request("A", xcdr2, big);
        assert_eq!(
            encode(&ast, &request, &a).unwrap(),
            octets,
            "xcdr2 {xcdr2} big {big}"
        );
        let back: DynamicData = decode(&ast, &request, octets).unwrap();
        for (name, value) in [("x", 1), ("id", 7), ("y", 2)] {
            assert_eq!(back.member(name), Some(&long(value)), "{name}");
        }
    }

    // `read_body_prefix`: the first sample of a buffer and its length.
    let mut packed = cases[0].2.to_vec();
    packed.extend_from_slice(&[0xAA; 4]);
    let (first, taken): (DynamicData, usize) =
        decode_prefix(&ast, &request("A", true, false), &packed).unwrap();
    assert_eq!(taken, 16);
    assert_eq!(first.member("id"), Some(&long(7)));

    // `read_body`: octets left behind a @final sample are refused.
    let f = DynamicData::Aggregate(vec![
        ("color".into(), DynamicData::Text("BLUE".into())),
        ("n".into(), long(3)),
    ]);
    let mut body = encode(&ast, &request("F", true, false), &f).unwrap();
    assert_eq!(
        body,
        [5, 0, 0, 0, b'B', b'L', b'U', b'E', 0, 0, 0, 0, 3, 0, 0, 0]
    );
    body.extend_from_slice(&[0xAA; 4]);
    let refused: Result<DynamicData, ProbeError> = decode(&ast, &request("F", true, false), &body);
    assert!(refused.is_err());
}

#[test]
fn xtypes_7_2_4_1_a_value_the_type_cannot_hold_is_discarded() {
    let ast = model();
    let narrow = request("Narrow", true, false);
    // `string<3>` with no `@try_construct`: DISCARD (§7.2.2.7).
    let orange = [7, 0, 0, 0, b'O', b'R', b'A', b'N', b'G', b'E', 0];
    let refused: Result<DynamicData, ProbeError> = decode(&ast, &narrow, &orange);
    assert!(
        matches!(refused, Err(ProbeError::Discarded { .. })),
        "{refused:?}"
    );
    let red = [4, 0, 0, 0, b'R', b'E', b'D', 0];
    let kept: DynamicData = decode(&ast, &narrow, &red).unwrap();
    assert_eq!(kept.member("s"), Some(&DynamicData::Text("RED".into())));
    // The name a refusal gives for a value of the wrong shape.
    assert!(!Source::kind(&kept).is_empty());
}

// ---------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_8_the_key_holder_and_key_hash_of_a_sample() {
    let ast = model();
    // `SerializedTypeSupport::sample`: the holder and the hash from the
    // big-endian XCDR2 request.
    let mut keyed = request("A", true, true);
    zerodds_wire_codec::decode::validate_key_holder(&ast, &keyed).unwrap();
    let a = sample_a(1, 7, 2);
    let holder = zerodds_wire_codec::encode::key_holder(&ast, &keyed, &a).unwrap();
    assert_eq!(holder, [0, 0, 0, 7]);
    keyed.emit = Emit::KeyHash;
    // At most 4 octets: the holder, zero-padded to 16.
    let hash = encode(&ast, &keyed, &a).unwrap();
    assert_eq!(hash, [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    // An unbounded string key: no maximum below 16, so the MD5 of the
    // holder `00000005 424c5545 00` (RFC 1321, computed outside this code).
    let mut keyed_f = request("F", true, true);
    zerodds_wire_codec::decode::validate_key_holder(&ast, &keyed_f).unwrap();
    let f = DynamicData::Aggregate(vec![
        ("color".into(), DynamicData::Text("BLUE".into())),
        ("n".into(), long(3)),
    ]);
    assert_eq!(
        zerodds_wire_codec::encode::key_holder(&ast, &keyed_f, &f).unwrap(),
        [0, 0, 0, 5, b'B', b'L', b'U', b'E', 0]
    );
    keyed_f.emit = Emit::KeyHash;
    assert_eq!(
        encode(&ast, &keyed_f, &f).unwrap(),
        [
            0xca, 0xc2, 0x17, 0xc3, 0x18, 0x36, 0x3f, 0x8e, 0xf1, 0x16, 0x0e, 0xee, 0xde, 0xf9,
            0xe8, 0x86
        ]
    );
}

#[test]
fn xtypes_7_6_8_a_zero_padded_key_hash_and_a_holder_read_back() {
    let ast = model();
    let keyed = request("A", true, true);
    // `decode_key_holder`: the holder alone.
    let from_holder: DynamicData =
        zerodds_wire_codec::decode::key_holder(&ast, &keyed, &[0, 0, 0, 7]).unwrap();
    assert_eq!(from_holder.member("id"), Some(&long(7)));
    // `decode_zero_padded_key_holder`: the holder is a prefix of the hash
    // and every octet after it is zero.
    let hash = [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let (from_hash, consumed): (DynamicData, usize) =
        zerodds_wire_codec::decode::key_holder_prefix(&ast, &keyed, &hash).unwrap();
    assert_eq!(consumed, 4);
    assert!(hash[consumed..].iter().all(|octet| *octet == 0));
    assert_eq!(from_hash.member("id"), Some(&long(7)));
}

#[test]
fn rtps_9_4_2_11_the_serialized_key_is_the_key_members_in_the_types_own_framing() {
    let ast = model();
    // `write_key_only_data` for an @appendable type under XCDR2 LE:
    // D_CDR2_LE, a DHEADER over `id` alone. The octets Cyclone 11.0.1 was
    // measured to write for `id = 7`.
    let mut write = ProbeRequest::in_dialect(
        "A",
        Encoding::DelimitedCdr,
        Endianness::Little,
        Dialect::default(),
    );
    write.key_holder = KeyHolderExtensibility::AsDeclared;
    write.key_holder_order = KeyHolderOrder::Declaration;
    write.emit = Emit::PayloadWithEncapsulation;
    let key_only = DynamicData::Aggregate(vec![("id".into(), long(7))]);
    assert_eq!(
        zerodds_wire_codec::encode::serialized_key(&ast, &write, &key_only).unwrap(),
        [
            0x00, 0x09, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00
        ]
    );

    // `read_key_only`: the body behind the header, read the same way;
    // the members outside the key come back at their default.
    let mut read = request("A", true, false);
    read.key_holder = KeyHolderExtensibility::AsDeclared;
    read.key_holder_order = KeyHolderOrder::Declaration;
    let data: DynamicData =
        zerodds_wire_codec::decode::serialized_key(&ast, &read, &[4, 0, 0, 0, 7, 0, 0, 0]).unwrap();
    assert_eq!(data.member("id"), Some(&long(7)));
    assert_eq!(data.member("x"), Some(&long(0)));
}
