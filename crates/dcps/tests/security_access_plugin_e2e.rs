// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.4.2.9 on the DCPS path with an AccessControl plugin
//! of the application: the runtime asks the plugin — not the governance and
//! not the permissions — for the creation of a Topic, the `set_qos` of the
//! participant, the creation of a DataWriter with its partitions, data tags
//! and QoS, and the registration and disposal of an instance. The plugin is
//! the mock of `zerodds-security`, which records each request and answers
//! with the function the test gives it.
//!
//! No peer, no handshake, no traffic.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::{Arc, Mutex};

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::entity::Entity;
use zerodds_dcps::error::DdsError;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataWriterQos, DdsType, DomainParticipant, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, TopicQos,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::mock::{MockAccessCall, MockAccessControlPlugin};
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_runtime::SharedSecurityGate;

const DOMAIN: i32 = 72;

/// A governance that access-controls everything: only a plugin of the
/// application can let a topic or an endpoint through here.
const GOVERNANCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><domain_access_rules><domain_rule>
  <domains><id>72</id></domains>
  <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
  <enable_join_access_control>TRUE</enable_join_access_control>
  <discovery_protection_kind>NONE</discovery_protection_kind>
  <liveliness_protection_kind>NONE</liveliness_protection_kind>
  <rtps_protection_kind>NONE</rtps_protection_kind>
  <topic_access_rules><topic_rule>
    <topic_expression>*</topic_expression>
    <enable_read_access_control>TRUE</enable_read_access_control>
    <enable_write_access_control>TRUE</enable_write_access_control>
    <metadata_protection_kind>NONE</metadata_protection_kind>
    <data_protection_kind>NONE</data_protection_kind>
  </topic_rule></topic_access_rules>
</domain_rule></domain_access_rules></dds>"#;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Rec {
    id: u32,
    value: u32,
}

impl DdsType for Rec {
    const TYPE_NAME: &'static str = "test::AccessRec";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.value.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize| -> Result<u32, DecodeError> {
            let b: [u8; 4] = bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid { what: "truncated" })?;
            Ok(u32::from_le_bytes(b))
        };
        Ok(Self {
            id: word(0)?,
            value: word(4)?,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "truncated" })?;
        Ok(Self {
            id: u32::from_be_bytes(head),
            value: 0,
        })
    }
}

type Calls = Arc<Mutex<Vec<MockAccessCall>>>;

/// A participant whose gate holds the mock plugin deciding with `decide`.
fn participant(
    prefix: u8,
    decide: impl Fn(&MockAccessCall) -> bool + Send + Sync + 'static,
) -> (DomainParticipant, Calls) {
    let gate = Arc::new(SharedSecurityGate::new(
        DOMAIN as u32,
        zerodds_security_permissions::parse_governance_xml(GOVERNANCE).expect("governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    let (plugin, calls) = MockAccessControlPlugin::deciding(decide);
    gate.set_access_control(Box::new(plugin), PermissionsHandle(1))
        .expect("arm");
    let cfg = RuntimeConfig {
        security: Some(gate),
        security_guid_prefix: Some(GuidPrefix::from_bytes([prefix; 12])),
        spdp_multicast_send: false,
        ..RuntimeConfig::default()
    };
    let participant = DomainParticipantFactory::instance()
        .create_participant_with_config(DOMAIN, DomainParticipantQos::default(), cfg)
        .expect("participant");
    (participant, calls)
}

fn asked(calls: &Calls, operation: &str) -> Vec<MockAccessCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.operation == operation)
        .cloned()
        .collect()
}

fn assert_denied<T>(result: Result<T, DdsError>, case: &str) {
    match result {
        Err(DdsError::NotAllowedBySecurity { .. }) => {}
        Err(other) => panic!("{case}: expected NotAllowedBySecurity, got {other:?}"),
        Ok(_) => panic!("{case}: the plugin refused, the operation went through"),
    }
}

/// §9.4.2.9.6 `check_create_topic`: the plugin sees the topic and its QoS,
/// and a topic it refuses is not created (NOT_ALLOWED_BY_SECURITY).
#[test]
fn create_topic_is_asked_of_the_plugin() {
    let (p, calls) = participant(0x81, |call| call.topic.as_deref() != Some("Forbidden"));
    p.create_topic::<Rec>("Allowed", TopicQos::default())
        .expect("the plugin permits the topic");
    assert_denied(
        p.create_topic::<Rec>("Forbidden", TopicQos::default()),
        "create_topic Forbidden",
    );
    let topics = asked(&calls, "check_create_topic");
    assert_eq!(topics.len(), 2, "{topics:?}");
    assert_eq!(topics[0].topic.as_deref(), Some("Allowed"));
    assert_eq!(topics[0].domain_id, Some(DOMAIN as u32));
    assert!(
        topics[0].qos.iter().any(|(name, _)| name == "reliability"),
        "the TopicQos reaches the plugin: {:?}",
        topics[0].qos
    );
}

/// §9.4.2.9.3: `set_qos` of the participant is checked with the new QoS; a
/// QoS the plugin refuses makes `set_qos` fail with NOT_ALLOWED_BY_SECURITY.
#[test]
fn participant_set_qos_is_asked_of_the_plugin() {
    let (p, calls) = participant(0x82, |call| {
        !call
            .properties
            .iter()
            .any(|(name, _)| name == "x.forbidden")
    });
    p.set_qos(DomainParticipantQos::default())
        .expect("a QoS the plugin permits");
    let mut qos = DomainParticipantQos::default();
    qos.property.set("x.forbidden", "1");
    assert_denied(p.set_qos(qos), "set_qos with a forbidden property");
    let asked = asked(&calls, "check_create_participant");
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(asked[1].properties.iter().any(|(n, _)| n == "x.forbidden"));
}

/// §9.4.2.9.4: the plugin gets the partitions, the data tags and the QoS of
/// the DataWriter, and refuses by them; a `set_qos` of the writer asks again.
#[test]
fn a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands() {
    let (p, calls) = participant(0x83, |call| {
        !call
            .data_tags
            .iter()
            .any(|(name, value)| name == "class" && value == "top")
    });
    let topic = p
        .create_topic::<Rec>("Tagged", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());

    let mut qos = DataWriterQos::default();
    qos.partition.names = vec!["P1".into()];
    qos.data_tags = qos.data_tags.with("class", "low");
    qos.property.set("x.note", "n");
    let writer = publisher
        .create_datawriter::<Rec>(&topic, qos.clone())
        .expect("a tag the plugin permits");
    let created = asked(&calls, "check_create_datawriter");
    assert_eq!(created.len(), 1, "{created:?}");
    assert_eq!(created[0].topic.as_deref(), Some("Tagged"));
    assert_eq!(created[0].partitions, ["P1"]);
    assert_eq!(
        created[0].data_tags,
        [("class".to_string(), "low".to_string())]
    );
    assert!(
        created[0]
            .qos
            .iter()
            .any(|(name, value)| name == "property.x.note" && value == "n"),
        "{:?}",
        created[0].qos
    );
    assert!(created[0].qos.iter().any(|(name, _)| name == "durability"));

    let mut refused = qos.clone();
    refused.data_tags = zerodds_qos::DataTagQosPolicy::new().with("class", "top");
    assert_denied(
        publisher.create_datawriter::<Rec>(&topic, refused),
        "create_datawriter with the tag the plugin refuses",
    );

    // set_qos of the writer: the create check again, with the new QoS.
    let mut changed = writer.qos();
    changed.deadline.period = zerodds_qos::Duration::from_secs(5);
    writer
        .set_qos(changed)
        .expect("a change the plugin permits");
    assert_eq!(
        asked(&calls, "check_create_datawriter").len(),
        3,
        "creation, the refused creation, set_qos"
    );
}

/// §9.4.2.9.7, §9.4.2.9.8: `register_instance` and `dispose` ask the plugin
/// with the serialized key; an instance it refuses is not registered.
#[test]
fn instance_operations_are_asked_of_the_plugin() {
    let (p, calls) = participant(0x84, |call| {
        // The key holder is the instance id as 4 octets big endian.
        call.key.as_deref() != Some(&[0, 0, 0, 13][..])
    });
    let topic = p
        .create_topic::<Rec>("Instances", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Rec>(&topic, DataWriterQos::default())
        .expect("writer");
    let ok = Rec { id: 7, value: 1 };
    let handle = writer.register_instance(&ok).expect("instance 7");
    assert_denied(
        writer.register_instance(&Rec { id: 13, value: 1 }),
        "register_instance of instance 13",
    );
    assert_denied(
        writer.write(&Rec { id: 13, value: 2 }),
        "a write that would register instance 13",
    );
    writer.dispose(&ok, handle).expect("dispose of instance 7");
    let registered = asked(&calls, "check_local_datawriter_register_instance");
    assert_eq!(registered.len(), 3, "{registered:?}");
    assert_eq!(registered[0].key.as_deref(), Some(&[0, 0, 0, 7][..]));
    let disposed = asked(&calls, "check_local_datawriter_dispose_instance");
    assert_eq!(disposed.len(), 1, "{disposed:?}");
    assert_eq!(disposed[0].key.as_deref(), Some(&[0, 0, 0, 7][..]));
}
