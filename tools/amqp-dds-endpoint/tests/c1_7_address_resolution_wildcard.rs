#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.7 — Address-Resolution Wildcard.
//!
//! Spec §C.1.7: ein attached link mit Wildcard-Adresse SHALL
//! Transfers von allen matchenden Topics empfangen.

mod common;

use zerodds_amqp_endpoint::routing::{AddressResolution, AddressRouter};

#[test]
fn c1_7_prefix_wildcard_matches_multiple_topics() {
    let mut router = AddressRouter::new();
    router.add_route(
        "sensor.*",
        AddressResolution {
            topic: "AllSensors".into(),
            domain_id: 0,
            partitions: vec![],
        },
    );
    assert_eq!(
        router.resolve("sensor.temperature").unwrap().topic,
        "AllSensors"
    );
    assert_eq!(
        router.resolve("sensor.humidity").unwrap().topic,
        "AllSensors"
    );
    // Bare-Adresse, die nicht auf Pattern matched, wird als
    // direkter Topic-Name interpretiert (Spec §7.3 Fallback).
    assert_eq!(router.resolve("actuator.x").unwrap().topic, "actuator.x");
}

#[test]
fn c1_7_suffix_wildcard_matches() {
    let mut router = AddressRouter::new();
    router.add_route(
        "*.cmd",
        AddressResolution {
            topic: "Commands".into(),
            domain_id: 0,
            partitions: vec![],
        },
    );
    assert_eq!(router.resolve("motor.cmd").unwrap().topic, "Commands");
    assert_eq!(router.resolve("light.cmd").unwrap().topic, "Commands");
}

#[test]
fn c1_7_global_wildcard_matches_anything() {
    let mut router = AddressRouter::new();
    router.add_route(
        "*",
        AddressResolution {
            topic: "Catchall".into(),
            domain_id: 0,
            partitions: vec![],
        },
    );
    assert_eq!(router.resolve("any.topic").unwrap().topic, "Catchall");
    assert_eq!(router.resolve("foo").unwrap().topic, "Catchall");
}

#[test]
fn c1_7_static_alias_takes_precedence_over_wildcard() {
    // Spec §7.3: Static aliases werden zuerst probiert.
    let mut router = AddressRouter::new();
    router.add_route(
        "specific",
        AddressResolution {
            topic: "ExactTopic".into(),
            domain_id: 5,
            partitions: vec![],
        },
    );
    router.add_route(
        "*",
        AddressResolution {
            topic: "Catchall".into(),
            domain_id: 0,
            partitions: vec![],
        },
    );
    let r = router.resolve("specific").unwrap();
    assert_eq!(r.topic, "ExactTopic");
    assert_eq!(r.domain_id, 5);
}
