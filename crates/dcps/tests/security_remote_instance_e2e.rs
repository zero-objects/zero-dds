// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.8.8.2, Figure 27 steps 4 and 6: the DataReader asks
//! its AccessControl plugin whether the remote DataWriter may register an
//! instance (`check_remote_datawriter_register_instance`) and dispose of it
//! (`check_remote_datawriter_dispose_instance`). If the check fails, the
//! sample is ignored, or the disposal is ignored.
//!
//! Two participants on one domain over UDP: the writer's participant has no
//! security, the reader's has a gate with the mock plugin of `zerodds-security`
//! that decides with the function the test gives it. The governance allows
//! unauthenticated participants and does not access-control the topic, so
//! the endpoints match and only the instance operations are decided.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::mock::{MockAccessCall, MockAccessControlPlugin};
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_runtime::SharedSecurityGate;

#[path = "common/mod.rs"]
mod common;

const GOVERNANCE: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>@D@</id></domains><allow_unauthenticated_participants>true</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><topic_access_rules><topic_rule><topic_expression>*</topic_expression><enable_discovery_protection>false</enable_discovery_protection><enable_read_access_control>false</enable_read_access_control><enable_write_access_control>false</enable_write_access_control><metadata_protection_kind>NONE</metadata_protection_kind><data_protection_kind>NONE</data_protection_kind></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;

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

/// What the reader's participant received, and which instance operations
/// the plugin was asked about.
struct Outcome {
    /// (instance state, valid data) of each sample taken.
    samples: Vec<(InstanceStateKind, bool)>,
    asked_register: usize,
    asked_dispose: usize,
}

/// The writer registers and writes an instance and disposes of it; the
/// plugin of the reader's participant permits (`true`) what `decide` says.
fn run(decide: impl Fn(&MockAccessCall) -> bool + Send + Sync + 'static) -> Outcome {
    let lease = common::lease_domain();
    let domain = lease.id();
    let cfg = common::isolated_cfg();
    let factory = DomainParticipantFactory::instance();

    let gate = Arc::new(SharedSecurityGate::new(
        domain as u32,
        zerodds_security_permissions::parse_governance_xml(
            &GOVERNANCE.replace("@D@", &domain.to_string()),
        )
        .expect("governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    let (plugin, calls) = MockAccessControlPlugin::deciding(decide);
    // The config of the plugin is what the gate follows (§9.4.2.4 Table 33):
    // it allows the unauthenticated writer's participant.
    let plugin = plugin.with_participant_config(|config| {
        config.allow_unauthenticated_participants = true;
    });
    gate.set_access_control(Box::new(plugin), PermissionsHandle(1))
        .expect("arm");
    let reader_cfg = zerodds_dcps::runtime::RuntimeConfig {
        security: Some(gate),
        security_guid_prefix: Some(GuidPrefix::from_bytes([0x71; 12])),
        ..cfg.clone()
    };
    let reader_participant = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), reader_cfg)
            .expect("reader participant"),
    );
    let writer_participant = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("writer participant"),
    );

    let r_topic = reader_participant
        .create_topic::<Rec>("RemoteInstances", TopicQos::default())
        .expect("topic");
    let reader = reader_participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Rec>(&r_topic, DataReaderQos::default())
        .expect("reader");
    let w_topic = writer_participant
        .create_topic::<Rec>("RemoteInstances", TopicQos::default())
        .expect("topic");
    let writer = writer_participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Rec>(&w_topic, DataWriterQos::default())
        .expect("writer");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("the endpoints match");

    reader
        .wait_for_matched_publication(1, common::match_timeout())
        .expect("the reader matches the writer");
    let sample = Rec { id: 1, value: 10 };
    let handle = writer.register_instance(&sample).expect("register");
    writer.write(&sample).expect("write");
    // The check of the register operation is made when the sample arrives.
    let asked = |operation: &str| {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.operation == operation)
            .count()
    };
    let wait = |ready: &dyn Fn() -> bool| {
        let deadline = Instant::now() + common::match_timeout();
        while Instant::now() < deadline && !ready() {
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    wait(&|| asked("check_remote_datawriter_register_instance") > 0);
    writer.dispose(&sample, handle).expect("dispose");
    wait(&|| asked("check_remote_datawriter_dispose_instance") > 0);
    // Time for what the check lets through to reach the cache.
    std::thread::sleep(Duration::from_millis(500));
    let samples = reader
        .take_with_info()
        .expect("take")
        .into_iter()
        .map(|s| (s.info.instance_state, s.info.valid_data))
        .collect();
    Outcome {
        samples,
        asked_register: asked("check_remote_datawriter_register_instance"),
        asked_dispose: asked("check_remote_datawriter_dispose_instance"),
    }
}

/// Control: where the plugin permits both, the reader gets the sample and
/// the disposal marker. Without this the tests below could pass on a path
/// that delivers nothing.
#[test]
fn a_remote_writer_that_the_plugin_permits_registers_and_disposes() {
    let outcome = run(|_| true);
    assert!(outcome.asked_register > 0, "register asked");
    assert!(outcome.asked_dispose > 0, "dispose asked");
    // The instance is disposed when the reader takes: the sample and the
    // marker both report NOT_ALIVE_DISPOSED.
    assert_eq!(
        outcome.samples,
        [
            (InstanceStateKind::NotAliveDisposed, true),
            (InstanceStateKind::NotAliveDisposed, false)
        ]
    );
}

/// Figure 27 step 4: the plugin refuses `check_remote_datawriter_register_instance`;
/// the sample is ignored.
#[test]
fn a_sample_of_an_instance_the_plugin_refuses_to_register_is_ignored() {
    let outcome = run(|call| call.operation != "check_remote_datawriter_register_instance");
    assert!(outcome.asked_register > 0, "register asked");
    assert!(
        outcome.samples.iter().all(|(_, valid)| !valid),
        "no sample with data: {:?}",
        outcome.samples
    );
}

/// Figure 27 step 6: the plugin refuses `check_remote_datawriter_dispose_instance`;
/// the disposal is ignored.
#[test]
fn a_disposal_the_plugin_refuses_is_ignored() {
    let outcome = run(|call| call.operation != "check_remote_datawriter_dispose_instance");
    assert!(outcome.asked_dispose > 0, "dispose asked");
    assert_eq!(
        outcome.samples,
        [(InstanceStateKind::Alive, true)],
        "the sample was taken, the disposal was ignored"
    );
}
