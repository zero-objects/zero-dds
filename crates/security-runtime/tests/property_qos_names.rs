//! Plugin configuration through the `PropertyQosPolicy`.
//!
//! DDS-Security 1.2 names the properties the builtin plugins read in
//! Table 50 (Authentication), Table 63 (Access Control) and Table 71
//! (Crypto), and §7.3.2 sends a property to remote participants only
//! when it is marked `propagate` — the private key must never leave the
//! host. §9.1.2 leaves how plugins are selected to the implementation;
//! ZeroDDS selects and configures them from the property list and
//! nothing else. `dds.sec.log.*` is a ZeroDDS name, not the spec's.
//!
//! The tests below check both ends of each name: the exact spec name
//! configures the plugin, and a near-miss name does not silently fall
//! back to a default. A name that is read but not required would let a
//! typo in a deployment's XML disable access control without a word.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_security::access_control::{AccessControlPlugin, AccessDecision};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::SecurityErrorKind;
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security_logging::{
    PROP_LOG_BUILTIN_DISTRIBUTE, PROP_LOG_BUILTIN_LEGACY_TOPIC, PROP_LOG_BUILTIN_LOG_FILE,
    PROP_LOG_BUILTIN_LOGGING_LEVEL, PROP_LOG_JSONL_PATH, PROP_LOG_LEVEL, PROP_LOG_PLUGIN,
    PROP_LOG_SYSLOG_ADDR, PROP_LOG_SYSLOG_APP, PROP_LOG_SYSLOG_HOST,
    logging_plugin_from_properties,
};
use zerodds_security_permissions::{
    PROP_PERMISSIONS_CA, PROP_PERMISSIONS_XML, PROP_PSK_PERMISSIONS_XML, PermissionsAccessControl,
    PskPermissionsAccessControl,
};

const PERMISSIONS_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=alice</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id_range><min>0</min></id_range></domains><publish><topics><topic>Chatter</topic></topics></publish></allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

/// Tables 50/63 — the names the builtins read are the ones the spec names.
/// Written out as literals so that renaming a constant cannot quietly
/// rename the wire-visible configuration key.
#[test]
fn builtin_property_names_match_the_spec_strings() {
    // §9.3.1 access control.
    assert_eq!(PROP_PERMISSIONS_XML, "dds.sec.access.permissions");
    assert_eq!(PROP_PERMISSIONS_CA, "dds.sec.access.permissions_ca");

    // §11 logging plugin selection.
    assert_eq!(PROP_LOG_PLUGIN, "dds.sec.log.plugin");
    assert_eq!(PROP_LOG_LEVEL, "org.zerodds.sec.log.level");
    assert_eq!(PROP_LOG_JSONL_PATH, "org.zerodds.sec.log.jsonl.path");
    assert_eq!(PROP_LOG_SYSLOG_ADDR, "org.zerodds.sec.log.syslog.addr");
    assert_eq!(PROP_LOG_SYSLOG_APP, "org.zerodds.sec.log.syslog.app");
    assert_eq!(PROP_LOG_SYSLOG_HOST, "org.zerodds.sec.log.syslog.host");

    // §10.6.1 — the builtin Logging plugin's LogOptions.
    assert_eq!(
        PROP_LOG_BUILTIN_LOGGING_LEVEL,
        "org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level"
    );
    assert_eq!(
        PROP_LOG_BUILTIN_LOG_FILE,
        "dds.sec.log.builtin.DDS_LogTopic.log_file"
    );
    assert_eq!(
        PROP_LOG_BUILTIN_DISTRIBUTE,
        "dds.sec.log.builtin.DDS_LogTopic.distribute"
    );
    assert_eq!(
        PROP_LOG_BUILTIN_LEGACY_TOPIC,
        "org.zerodds.sec.log.builtin.DDS_LogTopic.legacy_topic"
    );

    // §10.8 PSK access control.
    assert_eq!(
        PROP_PSK_PERMISSIONS_XML,
        "org.zerodds.sec.access.psk_permissions"
    );

    // The names of the specification are in the reserved `dds.sec.` namespace;
    // a name of ZeroDDS is not (§7.3.2): it starts with a reversed domain name.
    for name in [
        PROP_PERMISSIONS_XML,
        PROP_PERMISSIONS_CA,
        PROP_LOG_PLUGIN,
        PROP_LOG_BUILTIN_LOG_FILE,
        PROP_LOG_BUILTIN_DISTRIBUTE,
    ] {
        assert!(
            name.starts_with("dds.sec."),
            "{name} is a name of the specification"
        );
    }
    for name in [
        PROP_LOG_LEVEL,
        PROP_LOG_JSONL_PATH,
        PROP_LOG_SYSLOG_ADDR,
        PROP_LOG_SYSLOG_APP,
        PROP_LOG_SYSLOG_HOST,
        PROP_LOG_BUILTIN_LOGGING_LEVEL,
        PROP_LOG_BUILTIN_LEGACY_TOPIC,
        PROP_PSK_PERMISSIONS_XML,
    ] {
        assert!(
            name.starts_with("org.zerodds.sec."),
            "{name} is not under the vendor prefix"
        );
    }
}

/// §7.3.2 — `propagate` decides what goes out over SEDP. A secret
/// belongs in the local half of the list and must not appear in
/// `propagatable()`.
#[test]
fn only_propagatable_properties_reach_the_wire() {
    let props = PropertyList::new()
        .with(Property::local(
            "dds.sec.auth.private_key",
            "-----BEGIN PRIVATE KEY-----",
        ))
        .with(Property::local(
            "dds.sec.access.permissions",
            PERMISSIONS_XML,
        ))
        .with(Property::propagated("dds.sec.identity_status_token", "ok"));

    let on_wire: Vec<&str> = props.propagatable().map(|p| p.name.as_ref()).collect();
    assert_eq!(on_wire, vec!["dds.sec.identity_status_token"]);
    assert!(
        !on_wire.contains(&"dds.sec.auth.private_key"),
        "the private key must never be propagated",
    );
    // The local half is still readable by the plugins on this host.
    assert!(props.get("dds.sec.auth.private_key").is_some());
}

/// Table 63 — the property list alone configures the access-control
/// plugin, and the exact name is load-bearing: a near-miss does not
/// fall through to a permit-everything default.
#[test]
fn access_control_is_configured_only_by_the_exact_property_names() {
    let mut ac = PermissionsAccessControl::new();
    let ok = PropertyList::new()
        .with(Property::local(
            "dds.sec.access.permissions",
            PERMISSIONS_XML,
        ))
        .with(Property::local(
            "org.zerodds.sec.access.subject_name",
            "CN=alice",
        ));
    let handle = ac
        .validate_local_permissions(
            IdentityHandle(1),
            &zerodds_security::access_control::ParticipantRequest::new(0, [0xAA; 16], &ok),
        )
        .expect("the spec-named properties configure the plugin");
    assert_eq!(
        ac.check_create_datawriter(
            handle,
            &zerodds_security::access_control::EndpointRequest::new(0, "Chatter")
        )
        .unwrap(),
        AccessDecision::Permit,
    );

    // One character off on the permissions key.
    let typo = PropertyList::new()
        .with(Property::local(
            "dds.sec.access.permission",
            PERMISSIONS_XML,
        ))
        .with(Property::local(
            "org.zerodds.sec.access.subject_name",
            "CN=alice",
        ));
    let err = ac
        .validate_local_permissions(
            IdentityHandle(2),
            &zerodds_security::access_control::ParticipantRequest::new(0, [0xBB; 16], &typo),
        )
        .expect_err("a misspelled key must not configure anything");
    assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);

    // Missing subject name is equally fatal.
    let no_subject = PropertyList::new().with(Property::local(
        "dds.sec.access.permissions",
        PERMISSIONS_XML,
    ));
    let err = ac
        .validate_local_permissions(
            IdentityHandle(3),
            &zerodds_security::access_control::ParticipantRequest::new(0, [0xCC; 16], &no_subject),
        )
        .expect_err("no subject name, no decision base");
    assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
}

/// Table 84, Table 85 — the PSK access-control plugin needs no document: it
/// succeeds without the X.509 key, and reads its permissions from its own
/// key when given one; neither changes a check, which permits.
#[test]
fn psk_access_control_reads_its_own_property_key() {
    let mut ac = PskPermissionsAccessControl::new();
    let x509_key = PropertyList::new()
        .with(Property::local(
            "dds.sec.access.permissions",
            PERMISSIONS_XML,
        ))
        .with(Property::local(
            "org.zerodds.sec.access.subject_name",
            "CN=alice",
        ));
    ac.validate_local_permissions(
        IdentityHandle(1),
        &zerodds_security::access_control::ParticipantRequest::new(0, [0xAA; 16], &x509_key),
    )
    .expect("Table 85: validate_local_permissions succeeds");

    let psk_key = PropertyList::new()
        .with(Property::local(
            "org.zerodds.sec.access.psk_permissions",
            PERMISSIONS_XML,
        ))
        .with(Property::local(
            "org.zerodds.sec.access.subject_name",
            "CN=alice",
        ));
    let handle = ac
        .validate_local_permissions(
            IdentityHandle(2),
            &zerodds_security::access_control::ParticipantRequest::new(0, [0xBB; 16], &psk_key),
        )
        .expect("the PSK-named property configures the PSK plugin");
    assert_eq!(
        ac.check_create_datawriter(
            handle,
            &zerodds_security::access_control::EndpointRequest::new(0, "Chatter")
        )
        .unwrap(),
        AccessDecision::Permit,
    );
}

/// ZeroDDS — the value of `dds.sec.log.plugin` picks the sink; an unknown
/// value is an error rather than a silent stderr fallback, and an
/// absent property selects no plugin at all.
#[test]
fn logging_plugin_selection_follows_the_property_value() {
    let none = logging_plugin_from_properties(&[]).expect("absent is not an error");
    assert!(none.is_none(), "no property, no logging plugin");

    let stderr =
        logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "stderr")]).expect("stderr sink builds");
    assert!(stderr.is_some());

    let unknown = logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "carrier-pigeon")]);
    assert!(
        unknown.is_err(),
        "an unknown sink name must fail loudly, not fall back",
    );

    // jsonl needs its path property — selecting the sink without it is
    // a configuration error, not a silently dropped log.
    let jsonl_without_path = logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "jsonl")]);
    assert!(jsonl_without_path.is_err());
}
