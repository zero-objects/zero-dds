// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-snitch` — kurzer Discovery-Probe-Run, druckt entdeckte
//! Participants + Endpoint-Zähler.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_cli_common::{install_signal_handler, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_snitch::{Command, ProbeArgs, ProbeFormat, parse_args};

const MARKER_SNITCH: u8 = 0xFA;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-snitch {}", env!("CARGO_PKG_VERSION"));
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
        Command::Probe(p) => run_probe(&p),
    }
}

fn run_probe(p: &ProbeArgs) -> ExitCode {
    let domain_id: i32 = match p.domain.try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: domain {} does not fit i32", p.domain);
            return ExitCode::from(2);
        }
    };
    let prefix = stable_prefix(MARKER_SNITCH);
    let runtime = match DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: DcpsRuntime::start failed: {e:?}");
            return ExitCode::from(3);
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    eprintln!(
        "zerodds-snitch: probing domain {} for {}s",
        p.domain,
        p.duration.as_secs()
    );

    let deadline = Instant::now() + p.duration;
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }

    let participants = runtime.discovered_participants();
    let pubs = runtime.discovered_publications_count();
    let subs = runtime.discovered_subscriptions_count();

    match p.format {
        ProbeFormat::Text => render_text(&participants, pubs, subs),
        ProbeFormat::Json => render_json(&participants, pubs, subs),
    }

    drop(runtime);
    ExitCode::SUCCESS
}

fn render_text(
    participants: &[zerodds_discovery::spdp::DiscoveredParticipant],
    pubs: usize,
    subs: usize,
) {
    println!();
    println!("=== Discovery snapshot ===");
    println!("participants:       {}", participants.len());
    println!("publications seen:  {pubs}");
    println!("subscriptions seen: {subs}");
    if !participants.is_empty() {
        println!();
        println!("participants:");
        for p in participants {
            println!(
                "  · prefix={} vendor={:02x}{:02x} key={}",
                hex12(&p.sender_prefix.0),
                p.sender_vendor.0[0],
                p.sender_vendor.0[1],
                hex_guid(&p.data.guid.to_bytes())
            );
            if !p.data.user_data.is_empty() {
                println!("    user_data: {} bytes", p.data.user_data.len());
            }
        }
    }
}

fn render_json(
    participants: &[zerodds_discovery::spdp::DiscoveredParticipant],
    pubs: usize,
    subs: usize,
) {
    print!(
        "{{\"participants\":{},\"publications\":{pubs},\"subscriptions\":{subs},\"entries\":[",
        participants.len()
    );
    for (i, p) in participants.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!(
            "{{\"prefix\":\"{}\",\"vendor\":\"{:02x}{:02x}\",\"key\":\"{}\",\"user_data_bytes\":{}}}",
            hex12(&p.sender_prefix.0),
            p.sender_vendor.0[0],
            p.sender_vendor.0[1],
            hex_guid(&p.data.guid.to_bytes()),
            p.data.user_data.len()
        );
    }
    println!("]}}");
}

fn hex12(b: &[u8; 12]) -> String {
    let mut s = String::with_capacity(24);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

fn hex_guid(b: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-snitch {v}\n\
         DDS discovery probe — listens for SPDP/SEDP and prints all\n\
         participants and endpoint counts seen during the run.\n\
\n\
         USAGE:\n  \
           zerodds-snitch [probe] [OPTIONS]\n\
\n\
         OPTIONS:\n  \
           -d, --domain <ID>      DDS Domain ID (default 0)\n  \
               --duration <DUR>   Probe duration (default 5s)\n  \
           -f, --format <FORMAT>  text | json  (default text)\n\
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
