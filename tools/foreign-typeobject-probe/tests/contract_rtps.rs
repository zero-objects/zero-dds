// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-foreign-typeobject-probe` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`):
//! `wire_types::GuidPrefix::zerodds` for its participant, the prefix of a
//! discovered writer's participant key as the TypeLookup peer, and
//! `EntityKind::UserWriterNoKey` to tell a keyless writer from a keyed one
//! (`entity_id.entity_kind != EntityKind::UserWriterNoKey`). Oracle:
//! DDSI-RTPS 2.5 §9.3.1.2 Table 9.1, §9.3.1.5.

use zerodds_rtps::wire_types::{EntityId, EntityKind, GuidPrefix, VendorId};

#[test]
fn rtps_9_3_1_2_the_keyless_writer_kind_is_0x03_and_the_keyed_one_0x02() {
    assert_eq!(EntityKind::UserWriterNoKey.to_byte(), 0x03);
    assert_eq!(EntityKind::UserWriterWithKey.to_byte(), 0x02);
    assert_eq!(EntityKind::from_byte(0x03), EntityKind::UserWriterNoKey);
    // What the probe reads from a foreign writer's GUID.
    let foreign = EntityId::from_bytes([0x00, 0x00, 0x12, 0x02]);
    assert_ne!(foreign.entity_kind, EntityKind::UserWriterNoKey, "keyed");
    let keyless = EntityId::from_bytes([0x00, 0x00, 0x13, 0x03]);
    assert_eq!(keyless.entity_kind, EntityKind::UserWriterNoKey);
}

#[test]
fn rtps_9_3_1_5_the_probe_prefix_starts_with_the_vendor_id() {
    let prefix = GuidPrefix::zerodds([1, 2, 3, 4], [0; 6]);
    assert_eq!(prefix.0[..2], VendorId::ZERODDS.to_bytes());
}
