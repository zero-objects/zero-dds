//! #32 typed reporter-equivalent writer — generated `Robot { id, label }`.
//!
//! Mirrors restart_writer but exchanges the codegen `Robot` type (from
//! idl/Robot.idl via zerodds-idlc) instead of RawBytes, so the final #32
//! acceptance asserts concrete typed generation-2 values rather than an ASCII
//! payload. Generation `g` writes `Robot { id: start_seq + seq, label: g }`;
//! the orchestrator gives generation 2 a disjoint `start_seq` so its id range
//! is distinguishable on the reader.
//!
//! Args: <domain> <topic> <generation> <cap_seconds> <reliable|besteffort>
//!       <start_seq> <match_timeout_seconds> [stop_file]
//! Machine-readable stdout: WRITER, WRITER_MATCHED / WRITER_NOMATCH,
//! WRITER_STREAMING, WRITER_SENT, WRITER_STOPPING, WRITER_DONE.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::{Duration, Instant};

use issue32_writer_restart::robot::Robot;
use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
};
use zerodds_qos::ReliabilityKind;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let domain: i32 = a.get(1).and_then(|x| x.parse().ok()).expect("arg1 domain");
    let topic = a.get(2).cloned().unwrap_or_else(|| "RobotRestart".into());
    let generation: u32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(1);
    let secs: u64 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(120);
    let reliable = a.get(5).map(|x| x != "besteffort").unwrap_or(true);
    let start_seq: u32 = a.get(6).and_then(|x| x.parse().ok()).unwrap_or(0);
    let match_to: u64 = a.get(7).and_then(|x| x.parse().ok()).unwrap_or(10);
    let stop_file = a.get(8).cloned();

    let pid = std::process::id();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .expect("create_participant");
    let topic_h = participant
        .create_topic::<Robot>(&topic, TopicQos::default())
        .expect("create_topic");
    let publisher = participant.create_publisher(PublisherQos::default());
    let mut wqos = DataWriterQos::default();
    wqos.reliability.kind = if reliable {
        ReliabilityKind::Reliable
    } else {
        ReliabilityKind::BestEffort
    };
    let writer = publisher
        .create_datawriter::<Robot>(&topic_h, wqos)
        .expect("create_datawriter");

    println!(
        "WRITER pid={pid} gen={generation} domain={domain} topic={topic} type=Robot reliability={}",
        if reliable { "reliable" } else { "besteffort" }
    );

    match writer.wait_for_matched_subscription(1, Duration::from_secs(match_to)) {
        Ok(()) => println!(
            "WRITER_MATCHED gen={generation} count={}",
            writer.matched_subscription_count()
        ),
        Err(e) => println!(
            "WRITER_NOMATCH gen={generation} matched={} err={e:?}",
            writer.matched_subscription_count()
        ),
    }

    let stop_requested = || {
        stop_file
            .as_ref()
            .is_some_and(|f| std::path::Path::new(f).exists())
    };
    let start = Instant::now();
    let mut seq: u32 = 0;
    let mut sent = 0u64;
    let mut streaming_logged = false;
    while start.elapsed() < Duration::from_secs(secs) {
        if stop_requested() {
            break;
        }
        let sample = Robot {
            id: start_seq.wrapping_add(seq),
            label: generation,
        };
        match writer.write(&sample) {
            Ok(()) => {
                sent += 1;
                if !streaming_logged {
                    streaming_logged = true;
                    println!(
                        "WRITER_STREAMING gen={generation} pid={pid} id={} label={} t={:.2} matched={}",
                        sample.id,
                        sample.label,
                        start.elapsed().as_secs_f64(),
                        writer.matched_subscription_count()
                    );
                }
                if sent % 5 == 0 {
                    println!(
                        "WRITER_SENT gen={generation} id={} label={} matched={}",
                        sample.id,
                        sample.label,
                        writer.matched_subscription_count()
                    );
                }
            }
            Err(e) => println!("WRITER_WRITE_ERR gen={generation} id={} {e:?}", sample.id),
        }
        seq = seq.wrapping_add(1);
        std::thread::sleep(Duration::from_millis(300));
    }

    if stop_requested() {
        println!("WRITER_STOPPING gen={generation} pid={pid} sent={sent} reason=cooperative");
    }
    println!(
        "WRITER_DONE gen={generation} pid={pid} sent={sent} matched={}",
        writer.matched_subscription_count()
    );
}
