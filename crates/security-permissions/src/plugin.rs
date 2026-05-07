// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `AccessControlPlugin`-Impl auf Basis der parsten Permissions-XML.

use alloc::collections::BTreeMap;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_security::access_control::{AccessControlPlugin, AccessDecision, PermissionsHandle};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;

use crate::topic_match::topic_match;
use crate::xml::{Grant, Permissions, parse_permissions_xml};

/// Property-Key fuer das Permissions-XML (inline als String).
pub const PROP_PERMISSIONS_XML: &str = "dds.sec.access.permissions";
/// Property-Key fuer das Subject-Name (CN aus dem X.509). Bis future-major
/// wird das explizit vom Caller gesetzt — spaeter direkt aus dem
/// `IdentityHandle` abgeleitet.
pub const PROP_SUBJECT_NAME: &str = "dds.sec.access.subject_name";

/// Access-Control-Plugin: erlaubt Topics nur, wenn sie im Permissions-
/// XML fuer den Subject-Name matchen.
pub struct PermissionsAccessControl {
    next_handle: AtomicU64,
    slots: BTreeMap<PermissionsHandle, Slot>,
}

struct Slot {
    subject_name: String,
    permissions: Permissions,
}

impl Default for PermissionsAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionsAccessControl {
    /// Konstruktor.
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

    /// Programmatischer Constructor fuer Slot — nuetzlich fuer Tests
    /// ohne PropertyList-Weg.
    pub fn register(
        &mut self,
        subject_name: String,
        permissions: Permissions,
    ) -> PermissionsHandle {
        let handle = PermissionsHandle(self.next_id());
        self.slots.insert(
            handle,
            Slot {
                subject_name,
                permissions,
            },
        );
        handle
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
                // Default-Allow (selten) — trotzdem respektieren.
                AccessDecision::Permit
            }
        }
    }
}

impl AccessControlPlugin for PermissionsAccessControl {
    fn validate_local_permissions(
        &mut self,
        _local: IdentityHandle,
        _participant_guid: [u8; 16],
        props: &PropertyList,
    ) -> SecurityResult<PermissionsHandle> {
        let xml = props.get(PROP_PERMISSIONS_XML).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "permissions: fehlt dds.sec.access.permissions",
            )
        })?;
        let subject = props.get(PROP_SUBJECT_NAME).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "permissions: fehlt dds.sec.access.subject_name",
            )
        })?;
        let perms = parse_permissions_xml(xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("permissions: {e}"),
            )
        })?;
        Ok(self.register(subject.to_string(), perms))
    }

    fn validate_remote_permissions(
        &mut self,
        _local: IdentityHandle,
        _remote: IdentityHandle,
        remote_permissions_token: &[u8],
        _remote_credential: &[u8],
    ) -> SecurityResult<PermissionsHandle> {
        // Remote-Permissions-Token = das Permissions-XML als UTF-8.
        // Subject-Name aus dem Credential extrahieren ist future-major —
        // hier nutzen wir den Token selbst als Subject-Quelle.
        let xml = core::str::from_utf8(remote_permissions_token).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "permissions: remote_permissions_token ist kein UTF-8",
            )
        })?;
        let perms = parse_permissions_xml(xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("permissions: {e}"),
            )
        })?;
        // Wir speichern den ersten Subject-Namen als den des Remote.
        let subject = perms
            .grants
            .first()
            .map(|g| g.subject_name.clone())
            .unwrap_or_default();
        Ok(self.register(subject, perms))
    }

    fn check_create_datawriter(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        let (_, g) = self.grant(perms).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "permissions: unbekannter PermissionsHandle",
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
                "permissions: unbekannter PermissionsHandle",
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
        // Match nur wenn der Remote-Writer auch wirklich publish-Recht
        // auf dem Topic hat.
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
        "DDS:Access:Permissions:1.2"
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

    fn build_alice() -> (PermissionsAccessControl, PermissionsHandle) {
        let mut ac = PermissionsAccessControl::new();
        let perms = parse_permissions_xml(ALICE_XML).unwrap();
        let h = ac.register("CN=alice".into(), perms);
        (ac, h)
    }

    #[test]
    fn permit_on_exact_match() {
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
        let d = ac.check_create_datawriter(h, "actuator_x").unwrap();
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
    fn plugin_class_id_matches_spec() {
        let ac = PermissionsAccessControl::new();
        assert_eq!(ac.plugin_class_id(), "DDS:Access:Permissions:1.2");
    }

    #[test]
    fn property_list_driver_loads_permissions() {
        let mut ac = PermissionsAccessControl::new();
        let props = PropertyList::new()
            .with(Property::local(PROP_PERMISSIONS_XML, ALICE_XML.to_string()))
            .with(Property::local(PROP_SUBJECT_NAME, "CN=alice"));
        let h = ac
            .validate_local_permissions(IdentityHandle(1), [0xAA; 16], &props)
            .expect("validate via props");
        assert_eq!(
            ac.check_create_datawriter(h, "Chatter").unwrap(),
            AccessDecision::Permit,
        );
    }

    #[test]
    fn missing_permissions_property_is_invalid_configuration() {
        let mut ac = PermissionsAccessControl::new();
        let props = PropertyList::new();
        let err = ac
            .validate_local_permissions(IdentityHandle(1), [0xAA; 16], &props)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }
}
