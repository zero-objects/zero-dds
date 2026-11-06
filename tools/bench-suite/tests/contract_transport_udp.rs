// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-transport-udp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-udp`: no module of `src/`
//! names it; the benches do, as the `zerodds-rtps` contract of this crate
//! counts them (`benches/transports_e2e.rs`, `benches/recvmmsg_on_off.rs`,
//! `benches/recv_threads_sweep.rs`), and `Cargo.toml` forwards
//! `recvmmsg-batch` to it. The benches call `UdpTransport::bind_v4`,
//! `bind_v4_reuse`, `with_timeout(Some(_))`, `std_socket`, the `Transport`
//! methods `local_locator`, `send` and `recv`, move a transport into a
//! drain thread and share one behind an `Arc` across threads, and on Linux
//! call `recv_batch::recv_batch_linux(socket, &mut Vec<ReceivedDatagram>,
//! 32)`.
//!
//! Every one of these opens a socket, which a contract target does not:
//! what is checked is the signatures and bounds the benches compile
//! against, so a change that breaks them fails here and not first in a
//! bench run.

use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;

use zerodds_transport::Transport;
use zerodds_transport_udp::{UdpTransport, UdpTransportError};

#[test]
fn the_constructors_and_settings_the_benches_call() {
    let _: fn(Ipv4Addr, u16) -> Result<UdpTransport, UdpTransportError> = UdpTransport::bind_v4;
    let _: fn(Ipv4Addr, u16) -> Result<UdpTransport, UdpTransportError> =
        UdpTransport::bind_v4_reuse;
    let _: fn(UdpTransport, Option<Duration>) -> Result<UdpTransport, UdpTransportError> =
        UdpTransport::with_timeout;
    let _: fn(&UdpTransport) -> &UdpSocket = UdpTransport::std_socket;
}

#[test]
fn a_udp_transport_is_a_transport_a_thread_can_own_and_share() {
    fn drained<T: Transport + Send + Sync + 'static>() {}
    drained::<UdpTransport>();
}

#[cfg(target_os = "linux")]
#[test]
fn the_linux_batch_receive_the_recvmmsg_bench_calls() {
    use zerodds_transport::{ReceivedDatagram, RecvError};
    let _: fn(&UdpSocket, &mut Vec<ReceivedDatagram>, usize) -> Result<usize, RecvError> =
        zerodds_transport_udp::recv_batch::recv_batch_linux;
}
