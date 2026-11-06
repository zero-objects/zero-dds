// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: the codec API of its `codec`
//! module (`DdsSerializer::new` and `::key_holder`, `DdsDeserializer::new`
//! and `::key_holder`, the `PlanSet` the model fills, the trait impls for
//! the primitives, `String`, `NonZero*`, `Vec`, arrays, `Option`,
//! `Duration` and, with the `std` feature, `SystemTime`); `Writer::appending`,
//! `Writer::continuing`, `Writer::with_max_align`, `Writer::check_deferred`,
//! `Writer::finish`, `Writer::finish_checked`; `Reader::with_max_align` and
//! `Reader::skip_to`; `md5`; `WireError::OutOfRange` and `::Discarded`;
//! `WireRepr`, `IdlOut::display`, `IdlOut::value` and
//! `dds_member_via_wire_repr!` for `derive(WireRepr)`.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1 (alignment from the start of the body,
//! capped at 8 under XCDR1 and 4 under XCDR2), DDS-RPC 1.0 §7.5.1.1 (a
//! body behind the 28-octet `RequestHeader`), RFC 1321 Appendix A.5 (the
//! MD5 test suite).

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_codec::{DdsType, Encapsulation};
use zerodds_dcps::DdsType as _;

#[derive(DdsType, Debug, PartialEq)]
#[dds(name = "w::Wide", extensibility = "final")]
struct Wide {
    a: u8,
    b: i64,
}

#[test]
fn xtypes_7_4_1_the_alignment_cap_is_the_encodings() {
    let value = Wide { a: 1, b: -2 };
    let mut xcdr2 = Vec::new();
    zerodds_codec::encode(&value, Encapsulation::Xcdr2Le, &mut xcdr2).unwrap();
    assert_eq!(xcdr2.len(), 4 + 8, "XCDR2 aligns an int64 to 4");
    let mut xcdr1 = Vec::new();
    zerodds_codec::encode(&value, Encapsulation::Xcdr1Le, &mut xcdr1).unwrap();
    assert_eq!(xcdr1.len(), 8 + 8, "XCDR1 aligns an int64 to 8");
    assert_eq!(&xcdr1[8..], (-2i64).to_le_bytes());
    let mut big = Vec::new();
    zerodds_codec::encode(&value, Encapsulation::Xcdr2Be, &mut big).unwrap();
    assert_eq!(&big[4..], (-2i64).to_be_bytes());
}

#[test]
fn dds_rpc_7_5_1_1_a_body_behind_a_header_aligns_from_the_start_of_the_sample() {
    let value = Wide { a: 1, b: 9 };
    let mut sample = vec![0xaa; 28];
    value.encode_continuing(&mut sample, true).unwrap();
    assert_eq!(&sample[..28], [0xaa; 28], "the prefix is kept");
    assert_eq!(sample[28], 1);
    assert_eq!(
        sample.len(),
        40,
        "the int64 at 32, a multiple of 8 from the sample's start"
    );
    assert_eq!(&sample[32..], 9i64.to_le_bytes());
    assert_eq!(Wide::decode_continuing(&sample, 28, true).unwrap(), value);
}

#[test]
fn rfc_1321_a_5_the_key_hash_digest() {
    assert_eq!(
        zerodds_wire::md5(b"abc"),
        [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
            0x7f, 0x72
        ]
    );
}

#[test]
fn xtypes_7_4_a_truncated_body_is_refused_with_its_kind() {
    let value = Wide { a: 1, b: 9 };
    let mut bytes = Vec::new();
    zerodds_codec::encode(&value, Encapsulation::Xcdr2Le, &mut bytes).unwrap();
    bytes.truncate(10);
    match zerodds_codec::decode::<Wide>(&bytes, Encapsulation::Xcdr2Le) {
        Err(zerodds_codec::CodecError::Wire(zerodds_wire::WireError::Truncated { .. })) => {}
        other => panic!("a truncated stream, got {other:?}"),
    }
}
