// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-durability-service` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`, `user_writer_cfg`):
//! `TypeIdentifier::None` as the `type_identifier` of the replay writer, so
//! that the writer announces no type identity and is matched by topic and
//! type name. Oracle: DDS-XTypes 1.3 §7.3.4.2 Table 29 (`TK_NONE`: "Invalid
//! identifier") and Annex B (`const octet TK_NONE = 0x00`, `TypeIdentifier`
//! a union discriminated by that octet, serialized with XCDR2 little endian).
#![allow(clippy::unwrap_used)]

use zerodds_types::type_identifier::kinds::TK_NONE;
use zerodds_types::{EquivalenceHash, TypeIdentifier};

#[test]
fn xtypes_7_3_4_2_the_replay_writer_identifier_is_tk_none() {
    let none = TypeIdentifier::None;
    assert_eq!(TK_NONE, 0x00, "Annex B: TK_NONE = 0x00");
    assert_eq!(none.discriminator(), 0x00);
    // The union carries no member for TK_NONE: the discriminator octet is
    // the whole identifier.
    assert_eq!(none.to_bytes_le().unwrap(), [0x00]);
    assert_eq!(TypeIdentifier::from_bytes_le(&[0x00]).unwrap(), none);
    // It names no type, so it is not any hashed identity.
    assert_ne!(
        none,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash::ZERO)
    );
    assert_ne!(
        none,
        TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash::ZERO)
    );
}
