// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The SHM user transport (`transport.user_transport = SHM`) between two
//! processes: a process with a best-effort writer and a process with a
//! best-effort reader.
//!
//! A best-effort reader "is completely passive as it only receives data and
//! does not send messages itself" (DDSI-RTPS 2.5 §8.4.2.3), so the reader's
//! process sends the writer's process no user datagram at all. The SHM
//! adapter of the runtime opened the ring it reads a peer's datagrams from
//! at its own first send to that peer, and such a reader took nothing: 0 of
//! 30 000 echoes in the SHM cells of `bench-regression` (job 229228). ZeroDDS
//! SHM Transport 1.0 §4.3: the reader side may open before the writer side;
//! whoever comes first creates the segment, whoever comes later attaches.
//!
//! Oracle: the samples are fixed before the run, their number and the
//! octets of each. The reader takes exactly them, in the order written:
//! one writer writes into one ring, a single-producer single-consumer FIFO
//! (§4.1), and the reader's channel drops nothing.

#![cfg(all(target_os = "linux", feature = "same-host-shm"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserTransportKind, UserWriterConfig,
};
use zerodds_rtps::wire_types::GuidPrefix;

const TEST: &str = "shm_4_3_a_best_effort_reader_takes_what_a_process_it_never_sent_to_writes";
/// Set in the writer process: the domain, the topic and the parent's pid.
const CHILD_ENV: &str = "ZERODDS_SHM_USER_WRITER";
const TOPIC: &str = "ShmUserTransport";
const SAMPLES: u32 = 64;
/// What one wait may take: discovery with a 100 ms SPDP period matches in
/// well under a second.
const STEP: Duration = Duration::from_secs(10);

/// Sample `i`: its number and a pattern of it, 64 octets.
fn sample(i: u32) -> Vec<u8> {
    let mut s = Vec::with_capacity(64);
    s.extend_from_slice(b"shm-user-transport ");
    s.extend_from_slice(&i.to_be_bytes());
    while s.len() < 64 {
        s.push((i as u8).wrapping_mul(31).wrapping_add(s.len() as u8));
    }
    s
}

/// The SHM user transport and nothing else for user traffic: no same-host
/// channel, no user multicast.
fn config() -> RuntimeConfig {
    let mut cfg = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    cfg.user_transport = Some(UserTransportKind::Shm);
    cfg.same_host_shm = false;
    cfg.user_multicast = false;
    cfg
}

/// A prefix of its own per process and role: the segment names come from
/// it, so two runs on one host do not share a ring.
fn prefix(pid: u32, role: u8) -> GuidPrefix {
    let mut p = [0u8; 12];
    p[..2].copy_from_slice(&[0x01, 0xF0]);
    p[2..6].copy_from_slice(&pid.to_be_bytes());
    p[6] = role;
    p[11] = 0x43;
    GuidPrefix::from_bytes(p)
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + STEP;
    while !done() {
        assert!(Instant::now() < deadline, "{what} within {STEP:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The writer process: matches, waits for the reader's go on stdin, writes
/// the samples, and stays until stdin closes.
fn writer(spec: &str) {
    let mut parts = spec.split(',');
    let domain: i32 = parts.next().unwrap().parse().unwrap();
    let pid: u32 = parts.next().unwrap().parse().unwrap();
    let rt = Arc::new(common::ShutsDown(
        DcpsRuntime::start(domain, prefix(pid, b'W'), config()).expect("start"),
    ));
    let eid = rt
        .register_user_writer(UserWriterConfig {
            topic_name: TOPIC.into(),
            type_name: "RawBytes".into(),
            reliable: false,
            ..Default::default()
        })
        .expect("writer");
    wait_until("the writer matches the reader", || {
        rt.user_writer_matched_count(eid) >= 1
    });
    let mut stdin = BufReader::new(std::io::stdin());
    let mut line = String::new();
    stdin.read_line(&mut line).expect("the go");
    assert_eq!(line.trim(), "go");
    for i in 0..SAMPLES {
        rt.write_user_sample(eid, sample(i)).expect("write");
    }
    eprintln!("writer: {SAMPLES} written");
    line.clear();
    let _ = stdin.read_line(&mut line);
}

#[test]
fn shm_4_3_a_best_effort_reader_takes_what_a_process_it_never_sent_to_writes() {
    if let Ok(spec) = std::env::var(CHILD_ENV) {
        writer(&spec);
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let pid = std::process::id();
    let rt = Arc::new(common::ShutsDown(
        DcpsRuntime::start(lease.id(), prefix(pid, b'R'), config()).expect("start"),
    ));
    let (eid, rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: TOPIC.into(),
            type_name: "RawBytes".into(),
            reliable: false,
            ..Default::default()
        })
        .expect("reader");

    let exe = std::env::current_exe().expect("current_exe");
    // `ZERODDS_TEST_RUNNER` starts the child through a runner, as cargo's
    // target runner starts this process.
    let runner = std::env::var("ZERODDS_TEST_RUNNER").unwrap_or_default();
    let mut runner = runner.split_whitespace();
    let mut command = match runner.next() {
        Some(program) => {
            let mut c = Command::new(program);
            c.args(runner).arg(&exe);
            c
        }
        None => Command::new(&exe),
    };
    let mut child = command
        .env(CHILD_ENV, format!("{},{pid}", lease.id()))
        .args(["--exact", TEST, "--nocapture"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the writer process");

    wait_until("the reader matches the writer", || {
        rt.user_reader_matched_count(eid) >= 1
    });
    let mut go = child.stdin.take().expect("stdin");
    writeln!(go, "go").expect("the go");

    let mut taken = Vec::new();
    let deadline = Instant::now() + STEP;
    while taken.len() < SAMPLES as usize {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { payload, .. }) => taken.push(payload.as_slice().to_vec()),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    drop(go);
    let output = child.wait_with_output().expect("the writer ends");
    let expected: Vec<Vec<u8>> = (0..SAMPLES).map(sample).collect();
    assert_eq!(
        taken.len(),
        expected.len(),
        "{} of {SAMPLES} samples taken; writer process: {}, {}",
        taken.len(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(taken, expected);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[path = "common/mod.rs"]
mod common;
