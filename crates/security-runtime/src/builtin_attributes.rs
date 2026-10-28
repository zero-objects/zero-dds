// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The security attributes of the builtin secure endpoints and of the
//! `ParticipantVolatileMessageSecure` topic.
//!
//! DDS-Security 1.2 §7.5.13: "the middleware shall call the operations
//! `get_datawriter_security_config` and `get_datareader_security_config` on
//! the AccessControl interface to obtain the EndpointSecurityConfig
//! associated with DataReader and DataWriter entities on all the 'Builtin
//! Secure Endpoints'. The specific values [...] shall be as shown in Table
//! 21", and §7.5.4.2 Table 17 gives the TopicSecurityConfig of the volatile
//! topic. This module answers both; the middleware asks it where it makes the
//! key material of a builtin endpoint.

use zerodds_rtps::wire_types::EntityId;
use zerodds_security_permissions::ProtectionKind;

use crate::{ProtectionLevel, SharedSecurityGate};

/// The `TopicSecurityAttributes` of a builtin topic (§9.4.2.3 Table 31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopicSecurityAttributes {
    /// `is_read_protected`.
    pub is_read_protected: bool,
    /// `is_write_protected`.
    pub is_write_protected: bool,
    /// `is_discovery_protected`.
    pub is_discovery_protected: bool,
    /// `is_liveliness_protected`.
    pub is_liveliness_protected: bool,
}

/// The `EndpointSecurityAttributes` of a builtin endpoint (§9.4.2.3 Table 32,
/// §7.5.13 Table 21). `is_discovery_protected` and `is_liveliness_protected`
/// are "N/A" there; they are `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointSecurityAttributes {
    /// `is_read_protected`.
    pub is_read_protected: bool,
    /// `is_write_protected`.
    pub is_write_protected: bool,
    /// `is_discovery_protected`.
    pub is_discovery_protected: bool,
    /// `is_liveliness_protected`.
    pub is_liveliness_protected: bool,
    /// `is_submessage_protected`.
    pub is_submessage_protected: bool,
    /// `is_payload_protected`.
    pub is_payload_protected: bool,
    /// `is_key_protected`.
    pub is_key_protected: bool,
}

/// §7.5.4.2 Table 17: the `TopicSecurityConfig` of the
/// `DCPSParticipantVolatileMessageSecure` topic: nothing is access protected,
/// discovery and liveliness are not protected.
#[must_use]
pub const fn volatile_topic_security_attributes() -> TopicSecurityAttributes {
    TopicSecurityAttributes {
        is_read_protected: false,
        is_write_protected: false,
        is_discovery_protected: false,
        is_liveliness_protected: false,
    }
}

/// §7.5.13 Table 21: the attributes of the builtin secure endpoint `eid`;
/// `None` for an endpoint that is not one of them.
///
/// `is_submessage_protected` is "set to match" the participant's
/// `is_discovery_protected` for `DCPSParticipantsSecure`,
/// `DCPSPublicationsSecure` and `DCPSSubscriptionsSecure`, its
/// `is_liveliness_protected` for `DCPSParticipantMessageSecure`, `false` for
/// `DCPSParticipantStatelessMessage` and `true` for
/// `DCPSParticipantVolatileMessageSecure` and the secure TypeLookup
/// endpoints. Everything else is `false`.
#[must_use]
pub fn builtin_endpoint_security_attributes(
    gate: &SharedSecurityGate,
    eid: EntityId,
) -> Option<EndpointSecurityAttributes> {
    let discovery = gate
        .discovery_protection()
        .is_ok_and(|level| level != ProtectionLevel::None);
    let liveliness = gate
        .liveliness_protection()
        .is_ok_and(|level| level != ProtectionLevel::None);
    let submessage_protected = match eid {
        EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER
        | EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER
        | EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
        | EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER
        | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER
        | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER => discovery,
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
        | EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER => liveliness,
        EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
        | EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER => false,
        EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
        | EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
        | EntityId::TL_SVC_REQ_SECURE_WRITER
        | EntityId::TL_SVC_REQ_SECURE_READER
        | EntityId::TL_SVC_REPLY_SECURE_WRITER
        | EntityId::TL_SVC_REPLY_SECURE_READER => true,
        _ => return None,
    };
    Some(EndpointSecurityAttributes {
        is_read_protected: false,
        is_write_protected: false,
        is_discovery_protected: false,
        is_liveliness_protected: false,
        is_submessage_protected: submessage_protected,
        is_payload_protected: false,
        is_key_protected: false,
    })
}

/// The Metadata and Data Protection Kind the key material of the builtin
/// secure endpoint `eid` is made for: none where Table 21 says its
/// submessages are not protected; the submessages of the secure TypeLookup
/// endpoints encrypted; the `discovery_protection_kind` or
/// `liveliness_protection_kind` of the domain for the others (§10.4.1.2.5),
/// with no payload protection.
///
/// # Errors
/// Mutex poison.
pub fn builtin_endpoint_kinds(
    gate: &SharedSecurityGate,
    eid: EntityId,
) -> Result<(ProtectionKind, ProtectionKind), crate::SecurityGateError> {
    let protected = builtin_endpoint_security_attributes(gate, eid)
        .is_some_and(|attributes| attributes.is_submessage_protected);
    if !protected {
        return Ok((ProtectionKind::None, ProtectionKind::None));
    }
    match eid {
        EntityId::TL_SVC_REQ_SECURE_WRITER
        | EntityId::TL_SVC_REQ_SECURE_READER
        | EntityId::TL_SVC_REPLY_SECURE_WRITER
        | EntityId::TL_SVC_REPLY_SECURE_READER => {
            Ok(SharedSecurityGate::secure_type_lookup_kinds())
        }
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
        | EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER => {
            gate.endpoint_protection_kinds(None, true)
        }
        _ => gate.endpoint_protection_kinds(None, false),
    }
}

/// The Metadata and Data Protection Kind a
/// [`EndpointSecurityConfig`](zerodds_security::access_control::EndpointSecurityConfig)
/// of an AccessControl plugin of the application describes: no submessage
/// protection is `NONE`; otherwise `ENCRYPT` where `plugin_endpoint_attributes`
/// has `IS_SUBMESSAGE_ENCRYPTED` and `SIGN` where it has not, each with origin
/// authentication where `IS_SUBMESSAGE_ORIGIN_AUTHENTICATED` is set; the data
/// kind likewise from `is_payload_protected` and `IS_PAYLOAD_ENCRYPTED`
/// (DDS-Security 1.2 §9.4.2.7 Table 37, `plugin_endpoint_attributes`).
#[must_use]
pub fn kinds_of_endpoint_config(
    config: &zerodds_security::access_control::EndpointSecurityConfig,
) -> (ProtectionKind, ProtectionKind) {
    use zerodds_security::cryptographic::{
        PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED,
        PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
    };
    let attributes = config.plugin_endpoint_attributes;
    let metadata = if !config.is_submessage_protected {
        ProtectionKind::None
    } else {
        let encrypt = attributes & PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED != 0;
        let origin = attributes & PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED != 0;
        match (encrypt, origin) {
            (true, true) => ProtectionKind::EncryptWithOriginAuthentication,
            (true, false) => ProtectionKind::Encrypt,
            (false, true) => ProtectionKind::SignWithOriginAuthentication,
            (false, false) => ProtectionKind::Sign,
        }
    };
    let data = if !config.is_payload_protected {
        ProtectionKind::None
    } else if attributes & PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED != 0 {
        ProtectionKind::Encrypt
    } else {
        ProtectionKind::Sign
    };
    (metadata, data)
}

/// The topic name a secure builtin endpoint is asked about when the
/// AccessControl plugin of the application gives its security config
/// (DDS-Security 1.2 §9.8.4, §7.5.1.6 to §7.5.1.8, §7.5.2).
#[must_use]
pub const fn builtin_topic_name(eid: EntityId) -> Option<&'static str> {
    match eid {
        EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER
        | EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER => {
            Some("DCPSParticipantsSecure")
        }
        EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
        | EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER => Some("DCPSPublicationsSecure"),
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER
        | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER => Some("DCPSSubscriptionsSecure"),
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
        | EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER => {
            Some("DCPSParticipantMessageSecure")
        }
        _ => None,
    }
}

/// [`builtin_endpoint_kinds`] where an AccessControl plugin of the
/// application is installed: for the secure builtin endpoints of
/// [`builtin_topic_name`] the plugin is asked
/// (`get_datawriter_security_config` / `get_datareader_security_config`,
/// DDS-Security 1.2 §9.8.4), and its config gives the kinds. Without such a
/// plugin, or for the other builtin endpoints (their protection is fixed by
/// Table 21), what [`builtin_endpoint_kinds`] says.
///
/// # Errors
/// The plugin fails: the caller reports it and uses
/// [`builtin_endpoint_kinds`], which protects as the governance does.
pub fn builtin_endpoint_kinds_asked(
    gate: &SharedSecurityGate,
    eid: EntityId,
    is_writer: bool,
) -> Result<(ProtectionKind, ProtectionKind), crate::SecurityGateError> {
    if let Some(topic) = builtin_topic_name(eid) {
        if gate.application_access_control()? {
            let request = zerodds_security::access_control::EndpointRequest::new(0, topic);
            let config = gate.endpoint_security_config(!is_writer, &request)?;
            return Ok(kinds_of_endpoint_config(&config));
        }
    }
    builtin_endpoint_kinds(gate, eid)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;

    fn gate(discovery: &str, liveliness: &str) -> SharedSecurityGate {
        let xml = alloc::format!(
            "<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><discovery_protection_kind>{discovery}</discovery_protection_kind><liveliness_protection_kind>{liveliness}</liveliness_protection_kind><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"
        );
        SharedSecurityGate::new(
            0,
            parse_governance_xml(&xml).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        )
    }

    fn submessage_protected(gate: &SharedSecurityGate, eid: EntityId) -> bool {
        builtin_endpoint_security_attributes(gate, eid)
            .unwrap()
            .is_submessage_protected
    }

    /// §7.5.13 Table 21: the values, column by column, with the governance of
    /// the domain deciding the two "set to match" columns.
    #[test]
    fn table_21_has_the_values_of_the_spec() {
        let protected = gate("ENCRYPT", "NONE");
        let open = gate("NONE", "ENCRYPT");
        for eid in [
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
        ] {
            assert!(submessage_protected(&protected, eid), "{eid:?}");
            assert!(!submessage_protected(&open, eid), "{eid:?}");
        }
        for eid in [
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
        ] {
            assert!(!submessage_protected(&protected, eid), "{eid:?}");
            assert!(submessage_protected(&open, eid), "{eid:?}");
        }
        for eid in [
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        ] {
            assert!(!submessage_protected(&protected, eid), "{eid:?}");
            assert!(!submessage_protected(&open, eid), "{eid:?}");
        }
        for eid in [
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            EntityId::TL_SVC_REQ_SECURE_WRITER,
            EntityId::TL_SVC_REQ_SECURE_READER,
            EntityId::TL_SVC_REPLY_SECURE_WRITER,
            EntityId::TL_SVC_REPLY_SECURE_READER,
        ] {
            assert!(submessage_protected(&protected, eid), "{eid:?}");
            assert!(submessage_protected(&open, eid), "{eid:?}");
        }
        // The other rows are `false` everywhere.
        let all = builtin_endpoint_security_attributes(
            &protected,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
        )
        .unwrap();
        assert!(!all.is_read_protected && !all.is_write_protected);
        assert!(!all.is_discovery_protected && !all.is_liveliness_protected);
        assert!(!all.is_payload_protected && !all.is_key_protected);
        // A user endpoint is none of them.
        assert_eq!(
            builtin_endpoint_security_attributes(
                &protected,
                EntityId::user_writer_with_key([0, 0, 1])
            ),
            None
        );
    }

    /// §7.5.4.2 Table 17: the volatile topic has all four attributes `false`.
    #[test]
    fn table_17_has_the_values_of_the_spec() {
        let a = volatile_topic_security_attributes();
        assert!(!a.is_read_protected);
        assert!(!a.is_write_protected);
        assert!(!a.is_discovery_protected);
        assert!(!a.is_liveliness_protected);
    }

    /// The kinds the key material of a builtin endpoint is made for follow
    /// Table 21: none where the submessages are not protected, the domain's
    /// kind for discovery and liveliness, ENCRYPT for the TypeLookup endpoints.
    #[test]
    fn the_kinds_follow_table_21() {
        let g = gate("SIGN", "ENCRYPT");
        let kinds = |eid| builtin_endpoint_kinds(&g, eid).unwrap();
        assert_eq!(
            kinds(EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER),
            (ProtectionKind::Sign, ProtectionKind::None)
        );
        assert_eq!(
            kinds(EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER),
            (ProtectionKind::Encrypt, ProtectionKind::None)
        );
        assert_eq!(
            kinds(EntityId::TL_SVC_REPLY_SECURE_READER),
            (ProtectionKind::Encrypt, ProtectionKind::None)
        );
        assert_eq!(
            kinds(EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER),
            (ProtectionKind::None, ProtectionKind::None)
        );
        let open = gate("NONE", "NONE");
        assert_eq!(
            builtin_endpoint_kinds(&open, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER)
                .unwrap(),
            (ProtectionKind::None, ProtectionKind::None)
        );
    }

    /// §9.8.4 and §7.5.13 Table 21: an AccessControl plugin of the application
    /// is asked for the secure endpoints whose protection follows the
    /// participant (discovery, liveliness) and is not asked for the
    /// VolatileMessageSecure and TypeLookup secure endpoints, whose
    /// `is_submessage_protected` Table 21 fixes to true. The plugin here
    /// protects nothing; the fixed endpoints are protected all the same.
    #[test]
    fn the_plugin_is_not_asked_for_the_endpoints_table_21_fixes() {
        use zerodds_security::access_control::PermissionsHandle;
        use zerodds_security::mock::MockAccessControlPlugin;
        let g = gate("ENCRYPT", "ENCRYPT");
        let (plugin, calls) = MockAccessControlPlugin::deciding(|_| true);
        g.set_access_control(alloc::boxed::Box::new(plugin), PermissionsHandle(7))
            .unwrap();
        // Asked: the plugin says nothing is protected, the answer is followed.
        assert_eq!(
            builtin_endpoint_kinds_asked(
                &g,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
                true
            )
            .unwrap(),
            (ProtectionKind::None, ProtectionKind::None)
        );
        let asked_before = calls.lock().unwrap().len();
        assert!(asked_before >= 1);
        // Not asked: the kinds of Table 21, as without a plugin.
        for (eid, is_writer) in [
            (
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
                true,
            ),
            (
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
                false,
            ),
            (EntityId::TL_SVC_REQ_SECURE_WRITER, true),
            (EntityId::TL_SVC_REQ_SECURE_READER, false),
            (EntityId::TL_SVC_REPLY_SECURE_WRITER, true),
            (EntityId::TL_SVC_REPLY_SECURE_READER, false),
        ] {
            assert_eq!(
                builtin_endpoint_kinds_asked(&g, eid, is_writer).unwrap(),
                builtin_endpoint_kinds(&g, eid).unwrap(),
                "{eid:?}"
            );
            assert_eq!(builtin_topic_name(eid), None, "{eid:?}");
        }
        assert_eq!(
            calls.lock().unwrap().len(),
            asked_before,
            "the plugin was not asked"
        );
    }
}
