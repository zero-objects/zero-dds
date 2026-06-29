// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-spy` — abonniert ein DDS-Topic und dumpt die Samples.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use std::sync::mpsc::Receiver;

use zerodds_cli_common::{
    TypeFollower, install_signal_handler, raw_reader_config_typed, stable_prefix,
};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserSample};
use zerodds_recorder_decode::json_sink::data_to_json;
use zerodds_recorder_decode::type_source::TypeBook;
use zerodds_spy::{Command, SubscribeArgs, format_hex_snippet, parse_args};
use zerodds_types::dynamic::codec::decode_dynamic;
use zerodds_types::dynamic::type_::DynamicType;

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

    let mut decoder = match SpyDecoder::from_args(s) {
        Ok(d) => d,
        Err(code) => return code,
    };

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-spy: domain={} topic={} type={} hex-bytes={} max-samples={}",
        s.domain,
        s.topic,
        s.type_name.as_deref().unwrap_or("<discover>"),
        s.hex_bytes,
        s.max_samples.map_or("∞".to_string(), |n| n.to_string())
    );

    // The set of attached readers (one per discovered writer type on the topic).
    // Each entry keeps the announced type_name for the "attached" log line.
    let mut readers: Vec<(String, Receiver<UserSample>)> = Vec::new();

    // `--type` = direct attach (skip discovery, useful for silent/late writers).
    // Otherwise type-following: discover each writer's real type_name and attach.
    let mut follower = TypeFollower::new([s.topic.clone()]);
    if let Some(ty) = &s.type_name {
        match attach_reader(&runtime, &s.topic, ty) {
            Some(rx) => readers.push((ty.clone(), rx)),
            None => return ExitCode::from(3),
        }
    }

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

        // Type-following: attach a raw reader to every newly-discovered writer
        // type on the topic (incl. late joiners). No-op when `--type` was given.
        if s.type_name.is_none() {
            for (topic, ty) in follower.poll(&runtime) {
                if let Some(rx) = attach_reader(&runtime, &topic, &ty) {
                    readers.push((ty, rx));
                }
            }
        }

        // Drain every attached reader (non-blocking sweep), then sleep briefly.
        let mut got_any = false;
        for (ty_name, rx) in &readers {
            while let Ok(sample) = rx.try_recv() {
                got_any = true;
                match sample {
                    UserSample::Alive {
                        payload,
                        writer_guid,
                        representation,
                        big_endian,
                        ..
                    } => {
                        received += 1;
                        if let Some(json) =
                            decoder.decode_json(ty_name, &payload, representation, big_endian)
                        {
                            println!(
                                "[{:>6}] writer={} type={ty_name} {json}",
                                received,
                                short_guid(&writer_guid)
                            );
                        } else {
                            println!(
                                "[{:>6}] writer={} bytes={} {}",
                                received,
                                short_guid(&writer_guid),
                                payload.len(),
                                format_hex_snippet(&payload, s.hex_bytes)
                            );
                        }
                    }
                    UserSample::Lifecycle { kind, .. } => {
                        println!("[lifecycle] {kind:?}");
                    }
                }
                if s.max_samples.is_some_and(|max| received >= max) {
                    break;
                }
            }
        }
        if !got_any {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "zerodds-spy: stopped after {elapsed:.1}s · attached-types={} received={received}",
        readers.len()
    );
    drop(runtime);
    ExitCode::SUCCESS
}

/// Optional typed-decode of spied samples to JSON (the `--decode` path).
///
/// Disabled by default → `decode_json` returns `None` and the spy prints the
/// usual hex line. When enabled, types are resolved lazily per discovered
/// writer `type_name` and cached (the resolution may legitimately fail for a
/// type absent from the IDL — cached as `None` so we don't re-attempt).
struct SpyDecoder {
    book: Option<TypeBook>,
    cache: std::collections::HashMap<String, Option<DynamicType>>,
}

impl SpyDecoder {
    /// Build from CLI flags. `--decode` requires `--type-file`; an unreadable or
    /// unparseable IDL is a hard error (exit 3).
    fn from_args(s: &SubscribeArgs) -> Result<Self, ExitCode> {
        if !s.decode {
            return Ok(Self {
                book: None,
                cache: std::collections::HashMap::new(),
            });
        }
        let Some(type_file) = s.type_file.as_deref() else {
            eprintln!("error: --decode requires --type-file <IDL>");
            return Err(ExitCode::from(2));
        };
        let idl = match std::fs::read_to_string(type_file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read type-file {type_file}: {e}");
                return Err(ExitCode::from(3));
            }
        };
        let book = match TypeBook::from_idl(&idl) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: parsing {type_file}: {e:?}");
                return Err(ExitCode::from(3));
            }
        };
        Ok(Self {
            book: Some(book),
            cache: std::collections::HashMap::new(),
        })
    }

    /// Decode `payload` against the IDL type named `type_name`, returning a
    /// compact JSON string, or `None` when decode is off / the type is unknown /
    /// the bytes don't decode (caller falls back to the hex dump).
    fn decode_json(
        &mut self,
        type_name: &str,
        payload: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> Option<String> {
        let book = self.book.as_ref()?;
        let ty = self
            .cache
            .entry(type_name.to_string())
            .or_insert_with(|| book.resolve(type_name).ok())
            .as_ref()?;
        match decode_dynamic(ty, payload, representation == 1, big_endian) {
            Ok(d) => Some(data_to_json(&d).to_string()),
            Err(e) => {
                eprintln!("warn: decode {type_name} failed: {e:?}");
                None
            }
        }
    }
}

/// Register a raw (opaque-payload) reader announcing `type_name` on `topic` so
/// it matches the typed writer on both ends. Logs the attach; returns the
/// sample receiver, or `None` on registration failure.
fn attach_reader(
    runtime: &DcpsRuntime,
    topic: &str,
    type_name: &str,
) -> Option<Receiver<UserSample>> {
    match runtime.register_user_reader(raw_reader_config_typed(topic, type_name)) {
        Ok((_eid, rx)) => {
            println!("zerodds-spy: attached topic={topic} type={type_name}");
            Some(rx)
        }
        Err(e) => {
            eprintln!("error: register_user_reader({topic}, {type_name}): {e:?}");
            None
        }
    }
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
               --type <NAME>     Announce this IDL type directly (default: discover & follow)\n  \
           -n, --count <N>       Stop after N samples (default: unlimited)\n  \
               --duration <DUR>  Stop after duration (5, 30s, 2m, 1h)\n  \
           -x, --hex <BYTES>     Print first BYTES as hex (default 32, 0=off)\n  \
               --decode          Decode samples to typed JSON (needs --type-file)\n  \
               --type-file <IDL> Out-of-band IDL providing the types to decode\n\
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
