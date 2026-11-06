// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-isolation-smoke` with `zerodds-transport-shm`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-transport-shm` (`src/main.rs`,
//! `run_shm`, Unix only): `ShmConfig { capacity: 1 << 16, flink_dir,
//! max_datagram: 4096, recv_timeout: Some(10 s) }`, cloned for each retry of
//! `PosixShmTransport::open_consumer(local, peer, cfg)` on the receiving
//! side and handed to `open_owner(local, peer, cfg)` on the sending side;
//! then `send` / `recv` of `Transport`; an error is printed with `{}`.
//!
//! Opening maps shared memory, which a contract target does not. What is
//! checked: the four configuration fields, the constructor signatures, and
//! that the tool's numbers meet the precondition `open` checks before it
//! touches the file system (`capacity >= 2 * max_datagram + 16`, refused
//! as `PosixShmError::InvalidConfig`).
#![cfg(unix)]
#![allow(clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use zerodds_transport::Transport;
use zerodds_transport_shm::{PosixShmError, PosixShmTransport, ShmConfig};

fn tool_config() -> ShmConfig {
    ShmConfig {
        capacity: 1 << 16,
        flink_dir: PathBuf::from("/nonexistent/zerodds-contract"),
        max_datagram: 4096,
        recv_timeout: Some(Duration::from_secs(10)),
    }
}

#[test]
fn the_calls_run_shm_makes() {
    let config = tool_config();
    let retry = config.clone();
    assert_eq!((retry.capacity, retry.max_datagram), (65_536, 4096));
    let _: fn([u8; 16], [u8; 16], ShmConfig) -> Result<PosixShmTransport, PosixShmError> =
        PosixShmTransport::open_consumer;
    let _: fn([u8; 16], [u8; 16], ShmConfig) -> Result<PosixShmTransport, PosixShmError> =
        PosixShmTransport::open_owner;
    fn loops<T: Transport>() {}
    loops::<PosixShmTransport>();
    fn printed<E: core::fmt::Display>() {}
    printed::<PosixShmError>();
}

#[test]
fn the_tools_ring_meets_the_precondition_and_a_smaller_one_is_refused() {
    let config = tool_config();
    assert!(config.capacity >= 2 * config.max_datagram + 16);
    let small = ShmConfig {
        capacity: 2 * 4096 + 15,
        ..tool_config()
    };
    match PosixShmTransport::open_owner([1; 16], [2; 16], small) {
        Err(PosixShmError::InvalidConfig { .. }) => {}
        Err(other) => panic!("expected InvalidConfig, got {other:?}"),
        Ok(_) => panic!("a ring one octet short was opened"),
    }
}
