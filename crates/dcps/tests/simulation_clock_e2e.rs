// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A4 — simulation-safe defaults (no `/clock` gap jitter).
//!
//! ROS-2 painpoint (ros.discourse #54553): Fast DDS' default config produces
//! ~0.5 s gaps + jitter on a high-rate best-effort `/clock`-style stream, so sim
//! users need app-level guards. The root cause is Fast DDS' async writer + flow
//! controller batching/delaying samples.
//!
//! ZeroDDS has no such default: the writer publishes synchronously and delivery
//! is event-driven (futex wake, never a sleep-poll), so a high-rate best-effort
//! stream arrives promptly with no protocol-induced stalls — sim-safe out of the
//! box, no profile needed.
//!
//! This test mimics a sim clock: a best-effort writer emits at ~200 Hz for ~2 s;
//! the subscriber records arrival times and asserts (a) it receives the large
//! majority and (b) the worst inter-arrival gap stays far below the Fast DDS
//! 0.5 s pathology.
//!
//! macOS: ignored — multicast loopback is unreliable (same as the other
//! same-host SPDP e2e tests); runs on Linux / the CI bench.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::GuidPrefix;

const HZ: u64 = 200;
const SECS: u64 = 2;
const TOTAL: u64 = HZ * SECS;
/// Worst tolerated inter-arrival gap. Generous vs scheduling slack on a loaded
/// CI box, but an order of magnitude under the Fast DDS 0.5 s clock-gap.
const MAX_GAP: Duration = Duration::from_millis(150);

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: false, // best-effort, like /clock
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

fn reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: false,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        partition: vec![],
        user_data: vec![],
        group_data: vec![],
        topic_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn best_effort_clock_stream_has_no_gap_jitter() {
    let domain = 93;
    let host = [0x4Bu8; 4];
    let mk_prefix = |tag: u8| {
        let mut p = [tag; 12];
        p[..4].copy_from_slice(&host);
        GuidPrefix::from_bytes(p)
    };
    let rt_pub = DcpsRuntime::start(domain, mk_prefix(0xC1), RuntimeConfig::default()).unwrap();
    let rt_sub = DcpsRuntime::start(domain, mk_prefix(0xC2), RuntimeConfig::default()).unwrap();

    let topic = "A4SimClock";
    let w = rt_pub.register_user_writer(writer_cfg(topic)).unwrap();
    let (r, rx) = rt_sub.register_user_reader(reader_cfg(topic)).unwrap();

    // Wait for the bidirectional SEDP match so best-effort samples are not lost
    // purely to a not-yet-wired endpoint.
    let mdl = Instant::now() + Duration::from_secs(15);
    while Instant::now() < mdl
        && (rt_sub.user_reader_matched_count(r) < 1 || rt_pub.user_writer_matched_count(w) < 1)
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        rt_sub.user_reader_matched_count(r) >= 1 && rt_pub.user_writer_matched_count(w) >= 1,
        "reader/writer did not match before the clock burst"
    );

    // Subscriber thread: timestamp every arrival.
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_rx = Arc::clone(&stop);
    let sub = std::thread::spawn(move || {
        let mut arrivals: Vec<Instant> = Vec::with_capacity(TOTAL as usize);
        while !stop_rx.load(std::sync::atomic::Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(UserSample::Alive { .. }) => arrivals.push(Instant::now()),
                Ok(_) => {}
                Err(_) => {}
            }
        }
        // Drain any stragglers.
        while let Ok(s) = rx.try_recv() {
            if matches!(s, UserSample::Alive { .. }) {
                arrivals.push(Instant::now());
            }
        }
        arrivals
    });

    // Publisher: ~200 Hz best-effort for SECS seconds.
    let period = Duration::from_nanos(1_000_000_000 / HZ);
    for i in 0..TOTAL {
        let t = (i as u32).to_le_bytes();
        rt_pub
            .write_user_sample(w, t.to_vec())
            .expect("clock write");
        std::thread::sleep(period);
    }
    // Let the tail drain, then stop the subscriber.
    std::thread::sleep(Duration::from_millis(300));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let arrivals = sub.join().expect("subscriber thread");

    // (a) best-effort over loopback should land the large majority.
    let received = arrivals.len() as u64;
    assert!(
        received >= TOTAL / 2,
        "received only {received}/{TOTAL} clock ticks — best-effort loopback should keep most"
    );

    // (b) the worst inter-arrival gap must be far below Fast DDS' 0.5 s pathology.
    let mut worst = Duration::ZERO;
    for w2 in arrivals.windows(2) {
        let gap = w2[1].duration_since(w2[0]);
        if gap > worst {
            worst = gap;
        }
    }
    assert!(
        worst < MAX_GAP,
        "worst inter-arrival gap {worst:?} exceeds {MAX_GAP:?} (Fast DDS-style clock stall); \
         received {received}/{TOTAL}"
    );
}
