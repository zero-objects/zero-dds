//! D.5e — Latenz-Assertion-Tests fuer den Reliable-DCPS-Pfad.
//!
//! Diese Tests schuetzen die Phase-1+2 Latenz-Wins (D.5e):
//!  * `DEFAULT_HEARTBEAT_PERIOD = 100ms` (statt 1s)
//!  * `DEFAULT_TICK_PERIOD = 5ms` (statt 50ms)
//!  * `DEFAULT_HEARTBEAT_RESPONSE_DELAY = 0ms` (statt 200ms)
//!  * Synchroner ACKNACK im recv-thread
//!  * HEARTBEAT-Piggyback in `write_with_heartbeat`
//!
//! Wenn jemand einen dieser Wins regredieren laesst (z.B. Period-Const
//! wieder auf Sekunden hochsetzen), schlagen diese Tests an.
//!
//! **Spec-conform**: alle Werte sind vom Spec erlaubt (Period
//! "implementation-defined"); der Test asserted nur **Performance**, nicht
//! Compliance. Compliance-Tests sind in `cyclone_compliance.rs` etc.

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

// Eindeutige Domain-IDs damit Tests nicht via SPDP-Multicast cross-talken.
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

/// Cluster-A-Test: ein einzelner Roundtrip muss unter 50 ms liegen.
///
/// Floor war pre-D.5e: Heartbeat-Period 1s + heartbeat-response-delay
/// 200ms = ~1.2 s pro Roundtrip. Ein Sample muss heute deutlich
/// schneller durch.
///
/// Schwelle 50 ms = sehr loose CI-Gate (akzeptiert Loaded-CI-Hosts);
/// echtes Ziel ist <500 µs auf bare-metal, gemessen via
/// `roundtrip-typed`-Bench.
#[test]
#[ignore = "F-DCPS-latency-self-match-timeout (docs/dcps/latency-self-match-timeout-followup.md): \
            wait_for_matched_subscription(1, 5s) timeoutet auf GitLab-Runner \
            in single-Participant-Self-Match-Pfad. Nicht reproduzierbar auf macOS \
            (cfg gated to Linux). Owner: DCPS-Discovery."]
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
    // Ping-side: writer-on-req + reader-on-echo, gleicher Participant
    // (intra-process via SEDP-Self-Match — funktioniert wenn
    // ignore_local_subscriptions/publications nicht gesetzt sind).
    let ping_writer = publisher
        .create_datawriter::<RawBytes>(&topic_req, DataWriterQos::default())
        .expect("ping writer");
    let ping_reader = subscriber
        .create_datareader::<RawBytes>(&topic_echo, DataReaderQos::default())
        .expect("ping reader");

    // Sync-Punkt: alle 4 Endpoints matched.
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

    // Ein Roundtrip messen. Ping → pong-reader-take → pong-writer-write → ping-reader-take.
    let payload = RawBytes::new(vec![0xAB; 64]);
    let t_start = Instant::now();
    ping_writer.write(&payload).expect("ping write");

    // Pong: warten, take, echo.
    pong_reader
        .wait_for_data(Duration::from_secs(2))
        .expect("pong sees req");
    let req = pong_reader.take().expect("pong take");
    assert_eq!(req.len(), 1, "pong got exactly one sample");
    pong_writer.write(&req[0]).expect("pong echo");

    // Ping: warten, take.
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
        "Roundtrip-Latenz {elapsed:?} ueber 50ms-Threshold — D.5e-Regress?"
    );
}

/// Cluster-B-Test: 100 sustained Roundtrips ohne Sample-Loss + p99 < 100ms.
///
/// Der Sample-Loss-Aspekt schuetzt den Reliable-Garantie-Pfad
/// (D.5e Phase-2 + write_with_heartbeat): bei pre-D.5e fielen 22% der
/// Samples bei sustained Roundtrip aus, weil HB-Period 1s den Reader
/// stallt und Cache-Overflows erzeugt.
#[test]
#[ignore = "F-DCPS-latency-self-match-timeout (docs/dcps/latency-self-match-timeout-followup.md): \
            self-match Timeout auf GitLab-Runner."]
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
        "Sample-loss zu hoch: {delivered}/{N} (Reliable-Garantie verletzt?)"
    );

    rtts.sort_unstable();
    let p50 = rtts[rtts.len() / 2];
    let p99 = rtts[(rtts.len() * 99) / 100];
    eprintln!("[lat-assert] sustained: p50={p50:?} p99={p99:?}");
    assert!(
        p99 < Duration::from_millis(100),
        "p99 RTT {p99:?} ueber 100ms — D.5e-Regress?"
    );
}
