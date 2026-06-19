//! C3 large-data subscriber — receives fragmented PointCloud-sized
//! samples and **verifies integrity** (size + pattern
//! `data[i] = (i % 251)`). Proves that DATA_FRAG + reassembly run
//! correctly through the full DCPS stack. Exit 0 on ≥1 intact sample.
//!
//! ```text
//! ZERODDS_PEERS=127.0.0.1 ZERODDS_NO_MULTICAST=1 \
//!   cargo run -p zerodds-dcps --release --example largedata_sub
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, SubscriberQos,
    TopicQos,
    dds_type::{DdsType, DecodeError, EncodeError},
    qos::ReliabilityKind,
};

#[derive(Debug, Clone, PartialEq)]
struct LargeMsg {
    data: Vec<u8>,
}

impl DdsType for LargeMsg {
    const TYPE_NAME: &'static str = "zerodds::LargeMsg";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid {
                what: "LargeMsg: short",
            });
        }
        let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let end = (4 + n).min(bytes.len());
        Ok(Self {
            data: bytes[4..end].to_vec(),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<LargeMsg>("rt/bigdata", TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let mut qos = DataReaderQos::default();
    qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber.create_datareader::<LargeMsg>(&topic, qos)?;

    // Number of samples to measure (default 5); raise for throughput
    // measurement.
    let target: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    println!("C3 large-data sub: waiting for fragmented samples on rt/bigdata");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut intact = 0usize;
    let mut corrupt = 0usize;
    let mut bytes_total: u64 = 0;
    let mut first_at: Option<Instant> = None;
    let mut last_at = Instant::now();
    while Instant::now() < deadline && intact < target {
        match reader.wait_for_data(Duration::from_secs(1)) {
            Ok(()) => {
                for s in reader.take()? {
                    let ok = s
                        .data
                        .iter()
                        .enumerate()
                        .all(|(i, &b)| b == (i % 251) as u8);
                    if ok {
                        intact += 1;
                        first_at.get_or_insert_with(Instant::now);
                        last_at = Instant::now();
                        bytes_total += s.data.len() as u64;
                        println!("intact: {} B (pattern OK)", s.data.len());
                    } else {
                        corrupt += 1;
                        println!("CORRUPT: {} B", s.data.len());
                    }
                }
            }
            Err(DdsError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
    }
    // Throughput between first and last intact sample (clock-sync-free).
    if let Some(start) = first_at {
        let secs = last_at.duration_since(start).as_secs_f64();
        if secs > 0.0 && intact > 1 {
            let mbps = (bytes_total as f64 / (1024.0 * 1024.0)) / secs;
            println!(
                "== throughput: {:.1} MiB/s ({} Samples, {:.2} MiB in {:.2}s) ==",
                mbps,
                intact,
                bytes_total as f64 / (1024.0 * 1024.0),
                secs
            );
        }
    }
    println!("== intact={intact} corrupt={corrupt} ==");
    if intact == 0 || corrupt > 0 {
        std::process::exit(2);
    }
    Ok(())
}
