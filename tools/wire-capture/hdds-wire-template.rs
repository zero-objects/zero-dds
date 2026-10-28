// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! HDDS over RTPS/UDP: a real `write` through the published runtime's
//! `DataWriter`, so what a capture holds is what the product sends.
//!
//! `wireprobe-hdds` — the in-memory probe — calls hddsgen 1.2.0's
//! generated `encode_xcdr{1,2}_le` and builds no participant. This one
//! builds one with `hdds` 1.1.2, the runtime crates.io publishes, and
//! hands it the same generated type.
//!
//! ## The adapter, and what it does not decide
//!
//! The two are versioned apart (`tools/wireprobe/hdds-shim/src/lib.rs`):
//! hddsgen 1.2.0 implements a `DDS` trait the published runtime does not
//! have, and the runtime's own `dds::DDS` asks for `encode_cdr2` and
//! `decode_cdr2`. `ZdWire` is that trait over the generated type and
//! nothing else: `encode_cdr2` is the generated `encode_xcdr2_le`,
//! `decode_cdr2` the generated `decode_xcdr2_le`, and the type
//! descriptor, the `TypeObject` and the key come from the generated
//! `DDS` implementation. No octet of the sample is written here. What
//! the runtime adds — the encapsulation it chooses from the
//! `TypeObject`, its trailing padding, its inline QoS — is what the
//! capture measures.
//!
//! The runtime's encoding entry point is XCDR2 only, so version 1 is
//! refused by the probe rather than written as 2 under that name.
//!
//! @TYPE@ is substituted by `wire-golden.sh`.

#![allow(dead_code, unused_imports, clippy::all)]

include!("generated.rs");

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The runtime's writer has no matched-count query; its listener's
/// `on_publication_matched` is how it says a reader matched.
struct ZdMatched(AtomicU32);

impl hdds_rt::dds::DataWriterListener<ZdWire> for ZdMatched {
    fn on_publication_matched(&self, status: hdds_rt::dds::PublicationMatchedStatus) {
        self.0.store(status.current_count, Ordering::SeqCst);
    }
}

struct ZdWire(@TYPE@);

impl hdds_rt::dds::DDS for ZdWire {
    fn type_descriptor() -> &'static hdds_rt::core::types::TypeDescriptor {
        <@TYPE@ as ::hdds::api::DDS>::type_descriptor()
    }

    fn encode_cdr2(&self, buf: &mut [u8]) -> hdds_rt::dds::Result<usize> {
        // `BufferTooSmall` is the one error the runtime retries on (it
        // doubles its buffer); every other answer of the generated
        // encoder is a refusal of the sample.
        self.0.encode_xcdr2_le(buf).map_err(|error| {
            if format!("{error:?}").contains("BufferTooSmall") {
                hdds_rt::dds::Error::BufferTooSmall
            } else {
                eprintln!("hdds refused the sample: {error:?}");
                hdds_rt::dds::Error::SerializationError
            }
        })
    }

    fn decode_cdr2(buf: &[u8]) -> hdds_rt::dds::Result<Self> {
        @TYPE@::decode_xcdr2_le(buf)
            .map(|(value, _length)| ZdWire(value))
            .map_err(|error| {
                eprintln!("hdds could not read a sample back: {error:?}");
                hdds_rt::dds::Error::SerializationError
            })
    }

    fn get_type_object() -> Option<hdds_rt::xtypes::CompleteTypeObject> {
        <@TYPE@ as ::hdds::api::DDS>::get_type_object()
    }

    fn compute_key(&self) -> [u8; 16] {
        <@TYPE@ as ::hdds::api::DDS>::compute_key(&self.0)
    }

    fn has_key() -> bool {
        <@TYPE@ as ::hdds::api::DDS>::has_key()
    }
}

fn zd_fill_sample() -> @TYPE@ {
    include!("fill-sample.inc")
}

fn zd_fill_poison() -> @TYPE@ {
    include!("fill-poison.inc")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        eprintln!("usage: probe <pub|sub> <domain> <1|2> <count> <topic>");
        std::process::exit(2);
    }
    let role = args[1].as_str();
    let domain: u32 = args[2].parse().expect("a domain id");
    let version = args[3].as_str();
    let count: usize = args[4].parse().expect("a sample count");
    let topic_name = args[5].as_str();
    if version != "2" {
        eprintln!("the published HDDS runtime (1.1.2) encodes XCDR2 only");
        std::process::exit(2);
    }

    // Participant ids pinned per role, so the ports are DDSI-RTPS 2.5
    // §9.6.1.1's for this domain and the unicast discovery peers
    // `wire-golden.sh` names are the other process's.
    let participant_id = if role == "pub" { 1 } else { 0 };
    let participant = match hdds_rt::Participant::builder(&format!("zdw_{role}"))
        .with_transport(hdds_rt::TransportMode::UdpMulticast)
        .domain_id(domain)
        .participant_id(Some(participant_id))
        .shm_disable()
        .build()
    {
        Ok(participant) => participant,
        Err(error) => {
            eprintln!("no participant: {error:?}");
            std::process::exit(1);
        }
    };
    let topic = match participant.topic::<ZdWire>(topic_name) {
        Ok(topic) => topic,
        Err(error) => {
            eprintln!("no topic: {error:?}");
            std::process::exit(1);
        }
    };
    let qos = hdds_rt::QoS::reliable().keep_all();

    if role == "pub" {
        let matched_status = Arc::new(ZdMatched(AtomicU32::new(0)));
        let listener: Arc<dyn hdds_rt::dds::DataWriterListener<ZdWire>> = matched_status.clone();
        let writer = match topic.writer().qos(qos).with_listener(listener).build() {
            Ok(writer) => writer,
            Err(error) => {
                eprintln!("no writer: {error:?}");
                std::process::exit(1);
            }
        };
        // A write before the reader is matched goes nowhere and the
        // capture would be short by exactly that many samples.
        let mut matched = 0;
        for _ in 0..1200 {
            matched = matched_status.0.load(Ordering::SeqCst);
            if matched > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if matched == 0 {
            eprintln!("no reader matched in 60 s");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(400));
        for i in 0..count {
            let value = if i % 2 == 0 { zd_fill_poison() } else { zd_fill_sample() };
            if let Err(error) = writer.write(&ZdWire(value)) {
                eprintln!("hdds refused the sample: {error:?}");
                std::process::exit(1);
            }
            println!("wrote {i} {}", if i % 2 == 0 { "poison" } else { "sample" });
            std::thread::sleep(Duration::from_millis(120));
        }
        std::thread::sleep(Duration::from_secs(2));
        println!("matched {matched}");
        return;
    }

    let reader = match topic.reader().qos(qos).build() {
        Ok(reader) => reader,
        Err(error) => {
            eprintln!("no reader: {error:?}");
            std::process::exit(1);
        }
    };
    let mut seen = 0;
    for _ in 0..1800 {
        loop {
            match reader.take() {
                Ok(Some(_)) => seen += 1,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("take failed: {error:?}");
                    break;
                }
            }
        }
        if seen >= count {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("read {seen}");
    std::process::exit(if seen == count { 0 } else { 1 });
}
