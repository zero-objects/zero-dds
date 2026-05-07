// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! roundtrip-typed — Apples-to-apples Roundtrip-Latency-Bench durch
//! die volle ZeroDDS DCPS-Pipeline mit typed XCDR2-Encode/Decode.
//!
//! Pendant zu `cyclone_app.cpp` / `rti_app.cpp` / `fastdds_app.cpp` —
//! gleicher Pattern (ping/pong, 1-in-flight, reliable + KeepLast(64)),
//! gleiche Payload-Sizes (32/64/256/1024/4096), gleicher Sample-Count
//! (5000), gleicher Transport (UDP-Loopback).
//!
//! Im Gegensatz zu `roundtrip-1us` (das `zerodds_writer_write()` mit
//! pre-encoded raw bytes aufruft und damit die CDR-Schicht
//! ueberspringt) durchlaeuft dieser Bench den vollen typed-Pfad:
//!
//!   App-Code (Rust)                  -- typed `Roundtrip`-struct
//!   ↓
//!   `DdsType::encode`/`decode`       -- XCDR2-LE @final encoding
//!   ↓
//!   `DataWriter<Roundtrip>::write`   -- HistoryCache, QoS
//!   ↓
//!   RTPS 2.5                         -- Wire-Protokoll
//!   ↓
//!   UDP-Loopback                     -- Transport
//!
//! # Verwendung
//!
//! ```ignore
//! taskset -c 4 chrt -f 80 \
//!   roundtrip-typed --role pong --runtime-secs 30
//! taskset -c 5 chrt -f 80 \
//!   roundtrip-typed --role ping --payload 64 --warmup 200 --samples 5000
//! ```

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use hdrhistogram::Histogram;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};
use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DecodeError, DomainParticipantFactory,
    DomainParticipantQos, EncodeError, Extensibility, PublisherQos, SubscriberQos, TopicQos,
};

// -----------------------------------------------------------------------------
// Typed user struct — entspricht roundtrip.idl (RoundtripBench::Roundtrip).
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Roundtrip {
    sequence_id: u32,
    t_send_ns: u64,
    payload: Vec<u8>,
}

/// CDR-Encapsulation-Header fuer PLAIN_CDR2-LE (XTypes 1.3 §7.6.3.2.1).
const ENCAP_HEADER_PLAIN_CDR2_LE: [u8; 4] = [0x00, 0x07, 0x00, 0x00];

impl DdsType for Roundtrip {
    const TYPE_NAME: &'static str = "RoundtripBench::Roundtrip";
    const EXTENSIBILITY: Extensibility = Extensibility::Final;
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&ENCAP_HEADER_PLAIN_CDR2_LE);
        let mut w = BufferWriter::new(Endianness::Little);
        // @final: tight body, kein DHEADER (XTypes §7.4.5).
        self.sequence_id
            .encode(&mut w)
            .map_err(|_| EncodeError::Invalid {
                what: "sequence_id encode",
            })?;
        self.t_send_ns
            .encode(&mut w)
            .map_err(|_| EncodeError::Invalid {
                what: "t_send_ns encode",
            })?;
        self.payload
            .encode(&mut w)
            .map_err(|_| EncodeError::Invalid {
                what: "payload encode",
            })?;
        out.extend_from_slice(w.as_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid {
                what: "encap header missing",
            });
        }
        let mut r = BufferReader::new(&bytes[4..], Endianness::Little);
        let sequence_id = u32::decode(&mut r).map_err(|_| DecodeError::Invalid {
            what: "sequence_id decode",
        })?;
        let t_send_ns = u64::decode(&mut r).map_err(|_| DecodeError::Invalid {
            what: "t_send_ns decode",
        })?;
        let payload = Vec::<u8>::decode(&mut r).map_err(|_| DecodeError::Invalid {
            what: "payload decode",
        })?;
        Ok(Roundtrip {
            sequence_id,
            t_send_ns,
            payload,
        })
    }
}

// -----------------------------------------------------------------------------
// CLI
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Ping,
    Pong,
}

struct Args {
    role: Role,
    domain: i32,
    payload: usize,
    warmup: u64,
    samples: u64,
    runtime_secs: u64,
}

fn parse_args() -> Result<Args, String> {
    let mut role: Option<Role> = None;
    let mut domain: i32 = 200;
    let mut payload: usize = 64;
    let mut warmup: u64 = 200;
    let mut samples: u64 = 5000;
    let mut runtime_secs: u64 = 30;

    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--role" => {
                i += 1;
                role = Some(match argv.get(i).map(String::as_str) {
                    Some("ping") => Role::Ping,
                    Some("pong") => Role::Pong,
                    _ => return Err("--role expects ping|pong".into()),
                });
            }
            "--domain" => {
                i += 1;
                domain = argv[i].parse().map_err(|_| "--domain")?;
            }
            "--payload" => {
                i += 1;
                payload = argv[i].parse().map_err(|_| "--payload")?;
            }
            "--warmup" => {
                i += 1;
                warmup = argv[i].parse().map_err(|_| "--warmup")?;
            }
            "--samples" => {
                i += 1;
                samples = argv[i].parse().map_err(|_| "--samples")?;
            }
            "--runtime-secs" => {
                i += 1;
                runtime_secs = argv[i].parse().map_err(|_| "--runtime-secs")?;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }
    Ok(Args {
        role: role.ok_or("--role required")?,
        domain,
        payload,
        warmup,
        samples,
        runtime_secs,
    })
}

// -----------------------------------------------------------------------------
// QoS — reliable + KeepLast(64) wie Cyclone+RTI+FastDDS Custom-Apps.
// -----------------------------------------------------------------------------

fn writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepLast;
    q.history.depth = 64;
    q
}

fn reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepLast;
    q.history.depth = 64;
    q
}

const REQ_TOPIC: &str = "RoundtripBench_Request";
const ECHO_TOPIC: &str = "RoundtripBench_Echo";

fn now_ns() -> u64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    u64::try_from(epoch.elapsed().as_nanos()).unwrap_or(0)
}

// -----------------------------------------------------------------------------
// Pong: take + write echo in tight poll loop.
// -----------------------------------------------------------------------------

fn run_pong(args: &Args) -> Result<(), String> {
    let factory = DomainParticipantFactory::instance();
    let dp = factory
        .create_participant(args.domain, DomainParticipantQos::default())
        .map_err(|e| format!("create_participant: {e:?}"))?;
    let topic_req = dp
        .create_topic::<Roundtrip>(REQ_TOPIC, TopicQos::default())
        .map_err(|e| format!("create_topic req: {e:?}"))?;
    let topic_echo = dp
        .create_topic::<Roundtrip>(ECHO_TOPIC, TopicQos::default())
        .map_err(|e| format!("create_topic echo: {e:?}"))?;
    let publisher = dp.create_publisher(PublisherQos::default());
    let subscriber = dp.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<Roundtrip>(&topic_echo, writer_qos())
        .map_err(|e| format!("create_datawriter: {e:?}"))?;
    let reader = subscriber
        .create_datareader::<Roundtrip>(&topic_req, reader_qos())
        .map_err(|e| format!("create_datareader: {e:?}"))?;

    // Auf Match warten (max 10s).
    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(10));
    let _ = reader.wait_for_matched_publication(1, Duration::from_secs(10));
    println!("pong[zerodds]: matched, polling");
    let deadline = Instant::now() + Duration::from_secs(args.runtime_secs);
    while Instant::now() < deadline {
        match reader.take() {
            Ok(samples) if !samples.is_empty() => {
                for s in samples {
                    let _ = writer.write(&s);
                }
            }
            _ => std::hint::spin_loop(),
        }
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Ping: send + spin until echo received (1-in-flight pattern).
// -----------------------------------------------------------------------------

struct PingStats {
    received: AtomicU64,
    rtts: Mutex<Vec<u64>>,
    warmup: u64,
}

fn run_ping(args: &Args) -> Result<(), String> {
    let factory = DomainParticipantFactory::instance();
    let dp = factory
        .create_participant(args.domain, DomainParticipantQos::default())
        .map_err(|e| format!("create_participant: {e:?}"))?;
    let topic_req = dp
        .create_topic::<Roundtrip>(REQ_TOPIC, TopicQos::default())
        .map_err(|e| format!("create_topic req: {e:?}"))?;
    let topic_echo = dp
        .create_topic::<Roundtrip>(ECHO_TOPIC, TopicQos::default())
        .map_err(|e| format!("create_topic echo: {e:?}"))?;
    let publisher = dp.create_publisher(PublisherQos::default());
    let subscriber = dp.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<Roundtrip>(&topic_req, writer_qos())
        .map_err(|e| format!("create_datawriter: {e:?}"))?;
    let reader = Arc::new(
        subscriber
            .create_datareader::<Roundtrip>(&topic_echo, reader_qos())
            .map_err(|e| format!("create_datareader: {e:?}"))?,
    );

    // Auf Match warten (max 10s).
    let _ = writer.wait_for_matched_subscription(1, Duration::from_secs(10));
    let _ = reader.wait_for_matched_publication(1, Duration::from_secs(10));
    eprintln!("ping[zerodds]: matched");

    let stats = Arc::new(PingStats {
        received: AtomicU64::new(0),
        rtts: Mutex::new(Vec::with_capacity(
            usize::try_from(args.samples).unwrap_or(0),
        )),
        warmup: args.warmup,
    });

    // Drainer-Thread: simuliert on_data_available — pollt in tight loop.
    let stats_d = stats.clone();
    let reader_d = reader.clone();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_d = stop.clone();
    let drainer = thread::spawn(move || {
        while !stop_d.load(Ordering::Relaxed) {
            if let Ok(samples) = reader_d.take() {
                for s in samples {
                    let now = now_ns();
                    let rtt = now.saturating_sub(s.t_send_ns).max(1);
                    let r = stats_d.received.fetch_add(1, Ordering::Relaxed);
                    if r >= stats_d.warmup {
                        if let Ok(mut g) = stats_d.rtts.lock() {
                            g.push(rtt);
                        }
                    }
                }
            } else {
                std::hint::spin_loop();
            }
        }
    });

    // Discovery stabilization.
    thread::sleep(Duration::from_millis(500));

    let mut sample = Roundtrip {
        sequence_id: 0,
        t_send_ns: 0,
        payload: vec![0xABu8; args.payload],
    };
    let total = args.warmup + args.samples;
    for seq in 0..total {
        sample.sequence_id = u32::try_from(seq).unwrap_or(u32::MAX);
        sample.t_send_ns = now_ns();
        let _ = writer.write(&sample);

        // 1-in-flight: spin auf received > seq, max 50ms.
        let deadline = Instant::now() + Duration::from_millis(50);
        while stats.received.load(Ordering::Relaxed) <= seq {
            if Instant::now() >= deadline {
                break;
            }
            std::hint::spin_loop();
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = drainer.join();

    let mut rtts = stats.rtts.lock().unwrap().clone();
    print_quantiles(&mut rtts, args.payload);
    Ok(())
}

fn print_quantiles(rtts: &mut [u64], payload_size: usize) {
    if rtts.is_empty() {
        println!("no samples");
        return;
    }
    let mut hist: Histogram<u64> =
        Histogram::new_with_bounds(1, 10_000_000_000, 3).expect("hist bounds");
    for &v in rtts.iter() {
        let _ = hist.record(v);
    }
    rtts.sort_unstable();
    let n = rtts.len();
    let p50 = hist.value_at_quantile(0.50);
    let p90 = hist.value_at_quantile(0.90);
    let p99 = hist.value_at_quantile(0.99);
    let p999 = hist.value_at_quantile(0.999);
    println!(
        "payload={}  n={}  min={:.3}us  p50={:.3}us  p90={:.3}us  p99={:.3}us  p999={:.3}us  max={:.3}us",
        payload_size,
        n,
        (*rtts.first().unwrap() as f64) / 1000.0,
        (p50 as f64) / 1000.0,
        (p90 as f64) / 1000.0,
        (p99 as f64) / 1000.0,
        (p999 as f64) / 1000.0,
        (*rtts.last().unwrap() as f64) / 1000.0,
    );
}

fn main() -> std::process::ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return std::process::ExitCode::from(2);
        }
    };
    let res = match args.role {
        Role::Pong => run_pong(&args),
        Role::Ping => run_ping(&args),
    };
    match res {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::from(1)
        }
    }
}
