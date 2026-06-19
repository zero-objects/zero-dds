// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! D.5e Phase 3 — the event-driven scheduler tick (`RuntimeConfig::scheduler_tick`)
//! drives the full DCPS lifecycle (SPDP/SEDP discovery, reliable delivery,
//! HEARTBEAT/ACKNACK) exactly like the classic fixed-period tick — proving the
//! deadline-heap worker + raise-on-write/recv wiring is functionally
//! equivalent. The per-wake work (`run_tick_iteration`) is unchanged; only the
//! park mechanism differs.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(200);

fn scheduler_config() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(5),
        spdp_period: Duration::from_millis(100),
        scheduler_tick: true, // <-- the event-driven worker under test
        ..RuntimeConfig::default()
    }
}

/// Intra-process reliable roundtrip (ping → pong → ping) driven entirely by the
/// scheduler tick. If the deadline-heap worker did not drive SEDP matching +
/// HEARTBEAT/ACKNACK, the endpoints would never match or the sample would never
/// arrive.
#[test]
fn scheduler_tick_drives_reliable_roundtrip() {
    let domain: i32 = NEXT_DOMAIN
        .fetch_add(1, Ordering::Relaxed)
        .try_into()
        .expect("domain fits i32");
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), scheduler_config())
        .expect("participant");

    let topic_req = p
        .create_topic::<RawBytes>("SchedReq", TopicQos::default())
        .expect("req topic");
    let topic_echo = p
        .create_topic::<RawBytes>("SchedEcho", TopicQos::default())
        .expect("echo topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());

    let pong_writer = publisher
        .create_datawriter::<RawBytes>(&topic_echo, DataWriterQos::default())
        .expect("pong writer");
    let pong_reader = subscriber
        .create_datareader::<RawBytes>(&topic_req, DataReaderQos::default())
        .expect("pong reader");
    let ping_writer = publisher
        .create_datawriter::<RawBytes>(&topic_req, DataWriterQos::default())
        .expect("ping writer");
    let ping_reader = subscriber
        .create_datareader::<RawBytes>(&topic_echo, DataReaderQos::default())
        .expect("ping reader");

    let match_to = Duration::from_secs(10);
    ping_writer
        .wait_for_matched_subscription(1, match_to)
        .expect("ping writer matches (scheduler drove SEDP)");
    pong_reader
        .wait_for_matched_publication(1, match_to)
        .expect("pong reader matches");
    pong_writer
        .wait_for_matched_subscription(1, match_to)
        .expect("pong writer matches");
    ping_reader
        .wait_for_matched_publication(1, match_to)
        .expect("ping reader matches");

    let payload = RawBytes::new(vec![0xD5; 64]);
    let t0 = Instant::now();
    ping_writer.write(&payload).expect("ping write");

    pong_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("pong sees req (scheduler drove delivery)");
    let req = pong_reader.take().expect("pong take");
    assert_eq!(req.len(), 1);
    pong_writer.write(&req[0]).expect("pong echo");

    ping_reader
        .wait_for_data(Duration::from_secs(5))
        .expect("ping sees echo");
    let echo = ping_reader.take().expect("ping take");
    let elapsed = t0.elapsed();
    assert_eq!(echo.len(), 1);
    assert_eq!(echo[0].data, payload.data, "echo payload-stable");
    eprintln!("[sched-tick] roundtrip elapsed = {elapsed:?}");
    // Loose CI gate: must complete promptly (raise-on-recv → no multi-second
    // stalls). The bench measures the real latency.
    assert!(
        elapsed < Duration::from_secs(2),
        "scheduler-tick roundtrip {elapsed:?} too slow — event-driven wake regressed?"
    );
}

/// The concrete idle-CPU win: a discovery-only participant (no user endpoints)
/// under the scheduler tick runs `run_tick_iteration` far less often than the
/// fixed 5 ms poll — it parks until the next SPDP announce / idle floor instead
/// of spinning every 5 ms.
#[test]
fn scheduler_tick_idle_participant_parks_long() {
    let factory = DomainParticipantFactory::instance();

    // Baseline participant: classic fixed 5 ms tick_loop → ~200 iterations/s.
    // `scheduler_tick` defaults to true since D.5e Phase 3 C, so force it off
    // here to measure the fixed-period path this test compares against.
    let d_dom: i32 = NEXT_DOMAIN
        .fetch_add(1, Ordering::Relaxed)
        .try_into()
        .unwrap();
    let p_default = factory
        .create_participant_with_config(
            d_dom,
            DomainParticipantQos::default(),
            RuntimeConfig {
                tick_period: Duration::from_millis(5),
                spdp_period: Duration::from_millis(100),
                scheduler_tick: false,
                ..RuntimeConfig::default()
            },
        )
        .expect("baseline participant");

    // Scheduler participant: parks until SPDP/idle floor → far fewer iterations.
    let s_dom: i32 = NEXT_DOMAIN
        .fetch_add(1, Ordering::Relaxed)
        .try_into()
        .unwrap();
    let p_sched = factory
        .create_participant_with_config(s_dom, DomainParticipantQos::default(), scheduler_config())
        .expect("scheduler participant");

    let d_rt = p_default.runtime().expect("default rt").clone();
    let s_rt = p_sched.runtime().expect("sched rt").clone();

    // Settle, then measure tick iterations over a 1 s window.
    std::thread::sleep(Duration::from_millis(300));
    let d0 = d_rt.tick_count();
    let s0 = s_rt.tick_count();
    std::thread::sleep(Duration::from_secs(1));
    let d_ticks = d_rt.tick_count() - d0;
    let s_ticks = s_rt.tick_count() - s0;
    eprintln!("[sched-tick] idle 1s: default={d_ticks} ticks, scheduler={s_ticks} ticks");

    // Default ≈ 200/s (1000/5 ms). Scheduler ≈ SPDP rate (10/s @ 100 ms) + idle
    // floor — at least 4× fewer. (No endpoints → no HB/ACKNACK fine cadence.)
    assert!(d_ticks >= 100, "default should poll ~200/s, saw {d_ticks}");
    assert!(
        s_ticks * 4 < d_ticks,
        "scheduler idle ticks {s_ticks} must be far below default {d_ticks} (idle-CPU win)"
    );
}

/// Sustained delivery under the scheduler tick — 50 samples, all received in
/// order, no loss (proves the worker keeps re-arming + the raise coalescing
/// does not drop work).
#[test]
fn scheduler_tick_sustained_no_loss() {
    let domain: i32 = NEXT_DOMAIN
        .fetch_add(1, Ordering::Relaxed)
        .try_into()
        .expect("domain fits i32");
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), scheduler_config())
        .expect("participant");
    let topic = p
        .create_topic::<RawBytes>("SchedStream", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, Duration::from_secs(10))
        .expect("match");

    const N: usize = 50;
    let mut received = Vec::new();
    for i in 0..N {
        writer
            .write(&RawBytes::new(vec![i as u8; 16]))
            .expect("write");
        if reader.wait_for_data(Duration::from_secs(2)).is_ok() {
            for s in reader.take().expect("take") {
                received.push(s.data[0]);
            }
        }
    }
    // Drain any stragglers.
    let deadline = Instant::now() + Duration::from_secs(2);
    while received.len() < N && Instant::now() < deadline {
        if reader.wait_for_data(Duration::from_millis(200)).is_ok() {
            for s in reader.take().expect("take") {
                received.push(s.data[0]);
            }
        }
    }
    assert_eq!(
        received.len(),
        N,
        "all {N} samples delivered under scheduler tick"
    );
    for (i, v) in received.iter().enumerate() {
        assert_eq!(*v, i as u8, "in-order, no loss at {i}");
    }
}
