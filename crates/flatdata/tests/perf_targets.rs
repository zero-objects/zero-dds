// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-flatdata-1.0 §11 — performance targets, asserted rather than
//! only benchmarked.
//!
//! The spec states four numbers for a 1 kB sample on the same host:
//! §11.1 P99 latency below 5 µs, §11.2 roughly 1 GB/s throughput per
//! writer, §11.3 zero heap allocations per write (see `zero_alloc.rs`)
//! and §11.4 a same-host publish latency below a fifth of the UDP
//! publish latency. `criterion` reports those numbers; these tests turn
//! them into a gate that fails when the path regresses past the target.
//!
//! The targets are release-profile figures. A debug build carries no
//! optimisation, so §11.1 and §11.2 apply the spec bound under
//! `--release` and a looser bound in a debug build — the measurement
//! runs in both profiles, only the budget differs. §11.4 needs no such
//! split; see the note on that test.
//!
//! Each target is measured on both backends: the `InMemorySlotAllocator`
//! (one process) and the `PosixSlotAllocator`, the cross-process segment
//! the C API's SHM loan path uses, with the writer on the owner's mapping
//! and the reader on a second mapping of the same segment. A wake of a
//! reader parked in another process is not part of these rounds; the
//! notify paths have their own tests.
//!
//! Every bound here is absolute and traced to a spec sentence. None of
//! them is a ratio against a second quantity measured on the same
//! machine: such a gate also measures its own denominator, so an
//! unchanged code path passes on a host with a slow reference and fails
//! on a host with a fast one. The live UDP-versus-slot-path comparison
//! lives in `benches/loopback.rs`, where criterion measures it and CI
//! compares it against the previous baseline of the same host.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    missing_docs
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_flatdata::{FlatReader, FlatStruct, FlatWriter, InMemorySlotAllocator};
#[cfg(all(unix, feature = "posix-mmap"))]
use zerodds_flatdata::{PosixSlotAllocator, SlotBackend};

/// 1 kB sample — the size every §11 target is stated for.
#[derive(Copy, Clone)]
#[repr(C)]
struct Kb {
    bytes: [u8; 1024],
}

// SAFETY: repr(C), Copy + 'static, a single array of primitives.
unsafe impl FlatStruct for Kb {
    const TYPE_HASH: [u8; 16] = [0x11; 16];
}

const SAMPLES: usize = 20_000;

/// Untimed rounds before every measurement. The slot pool, its page
/// mapping and the branch predictor are cold on the first writes; with
/// `SAMPLES` = 20 000 those rounds would otherwise sit in the top 1 % of
/// the sorted sample and shift the reported P99.
const WARMUP: usize = 2_000;

/// Rounds per timed batch of §11.2: about 0.2 ms in a debug build.
const BATCH: usize = 100;

const fn debug_build() -> bool {
    cfg!(debug_assertions)
}

fn percentile(sorted: &[Duration], pct: usize) -> Duration {
    sorted[(sorted.len() * pct / 100).min(sorted.len() - 1)]
}

/// A writer and a reader on one slot backend: `write` publishes a sample,
/// `read` takes the newest back and frees its slot.
trait Pair {
    fn write(&self, sample: &Kb);
    fn read(&self) -> Kb;
}

/// `FlatWriter` and `FlatReader` over the in-process allocator.
struct InMemory {
    writer: FlatWriter<Kb>,
    reader: FlatReader<Kb>,
}

impl InMemory {
    fn new() -> Self {
        let alloc = Arc::new(InMemorySlotAllocator::new(0, 4, 1024));
        Self {
            writer: FlatWriter::<Kb>::new(Arc::clone(&alloc), 0b1),
            reader: FlatReader::<Kb>::new(alloc, 0),
        }
    }
}

impl Pair for InMemory {
    fn write(&self, sample: &Kb) {
        self.writer.write(sample).expect("write");
    }

    fn read(&self) -> Kb {
        self.reader.read().expect("read").expect("sample")
    }
}

/// The owner's mapping of a POSIX segment writes, a second mapping of the
/// same segment reads, as a reader process would.
#[cfg(all(unix, feature = "posix-mmap"))]
struct Posix {
    owner: PosixSlotAllocator,
    reader: PosixSlotAllocator,
}

#[cfg(all(unix, feature = "posix-mmap"))]
impl Posix {
    fn new(test: &str) -> Self {
        let flink = std::env::temp_dir().join(format!(
            "zerodds-flatdata-perf-{}-{test}",
            std::process::id()
        ));
        let owner = PosixSlotAllocator::create(&flink, 4, 1024).expect("create");
        let reader = PosixSlotAllocator::attach(&flink).expect("attach");
        Self { owner, reader }
    }
}

#[cfg(all(unix, feature = "posix-mmap"))]
impl Pair for Posix {
    fn write(&self, sample: &Kb) {
        let slot = self.owner.reserve_slot(0b1).expect("reserve");
        self.owner
            .commit_slot(slot, sample.as_bytes())
            .expect("commit");
    }

    fn read(&self) -> Kb {
        let slot = self
            .reader
            .next_unread_slot(0)
            .expect("scan")
            .expect("sample");
        let (ptr, len) = self.reader.slot_read_ptr(slot).expect("slot");
        let mut out = Kb { bytes: [0; 1024] };
        // SAFETY: `ptr` points at `len` committed bytes in the reader's
        // mapping, which stay put until `mark_read` frees the slot.
        unsafe { std::ptr::copy_nonoverlapping(ptr, out.bytes.as_mut_ptr(), len.min(1024)) };
        self.reader.mark_read(slot, 0).expect("mark");
        out
    }
}

fn warm_up(pair: &impl Pair, sample: &Kb) {
    for _ in 0..WARMUP {
        pair.write(sample);
        let _ = pair.read();
    }
}

const SAMPLE: Kb = Kb {
    bytes: [0x5A; 1024],
};

/// §11.1 — same-host P99 below 5 µs for a 1 kB sample.
fn p99_latency_meets_target(pair: &impl Pair, backend: &str) {
    warm_up(pair, &SAMPLE);
    let mut lat = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        pair.write(&SAMPLE);
        let got = pair.read();
        lat.push(start.elapsed());
        assert_eq!(got.bytes[0], 0x5A);
    }
    lat.sort_unstable();
    let p99 = percentile(&lat, 99);
    // 5 µs is the spec figure for an optimised build; a debug build is
    // given five times that.
    let budget = if debug_build() {
        Duration::from_micros(25)
    } else {
        Duration::from_micros(5)
    };
    eprintln!("§11.1 {backend} same-host round trip 1 kB: P99 = {p99:?} (budget {budget:?})");
    assert!(
        p99 <= budget,
        "§11.1 {backend}: P99 same-host round trip for 1 kB was {p99:?}, budget {budget:?}"
    );
}

/// §11.2 — roughly 1 GB/s per writer, memcpy-bound.
fn throughput_meets_target(pair: &impl Pair, backend: &str) {
    warm_up(pair, &SAMPLE);
    // Timed in batches of BATCH rounds, gated on the fastest batch. What
    // the host adds to a round — the time slices of other threads, a busy
    // SMT sibling, a cold cache line — only ever adds time, so the fastest
    // batch is the path's own rate and a slower path slows it too. One
    // span over all rounds and the median batch both move with the host:
    // on the Linux test host at a load of 67 on 16 cores the median batch measured
    // 378–575 MB/s over 12 debug runs of an unchanged path, and once fell
    // below the floor.
    let mut batches = Vec::with_capacity(SAMPLES / BATCH);
    for _ in 0..SAMPLES / BATCH {
        let start = Instant::now();
        for _ in 0..BATCH {
            pair.write(&SAMPLE);
            let _ = pair.read();
        }
        batches.push(start.elapsed());
    }
    batches.sort_unstable();
    #[allow(clippy::cast_precision_loss)]
    let rate = |batch: Duration| (BATCH * 1024) as f64 / batch.as_secs_f64();
    let bytes_per_s = rate(batches[0]);
    let median_bytes_per_s = rate(percentile(&batches, 50));
    // 1 GB/s in the release profile; a quarter of it in a debug build.
    let floor = if debug_build() { 250e6 } else { 1e9 };
    eprintln!(
        "§11.2 {backend} writer throughput 1 kB samples: {:.2} MB/s, median batch {:.2} MB/s (floor {:.2} MB/s)",
        bytes_per_s / 1e6,
        median_bytes_per_s / 1e6,
        floor / 1e6
    );
    assert!(
        bytes_per_s >= floor,
        "§11.2 {backend}: writer throughput was {:.2} MB/s, floor {:.2} MB/s",
        bytes_per_s / 1e6,
        floor / 1e6
    );
}

/// §11.4 — same-host publish latency below a fifth of the UDP publish
/// latency.
///
/// The spec supplies both sides of that comparison, so neither number
/// below is chosen here:
///
/// * the UDP side — Motivation of `docs/specs/zerodds-flatdata-1.0.md`:
///   "Same-Host Pub/Sub kostet ueber UDP/RTPS heute ~30 µs Latenz pro
///   1 kB Sample (Loopback-Roundtrip + Encap-Header + 2x Memcpy)", which
///   §11 repeats as the reference of the whole section ("gegen
///   UDP-Loopback ~30 µs");
/// * the factor — §11: "**Same-Host-Pub-Latenz < UDP-Pub-Latenz / 5**".
///
/// 30 µs / 5 = 6 µs, the budget asserted below.
///
/// The budget is absolute on purpose. The earlier form of this test
/// divided by a UDP socket measured in the same process, which makes the
/// gate a property of the host's `sendto` cost rather than of the code
/// under test: five runs of the unchanged binary on one dev machine
/// (aarch64 macOS, release, 2 000 rounds) produced UDP medians of
/// 3.375 / 3.541 / 4.458 / 5.125 / 5.791 µs — a spread of 71 % — while
/// the publish median stayed at 83 ns across all five. On the CI
/// container the same `sendto` measured 1.704 µs, and the ratio gate
/// went red at an unchanged publish path.
///
/// The live side-by-side measurement is not dropped, it moves to where
/// measuring is done properly: the `flat_vs_udp_publish_1kb` group in
/// `benches/loopback.rs` runs the same 1 kB publish over the slot path
/// and over a loopback UDP socket under criterion (warm-up, batched
/// timing, outlier report), and the `bench-main`/`bench-compare` CI jobs
/// compare it against the previous baseline of the SAME host — the only
/// comparison a ratio of two host-bound latencies can carry.
fn publish_beats_a_fifth_of_udp_publish(pair: &impl Pair, backend: &str) {
    // Untimed warm-up — the slot pool, its page mapping and the branch
    // predictor are cold on the first writes. The earlier form of this
    // test had no warm-up at all.
    warm_up(pair, &SAMPLE);

    let mut publish = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        pair.write(&SAMPLE);
        publish.push(start.elapsed());
        // Drained outside the timed span: the read only recycles the slot
        // for the next round, it is not part of the publish §11.4 bounds.
        let _ = pair.read();
    }

    publish.sort_unstable();
    let p50 = percentile(&publish, 50);
    let p99 = percentile(&publish, 99);
    // 30 µs (spec) / 5 (spec) = 6 µs. No separate debug budget: the debug
    // profile measures a 292 ns median on the dev machine and 481 ns on
    // the CI container, both a factor of 12 or more below the bound, so a
    // profile split would only widen an already unreached budget.
    let budget = Duration::from_micros(6);
    // Gated on the median. `tools/bench-suite/src/bin/regression.rs`
    // records the reason for that choice from this project's own CI
    // hosts: on shared, loaded runners the p50 is stable to ±0.5 %, while
    // single-sample extremes vary about 3x run to run. The p99 is
    // reported as an info column; the tail of this path is gated by
    // §11.1, whose 5 µs P99 covers this publish plus the matching read.
    eprintln!(
        "§11.4 {backend} same-host publish 1 kB: p50 = {p50:?}, p99 = {p99:?} (budget {budget:?})"
    );
    assert!(
        p50 < budget,
        "§11.4 {backend}: same-host publish median was {p50:?}, budget {budget:?} \
         (spec: UDP publish ~30 µs / 5)"
    );
}

#[test]
fn same_host_p99_latency_meets_target() {
    p99_latency_meets_target(&InMemory::new(), "in-memory");
}

#[test]
fn write_throughput_meets_target() {
    throughput_meets_target(&InMemory::new(), "in-memory");
}

#[test]
fn same_host_publish_beats_a_fifth_of_udp_publish() {
    publish_beats_a_fifth_of_udp_publish(&InMemory::new(), "in-memory");
}

#[cfg(all(unix, feature = "posix-mmap"))]
#[test]
fn posix_same_host_p99_latency_meets_target() {
    p99_latency_meets_target(&Posix::new("p99"), "posix");
}

#[cfg(all(unix, feature = "posix-mmap"))]
#[test]
fn posix_write_throughput_meets_target() {
    throughput_meets_target(&Posix::new("throughput"), "posix");
}

#[cfg(all(unix, feature = "posix-mmap"))]
#[test]
fn posix_same_host_publish_beats_a_fifth_of_udp_publish() {
    publish_beats_a_fifth_of_udp_publish(&Posix::new("publish"), "posix");
}
