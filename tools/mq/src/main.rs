// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mq` — leitet Samples zwischen zwei DDS-Domains durch.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use zerodds_cli_common::{install_signal_handler, raw_reader_config, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample, UserWriterConfig};
use zerodds_mq::{BridgeArgs, Command, parse_args};

const TYPE_NAME: &str = "zerodds::RawBytes";
const MARKER_MQ_SRC: u8 = 0xF9;
const MARKER_MQ_DST: u8 = 0xF8;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-mq {}", env!("CARGO_PKG_VERSION"));
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
        Command::Bridge(b) => run_bridge(&b),
    }
}

fn run_bridge(b: &BridgeArgs) -> ExitCode {
    if b.src_topic.is_empty() {
        eprintln!("error: --topic is required");
        return ExitCode::from(2);
    }
    if b.src_domain == b.dst_domain {
        eprintln!("error: --src-domain and --dst-domain must differ");
        return ExitCode::from(2);
    }
    let src_id: i32 = match b.src_domain.try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: src-domain {} does not fit i32", b.src_domain);
            return ExitCode::from(2);
        }
    };
    let dst_id: i32 = match b.dst_domain.try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: dst-domain {} does not fit i32", b.dst_domain);
            return ExitCode::from(2);
        }
    };

    let src_rt = match DcpsRuntime::start(
        src_id,
        stable_prefix(MARKER_MQ_SRC),
        RuntimeConfig::default(),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: src DcpsRuntime start: {e:?}");
            return ExitCode::from(3);
        }
    };
    let dst_rt = match DcpsRuntime::start(
        dst_id,
        stable_prefix(MARKER_MQ_DST),
        RuntimeConfig::default(),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: dst DcpsRuntime start: {e:?}");
            return ExitCode::from(3);
        }
    };

    // Forward direction: read from src.src_topic → write to dst.dst_topic
    let (_src_reader_eid, src_rx) =
        match src_rt.register_user_reader(raw_reader_config(&b.src_topic)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: src reader: {e:?}");
                return ExitCode::from(3);
            }
        };
    let dst_writer_eid = match dst_rt.register_user_writer(writer_cfg(&b.dst_topic)) {
        Ok(eid) => eid,
        Err(e) => {
            eprintln!("error: dst writer: {e:?}");
            return ExitCode::from(3);
        }
    };

    // Optional reverse direction.
    let reverse = if b.bidirectional {
        let (_eid, rx) = match dst_rt.register_user_reader(raw_reader_config(&b.dst_topic)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: dst reader (reverse): {e:?}");
                return ExitCode::from(3);
            }
        };
        let writer = match src_rt.register_user_writer(writer_cfg(&b.src_topic)) {
            Ok(eid) => eid,
            Err(e) => {
                eprintln!("error: src writer (reverse): {e:?}");
                return ExitCode::from(3);
            }
        };
        Some((rx, writer))
    } else {
        None
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    let forwarded = Arc::new(AtomicU64::new(0));
    let reversed = Arc::new(AtomicU64::new(0));

    println!(
        "zerodds-mq: bridging {}@{} → {}@{}{}",
        b.src_topic,
        b.src_domain,
        b.dst_topic,
        b.dst_domain,
        if b.bidirectional {
            " (bidirectional)"
        } else {
            ""
        }
    );

    // SEDP-Settle.
    std::thread::sleep(Duration::from_millis(200));

    let started = Instant::now();
    let deadline = b.duration.map(|d| started + d);

    // Forward thread.
    let stop_fwd = Arc::clone(&stop);
    let fwd_count = Arc::clone(&forwarded);
    let dst_rt_fwd = Arc::clone(&dst_rt);
    let fwd_thread = std::thread::spawn(move || {
        while !stop_fwd.load(Ordering::Relaxed) {
            match src_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(UserSample::Alive { payload, .. }) => {
                    if dst_rt_fwd
                        .write_user_sample_borrowed(dst_writer_eid, payload.as_slice())
                        .is_ok()
                    {
                        fwd_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
    });

    // Reverse thread (optional).
    let rev_thread = reverse.map(|(rev_rx, src_writer_eid)| {
        let stop_rev = Arc::clone(&stop);
        let rev_count = Arc::clone(&reversed);
        let src_rt_rev = Arc::clone(&src_rt);
        std::thread::spawn(move || {
            while !stop_rev.load(Ordering::Relaxed) {
                match rev_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(UserSample::Alive { payload, .. }) => {
                        if src_rt_rev
                            .write_user_sample_borrowed(src_writer_eid, payload.as_slice())
                            .is_ok()
                        {
                            rev_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        })
    });

    while !stop.load(Ordering::Relaxed) {
        if let Some(end) = deadline {
            if Instant::now() >= end {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    stop.store(true, Ordering::Relaxed);
    let _ = fwd_thread.join();
    if let Some(t) = rev_thread {
        let _ = t.join();
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "zerodds-mq: stopped after {elapsed:.1}s · forwarded={} reversed={}",
        forwarded.load(Ordering::Relaxed),
        reversed.load(Ordering::Relaxed)
    );
    drop(src_rt);
    drop(dst_rt);
    ExitCode::SUCCESS
}

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: TYPE_NAME.to_string(),
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
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

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-mq {v}\n\
         Cross-domain DDS bridge — passes raw samples between two domains.\n\
\n\
         USAGE:\n  \
           zerodds-mq [bridge] -t TOPIC --src-domain N --dst-domain M [OPTIONS]\n\
\n\
         OPTIONS:\n  \
               --src-domain <ID>  Source domain ID (default 0)\n  \
               --dst-domain <ID>  Destination domain ID (default 1)\n  \
           -t, --topic <NAME>     Same topic on both sides (REQUIRED)\n  \
               --src-topic <N>    Source topic name (overrides -t)\n  \
               --dst-topic <N>    Destination topic name (overrides -t)\n  \
               --duration <DUR>   Stop after duration (5, 30s, 2m, 1h)\n  \
               --bidirectional    Mirror traffic in both directions\n\
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
