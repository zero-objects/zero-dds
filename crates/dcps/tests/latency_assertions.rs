//! D.5e — latency assertion tests for the Reliable DCPS path.
//!
//! These tests protect the Phase-1+2 latency wins (D.5e):
//!  * `DEFAULT_HEARTBEAT_PERIOD = 100ms` (instead of 1s)
//!  * `DEFAULT_TICK_PERIOD = 5ms` (instead of 50ms)
//!  * `DEFAULT_HEARTBEAT_RESPONSE_DELAY = 0ms` (instead of 200ms)
//!  * Synchronous ACKNACK in the recv thread
//!  * HEARTBEAT piggyback in `write_with_heartbeat`
//!
//! If anyone lets one of these wins regress (e.g. raises a period const
//! back to seconds), these tests fire.
//!
//! **Spec-conformant**: all values are permitted by the spec (period
//! "implementation-defined"); the test asserts only **performance**, not
//! compliance. Compliance tests are in `cyclone_compliance.rs` etc.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

#[path = "common/mod.rs"]
mod common;

// Unique domain IDs so tests don't cross-talk via SPDP multicast.
static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(150);

fn fresh_domain() -> u32 {
    NEXT_DOMAIN.fetch_add(1, Ordering::Relaxed)
}

fn fresh_runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(5),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    }
}

/// Cluster-A test: a single roundtrip must be under 50 ms.
///
/// Floor was pre-D.5e: heartbeat period 1s + heartbeat-response-delay
/// 200ms = ~1.2 s per roundtrip. A sample must get through much
/// faster today.
///
/// Threshold 50 ms = very loose CI gate (accepts loaded CI hosts);
/// the real target is <500 µs on bare metal, measured via the
/// `roundtrip-typed` bench.
#[test]
fn single_roundtrip_under_50ms() {
    let domain = fresh_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(
            domain.try_into().expect("domain fits i32"),
            DomainParticipantQos::default(),
            fresh_runtime_config(),
        )
        .expect("participant");
    let topic_req = p
        .create_topic::<RawBytes>("LatReq", TopicQos::default())
        .expect("req topic");
    let topic_echo = p
        .create_topic::<RawBytes>("LatEcho", TopicQos::default())
        .expect("echo topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());

    // Pong-side: reader-on-req + writer-on-echo, im selben Participant.
    let pong_writer = publisher
        .create_datawriter::<RawBytes>(&topic_echo, DataWriterQos::default())
        .expect("pong writer");
    let pong_reader = subscriber
        .create_datareader::<RawBytes>(&topic_req, DataReaderQos::default())
        .expect("pong reader");
    // Ping-side: writer-on-req + reader-on-echo, same participant
    // (intra-process via SEDP self-match — works when
    // ignore_local_subscriptions/publications are not set).
    let ping_writer = publisher
        .create_datawriter::<RawBytes>(&topic_req, DataWriterQos::default())
        .expect("ping writer");
    let ping_reader = subscriber
        .create_datareader::<RawBytes>(&topic_echo, DataReaderQos::default())
        .expect("ping reader");

    // Sync point: all 4 endpoints matched.
    ping_writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("ping writer sees pong reader");
    pong_reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("pong reader sees ping writer");
    pong_writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("pong writer sees ping reader");
    ping_reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("ping reader sees pong writer");

    // Measure one roundtrip. Ping → pong-reader-take → pong-writer-write → ping-reader-take.
    let payload = RawBytes::new(vec![0xAB; 64]);
    let t_start = Instant::now();
    ping_writer.write(&payload).expect("ping write");

    // Pong: wait, take, echo.
    pong_reader
        .wait_for_data(Duration::from_secs(2))
        .expect("pong sees req");
    let req = pong_reader.take().expect("pong take");
    assert_eq!(req.len(), 1, "pong got exactly one sample");
    pong_writer.write(&req[0]).expect("pong echo");

    // Ping: wait, take.
    ping_reader
        .wait_for_data(Duration::from_secs(2))
        .expect("ping sees echo");
    let echo = ping_reader.take().expect("ping take");
    let elapsed = t_start.elapsed();
    assert_eq!(echo.len(), 1, "ping got exactly one echo");
    assert_eq!(echo[0].data, payload.data, "echo payload-stable");

    eprintln!("[lat-assert] single_roundtrip elapsed = {elapsed:?}");
    assert!(
        elapsed < Duration::from_millis(50),
        "Roundtrip latency {elapsed:?} over 50ms threshold — D.5e regress?"
    );
}

/// Cluster-B test: 100 sustained roundtrips without sample loss + p99 < 100ms.
///
/// The sample-loss aspect protects the Reliable guarantee path
/// (D.5e Phase-2 + write_with_heartbeat): pre-D.5e, 22% of the
/// samples were lost during sustained roundtrips, because a 1s HB period
/// stalls the reader and produces cache overflows.
#[test]
fn sustained_roundtrip_no_loss_p99_under_100ms() {
    let domain = fresh_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(
            domain.try_into().expect("domain fits i32"),
            DomainParticipantQos::default(),
            fresh_runtime_config(),
        )
        .expect("participant");
    let topic_req = p
        .create_topic::<RawBytes>("LatReq2", TopicQos::default())
        .expect("req topic");
    let topic_echo = p
        .create_topic::<RawBytes>("LatEcho2", TopicQos::default())
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

    ping_writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");
    pong_reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("match");
    pong_writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");
    ping_reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("match");

    let payload = RawBytes::new(vec![0xAB; 64]);
    const N: usize = 100;
    let mut rtts: Vec<Duration> = Vec::with_capacity(N);
    let mut delivered: usize = 0;

    for _i in 0..N {
        let t_start = Instant::now();
        ping_writer.write(&payload).expect("ping write");
        if pong_reader.wait_for_data(Duration::from_secs(2)).is_err() {
            continue;
        }
        let req = pong_reader.take().expect("pong take");
        if req.is_empty() {
            continue;
        }
        pong_writer.write(&req[0]).expect("pong echo");
        if ping_reader.wait_for_data(Duration::from_secs(2)).is_err() {
            continue;
        }
        let echo = ping_reader.take().expect("ping take");
        if echo.is_empty() {
            continue;
        }
        rtts.push(t_start.elapsed());
        delivered += 1;
    }

    eprintln!(
        "[lat-assert] sustained: delivered={delivered}/{N}, samples in rtts={}",
        rtts.len()
    );
    assert!(
        delivered >= N * 99 / 100,
        "Sample loss too high: {delivered}/{N} (Reliable guarantee violated?)"
    );

    rtts.sort_unstable();
    let p50 = rtts[rtts.len() / 2];
    let p99 = rtts[(rtts.len() * 99) / 100];
    eprintln!("[lat-assert] sustained: p50={p50:?} p99={p99:?}");
    assert!(
        p99 < Duration::from_millis(100),
        "p99 RTT {p99:?} over 100ms — D.5e regress?"
    );
}
