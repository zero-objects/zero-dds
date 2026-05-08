// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-monitor` — Live-Snapshot oder Prometheus-Server für die
//! ZeroDDS-Runtime.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_cli_common::{install_signal_handler, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_monitor_cli::{Command, ServeArgs, SnapshotArgs, SnapshotFormat, parse_args};
use zerodds_monitor_lib::{default_registry, render_prometheus, serve_prometheus};

const MARKER_MONITOR: u8 = 0xFC;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-monitor {}", env!("CARGO_PKG_VERSION"));
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
        Command::Snapshot(s) => run_snapshot(&s),
        Command::Serve(s) => run_serve(&s),
        Command::Names => {
            print_names();
            ExitCode::SUCCESS
        }
    }
}

fn run_snapshot(s: &SnapshotArgs) -> ExitCode {
    let _runtime = match start_runtime(s.domain) {
        Ok(rt) => rt,
        Err(e) => return e,
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-monitor: collecting metrics on domain {} for {}s",
        s.domain,
        s.duration.as_secs()
    );

    let deadline = Instant::now() + s.duration;
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }

    let registry = default_registry();
    let snap = registry.snapshot();
    println!();
    match s.format {
        SnapshotFormat::Text => render_text(&snap),
        SnapshotFormat::Prometheus => println!("{}", render_prometheus(&snap)),
    }
    ExitCode::SUCCESS
}

fn render_text(snap: &zerodds_monitor_lib::RegistrySnapshot) {
    println!("=== counters ({}) ===", snap.counters.len());
    for (key, value) in &snap.counters {
        println!("  {value:>12}  {}{}", key.name, format_labels(&key.labels));
    }
    println!();
    println!("=== gauges ({}) ===", snap.gauges.len());
    for (key, value) in &snap.gauges {
        println!("  {value:>12}  {}{}", key.name, format_labels(&key.labels));
    }
    println!();
    println!("=== histograms ({}) ===", snap.histograms.len());
    for (key, hist) in &snap.histograms {
        println!(
            "  {:<30} count={} sum_ns={}",
            format!("{}{}", key.name, format_labels(&key.labels)),
            hist.count,
            hist.sum_ns
        );
    }
}

fn format_labels(labels: &zerodds_monitor_lib::Labels) -> String {
    let parts: Vec<String> = labels.iter().map(|(k, v)| format!("{k}={v}")).collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{{{}}}", parts.join(","))
    }
}

fn run_serve(s: &ServeArgs) -> ExitCode {
    let _runtime = match start_runtime(s.domain) {
        Ok(rt) => rt,
        Err(e) => return e,
    };

    let addr = match s.addr.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: invalid --addr {}: {e}", s.addr);
            return ExitCode::from(2);
        }
    };
    let registry = default_registry();
    let _server = match serve_prometheus(addr, registry) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: serve_prometheus failed: {e}");
            return ExitCode::from(3);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-monitor: serving /metrics on http://{} (domain={})",
        s.addr, s.domain
    );

    let deadline = s.duration.map(|d| Instant::now() + d);
    while !stop.load(Ordering::Relaxed) {
        if let Some(end) = deadline {
            if Instant::now() >= end {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("zerodds-monitor: shutting down");
    ExitCode::SUCCESS
}

fn print_names() {
    use zerodds_monitor_lib::metric_names::*;
    let names = &[
        DDS_TRANSPORT_PACKETS_SENT_TOTAL,
        DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL,
        DDS_TRANSPORT_BYTES_SENT_TOTAL,
        DDS_TRANSPORT_BYTES_RECEIVED_TOTAL,
        DDS_TRANSPORT_SEND_ERRORS_TOTAL,
        DDS_TRANSPORT_SOCKET_BUFFER_BYTES,
        DDS_RTPS_HEARTBEATS_SENT_TOTAL,
        DDS_RTPS_ACKNACKS_RECEIVED_TOTAL,
        DDS_RTPS_RETRANSMITS_TOTAL,
        DDS_RTPS_SAMPLES_DROPPED_TOTAL,
        DDS_RTPS_FRAGMENTED_SAMPLES_TOTAL,
        DDS_RTPS_UNKNOWN_SUBMESSAGES_TOTAL,
        DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
        DDS_DCPS_SAMPLES_READ_TOTAL,
        DDS_DCPS_SAMPLES_LOST_TOTAL,
    ];
    println!("Known ZeroDDS metric names:");
    for n in names {
        println!("  {n}");
    }
}

fn start_runtime(domain: u32) -> Result<Arc<DcpsRuntime>, ExitCode> {
    let domain_id: i32 = domain.try_into().map_err(|_| {
        eprintln!("error: domain {domain} does not fit i32");
        ExitCode::from(2)
    })?;
    let prefix = stable_prefix(MARKER_MONITOR);
    DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default()).map_err(|e| {
        eprintln!("error: DcpsRuntime::start failed: {e:?}");
        ExitCode::from(3)
    })
}

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-monitor {v}\n\
         Live-snapshot or Prometheus /metrics server for the ZeroDDS\n\
         runtime registry.\n\
\n\
         USAGE:\n  \
           zerodds-monitor <SUBCOMMAND> [OPTIONS]\n\
\n\
         SUBCOMMANDS:\n  \
           snapshot   Print registry snapshot to stdout (default text format)\n  \
           serve      Run /metrics HTTP server (Prometheus exposition)\n  \
           names      List known ZeroDDS metric names\n\
\n\
         OPTIONS for `snapshot`:\n  \
           -d, --domain <ID>      DDS Domain ID (default 0)\n  \
               --duration <DUR>   Collection window (default 5s)\n  \
           -f, --format <FORMAT>  text | prometheus  (default text)\n\
\n\
         OPTIONS for `serve`:\n  \
           -d, --domain <ID>      DDS Domain ID (default 0)\n  \
           -a, --addr <ADDR>      Listen address (default 127.0.0.1:9991)\n  \
               --duration <DUR>   Auto-stop after duration (default: until SIGINT)\n\
\n\
         GLOBAL OPTIONS:\n  \
           -h, --help             Show this message\n  \
           -V, --version          Print version\n\
\n\
         EXIT CODES:\n  \
           0    success\n  \
           2    CLI parse error\n  \
           3    DDS / I/O error\n"
    );
}
