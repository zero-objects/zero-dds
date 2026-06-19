//! ShapesDemo subscriber — reads ShapeType samples on one of the
//! standard ShapesDemo topics and prints them.
//!
//! Interop goal: receive and correctly decode samples from a running
//! Cyclone, Fast-DDS or RTI ShapesDemo publisher.

//! Set `ZERODDS_SHAPE_EXTENDED=1` to subscribe to `ShapeExtendedType` (the RTI
//! Connext 7.x default), so an unmodified RTI ShapesDemo publisher matches.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::time::Duration;

use zerodds_dcps::interop::{ShapeExtendedType, ShapeType};
use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, SubscriberQos,
    TopicQos,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let topic_name = args.get(1).map_or("Square", String::as_str);
    let domain_id: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let extended = env::var("ZERODDS_SHAPE_EXTENDED").is_ok();

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(domain_id, DomainParticipantQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());

    let kind = if extended {
        "ShapeExtendedType"
    } else {
        "ShapeType"
    };
    println!(
        "shapes_demo_subscriber: Type={kind} Topic={topic_name} Domain={domain_id} — Ctrl-C to stop"
    );

    if extended {
        let topic =
            participant.create_topic::<ShapeExtendedType>(topic_name, TopicQos::default())?;
        let reader =
            subscriber.create_datareader::<ShapeExtendedType>(&topic, DataReaderQos::default())?;
        loop {
            match reader.wait_for_data(Duration::from_secs(1)) {
                Ok(()) => {
                    for s in reader.take()? {
                        println!(
                            "  <- [ext] color={:8} x={:4} y={:4} size={} fill={:?} angle={:.1}",
                            s.color, s.x, s.y, s.shapesize, s.fill_kind, s.angle
                        );
                    }
                }
                Err(DdsError::Timeout) => {}
                Err(e) => return Err(e.into()),
            }
        }
    } else {
        let topic = participant.create_topic::<ShapeType>(topic_name, TopicQos::default())?;
        let reader = subscriber.create_datareader::<ShapeType>(&topic, DataReaderQos::default())?;
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
}
