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
use zerodds_rtps::wire_types::GuidPrefix;

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
    let out_path = r.output.unwrap_or_else(|| {
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

    // Recording-Session vorbereiten. Der Header wird beim ersten
    // record_sample-Call lazy geschrieben — wir geben Topics +
    // Participants in den SessionOptions vor.
    let topics: Vec<TopicKey> = r
        .topics
        .iter()
        .map(|name| TopicKey {
            topic: name.clone(),
            type_name: "zerodds::RawBytes".to_string(),
        })
        .collect();
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

    // Pro Topic einen User-Reader registrieren.
    let mut readers = Vec::with_capacity(topics.len());
    for (idx, t) in topics.iter().enumerate() {
        let cfg = user_reader_config(&t.topic);
        match runtime.register_user_reader(cfg) {
            Ok((eid, rx)) => readers.push((idx, eid, rx)),
            Err(e) => {
                eprintln!("error: register_user_reader({}): {e:?}", t.topic);
                return ExitCode::from(3);
            }
        }
    }

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
                        ..
                    } => session_for_loop.record_sample(
                        unix_ns_now(),
                        writer_guid,
                        topic,
                        SampleKind::Alive,
                        payload.to_vec(),
                    ),
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

fn user_reader_config(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: "zerodds::RawBytes".to_string(),
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
