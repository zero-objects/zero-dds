// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-zenoh-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/runtime.rs`,
//! `src/daemon/server.rs`, features `zenoh-runtime`/`daemon`): the root
//! re-export `zerodds_rtps::EntityId` as the reverse writer's handle behind
//! an `Arc<Mutex<..>>`; `inline_qos::status_info::DISPOSED` for a Zenoh
//! delete; `EntityKind::from_byte(guid[15]).topic_kind()` to tell a keyed
//! endpoint from a keyless one by its GUID (`topic_kind_from_guid`); and
//! `GuidPrefix::from_bytes` of the first 12 GUID octets as the TypeLookup
//! peer. Oracle: DDSI-RTPS 2.5 §9.3.1.2 Table 9.1, §9.3.1.5, §9.6.4.9.

use zerodds_rtps::inline_qos::status_info::DISPOSED;
use zerodds_rtps::wire_types::{EntityKind, GuidPrefix, TopicKind};

#[test]
fn the_root_entity_id_is_a_shareable_handle() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<zerodds_rtps::EntityId>();
}

#[test]
fn rtps_9_6_4_9_a_delete_is_the_disposed_flag() {
    assert_eq!(DISPOSED, 0x1);
}

#[test]
fn rtps_9_3_1_2_the_last_guid_octet_tells_keyed_from_keyless() {
    // Cyclone's keyed writer (0x02) and a keyless reader (0x04): the case
    // the bridge measured, where matching never pairs the two.
    let keyed_writer = [0x01, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0x02];
    let keyless_reader = [0x01, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0x04];
    assert_eq!(
        EntityKind::from_byte(keyed_writer[15]).topic_kind(),
        Some(TopicKind::WithKey)
    );
    assert_eq!(
        EntityKind::from_byte(keyless_reader[15]).topic_kind(),
        Some(TopicKind::NoKey)
    );
    assert_eq!(
        EntityKind::from_byte(0xc1).topic_kind(),
        None,
        "a participant is neither"
    );
}

#[test]
fn rtps_9_3_1_5_the_peer_prefix_is_the_first_twelve_octets() {
    let guid = [0x01, 0x0f, 9, 9, 9, 9, 1, 2, 3, 4, 5, 6, 0, 0, 1, 0x02];
    let mut prefix = [0_u8; 12];
    prefix.copy_from_slice(&guid[..12]);
    assert_eq!(GuidPrefix::from_bytes(prefix).to_bytes(), guid[..12]);
}
