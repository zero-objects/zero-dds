// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: a route forwards a sample under the representation identifier it came
//! in (DDSI-RTPS 2.5 §10.5: the identifier names the format and the byte order
//! of the serialized body), and a filtering route reads a big-endian body
//! big-endian.
//!
//! A writer on the input domain sends the vendors' octets of the interop corpus
//! (`tests/interop-corpus/expected-bytes.tsv`, `struct Sample { long a;
//! double b; short c; }`, a = -1234567) under three headers; a reader on the
//! output domain takes what the router forwarded. Before, the output writer
//! framed every body under the XCDR version alone: a `D_CDR2_BE` body went out
//! as `CDR2_LE`, a `PL_CDR2_BE` one as `CDR2_LE`, and the filter read the
//! `CDR_BE` sample as a = 2032791039 and dropped it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_routing_service::{Router, RouterConfig, TypeRegistry};
use zerodds_rtps::EntityId;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::qos::ExtensibilityForRepr;

const TYPE: &str = "zerodds::RawBytes";

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// One topic of the test: the input writer's framing and what it sends.
struct Case {
    topic: &'static str,
    representation: i16,
    extensibility: ExtensibilityForRepr,
    identifier: [u8; 2],
    octets: Vec<u8>,
}

fn cases() -> [Case; 3] {
    [
        // struct-appendable, XCDR2 big endian.
        Case {
            topic: "Delimited",
            representation: 2,
            extensibility: ExtensibilityForRepr::Appendable,
            identifier: [0x00, 0x08],
            octets: hex("0000000effed2979400921fb54442d18fffe"),
        },
        // struct-mutable, XCDR2 big endian.
        Case {
            topic: "ParameterList",
            representation: 2,
            extensibility: ExtensibilityForRepr::Mutable,
            identifier: [0x00, 0x0a],
            octets: hex("0000001a20000000ffed297930000001400921fb54442d1810000002fffe"),
        },
        // struct-final, XCDR1 big endian, through the filter `a < 0`.
        Case {
            topic: "Filtered",
            representation: 0,
            extensibility: ExtensibilityForRepr::Final,
            identifier: [0x00, 0x00],
            octets: hex("ffed297900000000400921fb54442d18fffe"),
        },
    ]
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn rtps_10_5_a_forwarded_sample_keeps_the_representation_identifier_of_its_input() {
    let input = zerodds_test_domains::unique_domain();
    let output = zerodds_test_domains::unique_domain();
    let route = |name: &str, extra: &str| {
        format!(
            r#"{{ "name": "{name}", {extra}
                 "input":  {{ "domain": {input}, "topic": "{name}", "type_name": "{TYPE}" }},
                 "output": {{ "domain": {output}, "topic": "{name}", "type_name": "{TYPE}" }} }}"#
        )
    };
    let config = RouterConfig::from_json(&format!(
        r#"{{ "name": "encapsulation", "routes": [ {}, {}, {} ] }}"#,
        route("Delimited", ""),
        route("ParameterList", ""),
        route("Filtered", r#""filter": { "expression": "a < 0" },"#),
    ))
    .unwrap();
    let shapes = TypeRegistry::from_json(&format!(
        r#"[ {{ "name": "{TYPE}", "members": [ {{ "name": "a", "kind": "i32" }},
              {{ "name": "b", "kind": "f64" }}, {{ "name": "c", "kind": "i16" }} ] }} ]"#
    ))
    .unwrap();
    let router = Router::start_with_types(&config, shapes).unwrap();

    let alice = DcpsRuntime::start(
        input,
        GuidPrefix::from_bytes([0xA1; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let carol = DcpsRuntime::start(
        output,
        GuidPrefix::from_bytes([0xC3; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let mut endpoints: Vec<(EntityId, EntityId, _)> = Vec::new();
    for case in cases() {
        let writer = alice
            .register_user_writer(UserWriterConfig {
                topic_name: case.topic.into(),
                type_name: TYPE.into(),
                reliable: true,
                data_representation_offer: Some(vec![case.representation]),
                ..Default::default()
            })
            .unwrap();
        alice
            .set_user_writer_wire_extensibility(writer, case.extensibility)
            .unwrap();
        alice
            .set_user_writer_byte_order_override(writer, true)
            .unwrap();
        let (reader, rx) = carol
            .register_user_reader(UserReaderConfig {
                topic_name: case.topic.into(),
                type_name: TYPE.into(),
                reliable: true,
                data_representation_offer: Some(vec![0, 2]),
                ..Default::default()
            })
            .unwrap();
        endpoints.push((writer, reader, rx));
    }
    for (writer, reader, _) in &endpoints {
        wait_until("the route's endpoints did not match", || {
            alice.user_writer_matched_reader_guids(*writer).len() == 1
                && carol.user_reader_matched_writer_guids(*reader).len() == 1
        });
    }

    for (case, (writer, _, rx)) in cases().iter().zip(&endpoints) {
        alice
            .write_user_sample(*writer, case.octets.clone())
            .unwrap();
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(UserSample::Alive {
                payload,
                representation_identifier,
                ..
            }) => {
                assert_eq!(
                    representation_identifier, case.identifier,
                    "{}: the output header names the input's format and byte order",
                    case.topic
                );
                assert_eq!(payload.as_ref(), case.octets.as_slice(), "{}", case.topic);
            }
            other => panic!("{}: {other:?}", case.topic),
        }
    }
    for name in ["Delimited", "ParameterList", "Filtered"] {
        let metrics = router.route_metrics(name).unwrap();
        assert_eq!((metrics.errors, metrics.dropped_filter), (0, 0), "{name}");
    }
    alice.shutdown();
    carol.shutdown();
}
