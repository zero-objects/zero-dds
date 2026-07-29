// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reliable endpoint demo (DDS-XRCE reliable stream).
//!
//! Modes:
//! - `example_reliable <port>` — reliable **sender** to `127.0.0.1:<port>`
//!   (used by the `endpoint-e2e` loss-recovery test). Enqueues N samples through
//!   the async-decoupled writer; the drain thread retransmits on ACKNACK until
//!   the window drains, then exits.
//! - `example_reliable bench` — producer latency: wait-free `enqueue` (decoupled)
//!   vs. inline `send` (syscall on the producer). Prints ns figures.
//! - `example_reliable` — standalone in-process demo: a lossy receiver + a sender;
//!   the receiver prints the recovered contiguous sequence.

// A runnable demo: printing to stdout, `expect` on in-memory paths, and the
// index→float/u16 casts of the toy sample are intentional here.
#![allow(
    clippy::print_stdout,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use zerodds_cdr::Endianness;
use zerodds_endpoint_rust::Reading;
use zerodds_endpoint_rust::reliable::{
    AsyncReliableWriter, ReliableReceiver, acknack_frame, parse_heartbeat, unframe, write_frame,
};

const N: u32 = 12;

fn sample(i: u32) -> Vec<u8> {
    Reading {
        id: i,
        value: i as f32,
        label: format!("s{i}"),
    }
    .marshal(Endianness::Little)
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("bench") => bench(),
        Some(arg) if arg.parse::<u16>().is_ok() => send_to_peer(arg.parse().expect("port")),
        _ => standalone_demo(),
    }
}

/// E2E sender: enqueue N samples through the decoupled writer, drain, exit.
fn send_to_peer(port: u16) {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    sock.connect(("127.0.0.1", port)).expect("connect");
    let mut writer = AsyncReliableWriter::start(sock, 64);
    let handle = writer.handle();
    for i in 0..N {
        let mut item = sample(i);
        // Wait-free enqueue; spin on backpressure (ring full).
        while let Err(back) = handle.enqueue(item) {
            item = back;
            thread::yield_now();
        }
    }
    writer.shutdown(); // retransmits until the window drains, then joins
    println!("SENT {N} reliable samples");
}

/// Producer latency: decoupled `enqueue` (wait-free ring push) vs inline `send`
/// (syscall). `iters` stays below the ring capacity so the enqueue path never
/// hits backpressure — we measure the pure producer cost, not the drain.
fn bench() {
    let iters = 50_000u32;

    // Decoupled: the drain thread consumes; the producer only pushes the ring.
    let sink = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let sink_port = sink.local_addr().expect("addr").port();
    // A thread that just empties the sink socket, so sends don't block.
    thread::spawn(move || {
        let mut buf = [0u8; 2048];
        loop {
            if sink.recv(&mut buf).is_err() {
                break;
            }
        }
    });
    let dsock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    dsock.connect(("127.0.0.1", sink_port)).expect("connect");
    let mut writer = AsyncReliableWriter::start(dsock, 1 << 20);
    let handle = writer.handle();
    let s = sample(1);
    let t0 = Instant::now();
    for _ in 0..iters {
        let mut item = s.clone();
        while let Err(back) = handle.enqueue(item) {
            item = back;
            thread::yield_now();
        }
    }
    let enqueue_ns = t0.elapsed().as_nanos() / u128::from(iters);
    writer.shutdown();

    // Inline: marshal + syscall on the producer thread.
    let isock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    isock.connect(("127.0.0.1", sink_port)).expect("connect");
    let t1 = Instant::now();
    for i in 0..iters {
        let _ = isock.send(&write_frame(i as u16, &s));
    }
    let inline_ns = t1.elapsed().as_nanos() / u128::from(iters);

    println!("LATENCY enqueue(decoupled)={enqueue_ns}ns inline(send)={inline_ns}ns");
}

/// In-process demo: a lossy receiver + the decoupled sender; the receiver prints
/// the recovered contiguous sequence.
fn standalone_demo() {
    let rx = UdpSocket::bind("127.0.0.1:0").expect("bind rx");
    let rx_port = rx.local_addr().expect("addr").port();

    let receiver = thread::spawn(move || {
        rx.set_read_timeout(Some(Duration::from_millis(200))).ok();
        let mut state = ReliableReceiver::new();
        let mut delivered: Vec<u32> = Vec::new();
        let mut dropped_once = std::collections::HashSet::new();
        let mut count = 0u32;
        let mut last_from: Option<std::net::SocketAddr> = None;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut buf = [0u8; 2048];
        while delivered.len() < N as usize && Instant::now() < deadline {
            let Ok((n, from)) = rx.recv_from(&mut buf) else {
                continue;
            };
            last_from = Some(from);
            let frame = &buf[..n];
            if let Some((seq, body)) = unframe(frame) {
                count += 1;
                // Drop every 3rd distinct sample once, to force retransmit.
                if count % 3 == 0 && dropped_once.insert(seq) {
                    continue;
                }
                let _ = state.recv_data(seq, body.to_vec());
                for (_, payload) in state.drain_in_order() {
                    delivered.push(Reading::decode(&payload).id);
                }
                let _ = rx.send_to(&acknack_frame(state.pending_acknack(), 1), from);
            } else if parse_heartbeat(frame).is_some() {
                let _ = rx.send_to(&acknack_frame(state.pending_acknack(), 1), from);
            }
        }
        // Final all-acked ACKNACKs (expected == N now) so the sender's window
        // drains deterministically instead of relying on its close-deadline.
        if let Some(from) = last_from {
            for _ in 0..3 {
                let _ = rx.send_to(&acknack_frame(state.pending_acknack(), 1), from);
            }
        }
        delivered
    });

    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind tx");
    sock.connect(("127.0.0.1", rx_port)).expect("connect");
    let mut writer = AsyncReliableWriter::start(sock, 64);
    let handle = writer.handle();
    for i in 0..N {
        let mut item = sample(i);
        while let Err(back) = handle.enqueue(item) {
            item = back;
            thread::yield_now();
        }
    }
    writer.shutdown();

    let delivered = receiver.join().expect("receiver");
    println!("RECOVERED {delivered:?}");
    assert_eq!(
        delivered,
        (0..N).collect::<Vec<_>>(),
        "all samples must arrive gap-free in order despite loss"
    );
    println!("OK: {N} samples delivered gap-free in order despite injected loss");
}
