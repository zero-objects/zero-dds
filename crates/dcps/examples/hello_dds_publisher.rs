//! hello_dds_publisher — minimal DDS publisher that periodically
//! sends samples on the "Chatter" topic.
//!
//! # Usage
//!
//! ```text
//! # Terminal 1:
//! cargo run -p zerodds-dcps --example hello_dds_publisher
//! # Terminal 2 (same host, different process):
//! cargo run -p zerodds-dcps --example hello_dds_subscriber
//! ```
//!
//! The publisher runs until Ctrl-C. Every second one sample
//! "hello #<counter>" is sent to all matched subscribers on domain 0.
//!
//! # What happens here
//!
//! 1. `DomainParticipantFactory::create_participant(0, ...)` starts
//!    the `DcpsRuntime` — that is 4 UDP sockets + SPDP/SEDP threads.
//! 2. SPDP beacons are sent every 5 s to the multicast group 239.255.0.1;
//!    this is how the subscriber finds us.
//! 3. `create_topic("Chatter", ...)` creates the topic registry entry.
//! 4. `create_datawriter` registers a user writer + EntityId with
//!    the runtime and announces it via SEDP.
//! 5. `writer.write(&sample)` encodes the sample and hands it
//!    to the internal `ReliableWriter`, which sends it to all matched
//!    readers via UDP.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::thread;
use std::time::Duration;

use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;

    let topic = participant.create_topic::<RawBytes>("Chatter", TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher.create_datawriter::<RawBytes>(&topic, DataWriterQos::default())?;

    println!("hello_dds_publisher: sending on Domain 0 Topic 'Chatter' — Ctrl-C to stop");

    // Wait a few seconds so that SPDP+SEDP can discover the subscriber.
    // Without a wait phase the first samples go nowhere (no reader
    // matched yet). v1.3 brings `wait_for_matched_subscription`.
    thread::sleep(Duration::from_secs(3));

    let mut counter: u32 = 0;
    loop {
        let payload = format!("hello #{counter}");
        writer.write(&RawBytes::new(payload.clone().into_bytes()))?;
        println!("  -> {payload}");
        counter = counter.wrapping_add(1);
        thread::sleep(Duration::from_secs(1));
    }
}
