// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-record` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`):
//! `wire_types::EntityId` as the handle of each reader it creates, and
//! `history_cache::ChangeKind` of a lifecycle sample: `NotAliveDisposed` is
//! recorded as disposed, every other kind as unregistered. Oracle:
//! DDSI-RTPS 2.5 §8.2.1.2 (ChangeKind_t) and §9.6.4.9 (a marker can carry
//! D and U together).

use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::wire_types::EntityId;

#[test]
fn rtps_8_2_1_2_the_not_alive_kinds_a_lifecycle_sample_carries() {
    // Exhaustive on purpose: a new kind makes this contract fail to
    // compile, where the recorder would file it as unregistered.
    let disposed = |kind: ChangeKind| match kind {
        ChangeKind::NotAliveDisposed | ChangeKind::NotAliveDisposedUnregistered => Some(true),
        ChangeKind::NotAliveUnregistered => Some(false),
        ChangeKind::Alive | ChangeKind::AliveFiltered => None,
    };
    assert_eq!(disposed(ChangeKind::NotAliveDisposed), Some(true));
    assert_eq!(disposed(ChangeKind::NotAliveUnregistered), Some(false));
    // D and U together exist as a kind of their own (§9.6.4.9).
    assert_eq!(
        disposed(ChangeKind::NotAliveDisposedUnregistered),
        Some(true)
    );
    assert_ne!(
        ChangeKind::NotAliveDisposed,
        ChangeKind::NotAliveDisposedUnregistered
    );
}

#[test]
fn entity_id_is_a_plain_handle() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<EntityId>();
}
