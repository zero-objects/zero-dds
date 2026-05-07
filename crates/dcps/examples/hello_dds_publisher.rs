//! hello_dds_publisher — minimal-DDS-Publisher, der regelmaessig
//! Samples auf Topic "Chatter" schickt.
//!
//! # Usage
//!
//! ```text
//! # Terminal 1:
//! cargo run -p zerodds-dcps --example hello_dds_publisher
//! # Terminal 2 (gleicher Host, anderer Prozess):
//! cargo run -p zerodds-dcps --example hello_dds_subscriber
//! ```
//!
//! Der Publisher laeuft bis Ctrl-C. Jede Sekunde wird ein Sample
//! "hello #<counter>" an alle matched Subscriber auf Domain 0 gesendet.
//!
//! # Was hier passiert
//!
//! 1. `DomainParticipantFactory::create_participant(0, ...)` startet
//!    die `DcpsRuntime` — das sind 4 UDP-Sockets + SPDP/SEDP-Threads.
//! 2. SPDP-Beacons werden alle 5 s auf die Multicast-Gruppe 239.255.0.1
//!    gesendet; damit findet uns der Subscriber.
//! 3. `create_topic("Chatter", ...)` legt die Topic-Registry-Eintrag an.
//! 4. `create_datawriter` registriert einen User-Writer + EntityId bei
//!    der Runtime und announced ihn via SEDP.
//! 5. `writer.write(&sample)` encodiert den Sample und uebergibt ihn
//!    an den internen `ReliableWriter`, der ihn an alle matched Reader
//!    via UDP schickt.

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

    // Ein paar Sekunden warten, damit SPDP+SEDP Subscriber entdecken kann.
    // Ohne Warte-Phase gehen die ersten Samples ins Leere (noch kein Reader
    // gematched). v1.3 bringt `wait_for_matched_subscription`.
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
