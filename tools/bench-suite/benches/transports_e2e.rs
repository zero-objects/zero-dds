// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Transport-E2E Benchmark-Suite (v1.2 baseline).
//!
//! Misst die `send()`-Pfade aller ZeroDDS-Transports ueber die
//! kanonische Payload-Achse. Ein Receiver-Thread drainiert den
//! Kernel-Socket-Buffer im Hintergrund, damit die Messung nicht
//! durch volle OS-Puffer blockt oder dropped.
//!
//! # Scope
//!
//! Zeit pro `send()` bei unterschiedlichen Payload-Groessen auf
//! dem TX-Pfad. Der RX-Thread ist kein Teil der Messung — er hebt
//! nur die Socket-Buffer-Grenze auf. Echte E2E-Ping/Pong-Latenz
//! liefert das `roundtrip-1us`-Binary.
//!
//! # Was hier bewusst NICHT gemessen wird
//!
//! - E2E-Latenz (Send→Recv Roundtrip).
//! - Multi-producer/consumer contention.
//! - Cross-Vendor-Vergleich gegen Cyclone/FastDDS — Sache des
//!   Live-Interop-Harness in `internal/interop/`.
//!
//! # Run
//!
//! ```bash
//! # Baseline auf getuntem llvm-Host:
//! sudo ./benches/hosts/llvm/tune.sh on
//! taskset -c 4-11 cargo bench -p zerodds-bench-suite \
//!     --bench transports_e2e -- --save-baseline v1.2-baseline
//! sudo ./benches/hosts/llvm/tune.sh off
//! ```

// transports_e2e nutzt UDS + POSIX-SHM — POSIX-only.
#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_bench_suite::{PAYLOAD_SIZES, make_payload, size_label};
use zerodds_rtps::wire_types::Locator;
use zerodds_transport::Transport;
use zerodds_transport_shm::{PosixShmTransport, ShmConfig};
use zerodds_transport_tcp::TcpTransport;
use zerodds_transport_udp::UdpTransport;
#[cfg(unix)]
use zerodds_transport_uds::{UdsConfig, UdsTransport};

/// Maximum DGRAM-Payload (UDP + UDS): 64 KiB - Header-Overhead.
const DGRAM_MAX: usize = 60 * 1024;

fn id(n: u8) -> [u8; 16] {
    let mut a = [0u8; 16];
    a[0] = 0xE0;
    a[15] = n;
    a
}

/// Drain-Guard: haelt einen Thread am Leben der recv() in einer Loop
/// aufruft, bis `stop` gesetzt wird. `Drop` signalisiert + joined.
///
/// Damit der Kernel-Socket-Buffer bei grossen TX-Raten nicht volllaeuft
/// und `send()` blockt oder dropped.
struct DrainGuard {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl DrainGuard {
    fn spawn<T: Transport + Send + 'static>(t: T) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_cl = stop.clone();
        let handle = thread::spawn(move || {
            while !stop_cl.load(Ordering::Relaxed) {
                let _ = t.recv();
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for DrainGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Eine kleine Send-to-self triggert den recv-Wakeup. Fuer
        // UDP+UDS funktioniert das ohnehin durch recv_timeout, den
        // wir beim Drainer setzen muessen — siehe spawn-helfer.
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// --------------------------------------------------------------------
// UDP
// --------------------------------------------------------------------

fn bench_udp(c: &mut Criterion) {
    let mut group = c.benchmark_group("udp_send");
    group.measurement_time(Duration::from_secs(5));

    let rx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
        .unwrap()
        .with_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let rx_locator = <UdpTransport as Transport>::local_locator(&rx);
    let tx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
    let _drain = DrainGuard::spawn(rx);

    for &size in PAYLOAD_SIZES {
        if size > DGRAM_MAX {
            continue;
        }
        let payload = make_payload(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_label(size)),
            &payload,
            |b, p| {
                b.iter(|| tx.send(black_box(&rx_locator), black_box(p)).unwrap());
            },
        );
    }
    group.finish();
}

// --------------------------------------------------------------------
// UDP recv (Welle 4-Folge — R1 Slab-Pool Entscheidungs-Baseline)
// --------------------------------------------------------------------
//
// Misst die Recv-Hot-Path-Latenz (recv_from + Arc::from). Sender im
// Background-Thread floodet kontinuierlich; Bench-Thread misst nur
// die Zeit fuer einen `recv()`-Call. Heap-Alloc-Druck pro Iteration
// = 1× `Arc::from(&buf[..len])`. Vergleichswert fuer ein zukuenftiges
// Slab-Pool-Re-Implementation (`Arc<[u8; SIZE]>`-Pool ohne Per-
// Datagram-Alloc).
fn bench_udp_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("udp_recv");
    group.measurement_time(Duration::from_secs(5));

    let rx = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
        .unwrap()
        .with_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let rx_locator = <UdpTransport as Transport>::local_locator(&rx);
    let tx = Arc::new(UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());

    for &size in PAYLOAD_SIZES {
        if size > DGRAM_MAX {
            continue;
        }
        let payload = make_payload(size);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_tx = Arc::clone(&stop);
        let tx_clone = Arc::clone(&tx);
        let rx_loc = rx_locator;
        let payload_clone = payload.clone();
        // Background-Sender flutet kontinuierlich. Wir senden mit
        // 50us-Pause damit der Sender selbst nicht die CPU monopolisiert.
        let sender = thread::spawn(move || {
            while !stop_tx.load(Ordering::Relaxed) {
                let _ = tx_clone.send(&rx_loc, &payload_clone);
                std::thread::sleep(Duration::from_micros(50));
            }
        });

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_label(size)),
            &(),
            |b, _| {
                b.iter(|| {
                    // recv() = recv_from(stack-buf) + Arc::from-Alloc
                    let dg = rx.recv().unwrap();
                    black_box(dg);
                });
            },
        );

        stop.store(true, Ordering::Relaxed);
        sender.join().unwrap();
    }
    group.finish();
}

// --------------------------------------------------------------------
// UDS Filesystem (T1)
// --------------------------------------------------------------------

#[cfg(not(unix))]
fn bench_uds_fs(_: &mut Criterion) {
    // Unix-only; no-op on Windows.
}

#[cfg(unix)]
fn bench_uds_fs(c: &mut Criterion) {
    let mut group = c.benchmark_group("uds_fs_send");
    group.measurement_time(Duration::from_secs(5));
    let tmp = tempfile_dir("uds-fs");

    let rx_cfg = UdsConfig {
        base_dir: tmp.clone(),
        max_datagram: DGRAM_MAX,
        recv_timeout: Some(Duration::from_millis(50)),
    };
    let tx_cfg = UdsConfig {
        base_dir: tmp.clone(),
        max_datagram: DGRAM_MAX,
        recv_timeout: None,
    };
    let rx = UdsTransport::bind(id(1), rx_cfg).unwrap();
    let tx = UdsTransport::bind(id(2), tx_cfg).unwrap();
    let rx_locator = Locator::uds(id(1));
    let _drain = DrainGuard::spawn(rx);

    for &size in PAYLOAD_SIZES {
        if size > DGRAM_MAX {
            continue;
        }
        let payload = make_payload(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_label(size)),
            &payload,
            |b, p| {
                b.iter(|| tx.send(black_box(&rx_locator), black_box(p)).unwrap());
            },
        );
    }
    group.finish();
    drop(_drain);
    cleanup_dir(&tmp);
}

// --------------------------------------------------------------------
// UDS Abstract (T5, Linux-only)
// --------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn bench_uds_abstract(c: &mut Criterion) {
    use zerodds_transport_uds::abstract_dgram::{
        AbstractDgramConfig, UdsAbstractDgramTransport, UdsAddress,
    };
    let mut group = c.benchmark_group("uds_abstract_send");
    group.measurement_time(Duration::from_secs(5));
    let prefix = format!("zerodds-bench-{}", std::process::id());
    let rx_cfg = AbstractDgramConfig {
        address: UdsAddress::Abstract {
            prefix: prefix.clone(),
        },
        recv_buf: DGRAM_MAX,
        recv_timeout: Some(Duration::from_millis(50)),
    };
    let tx_cfg = AbstractDgramConfig {
        address: UdsAddress::Abstract { prefix },
        recv_buf: DGRAM_MAX,
        recv_timeout: None,
    };
    let rx = UdsAbstractDgramTransport::bind(id(1), rx_cfg).unwrap();
    let tx = UdsAbstractDgramTransport::bind(id(2), tx_cfg).unwrap();
    let rx_locator = Locator::uds(id(1));
    let _drain = DrainGuard::spawn(rx);

    for &size in PAYLOAD_SIZES {
        if size > DGRAM_MAX {
            continue;
        }
        let payload = make_payload(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_label(size)),
            &payload,
            |b, p| {
                b.iter(|| tx.send(black_box(&rx_locator), black_box(p)).unwrap());
            },
        );
    }
    group.finish();
}

#[cfg(not(target_os = "linux"))]
fn bench_uds_abstract(_: &mut Criterion) {
    // Linux-only; no-op auf macOS/Windows.
}

// --------------------------------------------------------------------
// POSIX-SHM (T3) — Shmem ist nicht Send, Consumer drain inline
// --------------------------------------------------------------------

fn bench_shm(c: &mut Criterion) {
    let mut group = c.benchmark_group("shm_send");
    group.measurement_time(Duration::from_secs(5));
    let tmp = tempfile_dir("shm");

    for &size in PAYLOAD_SIZES {
        let max_datagram = size;
        let capacity = (max_datagram * 4).max(4096);
        let cfg = ShmConfig {
            capacity,
            flink_dir: tmp.clone(),
            max_datagram,
            recv_timeout: Some(Duration::from_millis(50)),
        };
        let owner_id = id(10u8.wrapping_add((size as u8).wrapping_mul(3)));
        let consumer_id = id(11u8.wrapping_add((size as u8).wrapping_mul(3)));
        let owner = PosixShmTransport::open_owner(owner_id, consumer_id, cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(consumer_id, owner_id, cfg).unwrap();
        let rx_locator = Locator::shm(consumer_id);
        let payload = make_payload(size);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_label(size)),
            &payload,
            |b, p| {
                b.iter(|| {
                    owner.send(black_box(&rx_locator), black_box(p)).unwrap();
                    // Inline-drain (Shmem is !Send, kein Thread moeglich).
                    let _ = consumer.recv();
                });
            },
        );
        drop(owner);
        drop(consumer);
    }
    group.finish();
    cleanup_dir(&tmp);
}

// --------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------

fn tempfile_dir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "zerodds-bench-{}-{}-{}",
        tag,
        std::process::id(),
        rand_tag()
    ));
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn rand_tag() -> u64 {
    // Nanos seit UNIX_EPOCH als poor-man-unique-tag.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}

fn cleanup_dir(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

// --------------------------------------------------------------------
// TCP
// --------------------------------------------------------------------

fn bench_tcp(c: &mut Criterion) {
    let mut group = c.benchmark_group("tcp_send");
    group.measurement_time(Duration::from_secs(5));

    // RX: TcpTransport bindet einen Listener. TX: zweite Instanz, die
    // sich auf den Listener verbindet. accept_one() in Background-Thread
    // holt die Connection rein, damit `tx.send()` sofort losschicken kann.
    let rx = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("rx bind");
    let rx_locator = <TcpTransport as Transport>::local_locator(&rx);
    let rx_arc = Arc::new(rx);
    let accept_rx = Arc::clone(&rx_arc);
    let accept_handle = thread::spawn(move || {
        let _ = accept_rx.accept_one();
        loop {
            match accept_rx.try_recv() {
                Ok(_dg) => {}
                Err(_) => {
                    thread::sleep(Duration::from_micros(50));
                }
            }
        }
    });

    let tx = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("tx bind");

    for &size in PAYLOAD_SIZES {
        if size > DGRAM_MAX {
            continue;
        }
        let payload = make_payload(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_label(size)),
            &payload,
            |b, p| {
                b.iter(|| tx.send(black_box(&rx_locator), black_box(p)).unwrap());
            },
        );
    }
    group.finish();
    drop(tx);
    drop(rx_arc);
    let _ = accept_handle; // Best-effort cleanup; thread läuft im idle-poll.
}

criterion_group!(
    benches,
    bench_udp,
    bench_udp_recv,
    bench_uds_fs,
    bench_uds_abstract,
    bench_shm,
    bench_tcp
);
criterion_main!(benches);
