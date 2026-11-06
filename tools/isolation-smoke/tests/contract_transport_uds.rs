// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-isolation-smoke` with `zerodds-transport-uds`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-transport-uds` (a Unix-only dependency;
//! `src/main.rs`, `run_uds` and, on Linux, `run_uds_abstract`):
//! `UdsConfig { base_dir, max_datagram: 8192, recv_timeout: Some(10 s) }`
//! and `UdsTransport::bind(local_id, cfg)`;
//! `abstract_dgram::AbstractDgramConfig { address: UdsAddress::Abstract {
//! prefix }, recv_buf: 8192, recv_timeout }` and
//! `UdsAbstractDgramTransport::bind(local_id, cfg)`; then `send` / `recv` of
//! `Transport`; an error is printed with `{}`.
//!
//! Binding creates a socket, which a contract target does not: what is
//! checked is the configuration fields, the constructor signatures and the
//! bounds the tool compiles against.
#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use zerodds_transport::Transport;
use zerodds_transport_uds::{UdsConfig, UdsTransport};

#[test]
fn the_calls_run_uds_makes() {
    let config = UdsConfig {
        base_dir: PathBuf::from("/nonexistent/zerodds-contract"),
        max_datagram: 8192,
        recv_timeout: Some(Duration::from_secs(10)),
    };
    assert_eq!(config.max_datagram, 8192);
    let _: fn([u8; 16], UdsConfig) -> std::io::Result<UdsTransport> = UdsTransport::bind;
    fn loops<T: Transport>() {}
    loops::<UdsTransport>();
}

#[cfg(target_os = "linux")]
#[test]
fn the_calls_run_uds_abstract_makes_on_linux() {
    use zerodds_transport_uds::abstract_dgram::{
        AbstractDgramConfig, UdsAbstractDgramTransport, UdsAddress,
    };
    let config = AbstractDgramConfig {
        address: UdsAddress::Abstract {
            prefix: String::from("zerodds"),
        },
        recv_buf: 8192,
        recv_timeout: Some(Duration::from_secs(10)),
    };
    assert_eq!(config.recv_buf, 8192);
    let _: fn([u8; 16], AbstractDgramConfig) -> std::io::Result<UdsAbstractDgramTransport> =
        UdsAbstractDgramTransport::bind;
    fn loops<T: Transport>() {}
    loops::<UdsAbstractDgramTransport>();
}
