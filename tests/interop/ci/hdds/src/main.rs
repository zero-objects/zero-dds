// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// HDDS <-> ZeroDDS interop client for the #28 CI gate, the counterpart of
// ../fastdds/fastdds_robot.cpp and ../rti/rti_robot.cxx.
//
// Rust and not C++: the C SDK of HDDS v1.1.5 answers
// `hdds_reader_set_listener` with HddsUnsupported, see
// tests/perf/dds-roundtrip-bench/hdds/src/main.rs.
//
// Usage:
//   hdds_robot pub [seconds] [domain]   write Robot samples on topic "robot"
//   hdds_robot sub [seconds] [domain]   count decoded samples, print result
//   hdds_robot version                  print the HDDS version and source
//
// The domain defaults to 100, as in the other clients. The QoS are the DDS
// defaults of the other clients: a RELIABLE writer, a BEST_EFFORT reader.
// The participants use UDPv4: HDDS takes its shared-memory transport
// between two endpoints of one host unless told otherwise.
//
// The subscriber prints exactly one machine-readable line to stdout:
//   HDDS_RESULT matched=<0|1> samples=<n>

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use hdds::dds::{
    DataReaderListener, DataWriterListener, PublicationMatchedStatus, SubscriptionMatchedStatus,
};
use hdds::{DataReader, Participant, QoS, TransportMode};

#[allow(dead_code, non_snake_case, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/robot.rs"));
}
use generated::Robot;

const TOPIC: &str = "robot";

fn participant(name: &str, domain: u32) -> Arc<Participant> {
    Participant::builder(name)
        .with_transport(TransportMode::UdpMulticast)
        .domain_id(domain)
        .shm_disable()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("hdds: participant: {e:?}");
            std::process::exit(2);
        })
}

#[derive(Default)]
struct WriterMatched(AtomicU32);

impl DataWriterListener<Robot> for WriterMatched {
    fn on_publication_matched(&self, status: PublicationMatchedStatus) {
        self.0.store(status.current_count, Ordering::SeqCst);
    }
}

/// Counts the samples the reader decodes, and takes its cache empty: HDDS
/// keeps a sample in the cache beside handing it to the listener.
#[derive(Default)]
struct Count {
    matched: AtomicU32,
    samples: AtomicU32,
    reader: OnceLock<Arc<DataReader<Robot>>>,
}

impl DataReaderListener<Robot> for Count {
    fn on_data_available(&self, _sample: &Robot) {
        self.samples.fetch_add(1, Ordering::SeqCst);
        if let Some(r) = self.reader.get() {
            while let Ok(Some(_)) = r.take() {}
        }
    }
    fn on_subscription_matched(&self, status: SubscriptionMatchedStatus) {
        self.matched.store(status.current_count, Ordering::SeqCst);
    }
}

fn run_pub(seconds: f64, domain: u32) -> i32 {
    let p = participant("hdds_robot_pub", domain);
    let matched = Arc::new(WriterMatched::default());
    let w = p.topic::<Robot>(TOPIC).and_then(|t| {
        t.writer()
            .qos(QoS::reliable())
            .with_listener(Arc::clone(&matched) as Arc<dyn DataWriterListener<Robot>>)
            .build()
    });
    let w = match w {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hdds: writer: {e:?}");
            return 2;
        }
    };
    eprintln!("[hdds pub] domain={domain} topic={TOPIC}");
    let t0 = Instant::now();
    let mut c: u32 = 0;
    while t0.elapsed().as_secs_f64() < seconds {
        let sample = Robot {
            id: 1,
            label: c % 1000,
        };
        if let Err(e) = w.write(&sample) {
            eprintln!("hdds: write {c}: {e:?}");
        }
        c += 1;
        std::thread::sleep(Duration::from_millis(300));
    }
    eprintln!(
        "[hdds pub] wrote {c}, matched {}",
        matched.0.load(Ordering::SeqCst)
    );
    0
}

fn run_sub(seconds: f64, domain: u32) -> i32 {
    let p = participant("hdds_robot_sub", domain);
    let count = Arc::new(Count::default());
    let r = p.topic::<Robot>(TOPIC).and_then(|t| {
        t.reader()
            .qos(QoS::best_effort())
            .with_listener(Arc::clone(&count) as Arc<dyn DataReaderListener<Robot>>)
            .build()
    });
    let r = match r {
        Ok(r) => Arc::new(r),
        Err(e) => {
            eprintln!("hdds: reader: {e:?}");
            return 2;
        }
    };
    let _ = count.reader.set(Arc::clone(&r));
    eprintln!("[hdds sub] domain={domain} topic={TOPIC}");
    std::thread::sleep(Duration::from_secs_f64(seconds));
    let matched = u32::from(count.matched.load(Ordering::SeqCst) > 0);
    println!(
        "HDDS_RESULT matched={matched} samples={}",
        count.samples.load(Ordering::SeqCst)
    );
    0
}

fn domain_arg(arg: Option<String>) -> u32 {
    let Some(arg) = arg else { return 100 };
    match arg.parse::<u32>() {
        Ok(d) if d <= 232 => d,
        _ => {
            eprintln!("invalid domain {arg}: expected an integer in 0..=232");
            std::process::exit(2);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    let seconds = args
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(12.0);
    let domain = domain_arg(args.next());
    let code = match mode.as_str() {
        "version" => {
            println!("hdds {}", env!("HDDS_ROBOT_HDDS"));
            0
        }
        "pub" => run_pub(seconds, domain),
        "sub" => run_sub(seconds, domain),
        _ => {
            eprintln!("usage: hdds_robot <pub|sub|version> [seconds] [domain]");
            2
        }
    };
    std::process::exit(code);
}
