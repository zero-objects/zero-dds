//! C3 latency: ponger — echoes every sample received on `rt/ping`
//! back unchanged on `rt/pong`. Counterpart to `latency_ping`.
//! Measures nothing itself; the pinger measures the round-trip time
//! (clock-sync-free). Runs until killed.
//!
//! ```text
//! ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=<ping-host-ip> \
//!   cargo run -p zerodds-dcps --release --example latency_pong
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Duration;

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
    dds_type::{DdsType, DecodeError, EncodeError},
    qos::ReliabilityKind,
};

#[derive(Debug, Clone, PartialEq)]
struct LatencyMsg {
    seq: u32,
    data: Vec<u8>,
}

impl DdsType for LatencyMsg {
    const TYPE_NAME: &'static str = "zerodds::LatencyMsg";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "LatencyMsg: short",
            });
        }
        let seq = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let end = (8 + n).min(bytes.len());
        Ok(Self {
            seq,
            data: bytes[8..end].to_vec(),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let ping_topic = participant.create_topic::<LatencyMsg>("rt/ping", TopicQos::default())?;
    let pong_topic = participant.create_topic::<LatencyMsg>("rt/pong", TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let publisher = participant.create_publisher(PublisherQos::default());
    let mut rqos = DataReaderQos::default();
    rqos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber.create_datareader::<LatencyMsg>(&ping_topic, rqos)?;
    let writer =
        publisher.create_datawriter::<LatencyMsg>(&pong_topic, DataWriterQos::default())?;

    println!("latency_pong: echo rt/ping -> rt/pong");
    loop {
        match reader.wait_for_data(Duration::from_secs(2)) {
            Ok(()) => {
                for s in reader.take()? {
                    writer.write(&s)?;
                }
            }
            Err(DdsError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
    }
}
