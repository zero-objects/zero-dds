// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.8.1 and §9.8.6 (Figures 22 and 24): which operations of
//! the AccessControl plugin the middleware calls, and in which order, when a
//! DomainParticipant, a Topic, a DataWriter and a DataReader are created and a
//! writer registers and disposes an instance. The plugin is the recording
//! mock of `zerodds-security`; it permits everything. (Steps 7 and 8 meet in
//! the middleware: the credential token goes to the Authentication plugin; a
//! failure there is an `operation_failed` event, of which the test expects
//! none.)
//!
//! No peer, no handshake, no traffic: runs on every platform.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::qos::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos, TopicQos};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_rtps::wire_types::{GuidPrefix, VendorId};
use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::authentication::AuthenticationPlugin;
use zerodds_security::mock::MockAuthenticationPlugin;
use zerodds_security::mock::{MockAccessCall, MockAccessControlPlugin};
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_runtime::SharedSecurityGate;

#[path = "common/mod.rs"]
mod common;
#[path = "common/recording_crypto.rs"]
mod recording_crypto;

const GOVERNANCE: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>@D@</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><topic_access_rules><topic_rule><topic_expression>*</topic_expression><enable_discovery_protection>true</enable_discovery_protection><enable_read_access_control>false</enable_read_access_control><enable_write_access_control>false</enable_write_access_control><metadata_protection_kind>NONE</metadata_protection_kind><data_protection_kind>NONE</data_protection_kind></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;

fn operations(calls: &Arc<Mutex<Vec<MockAccessCall>>>) -> Vec<String> {
    calls
        .lock()
        .unwrap()
        .iter()
        .map(|c| {
            format!(
                "{}{}",
                c.operation,
                c.topic
                    .as_deref()
                    .map_or(String::new(), |t| format!("({t})"))
            )
        })
        .collect()
}

#[test]
fn the_access_control_operations_are_called_in_the_order_of_figures_22_and_24() {
    let lease = common::lease_domain();
    let domain = lease.id();
    let gate = Arc::new(SharedSecurityGate::new(
        domain as u32,
        zerodds_security_permissions::parse_governance_xml(
            &GOVERNANCE.replace("@D@", &domain.to_string()),
        )
        .unwrap(),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    let (plugin, calls) = MockAccessControlPlugin::deciding(|_| true);
    gate.set_access_control(Box::new(plugin), PermissionsHandle(7))
        .unwrap();
    let prefix = GuidPrefix::from_bytes([0x44; 12]);
    let sink = Arc::new(zerodds_foundation::observability::VecSink::new());
    let cfg = RuntimeConfig {
        observability: sink.clone(),
        security: Some(gate),
        security_guid_prefix: Some(prefix),
        spdp_multicast_send: false,
        ..common::isolated_cfg()
    };
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant");
    let rt = p.runtime().expect("runtime").clone();
    let auth: Arc<Mutex<dyn AuthenticationPlugin>> =
        Arc::new(Mutex::new(MockAuthenticationPlugin::new()));
    let local = auth
        .lock()
        .unwrap()
        .validate_local_identity(
            domain as u32,
            &zerodds_security::properties::PropertyList::new(),
            [0x44; 16],
        )
        .unwrap()
        .identity;
    rt.enable_security_builtins_with_auth(VendorId::ZERODDS, auth, local);
    let topic = p
        .create_topic::<ShapeType>("Square", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let _reader = subscriber
        .create_datareader::<ShapeType>(&topic, DataReaderQos::default())
        .unwrap();
    let sample = ShapeType {
        color: "BLUE".into(),
        x: 1,
        y: 2,
        shapesize: 3,
        additional_payload_size: Default::default(),
    };
    let handle = writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();
    let disposed = writer.dispose(&sample, handle);
    assert_eq!(disposed, Ok(()));
    // The discovery announcement of each endpoint asks for the topic config
    // again (`is_discovery_protected` picks the discovery endpoints, steps 7
    // and 11); the creation sequence is what follows the first request.
    let mut seen_topic_check = false;
    let ops: Vec<String> = operations(&calls)
        .into_iter()
        .filter(|op| {
            if op.starts_with("check_create_topic") {
                seen_topic_check = true;
            }
            !(seen_topic_check
                && (op.starts_with("get_topic_security_config")
                    || op.starts_with("return_topic_security_config")))
        })
        .collect();
    assert_eq!(
        ops,
        [
            // §9.8.1 step 6: get_permissions_token; step 7:
            // get_permissions_credential_token (handed to the Authentication
            // plugin, step 8); step 9: get_participant_security_config.
            "get_permissions_token",
            "return_permissions_token",
            "get_permissions_credential_token",
            "return_permissions_credential_token",
            "get_participant_security_config",
            // §9.8.6 steps 2 and 3: the Topic.
            "get_topic_security_config(Square)",
            "return_topic_security_config",
            "check_create_topic(Square)",
            // Steps 5 and 6: the DataWriter.
            "check_create_datawriter(Square)",
            "get_datawriter_security_config(Square)",
            "return_datawriter_security_config",
            // Steps 9 and 10: the DataReader.
            "check_create_datareader(Square)",
            "get_datareader_security_config(Square)",
            "return_datareader_security_config",
            // Steps 12 to 15: register and dispose of an instance.
            "check_local_datawriter_register_instance",
            "check_local_datawriter_dispose_instance",
        ]
    );
    let failed: Vec<_> = sink
        .snapshot()
        .into_iter()
        .filter(|e| e.name.ends_with("operation_failed"))
        .map(|e| e.name)
        .collect();
    assert!(failed.is_empty(), "{failed:?}");
}

/// DDS-Security 1.2 §9.8.1 step 11 and §9.8.9.1: the Cryptographic plugin is
/// asked to `register_local_participant` with the ParticipantSecurityConfig
/// the AccessControl plugin gave, and registers the volatile message writer
/// and reader with the property `dds.sec.builtin_endpoint_name`.
#[test]
fn the_cryptographic_plugin_registers_the_participant_and_the_volatile_endpoints() {
    let lease = common::lease_domain();
    let domain = lease.id();
    let (crypto, log) = recording_crypto::RecordingCrypto::new();
    let gate = Arc::new(SharedSecurityGate::new(
        domain as u32,
        zerodds_security_permissions::parse_governance_xml(
            &GOVERNANCE.replace("@D@", &domain.to_string()),
        )
        .unwrap(),
        Box::new(crypto),
    ));
    // The plugin's config says `is_rtps_axk_protected` TRUE; the governance
    // has no rtps_protection_kind, so the builtin config would say FALSE.
    let (plugin, _calls) = MockAccessControlPlugin::deciding(|_| true);
    let plugin = plugin.with_participant_config(|config| config.is_rtps_axk_protected = true);
    gate.set_access_control(Box::new(plugin), PermissionsHandle(7))
        .unwrap();
    let prefix = GuidPrefix::from_bytes([0x45; 12]);
    let cfg = RuntimeConfig {
        security: Some(gate.clone()),
        security_guid_prefix: Some(prefix),
        spdp_multicast_send: false,
        ..common::isolated_cfg()
    };
    let _p = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant");
    gate.ensure_local()
        .expect("the local participant is registered");
    let log = log.lock().unwrap();
    let ops: Vec<&str> = log.iter().map(|c| c.operation).collect();
    assert_eq!(
        ops,
        [
            "register_local_participant",
            "register_local_datawriter",
            "register_local_datareader"
        ]
    );
    assert_eq!(
        log[0].participant_axk,
        Some(true),
        "the config of the AccessControl plugin reaches the Cryptographic plugin"
    );
    let name = |c: &recording_crypto::Call| {
        c.properties
            .iter()
            .find(|(n, _)| n == "dds.sec.builtin_endpoint_name")
            .map(|(_, v)| v.clone())
    };
    assert_eq!(
        name(&log[1]).as_deref(),
        Some("BuiltinParticipantVolatileMessageSecureWriter")
    );
    assert_eq!(
        name(&log[2]).as_deref(),
        Some("BuiltinParticipantVolatileMessageSecureReader")
    );
}
