// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A dispose forwarded by the routing service, read by a DDS reader of
//! another vendor (D10).
//!
//! A config route registers its endpoints without a TypeIdentifier and
//! forwards a dispose by its key hash: without a type the output marker
//! carries `PID_KEY_HASH` alone, which Cyclone 11.0.1 and OpenDDS 3.35 do
//! not act on; Fast DDS 3.6.1 and OpenSplice 6.9 dispose the instance they
//! know by that hash (measured with this test). With `type_file` on the
//! output endpoint the
//! output writer reads instance keys with it — each forwarded sample carries
//! its KeyHash and the writer records the instance's key — and the marker
//! carries the `serializedKey`.
//!
//! A ZeroDDS writer on the input domain writes instances 7..=10 of
//! `KeyedPointA`, then disposes 7, unregisters 8, unregisters and disposes 9
//! and disposes 10; a reader of another vendor on the output domain reports
//! the last state per instance. Both routes run: with `type_file` the
//! states are asserted, without it they are printed.
//!
//! Live against another vendor, so `#[ignore]`: the Cyclone reader needs
//! python3 with the cyclonedds binding, the other test the reader
//! `ZERODDS_DISPOSE_READER` names (`tools/dispose-reader/build.sh`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_routing_service::{Router, RouterConfig};
use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_test_vendor_reader::{
    VendorReader, keyed_point_a, keyed_point_a_key_hash, keyed_point_a_type_object, reader_command,
};

fn config(input: i32, output: i32, type_file: Option<&std::path::Path>) -> String {
    let type_file = type_file.map_or_else(String::new, |p| {
        format!(r#", "type_file": "{}""#, p.display())
    });
    format!(
        r#"{{
  "name": "dispose-foreign",
  "routes": [
    {{
      "name": "keyed",
      "input":  {{ "domain": {input}, "topic": "RoutedPoint", "type_name": "KeyedPointA", "keyed": true }},
      "output": {{ "domain": {output}, "topic": "RoutedPoint", "type_name": "KeyedPointA", "keyed": true{type_file} }}
    }}
  ]
}}"#
    )
}

/// The states the reader `which` reports after the routed lifecycle calls,
/// the input writer alive.
fn route_and_dispose(which: &str, with_type_file: bool) -> BTreeMap<String, String> {
    let input_lease = zerodds_test_vendor_reader::lease_domain();
    let output_lease = zerodds_test_vendor_reader::lease_domain();
    let (input, output) = (input_lease.id(), output_lease.id());
    let type_object = keyed_point_a_type_object();
    let type_path = std::env::temp_dir().join(format!(
        "zerodds-routing-keyed-point-a-{}.typeobject",
        std::process::id()
    ));
    std::fs::write(&type_path, &type_object).unwrap();
    let cfg = RouterConfig::from_json(&config(
        input,
        output,
        with_type_file.then_some(type_path.as_path()),
    ))
    .unwrap();
    let router = Router::start(&cfg).unwrap();

    let (command, label) = reader_command(which);
    let mut reader = VendorReader::start(command, &label, output, "RoutedPoint");

    let object = zerodds_types::type_object::TypeObject::from_bytes_le(&type_object).unwrap();
    let hash = zerodds_types::compute_hash(&object).unwrap();
    let rt = DcpsRuntime::start(
        input,
        GuidPrefix::from_bytes([0x5c; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    rt.register_type_object(object).unwrap();
    let eid = rt
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "RoutedPoint".into(),
                type_name: "KeyedPointA".into(),
                type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
                ..Default::default()
            },
            true,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while rt.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the router's input did not match"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    reader.wait_matched();

    // Written until the reader has every instance: the reader reports a
    // match once it knows the router's writer, and a sample the writer
    // sends before it knows the reader is lost (VOLATILE).
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for id in [7, 8, 9, 10] {
            rt.write_user_sample_keyed(
                eid,
                &keyed_point_a(id, 100 + id),
                keyed_point_a_key_hash(id),
                None,
            )
            .unwrap();
        }
        if reader.wait_seen(&[7, 8, 9, 10], Duration::from_secs(1)) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the routed samples did not reach the reader"
        );
    }
    for (id, bits) in [
        (7, DISPOSED),
        (8, UNREGISTERED),
        (9, DISPOSED | UNREGISTERED),
        (10, DISPOSED),
    ] {
        rt.write_user_lifecycle(eid, keyed_point_a_key_hash(id), None, bits, None)
            .unwrap();
    }
    std::thread::sleep(Duration::from_millis(1500));
    let states = reader.snapshot();
    rt.shutdown();
    drop(router);
    let _ = reader.finish();
    let _ = std::fs::remove_file(&type_path);
    println!(
        "{label}, route {} type_file: {states:?}",
        if with_type_file { "with" } else { "without" }
    );
    states
}

fn assert_routed_states(label: &str, states: &BTreeMap<String, String>) {
    for (id, want) in [
        ("7", "NOT_ALIVE_DISPOSED"),
        ("8", "NOT_ALIVE_NO_WRITERS"),
        ("9", "NOT_ALIVE_DISPOSED"),
        ("10", "NOT_ALIVE_DISPOSED"),
    ] {
        assert_eq!(
            states.get(id).map(String::as_str),
            Some(want),
            "instance {id} at the {label} reader behind the route (all: {states:?})"
        );
    }
}

#[test]
#[ignore = "live against Eclipse Cyclone DDS: needs python3 with the cyclonedds binding; run with --ignored"]
fn a_routed_dispose_reaches_a_cyclone_reader() {
    let _ = route_and_dispose("cyclone", false);
    let states = route_and_dispose("cyclone", true);
    assert_routed_states("cyclone", &states);
}

#[test]
#[ignore = "live against another vendor: set ZERODDS_DISPOSE_READER (tools/dispose-reader/build.sh); run with --ignored"]
fn a_routed_dispose_reaches_a_foreign_reader() {
    let _ = route_and_dispose("env", false);
    let states = route_and_dispose("env", true);
    assert_routed_states("foreign", &states);
}
