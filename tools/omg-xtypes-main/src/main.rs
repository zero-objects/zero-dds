// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `xtypes_main` — ZeroDDS reference client for the OMG `dds-xtypes`
//! interoperability test suite (`github.com/omg-dds/dds-xtypes`).
//!
//! BUILD + LOCAL-VALIDATE phase artifact. This binary is **not**
//! submitted anywhere — no CLA PR, no source PR, no release asset, no
//! push to `github.com/omg-dds`. That step is Sandra's.
//!
//! It is the Rust counterpart of the suite's own
//! `srcCxx/ShapeType_main.cxx` and `srcCxx/ShapeType_variants.h`: the
//! same command line, the same output lines, the same 25 types — with
//! the types generated from the suite's IDL rather than written beside
//! it. See `tools/omg-xtypes-main/README.md` for the flag and status
//! coverage table and for what the suite measures that our
//! implementation does not yet carry.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use zerodds_omg_xtypes_main::{cli, shape_type};

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};

use cli::{ExitStatus, Mode, Options, Parsed};

/// `send_period` / `receive_period` of `ShapeType_main.cxx`: one second,
/// on both sides.
const PERIOD: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let program = args
        .first()
        .map_or("xtypes_main", |s| s.as_str())
        .to_owned();

    let (parsed, defaults) = cli::parse(&args, shape_type::is_variant);
    let options = match parsed {
        Parsed::Help => {
            eprint!("{}", cli::usage(&program));
            return ExitCode::from(ExitStatus::Ok.code());
        }
        Parsed::HelpTypes => {
            let mut out = String::new();
            shape_type::print_variants(&mut out);
            eprint!("{out}");
            return ExitCode::from(ExitStatus::Ok.code());
        }
        Parsed::Rejected {
            message,
            with_usage,
        } => {
            eprintln!("{message}");
            if with_usage {
                eprint!("{}", cli::usage(&program));
            } else if message.contains("is not a valid type") {
                // `ShapeType_main.cxx` follows this one message with the
                // list, so the reader sees what they could have typed.
                let mut out = String::new();
                shape_type::print_variants(&mut out);
                eprint!("{out}");
            }
            return ExitCode::from(ExitStatus::Error.code());
        }
        Parsed::Run(o) => *o,
    };
    for note in &defaults.notes {
        eprintln!("{note}");
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        if let Err(e) = ctrlc::set_handler(move || {
            // `signal_handler` of `ShapeType_main.cxx`.
            eprintln!("Stopping...");
            stop.store(true, Ordering::SeqCst);
        }) {
            eprintln!("xtypes_main: warning: signal handler not installed: {e}");
        }
    }

    ExitCode::from(run(&options, &stop).code())
}

/// `run()` of `ShapeType_main.cxx`.
fn run(options: &Options, stop: &Arc<AtomicBool>) -> ExitStatus {
    let factory = DomainParticipantFactory::instance();
    let participant =
        match factory.create_participant(options.domain_id, DomainParticipantQos::default()) {
            Ok(p) => p,
            Err(e) => {
                println!("create_participant error");
                eprintln!("xtypes_main: {e}");
                return ExitStatus::Error;
            }
        };

    eprintln!(
        "Info: Starting {} application. Domain: {}, Type: {}\n",
        if options.mode == Mode::Publish {
            "publishing"
        } else {
            "subscribing"
        },
        options.domain_id,
        options.type_name
    );
    if let Some(seconds) = options.seconds {
        eprintln!("Info: will stop after {seconds} seconds");
    }
    if let Some(samples) = options.samples {
        eprintln!("Info: will stop after {samples} samples");
    }

    match options.mode {
        Mode::Publish => publish(&participant, options, stop),
        Mode::Subscribe => subscribe(&participant, options, stop),
    }
}

/// `publish()` of `ShapeType_main.cxx`. One sample per second, colour
/// `"BLUE"`, `count` increasing without bound.
fn publish(
    participant: &zerodds_dcps::DomainParticipant,
    options: &Options,
    stop: &Arc<AtomicBool>,
) -> ExitStatus {
    let Some(built) =
        shape_type::create_writer(&options.type_name, participant, &options.topic_name)
    else {
        eprintln!(
            "create_writer: Unrecognized type: \"{}\"",
            options.type_name
        );
        return ExitStatus::Error;
    };
    let writer = match built {
        Ok(w) => w,
        Err(e) => {
            println!(
                "create_datawriter error for topic \"{}\"",
                options.topic_name
            );
            eprintln!("xtypes_main: {e}");
            return ExitStatus::Error;
        }
    };

    let start = Instant::now();
    let mut count: i32 = 0;
    let mut status = ExitStatus::Ok;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(PERIOD);
        writer.drive_listeners();
        writer.poll_inconsistent_topic();
        if let Err(e) = writer.write_data("BLUE", count) {
            eprintln!("xtypes_main: write failed: {e}");
            return ExitStatus::Error;
        }
        if let Some(samples) = options.samples {
            if i64::from(count) + 1 >= samples {
                eprintln!("reached {samples} samples, exiting");
                break;
            }
        }
        if let Some(seconds) = options.seconds {
            #[allow(clippy::cast_possible_wrap)]
            if start.elapsed().as_secs() as i64 > seconds {
                eprintln!("reached {seconds} seconds, exiting");
                status = ExitStatus::Timeout;
                break;
            }
        }
        count = count.wrapping_add(1);
    }
    if stop.load(Ordering::SeqCst) {
        return ExitStatus::Interrupt;
    }
    status
}

/// `subscribe()` of `ShapeType_main.cxx`.
///
/// The suite's reader exits `OK` on a timeout, unlike its writer — the
/// `exit_status` there is never set to `TIMEOUT` on the subscribe path.
/// That asymmetry is the suite's and is kept: a subscriber that saw no
/// sample because the types are not assignable is the *expected* result
/// for most of the 625 pairs, and reporting it as a failure would invert
/// the measurement.
fn subscribe(
    participant: &zerodds_dcps::DomainParticipant,
    options: &Options,
    stop: &Arc<AtomicBool>,
) -> ExitStatus {
    let Some(built) =
        shape_type::create_reader(&options.type_name, participant, &options.topic_name)
    else {
        eprintln!(
            "create_reader: Unrecognized type: \"{}\"",
            options.type_name
        );
        return ExitStatus::Error;
    };
    let reader = match built {
        Ok(r) => r,
        Err(e) => {
            println!(
                "create datareader error for topic \"{}\"",
                options.topic_name
            );
            eprintln!("xtypes_main: {e}");
            return ExitStatus::Error;
        }
    };

    println!(
        "Waiting for data on topic \"{}\", type \"{}\"",
        reader.topic_name(),
        reader.type_name()
    );

    let start = Instant::now();
    let mut total: i64 = 0;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(PERIOD);
        reader.drive_listeners();
        reader.poll_inconsistent_topic();
        let count = reader.take_data();
        if count > 0 {
            total += i64::try_from(count).unwrap_or(i64::MAX);
            if let Some(samples) = options.samples {
                if total >= samples {
                    eprintln!("reached {samples} samples, exiting");
                    break;
                }
            }
        }
        if let Some(seconds) = options.seconds {
            #[allow(clippy::cast_possible_wrap)]
            if start.elapsed().as_secs() as i64 > seconds {
                eprintln!("reached {seconds} seconds, exiting");
                break;
            }
        }
    }
    ExitStatus::Ok
}
