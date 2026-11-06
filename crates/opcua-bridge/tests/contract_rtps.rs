// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/dds_bridge.rs`,
//! `src/daemon/server.rs`, feature `daemon`): `wire_types::EntityId` as the
//! handle of each registered writer and reader, and
//! `header_extension::HeTimestamp`, the source timestamp of a sample, which
//! it turns into an OPC UA `DateTime` from its `seconds` and `fraction`.
//! Oracle: DDSI-RTPS 2.5 §9.3.2.1 (`Time_t`: seconds, fraction in sec/2^32;
//! `TIME_INVALID`, `TIME_INFINITE`) and Table 9.1.

use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::wire_types::EntityId;

#[test]
fn rtps_9_3_2_1_he_timestamp_is_seconds_and_a_binary_fraction() {
    // Half a second is 2^31 in units of 2^-32 s, not 500 000 000.
    let half = HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 1 << 31,
    };
    let as_seconds = f64::from(half.seconds) + f64::from(half.fraction) / 4_294_967_296.0;
    assert!((as_seconds - 1_700_000_000.5).abs() < 1e-6);
    assert_eq!(
        HeTimestamp::default(),
        HeTimestamp {
            seconds: 0,
            fraction: 0
        },
        "TIME_ZERO"
    );
}

#[test]
fn rtps_9_3_2_1_time_invalid_and_time_infinite() {
    assert_eq!(
        (
            HeTimestamp::TIME_INVALID.seconds,
            HeTimestamp::TIME_INVALID.fraction
        ),
        (0xffff_ffff, 0xffff_ffff)
    );
    assert_eq!(
        (
            HeTimestamp::TIME_INFINITE.seconds,
            HeTimestamp::TIME_INFINITE.fraction
        ),
        (0xffff_ffff, 0xffff_fffe)
    );
}

#[test]
fn entity_id_is_a_plain_handle() {
    fn handle<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    handle::<EntityId>();
    handle::<HeTimestamp>();
    let key = [0, 2, 9];
    assert_ne!(
        EntityId::user_writer_with_key(key),
        EntityId::user_reader_with_key(key)
    );
}
