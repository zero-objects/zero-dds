// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! roundtrip-typed — apples-to-apples roundtrip latency bench through
//! the full ZeroDDS DCPS pipeline with typed XCDR2 encode/decode.
//!
//! Counterpart to `cyclone_app.cpp` / `rti_app.cpp` / `fastdds_app.cpp` —
//! same pattern (ping/pong, 1-in-flight, reliable + KeepLast(64)),
//! same payload sizes (32/64/256/1024/4096), same sample count
//! (5000), same transport (UDP loopback).
//!
//! Where `roundtrip-1us` hands `zerodds_writer_write()` bytes that were
//! encoded beforehand and so measures the pipeline with the codec taken
//! out, this one runs the whole typed path:
//!
//!   application (Rust)               -- the typed `Roundtrip` struct
//!   ↓
//!   `DdsType::encode`/`decode`       -- XCDR2-LE, `@final`
//!   ↓
//!   `DataWriter<Roundtrip>::write`   -- history cache, QoS
//!   ↓
//!   RTPS 2.5                         -- the wire protocol
//!   ↓
//!   UDP loopback                     -- the transport
//!
//! # The body is XCDR2 and it is written at XCDR2's alignment
//!
//! Both halves of this file used to disagree, and because both halves
//! are in this file the disagreement round-tripped and the numbers
//! stood. Two faults, in the same six lines:
//!
//! * the body was built by `BufferWriter::new` and read by
//!   `BufferReader::new`, whose `MAXALIGN` is **eight** — XCDR1's.
//!   XCDR2 caps alignment at **four** (XTypes 1.3 §7.4.1.1.1, and
//!   `Encoding::max_alignment` in `crates/ast-layout`, which was read
//!   off Cyclone's and RTI Connext 7.7's own serialisers). `t_send_ns`
//!   is eight octets behind a four-octet `sequence_id`, so it sat at
//!   offset 8 where XCDR2 puts it at 4, and everything after it moved
//!   with it.
//! * `encode` prepended a four-octet encapsulation header of its own.
//!   `DdsType::encode` returns the **body**; `DataWriter::write` writes
//!   the header, from the representation the writer's SEDP offer
//!   locked. The header this file wrote therefore went out *inside* the
//!   body, and `decode` skipped four octets to put it back.
//!
//! Either fault alone makes every octet after `sequence_id` land
//! somewhere a peer does not look for it, and the bench is a comparison
//! against Cyclone, RTI and Fast DDS — where a foreign peer *is* the
//! other end. `crates/dcps/examples/classic_shapes.rs` is the calling
//! convention this now follows, and it is the one that has been run
//! against RTI's and OpenDDS' shape demos.
//!
//! # Usage
//!
//! ```text
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

use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    SubscriberQos, TopicQos,
};

// -----------------------------------------------------------------------------
// The topic type, generated from the IDL the vendor apps are generated from
// -----------------------------------------------------------------------------

// `tests/perf/dds-roundtrip-bench/roundtrip.idl` is the file Cyclone's
// `idlc`, RTI's `rtiddsgen` and `fastddsgen` compile for the other three
// apps of this bench. `build.rs` compiles the crate's copy of it, held
// byte-equal to it, for this one, so the number this file reports is a
// comparison of four codecs generated from one declaration rather than of
// three generated ones against a fourth written beside them.
//
// `non_snake_case`: the generated module keeps the IDL's name,
// `RoundtripBench`.
#[allow(non_snake_case)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/roundtrip.rs"));
}

use generated::RoundtripBench::Roundtrip;

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
// QoS — reliable + KeepLast(64) like the Cyclone+RTI+FastDDS custom apps.
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

    // Wait for match (max 10s).
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

    // Wait for match (max 10s).
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

    // Drainer thread: simulates on_data_available — polls in tight loop.
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
                    if r >= stats_d.warmup
                        && let Ok(mut g) = stats_d.rtts.lock()
                    {
                        g.push(rtt);
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

        // 1-in-flight: spin on received > seq, max 50ms.
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

#[cfg(test)]
mod tests {
    use super::Roundtrip;

    /// XCDR2's `MAXALIGN`. XTypes 1.3 §7.4.1.1.1.
    ///
    /// `Writer::new` and `Reader::new` are eight, which is XCDR1's. The
    /// generated codec declares PLAIN_CDR2, so the version it declares
    /// is named here and the tests below hold it to that.
    const XCDR2_MAX_ALIGN: usize = 4;
    use zerodds_dcps::DdsType;
    use zerodds_wire::{Endianness, Reader, Writer};

    fn sample() -> Roundtrip {
        Roundtrip {
            sequence_id: 0x1111_1111,
            t_send_ns: 0x2222_2222_2222_2222,
            payload: vec![0xAB; 4],
        }
    }

    /// The body is what `DataWriter::write` expects: no encapsulation
    /// header, and `t_send_ns` where XCDR2 puts it.
    ///
    /// Twenty octets, not twenty-four. The four that are not there are
    /// the pad an eight-octet alignment would put between
    /// `sequence_id` and `t_send_ns`, and the four an encapsulation
    /// header written here would add in front. The same twenty octets
    /// the hand-written codec this file used to carry produced — the
    /// generated one replaced it without moving a single one.
    #[test]
    fn body_is_xcdr2_and_carries_no_encapsulation_header() {
        let mut out = Vec::new();
        DdsType::encode(&sample(), &mut out).expect("encode");
        assert_eq!(
            out,
            [
                0x11, 0x11, 0x11, 0x11, // sequence_id  @0
                0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, // t_send_ns @4
                0x04, 0x00, 0x00, 0x00, // sequence length @12
                0xAB, 0xAB, 0xAB, 0xAB, // payload @16
            ],
            "the body is not the twenty octets XCDR2 asks for"
        );
    }

    /// The big-endian body is big-endian.
    ///
    /// The hand-written codec did not override `encode_be`, and the
    /// trait default delegates to `encode` — so a writer that stamped
    /// `CDR2_BE` on the encapsulation header shipped the little-endian
    /// octets above behind it. Only the sequence length shows it in
    /// this sample: `04 00 00 00` little-endian against `00 00 00 04`
    /// big-endian, the three other members being byte-symmetric
    /// values. That is why the assertion is on the whole body and not
    /// on "the two differ".
    #[test]
    fn the_big_endian_body_is_the_members_in_the_other_order() {
        let mut out = Vec::new();
        DdsType::encode_be(&sample(), &mut out).expect("encode_be");
        assert_eq!(
            out,
            [
                0x11, 0x11, 0x11, 0x11, // sequence_id  @0
                0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, // t_send_ns @4
                0x00, 0x00, 0x00, 0x04, // sequence length @12
                0xAB, 0xAB, 0xAB, 0xAB, // payload @16
            ],
            "encode_be handed out a little-endian body"
        );
        let back = <Roundtrip as DdsType>::decode_be(&out).expect("decode_be");
        assert_eq!(back.t_send_ns, 0x2222_2222_2222_2222);
        assert_eq!(back.payload.len(), 4);
    }

    /// A topic type name with a type behind it.
    ///
    /// The hand-written codec left `type_object()` at the trait's
    /// `None` and `TYPE_IDENTIFIER` at `TypeIdentifier::None`: SEDP
    /// announced `RoundtripBench::Roundtrip` and carried no
    /// `PID_TYPE_INFORMATION`, so a peer that asked the TypeLookup
    /// service what that name is got nothing (XTypes 1.3 §7.6.3.2).
    /// The vendor apps this bench compares against all announce one.
    #[test]
    fn the_type_announces_itself() {
        assert_eq!(
            <Roundtrip as DdsType>::TYPE_NAME,
            "RoundtripBench::Roundtrip"
        );
        assert!(
            <Roundtrip as DdsType>::type_object().is_some(),
            "the topic type announces a name with no type behind it"
        );
        assert_ne!(
            <Roundtrip as DdsType>::type_identifier(),
            zerodds_types::TypeIdentifier::None,
            "SEDP would carry no PID_TYPE_INFORMATION"
        );
    }

    /// A round trip through this file cannot catch the alignment: both
    /// ends are this file. So the reader is built at the other
    /// alignment on purpose, and what it reads back is asserted —
    /// because it does not fail, it returns a different number.
    #[test]
    fn the_other_alignment_reads_a_different_value_and_does_not_fail() {
        let mut out = Vec::new();
        DdsType::encode(&sample(), &mut out).expect("encode");

        let mut good = Reader::with_max_align(&out, Endianness::Little, XCDR2_MAX_ALIGN);
        assert_eq!(good.read_u32().expect("sequence_id"), 0x1111_1111);
        assert_eq!(good.read_u64().expect("t_send_ns"), 0x2222_2222_2222_2222);

        let mut xcdr1 = Reader::with_max_align(&out, Endianness::Little, 8);
        assert_eq!(xcdr1.read_u32().expect("sequence_id"), 0x1111_1111);
        let wrong = xcdr1.read_u64().expect("no error — that is the point");
        assert_ne!(
            wrong, 0x2222_2222_2222_2222,
            "an eight-octet alignment must not happen to land on the same value"
        );
    }

    /// `Writer::new` is XCDR1's `MAXALIGN`, which is what this file
    /// used while declaring PLAIN_CDR2. Pinned so the two spellings
    /// cannot converge again unnoticed.
    #[test]
    fn writer_new_is_eight_and_this_bench_is_four() {
        assert_eq!(Writer::new(Endianness::Little).max_align(), 8);
        assert_eq!(XCDR2_MAX_ALIGN, 4);
    }

    #[test]
    fn round_trip() {
        let mut out = Vec::new();
        let s = sample();
        DdsType::encode(&s, &mut out).expect("encode");
        let back = <Roundtrip as DdsType>::decode(&out).expect("decode");
        assert_eq!(back.sequence_id, s.sequence_id);
        assert_eq!(back.t_send_ns, s.t_send_ns);
        assert_eq!(back.payload, s.payload);
    }
}
