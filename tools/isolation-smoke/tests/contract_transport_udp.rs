// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-isolation-smoke` with `zerodds-transport-udp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-transport-udp` (`src/main.rs`,
//! `run_udp`): `UdpTransport::bind_v4(ip, port)`, on the receiving side
//! `with_timeout(Some(10 s))`, then `send` / `recv` of `Transport` in
//! `send_loop` / `recv_loop`; an error is printed with `{}` and turned into
//! the exit code 2.
//!
//! Every one of these opens a socket, which a contract target does not:
//! what is checked is the signatures and bounds the tool compiles against.

use std::net::Ipv4Addr;
use std::time::Duration;

use zerodds_transport::Transport;
use zerodds_transport_udp::{UdpTransport, UdpTransportError};

#[test]
fn the_calls_run_udp_makes() {
    let _: fn(Ipv4Addr, u16) -> Result<UdpTransport, UdpTransportError> = UdpTransport::bind_v4;
    let _: fn(UdpTransport, Option<Duration>) -> Result<UdpTransport, UdpTransportError> =
        UdpTransport::with_timeout;
    fn loops<T: Transport>() {}
    loops::<UdpTransport>();
    fn printed<E: core::fmt::Display>() {}
    printed::<UdpTransportError>();
}
