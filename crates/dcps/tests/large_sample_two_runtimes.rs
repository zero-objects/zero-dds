// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A sample larger than one datagram between two runtimes of one process:
//! the writer fragments it (DATA_FRAG, RTPS 2.5 §8.4.14.1), the reader puts
//! it back together. Sizes below and above one UDP datagram, and GUID
//! prefixes that do and do not share the host bytes (the same-host and the
//! plain UDP path).
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::GuidPrefix;

fn writer(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "Big".into(),
        ..Default::default()
    }
}

fn round_trip(prefix_b: [u8; 12], sizes: &[usize]) -> Vec<(usize, Option<usize>)> {
    let domain = common::unique_domain();
    let a = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0xA1; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let b = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes(prefix_b),
        RuntimeConfig::default(),
    )
    .unwrap();
    let w = a
        .register_user_writer_kind(writer("BigSample"), false)
        .unwrap();
    let (_r, rx) = b
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "BigSample".into(),
                type_name: "Big".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.user_writer_matched_count(w) == 0 {
        assert!(Instant::now() < deadline, "no match");
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut out = Vec::new();
    for &n in sizes {
        let payload: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
        a.write_user_sample(w, payload.clone()).unwrap();
        let mut got = None;
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end && got.is_none() {
            if let Ok(UserSample::Alive { payload: p, .. }) =
                rx.recv_timeout(Duration::from_millis(100))
            {
                assert_eq!(p.as_ref(), &payload[..], "the bytes of a {n}-byte sample");
                got = Some(p.len());
            }
        }
        out.push((n, got));
    }
    drop((Arc::clone(&a), Arc::clone(&b)));
    out
}

const SIZES: &[usize] = &[1_000, 8_000, 20_000, 70_000];

#[test]
fn a_fragmented_sample_arrives_whole_between_hosts() {
    // Other host bytes: the plain UDP path.
    let got = round_trip([0xB2; 12], SIZES);
    assert!(got.iter().all(|(n, g)| *g == Some(*n)), "{got:?}");
}

#[test]
fn a_fragmented_sample_arrives_whole_on_one_host() {
    // The host bytes of the writer: the same-host path.
    let mut p = [0xB2; 12];
    p[2..6].copy_from_slice(&[0xA1; 4]);
    let got = round_trip(p, SIZES);
    assert!(got.iter().all(|(n, g)| *g == Some(*n)), "{got:?}");
}
