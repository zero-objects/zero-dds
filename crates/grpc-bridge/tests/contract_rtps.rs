// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-grpc-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/bin/zerodds-grpc-bridged.rs`,
//! feature `std`): `wire_types::GuidPrefix::zerodds(host_id, unique)` for the
//! prefix of its participant, and `wire_types::EntityId` as the handle of each
//! per-topic writer and reader. Oracle: DDSI-RTPS 2.5 §9.3.1.5 (the first two
//! octets of a prefix are the vendor id) and Table 9.1.

use zerodds_rtps::wire_types::{EntityId, GuidPrefix, VendorId};

#[test]
fn rtps_9_3_1_5_the_participant_prefix_starts_with_the_vendor_id() {
    let prefix = GuidPrefix::zerodds([1, 2, 3, 4], [5, 6, 7, 8, 9, 10]);
    assert_eq!(prefix.0[..2], VendorId::ZERODDS.to_bytes());
    assert_eq!(prefix.host_id(), [1, 2, 3, 4]);
    assert_eq!(prefix.0[6..], [5, 6, 7, 8, 9, 10]);
}

#[test]
fn rtps_9_3_1_2_writer_and_reader_handles_are_plain_distinct_values() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<EntityId>();
    let key = [0, 0, 3];
    assert_ne!(
        EntityId::user_writer_with_key(key),
        EntityId::user_reader_with_key(key)
    );
}
