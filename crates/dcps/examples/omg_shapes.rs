//! OMG `dds-rtps` ShapesDemo interop probe — the **five-member
//! `@appendable`** `ShapeType` of the interoperability test suite's
//! `srcCxx/shape.idl`, which the vendors' `shape_main` release binaries
//! speak ([`zerodds_dcps::interop::ShapeType`]).
//!
//! The counterpart probe for the classic four-member `@final` type is
//! `classic_shapes.rs`; the two wire forms are not compatible.
//!
//! # Usage
//!
//! ```text
//! cargo run -p zerodds-dcps --example omg_shapes -- pub [topic] [domain] [color]
//! cargo run -p zerodds-dcps --example omg_shapes -- sub [topic] [domain]
//! ```
//!
//! Both modes exit by themselves after `ZERODDS_RUN_SECS` (default 30) and
//! print a final `SENT <n>` / `RECEIVED <n>` line, so a harness never has to
//! guess whether a silent process was working or hung.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map_or("sub", String::as_str);
    let topic_name = args.get(2).map_or("Square", String::as_str);
    let domain_id: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let color = args.get(4).map_or("BLUE", String::as_str);
    let run_secs: u64 = env::var("ZERODDS_RUN_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(domain_id, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ShapeType>(topic_name, TopicQos::default())?;
    let deadline = Instant::now() + Duration::from_secs(run_secs);

    match mode {
        "pub" => {
            let publisher = participant.create_publisher(PublisherQos::default());
            let writer =
                publisher.create_datawriter::<ShapeType>(&topic, DataWriterQos::default())?;
            println!(
                "omg_shapes pub: Type=ShapeType(@appendable,5) Topic={topic_name} \
                 Color={color} Domain={domain_id} for {run_secs}s"
            );
            let matched = writer.wait_for_matched_subscription(1, Duration::from_secs(15));
            println!("  matched_subscription: {matched:?}");
            let mut sent = 0_u32;
            let mut t = 0.0_f32;
            while Instant::now() < deadline {
                let sample = ShapeType::new(
                    color,
                    (120.0 + 80.0 * t.sin()) as i32,
                    (135.0 + 90.0 * (t * 1.3).cos()) as i32,
                    30,
                );
                writer.write(&sample)?;
                println!(
                    "  -> color={:8} x={:4} y={:4} size={}",
                    sample.color, sample.x, sample.y, sample.shapesize
                );
                sent += 1;
                t += 0.1;
                thread::sleep(Duration::from_millis(100));
            }
            println!("SENT {sent}");
        }
        "sub" => {
            let subscriber = participant.create_subscriber(SubscriberQos::default());
            let reader =
                subscriber.create_datareader::<ShapeType>(&topic, DataReaderQos::default())?;
            println!(
                "omg_shapes sub: Type=ShapeType(@appendable,5) Topic={topic_name} \
                 Domain={domain_id} for {run_secs}s"
            );
            let mut got = 0_u32;
            while Instant::now() < deadline {
                match reader.wait_for_data(Duration::from_secs(1)) {
                    Ok(()) => {
                        for s in reader.take()? {
                            got += 1;
                            println!(
                                "  <- color={:8} x={:4} y={:4} size={} payload_len={}",
                                s.color,
                                s.x,
                                s.y,
                                s.shapesize,
                                s.additional_payload_size.len()
                            );
                        }
                    }
                    Err(DdsError::Timeout) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            println!("RECEIVED {got}");
        }
        other => {
            eprintln!("unknown mode '{other}' — expected 'pub' or 'sub'");
            std::process::exit(2);
        }
    }
    Ok(())
}
