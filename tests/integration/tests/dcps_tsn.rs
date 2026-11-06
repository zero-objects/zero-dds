// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS over TSN: RTPS directly in Ethernet frames (EtherType 0x88B5, an
//! IEEE 802.1Q tag with VLAN id and priority; DDS-TSN 1.0 Annex A), between
//! a writer process and a reader process bound to the two ends of a veth
//! pair this test creates. Discovery stays SPDP/SEDP over UDPv4; the user
//! traffic takes the link. Two cases: VLAN 42 with priority 5, and priority
//! 5 without a VLAN (`tsn.vlan = 0`), which IEEE 802.1Q-2022 sends as a
//! priority-tagged frame, a tag with the null VID 0 (§3 "priority-tagged
//! frame", §9.6 Table 9-2).
//!
//! The oracle:
//!
//! * `tcpdump` on the reader's end of the link, not ZeroDDS, reports the
//!   frames: EtherType 0x88B5 inside an 802.1Q tag with the configured VLAN
//!   id and priority, sent from the writer's interface address (as the
//!   kernel reports it in sysfs), each payload an RTPS message
//!   (`RTPS`, DDSI-RTPS 2.5 §8.3.3.1, §9.4.5.1.1);
//! * RELIABLE and KEEP_ALL deliver every sample once, in order
//!   (DDS 1.4 §2.2.3.14, §2.2.3.18; DDSI-RTPS 2.5 §8.4.2.2).
//!
//! Needs CAP_NET_ADMIN (the veth pair) and CAP_NET_RAW (AF_PACKET and the
//! capture), which no CI runner grants: a manual test on the Linux test host
//! as root (ci/manual-tests.tsv; internal/release/rc8-test-suite.md,
//! decision 11).

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::process::Command;
use std::time::Duration;

use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dcps::runtime::{RuntimeConfig, UserTransportKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};
use zerodds_integration::{Peer, event, lease_domain, role, role_command, tools};

const SAMPLES: u32 = 50;
const STEP: Duration = Duration::from_secs(10);
const TOPIC: &str = "TsnReadings";

fn config() -> RuntimeConfig {
    let mut cfg = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    cfg.user_transport = Some(UserTransportKind::Tsn);
    cfg.tsn_interface = Some(std::env::var("IT_IFACE").unwrap());
    cfg.tsn_vlan = std::env::var("IT_VLAN").unwrap().parse().unwrap();
    cfg.tsn_pcp = std::env::var("IT_PCP").unwrap().parse().unwrap();
    cfg.user_multicast = false;
    cfg.same_host_shm = false;
    cfg
}

fn participant() -> zerodds_dcps::DomainParticipant {
    let domain: i32 = std::env::var("IT_DOMAIN").unwrap().parse().unwrap();
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), config())
        .expect("TSN participant")
}

fn writer_role() {
    let p = participant();
    let t = p
        .create_topic::<RawBytes>(TOPIC, TopicQos::default())
        .unwrap();
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    let w = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&t, q)
        .unwrap();
    w.wait_for_matched_subscription(1, STEP).expect("matched");
    event("matched");
    for seq in 0..SAMPLES {
        w.write(&RawBytes::new(format!("tsn-{seq:04}").into_bytes()))
            .unwrap();
    }
    w.wait_for_acknowledgments(STEP).expect("acknowledged");
    event("acknowledged");
    zerodds_integration::wait_for_release();
}

fn reader_role() {
    let p = participant();
    let t = p
        .create_topic::<RawBytes>(TOPIC, TopicQos::default())
        .unwrap();
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    let r = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&t, q)
        .unwrap();
    r.wait_for_matched_publication(1, STEP).expect("matched");
    event("matched");
    let deadline = std::time::Instant::now() + STEP;
    let mut got = 0;
    while got < SAMPLES {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if r.wait_for_data(left).is_err() {
            event(format!("incomplete got={got}"));
            break;
        }
        for s in r.take_with_info().unwrap() {
            if s.info.valid_data {
                got += 1;
                event(format!(
                    "sample text={}",
                    String::from_utf8_lossy(&s.data.data)
                ));
            }
        }
    }
    event("done");
    zerodds_integration::wait_for_release();
}

/// A veth pair that is removed when the value drops.
struct Veth {
    a: String,
    b: String,
}

impl Veth {
    fn new() -> Self {
        let tag = std::process::id() % 100_000;
        let (a, b) = (format!("zdit{tag}a"), format!("zdit{tag}b"));
        let ip = tools::require("ip", &["/usr/sbin", "/sbin"]);
        let _ = Command::new(&ip).args(["link", "del", &a]).status();
        let ok = |args: &[&str]| {
            let out = Command::new(&ip).args(args).output().unwrap();
            assert!(
                out.status.success(),
                "ip {args:?}: {} (needs CAP_NET_ADMIN)",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        ok(&["link", "add", &a, "type", "veth", "peer", "name", &b]);
        ok(&["link", "set", &a, "up"]);
        ok(&["link", "set", &b, "up"]);
        Self { a, b }
    }

    fn mac(name: &str) -> String {
        std::fs::read_to_string(format!("/sys/class/net/{name}/address"))
            .unwrap()
            .trim()
            .to_owned()
    }
}

impl Drop for Veth {
    fn drop(&mut self) {
        if let Some(ip) = tools::find("ip", &["/usr/sbin", "/sbin"]) {
            let _ = Command::new(ip).args(["link", "del", &self.a]).status();
        }
    }
}

/// The scenario of the test function `test`: frames with VLAN id `vlan`
/// (0: none) and priority `pcp`.
fn scenario(test: &str, vlan: u16, pcp: u8) {
    let lease = lease_domain();
    let domain = lease.id();
    let veth = Veth::new();
    let writer_mac = Veth::mac(&veth.a);

    // The capture on the reader's end; `-l` line-buffers, `-e` prints the
    // link-level header, `-x` the frame bytes after it.
    let mut capture = Command::new(tools::require("tcpdump", &["/usr/sbin", "/usr/bin"]));
    let filter = format!("ether src {writer_mac} and vlan {vlan} and ether proto 0x88b5");
    capture
        .args(["-i", &veth.b, "-n", "-e", "-x", "-l", "-c"])
        .arg(CAPTURED.to_string())
        .arg(filter);
    let mut capture = Peer::spawn("tcpdump", capture);
    capture.wait_for("tcpdump listening", STEP, |l| {
        l.text.contains("listening on")
    });

    let start = |name: &str, iface: &str| {
        let mut c = role_command(test, name);
        c.env("IT_DOMAIN", domain.to_string())
            .env("IT_IFACE", iface)
            .env("IT_VLAN", vlan.to_string())
            .env("IT_PCP", pcp.to_string());
        Peer::spawn(name, c)
    };
    let mut reader = start("reader", &veth.b);
    let mut writer = start("writer", &veth.a);
    writer.wait_for_event("matched", STEP);
    reader.wait_for_event("matched", STEP);
    writer.wait_for_event("acknowledged", STEP);
    reader.wait_for_event("done", STEP);
    let lines: Vec<String> = reader.lines().iter().map(|l| l.text.clone()).collect();
    assert!(writer.release(STEP).success(), "writer role failed");
    assert!(reader.release(STEP).success(), "reader role failed");
    let (capture_status, capture_lines) = capture.wait_exit_lines(STEP);
    drop(veth);

    let texts: Vec<String> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT sample text="))
        .map(str::to_owned)
        .collect();
    let expected: Vec<String> = (0..SAMPLES).map(|s| format!("tsn-{s:04}")).collect();
    assert_eq!(
        texts, expected,
        "every sample once, in order, over TSN (DDS 1.4 §2.2.3.14, §2.2.3.18)"
    );

    // tcpdump's own report of the frames the writer sent.
    let dump: String = capture_lines
        .iter()
        .map(|l| format!("  {}\n", l.text))
        .collect();
    assert!(capture_status.success(), "tcpdump failed:\n{dump}");
    let frames = frames(&capture_lines);
    assert_eq!(
        frames.len(),
        CAPTURED,
        "tcpdump reports {CAPTURED} frames:\n{dump}"
    );
    for (header, hex) in &frames {
        assert!(
            header.contains(&format!("{writer_mac} >")),
            "a frame from the writer's interface {writer_mac}: {header}"
        );
        assert!(
            header.contains(&format!("vlan {vlan}, p {pcp}")),
            "the 802.1Q tag carries VLAN {vlan} and priority {pcp}: {header}"
        );
        assert!(
            header.contains("(0x88b5)"),
            "EtherType 0x88B5 (DDS-TSN 1.0 Annex A): {header}"
        );
        // `-x` starts after the Ethernet header; with the tag in front, the
        // tag's last four octets come first.
        let rtps = "52545053";
        assert!(
            hex.starts_with(rtps) || hex.get(8..).is_some_and(|h| h.starts_with(rtps)),
            "the payload is an RTPS message, `RTPS` first (DDSI-RTPS 2.5 §8.3.3.1): {hex}"
        );
    }
    drop(lease);
}

/// Frames tcpdump captures before it exits (`-c`).
const CAPTURED: usize = 5;

/// tcpdump's `-e -x` output as (header line, payload hex) per frame.
fn frames(lines: &[zerodds_integration::Line]) -> Vec<(String, String)> {
    let mut frames: Vec<(String, String)> = Vec::new();
    for line in lines {
        if line.stream != zerodds_integration::Stream::Out {
            continue;
        }
        let text = line.text.trim_start();
        if let Some(rest) = text.strip_prefix("0x") {
            if let Some((_, hex)) = rest.split_once(':')
                && let Some(frame) = frames.last_mut()
            {
                for group in hex.split_whitespace() {
                    if group.len() <= 4 && group.chars().all(|c| c.is_ascii_hexdigit()) {
                        frame.1.push_str(group);
                    }
                }
            }
        } else if text.contains(" > ") {
            frames.push((text.to_owned(), String::new()));
        }
    }
    frames
}

fn run(test: &str, vlan: u16, pcp: u8) {
    match role().as_deref() {
        Some("writer") => writer_role(),
        Some("reader") => reader_role(),
        Some(other) => panic!("unknown role {other}"),
        None => scenario(test, vlan, pcp),
    }
}

#[test]
fn dcps_over_tsn_veth() {
    run("dcps_over_tsn_veth", 42, 5);
}

/// `tsn.vlan = 0` with a priority: a priority-tagged frame, VID 0 and the
/// PCP in the tag (IEEE 802.1Q-2022 §3, §9.6 Table 9-2), not an untagged
/// frame that drops the priority.
#[test]
fn dcps_over_tsn_priority_tagged() {
    run("dcps_over_tsn_priority_tagged", 0, 5);
}
