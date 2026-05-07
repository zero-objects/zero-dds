//! ShapesDemo-Subscriber — liest ShapeType-Samples auf einem der
//! Standard-ShapesDemo-Topics und druckt sie.
//!
//! Interop-Ziel: Samples eines laufenden Cyclone-, Fast-DDS- oder
//! RTI-ShapesDemo-Publishers empfangen und korrekt dekodieren.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::time::Duration;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, SubscriberQos,
    TopicQos,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let topic_name = args.get(1).map_or("Square", String::as_str);
    let domain_id: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(domain_id, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeType>(topic_name, TopicQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber.create_datareader::<ShapeType>(&topic, DataReaderQos::default())?;

    println!("shapes_demo_subscriber: Topic={topic_name} Domain={domain_id} — Ctrl-C to stop");

    loop {
        match reader.wait_for_data(Duration::from_secs(1)) {
            Ok(()) => {
                for sample in reader.take()? {
                    println!(
                        "  <- color={:8} x={:4} y={:4} size={}",
                        sample.color, sample.x, sample.y, sample.shapesize
                    );
                }
            }
            Err(DdsError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
    }
}
