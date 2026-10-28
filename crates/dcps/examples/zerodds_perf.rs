//! ZeroDDS performance bench binary (CI-3b wave).
//!
//! Three modes:
//! * `zerodds_perf pub <size> <runtime_secs>` — writes ShapeType samples
//!   as fast as possible (write-best-effort), counts total+rate.
//! * `zerodds_perf sub <runtime_secs>` — reads and counts received samples,
//!   measures the rate every second.
//! * `zerodds_perf pingpong <runtime_secs>` — round-trip-time test:
//!   sends PING on topic "PerfPing", waits for PONG on "PerfPong".
//!   Measures RTT in µs (timestamp encoded in shapesize i32).
//!
//! Set `ZERODDS_PERF_DOMAIN` to keep concurrent measurements apart; see
//! [`perf_domain`]. Without it every run lands on domain 0 together.
//!
//! # Output format
//!
//! ddsperf-compatible form (so that `llvm_bench_runner.sh` can use the
//! same regex parser):
//! ```text
//! 1.000  size 1024 total 1234 rate 1234.5 kS/s 9876.5 Mb/s
//! 1.000  rtt mean 145us min 100 50% 130 90% 180 99% 250 max 500
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zerodds_dcps::InstanceHandle;
use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    SubscriberQos, TopicQos,
};

const PERF_TOPIC: &str = "PerfTest";
const PING_TOPIC: &str = "PerfPing";
const PONG_TOPIC: &str = "PerfPong";

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

/// The domain to measure on, from `ZERODDS_PERF_DOMAIN` (default 0).
///
/// All four modes used to hard-code domain 0. Two runs started on the same
/// host then joined the same domain and discovered each other's endpoints:
/// a `pub` meant for one measurement fed the `sub` of another, and the rate
/// each reported was the sum of writers it never knew about. Measured on
/// the Linux test host, a parallel pair on domain 0 completed 2 of 10 runs against
/// 10 of 10 once the runs were separated.
///
/// `tests/perf/dds-roundtrip-bench/self_compare.sh` gives every cell its own
/// domain for the same reason; this is the same knob for the throughput
/// binary, and it is an environment variable rather than a positional
/// argument so the existing three call shapes keep working unchanged.
fn perf_domain() -> i32 {
    match env::var("ZERODDS_PERF_DOMAIN") {
        Ok(v) => v.trim().parse().unwrap_or_else(|_| {
            eprintln!("ZERODDS_PERF_DOMAIN={v:?} is not a domain id; using 0");
            0
        }),
        Err(_) => 0,
    }
}

/// Reads a `usize` knob from the environment, or `default` when unset or
/// unparseable.
fn env_usize(key: &str, default: usize) -> usize {
    match env::var(key) {
        Ok(v) => v.trim().parse().unwrap_or_else(|_| {
            eprintln!("{key}={v:?} is not a number; using {default}");
            default
        }),
        Err(_) => default,
    }
}

/// Reliability for **both** endpoints, from `ZERODDS_PERF_RELIABILITY`
/// (`reliable` | `besteffort`, default `besteffort`).
///
/// The shipped defaults are asymmetric — a `DataWriterQos::default()` is
/// RELIABLE and a `DataReaderQos::default()` is BEST_EFFORT, both per spec
/// (§2.2.3.14.3). That pair matches, and the resulting stream is best-effort:
/// a best-effort RTPS reader never sends an ACKNACK, so nothing is repaired.
/// A run that leaves the defaults in place therefore measures a best-effort
/// leg while its command line says nothing about reliability, and cannot be
/// compared against a vendor run whose defaults differ. This knob makes the
/// leg part of the invocation.
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

/// History for **both** endpoints, from `ZERODDS_PERF_HISTORY`
/// (`all` | depth, default `1` = the shipped default).
fn perf_history() -> HistoryQosPolicy {
    let raw = env::var("ZERODDS_PERF_HISTORY").unwrap_or_else(|_| String::from("1"));
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("all") {
        return HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 0,
        };
    }
    match raw.parse::<i32>() {
        Ok(d) if d > 0 => HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: d,
        },
        _ => {
            eprintln!("ZERODDS_PERF_HISTORY={raw:?} is neither `all` nor a depth; using 1");
            HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: 1,
            }
        }
    }
}

/// Resource limits for **both** endpoints. `max_samples` follows
/// `ZERODDS_PERF_MAX_SAMPLES` (default 1000 = the shipped default);
/// `max_samples_per_instance` follows it as well, because the perf topic is
/// a single-instance stream and the shipped per-instance cap of 100 would
/// otherwise bound a KEEP_ALL run at 100 regardless of `max_samples`.
fn perf_resource_limits() -> ResourceLimitsQosPolicy {
    let max = i32::try_from(env_usize("ZERODDS_PERF_MAX_SAMPLES", 1000)).unwrap_or(1000);
    ResourceLimitsQosPolicy {
        max_samples: max,
        max_instances: 10,
        max_samples_per_instance: max,
    }
}

/// LATENCY_BUDGET for **both** endpoints, from
/// `ZERODDS_PERF_LATENCY_BUDGET_NS` (default `0` = the spec default,
/// §2.2.3.10.3 "as fast as possible").
///
/// Set on both sides because the policy is request/offered: offered >
/// requested makes the pair incompatible (§2.2.3.10), so a run that raised
/// only the writer's budget would measure an unmatched pair.
fn perf_latency_budget() -> LatencyBudgetQosPolicy {
    let ns = env_usize("ZERODDS_PERF_LATENCY_BUDGET_NS", 0) as u64;
    LatencyBudgetQosPolicy {
        duration: zerodds_qos::Duration::from_sec_nanosec(
            i32::try_from(ns / 1_000_000_000).unwrap_or(i32::MAX),
            (ns % 1_000_000_000) as u32,
        ),
    }
}

/// The writer QoS a measurement runs under — see [`perf_reliability`].
fn perf_writer_qos() -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: perf_reliability(),
            ..DataWriterQos::default().reliability
        },
        history: perf_history(),
        resource_limits: perf_resource_limits(),
        latency_budget: perf_latency_budget(),
        ..DataWriterQos::default()
    }
}

/// The reader QoS a measurement runs under — see [`perf_reliability`].
fn perf_reader_qos() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: perf_reliability(),
            ..DataReaderQos::default().reliability
        },
        history: perf_history(),
        resource_limits: perf_resource_limits(),
        latency_budget: perf_latency_budget(),
        ..DataReaderQos::default()
    }
}

/// One line naming the QoS a run measured under, so a number in a log can
/// never be read without the leg that produced it.
fn perf_qos_banner(role: &str) -> String {
    let h = perf_history();
    let hist = match h.kind {
        HistoryKind::KeepAll => String::from("keep_all"),
        HistoryKind::KeepLast => format!("keep_last({})", h.depth),
    };
    let rel = match perf_reliability() {
        ReliabilityKind::Reliable => "reliable",
        ReliabilityKind::BestEffort => "besteffort",
    };
    let rl = perf_resource_limits();
    format!(
        "# {role}: domain={} reliability={rel} history={hist} max_samples={} \
         max_samples_per_instance={} latency_budget_ns={}",
        perf_domain(),
        rl.max_samples,
        rl.max_samples_per_instance,
        env_usize("ZERODDS_PERF_LATENCY_BUDGET_NS", 0)
    )
}

/// How long the `sub` poll loop sleeps between `take()` calls, in
/// microseconds, from `ZERODDS_PERF_SUB_POLL_US`; `0` polls without
/// sleeping. Unset, `sub` does not poll: it takes when its listener's
/// `on_data_available` wakes it, as `ddsperf` takes in its listener.
///
/// The loop used to sleep 1 ms between `take()` calls unconditionally.
/// Under `KEEP_LAST(1)` a reader that looks once a millisecond keeps at
/// most a sample a millisecond of each instance, so the loss column
/// measured the bench's loop: on the Linux test host it read about 500
/// samples a second of a 440 kS/s stream, and 91 % of it polling
/// without pause.
fn perf_sub_poll() -> Option<Duration> {
    env::var("ZERODDS_PERF_SUB_POLL_US")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_micros)
}

/// Wakes the `sub` loop when the reader has data.
struct WakeOnData(std::thread::Thread);

impl DataReaderListener for WakeOnData {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.unpark();
    }
}

/// Prints the runtime's write/receive phase accumulators, which are filled
/// only while `ZERODDS_PHASE_TIMING=1`. The runtime dumps them itself in its
/// `Drop`, but the factory is a process-global static, so a bench binary that
/// simply returns from `main` never reaches that dump. Printing them here
/// makes the sender and receiver hot-path breakdown available to a run.
fn print_phase_dump(role: &str) {
    use core::sync::atomic::Ordering as O;
    use zerodds_dcps::runtime::{
        PHASE_HANDLE_SUB_NS, PHASE_HANDLE_USER_CALLS, PHASE_HANDLE_USER_NS, PHASE_WRITE_SUB_NS,
        PHASE_WRITE_USER_CALLS, PHASE_WRITE_USER_NS,
    };
    if env::var("ZERODDS_PHASE_TIMING").ok().as_deref() != Some("1") {
        return;
    }
    let per = |ns: u64, n: u64| {
        if n > 0 {
            ns as f64 / n as f64 / 1000.0
        } else {
            0.0
        }
    };
    let wn = PHASE_WRITE_USER_CALLS.load(O::Relaxed);
    let wns = PHASE_WRITE_USER_NS.load(O::Relaxed);
    let hn = PHASE_HANDLE_USER_CALLS.load(O::Relaxed);
    let hns = PHASE_HANDLE_USER_NS.load(O::Relaxed);
    println!(
        "# phase[{role}] write_user_sample   N={wn} avg={:.3}us",
        per(wns, wn)
    );
    for (i, label) in [
        "slot_lookup",
        "slot_lock_acquire",
        "writer.write+frame",
        "dispatch(UDP+inproc)",
    ]
    .iter()
    .enumerate()
    {
        let v = PHASE_WRITE_SUB_NS[i].load(O::Relaxed);
        println!("# phase[{role}]   w.{label:<22} avg={:.3}us", per(v, wn));
    }
    println!(
        "# phase[{role}] handle_user_datagram N={hn} avg={:.3}us",
        per(hns, hn)
    );
    for (i, label) in [
        "decode_datagram",
        "slot_lookup+lock",
        "reader.handle_data",
        "delivered_to_user_sample",
        "listener+sender_dispatch",
    ]
    .iter()
    .enumerate()
    {
        let v = PHASE_HANDLE_SUB_NS[i].load(O::Relaxed);
        println!("# phase[{role}]   h.{label:<26} avg={:.3}us", per(v, hn));
    }
}

fn run_pub(size: usize, runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(perf_domain(), DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeType>(PERF_TOPIC, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher.create_datawriter::<ShapeType>(&topic, perf_writer_qos())?;
    println!("{}", perf_qos_banner("pub"));

    // Discovery wait
    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(5));

    // The size goes into OMG `shape.idl`'s `additional_payload_size`
    // sequence and not into the `@key` `color`, as `ddsperf` keeps its
    // baggage out of its key: a 1 kB key made every write hash 1 kB
    // (XTypes 1.3 §7.6.8) and every reader compare it, which is a cost of
    // the bench's key and not of the throughput it compares. The body stays
    // about `size` octets: DHEADER, `"BLUE"`, three `long`s and the
    // sequence's length are the 16 octets left out.
    //
    // One sample, its counter moved per write, as `ddsperf` writes one
    // sample over and over: a sample built per write cost two allocations
    // and a 1 kB copy of the bench's own, and the clock read twice per
    // write is read once per 256 writes.
    let mut sample =
        ShapeType::with_payload("BLUE", 0, 0, 0, vec![b'X'; size.saturating_sub(4 * 4)]);
    let start = Instant::now();
    let mut total = 0u64;
    let mut last_report = start;
    let mut samples_since_report = 0u64;
    let mut iterations = 0u64;

    loop {
        sample.shapesize = total as i32;
        if writer.write(&sample).is_ok() {
            total += 1;
            samples_since_report += 1;
        }
        iterations += 1;
        if iterations % 256 != 0 {
            continue;
        }
        if start.elapsed() >= runtime {
            break;
        }
        // Report every second
        if last_report.elapsed() >= Duration::from_secs(1) {
            let elapsed_s = last_report.elapsed().as_secs_f64();
            let rate_ks = (samples_since_report as f64 / elapsed_s) / 1000.0;
            let rate_mb =
                (samples_since_report as f64 * size as f64 * 8.0 / elapsed_s) / 1_000_000.0;
            println!(
                "{:.3}  size {} total {} rate {:.2} kS/s {:.2} Mb/s",
                start.elapsed().as_secs_f64(),
                size,
                total,
                rate_ks,
                rate_mb
            );
            last_report = Instant::now();
            samples_since_report = 0;
        }
    }
    println!(
        "# pub-done: total={total} runtime={:.3}s",
        start.elapsed().as_secs_f64()
    );
    // Under RELIABLE what the reader has not acknowledged is still this
    // writer's to deliver, and a writer that goes away discards it (DDS 1.4
    // §2.2.2.4.1.12). A reader on a loaded host is up to a whole history
    // behind when the loop ends: the throughput gate's CI runs counted
    // 99 744 of 488 162 samples lost that were never given the time.
    if let Err(e) = writer.wait_for_acknowledgments(Duration::from_secs(60)) {
        eprintln!("# pub: not every sample was acknowledged within 60 s: {e:?}");
    }
    print_phase_dump("pub");
    Ok(())
}

fn run_sub(runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(perf_domain(), DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeType>(PERF_TOPIC, TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber.create_datareader::<ShapeType>(&topic, perf_reader_qos())?;
    println!("{}", perf_qos_banner("sub"));

    let total = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let mut last_report = start;
    let mut last_total = 0u64;
    let poll = perf_sub_poll();
    if poll.is_none() {
        reader.set_listener(
            Some(Arc::new(WakeOnData(std::thread::current()))),
            status::DATA_AVAILABLE,
        );
    }
    // `pub` writes its running sample counter into `shapesize`, so the span
    // between the first and the last sequence this reader saw is exactly the
    // number of samples the writer produced while the reader was listening.
    // `span - total` is the loss at the application boundary, and it is the
    // only loss figure that does not depend on the two processes agreeing on
    // when their measurement windows started.
    let mut first_seq: Option<i32> = None;
    let mut last_seq: i32 = 0;

    while start.elapsed() < runtime {
        let mut taken = 0usize;
        if let Ok(samples) = reader.take() {
            taken = samples.len();
            for s in samples {
                if first_seq.is_none() {
                    first_seq = Some(s.shapesize);
                }
                last_seq = s.shapesize;
                total.fetch_add(1, Ordering::Relaxed);
            }
        }
        if last_report.elapsed() >= Duration::from_secs(1) {
            let now_total = total.load(Ordering::Relaxed);
            let delta = now_total - last_total;
            let elapsed_s = last_report.elapsed().as_secs_f64();
            let rate_ks = (delta as f64 / elapsed_s) / 1000.0;
            println!(
                "{:.3}  size N total {} delta {} rate {:.2} kS/s",
                start.elapsed().as_secs_f64(),
                now_total,
                delta,
                rate_ks
            );
            last_report = Instant::now();
            last_total = now_total;
        }
        match poll {
            // The listener's unpark between the `take` and this park is kept
            // by the thread's token, so the next park returns at once; the
            // bound only keeps the report and the deadline going while
            // nothing arrives.
            None if taken == 0 => std::thread::park_timeout(Duration::from_millis(100)),
            None => {}
            Some(pause) if !pause.is_zero() => std::thread::sleep(pause),
            Some(_) => {}
        }
    }
    let got = total.load(Ordering::Relaxed);
    let first = first_seq.unwrap_or(0);
    let span = i64::from(last_seq) - i64::from(first) + 1;
    let missing = span - got as i64;
    let loss_pct = if span > 0 {
        (missing as f64 / span as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "# sub-done: total={got} runtime={:.3}s first_seq={first} last_seq={last_seq} \
         span={span} missing={missing} loss={loss_pct:.2}% poll_us={}",
        start.elapsed().as_secs_f64(),
        poll.map_or_else(|| String::from("listener"), |p| p.as_micros().to_string()),
    );
    print_phase_dump("sub");
    Ok(())
}

fn run_pingpong(runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    // Both roles in one process via two topics; the pinger writes
    // PerfPing (with a "send-time" marker as a shapesize-i32 diff), the
    // ponger (second process) reads and echoes onto PerfPong. This
    // process is the PINGER.
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(perf_domain(), DomainParticipantQos::default())?;
    let ping_topic = participant.create_topic::<ShapeType>(PING_TOPIC, TopicQos::default())?;
    let pong_topic = participant.create_topic::<ShapeType>(PONG_TOPIC, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    // The same QoS knobs as `pub`/`sub`: a latency figure that was measured
    // under a different QoS than the throughput figure next to it cannot be
    // read against it — and LATENCY_BUDGET in particular is a statement about
    // latency, so a round-trip run that left it at the default measured the
    // wrong writer.
    let writer = publisher.create_datawriter::<ShapeType>(&ping_topic, perf_writer_qos())?;
    let reader = subscriber.create_datareader::<ShapeType>(&pong_topic, perf_reader_qos())?;
    println!("{}", perf_qos_banner("pingpong"));

    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(5));
    let _ = reader.wait_for_matched_publication(1, Duration::from_secs(5));

    let mut rtts_us: Vec<u64> = Vec::new();
    let start = Instant::now();
    let mut seq = 0i32;
    while start.elapsed() < runtime {
        let send_us = now_micros();
        // shapesize encodes the lower 32 bits of send_us as a pseudo-id
        // (collision-prone, but only for matching in the pong); the real
        // RTT computation happens locally via timestamp storage.
        let _ = writer.write(&ShapeType::new("PING", 0, 0, seq));
        seq = seq.wrapping_add(1);

        // Waits up to 50ms for the pong. Bug-fix 2026-05-19: previously
        // the break checked `!rtts_us.is_empty()` — always true from
        // iter 2 onward → the inner loop bailed out immediately on the
        // first take(), even if the PONG for THIS seq had not arrived
        // yet. Result: only iter 1 produced an RTT, then 99% were lost.
        // A per-iter `got` flag fixes this cleanly.
        let deadline = Instant::now() + Duration::from_millis(50);
        let mut got = false;
        while Instant::now() < deadline && !got {
            if let Ok(samples) = reader.take() {
                for s in samples {
                    if s.color == "PONG" && s.shapesize == seq.wrapping_sub(1) {
                        let recv_us = now_micros();
                        rtts_us.push(recv_us - send_us);
                        got = true;
                        break;
                    }
                }
            }
            if !got {
                std::thread::sleep(Duration::from_micros(100));
            }
        }
        std::thread::sleep(Duration::from_millis(100)); // 10Hz ping rate
    }

    if rtts_us.is_empty() {
        println!("# pingpong: no RTTs collected — pong missing?");
        return Ok(());
    }
    rtts_us.sort_unstable();
    let n = rtts_us.len();
    let mean = rtts_us.iter().sum::<u64>() / n as u64;
    let p50 = rtts_us[n / 2];
    let p90 = rtts_us[(n * 9) / 10];
    let p99 = rtts_us[(n * 99) / 100];
    println!(
        "{:.3}  rtt mean {}us min {} 50% {} 90% {} 99% {} max {} cnt {}",
        start.elapsed().as_secs_f64(),
        mean,
        rtts_us.first().copied().unwrap_or(0),
        p50,
        p90,
        p99,
        rtts_us.last().copied().unwrap_or(0),
        n
    );
    Ok(())
}

fn run_pong(runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    // Reads PerfPing, writes PerfPong with the same shapesize.
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(perf_domain(), DomainParticipantQos::default())?;
    let ping_topic = participant.create_topic::<ShapeType>(PING_TOPIC, TopicQos::default())?;
    let pong_topic = participant.create_topic::<ShapeType>(PONG_TOPIC, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber.create_datareader::<ShapeType>(&ping_topic, perf_reader_qos())?;
    let writer = publisher.create_datawriter::<ShapeType>(&pong_topic, perf_writer_qos())?;
    println!("{}", perf_qos_banner("pong"));
    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(5));
    let _ = reader.wait_for_matched_publication(1, Duration::from_secs(5));

    let start = Instant::now();
    let mut echoed = 0u64;
    while start.elapsed() < runtime {
        if let Ok(samples) = reader.take() {
            for s in samples {
                if s.color == "PING" {
                    let _ = writer.write(&ShapeType::new("PONG", s.x, s.y, s.shapesize));
                    echoed += 1;
                }
            }
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    println!(
        "# pong-done: echoed={echoed} runtime={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn usage() -> ! {
    eprintln!("usage:");
    eprintln!("  zerodds_perf pub <size_bytes> <runtime_secs>");
    eprintln!("  zerodds_perf sub <runtime_secs>");
    eprintln!("  zerodds_perf pingpong <runtime_secs>");
    eprintln!("  zerodds_perf pong <runtime_secs>");
    eprintln!();
    eprintln!("env:");
    eprintln!("  ZERODDS_PERF_DOMAIN=<id>   domain to measure on (default 0)");
    eprintln!("  ZERODDS_PERF_RELIABILITY=reliable|besteffort  both ends (default besteffort)");
    eprintln!("  ZERODDS_PERF_HISTORY=all|<depth>              both ends (default 1)");
    eprintln!("  ZERODDS_PERF_MAX_SAMPLES=<n>                  both ends (default 1000)");
    eprintln!(
        "  ZERODDS_PERF_SUB_POLL_US=<us>                 sub polls, sleeping <us> between takes"
    );
    eprintln!(
        "                                                (default: takes when its listener wakes it)"
    );
    eprintln!("      Give each concurrent run its own domain: two runs sharing");
    eprintln!("      one domain discover each other and report a summed rate.");
    std::process::exit(1);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
    }
    let mode = args[1].as_str();
    match mode {
        "pub" => {
            let size: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1024);
            let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
            run_pub(size, Duration::from_secs(secs))
        }
        "sub" => {
            let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            run_sub(Duration::from_secs(secs))
        }
        "pingpong" => {
            let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
            run_pingpong(Duration::from_secs(secs))
        }
        "pong" => {
            let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
            run_pong(Duration::from_secs(secs))
        }
        _ => usage(),
    }
}
