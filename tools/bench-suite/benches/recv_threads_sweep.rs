// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bench-A4 — Multi-Thread-Recv-Pool Skalierung (Linux-only).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    missing_docs,
    clippy::print_stderr,
    clippy::print_stdout
)]

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("recv_threads_sweep: Linux-only (SO_REUSEPORT)");
}

#[cfg(target_os = "linux")]
use criterion::{criterion_group, criterion_main};

#[cfg(target_os = "linux")]
mod inner {
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use criterion::{BenchmarkId, Criterion, Throughput, black_box};
    use zerodds_rtps::wire_types::Locator;
    use zerodds_transport::Transport;
    use zerodds_transport_udp::UdpTransport;

    const PAYLOAD_SIZE: usize = 1024;
    // Per-iter Drain-Ziel. 50k * 1KiB = 50 MiB. Bei ~200 MiB/s
    // Loopback-Throughput ~250ms/iter; 20 samples * 250ms = 5s.
    const SAMPLES_PER_ITER: u64 = 50_000;

    fn drain_n_samples(n: usize, port: u16, rx_loc: Locator) -> Duration {
        let sockets: Vec<Arc<UdpTransport>> = (0..n)
            .map(|_| {
                Arc::new(
                    UdpTransport::bind_v4_reuse(Ipv4Addr::LOCALHOST, port)
                        .unwrap()
                        .with_timeout(Some(Duration::from_millis(50)))
                        .unwrap(),
                )
            })
            .collect();
        let stop = Arc::new(AtomicBool::new(false));
        let counter = Arc::new(AtomicU64::new(0));

        let workers: Vec<thread::JoinHandle<()>> = sockets
            .into_iter()
            .map(|s| {
                let stop = Arc::clone(&stop);
                let counter = Arc::clone(&counter);
                thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        if s.recv().is_ok() {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();

        let payload = vec![0xAB; PAYLOAD_SIZE];
        let stop_tx = Arc::clone(&stop);
        let sender = thread::spawn(move || {
            let tx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
            while !stop_tx.load(Ordering::Relaxed) {
                let _ = tx.send(&rx_loc, &payload);
            }
        });

        // Drain-Ziel ist die Mess-Groesse. Criterion bekommt die
        // wall-clock-Dauer bis SAMPLES_PER_ITER pakete durch sind.
        let start = Instant::now();
        while counter.load(Ordering::Relaxed) < SAMPLES_PER_ITER {
            std::hint::spin_loop();
        }
        let elapsed = start.elapsed();
        stop.store(true, Ordering::Relaxed);
        for w in workers {
            let _ = w.join();
        }
        let _ = sender.join();
        elapsed
    }

    pub fn bench_recv_threads_sweep(c: &mut Criterion) {
        let mut group = c.benchmark_group("recv_threads_sweep");
        group.measurement_time(Duration::from_secs(10));
        group.sample_size(20);

        let probe = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(probe.local_locator().port).unwrap();
        let rx_loc = probe.local_locator();
        drop(probe);

        group.throughput(Throughput::Bytes(SAMPLES_PER_ITER * PAYLOAD_SIZE as u64));
        for &n in &[1usize, 2, 4] {
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        total += black_box(drain_n_samples(n, port, rx_loc));
                    }
                    total
                });
            });
        }
        group.finish();
    }
}

#[cfg(target_os = "linux")]
criterion_group!(benches, inner::bench_recv_threads_sweep);
#[cfg(target_os = "linux")]
criterion_main!(benches);
