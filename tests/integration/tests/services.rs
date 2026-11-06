// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two ZeroDDS services as daemons, each between writer and reader
//! processes made through the public DCPS API.
//!
//! **Durability** (`zerodds-durability-svc`, SQLite store). A writer offers
//! PERSISTENT data with a KEEP_ALL durability service history
//! (DDS 1.4 §2.2.3.5), writes `SAMPLES` samples, has them acknowledged and
//! exits. Two readers join later, each RELIABLE and KEEP_ALL, and each must
//! receive every sample, in order:
//!
//! * the first while the daemon that stored them still runs — TRANSIENT
//!   and stronger "are not tied to the lifecycle of the DataWriter"
//!   (DDS 1.4 §2.2.3.4);
//! * the second after the daemon was killed (SIGKILL) and started again on
//!   the same store — PERSISTENT data "outlive a system session"
//!   (§2.2.3.4).
//!
//! The readers wait with `wait_for_historical_data` (§2.2.2.5.2.15), an
//! event, not a delay.
//!
//! **Routing** (`zerodds-router`). Two routes from domain A to domain B:
//! `Readings` as it is, renamed to `ReadingsRouted` on B (the topic
//! renaming of the router's README), and `Temps` through the content
//! filter `v > 50` over a `@final struct { uint32 v; }` (DDS 1.4 Annex B,
//! the SQL subset of filter expressions). The reader on B receives every
//! `Readings` sample in order under the new name, and of `Temps` exactly
//! the values the expression selects, computed here from the expression
//! and the written values.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::time::Duration;

use zerodds_dcps::qos::{DurabilityKind, HistoryKind, ReliabilityKind};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};
use zerodds_integration::{Peer, Scratch, event, lease_domain, role, role_command};

const SAMPLES: u32 = 100;
const STEP: Duration = Duration::from_secs(10);
const DURABLE: &str = "DurableReadings";

fn participant(domain: i32) -> DomainParticipant {
    let cfg = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant")
}

fn writer_qos(durability: DurabilityKind) -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    q.durability.kind = durability;
    // §2.2.3.5: the service keeps what this history says; KEEP_ALL, so
    // every sample, not the default last one.
    q.durability_service.history_kind = HistoryKind::KeepAll;
    q
}

fn reader_qos(durability: DurabilityKind) -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    q.durability.kind = durability;
    q
}

fn env_domain(name: &str) -> i32 {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} unset"))
        .parse()
        .unwrap()
}

fn text(seq: u32) -> Vec<u8> {
    format!("durable-{seq:04}").into_bytes()
}

// ---------------------------------------------------------------------
// Durability
// ---------------------------------------------------------------------

fn durable_writer() {
    let p = participant(env_domain("IT_DOMAIN"));
    let t = p
        .create_topic::<RawBytes>(DURABLE, TopicQos::default())
        .unwrap();
    let w = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&t, writer_qos(DurabilityKind::Persistent))
        .unwrap();
    // The daemon's reader (auto-discovery of a PERSISTENT writer).
    w.wait_for_matched_subscription(1, STEP)
        .expect("the durability service matches the writer");
    for seq in 0..SAMPLES {
        w.write(&RawBytes::new(text(seq))).unwrap();
    }
    w.wait_for_acknowledgments(STEP)
        .expect("the durability service acknowledges every sample");
    event("acknowledged");
    // The writer ends here: what follows comes from the service.
}

fn late_reader() {
    let durability = match std::env::var("IT_DURABILITY").as_deref() {
        Ok("TRANSIENT") => DurabilityKind::Transient,
        _ => DurabilityKind::Persistent,
    };
    let p = participant(env_domain("IT_DOMAIN"));
    let t = p
        .create_topic::<RawBytes>(DURABLE, TopicQos::default())
        .unwrap();
    let r = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&t, reader_qos(durability))
        .unwrap();
    r.wait_for_matched_publication(1, STEP)
        .expect("the durability service's writer matches the late reader");
    event("matched");
    r.wait_for_historical_data(STEP)
        .expect("historical data arrives");
    event("historical");
    let deadline = std::time::Instant::now() + STEP;
    let mut got = 0;
    while got < SAMPLES {
        for s in r.take_with_info().unwrap() {
            if s.info.valid_data {
                got += 1;
                event(format!(
                    "sample text={}",
                    String::from_utf8_lossy(&s.data.data)
                ));
            }
        }
        if got >= SAMPLES {
            break;
        }
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if r.wait_for_data(left).is_err() {
            event(format!("incomplete got={got}"));
            break;
        }
    }
    event("done");
}

fn durability_daemon(domain: i32, store: &std::path::Path, label: &str) -> Peer {
    let mut c = zerodds_integration::daemon_command("ZERODDS_DURABILITY_SVC");
    c.args([
        "--domain",
        &domain.to_string(),
        "--store",
        "sqlite",
        "--path",
    ])
    .arg(store)
    .args(["--auto", "--topic", DURABLE]);
    let mut daemon = Peer::spawn(label, c);
    daemon.wait_for("its ready line", STEP, |l| l.text.starts_with("running"));
    daemon
}

fn check_late_reader(reader: Peer, label: &str) {
    let status_and_lines = {
        let mut reader = reader;
        reader.wait_for_event("done", STEP);
        let lines: Vec<String> = reader.lines().iter().map(|l| l.text.clone()).collect();
        (reader.wait_exit(STEP), lines)
    };
    let (status, lines) = status_and_lines;
    assert!(status.success(), "{label}: reader role failed");
    let texts: Vec<String> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT sample text="))
        .map(str::to_owned)
        .collect();
    let expected: Vec<String> = (0..SAMPLES)
        .map(|s| String::from_utf8(text(s)).unwrap())
        .collect();
    assert_eq!(
        texts, expected,
        "{label}: every PERSISTENT sample, once, in order (DDS 1.4 §2.2.3.4, §2.2.3.5)"
    );
}

fn durability_scenario() {
    let lease = lease_domain();
    let domain = lease.id();
    let scratch = Scratch::new("durability");
    let store = scratch.path().join("store.sqlite");
    let start = |name: &str, durability: &str| {
        let mut c = role_command("durability_outlives_writer_and_daemon", name);
        c.env("IT_DOMAIN", domain.to_string())
            .env("IT_DURABILITY", durability);
        Peer::spawn(name, c)
    };

    let first = durability_daemon(domain, &store, "daemon-1");
    let mut writer = start("writer", "PERSISTENT");
    writer.wait_for_event("acknowledged", STEP);
    assert!(writer.wait_exit(STEP).success(), "writer role failed");

    check_late_reader(start("reader", "TRANSIENT"), "late reader, writer gone");

    // A crash, not a shutdown: the store is all that is left.
    let _ = first.stop();
    let second = durability_daemon(domain, &store, "daemon-2");
    check_late_reader(
        start("reader", "PERSISTENT"),
        "late reader, daemon restarted",
    );
    let _ = second.stop();
    drop(lease);
}

#[test]
fn durability_outlives_writer_and_daemon() {
    match role().as_deref() {
        Some("writer") => durable_writer(),
        Some("reader") => late_reader(),
        Some(other) => panic!("unknown role {other}"),
        None => durability_scenario(),
    }
}

// ---------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------

const READINGS: &str = "Readings";
/// The name the route gives `Readings` on domain B.
const READINGS_ROUTED: &str = "ReadingsRouted";
const TEMPS: &str = "Temps";
/// The values written to `Temps`, and the filter expression of its route.
const TEMP_VALUES: [u32; 10] = [10, 60, 30, 80, 55, 100, 50, 51, 0, 99];
const FILTER: &str = "v > 50";

fn route_writer() {
    let p = participant(env_domain("IT_DOMAIN_A"));
    let publisher = p.create_publisher(PublisherQos::default());
    let mut qos = writer_qos(DurabilityKind::Volatile);
    qos.durability_service = Default::default();
    let writer = |topic: &str| {
        let t = p
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        publisher
            .create_datawriter::<RawBytes>(&t, qos.clone())
            .unwrap()
    };
    let (readings, temps) = (writer(READINGS), writer(TEMPS));
    // The router's input readers.
    readings
        .wait_for_matched_subscription(1, STEP)
        .expect("router input Readings");
    temps
        .wait_for_matched_subscription(1, STEP)
        .expect("router input Temps");
    event("matched");
    // The reader on B is matched to the router's output before writing.
    zerodds_integration::wait_for_line_on_stdin("go");
    for seq in 0..SAMPLES {
        readings
            .write(&RawBytes::new(format!("reading-{seq:04}").into_bytes()))
            .unwrap();
    }
    for v in TEMP_VALUES {
        temps
            .write(&RawBytes::new(v.to_le_bytes().to_vec()))
            .unwrap();
    }
    readings
        .wait_for_acknowledgments(STEP)
        .expect("router acks Readings");
    temps
        .wait_for_acknowledgments(STEP)
        .expect("router acks Temps");
    event("acknowledged");
    zerodds_integration::wait_for_release();
}

fn route_reader() {
    let p = participant(env_domain("IT_DOMAIN_B"));
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = |topic: &str| {
        let t = p
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        subscriber
            .create_datareader::<RawBytes>(&t, reader_qos(DurabilityKind::Volatile))
            .unwrap()
    };
    let (readings, temps) = (reader(READINGS_ROUTED), reader(TEMPS));
    readings
        .wait_for_matched_publication(1, STEP)
        .expect("router output Readings");
    temps
        .wait_for_matched_publication(1, STEP)
        .expect("router output Temps");
    event("matched");
    let want_temps = TEMP_VALUES.iter().filter(|v| **v > 50).count();
    let deadline = std::time::Instant::now() + STEP;
    let (mut got_readings, mut got_temps) = (0usize, 0usize);
    while got_readings < SAMPLES as usize || got_temps < want_temps {
        for s in readings.take_with_info().unwrap() {
            if s.info.valid_data {
                got_readings += 1;
                event(format!(
                    "reading text={}",
                    String::from_utf8_lossy(&s.data.data)
                ));
            }
        }
        for s in temps.take_with_info().unwrap() {
            if s.info.valid_data {
                got_temps += 1;
                let v = u32::from_le_bytes(s.data.data[..4].try_into().unwrap());
                event(format!("temp v={v}"));
            }
        }
        if got_readings >= SAMPLES as usize && got_temps >= want_temps {
            break;
        }
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let ready = if got_readings < SAMPLES as usize {
            readings.wait_for_data(left)
        } else {
            temps.wait_for_data(left)
        };
        if ready.is_err() {
            event(format!(
                "incomplete readings={got_readings} temps={got_temps}"
            ));
            break;
        }
    }
    event("done");
    zerodds_integration::wait_for_release();
    // Anything that arrived after `done` would be a value the filter drops.
    let late: Vec<u32> = temps
        .take()
        .unwrap()
        .iter()
        .map(|s| u32::from_le_bytes(s.data[..4].try_into().unwrap()))
        .collect();
    let late: Vec<String> = late.iter().map(u32::to_string).collect();
    event(format!("late temps=[{}]", late.join(",")));
}

fn routing_scenario() {
    let (lease_a, lease_b) = (lease_domain(), lease_domain());
    let (a, b) = (lease_a.id(), lease_b.id());
    let scratch = Scratch::new("routing");
    let routes = scratch.path().join("routes.json");
    let types = scratch.path().join("types.json");
    let route = |name: &str, topic: &str, output: &str, filter: Option<&str>| {
        let filter = filter
            .map(|f| format!(r#", "filter": {{ "expression": "{f}" }}"#))
            .unwrap_or_default();
        format!(
            r#"{{ "name": "{name}",
                 "input":  {{ "domain": {a}, "topic": "{topic}", "type_name": "zerodds::RawBytes" }},
                 "output": {{ "domain": {b}, "topic": "{output}", "type_name": "zerodds::RawBytes" }}{filter} }}"#
        )
    };
    std::fs::write(
        &routes,
        format!(
            r#"{{ "name": "integration", "routes": [ {}, {} ] }}"#,
            route("all-readings", READINGS, READINGS_ROUTED, None),
            route("hot-temps", TEMPS, TEMPS, Some(FILTER))
        ),
    )
    .unwrap();
    std::fs::write(
        &types,
        r#"[ { "name": "zerodds::RawBytes", "members": [ { "name": "v", "kind": "u32" } ] } ]"#,
    )
    .unwrap();

    let mut c = zerodds_integration::daemon_command("ZERODDS_ROUTER");
    c.arg("run")
        .arg("--routes")
        .arg(&routes)
        .arg("--types")
        .arg(&types);
    let mut router = Peer::spawn("router", c);
    router.wait_for("its start line", STEP, |l| l.text.starts_with("router '"));

    let start = |name: &str| {
        let mut c = role_command("routing_forwards_and_filters", name);
        c.env("IT_DOMAIN_A", a.to_string())
            .env("IT_DOMAIN_B", b.to_string());
        Peer::spawn(name, c)
    };
    let mut reader = start("reader");
    let mut writer = start("writer");
    reader.wait_for_event("matched", STEP);
    writer.wait_for_event("matched", STEP);
    writer.send_line("go");
    writer.wait_for_event("acknowledged", STEP);
    reader.wait_for_event("done", STEP);
    assert!(writer.release(STEP).success(), "writer role failed");
    reader.close_stdin();
    let late = reader.wait_for_event("late", STEP);
    let lines: Vec<String> = reader.lines().iter().map(|l| l.text.clone()).collect();
    assert!(reader.wait_exit(STEP).success(), "reader role failed");
    let _ = router.stop();

    let readings: Vec<String> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT reading text="))
        .map(str::to_owned)
        .collect();
    let expected: Vec<String> = (0..SAMPLES).map(|s| format!("reading-{s:04}")).collect();
    assert_eq!(
        readings, expected,
        "the unfiltered route forwards every sample from {READINGS} on domain {a} to \
         {READINGS_ROUTED} on {b}, in order"
    );
    let temps: Vec<u32> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT temp v="))
        .map(|v| v.parse().unwrap())
        .collect();
    let selected: Vec<u32> = TEMP_VALUES.iter().copied().filter(|v| *v > 50).collect();
    assert_eq!(
        temps, selected,
        "`{FILTER}` (DDS 1.4 Annex B) selects {selected:?} of {TEMP_VALUES:?}"
    );
    assert_eq!(late, "temps=[]", "no value the filter drops arrives later");
    drop((lease_a, lease_b));
}

#[test]
fn routing_forwards_and_filters() {
    match role().as_deref() {
        Some("writer") => route_writer(),
        Some("reader") => route_reader(),
        Some(other) => panic!("unknown role {other}"),
        None => routing_scenario(),
    }
}
