//! ZeroDDS Performance-Bench-Binary (CI-3b Welle).
//!
//! Drei Modi:
//! * `zerodds_perf pub <size> <runtime_secs>` — schreibt ShapeType-Samples
//!   so schnell wie möglich (write-best-effort), zählt total+rate.
//! * `zerodds_perf sub <runtime_secs>` — liest und zählt empfangene Samples,
//!   misst rate alle Sekunde.
//! * `zerodds_perf pingpong <runtime_secs>` — Round-Trip-Time-Test:
//!   sendet PING über Topic "PerfPing", wartet auf PONG über "PerfPong".
//!   Misst RTT in µs (timestamp encoded in shapesize i32).
//!
//! # Output-Format
//!
//! ddsperf-kompatible Form (dass `llvm_bench_runner.sh` denselben Regex-
//! Parser nutzen kann):
//! ```text
//! 1.000  size 1024 total 1234 rate 1234.5 kS/s 9876.5 Mb/s
//! 1.000  rtt mean 145us min 100 50% 130 90% 180 99% 250 max 500
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zerodds_dcps::interop::ShapeType;
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

fn run_pub(size: usize, runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeType>(PERF_TOPIC, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher.create_datawriter::<ShapeType>(&topic, DataWriterQos::default())?;

    // Discovery-Wait
    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(5));

    let payload_color = "X".repeat(size.saturating_sub(4 * 4)); // size minus i32-Felder approx
    let start = Instant::now();
    let mut total = 0u64;
    let mut last_report = start;
    let mut samples_since_report = 0u64;

    while start.elapsed() < runtime {
        let s = ShapeType::new(payload_color.clone(), 0, 0, total as i32);
        if writer.write(&s).is_ok() {
            total += 1;
            samples_since_report += 1;
        }
        // Report jede Sekunde
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
    Ok(())
}

fn run_sub(runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeType>(PERF_TOPIC, TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber.create_datareader::<ShapeType>(&topic, DataReaderQos::default())?;

    let total = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let mut last_report = start;
    let mut last_total = 0u64;

    while start.elapsed() < runtime {
        if let Ok(samples) = reader.take() {
            for _ in samples {
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
        std::thread::sleep(Duration::from_millis(1));
    }
    println!(
        "# sub-done: total={} runtime={:.3}s",
        total.load(Ordering::Relaxed),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn run_pingpong(runtime: Duration) -> Result<(), Box<dyn std::error::Error>> {
    // Beide Rollen in einem Prozess via zwei Topics; Pinger schreibt
    // PerfPing (mit "send-time"-Marker als shapesize-i32-Diff), Ponger
    // (zweiter Prozess) liest und echot auf PerfPong. Dieser Prozess
    // ist der PINGER.
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let ping_topic = participant.create_topic::<ShapeType>(PING_TOPIC, TopicQos::default())?;
    let pong_topic = participant.create_topic::<ShapeType>(PONG_TOPIC, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let writer = publisher.create_datawriter::<ShapeType>(&ping_topic, DataWriterQos::default())?;
    let reader =
        subscriber.create_datareader::<ShapeType>(&pong_topic, DataReaderQos::default())?;

    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(5));
    let _ = reader.wait_for_matched_publication(1, Duration::from_secs(5));

    let mut rtts_us: Vec<u64> = Vec::new();
    let start = Instant::now();
    let mut seq = 0i32;
    while start.elapsed() < runtime {
        let send_us = now_micros();
        // shapesize codiert die unteren 32 bit von send_us als pseudo-id
        // (kollisionsanfaellig, aber nur fuer matching im Pong); echte
        // RTT-Berechnung passiert lokal via timestamp-storage.
        let _ = writer.write(&ShapeType::new("PING", 0, 0, seq));
        seq = seq.wrapping_add(1);

        // Wartet bis 50ms auf Pong
        let deadline = Instant::now() + Duration::from_millis(50);
        while Instant::now() < deadline {
            if let Ok(samples) = reader.take() {
                for s in samples {
                    if s.color == "PONG" && s.shapesize == seq.wrapping_sub(1) {
                        let recv_us = now_micros();
                        rtts_us.push(recv_us - send_us);
                        break;
                    }
                }
                if !rtts_us.is_empty() && rtts_us.last().is_some() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_micros(100));
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
    // Liest PerfPing, schreibt PerfPong mit gleichem shapesize.
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let ping_topic = participant.create_topic::<ShapeType>(PING_TOPIC, TopicQos::default())?;
    let pong_topic = participant.create_topic::<ShapeType>(PONG_TOPIC, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader =
        subscriber.create_datareader::<ShapeType>(&ping_topic, DataReaderQos::default())?;
    let writer = publisher.create_datawriter::<ShapeType>(&pong_topic, DataWriterQos::default())?;
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
