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
//! * One participant per bridge process (zerodds_runtime_create).

#![warn(missing_docs)]
#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr)]

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zerodds_recorder::{ParticipantEntry, RecordingSession, SampleKind, SessionOptions, TopicKey};

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

/// Helper wrapper so the `unsafe` block pattern stays clean per call
/// gets its own `// SAFETY:` comment (zerodds-lint requirement).
///
/// # Safety
/// `rt`, `topic`, `type_name` must be NUL-terminated + valid.
unsafe fn create_reader(
    rt: *mut zerodds::ZeroDdsRuntime,
    topic: &std::ffi::CString,
    type_name: &std::ffi::CString,
) -> *mut zerodds::ZeroDdsReader {
    // SAFETY: caller contract from doc block.
    unsafe { zerodds::zerodds_reader_create(rt, topic.as_ptr(), type_name.as_ptr(), 1) }
}

fn now_unix_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
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
    // Open the output file.
    let file = match File::create(&args.output) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("create {:?}: {e}", args.output);
            return ExitCode::from(1);
        }
    };
    let time_base = now_unix_ns();
    let opts = SessionOptions {
        time_base_unix_ns: time_base,
        // Phase-C: participant is the bridge itself — we track
        // it as guid=zeros with name "recorder-bridge". Real
        // per-sample source GUID needs a DcpsRuntime built-in
        // Topic-Hook (Phase-D).
        participants: vec![ParticipantEntry {
            guid: [0u8; 16],
            name: "recorder-bridge".into(),
        }],
        topics: args.topics.clone(),
    };
    let session = Arc::new(RecordingSession::new(file, opts));
    let stop_flag = Arc::new(AtomicBool::new(false));
    // SIGINT handler — Phase C uses simple polling.
    let stop_for_signal = Arc::clone(&stop_flag);
    ctrl_c_install(move || {
        stop_for_signal.store(true, Ordering::Relaxed);
    });
    // SAFETY: zerodds_runtime_create returns a heap-allocated handle.
    let rt = unsafe { zerodds::zerodds_runtime_create(args.domain) };
    if rt.is_null() {
        eprintln!("zerodds_runtime_create failed");
        return ExitCode::from(1);
    }
    // Wait for peers, then create readers.
    // SAFETY: rt from create.
    unsafe {
        let _ = zerodds::zerodds_runtime_wait_for_peers(rt, 1, 5_000);
    }
    // One reader per topic.
    let mut readers: Vec<(*mut zerodds::ZeroDdsReader, TopicKey)> = Vec::new();
    for tk in &args.topics {
        let topic_c = std::ffi::CString::new(tk.topic.clone()).expect("static");
        let type_c = std::ffi::CString::new(tk.type_name.clone()).expect("static");
        // SAFETY: NUL-terminated C strings; rt from zerodds_runtime_create.
        let reader = unsafe { create_reader(rt, &topic_c, &type_c) };
        if reader.is_null() {
            eprintln!("reader_create failed for {}", tk.topic);
            // SAFETY: rt from create, cleanup.
            unsafe { zerodds::zerodds_runtime_destroy(rt) };
            return ExitCode::from(1);
        }
        readers.push((reader, tk.clone()));
    }
    println!("recording... (Ctrl-C to stop)");
    let start = Instant::now();
    let mut last_log = start;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            println!("recorder-bridge: SIGINT received, draining...");
            break;
        }
        if let Some(max) = args.duration {
            if start.elapsed() >= max {
                println!("recorder-bridge: duration reached");
                break;
            }
        }
        // Pro Reader take + write.
        let mut got_any = false;
        for (reader, tk) in &readers {
            let mut buf: *mut u8 = std::ptr::null_mut();
            let mut len: usize = 0;
            // SAFETY: reader from create.
            let rc = unsafe {
                zerodds::zerodds_reader_take(*reader, &mut buf, &mut len, std::ptr::null_mut())
            };
            if rc != 0 {
                continue;
            }
            if !buf.is_null() && len > 0 {
                // SAFETY: buf+len from take.
                let slice = unsafe { std::slice::from_raw_parts(buf, len) };
                let payload = slice.to_vec();
                // SAFETY: caller contract: free before next take.
                unsafe { zerodds::zerodds_buffer_free(buf, len) };
                let now_ns = now_unix_ns();
                if let Err(e) =
                    session.record_sample(now_ns, [0u8; 16], tk, SampleKind::Alive, payload)
                {
                    eprintln!("session error: {e}");
                }
                got_any = true;
            }
        }
        if !got_any {
            std::thread::sleep(Duration::from_millis(5));
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
    // Cleanup.
    for (reader, _) in readers {
        // SAFETY: reader from create.
        unsafe { zerodds::zerodds_reader_destroy(reader) };
    }
    // SAFETY: rt from create.
    unsafe { zerodds::zerodds_runtime_destroy(rt) };
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

/// Platform-independent SIGINT handler, in a background
/// Thread implementation. On Unix: signal(SIGINT, ...). On Windows:
/// console-control-handler. Phase-C uses a simple spawn loop
/// that polls on SIGINT — no libc crate dep needed.
fn ctrl_c_install<F: Fn() + Send + Sync + 'static>(handler: F) {
    let h = std::sync::Arc::new(handler);
    #[cfg(unix)]
    {
        let h2 = std::sync::Arc::clone(&h);
        std::thread::spawn(move || {
            // SAFETY: signal handling via raw libc FFI; SIGINT (=2)
            // is signal-safe if the handler only sets an atomic
            // variable — we set it via `handler()` which
            // guaranteed to only do AtomicBool::store (caller contract).
            unsafe extern "C" {
                fn signal(sig: i32, handler: extern "C" fn(i32)) -> *mut core::ffi::c_void;
            }
            // Local variable, because the C function needs a static
            // function pointer.
            type HandlerSlot = std::sync::Mutex<Option<Box<dyn Fn() + Send + Sync>>>;
            static GLOBAL_HANDLER: std::sync::OnceLock<HandlerSlot> = std::sync::OnceLock::new();
            let cell = GLOBAL_HANDLER.get_or_init(|| std::sync::Mutex::new(None));
            let h_inner = std::sync::Arc::clone(&h2);
            *cell.lock().expect("static") = Some(Box::new(move || {
                h_inner();
            }));
            extern "C" fn raw_handler(_sig: i32) {
                if let Some(cell) = GLOBAL_HANDLER.get() {
                    if let Ok(g) = cell.lock() {
                        if let Some(h) = g.as_ref() {
                            h();
                        }
                    }
                }
            }
            // SAFETY: signal() FFI-Call.
            unsafe {
                signal(2, raw_handler); // SIGINT
            }
        });
    }
    #[cfg(not(unix))]
    {
        // Phase-C stub on non-Unix: the caller must set --duration.
        let _ = h;
        eprintln!("(non-unix) SIGINT handler is Phase D; use --duration");
    }
}
