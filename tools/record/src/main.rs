// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-record` — CLI frontend for the `zddsrec` capture format.
//!
//! ## Sub-Commands
//!
//! ```text
//! zerodds-record record   -t TOPIC ...  [-o FILE] [-d DOMAIN] [--duration DUR]
//! zerodds-record info     <FILE>
//! zerodds-record list     <FILE>
//! ```
//!
//! Spec: `docs/specs/zddsrec-1.0.md` and
//! `docs/specs/zerodds-deployment-1.0.md` §1.1.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zerodds_ast_layout::key::Endianness;
use zerodds_cli_common::{participant_guid, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_record::{
    Command, RecordArgs, count_frames_per_topic, parse_args, read_header_summary,
};
use zerodds_recorder::format::{ParticipantEntry, SampleKind};
use zerodds_recorder::session::{RecordingSession, SessionOptions, TopicKey};
use zerodds_recorder_decode::json_sink::{JsonSink, to_hex};
use zerodds_recorder_decode::sqlite_sink::SqliteSink;
use zerodds_recorder_decode::type_source::{TypeBook, parse_topic_type_map};
use zerodds_recorder_decode::wire::encoding_of;
use zerodds_rtps::wire_types::EntityId;
use zerodds_types::dynamic::DynamicData;
use zerodds_types::dynamic::type_::DynamicType;
use zerodds_wire_codec::{ProbeRequest, decode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-record {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let configured = match zerodds_dcps::cli::Configured::from_args(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: configuration: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(code) = configured.check_config() {
        return code;
    }
    let config = &configured.runtime;

    let cmd = match parse_args(&configured.args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            return ExitCode::from(2);
        }
    };

    match cmd {
        Command::Record(r) => run_record(r, config),
        Command::Info(i) => run_info(&i.file),
        Command::List(i) => run_list(&i.file),
    }
}

fn run_record(r: RecordArgs, config: &RuntimeConfig) -> ExitCode {
    if r.topics.is_empty() {
        eprintln!("error: at least one --topic is required for capture");
        return ExitCode::from(2);
    }

    // Determine the output path.
    let out_path = r.output.clone().unwrap_or_else(|| {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("capture-{ts}.zddsrec")
    });
    let file = match File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot create {out_path}: {e}");
            return ExitCode::from(3);
        }
    };

    // Start the DCPS runtime.
    let domain_id: i32 = match r.domain.try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: domain {} does not fit i32", r.domain);
            return ExitCode::from(2);
        }
    };
    let prefix = stable_prefix(0xFE);
    let runtime = match DcpsRuntime::start(domain_id, prefix, config.clone()) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: DcpsRuntime::start failed: {e:?}");
            return ExitCode::from(3);
        }
    };

    // Type-following discovery settle: wait briefly for the requested topics'
    // writers so we learn each writer's REAL type_name. A recorder announcing a
    // generic `RawBytes` reader would never match a typed writer (e.g.
    // cuas::Track) and capture nothing — the root cause of the "record
    // validates and exits" report.
    //
    // A topic whose writer has not started yet is not settled here and not
    // given up on either: `attach_late_topics` picks it up in the drain loop
    // and adds it to the capture then. The window is a courtesy that saves a
    // `TopicAdd` frame for the common case, not a deadline.
    let mut topics = resolve_topic_types(&runtime, &r.topics, Duration::from_secs(3));

    let now_unix_ns = unix_ns_now();
    let opts = SessionOptions {
        topics: topics.clone(),
        participants: vec![ParticipantEntry {
            guid: participant_guid(prefix),
            name: format!("zerodds-record:{}", std::process::id()),
        }],
        time_base_unix_ns: now_unix_ns,
    };
    let session = Arc::new(RecordingSession::new(BufWriter::new(file), opts));
    let _ = r.max_sample_bytes; // DoS cap will be enforced in a future iteration via a config hook.

    // One type-following reader per resolved (topic, type) — announces the real
    // type so it matches the typed writer on both ends.
    let mut readers = Vec::with_capacity(topics.len());
    for (idx, t) in topics.iter().enumerate() {
        let cfg = user_reader_config(&t.topic, &t.type_name);
        match runtime.register_user_reader(cfg) {
            Ok((eid, rx)) => {
                println!(
                    "zerodds-record: attached topic={} type={}",
                    t.topic, t.type_name
                );
                readers.push((idx, eid, rx));
            }
            Err(e) => {
                eprintln!(
                    "error: register_user_reader({}, {}): {e:?}",
                    t.topic, t.type_name
                );
                return ExitCode::from(3);
            }
        }
    }

    // Optional decode-sinks (typed JSON / SQLite) on top of the opaque capture.
    let mut decode = match setup_decode(&r, &topics, config.codec_dialect) {
        Ok(d) => d,
        Err(code) => return code,
    };

    // Shutdown flag — SIGINT triggers it.
    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));

    println!(
        "zerodds-record: capturing {} topics on domain {} → {}",
        topics.len(),
        domain_id,
        out_path
    );
    let started = Instant::now();
    let deadline = r.duration.map(|d| started + d);
    let session_for_loop = Arc::clone(&session);
    let part_guid = participant_guid(prefix);
    // Per topic, so a topic that records nothing is visible at the end.
    // The totals alone hide it: a run with one silent topic among
    // seventeen looks exactly like a healthy one.
    let mut per_topic: Vec<u64> = vec![0; topics.len()];
    let mut next_writer_check = Instant::now() + Duration::from_secs(2);

    // Drain loop: sweep every reader channel without blocking and write each
    // sample into the session. Ends on stop flag or deadline.
    //
    // A blocking receive per channel would make every idle topic hold up the
    // whole sweep for the length of its timeout, and taking a single sample per
    // channel per sweep would cap each topic at one sample per sweep however
    // fast its writer publishes. The channels behind `register_user_reader` are
    // unbounded, so neither costs samples outright — the backlog just grows and
    // whatever is still queued at SIGINT never reaches the file, which turns
    // the capture into a truncated prefix of the traffic.
    while !stop.load(Ordering::Relaxed) {
        if let Some(end) = deadline {
            if Instant::now() >= end {
                break;
            }
        }
        let mut got_any = false;
        'sweep: for (topic_idx, _eid, rx) in &readers {
            while let Ok(sample) = rx.try_recv() {
                got_any = true;
                let topic = &topics[*topic_idx];
                write_sample(&mut decode, &session_for_loop, part_guid, topic, sample);
                if let Some(count) = per_topic.get_mut(*topic_idx) {
                    *count += 1;
                }
                // A writer that publishes faster than this loop consumes would
                // keep the inner drain running indefinitely, so shutdown and
                // `--duration` have to be observed between samples rather than
                // only between sweeps.
                if stop.load(Ordering::Relaxed) {
                    break 'sweep;
                }
                if let Some(end) = deadline {
                    if Instant::now() >= end {
                        break 'sweep;
                    }
                }
            }
        }
        // Nothing was pending on any channel. The pause is short because it is
        // now the only delay between a sample arriving and being written.
        if !got_any {
            std::thread::sleep(Duration::from_millis(1));
        }
        // Discovery keeps running after the settle window closed, so a
        // writer that arrives late is knowable. Checked here rather than
        // per sweep because it walks the discovery table.
        if Instant::now() >= next_writer_check {
            attach_late_topics(
                &runtime,
                &session,
                &r.topics,
                &mut topics,
                &mut readers,
                &mut per_topic,
            );
            next_writer_check = Instant::now() + Duration::from_secs(1);
        }
    }

    // What is still queued when the loop ends. The channels are unbounded, so
    // a stop leaves whatever arrived but was not yet written sitting in them,
    // and dropping the readers below discards it without a word — the capture
    // would end early and nothing in the output would say so. This sweep
    // writes what it can reach and reports what it could not.
    //
    // It is bounded by a deadline rather than run until the channels are
    // empty: a writer that is still publishing keeps them from ever being
    // empty, and a recorder that will not exit on SIGINT is worse than one
    // that ends a sample short.
    let flush_until = Instant::now() + Duration::from_millis(500);
    let mut flushed: u64 = 0;
    let mut left_behind: u64 = 0;
    'flush: for (topic_idx, _eid, rx) in &readers {
        loop {
            // The deadline before the receive, so a sample is either
            // written or counted as left behind — checking it after the
            // write says nothing about whether anything is still there,
            // and the message would claim a backlog on the strength of
            // one slow write.
            if Instant::now() >= flush_until {
                break 'flush;
            }
            match rx.try_recv() {
                Ok(sample) => {
                    let topic = &topics[*topic_idx];
                    write_sample(&mut decode, &session_for_loop, part_guid, topic, sample);
                    flushed += 1;
                    if let Some(count) = per_topic.get_mut(*topic_idx) {
                        *count += 1;
                    }
                }
                Err(_) => break,
            }
        }
    }
    // What the deadline cut off, counted rather than guessed at. These
    // samples are discarded — the capture ends here — so the count is
    // the one number that says how much of the traffic the file does
    // not hold.
    for (_topic_idx, _eid, rx) in &readers {
        while rx.try_recv().is_ok() {
            left_behind += 1;
        }
    }
    if flushed > 0 {
        eprintln!("zerodds-record: {flushed} sample(s) written while shutting down");
    }
    if left_behind > 0 {
        eprintln!(
            "zerodds-record: {left_behind} sample(s) were still queued after 500ms of \
             shutdown and are not in the capture"
        );
    }

    // Per topic, before the totals: a topic at zero is the whole point
    // of the line, and a reader that never matched its writer is what it
    // looks like.
    for (index, topic) in topics.iter().enumerate() {
        let count = per_topic.get(index).copied().unwrap_or(0);
        if count == 0 {
            println!(
                "zerodds-record: {} — NO samples recorded (type {})",
                topic.topic, topic.type_name
            );
        } else {
            println!("zerodds-record: {} — {count} samples", topic.topic);
        }
    }

    let stats = session.stats();
    let elapsed = started.elapsed();
    println!(
        "zerodds-record: stopped after {:.1}s · samples={} dropped={} bytes={}",
        elapsed.as_secs_f64(),
        stats.samples_total,
        stats.samples_dropped,
        stats.bytes_total
    );
    drop(readers);
    // Not just a drop: the worker threads hold the runtime, so dropping
    // this handle neither stops it nor sends the SPDP departure nor closes
    // the same-host channels; their segments stayed in /dev/shm after
    // every capture (a multi-process test chain, 16 rings per run).
    runtime.shutdown();
    drop(runtime);
    ExitCode::SUCCESS
}

/// One sample, into the capture and into whatever decoded sinks are on.
///
/// Its own function because the drain loop is not the only caller: the sweep
/// that runs after the loop, over what is still queued at shutdown, has to do
/// exactly the same thing with a sample, and two copies of this would drift.
fn write_sample(
    decode: &mut DecodeSinks,
    session: &RecordingSession<BufWriter<File>>,
    part_guid: [u8; 16],
    topic: &TopicKey,
    sample: UserSample,
) {
    let res = match sample {
        UserSample::Alive {
            payload,
            writer_guid,
            representation,
            big_endian,
            ..
        } => {
            let ts = unix_ns_now();
            decode.consume(
                &topic.topic,
                ts,
                writer_guid,
                &payload,
                representation,
                big_endian,
            );
            session.record_sample(ts, writer_guid, topic, SampleKind::Alive, payload.to_vec())
        }
        UserSample::Lifecycle { kind, .. } => {
            let mapped = match kind {
                zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed => {
                    SampleKind::NotAliveDisposed
                }
                _ => SampleKind::NotAliveUnregistered,
            };
            session.record_sample(unix_ns_now(), part_guid, topic, mapped, Vec::new())
        }
        // DDSI-RTPS 2.5 §8.7.6 — the end-of-coherent-set marker carries no user
        // data. `.zddsrec` has no sample kind for it, so it is not recorded;
        // a replay of the capture reproduces the samples, not the set cut.
        UserSample::CoherentSetEnd { .. } => Ok(()),
    };
    if let Err(e) = res {
        eprintln!("warn: record_sample dropped: {e:?}");
    }
}

/// Optional typed sinks layered on top of the opaque `.zddsrec` capture.
///
/// When `--decode` is off (or no type resolves for a topic) `consume` is a
/// no-op: the raw `.zddsrec` capture is always written regardless. The decoded
/// outputs additionally retain the raw CDR bytes per sample so `zerodds-replay`
/// can re-publish byte-exact from JSON or SQLite.
struct DecodeSinks {
    json: Option<JsonSink<BufWriter<File>>>,
    sqlite: Option<SqliteSink>,
    /// topic → (resolved `DynamicType`, DDS type name) for decoded topics.
    types: std::collections::HashMap<String, (DynamicType, String)>,
    /// The dialect samples are decoded in (`--dialect`).
    dialect: zerodds_ast_layout::Dialect,
}

impl DecodeSinks {
    fn disabled() -> Self {
        Self {
            json: None,
            sqlite: None,
            types: std::collections::HashMap::new(),
            dialect: zerodds_ast_layout::Dialect::default(),
        }
    }

    /// Decode one Alive sample (if a type is known for `topic`) and fan it out
    /// to the configured sinks. Decode/write failures warn but never abort the
    /// capture — the opaque `.zddsrec` write already happened.
    fn consume(
        &mut self,
        topic: &str,
        recv_ts_ns: i64,
        writer_guid: [u8; 16],
        payload: &[u8],
        representation: u8,
        big_endian: bool,
    ) {
        let Some((ty, type_name)) = self.types.get(topic).cloned() else {
            return;
        };
        // The reader is told the XCDR version, the byte order and the
        // dialect; the framing follows from the type's extensibility
        // (XTypes 1.3 §7.4 Table 17).
        let request = ProbeRequest::in_dialect(
            &type_name,
            encoding_of(&ty, representation == 1),
            if big_endian {
                Endianness::Big
            } else {
                Endianness::Little
            },
            self.dialect,
        );
        let data: DynamicData = match decode(ty.ast(), &request, payload) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("warn: decode {topic} dropped: {e:?}");
                return;
            }
        };
        let writer_hex = to_hex(&writer_guid);
        if let Some(j) = self.json.as_mut() {
            if let Err(e) =
                j.write_sample(topic, &type_name, recv_ts_ns, &writer_hex, &data, payload)
            {
                eprintln!("warn: json write {topic} dropped: {e}");
            }
        }
        if let Some(s) = self.sqlite.as_mut() {
            if let Err(e) = s.insert_sample(topic, &ty, recv_ts_ns, &writer_hex, payload, &data) {
                eprintln!("warn: sqlite write {topic} dropped: {e:?}");
            }
        }
    }
}

/// Build the typed sinks from CLI flags. Returns a disabled sink when
/// `--decode` is absent. Errors (exit code 2/3) on a missing `--type-file` or
/// unreadable IDL / unopenable sink.
fn setup_decode(
    r: &RecordArgs,
    topics: &[TopicKey],
    configured: zerodds_ast_layout::Dialect,
) -> Result<DecodeSinks, ExitCode> {
    if !r.decode {
        return Ok(DecodeSinks::disabled());
    }
    let Some(type_file) = r.type_file.as_deref() else {
        eprintln!("error: --decode requires --type-file <IDL>");
        return Err(ExitCode::from(2));
    };
    if r.out_json.is_none() && r.out_sqlite.is_none() {
        eprintln!("warn: --decode without --out-json/--out-sqlite produces no typed output");
    }
    let idl_src = match std::fs::read_to_string(type_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read type-file {type_file}: {e}");
            return Err(ExitCode::from(3));
        }
    };
    let book = match TypeBook::from_idl(&idl_src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: parsing {type_file}: {e:?}");
            return Err(ExitCode::from(3));
        }
    };
    let overrides: std::collections::HashMap<String, String> = match parse_topic_type_map(&r.map) {
        Ok(v) => v.into_iter().collect(),
        Err(e) => {
            eprintln!("error: bad --map entry: {e:?}");
            return Err(ExitCode::from(2));
        }
    };

    let json = match r.out_json.as_deref() {
        Some(p) => match File::create(p) {
            Ok(f) => Some(JsonSink::new(BufWriter::new(f))),
            Err(e) => {
                eprintln!("error: cannot create {p}: {e}");
                return Err(ExitCode::from(3));
            }
        },
        None => None,
    };
    let mut sqlite = match r.out_sqlite.as_deref() {
        Some(p) => match SqliteSink::create(p) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("error: cannot create sqlite {p}: {e}");
                return Err(ExitCode::from(3));
            }
        },
        None => None,
    };

    let mut types = std::collections::HashMap::new();
    for t in topics {
        if t.type_name == RAW_BYTES_TYPE_NAME {
            continue; // opaque fallback — nothing to decode against
        }
        let want = overrides.get(&t.topic).unwrap_or(&t.type_name);
        let ty = match book.resolve(want) {
            Ok(ty) => ty,
            Err(e) => {
                eprintln!(
                    "warn: no IDL type '{want}' for topic '{}' — captured opaque only ({e:?})",
                    t.topic
                );
                continue;
            }
        };
        if let Some(s) = sqlite.as_mut() {
            if let Err(e) = s.ensure_topic(&t.topic, want, &idl_src, &ty) {
                eprintln!("error: sqlite schema for {}: {e:?}", t.topic);
                return Err(ExitCode::from(3));
            }
        }
        println!("zerodds-record: decode topic={} as {want}", t.topic);
        types.insert(t.topic.clone(), (ty, want.clone()));
    }

    Ok(DecodeSinks {
        json,
        sqlite,
        types,
        // `--dialect`, else the configuration's `wire.codec_dialect`.
        dialect: r.dialect.unwrap_or(configured),
    })
}

fn run_info(file: &str) -> ExitCode {
    match read_header_summary(file) {
        Ok(s) => {
            println!("zddsrec header: {file}");
            println!("  time-base (unix-ns): {}", s.time_base_unix_ns);
            println!("  participants:        {}", s.participants);
            println!("  topics ({}):", s.topics.len());
            for t in &s.topics {
                println!("    - {t}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error reading {file}: {e}");
            ExitCode::from(3)
        }
    }
}

fn run_list(file: &str) -> ExitCode {
    match count_frames_per_topic(file) {
        Ok(counts) => {
            println!("topic-frame counts: {file}");
            let total: u64 = counts.iter().map(|(_, c)| *c).sum();
            println!("  total frames: {total}");
            for (t, c) in &counts {
                println!("    {c:>10}  {t}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error reading {file}: {e}");
            ExitCode::from(3)
        }
    }
}

/// The opaque type announced when no concrete writer type is discovered for a
/// requested topic (so a genuine RawBytes writer still records).
const RAW_BYTES_TYPE_NAME: &str = "zerodds::RawBytes";

/// Build an opaque (raw-payload) reader config that announces `type_name`.
///
/// Type-following: DDS matches reader↔writer by type-name equality on both ends,
/// so to capture a typed topic (e.g. `cuas::Track`) the recorder must announce
/// the writer's *real* `type_name`, discovered via SEDP — not a generic
/// `RawBytes`. The payload is stored opaquely (lossless CDR bytes).
fn user_reader_config(topic: &str, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        ..Default::default()
    }
}

/// Resolve each requested topic to the real `type_name`(s) of its discovered
/// writers, polling SEDP for up to `settle`. Returns one [`TopicKey`] per
/// distinct `(topic, type)` pair (so a topic carrying two writer types yields
/// two keys → two readers).
///
/// A requested topic with no writer in the window is reported and left out.
/// It is not lost: [`attach_late_topics`] adds it once a writer appears. The
/// alternative — a reader announcing the opaque `RawBytes` type — matched no
/// typed writer and recorded nothing for the whole run.
fn resolve_topic_types(rt: &DcpsRuntime, requested: &[String], settle: Duration) -> Vec<TopicKey> {
    let want: std::collections::HashSet<&str> = requested.iter().map(String::as_str).collect();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut found: Vec<TopicKey> = Vec::new();
    let deadline = Instant::now() + settle;
    loop {
        for (t, ty) in rt.discovered_publication_topics() {
            if want.contains(t.as_str()) && seen.insert((t.clone(), ty.clone())) {
                found.push(TopicKey {
                    topic: t,
                    type_name: ty,
                });
            }
        }
        // Stop early once every requested topic has at least one writer type.
        let covered = requested
            .iter()
            .all(|t| found.iter().any(|k| &k.topic == t));
        if covered || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for t in requested {
        if !found.iter().any(|k| &k.topic == t) {
            eprintln!(
                "warn: no writer discovered for topic '{t}' within the settle window. \
                 The recorder keeps watching and starts recording it as soon as a \
                 writer appears; only what was published before that is missed."
            );
        }
    }
    found
}

/// Attach a reader to a requested topic whose writer has now appeared.
///
/// A recorder learns a topic's type from the writer that publishes it,
/// and a writer that starts after the recorder is an ordinary case, not
/// an error. The settle window at startup is therefore a courtesy and
/// not a deadline: what it does not find, this finds later, and the
/// topic is recorded from that moment on.
///
/// It used to end differently. A topic with no writer in the window got
/// a reader announcing the opaque `zerodds::RawBytes`, which no typed
/// writer ever matches — so the topic recorded nothing for the whole
/// run, and after one warning at startup nothing said so. That cost a
/// measurement run: sixty seconds, attached cleanly, zero frames.
///
/// The addition travels in the capture as a `TopicAdd` frame, so the
/// run stays one file and the topic keeps its name and type in it.
fn attach_late_topics(
    rt: &DcpsRuntime,
    session: &RecordingSession<BufWriter<File>>,
    wanted: &[String],
    topics: &mut Vec<TopicKey>,
    readers: &mut Vec<(usize, EntityId, Receiver<UserSample>)>,
    per_topic: &mut Vec<u64>,
) {
    for (topic, type_name) in rt.discovered_publication_topics() {
        if !wanted.iter().any(|w| w == &topic) {
            continue;
        }
        let key = TopicKey {
            topic: topic.clone(),
            type_name: type_name.clone(),
        };
        if topics.contains(&key) {
            continue;
        }
        // Into the capture's index first. A reader whose samples have
        // nowhere to go would drop them and count them as belonging to
        // an unknown topic.
        let index = match session.add_topic(&key) {
            Ok(index) => index as usize,
            Err(e) => {
                eprintln!("warn: cannot add topic '{topic}' to the capture: {e}");
                continue;
            }
        };
        match rt.register_user_reader(user_reader_config(&topic, &type_name)) {
            Ok((eid, rx)) => {
                println!(
                    "zerodds-record: attached topic={topic} type={type_name} \
                     (its writer appeared after the settle window)"
                );
                topics.push(key);
                if per_topic.len() <= index {
                    per_topic.resize(index + 1, 0);
                }
                readers.push((index, eid, rx));
            }
            Err(e) => {
                eprintln!("warn: register_user_reader({topic}, {type_name}): {e:?}");
            }
        }
    }
}

fn unix_ns_now() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total = dur
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(dur.subsec_nanos()));
    total as i64
}

#[cfg(unix)]
fn install_signal_handler(stop: Arc<AtomicBool>) {
    use std::sync::Mutex;
    static HOOK: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
    if let Ok(mut g) = HOOK.lock() {
        *g = Some(stop);
    }
    extern "C" fn handler(_: i32) {
        if let Ok(g) = HOOK.lock() {
            if let Some(s) = g.as_ref() {
                s.store(true, Ordering::Relaxed);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: libc::signal takes a C-ABI function pointer; `handler`
        // is `extern "C"` and matches the libc signature.
        unsafe {
            libc::signal(libc::SIGINT, handler as usize);
            libc::signal(libc::SIGTERM, handler as usize);
        }
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: libc::signal call as on Linux; `handler` is
        // `extern "C"` and ABI-compatible.
        unsafe {
            libc::signal(libc::SIGINT, handler as usize);
            libc::signal(libc::SIGTERM, handler as usize);
        }
    }
}

#[cfg(not(unix))]
fn install_signal_handler(_stop: Arc<AtomicBool>) {
    // Windows: no graceful CTRL+C path without further deps.
    // User stops with task-kill; --duration is an alternative stop.
}

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-record {v}\n\
         Capture and inspect `.zddsrec` files.\n\
\n\
         USAGE:\n  \
           zerodds-record <SUBCOMMAND> [OPTIONS]\n\
\n\
         SUBCOMMANDS:\n  \
           record                       Live-capture into a `.zddsrec` (Ctrl-C or --duration to stop)\n  \
           info  <FILE>                 Print zddsrec header (time-base, participants, topics)\n  \
           list  <FILE>                 Print frame counts per topic\n\
\n\
         OPTIONS for `record` (at least one --topic required):\n  \
           -o, --output <FILE>          Output `.zddsrec` (default: capture-<unix-ts>.zddsrec)\n  \
           -d, --domain <ID>            DDS-Domain-ID (default 0)\n  \
           -t, --topic <NAME>           Topic name to subscribe (repeatable, REQUIRED)\n  \
               --duration <DUR>         Recording duration (5, 30s, 2m, 1h)\n  \
               --max-sample-bytes <N>   DoS cap per sample (default 1048576)\n\
\n\
         DECODED OUTPUT for `record` (opaque .zddsrec is always written too):\n  \
               --decode                 Decode samples to typed values (needs --type-file)\n  \
               --type-file <IDL>        Out-of-band IDL providing the types\n  \
               --map <TOPIC=Type>       Override which IDL type decodes a topic (repeatable)\n  \
               --out-json <FILE>        Write decoded samples as NDJSON\n  \
               --out-sqlite <FILE>      Write decoded samples to per-topic SQLite (replaces FILE)\n  \
               --dialect <NAME>         Wire dialect to decode in (default: wire.codec_dialect,\n                                \
                                        specification; cyclone, rti, fastdds, opendds, ...)\n\
\n\
         GLOBAL OPTIONS:\n  \
           -h, --help                   Show this message\n  \
           -V, --version                Print version\n\
\n\
         EXIT CODES:\n  \
           0    success\n  \
           2    CLI parse error\n  \
           3    DDS / I/O error\n\
\n\
         SPEC: docs/specs/zddsrec-1.0.md"
    );
    println!("\n{}", zerodds_dcps::cli::HELP);
}
