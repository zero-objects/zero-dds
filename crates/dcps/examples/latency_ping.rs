//! C3 latency: pinger — measures the **round-trip latency** (clock-sync-free)
//! against `latency_pong`. Sends `rt/ping` with a sequence number, waits for
//! the echo on `rt/pong`, measures RTT; after N iterations p50/p90/p99/max.
//!
//! ```text
//! # Pong host (e.g. the macOS host over WiFi): latency_pong
//! ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=<pong-host-ip> \
//!   cargo run -p zerodds-dcps --release --example latency_ping -- <size> <count>
//! ```
//! Args: size (bytes of payload, default 256), count (default 200).
//!
//! # ⚠ Cross-machine over WiFi: discovery over a lossy / power-saving link
//! Loopback yields clean numbers (256 B p50≈40 µs / p99≈83 µs).
//! Cross-machine over WiFi measured: **wired host ↔ macOS host on WiFi, 256 B
//! p50≈4342 µs** (0 lost, full discovery). The trigger for a `participants=0`
//! discovery timeout is 802.11 power-save on the WiFi client (the NIC drops the
//! latency-sensitive unicast SPDP frames during the idle discovery window —
//! A/B-proven: `sudo tcpdump` keeps the NIC awake → discovery runs immediately).
//! **But the DDS stack carries its share:** ZeroDDS now sends an
//! **initial-announcement burst** at startup (`RuntimeConfig::
//! initial_announce_count` × `initial_announce_period`, default 10 × 200 ms,
//! until matched — analogous to Fast DDS) instead of a single beacon + 5 s
//! period, so a lost first beacon no longer means `participants=0`. The burst
//! also keeps the NIC awake via frequent TX. Residual power-save tuning stays an
//! OS/AP concern, but is no longer the *sole* mitigation.
//! Detail: `internal/interop/ros2-c3-large-data-wifi-followup.md`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
    dds_type::{DdsType, DecodeError, EncodeError},
    qos::ReliabilityKind,
};

#[derive(Debug, Clone, PartialEq)]
struct LatencyMsg {
    seq: u32,
    data: Vec<u8>,
}

impl DdsType for LatencyMsg {
    const TYPE_NAME: &'static str = "zerodds::LatencyMsg";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "LatencyMsg: short",
            });
        }
        let seq = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let end = (8 + n).min(bytes.len());
        Ok(Self {
            seq,
            data: bytes[8..end].to_vec(),
        })
    }
}

fn pct(sorted_us: &[u64], p: f64) -> u64 {
    if sorted_us.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted_us.len() - 1) as f64).round() as usize;
    sorted_us[idx.min(sorted_us.len() - 1)]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let size: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);
    let count: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let payload = vec![0xABu8; size];

    let factory = DomainParticipantFactory::instance();
    // Default config (like largedata, which discovers over WiFi). ZeroDDS↔ZeroDDS
    // does not need ros_defaults/XCDR1. ZERODDS_PEERS/NO_MULTICAST apply.
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let ping_topic = participant.create_topic::<LatencyMsg>("rt/ping", TopicQos::default())?;
    let pong_topic = participant.create_topic::<LatencyMsg>("rt/pong", TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let writer =
        publisher.create_datawriter::<LatencyMsg>(&ping_topic, DataWriterQos::default())?;
    let mut rqos = DataReaderQos::default();
    rqos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber.create_datareader::<LatencyMsg>(&pong_topic, rqos)?;

    println!("latency_ping: {size} B payload, {count} RTTs (+10 warmup), waiting for discovery");
    // Wait for the matched pong (rt/pong writer discovered) instead of
    // blindly sleeping 8s. Diagnostic: shows which discovery direction stalls.
    let disc_deadline = Instant::now() + Duration::from_secs(40);
    while Instant::now() < disc_deadline {
        let parts = participant.discovered_participants_count();
        let pubs = participant.discovered_publications_count();
        let subs = participant.discovered_subscriptions_count();
        if pubs > 0 && subs > 0 {
            println!("DIAG: discovery ok (participants={parts} pubs={pubs} subs={subs})");
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
        if Instant::now() >= disc_deadline {
            println!("DIAG: discovery TIMEOUT (participants={parts} pubs={pubs} subs={subs})");
        }
    }
    std::thread::sleep(Duration::from_millis(500)); // let reliable handshake settle

    let warmup = 10u32;
    let total = warmup + count as u32;
    let mut rtts_us: Vec<u64> = Vec::with_capacity(count);
    let mut lost = 0u32;
    for seq in 0..total {
        let t0 = Instant::now();
        writer.write(&LatencyMsg {
            seq,
            data: payload.clone(),
        })?;
        // Wait for the echo with the matching seq (max 2s).
        let echo_deadline = Instant::now() + Duration::from_secs(2);
        let mut got = false;
        while Instant::now() < echo_deadline {
            match reader.wait_for_data(Duration::from_millis(500)) {
                Ok(()) => {
                    for s in reader.take()? {
                        if s.seq == seq {
                            let rtt = t0.elapsed().as_micros() as u64;
                            if seq >= warmup {
                                rtts_us.push(rtt);
                            }
                            got = true;
                        }
                    }
                }
                Err(DdsError::Timeout) => {}
                Err(e) => return Err(e.into()),
            }
            if got {
                break;
            }
        }
        if !got && seq >= warmup {
            lost += 1;
        }
    }

    rtts_us.sort_unstable();
    if rtts_us.is_empty() {
        println!("== no RTTs measured (no pong?) ==");
        std::process::exit(2);
    }
    let n = rtts_us.len();
    let mean = rtts_us.iter().sum::<u64>() as f64 / n as f64;
    println!(
        "== RTT ({size} B, {n} samples, {lost} lost): min={} p50={} p90={} p99={} max={} mean={:.0} (µs) ==",
        rtts_us[0],
        pct(&rtts_us, 50.0),
        pct(&rtts_us, 90.0),
        pct(&rtts_us, 99.0),
        rtts_us[n - 1],
        mean
    );
    Ok(())
}
