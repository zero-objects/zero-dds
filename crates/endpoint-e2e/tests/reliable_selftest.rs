// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Reliable-stream self-evidence: a Rust reliable **sender** (the reference
//! `ReliableStreamState` + reliable `WRITE_DATA` framing + retransmit on
//! `ACKNACK` + periodic `HEARTBEAT`) sends N samples over real UDP to the
//! [`ReliablePeer`]. With loss injected on the peer, the sender must retransmit
//! the dropped sequence numbers; the peer delivers **all** N gap-free in order.
//! This proves the peer + loss-recovery mechanics Rust↔Rust, independent of any
//! language SDK — the same peer every language reliable agent tests against.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_endpoint_e2e::{
    ReliableConfig, ReliableStreamState, SerialNumber16, StreamId, bind_reliable_peer,
    heartbeat_frame, parse_acknack, reliable_collect, reliable_write_frame,
};

/// One sample per sequence number: a single distinct byte so the receiver can
/// assert exact in-order content.
fn sample(i: usize) -> Vec<u8> {
    vec![i as u8]
}

/// A reference reliable sender: submits N samples, sends them, then retransmits
/// in-flight samples in response to `ACKNACK` and emits `HEARTBEAT`, until its
/// window drains (all acked) or `stop` is set.
fn reliable_sender(target_port: u16, n: usize, stop: &Arc<AtomicBool>) {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("sender bind");
    sock.connect(("127.0.0.1", target_port)).expect("connect");
    sock.set_read_timeout(Some(Duration::from_millis(50)))
        .expect("timeout");

    let mut state = ReliableStreamState::new(StreamId::BUILTIN_RELIABLE, ReliableConfig::default());

    // Submit + send the initial burst.
    for i in 0..n {
        let payload = sample(i);
        let seq = state.submit(payload.clone()).expect("submit");
        let _ = sock.send(&reliable_write_frame(seq.raw(), &payload));
    }

    let start = Instant::now();
    let mut buf = [0u8; 512];
    while state.in_flight_count() > 0
        && !stop.load(Ordering::Relaxed)
        && start.elapsed() < Duration::from_secs(20)
    {
        if let Some(hb) = state.pending_heartbeat(start.elapsed()) {
            let _ = sock.send(&heartbeat_frame(hb));
        }
        if let Ok(m) = sock.recv(&mut buf) {
            if let Some(ack) = parse_acknack(&buf[..m]) {
                state.recv_acknack(ack);
                // Retransmit every still-in-flight sample in the ACKNACK window.
                let base = u16::from_le_bytes(ack.first_unacked_seq_num.to_le_bytes());
                for i in 0..16u16 {
                    let s = SerialNumber16::new(base.wrapping_add(i));
                    if let Some(p) = state.get_in_flight(s) {
                        let _ = sock.send(&reliable_write_frame(s.raw(), p));
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run(drop_every: Option<usize>, label: &str) {
    let n = 12usize;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let port = peer.port;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_tx = Arc::clone(&stop);
    let handle = std::thread::spawn(move || reliable_sender(port, n, &stop_tx));

    let got = reliable_collect(&peer, label, n);
    stop.store(true, Ordering::Relaxed);
    handle.join().ok();

    assert_eq!(got.len(), n, "{label}: delivered count");
    for (i, p) in got.iter().enumerate() {
        assert_eq!(p, &sample(i), "{label}: sample {i} wrong or out of order");
    }
}

/// Loss-recovery: every 3rd distinct datagram dropped once → retransmit closes
/// the gaps → all 12 delivered contiguously in order.
#[test]
fn reliable_loss_recovery_drop_every_3() {
    run(Some(3), "reliable/drop3");
}

/// Lossless baseline: all 12 delivered in order with no retransmit needed.
#[test]
fn reliable_no_loss_baseline() {
    run(None, "reliable/noloss");
}
