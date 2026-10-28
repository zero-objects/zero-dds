// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-async-1.0 §9.1 — `write().await` no more than 5 % slower than
//! the sync `write()`.
//!
//! `benches/write_async_vs_sync.rs` reports the two numbers; this turns
//! the comparison into a gate. §9 measures the optimized build, and so does
//! this: under `debug_assertions` it is ignored, and the CI job `perf-gate`
//! runs it with `--release`. In the instrumented debug build of the `test`
//! job it measured the instrumentation of an async state machine against a
//! sync call, not the API's cost.
//!
//! What is compared is one writer — the async writer's own sync writer and
//! its `write().await` — of a live participant on a leased domain with no
//! reader, so both go the whole DCPS write path and send nothing. The async
//! side is driven on a current-thread runtime so no work-stealing shows up
//! in the figure, and the two batches swap places every round.
//!
//! The gate used to compare two offline writers of two participants, whose
//! writes only fill a test queue (78 ns per write on the Linux test host)
//! and whose queues grew without bound. Two sync writers measured that way
//! differed by as much as the overhead it was looking for (136–146 µs
//! against 143–145 µs per 1000 writes). And against 78 ns any `.await`
//! is 7 %: an `async` block around the sync write cost 5.2–5.9 ns more than
//! the write, `AsyncDataWriter::write` 5.8–6.4 ns, so what that ratio
//! measured was the `.await`, not this API.
//!
//! The per-batch minimum is compared: a minimum is the least noisy
//! statistic available here.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    missing_docs
)]

use std::time::{Duration, Instant};

use zerodds_dcps::{DataWriterQos, PublisherQos, RawBytes, TopicQos};
use zerodds_dcps_async::AsyncDomainParticipantFactory;

const BATCHES: usize = 40;
const PER_BATCH: usize = 1_000;

/// Writer QoS for the measurement: limits that a batch cannot reach, so
/// what is measured is the write path and never the `OutOfResources` one.
fn roomy_writer_qos() -> DataWriterQos {
    DataWriterQos {
        resource_limits: zerodds_qos::ResourceLimitsQosPolicy {
            max_samples: i32::MAX,
            max_instances: i32::MAX,
            max_samples_per_instance: i32::MAX,
        },
        ..DataWriterQos::default()
    }
}

/// §9.1 — the async write must not cost more than 5 % over the sync one.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "§9 measures the optimized build; `perf-gate` runs this with --release"
)]
fn async_write_is_within_five_percent_of_sync_write() {
    let payload = RawBytes::new(vec![0xAA; 64]);

    let lease = zerodds_test_domains::lease_domain();
    let participant = AsyncDomainParticipantFactory::instance()
        .create_participant(lease.id())
        .expect("participant");
    let topic = participant
        .create_topic::<RawBytes>("PerfWrite", TopicQos::default())
        .expect("topic");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, roomy_writer_qos())
        .expect("writer");
    let sync_writer = writer.as_sync();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");

    let sync_batch = || {
        let t = Instant::now();
        for _ in 0..PER_BATCH {
            sync_writer.write(&payload).expect("write");
        }
        t.elapsed()
    };
    let async_batch = || {
        let t = Instant::now();
        rt.block_on(async {
            for _ in 0..PER_BATCH {
                writer.write(&payload).await.expect("write");
            }
        });
        t.elapsed()
    };

    // Warm-up: first-touch allocation must not land in a measured batch.
    let _ = sync_batch();
    let _ = async_batch();

    let mut sync_best = Duration::MAX;
    let mut async_best = Duration::MAX;
    for round in 0..BATCHES {
        if round % 2 == 0 {
            sync_best = sync_best.min(sync_batch());
            async_best = async_best.min(async_batch());
        } else {
            async_best = async_best.min(async_batch());
            sync_best = sync_best.min(sync_batch());
        }
    }

    let ratio = async_best.as_secs_f64() / sync_best.as_secs_f64();
    eprintln!(
        "§9.1 write: sync {sync_best:?} / async {async_best:?} per {PER_BATCH} writes — ratio {ratio:.3}"
    );
    assert!(
        ratio <= 1.05,
        "§9.1: async write must stay within 5 % of sync write, ratio was {ratio:.3} \
         (sync {sync_best:?}, async {async_best:?})"
    );
}
