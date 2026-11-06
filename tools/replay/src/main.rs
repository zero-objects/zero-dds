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

use std::collections::HashMap;
use std::sync::Arc;

use zerodds_cli_common::{reliable_raw_writer_config, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_recorder::{RecordReader, SampleKind, ZDDSREC_VERSION};
use zerodds_recorder_decode::replay_source::{RecordedSample, from_ndjson, from_sqlite};
use zerodds_rtps::wire_types::EntityId;

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
                      publishes the sample stream byte-for-byte, each
                      sample under the representation identifier of
                      its encapsulation header as recorded.
  --inject-domain N   DCPS domain id for inject (default 0).
  --representation H  The representation identifier (two octets in hex,
                      e.g. 0007 for CDR2_LE) for samples whose capture
                      does not record it (format version 2, or a decoded
                      capture made before it was kept). Without it such
                      samples are not injected.
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
    println!(
        "version       = {} (this tool reads up to {ZDDSREC_VERSION})",
        r.version()
    );
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
    /// `--representation`: the identifier for samples recorded without one.
    representation: Option<[u8; 2]>,
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
        Some(InjectState::new(opts.inject_domain, opts.representation)?)
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
            Some(st) => st.publish(
                &s.topic,
                &s.type_name,
                s.representation_identifier,
                &s.raw_cdr,
            )?,
            None => println!(
                "REPLAY t={} topic={} type={} repr={} bytes={}",
                s.recv_ts_ns,
                s.topic,
                s.type_name,
                identifier_text(s.representation_identifier),
                s.raw_cdr.len()
            ),
        }
        emitted += 1;
    }
    eprintln!("replay: {emitted} samples emitted");
    if let Some(st) = inject_state {
        st.finish()?;
    }
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
        Some(InjectState::new(opts.inject_domain, opts.representation)?)
    } else {
        None
    };
    let start_wall = Instant::now();
    let mut first_ts: Option<i64> = None;
    let mut emitted = 0u64;
    let mut lifecycle_not_injected = 0u64;
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
        // A topic a `TopicAdd` put into the index continues the header's list.
        let topic_entry = h.topics.get(f.topic_idx as usize).or_else(|| {
            r.added_topics()
                .get((f.topic_idx as usize).checked_sub(h.topics.len())?)
        });
        let topic = topic_entry.map(|t| t.name.as_str()).unwrap_or("<oor>");
        let type_name = topic_entry.map(|t| t.type_name.as_str()).unwrap_or("<oor>");
        match &mut inject_state {
            // A dispose or unregister carries no body; it is no sample to write.
            Some(_) if f.sample_kind != SampleKind::Alive => {
                lifecycle_not_injected += 1;
                continue;
            }
            Some(st) => {
                st.publish(topic, type_name, f.representation_identifier, f.payload)?;
            }
            None => {
                println!(
                    "REPLAY t_delta={} topic={} kind={:?} repr={} bytes={}",
                    f.timestamp_delta_ns,
                    topic,
                    f.sample_kind,
                    identifier_text(f.representation_identifier),
                    f.payload.len()
                );
            }
        }
        emitted += 1;
    }
    eprintln!("replay: {emitted} frames emitted");
    if lifecycle_not_injected > 0 {
        eprintln!(
            "replay: {lifecycle_not_injected} dispose/unregister frames not injected \
             (no body to write)"
        );
    }
    if let Some(st) = inject_state {
        st.finish()?;
    }
    Ok(())
}

/// The identifier as the help text names it (`0007`), or `unknown`.
fn identifier_text(id: Option<[u8; 2]>) -> String {
    id.map_or_else(
        || "unknown".to_string(),
        |id| format!("{:02x}{:02x}", id[0], id[1]),
    )
}

/// `--representation`: two octets in hex.
fn parse_identifier(s: &str) -> Result<[u8; 2], String> {
    let s = s.trim_start_matches("0x");
    if s.len() != 4 {
        return Err(format!(
            "--representation {s}: two octets in hex, e.g. 0007"
        ));
    }
    let v = u16::from_str_radix(s, 16)
        .map_err(|_| format!("--representation {s}: two octets in hex, e.g. 0007"))?;
    Ok(v.to_be_bytes())
}

/// Helper state for inject mode: a DCPS runtime and one writer per topic,
/// with the representation identifier it sends under now.
struct InjectState {
    runtime: Arc<DcpsRuntime>,
    writers: HashMap<String, (EntityId, Option<[u8; 2]>)>,
    /// `--representation`: for samples recorded without an identifier.
    representation: Option<[u8; 2]>,
}

impl InjectState {
    fn new(domain: u32, representation: Option<[u8; 2]>) -> Result<Self, String> {
        let domain =
            i32::try_from(domain).map_err(|_| format!("domain {domain} does not fit i32"))?;
        let config =
            RuntimeConfig::from_standard_paths().map_err(|e| format!("runtime config: {e}"))?;
        let runtime = DcpsRuntime::start(domain, stable_prefix(0xF9), config)
            .map_err(|e| format!("dcps runtime start: {e:?}"))?;
        // A peer to replay to, for at most 5 s, as before.
        let deadline = Instant::now() + Duration::from_secs(5);
        while runtime.discovered_participants().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        Ok(Self {
            runtime,
            writers: HashMap::new(),
            representation,
        })
    }

    /// Writes `payload` on `topic` under `identifier`, the representation
    /// identifier it was recorded with (DDSI-RTPS 2.5 §10.5): the writer
    /// offers that XCDR version and declares that extensibility and byte
    /// order. Writing it under the writer's own header (XCDR2 little endian,
    /// final) told a reader to read an XCDR1, big-endian, `@appendable` or
    /// `@mutable` body as something it is not.
    fn publish(
        &mut self,
        topic: &str,
        type_name: &str,
        identifier: Option<[u8; 2]>,
        payload: &[u8],
    ) -> Result<(), String> {
        let identifier = identifier.or(self.representation).ok_or_else(|| {
            format!(
                "{topic}: the capture does not record the representation identifier of its \
                 samples (format version 2, or a decoded capture made before it was kept); \
                 --representation names it"
            )
        })?;
        let (eid, current) = match self.writers.get(topic) {
            Some(entry) => *entry,
            None => {
                let eid = self
                    .runtime
                    .register_user_writer(reliable_raw_writer_config(topic, type_name))
                    .map_err(|e| format!("writer for {topic}: {e:?}"))?;
                // A matched reader, for at most 2 s, as before.
                let deadline = Instant::now() + Duration::from_secs(2);
                while self.runtime.user_writer_matched_count(eid) == 0 && Instant::now() < deadline
                {
                    thread::sleep(Duration::from_millis(20));
                }
                self.writers.insert(topic.to_string(), (eid, None));
                (eid, None)
            }
        };
        if current != Some(identifier) {
            self.runtime
                .set_user_writer_representation_identifier(eid, identifier)
                .map_err(|e| {
                    format!("{topic}: representation identifier {identifier:02x?}: {e:?}")
                })?;
            self.writers
                .insert(topic.to_string(), (eid, Some(identifier)));
        }
        self.runtime
            .write_user_sample(eid, payload.to_vec())
            .map_err(|e| format!("write on {topic}: {e:?}"))
    }
}

impl InjectState {
    /// Waits until the reliable readers of every writer have acknowledged
    /// what it wrote, at most 5 s for all of them (DDS 1.4 §2.2.2.4.2.15
    /// `wait_for_acknowledgments`). The runtime ends with the process: a
    /// sample a reader has not received yet, because it matched the writer
    /// after the write, would go with it unrepaired.
    fn finish(self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut missing: Vec<&str> = self
            .writers
            .iter()
            .filter(|(_, (eid, _))| !self.runtime.await_user_writer_acknowledged(*eid, deadline))
            .map(|(topic, _)| topic.as_str())
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        missing.sort_unstable();
        Err(format!(
            "not acknowledged by every reader within 5 s: {}",
            missing.join(", ")
        ))
    }
}

impl Drop for InjectState {
    fn drop(&mut self) {
        self.runtime.shutdown();
    }
}

fn parse_replay_opts(argv: &[String]) -> Result<(PathBuf, ReplayOptions), String> {
    let mut path: Option<PathBuf> = None;
    let mut time_scale = 1.0_f64;
    let mut topics = HashSet::new();
    let mut inject_dcps = false;
    let mut inject_domain: u32 = 0;
    let mut representation: Option<[u8; 2]> = None;
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
            "--representation" => {
                let v = argv.get(i + 1).ok_or("--representation needs value")?;
                representation = Some(parse_identifier(v)?);
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
            representation,
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
