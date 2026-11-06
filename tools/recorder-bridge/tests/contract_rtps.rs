// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-recorder-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`, `lifecycle_kinds`):
//! `history_cache::ChangeKind` of a lifecycle sample. `NotAliveDisposed` is
//! recorded as disposed, `NotAliveUnregistered` as unregistered, and
//! `NotAliveDisposedUnregistered` as both, the dispose first: `.zddsrec`
//! has one kind per entry. Oracle: DDSI-RTPS 2.5 §8.2.1.2 (ChangeKind_t)
//! and §9.6.4.9 (a marker can carry D and U together).

use zerodds_rtps::history_cache::ChangeKind;

#[test]
fn rtps_8_2_1_2_the_not_alive_kinds_a_lifecycle_sample_carries() {
    // Exhaustive on purpose: a new kind makes this contract fail to
    // compile, where the bridge would record nothing for it.
    let entries = |kind: ChangeKind| match kind {
        ChangeKind::NotAliveDisposed | ChangeKind::NotAliveUnregistered => 1,
        ChangeKind::NotAliveDisposedUnregistered => 2,
        ChangeKind::Alive | ChangeKind::AliveFiltered => 0,
    };
    assert_eq!(entries(ChangeKind::NotAliveDisposedUnregistered), 2);
    // D and U together are a kind of their own (§9.6.4.9).
    assert_ne!(
        ChangeKind::NotAliveDisposed,
        ChangeKind::NotAliveDisposedUnregistered
    );
    assert_ne!(
        ChangeKind::NotAliveUnregistered,
        ChangeKind::NotAliveDisposedUnregistered
    );
}
