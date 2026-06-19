// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bench-A2 — C-FFI Read-Pfad: `take` vs `loan/_return` (Linux-only).

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
    eprintln!("cffi_take_vs_loan: Linux-only");
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

    fn setup() -> (
        *mut zerodds::ZeroDdsRuntime,
        *mut zerodds::ZeroDdsRuntime,
        *mut zerodds::ZeroDdsWriter,
        *mut zerodds::ZeroDdsReader,
    ) {
        // Domain max ~232 wegen RTPS spdp_port = 7400 + 250*domain
        // (u16-Cap, Spec §9.6.1.4.1). Disjunkt zu dcps_roundtrip.
        let domain: u32 = 150 + (std::process::id() % 50);
        let topic = CString::new("BenchA2TakeVsLoan").unwrap();
        let typ = CString::new("RawBytes").unwrap();
        unsafe {
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
            // Symmetrisch warten — Bug-2 (2026-05-19): writer-side
            // matched zuerst, reader-side asynchron danach. Ohne
            // Reader-Wait sendet write an leere reader_proxy-Liste.
            assert_eq!(
                zerodds::zerodds_reader_wait_for_matched(reader, 1, 5_000),
                0
            );
            (rt_pub, rt_sub, writer, reader)
        }
    }

    fn drain(reader: *mut zerodds::ZeroDdsReader) {
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
    }

    pub fn bench_cffi_read(c: &mut Criterion) {
        let (rt_pub, rt_sub, writer, reader) = setup();
        thread::sleep(Duration::from_millis(50));
        drain(reader);

        let mut group_take = c.benchmark_group("cffi_take");
        group_take.measurement_time(Duration::from_secs(6));
        for &size in PAYLOAD_SIZES {
            if size > DGRAM_MAX {
                continue;
            }
            let payload = make_payload(size);
            group_take.throughput(Throughput::Bytes(size as u64));
            group_take.bench_with_input(
                BenchmarkId::from_parameter(size_label(size)),
                &payload,
                |b, p| {
                    b.iter(|| unsafe {
                        let rc = zerodds::zerodds_writer_write(writer, p.as_ptr(), p.len());
                        assert_eq!(rc, 0);
                        let mut out_buf: *mut u8 = ptr::null_mut();
                        let mut out_len: usize = 0;
                        for _ in 0..200_000 {
                            let rc = zerodds::zerodds_reader_take(
                                reader,
                                &mut out_buf,
                                &mut out_len,
                                ptr::null_mut(),
                            );
                            assert_eq!(rc, 0);
                            if !out_buf.is_null() {
                                black_box(out_buf);
                                zerodds::zerodds_buffer_free(out_buf, out_len);
                                return;
                            }
                            std::hint::spin_loop();
                        }
                        panic!("take: no sample within 200_000 spins");
                    });
                },
            );
        }
        group_take.finish();

        drain(reader);

        let mut group_loan = c.benchmark_group("cffi_loan");
        group_loan.measurement_time(Duration::from_secs(6));
        for &size in PAYLOAD_SIZES {
            if size > DGRAM_MAX {
                continue;
            }
            let payload = make_payload(size);
            group_loan.throughput(Throughput::Bytes(size as u64));
            group_loan.bench_with_input(
                BenchmarkId::from_parameter(size_label(size)),
                &payload,
                |b, p| {
                    b.iter(|| unsafe {
                        let rc = zerodds::zerodds_writer_write(writer, p.as_ptr(), p.len());
                        assert_eq!(rc, 0);
                        let mut buf: *const u8 = ptr::null();
                        let mut len: usize = 0;
                        let mut loan: *mut c_void = ptr::null_mut();
                        for _ in 0..200_000 {
                            let rc =
                                zerodds::zerodds_reader_loan(reader, &mut buf, &mut len, &mut loan);
                            assert_eq!(rc, 0);
                            if !loan.is_null() {
                                black_box(buf);
                                zerodds::zerodds_reader_return_loan(loan);
                                return;
                            }
                            std::hint::spin_loop();
                        }
                        panic!("loan: no sample within 200_000 spins");
                    });
                },
            );
        }
        group_loan.finish();

        unsafe {
            zerodds::zerodds_writer_destroy(writer);
            zerodds::zerodds_reader_destroy(reader);
            zerodds::zerodds_runtime_destroy(rt_pub);
            zerodds::zerodds_runtime_destroy(rt_sub);
        }
    }
}

#[cfg(target_os = "linux")]
criterion_group!(benches, inner::bench_cffi_read);
#[cfg(target_os = "linux")]
criterion_main!(benches);
