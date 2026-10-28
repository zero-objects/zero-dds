// SPDX-License-Identifier: Apache-2.0
//! `typelookup-probe <writer|reader> <topic> <seconds> <domain>`: one
//! endpoint of `RoundtripBench::Roundtrip` (@appendable, see
//! `roundtrip_probe.idl`) on `topic`. Prints
//! `RESULT discovered=.. matched=.. samples=..`.
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::print_stderr)]

#[allow(non_snake_case, dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/roundtrip_probe.rs"));
}
use generated::RoundtripBench::Roundtrip;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    SubscriberQos, TopicQos,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let role = args.get(1).map(String::as_str).unwrap_or("reader");
    let topic_name = args.get(2).cloned().unwrap_or_else(|| "RoundtripBench_Request".into());
    let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(12);
    let domain: i32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);
    let p = DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .unwrap();
    let t = p
        .create_topic::<Roundtrip>(&topic_name, TopicQos::default())
        .expect("topic");
    eprintln!("[probe] role={role} topic={topic_name} domain={domain} window={secs}s");
    let start = std::time::Instant::now();
    let (mut discovered, mut matched, mut samples) = (0usize, 0usize, 0u64);
    if role == "writer" {
        let w = p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<Roundtrip>(&t, DataWriterQos::default())
            .expect("writer");
        let mut n = 0u32;
        while start.elapsed().as_secs() < secs {
            discovered = discovered.max(p.discovered_participants_count());
            matched = matched.max(w.matched_subscription_count());
            let sample = Roundtrip {
                sequence_id: n,
                ..Roundtrip::default()
            };
            if w.write(&sample).is_ok() {
                samples += 1;
            }
            n += 1;
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    } else {
        let r = p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<Roundtrip>(&t, DataReaderQos::default())
            .expect("reader");
        while start.elapsed().as_secs() < secs {
            discovered = discovered.max(p.discovered_participants_count());
            matched = matched.max(r.matched_publication_count());
            if let Ok(v) = r.take() {
                samples += v.len() as u64;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    println!("RESULT discovered={discovered} matched={matched} samples={samples}");
}
