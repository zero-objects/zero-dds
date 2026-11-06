// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-record` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-wire-codec` (`src/main.rs`,
//! `DecodeSinks::consume`, the `--decode` path): `ProbeRequest::in_dialect`
//! with the sample's type name, the XCDR version and framing chosen from the
//! type's extensibility, the byte order the encapsulation stated and the
//! dialect of `--dialect`; then `decode::<DynamicData>` on the payload behind
//! the encapsulation header. An `Err` drops the typed record of that sample
//! with a warning; the opaque capture already holds it. The library of this
//! tool parses arguments only, so the contract calls `zerodds-wire-codec`
//! directly with what `main.rs` passes.
//!
//! The type is built the way a peer's type reaches a reader: CycloneDDS
//! 11.0.1's COMPLETE TypeObjects of `module to { @final struct Plain {...};
//! @appendable struct Appendable {...}; @mutable struct Mutable {...}; }`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`), turned into a model
//! with `zerodds_types::dynamic::to_ast::dynamic_type`.
//!
//! Oracle for the sample octets, written out by hand: DDS-XTypes 1.3
//! §7.4.1.1.1 (PLAIN_CDR, alignment to the size, byte order of the stream),
//! §7.4.2 (PLAIN_CDR2 for FINAL, DELIMITED_CDR = DHEADER + PLAIN_CDR2 for
//! APPENDABLE, PL_CDR2 = DHEADER + EMHEADER per member for MUTABLE),
//! §7.4.3.4.2 (EMHEADER: M flag bit 31, length code in bits 28..30, LC 1 = two
//! octets, LC 2 = four, member id in bits 0..27).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast_layout::Dialect;
use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_types::dynamic::DynamicData;
use zerodds_types::dynamic::to_ast::dynamic_type;
use zerodds_types::dynamic::type_::DynamicType;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::TypeObject;
use zerodds_wire_codec::{ProbeRequest, decode};

const PLAIN: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const APPENDABLE: &str = "50000000f251020017000000000000000f000000746f3a3a417070656e6461626c6500002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const MUTABLE: &str = "4c000000f251040014000000000000000c000000746f3a3a4d757461626c65002c00000002000000100000000a0000000100040002000000610000001000000014000000010003000200000062000000";

fn peer_type(complete_hex: &str) -> DynamicType {
    let bytes: Vec<u8> = (0..complete_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&complete_hex[i..i + 2], 16).unwrap())
        .collect();
    let TypeObject::Complete(complete) = TypeObject::from_bytes_le(&bytes).unwrap() else {
        panic!("a COMPLETE object");
    };
    dynamic_type(&complete, &TypeRegistry::new()).unwrap()
}

/// `a = 7` (long), `b = -2` (short), whatever the framing.
fn is_seven_and_minus_two(data: &DynamicData) -> bool {
    matches!(
        data.member("a"),
        Some(DynamicData::Integer { value: 7, .. })
    ) && matches!(
        data.member("b"),
        Some(DynamicData::Integer { value: -2, .. })
    )
}

fn read(
    ty: &DynamicType,
    encoding: Encoding,
    endianness: Endianness,
    payload: &[u8],
) -> Result<DynamicData, zerodds_wire_codec::ProbeError> {
    let request = ProbeRequest::in_dialect(
        &ty.qualified_name(),
        encoding,
        endianness,
        Dialect::default(),
    );
    decode::<DynamicData>(ty.ast(), &request, payload)
}

#[test]
fn xtypes_7_4_2_a_final_sample_under_xcdr2_little_endian() {
    let ty = peer_type(PLAIN);
    let data = read(
        &ty,
        Encoding::PlainCdr2,
        Endianness::Little,
        &[0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
    )
    .unwrap();
    assert!(is_seven_and_minus_two(&data), "{data:?}");
}

#[test]
fn xtypes_7_4_1_1_1_a_final_sample_under_xcdr1_big_endian() {
    // The byte order the encapsulation states (CDR_BE) reaches the reader.
    let ty = peer_type(PLAIN);
    let data = read(
        &ty,
        Encoding::PlainCdr1,
        Endianness::Big,
        &[0x00, 0x00, 0x00, 0x07, 0xff, 0xfe],
    )
    .unwrap();
    assert!(is_seven_and_minus_two(&data), "{data:?}");
}

#[test]
fn xtypes_7_4_2_an_appendable_sample_is_a_dheader_then_the_members() {
    let ty = peer_type(APPENDABLE);
    let data = read(
        &ty,
        Encoding::DelimitedCdr,
        Endianness::Little,
        &[
            0x06, 0x00, 0x00, 0x00, // DHEADER: 6
            0x07, 0x00, 0x00, 0x00, 0xfe, 0xff,
        ],
    )
    .unwrap();
    assert!(is_seven_and_minus_two(&data), "{data:?}");
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_sample_is_found_by_member_id() {
    let ty = peer_type(MUTABLE);
    let data = read(
        &ty,
        Encoding::PlCdr2,
        Endianness::Little,
        &[
            0x0e, 0x00, 0x00, 0x00, // DHEADER: 14
            0x0a, 0x00, 0x00, 0x20, // EMHEADER: LC 2, id 10 (`a`)
            0x07, 0x00, 0x00, 0x00, //
            0x14, 0x00, 0x00, 0x10, // EMHEADER: LC 1, id 20 (`b`)
            0xfe, 0xff,
        ],
    )
    .unwrap();
    assert!(is_seven_and_minus_two(&data), "{data:?}");
}

#[test]
fn a_payload_that_does_not_hold_the_sample_is_an_error_not_a_value() {
    let ty = peer_type(PLAIN);
    // Cut inside `b`.
    assert!(
        read(
            &ty,
            Encoding::PlainCdr2,
            Endianness::Little,
            &[0x07, 0x00, 0x00, 0x00, 0xfe]
        )
        .is_err()
    );
    // A DHEADER that announces more than the payload holds.
    let appendable = peer_type(APPENDABLE);
    assert!(
        read(
            &appendable,
            Encoding::DelimitedCdr,
            Endianness::Little,
            &[0x40, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xfe, 0xff],
        )
        .is_err()
    );
    // A type name the model does not hold.
    let request = ProbeRequest::in_dialect(
        "to::Absent",
        Encoding::PlainCdr2,
        Endianness::Little,
        Dialect::default(),
    );
    assert!(decode::<DynamicData>(ty.ast(), &request, &[0; 6]).is_err());
}
