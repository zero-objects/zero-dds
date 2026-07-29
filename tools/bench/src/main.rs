// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-bench` — End-to-End Latency / Throughput-CLI.
//!
//! ## Sub-Commands
//!
//! ```text
//! zerodds-bench latency    [-d DOMAIN] [-t TOPIC] [-p PAYLOAD] [--duration DUR]
//! zerodds-bench throughput [-d DOMAIN] [-t TOPIC] [-p PAYLOAD] [--duration DUR]
//! zerodds-bench info
//! ```
//!
//! Beide Bench-Modes nutzen das gleiche topic loopback in einem
//! einzigen DcpsRuntime-Process — ein Writer und ein Reader auf dem
//! gleichen Topic. Das verifiziert die volle DCPS-Pipeline (HistoryCache,
//! XCDR2-Encap, RTPS-Wire, UDP-Transport) ohne Netz-Setup.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use zerodds_bench::{BenchArgs, Command, compute_stats, parse_args};
use zerodds_cli_common::{install_signal_handler, raw_reader_config, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample, UserWriterConfig};
use zerodds_rtps::wire_types::GuidPrefix;

const TYPE_NAME: &str = "zerodds::RawBytes";
const MARKER_BENCH: u8 = 0xFD;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-bench {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let cmd = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            return ExitCode::from(2);
        }
    };

    match cmd {
        Command::Latency(b) => run_latency(&b),
        Command::Throughput(b) => run_throughput(&b),
        Command::Info => {
            print_info();
            ExitCode::SUCCESS
        }
    }
}

fn print_info() {
    println!(
        "zerodds-bench {} · backend: ZeroDDS DCPS {}",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_VERSION")
    );
    println!("transports: udpv4 (default), shm (planned)");
    println!("encodings:  XCDR2-LE @final");
    println!("modes:      latency, throughput");
}

fn run_latency(b: &BenchArgs) -> ExitCode {
    let runtime = match start_runtime(b.domain, MARKER_BENCH) {
        Ok(rt) => rt,
        Err(e) => return e,
    };

    let writer_eid = match runtime.register_user_writer(writer_cfg(&b.topic)) {
        Ok(eid) => eid,
        Err(e) => {
            eprintln!("error: register_user_writer: {e:?}");
            return ExitCode::from(3);
        }
    };
    let (_reader_eid, rx) = match runtime.register_user_reader(raw_reader_config(&b.topic)) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: register_user_reader: {e:?}");
            return ExitCode::from(3);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-bench latency · domain={} topic={} payload={}B duration={}s",
        b.domain,
        b.topic,
        b.payload,
        b.duration.as_secs()
    );

    // Settle-Window für SEDP-Match. Ohne kurzen Delay verlieren die
    // ersten samples bevor der writer/reader-match etabliert ist.
    std::thread::sleep(Duration::from_millis(200));

    let payload_bytes = b.payload.max(8);
    let mut payload = vec![0u8; payload_bytes];
    let deadline = Instant::now() + b.duration;
    let mut rtts: Vec<u64> = Vec::with_capacity(8192);

    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        let send_ns = elapsed_ns_since_start();
        payload[0..8].copy_from_slice(&send_ns.to_le_bytes());
        if let Err(e) = runtime.write_user_sample(writer_eid, payload.clone()) {
            eprintln!("warn: write failed: {e:?}");
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        // Empfange echo (in single-process loopback ist das das eigene
        // sample, da pub und sub im gleichen Participant matched).
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(UserSample::Alive { payload: rcv, .. }) if rcv.len() >= 8 => {
                let recv_ns = elapsed_ns_since_start();
                let bytes: [u8; 8] = rcv[0..8].try_into().unwrap_or([0u8; 8]);
                let sent = u64::from_le_bytes(bytes);
                let rtt = recv_ns.saturating_sub(sent);
                rtts.push(rtt);
            }
            Ok(_) => {}
            Err(_) => {}
        }
        // Pacing 1 ms — wir wollen Latency, nicht Throughput-Stress.
        std::thread::sleep(Duration::from_micros(900));
    }

    if let Some(stats) = compute_stats(&mut rtts) {
        println!(
            "samples={} min={:.1}us p50={:.1}us p99={:.1}us max={:.1}us",
            stats.samples,
            stats.min_ns as f64 / 1000.0,
            stats.p50_ns as f64 / 1000.0,
            stats.p99_ns as f64 / 1000.0,
            stats.max_ns as f64 / 1000.0,
        );
    } else {
        eprintln!("warn: no samples received — discovery match may have failed");
    }

    drop(runtime);
    ExitCode::SUCCESS
}

fn run_throughput(b: &BenchArgs) -> ExitCode {
    let runtime = match start_runtime(b.domain, MARKER_BENCH) {
        Ok(rt) => rt,
        Err(e) => return e,
    };

    let writer_eid = match runtime.register_user_writer(writer_cfg(&b.topic)) {
        Ok(eid) => eid,
        Err(e) => {
            eprintln!("error: register_user_writer: {e:?}");
            return ExitCode::from(3);
        }
    };
    let (_reader_eid, rx) = match runtime.register_user_reader(raw_reader_config(&b.topic)) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: register_user_reader: {e:?}");
            return ExitCode::from(3);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-bench throughput · domain={} topic={} payload={}B duration={}s",
        b.domain,
        b.topic,
        b.payload,
        b.duration.as_secs()
    );

    std::thread::sleep(Duration::from_millis(200));

    let received_count = Arc::new(AtomicU64::new(0));
    let received_bytes = Arc::new(AtomicU64::new(0));
    let stop_recv = Arc::clone(&stop);
    let recv_count = Arc::clone(&received_count);
    let recv_bytes = Arc::clone(&received_bytes);

    let recv_thread = std::thread::spawn(move || {
        while !stop_recv.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(UserSample::Alive { payload, .. }) => {
                    recv_count.fetch_add(1, Ordering::Relaxed);
                    recv_bytes.fetch_add(payload.len() as u64, Ordering::Relaxed);
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
    });

    let payload = vec![0xAAu8; b.payload];
    let mut sent: u64 = 0;
    let deadline = Instant::now() + b.duration;
    let started = Instant::now();
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        if runtime
            .write_user_sample(writer_eid, payload.clone())
            .is_ok()
        {
            sent += 1;
        }
        // Tight loop — kein sleep für maximalen Throughput.
    }
    let elapsed = started.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let _ = recv_thread.join();

    let rcv = received_count.load(Ordering::Relaxed);
    let bytes = received_bytes.load(Ordering::Relaxed);
    let mbps = if elapsed > 0.0 {
        (bytes as f64) / 1_048_576.0 / elapsed
    } else {
        0.0
    };
    println!(
        "sent={sent} received={rcv} elapsed={elapsed:.2}s rate={:.0}msgs/s {mbps:.1}MiB/s",
        rcv as f64 / elapsed.max(1e-9)
    );

    drop(runtime);
    ExitCode::SUCCESS
}

fn start_runtime(domain: u32, marker: u8) -> Result<Arc<DcpsRuntime>, ExitCode> {
    let domain_id: i32 = domain.try_into().map_err(|_| {
        eprintln!("error: domain {domain} does not fit i32");
        ExitCode::from(2)
    })?;
    let prefix: GuidPrefix = stable_prefix(marker);
    DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default()).map_err(|e| {
        eprintln!("error: DcpsRuntime::start failed: {e:?}");
        ExitCode::from(3)
    })
}

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: TYPE_NAME.to_string(),
        reliable: true,
        durability: zerodds_qos_kind(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

fn zerodds_qos_kind() -> zerodds_qos::DurabilityKind {
    zerodds_qos::DurabilityKind::Volatile
}

/// Monotonic-clock elapsed time seit static start in ns.
fn elapsed_ns_since_start() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    let s = START.get_or_init(Instant::now);
    s.elapsed().as_nanos() as u64
}

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-bench {v}\n\
         End-to-End latency and throughput benchmarking on the ZeroDDS\n\
         DCPS pipeline.\n\
\n\
         USAGE:\n  \
           zerodds-bench <SUBCOMMAND> [OPTIONS]\n\
\n\
         SUBCOMMANDS:\n  \
           latency       Measure round-trip latency (single-process loopback)\n  \
           throughput    Measure publish throughput (msgs/s, MiB/s)\n  \
           info          Print backend capabilities\n\
\n\
         OPTIONS for `latency` and `throughput`:\n  \
           -d, --domain <ID>     DDS-Domain-ID (default 0)\n  \
           -t, --topic <NAME>    Topic name (default zerodds/bench/loopback)\n  \
           -p, --payload <N>     Payload size in bytes (default 64)\n  \
               --duration <DUR>  Run duration: 5, 30s, 2m, 1h (default 5s)\n\
\n\
         GLOBAL OPTIONS:\n  \
           -h, --help            Show this message\n  \
           -V, --version         Print version\n\
\n\
         EXIT CODES:\n  \
           0    success\n  \
           2    CLI parse error\n  \
           3    DDS / I/O error\n"
    );
}
