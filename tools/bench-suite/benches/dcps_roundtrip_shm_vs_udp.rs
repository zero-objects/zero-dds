// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bench-A1 — DCPS End-to-End Roundtrip (Linux-only).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs,
    clippy::print_stderr,
    clippy::print_stdout
)]

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("dcps_roundtrip_shm_vs_udp: Linux-only");
}

#[cfg(target_os = "linux")]
use criterion::{criterion_group, criterion_main};

#[cfg(target_os = "linux")]
mod inner {
    use std::ffi::{CString, c_void};
    use std::ptr;
    use std::thread;
    use std::time::Duration;

    use criterion::{BenchmarkId, Criterion, Throughput, black_box};
    use zerodds_bench_suite::{PAYLOAD_SIZES, make_payload, size_label};

    const DGRAM_MAX: usize = 65_507;

    pub fn bench_dcps_roundtrip(c: &mut Criterion) {
        let mut group = c.benchmark_group("dcps_roundtrip");
        group.measurement_time(Duration::from_secs(8));
        group.sample_size(50);

        // Domain max ~232 wegen RTPS spdp_port = 7400 + 250*domain
        // (u16-Cap, Spec §9.6.1.4.1). Disjunkt zu cffi_take_vs_loan.
        let domain: u32 = 100 + (std::process::id() % 50);
        let topic = CString::new("BenchA1Roundtrip").unwrap();
        let typ = CString::new("RawBytes").unwrap();
        let (rt_pub, rt_sub, writer, reader) = unsafe {
            let rt_pub = zerodds::zerodds_runtime_create(domain);
            let rt_sub = zerodds::zerodds_runtime_create(domain);
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rt_pub, 1, 5_000), 0);
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rt_sub, 1, 5_000), 0);
            let writer = zerodds::zerodds_writer_create(rt_pub, topic.as_ptr(), typ.as_ptr(), 1);
            let reader = zerodds::zerodds_reader_create(rt_sub, topic.as_ptr(), typ.as_ptr(), 1);
            assert_eq!(
                zerodds::zerodds_writer_wait_for_matched(writer, 1, 5_000),
                0
            );
            // Bug-2 (2026-05-19): symmetric match wait.
            assert_eq!(
                zerodds::zerodds_reader_wait_for_matched(reader, 1, 5_000),
                0
            );
            (rt_pub, rt_sub, writer, reader)
        };
        thread::sleep(Duration::from_millis(50));
        // Drain alte Samples vor Mess-Start.
        unsafe {
            let mut buf: *const u8 = ptr::null();
            let mut len: usize = 0;
            let mut loan: *mut c_void = ptr::null_mut();
            while zerodds::zerodds_reader_loan(reader, &mut buf, &mut len, &mut loan) == 0
                && !loan.is_null()
            {
                zerodds::zerodds_reader_return_loan(loan);
            }
        }

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
                    b.iter(|| unsafe {
                        let rc = zerodds::zerodds_writer_write(writer, p.as_ptr(), p.len());
                        assert_eq!(rc, 0);
                        let mut buf: *const u8 = ptr::null();
                        let mut len: usize = 0;
                        let mut loan: *mut c_void = ptr::null_mut();
                        // Wall-clock-Deadline: 2s reicht selbst fuer
                        // worst-case-Scheduler-Hiccups. Spin-only mit
                        // 200k Iterationen sind ~1ms — viel zu eng
                        // fuer DCPS-Sample-Delivery (~10-100us
                        // typisch, aber 10ms+ moeglich).
                        let deadline =
                            std::time::Instant::now() + std::time::Duration::from_secs(2);
                        loop {
                            let rc =
                                zerodds::zerodds_reader_loan(reader, &mut buf, &mut len, &mut loan);
                            assert_eq!(rc, 0);
                            if !loan.is_null() {
                                black_box(buf);
                                zerodds::zerodds_reader_return_loan(loan);
                                return;
                            }
                            if std::time::Instant::now() >= deadline {
                                panic!("no sample within 2s");
                            }
                            std::hint::spin_loop();
                        }
                    });
                },
            );
        }
        group.finish();

        unsafe {
            zerodds::zerodds_writer_destroy(writer);
            zerodds::zerodds_reader_destroy(reader);
            zerodds::zerodds_runtime_destroy(rt_pub);
            zerodds::zerodds_runtime_destroy(rt_sub);
        }
    }
}

#[cfg(target_os = "linux")]
criterion_group!(benches, inner::bench_dcps_roundtrip);
#[cfg(target_os = "linux")]
criterion_main!(benches);
