// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-security-main` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! The tool depends on `zerodds-dcps` with the feature `security`, so this
//! target builds with it as well.
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`; `src/shape_type.rs`
//! names `DdsType` for the generated `ShapeType`):
//! - `DomainParticipantQos { property, ..Default }` with the `dds.sec.*`
//!   list its flags build, and `DomainParticipantFactory::create(domain)
//!   .with_qos(qos)`, `.with_security(bundle)` for `-logging` (a
//!   logger-only `SecurityBundle`), `.build()`.
//! - `create_topic::<ShapeType>` once for publish and subscribe on the same
//!   name (the topic is cloned for the reader), `create_publisher` /
//!   `create_subscriber` with `PublisherQos`/`SubscriberQos { partition,
//!   ..Default }` (`PartitionQosPolicy::default()` without `-partition`),
//!   `create_datawriter` / `create_datareader` with the default QoS and
//!   `liveliness.lease_duration` from `-livelinessPeriod`,
//!   `DataWriter::write`, `drive_listeners`, `DataReader::take_with_info`
//!   (`info.valid_data`, `data`).
//!
//! What it relies on and is checked here:
//! - The participant QoS carries the property list as given; with no
//!   property it asks for no security. Oracles: DDS-Security 1.2 §7.3.21
//!   (`DomainParticipantQos` extended with `property`), Table 50
//!   (`dds.sec.auth.*`) and Table 63 (`dds.sec.access.*`) for the names;
//!   the rustdoc of the participant's property path in `participant.rs`
//!   ("No `dds.sec.auth.*` / `dds.sec.access.*` property — the QoS does not
//!   ask for security").
//! - A partition and a liveliness lease reach the endpoints, the reader on
//!   the cloned topic is created, a written sample is accepted, a reader
//!   that received nothing takes nothing. Oracles: DDS 1.4 §2.2.3, the QoS
//!   table (PARTITION: the default is the empty sequence; LIVELINESS: the
//!   default lease is infinite), §2.2.2.4.1.5 and §2.2.2.5.2.5.
//!
//! `build()` opens sockets and, with a property list, reads the files it
//! names; the builder chain is checked at compile time.
#![allow(clippy::unwrap_used)]

use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, PartitionQosPolicy, PropertyQosPolicy, PublisherQos,
    SubscriberQos,
};
use zerodds_dcps::{
    DomainParticipant, DomainParticipantFactory, DomainParticipantQos, Sample, Topic, TopicQos,
};
use zerodds_qos::Duration;
use zerodds_security_runtime::SecurityBundle;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-security.rs"));
}

use generated::ShapeType;

#[test]
fn security_7_3_21_the_participant_qos_carries_the_property_list() {
    assert!(
        DomainParticipantQos::default()
            .property
            .iter()
            .next()
            .is_none(),
        "no property: no security asked for"
    );
    let entries = [
        ("dds.sec.auth.identity_ca", "file:../certs/identity_ca.pem"),
        (
            "dds.sec.auth.identity_certificate",
            "file:../certs/id_cert.pem",
        ),
        (
            "dds.sec.auth.private_key",
            "file:../certs/private/id_key.pem",
        ),
        (
            "dds.sec.access.permissions_ca",
            "file:../certs/permissions_ca.pem",
        ),
        ("dds.sec.access.governance", "file:../governance.p7s"),
        ("dds.sec.access.permissions", "file:../permissions.p7s"),
    ];
    let mut property = PropertyQosPolicy::default();
    for (name, value) in entries {
        property.set(name, value);
    }
    let qos = DomainParticipantQos {
        property,
        ..DomainParticipantQos::default()
    };
    // Without a runtime the list is not read; it is kept as given.
    let participant =
        DomainParticipantFactory::instance().create_participant_offline(0, qos.clone());
    let kept = participant.qos().property;
    for (name, value) in entries {
        assert_eq!(kept.get(name), Some(value), "{name}");
    }
    assert_eq!(kept.iter().count(), entries.len());
    assert_eq!(participant.qos(), qos);
}

#[test]
fn dcps_2_2_3_a_partition_and_a_lease_reach_the_endpoints() {
    assert!(PartitionQosPolicy::default().names.is_empty());
    assert_eq!(
        DataWriterQos::default().liveliness.lease_duration,
        Duration::INFINITE
    );
    let participant: DomainParticipant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic: Topic<ShapeType> = participant
        .create_topic::<ShapeType>("contract-dcps-Square", TopicQos::default())
        .unwrap();
    let partition = PartitionQosPolicy {
        names: vec!["A".to_string()],
    };
    let publisher = participant.create_publisher(PublisherQos {
        partition: partition.clone(),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        partition,
        ..SubscriberQos::default()
    });
    let mut writer_qos = DataWriterQos::default();
    writer_qos.liveliness.lease_duration = Duration::from_secs(2);
    let mut reader_qos = DataReaderQos::default();
    reader_qos.liveliness.lease_duration = Duration::from_secs(2);

    let writer = publisher
        .create_datawriter::<ShapeType>(&topic, writer_qos)
        .unwrap();
    let reader = subscriber
        .create_datareader::<ShapeType>(&topic.clone(), reader_qos)
        .unwrap();
    writer
        .write(&ShapeType {
            color: "BLUE".to_string(),
            x: 10,
            y: 20,
            shapesize: 30,
        })
        .unwrap();
    writer.drive_listeners();
    reader.drive_listeners();
    let samples: Vec<Sample<ShapeType>> = reader.take_with_info().unwrap();
    assert!(samples.iter().filter_map(printed).next().is_none());
    assert!(samples.is_empty(), "nothing was delivered to it");
}

/// What the tool prints of a received sample; a dispose or unregister
/// notification (`valid_data == false`) is not counted.
fn printed(s: &Sample<ShapeType>) -> Option<(&str, i32, i32, i32)> {
    if !s.info.valid_data {
        return None;
    }
    Some((s.data.color.as_str(), s.data.x, s.data.y, s.data.shapesize))
}

#[test]
fn the_participant_builder_keeps_the_chain_the_tool_uses() {
    let _ = |qos: DomainParticipantQos, bundle: SecurityBundle| {
        DomainParticipantFactory::create(0)
            .with_qos(qos)
            .with_security(bundle)
            .build()
    };
    let _ = || SecurityBundle::builder().build();
}
