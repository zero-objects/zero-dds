// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
// zerodds-lint: allow no_dyn_in_safe (the error type of an example binary's main and run functions)
//! The subscriber of `zerodds_perf` (crates/dcps/examples) with a loaned
//! take: `zerodds_perf_loan sub <runtime_secs>`.
//!
//! It reads the stream `zerodds_perf pub` writes on the topic `PerfTest`.
//! Where `zerodds_perf sub` takes a `ShapeType` the reader decoded and owns
//! (`DataReader::take`), this one takes the samples as the octets they
//! arrived in (`DataReader::take_loan`) and reads each as a type whose
//! string and octet sequence borrow from them (`LoanExt::read_as`), so no
//! sample is decoded into an owned value and none of its payload is copied.
//!
//! The settings and the output lines are those of `zerodds_perf sub`, so
//! the speed setup (`tests/perf/speed`) parses both alike:
//! `ZERODDS_PERF_DOMAIN`, `ZERODDS_PERF_RELIABILITY`, `ZERODDS_PERF_HISTORY`,
//! `ZERODDS_PERF_MAX_SAMPLES`, `ZERODDS_PERF_SUB_IDLE_EXIT_MS`. It takes
//! when its listener wakes it; LATENCY_BUDGET stays the default.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_codec::{DdsType, LoanExt};
use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
};

const PERF_TOPIC: &str = "PerfTest";

/// OMG `shape.idl`'s `ShapeType` with the fields that have a length
/// borrowed: the type `zerodds_dcps::interop::ShapeType` is on the wire.
#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "ShapeType", extensibility = "appendable")]
struct ShapeView<'a> {
    #[dds(key)]
    color: &'a str,
    x: i32,
    y: i32,
    shapesize: i32,
    additional_payload_size: &'a [u8],
}

fn env_usize(key: &str, default: usize) -> usize {
    match env::var(key) {
        Ok(v) => v.trim().parse().unwrap_or_else(|_| {
            eprintln!("{key}={v:?} is not a number; using {default}");
            default
        }),
        Err(_) => default,
    }
}

fn perf_domain() -> i32 {
    i32::try_from(env_usize("ZERODDS_PERF_DOMAIN", 0)).unwrap_or(0)
}

fn perf_reliability() -> ReliabilityKind {
    match env::var("ZERODDS_PERF_RELIABILITY")
        .unwrap_or_else(|_| String::from("besteffort"))
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "reliable" => ReliabilityKind::Reliable,
        "besteffort" | "best_effort" | "be" => ReliabilityKind::BestEffort,
        other => {
            eprintln!("ZERODDS_PERF_RELIABILITY={other:?} unknown; using besteffort");
            ReliabilityKind::BestEffort
        }
    }
}

fn perf_history() -> HistoryQosPolicy {
    let raw = env::var("ZERODDS_PERF_HISTORY").unwrap_or_else(|_| String::from("1"));
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("all") {
        return HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 0,
        };
    }
    let depth = raw
        .parse::<i32>()
        .ok()
        .filter(|d| *d > 0)
        .unwrap_or_else(|| {
            eprintln!("ZERODDS_PERF_HISTORY={raw:?} is neither `all` nor a depth; using 1");
            1
        });
    HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth,
    }
}

fn perf_reader_qos() -> DataReaderQos {
    let max = i32::try_from(env_usize("ZERODDS_PERF_MAX_SAMPLES", 1000)).unwrap_or(1000);
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: perf_reliability(),
            ..DataReaderQos::default().reliability
        },
        history: perf_history(),
        resource_limits: ResourceLimitsQosPolicy {
            max_samples: max,
            max_instances: 10,
            max_samples_per_instance: max,
        },
        ..DataReaderQos::default()
    }
}

/// CPU time of the whole process in nanoseconds (`utime + stime` of
/// `/proc/self/stat`, in ticks of 1/100 s); `None` without procfs.
fn process_cpu_ns() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    let mut fields = rest.split_ascii_whitespace();
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    Some((utime + stime) * 10_000_000)
}

struct WakeOnData(std::thread::Thread);

impl DataReaderListener for WakeOnData {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.unpark();
    }
}

fn run_sub(runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(perf_domain(), DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeView<'static>>(PERF_TOPIC, TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber.create_datareader::<ShapeView<'static>>(&topic, perf_reader_qos())?;
    println!(
        "# sub: domain={} reliability={:?} history={:?} take=loan",
        perf_domain(),
        perf_reliability(),
        perf_history().kind
    );
    reader.set_listener(
        Some(Arc::new(WakeOnData(std::thread::current()))),
        status::DATA_AVAILABLE,
    );

    let idle_exit = env::var("ZERODDS_PERF_SUB_IDLE_EXIT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_millis);
    let start = Instant::now();
    let mut total = 0u64;
    let mut unread = 0u64;
    let mut payload_octets = 0u64;
    let mut first_seq: Option<i32> = None;
    let mut last_seq = 0i32;
    let mut last_report = start;
    let mut last_total = 0u64;
    let mut next_cpu_read = start;
    let mut cpu_seen = 0u64;
    let mut cpu_first: Option<(u64, u64)> = None;
    let mut cpu_last: (u64, u64) = (0, 0);
    let mut last_progress = start;

    while start.elapsed() < runtime {
        let mut taken = 0usize;
        if let Ok(loan) = reader.take_loan() {
            taken = loan.len();
            for index in 0..taken {
                match loan.read_as::<ShapeView<'_>>(index) {
                    Some(Ok(sample)) => {
                        if first_seq.is_none() {
                            first_seq = Some(sample.shapesize);
                        }
                        last_seq = sample.shapesize;
                        payload_octets += sample.additional_payload_size.len() as u64;
                        total += 1;
                    }
                    // A sample without data (a dispose) or one that does
                    // not read: counted, and named at the end.
                    Some(Err(_)) | None => unread += 1,
                }
            }
        }
        if last_report.elapsed() >= Duration::from_secs(1) {
            let delta = total - last_total;
            let rate_ks = (delta as f64 / last_report.elapsed().as_secs_f64()) / 1000.0;
            println!(
                "{:.3}  size N total {total} delta {delta} rate {rate_ks:.2} kS/s",
                start.elapsed().as_secs_f64()
            );
            last_report = Instant::now();
            last_total = total;
        }
        let now = Instant::now();
        if now >= next_cpu_read {
            next_cpu_read = now + Duration::from_millis(20);
            if total != cpu_seen {
                cpu_seen = total;
                last_progress = now;
                if let Some(cpu) = process_cpu_ns() {
                    if cpu_first.is_none() {
                        cpu_first = Some((cpu, total));
                    }
                    cpu_last = (cpu, total);
                }
            } else if idle_exit
                .is_some_and(|idle| cpu_seen > 0 && now.duration_since(last_progress) >= idle)
            {
                break;
            }
        }
        if taken == 0 {
            std::thread::park_timeout(Duration::from_millis(100));
        }
    }
    if let Some((cpu_a, cnt_a)) = cpu_first {
        println!(
            "# sub-cpu: cpu_ns={} cpu_samples={}",
            cpu_last.0.saturating_sub(cpu_a),
            cpu_last.1.saturating_sub(cnt_a)
        );
    }
    let first = first_seq.unwrap_or(0);
    let span = i64::from(last_seq) - i64::from(first) + 1;
    let missing = span - total as i64;
    let loss_pct = if span > 0 {
        (missing as f64 / span as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "# sub-done: total={total} runtime={:.3}s first_seq={first} last_seq={last_seq} \
         span={span} missing={missing} loss={loss_pct:.2}% poll_us=listener",
        start.elapsed().as_secs_f64(),
    );
    println!("# sub-loan: unread={unread} payload_octets={payload_octets}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("sub"), Some(secs)) => run_sub(Duration::from_secs(secs.parse()?)),
        _ => {
            eprintln!("usage: zerodds_perf_loan sub <runtime_secs>");
            std::process::exit(2);
        }
    }
}
