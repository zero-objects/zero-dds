// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-main` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps`:
//! - `src/main.rs`: `DomainParticipantFactory::instance().create_participant`
//!   with `DomainParticipantQos::default()`.
//! - `src/shape_type.rs`: per run one of the 25 generated types (each a
//!   `DdsType` with `TYPE_NAME`, `EXTENSIBILITY`, `HAS_KEY`,
//!   `TYPE_IDENTIFIER`, `type_object()`); `create_publisher` /
//!   `create_subscriber` with their default QoS, `create_topic::<T>`,
//!   `Topic::set_listener` with `psm_constants::status::ANY`,
//!   `create_datawriter` / `create_datareader` with the default QoS,
//!   `DataReader::set_listener`, `DataWriter::write`,
//!   `DataReader::take_with_info` (`info.valid_data`, `data`),
//!   `drive_listeners`, `Topic::inconsistent_topic_status`, `Result`,
//!   `Extensibility`.
//! - `src/listeners.rs`: `listener::TopicListener::on_inconsistent_topic`,
//!   `listener::DataReaderListener::on_requested_incompatible_qos` and
//!   `on_subscription_matched`, with `status::{InconsistentTopicStatus,
//!   RequestedIncompatibleQosStatus, SubscriptionMatchedStatus}` and
//!   `InstanceHandle`.
//!
//! What it relies on and is checked here:
//! - Each of the 25 types becomes a topic with a writer and, on another
//!   participant, a reader, through the tool's own `create_writer` and
//!   `create_reader`; a written sample is accepted, a reader that received
//!   nothing takes nothing, the status calls return. Oracles: the suite's
//!   `ShapeTypeVariants::create_writer` / `create_reader`
//!   (`srcCxx/ShapeType_variants.h`, one type per run), DDS 1.4
//!   §2.2.2.2.1.5 (`create_topic`), §2.2.2.4.1.5 and §2.2.2.5.2.5 (an entity
//!   with the default QoS is created), §2.2.2.5.3.8 (`take` returns what is
//!   available, here nothing).
//! - The mask `status::ANY` holds the three statuses the suite arms
//!   (INCONSISTENT_TOPIC, REQUESTED_INCOMPATIBLE_QOS, SUBSCRIPTION_MATCHED),
//!   with the values of DDS 1.4 §2.3.3.
//!
//! `create_participant` opens sockets; the participants here have no
//! runtime.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use zerodds_dcps::psm_constants::status;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_omg_xtypes_main::shape_type::{VARIANT_NAMES, create_reader, create_writer};

#[test]
fn every_variant_gets_its_topic_writer_and_reader() {
    let factory = DomainParticipantFactory::instance();
    let publishing = factory.create_participant_offline(0, DomainParticipantQos::default());
    let subscribing = factory.create_participant_offline(0, DomainParticipantQos::default());
    for name in VARIANT_NAMES {
        let topic = format!("contract-dcps-{name}");
        let writer = create_writer(name, &publishing, &topic)
            .expect("every name of VARIANT_NAMES has a dispatch arm")
            .unwrap();
        assert_eq!(writer.topic_name(), topic);
        writer.write_data("BLUE", 1).unwrap();
        writer.drive_listeners();
        writer.poll_inconsistent_topic();

        let reader = create_reader(name, &subscribing, &topic)
            .expect("every name of VARIANT_NAMES has a dispatch arm")
            .unwrap();
        assert_eq!(reader.topic_name(), topic);
        assert_eq!(reader.type_name(), writer.type_name());
        reader.drive_listeners();
        reader.poll_inconsistent_topic();
        assert_eq!(reader.take_data(), 0, "nothing was delivered to it");
    }
    assert!(create_writer("NoSuchShape", &publishing, "t").is_none());
    assert!(create_reader("NoSuchShape", &subscribing, "t").is_none());
}

#[test]
fn dcps_2_3_3_the_any_mask_holds_the_statuses_the_suite_arms() {
    assert_eq!(status::INCONSISTENT_TOPIC, 0x0001);
    assert_eq!(status::REQUESTED_INCOMPATIBLE_QOS, 0x0001 << 6);
    assert_eq!(status::SUBSCRIPTION_MATCHED, 0x0001 << 14);
    for bit in [
        status::INCONSISTENT_TOPIC,
        status::REQUESTED_INCOMPATIBLE_QOS,
        status::SUBSCRIPTION_MATCHED,
    ] {
        assert_eq!(status::ANY & bit, bit);
    }
}
