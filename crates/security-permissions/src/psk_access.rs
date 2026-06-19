// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Built-in pre-shared-key access-control plugin (spec §10.8).
//!
//! Spec class ID `"DDS:Access:PSK:Permissions:1.2"`. A variant of
//! [`crate::PermissionsAccessControl`] WITHOUT an S/MIME wrapper:
//!
//! * The permissions XML is read **directly** as a string (no
//!   PKCS#7 SignedData outer wrap, because the PSK path has no
//!   permissions CA at all).
//! * The governance XML (optional) is also plain.
//! * Authenticity stems from the pre-shared key — whoever knows the PSK
//!   counts as the configured subject.
//!
//! The topic-filter/allow/deny evaluation goes 1:1 through the
//! existing [`crate::xml`]/[`crate::governance`] pipeline; no code
//! in it is duplicated.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_security::access_control::{AccessControlPlugin, AccessDecision, PermissionsHandle};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;
use zerodds_security::token::DataHolder;

use zerodds_security_crypto::{PskCryptoPlugin, Suite};
use zerodds_security_pki::PskAuthenticationPlugin;

use crate::governance::{Governance, parse_governance_xml};
use crate::topic_match::topic_match;
use crate::xml::{Grant, Permissions, parse_permissions_xml};

/// Plugin-Class-Id (Spec §10.8).
pub const CLASS_ID_PSK_PERMISSIONS: &str = "DDS:Access:PSK:Permissions:1.2";

/// Property key for the permissions XML (local, not propagated).
pub const PROP_PSK_PERMISSIONS_XML: &str = "dds.sec.access.psk_permissions";
/// Property key for the governance XML (optional).
pub const PROP_PSK_GOVERNANCE_XML: &str = "dds.sec.access.psk_governance";
/// Property key for the subject name.
pub const PROP_PSK_SUBJECT_NAME: &str = "dds.sec.access.subject_name";
/// Property key for the permissions identity (in the wire token).
pub const PROP_PSK_PERMISSIONS_ID: &str = "dds.psk.permissions_id";

/// PSK access-control plugin. The holding container is the same
/// `Permissions` struct as the X.509 path — only the loader differs
/// (no S/MIME decode).
pub struct PskPermissionsAccessControl {
    next_handle: AtomicU64,
    slots: BTreeMap<PermissionsHandle, Slot>,
}

struct Slot {
    subject_name: String,
    permissions: Permissions,
    /// Optional: domain-level governance. Stored here only for
    /// spec compliance; active filters (discovery-protect,
    /// rtps_protection) live in the PSK crypto plugin on the wire path.
    #[allow(dead_code)]
    governance: Option<Governance>,
}

impl Default for PskPermissionsAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

impl PskPermissionsAccessControl {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            slots: BTreeMap::new(),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Programmatic constructor — registers a pre-parsed
    /// permissions structure for a subject.
    pub fn register(
        &mut self,
        subject_name: String,
        permissions: Permissions,
        governance: Option<Governance>,
    ) -> PermissionsHandle {
        let handle = PermissionsHandle(self.next_id());
        self.slots.insert(
            handle,
            Slot {
                subject_name,
                permissions,
                governance,
            },
        );
        handle
    }

    /// Creates a wire PermissionsToken in the PSK variant
    /// (`DDS:Access:PSK:Permissions:1.2`). The token body propagates
    /// only the `permissions_id` — the XML itself is not sent over the
    /// wire (the S/MIME wrap is missing on the PSK path).
    #[must_use]
    pub fn build_permissions_token(permissions_id: &str) -> alloc::vec::Vec<u8> {
        DataHolder::new(CLASS_ID_PSK_PERMISSIONS)
            .with_property(PROP_PSK_PERMISSIONS_ID, permissions_id)
            .to_cdr_le()
    }

    fn grant(&self, handle: PermissionsHandle) -> Option<(&str, Option<&Grant>)> {
        let slot = self.slots.get(&handle)?;
        let g = slot.permissions.find_grant(&slot.subject_name);
        Some((slot.subject_name.as_str(), g))
    }
}

fn topics_allow(patterns: &[String], topic: &str) -> bool {
    patterns.iter().any(|p| topic_match(p, topic))
}

fn decide(grant: Option<&Grant>, topic: &str, is_publish: bool) -> AccessDecision {
    match grant {
        None => AccessDecision::Deny,
        Some(g) => {
            let hit = if is_publish {
                topics_allow(&g.allow_publish_topics, topic)
            } else {
                topics_allow(&g.allow_subscribe_topics, topic)
            };
            if hit {
                AccessDecision::Permit
            } else if g.default_deny {
                AccessDecision::Deny
            } else {
                AccessDecision::Permit
            }
        }
    }
}

impl AccessControlPlugin for PskPermissionsAccessControl {
    fn validate_local_permissions(
        &mut self,
        _local: IdentityHandle,
        _participant_guid: [u8; 16],
        props: &PropertyList,
    ) -> SecurityResult<PermissionsHandle> {
        let xml = props.get(PROP_PSK_PERMISSIONS_XML).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "psk-access: missing dds.sec.access.psk_permissions",
            )
        })?;
        let subject = props.get(PROP_PSK_SUBJECT_NAME).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "psk-access: missing dds.sec.access.subject_name",
            )
        })?;
        let perms = parse_permissions_xml(xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("psk-access: permissions: {e}"),
            )
        })?;
        let governance = if let Some(gov_xml) = props.get(PROP_PSK_GOVERNANCE_XML) {
            Some(parse_governance_xml(gov_xml).map_err(|e| {
                SecurityError::new(
                    SecurityErrorKind::InvalidConfiguration,
                    alloc::format!("psk-access: governance: {e}"),
                )
            })?)
        } else {
            None
        };
        Ok(self.register(subject.to_string(), perms, governance))
    }

    fn validate_remote_permissions(
        &mut self,
        _local: IdentityHandle,
        _remote: IdentityHandle,
        remote_permissions_token: &[u8],
        remote_credential: &[u8],
    ) -> SecurityResult<PermissionsHandle> {
        // Spec §10.8 — the remote token contains only the permissions ID
        // (no S/MIME doc); the actual permissions XML comes
        // in the `remote_credential` (UTF-8). On the PSK path we trust
        // the binding because the authentication layer has already
        // validated the peer via the pre-shared key.
        let dh = DataHolder::from_cdr_le(remote_permissions_token).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-access: remote PermissionsToken not parseable",
            )
        })?;
        if dh.class_id != CLASS_ID_PSK_PERMISSIONS {
            return Err(SecurityError::new(
                SecurityErrorKind::AccessDenied,
                alloc::format!(
                    "psk-access: remote PermissionsToken has wrong class_id '{}'",
                    dh.class_id
                ),
            ));
        }
        let xml = core::str::from_utf8(remote_credential).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-access: remote_credential is not UTF-8",
            )
        })?;
        let perms = parse_permissions_xml(xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("psk-access: {e}"),
            )
        })?;
        let subject = perms
            .grants
            .first()
            .map(|g| g.subject_name.clone())
            .unwrap_or_default();
        Ok(self.register(subject, perms, None))
    }

    fn check_create_datawriter(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        let (_, g) = self.grant(perms).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-access: unknown PermissionsHandle",
            )
        })?;
        Ok(decide(g, topic_name, true))
    }

    fn check_create_datareader(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        let (_, g) = self.grant(perms).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-access: unknown PermissionsHandle",
            )
        })?;
        Ok(decide(g, topic_name, false))
    }

    fn check_remote_datawriter_match(
        &self,
        _local: PermissionsHandle,
        remote: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        self.check_create_datawriter(remote, topic_name)
    }

    fn check_remote_datareader_match(
        &self,
        _local: PermissionsHandle,
        remote: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        self.check_create_datareader(remote, topic_name)
    }

    fn plugin_class_id(&self) -> &str {
        CLASS_ID_PSK_PERMISSIONS
    }
}

// ---------------------------------------------------------------------
// PSK-Profile-Bundle (Spec §10.7-§10.9)
// ---------------------------------------------------------------------

/// Bundle of the three PSK plugins for a simple caller path.
/// Spec ref: §10.7 auth, §10.8 access, §10.9 crypto.
pub struct PskProfile {
    /// Authentication plugin (`DDS:Auth:PSK:1.2`).
    pub auth: PskAuthenticationPlugin,
    /// Access-control plugin (`DDS:Access:PSK:Permissions:1.2`).
    pub access_control: PskPermissionsAccessControl,
    /// Crypto plugin (`DDS:Crypto:PSK:AES-GCM-GMAC:1.2`).
    pub crypto: PskCryptoPlugin,
}

impl PskProfile {
    /// Creates a PSK profile bundle. Configures:
    ///
    /// 1. Authentication with `(identity_id, pre_shared_key)`.
    /// 2. Access control with `governance_xml` (optional) +
    ///    `permissions_xml` (plain, without an S/MIME wrap).
    /// 3. Crypto with `Aes128Gcm` (default suite) and `psk_id=1`.
    ///
    /// # Errors
    /// Configuration/XML errors are passed through as
    /// [`zerodds_security::SecurityError`].
    pub fn with_pre_shared_key(
        identity_id: &str,
        key: alloc::vec::Vec<u8>,
        governance_xml: Option<&str>,
        permissions_xml: &str,
    ) -> SecurityResult<Self> {
        let mut auth = PskAuthenticationPlugin::new();
        auth.register_psk(identity_id.to_string(), key.clone())?;
        auth.validate_local_psk_identity(identity_id)?;

        let mut access = PskPermissionsAccessControl::new();
        let perms = parse_permissions_xml(permissions_xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("psk-profile: permissions: {e}"),
            )
        })?;
        let governance = governance_xml
            .map(|x| {
                parse_governance_xml(x).map_err(|e| {
                    SecurityError::new(
                        SecurityErrorKind::InvalidConfiguration,
                        alloc::format!("psk-profile: governance: {e}"),
                    )
                })
            })
            .transpose()?;
        access.register(identity_id.to_string(), perms, governance);

        let mut crypto = PskCryptoPlugin::with_suite(Suite::Aes128Gcm);
        crypto.register_psk(1, key)?;

        Ok(Self {
            auth,
            access_control: access,
            crypto,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::properties::Property;

    const ALICE_XML: &str = r#"
<permissions>
  <grant><subject_name>CN=alice</subject_name>
    <allow_rule>
      <publish><topic>Chatter</topic><topic>sensor_*</topic></publish>
      <subscribe><topic>Echo</topic></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions>
"#;

    fn build_alice() -> (PskPermissionsAccessControl, PermissionsHandle) {
        let mut ac = PskPermissionsAccessControl::new();
        let perms = parse_permissions_xml(ALICE_XML).unwrap();
        let h = ac.register("CN=alice".into(), perms, None);
        (ac, h)
    }

    #[test]
    fn plugin_class_id_matches_spec() {
        let ac = PskPermissionsAccessControl::new();
        assert_eq!(ac.plugin_class_id(), "DDS:Access:PSK:Permissions:1.2");
    }

    #[test]
    fn permit_on_exact_match_uses_same_topic_logic_as_x509() {
        let (ac, h) = build_alice();
        let d = ac.check_create_datawriter(h, "Chatter").unwrap();
        assert_eq!(d, AccessDecision::Permit);
    }

    #[test]
    fn permit_on_wildcard_match() {
        let (ac, h) = build_alice();
        let d = ac.check_create_datawriter(h, "sensor_temp").unwrap();
        assert_eq!(d, AccessDecision::Permit);
    }

    #[test]
    fn deny_on_non_matching_topic() {
        let (ac, h) = build_alice();
        let d = ac.check_create_datawriter(h, "actuator").unwrap();
        assert_eq!(d, AccessDecision::Deny);
    }

    #[test]
    fn deny_writer_when_only_subscribe_granted() {
        let (ac, h) = build_alice();
        let d = ac.check_create_datawriter(h, "Echo").unwrap();
        assert_eq!(d, AccessDecision::Deny);
    }

    #[test]
    fn permit_reader_on_subscribe_allowed_topic() {
        let (ac, h) = build_alice();
        let d = ac.check_create_datareader(h, "Echo").unwrap();
        assert_eq!(d, AccessDecision::Permit);
    }

    #[test]
    fn property_list_driver_loads_permissions_without_smime() {
        let mut ac = PskPermissionsAccessControl::new();
        let props = PropertyList::new()
            .with(Property::local(
                PROP_PSK_PERMISSIONS_XML,
                ALICE_XML.to_string(),
            ))
            .with(Property::local(PROP_PSK_SUBJECT_NAME, "CN=alice"));
        let h = ac
            .validate_local_permissions(IdentityHandle(1), [0xAA; 16], &props)
            .expect("psk-access: validate via props");
        assert_eq!(
            ac.check_create_datawriter(h, "Chatter").unwrap(),
            AccessDecision::Permit,
        );
    }

    #[test]
    fn missing_permissions_property_is_invalid_configuration() {
        let mut ac = PskPermissionsAccessControl::new();
        let err = ac
            .validate_local_permissions(IdentityHandle(1), [0xAA; 16], &PropertyList::new())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    #[test]
    fn permissions_token_class_id_matches_spec() {
        let bytes = PskPermissionsAccessControl::build_permissions_token("perm-id-1");
        let dh = DataHolder::from_cdr_le(&bytes).unwrap();
        assert_eq!(dh.class_id, CLASS_ID_PSK_PERMISSIONS);
        assert_eq!(dh.property(PROP_PSK_PERMISSIONS_ID), Some("perm-id-1"));
    }

    #[test]
    fn validate_remote_permissions_rejects_wrong_class_id() {
        let mut ac = PskPermissionsAccessControl::new();
        let bogus = DataHolder::new("DDS:Access:Permissions:1.2")
            .with_property(PROP_PSK_PERMISSIONS_ID, "x")
            .to_cdr_le();
        let err = ac
            .validate_remote_permissions(IdentityHandle(1), IdentityHandle(2), &bogus, b"")
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AccessDenied);
    }

    #[test]
    fn validate_remote_permissions_happy_path() {
        let mut ac = PskPermissionsAccessControl::new();
        let token = PskPermissionsAccessControl::build_permissions_token("alice");
        let h = ac
            .validate_remote_permissions(
                IdentityHandle(1),
                IdentityHandle(2),
                &token,
                ALICE_XML.as_bytes(),
            )
            .unwrap();
        assert_eq!(
            ac.check_create_datawriter(h, "Chatter").unwrap(),
            AccessDecision::Permit
        );
    }
}
