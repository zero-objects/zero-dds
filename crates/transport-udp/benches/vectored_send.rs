//! WP 2.0a-2 zero-copy completion — bench: flat `send` vs vectored
//! `sendmsg` on UDP loopback.
//!
//! Question to the bench: is the MessageBuilder refactor to
//! `Vec<IoSlice>` + `sendmsg(2)` worth it? WP 2.0a showed that
//! eliminating a single payload copy yields only 3-7 % —
//! the rest is in `MessageBuilder::try_add_submessage` (concat
//! into a flat Vec) and `Transport::send` (serialize Vec).
//!
//! The bench simulates the typical writer output: RTPS header
//! (20 B) + submessage header (4 B) + payload (variable). We
//! compare:
//!
//! - **flat**: three slices are copied into a Vec<u8>, then
//!   `socket.send_to(&vec, addr)` → one `sendto(2)` with already
//!   concatenated bytes.
//! - **vectored**: `socket2::Socket::send_to_vectored_with_flags`
//!   with `[IoSlice]` — the kernel does the scatter-gather itself.
//!
//! If the delta between the two is < 5 %, the complete
//! MessageBuilder refactor makes no sense for UDP. If >= 20 %,
//! the WP 2.0a-2 full migration is worth it.
//!
//! Run:
//!   cargo bench -p zerodds-transport-udp --bench vectored_send \
//!       -- --save-baseline iovec

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::io::IoSlice;
use std::net::{Ipv4Addr, SocketAddrV4};

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

const RTPS_HEADER: [u8; 20] = [
    b'R', b'T', b'P', b'S', 2, 5, 0x01, 0x0F, // 8B: magic+version+vendor
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, // 12B GuidPrefix
];
const SUBMSG_HEADER: [u8; 4] = [0x15, 0x05, 0, 0]; // DATA + E-flag + placeholder len

fn setup_pair() -> (Socket, SockAddr) {
    let rx = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    rx.bind(&SockAddr::from(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    // bench-relevant: rx is never read, we only measure TX.
    let tx = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    let rx_addr = rx.local_addr().unwrap();
    // keep rx alive via leak — bench lives for process lifetime.
    Box::leak(Box::new(rx));
    (tx, rx_addr)
}

fn bench_send(c: &mut Criterion) {
    let mut group = c.benchmark_group("udp_send");
    let (tx, rx_addr) = setup_pair();
    for payload_size in [64usize, 1024, 4096, 16_384, 65_000] {
        let payload = vec![0xABu8; payload_size];
        group.throughput(Throughput::Bytes(
            (RTPS_HEADER.len() + SUBMSG_HEADER.len() + payload_size) as u64,
        ));

        // flat: concat in ein Vec, ein sendto.
        group.bench_with_input(
            BenchmarkId::new("flat", payload_size),
            &payload,
            |b, payload| {
                let mut buf =
                    Vec::with_capacity(RTPS_HEADER.len() + SUBMSG_HEADER.len() + payload.len());
                b.iter(|| {
                    buf.clear();
                    buf.extend_from_slice(&RTPS_HEADER);
                    buf.extend_from_slice(&SUBMSG_HEADER);
                    buf.extend_from_slice(payload);
                    tx.send_to(black_box(&buf), &rx_addr).unwrap();
                });
            },
        );

        // vectored: three IoSlices, ein sendmsg.
        group.bench_with_input(
            BenchmarkId::new("vectored", payload_size),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let slices = [
                        IoSlice::new(&RTPS_HEADER),
                        IoSlice::new(&SUBMSG_HEADER),
                        IoSlice::new(payload),
                    ];
                    tx.send_to_vectored(black_box(&slices), &rx_addr).unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_send);
criterion_main!(benches);
