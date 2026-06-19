//! ZeroDDS subscriber on the **ROS 2 wire** — interop proof against CycloneDDS
//! (= `rmw_cyclonedds` = real ROS 2). Topic `rt/chatter`, type
//! `std_msgs::msg::dds_::String_`, RMW default QoS.
//!
//! # Usage (the Linux test host, alongside the Cyclone talker)
//! ```text
//! # Terminal 1: ZeroDDS subscriber
//! cargo run -p zerodds-dcps --example ros2_chatter_subscriber
//! # Terminal 2: Cyclone talker (= ROS 2 talker)
//! crates/ros2-rmw/interop/run_capture.sh   # or just the talker
//! ```
//! Exits after 20 samples or a 15 s timeout.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, SubscriberQos,
    TopicQos,
    dds_type::{DdsType, DecodeError, EncodeError},
    qos::ReliabilityKind,
};

/// `std_msgs/msg/String` on the ROS 2 wire. `TYPE_NAME` must exactly
/// match the name announced by `rmw_cyclonedds` (strict matching).
#[derive(Debug, Clone, PartialEq)]
struct RosString {
    data: String,
}

impl DdsType for RosString {
    const TYPE_NAME: &'static str = "std_msgs::msg::dds_::String_";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        // CDR string: u32 length (incl. NUL, little-endian) + bytes + NUL.
        let n = (self.data.len() + 1) as u32;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(self.data.as_bytes());
        out.push(0);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        // Payload is already header-stripped (the runtime removes the
        // 4-byte encapsulation). Cyclone emits CDR_LE.
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid {
                what: "RosString: payload shorter than CDR length prefix",
            });
        }
        let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let end = (4 + n.saturating_sub(1)).min(bytes.len());
        let s = String::from_utf8_lossy(&bytes[4..end]).into_owned();
        Ok(Self { data: s })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    // C4: ROS profile out-of-the-box — the reader offers [XCDR1, XCDR2]
    // and thus matches the XCDR1 writer of ROS 2/Cyclone WITHOUT the
    // ZERODDS_DATA_REPR_OFFER env workaround. ZERODDS_PEERS/NO_MULTICAST
    // are still merged in DcpsRuntime::start (C1).
    let participant = factory.create_participant_with_config(
        0,
        DomainParticipantQos::default(),
        zerodds_dcps::runtime::RuntimeConfig::ros_defaults(),
    )?;

    // ROS 2 wire: topic "rt/chatter" (= topic_mangling of "/chatter"),
    // type name std_msgs::msg::dds_::String_.
    let topic = participant.create_topic::<RosString>("rt/chatter", TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    // RMW default QoS: RELIABLE (the DDS reader default is BEST_EFFORT — does not
    // match the reliable-volatile rmw_cyclonedds writer). Durability VOLATILE +
    // History KEEP_LAST remain default.
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber.create_datareader::<RosString>(&topic, reader_qos)?;

    println!(
        "ZeroDDS sub on ROS wire rt/chatter (std_msgs::msg::dds_::String_) — waiting for Cyclone talker"
    );

    let deadline = Instant::now() + Duration::from_secs(25);
    let mut got = 0usize;
    while Instant::now() < deadline && got < 20 {
        match reader.wait_for_data(Duration::from_secs(1)) {
            Ok(()) => {
                for sample in reader.take()? {
                    println!("ZeroDDS <- rt/chatter: {}", sample.data);
                    got += 1;
                }
            }
            Err(DdsError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
    }
    println!("== ZeroDDS received {got} samples from the Cyclone ROS talker ==");
    if got == 0 {
        std::process::exit(2);
    }
    Ok(())
}
