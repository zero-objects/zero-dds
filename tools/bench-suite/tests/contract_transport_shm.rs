// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-transport-shm`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-shm`: no module of `src/`
//! names it; `benches/transports_e2e.rs` (POSIX only) does, as the
//! `zerodds-rtps` contract of this crate counts the benches. Per payload
//! size it builds `ShmConfig { capacity: (max_datagram * 4).max(4096),
//! flink_dir, max_datagram, recv_timeout }`, clones it, opens
//! `PosixShmTransport::open_owner(owner, consumer, cfg)` and
//! `open_consumer(consumer, owner, cfg)` and sends and receives through
//! `Transport`.
//!
//! Opening maps shared memory, which a contract target does not. What is
//! checked: the configuration has exactly these four fields, the two
//! constructors have the signatures the bench calls, and the bench's
//! capacity rule meets the precondition `open` checks before it touches the
//! file system (`capacity >= 2 * max_datagram + 16`, refused as
//! `PosixShmError::InvalidConfig`, documented on `ShmConfig::capacity`).
#![cfg(unix)]
#![allow(clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use zerodds_bench_suite::PAYLOAD_SIZES;
use zerodds_transport::Transport;
use zerodds_transport_shm::{PosixShmError, PosixShmTransport, ShmConfig};

fn bench_config(max_datagram: usize) -> ShmConfig {
    ShmConfig {
        capacity: (max_datagram * 4).max(4096),
        flink_dir: PathBuf::from("/nonexistent/zerodds-contract"),
        max_datagram,
        recv_timeout: Some(Duration::from_millis(50)),
    }
}

#[test]
fn the_configuration_and_the_constructors_the_bench_uses() {
    let config = bench_config(1024);
    let copy = config.clone();
    assert_eq!(copy.capacity, 4096);
    assert_eq!(copy.max_datagram, 1024);
    let _: fn([u8; 16], [u8; 16], ShmConfig) -> Result<PosixShmTransport, PosixShmError> =
        PosixShmTransport::open_owner;
    let _: fn([u8; 16], [u8; 16], ShmConfig) -> Result<PosixShmTransport, PosixShmError> =
        PosixShmTransport::open_consumer;
    fn transport<T: Transport>() {}
    transport::<PosixShmTransport>();
}

#[test]
fn the_benchs_capacity_rule_meets_the_ring_precondition_at_every_size() {
    for &size in PAYLOAD_SIZES {
        let config = bench_config(size);
        assert!(
            config.capacity >= 2 * config.max_datagram + 16,
            "{size}: capacity {}",
            config.capacity
        );
    }
}

#[test]
fn a_capacity_below_the_precondition_is_refused_before_any_file_is_touched() {
    let config = ShmConfig {
        capacity: 100,
        ..bench_config(64)
    };
    match PosixShmTransport::open_owner([1; 16], [2; 16], config) {
        Err(PosixShmError::InvalidConfig { .. }) => {}
        Err(other) => panic!("expected InvalidConfig, got {other:?}"),
        Ok(_) => panic!("a ring smaller than two datagrams was opened"),
    }
}
