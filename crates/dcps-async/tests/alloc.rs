// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec §9.3 — heap-allocation overhead of `write().await` vs sync `write`.
//!
//! Target: the async write path adds **no extra heap allocations** over the sync
//! path on the hot loop. Measured with `dhat` as the global allocator: N sync
//! writes and N async writes (the whole async loop under a single `block_on`, so
//! runtime setup is amortized), comparing the per-write allocation delta. Offline
//! participants — a `write` encodes + caches without any network, so the
//! allocation profile is the pure API cost.

#![allow(clippy::expect_used, clippy::print_stderr, missing_docs)]

use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_dcps_async::AsyncDomainParticipantFactory;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const N: u64 = 200;

#[test]
fn async_write_adds_no_extra_heap_vs_sync() {
    let _profiler = dhat::Profiler::builder().testing().build();

    // --- sync writer (offline) ---
    let sf = DomainParticipantFactory::instance();
    let sp = sf.create_participant_offline(0, DomainParticipantQos::default());
    let st = sp
        .create_topic::<RawBytes>("AllocSync", TopicQos::default())
        .expect("topic");
    let sw = sp
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&st, DataWriterQos::default())
        .expect("writer");

    // --- async writer (offline) ---
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let af = AsyncDomainParticipantFactory::instance();
    let ap = af.create_participant_offline(1);
    let at = ap
        .create_topic::<RawBytes>("AllocAsync", TopicQos::default())
        .expect("topic");
    let aw = ap
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&at, DataWriterQos::default())
        .expect("writer");

    let payload = RawBytes::new(vec![0xAA; 32]);

    // Warm up both paths (first write may lazily allocate caches) before the
    // measured loops, so we compare steady-state per-write cost.
    sw.write(&payload).expect("warm sync");
    rt.block_on(async { aw.write(&payload).await.expect("warm async") });

    let s0 = dhat::HeapStats::get();
    for _ in 0..N {
        sw.write(&payload).expect("sync write");
    }
    let s1 = dhat::HeapStats::get();
    rt.block_on(async {
        for _ in 0..N {
            aw.write(&payload).await.expect("async write");
        }
    });
    let s2 = dhat::HeapStats::get();

    let sync_blocks = s1.total_blocks - s0.total_blocks;
    let async_blocks = s2.total_blocks - s1.total_blocks;
    eprintln!(
        "§9.3 alloc/write: sync={:.2} async={:.2} (N={N}, sync_total={sync_blocks} async_total={async_blocks})",
        sync_blocks as f64 / N as f64,
        async_blocks as f64 / N as f64,
    );

    assert!(
        async_blocks <= sync_blocks,
        "async write must not allocate more than sync (async={async_blocks}, sync={sync_blocks} over {N} writes)"
    );
}
