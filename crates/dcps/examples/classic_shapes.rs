//! Classic ShapesDemo interop probe — the **four-member `@final`**
//! `ShapeType`, which is the wire form the RTI `ShapeType_publisher` /
//! `ShapeType_subscriber` examples and the OpenDDS shapes example speak.
//!
//! [`zerodds_dcps::interop::ShapeType`] is the *other* ShapeType: the OMG
//! `dds-rtps` `shape.idl` one, five members and `@appendable`. Both are in
//! use in the field and they are not wire-compatible — `@appendable` frames
//! the sample with a DHEADER and stamps encapsulation `0x0009`, `@final`
//! writes neither. A peer built from
//!
//! ```idl
//! @final
//! struct ShapeType {
//!     @key string color;
//!     long x;
//!     long y;
//!     long shapesize;
//! };
//! ```
//!
//! therefore needs this type and not that one. The IDL above is the one in
//! `zerodds-examples/cross-vendor-shapes/ShapeType.idl`.
//!
//! # Usage
//!
//! ```text
//! cargo run -p zerodds-dcps --example classic_shapes -- pub [topic] [domain] [color]
//! cargo run -p zerodds-dcps --example classic_shapes -- sub [topic] [domain]
//! ```
//!
//! The subscriber prints one `<- color=… x=… y=… size=…` line per received
//! sample and, on exit, a `RECEIVED <n>` total — a run that matched but
//! carried no payload is then visibly different from one that carried some.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError, Extensibility};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_wire::{Endianness, Reader, Writer};

/// XCDR2's `MAXALIGN` (XTypes 1.3 §7.4.1).
const XCDR2_MAX_ALIGN: usize = 4;

/// The classic four-member `@final` ShapesDemo type.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassicShapeType {
    color: String,
    x: i32,
    y: i32,
    shapesize: i32,
}

impl DdsType for ClassicShapeType {
    /// The same type name as the five-member `@appendable` one — the name is
    /// what the peer matches on, and both IDLs call the struct `ShapeType`.
    const TYPE_NAME: &'static str = "ShapeType";
    const HAS_KEY: bool = true;
    const EXTENSIBILITY: Extensibility = Extensibility::Final;

    fn encode_key_holder_be(&self, holder: &mut zerodds_dcps::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_string(&self.color);
    }

    /// The inverse: the holder is one PLAIN_CDR2-BE `string` and no DHEADER,
    /// which `Self::decode` would read as one. DDS 1.4 §2.2.2.5.1.13 makes
    /// the key the only meaningful part of a dispose/unregister sample.
    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, Endianness::Big, XCDR2_MAX_ALIGN);
        Ok(Self {
            color: r.read_string()?,
            x: 0,
            y: 0,
            shapesize: 0,
        })
    }

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        // `@final`: no DHEADER, members tight-packed.
        let mut w = Writer::with_max_align(Endianness::Little, XCDR2_MAX_ALIGN);
        w.write_string(&self.color);
        w.write_u32(self.x as u32);
        w.write_u32(self.y as u32);
        w.write_u32(self.shapesize as u32);
        out.extend_from_slice(w.as_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, Endianness::Little, XCDR2_MAX_ALIGN);
        let color = r.read_string()?;
        let x = r.read_u32()? as i32;
        let y = r.read_u32()? as i32;
        let shapesize = r.read_u32()? as i32;
        Ok(Self {
            color,
            x,
            y,
            shapesize,
        })
    }

    /// `@final` with no eight-octet member: the XCDR1 body is the XCDR2 body.
    /// Only the encapsulation identifier differs, and that is the runtime's.
    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode(out)
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode(bytes)
    }

    fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let mut w = Writer::with_max_align(Endianness::Big, XCDR2_MAX_ALIGN);
        w.write_string(&self.color);
        w.write_u32(self.x as u32);
        w.write_u32(self.y as u32);
        w.write_u32(self.shapesize as u32);
        out.extend_from_slice(w.as_bytes());
        Ok(())
    }

    fn decode_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, Endianness::Big, XCDR2_MAX_ALIGN);
        let color = r.read_string()?;
        let x = r.read_u32()? as i32;
        let y = r.read_u32()? as i32;
        let shapesize = r.read_u32()? as i32;
        Ok(Self {
            color,
            x,
            y,
            shapesize,
        })
    }

    /// Same argument as the little-endian pair, one axis over: `@final` with no
    /// eight-octet member, so the XCDR1 big-endian body is the XCDR2 big-endian
    /// body. Without these two the type would refuse `CDR_BE` (`0x0000`), which
    /// is what a big-endian peer or `ZERODDS_WIRE_BIG_ENDIAN` puts on the wire.
    fn encode_xcdr1_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_be(out)
    }

    fn decode_xcdr1_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_be(bytes)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map_or("sub", String::as_str);
    let topic_name = args.get(2).map_or("Square", String::as_str);
    let domain_id: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let color = args.get(4).map_or("GREEN", String::as_str);
    // Seconds after which the process exits by itself, so a harness never
    // has to decide whether a silent process is working or hung.
    let run_secs: u64 = env::var("ZERODDS_RUN_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(domain_id, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<ClassicShapeType>(topic_name, TopicQos::default())?;
    let deadline = Instant::now() + Duration::from_secs(run_secs);

    match mode {
        "pub" => {
            let publisher = participant.create_publisher(PublisherQos::default());
            let writer = publisher
                .create_datawriter::<ClassicShapeType>(&topic, DataWriterQos::default())?;
            println!(
                "classic_shapes pub: Type=ShapeType(@final,4) Topic={topic_name} \
                 Color={color} Domain={domain_id} for {run_secs}s"
            );
            let matched = writer.wait_for_matched_subscription(1, Duration::from_secs(15));
            println!("  matched_subscription: {matched:?}");
            let mut sent = 0_u32;
            let mut t = 0.0_f32;
            while Instant::now() < deadline {
                let sample = ClassicShapeType {
                    color: color.to_string(),
                    x: (120.0 + 80.0 * t.sin()) as i32,
                    y: (135.0 + 90.0 * (t * 1.3).cos()) as i32,
                    shapesize: 30,
                };
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
            let reader = subscriber
                .create_datareader::<ClassicShapeType>(&topic, DataReaderQos::default())?;
            println!(
                "classic_shapes sub: Type=ShapeType(@final,4) Topic={topic_name} \
                 Domain={domain_id} for {run_secs}s"
            );
            let mut got = 0_u32;
            while Instant::now() < deadline {
                match reader.wait_for_data(Duration::from_secs(1)) {
                    Ok(()) => {
                        for s in reader.take()? {
                            got += 1;
                            println!(
                                "  <- color={:8} x={:4} y={:4} size={}",
                                s.color, s.x, s.y, s.shapesize
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
