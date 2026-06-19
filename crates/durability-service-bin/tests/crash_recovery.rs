// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! P4 crash-recovery: the headline scenario — `kill -9` the running daemon
//! PROCESS and restart it; all PERSISTENT samples survive (re-served from the
//! sqlite file). Cross-process by necessity (separate daemon process), so it
//! relies on real RTPS discovery — run it where cross-process multicast works
//! (Linux/codepit). Linux-gated.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(target_os = "linux")]

use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime};

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_durability_store_sqlite::SqliteStore;
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

const SVC_BIN: &str = env!("CARGO_BIN_EXE_zerodds-durability-svc");
const N: usize = 20;

fn keep_all() -> Contract {
    Contract {
        history_kind: HistoryKind::KeepAll,
        history_depth: 0,
        max_samples: LENGTH_UNLIMITED,
        max_instances: LENGTH_UNLIMITED,
        max_samples_per_instance: LENGTH_UNLIMITED,
        cleanup_delay: Duration::ZERO,
    }
}

fn writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

fn reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::TransientLocal;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

fn spawn_daemon(domain: i32, db: &str, topic: &str) -> Child {
    Command::new(SVC_BIN)
        .args([
            "--domain",
            &domain.to_string(),
            "--store",
            "sqlite",
            "--path",
            db,
            "--topic",
            topic,
        ])
        .spawn()
        .expect("spawn daemon")
}

/// Polls the sqlite file (a fresh read connection) until it holds `n` samples.
fn wait_db_samples(db: &str, topic: &str, n: usize, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let got = SqliteStore::open(db, keep_all())
            .ok()
            .and_then(|s| s.stats(topic).ok())
            .map(|s| s.samples)
            .unwrap_or(0);
        if got >= n || Instant::now() >= deadline {
            return got;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn late_joiner_distinct(domain: i32, topic: &str, expect: usize, timeout: Duration) -> usize {
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .unwrap();
    let sub = participant.create_subscriber(SubscriberQos::default());
    let topic_h = participant
        .create_topic::<RawBytes>(topic, TopicQos::default())
        .unwrap();
    let reader = sub
        .create_datareader::<RawBytes>(&topic_h, reader_qos())
        .unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let deadline = Instant::now() + timeout;
    while seen.len() < expect && Instant::now() < deadline {
        let _ = reader.wait_for_data(Duration::from_millis(200));
        if let Ok(samples) = reader.take_with_info() {
            for s in samples {
                if s.info.valid_data && s.data.data.len() >= 2 {
                    // sample id = first two payload bytes (u16 le)
                    seen.insert(u16::from_le_bytes([s.data.data[0], s.data.data[1]]));
                }
            }
        }
    }
    seen.len()
}

fn tmp_db() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("/tmp/zerodds-crash-{nanos}.db")
}

#[test]
fn daemon_kill9_restart_survives() {
    let domain = 81;
    let topic = "CrashTopic";
    let db = tmp_db();

    // 1) Start the daemon, let it come up.
    let mut daemon = spawn_daemon(domain, &db, topic);
    std::thread::sleep(Duration::from_secs(2));

    // 2) An application publishes N PERSISTENT-class samples.
    {
        let factory = DomainParticipantFactory::instance();
        let app = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let publisher = app.create_publisher(PublisherQos::default());
        let topic_h = app
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        let writer = publisher
            .create_datawriter::<RawBytes>(&topic_h, writer_qos())
            .unwrap();
        // Wait for the daemon's ingest reader to match.
        assert!(
            writer
                .wait_for_matched_subscription(1, Duration::from_secs(15))
                .is_ok(),
            "daemon ingest reader should match the app writer"
        );
        for i in 0..N as u16 {
            let mut payload = i.to_le_bytes().to_vec();
            payload.extend_from_slice(b"-crash-payload");
            writer.write(&RawBytes::new(payload)).unwrap();
        }
        // Deterministic: poll the sqlite file until the daemon has ingested all.
        assert_eq!(
            wait_db_samples(&db, topic, N, Duration::from_secs(15)),
            N,
            "daemon should have persisted all {N} samples before the crash"
        );
    }

    // 3) CRASH: kill -9 the daemon process.
    daemon.kill().expect("kill -9 daemon");
    let _ = daemon.wait();
    std::thread::sleep(Duration::from_secs(1));

    // 4) RESTART on the same sqlite file.
    let mut daemon2 = spawn_daemon(domain, &db, topic);
    std::thread::sleep(Duration::from_secs(3));

    // 5) A fresh subscriber must receive all N from the restarted daemon
    //    (startup-sync re-primed the replay writer from disk; no app writer).
    let distinct = late_joiner_distinct(domain, topic, N, Duration::from_secs(20));
    let _ = daemon2.kill();
    let _ = daemon2.wait();
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(format!("{db}-wal"));
    let _ = std::fs::remove_file(format!("{db}-shm"));

    assert_eq!(
        distinct, N,
        "after kill -9 + restart, all {N} samples must survive (got {distinct} distinct)"
    );
}
