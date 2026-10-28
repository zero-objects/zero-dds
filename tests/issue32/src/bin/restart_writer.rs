//! #32 writer-restart reproducer — ZeroDDS writer, one "generation" per process.
//!
//! Each generation is a fresh OS process (new participant, normally a new GUID
//! prefix). Generation 1 establishes the stream; the orchestrator then ends
//! this process (graceful self-exit → participant drop → SEDP dispose, or an
//! external SIGKILL → hard death) and starts generation 2 with a higher seq
//! base. The writer reports whether the RESTARTED participant re-learns the
//! persistent reader's subscription (matched-subscription count) — the key
//! question of #32.
//!
//! Args: <domain> <topic> <generation> <cap_seconds> <reliable|besteffort>
//!       <start_seq> <match_timeout_seconds> [stop_file]
//!
//! `cap_seconds` is only a safety cap. Generation-1 lifetime is controlled by
//! the orchestrator, measured from the parseable `WRITER_STREAMING` event:
//!   - graceful: the orchestrator creates `stop_file`; the writer leaves its
//!     loop cooperatively and returns, so Rust drops the participant and emits
//!     an SEDP dispose;
//!   - hard: the orchestrator sends SIGKILL; no drop, no dispose.
//! Machine-readable stdout: WRITER, WRITER_MATCHED / WRITER_NOMATCH,
//! WRITER_STREAMING, WRITER_SENT, WRITER_STOPPING, WRITER_DONE.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_qos::ReliabilityKind;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let domain: i32 = a.get(1).and_then(|x| x.parse().ok()).expect("arg1 domain");
    let topic = a.get(2).cloned().unwrap_or_else(|| "Restart".into());
    let generation: u32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(1);
    let secs: u64 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(15);
    let reliable = a.get(5).map(|x| x != "besteffort").unwrap_or(true);
    let start_seq: u64 = a.get(6).and_then(|x| x.parse().ok()).unwrap_or(0);
    let match_to: u64 = a.get(7).and_then(|x| x.parse().ok()).unwrap_or(10);
    let stop_file = a.get(8).cloned();

    let pid = std::process::id();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .expect("create_participant");
    let topic_h = participant
        .create_topic::<RawBytes>(&topic, TopicQos::default())
        .expect("create_topic");
    let publisher = participant.create_publisher(PublisherQos::default());
    let mut wqos = DataWriterQos::default();
    wqos.reliability.kind = if reliable {
        ReliabilityKind::Reliable
    } else {
        ReliabilityKind::BestEffort
    };
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic_h, wqos)
        .expect("create_datawriter");

    println!(
        "WRITER pid={pid} gen={generation} domain={domain} topic={topic} reliability={}",
        if reliable { "reliable" } else { "besteffort" }
    );

    // The decisive #32 signal: does THIS (possibly restarted) writer re-learn
    // the persistent reader's SEDP subscription and reach matched > 0?
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
    let parts: Vec<String> = participant
        .get_discovered_participants()
        .iter()
        .map(|h| format!("{h:?}"))
        .collect();
    println!(
        "WRITER_DISCOVERY gen={generation} discovered={} parts=[{}]",
        participant.discovered_participants_count(),
        parts.join(",")
    );

    let stop_requested = || {
        stop_file
            .as_ref()
            .is_some_and(|f| std::path::Path::new(f).exists())
    };

    let start = Instant::now();
    let mut seq = start_seq;
    let mut sent = 0u64;
    let mut streaming_logged = false;
    // The loop runs until the orchestrator requests a cooperative stop (graceful)
    // or SIGKILLs the process (hard). `secs` is only a safety cap so a hung
    // orchestrator cannot leak the process.
    while start.elapsed() < Duration::from_secs(secs) {
        if stop_requested() {
            break;
        }
        let payload = format!("gen={generation} seq={seq}");
        match writer.write(&RawBytes::new(payload.into_bytes())) {
            Ok(()) => {
                sent += 1;
                if !streaming_logged {
                    streaming_logged = true;
                    // Readiness/streaming anchor: the orchestrator measures the
                    // identical gen-1 lifetime for graceful and hard from here.
                    println!(
                        "WRITER_STREAMING gen={generation} pid={pid} seq={seq} t={:.2} matched={}",
                        start.elapsed().as_secs_f64(),
                        writer.matched_subscription_count()
                    );
                }
                if sent % 5 == 0 {
                    println!(
                        "WRITER_SENT gen={generation} seq={seq} matched={}",
                        writer.matched_subscription_count()
                    );
                }
            }
            Err(e) => println!("WRITER_WRITE_ERR gen={generation} seq={seq} {e:?}"),
        }
        seq += 1;
        std::thread::sleep(Duration::from_millis(300));
    }

    if stop_requested() {
        // Cooperative shutdown path: fall out of main so the participant, its
        // publisher and writer are dropped in order (SEDP dispose).
        println!("WRITER_STOPPING gen={generation} pid={pid} sent={sent} reason=cooperative");
    }
    println!(
        "WRITER_DONE gen={generation} pid={pid} sent={sent} matched={}",
        writer.matched_subscription_count()
    );
    // Normal return drops the participant → graceful SEDP dispose. An external
    // SIGKILL from the orchestrator skips this and models hard death.
}
