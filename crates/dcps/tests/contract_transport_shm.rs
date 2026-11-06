// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-transport-shm`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-shm`, with the feature
//! `same-host-shm`:
//! - `ShmConfig`, built field by field in `same_host_shm.rs` (`pair_config`:
//!   capacity, flink directory, `max_datagram`, a 250 ms receive timeout)
//!   and taken as `ShmConfig::default()` in `runtime.rs` for the SHM user
//!   transport (`shm_user.rs`).
//! - `PosixShmTransport::open_owner` / `open_consumer`, `peer_attached`,
//!   `peer_locator`, `send_with_prefix`, `try_recv`, `interrupt_recv`,
//!   `set_wake_on_push`, `consumed_bytes`, `occupied_bytes` and the
//!   `Transport` impl (`same_host_channel.rs`, `shm_user.rs`, the receive
//!   loop in `runtime.rs`); `ShmDoorbell::open`, `ring`, `park`, `sequence`;
//!   `PosixShmError` (`InvalidHeader` built in `same_host_channel.rs`).
//! - `posix::sweep_orphaned_segments`, `posix::sweep_orphaned_flinks`,
//!   `posix::sweep_stale_pair_files`, `bell::sweep_orphaned_doorbells` and
//!   `posix::SweepReport` (`same_host_shm.rs`: `removed: 0` means there is
//!   nothing to report, two reports are combined with `and`, an `Err` is
//!   logged as `shm.orphan_sweep_failed`).
//!
//! Oracles: ZeroDDS SHM Transport 1.0 §3 (the ring holds length-prefixed
//! frames; `ShmConfig::capacity` documented as at least twice
//! `max_datagram`, the bound `open_owner` refuses below with
//! `PosixShmError::InvalidConfig` being `2 × max_datagram + 16`),
//! `docs/spec-coverage/zerodds-shm-transport-1.0.md`, and the documented
//! semantics of `SweepReport::and` and of a sweep of a directory that does
//! not exist. No segment and no doorbell is created; the pair, doorbell
//! and receive uses are left to the integration pillar. The sweeps run
//! against a `tempfile` directory and write nothing.
#![allow(clippy::unwrap_used)]

use std::time::Duration;

use zerodds_dcps::same_host_shm::{
    DEFAULT_CAPACITY, DEFAULT_MAX_DATAGRAM, shm_config_for_pair, shm_config_for_pair_with_limit,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_transport::Transport;
use zerodds_transport_shm::posix::SweepReport;
use zerodds_transport_shm::{PosixShmTransport, ShmConfig, ShmDoorbell};

/// The ring bound of `open_owner`: two datagrams and their framing.
fn ring_holds(config: &ShmConfig) -> bool {
    config.capacity >= config.max_datagram * 2 + 16
}

#[test]
fn every_segment_config_the_runtime_builds_passes_the_ring_bound() {
    // The SHM user transport takes the transport's own default.
    let user = ShmConfig::default();
    assert!(ring_holds(&user), "{user:?}");
    assert_eq!(user.max_datagram, 64 * 1024);
    assert_eq!(user.recv_timeout, None, "stopped by `interrupt_recv`");

    // The same-host channel builds its own, per host and domain.
    let prefix = GuidPrefix::zerodds([1, 2, 3, 4], [5, 6, 7, 8, 9, 10]);
    let pair = shm_config_for_pair(prefix, 3);
    assert!(ring_holds(&pair), "{pair:?}");
    assert_eq!(
        (pair.capacity, pair.max_datagram),
        (DEFAULT_CAPACITY, DEFAULT_MAX_DATAGRAM)
    );
    assert_eq!(pair.recv_timeout, Some(Duration::from_millis(250)));
    // `transport.shm_max_datagram` from one octet to 16 MiB.
    for limit in [1, 1472, 64 * 1024, 1024 * 1024, 16 * 1024 * 1024] {
        let config = shm_config_for_pair_with_limit(prefix, 3, limit).unwrap();
        assert_eq!(config.max_datagram, limit);
        assert!(ring_holds(&config), "{config:?}");
    }
}

#[test]
fn sweep_reports_add_up_field_by_field() {
    assert_eq!(
        SweepReport::default(),
        SweepReport {
            removed: 0,
            held: 0,
            young: 0,
            foreign: 0
        }
    );
    let rings = SweepReport {
        removed: 2,
        held: 1,
        young: 0,
        foreign: 3,
    };
    let bells = SweepReport {
        removed: 1,
        held: 0,
        young: 4,
        foreign: 0,
    };
    assert_eq!(
        rings.and(bells),
        SweepReport {
            removed: 3,
            held: 1,
            young: 4,
            foreign: 3
        }
    );
}

#[cfg(unix)]
#[test]
fn a_sweep_of_a_fresh_or_missing_flink_directory_removes_nothing() {
    // The runtime sweeps its flink directory once on the first start, when
    // it may not exist yet; an `Err` there would be logged as a failure.
    let age = Duration::from_secs(60);
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("zerodds-shm").join("01020304").join("d3");
    let none = SweepReport::default();
    assert_eq!(
        zerodds_transport_shm::posix::sweep_stale_pair_files(&missing, age).unwrap(),
        none
    );
    assert_eq!(
        zerodds_transport_shm::posix::sweep_stale_pair_files(root.path(), age).unwrap(),
        none
    );
    #[cfg(not(target_os = "linux"))]
    {
        assert_eq!(
            zerodds_transport_shm::posix::sweep_orphaned_flinks(&missing, age).unwrap(),
            none
        );
        assert_eq!(
            zerodds_transport_shm::posix::sweep_orphaned_flinks(root.path(), age).unwrap(),
            none
        );
    }
    // Nothing was written.
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn the_ring_and_the_doorbell_are_shared_across_threads() {
    // `Arc<PosixShmTransport>` sits behind `dyn Transport + Send + Sync` and
    // in the per-participant receive thread; `Arc<ShmDoorbell>` is rung
    // from the sending thread and parked on in the receiving one.
    fn behind_dyn<T: Transport + Send + Sync + 'static>() {}
    fn shared<T: Send + Sync + 'static>() {}
    behind_dyn::<PosixShmTransport>();
    shared::<ShmDoorbell>();
}
