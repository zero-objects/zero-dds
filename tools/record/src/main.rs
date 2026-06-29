// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-record` — CLI-Frontend für das `zddsrec`-Capture-Format.
//!
//! ## Sub-Commands
//!
//! ```text
//! zerodds-record record   -t TOPIC ...  [-o FILE] [-d DOMAIN] [--duration DUR]
//! zerodds-record info     <FILE>
//! zerodds-record list     <FILE>
//! ```
//!
//! Spec: `docs/specs/zddsrec-1.0.md` und
//! `docs/specs/zerodds-deployment-1.0.md` §1.1.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LivelinessQosPolicy, OwnershipKind};
use zerodds_record::{
    Command, RecordArgs, count_frames_per_topic, parse_args, read_header_summary,
};
use zerodds_recorder::format::{ParticipantEntry, SampleKind};
use zerodds_recorder::session::{RecordingSession, SessionOptions, TopicKey};
use zerodds_recorder_decode::json_sink::{JsonSink, to_hex};
use zerodds_recorder_decode::sqlite_sink::SqliteSink;
use zerodds_recorder_decode::type_source::{TypeBook, parse_topic_type_map};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::dynamic::codec::decode_dynamic;
use zerodds_types::dynamic::type_::DynamicType;

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

    let cmd = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            return ExitCode::from(2);
        }
    };

    match cmd {
        Command::Record(r) => run_record(r),
        Command::Info(i) => run_info(&i.file),
        Command::List(i) => run_list(&i.file),
    }
}

fn run_record(r: RecordArgs) -> ExitCode {
    if r.topics.is_empty() {
        eprintln!("error: at least one --topic is required for capture");
        return ExitCode::from(2);
    }

    // Output-Pfad festlegen.
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

    // DCPS-Runtime starten.
    let domain_id: i32 = match r.domain.try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: domain {} does not fit i32", r.domain);
            return ExitCode::from(2);
        }
    };
    let prefix = stable_prefix();
    let runtime = match DcpsRuntime::start(domain_id, prefix, RuntimeConfig::default()) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: DcpsRuntime::start failed: {e:?}");
            return ExitCode::from(3);
        }
    };

    // Type-following discovery settle: before freezing the .zddsrec header
    // (RecordingSession fixes its topic set at construction), wait briefly for
    // the requested topics' writers to be discovered so we learn each writer's
    // REAL type_name. A recorder announcing a generic `RawBytes` reader would
    // never match a typed writer (e.g. cuas::Track) and capture nothing — the
    // root cause of the "record validates and exits" report. A requested topic
    // with no writer in the window falls back to RawBytes (so a genuine RawBytes
    // writer still records) with a warning.
    let topics = resolve_topic_types(&runtime, &r.topics, Duration::from_secs(3));

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
    let _ = r.max_sample_bytes; // DoS-Cap wird in einer kuenftigen Iteration ueber Config-Hook erzwungen.

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
    let mut decode = match setup_decode(&r, &topics) {
        Ok(d) => d,
        Err(code) => return code,
    };

    // Shutdown-flag — SIGINT triggert ihn.
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

    // Drain-Loop: poll alle reader-channels mit kurzem timeout, schreibe
    // jedes Sample in die Session. Beendet bei stop-flag oder Deadline.
    while !stop.load(Ordering::Relaxed) {
        if let Some(end) = deadline {
            if Instant::now() >= end {
                break;
            }
        }
        let mut got_any = false;
        for (topic_idx, _eid, rx) in &readers {
            if let Ok(sample) = rx.recv_timeout(Duration::from_millis(20)) {
                got_any = true;
                let topic = &topics[*topic_idx];
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
                        session_for_loop.record_sample(
                            ts,
                            writer_guid,
                            topic,
                            SampleKind::Alive,
                            payload.to_vec(),
                        )
                    }
                    UserSample::Lifecycle { kind, .. } => {
                        let mapped = match kind {
                            zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed => {
                                SampleKind::NotAliveDisposed
                            }
                            _ => SampleKind::NotAliveUnregistered,
                        };
                        session_for_loop.record_sample(
                            unix_ns_now(),
                            part_guid,
                            topic,
                            mapped,
                            Vec::new(),
                        )
                    }
                };
                if let Err(e) = res {
                    eprintln!("warn: record_sample dropped: {e:?}");
                }
            }
        }
        if !got_any {
            std::thread::sleep(Duration::from_millis(5));
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
    drop(runtime);
    ExitCode::SUCCESS
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
}

impl DecodeSinks {
    fn disabled() -> Self {
        Self {
            json: None,
            sqlite: None,
            types: std::collections::HashMap::new(),
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
        let data = match decode_dynamic(&ty, payload, representation == 1, big_endian) {
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
fn setup_decode(r: &RecordArgs, topics: &[TopicKey]) -> Result<DecodeSinks, ExitCode> {
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
        Some(p) => match SqliteSink::open(p) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("error: cannot open sqlite {p}: {e:?}");
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
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

/// Resolve each requested topic to the real `type_name`(s) of its discovered
/// writers, polling SEDP for up to `settle`. Returns one [`TopicKey`] per
/// distinct `(topic, type)` pair (so a topic carrying two writer types yields
/// two keys → two readers). Requested topics with no writer in the window get a
/// `RawBytes` fallback (+ warning) so a genuine RawBytes writer still records.
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
                "warn: no writer discovered for topic '{t}' within the settle window — \
                 falling back to {RAW_BYTES_TYPE_NAME} (records only a RawBytes writer)"
            );
            found.push(TopicKey {
                topic: t.clone(),
                type_name: RAW_BYTES_TYPE_NAME.to_string(),
            });
        }
    }
    found
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

fn stable_prefix() -> GuidPrefix {
    let mut bytes = [0u8; 12];
    let pid = std::process::id();
    bytes[0..4].copy_from_slice(&pid.to_le_bytes());
    let host = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    bytes[4..8].copy_from_slice(&host.to_le_bytes());
    bytes[8] = 0xFE; // marker: "zerodds-record"
    GuidPrefix::from_bytes(bytes)
}

fn participant_guid(prefix: GuidPrefix) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&prefix.0);
    g[12..15].copy_from_slice(&[0, 0, 0]);
    g[15] = 0xC1; // ENTITYID_PARTICIPANT
    g
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
        // SAFETY: libc::signal nimmt einen C-ABI-Funktionspointer; `handler`
        // ist `extern "C"` und passt auf die libc-Signatur.
        unsafe {
            libc::signal(libc::SIGINT, handler as usize);
            libc::signal(libc::SIGTERM, handler as usize);
        }
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: libc::signal-Aufruf wie unter Linux; `handler` ist
        // `extern "C"` und ABI-kompatibel.
        unsafe {
            libc::signal(libc::SIGINT, handler as usize);
            libc::signal(libc::SIGTERM, handler as usize);
        }
    }
}

#[cfg(not(unix))]
fn install_signal_handler(_stop: Arc<AtomicBool>) {
    // Windows: kein graceful CTRL+C-Pfad ohne weitere Deps.
    // User stoppt mit Task-Kill; --duration ist alternativer Stop.
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
               --out-sqlite <FILE>      Write decoded samples to per-topic SQLite\n\
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
}
