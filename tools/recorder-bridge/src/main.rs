// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-recorder-bridge` — live recorder for a DCPS domain.
//!
//! Crate `zerodds-recorder-bridge`. Safety classification: **COMFORT**.
//!
//! # What does the tool do?
//!
//! For a configured list of topics (`name:type` pairs)
//! it creates one ZeroDDS reader per topic, take()s samples in
//! a loop and writes them via [`zerodds_recorder::RecordingSession`]
//! into a `.zddsrec` file.
//!
//! # Architecture
//!
//! * The topic set is configured up front (via CLI or config file).
//!   Discovery-based auto-subscribe would be an optional
//!   follow mode, as soon as `zerodds-c-api` provides a built-in-topics
//!   reader API path.
//! * One participant per bridge process (`DcpsRuntime`), one reader per
//!   topic; each sample is recorded with the representation identifier of
//!   its encapsulation header (DDSI-RTPS 2.5 §10.5), which a replay sends
//!   it under.

#![warn(missing_docs)]
#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr)]

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use zerodds_cli_common::{
    MonitorReader, WriterReliability, install_signal_handler, monitor_reader_configs,
    participant_guid, stable_prefix, unix_ns_now,
};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample};
use zerodds_recorder::{ParticipantEntry, RecordingSession, SampleKind, SessionOptions, TopicKey};
use zerodds_rtps::history_cache::ChangeKind;

/// Samples taken from one reader's queue per turn after its wait.
const DRAIN: usize = 1024;

/// CLI configuration.
struct Args {
    /// `.zddsrec` output path.
    output: PathBuf,
    /// DDS-Domain-ID.
    domain: u32,
    /// List of `topic_name:type_name` pairs.
    topics: Vec<TopicKey>,
    /// Wallclock limit (None = unlimited, stop via SIGINT).
    duration: Option<Duration>,
}

fn print_help() {
    println!(
        "zerodds-recorder-bridge - live capture to .zddsrec

USAGE:
  zerodds-recorder-bridge --out FILE [--domain N] \\
    --topic NAME:TYPE [--topic NAME:TYPE ...] \\
    [--duration N(ms|s)]

OPTIONS:
  --out FILE        Output path for .zddsrec
  --domain N        DDS-Domain-ID (default 0)
  --topic NAME:TYPE Topic + type — repeatable
  --duration X      Stop after X (ms default, or suffix s)
"
    );
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let n: u64 = if let Some(rest) = s.strip_suffix("ms") {
        rest.parse().map_err(|e| format!("ms: {e}"))?
    } else if let Some(rest) = s.strip_suffix('s') {
        let v: u64 = rest.parse().map_err(|e| format!("s: {e}"))?;
        v * 1000
    } else {
        s.parse().map_err(|e| format!("ms-default: {e}"))?
    };
    Ok(Duration::from_millis(n))
}

fn parse_args() -> Result<Args, String> {
    let mut output: Option<PathBuf> = None;
    let mut domain: u32 = 0;
    let mut topics: Vec<TopicKey> = Vec::new();
    let mut duration: Option<Duration> = None;
    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        let val = || {
            argv.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {}", argv[i]))
        };
        match argv[i].as_str() {
            "--out" => {
                output = Some(PathBuf::from(val()?));
                i += 2;
            }
            "--domain" => {
                domain = val()?.parse().map_err(|e| format!("domain: {e}"))?;
                i += 2;
            }
            "--topic" => {
                let v = val()?;
                let (n, t) = v
                    .split_once(':')
                    .ok_or_else(|| format!("topic must be NAME:TYPE, got {v}"))?;
                topics.push(TopicKey {
                    topic: n.into(),
                    type_name: t.into(),
                });
                i += 2;
            }
            "--duration" => {
                duration = Some(parse_duration(&val()?)?);
                i += 2;
            }
            "--help" | "-h" | "help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    let output = output.ok_or("--out FILE required")?;
    if topics.is_empty() {
        return Err("at least one --topic NAME:TYPE required".into());
    }
    Ok(Args {
        output,
        domain,
        topics,
        duration,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            return ExitCode::from(2);
        }
    };
    println!(
        "zerodds-recorder-bridge: domain={} topics={} → {:?}",
        args.domain,
        args.topics.len(),
        args.output
    );
    let Ok(domain) = i32::try_from(args.domain) else {
        eprintln!("error: domain {} does not fit i32", args.domain);
        return ExitCode::from(2);
    };
    // Open the output file.
    let file = match File::create(&args.output) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("create {:?}: {e}", args.output);
            return ExitCode::from(1);
        }
    };
    let prefix = stable_prefix(0xF8);
    let part_guid = participant_guid(prefix);
    let opts = SessionOptions {
        time_base_unix_ns: unix_ns_now(),
        participants: vec![ParticipantEntry {
            guid: part_guid,
            name: "recorder-bridge".into(),
        }],
        topics: args.topics.clone(),
    };
    let session = RecordingSession::new(file, opts);
    let stop_flag = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop_flag));
    let config = match RuntimeConfig::from_standard_paths() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: runtime config: {e}");
            return ExitCode::from(1);
        }
    };
    let runtime = match DcpsRuntime::start(domain, prefix, config) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: DcpsRuntime::start failed: {e:?}");
            return ExitCode::from(1);
        }
    };
    // Three readers per topic, RELIABLE and TRANSIENT_LOCAL, RELIABLE,
    // BEST_EFFORT, so that every writer and the history a writer keeps are
    // recorded (`monitor_reader_configs`). A failure leaves nothing behind:
    // the runtime owns the readers and is shut down on every way out.
    let mut readers: Vec<(Receiver<UserSample>, TopicKey, MonitorReader)> = Vec::new();
    for tk in &args.topics {
        for (cfg, role) in monitor_reader_configs(&tk.topic, &tk.type_name) {
            match runtime.register_user_reader(cfg) {
                Ok((_eid, rx)) => readers.push((rx, tk.clone(), role)),
                Err(e) => {
                    eprintln!("reader for {} failed: {e:?}", tk.topic);
                    runtime.shutdown();
                    return ExitCode::from(1);
                }
            }
        }
    }
    // What a writer wrote reaches every reader that matches it; the first
    // copy is recorded.
    let mut reliability = WriterReliability::new();
    println!("recording... (Ctrl-C to stop)");
    let start = Instant::now();
    let mut last_log = start;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            println!("recorder-bridge: signal received, draining...");
            break;
        }
        if let Some(max) = args.duration
            && start.elapsed() >= max
        {
            println!("recorder-bridge: duration reached");
            break;
        }
        // Every reader in turn: one wait for a sample, then at most
        // `DRAIN` of those already queued, so a busy topic holds up neither
        // the other readers nor the stop. With one topic (its three
        // readers) the waits are the channels'.
        let wait = Duration::from_millis(if args.topics.len() == 1 { 25 } else { 5 });
        for (rx, tk, role) in &readers {
            if let Ok(sample) = rx.recv_timeout(wait) {
                if reliability.keep(&runtime, *role, &sample) {
                    record(&session, part_guid, tk, sample);
                }
                for sample in rx.try_iter().take(DRAIN) {
                    if reliability.keep(&runtime, *role, &sample) {
                        record(&session, part_guid, tk, sample);
                    }
                }
            }
        }
        // Periodic status log every 5s.
        if last_log.elapsed() >= Duration::from_secs(5) {
            let s = session.stats();
            println!(
                "  recorded={} dropped={} bytes={}",
                s.samples_total, s.samples_dropped, s.bytes_total
            );
            last_log = Instant::now();
        }
    }
    // What arrived before the stop is recorded too.
    for (rx, tk, role) in &readers {
        while let Ok(sample) = rx.try_recv() {
            if reliability.keep(&runtime, *role, &sample) {
                record(&session, part_guid, tk, sample);
            }
        }
    }
    runtime.shutdown();
    let final_stats = session.stats();
    println!(
        "recorder-bridge done: samples={} dropped={} bytes={} elapsed={:.1}s",
        final_stats.samples_total,
        final_stats.samples_dropped,
        final_stats.bytes_total,
        start.elapsed().as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// Records one sample: a live one with the representation identifier of its
/// encapsulation header, a dispose or unregister without body.
fn record(
    session: &RecordingSession<File>,
    part_guid: [u8; 16],
    topic: &TopicKey,
    sample: UserSample,
) {
    let res = match sample {
        UserSample::Alive {
            payload,
            writer_guid,
            representation_identifier,
            ..
        } => session.record_sample(
            unix_ns_now(),
            writer_guid,
            topic,
            SampleKind::Alive,
            Some(representation_identifier),
            payload.to_vec(),
        ),
        UserSample::Lifecycle { kind, .. } => {
            let ts = unix_ns_now();
            lifecycle_kinds(kind).iter().try_for_each(|kind| {
                session.record_sample(ts, part_guid, topic, *kind, None, Vec::new())
            })
        }
        // DDSI-RTPS 2.5 §8.7.6: the end of a coherent set carries no data.
        UserSample::CoherentSetEnd { .. } => Ok(()),
    };
    if let Err(e) = res {
        eprintln!("session error: {e}");
    }
}

/// The entries a lifecycle change becomes. DDSI-RTPS 2.5 §9.6.4.9: a status
/// info with the D and U bits is a dispose and an unregister of the
/// instance; `.zddsrec` has a kind for each, so both are recorded, the
/// dispose first.
fn lifecycle_kinds(kind: ChangeKind) -> &'static [SampleKind] {
    match kind {
        ChangeKind::NotAliveDisposed => &[SampleKind::NotAliveDisposed],
        ChangeKind::NotAliveUnregistered => &[SampleKind::NotAliveUnregistered],
        ChangeKind::NotAliveDisposedUnregistered => &[
            SampleKind::NotAliveDisposed,
            SampleKind::NotAliveUnregistered,
        ],
        ChangeKind::Alive | ChangeKind::AliveFiltered => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtps_9_6_4_9_a_dispose_with_unregister_is_recorded_as_both() {
        assert_eq!(
            lifecycle_kinds(ChangeKind::NotAliveDisposedUnregistered),
            [
                SampleKind::NotAliveDisposed,
                SampleKind::NotAliveUnregistered
            ]
        );
        assert_eq!(
            lifecycle_kinds(ChangeKind::NotAliveDisposed),
            [SampleKind::NotAliveDisposed]
        );
        assert_eq!(
            lifecycle_kinds(ChangeKind::NotAliveUnregistered),
            [SampleKind::NotAliveUnregistered]
        );
    }
}
