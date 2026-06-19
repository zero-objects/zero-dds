// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-async-1.0 §4 — `spawn_in_tokio` e2e (feature `tokio-glue`). Proves
//! the factory can create a live participant whose periodic DDS tick loop runs
//! as a task on a tokio runtime instead of a dedicated `std::thread`: the
//! runtime's `tick_count()` advances over time WITHOUT any internal tick
//! thread, and the participant is fully functional (topic + writer + write).

#![cfg(feature = "tokio-glue")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use zerodds_dcps::{DdsType, RawBytes};
use zerodds_dcps_async::{AsyncDomainParticipantFactory, DataWriterQos, PublisherQos, TopicQos};

#[tokio::test(flavor = "multi_thread")]
async fn spawn_in_tokio_drives_tick_on_tokio_runtime() {
    let factory = AsyncDomainParticipantFactory::instance();
    let handle = tokio::runtime::Handle::current();

    // Live participant whose tick loop is a tokio task (no zdds-tick thread).
    let p = factory
        .spawn_in_tokio(71, &handle)
        .expect("spawn_in_tokio participant");

    // The tick is driven by tokio: its count must climb across a few periods,
    // proving the spawned task actually runs the iteration body.
    let rt = p.as_sync().runtime().expect("live runtime");
    let before = rt.tick_count();
    tokio::time::sleep(Duration::from_millis(120)).await;
    let after = rt.tick_count();
    assert!(
        after > before,
        "tokio-driven tick must advance ({before} -> {after})"
    );

    // The participant is fully functional — topic + writer + async write.
    let topic = p
        .create_topic::<RawBytes>("SpawnTokioTopic", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    writer
        .write(&RawBytes::new(vec![0xDD, 0x5A]))
        .await
        .expect("async write through tokio-ticked participant");
}

#[tokio::test(flavor = "multi_thread")]
async fn spawn_in_tokio_task_terminates_on_shutdown() {
    let factory = AsyncDomainParticipantFactory::instance();
    let handle = tokio::runtime::Handle::current();
    let p = factory
        .spawn_in_tokio(72, &handle)
        .expect("spawn_in_tokio participant");

    let rt = std::sync::Arc::clone(p.as_sync().runtime().expect("live runtime"));

    // Tick advances while running.
    let a = rt.tick_count();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(
        rt.tick_count() > a,
        "tick should advance while the participant runs"
    );

    // Explicit shutdown flips the stop flag; the tokio task sees `is_stopped()`
    // within one tick period and returns — so the count must freeze. (Without
    // a responsive driver the task would otherwise run until process exit.)
    let rt_for_shutdown = std::sync::Arc::clone(&rt);
    tokio::task::spawn_blocking(move || rt_for_shutdown.shutdown())
        .await
        .expect("shutdown join");

    tokio::time::sleep(Duration::from_millis(40)).await;
    let frozen = rt.tick_count();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        rt.tick_count(),
        frozen,
        "tokio tick task must stop advancing after runtime shutdown"
    );

    drop(p);
}

// `RawBytes` already implements `DdsType`; the import is exercised above.
const _: fn() = || {
    fn _assert_dds_type<T: DdsType>() {}
    _assert_dds_type::<RawBytes>();
};
