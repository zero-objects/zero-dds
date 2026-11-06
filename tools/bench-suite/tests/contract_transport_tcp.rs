// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-transport-tcp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-tcp`: no module of `src/`
//! names it; `benches/transports_e2e.rs` does, as the `zerodds-rtps`
//! contract of this crate counts the benches. It calls
//! `TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)`, `local_locator` and
//! `send` of `Transport`, and on the receiving side, behind an `Arc` in an
//! accept thread, `accept_one` and `try_recv`.
//!
//! Every one of these opens a socket, which a contract target does not:
//! what is checked is the signatures and bounds the bench compiles against.

use std::net::Ipv4Addr;

use zerodds_transport::{ReceivedDatagram, RecvError, Transport};
use zerodds_transport_tcp::{TcpTransport, TcpTransportError};

#[test]
fn the_calls_the_bench_makes() {
    let _: fn(Ipv4Addr, u16) -> Result<TcpTransport, TcpTransportError> = TcpTransport::bind_v4;
    let _: fn(&TcpTransport) -> Result<(), TcpTransportError> = TcpTransport::accept_one;
    let _: fn(&TcpTransport) -> Result<ReceivedDatagram, RecvError> = TcpTransport::try_recv;
}

#[test]
fn a_tcp_transport_is_a_transport_an_accept_thread_can_share() {
    fn shared<T: Transport + Send + Sync + 'static>() {}
    shared::<TcpTransport>();
}
