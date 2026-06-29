//! ZeroDDS side of the external-iceoryx2 oracle proof.
//!
//! (Oracle proof binary — `expect`/`println!` are intentional here.)
#![allow(clippy::expect_used, clippy::print_stdout)]
//!
//! Publishes a fixed byte payload over iceoryx2 shared memory using ZeroDDS'
//! `RawIceoryx2Publisher`, on a verbatim service name. The counterpart
//! `iceoryx2-oracle-consumer` binary — which depends only on the upstream
//! iceoryx2 crate, with zero ZeroDDS code — subscribes to the same name and
//! asserts it reads these exact bytes. Together they prove ZeroDDS emits real
//! iceoryx2 wire that an independent iceoryx2 application consumes.
//!
//! Usage: iceoryx2_oracle_publisher <service-name> <payload-hex> [count] [gap-ms]
//!
//! Build/run requires the `iceoryx2-bridge` feature:
//!   cargo run -p zerodds-flatdata --features iceoryx2-bridge \
//!     --example iceoryx2_oracle_publisher -- my_svc 010203040506 50 50

use std::time::Duration;

use zerodds_flatdata::RawIceoryx2Publisher;

fn parse_hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex string has odd length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() >= 3,
        "usage: {} <service-name> <payload-hex> [count] [gap-ms]",
        args[0]
    );
    let service_name = &args[1];
    let payload = parse_hex(&args[2]);
    let count: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(50);
    let gap_ms: u64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(50);

    let publisher = RawIceoryx2Publisher::create(service_name, payload.len())
        .expect("create iceoryx2 publisher");

    // Repeat: the external subscriber may connect a touch after us, and
    // iceoryx2 keeps no late-joiner history by default. Each send also pings
    // the event notifier so the consumer wakes promptly.
    for _ in 0..count {
        publisher.send(&payload).expect("send sample");
        std::thread::sleep(Duration::from_millis(gap_ms));
    }

    println!(
        "published {} bytes x{} on iceoryx2 service {:?}",
        payload.len(),
        count,
        service_name
    );
}
