//! #32 writer-restart reproducer — persistent, timeout-tolerant ZeroDDS reader.
//!
//! Fixes the submitted reproducer's `wait_for_data(10s).unwrap()`: a wait
//! timeout is an idle tick, NOT process death. This reader stays alive for the
//! whole run, logs its liveness, match, discovery and per-generation sample
//! delivery separately, and records the source writer handle + instance state
//! of every sample so a stopped stream after a writer restart is distinguished
//! from a dead reader.
//!
//! Payload contract (from restart_writer): ASCII `gen=<u32> seq=<u64>`.
//!
//! Args: <domain> <topic> <run_seconds> <reliable|besteffort>
//! Machine-readable stdout lines: READER, READER_READY, READER_SAMPLE,
//! READER_STAT (once per second), READER_DONE.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, RawBytes,
    SubscriberQos, TopicQos,
};
use zerodds_qos::ReliabilityKind;

fn parse_gen_seq(b: &[u8]) -> Option<(u32, u64)> {
    let s = std::str::from_utf8(b).ok()?;
    let mut g = None;
    let mut q = None;
    for tok in s.split_whitespace() {
        if let Some(v) = tok.strip_prefix("gen=") {
            g = v.parse().ok();
        }
        if let Some(v) = tok.strip_prefix("seq=") {
            q = v.parse().ok();
        }
    }
    Some((g?, q?))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let domain: i32 = a.get(1).and_then(|x| x.parse().ok()).expect("arg1 domain");
    let topic = a.get(2).cloned().unwrap_or_else(|| "Restart".into());
    let secs: u64 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(60);
    let reliable = a.get(4).map(|x| x != "besteffort").unwrap_or(true);

    let pid = std::process::id();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .expect("create_participant");
    let topic_h = participant
        .create_topic::<RawBytes>(&topic, TopicQos::default())
        .expect("create_topic");
    let sub = participant.create_subscriber(SubscriberQos::default());
    let mut rqos = DataReaderQos::default();
    rqos.reliability.kind = if reliable {
        ReliabilityKind::Reliable
    } else {
        ReliabilityKind::BestEffort
    };
    let reader = sub
        .create_datareader::<RawBytes>(&topic_h, rqos)
        .expect("create_datareader");

    println!(
        "READER pid={pid} domain={domain} topic={topic} reliability={}",
        if reliable { "reliable" } else { "besteffort" }
    );
    println!("READER_READY pid={pid}");

    let start = Instant::now();
    let mut gen_count: BTreeMap<u32, u64> = BTreeMap::new();
    let mut gen_lastseq: BTreeMap<u32, u64> = BTreeMap::new();
    let mut errors: u64 = 0;
    let mut timeouts: u64 = 0;
    let mut last_stat = Instant::now();

    while start.elapsed() < Duration::from_secs(secs) {
        match reader.wait_for_data(Duration::from_millis(500)) {
            Ok(()) => match reader.take_with_info() {
                Ok(samples) => {
                    for s in samples {
                        if !s.info.valid_data {
                            println!(
                                "READER_LIFECYCLE t={:.1} istate={:?} pub={:?}",
                                start.elapsed().as_secs_f64(),
                                s.info.instance_state,
                                s.info.publication_handle
                            );
                            continue;
                        }
                        if let Some((g, q)) = parse_gen_seq(&s.data.data) {
                            *gen_count.entry(g).or_default() += 1;
                            gen_lastseq.insert(g, q);
                            println!(
                                "READER_SAMPLE t={:.1} gen={g} seq={q} pub={:?} istate={:?}",
                                start.elapsed().as_secs_f64(),
                                s.info.publication_handle,
                                s.info.instance_state
                            );
                        }
                    }
                }
                Err(e) => {
                    errors += 1;
                    println!("READER_TAKE_ERR {e:?}");
                }
            },
            Err(DdsError::Timeout) => timeouts += 1,
            Err(e) => {
                errors += 1;
                println!("READER_WAIT_ERR {e:?}");
            }
        }

        if last_stat.elapsed() >= Duration::from_secs(1) {
            last_stat = Instant::now();
            let parts: Vec<String> = participant
                .get_discovered_participants()
                .iter()
                .map(|h| format!("{h:?}"))
                .collect();
            let genstr: Vec<String> = gen_count
                .iter()
                .map(|(g, c)| {
                    format!(
                        "gen{g}={c}(last={})",
                        gen_lastseq.get(g).copied().unwrap_or(0)
                    )
                })
                .collect();
            println!(
                "READER_STAT t={:.1} alive matched={} discovered={} parts=[{}] {} timeouts={timeouts} errors={errors}",
                start.elapsed().as_secs_f64(),
                reader.matched_publication_count(),
                participant.discovered_participants_count(),
                parts.join(","),
                genstr.join(" ")
            );
        }
    }

    let genstr: Vec<String> = gen_count
        .iter()
        .map(|(g, c)| format!("gen{g}={c}"))
        .collect();
    println!(
        "READER_DONE pid={pid} matched={} {} timeouts={timeouts} errors={errors}",
        reader.matched_publication_count(),
        genstr.join(" ")
    );
}
