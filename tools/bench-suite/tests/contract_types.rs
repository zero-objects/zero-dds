// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`: the `DdsType` surface
//! `zerodds-build` generates for `RoundtripBench::Roundtrip` (`build.rs`,
//! included by `src/bin/roundtrip_typed.rs`) declares
//! `const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier =
//! TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([…]))` and
//! answers `type_object()` with
//! `type_object::TypeObject::from_bytes_le(&[…]).ok()` over the complete
//! TypeObject in XCDR2 little-endian. (The `TypeIdentifier::None` of that
//! file is in its `#[cfg(test)]` module.)
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.2 and Annex B (`EK_COMPLETE` = 0xF2, a
//! 14-octet hash), §7.3.4 (the TypeObject is an appendable union opened by
//! a DHEADER). The complete TypeObject of `@final struct Robot { uint32 id;
//! uint32 label; }` below was written by hand from Annex B; its MD5,
//! computed with Python's `hashlib`, gives the complete hash Cyclone DDS
//! 11.0.1 announces for the type
//! (`crates/types/tests/typeobject_serialized_size.rs`).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// As the generated code declares it: a constant.
const TYPE_IDENTIFIER: TypeIdentifier = TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
    0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
]));

#[rustfmt::skip]
const ROBOT_COMPLETE: [u8; 84] = [
    0x50, 0x00, 0x00, 0x00, 0xf2, 0x51, 0x01, 0x00, // DHEADER 80, EK_COMPLETE, TK_STRUCTURE, IS_FINAL
    0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // header: DHEADER 14, TK_NONE, no annotations
    0x06, 0x00, 0x00, 0x00, b'R', b'o', b'b', b'o', b't', 0x00, 0x00, 0x00,
    0x34, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, // member_seq: DHEADER 52, two
    0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x03, 0x00, 0x00, 0x00, b'i', b'd', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x06, 0x00, 0x00, 0x00, b'l', b'a', b'b', b'e', b'l', 0x00, 0x00, 0x00,
];

#[test]
fn xtypes_7_3_4_2_the_generated_identifier_is_the_complete_hash_form() {
    assert_eq!(TYPE_IDENTIFIER.discriminator(), 0xf2);
    assert_ne!(TYPE_IDENTIFIER, TypeIdentifier::None);
    let mut expected = vec![0xf2];
    expected.extend_from_slice(&[
        0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
    ]);
    assert_eq!(TYPE_IDENTIFIER.to_bytes_le().unwrap(), expected);
}

#[test]
fn xtypes_7_3_4_a_complete_type_object_reads_from_its_xcdr2_bytes() {
    match TypeObject::from_bytes_le(&ROBOT_COMPLETE) {
        Ok(TypeObject::Complete(_)) => {}
        other => panic!("expected a complete TypeObject, got {other:?}"),
    }
    // `type_object()` maps a refusal to `None`; a cut stream is refused.
    assert!(TypeObject::from_bytes_le(&ROBOT_COMPLETE[..40]).is_err());
}
