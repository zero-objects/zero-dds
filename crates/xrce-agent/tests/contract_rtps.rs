// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xrce-agent` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier"). Built with the
//! `daemon` feature (`required-features` in `Cargo.toml`): only the daemon
//! links `zerodds-rtps`.
//!
//! What the daemon uses of `zerodds-rtps` (`src/daemon/dcps_bridge.rs`):
//! `wire_types::EntityId` as the handle of each writer, reader and group;
//! `EntityKind::UserWriterGroup`/`UserReaderGroup` for a publisher and a
//! subscriber; `header_extension::HeTimestamp` both ways between an XRCE
//! source time (seconds, nanoseconds) and the RTPS `Time_t`;
//! `inline_qos::status_info::{DISPOSED, UNREGISTERED}` for a dispose or
//! unregister it writes; `history_cache::ChangeKind` of a lifecycle sample it
//! hands to the client as `SampleInfoFlags`. Oracle: DDSI-RTPS 2.5 §9.3.1.2
//! Table 9.1, §9.3.2.1 (`Time_t` fraction in sec/2^32), §9.6.4.9, §8.2.1.2.

use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
use zerodds_rtps::wire_types::{EntityId, EntityKind};

#[test]
fn rtps_9_3_1_2_group_kinds_are_0x08_and_0x09() {
    assert_eq!(EntityKind::UserWriterGroup.to_byte(), 0x08);
    assert_eq!(EntityKind::UserReaderGroup.to_byte(), 0x09);
    assert_eq!(EntityKind::from_byte(0x08), EntityKind::UserWriterGroup);
    let group = EntityId {
        entity_key: [0, 0, 5],
        entity_kind: EntityKind::UserWriterGroup,
    };
    assert_eq!(group.to_bytes(), [0, 0, 5, 0x08]);
}

#[test]
fn rtps_9_3_2_1_time_t_zero_invalid_and_infinite() {
    // The daemon converts with `zerodds_dcps::time` in both directions,
    // which keeps the two reserved values apart from points in time.
    assert_eq!(
        HeTimestamp::default(),
        HeTimestamp {
            seconds: 0,
            fraction: 0
        }
    );
    assert_eq!(
        HeTimestamp::TIME_INVALID,
        HeTimestamp {
            seconds: u32::MAX,
            fraction: u32::MAX
        }
    );
    assert_eq!(
        HeTimestamp::TIME_INFINITE,
        HeTimestamp {
            seconds: u32::MAX,
            fraction: u32::MAX - 1
        }
    );
}

#[test]
fn rtps_9_6_4_9_dispose_and_unregister_bits() {
    assert_eq!(
        (DISPOSED, UNREGISTERED, DISPOSED | UNREGISTERED),
        (0x1, 0x2, 0x3)
    );
}

#[test]
fn rtps_8_2_1_2_the_lifecycle_kinds_the_client_sees() {
    // The daemon maps the three NOT_ALIVE kinds to instance-state bits and
    // skips the rest; exhaustive so a new kind is not skipped unseen.
    let state = |kind: ChangeKind| match kind {
        ChangeKind::NotAliveDisposed => Some(0b10),
        ChangeKind::NotAliveUnregistered => Some(0b01),
        ChangeKind::NotAliveDisposedUnregistered => Some(0b11),
        ChangeKind::Alive | ChangeKind::AliveFiltered => None,
    };
    assert_eq!(state(ChangeKind::NotAliveDisposedUnregistered), Some(0b11));
}
