// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-amqp-endpoint` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/bridge_daemon.rs`, feature
//! `std`): `wire_types::EntityId` as the writer handle;
//! `history_cache::ChangeKind` of a lifecycle sample, `NotAliveUnregistered`
//! becoming an AMQP unregister and every other kind a dispose;
//! `inline_qos::status_info::{DISPOSED, UNREGISTERED}`, the bits it writes
//! for an AMQP dispose or unregister; and
//! `publication_data::data_representation::{XCDR, XCDR2}`, chosen from the
//! encapsulation of an AMQP body. Oracle: DDSI-RTPS 2.5 §9.6.4.9 (StatusInfo
//! flags D = bit 0, U = bit 1, F = bit 2), §8.2.1.2 (ChangeKind), DDS-XTypes
//! 1.3 §7.6.3.1.1 (XCDR = 0, XCDR2 = 2).

use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::inline_qos::status_info::{DISPOSED, FILTERED, UNREGISTERED};
use zerodds_rtps::publication_data::data_representation as dr;
use zerodds_rtps::wire_types::EntityId;

#[test]
fn rtps_9_6_4_9_status_info_disposed_and_unregistered_bits() {
    assert_eq!(DISPOSED, 0x1, "D");
    assert_eq!(UNREGISTERED, 0x2, "U");
    assert_eq!(FILTERED, 0x4, "F");
    assert_eq!(DISPOSED & UNREGISTERED, 0, "separate flags");
}

#[test]
fn rtps_8_2_1_2_the_not_alive_kinds_a_lifecycle_sample_carries() {
    // Exhaustive on purpose: a new kind makes this contract fail to
    // compile, where the bridge would silently map it to a dispose.
    let lifecycle = |kind: ChangeKind| match kind {
        ChangeKind::Alive | ChangeKind::AliveFiltered => false,
        ChangeKind::NotAliveDisposed
        | ChangeKind::NotAliveUnregistered
        | ChangeKind::NotAliveDisposedUnregistered => true,
    };
    assert!(lifecycle(ChangeKind::NotAliveUnregistered));
    assert!(lifecycle(ChangeKind::NotAliveDisposed));
    assert!(lifecycle(ChangeKind::NotAliveDisposedUnregistered));
    assert_ne!(
        ChangeKind::NotAliveUnregistered,
        ChangeKind::NotAliveDisposedUnregistered
    );
}

#[test]
fn xtypes_7_6_3_1_1_xcdr_and_xcdr2_representation_ids() {
    assert_eq!(dr::XCDR, 0);
    assert_eq!(dr::XCDR2, 2);
}

#[test]
fn entity_id_is_a_plain_handle() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<EntityId>();
}
