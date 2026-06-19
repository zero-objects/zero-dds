//! ShapesDemo publisher — periodically sends ShapeType samples on one
//! of the standard ShapesDemo topics (Square/Circle/Triangle).
//!
//! Interop goal: a Cyclone, Fast-DDS, or RTI ShapesDemo client draws
//! the shapes live on its canvas.
//!
//! # Usage
//!
//! ```text
//! # Default: Square topic, Color=BLUE, Domain 0
//! cargo run -p zerodds-dcps --example shapes_demo_publisher
//!
//! # With parameters:
//! cargo run -p zerodds-dcps --example shapes_demo_publisher -- Circle RED 0
//!
//! # ShapeExtendedType (RTI Connext 7.x default — no `-dataType Shape` needed):
//! ZERODDS_SHAPE_EXTENDED=1 cargo run -p zerodds-dcps --example shapes_demo_publisher
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::thread;
use std::time::Duration;

use zerodds_dcps::interop::{ShapeExtendedType, ShapeFillKind, ShapeType};
use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let topic_name = args.get(1).map_or("Square", String::as_str);
    let color = args.get(2).map_or("BLUE", String::as_str);
    let domain_id: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    // RTI 7.x ShapesDemo publishes `ShapeExtendedType` by default. Set this to
    // interop with an unmodified RTI client (no `-dataType Shape` flag).
    let extended = env::var("ZERODDS_SHAPE_EXTENDED").is_ok();

    if !["Square", "Circle", "Triangle"].contains(&topic_name) {
        eprintln!(
            "warning: topic '{topic_name}' is not a standard ShapesDemo topic (Square/Circle/Triangle)",
        );
    }

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(domain_id, DomainParticipantQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());

    let kind = if extended {
        "ShapeExtendedType"
    } else {
        "ShapeType"
    };
    println!(
        "shapes_demo_publisher: Type={kind} Topic={topic_name} Color={color} Domain={domain_id} — Ctrl-C to stop"
    );

    // Sine-wave motion on a 240x270 canvas (ShapesDemo default).
    let shapesize = 30;
    let motion = |t: f32| -> (i32, i32) {
        let x = (120.0 + 80.0 * t.sin()) as i32;
        let y = (135.0 + 90.0 * (t * 1.3).cos()) as i32;
        (x, y)
    };

    if extended {
        let topic =
            participant.create_topic::<ShapeExtendedType>(topic_name, TopicQos::default())?;
        let writer =
            publisher.create_datawriter::<ShapeExtendedType>(&topic, DataWriterQos::default())?;
        wait_matched(&writer.wait_for_matched_subscription(1, Duration::from_secs(10)));
        let mut t: f32 = 0.0;
        loop {
            let (x, y) = motion(t);
            // Rotate the fill/angle so an RTI viewer shows the extended fields.
            let sample = ShapeExtendedType::new(
                color,
                x,
                y,
                shapesize,
                ShapeFillKind::SolidFill,
                t.to_degrees() % 360.0,
            );
            writer.write(&sample)?;
            println!(
                "  -> [ext] color={color} x={x} y={y} size={shapesize} angle={:.1}",
                sample.angle
            );
            t += 0.15;
            thread::sleep(Duration::from_millis(100));
        }
    } else {
        let topic = participant.create_topic::<ShapeType>(topic_name, TopicQos::default())?;
        let writer = publisher.create_datawriter::<ShapeType>(&topic, DataWriterQos::default())?;
        wait_matched(&writer.wait_for_matched_subscription(1, Duration::from_secs(10)));
        let mut t: f32 = 0.0;
        loop {
            let (x, y) = motion(t);
            let sample = ShapeType::new(color, x, y, shapesize);
            writer.write(&sample)?;
            println!("  -> color={color} x={x} y={y} size={shapesize}");
            t += 0.15;
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn wait_matched(matched: &zerodds_dcps::Result<()>) {
    match matched {
        Ok(()) => println!("matched subscriber found, starting publication"),
        Err(_) => println!("no subscriber in 10s — publishing anyway, samples may drop"),
    }
}
