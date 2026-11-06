// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-transport-uds`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-uds` (a Unix-only dependency):
//! no module of `src/` names it; `benches/transports_e2e.rs` does, as the
//! `zerodds-rtps` contract of this crate counts the benches. It builds
//! `UdsConfig { base_dir, max_datagram, recv_timeout }`, binds
//! `UdsTransport::bind(id, cfg)` and moves the receiver into a drain thread;
//! on Linux it builds `abstract_dgram::AbstractDgramConfig { address:
//! UdsAddress::Abstract { prefix }, recv_buf, recv_timeout }` and binds
//! `UdsAbstractDgramTransport::bind(id, cfg)` the same way.
//!
//! Binding creates a socket, which a contract target does not: what is
//! checked is the configuration fields, the constructor signatures and the
//! bounds the bench compiles against.
#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use zerodds_transport::Transport;
use zerodds_transport_uds::{UdsConfig, UdsTransport};

#[test]
fn the_filesystem_transport_the_bench_binds() {
    let config = UdsConfig {
        base_dir: PathBuf::from("/nonexistent/zerodds-contract"),
        max_datagram: 60 * 1024,
        recv_timeout: Some(Duration::from_millis(50)),
    };
    assert_eq!(config.max_datagram, 61_440);
    let _: fn([u8; 16], UdsConfig) -> std::io::Result<UdsTransport> = UdsTransport::bind;
    fn drained<T: Transport + Send + 'static>() {}
    drained::<UdsTransport>();
}

#[cfg(target_os = "linux")]
#[test]
fn the_abstract_namespace_transport_the_bench_binds_on_linux() {
    use zerodds_transport_uds::abstract_dgram::{
        AbstractDgramConfig, UdsAbstractDgramTransport, UdsAddress,
    };
    let config = AbstractDgramConfig {
        address: UdsAddress::Abstract {
            prefix: String::from("zerodds-contract"),
        },
        recv_buf: 60 * 1024,
        recv_timeout: None,
    };
    assert_eq!(config.recv_buf, 61_440);
    let _: fn([u8; 16], AbstractDgramConfig) -> std::io::Result<UdsAbstractDgramTransport> =
        UdsAbstractDgramTransport::bind;
    fn drained<T: Transport + Send + 'static>() {}
    drained::<UdsAbstractDgramTransport>();
}
