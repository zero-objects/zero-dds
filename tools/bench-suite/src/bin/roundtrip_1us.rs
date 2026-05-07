// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! roundtrip-1us — Sub-µs Latency-Bench fuer ZeroDDS.
//!
//! Crate `zerodds-bench-suite`. Safety classification: **COMFORT** (Bench-Tool,
//! kein Runtime-Pfad). Panics auf Argument-Fehlern erlaubt.
//!
//! # Architektur
//!
//! Zwei-Prozess-Modus, beide busy-poll auf UDP:
//!
//!   ping  ──[Data + t_send]──▶  pong
//!   ping  ◀─[Echo  + t_send]──  pong
//!   ping  Histogram += now − t_send
//!
//! Reine UDP-Transport-Roundtrip-Latenz, ohne RTPS-Encap und ohne
//! DCPS-Discovery — das ist bewusst die *unterste* messbare Schicht.
//! Spaetere Sprint-Erweiterung kann denselben Hot-Path durch
//! DCPS-Userspace pumpen, dann steht die Differenz fuer die
//! Stack-Overhead-Story zur Verfuegung.
//!
//! # Verwendung
//!
//! Pong-Prozess (CPU-pinned, RT-Prio empfohlen):
//!
//! ```ignore
//! taskset -c 2 chrt -f 80 \
//!   roundtrip-1us --role pong --addr 0.0.0.0:7400 --payload 64
//! ```
//!
//! Ping-Prozess auf der Gegenmaschine:
//!
//! ```ignore
//! taskset -c 3 chrt -f 80 \
//!   roundtrip-1us --role ping \
//!     --remote 192.0.2.10:7400 --bind 0.0.0.0:7401 \
//!     --payload 64 --warmup 5000 --samples 100000 \
//!     --hgrm out.hgrm
//! ```
//!
//! Output: stdout p50/p90/p99/p999/p9999/p99999 + min/max/n; optional
//! `--hgrm` schreibt das full HdrHistogram im `histlog`-V2-Text-Format,
//! kompatibel mit `HdrHistogramVisualizer`.
//!
//! # CI-Gate (D.5-Plan)
//!
//! Auf llvm-bare-metal mit Linux 6.x preempt_rt + isolcpu=2-7:
//!
//! - p99    < 5 µs
//! - p999   < 20 µs
//! - p9999  < 100 µs
//!
//! Der Bench-Runner-Wrapper (`tests/perf/llvm_bench_runner.sh` Step 5)
//! ruft das Binary mit `--ci-gate` und failt wenn die Schwellen
//! ueberschritten sind.

#![warn(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr
)]

use hdrhistogram::Histogram;
use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const HEADER_LEN: usize = 16; // 8 byte seq + 8 byte t_send_ns

/// CLI-Konfiguration.
struct Args {
    role: Role,
    bind: SocketAddr,
    remote: Option<SocketAddr>,
    payload: usize,
    warmup: u64,
    samples: u64,
    hgrm: Option<PathBuf>,
    rate_hz: Option<u32>,
    ci_gate: bool,
    max_runtime: Option<Duration>,
    /// Wenn aktiv: ueber DCPS-Runtime statt rohes UDP.
    use_dcps: bool,
    /// DCPS-Domain-ID (nur mit --use-dcps).
    dcps_domain: u32,
    /// DCPS-Topic (nur mit --use-dcps). Default eindeutig pro PID.
    dcps_topic: Option<String>,
    /// Listener-Callback statt Polling-Take.
    /// Pong: Echo direkt aus dem Recv-Thread schreiben.
    /// Ping: RTT direkt im Recv-Thread messen.
    /// Eliminiert die User-Polling-Latenz (~50-100 µs raus).
    listener: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Ping,
    Pong,
}

fn parse_args() -> Result<Args, String> {
    let mut role = None;
    let mut addr: Option<SocketAddr> = None;
    let mut bind: Option<SocketAddr> = None;
    let mut remote: Option<SocketAddr> = None;
    let mut payload: usize = 64;
    let mut warmup: u64 = 5_000;
    let mut samples: u64 = 100_000;
    let mut hgrm: Option<PathBuf> = None;
    let mut rate_hz: Option<u32> = None;
    let mut ci_gate = false;
    let mut max_runtime: Option<Duration> = None;
    let mut use_dcps = false;
    let mut dcps_domain: u32 = 200;
    let mut dcps_topic: Option<String> = None;
    let mut listener = false;

    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        let a = &argv[i];
        let val = || {
            argv.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {a}"))
        };
        match a.as_str() {
            "--role" => {
                role = Some(match val()?.as_str() {
                    "ping" => Role::Ping,
                    "pong" => Role::Pong,
                    other => return Err(format!("invalid role: {other}")),
                });
                i += 2;
            }
            "--addr" => {
                addr = Some(val()?.parse().map_err(|e| format!("addr: {e}"))?);
                i += 2;
            }
            "--bind" => {
                bind = Some(val()?.parse().map_err(|e| format!("bind: {e}"))?);
                i += 2;
            }
            "--remote" => {
                remote = Some(val()?.parse().map_err(|e| format!("remote: {e}"))?);
                i += 2;
            }
            "--payload" => {
                payload = val()?.parse().map_err(|e| format!("payload: {e}"))?;
                i += 2;
            }
            "--warmup" => {
                warmup = val()?.parse().map_err(|e| format!("warmup: {e}"))?;
                i += 2;
            }
            "--samples" => {
                samples = val()?.parse().map_err(|e| format!("samples: {e}"))?;
                i += 2;
            }
            "--hgrm" => {
                hgrm = Some(PathBuf::from(val()?));
                i += 2;
            }
            "--rate" => {
                rate_hz = Some(val()?.parse().map_err(|e| format!("rate: {e}"))?);
                i += 2;
            }
            "--max-runtime" => {
                let s: u64 = val()?.parse().map_err(|e| format!("max-runtime: {e}"))?;
                max_runtime = Some(Duration::from_secs(s));
                i += 2;
            }
            "--ci-gate" => {
                ci_gate = true;
                i += 1;
            }
            "--use-dcps" => {
                use_dcps = true;
                i += 1;
            }
            "--dcps-domain" => {
                dcps_domain = val()?.parse().map_err(|e| format!("dcps-domain: {e}"))?;
                i += 2;
            }
            "--dcps-topic" => {
                dcps_topic = Some(val()?);
                i += 2;
            }
            "--listener" => {
                listener = true;
                i += 1;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let role = role.ok_or("--role pong|ping required")?;
    // bind ist ohne --use-dcps Pflicht; mit --use-dcps optional (UDP-Frei).
    let bind = if use_dcps {
        bind.or(addr).unwrap_or_else(|| {
            // Platzhalter — wird im DCPS-Pfad nicht benutzt.
            "127.0.0.1:0".parse().expect("static")
        })
    } else {
        bind.or(addr).ok_or("--bind or --addr required")?
    };
    if payload < HEADER_LEN {
        return Err(format!("payload must be >= {HEADER_LEN} bytes (header)"));
    }
    if !use_dcps && role == Role::Ping && remote.is_none() {
        return Err("--remote required for UDP ping role".into());
    }

    Ok(Args {
        role,
        bind,
        remote,
        payload,
        warmup,
        samples,
        hgrm,
        rate_hz,
        ci_gate,
        max_runtime,
        use_dcps,
        dcps_domain,
        dcps_topic,
        listener,
    })
}

fn print_help() {
    println!(
        "roundtrip-1us — sub-µs UDP latency bench

USAGE:
  roundtrip-1us --role pong --bind 0.0.0.0:7400 [--payload N]
  roundtrip-1us --role ping --remote IP:PORT --bind 0.0.0.0:7401 \\
                [--payload N] [--warmup K] [--samples N] [--rate HZ] \\
                [--hgrm FILE] [--ci-gate] [--max-runtime SECS]

OPTIONS:
  --role pong|ping     pong = echo, ping = measurer
  --bind ADDR:PORT     local UDP bind (required)
  --remote ADDR:PORT   peer (ping only)
  --payload N          bytes per sample, >= 16 (header). default 64
  --warmup N           samples to discard before measuring. default 5000
  --samples N          samples to record. default 100000
  --rate HZ            optional rate-limit (default: free-running busy-poll)
  --hgrm FILE          write HdrHistogram histlog v2 text to FILE
  --ci-gate            enforce p99<5µs, p999<20µs, p9999<100µs (exit 1 on violation)
  --max-runtime SECS   abort after wallclock SECS (CI safety net)

DCPS-MODE (D.5b — durch ZeroDDS-Stack statt rohes UDP):
  --use-dcps           pump samples via zerodds-c-api/DcpsRuntime
  --dcps-domain N      DDS-Domain-ID (default 200)
  --dcps-topic NAME    Topic-Name (default: roundtrip-{{pid}})
"
    );
}

fn write_packet(buf: &mut [u8], seq: u64, t_send_ns: u64) {
    buf[0..8].copy_from_slice(&seq.to_le_bytes());
    buf[8..16].copy_from_slice(&t_send_ns.to_le_bytes());
}

fn read_seq_t(buf: &[u8]) -> Option<(u64, u64)> {
    if buf.len() < HEADER_LEN {
        return None;
    }
    let seq = u64::from_le_bytes(buf[0..8].try_into().ok()?);
    let ts = u64::from_le_bytes(buf[8..16].try_into().ok()?);
    Some((seq, ts))
}

// ============================================================================
// DCPS-Roundtrip (D.5b) — gleicher Bench durch zerodds-c-api.
// ============================================================================

/// Topic-Default basierend auf der PID, damit parallele Test-Runs sich
/// nicht stoeren.
fn dcps_topics(args: &Args) -> (String, String) {
    let suffix = args
        .dcps_topic
        .clone()
        .unwrap_or_else(|| format!("roundtrip-{}", std::process::id()));
    (format!("{suffix}/req"), format!("{suffix}/echo"))
}

fn run_pong_dcps(args: &Args) -> std::io::Result<()> {
    let (req_topic, echo_topic) = dcps_topics(args);
    let req_topic_c = std::ffi::CString::new(req_topic).expect("static");
    let echo_topic_c = std::ffi::CString::new(echo_topic).expect("static");
    let type_c = std::ffi::CString::new("RoundtripBytes").expect("static");
    // SAFETY: alle FFI-Calls mit korrekten C-Strings; Handles werden
    // bei Fehler verworfen, bei Success destroyed.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        let rt = zerodds::zerodds_runtime_create(args.dcps_domain);
        if rt.is_null() {
            return Err(std::io::Error::other("dcps runtime create failed"));
        }
        // Erst auf Peer warten, dann Endpoints — sonst leerer SPDP-Cache.
        let _ = zerodds::zerodds_runtime_wait_for_peers(rt, 1, 5_000);
        let writer = zerodds::zerodds_writer_create(
            rt,
            echo_topic_c.as_ptr(),
            type_c.as_ptr(),
            0, // best-effort fuer Latenz
        );
        let reader = zerodds::zerodds_reader_create(rt, req_topic_c.as_ptr(), type_c.as_ptr(), 0);
        if writer.is_null() || reader.is_null() {
            zerodds::zerodds_runtime_destroy(rt);
            return Err(std::io::Error::other("dcps endpoint create failed"));
        }
        let _ = zerodds::zerodds_writer_wait_for_matched(writer, 1, 5_000);
        let _ = zerodds::zerodds_reader_wait_for_matched(reader, 1, 5_000);
        println!(
            "pong: dcps domain={} req<-{} echo<-{}",
            args.dcps_domain,
            req_topic_c.to_string_lossy(),
            echo_topic_c.to_string_lossy()
        );
        let start = Instant::now();
        loop {
            if let Some(max) = args.max_runtime {
                if start.elapsed() >= max {
                    break;
                }
            }
            let mut buf: *mut u8 = std::ptr::null_mut();
            let mut len: usize = 0;
            let rc = zerodds::zerodds_reader_take(reader, &mut buf, &mut len);
            if rc != 0 {
                continue;
            }
            if !buf.is_null() && len > 0 {
                // Sample echoen.
                let _ = zerodds::zerodds_writer_write(writer, buf, len);
                zerodds::zerodds_buffer_free(buf, len);
            } else {
                std::hint::spin_loop();
            }
        }
        zerodds::zerodds_writer_destroy(writer);
        zerodds::zerodds_reader_destroy(reader);
        zerodds::zerodds_runtime_destroy(rt);
    }
    Ok(())
}

fn run_ping_dcps(args: &Args) -> std::io::Result<Stats> {
    let (req_topic, echo_topic) = dcps_topics(args);
    let req_topic_c = std::ffi::CString::new(req_topic).expect("static");
    let echo_topic_c = std::ffi::CString::new(echo_topic).expect("static");
    let type_c = std::ffi::CString::new("RoundtripBytes").expect("static");
    println!(
        "ping: dcps domain={} req->{} echo->{} payload={} warmup={} samples={}",
        args.dcps_domain,
        req_topic_c.to_string_lossy(),
        echo_topic_c.to_string_lossy(),
        args.payload,
        args.warmup,
        args.samples
    );
    let mut hist: Histogram<u64> =
        Histogram::new_with_bounds(1, 10_000_000_000, 3).expect("histogram bounds valid");
    let total = args.warmup + args.samples;
    let t_origin = Instant::now();
    let rate_period_ns: Option<u64> = args.rate_hz.map(|hz| 1_000_000_000u64 / u64::from(hz));
    let mut next_send_ns: u64 = 0;
    let mut seq: u64 = 0;
    let mut received: u64 = 0;
    let start = Instant::now();
    let mut payload = vec![0u8; args.payload];

    // SAFETY: gesamter DCPS-Block; Handles werden am Ende sauber
    // destroyed.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        let rt = zerodds::zerodds_runtime_create(args.dcps_domain);
        if rt.is_null() {
            return Err(std::io::Error::other("dcps runtime create failed"));
        }
        let _ = zerodds::zerodds_runtime_wait_for_peers(rt, 1, 5_000);
        let writer = zerodds::zerodds_writer_create(rt, req_topic_c.as_ptr(), type_c.as_ptr(), 0);
        let reader = zerodds::zerodds_reader_create(rt, echo_topic_c.as_ptr(), type_c.as_ptr(), 0);
        if writer.is_null() || reader.is_null() {
            zerodds::zerodds_runtime_destroy(rt);
            return Err(std::io::Error::other("dcps endpoint create failed"));
        }
        let _ = zerodds::zerodds_writer_wait_for_matched(writer, 1, 5_000);
        let _ = zerodds::zerodds_reader_wait_for_matched(reader, 1, 5_000);
        // Stabilisierungs-Pause: pong braucht Zeit, seine Reader-/
        // Writer-Wires intern fertig zu wirten BEVOR der Bench startet.
        // Ohne diesen Sleep kommen die ersten Samples mit Sekunden-RTT
        // an (Queue-Drain-Effekt).
        std::thread::sleep(Duration::from_millis(500));
        // Eingangsqueue droppen — alles was vor dem Bench-Start
        // ankam ist Discovery-/Heartbeat-Garbage.
        loop {
            let mut b: *mut u8 = std::ptr::null_mut();
            let mut l: usize = 0;
            let rc = zerodds::zerodds_reader_take(reader, &mut b, &mut l);
            if rc != 0 || b.is_null() || l == 0 {
                break;
            }
            zerodds::zerodds_buffer_free(b, l);
        }
        println!("ping: matched (both directions), bench start");

        while received < total {
            if let Some(max) = args.max_runtime {
                if start.elapsed() >= max {
                    eprintln!("ping: max-runtime reached after {received}/{total}");
                    break;
                }
            }
            let should_send = match rate_period_ns {
                None => seq == received,
                Some(_period) => {
                    let now_ns = t_origin.elapsed().as_nanos() as u64;
                    now_ns >= next_send_ns && (seq - received) < 1024
                }
            };
            if should_send && seq < total {
                let now_ns = t_origin.elapsed().as_nanos() as u64;
                write_packet(&mut payload, seq, now_ns);
                let rc = zerodds::zerodds_writer_write(writer, payload.as_ptr(), payload.len());
                if rc != 0 {
                    eprintln!("ping: write rc={rc}");
                    break;
                }
                seq += 1;
                if let Some(period) = rate_period_ns {
                    next_send_ns = next_send_ns.max(now_ns) + period;
                }
            }
            let mut rxbuf: *mut u8 = std::ptr::null_mut();
            let mut rxlen: usize = 0;
            let rc = zerodds::zerodds_reader_take(reader, &mut rxbuf, &mut rxlen);
            if rc != 0 {
                continue;
            }
            if !rxbuf.is_null() && rxlen > 0 {
                let slice = std::slice::from_raw_parts(rxbuf, rxlen);
                if let Some((_seq_back, t_send_ns)) = read_seq_t(slice) {
                    let now_ns = t_origin.elapsed().as_nanos() as u64;
                    let rtt = now_ns.saturating_sub(t_send_ns);
                    if received >= args.warmup {
                        let _ = hist.record(rtt.max(1));
                    }
                    received += 1;
                }
                zerodds::zerodds_buffer_free(rxbuf, rxlen);
            } else {
                std::hint::spin_loop();
            }
        }

        zerodds::zerodds_writer_destroy(writer);
        zerodds::zerodds_reader_destroy(reader);
        zerodds::zerodds_runtime_destroy(rt);
    }
    Ok(Stats::from_hist(&hist).maybe_dump_hgrm(&hist, args.hgrm.as_ref()))
}

// ============================================================================
// DCPS-Roundtrip Listener-Variante — Recv-driven via
// `zerodds_reader_set_data_callback`. Eliminiert das User-Polling in
// `zerodds_reader_take()`.
// ============================================================================

/// Pong-Listener-Variante: Echo direkt aus dem Recv-Thread schreiben.
/// Hauptthread schlaeft bis max-runtime.
fn run_pong_dcps_listener(args: &Args) -> std::io::Result<()> {
    let (req_topic, echo_topic) = dcps_topics(args);
    let req_topic_c = std::ffi::CString::new(req_topic).expect("static");
    let echo_topic_c = std::ffi::CString::new(echo_topic).expect("static");
    let type_c = std::ffi::CString::new("RoundtripBytes").expect("static");
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        let rt = zerodds::zerodds_runtime_create(args.dcps_domain);
        if rt.is_null() {
            return Err(std::io::Error::other("dcps runtime create failed"));
        }
        let _ = zerodds::zerodds_runtime_wait_for_peers(rt, 1, 5_000);
        let writer = zerodds::zerodds_writer_create(rt, echo_topic_c.as_ptr(), type_c.as_ptr(), 0);
        let reader = zerodds::zerodds_reader_create(rt, req_topic_c.as_ptr(), type_c.as_ptr(), 0);
        if writer.is_null() || reader.is_null() {
            zerodds::zerodds_runtime_destroy(rt);
            return Err(std::io::Error::other("dcps endpoint create failed"));
        }
        let _ = zerodds::zerodds_writer_wait_for_matched(writer, 1, 5_000);
        let _ = zerodds::zerodds_reader_wait_for_matched(reader, 1, 5_000);
        println!(
            "pong[listener]: dcps domain={} req<-{} echo<-{}",
            args.dcps_domain,
            req_topic_c.to_string_lossy(),
            echo_topic_c.to_string_lossy()
        );

        // Writer-Pointer als usize in einer Box puffern, dem Listener
        // ueber `user_data` reichen. Der Listener feuert das Echo
        // direkt — derselbe Recv-Thread, kein User-Polling.
        let writer_box: Box<usize> = Box::new(writer as usize);
        let user_data = Box::into_raw(writer_box) as *mut core::ffi::c_void;

        extern "C" fn pong_cb(user_data: *mut core::ffi::c_void, payload: *const u8, len: usize) {
            // SAFETY: writer_addr lebt bis pong_cb deregistriert ist.
            let writer_addr = unsafe { *(user_data as *const usize) };
            let writer = writer_addr as *mut zerodds::ZeroDdsWriter;
            // SAFETY: writer ist gueltig bis Cleanup, payload kommt vom
            // Runtime-Recv-Thread mit gueltigem Slice.
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe {
                let _ = zerodds::zerodds_writer_write(writer, payload, len);
            }
        }
        let rc = zerodds::zerodds_reader_set_data_callback(reader, Some(pong_cb), user_data);
        if rc != 0 {
            zerodds::zerodds_writer_destroy(writer);
            zerodds::zerodds_reader_destroy(reader);
            zerodds::zerodds_runtime_destroy(rt);
            let _ = Box::from_raw(user_data as *mut usize);
            return Err(std::io::Error::other("set_data_callback failed"));
        }

        // Hauptthread schlaeft bis max-runtime.
        let start = Instant::now();
        let until = start + args.max_runtime.unwrap_or(Duration::from_secs(60));
        loop {
            let now = Instant::now();
            if now >= until {
                break;
            }
            std::thread::sleep(Duration::from_millis(50).min(until - now));
        }

        // Listener loeschen BEVOR der Reader/Writer destroyed wird —
        // sonst kann der Recv-Thread mit Dangling-Writer-Pointer
        // weiterfeuern.
        zerodds::zerodds_reader_set_data_callback(reader, None, std::ptr::null_mut());
        let _ = Box::from_raw(user_data as *mut usize);

        zerodds::zerodds_writer_destroy(writer);
        zerodds::zerodds_reader_destroy(reader);
        zerodds::zerodds_runtime_destroy(rt);
    }
    Ok(())
}

/// Ping-Listener-Variante: RTT direkt im Recv-Thread messen, ueber
/// Mutex<Histogram> + Condvar mit dem Hauptthread synchronisieren.
/// Hauptthread schreibt sequenziell (1 in-flight), Listener feuert
/// Histogramm-Update + signalisiert.
fn run_ping_dcps_listener(args: &Args) -> std::io::Result<Stats> {
    use std::sync::{Arc, Condvar, Mutex};

    let (req_topic, echo_topic) = dcps_topics(args);
    let req_topic_c = std::ffi::CString::new(req_topic).expect("static");
    let echo_topic_c = std::ffi::CString::new(echo_topic).expect("static");
    let type_c = std::ffi::CString::new("RoundtripBytes").expect("static");
    println!(
        "ping[listener]: dcps domain={} req->{} echo->{} payload={} warmup={} samples={}",
        args.dcps_domain,
        req_topic_c.to_string_lossy(),
        echo_topic_c.to_string_lossy(),
        args.payload,
        args.warmup,
        args.samples
    );

    // Shared state zwischen Hauptthread und Listener-Recv-Thread.
    struct PingState {
        hist: Mutex<Histogram<u64>>,
        received: Mutex<u64>,
        cvar: Condvar,
        warmup: u64,
        t_origin: Instant,
    }
    let state = Arc::new(PingState {
        hist: Mutex::new(
            Histogram::<u64>::new_with_bounds(1, 10_000_000_000, 3)
                .expect("histogram bounds valid"),
        ),
        received: Mutex::new(0),
        cvar: Condvar::new(),
        warmup: args.warmup,
        t_origin: Instant::now(),
    });

    let total = args.warmup + args.samples;
    let start = Instant::now();
    let mut payload = vec![0u8; args.payload];
    let rate_period_ns: Option<u64> = args.rate_hz.map(|hz| 1_000_000_000u64 / u64::from(hz));
    let mut next_send_ns: u64 = 0;
    let mut seq: u64 = 0;

    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        let rt = zerodds::zerodds_runtime_create(args.dcps_domain);
        if rt.is_null() {
            return Err(std::io::Error::other("dcps runtime create failed"));
        }
        let _ = zerodds::zerodds_runtime_wait_for_peers(rt, 1, 5_000);
        let writer = zerodds::zerodds_writer_create(rt, req_topic_c.as_ptr(), type_c.as_ptr(), 0);
        let reader = zerodds::zerodds_reader_create(rt, echo_topic_c.as_ptr(), type_c.as_ptr(), 0);
        if writer.is_null() || reader.is_null() {
            zerodds::zerodds_runtime_destroy(rt);
            return Err(std::io::Error::other("dcps endpoint create failed"));
        }
        let _ = zerodds::zerodds_writer_wait_for_matched(writer, 1, 5_000);
        let _ = zerodds::zerodds_reader_wait_for_matched(reader, 1, 5_000);

        // Stabilisierungs-Pause + Drain, gleich wie Polling-Variante.
        std::thread::sleep(Duration::from_millis(500));
        loop {
            let mut b: *mut u8 = std::ptr::null_mut();
            let mut l: usize = 0;
            let rc = zerodds::zerodds_reader_take(reader, &mut b, &mut l);
            if rc != 0 || b.is_null() || l == 0 {
                break;
            }
            zerodds::zerodds_buffer_free(b, l);
        }
        println!("ping[listener]: matched (both directions), bench start");

        // Listener registrieren.
        let state_for_cb = Arc::into_raw(Arc::clone(&state)) as *mut core::ffi::c_void;

        extern "C" fn ping_cb(user_data: *mut core::ffi::c_void, payload: *const u8, len: usize) {
            // SAFETY: state lebt bis Listener deregistriert + Arc::from_raw cleanup.
            let state = unsafe { &*(user_data as *const PingState) };
            // SAFETY: payload + len kommen vom Runtime-Recv-Thread.
            let slice = unsafe { core::slice::from_raw_parts(payload, len) };
            if slice.len() < HEADER_LEN {
                return;
            }
            let t_send_ns = u64::from_le_bytes(slice[8..16].try_into().unwrap_or([0u8; 8]));
            let now_ns = state.t_origin.elapsed().as_nanos() as u64;
            let rtt = now_ns.saturating_sub(t_send_ns).max(1);
            let mut received = state.received.lock().expect("ping rx lock");
            if *received >= state.warmup {
                if let Ok(mut h) = state.hist.lock() {
                    let _ = h.record(rtt);
                }
            }
            *received += 1;
            state.cvar.notify_all();
        }

        let rc = zerodds::zerodds_reader_set_data_callback(reader, Some(ping_cb), state_for_cb);
        if rc != 0 {
            // state_for_cb retten:
            let _ = Arc::from_raw(state_for_cb as *const PingState);
            zerodds::zerodds_writer_destroy(writer);
            zerodds::zerodds_reader_destroy(reader);
            zerodds::zerodds_runtime_destroy(rt);
            return Err(std::io::Error::other("set_data_callback failed"));
        }

        // Hauptthread: senden + auf received-Counter warten.
        loop {
            // Abbruch-Bedingungen.
            {
                let received = state.received.lock().expect("rx lock");
                if *received >= total {
                    break;
                }
            }
            if let Some(max) = args.max_runtime {
                if start.elapsed() >= max {
                    let received = state.received.lock().expect("rx lock");
                    eprintln!(
                        "ping[listener]: max-runtime reached after {}/{}",
                        *received, total
                    );
                    break;
                }
            }

            // Send-Decision: nur wenn der Vorgaenger acknowledged ist
            // (1 in-flight). Auf Rate gehen wir hier nicht ein —
            // Listener-Bench ist primaer Latenz-orientiert.
            let received_now = *state.received.lock().expect("rx lock");
            let should_send = match rate_period_ns {
                None => seq == received_now,
                Some(_) => {
                    let now_ns = state.t_origin.elapsed().as_nanos() as u64;
                    now_ns >= next_send_ns && (seq - received_now) < 1024
                }
            };
            if should_send && seq < total {
                let now_ns = state.t_origin.elapsed().as_nanos() as u64;
                write_packet(&mut payload, seq, now_ns);
                let rc = zerodds::zerodds_writer_write(writer, payload.as_ptr(), payload.len());
                if rc != 0 {
                    eprintln!("ping[listener]: write rc={rc}");
                    break;
                }
                seq += 1;
                if let Some(period) = rate_period_ns {
                    next_send_ns = next_send_ns.max(now_ns) + period;
                }
            } else {
                // Auf naechsten Echo warten — kurz im Condvar parken
                // statt CPU zu pegasten.
                let received = state.received.lock().expect("rx lock");
                let _ = state
                    .cvar
                    .wait_timeout(received, Duration::from_millis(1))
                    .expect("cvar");
            }
        }

        // Listener vor Cleanup loeschen + Arc rueckholen.
        zerodds::zerodds_reader_set_data_callback(reader, None, std::ptr::null_mut());
        let _state_arc = Arc::from_raw(state_for_cb as *const PingState);

        zerodds::zerodds_writer_destroy(writer);
        zerodds::zerodds_reader_destroy(reader);
        zerodds::zerodds_runtime_destroy(rt);
    }

    let hist = state.hist.lock().expect("hist lock").clone();
    Ok(Stats::from_hist(&hist).maybe_dump_hgrm(&hist, args.hgrm.as_ref()))
}

fn run_pong(args: &Args) -> std::io::Result<()> {
    let sock = UdpSocket::bind(args.bind)?;
    sock.set_nonblocking(true)?;
    println!("pong: listening on {} (busy-poll)", args.bind);

    let mut buf = vec![0u8; args.payload.max(HEADER_LEN) + 64];
    let start = Instant::now();
    loop {
        if let Some(max) = args.max_runtime {
            if start.elapsed() >= max {
                println!("pong: max-runtime reached");
                return Ok(());
            }
        }
        match sock.recv_from(&mut buf) {
            Ok((n, peer)) => {
                let _ = sock.send_to(&buf[..n], peer);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::hint::spin_loop();
            }
            Err(e) => return Err(e),
        }
    }
}

/// Latenz-Statistiken aus einem Histogramm.
struct Stats {
    n: u64,
    min_ns: u64,
    max_ns: u64,
    p50_ns: u64,
    p90_ns: u64,
    p99_ns: u64,
    p999_ns: u64,
    p9999_ns: u64,
    p99999_ns: u64,
}

impl Stats {
    fn from_hist(h: &Histogram<u64>) -> Self {
        Self {
            n: h.len(),
            min_ns: h.min(),
            max_ns: h.max(),
            p50_ns: h.value_at_quantile(0.50),
            p90_ns: h.value_at_quantile(0.90),
            p99_ns: h.value_at_quantile(0.99),
            p999_ns: h.value_at_quantile(0.999),
            p9999_ns: h.value_at_quantile(0.9999),
            p99999_ns: h.value_at_quantile(0.99999),
        }
    }
}

fn run_ping(args: &Args) -> std::io::Result<Stats> {
    let sock = UdpSocket::bind(args.bind)?;
    sock.set_nonblocking(true)?;
    let remote = args.remote.expect("checked in parse");
    println!(
        "ping: bind={} remote={} payload={} warmup={} samples={}",
        args.bind, remote, args.payload, args.warmup, args.samples
    );

    let mut buf = vec![0u8; args.payload];
    let mut rx = vec![0u8; args.payload + 64];
    // 1ns granularity, max 10s — sigfig=3 gives ~1% precision.
    let mut hist: Histogram<u64> =
        Histogram::new_with_bounds(1, 10_000_000_000, 3).expect("histogram bounds valid");

    let total = args.warmup + args.samples;
    let t_origin = Instant::now();
    let rate_period_ns: Option<u64> = args.rate_hz.map(|hz| 1_000_000_000u64 / u64::from(hz));
    let mut next_send_ns: u64 = 0;
    let mut seq: u64 = 0;
    let mut received: u64 = 0;
    let start = Instant::now();

    while received < total {
        if let Some(max) = args.max_runtime {
            if start.elapsed() >= max {
                eprintln!("ping: max-runtime reached after {received}/{total}");
                break;
            }
        }
        // --- Send-Side: rate-limited oder free-running ---
        let should_send = match rate_period_ns {
            None => seq == received, // strict ping-pong: only send if previous returned
            Some(_period) => {
                let now_ns = t_origin.elapsed().as_nanos() as u64;
                now_ns >= next_send_ns && (seq - received) < 1024
            }
        };
        if should_send && seq < total {
            let now_ns = t_origin.elapsed().as_nanos() as u64;
            write_packet(&mut buf, seq, now_ns);
            sock.send_to(&buf, remote)?;
            seq += 1;
            if let Some(period) = rate_period_ns {
                next_send_ns = next_send_ns.max(now_ns) + period;
            }
        }

        // --- Recv-Side ---
        match sock.recv_from(&mut rx) {
            Ok((n, _)) => {
                if let Some((_seq_back, t_send_ns)) = read_seq_t(&rx[..n]) {
                    let now_ns = t_origin.elapsed().as_nanos() as u64;
                    let rtt = now_ns.saturating_sub(t_send_ns);
                    if received >= args.warmup {
                        // record() rejects 0; minimum rtt 1 ns.
                        let _ = hist.record(rtt.max(1));
                    }
                    received += 1;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::hint::spin_loop();
            }
            Err(e) => return Err(e),
        }
    }

    Ok(Stats::from_hist(&hist).maybe_dump_hgrm(&hist, args.hgrm.as_ref()))
}

impl Stats {
    fn maybe_dump_hgrm(self, h: &Histogram<u64>, path: Option<&PathBuf>) -> Self {
        if let Some(p) = path {
            if let Err(e) = dump_hgrm(h, p) {
                eprintln!("warning: failed to write hgrm to {p:?}: {e}");
            }
        }
        self
    }
}

/// Schreibt Histogramm im histlog-V2-Text-Format (HdrHistogramVisualizer-
/// kompatibel). Format: `Tag Value,Percentile,Count,1/(1-Percentile)`.
fn dump_hgrm(h: &Histogram<u64>, path: &PathBuf) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "#[HdrHistogram-V2 text]")?;
    writeln!(f, "Value,Percentile,TotalCount")?;
    for v in h.iter_recorded() {
        writeln!(
            f,
            "{},{:.6},{}",
            v.value_iterated_to(),
            v.percentile() / 100.0,
            v.count_at_value()
        )?;
    }
    writeln!(
        f,
        "#[Mean    = {:.3}, StdDeviation   = {:.3}]",
        h.mean(),
        h.stdev()
    )?;
    writeln!(f, "#[Max     = {}, Total count    = {}]", h.max(), h.len())?;
    Ok(())
}

fn report(stats: &Stats) {
    println!("--- ZeroDDS roundtrip-1us results ---");
    println!("n          = {}", stats.n);
    println!("min        = {} ns", stats.min_ns);
    println!("p50        = {} ns", stats.p50_ns);
    println!("p90        = {} ns", stats.p90_ns);
    println!("p99        = {} ns", stats.p99_ns);
    println!("p99.9      = {} ns", stats.p999_ns);
    println!("p99.99     = {} ns", stats.p9999_ns);
    println!("p99.999    = {} ns", stats.p99999_ns);
    println!("max        = {} ns", stats.max_ns);
}

fn enforce_ci_gate(stats: &Stats) -> bool {
    let mut ok = true;
    if stats.p99_ns >= 5_000 {
        eprintln!("CI-GATE FAIL: p99 = {} ns >= 5_000", stats.p99_ns);
        ok = false;
    }
    if stats.p999_ns >= 20_000 {
        eprintln!("CI-GATE FAIL: p999 = {} ns >= 20_000", stats.p999_ns);
        ok = false;
    }
    if stats.p9999_ns >= 100_000 {
        eprintln!("CI-GATE FAIL: p9999 = {} ns >= 100_000", stats.p9999_ns);
        ok = false;
    }
    ok
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
    match (args.role, args.use_dcps) {
        (Role::Pong, false) => {
            if let Err(e) = run_pong(&args) {
                eprintln!("pong error: {e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        (Role::Pong, true) => {
            let res = if args.listener {
                run_pong_dcps_listener(&args)
            } else {
                run_pong_dcps(&args)
            };
            if let Err(e) = res {
                eprintln!("pong-dcps error: {e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        (Role::Ping, false) => match run_ping(&args) {
            Ok(stats) => {
                report(&stats);
                if args.ci_gate && !enforce_ci_gate(&stats) {
                    return ExitCode::from(1);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("ping error: {e}");
                ExitCode::from(1)
            }
        },
        (Role::Ping, true) => {
            let res = if args.listener {
                run_ping_dcps_listener(&args)
            } else {
                run_ping_dcps(&args)
            };
            match res {
                Ok(stats) => {
                    report(&stats);
                    if args.ci_gate && !enforce_ci_gate(&stats) {
                        return ExitCode::from(1);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("ping-dcps error: {e}");
                    ExitCode::from(1)
                }
            }
        }
    }
}
