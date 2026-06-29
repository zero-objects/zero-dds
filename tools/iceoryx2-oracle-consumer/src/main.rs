//! External iceoryx2 oracle consumer.
//!
//! A standalone binary whose ONLY dependency is the upstream Eclipse iceoryx2
//! crate (see Cargo.toml — no `zerodds-*` anywhere). It subscribes to an
//! iceoryx2 `publish_subscribe::<[u8]>` service by verbatim name and reads the
//! raw payload, then asserts it byte-equals the expected sample. Run alongside
//! a ZeroDDS publisher (`RawIceoryx2Publisher` / the C-API `Iceoryx` delivery
//! mode), this proves ZeroDDS produces genuine iceoryx2 wire — the same service
//! variant (`ipc_threadsafe::Service`), service name and chunk payload that an
//! independent iceoryx2 application expects — rather than only interoperating
//! with itself.
//!
//! Usage:
//!   iceoryx2-oracle-consumer <service-name> <expected-hex> [timeout-ms]
//!
//! Environment:
//!   ORACLE_READY_FILE  if set, the path is created once the subscriber is live
//!                      (handshake so the publisher starts after we are ready).
//!
//! Exit code 0 on a matching sample, 1 otherwise.

use std::time::{Duration, Instant};

// Match the service variant ZeroDDS' RawIceoryx2Publisher uses
// (crates/flatdata/src/iceoryx.rs: `ipc_threadsafe::Service`). The service
// variant is part of the shared-memory identity — pub and sub must agree.
use iceoryx2::node::NodeBuilder;
use iceoryx2::service::ipc_threadsafe::Service;

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err(format!("hex string has odd length: {}", s.len()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("bad hex at {i}: {e}")))
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: {} <service-name> <expected-hex> [timeout-ms]",
            args[0]
        );
        std::process::exit(2);
    }
    let service_name = &args[1];
    let expected = match parse_hex(&args[2]) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ORACLE FAIL: {e}");
            std::process::exit(2);
        }
    };
    let timeout_ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10_000);

    if let Err(e) = run(service_name, &expected, Duration::from_millis(timeout_ms)) {
        eprintln!("ORACLE FAIL: {e}");
        std::process::exit(1);
    }
}

fn run(service_name: &str, expected: &[u8], timeout: Duration) -> Result<(), String> {
    let node = NodeBuilder::new()
        .create::<Service>()
        .map_err(|e| format!("node create: {e:?}"))?;

    let svc_id = service_name
        .try_into()
        .map_err(|e| format!("invalid service name {service_name:?}: {e:?}"))?;

    // Same messaging pattern + payload type as the ZeroDDS publisher: an
    // unsized byte slice carried in the iceoryx2 chunk.
    let factory = node
        .service_builder(&svc_id)
        .publish_subscribe::<[u8]>()
        .open_or_create()
        .map_err(|e| format!("service open_or_create: {e:?}"))?;
    let subscriber = factory
        .subscriber_builder()
        .create()
        .map_err(|e| format!("subscriber create: {e:?}"))?;

    // Event listener on the same service name — ZeroDDS' RawIceoryx2Publisher
    // notifies on every send, so we wake event-driven instead of busy-polling.
    let event = node
        .service_builder(&svc_id)
        .event()
        .open_or_create()
        .map_err(|e| format!("event open_or_create: {e:?}"))?;
    let listener = event
        .listener_builder()
        .create()
        .map_err(|e| format!("listener create: {e:?}"))?;

    // Handshake: signal readiness so the publisher only starts now that the
    // subscriber is connected (iceoryx2 has no late-joiner history by default).
    if let Ok(ready) = std::env::var("ORACLE_READY_FILE") {
        std::fs::write(&ready, b"ready").map_err(|e| format!("ready file {ready}: {e}"))?;
    }

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // Block until notified (or a short slice elapses) — no spin.
        let _ = listener.timed_wait_one(Duration::from_millis(250));
        loop {
            let sample = subscriber
                .receive()
                .map_err(|e| format!("receive: {e:?}"))?;
            let Some(sample) = sample else { break };
            let payload: &[u8] = sample.payload();
            if payload == expected {
                println!(
                    "ORACLE OK: external iceoryx2 consumer read {} bytes matching the ZeroDDS sample on service {:?}",
                    payload.len(),
                    service_name
                );
                return Ok(());
            }
            eprintln!(
                "ORACLE: received {} bytes that do not match (got {:02x?}, want {:02x?})",
                payload.len(),
                payload,
                expected
            );
        }
    }
    Err(format!(
        "timed out after {timeout:?} without a matching sample on service {service_name:?}"
    ))
}
