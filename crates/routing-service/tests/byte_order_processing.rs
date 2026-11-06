// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A route's filter and transform read a sample in the byte order and the
//! format its encapsulation header names (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3
//! §7.4.1, §7.4.3.4), and write a transformed body in the input's order.
//!
//! The oracle is the vendors' octets of the interop corpus cases
//! `struct-final` and `struct-appendable` (`struct Sample { long a; double b;
//! short c; }` with a = -1234567, b = pi, c = -2) in
//! `tests/interop-corpus/expected-bytes.tsv`: the octets Cyclone, RTI,
//! Fast DDS and OpenDDS write in each of the cells used here. Before, the
//! router read every body little-endian: under `CDR_BE` the filter saw
//! a = 2032791039 and dropped the sample.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_routing_service::forwarding::{Processed, SampleProcessor};
use zerodds_routing_service::transform::DynamicProcessor;
use zerodds_routing_service::{RouterConfig, TypeRegistry};

const CDR_BE: [u8; 2] = [0x00, 0x00];
const CDR_LE: [u8; 2] = [0x00, 0x01];
const PL_CDR_LE: [u8; 2] = [0x00, 0x03];
const CDR2_BE: [u8; 2] = [0x00, 0x06];
const CDR2_LE: [u8; 2] = [0x00, 0x07];
const D_CDR2_BE: [u8; 2] = [0x00, 0x08];
const D_CDR2_LE: [u8; 2] = [0x00, 0x09];
const PL_CDR2_BE: [u8; 2] = [0x00, 0x0a];

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// `struct-final`: (identifier, the vendors' octets).
fn final_cells() -> [([u8; 2], Vec<u8>); 4] {
    [
        (CDR_BE, hex("ffed297900000000400921fb54442d18fffe")),
        (CDR_LE, hex("7929edff00000000182d4454fb210940feff")),
        (CDR2_BE, hex("ffed2979400921fb54442d18fffe")),
        (CDR2_LE, hex("7929edff182d4454fb210940feff")),
    ]
}

/// `struct-appendable` under XCDR2: (identifier, the vendors' octets).
fn appendable_cells() -> [([u8; 2], Vec<u8>); 2] {
    [
        (D_CDR2_BE, hex("0000000effed2979400921fb54442d18fffe")),
        (D_CDR2_LE, hex("0e0000007929edff182d4454fb210940feff")),
    ]
}

fn shapes() -> TypeRegistry {
    let members = r#"[ { "name": "a", "kind": "i32" }, { "name": "b", "kind": "f64" },
                       { "name": "c", "kind": "i16" } ]"#;
    TypeRegistry::from_json(&format!(
        r#"[ {{ "name": "Final", "members": {members} }},
             {{ "name": "Appendable", "members": {members}, "appendable": true }} ]"#
    ))
    .unwrap()
}

/// A route over `type_name` with `rules` (the `filter` and `transform`
/// members of a route, as JSON).
fn processor(type_name: &str, rules: &str) -> DynamicProcessor {
    let config = RouterConfig::from_json(&format!(
        r#"{{ "name": "p", "routes": [ {{
              "name": "r",
              "input":  {{ "domain": 0, "topic": "A", "type_name": "{type_name}" }},
              "output": {{ "domain": 1, "topic": "A", "type_name": "{type_name}" }},
              {rules} }} ] }}"#
    ))
    .unwrap();
    DynamicProcessor::build(&config.routes[0], type_name, type_name, &shapes()).unwrap()
}

#[test]
fn xtypes_7_4_1_a_filter_reads_the_body_in_the_byte_order_of_its_header() {
    let mut matching = processor("Final", r#""filter": { "expression": "a < 0" }"#);
    let mut other = processor("Final", r#""filter": { "expression": "a > 0" }"#);
    for (identifier, octets) in final_cells() {
        assert_eq!(
            matching.process(&octets, identifier),
            Processed::Forward(octets.clone(), identifier),
            "{identifier:02x?}: forwarded as it came"
        );
        assert_eq!(
            other.process(&octets, identifier),
            Processed::Filtered,
            "{identifier:02x?}"
        );
    }
    let mut appendable = processor("Appendable", r#""filter": { "expression": "c < 0" }"#);
    for (identifier, octets) in appendable_cells() {
        assert_eq!(
            appendable.process(&octets, identifier),
            Processed::Forward(octets.clone(), identifier),
            "{identifier:02x?}"
        );
    }
}

#[test]
fn xtypes_7_4_1_a_transformed_body_is_written_in_the_byte_order_it_came_in() {
    // The constant is the value c already has: the transform re-encodes the
    // sample, and the output has to be the vendors' octets again.
    let transform = r#""transform": { "set_const": [ { "field": "c", "value": "-2" } ] }"#;
    let mut plain = processor("Final", transform);
    for (identifier, octets) in final_cells() {
        assert_eq!(
            plain.process(&octets, identifier),
            Processed::Forward(octets.clone(), identifier),
            "{identifier:02x?}"
        );
    }
    let mut delimited = processor("Appendable", transform);
    for (identifier, octets) in appendable_cells() {
        assert_eq!(
            delimited.process(&octets, identifier),
            Processed::Forward(octets.clone(), identifier),
            "{identifier:02x?}"
        );
    }
}

#[test]
fn xtypes_7_4_3_4_a_parameter_list_body_is_refused_and_not_read_as_plain() {
    // A @mutable type's body: no flat shape describes it.
    let mut filter = processor("Final", r#""filter": { "expression": "a > 0" }"#);
    for identifier in [PL_CDR_LE, PL_CDR2_BE] {
        let (_, octets) = &final_cells()[0];
        assert!(
            matches!(filter.process(octets, identifier), Processed::Refused(_)),
            "{identifier:02x?}"
        );
    }
}

#[test]
fn xtypes_7_4_3_4_2_a_header_that_disagrees_with_the_shape_is_refused() {
    // XCDR2 puts a DHEADER in front of an @appendable body only: read with
    // the other shape, every member sits four octets off.
    let filter = r#""filter": { "expression": "a > 0" }"#;
    let (identifier, octets) = &appendable_cells()[0];
    assert!(matches!(
        processor("Final", filter).process(octets, *identifier),
        Processed::Refused(_)
    ));
    let (identifier, octets) = &final_cells()[2];
    assert!(matches!(
        processor("Appendable", filter).process(octets, *identifier),
        Processed::Refused(_)
    ));
}
