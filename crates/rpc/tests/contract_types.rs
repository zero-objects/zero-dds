// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`:
//! - `type_lookup::sequence_number_parts` and `sequence_number_from_parts`
//!   (`src/common_types.rs`): the `SampleIdentity` of a request or reply
//!   header goes on the wire as the two words of `SequenceNumber_t`.
//! - `hash::hash_bytes` (`src/rpc_hash.rs`): its first four octets, read
//!   little endian, are the operation and parameter hash.
//! - `TypeObject::from_bytes_le` (`src/endpoint_options.rs`,
//!   `TopicTypeBytes::decode`): the generator's `CompleteTypeObject` octets of
//!   a service's topic types, an error mapped to `RpcError::Dcps`.
//! - `TypeIdentifier::None` and `qos::TypeConsistencyEnforcement::default()`
//!   (`src/endpoint.rs`): what a request/reply endpoint announces when it has
//!   no TypeObject of its own.
//!
//! Oracle: DDS-XTypes 1.3 §7.6.3.3.2 (`struct SequenceNumber_t { long high;
//! unsigned long low; }`, copied from DDS-RPC 1.0), DDS-RPC 1.0 §7.5.1.1.2
//! (the hash is MD5 of the name, octets 0 to 3 summed little endian), RFC 1321
//! Appendix A.5 (MD5 test suite), DDS-XTypes 1.3 §7.3.4.5 and Annex B (the
//! TypeObject octets; CycloneDDS 11.0.1's COMPLETE object of
//! `module to { @final struct Plain { long a; short b; }; }` from
//! `crates/types/tests/typeobject_vendor_dheader.rs`), §7.3.4.2 Table 29
//! (`TK_NONE = 0x00`) and §7.6.3.4.1 (the defaults of the
//! TypeConsistencyEnforcement policy).
#![allow(clippy::unwrap_used)]

use zerodds_types::hash::hash_bytes;
use zerodds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use zerodds_types::type_lookup::{sequence_number_from_parts, sequence_number_parts};
use zerodds_types::{CompleteTypeObject, TypeIdentifier, TypeObject};

#[test]
fn xtypes_7_6_3_3_2_a_sequence_number_is_high_then_low() {
    // value = high * 2^32 + low.
    assert_eq!(sequence_number_parts(0x0000_0001_0000_0002), (1, 2));
    assert_eq!(sequence_number_parts(7), (0, 7));
    assert_eq!(sequence_number_from_parts(1, 2), 0x0000_0001_0000_0002);
    assert_eq!(sequence_number_from_parts(0, 7), 7);
    // SEQUENCENUMBER_UNKNOWN = { -1, 0 }: high is the long -1, whose two's
    // complement word is 0xFFFFFFFF.
    assert_eq!(
        sequence_number_parts(0xFFFF_FFFF_0000_0000),
        ((-1i32).cast_unsigned(), 0)
    );
    assert_eq!(
        sequence_number_from_parts((-1i32).cast_unsigned(), 0),
        0xFFFF_FFFF_0000_0000
    );
    // The two are inverse over the whole range.
    for value in [0, 1, u64::from(u32::MAX), 1 << 32, u64::MAX] {
        let (high, low) = sequence_number_parts(value);
        assert_eq!(sequence_number_from_parts(high, low), value);
    }
}

#[test]
fn rpc_7_5_1_1_2_the_hash_is_the_first_four_md5_octets_little_endian() {
    // RFC 1321 A.5: MD5("") = d41d8cd98f00b204e9800998ecf8427e,
    // MD5("abc") = 900150983cd24fb0d6963f7d28e17f72.
    assert_eq!(
        hash_bytes(b"").0,
        [
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8
        ]
    );
    assert_eq!(
        hash_bytes(b"abc").0,
        [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1
        ]
    );
    // What `rpc_member_hash` builds from it: md5[0] + 256·md5[1] +
    // 256²·md5[2] + 256³·md5[3]. MD5("getName") opens 32 13 2c 89,
    // MD5("add") opens 34 ec 78 fc.
    let first_four = |name: &str| {
        let h = hash_bytes(name.as_bytes()).0;
        u32::from_le_bytes([h[0], h[1], h[2], h[3]])
    };
    assert_eq!(first_four("getName"), 0x892c_1332);
    assert_eq!(first_four("add"), 0xfc78_ec34);
}

#[test]
fn xtypes_7_3_4_5_generated_type_object_octets_decode_and_bad_octets_are_refused() {
    const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
    let bytes: Vec<u8> = (0..PLAIN_COMPLETE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&PLAIN_COMPLETE[i..i + 2], 16).unwrap())
        .collect();
    let object = TypeObject::from_bytes_le(&bytes).unwrap();
    assert!(
        matches!(&object, TypeObject::Complete(CompleteTypeObject::Struct(s))
            if s.header.detail.type_name == "to::Plain"),
        "{object:?}"
    );
    // The octets the generator emits are the ones `to_bytes_le` writes.
    assert_eq!(object.to_bytes_le().unwrap(), bytes);
    // A blob cut short is an error, not a partial object.
    assert!(TypeObject::from_bytes_le(&bytes[..bytes.len() - 1]).is_err());
    assert!(TypeObject::from_bytes_le(&[]).is_err());
}

#[test]
fn xtypes_7_3_4_2_an_endpoint_without_a_type_object_announces_tk_none() {
    assert_eq!(TypeIdentifier::None.discriminator(), 0x00);
    assert_eq!(TypeIdentifier::None.to_bytes_le().unwrap(), [0x00]);
}

#[test]
fn xtypes_7_6_3_4_1_the_default_type_consistency_enforcement() {
    let tce = TypeConsistencyEnforcement::default();
    assert_eq!(tce.kind, TypeConsistencyKind::AllowTypeCoercion);
    assert!(tce.ignore_sequence_bounds, "TRUE (the default)");
    assert!(tce.ignore_string_bounds, "TRUE (the default)");
    assert!(!tce.ignore_member_names, "FALSE (the default)");
    assert!(!tce.prevent_type_widening, "FALSE (the default)");
    assert!(!tce.force_type_validation, "the default value is false");
}
