//! C3 large-data publisher — drives RTPS DATA_FRAG/reassembly through the
//! full DCPS stack (PointCloud-sized samples). Payload size via arg
//! (bytes, default 2 MiB ≈ 1560 fragments at 1344 B). Content is a
//! verifiable pattern `data[i] = (i % 251)`.
//!
//! ```text
//! ZERODDS_PEERS=127.0.0.1 ZERODDS_NO_MULTICAST=1 \
//!   cargo run -p zerodds-dcps --release --example largedata_pub -- 2097152
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Duration;

use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
    dds_type::{DdsType, DecodeError, EncodeError},
};

/// `sequence<octet>` on the wire (u32 LE length + bytes).
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
    let size: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2 * 1024 * 1024);
    let count: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    // 3rd arg: interval in ms between writes (default 300). 0 = back-to-back
    // for throughput measurement.
    let interval_ms: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let payload: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<LargeMsg>("rt/bigdata", TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher.create_datawriter::<LargeMsg>(&topic, DataWriterQos::default())?;

    println!("C3 large-data pub: {size} B/sample, {count} samples on rt/bigdata");
    std::thread::sleep(Duration::from_secs(6)); // let discovery (5s SPDP) settle
    for i in 0..count {
        writer.write(&LargeMsg {
            data: payload.clone(),
        })?;
        println!("pub #{i} ({size} B)");
        if interval_ms > 0 {
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
    }
    println!("== published {count}x{size} B ==");
    std::thread::sleep(Duration::from_secs(1));
    Ok(())
}
