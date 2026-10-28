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

/// Resolves the DDS domain from an optional CLI argument. Omitted -> the
/// documented manual default 100; present but malformed / out of 0..=232 ->
/// a diagnostic and a non-zero exit (never a silent fallback).
fn resolve_domain(arg: Option<String>) -> i32 {
    match arg {
        None => 100,
        Some(s) => match s.parse::<i32>() {
            Ok(d) if (0..=232).contains(&d) => d,
            _ => {
                eprintln!(
                    "invalid domain argument {s:?}: expected an integer in 0..=232"
                );
                std::process::exit(2);
            }
        },
    }
}

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let domain: i32 = resolve_domain(std::env::args().nth(2));
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
    let mut failed: u64 = 0;
    while start.elapsed().as_secs() < secs {
        // A discarded `write()` error is why the reverse leg looked like a
        // matched pair that delivers nothing: the encoder refused the sample and
        // the writer kept going in silence. The cell is only readable if the
        // refusal reaches the log.
        if let Err(e) = w.write(&Robot {
            id: 1,
            label: c % 1000,
        }) {
            failed += 1;
            if failed <= 3 {
                eprintln!("[zerodds writer] write error: {e:?}");
            }
        }
        c += 1;
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    eprintln!("[zerodds writer] written={c} failed={failed}");
}
