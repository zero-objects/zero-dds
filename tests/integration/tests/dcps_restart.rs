// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A reader across writer restarts (GitHub #32): one reader process stays up
//! while writer processes on the same topic come and go — killed (SIGKILL, no
//! unregister, no goodbye), ended gracefully, restarted several times, or
//! alive at the same time. Each writer process is a new participant with a
//! new GUID.
//!
//! The oracle is what DDS 1.4 and DDSI-RTPS 2.5 say of these writers:
//!
//! * every writer process is a DataWriter of its own (a new GUID, §8.2.4.1);
//!   the reader matches each as a further publication (DDS 1.4 §2.2.4.1
//!   SUBSCRIPTION_MATCHED, `total_count` one per writer process);
//! * RELIABLE with KEEP_ALL delivers every sample of every writer, in order,
//!   from its first sequence number (DDS 1.4 §2.2.3.14, DDSI-RTPS 2.5
//!   §8.4.12.2), whatever the reader holds about a writer that is gone;
//! * BEST_EFFORT promises no delivery, but a best-effort reader accepts only
//!   a sequence number above the highest it has seen from that writer
//!   (§8.4.11.1): what arrives of each writer arrives in order, and a
//!   restarted writer starts its own numbering;
//! * a DataWriter deleted by its application is a deletion in the
//!   SEDPbuiltinPublicationsWriter's history (DDSI-RTPS 2.5 Table 8.82), so
//!   the reader stops counting it as matched (`current_count` 0) without
//!   waiting for the participant lease (100 s by default) to expire.
//!
//! The values are stated before the run: generation g writes 0..SAMPLES with
//! key g.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::io::BufRead;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_integration::dds::{Reading, participant, reliable_reader, reliable_writer};
use zerodds_integration::{Line, Peer, Stream, event, fields, lease_domain, role, role_command};

const SAMPLES: u32 = 50;
const STEP: Duration = Duration::from_secs(10);
const TOPIC: &str = "RestartReadings";
/// How long the reader waits for data before it looks at its matched status
/// again, and how often a best-effort writer writes: the rate of a loop that
/// waits on an event, not a synchronisation.
const POLL: Duration = Duration::from_millis(20);
const PACE: Duration = Duration::from_millis(2);

fn domain() -> i32 {
    std::env::var("IT_DOMAIN").unwrap().parse().unwrap()
}

fn reliable() -> bool {
    std::env::var("IT_RELIABILITY").as_deref() != Ok("best_effort")
}

/// The lines of standard input, read by a thread of their own.
fn stdin_lines() -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line.trim_end().to_owned()).is_err() {
                break;
            }
        }
    });
    rx
}

fn writer_qos() -> DataWriterQos {
    let mut q = reliable_writer();
    if !reliable() {
        // A best-effort writer keeps nothing for anyone: the last sample.
        q.reliability.kind = ReliabilityKind::BestEffort;
        q.history.kind = HistoryKind::KeepLast;
        q.history.depth = 1;
    }
    q
}

fn reader_qos() -> DataReaderQos {
    let mut q = reliable_reader();
    if !reliable() {
        q.reliability.kind = ReliabilityKind::BestEffort;
    }
    q
}

/// Writes generation `IT_GENERATION`, then waits to be killed or to be told
/// `exit`, which deletes the writer and its participant through the API.
fn writer_role() {
    let generation: u32 = std::env::var("IT_GENERATION").unwrap().parse().unwrap();
    let commands = stdin_lines();
    let p = participant(domain());
    let t = p
        .create_topic::<Reading>(TOPIC, TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let w = publisher
        .create_datawriter::<Reading>(&t, writer_qos())
        .unwrap();
    w.wait_for_matched_subscription(1, STEP).expect("matched");
    if reliable() {
        for seq in 0..SAMPLES {
            w.write(&Reading {
                id: generation,
                seq,
            })
            .unwrap();
        }
        w.wait_for_acknowledgments(STEP).expect("acknowledged");
        event("acknowledged");
    } else {
        // No acknowledgment exists: write until the scenario has seen
        // samples of this generation arrive.
        let mut seq = 0;
        loop {
            w.write(&Reading {
                id: generation,
                seq,
            })
            .unwrap();
            seq += 1;
            match commands.recv_timeout(PACE) {
                Ok(command) if command == "stop" => break,
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        event(format!("streamed count={seq}"));
    }
    // Killed here by the scenario (a crash), or told to end.
    if commands.recv().as_deref() == Ok("exit") {
        publisher.delete_datawriter(&w).unwrap();
        event("writer deleted");
        p.delete_contained_entities().unwrap();
        DomainParticipantFactory::instance()
            .delete_participant(&p)
            .unwrap();
        event("participant deleted");
    }
}

/// Prints every sample and every change of its matched status until told
/// `report`.
fn reader_role() {
    let commands = stdin_lines();
    let p = participant(domain());
    let t = p
        .create_topic::<Reading>(TOPIC, TopicQos::default())
        .unwrap();
    let r = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&t, reader_qos())
        .unwrap();
    event("ready");
    let mut matched = (0, 0);
    loop {
        match commands.try_recv() {
            Ok(command) if command == "report" => break,
            Ok(_) | Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return,
        }
        if r.wait_for_data(POLL).is_ok() {
            for s in r.take_with_info().unwrap() {
                if s.info.valid_data {
                    event(format!(
                        "sample generation={} seq={}",
                        s.data.id, s.data.seq
                    ));
                }
            }
        }
        let status = r.get_subscription_matched_status().unwrap();
        if (status.current_count, status.total_count) != matched {
            matched = (status.current_count, status.total_count);
            event(format!("matched current={} total={}", matched.0, matched.1));
        }
    }
    event(format!(
        "done current={} total_matched={}",
        matched.0, matched.1
    ));
    let _ = commands.recv();
}

/// The reader of one scenario and the writers it starts.
struct Run {
    test: &'static str,
    domain: i32,
    reliability: &'static str,
    reader: Peer,
}

impl Run {
    fn start(test: &'static str, domain: i32, reliability: &'static str) -> Self {
        let mut reader = Peer::spawn(
            "reader",
            Self::command(test, domain, reliability, "reader", 0),
        );
        reader.wait_for_event("ready", STEP);
        Self {
            test,
            domain,
            reliability,
            reader,
        }
    }

    fn command(
        test: &str,
        domain: i32,
        reliability: &str,
        name: &str,
        generation: u32,
    ) -> std::process::Command {
        let mut c = role_command(test, name);
        c.env("IT_DOMAIN", domain.to_string())
            .env("IT_RELIABILITY", reliability)
            .env("IT_GENERATION", generation.to_string());
        c
    }

    fn writer(&self, generation: u32) -> Peer {
        Peer::spawn(
            &format!("writer {generation}"),
            Self::command(
                self.test,
                self.domain,
                self.reliability,
                "writer",
                generation,
            ),
        )
    }

    /// Waits until the reader has printed `line`, unless it already has.
    fn reader_saw(&mut self, line: &str, timeout: Duration) {
        let expected = format!("EVENT {line}");
        let is = |l: &Line| l.stream == Stream::Out && l.text == expected;
        if !self.reader.lines().iter().any(is) {
            self.reader.wait_for(&format!("`{line}`"), timeout, is);
        }
    }

    /// Ends the reader and returns its matched counts and the sequence
    /// numbers it received of each generation, in arrival order.
    fn report(mut self) -> (Matched, Vec<Line>) {
        self.reader.send_line("report");
        let done = fields(&self.reader.wait_for_event("done", STEP));
        let lines = self.reader.lines().to_vec();
        assert!(self.reader.release(STEP).success(), "reader failed");
        (
            Matched {
                current: done["current"].parse().unwrap(),
                total: done["total_matched"].parse().unwrap(),
            },
            lines,
        )
    }
}

struct Matched {
    current: i32,
    total: i32,
}

fn seqs(lines: &[Line], generation: u32) -> Vec<u32> {
    lines
        .iter()
        .filter_map(|l| l.text.strip_prefix("EVENT sample "))
        .map(fields)
        .filter(|f| f["generation"] == generation.to_string())
        .map(|f| f["seq"].parse().unwrap())
        .collect()
}

fn every_sample_in_order(lines: &[Line], generations: &[u32]) {
    for &generation in generations {
        assert_eq!(
            seqs(lines, generation),
            (0..SAMPLES).collect::<Vec<_>>(),
            "generation {generation}: every sample once, in order \
             (DDS 1.4 §2.2.3.14, DDSI-RTPS 2.5 §8.4.12.2)"
        );
    }
}

/// The last sample of a reliable generation, as the reader prints it.
fn last_of(generation: u32) -> String {
    format!("sample generation={generation} seq={}", SAMPLES - 1)
}

fn dispatch(scenario: fn()) {
    match role().as_deref() {
        Some("writer") => writer_role(),
        Some("reader") => reader_role(),
        Some(other) => panic!("unknown role {other}"),
        None => scenario(),
    }
}

#[test]
fn reader_survives_a_writer_restart() {
    dispatch(|| {
        let lease = lease_domain();
        let mut run = Run::start("reader_survives_a_writer_restart", lease.id(), "reliable");
        let mut first = run.writer(1);
        first.wait_for_event("acknowledged", STEP);
        run.reader_saw(&last_of(1), STEP);
        // A crash: the process is gone without unregistering anything.
        let _ = first.stop();
        let mut second = run.writer(2);
        second.wait_for_event("acknowledged", STEP);
        run.reader_saw(&last_of(2), STEP);
        let (counts, lines) = run.report();
        let _ = second.stop();
        assert_eq!(
            counts.total, 2,
            "the restarted writer is a second publication (DDS 1.4 §2.2.4.1)"
        );
        every_sample_in_order(&lines, &[1, 2]);
        drop(lease);
    });
}

#[test]
fn reader_survives_a_graceful_writer_restart() {
    dispatch(|| {
        let lease = lease_domain();
        let mut run = Run::start(
            "reader_survives_a_graceful_writer_restart",
            lease.id(),
            "reliable",
        );
        let mut first = run.writer(1);
        first.wait_for_event("acknowledged", STEP);
        run.reader_saw(&last_of(1), STEP);
        first.send_line("exit");
        first.wait_for_event("participant deleted", STEP);
        assert!(first.wait_exit(STEP).success(), "writer 1 failed");
        // Table 8.82: the deletion reaches the reader through SEDP, long
        // before the 100 s participant lease would.
        run.reader_saw("matched current=0 total=1", STEP);
        let mut second = run.writer(2);
        second.wait_for_event("acknowledged", STEP);
        run.reader_saw(&last_of(2), STEP);
        let (counts, lines) = run.report();
        let _ = second.stop();
        assert_eq!(counts.total, 2, "DDS 1.4 §2.2.4.1");
        every_sample_in_order(&lines, &[1, 2]);
        drop(lease);
    });
}

#[test]
fn best_effort_reader_survives_a_writer_restart() {
    dispatch(|| {
        let lease = lease_domain();
        let mut run = Run::start(
            "best_effort_reader_survives_a_writer_restart",
            lease.id(),
            "best_effort",
        );
        let first_sample = |generation: u32| {
            let prefix = format!("EVENT sample generation={generation} ");
            move |l: &Line| l.stream == Stream::Out && l.text.starts_with(&prefix)
        };
        let mut first = run.writer(1);
        run.reader
            .wait_for("a sample of generation 1", STEP, first_sample(1));
        first.send_line("stop");
        first.wait_for_event("streamed", STEP);
        let _ = first.stop();
        let mut second = run.writer(2);
        run.reader
            .wait_for("a sample of generation 2", STEP, first_sample(2));
        second.send_line("stop");
        second.wait_for_event("streamed", STEP);
        let (counts, lines) = run.report();
        let _ = second.stop();
        assert_eq!(counts.total, 2, "DDS 1.4 §2.2.4.1");
        for generation in [1u32, 2] {
            let got = seqs(&lines, generation);
            assert!(!got.is_empty(), "generation {generation}: nothing arrived");
            assert!(
                got.windows(2).all(|w| w[0] < w[1]),
                "generation {generation}: out of order or twice (DDSI-RTPS 2.5 §8.4.11.1): {got:?}"
            );
        }
        drop(lease);
    });
}

#[test]
fn reader_follows_four_writer_generations() {
    dispatch(|| {
        let lease = lease_domain();
        let mut run = Run::start(
            "reader_follows_four_writer_generations",
            lease.id(),
            "reliable",
        );
        for generation in 1..=4 {
            let mut writer = run.writer(generation);
            writer.wait_for_event("acknowledged", STEP);
            run.reader_saw(&last_of(generation), STEP);
            let _ = writer.stop();
        }
        let (counts, lines) = run.report();
        assert_eq!(counts.total, 4, "DDS 1.4 §2.2.4.1");
        every_sample_in_order(&lines, &[1, 2, 3, 4]);
        drop(lease);
    });
}

#[test]
fn two_live_writers_both_deliver() {
    dispatch(|| {
        let lease = lease_domain();
        let mut run = Run::start("two_live_writers_both_deliver", lease.id(), "reliable");
        let mut first = run.writer(1);
        let mut second = run.writer(2);
        run.reader_saw("matched current=2 total=2", STEP);
        first.wait_for_event("acknowledged", STEP);
        second.wait_for_event("acknowledged", STEP);
        run.reader_saw(&last_of(1), STEP);
        run.reader_saw(&last_of(2), STEP);
        let (counts, lines) = run.report();
        let _ = first.stop();
        let _ = second.stop();
        assert_eq!((counts.current, counts.total), (2, 2), "DDS 1.4 §2.2.4.1");
        every_sample_in_order(&lines, &[1, 2]);
        drop(lease);
    });
}
