//! #32 typed reporter-equivalent reader — generated `Robot { id, label }`.
//!
//! Persistent, timeout-tolerant reader for the typed acceptance cell. It takes
//! `Robot` samples (from idl/Robot.idl via zerodds-idlc) and asserts CONCRETE
//! generation-2 values: a generation-2 sample (`label == 2`) must carry an
//! `id` in the disjoint generation-2 range (`id >= GEN2_MIN_ID`). Match alone
//! is never a pass; a run with zero valid generation-2 samples fails.
//!
//! Args: <domain> <topic> <run_seconds> <reliable|besteffort> [gen2_min_id]
//! Machine-readable stdout: READER, READER_READY, ROBOT_SAMPLE,
//! ROBOT_GEN2_OK / ROBOT_GEN2_BAD, READER_STAT, READER_DONE.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use issue32_writer_restart::robot::Robot;
use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, SubscriberQos,
    TopicQos,
};
use zerodds_qos::ReliabilityKind;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let domain: i32 = a.get(1).and_then(|x| x.parse().ok()).expect("arg1 domain");
    let topic = a.get(2).cloned().unwrap_or_else(|| "RobotRestart".into());
    let secs: u64 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(60);
    let reliable = a.get(4).map(|x| x != "besteffort").unwrap_or(true);
    let gen2_min_id: u32 = a.get(5).and_then(|x| x.parse().ok()).unwrap_or(100_000);

    let pid = std::process::id();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant(domain, DomainParticipantQos::default())
        .expect("create_participant");
    let topic_h = participant
        .create_topic::<Robot>(&topic, TopicQos::default())
        .expect("create_topic");
    let sub = participant.create_subscriber(SubscriberQos::default());
    let mut rqos = DataReaderQos::default();
    rqos.reliability.kind = if reliable {
        ReliabilityKind::Reliable
    } else {
        ReliabilityKind::BestEffort
    };
    let reader = sub
        .create_datareader::<Robot>(&topic_h, rqos)
        .expect("create_datareader");

    println!(
        "READER pid={pid} domain={domain} topic={topic} type=Robot reliability={} gen2_min_id={gen2_min_id}",
        if reliable { "reliable" } else { "besteffort" }
    );
    println!("READER_READY pid={pid}");

    let start = Instant::now();
    let mut gen_count: BTreeMap<u32, u64> = BTreeMap::new();
    let mut gen2_ok: u64 = 0;
    let mut gen2_bad: u64 = 0;
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
                        let robot = &s.data;
                        let gen = robot.label;
                        *gen_count.entry(gen).or_default() += 1;
                        println!(
                            "ROBOT_SAMPLE t={:.1} gen={gen} id={} label={} pub={:?} istate={:?}",
                            start.elapsed().as_secs_f64(),
                            robot.id,
                            robot.label,
                            s.info.publication_handle,
                            s.info.instance_state
                        );
                        // Concrete typed generation-2 assertion: a label==2
                        // sample must carry an id in the disjoint gen-2 range.
                        if gen == 2 {
                            if robot.id >= gen2_min_id {
                                gen2_ok += 1;
                                println!("ROBOT_GEN2_OK id={} label={}", robot.id, robot.label);
                            } else {
                                gen2_bad += 1;
                                println!(
                                    "ROBOT_GEN2_BAD id={} label={} (expected id >= {gen2_min_id})",
                                    robot.id, robot.label
                                );
                            }
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
            let genstr: Vec<String> = gen_count
                .iter()
                .map(|(g, c)| format!("gen{g}={c}"))
                .collect();
            println!(
                "READER_STAT t={:.1} alive matched={} discovered={} {} gen2_ok={gen2_ok} gen2_bad={gen2_bad} timeouts={timeouts} errors={errors}",
                start.elapsed().as_secs_f64(),
                reader.matched_publication_count(),
                participant.discovered_participants_count(),
                genstr.join(" ")
            );
        }
    }

    let genstr: Vec<String> = gen_count
        .iter()
        .map(|(g, c)| format!("gen{g}={c}"))
        .collect();
    println!(
        "READER_DONE pid={pid} matched={} {} gen2_ok={gen2_ok} gen2_bad={gen2_bad} timeouts={timeouts} errors={errors}",
        reader.matched_publication_count(),
        genstr.join(" ")
    );
}
