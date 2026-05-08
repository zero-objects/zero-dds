// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-spy` — abonniert ein DDS-Topic und dumpt die Samples.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_cli_common::{install_signal_handler, raw_reader_config, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample};
use zerodds_spy::{Command, SubscribeArgs, format_hex_snippet, parse_args};

const MARKER_SPY: u8 = 0xFB;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-spy {}", env!("CARGO_PKG_VERSION"));
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
        Command::Subscribe(s) => run_subscribe(&s),
    }
}

fn run_subscribe(s: &SubscribeArgs) -> ExitCode {
    if s.topic.is_empty() {
        eprintln!("error: --topic is required");
        return ExitCode::from(2);
    }
    let domain_id: i32 = match s.domain.try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: domain {} does not fit i32", s.domain);
            return ExitCode::from(2);
        }
    };
    let prefix = stable_prefix(MARKER_SPY);
    let runtime = match DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: DcpsRuntime::start failed: {e:?}");
            return ExitCode::from(3);
        }
    };

    let (_eid, rx) = match runtime.register_user_reader(raw_reader_config(&s.topic)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: register_user_reader: {e:?}");
            return ExitCode::from(3);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-spy: domain={} topic={} hex-bytes={} max-samples={}",
        s.domain,
        s.topic,
        s.hex_bytes,
        s.max_samples.map_or("∞".to_string(), |n| n.to_string())
    );

    let started = Instant::now();
    let deadline = s.duration.map(|d| started + d);
    let mut received: u64 = 0;

    while !stop.load(Ordering::Relaxed) {
        if let Some(end) = deadline {
            if Instant::now() >= end {
                break;
            }
        }
        if let Some(max) = s.max_samples {
            if received >= max {
                break;
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(UserSample::Alive {
                payload,
                writer_guid,
                ..
            }) => {
                received += 1;
                println!(
                    "[{:>6}] writer={} bytes={} {}",
                    received,
                    short_guid(&writer_guid),
                    payload.len(),
                    format_hex_snippet(&payload, s.hex_bytes)
                );
            }
            Ok(UserSample::Lifecycle { kind, .. }) => {
                println!("[lifecycle] {kind:?}");
            }
            Err(_) => {}
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!("zerodds-spy: stopped after {elapsed:.1}s · received={received}");
    drop(runtime);
    ExitCode::SUCCESS
}

fn short_guid(guid: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        guid[12], guid[13], guid[14], guid[15]
    )
}

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-spy {v}\n\
         Subscribe to a DDS topic and dump samples (hex / metadata).\n\
\n\
         USAGE:\n  \
           zerodds-spy [subscribe] -t TOPIC [OPTIONS]\n\
\n\
         OPTIONS:\n  \
           -d, --domain <ID>     DDS Domain ID (default 0)\n  \
           -t, --topic <NAME>    Topic to subscribe (REQUIRED)\n  \
           -n, --count <N>       Stop after N samples (default: unlimited)\n  \
               --duration <DUR>  Stop after duration (5, 30s, 2m, 1h)\n  \
           -x, --hex <BYTES>     Print first BYTES as hex (default 32, 0=off)\n\
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
