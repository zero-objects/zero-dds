//! ZeroDDS writer for the reverse-direction leg of the #27 interop matrix
//! (ZeroDDS writer -> CycloneDDS reader). Writes topic `robot` on domain 100
//! for a fixed window. Extensibility follows the generated `src/robot.rs`.
#![allow(clippy::unwrap_used, clippy::print_stderr)]

#[path = "robot.rs"]
mod robot;
use robot::Robot;
use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
};

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    // Configurable domain (ZERODDS_DOMAIN), default 100 — see reader main.rs.
    let domain: i32 = std::env::var("ZERODDS_DOMAIN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let f = DomainParticipantFactory::instance();
    let p = f
        .create_participant(domain, DomainParticipantQos::default())
        .unwrap();
    let t = p
        .create_topic::<Robot>("robot", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let w = pubr
        .create_datawriter::<Robot>(&t, DataWriterQos::default())
        .expect("writer");
    eprintln!("[zerodds writer] domain={domain} topic=robot window={secs}s");
    let start = std::time::Instant::now();
    let mut c: u32 = 0;
    while start.elapsed().as_secs() < secs {
        let _ = w.write(&Robot {
            id: 1,
            label: c % 1000,
        });
        c += 1;
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}
