//! hello_dds_subscriber — minimal-DDS-Subscriber, der alle Samples
//! auf Topic "Chatter" liest und druckt.
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
//! Laeuft bis Ctrl-C; nutzt `wait_for_data(1s)` als Wake-on-Sample,
//! spart busy-polling. Der Re-Timeout von 1 s ist nur
//! der max. Idle-Intervall — bei Sample-Traffic kommt die Schleife
//! sofort durch.

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
