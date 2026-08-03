//! ZeroDDS reader for the CycloneDDS XTypes interop matrix (#27).
//!
//! Reads topic `robot` on domain 100 for a fixed window and reports three
//! counters SEPARATELY, so a match without data (the #27 symptom) is visible:
//!   * `matched`  — highest matched_publication_count seen (0 or 1)
//!   * `samples`  — successfully decoded samples (`take()` Ok)
//!   * errors     — `take()` calls that returned WireError (decode failures)
//!
//! The reader's extensibility is whatever `src/robot.rs` was generated with
//! (`run_matrix.sh` regenerates it per case: default `@appendable`, or `@final`
//! via `zerodds-idlc --cyclone`). Output line: `RESULT matched=.. samples=.. errors=..`.
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::print_stderr)]

#[path = "robot.rs"]
mod robot;
use robot::Robot;
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
};

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let f = DomainParticipantFactory::instance();
    let p = f
        .create_participant(100, DomainParticipantQos::default())
        .unwrap();
    let t = p
        .create_topic::<Robot>("robot", TopicQos::default())
        .expect("topic");
    let s = p.create_subscriber(SubscriberQos::default());
    let r = s
        .create_datareader::<Robot>(&t, DataReaderQos::default())
        .expect("reader");
    eprintln!("[zerodds reader] domain=100 topic=robot window={secs}s");

    let start = std::time::Instant::now();
    let mut matched = 0usize;
    let mut discovered = 0usize;
    let mut samples = 0u64;
    let mut errors = 0u64;
    while start.elapsed().as_secs() < secs {
        matched = matched.max(r.matched_publication_count());
        discovered = discovered.max(p.discovered_participants_count());
        match r.take() {
            Ok(v) => samples += v.len() as u64,
            Err(e) => {
                errors += 1;
                if errors <= 3 {
                    eprintln!("[zerodds reader] take error: {e:?}");
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("RESULT discovered={discovered} matched={matched} samples={samples} errors={errors}");
}
