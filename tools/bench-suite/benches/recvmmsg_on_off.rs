// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bench-A3 — recvmmsg-batch ON vs OFF (Linux-only).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    missing_docs,
    clippy::print_stderr,
    clippy::print_stdout
)]

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("recvmmsg_on_off: Linux-only");
}

#[cfg(target_os = "linux")]
use criterion::{criterion_group, criterion_main};

#[cfg(target_os = "linux")]
mod inner {
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration;

    use criterion::{BenchmarkId, Criterion, Throughput, black_box};
    use zerodds_bench_suite::{PAYLOAD_SIZES, make_payload, size_label};
    use zerodds_rtps::wire_types::Locator;
    use zerodds_transport::Transport;
    use zerodds_transport_udp::UdpTransport;

    const DGRAM_MAX: usize = 65_507;

    pub struct FlooderGuard {
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl Drop for FlooderGuard {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    fn spawn_flooder(target: Locator, payload: Vec<u8>) -> FlooderGuard {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_tx = Arc::clone(&stop);
        let tx = Arc::new(UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let h = thread::spawn(move || {
            while !stop_tx.load(Ordering::Relaxed) {
                let _ = tx.send(&target, &payload);
                std::thread::sleep(Duration::from_micros(5));
            }
        });
        FlooderGuard {
            stop,
            handle: Some(h),
        }
    }

    pub fn bench_single_recv(c: &mut Criterion) {
        let mut group = c.benchmark_group("recvmmsg_off_single_recv");
        group.measurement_time(Duration::from_secs(6));
        for &size in PAYLOAD_SIZES {
            if size > DGRAM_MAX {
                continue;
            }
            let payload = make_payload(size);
            let rx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            let rx_loc = <UdpTransport as Transport>::local_locator(&rx);
            let _flooder = spawn_flooder(rx_loc, payload.clone());
            group.throughput(Throughput::Bytes(size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size_label(size)),
                &(),
                |b, _| {
                    b.iter(|| {
                        let dg = rx.recv().unwrap();
                        black_box(dg);
                    });
                },
            );
        }
        group.finish();
    }

    #[cfg(feature = "recvmmsg-batch")]
    pub fn bench_batch_recv(c: &mut Criterion) {
        use zerodds_transport::ReceivedDatagram;
        use zerodds_transport_udp::recv_batch::recv_batch_linux;
        let mut group = c.benchmark_group("recvmmsg_on_batch_recv");
        group.measurement_time(Duration::from_secs(6));
        for &size in PAYLOAD_SIZES {
            if size > DGRAM_MAX {
                continue;
            }
            let payload = make_payload(size);
            let rx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            let rx_loc = <UdpTransport as Transport>::local_locator(&rx);
            let _flooder = spawn_flooder(rx_loc, payload.clone());
            group.throughput(Throughput::Bytes(size as u64 * 32));
            let mut out: Vec<ReceivedDatagram> = Vec::with_capacity(32);
            group.bench_with_input(
                BenchmarkId::from_parameter(size_label(size)),
                &(),
                |b, _| {
                    b.iter(|| {
                        out.clear();
                        let n = recv_batch_linux(rx.std_socket(), &mut out, 32).unwrap_or(0);
                        if n == 0 {
                            let _ = rx.recv();
                        }
                        black_box(n);
                    });
                },
            );
        }
        group.finish();
    }
}

#[cfg(all(target_os = "linux", feature = "recvmmsg-batch"))]
criterion_group!(benches, inner::bench_single_recv, inner::bench_batch_recv);
#[cfg(all(target_os = "linux", not(feature = "recvmmsg-batch")))]
criterion_group!(benches, inner::bench_single_recv);
#[cfg(target_os = "linux")]
criterion_main!(benches);
