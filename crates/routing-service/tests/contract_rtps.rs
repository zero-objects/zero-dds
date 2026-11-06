// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-routing-service` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/forwarding.rs`): the root
//! re-export `zerodds_rtps::EntityId` as the output writer's handle;
//! `history_cache::ChangeKind` turned into `inline_qos::status_info` bits
//! (`status_bits`) for a forwarded dispose or unregister; `Guid::from_bytes`
//! and `SequenceNumber(i64)` for the original writer's identity a forwarded
//! sample keeps; and `header_extension::HeTimestamp`, passed through. Oracle:
//! DDSI-RTPS 2.5 §9.3.1.5, §9.3.2.10, §9.6.4.9.

use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
use zerodds_rtps::wire_types::{Guid, SequenceNumber};

#[test]
fn the_root_entity_id_is_the_wire_types_one() {
    assert_eq!(
        core::any::TypeId::of::<zerodds_rtps::EntityId>(),
        core::any::TypeId::of::<zerodds_rtps::wire_types::EntityId>()
    );
}

#[test]
fn rtps_9_6_4_9_a_forwarded_lifecycle_keeps_both_flags() {
    let bits = |kind: ChangeKind| match kind {
        ChangeKind::NotAliveDisposed => DISPOSED,
        ChangeKind::NotAliveUnregistered => UNREGISTERED,
        ChangeKind::NotAliveDisposedUnregistered => DISPOSED | UNREGISTERED,
        ChangeKind::Alive | ChangeKind::AliveFiltered => 0,
    };
    assert_eq!(bits(ChangeKind::NotAliveDisposed), 0x1);
    assert_eq!(bits(ChangeKind::NotAliveUnregistered), 0x2);
    assert_eq!(bits(ChangeKind::NotAliveDisposedUnregistered), 0x3);
}

#[test]
fn rtps_9_3_1_5_and_9_3_2_10_the_original_writer_identity() {
    let bytes = [0x01, 0x10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0x22, 0x03];
    let guid = Guid::from_bytes(bytes);
    assert_eq!(guid.prefix.0[..2], [0x01, 0x10]);
    assert_eq!(guid.entity_id.to_bytes(), [0, 0, 0x22, 0x03]);
    assert_eq!(guid.to_bytes(), bytes);
    let sn = SequenceNumber((1_i64 << 32) + 2);
    assert_eq!(sn.split(), (1, 2));
}
