// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// zerodds-lint: allow no_dyn_in_safe

//! `DDS:Access:Permissions` (DDS-Security 1.2 §10.4): the builtin
//! `AccessControlPlugin` over the governance and permissions documents.
//!
//! The plugin implements every operation of Table 39 as Table 70 defines
//! it. It holds the Governance document of the participant and, per
//! [`PermissionsHandle`], the grant of one participant: the local one
//! ([`AccessControlPlugin::validate_local_permissions`]) or a remote one
//! ([`AccessControlPlugin::validate_remote_permissions`]). The time the
//! validity of a grant is checked against comes from the clock of the plugin
//! ([`PermissionsAccessControl::with_clock`], the system clock by default).
//!
//! Without a Governance document ([`PermissionsAccessControl::new`] and no
//! [`PermissionsAccessControl::set_governance`]) every topic is
//! access-controlled and the grant alone decides; the operations that need
//! the Governance (the three `get_*_security_config` and the join check of
//! the domain rule) then fail.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_security::access_control::{
    AccessControlListener, AccessControlPlugin, AccessDecision, EndpointRequest,
    EndpointSecurityConfig, ParticipantRequest, ParticipantSecurityAlgorithmInfo,
    ParticipantSecurityConfig, PermissionsHandle, RelayRole, RemoteReaderDecision, TopicRequest,
    TopicSecurityConfig, plugin_endpoint_attributes_mask as endpoint_bits,
    plugin_participant_attributes_mask as participant_bits,
};
use zerodds_security::algorithm::{
    CRYPTO_ALGORITHM_COMPATIBILITY_MODE, CRYPTO_ALGORITHM_SET_ALL, CRYPTO_ALGORITHM_SET_EMPTY,
};
use zerodds_security::authentication::{AlgorithmMasks, IdentityHandle};
use zerodds_security::crypto::{
    ENDPOINT_DATA_PROTECTION_KIND, ENDPOINT_METADATA_PROTECTION_KIND,
    PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::Property;
use zerodds_security::token::{
    DataHolder, PermissionsCredentialToken, PermissionsToken, class_id, prop,
};

use crate::cms::{CmsPkcs7Verifier, PROP_PERMISSIONS_CA};
use crate::governance::{Domain, Governance, ProtectionKind, ReaderAccess, parse_governance_xml};
use crate::signature::{XmlSignatureVerifier, open_signed_permissions};
use crate::xml::{
    AccessRequest, Action, DataTag, Grant, PartitionMatching, Permissions, PermissionsError,
    parse_permissions_xml, subject_name_attributes,
};

/// Property key for the permissions document: inline plain XML, or the
/// signed S/MIME document (Table 63 `dds.sec.access.permissions`).
pub const PROP_PERMISSIONS_XML: &str = "dds.sec.access.permissions";
/// Property key for the subject name (CN from the X.509). Used with a plain
/// permissions XML, which has no identity certificate to take it from.
pub const PROP_SUBJECT_NAME: &str = "org.zerodds.sec.access.subject_name";
/// Property key for the governance document: inline plain XML, or the signed
/// S/MIME document (Table 63 `dds.sec.access.governance`).
pub const PROP_GOVERNANCE: &str = "dds.sec.access.governance";
/// Property key for the PEM of the identity certificate (Table 50
/// `dds.sec.auth.identity_certificate`), where the subject name of a signed
/// permissions document is taken from.
pub const PROP_IDENTITY_CERTIFICATE: &str = "dds.sec.auth.identity_certificate";
/// Named setting: the `class_id` of the PermissionsToken the plugin sends.
/// Table 65 gives `DDS:Access:Permissions:1.2`, the text below it
/// MinorVersion 0, and other implementations send
/// `DDS:Access:Permissions:1.0`. A receiver compares PluginClassName and
/// MajorVersion only (Table 70), so either value is read by every peer.
pub const PROP_TOKEN_CLASS_ID: &str = "org.zerodds.sec.access.permissions_token_class_id";
/// The property of the PermissionsCredentialToken that holds the signed
/// permissions document (Table 64).
pub const PROP_PERM_CERT: &str = "dds.perm.cert";

/// The `minor_code` of the `SecurityException` of a refused
/// `validate_remote_permissions` and `get_permissions_credential_token`.
pub mod minor_code {
    /// No Permissions CA is armed: the document cannot be authenticated.
    pub const NO_VERIFIER: i32 = 1;
    /// The signature does not verify against the Permissions CA.
    pub const NOT_VERIFIED: i32 = 2;
    /// No grant of the document names the subject of the certificate.
    pub const NO_GRANT_FOR_SUBJECT: i32 = 3;
    /// The credential token carries no `c.id` or `c.perm`.
    pub const NO_CREDENTIAL: i32 = 4;
    /// `c.id` is no X.509 certificate.
    pub const NOT_A_CERTIFICATE: i32 = 5;
}

/// The seconds since the Unix epoch.
fn system_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Access-control plugin `DDS:Access:Permissions`.
pub struct PermissionsAccessControl {
    next_handle: u64,
    slots: BTreeMap<PermissionsHandle, Slot>,
    /// The Governance document of the local participant (Table 70 reads it
    /// through the handle; it is the same for all handles of the plugin).
    governance: Option<Governance>,
    /// The DomainTag of the local participant (§10.4.1.2.5.1).
    domain_tag: String,
    /// The permissions CA a remote participant's signed permissions
    /// document is verified against (DDS-Security 1.2 §9.4.1.2.1: the
    /// document "shall be signed by the Permissions CA"). Without it,
    /// [`AccessControlPlugin::validate_remote_permissions`] refuses every
    /// remote token instead of reading it unverified.
    remote_verifier: Option<CmsPkcs7Verifier>,
    listener: Option<Arc<dyn AccessControlListener>>,
    clock: fn() -> u64,
    token_class_id: String,
    /// The form of `dds.perm_ca.sn` in the PermissionsToken.
    subject_name_form: zerodds_security::SubjectNameForm,
    /// The names of `dds.perm_ca.algo` in the PermissionsToken.
    algorithm_names: zerodds_security_pki::AlgorithmNames,
    issued_tokens: Vec<PermissionsToken>,
    issued_credential_tokens: Vec<PermissionsCredentialToken>,
    issued_participant_configs: Vec<ParticipantSecurityConfig>,
    issued_topic_configs: Vec<TopicSecurityConfig>,
    issued_writer_configs: Vec<EndpointSecurityConfig>,
    issued_reader_configs: Vec<EndpointSecurityConfig>,
}

/// What a handle stands for.
struct Slot {
    /// `true` for the local participant, `false` for a remote one.
    local: bool,
    subject_name: String,
    /// The document reduced to the grant of the subject.
    permissions: Permissions,
    /// The domain the local participant was validated for.
    domain_id: Option<u32>,
    /// A remote participant: the `class_id` of its PermissionsToken.
    token_class_id: Option<String>,
    /// The local participant: its signed permissions document.
    signed_permissions: Option<Vec<u8>>,
    /// The local participant: the algorithms of its identity certificate.
    identity_algorithms: Option<AlgorithmMasks>,
}

impl Default for PermissionsAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionsAccessControl {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: 0,
            slots: BTreeMap::new(),
            governance: None,
            domain_tag: String::new(),
            remote_verifier: None,
            listener: None,
            clock: system_seconds,
            token_class_id: class_id::ACCESS_PERMISSIONS_V12.to_string(),
            subject_name_form: zerodds_security::SubjectNameForm::default(),
            algorithm_names: zerodds_security_pki::AlgorithmNames::default(),
            issued_tokens: Vec::new(),
            issued_credential_tokens: Vec::new(),
            issued_participant_configs: Vec::new(),
            issued_topic_configs: Vec::new(),
            issued_writer_configs: Vec::new(),
            issued_reader_configs: Vec::new(),
        }
    }

    /// The plugin with the Governance document `governance`.
    #[must_use]
    pub fn with_governance(mut self, governance: Governance) -> Self {
        self.governance = Some(governance);
        self
    }

    /// Sets the Governance document of the participant.
    pub fn set_governance(&mut self, governance: Governance) {
        self.governance = Some(governance);
    }

    /// The Governance document, if the plugin has one.
    #[must_use]
    pub fn governance(&self) -> Option<&Governance> {
        self.governance.as_ref()
    }

    /// The plugin with the clock `clock` (seconds since the Unix epoch) that
    /// the validity of the grants is checked against.
    #[must_use]
    pub fn with_clock(mut self, clock: fn() -> u64) -> Self {
        self.clock = clock;
        self
    }

    /// The DomainTag of the local participant; the domain rules of the
    /// Governance and the rules of the grants match on it.
    pub fn set_domain_tag(&mut self, tag: &str) {
        self.domain_tag = tag.to_string();
    }

    /// Sets the `class_id` of the PermissionsToken ([`PROP_TOKEN_CLASS_ID`]).
    ///
    /// # Errors
    /// `InvalidConfiguration` for a class that is not of PluginClassName
    /// `DDS:Access:Permissions` and MajorVersion 1: a peer would not read it.
    pub fn set_token_class_id(&mut self, class: &str) -> SecurityResult<()> {
        if !class_id::same_class_and_major_version(class, class_id::ACCESS_PERMISSIONS_V12) {
            return Err(SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!(
                    "permissions: {PROP_TOKEN_CLASS_ID} = '{class}' is no class of \
                     DDS:Access:Permissions, major version 1"
                ),
            ));
        }
        self.token_class_id = class.to_string();
        Ok(())
    }

    /// Sets the form of `dds.perm_ca.sn` in the PermissionsToken, see
    /// [`zerodds_security::SubjectNameForm`]; RFC 4514 by default
    /// (DDS-Security 1.2 §9.4.1.2.3).
    pub fn set_subject_name_form(&mut self, form: zerodds_security::SubjectNameForm) {
        self.subject_name_form = form;
    }

    /// Sets the names `dds.perm_ca.algo` carries in the PermissionsToken
    /// (`org.zerodds.sec.algorithm_names`): the name of DDS-Security 1.2 §8.2
    /// Table 25 by default, the one of 1.1 (`EC-prime256v1`, `RSA-2048`) with
    /// [`zerodds_security_pki::AlgorithmNames::V1_1`]. The token goes to every
    /// peer in the SPDP announcement, before any peer version is known, so
    /// `Auto` gives the names of Table 25.
    pub fn set_algorithm_names(&mut self, names: zerodds_security_pki::AlgorithmNames) {
        self.algorithm_names = names;
    }

    /// Arms the permissions CA (PEM bundle) that a remote permissions
    /// token is verified against, as the runtime's gate does with
    /// `arm_remote_permissions_verifier`.
    ///
    /// # Errors
    /// [`PermissionsError`] for a CA PEM that does not parse.
    pub fn arm_remote_permissions_verifier(
        &mut self,
        ca_pem: &[u8],
    ) -> Result<(), PermissionsError> {
        self.remote_verifier = Some(CmsPkcs7Verifier::new(ca_pem)?);
        Ok(())
    }

    fn next_id(&mut self) -> u64 {
        self.next_handle += 1;
        self.next_handle
    }

    fn insert(&mut self, slot: Slot) -> PermissionsHandle {
        let handle = PermissionsHandle(self.next_id());
        self.slots.insert(handle, slot);
        handle
    }

    /// Programmatic constructor for a slot — useful for tests
    /// without the PropertyList path.
    pub fn register(
        &mut self,
        subject_name: String,
        permissions: Permissions,
    ) -> PermissionsHandle {
        self.insert(Slot {
            local: true,
            subject_name,
            permissions,
            domain_id: None,
            token_class_id: None,
            signed_permissions: None,
            identity_algorithms: None,
        })
    }

    /// A remote participant, from a permissions document the caller verified
    /// itself and reduced to the grant of the participant's subject, with the
    /// `class_id` of the participant's PermissionsToken (compared by the
    /// checks of Table 70). [`AccessControlPlugin::validate_remote_permissions`]
    /// does the verification itself.
    pub fn register_remote(
        &mut self,
        permissions: Permissions,
        token_class_id: &str,
    ) -> PermissionsHandle {
        let subject_name = permissions
            .grants
            .first()
            .map(|g| g.subject_text().to_string())
            .unwrap_or_default();
        self.insert(Slot {
            local: false,
            subject_name,
            permissions,
            domain_id: None,
            token_class_id: Some(token_class_id.to_string()),
            signed_permissions: None,
            identity_algorithms: None,
        })
    }

    /// The local participant, from documents the caller verified itself:
    /// the Governance, the permissions reduced to the grant of the
    /// participant's subject, the signed permissions document (for
    /// [`AccessControlPlugin::get_permissions_credential_token`]) and the
    /// PEM of the Permissions CA. The caller did what Table 70 asks of
    /// `validate_local_permissions` (signatures, subject).
    ///
    /// # Errors
    /// `InvalidConfiguration` for a CA PEM that does not parse.
    pub fn register_local(
        &mut self,
        governance: Governance,
        permissions: Permissions,
        signed_permissions: Vec<u8>,
        permissions_ca_pem: &[u8],
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle> {
        self.remote_verifier = Some(CmsPkcs7Verifier::new(permissions_ca_pem).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("permissions: the permissions CA: {e}"),
            )
        })?);
        let subject_name = permissions
            .grants
            .first()
            .map(|g| g.subject_text().to_string())
            .unwrap_or_default();
        self.governance = Some(governance);
        self.domain_tag = participant.domain_tag.to_string();
        Ok(self.insert(Slot {
            local: true,
            subject_name,
            permissions,
            domain_id: Some(participant.domain_id),
            token_class_id: None,
            signed_permissions: Some(signed_permissions),
            identity_algorithms: participant.identity_algorithms,
        }))
    }

    /// The local participant from a permissions document the caller parsed
    /// (and verified) itself and reduced to the grant of the participant's
    /// subject; the plugin has no signed document of it, so
    /// [`AccessControlPlugin::get_permissions_credential_token`] fails for
    /// the handle. The Governance is the one set with
    /// [`Self::set_governance`].
    pub fn register_local_parsed(
        &mut self,
        permissions: Permissions,
        participant: &ParticipantRequest<'_>,
    ) -> PermissionsHandle {
        let subject_name = permissions
            .grants
            .first()
            .map(|g| g.subject_text().to_string())
            .unwrap_or_default();
        self.domain_tag = participant.domain_tag.to_string();
        self.insert(Slot {
            local: true,
            subject_name,
            permissions,
            domain_id: Some(participant.domain_id),
            token_class_id: None,
            signed_permissions: None,
            identity_algorithms: participant.identity_algorithms,
        })
    }

    /// `validate_local_permissions` of Table 70 on the documents themselves:
    /// the signature of the Governance and of the permissions document is
    /// verified against the Permissions CA `permissions_ca_pem`, the
    /// document is reduced to the grant whose subject is that of the
    /// identity certificate `identity_certificate` (PEM or DER), and the
    /// plugin caches both.
    ///
    /// # Errors
    /// `InvalidConfiguration` for a CA that does not parse; `AccessDenied`
    /// for a signature that does not verify and for a document without a
    /// grant for the certificate's subject.
    pub fn validate_local_signed(
        &mut self,
        identity_certificate: &[u8],
        governance_signed: &[u8],
        permissions_signed: &[u8],
        permissions_ca_pem: &[u8],
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle> {
        let verifier = CmsPkcs7Verifier::new(permissions_ca_pem).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("permissions: the permissions CA: {e}"),
            )
        })?;
        let refused = |what: &str, e: PermissionsError| {
            SecurityError::new(
                SecurityErrorKind::AccessDenied,
                alloc::format!("permissions: {what}: {e}"),
            )
            .with_minor_code(minor_code::NOT_VERIFIED)
        };
        let governance_xml = verifier
            .verify_and_extract(governance_signed)
            .map_err(|e| refused("the governance does not verify against the CA", e))?;
        let governance_xml = core::str::from_utf8(&governance_xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("permissions: the governance is no UTF-8: {e}"),
            )
        })?;
        let governance = parse_governance_xml(governance_xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("permissions: the governance: {e}"),
            )
        })?;
        let all = open_signed_permissions(permissions_signed, &verifier)
            .map_err(|e| refused("the permissions do not verify against the CA", e))?;
        let subject = certificate_subject(identity_certificate)?;
        let permissions = all.for_subject(&subject).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AccessDenied,
                "permissions: no grant of the permissions names the identity certificate's subject",
            )
            .with_minor_code(minor_code::NO_GRANT_FOR_SUBJECT)
        })?;
        self.register_local(
            governance,
            permissions,
            permissions_signed.to_vec(),
            permissions_ca_pem,
            participant,
        )
    }

    /// Revokes the permissions of `handle` (§9.4.2.10): the handle is gone
    /// for every check afterwards, and the listener is called. `false` when
    /// the handle is unknown or the listener returns `false`.
    pub fn revoke_permissions(&mut self, handle: PermissionsHandle) -> bool {
        if self.slots.remove(&handle).is_none() {
            return false;
        }
        match &self.listener {
            Some(listener) => listener.on_revoke_permissions(&self.token_class_id, handle),
            None => true,
        }
    }

    /// Revokes the handles whose grants are all expired at the time of the
    /// plugin's clock, and returns them.
    pub fn revoke_expired(&mut self) -> Vec<PermissionsHandle> {
        let now = (self.clock)();
        let expired: Vec<PermissionsHandle> = self
            .slots
            .iter()
            .filter(|(_, slot)| {
                !slot.permissions.grants.is_empty()
                    && slot.permissions.grants.iter().all(|g| !g.is_valid_at(now))
            })
            .map(|(handle, _)| *handle)
            .collect();
        for handle in &expired {
            self.revoke_permissions(*handle);
        }
        expired
    }

    // ---- helpers of the checks ------------------------------------------

    fn now(&self) -> u64 {
        (self.clock)()
    }

    fn slot(&self, handle: PermissionsHandle) -> SecurityResult<&Slot> {
        self.slots.get(&handle).ok_or_else(unknown_handle)
    }

    /// The slot of a handle a check is asked about: `None` for
    /// [`PermissionsHandle::NIL`] (a participant whose permissions are not
    /// known), the slot otherwise.
    fn slot_or_nil(&self, handle: PermissionsHandle) -> SecurityResult<Option<&Slot>> {
        if handle.is_nil() {
            Ok(None)
        } else {
            self.slot(handle).map(Some)
        }
    }

    fn domain<'a>(&'a self, id: u32) -> Domain<'a> {
        Domain {
            id,
            tag: &self.domain_tag,
        }
    }

    /// Table 70: "If the PluginClassName or the MajorVersion of the local
    /// permissions_token differ from those in the remote_permissions_token,
    /// the operation shall return FALSE."
    fn token_matches(&self, slot: &Slot) -> bool {
        slot.token_class_id.as_deref().is_none_or(|remote| {
            class_id::same_class_and_major_version(&self.token_class_id, remote)
        })
    }

    fn data_tags(request: &EndpointRequest<'_>) -> Vec<DataTag> {
        request
            .data_tags
            .iter()
            .map(|(name, value)| DataTag {
                name: (*name).into(),
                value: (*value).into(),
            })
            .collect()
    }

    /// May the participant of `permissions` publish (`Action::Publish`) or
    /// subscribe the endpoint of `request`? The Governance first (a topic no
    /// rule covers is refused, a topic without access control is open), then
    /// the grant.
    fn endpoint_access(
        &self,
        permissions: &Permissions,
        action: Action,
        request: &EndpointRequest<'_>,
    ) -> ReaderAccess {
        let now = self.now();
        let domain = self.domain(request.domain_id);
        let tags = Self::data_tags(request);
        match (&self.governance, action) {
            (Some(governance), Action::Publish) => {
                if governance.check_datawriter(
                    permissions,
                    domain,
                    request.topic_name,
                    request.partitions,
                    &tags,
                    now,
                ) {
                    ReaderAccess::Allowed
                } else {
                    ReaderAccess::Denied
                }
            }
            (Some(governance), _) => governance.check_datareader(
                permissions,
                domain,
                request.topic_name,
                request.partitions,
                &tags,
                now,
            ),
            (None, _) => {
                let ask = |action| {
                    permissions.grants.iter().any(|g| {
                        g.is_valid_at(now)
                            && g.decide(
                                &AccessRequest {
                                    action,
                                    domain,
                                    topic: request.topic_name,
                                    partitions: request.partitions,
                                    data_tags: &tags,
                                },
                                PartitionMatching::default(),
                            )
                            .allowed()
                    })
                };
                if ask(action) {
                    ReaderAccess::Allowed
                } else if action == Action::Subscribe && ask(Action::Relay) {
                    ReaderAccess::RelayOnly
                } else {
                    ReaderAccess::Denied
                }
            }
        }
    }

    /// `true` when the Governance leaves the topic of `request` without
    /// access control for `action` (`enable_write_access_control` or
    /// `enable_read_access_control` FALSE): no grant is needed.
    fn open_topic(&self, action: Action, request_domain: u32, topic: &str) -> bool {
        self.governance.as_ref().is_some_and(|governance| {
            let domain = self.domain(request_domain);
            governance.has_topic_rule(domain, topic)
                && if action == Action::Publish {
                    !governance.write_access_controlled(domain, topic)
                } else {
                    !governance.read_access_controlled(domain, topic)
                }
        })
    }

    /// `false` when the Governance has no topic rule for the topic
    /// (§10.4.1.2.7: the operation fails).
    fn topic_has_rule(&self, domain_id: u32, topic: &str) -> bool {
        self.governance
            .as_ref()
            .is_none_or(|governance| governance.has_topic_rule(self.domain(domain_id), topic))
    }

    /// The access the participant of a remote handle has to the remote
    /// endpoint of `request` (Table 70, `check_remote_datawriter` and
    /// `check_remote_datareader`).
    fn remote_endpoint_access(
        &self,
        remote: PermissionsHandle,
        action: Action,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<ReaderAccess> {
        let slot = self.slot_or_nil(remote)?;
        if !self.topic_has_rule(request.domain_id, request.topic_name) {
            return Ok(ReaderAccess::Denied);
        }
        if self.open_topic(action, request.domain_id, request.topic_name) {
            return Ok(ReaderAccess::Allowed);
        }
        let Some(slot) = slot else {
            return Ok(ReaderAccess::Denied);
        };
        if !self.token_matches(slot) {
            return Ok(ReaderAccess::Denied);
        }
        Ok(self.endpoint_access(&slot.permissions, action, request))
    }

    /// The access of the local participant to create the endpoint.
    fn local_endpoint_access(
        &self,
        permissions: PermissionsHandle,
        action: Action,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<bool> {
        let slot = self.slot(permissions)?;
        Ok(self.endpoint_access(&slot.permissions, action, request) == ReaderAccess::Allowed)
    }

    /// Table 70, `check_create_topic`/`check_remote_topic`.
    fn topic_access(&self, permissions: &Permissions, request: &TopicRequest<'_>) -> bool {
        let now = self.now();
        let domain = self.domain(request.domain_id);
        permissions
            .grants
            .iter()
            .any(|g| g.is_valid_at(now) && g.allows_topic(domain, request.topic_name))
    }

    /// The topic rule of the Governance, for the three configs.
    fn topic_rule(
        &self,
        domain_id: u32,
        topic: &str,
    ) -> SecurityResult<crate::governance::TopicRule> {
        let governance = self.governance.as_ref().ok_or_else(no_governance)?;
        let domain = self.domain(domain_id);
        if !governance.has_topic_rule(domain, topic) {
            return Err(SecurityError::new(
                SecurityErrorKind::AccessDenied,
                alloc::format!("permissions: no topic rule of the governance covers '{topic}'"),
            ));
        }
        Ok(governance.effective_topic_rule(domain, topic))
    }

    fn participant_config_of(&self, slot: &Slot) -> SecurityResult<ParticipantSecurityConfig> {
        let governance = self.governance.as_ref().ok_or_else(no_governance)?;
        let domain_id = slot.domain_id.ok_or_else(|| {
            SecurityError::bad_argument(
                "permissions: get_participant_security_config needs the handle of the local \
                 participant",
            )
        })?;
        let rule = governance
            .find_domain_rule(self.domain(domain_id))
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::AccessDenied,
                    alloc::format!(
                        "permissions: no domain rule of the governance covers {domain_id}"
                    ),
                )
            })?;
        let mut plugin_attributes = 0;
        for (set, bit) in [
            (
                rule.rtps_protection_kind.is_encrypted(),
                participant_bits::IS_RTPS_AXK_ENCRYPTED,
            ),
            (
                rule.discovery_protection_kind.is_encrypted(),
                participant_bits::IS_DISCOVERY_ENCRYPTED,
            ),
            (
                rule.liveliness_protection_kind.is_encrypted(),
                participant_bits::IS_LIVELINESS_ENCRYPTED,
            ),
            (
                rule.rtps_protection_kind.is_origin_authenticated(),
                participant_bits::IS_RTPS_ORIGIN_AUTHENTICATED,
            ),
            (
                rule.discovery_protection_kind.is_origin_authenticated(),
                participant_bits::IS_DISCOVERY_ORIGIN_AUTHENTICATED,
            ),
            (
                rule.liveliness_protection_kind.is_origin_authenticated(),
                participant_bits::IS_LIVELINESS_ORIGIN_AUTHENTICATED,
            ),
            (
                rule.rtps_psk_protection_kind == ProtectionKind::Encrypt,
                participant_bits::IS_RTPS_PSK_ENCRYPTED,
            ),
        ] {
            if set {
                plugin_attributes |= bit;
            }
        }
        // What the Cryptographic plugin needs of the Governance beyond the
        // mask: `register_local_participant` takes it as a property.
        let mut ac_participant_properties = Vec::new();
        if rule.rtps_protection_kind.is_origin_authenticated() {
            ac_participant_properties.push(Property::local(
                PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
                "true",
            ));
        }
        Ok(ParticipantSecurityConfig {
            allow_unauthenticated_participants: rule.allow_unauthenticated_participants,
            is_access_protected: rule.enable_join_access_control,
            is_rtps_axk_protected: rule.rtps_protection_kind.is_protected(),
            is_rtps_psk_protected: rule.rtps_psk_protection_kind.is_protected(),
            is_discovery_protected: rule.discovery_protection_kind.is_protected(),
            is_liveliness_protected: rule.liveliness_protection_kind.is_protected(),
            is_key_revision_enabled: rule.enable_key_revision,
            plugin_participant_attributes: plugin_attributes,
            ac_participant_properties,
            algorithm_info: algorithm_info(
                rule.allowed_crypto_algorithms,
                slot.identity_algorithms,
            )?,
        })
    }

    fn endpoint_config(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.slot(permissions)?;
        let rule = self.topic_rule(request.domain_id, request.topic_name)?;
        let metadata = rule.metadata_protection_kind;
        let data = rule.data_protection_kind;
        let mut plugin_attributes = 0;
        for (set, bit) in [
            (
                metadata.is_encrypted(),
                endpoint_bits::IS_SUBMESSAGE_ENCRYPTED,
            ),
            (data.is_encrypted(), endpoint_bits::IS_PAYLOAD_ENCRYPTED),
            (
                metadata.is_origin_authenticated(),
                endpoint_bits::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
            ),
        ] {
            if set {
                plugin_attributes |= bit;
            }
        }
        // The symmetric ciphers the Governance allows are what the
        // endpoint's key material may use; what it requires of a peer is the
        // Cryptographic plugin's to say (Table 70 leaves it open).
        let allowed = self
            .governance
            .as_ref()
            .and_then(|g| g.find_domain_rule(self.domain(request.domain_id)))
            .and_then(|r| r.allowed_crypto_algorithms)
            .and_then(|a| a.symmetric_cipher)
            .unwrap_or(CRYPTO_ALGORITHM_SET_ALL);
        Ok(EndpointSecurityConfig {
            topic: topic_config(&rule),
            is_submessage_protected: metadata.is_protected(),
            is_payload_protected: data.is_protected(),
            is_key_protected: data == ProtectionKind::Encrypt,
            plugin_endpoint_attributes: plugin_attributes,
            ac_endpoint_properties: alloc::vec![
                Property::local(ENDPOINT_METADATA_PROTECTION_KIND, metadata.as_str()),
                Property::local(ENDPOINT_DATA_PROTECTION_KIND, data.as_str()),
            ],
            algorithm_info: zerodds_security::access_control::EndpointSecurityAlgorithmInfo {
                symmetric_cipher:
                    zerodds_security::access_control::EndpointSymmetricCipherAlgorithmInfo {
                        required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                        supported_mask: allowed,
                    },
            },
        })
    }
}

fn topic_config(rule: &crate::governance::TopicRule) -> TopicSecurityConfig {
    TopicSecurityConfig {
        is_read_protected: rule.enable_read_access_control,
        is_write_protected: rule.enable_write_access_control,
        is_discovery_protected: rule.enable_discovery_protection,
        is_liveliness_protected: rule.enable_liveliness_protection,
    }
}

/// `algorithm_info` of `get_participant_security_config` (Table 70).
fn algorithm_info(
    allowed: Option<crate::governance::AllowedCryptoAlgorithms>,
    identity: Option<AlgorithmMasks>,
) -> SecurityResult<ParticipantSecurityAlgorithmInfo> {
    use zerodds_security::access_control::{
        CryptoAlgorithmRequirements as Requirements, DigitalSignatureAlgorithmInfo,
        KeyEstablishmentAlgorithmInfo, SymmetricCipherAlgorithmInfo,
    };
    let allowed = allowed.unwrap_or_default();
    let message_auth = allowed
        .digital_signature
        .unwrap_or(CRYPTO_ALGORITHM_SET_ALL);
    let trust_chain = allowed
        .digital_signature_trust_chain
        .unwrap_or(message_auth);
    // The algorithms of the identity certificate; the compatibility mode bit
    // "if there are 2 or more algorithms".
    let mut trust_required = identity.map_or(CRYPTO_ALGORITHM_SET_EMPTY, |m| m.trust_chain.1)
        & !CRYPTO_ALGORITHM_COMPATIBILITY_MODE;
    let algorithms_required = trust_required;
    if algorithms_required.count_ones() >= 2 {
        trust_required |= CRYPTO_ALGORITHM_COMPATIBILITY_MODE;
    }
    if algorithms_required & !trust_chain != 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::AccessDenied,
            "permissions: the identity certificate uses a digital signature algorithm that \
             <allowed_crypto_algorithms> of the governance does not allow",
        ));
    }
    // The algorithms the authentication plugin signs and agrees keys with
    // must be allowed as well (§10.4.1.2.5.9: the section "defines rules that
    // control the cryptographic algorithms that may be used in the domain").
    if let Some(masks) = identity {
        let key_establishment = allowed
            .key_establishment
            .unwrap_or(CRYPTO_ALGORITHM_SET_ALL);
        for (used, allowed_set, what) in [
            (masks.message_auth.1, message_auth, "digital signature"),
            (
                masks.key_establishment.1,
                key_establishment,
                "key establishment",
            ),
        ] {
            if (used & !CRYPTO_ALGORITHM_COMPATIBILITY_MODE) & !allowed_set != 0 {
                return Err(SecurityError::new(
                    SecurityErrorKind::AccessDenied,
                    alloc::format!(
                        "permissions: the authentication plugin uses a {what} algorithm that \
                         <allowed_crypto_algorithms> of the governance does not allow"
                    ),
                ));
            }
        }
    }
    Ok(ParticipantSecurityAlgorithmInfo {
        digital_signature: DigitalSignatureAlgorithmInfo {
            trust_chain: Requirements {
                supported_mask: trust_chain,
                required_mask: trust_required,
            },
            message_auth: Requirements {
                supported_mask: message_auth,
                required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
            },
        },
        key_establishment: KeyEstablishmentAlgorithmInfo {
            shared_secret: Requirements {
                supported_mask: allowed
                    .key_establishment
                    .unwrap_or(CRYPTO_ALGORITHM_SET_ALL),
                required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
            },
        },
        symmetric_cipher: SymmetricCipherAlgorithmInfo {
            supported_mask: allowed.symmetric_cipher.unwrap_or(CRYPTO_ALGORITHM_SET_ALL),
            builtin_endpoints_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
            builtin_kx_endpoints_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
            user_endpoints_default_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
        },
    })
}

/// The subject of an identity certificate given as PEM or DER (the `c.id`
/// bytes of a handshake may also carry a trailing NUL).
fn certificate_subject(certificate: &[u8]) -> SecurityResult<Vec<(String, String)>> {
    let not_a_certificate = |what: String| {
        SecurityError::new(
            SecurityErrorKind::AccessDenied,
            alloc::format!("permissions: the identity certificate: {what}"),
        )
        .with_minor_code(minor_code::NOT_A_CERTIFICATE)
    };
    // Only the PEM form may end with a NUL (§7.3.3.1, OpenDDS); a DER
    // certificate ends with its signature, whose last octet is 0x00 in one
    // of 256 certificates, and is taken as it is. Cutting the zeros off DER
    // cut that certificate short ("ASN.1 DER message is incomplete").
    const BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
    let der = match certificate.windows(BEGIN.len()).position(|w| w == BEGIN) {
        Some(start) => {
            let pem = &certificate[start..];
            let pem = match pem.iter().rposition(|&b| b != 0) {
                Some(last) => &pem[..=last],
                None => pem,
            };
            zerodds_security_pki::first_cert_der(pem)
                .map_err(|e| not_a_certificate(e.to_string()))?
        }
        None => certificate.to_vec(),
    };
    zerodds_security_pki::subject::subject_attributes(&der)
        .map_err(|e| not_a_certificate(e.to_string()))
}

/// The description of a CA certificate for the PermissionsToken (Table 65):
/// the subject name and the name of its signature algorithm, of Table 25, or
/// of DDS-Security 1.1 with `legacy_names`. `None` where Table 25 has no
/// name for the algorithm: the presence of both properties is optional.
fn describe_ca(
    der: &[u8],
    form: zerodds_security::SubjectNameForm,
    legacy_names: bool,
) -> Option<(String, String)> {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;
    let cert = Certificate::from_der(der).ok()?;
    let oid = cert.signature_algorithm.oid.to_string();
    if zerodds_security_pki::certificate_algorithm_name(&oid, false).is_empty() {
        return None;
    }
    Some((
        zerodds_security_pki::format_subject_name(&cert.tbs_certificate.subject, form),
        zerodds_security_pki::certificate_algorithm_name(&oid, legacy_names),
    ))
}

fn unknown_handle() -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::BadArgument,
        "permissions: unknown PermissionsHandle",
    )
}

fn no_governance() -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::InvalidConfiguration,
        "permissions: the plugin holds no governance document",
    )
}

impl AccessControlPlugin for PermissionsAccessControl {
    /// §9.4.2.9.1 with Table 70. `participant.properties` holds the
    /// documents: `dds.sec.access.permissions` and `dds.sec.access.governance`
    /// as signed S/MIME text with `dds.sec.access.permissions_ca` and
    /// `dds.sec.auth.identity_certificate` (PEM), or both as plain XML with
    /// `org.zerodds.sec.access.subject_name` (the governance may then be absent).
    fn validate_local_permissions(
        &mut self,
        _identity: IdentityHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle> {
        let props = participant.properties;
        let invalid =
            |message: String| SecurityError::new(SecurityErrorKind::InvalidConfiguration, message);
        let document = props.get(PROP_PERMISSIONS_XML).ok_or_else(|| {
            invalid("permissions: missing dds.sec.access.permissions".to_string())
        })?;
        if let Some(class) = props.get(PROP_TOKEN_CLASS_ID) {
            self.set_token_class_id(class)?;
        }
        if document.trim_start().starts_with('<') {
            let subject = props.get(PROP_SUBJECT_NAME).ok_or_else(|| {
                invalid("permissions: missing org.zerodds.sec.access.subject_name".to_string())
            })?;
            let permissions = parse_permissions_xml(document)
                .map_err(|e| invalid(alloc::format!("permissions: {e}")))?;
            if let Some(governance) = props.get(PROP_GOVERNANCE) {
                self.governance = Some(
                    parse_governance_xml(governance)
                        .map_err(|e| invalid(alloc::format!("permissions: governance: {e}")))?,
                );
            }
            self.domain_tag = participant.domain_tag.to_string();
            return Ok(self.insert(Slot {
                local: true,
                subject_name: subject.to_string(),
                permissions,
                domain_id: Some(participant.domain_id),
                token_class_id: None,
                signed_permissions: None,
                identity_algorithms: participant.identity_algorithms,
            }));
        }
        let governance = props
            .get(PROP_GOVERNANCE)
            .ok_or_else(|| invalid("permissions: missing dds.sec.access.governance".to_string()))?;
        let ca = props.get(PROP_PERMISSIONS_CA).ok_or_else(|| {
            invalid("permissions: missing dds.sec.access.permissions_ca".to_string())
        })?;
        let identity = props.get(PROP_IDENTITY_CERTIFICATE).ok_or_else(|| {
            invalid("permissions: missing dds.sec.auth.identity_certificate".to_string())
        })?;
        self.validate_local_signed(
            identity.as_bytes(),
            governance.as_bytes(),
            document.as_bytes(),
            ca.as_bytes(),
            participant,
        )
    }

    /// DDS-Security 1.2 §9.4.2.9.2 with Table 70. The
    /// `AuthenticatedPeerCredentialToken` carries the DER or PEM of the
    /// remote identity certificate (`c.id`) and its signed permissions
    /// document (`c.perm`), as §10.3.2.3 has it. The signature is checked
    /// against the armed permissions CA, and the grant is the one whose
    /// subject is the certificate's (§10.4.1.5.3.1) — not the document's
    /// first. The class of `remote_permissions_token` is kept: the checks of
    /// a remote participant compare it with the local one.
    fn validate_remote_permissions(
        &mut self,
        _local_identity: IdentityHandle,
        _remote_identity: IdentityHandle,
        remote_permissions_token: &PermissionsToken,
        remote_credential_token: &DataHolder,
    ) -> SecurityResult<PermissionsHandle> {
        let refused = |message: String, minor: i32| {
            SecurityError::new(SecurityErrorKind::AccessDenied, message).with_minor_code(minor)
        };
        let verifier = self.remote_verifier.as_ref().ok_or_else(|| {
            refused(
                "permissions: no permissions CA is armed; a remote permissions token is not \
                 read unverified"
                    .into(),
                minor_code::NO_VERIFIER,
            )
        })?;
        let (Some(c_id), Some(c_perm)) = (
            remote_credential_token.binary_property("c.id"),
            remote_credential_token.binary_property("c.perm"),
        ) else {
            return Err(refused(
                "permissions: the credential token of the remote participant carries no c.id \
                 and c.perm"
                    .into(),
                minor_code::NO_CREDENTIAL,
            ));
        };
        // A `c.perm` set from the S/MIME text counts its terminating NUL
        // (DDS-Security 1.2 §7.3.3.1); the signed document ends before it.
        let c_perm = match c_perm.iter().rposition(|&b| b != 0) {
            Some(last) => &c_perm[..=last],
            None => c_perm,
        };
        let perms = open_signed_permissions(c_perm, verifier).map_err(|e| {
            refused(
                alloc::format!(
                    "permissions: the remote permissions token does not verify against the \
                     permissions CA: {e}"
                ),
                minor_code::NOT_VERIFIED,
            )
        })?;
        let subject = certificate_subject(c_id)?;
        let granted = perms.for_subject(&subject).ok_or_else(|| {
            refused(
                "permissions: no grant of the remote permissions names the certificate's subject"
                    .into(),
                minor_code::NO_GRANT_FOR_SUBJECT,
            )
        })?;
        let subject_name = granted
            .grants
            .first()
            .map(|g| g.subject_text().to_string())
            .unwrap_or_default();
        Ok(self.insert(Slot {
            local: false,
            subject_name,
            permissions: granted,
            domain_id: None,
            token_class_id: Some(remote_permissions_token.class_id.clone()),
            signed_permissions: None,
            identity_algorithms: None,
        }))
    }

    /// Table 70 `check_create_participant`: "If the ParticipantSecurityConfig
    /// has is_access_protected set to FALSE" TRUE; else the Grant's first
    /// matching rule for the domain: an allow rule permits, a deny rule
    /// without publish or subscribe refuses, else the default.
    fn check_create_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        let slot = self.slot(permissions)?;
        let domain = Domain {
            id: participant.domain_id,
            tag: if participant.domain_tag.is_empty() {
                self.domain_tag.as_str()
            } else {
                participant.domain_tag
            },
        };
        let now = self.now();
        Ok(self.join_allowed(&slot.permissions, domain, now).into())
    }

    /// Table 70 `check_create_datawriter`: the topic is not
    /// write-access-controlled, or the grant allows publishing it on all the
    /// Publisher's partitions with all the DataWriter's data tags.
    fn check_create_datawriter(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        Ok(self
            .local_endpoint_access(permissions, Action::Publish, request)?
            .into())
    }

    /// Table 70 `check_create_datareader`.
    fn check_create_datareader(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        Ok(self
            .local_endpoint_access(permissions, Action::Subscribe, request)?
            .into())
    }

    /// Table 70 `check_create_topic`: the Governance leaves the topic
    /// without read or without write access control, or the grant allows
    /// publishing or subscribing it.
    fn check_create_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        let slot = self.slot(permissions)?;
        if !self.topic_has_rule(request.domain_id, request.topic_name) {
            return Ok(AccessDecision::Deny);
        }
        if self.open_topic(Action::Publish, request.domain_id, request.topic_name)
            || self.open_topic(Action::Subscribe, request.domain_id, request.topic_name)
        {
            return Ok(AccessDecision::Permit);
        }
        Ok(self.topic_access(&slot.permissions, request).into())
    }

    /// Table 70: "This operation shall return TRUE." The handle is checked.
    fn check_local_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        _writer: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.slot(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 70: "This operation shall return TRUE."
    fn check_local_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        _writer: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.slot(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 70 `check_remote_participant`: TRUE when the domain is not
    /// access protected; FALSE when the PluginClassName or MajorVersion of
    /// the remote token differ; else the first matching rule of the remote
    /// grant for the domain, as `check_create_participant`.
    fn check_remote_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        let slot = self.slot_or_nil(permissions)?;
        let domain = self.domain(participant.domain_id);
        let protected = match &self.governance {
            Some(governance) => governance
                .find_domain_rule(domain)
                .is_none_or(|rule| rule.enable_join_access_control),
            None => true,
        };
        if !protected {
            return Ok(AccessDecision::Permit);
        }
        let Some(slot) = slot else {
            return Ok(AccessDecision::Deny);
        };
        if !self.token_matches(slot) {
            return Ok(AccessDecision::Deny);
        }
        Ok(self
            .join_allowed(&slot.permissions, domain, self.now())
            .into())
    }

    /// Table 70 `check_remote_datawriter`.
    fn check_remote_datawriter(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        Ok(
            (self.remote_endpoint_access(permissions, Action::Publish, request)?
                == ReaderAccess::Allowed)
                .into(),
        )
    }

    /// Table 70 `check_remote_datareader`: subscribing allowed, or relaying
    /// only (`allow_relay_only`).
    fn check_remote_datareader(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<RemoteReaderDecision> {
        Ok(
            match self.remote_endpoint_access(permissions, Action::Subscribe, request)? {
                ReaderAccess::Allowed => RemoteReaderDecision::Permit,
                ReaderAccess::RelayOnly => RemoteReaderDecision::RelayOnly,
                ReaderAccess::Denied => RemoteReaderDecision::Deny,
            },
        )
    }

    /// Table 70 `check_remote_topic`.
    fn check_remote_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        let slot = self.slot_or_nil(permissions)?;
        if !self.topic_has_rule(request.domain_id, request.topic_name) {
            return Ok(AccessDecision::Deny);
        }
        if self.open_topic(Action::Publish, request.domain_id, request.topic_name)
            || self.open_topic(Action::Subscribe, request.domain_id, request.topic_name)
        {
            return Ok(AccessDecision::Permit);
        }
        let Some(slot) = slot else {
            return Ok(AccessDecision::Deny);
        };
        if !self.token_matches(slot) {
            return Ok(AccessDecision::Deny);
        }
        Ok(self.topic_access(&slot.permissions, request).into())
    }

    /// Table 70: "This operation shall return TRUE." The handles are
    /// checked.
    fn check_local_datawriter_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        _publication: &EndpointRequest<'_>,
        _subscription: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.slot(writer_permissions)?;
        self.slot_or_nil(reader_permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 70: "This operation shall return TRUE."
    fn check_local_datareader_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        _subscription: &EndpointRequest<'_>,
        _publication: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.slot_or_nil(writer_permissions)?;
        self.slot(reader_permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 70: "This operation shall return TRUE."
    fn check_remote_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        _reader: [u8; 16],
        _publication: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.slot_or_nil(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 70: "This operation shall return TRUE."
    fn check_remote_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        _reader: [u8; 16],
        _publication: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.slot_or_nil(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 70 `get_permissions_token`: the token of §10.4.2.2 (Table 65):
    /// the class and, when the plugin knows its Permissions CA, the
    /// properties `dds.perm_ca.sn` and `dds.perm_ca.algo`.
    fn get_permissions_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsToken> {
        self.slot(handle)?;
        let mut token = DataHolder::new(self.token_class_id.clone());
        if let Some((subject, algorithm)) = self
            .remote_verifier
            .as_ref()
            .and_then(|v| v.ca_certificates_der().first())
            .and_then(|der| {
                describe_ca(
                    der,
                    self.subject_name_form,
                    self.algorithm_names == zerodds_security_pki::AlgorithmNames::V1_1,
                )
            })
        {
            token = token
                .with_property(prop::PERM_CA_SN, subject)
                .with_property(prop::PERM_CA_ALGO, algorithm);
        }
        self.issued_tokens.push(token.clone());
        Ok(token)
    }

    /// Table 70 `get_permissions_credential_token`: the token of §10.4.2.1
    /// (Table 64), `dds.perm.cert` holding the signed permissions document
    /// of the participant.
    fn get_permissions_credential_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsCredentialToken> {
        let slot = self.slot(handle)?;
        let signed = slot.signed_permissions.as_ref().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "permissions: the handle has no signed permissions document to represent on \
                 the network",
            )
            .with_minor_code(minor_code::NO_CREDENTIAL)
        })?;
        let text = core::str::from_utf8(signed).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!(
                    "permissions: the signed permissions document is no text and cannot be a \
                     property value: {e}"
                ),
            )
        })?;
        let token = DataHolder::new(class_id::ACCESS_PERMISSIONS_CREDENTIAL)
            .with_property(PROP_PERM_CERT, text);
        self.issued_credential_tokens.push(token.clone());
        Ok(token)
    }

    /// Table 70 `set_listener`: "save a reference to the listener object".
    fn set_listener(&mut self, listener: Arc<dyn AccessControlListener>) -> SecurityResult<()> {
        self.listener = Some(listener);
        Ok(())
    }

    /// §9.4.2.9.20.
    fn return_permissions_token(&mut self, token: PermissionsToken) -> SecurityResult<()> {
        crate::issued::give_back(
            &mut self.issued_tokens,
            &token,
            "permissions",
            "the PermissionsToken",
        )
    }

    /// §9.4.2.9.21.
    fn return_permissions_credential_token(
        &mut self,
        token: PermissionsCredentialToken,
    ) -> SecurityResult<()> {
        crate::issued::give_back(
            &mut self.issued_credential_tokens,
            &token,
            "permissions",
            "the PermissionsCredentialToken",
        )
    }

    /// Table 70 `get_participant_security_config`.
    fn get_participant_security_config(
        &mut self,
        permissions: PermissionsHandle,
    ) -> SecurityResult<ParticipantSecurityConfig> {
        let config = self.participant_config_of(self.slot(permissions)?)?;
        self.issued_participant_configs.push(config.clone());
        Ok(config)
    }

    /// §9.4.2.9.23 with §10.4.1.2.6.2 to §10.4.1.2.6.5: the four settings of
    /// the topic rule.
    fn get_topic_security_config(
        &mut self,
        permissions: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<TopicSecurityConfig> {
        let slot = self.slot(permissions)?;
        let domain_id = slot.domain_id.ok_or_else(|| {
            SecurityError::bad_argument(
                "permissions: get_topic_security_config needs the handle of the local participant",
            )
        })?;
        let config = topic_config(&self.topic_rule(domain_id, topic_name)?);
        self.issued_topic_configs.push(config);
        Ok(config)
    }

    /// §9.4.2.9.24 with §10.4.1.2.6.6 and §10.4.1.2.6.7.
    fn get_datawriter_security_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        let config = self.endpoint_config(permissions, request)?;
        self.issued_writer_configs.push(config.clone());
        Ok(config)
    }

    /// §9.4.2.9.25 with §10.4.1.2.6.6 and §10.4.1.2.6.7.
    fn get_datareader_security_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        let config = self.endpoint_config(permissions, request)?;
        self.issued_reader_configs.push(config.clone());
        Ok(config)
    }

    // §9.4.2.9.26 to §9.4.2.9.29.
    return_security_configs!("permissions");

    /// The `<relay>` action (§10.4.1.5.3.4.1.2) for the relay service of
    /// §7.1.1.4: the topic is not access-controlled in that direction, or a
    /// grant allows relaying it. No topic rule refuses.
    fn check_relay(
        &self,
        permissions: PermissionsHandle,
        role: RelayRole,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        let slot = self.slot_or_nil(permissions)?;
        if !self.topic_has_rule(request.domain_id, request.topic_name) {
            return Ok(AccessDecision::Deny);
        }
        let direction = match role {
            RelayRole::Reader => Action::Subscribe,
            RelayRole::Writer => Action::Publish,
        };
        if self.open_topic(direction, request.domain_id, request.topic_name) {
            return Ok(AccessDecision::Permit);
        }
        let Some(slot) = slot else {
            return Ok(AccessDecision::Deny);
        };
        if !self.token_matches(slot) {
            return Ok(AccessDecision::Deny);
        }
        let now = self.now();
        let domain = self.domain(request.domain_id);
        let tags = Self::data_tags(request);
        Ok(match &self.governance {
            Some(governance) => match role {
                RelayRole::Reader => governance.check_relay_reader(
                    &slot.permissions,
                    domain,
                    request.topic_name,
                    request.partitions,
                    &tags,
                    now,
                ),
                RelayRole::Writer => governance.check_relay_writer(
                    &slot.permissions,
                    domain,
                    request.topic_name,
                    request.partitions,
                    &tags,
                    now,
                ),
            },
            None => slot.permissions.grants.iter().any(|g| {
                g.is_valid_at(now)
                    && g.decide(
                        &AccessRequest {
                            action: Action::Relay,
                            domain,
                            topic: request.topic_name,
                            partitions: request.partitions,
                            data_tags: &tags,
                        },
                        PartitionMatching::default(),
                    )
                    .allowed()
            }),
        }
        .into())
    }

    fn plugin_class_id(&self) -> &str {
        &self.token_class_id
    }
}

impl PermissionsAccessControl {
    /// The join check of Table 70 for the participant of `permissions` in
    /// `domain`: without a Governance the grant decides; with one a domain
    /// no rule covers refuses and `enable_join_access_control` FALSE
    /// permits.
    fn join_allowed(&self, permissions: &Permissions, domain: Domain<'_>, now: u64) -> bool {
        match &self.governance {
            Some(governance) => governance.check_create_participant(permissions, domain, now),
            None => permissions
                .grants
                .iter()
                .any(|g| g.is_valid_at(now) && g.may_join(domain)),
        }
    }

    /// The grant of the participant behind `handle`, for tests and
    /// diagnostics.
    ///
    /// # Errors
    /// `BadArgument` for an unknown handle.
    pub fn grant_of(&self, handle: PermissionsHandle) -> SecurityResult<Option<&Grant>> {
        let slot = self.slot(handle)?;
        Ok(subject_name_attributes(&slot.subject_name)
            .and_then(|subject| slot.permissions.grants.iter().find(|g| g.is_for(&subject))))
    }

    /// `true` for the handle of the local participant.
    ///
    /// # Errors
    /// `BadArgument` for an unknown handle.
    pub fn is_local(&self, handle: PermissionsHandle) -> SecurityResult<bool> {
        Ok(self.slot(handle)?.local)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// `validate_remote_permissions` reads the subject of the certificate in
    /// `c.id`. A DER certificate whose last octet is 0x00 is read whole, and
    /// its PEM with the NUL of DDS-Security 1.2 §7.3.3.1 gives the same
    /// subject. Cutting the zeros off DER read such a certificate as
    /// incomplete, and the permissions of the peer were refused: in about
    /// 3 % of the runs of `security_relay_e2e`, whose certificates are new in
    /// every run.
    #[test]
    fn the_subject_of_a_der_certificate_that_ends_in_zero_is_read() {
        // One certificate in 256 ends in 0x00, the last octet of its signature.
        let (der, pem) = (0..20_000)
            .find_map(|_| {
                let key = rcgen::KeyPair::generate().unwrap();
                let cert = rcgen::CertificateParams::new(vec!["zero-tail".into()])
                    .unwrap()
                    .self_signed(&key)
                    .unwrap();
                (cert.der().last() == Some(&0)).then(|| (cert.der().to_vec(), cert.pem()))
            })
            .expect("a certificate whose DER ends in 0x00");
        let from_der = certificate_subject(&der).expect("the DER is read whole");
        assert!(!from_der.is_empty());
        let mut c_id = pem.into_bytes();
        c_id.push(0);
        assert_eq!(
            certificate_subject(&c_id).expect("the PEM is read"),
            from_der
        );
    }
    use zerodds_security::access_control::ParticipantRequest;
    use zerodds_security::properties::{Property, PropertyList};

    const ALICE_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=alice</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id_range><min>0</min></id_range></domains>
      <publish><topics><topic>Chatter</topic><topic>sensor_*</topic></topics></publish>
      <subscribe><topics><topic>Echo</topic></topics></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    fn no_props() -> &'static PropertyList {
        static PROPS: std::sync::OnceLock<PropertyList> = std::sync::OnceLock::new();
        PROPS.get_or_init(PropertyList::new)
    }

    /// A participant in `domain` without properties.
    fn pr(domain: u32) -> ParticipantRequest<'static> {
        ParticipantRequest::new(domain, [0; 16], no_props())
    }

    fn build_alice() -> (PermissionsAccessControl, PermissionsHandle) {
        let mut ac = PermissionsAccessControl::new();
        let perms = parse_permissions_xml(ALICE_XML).unwrap();
        let h = ac.register("CN=alice".into(), perms);
        (ac, h)
    }

    #[test]
    fn permit_on_exact_match() {
        let (ac, h) = build_alice();
        let d = ac
            .check_create_datawriter(h, &EndpointRequest::new(0, "Chatter"))
            .unwrap();
        assert_eq!(d, AccessDecision::Permit);
    }

    #[test]
    fn permit_on_wildcard_match() {
        let (ac, h) = build_alice();
        let d = ac
            .check_create_datawriter(h, &EndpointRequest::new(0, "sensor_temp"))
            .unwrap();
        assert_eq!(d, AccessDecision::Permit);
    }

    #[test]
    fn deny_on_non_matching_topic() {
        let (ac, h) = build_alice();
        let d = ac
            .check_create_datawriter(h, &EndpointRequest::new(0, "actuator_x"))
            .unwrap();
        assert_eq!(d, AccessDecision::Deny);
    }

    #[test]
    fn deny_writer_when_only_subscribe_granted() {
        let (ac, h) = build_alice();
        let d = ac
            .check_create_datawriter(h, &EndpointRequest::new(0, "Echo"))
            .unwrap();
        assert_eq!(d, AccessDecision::Deny);
    }

    #[test]
    fn permit_reader_on_subscribe_allowed_topic() {
        let (ac, h) = build_alice();
        let d = ac
            .check_create_datareader(h, &EndpointRequest::new(0, "Echo"))
            .unwrap();
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
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(0, [0xAA; 16], &props),
            )
            .expect("validate via props");
        assert_eq!(
            ac.check_create_datawriter(h, &EndpointRequest::new(0, "Chatter"))
                .unwrap(),
            AccessDecision::Permit,
        );
    }

    #[test]
    fn missing_permissions_property_is_invalid_configuration() {
        let mut ac = PermissionsAccessControl::new();
        let props = PropertyList::new();
        let err = ac
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(0, [0xAA; 16], &props),
            )
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    // -----------------------------------------------------------------
    // §9.3 — one permissions XML, two rules, two different decisions.
    // -----------------------------------------------------------------

    /// `sensor/*` is allowed for publish and subscribe, but the
    /// `<deny_rule>` in front of it carves `sensor/secret` back out: the
    /// rules apply in document order and the first that matches decides
    /// (DDS-Security 1.2 §10.4.1.5.3). Domain 7-9 and, for publishing, the
    /// `internal` partition scope the grant.
    const BOB_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=bob</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <deny_rule><domains><id_range><min>7</min><max>9</max></id_range></domains>
      <publish><topics><topic>sensor/secret</topic></topics></publish>
      <subscribe><topics><topic>sensor/secret</topic></topics></subscribe>
    </deny_rule>
    <allow_rule><domains><id_range><min>7</min><max>9</max></id_range></domains>
      <publish><topics><topic>sensor/*</topic></topics>
        <partitions><partition>internal</partition></partitions>
      </publish>
      <subscribe><topics><topic>sensor/*</topic></topics></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    fn build_bob() -> (PermissionsAccessControl, PermissionsHandle) {
        let mut ac = PermissionsAccessControl::new();
        let perms = parse_permissions_xml(BOB_XML).expect("BOB_XML parses");
        let h = ac.register("CN=bob".into(), perms);
        (ac, h)
    }

    #[test]
    fn allow_rule_and_deny_rule_decide_the_same_xml_differently() {
        let (ac, h) = build_bob();
        let internal = |topic| EndpointRequest::new(7, topic).in_partitions(&["internal"]);
        // Allow rule hits.
        assert_eq!(
            ac.check_create_datawriter(h, &internal("sensor/temp"))
                .unwrap(),
            AccessDecision::Permit,
            "sensor/temp is covered by the allow_rule",
        );
        // The deny rule in front hits a topic the allow pattern covers too.
        assert_eq!(
            ac.check_create_datawriter(h, &internal("sensor/secret"))
                .unwrap(),
            AccessDecision::Deny,
            "the first matching rule decides (§10.4.1.5.3)",
        );
        // Without the partition the allow rule does not match: the default
        // partition is not in its set (§10.4.1.5.3.4.1.4).
        assert_eq!(
            ac.check_create_datawriter(h, &EndpointRequest::new(7, "sensor/temp"))
                .unwrap(),
            AccessDecision::Deny,
        );
    }

    #[test]
    fn deny_rule_also_binds_the_reader_side() {
        let (ac, h) = build_bob();
        assert_eq!(
            ac.check_create_datareader(h, &EndpointRequest::new(7, "sensor/temp"))
                .unwrap(),
            AccessDecision::Permit,
        );
        assert_eq!(
            ac.check_create_datareader(h, &EndpointRequest::new(7, "sensor/secret"))
                .unwrap(),
            AccessDecision::Deny,
        );
    }

    #[test]
    fn remote_match_inherits_the_deny_decision() {
        let (ac, h) = build_bob();
        assert_eq!(
            ac.check_remote_datawriter(h, &EndpointRequest::new(7, "sensor/secret"))
                .unwrap(),
            AccessDecision::Deny,
            "a remote writer must not match on a denied topic",
        );
        assert_eq!(
            ac.check_remote_datareader(h, &EndpointRequest::new(7, "sensor/temp"))
                .unwrap(),
            RemoteReaderDecision::Permit,
        );
    }

    #[test]
    fn partition_scope_narrows_an_otherwise_permitted_topic() {
        let (ac, h) = build_bob();
        assert_eq!(
            ac.check_create_datawriter(
                h,
                &EndpointRequest::new(7, "sensor/temp").in_partitions(&["internal"])
            )
            .unwrap(),
            AccessDecision::Permit,
            "the rule lists the internal partition",
        );
        assert_eq!(
            ac.check_create_datawriter(
                h,
                &EndpointRequest::new(7, "sensor/temp").in_partitions(&["public"])
            )
            .unwrap(),
            AccessDecision::Deny,
            "public is outside the rule's partition set (§9.4.1.3.2.3.1)",
        );
        assert_eq!(
            ac.check_create_datawriter(
                h,
                &EndpointRequest::new(7, "sensor/temp").in_partitions(&["internal", "public"])
            )
            .unwrap(),
            AccessDecision::Deny,
            "every requested partition must be covered, not just one",
        );
    }

    #[test]
    fn partition_check_never_widens_a_denied_topic() {
        let (ac, h) = build_bob();
        assert_eq!(
            ac.check_create_datawriter(
                h,
                &EndpointRequest::new(7, "sensor/secret").in_partitions(&["internal"])
            )
            .unwrap(),
            AccessDecision::Deny,
        );
    }

    #[test]
    fn grant_without_partitions_covers_only_the_default_partition() {
        let (ac, h) = build_alice();
        assert_eq!(
            ac.check_create_datawriter(h, &EndpointRequest::new(0, "Chatter").in_partitions(&[]))
                .unwrap(),
            AccessDecision::Permit,
            "no partition named = default partition",
        );
        assert_eq!(
            ac.check_create_datawriter(
                h,
                &EndpointRequest::new(0, "Chatter").in_partitions(&["internal"])
            )
            .unwrap(),
            AccessDecision::Deny,
        );
    }

    // -----------------------------------------------------------------
    // §9.6 — check_create_participant + handle caching.
    // -----------------------------------------------------------------

    #[test]
    fn check_create_participant_honours_the_domain_range() {
        let mut ac = PermissionsAccessControl::new();
        let allow_first = BOB_XML.replace(
            "<deny_rule><domains><id_range><min>7</min><max>9</max></id_range></domains>\n      \
             <publish><topics><topic>sensor/secret</topic></topics></publish>\n      \
             <subscribe><topics><topic>sensor/secret</topic></topics></subscribe>\n    \
             </deny_rule>",
            "",
        );
        assert!(
            !allow_first.contains("deny_rule"),
            "the allow rule is first"
        );
        let perms = parse_permissions_xml(&allow_first).expect("parses");
        let h = ac.register("CN=bob".into(), perms);
        assert_eq!(
            ac.check_create_participant(h, &pr(7)).unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_create_participant(h, &pr(9)).unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_create_participant(h, &pr(10)).unwrap(),
            AccessDecision::Deny,
            "domain 10 is outside <id_range>7..9</id_range>",
        );
        assert_eq!(
            ac.check_remote_participant(h, &pr(10)).unwrap(),
            AccessDecision::Deny,
        );
    }

    /// Table 70 `check_create_participant`: the grant's first rule for the
    /// domain decides; a deny rule with publish or subscribe actions in
    /// front leaves it to the default, DENY here.
    #[test]
    fn a_deny_rule_with_actions_first_leaves_joining_to_the_default() {
        let (ac, h) = build_bob();
        assert_eq!(
            ac.check_create_participant(h, &pr(7)).unwrap(),
            AccessDecision::Deny
        );
    }

    #[test]
    fn empty_domain_list_lets_the_participant_join_any_domain() {
        let (ac, h) = build_alice();
        assert_eq!(
            ac.check_create_participant(h, &pr(0)).unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_create_participant(h, &pr(232)).unwrap(),
            AccessDecision::Permit,
        );
    }

    #[test]
    fn unknown_handle_is_rejected_on_every_check() {
        let (ac, _) = build_alice();
        let bogus = PermissionsHandle(9999);
        for kind in [
            ac.check_create_datawriter(bogus, &EndpointRequest::new(0, "Chatter"))
                .err(),
            ac.check_create_datareader(bogus, &EndpointRequest::new(0, "Chatter"))
                .err(),
            ac.check_create_datawriter(
                bogus,
                &EndpointRequest::new(0, "Chatter").in_partitions(&[]),
            )
            .err(),
            ac.check_create_datareader(
                bogus,
                &EndpointRequest::new(0, "Chatter").in_partitions(&[]),
            )
            .err(),
            ac.check_create_participant(bogus, &pr(0)).err(),
        ] {
            assert_eq!(
                kind.expect("unknown handle must not silently permit").kind,
                SecurityErrorKind::BadArgument,
            );
        }
    }

    /// The XML is parsed once, at `validate_local_permissions`; the
    /// resulting handle caches the decision base. Two validations of two
    /// different subjects yield two handles that keep deciding
    /// independently, and the source strings are gone by then.
    #[test]
    fn permissions_are_cached_per_handle_and_decide_independently() {
        let mut ac = PermissionsAccessControl::new();
        let alice = ac
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(
                    0,
                    [0xAA; 16],
                    &PropertyList::new()
                        .with(Property::local(PROP_PERMISSIONS_XML, ALICE_XML.to_string()))
                        .with(Property::local(PROP_SUBJECT_NAME, "CN=alice")),
                ),
            )
            .expect("alice validates");
        let bob = ac
            .validate_local_permissions(
                IdentityHandle(2),
                &ParticipantRequest::new(
                    7,
                    [0xBB; 16],
                    &PropertyList::new()
                        .with(Property::local(PROP_PERMISSIONS_XML, BOB_XML.to_string()))
                        .with(Property::local(PROP_SUBJECT_NAME, "CN=bob")),
                ),
            )
            .expect("bob validates");
        assert_ne!(alice, bob, "each validation gets its own handle");

        // Repeated checks on the cached handles stay stable and stay
        // separate — alice may write Chatter, bob may not.
        for _ in 0..3 {
            assert_eq!(
                ac.check_create_datawriter(alice, &EndpointRequest::new(0, "Chatter"))
                    .unwrap(),
                AccessDecision::Permit,
            );
            assert_eq!(
                ac.check_create_datawriter(bob, &EndpointRequest::new(7, "Chatter"))
                    .unwrap(),
                AccessDecision::Deny,
            );
            assert_eq!(
                ac.check_create_datawriter(
                    bob,
                    &EndpointRequest::new(7, "sensor/temp").in_partitions(&["internal"])
                )
                .unwrap(),
                AccessDecision::Permit,
            );
            assert_eq!(
                ac.check_create_datawriter(alice, &EndpointRequest::new(0, "sensor/temp"))
                    .unwrap(),
                AccessDecision::Deny,
                "alice's sensor_* pattern does not cover bob's sensor/ tree",
            );
        }
    }

    // -----------------------------------------------------------------
    // Table 39 / Table 70: the operations beyond the endpoint checks.
    // -----------------------------------------------------------------

    /// A governance for domain 0 with the topics: `Secret` (read and write
    /// access control, discovery and liveliness protected, metadata
    /// ENCRYPT_WITH_ORIGIN_AUTHENTICATION, data ENCRYPT), `ReadOpen` (write
    /// controlled only), `Plain` (nothing controlled, SIGN, NONE).
    const GOVERNANCE: &str = r#"
<dds><domain_access_rules><domain_rule>
  <domains><id>0</id></domains>
  <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
  <enable_join_access_control>true</enable_join_access_control>
  <enable_key_revision>true</enable_key_revision>
  <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
  <liveliness_protection_kind>SIGN_WITH_ORIGIN_AUTHENTICATION</liveliness_protection_kind>
  <rtps_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</rtps_protection_kind>
  <rtps_psk_protection_kind>ENCRYPT</rtps_psk_protection_kind>
  <allowed_crypto_algorithms>
    <digital_signature><algorithm>ECDSA+P256+SHA256</algorithm></digital_signature>
    <key_establishment><algorithm>ECDHE-CEUM+P256</algorithm></key_establishment>
    <symmetric_cipher><algorithm>AES256+GCM</algorithm></symmetric_cipher>
  </allowed_crypto_algorithms>
  <topic_access_rules>
    <topic_rule><topic_expression>Secret</topic_expression>
      <enable_discovery_protection>true</enable_discovery_protection>
      <enable_liveliness_protection>true</enable_liveliness_protection>
      <enable_read_access_control>true</enable_read_access_control>
      <enable_write_access_control>true</enable_write_access_control>
      <metadata_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</metadata_protection_kind>
      <data_protection_kind>ENCRYPT</data_protection_kind></topic_rule>
    <topic_rule><topic_expression>WriteOnly</topic_expression>
      <enable_discovery_protection>false</enable_discovery_protection>
      <enable_liveliness_protection>false</enable_liveliness_protection>
      <enable_read_access_control>false</enable_read_access_control>
      <enable_write_access_control>true</enable_write_access_control>
      <metadata_protection_kind>SIGN</metadata_protection_kind>
      <data_protection_kind>SIGN</data_protection_kind></topic_rule>
    <topic_rule><topic_expression>Plain</topic_expression>
      <enable_discovery_protection>false</enable_discovery_protection>
      <enable_liveliness_protection>false</enable_liveliness_protection>
      <enable_read_access_control>false</enable_read_access_control>
      <enable_write_access_control>false</enable_write_access_control>
      <metadata_protection_kind>NONE</metadata_protection_kind>
      <data_protection_kind>NONE</data_protection_kind></topic_rule>
  </topic_access_rules>
</domain_rule></domain_access_rules></dds>
"#;

    /// carol may publish `Secret`, subscribe `Secret` and join domain 0.
    const CAROL_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=carol</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id>0</id></domains>
      <publish><topics><topic>Secret</topic></topics></publish>
      <subscribe><topics><topic>Secret</topic></topics></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    /// dave may publish `WriteOnly` and nothing else.
    const DAVE_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=dave</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id>0</id></domains>
      <publish><topics><topic>WriteOnly</topic></topics></publish>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    fn carol() -> (PermissionsAccessControl, PermissionsHandle) {
        let mut ac = PermissionsAccessControl::new()
            .with_governance(parse_governance_xml(GOVERNANCE).unwrap());
        let h = ac
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(
                    0,
                    [0xCC; 16],
                    &PropertyList::new()
                        .with(Property::local(PROP_PERMISSIONS_XML, CAROL_XML.to_string()))
                        .with(Property::local(PROP_SUBJECT_NAME, "CN=carol")),
                ),
            )
            .unwrap();
        (ac, h)
    }

    /// Table 70, `check_create_topic`: the governance leaves a topic without
    /// read or write access control open; the grant allows publishing or
    /// subscribing it otherwise; a topic no rule covers is refused.
    #[test]
    fn check_create_topic_follows_governance_and_grant() {
        let (ac, h) = carol();
        let topic = |name| {
            ac.check_create_topic(h, &TopicRequest::new(0, name))
                .unwrap()
        };
        assert_eq!(topic("Secret"), AccessDecision::Permit, "granted");
        assert_eq!(
            topic("WriteOnly"),
            AccessDecision::Permit,
            "read access control is off for the topic"
        );
        assert_eq!(topic("Plain"), AccessDecision::Permit, "no access control");
        assert_eq!(
            topic("Unruled"),
            AccessDecision::Deny,
            "no topic rule covers it (§10.4.1.2.7)"
        );
        // dave has a grant for WriteOnly only.
        let mut dave_ac = PermissionsAccessControl::new()
            .with_governance(parse_governance_xml(GOVERNANCE).unwrap());
        let dave = dave_ac.register("CN=dave".into(), parse_permissions_xml(DAVE_XML).unwrap());
        assert_eq!(
            dave_ac
                .check_create_topic(dave, &TopicRequest::new(0, "Secret"))
                .unwrap(),
            AccessDecision::Deny,
            "controlled for read and write, and not granted"
        );
        assert_eq!(
            dave_ac
                .check_create_topic(dave, &TopicRequest::new(0, "WriteOnly"))
                .unwrap(),
            AccessDecision::Permit
        );
    }

    /// Table 70: the topic checks of a remote participant also compare the
    /// PluginClassName and MajorVersion of the PermissionsToken.
    #[test]
    fn a_remote_token_of_another_class_or_major_version_is_refused() {
        let (mut ac, _) = carol();
        let carol_doc = parse_permissions_xml(CAROL_XML).unwrap();
        let same = ac.register_remote(carol_doc.clone(), "DDS:Access:Permissions:1.0");
        let other_major = ac.register_remote(carol_doc.clone(), "DDS:Access:Permissions:2.0");
        let other_class = ac.register_remote(carol_doc, "DDS:Access:PSK:1.2");
        let writer = EndpointRequest::new(0, "Secret");
        let reader = EndpointRequest::new(0, "Secret");
        let topic = TopicRequest::new(0, "Secret");
        assert_eq!(
            ac.check_remote_datawriter(same, &writer).unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_remote_datareader(same, &reader).unwrap(),
            RemoteReaderDecision::Permit
        );
        assert_eq!(
            ac.check_remote_topic(same, &topic).unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_remote_participant(same, &pr(0)).unwrap(),
            AccessDecision::Permit
        );
        for refused in [other_major, other_class] {
            assert_eq!(
                ac.check_remote_datawriter(refused, &writer).unwrap(),
                AccessDecision::Deny
            );
            assert_eq!(
                ac.check_remote_datareader(refused, &reader).unwrap(),
                RemoteReaderDecision::Deny
            );
            assert_eq!(
                ac.check_remote_topic(refused, &topic).unwrap(),
                AccessDecision::Deny
            );
            assert_eq!(
                ac.check_remote_participant(refused, &pr(0)).unwrap(),
                AccessDecision::Deny
            );
            // A topic without access control needs no grant and no token.
            assert_eq!(
                ac.check_remote_datawriter(refused, &EndpointRequest::new(0, "Plain"))
                    .unwrap(),
                AccessDecision::Permit
            );
        }
    }

    /// The nil handle is a participant whose permissions are not known: it is
    /// refused where the governance asks for access control and permitted
    /// where it does not.
    #[test]
    fn the_nil_handle_is_refused_on_a_controlled_topic_only() {
        let (ac, _) = carol();
        let nil = PermissionsHandle::NIL;
        assert_eq!(
            ac.check_remote_datawriter(nil, &EndpointRequest::new(0, "Secret"))
                .unwrap(),
            AccessDecision::Deny
        );
        assert_eq!(
            ac.check_remote_datawriter(nil, &EndpointRequest::new(0, "Plain"))
                .unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_remote_datareader(nil, &EndpointRequest::new(0, "Secret"))
                .unwrap(),
            RemoteReaderDecision::Deny
        );
        assert_eq!(
            ac.check_remote_participant(nil, &pr(0)).unwrap(),
            AccessDecision::Deny
        );
        // A local check does not take it.
        assert!(
            ac.check_create_datawriter(nil, &EndpointRequest::new(0, "Plain"))
                .is_err()
        );
    }

    /// Table 70: the operations on instances and matches return TRUE.
    #[test]
    fn instance_and_match_checks_return_true() {
        let (ac, h) = carol();
        let ep = EndpointRequest::new(0, "Secret").with_data_tags(&[("class", "secret")]);
        let key = [1u8, 2, 3];
        let guid = [7u8; 16];
        let yes = AccessDecision::Permit;
        assert_eq!(
            ac.check_local_datawriter_register_instance(h, guid, &key)
                .unwrap(),
            yes
        );
        assert_eq!(
            ac.check_local_datawriter_dispose_instance(h, guid, &key)
                .unwrap(),
            yes
        );
        assert_eq!(
            ac.check_local_datawriter_match(h, h, &ep, &ep).unwrap(),
            yes
        );
        assert_eq!(
            ac.check_local_datareader_match(h, h, &ep, &ep).unwrap(),
            yes
        );
        assert_eq!(
            ac.check_remote_datawriter_register_instance(h, guid, [8; 16], &key)
                .unwrap(),
            yes
        );
        assert_eq!(
            ac.check_remote_datawriter_dispose_instance(h, guid, [8; 16], &key)
                .unwrap(),
            yes
        );
        // An unknown handle is an error, not a TRUE.
        let bogus = PermissionsHandle(4242);
        assert!(
            ac.check_local_datawriter_register_instance(bogus, guid, &key)
                .is_err()
        );
        assert!(ac.check_local_datawriter_match(bogus, h, &ep, &ep).is_err());
        assert!(
            ac.check_remote_datawriter_dispose_instance(bogus, guid, [8; 16], &key)
                .is_err()
        );
    }

    /// Table 70 `get_participant_security_config` with §10.4.1.2.5.
    #[test]
    fn the_participant_config_follows_the_domain_rule() {
        use zerodds_security::access_control::{
            participant_attributes_mask as m, plugin_participant_attributes_mask as p,
        };
        let (mut ac, h) = carol();
        let config = ac.get_participant_security_config(h).unwrap();
        assert!(!config.allow_unauthenticated_participants);
        assert!(config.is_access_protected);
        assert!(config.is_rtps_axk_protected);
        assert!(config.is_rtps_psk_protected);
        assert!(config.is_discovery_protected);
        assert!(config.is_liveliness_protected);
        assert!(config.is_key_revision_enabled);
        assert_eq!(
            config.attributes_mask(),
            m::IS_VALID
                | m::IS_RTPS_AXK_PROTECTED
                | m::IS_DISCOVERY_PROTECTED
                | m::IS_LIVELINESS_PROTECTED
                | m::IS_KEY_REVISION_ENABLED
                | m::IS_RTPS_PSK_PROTECTED
        );
        // rtps ENCRYPT_WITH_OA, discovery ENCRYPT, liveliness SIGN_WITH_OA, psk ENCRYPT.
        assert_eq!(
            config.plugin_participant_attributes,
            p::IS_RTPS_AXK_ENCRYPTED
                | p::IS_DISCOVERY_ENCRYPTED
                | p::IS_RTPS_ORIGIN_AUTHENTICATED
                | p::IS_LIVELINESS_ORIGIN_AUTHENTICATED
                | p::IS_RTPS_PSK_ENCRYPTED
        );
        assert_eq!(
            config.ac_participant_properties,
            vec![Property::local(
                PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
                "true"
            )]
        );
        // allowed_crypto_algorithms: the three given elements restrict, the
        // trust chain follows the message authentication set, nothing is
        // required.
        let info = config.algorithm_info;
        assert_eq!(
            info.digital_signature.message_auth.supported_mask,
            zerodds_security::algorithm::CBIT_ECDSA_P256_SHA256
        );
        assert_eq!(
            info.digital_signature.trust_chain.supported_mask,
            zerodds_security::algorithm::CBIT_ECDSA_P256_SHA256
        );
        assert_eq!(
            info.key_establishment.shared_secret.supported_mask,
            zerodds_security::algorithm::CBIT_ECDHE_CEUM_P256
        );
        assert_eq!(
            info.symmetric_cipher.supported_mask,
            zerodds_security::algorithm::CBIT_AES256_GCM
        );
        assert_eq!(info.digital_signature.message_auth.required_mask, 0);
        assert_eq!(info.key_establishment.shared_secret.required_mask, 0);
        assert_eq!(info.symmetric_cipher.builtin_endpoints_required_mask, 0);
        assert_eq!(info.symmetric_cipher.builtin_kx_endpoints_required_mask, 0);
        assert_eq!(
            info.symmetric_cipher.user_endpoints_default_required_mask,
            0
        );
    }

    /// Table 70: "If the Governance document does not have the XML element
    /// <allowed_crypto_algorithms> the algorithm_info shall have the
    /// supported_mask [...] set to CRYPTO_ALGORITHM_SET_ALL"; the elements
    /// default to the open document: join access control, no unauthenticated
    /// participants stay as written.
    #[test]
    fn without_allowed_algorithms_every_algorithm_is_supported() {
        let governance = r#"<dds><domain_access_rules><domain_rule>
          <domains><id>0</id></domains>
          <allow_unauthenticated_participants>true</allow_unauthenticated_participants>
          <enable_join_access_control>false</enable_join_access_control>
          <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
        </domain_rule></domain_access_rules></dds>"#;
        let mut ac = PermissionsAccessControl::new()
            .with_governance(parse_governance_xml(governance).unwrap());
        let h = ac.register("CN=alice".into(), parse_permissions_xml(ALICE_XML).unwrap());
        // register() names no domain; validate_local_permissions does.
        assert!(ac.get_participant_security_config(h).is_err());
        let h = ac
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(
                    0,
                    [1; 16],
                    &PropertyList::new()
                        .with(Property::local(PROP_PERMISSIONS_XML, ALICE_XML.to_string()))
                        .with(Property::local(PROP_SUBJECT_NAME, "CN=alice")),
                ),
            )
            .unwrap();
        let config = ac.get_participant_security_config(h).unwrap();
        assert!(config.allow_unauthenticated_participants);
        assert!(!config.is_access_protected);
        assert!(!config.is_rtps_axk_protected);
        assert!(!config.is_rtps_psk_protected);
        assert!(!config.is_key_revision_enabled);
        assert!(config.ac_participant_properties.is_empty());
        let all = zerodds_security::algorithm::CRYPTO_ALGORITHM_SET_ALL;
        let info = config.algorithm_info;
        assert_eq!(info.digital_signature.message_auth.supported_mask, all);
        assert_eq!(info.digital_signature.trust_chain.supported_mask, all);
        assert_eq!(info.key_establishment.shared_secret.supported_mask, all);
        assert_eq!(info.symmetric_cipher.supported_mask, all);
        // Table 70: not access protected, so joining needs no grant.
        assert_eq!(
            ac.check_create_participant(h, &pr(0)).unwrap(),
            AccessDecision::Permit
        );
    }

    /// Table 70: the trust chain `required_mask` is the set of algorithms of
    /// the identity certificate, with the compatibility mode bit for two or
    /// more; one the governance does not allow is an error.
    #[test]
    fn the_identity_certificates_algorithms_must_be_allowed() {
        use zerodds_security::algorithm::{
            CBIT_ECDSA_P256_SHA256, CBIT_ECDSA_P384_SHA384, CRYPTO_ALGORITHM_COMPATIBILITY_MODE,
        };
        use zerodds_security::authentication::AlgorithmMasks;
        let masks = |required: u32| AlgorithmMasks {
            trust_chain: (required, required),
            message_auth: (0, 0),
            key_establishment: (0, 0),
        };
        let validate = |ac: &mut PermissionsAccessControl, required: u32| {
            let props = PropertyList::new()
                .with(Property::local(PROP_PERMISSIONS_XML, CAROL_XML.to_string()))
                .with(Property::local(PROP_SUBJECT_NAME, "CN=carol"));
            let mut request = ParticipantRequest::new(0, [0xCC; 16], &props);
            request.identity_algorithms = Some(masks(required));
            ac.validate_local_permissions(IdentityHandle(1), &request)
                .unwrap()
        };
        let mut ac = PermissionsAccessControl::new()
            .with_governance(parse_governance_xml(GOVERNANCE).unwrap());
        let one = validate(&mut ac, CBIT_ECDSA_P256_SHA256);
        let config = ac.get_participant_security_config(one).unwrap();
        assert_eq!(
            config
                .algorithm_info
                .digital_signature
                .trust_chain
                .required_mask,
            CBIT_ECDSA_P256_SHA256,
            "one algorithm: no compatibility bit"
        );
        // P-384 is not in <digital_signature>.
        let denied = validate(&mut ac, CBIT_ECDSA_P384_SHA384);
        assert_eq!(
            ac.get_participant_security_config(denied).unwrap_err().kind,
            SecurityErrorKind::AccessDenied
        );
        // Two algorithms, one of them not allowed.
        let both = validate(&mut ac, CBIT_ECDSA_P256_SHA256 | CBIT_ECDSA_P384_SHA384);
        assert!(ac.get_participant_security_config(both).is_err());
        // With every signature allowed, two give the compatibility bit.
        let open = r#"<dds><domain_access_rules><domain_rule>
          <domains><id>0</id></domains>
          <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
        </domain_rule></domain_access_rules></dds>"#;
        let mut ac =
            PermissionsAccessControl::new().with_governance(parse_governance_xml(open).unwrap());
        let two = validate(&mut ac, CBIT_ECDSA_P256_SHA256 | CBIT_ECDSA_P384_SHA384);
        let config = ac.get_participant_security_config(two).unwrap();
        assert_eq!(
            config
                .algorithm_info
                .digital_signature
                .trust_chain
                .required_mask,
            CBIT_ECDSA_P256_SHA256 | CBIT_ECDSA_P384_SHA384 | CRYPTO_ALGORITHM_COMPATIBILITY_MODE
        );
    }

    /// §10.4.1.2.6.2 to §10.4.1.2.6.7: the topic and endpoint configs.
    #[test]
    fn the_topic_and_endpoint_configs_follow_the_topic_rule() {
        use zerodds_security::access_control::{
            endpoint_attributes_mask as m, plugin_endpoint_attributes_mask as p,
        };
        let (mut ac, h) = carol();
        let secret = ac.get_topic_security_config(h, "Secret").unwrap();
        assert_eq!(
            secret,
            TopicSecurityConfig {
                is_read_protected: true,
                is_write_protected: true,
                is_discovery_protected: true,
                is_liveliness_protected: true
            }
        );
        let write_only = ac.get_topic_security_config(h, "WriteOnly").unwrap();
        assert!(!write_only.is_read_protected && write_only.is_write_protected);
        assert!(!write_only.is_discovery_protected && !write_only.is_liveliness_protected);
        assert!(ac.get_topic_security_config(h, "Unruled").is_err());

        let writer = ac
            .get_datawriter_security_config(h, &EndpointRequest::new(0, "Secret"))
            .unwrap();
        assert!(
            writer.is_submessage_protected
                && writer.is_payload_protected
                && writer.is_key_protected
        );
        assert_eq!(
            writer.plugin_endpoint_attributes,
            p::IS_SUBMESSAGE_ENCRYPTED
                | p::IS_PAYLOAD_ENCRYPTED
                | p::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED
        );
        assert_eq!(
            writer.attributes_mask(),
            m::IS_VALID
                | m::IS_READ_PROTECTED
                | m::IS_WRITE_PROTECTED
                | m::IS_DISCOVERY_PROTECTED
                | m::IS_SUBMESSAGE_PROTECTED
                | m::IS_PAYLOAD_PROTECTED
                | m::IS_KEY_PROTECTED
                | m::IS_LIVELINESS_PROTECTED
        );
        assert_eq!(
            writer.ac_endpoint_properties,
            vec![
                Property::local(
                    ENDPOINT_METADATA_PROTECTION_KIND,
                    "ENCRYPT_WITH_ORIGIN_AUTHENTICATION"
                ),
                Property::local(ENDPOINT_DATA_PROTECTION_KIND, "ENCRYPT"),
            ]
        );
        // data_protection_kind SIGN: protected, the key is not, nothing is encrypted.
        let sign = ac
            .get_datareader_security_config(h, &EndpointRequest::new(0, "WriteOnly"))
            .unwrap();
        assert!(
            sign.is_submessage_protected && sign.is_payload_protected && !sign.is_key_protected
        );
        assert_eq!(sign.plugin_endpoint_attributes, 0);
        let plain = ac
            .get_datawriter_security_config(h, &EndpointRequest::new(0, "Plain"))
            .unwrap();
        assert!(
            !plain.is_submessage_protected
                && !plain.is_payload_protected
                && !plain.is_key_protected
        );
        assert_eq!(plain.attributes_mask(), m::IS_VALID);
        assert_eq!(plain.algorithm_info.symmetric_cipher.required_mask, 0);
        assert_eq!(
            plain.algorithm_info.symmetric_cipher.supported_mask,
            zerodds_security::algorithm::CBIT_AES256_GCM
        );
    }

    /// §9.4.2.9.20 to §9.4.2.9.29: what was not issued cannot be returned,
    /// and what was is returned once.
    #[test]
    fn returned_tokens_and_configs_must_have_been_issued() {
        let (mut ac, h) = carol();
        let token = ac.get_permissions_token(h).unwrap();
        ac.return_permissions_token(token.clone()).unwrap();
        assert!(ac.return_permissions_token(token).is_err(), "returned once");
        assert!(ac.return_permissions_token(DataHolder::new("x")).is_err());
        assert!(
            ac.return_permissions_credential_token(DataHolder::new(
                "DDS:Access:PermissionsCredential"
            ))
            .is_err()
        );

        let config = ac.get_participant_security_config(h).unwrap();
        assert!(
            ac.return_participant_security_config(ParticipantSecurityConfig::default())
                .is_err()
        );
        ac.return_participant_security_config(config).unwrap();

        let topic = ac.get_topic_security_config(h, "Secret").unwrap();
        ac.return_topic_security_config(topic).unwrap();
        assert!(ac.return_topic_security_config(topic).is_err());

        let writer = ac
            .get_datawriter_security_config(h, &EndpointRequest::new(0, "Secret"))
            .unwrap();
        let reader = ac
            .get_datareader_security_config(h, &EndpointRequest::new(0, "Plain"))
            .unwrap();
        assert!(
            ac.return_datareader_security_config(writer.clone())
                .is_err(),
            "a writer's config is not a reader's"
        );
        ac.return_datawriter_security_config(writer).unwrap();
        ac.return_datareader_security_config(reader).unwrap();
    }

    /// Table 65: `dds.perm_ca.sn` and `dds.perm_ca.algo` describe the
    /// Permissions CA. The CA of the fixture is signed with ecdsa-with-SHA256,
    /// its subject in DER order is CN, O, C. By default the subject is the
    /// RFC 4514 string (§9.4.1.2.3, the last RDN first) and the algorithm the
    /// name of Table 25. With the settings, the form of OpenSSL
    /// `X509_NAME_oneline` (DER order, `/` before each RDN) and the name of
    /// DDS-Security 1.1, `EC-prime256v1`, which Fast DDS 3.6 maps to its own
    /// `ECDSA-SHA256` before it compares (`parse_token_algo`,
    /// `ECDSA_SHA256_FOR_TOKENS` in PKIIdentityHandle.h; sources read).
    #[test]
    fn the_permissions_ca_is_described_in_the_forms_that_are_set() {
        use zerodds_security::SubjectNameForm;
        use zerodds_security_pki::AlgorithmNames;
        const CA: &[u8] = include_bytes!("../tests/fixtures/permissions_ca_cert.pem");
        let (mut ac, h) = carol();
        ac.arm_remote_permissions_verifier(CA).unwrap();
        let described = |ac: &mut PermissionsAccessControl| {
            let token = ac.get_permissions_token(h).unwrap();
            (
                token.property(prop::PERM_CA_SN).map(str::to_string),
                token.property(prop::PERM_CA_ALGO).map(str::to_string),
            )
        };
        let rfc4514 = Some("C=DE,O=ZeroDDS,CN=ZeroDDS Permissions CA".to_string());
        let table_25 = Some("ECDSA+P256+SHA256".to_string());
        assert_eq!(described(&mut ac), (rfc4514.clone(), table_25.clone()));
        // `Auto` has no peer version for a token announced to every peer.
        ac.set_algorithm_names(AlgorithmNames::Auto);
        assert_eq!(described(&mut ac), (rfc4514.clone(), table_25));
        ac.set_algorithm_names(AlgorithmNames::V1_1);
        assert_eq!(
            described(&mut ac),
            (rfc4514, Some("EC-prime256v1".to_string()))
        );
        ac.set_subject_name_form(SubjectNameForm::X509NameOneline);
        assert_eq!(
            described(&mut ac),
            (
                Some("/CN=ZeroDDS Permissions CA/O=ZeroDDS/C=DE".to_string()),
                Some("EC-prime256v1".to_string())
            )
        );
    }

    /// Table 65: the PermissionsToken has the class `DDS:Access:Permissions:1.2`;
    /// the opt-in setting names 1.0; a class of another plugin is refused.
    #[test]
    fn the_permissions_token_class_is_1_2_unless_named() {
        let (mut ac, h) = carol();
        assert_eq!(ac.plugin_class_id(), "DDS:Access:Permissions:1.2");
        assert_eq!(
            ac.get_permissions_token(h).unwrap().class_id,
            "DDS:Access:Permissions:1.2"
        );
        ac.set_token_class_id("DDS:Access:Permissions:1.0").unwrap();
        assert_eq!(
            ac.get_permissions_token(h).unwrap().class_id,
            "DDS:Access:Permissions:1.0"
        );
        assert_eq!(
            ac.set_token_class_id("DDS:Access:PSK:1.2")
                .unwrap_err()
                .kind,
            SecurityErrorKind::InvalidConfiguration
        );
        assert!(ac.set_token_class_id("DDS:Access:Permissions:2.0").is_err());
        let props = PropertyList::new()
            .with(Property::local(PROP_PERMISSIONS_XML, CAROL_XML.to_string()))
            .with(Property::local(PROP_SUBJECT_NAME, "CN=carol"))
            .with(Property::local(
                PROP_TOKEN_CLASS_ID,
                "DDS:Access:Permissions:1.0",
            ));
        let mut other = PermissionsAccessControl::new();
        let handle = other
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(0, [1; 16], &props),
            )
            .unwrap();
        assert_eq!(
            other.get_permissions_token(handle).unwrap().class_id,
            "DDS:Access:Permissions:1.0"
        );
        // Without a signed document there is nothing to represent on the network.
        assert_eq!(
            other
                .get_permissions_credential_token(handle)
                .unwrap_err()
                .minor_code,
            minor_code::NO_CREDENTIAL
        );
    }

    struct Revocations(std::sync::Mutex<Vec<(String, PermissionsHandle)>>);
    impl AccessControlListener for Revocations {
        fn on_revoke_permissions(&self, plugin: &str, handle: PermissionsHandle) -> bool {
            self.0.lock().unwrap().push((plugin.to_string(), handle));
            true
        }
    }

    /// §9.4.2.10: the listener is told which handle is revoked, and the
    /// handle is gone afterwards.
    #[test]
    fn revoking_permissions_tells_the_listener() {
        let (mut ac, h) = carol();
        let listener = Arc::new(Revocations(std::sync::Mutex::new(Vec::new())));
        ac.set_listener(listener.clone()).unwrap();
        assert!(ac.revoke_permissions(h));
        assert_eq!(
            *listener.0.lock().unwrap(),
            vec![("DDS:Access:Permissions:1.2".to_string(), h)]
        );
        assert!(
            ac.check_create_datawriter(h, &EndpointRequest::new(0, "Secret"))
                .is_err(),
            "a revoked handle decides nothing"
        );
        assert!(!ac.revoke_permissions(h), "revoked once");
    }

    fn clock_2200() -> u64 {
        7_258_118_400
    }

    /// A grant that expired is not applied by a check and its handle is
    /// revoked by `revoke_expired`.
    #[test]
    fn a_grant_that_expired_decides_nothing() {
        let mut ac = PermissionsAccessControl::new().with_clock(clock_2200);
        let h = ac.register("CN=alice".into(), parse_permissions_xml(ALICE_XML).unwrap());
        assert_eq!(
            ac.check_create_datawriter(h, &EndpointRequest::new(0, "Chatter"))
                .unwrap(),
            AccessDecision::Deny,
            "ALICE_XML is valid until 2100"
        );
        assert_eq!(
            ac.check_create_participant(h, &pr(0)).unwrap(),
            AccessDecision::Deny
        );
        let listener = Arc::new(Revocations(std::sync::Mutex::new(Vec::new())));
        ac.set_listener(listener.clone()).unwrap();
        assert_eq!(ac.revoke_expired(), vec![h]);
        assert_eq!(listener.0.lock().unwrap().len(), 1);
        // A handle in time stays.
        let mut live = PermissionsAccessControl::new();
        let h = live.register("CN=alice".into(), parse_permissions_xml(ALICE_XML).unwrap());
        assert!(live.revoke_expired().is_empty());
        assert!(live.check_create_participant(h, &pr(0)).is_ok());
    }

    /// trent may relay `Secret` and nothing else.
    const TRENT_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=trent</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id>0</id></domains>
      <relay><topics><topic>Secret</topic></topics></relay>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    /// §7.1.1.4 with §10.4.1.5.3.4.1.2: where a topic is access-controlled, a
    /// reader relays it and a writer forwards for others only with the
    /// `<relay>` action; a grant to publish or subscribe does not do. A topic
    /// without access control is open to everyone, no rule refuses.
    #[test]
    fn relaying_needs_the_relay_action_where_the_topic_is_controlled() {
        use zerodds_security::access_control::RelayRole;
        let mut ac = PermissionsAccessControl::new()
            .with_governance(parse_governance_xml(GOVERNANCE).unwrap());
        let trent = ac.register("CN=trent".into(), parse_permissions_xml(TRENT_XML).unwrap());
        let carol = ac.register("CN=carol".into(), parse_permissions_xml(CAROL_XML).unwrap());
        let secret = EndpointRequest::new(0, "Secret");
        for role in [RelayRole::Reader, RelayRole::Writer] {
            assert_eq!(
                ac.check_relay(trent, role, &secret).unwrap(),
                AccessDecision::Permit,
                "{role:?}"
            );
            assert_eq!(
                ac.check_relay(carol, role, &secret).unwrap(),
                AccessDecision::Deny,
                "publish and subscribe are not relay: {role:?}"
            );
            assert_eq!(
                ac.check_relay(carol, role, &EndpointRequest::new(0, "Plain"))
                    .unwrap(),
                AccessDecision::Permit,
                "an open topic"
            );
            assert_eq!(
                ac.check_relay(trent, role, &EndpointRequest::new(0, "Unruled"))
                    .unwrap(),
                AccessDecision::Deny,
                "no topic rule"
            );
            assert_eq!(
                ac.check_relay(PermissionsHandle::NIL, role, &secret)
                    .unwrap(),
                AccessDecision::Deny
            );
        }
        // A writer of the open WriteOnly topic: reading is open, writing is not.
        let write_only = EndpointRequest::new(0, "WriteOnly");
        assert_eq!(
            ac.check_relay(PermissionsHandle::NIL, RelayRole::Reader, &write_only)
                .unwrap(),
            AccessDecision::Permit
        );
        assert_eq!(
            ac.check_relay(PermissionsHandle::NIL, RelayRole::Writer, &write_only)
                .unwrap(),
            AccessDecision::Deny
        );
        assert!(
            ac.check_relay(PermissionsHandle(4242), RelayRole::Reader, &secret)
                .is_err()
        );
    }

    /// Without a governance document the three config operations fail.
    #[test]
    fn the_config_operations_need_a_governance() {
        let (mut ac, h) = build_alice();
        assert_eq!(
            ac.get_participant_security_config(h).unwrap_err().kind,
            SecurityErrorKind::InvalidConfiguration
        );
        assert!(ac.get_topic_security_config(h, "Chatter").is_err());
        assert!(
            ac.get_datawriter_security_config(h, &EndpointRequest::new(0, "Chatter"))
                .is_err()
        );
    }

    /// Table 70 `validate_remote_permissions` needs the permissions CA and
    /// the credential of the authentication.
    #[test]
    fn a_remote_validation_without_ca_or_credential_is_refused() {
        let mut ac = PermissionsAccessControl::new();
        let token = DataHolder::new("DDS:Access:Permissions:1.2");
        let credential = DataHolder::new("DDS:Auth:PKI-DH:1.0");
        let err = ac
            .validate_remote_permissions(IdentityHandle(1), IdentityHandle(2), &token, &credential)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AccessDenied);
        assert_eq!(err.minor_code, minor_code::NO_VERIFIER);
    }
}
