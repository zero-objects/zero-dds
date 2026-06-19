//! ZeroDDS publisher on the **ROS 2 wire** — interop proof ZeroDDS → CycloneDDS
//! (= rmw_cyclonedds = real ROS 2). Topic `rt/chatter`, type
//! `std_msgs::msg::dds_::String_`. Opposite direction to the subscriber.
//!
//! # Usage (codepit)
//! ```text
//! # Cyclone listener: crates/ros2-rmw/interop/cyclone_ros_listener
//! ZERODDS_DATA_REPR_OFFER=XCDR1,XCDR2 \
//!   cargo run -p zerodds-dcps --release --example ros2_chatter_publisher
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Duration;

use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
    dds_type::{DdsType, DecodeError, EncodeError},
};

/// `std_msgs/msg/String` on the ROS 2 wire (as in the subscriber example).
#[derive(Debug, Clone, PartialEq)]
struct RosString {
    data: String,
}

impl DdsType for RosString {
    const TYPE_NAME: &'static str = "std_msgs::msg::dds_::String_";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let n = (self.data.len() + 1) as u32;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(self.data.as_bytes());
        out.push(0);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid {
                what: "RosString: short",
            });
        }
        let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let end = (4 + n.saturating_sub(1)).min(bytes.len());
        Ok(Self {
            data: String::from_utf8_lossy(&bytes[4..end]).into_owned(),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<RosString>("rt/chatter", TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher.create_datawriter::<RosString>(&topic, DataWriterQos::default())?;

    println!("ZeroDDS pub auf ROS-Wire rt/chatter (std_msgs::msg::dds_::String_)");
    std::thread::sleep(Duration::from_secs(1)); // Discovery-Match
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    for i in 0..count {
        let msg = RosString {
            data: format!("Hello Cyclone from ZeroDDS wire {i}"),
        };
        writer.write(&msg)?;
        println!("ZeroDDS -> rt/chatter: {}", msg.data);
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("== ZeroDDS publizierte {count} Samples ==");
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}
