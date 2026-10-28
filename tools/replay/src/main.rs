// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-replay` — inspect/replay CLI for `.zddsrec` files.
//!
//! Crate `zerodds-replay`. Safety classification: **COMFORT**.
//!
//! # Sub-Commands
//!
//! * `inspect FILE` — prints the header + frame counters.
//! * `dump FILE` — lists all frames (timestamp, topic, byte length).
//! * `replay FILE [--time-scale F] [--topic NAME]...` — sends frames
//!   at wallclock pace (scalable), filtered by topic whitelist.
//!   Default: prints them to stdout. With `--inject` they are handed to
//!   `zerodds-c-api` for live-domain re-injection.

#![warn(missing_docs)]
#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr)]

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use zerodds_recorder::{RecordReader, ZDDSREC_VERSION};
use zerodds_recorder_decode::replay_source::{RecordedSample, from_ndjson, from_sqlite};

fn print_help() {
    println!(
        "zerodds-replay — inspect/replay CLI for .zddsrec files

USAGE:
  zerodds-replay inspect FILE
  zerodds-replay dump FILE
  zerodds-replay replay FILE [--time-scale F] [--topic NAME]...
                         [--inject [--inject-domain N]]

  replay accepts .zddsrec (native), .json/.ndjson (decoded NDJSON) and
  .db/.sqlite (decoded SQLite). All three re-publish byte-exact from the
  retained raw CDR bytes.

OPTIONS:
  --time-scale F      1.0 = wallclock; 60.0 = 1h Trace in 1min, etc.
  --topic NAME        Filter — only frames of this ROS/DDS topic name.
                      Repeatable; an empty list = all topics.
  --inject            Re-injection into a live domain.
                      Caches one ZeroDDS writer per topic and
                      publishes the sample stream byte-for-byte.
  --inject-domain N   DCPS domain id for inject (default 0).
"
    );
}

fn cmd_inspect(path: &PathBuf) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let mut r = RecordReader::new(&bytes);
    let h = r.parse_header().map_err(|e| format!("parse header: {e}"))?;
    let mut frames = 0u64;
    let mut bytes_payload = 0u64;
    let mut min_ts = i64::MAX;
    let mut max_ts = i64::MIN;
    while let Some(f) = r
        .next_frame_view()
        .map_err(|e| format!("read frame: {e}"))?
    {
        frames += 1;
        bytes_payload += f.payload.len() as u64;
        if f.timestamp_delta_ns < min_ts {
            min_ts = f.timestamp_delta_ns;
        }
        if f.timestamp_delta_ns > max_ts {
            max_ts = f.timestamp_delta_ns;
        }
    }
    println!("file: {path:?}");
    println!("version       = {ZDDSREC_VERSION} (file claims compatible)");
    println!("time_base_ns  = {}", h.time_base_unix_ns);
    println!("participants  = {}", h.participants.len());
    for p in &h.participants {
        println!("  - {} guid={:02x?}", p.name, p.guid);
    }
    println!("topics        = {}", h.topics.len());
    for t in &h.topics {
        println!("  - {} type={}", t.name, t.type_name);
    }
    println!("frames        = {frames}");
    println!("payload_bytes = {bytes_payload}");
    if frames > 0 {
        println!("ts_range_ns   = [{min_ts} .. {max_ts}]");
        println!("duration_ms   = {:.3}", (max_ts - min_ts) as f64 / 1e6);
    }
    Ok(())
}

fn cmd_dump(path: &PathBuf) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let mut r = RecordReader::new(&bytes);
    let h = r.parse_header().map_err(|e| format!("parse header: {e}"))?;
    println!("# t_delta_ns,participant,topic,kind,payload_len");
    while let Some(f) = r
        .next_frame_view()
        .map_err(|e| format!("read frame: {e}"))?
    {
        let participant = h
            .participants
            .get(f.participant_idx as usize)
            .map(|p| p.name.as_str())
            .unwrap_or("<oor>");
        let topic = h
            .topics
            .get(f.topic_idx as usize)
            .map(|t| t.name.as_str())
            .unwrap_or("<oor>");
        println!(
            "{},{},{},{:?},{}",
            f.timestamp_delta_ns,
            participant,
            topic,
            f.sample_kind,
            f.payload.len()
        );
    }
    Ok(())
}

struct ReplayOptions {
    time_scale: f64,
    topic_filter: HashSet<String>,
    /// When active: no stdout print, but publish via a zerodds writer.
    inject_dcps: bool,
    /// DCPS domain id for re-injection.
    inject_domain: u32,
}

/// Input capture format, detected from the file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// Native `.zddsrec` binary capture (streamed via `RecordReader`).
    Zddsrec,
    /// Decoded NDJSON (one sample per line, `raw_cdr` hex).
    Ndjson,
    /// Decoded SQLite (per-topic tables, `raw_cdr` BLOB).
    Sqlite,
}

/// Detect the capture format from the path extension. Unknown / absent
/// extensions default to `.zddsrec` (the native binary format).
fn detect_format(path: &std::path::Path) -> Format {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json" | "ndjson" | "jsonl") => Format::Ndjson,
        Some("db" | "sqlite" | "sqlite3") => Format::Sqlite,
        _ => Format::Zddsrec,
    }
}

/// Replay dispatcher: pick the reader by format. All three formats carry the
/// raw CDR bytes, so re-injection is byte-exact regardless of source.
fn cmd_replay(path: &PathBuf, opts: &ReplayOptions) -> Result<(), String> {
    match detect_format(path) {
        Format::Zddsrec => cmd_replay_zddsrec(path, opts),
        Format::Ndjson => {
            let f = fs::File::open(path).map_err(|e| format!("open {path:?}: {e}"))?;
            let samples =
                from_ndjson(std::io::BufReader::new(f)).map_err(|e| format!("ndjson: {e}"))?;
            replay_decoded(&samples, opts)
        }
        Format::Sqlite => {
            let p = path.to_str().ok_or("non-UTF8 path")?;
            let samples = from_sqlite(p).map_err(|e| format!("sqlite: {e}"))?;
            replay_decoded(&samples, opts)
        }
    }
}

/// Replay a flat list of decoded samples (NDJSON / SQLite source). Shares the
/// pacing + inject/stdout behaviour of the `.zddsrec` path; the `raw_cdr` bytes
/// re-publish byte-exact.
fn replay_decoded(samples: &[RecordedSample], opts: &ReplayOptions) -> Result<(), String> {
    let scale = if opts.time_scale > 0.0 {
        opts.time_scale
    } else {
        1.0
    };
    let mut inject_state = if opts.inject_dcps {
        Some(InjectState::new(opts.inject_domain)?)
    } else {
        None
    };
    let start_wall = Instant::now();
    let mut first_ts: Option<i64> = None;
    let mut emitted = 0u64;
    for s in samples {
        if !opts.topic_filter.is_empty() && !opts.topic_filter.contains(&s.topic) {
            continue;
        }
        let first = *first_ts.get_or_insert(s.recv_ts_ns);
        let rel_ns = s.recv_ts_ns - first;
        let target = Duration::from_nanos((rel_ns as f64 / scale).max(0.0) as u64);
        let now = start_wall.elapsed();
        if target > now {
            thread::sleep(target - now);
        }
        match &mut inject_state {
            Some(st) => st.publish(&s.topic, &s.type_name, &s.raw_cdr)?,
            None => println!(
                "REPLAY t={} topic={} type={} bytes={}",
                s.recv_ts_ns,
                s.topic,
                s.type_name,
                s.raw_cdr.len()
            ),
        }
        emitted += 1;
    }
    eprintln!("replay: {emitted} samples emitted");
    Ok(())
}

fn cmd_replay_zddsrec(path: &PathBuf, opts: &ReplayOptions) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let mut r = RecordReader::new(&bytes);
    let h = r.parse_header().map_err(|e| format!("parse header: {e}"))?;
    let scale = if opts.time_scale > 0.0 {
        opts.time_scale
    } else {
        1.0
    };
    // ---- Inject mode: cache one writer per topic ----
    let mut inject_state = if opts.inject_dcps {
        Some(InjectState::new(opts.inject_domain)?)
    } else {
        None
    };
    let start_wall = Instant::now();
    let mut first_ts: Option<i64> = None;
    let mut emitted = 0u64;
    while let Some(f) = r
        .next_frame_view()
        .map_err(|e| format!("read frame: {e}"))?
    {
        if !opts.topic_filter.is_empty() {
            let topic = h
                .topics
                .get(f.topic_idx as usize)
                .map(|t| t.name.as_str())
                .unwrap_or("");
            if !opts.topic_filter.contains(topic) {
                continue;
            }
        }
        let first = *first_ts.get_or_insert(f.timestamp_delta_ns);
        let rel_ns = f.timestamp_delta_ns - first;
        let target = Duration::from_nanos((rel_ns as f64 / scale).max(0.0) as u64);
        let now = start_wall.elapsed();
        if target > now {
            thread::sleep(target - now);
        }
        let topic_entry = h.topics.get(f.topic_idx as usize);
        let topic = topic_entry.map(|t| t.name.as_str()).unwrap_or("<oor>");
        let type_name = topic_entry.map(|t| t.type_name.as_str()).unwrap_or("<oor>");
        match &mut inject_state {
            Some(st) => {
                st.publish(topic, type_name, f.payload)?;
            }
            None => {
                println!(
                    "REPLAY t_delta={} topic={} kind={:?} bytes={}",
                    f.timestamp_delta_ns,
                    topic,
                    f.sample_kind,
                    f.payload.len()
                );
            }
        }
        emitted += 1;
    }
    eprintln!("replay: {emitted} frames emitted");
    Ok(())
}

/// Helper state for inject mode: holds a DCPS runtime handle
/// + cache of topic writers.
struct InjectState {
    runtime: *mut zerodds::ZeroDdsRuntime,
    writers: std::collections::HashMap<String, *mut zerodds::ZeroDdsWriter>,
}

impl InjectState {
    fn new(domain: u32) -> Result<Self, String> {
        // SAFETY: zerodds_runtime_create is heap-allocated + NULL on error.
        let rt = unsafe { zerodds::zerodds_runtime_create(domain) };
        if rt.is_null() {
            return Err("dcps runtime create failed".into());
        }
        // SAFETY: rt from zerodds_runtime_create.
        unsafe {
            let _ = zerodds::zerodds_runtime_wait_for_peers(rt, 1, 5_000);
        }
        Ok(Self {
            runtime: rt,
            writers: std::collections::HashMap::new(),
        })
    }
    fn publish(&mut self, topic: &str, type_name: &str, payload: &[u8]) -> Result<(), String> {
        let writer = match self.writers.get(topic) {
            Some(w) => *w,
            None => {
                let topic_c = std::ffi::CString::new(topic).map_err(|e| format!("topic: {e}"))?;
                let type_c = std::ffi::CString::new(type_name).map_err(|e| format!("type: {e}"))?;
                // SAFETY: zerodds_writer_create accepts NULL-checked
                // C strings + heap runtime from zerodds_runtime_create.
                let w = unsafe {
                    zerodds::zerodds_writer_create(
                        self.runtime,
                        topic_c.as_ptr(),
                        type_c.as_ptr(),
                        0,
                    )
                };
                if w.is_null() {
                    return Err(format!("writer_create failed for {topic}"));
                }
                // SAFETY: w from zerodds_writer_create above.
                unsafe {
                    let _ = zerodds::zerodds_writer_wait_for_matched(w, 1, 2_000);
                }
                self.writers.insert(topic.into(), w);
                w
            }
        };
        // SAFETY: writer cached from above or just created;
        // payload + len from caller (frame.payload).
        let rc = unsafe { zerodds::zerodds_writer_write(writer, payload.as_ptr(), payload.len()) };
        if rc != 0 {
            return Err(format!("writer_write rc={rc}"));
        }
        Ok(())
    }
}

impl Drop for InjectState {
    fn drop(&mut self) {
        for (_, w) in self.writers.drain() {
            // SAFETY: w from zerodds_writer_create.
            unsafe { zerodds::zerodds_writer_destroy(w) };
        }
        if !self.runtime.is_null() {
            // SAFETY: runtime from zerodds_runtime_create.
            unsafe { zerodds::zerodds_runtime_destroy(self.runtime) };
        }
    }
}

fn parse_replay_opts(argv: &[String]) -> Result<(PathBuf, ReplayOptions), String> {
    let mut path: Option<PathBuf> = None;
    let mut time_scale = 1.0_f64;
    let mut topics = HashSet::new();
    let mut inject_dcps = false;
    let mut inject_domain: u32 = 0;
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        match a.as_str() {
            "--inject" => {
                inject_dcps = true;
                i += 1;
            }
            "--inject-domain" => {
                let v = argv.get(i + 1).ok_or("--inject-domain needs value")?;
                inject_domain = v.parse().map_err(|e| format!("inject-domain: {e}"))?;
                inject_dcps = true;
                i += 2;
            }
            "--time-scale" => {
                let v = argv.get(i + 1).ok_or("--time-scale needs value")?;
                time_scale = v.parse().map_err(|e| format!("time-scale: {e}"))?;
                i += 2;
            }
            "--topic" => {
                let v = argv.get(i + 1).ok_or("--topic needs value")?;
                topics.insert(v.clone());
                i += 2;
            }
            other if !other.starts_with("--") => {
                if path.is_some() {
                    return Err(format!("unexpected extra argument: {other}"));
                }
                path = Some(PathBuf::from(other));
                i += 1;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    let p = path.ok_or("FILE positional argument required")?;
    Ok((
        p,
        ReplayOptions {
            time_scale,
            topic_filter: topics,
            inject_dcps,
            inject_domain,
        },
    ))
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().skip(1).any(|a| a == "--version" || a == "-V") {
        println!("zerodds-replay {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if argv.len() < 2 {
        // Nothing to do is a command-line error, as for the other tools.
        print_help();
        return ExitCode::from(2);
    }
    if matches!(argv[1].as_str(), "-h" | "--help" | "help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    let cmd = argv[1].clone();
    let rest: Vec<String> = argv.iter().skip(2).cloned().collect();
    let r = match cmd.as_str() {
        "inspect" => {
            let p = match rest.first() {
                Some(p) => PathBuf::from(p),
                None => {
                    print_help();
                    return ExitCode::from(2);
                }
            };
            cmd_inspect(&p)
        }
        "dump" => {
            let p = match rest.first() {
                Some(p) => PathBuf::from(p),
                None => {
                    print_help();
                    return ExitCode::from(2);
                }
            };
            cmd_dump(&p)
        }
        "replay" => {
            let (p, opts) = match parse_replay_opts(&rest) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("error: {e}");
                    print_help();
                    return ExitCode::from(2);
                }
            };
            cmd_replay(&p, &opts)
        }
        other => {
            eprintln!("unknown subcommand: {other}");
            print_help();
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
