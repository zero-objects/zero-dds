// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §7.4.4 on the DCPS path: a keyed type under
//! `data_protection_kind` ENCRYPT. The KeyHash of a sample, a dispose and an
//! unregister is the MD5 of the key; the writer knows the key of the instance
//! from the sample it wrote (or registered), also for a key that can exceed 16
//! octets (`ShapeType.color`, a string).
//!
//! No peer, no traffic.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataWriterQos, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    TopicQos,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_runtime::SharedSecurityGate;

const DOMAIN: i32 = 73;

fn governance(data_protection: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><domain_access_rules><domain_rule>
  <domains><id>{DOMAIN}</id></domains>
  <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
  <enable_join_access_control>FALSE</enable_join_access_control>
  <discovery_protection_kind>NONE</discovery_protection_kind>
  <liveliness_protection_kind>NONE</liveliness_protection_kind>
  <rtps_protection_kind>NONE</rtps_protection_kind>
  <topic_access_rules><topic_rule>
    <topic_expression>*</topic_expression>
    <enable_read_access_control>FALSE</enable_read_access_control>
    <enable_write_access_control>FALSE</enable_write_access_control>
    <metadata_protection_kind>NONE</metadata_protection_kind>
    <data_protection_kind>{data_protection}</data_protection_kind>
  </topic_rule></topic_access_rules>
</domain_rule></domain_access_rules></dds>"#
    )
}

fn participant(prefix: u8, data_protection: &str) -> DomainParticipant {
    let gate = Arc::new(SharedSecurityGate::new(
        DOMAIN as u32,
        zerodds_security_permissions::parse_governance_xml(&governance(data_protection))
            .expect("governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    let cfg = RuntimeConfig {
        security: Some(gate),
        security_guid_prefix: Some(GuidPrefix::from_bytes([prefix; 12])),
        spdp_multicast_send: false,
        ..RuntimeConfig::default()
    };
    DomainParticipantFactory::instance()
        .create_participant_with_config(DOMAIN, DomainParticipantQos::default(), cfg)
        .expect("participant")
}

/// Writes, registers, disposes and unregisters instances of `ShapeType` with
/// the key `color`.
fn exercise(data_protection: &str, prefix: u8) {
    let p = participant(prefix, data_protection);
    let topic = p
        .create_topic::<ShapeType>("Square", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .expect("writer");

    // By sample: the first write of the instance, the next ones, a color
    // longer than 16 octets (its KeyHash is an MD5 anyway, §9.6.3.3 of RTPS).
    let red = ShapeType::new("RED", 10, 20, 30);
    let long = ShapeType::new("A_COLOR_OF_MORE_THAN_SIXTEEN_OCTETS", 1, 2, 3);
    writer.write(&red).expect("write of a new instance");
    writer.write(&red).expect("write of a known instance");
    writer.write(&long).expect("write of a long key");

    // By instance: registered first, then written.
    let blue = ShapeType::new("BLUE", 5, 6, 7);
    let blue_handle = writer.register_instance(&blue).expect("register_instance");
    writer.write(&blue).expect("write of a registered instance");

    // Dispose and unregister, by the handle and by the sample of the instance.
    let red_handle = writer.lookup_instance(&red);
    writer.dispose(&red, red_handle).expect("dispose of RED");
    writer
        .unregister_instance(&red, red_handle)
        .expect("unregister of RED");
    writer
        .dispose(&blue, blue_handle)
        .expect("dispose of BLUE by its handle");
    writer
        .unregister_instance(&blue, blue_handle)
        .expect("unregister of BLUE by its handle");
    let long_handle = writer.lookup_instance(&long);
    writer
        .dispose(&long, long_handle)
        .expect("dispose of the long key");
}

#[test]
fn a_keyed_type_with_a_string_key_is_written_without_protection() {
    exercise("NONE", 0x61);
}

/// The bug: under ENCRYPT a write of `ShapeType` failed with "the key of the
/// instance is unknown, so the KeyHash of an encrypted payload cannot be
/// formed".
#[test]
fn a_keyed_type_with_a_string_key_is_written_disposed_and_unregistered_under_encrypt() {
    exercise("ENCRYPT", 0x62);
}
