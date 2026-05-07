//! WP QoS-Wiring T4 — DestinationOrder QoS Tests.
//!
//! Spec DDS 1.4 §2.2.3.18: bei BY_SOURCE_TIMESTAMP liefert der Reader
//! nur Samples mit strikt groesserem source_ts; aeltere Samples werden
//! verworfen (Out-of-Order-Resolution). Bei BY_RECEPTION_TIMESTAMP
//! (Default) keine Filterung — alles wird angenommen.
//!
//! Diese Tests pruefen den Tracker-Hook direkt; das Reader-Pipeline-
//! Wiring auf den Hook ist im Subscriber-Code (siehe
//! `ingest_into_cache::should_deliver_under_destination_order`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::panic
)]

use zerodds_dcps::Time;
use zerodds_dcps::instance_tracker::{InstanceTracker, KeyHash};

fn kh(b: u8) -> KeyHash {
    [b; 16]
}

#[test]
fn by_reception_timestamp_always_delivers() {
    let t = InstanceTracker::new();
    let key = kh(1);
    t.observe_sample(key, vec![1, 2, 3, 4], Some(Time::new(10, 0)));
    t.record_delivery(&key, Time::new(10, 0));

    // Auch ein "veralteter" source_ts darf bei BY_RECEPTION durch.
    assert!(t.should_deliver_under_destination_order(&key, Time::new(5, 0), false));
    assert!(t.should_deliver_under_destination_order(&key, Time::new(15, 0), false));
}

#[test]
fn by_source_timestamp_drops_older_samples() {
    let t = InstanceTracker::new();
    let key = kh(2);
    t.observe_sample(key, vec![1, 2, 3, 4], Some(Time::new(10, 500_000_000)));
    t.record_delivery(&key, Time::new(10, 500_000_000));

    // Aelteres Sample → drop.
    assert!(!t.should_deliver_under_destination_order(&key, Time::new(10, 0), true));
    assert!(!t.should_deliver_under_destination_order(&key, Time::new(5, 999_999_999), true));
    // Gleicher Timestamp → drop (strict-greater Spec §2.2.3.18 simplified
    // ohne GUID-Tie-Breaker).
    assert!(!t.should_deliver_under_destination_order(&key, Time::new(10, 500_000_000), true));
}

#[test]
fn by_source_timestamp_passes_newer_samples() {
    let t = InstanceTracker::new();
    let key = kh(3);
    t.observe_sample(key, vec![1, 2, 3, 4], Some(Time::new(10, 0)));
    t.record_delivery(&key, Time::new(10, 0));

    assert!(t.should_deliver_under_destination_order(&key, Time::new(10, 1), true));
    assert!(t.should_deliver_under_destination_order(&key, Time::new(11, 0), true));
}

#[test]
fn unknown_instance_passes_first_sample() {
    let t = InstanceTracker::new();
    let key = kh(4);
    // Tracker kennt die Instanz noch nicht → erstes Sample muss durch.
    assert!(t.should_deliver_under_destination_order(&key, Time::new(0, 0), true));
    assert!(t.should_deliver_under_destination_order(&key, Time::new(0, 0), false));
}

#[test]
fn known_instance_no_prior_delivery_passes() {
    let t = InstanceTracker::new();
    let key = kh(5);
    // Instance registriert via observe_sample, aber kein record_delivery
    // → kein last_delivered_ts → erstes Delivery muss durch.
    t.observe_sample(key, vec![1, 2, 3, 4], Some(Time::new(10, 0)));
    assert!(t.should_deliver_under_destination_order(&key, Time::new(5, 0), true));
}

#[test]
fn destination_order_is_per_instance() {
    let t = InstanceTracker::new();
    let a = kh(10);
    let b = kh(11);
    t.observe_sample(a, vec![1, 2, 3, 4], Some(Time::new(100, 0)));
    t.record_delivery(&a, Time::new(100, 0));
    // Andere Instanz → unbeeinflusst, alter Timestamp passt durch.
    assert!(t.should_deliver_under_destination_order(&b, Time::new(50, 0), true));
}
