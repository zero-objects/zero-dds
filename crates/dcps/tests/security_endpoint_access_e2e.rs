// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.4.2.9.4 / §9.4.2.9.5 on the runtime path.
//!
//! The six refusals the OMG `dds-security` suite asks for, run through
//! `DcpsRuntime::register_user_writer` / `register_user_reader` — the
//! chokepoint every DataWriter and DataReader passes, from the Rust
//! `Publisher::create_datawriter` down to `zerodds-c-api`.
//!
//! The governance is shaped like the suite's `TESTONLY_governance.xml`: the
//! first `<topic_rule>` is the unprotected `OD_OA_OM_OD` one and the
//! access-controlled profiles stand behind it. A check that read
//! `topic_rules.first()` instead of the matching rule would permit all six.
//!
//! No peer, no handshake, no traffic: the decision is taken before the
//! endpoint is registered, so this runs on every platform.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;

use zerodds_dcps::error::DdsError;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_runtime::SharedSecurityGate;

const DOMAIN: i32 = 71;

/// The suite's topic profiles, in the suite's order — the unprotected rule
/// first.
const GOVERNANCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains><id>71</id></domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>TRUE</enable_join_access_control>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>OD_OA_OM_OD</topic_expression>
          <enable_read_access_control>FALSE</enable_read_access_control>
          <enable_write_access_control>FALSE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_RA_OM_OD</topic_expression>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>FALSE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_WA_OM_OD</topic_expression>
          <enable_read_access_control>FALSE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_RWA_OM_OD</topic_expression>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_RWA_EM_ED</topic_expression>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>"#;

fn permissions(rules: &str) -> String {
    format!(
        r#"<dds><permissions><grant name="g">
             <subject_name>/C=DE/ST=Hessen/O=ZeroDDS/CN=CN_TEST_ZERODDS_A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
             <allow_rule><domains><id>71</id></domains>{rules}</allow_rule>
             <default>DENY</default>
           </grant></permissions></dds>"#
    )
}

/// The suite's `..._permissions_write_signed.p7s`.
fn write_only() -> String {
    permissions(
        r#"<publish><topics>
             <topic>PD_WA_OM_OD</topic><topic>PD_RWA_OM_OD</topic>
             <topic>PD_RWA_EM_ED</topic>
           </topics></publish>"#,
    )
}

/// The suite's `..._permissions_read_signed.p7s`.
fn read_only() -> String {
    permissions(
        r#"<subscribe><topics>
             <topic>PD_RA_OM_OD</topic><topic>PD_RWA_OM_OD</topic>
             <topic>PD_RWA_EM_ED</topic>
           </topics></subscribe>"#,
    )
}

/// The suite's `..._permissions_join_signed.p7s` — the domain, nothing else.
fn join_only() -> String {
    permissions("")
}

fn runtime(prefix: u8, permissions_xml: &str) -> Arc<DcpsRuntime> {
    let gate = Arc::new(SharedSecurityGate::new(
        DOMAIN as u32,
        zerodds_security_permissions::parse_governance_xml(GOVERNANCE).expect("governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    gate.set_permissions(
        zerodds_security_permissions::parse_permissions_xml(permissions_xml).expect("permissions"),
    )
    .expect("arm");
    // §9.3.3: a secured runtime must start on the identity-derived prefix.
    // This test has no identity — it measures the access-control decision, not
    // the handshake — so the announced prefix and the started prefix are the
    // same literal.
    let guid_prefix = GuidPrefix::from_bytes([prefix; 12]);
    let cfg = RuntimeConfig {
        security: Some(gate),
        security_guid_prefix: Some(guid_prefix),
        spdp_multicast_send: false,
        ..RuntimeConfig::default()
    };
    DcpsRuntime::start(DOMAIN, guid_prefix, cfg).expect("runtime")
}

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    }
}

fn reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    }
}

fn assert_denied(result: &Result<impl Sized, DdsError>, case: &str) {
    match result {
        Err(DdsError::NotAllowedBySecurity { .. }) => {}
        Err(other) => panic!("{case}: expected NotAllowedBySecurity, got {other:?}"),
        Ok(_) => panic!("{case}: the endpoint was created, the suite asks for a refusal"),
    }
}

/// The six cases the OMG suite requires a refusal for.
#[test]
fn the_six_omg_suite_refusals_reach_the_runtime() {
    let rt = runtime(0x71, &write_only());
    assert_denied(
        &rt.register_user_reader(reader_cfg("PD_RA_OM_OD")),
        "sub PD_RA_OM_OD with write permissions",
    );
    assert_denied(
        &rt.register_user_reader(reader_cfg("PD_RWA_OM_OD")),
        "sub PD_RWA_OM_OD with write permissions",
    );

    let rt = runtime(0x72, &read_only());
    assert_denied(
        &rt.register_user_writer(writer_cfg("PD_WA_OM_OD")),
        "pub PD_WA_OM_OD with read permissions",
    );
    assert_denied(
        &rt.register_user_writer(writer_cfg("PD_RWA_OM_OD")),
        "pub PD_RWA_OM_OD with read permissions",
    );

    let rt = runtime(0x73, &join_only());
    assert_denied(
        &rt.register_user_reader(reader_cfg("PD_RWA_EM_ED")),
        "sub PD_RWA_EM_ED with join permissions",
    );
    assert_denied(
        &rt.register_user_writer(writer_cfg("PD_RWA_EM_ED")),
        "pub PD_RWA_EM_ED with join permissions",
    );
}

/// The granted direction of the same profiles still creates its endpoint,
/// and the uncontrolled topic needs no grant at all — a gate that refuses
/// everything is not access control.
#[test]
fn the_granted_direction_and_the_open_topic_still_create() {
    let rt = runtime(0x74, &write_only());
    rt.register_user_writer(writer_cfg("PD_WA_OM_OD"))
        .expect("publish is granted");
    rt.register_user_writer(writer_cfg("PD_RWA_EM_ED"))
        .expect("publish is granted");

    let rt = runtime(0x75, &read_only());
    rt.register_user_reader(reader_cfg("PD_RA_OM_OD"))
        .expect("subscribe is granted");
    rt.register_user_reader(reader_cfg("PD_RWA_EM_ED"))
        .expect("subscribe is granted");

    let rt = runtime(0x76, &join_only());
    rt.register_user_writer(writer_cfg("OD_OA_OM_OD"))
        .expect("the topic is not access-controlled");
    rt.register_user_reader(reader_cfg("OD_OA_OM_OD"))
        .expect("the topic is not access-controlled");
}

/// A gate whose permissions were never armed refuses an access-controlled
/// topic rather than creating the endpoint. The `NoPermissions` cause is a
/// wiring error, and a wiring error that comes up as an open domain is the
/// silent drop this check exists to prevent.
#[test]
fn an_unarmed_gate_refuses_instead_of_creating() {
    let gate = Arc::new(SharedSecurityGate::new(
        DOMAIN as u32,
        zerodds_security_permissions::parse_governance_xml(GOVERNANCE).expect("governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    assert!(!gate.has_permissions().expect("lock"));
    let guid_prefix = GuidPrefix::from_bytes([0x77; 12]);
    let cfg = RuntimeConfig {
        security: Some(gate),
        security_guid_prefix: Some(guid_prefix),
        spdp_multicast_send: false,
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(DOMAIN, guid_prefix, cfg).expect("runtime");
    assert_denied(
        &rt.register_user_writer(writer_cfg("PD_RWA_EM_ED")),
        "unarmed gate, access-controlled topic",
    );
    rt.register_user_writer(writer_cfg("OD_OA_OM_OD"))
        .expect("the topic is not access-controlled");
}

/// A participant with no security gate at all is untouched — an absent
/// governance is not a denial.
#[test]
fn a_plaintext_participant_is_not_access_controlled() {
    let cfg = RuntimeConfig {
        spdp_multicast_send: false,
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(DOMAIN, GuidPrefix::from_bytes([0x78; 12]), cfg).expect("runtime");
    rt.register_user_writer(writer_cfg("PD_RWA_EM_ED"))
        .expect("no gate, no access control");
    rt.register_user_reader(reader_cfg("PD_RWA_EM_ED"))
        .expect("no gate, no access control");
}

fn participant_with(prefix: u8, permissions_xml: &str) -> zerodds_dcps::DomainParticipant {
    let gate = Arc::new(SharedSecurityGate::new(
        DOMAIN as u32,
        zerodds_security_permissions::parse_governance_xml(GOVERNANCE).expect("governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ));
    gate.set_permissions(
        zerodds_security_permissions::parse_permissions_xml(permissions_xml).expect("permissions"),
    )
    .expect("arm");
    let cfg = RuntimeConfig {
        security: Some(gate),
        security_guid_prefix: Some(GuidPrefix::from_bytes([prefix; 12])),
        spdp_multicast_send: false,
        ..RuntimeConfig::default()
    };
    zerodds_dcps::DomainParticipantFactory::instance()
        .create_participant_with_config(DOMAIN, zerodds_dcps::DomainParticipantQos::default(), cfg)
        .expect("participant")
}

/// DDS-Security 1.2 §9.8.6 steps 1 to 3: before a Topic exists the middleware
/// calls `check_create_topic` (Table 70). "If the verification fails, the
/// Topic object is not created." A topic whose rule has read or write access
/// control off needs no grant; otherwise the permissions must allow
/// publishing or subscribing it; a topic no rule names is refused
/// (§10.4.1.2.7).
#[test]
fn a_topic_is_created_only_where_check_create_topic_permits() {
    use zerodds_dcps::dds_type::RawBytes;
    use zerodds_dcps::qos::TopicQos;

    let join = participant_with(0x79, &join_only());
    assert!(
        join.create_topic::<RawBytes>("OD_OA_OM_OD", TopicQos::default())
            .is_ok(),
        "no access control on the topic"
    );
    assert!(
        join.create_topic::<RawBytes>("PD_RA_OM_OD", TopicQos::default())
            .is_ok(),
        "write access control is off for this rule"
    );
    for topic in ["PD_RWA_EM_ED", "PD_RWA_OM_OD"] {
        assert_denied(
            &join.create_topic::<RawBytes>(topic, TopicQos::default()),
            &format!("join-only permissions, topic {topic}"),
        );
    }
    assert_denied(
        &join.create_topic::<RawBytes>("NoRuleNamesThis", TopicQos::default()),
        "a topic no rule covers",
    );

    for (prefix, permissions) in [(0x7A, write_only()), (0x7B, read_only())] {
        let participant = participant_with(prefix, &permissions);
        participant
            .create_topic::<RawBytes>("PD_RWA_EM_ED", TopicQos::default())
            .expect("the permissions publish or subscribe the topic");
    }
}
