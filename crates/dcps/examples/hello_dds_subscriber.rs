//! hello_dds_subscriber — minimal DDS subscriber that reads and prints
//! all samples on the "Chatter" topic.
//!
//! # Usage
//!
//! ```text
//! # Terminal 1:
//! cargo run -p zerodds-dcps --example hello_dds_publisher
//! # Terminal 2:
//! cargo run -p zerodds-dcps --example hello_dds_subscriber
//! ```
//!
//! Runs until Ctrl-C; uses `wait_for_data(1s)` as wake-on-sample,
//! avoiding busy-polling. The 1 s re-timeout is only
//! the max idle interval — under sample traffic the loop comes
//! through immediately.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Duration;

use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, RawBytes,
    SubscriberQos, TopicQos,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;

    let topic = participant.create_topic::<RawBytes>("Chatter", TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber.create_datareader::<RawBytes>(&topic, DataReaderQos::default())?;

    println!("hello_dds_subscriber: reading on Domain 0 Topic 'Chatter' — Ctrl-C to stop");

    loop {
        match reader.wait_for_data(Duration::from_secs(1)) {
            Ok(()) => {
                for sample in reader.take()? {
                    match std::str::from_utf8(&sample.data) {
                        Ok(s) => println!("  <- {s}"),
                        Err(_) => println!("  <- <{} bytes of non-UTF8>", sample.data.len()),
                    }
                }
            }
            Err(DdsError::Timeout) => {} // idle tick
            Err(e) => return Err(e.into()),
        }
    }
}
