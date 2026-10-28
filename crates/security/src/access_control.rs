// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Access control plugin SPI (OMG DDS-Security 1.2 §9.4).
//!
//! Decides per topic / per operation whether an authenticated
//! participant may perform the action. Inputs:
//! * Governance XML — topic-level rules (discovery, publishing,
//!   subscribing, encrypt flag).
//! * Permissions XML — who may do what (signed with the permissions CA).
//!
//! The module carries the whole interface of §9.4.2: the types
//! [`PermissionsHandle`], [`ParticipantSecurityConfig`],
//! [`TopicSecurityConfig`], [`EndpointSecurityConfig`] with the algorithm
//! infos (§7.3.14, §7.3.16) and the masks of Tables 34, 35 and 38, the 29
//! operations of Table 39 on [`AccessControlPlugin`] and the
//! [`AccessControlListener`] of Table 40.
//!
//! Mapping to the IDL of the specification: an operation that returns a
//! `Boolean` with a `SecurityException` returns
//! `SecurityResult<AccessDecision>` (`Ok(Permit)` is TRUE, `Ok(Deny)` is
//! FALSE without an exception, `Err` is FALSE with one); an operation that
//! returns a handle or a token returns it in `Ok`, and `HandleNIL` is
//! [`PermissionsHandle::NIL`]. A parameter that is a QoS or a
//! builtin-topic-data type is the part of it the decision depends on
//! (see [`EndpointRequest`], [`TopicRequest`], [`ParticipantRequest`]).
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn AccessControlPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::authentication::{AlgorithmMasks, IdentityHandle};
use crate::error::SecurityResult;
use crate::properties::{Property, PropertyList};
use crate::token::{DataHolder, PermissionsCredentialToken, PermissionsToken};

/// Opaque handle for validated permissions (§9.4.2.3). Created by
/// [`AccessControlPlugin::validate_local_permissions`] or
/// `validate_remote_permissions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PermissionsHandle(pub u64);

impl PermissionsHandle {
    /// `HandleNIL`: no permissions are known. The checks that take a handle
    /// answer for it as their definition says (the builtin plugin applies the
    /// governance alone; a plugin that cannot decide answers `Deny`).
    pub const NIL: Self = Self(0);

    /// `true` for [`Self::NIL`].
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0 == 0
    }
}

/// Whether an action may be performed. Deliberately sparse — no
/// reason string, because the reason must not reveal anything to the
/// remote (logging via [`crate::LoggingPlugin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessDecision {
    /// Allowed.
    Permit,
    /// Denied.
    Deny,
}

impl AccessDecision {
    /// `true` if `Permit`.
    #[must_use]
    pub fn is_permitted(self) -> bool {
        matches!(self, Self::Permit)
    }
}

impl From<bool> for AccessDecision {
    fn from(allowed: bool) -> Self {
        if allowed { Self::Permit } else { Self::Deny }
    }
}

/// The QoS policies of an entity as `(name, value)` pairs, the name that of
/// the DDS QoS policy (`reliability`, `durability`, `ownership`,
/// `liveliness`, ...) with the textual rendering of its value, and
/// `property.<name>` for an entry of its `PropertyQosPolicy`. This is what
/// the `qos` parameter of the checks of §9.4.2.9 carries; the builtin plugin
/// decides on the partitions and the data tags only (Table 70).
pub type QosEntries<'a> = &'a [(&'a str, &'a str)];

/// What the participant checks of §9.4.2.9.1, §9.4.2.9.3 and §9.4.2.9.9
/// decide on: the domain (id and DomainTag), the GUID and the
/// `DomainParticipantQos` of the participant.
#[derive(Debug, Clone, Copy)]
pub struct ParticipantRequest<'a> {
    /// The domain id of the participant.
    pub domain_id: u32,
    /// The DomainTag of the participant (§10.4.1.2.5.1); `""` for none.
    pub domain_tag: &'a str,
    /// The GUID of the participant.
    pub participant_guid: [u8; 16],
    /// The `PropertyQosPolicy` of the participant: where a plugin finds its
    /// settings (`dds.sec.access.*`).
    pub properties: &'a PropertyList,
    /// The algorithms of the identity certificate, from
    /// `AuthenticationPlugin::algorithm_masks` of the plugin that validated
    /// the identity: the `trust_chain.required_mask` of
    /// `get_participant_security_config` (Table 70). `None` when the
    /// authentication plugin has none.
    pub identity_algorithms: Option<AlgorithmMasks>,
}

impl<'a> ParticipantRequest<'a> {
    /// A request in `domain_id` for the participant `participant_guid`,
    /// with the empty DomainTag, no properties and no algorithms.
    #[must_use]
    pub fn new(domain_id: u32, participant_guid: [u8; 16], properties: &'a PropertyList) -> Self {
        Self {
            domain_id,
            domain_tag: "",
            participant_guid,
            properties,
            identity_algorithms: None,
        }
    }
}

/// What the topic checks of §9.4.2.9.6 and §9.4.2.9.12 decide on.
#[derive(Debug, Clone, Copy)]
pub struct TopicRequest<'a> {
    /// The domain id of the participant.
    pub domain_id: u32,
    /// The topic name.
    pub topic_name: &'a str,
    /// The `TopicQos`, see [`QosEntries`].
    pub qos: QosEntries<'a>,
}

impl<'a> TopicRequest<'a> {
    /// A request in `domain_id` for `topic_name` without QoS entries.
    #[must_use]
    pub const fn new(domain_id: u32, topic_name: &'a str) -> Self {
        Self {
            domain_id,
            topic_name,
            qos: &[],
        }
    }
}

/// What the endpoint checks of DDS-Security 1.2 §9.4.2.9.4 to
/// §9.4.2.9.14 decide on: the domain, the topic, the QoS of the endpoint,
/// the `PartitionQosPolicy` of the Publisher or Subscriber, and the
/// `DataTagQosPolicy` of the endpoint. The checks took the topic name
/// alone, so a plugin could not apply the rules of a grant, which hold per
/// domain, partition and tag.
#[derive(Debug, Clone, Copy)]
pub struct EndpointRequest<'a> {
    /// The domain id of the participant.
    pub domain_id: u32,
    /// The topic name.
    pub topic_name: &'a str,
    /// The partitions; none is the empty-string partition.
    pub partitions: &'a [&'a str],
    /// The data tags as (name, value).
    pub data_tags: &'a [(&'a str, &'a str)],
    /// The `DataWriterQos` or `DataReaderQos`, see [`QosEntries`].
    pub qos: QosEntries<'a>,
}

impl<'a> EndpointRequest<'a> {
    /// A request in `domain_id` for `topic_name`, in the default partition
    /// and without data tags and QoS entries.
    #[must_use]
    pub const fn new(domain_id: u32, topic_name: &'a str) -> Self {
        Self {
            domain_id,
            topic_name,
            partitions: &[],
            data_tags: &[],
            qos: &[],
        }
    }

    /// The request with `partitions`.
    #[must_use]
    pub const fn in_partitions(self, partitions: &'a [&'a str]) -> Self {
        Self { partitions, ..self }
    }

    /// The request with `data_tags`.
    #[must_use]
    pub const fn with_data_tags(self, data_tags: &'a [(&'a str, &'a str)]) -> Self {
        Self { data_tags, ..self }
    }

    /// The request with the QoS entries `qos`.
    #[must_use]
    pub const fn with_qos(self, qos: QosEntries<'a>) -> Self {
        Self { qos, ..self }
    }
}

/// The side of the relay service of DDS-Security 1.2 §7.1.1.4 an endpoint
/// is on, see [`AccessControlPlugin::check_relay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayRole {
    /// A DataReader that receives and stores the topic for forwarding,
    /// without the key to its data.
    Reader,
    /// A DataWriter that forwards samples on behalf of other writers.
    Writer,
}

/// What `check_remote_datareader` decides (DDS-Security 1.2 §9.4.2.9.11,
/// out parameter `relay_only`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteReaderDecision {
    /// The reader may subscribe.
    Permit,
    /// The reader may only relay the topic (`relay_only` TRUE).
    RelayOnly,
    /// Refused.
    Deny,
}

// ---------------------------------------------------------------------------
// Algorithm infos (§7.3.10 to §7.3.16)
// ---------------------------------------------------------------------------

/// `CryptoAlgorithmRequirements` (§7.3.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CryptoAlgorithmRequirements {
    /// The algorithms the plugin supports.
    pub supported_mask: u32,
    /// The subset of them the plugin uses and requires its peer to support.
    pub required_mask: u32,
}

/// `ParticipantSecurityDigitalSignatureAlgorithmInfo` (§7.3.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DigitalSignatureAlgorithmInfo {
    /// The algorithms of signed documents.
    pub trust_chain: CryptoAlgorithmRequirements,
    /// The algorithms of the signatures in the handshake.
    pub message_auth: CryptoAlgorithmRequirements,
}

/// `ParticipantSecurityKeyEstablishmentAlgorithmInfo` (§7.3.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyEstablishmentAlgorithmInfo {
    /// The key agreement algorithms.
    pub shared_secret: CryptoAlgorithmRequirements,
}

/// `ParticipantSecuritySymmetricCipherAlgorithmInfo` (§7.3.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SymmetricCipherAlgorithmInfo {
    /// The algorithms to decrypt or authenticate with.
    pub supported_mask: u32,
    /// What the builtin endpoints, except the Kx ones, require.
    pub builtin_endpoints_required_mask: u32,
    /// What the Kx builtin endpoints require.
    pub builtin_kx_endpoints_required_mask: u32,
    /// What a user endpoint requires by default.
    pub user_endpoints_default_required_mask: u32,
}

/// `ParticipantSecurityAlgorithmInfo` (§7.3.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityAlgorithmInfo {
    /// The digital signature algorithms.
    pub digital_signature: DigitalSignatureAlgorithmInfo,
    /// The key establishment algorithms.
    pub key_establishment: KeyEstablishmentAlgorithmInfo,
    /// The symmetric cipher algorithms.
    pub symmetric_cipher: SymmetricCipherAlgorithmInfo,
}

/// `EndpointSecuritySymmetricCipherAlgorithmInfo` (§7.3.15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EndpointSymmetricCipherAlgorithmInfo {
    /// The algorithms a peer must support to interoperate with the endpoint.
    pub required_mask: u32,
    /// The algorithms the plugin supports (`@non_serialized`).
    pub supported_mask: u32,
}

/// `EndpointSecurityAlgorithmInfo` (§7.3.16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EndpointSecurityAlgorithmInfo {
    /// The symmetric cipher algorithms of the endpoint.
    pub symmetric_cipher: EndpointSymmetricCipherAlgorithmInfo,
}

// ---------------------------------------------------------------------------
// Masks (§9.4.2.5, §9.4.2.8, §10.4.2.4, §10.4.2.6)
// ---------------------------------------------------------------------------

/// `ParticipantSecurityAttributesMask` (§9.4.2.5, Table 34).
pub mod participant_attributes_mask {
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_AXK_PROTECTED`.
    pub const IS_RTPS_AXK_PROTECTED: u32 = 1 << 0;
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_DISCOVERY_PROTECTED`.
    pub const IS_DISCOVERY_PROTECTED: u32 = 1 << 1;
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_LIVELINESS_PROTECTED`.
    pub const IS_LIVELINESS_PROTECTED: u32 = 1 << 2;
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_KEY_REVISION_ENABLED`.
    pub const IS_KEY_REVISION_ENABLED: u32 = 1 << 3;
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_PROTECTED`.
    pub const IS_RTPS_PSK_PROTECTED: u32 = 1 << 4;
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_VALID`.
    pub const IS_VALID: u32 = 1 << 31;
}

/// `ParticipantSecurityOptionalAttributesMask` (§9.4.2.5, Table 35); the
/// bits apply to both `is_set` and `value`.
pub mod participant_optional_attributes_mask {
    /// `PARTICIPANT_SECURITY_OPT_ATTRIBUTES_FLAG_ALLOW_UNAUTHENTICATED_PARTICIPANTS`.
    pub const ALLOW_UNAUTHENTICATED_PARTICIPANTS: u16 = 1 << 0;
    /// `PARTICIPANT_SECURITY_OPT_ATTRIBUTES_FLAG_IS_ACCESS_PROTECTED`.
    pub const IS_ACCESS_PROTECTED: u16 = 1 << 1;
}

/// `PluginParticipantSecurityAttributesMask` (§10.4.2.4, Table 67).
pub mod plugin_participant_attributes_mask {
    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_AXK_ENCRYPTED`.
    pub const IS_RTPS_AXK_ENCRYPTED: u32 = 1 << 0;
    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_DISCOVERY_ENCRYPTED`.
    pub const IS_DISCOVERY_ENCRYPTED: u32 = 1 << 1;
    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_LIVELINESS_ENCRYPTED`.
    pub const IS_LIVELINESS_ENCRYPTED: u32 = 1 << 2;
    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED`.
    pub const IS_RTPS_ORIGIN_AUTHENTICATED: u32 = 1 << 3;
    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_DISCOVERY_ORIGIN_AUTHENTICATED`.
    pub const IS_DISCOVERY_ORIGIN_AUTHENTICATED: u32 = 1 << 4;
    /// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_LIVELINESS_ORIGIN_AUTHENTICATED`
    /// (Table 67 spells this one without `PLUGIN_`).
    pub const IS_LIVELINESS_ORIGIN_AUTHENTICATED: u32 = 1 << 5;
    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_ENCRYPTED`.
    pub const IS_RTPS_PSK_ENCRYPTED: u32 = 1 << 6;
}

/// `EndpointSecurityAttributesMask` (§9.4.2.8, Table 38).
pub mod endpoint_attributes_mask {
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_READ_PROTECTED`.
    pub const IS_READ_PROTECTED: u32 = 1 << 0;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_WRITE_PROTECTED`.
    pub const IS_WRITE_PROTECTED: u32 = 1 << 1;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_DISCOVERY_PROTECTED`.
    pub const IS_DISCOVERY_PROTECTED: u32 = 1 << 2;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_SUBMESSAGE_PROTECTED`.
    pub const IS_SUBMESSAGE_PROTECTED: u32 = 1 << 3;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_PAYLOAD_PROTECTED`.
    pub const IS_PAYLOAD_PROTECTED: u32 = 1 << 4;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_KEY_PROTECTED`.
    pub const IS_KEY_PROTECTED: u32 = 1 << 5;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_LIVELINESS_PROTECTED`.
    pub const IS_LIVELINESS_PROTECTED: u32 = 1 << 6;
    /// `ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_VALID`.
    pub const IS_VALID: u32 = 1 << 31;
}

/// `PluginEndpointSecurityAttributesMask` (§10.4.2.6, Table 69).
pub mod plugin_endpoint_attributes_mask {
    /// `PLUGIN_ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_SUBMESSAGE_ENCRYPTED`.
    pub const IS_SUBMESSAGE_ENCRYPTED: u32 = 1 << 0;
    /// `PLUGIN_ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_PAYLOAD_ENCRYPTED`.
    pub const IS_PAYLOAD_ENCRYPTED: u32 = 1 << 1;
    /// `PLUGIN_ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED`.
    pub const IS_SUBMESSAGE_ORIGIN_AUTHENTICATED: u32 = 1 << 2;
}

// ---------------------------------------------------------------------------
// Security configs (§9.4.2.4, §9.4.2.6, §9.4.2.7)
// ---------------------------------------------------------------------------

/// `ParticipantSecurityConfig` (§9.4.2.4, Table 33).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticipantSecurityConfig {
    /// A participant that cannot authenticate may be matched.
    pub allow_unauthenticated_participants: bool,
    /// A remote participant is checked with `validate_remote_permissions`
    /// and `check_remote_participant`.
    pub is_access_protected: bool,
    /// The RTPS messages are protected with the participant key exchanged
    /// after authentication.
    pub is_rtps_axk_protected: bool,
    /// All RTPS messages are protected with a pre-shared key.
    pub is_rtps_psk_protected: bool,
    /// The submessages of the secure discovery endpoints are protected.
    pub is_discovery_protected: bool,
    /// The submessages of the secure liveliness endpoints are protected.
    pub is_liveliness_protected: bool,
    /// Key material is revised on certain events.
    pub is_key_revision_enabled: bool,
    /// `plugin_participant_attributes`: a
    /// `PluginParticipantSecurityAttributesMask` (Table 67,
    /// [`plugin_participant_attributes_mask`]).
    pub plugin_participant_attributes: u32,
    /// The properties added to the `participant_properties` of
    /// `register_local_participant`. The IDL names the member
    /// `ac_endpoint_properties`, Table 33 `ac_participant_properties`.
    pub ac_participant_properties: Vec<Property>,
    /// The algorithms the participant uses and supports.
    pub algorithm_info: ParticipantSecurityAlgorithmInfo,
}

impl ParticipantSecurityConfig {
    /// The `ParticipantSecurityAttributesMask` of the config (Table 34),
    /// with `IS_VALID`.
    #[must_use]
    pub fn attributes_mask(&self) -> u32 {
        use participant_attributes_mask as m;
        let mut mask = m::IS_VALID;
        for (set, bit) in [
            (self.is_rtps_axk_protected, m::IS_RTPS_AXK_PROTECTED),
            (self.is_discovery_protected, m::IS_DISCOVERY_PROTECTED),
            (self.is_liveliness_protected, m::IS_LIVELINESS_PROTECTED),
            (self.is_key_revision_enabled, m::IS_KEY_REVISION_ENABLED),
            (self.is_rtps_psk_protected, m::IS_RTPS_PSK_PROTECTED),
        ] {
            if set {
                mask |= bit;
            }
        }
        mask
    }

    /// The `ParticipantSecurityOptionalAttributesMask` of the config
    /// (Table 35) as `(is_set, value)`: both settings are set.
    #[must_use]
    pub fn optional_attributes_mask(&self) -> (u16, u16) {
        use participant_optional_attributes_mask as m;
        let is_set = m::ALLOW_UNAUTHENTICATED_PARTICIPANTS | m::IS_ACCESS_PROTECTED;
        let mut value = 0;
        if self.allow_unauthenticated_participants {
            value |= m::ALLOW_UNAUTHENTICATED_PARTICIPANTS;
        }
        if self.is_access_protected {
            value |= m::IS_ACCESS_PROTECTED;
        }
        (is_set, value)
    }
}

/// `TopicSecurityConfig` (§9.4.2.6, Table 36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TopicSecurityConfig {
    /// Read access to the topic is checked.
    pub is_read_protected: bool,
    /// Write access to the topic is checked.
    pub is_write_protected: bool,
    /// The discovery information of the entities uses the secure endpoints.
    pub is_discovery_protected: bool,
    /// The liveliness information uses the secure endpoint.
    pub is_liveliness_protected: bool,
}

/// `EndpointSecurityConfig` (§9.4.2.7, Table 37), which extends
/// [`TopicSecurityConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndpointSecurityConfig {
    /// The members of the base type `TopicSecurityConfig`.
    pub topic: TopicSecurityConfig,
    /// The submessages of the endpoint are protected.
    pub is_submessage_protected: bool,
    /// The serialized payload is protected.
    pub is_payload_protected: bool,
    /// The instance key is sensitive: its KeyHash is computed as §7.4.4.
    pub is_key_protected: bool,
    /// `plugin_endpoint_attributes`: a
    /// `PluginEndpointSecurityAttributesMask` (Table 69,
    /// [`plugin_endpoint_attributes_mask`]).
    pub plugin_endpoint_attributes: u32,
    /// The properties added to the `datawriter_properties` or
    /// `datareader_properties` of `register_local_datawriter` and
    /// `register_local_datareader`.
    pub ac_endpoint_properties: Vec<Property>,
    /// The algorithms the endpoint requires.
    pub algorithm_info: EndpointSecurityAlgorithmInfo,
}

impl EndpointSecurityConfig {
    /// The `EndpointSecurityAttributesMask` of the config (Table 38), with
    /// `IS_VALID`.
    #[must_use]
    pub fn attributes_mask(&self) -> u32 {
        use endpoint_attributes_mask as m;
        let mut mask = m::IS_VALID;
        for (set, bit) in [
            (self.topic.is_read_protected, m::IS_READ_PROTECTED),
            (self.topic.is_write_protected, m::IS_WRITE_PROTECTED),
            (self.topic.is_discovery_protected, m::IS_DISCOVERY_PROTECTED),
            (self.is_submessage_protected, m::IS_SUBMESSAGE_PROTECTED),
            (self.is_payload_protected, m::IS_PAYLOAD_PROTECTED),
            (self.is_key_protected, m::IS_KEY_PROTECTED),
            (
                self.topic.is_liveliness_protected,
                m::IS_LIVELINESS_PROTECTED,
            ),
        ] {
            if set {
                mask |= bit;
            }
        }
        mask
    }
}

// ---------------------------------------------------------------------------
// Listener (§9.4.2.10)
// ---------------------------------------------------------------------------

/// The `AccessControlListener` of §9.4.2.10, Table 40: notified when the
/// permissions of a participant change.
pub trait AccessControlListener: Send + Sync {
    /// `on_revoke_permissions` (§9.4.2.10.1): the permissions of the
    /// participant behind `handle` are revoked or changed. `plugin` is the
    /// class id of the AccessControl plugin that calls (its
    /// [`AccessControlPlugin::plugin_class_id`]). Returns `false` on an
    /// error.
    fn on_revoke_permissions(&self, plugin: &str, handle: PermissionsHandle) -> bool;
}

// ---------------------------------------------------------------------------
// The plugin (§9.4.2.9, Table 39)
// ---------------------------------------------------------------------------

/// Access control plugin trait (spec §9.4.2.9, Table 39).
pub trait AccessControlPlugin: Send + Sync {
    /// §9.4.2.9.1 `validate_local_permissions`: validates the permissions of
    /// the local participant before it is enabled and returns the handle
    /// that identifies them. `identity` is the handle of a successful
    /// `validate_local_identity`; the settings of the plugin are in
    /// `participant.properties`.
    ///
    /// # Errors
    /// The permissions do not validate (`HandleNIL` with an exception).
    fn validate_local_permissions(
        &mut self,
        identity: IdentityHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle>;

    /// §9.4.2.9.2 `validate_remote_permissions`: validates the permissions
    /// of a previously authenticated remote participant, given the
    /// `PermissionsToken` of its SPDP announcement and the
    /// `AuthenticatedPeerCredentialToken` the authentication plugin returned
    /// (for DDS:Auth:PKI-DH the binary properties `c.id` and `c.perm`).
    ///
    /// # Errors
    /// The permissions do not validate (`HandleNIL` with an exception).
    fn validate_remote_permissions(
        &mut self,
        local_identity: IdentityHandle,
        remote_identity: IdentityHandle,
        remote_permissions_token: &PermissionsToken,
        remote_credential_token: &DataHolder,
    ) -> SecurityResult<PermissionsHandle>;

    /// §9.4.2.9.3 `check_create_participant`: may the local participant join
    /// the domain with this QoS? Also called for `set_qos` of the
    /// participant.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check (FALSE with an
    /// exception).
    fn check_create_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.4 `check_create_datawriter`: may the local participant create
    /// a DataWriter for the topic with this QoS, in these partitions, with
    /// these data tags? Also called for `set_qos` of the DataWriter.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_create_datawriter(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.5 `check_create_datareader`, the mirror of
    /// [`Self::check_create_datawriter`].
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_create_datareader(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.6 `check_create_topic`: may the local participant create the
    /// topic with this QoS? Also called for `set_qos` of the Topic.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_create_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.7 `check_local_datawriter_register_instance`: may the local
    /// DataWriter `writer` (its GUID) register the instance with the
    /// serialized key `key`?
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_local_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        writer: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.8 `check_local_datawriter_dispose_instance`: may the local
    /// DataWriter dispose the instance with the serialized key `key`?
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_local_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        writer: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.9 `check_remote_participant`: may the remote participant
    /// (`permissions` is its handle, `participant` its data) join the
    /// domain? A FALSE makes the remote participant invalid.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_remote_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.10 `check_remote_datawriter`: may the remote DataWriter,
    /// whose participant has the handle `permissions`, match a local
    /// DataReader? Also called when the QoS of the remote DataWriter changes.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_remote_datawriter(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.11 `check_remote_datareader`: may the remote DataReader,
    /// whose participant has the handle `permissions`, match a local
    /// DataWriter, and only to relay (`relay_only`)?
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_remote_datareader(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<RemoteReaderDecision>;

    /// §9.4.2.9.12 `check_remote_topic`: may the remote participant create
    /// the topic (from its `topic_data`)?
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_remote_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.13 `check_local_datawriter_match`: may the local DataWriter
    /// `publication` (of the participant `writer_permissions`) match the
    /// DataReader `subscription` (of the participant `reader_permissions`),
    /// as far as the data tags decide? Called after the checks of both
    /// endpoints returned TRUE, and when the QoS of either changes.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_local_datawriter_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        publication: &EndpointRequest<'_>,
        subscription: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.14 `check_local_datareader_match`, the mirror of
    /// [`Self::check_local_datawriter_match`] for a local DataReader
    /// `subscription` and the DataWriter `publication` it matches. The
    /// specification names the participant handles `writer_permissions_handle`
    /// and `reader_permissions_handle` and describes the first as that of the
    /// participant of the local DataReader; here `reader_permissions` is that
    /// of the participant with the DataReader.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_local_datareader_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        subscription: &EndpointRequest<'_>,
        publication: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.15 `check_remote_datawriter_register_instance`: may the
    /// remote DataWriter `publication` (its GUID) register the instance with
    /// the serialized key `key`, as seen by the local DataReader `reader`?
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_remote_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        reader: [u8; 16],
        publication: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.16 `check_remote_datawriter_dispose_instance`: may the
    /// remote DataWriter dispose the instance with the key `key`?
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_remote_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        reader: [u8; 16],
        publication: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision>;

    /// §9.4.2.9.17 `get_permissions_token`: the `PermissionsToken` that
    /// summarizes the permissions of the participant for the SPDP
    /// announcement (`PID_PERMISSIONS_TOKEN`).
    ///
    /// # Errors
    /// An unknown handle.
    fn get_permissions_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsToken>;

    /// §9.4.2.9.18 `get_permissions_credential_token`: the
    /// `PermissionsCredentialToken` that represents the permissions of the
    /// participant on the network, for the authentication handshake.
    ///
    /// # Errors
    /// An unknown handle.
    fn get_permissions_credential_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsCredentialToken>;

    /// §9.4.2.9.19 `set_listener`: sets the listener of the plugin.
    ///
    /// # Errors
    /// The plugin cannot take the listener.
    fn set_listener(&mut self, listener: Arc<dyn AccessControlListener>) -> SecurityResult<()>;

    /// §9.4.2.9.20 `return_permissions_token`: returns a token of
    /// [`Self::get_permissions_token`] for disposal.
    ///
    /// # Errors
    /// The token is none that the plugin issued.
    fn return_permissions_token(&mut self, token: PermissionsToken) -> SecurityResult<()>;

    /// §9.4.2.9.21 `return_permissions_credential_token`: returns a token of
    /// [`Self::get_permissions_credential_token`] for disposal.
    ///
    /// # Errors
    /// The token is none that the plugin issued.
    fn return_permissions_credential_token(
        &mut self,
        token: PermissionsCredentialToken,
    ) -> SecurityResult<()>;

    /// §9.4.2.9.22 `get_participant_security_config`: how the middleware
    /// protects the information of the participant; called when the
    /// participant is created or enabled, and the result configures the
    /// Authentication plugin (`set_participant_security_config`) and the
    /// Cryptographic plugin (`register_local_participant`).
    ///
    /// # Errors
    /// An unknown handle, or a configuration that does not hold.
    fn get_participant_security_config(
        &mut self,
        permissions: PermissionsHandle,
    ) -> SecurityResult<ParticipantSecurityConfig>;

    /// §9.4.2.9.23 `get_topic_security_config`: how the middleware protects
    /// the topic; called when a topic is created, before the checks of its
    /// entities.
    ///
    /// # Errors
    /// An unknown handle, or a topic no rule covers.
    fn get_topic_security_config(
        &mut self,
        permissions: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<TopicSecurityConfig>;

    /// §9.4.2.9.24 `get_datawriter_security_config`: how the middleware
    /// protects a DataWriter; called after
    /// [`Self::check_create_datawriter`].
    ///
    /// # Errors
    /// An unknown handle, or a topic no rule covers.
    fn get_datawriter_security_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig>;

    /// §9.4.2.9.25 `get_datareader_security_config`: how the middleware
    /// protects a DataReader; called after
    /// [`Self::check_create_datareader`].
    ///
    /// # Errors
    /// An unknown handle, or a topic no rule covers.
    fn get_datareader_security_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig>;

    /// §9.4.2.9.26 `return_participant_security_config`.
    ///
    /// # Errors
    /// The config is none that the plugin issued.
    fn return_participant_security_config(
        &mut self,
        config: ParticipantSecurityConfig,
    ) -> SecurityResult<()>;

    /// §9.4.2.9.27 `return_topic_security_config`.
    ///
    /// # Errors
    /// The config is none that the plugin issued.
    fn return_topic_security_config(&mut self, config: TopicSecurityConfig) -> SecurityResult<()>;

    /// §9.4.2.9.28 `return_datawriter_security_config`.
    ///
    /// # Errors
    /// The config is none that the plugin issued.
    fn return_datawriter_security_config(
        &mut self,
        config: EndpointSecurityConfig,
    ) -> SecurityResult<()>;

    /// §9.4.2.9.29 `return_datareader_security_config`.
    ///
    /// # Errors
    /// The config is none that the plugin issued.
    fn return_datareader_security_config(
        &mut self,
        config: EndpointSecurityConfig,
    ) -> SecurityResult<()>;

    /// Not an operation of Table 39: the `<relay>` action of
    /// §10.4.1.5.3.4.1.2 that the relay service of §7.1.1.4 needs ("Bob,
    /// Mallory and others need to accept Trent as someone who is able to
    /// write on Topic T and relay messages from Alice"). May the participant
    /// behind `permissions` (the local one, to create a relay reader, or a
    /// remote one, to accept its writer as forwarder) relay the topic of
    /// `request` in the role `role`? The default is `Deny`: a plugin without
    /// a notion of relaying allows none.
    ///
    /// # Errors
    /// An unknown handle, or any failure of the check.
    fn check_relay(
        &self,
        _permissions: PermissionsHandle,
        _role: RelayRole,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Deny)
    }

    /// The `PluginClassName:MajorVersion.MinorVersion` of the plugin, the
    /// `class_id` of its tokens (e.g. `DDS:Access:Permissions:1.2`). Not an
    /// operation of Table 39: the runtime compares it with the class of a
    /// remote PermissionsToken and the configuration of the plugin names it.
    fn plugin_class_id(&self) -> &str;
}

/// Factory alias.
pub type AccessControlBox = Box<dyn AccessControlPlugin>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_decision_helper() {
        assert!(AccessDecision::Permit.is_permitted());
        assert!(!AccessDecision::Deny.is_permitted());
        assert_eq!(AccessDecision::from(true), AccessDecision::Permit);
        assert_eq!(AccessDecision::from(false), AccessDecision::Deny);
    }

    /// Table 34: the bits of the ParticipantSecurityAttributesMask.
    #[test]
    fn the_participant_mask_has_the_bits_of_table_34() {
        let all = ParticipantSecurityConfig {
            is_rtps_axk_protected: true,
            is_discovery_protected: true,
            is_liveliness_protected: true,
            is_key_revision_enabled: true,
            is_rtps_psk_protected: true,
            ..ParticipantSecurityConfig::default()
        };
        assert_eq!(all.attributes_mask(), 0x8000_001f);
        let none = ParticipantSecurityConfig::default();
        assert_eq!(none.attributes_mask(), 0x8000_0000);
        for (config, bit) in [
            (
                ParticipantSecurityConfig {
                    is_rtps_axk_protected: true,
                    ..ParticipantSecurityConfig::default()
                },
                0x1,
            ),
            (
                ParticipantSecurityConfig {
                    is_discovery_protected: true,
                    ..ParticipantSecurityConfig::default()
                },
                0x2,
            ),
            (
                ParticipantSecurityConfig {
                    is_liveliness_protected: true,
                    ..ParticipantSecurityConfig::default()
                },
                0x4,
            ),
            (
                ParticipantSecurityConfig {
                    is_key_revision_enabled: true,
                    ..ParticipantSecurityConfig::default()
                },
                0x8,
            ),
            (
                ParticipantSecurityConfig {
                    is_rtps_psk_protected: true,
                    ..ParticipantSecurityConfig::default()
                },
                0x10,
            ),
        ] {
            assert_eq!(config.attributes_mask(), 0x8000_0000 | bit);
        }
    }

    /// Table 35: the optional attributes mask has both settings, in is_set
    /// and value.
    #[test]
    fn the_optional_mask_has_the_bits_of_table_35() {
        let config = ParticipantSecurityConfig {
            allow_unauthenticated_participants: true,
            is_access_protected: false,
            ..ParticipantSecurityConfig::default()
        };
        assert_eq!(config.optional_attributes_mask(), (0x3, 0x1));
        let config = ParticipantSecurityConfig {
            allow_unauthenticated_participants: false,
            is_access_protected: true,
            ..ParticipantSecurityConfig::default()
        };
        assert_eq!(config.optional_attributes_mask(), (0x3, 0x2));
    }

    /// Table 38: the bits of the EndpointSecurityAttributesMask.
    #[test]
    fn the_endpoint_mask_has_the_bits_of_table_38() {
        let one = |f: fn(&mut EndpointSecurityConfig)| {
            let mut c = EndpointSecurityConfig::default();
            f(&mut c);
            c.attributes_mask()
        };
        assert_eq!(one(|c| c.topic.is_read_protected = true), 0x8000_0001);
        assert_eq!(one(|c| c.topic.is_write_protected = true), 0x8000_0002);
        assert_eq!(one(|c| c.topic.is_discovery_protected = true), 0x8000_0004);
        assert_eq!(one(|c| c.is_submessage_protected = true), 0x8000_0008);
        assert_eq!(one(|c| c.is_payload_protected = true), 0x8000_0010);
        assert_eq!(one(|c| c.is_key_protected = true), 0x8000_0020);
        assert_eq!(one(|c| c.topic.is_liveliness_protected = true), 0x8000_0040);
        assert_eq!(
            EndpointSecurityConfig::default().attributes_mask(),
            0x8000_0000
        );
    }

    /// Table 67 and Table 69: the bits of the plugin masks.
    #[test]
    fn the_plugin_masks_have_the_bits_of_tables_67_and_69() {
        use plugin_endpoint_attributes_mask as e;
        use plugin_participant_attributes_mask as p;
        assert_eq!(
            [
                p::IS_RTPS_AXK_ENCRYPTED,
                p::IS_DISCOVERY_ENCRYPTED,
                p::IS_LIVELINESS_ENCRYPTED,
                p::IS_RTPS_ORIGIN_AUTHENTICATED,
                p::IS_DISCOVERY_ORIGIN_AUTHENTICATED,
                p::IS_LIVELINESS_ORIGIN_AUTHENTICATED,
                p::IS_RTPS_PSK_ENCRYPTED,
            ],
            [0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40]
        );
        assert_eq!(
            [
                e::IS_SUBMESSAGE_ENCRYPTED,
                e::IS_PAYLOAD_ENCRYPTED,
                e::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED
            ],
            [0x1, 0x2, 0x4]
        );
    }

    #[test]
    fn the_nil_handle_is_zero() {
        assert!(PermissionsHandle::NIL.is_nil());
        assert!(!PermissionsHandle(1).is_nil());
    }
}
