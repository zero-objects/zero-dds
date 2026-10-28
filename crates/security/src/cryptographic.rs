// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Cryptographic plugin interfaces of DDS-Security 1.2 §9.5.1, with the
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin interfaces are used as trait objects.)
//! operations and the parameters of Tables 42, 43 and 44 and nothing else:
//!
//! * [`CryptoKeyFactory`] (§9.5.1.8, Table 42),
//! * [`CryptoKeyExchange`] (§9.5.1.9, Table 43),
//! * [`CryptoTransform`] (§9.5.1.10, Table 44).
//!
//! The handles are the three types of §9.5.1.2 to §9.5.1.4
//! ([`ParticipantCryptoHandle`], [`DatawriterCryptoHandle`],
//! [`DatareaderCryptoHandle`]); `0` is `HandleNIL`. The categories of
//! Table 41 are [`SecureSubmessageCategory`].
//!
//! # Mapping of the IDL results
//!
//! The operations of the tables return a `Boolean` or a handle and fill an
//! `out: exception`. Here:
//!
//! * `Err(`[`SecurityError`](crate::SecurityError)`)` is the return of FALSE (or `HandleNIL`) with
//!   the `SecurityException` filled;
//! * `Ok(true)` is TRUE;
//! * `Ok(false)` is FALSE without an exception: "This operation shall do
//!   nothing and return FALSE" (§10.9.3, Tables 87 to 89) and "This operation
//!   may optionally not perform any transformation ... the operation shall
//!   return false but not set the exception object" (§9.5.1.10.4);
//! * an operation with an `out` parameter next to the Boolean returns it in
//!   `Ok(Some(..))`; `Ok(None)` is FALSE without an exception.
//!
//! The operations of the plugin that the middleware of this workspace calls
//! on the bundled [`crate::crypto::CryptographicPlugin`] (the key tokens as
//! one blob, the Kx-key operations, the key-id lookups) are not part of these
//! three interfaces.

extern crate alloc;

use alloc::vec::Vec;

use crate::access_control::PermissionsHandle;
use crate::algorithm::{CryptoAlgorithmSet, CryptoTransformKeyRevisionIntHolder};
use crate::authentication::{IdentityHandle, SharedSecretHandle};
use crate::error::SecurityResult;
use crate::properties::Property;
use crate::token::CryptoToken;

/// The `GUID_t` of the operations that take an `endpoint_guid`: the 12 octets
/// of the GUID prefix and the 4 octets of the entity id.
pub type GuidT = [u8; 16];

/// A `ParticipantCryptoHandle` (§9.5.1.2): an opaque local reference to the
/// key material that encrypts and signs whole RTPS messages. `0` is
/// `HandleNIL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ParticipantCryptoHandle(pub u64);

/// A `DatawriterCryptoHandle` (§9.5.1.3). `0` is `HandleNIL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DatawriterCryptoHandle(pub u64);

/// A `DatareaderCryptoHandle` (§9.5.1.4). `0` is `HandleNIL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DatareaderCryptoHandle(pub u64);

impl ParticipantCryptoHandle {
    /// `HandleNIL`.
    pub const NIL: Self = Self(0);
    /// Whether this is `HandleNIL`.
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0 == 0
    }
}

impl DatawriterCryptoHandle {
    /// `HandleNIL`.
    pub const NIL: Self = Self(0);
    /// Whether this is `HandleNIL`.
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0 == 0
    }
}

impl DatareaderCryptoHandle {
    /// `HandleNIL`.
    pub const NIL: Self = Self(0);
    /// Whether this is `HandleNIL`.
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0 == 0
    }
}

/// `SecureSubmessageCategory_t` (§9.5.1.7, Table 41): the categories of
/// RTPS submessages that `preprocess_secure_submsg` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecureSubmessageCategory {
    /// `INFO_SUBMESSAGE`: an RTPS Info submessage (InfoSource,
    /// InfoDestination or InfoTimestamp).
    InfoSubmessage,
    /// `DATAWRITER_SUBMESSAGE` (the IDL spells it `DATAWRITER_SUMBESSAGE`): a
    /// submessage sent from a DataWriter (Data, DataFrag, HeartBeat, Gap).
    DatawriterSubmessage,
    /// `DATAREADER_SUBMESSAGE` (the IDL spells it `DATAREADER_SUMBESSAGE`): a
    /// submessage sent from a DataReader (AckNack, NackFrag).
    DatareaderSubmessage,
}

/// `CryptoAlgorithmRequirements` (§7.3.10): the algorithms a participant or
/// endpoint supports and the ones it requires of its peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CryptoAlgorithmRequirements {
    /// `supported_mask`.
    pub supported_mask: CryptoAlgorithmSet,
    /// `required_mask`.
    pub required_mask: CryptoAlgorithmSet,
}

/// `ParticipantSecurityDigitalSignatureAlgorithmInfo` (§7.3.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityDigitalSignatureAlgorithmInfo {
    /// `trust_chain`.
    pub trust_chain: CryptoAlgorithmRequirements,
    /// `message_auth`.
    pub message_auth: CryptoAlgorithmRequirements,
}

/// `ParticipantSecurityKeyEstablishmentAlgorithmInfo` (§7.3.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityKeyEstablishmentAlgorithmInfo {
    /// `shared_secret`.
    pub shared_secret: CryptoAlgorithmRequirements,
}

/// `ParticipantSecuritySymmetricCipherAlgorithmInfo` (§7.3.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecuritySymmetricCipherAlgorithmInfo {
    /// `supported_mask`.
    pub supported_mask: CryptoAlgorithmSet,
    /// `builtin_endpoints_required_mask`.
    pub builtin_endpoints_required_mask: CryptoAlgorithmSet,
    /// `builtin_kx_endpoints_required_mask`.
    pub builtin_kx_endpoints_required_mask: CryptoAlgorithmSet,
    /// `user_endpoints_default_required_mask`.
    pub user_endpoints_default_required_mask: CryptoAlgorithmSet,
}

/// `ParticipantSecurityAlgorithmInfo` (§7.3.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticipantSecurityAlgorithmInfo {
    /// `digital_signature`.
    pub digital_signature: ParticipantSecurityDigitalSignatureAlgorithmInfo,
    /// `key_establishment`.
    pub key_establishment: ParticipantSecurityKeyEstablishmentAlgorithmInfo,
    /// `symmetric_cipher`.
    pub symmetric_cipher: ParticipantSecuritySymmetricCipherAlgorithmInfo,
}

/// `EndpointSecuritySymmetricCipherAlgorithmInfo` (§7.3.15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EndpointSecuritySymmetricCipherAlgorithmInfo {
    /// `required_mask`.
    pub required_mask: CryptoAlgorithmSet,
    /// `supported_mask` (`@non_serialized`).
    pub supported_mask: CryptoAlgorithmSet,
}

/// `EndpointSecurityAlgorithmInfo` (§7.3.16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EndpointSecurityAlgorithmInfo {
    /// `symmetric_cipher`.
    pub symmetric_cipher: EndpointSecuritySymmetricCipherAlgorithmInfo,
}

/// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_AXK_ENCRYPTED`
/// (§10.4.2.4, Table 67).
pub const PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED: u32 = 1 << 0;
/// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_BUILTIN_IS_DISCOVERY_ENCRYPTED`.
pub const PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ENCRYPTED: u32 = 1 << 1;
/// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_LIVELINESS_ENCRYPTED`.
pub const PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ENCRYPTED: u32 = 1 << 2;
/// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED`.
pub const PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED: u32 = 1 << 3;
/// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_DISCOVERY_ORIGIN_AUTHENTICATED`.
pub const PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ORIGIN_AUTHENTICATED: u32 = 1 << 4;
/// `PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_LIVELINESS_ORIGIN_AUTHENTICATED`.
pub const PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ORIGIN_AUTHENTICATED: u32 = 1 << 5;
/// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_ENCRYPTED`.
pub const PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED: u32 = 1 << 6;

/// `PLUGIN_ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_SUBMESSAGE_ENCRYPTED`
/// (§10.4.2.6, Table 69).
pub const PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED: u32 = 1 << 0;
/// `PLUGIN_ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_PAYLOAD_ENCRYPTED`.
pub const PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED: u32 = 1 << 1;
/// `PLUGIN_ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED`.
pub const PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED: u32 = 1 << 2;

/// `ParticipantSecurityConfig` (§9.4.2.4, Table 33), the value the
/// AccessControl operation `get_participant_security_config` returns and
/// `register_local_participant` takes as `participant_security_config`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticipantSecurityConfig {
    /// `allow_unauthenticated_participants`.
    pub allow_unauthenticated_participants: bool,
    /// `is_access_protected`.
    pub is_access_protected: bool,
    /// `is_rtps_axk_protected`.
    pub is_rtps_axk_protected: bool,
    /// `is_rtps_psk_protected`.
    pub is_rtps_psk_protected: bool,
    /// `is_discovery_protected`.
    pub is_discovery_protected: bool,
    /// `is_liveliness_protected`.
    pub is_liveliness_protected: bool,
    /// `is_key_revision_enabled`.
    pub is_key_revision_enabled: bool,
    /// `plugin_participant_attributes`: the `PLUGIN_PARTICIPANT_FLAG_*` bits.
    pub plugin_participant_attributes: u32,
    /// `ac_endpoint_properties`.
    pub ac_endpoint_properties: Vec<Property>,
    /// `algorithm_info`.
    pub algorithm_info: ParticipantSecurityAlgorithmInfo,
}

/// `EndpointSecurityConfig` (§9.4.2.7, Table 37, with the members of
/// `TopicSecurityConfig`, Table 36), the value of
/// `get_datawriter_security_config` and `get_datareader_security_config`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndpointSecurityConfig {
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
    /// `plugin_endpoint_attributes`: the `PLUGIN_ENDPOINT_FLAG_*` bits.
    pub plugin_endpoint_attributes: u32,
    /// `ac_endpoint_properties`.
    pub ac_endpoint_properties: Vec<Property>,
    /// `algorithm_info`.
    pub algorithm_info: EndpointSecurityAlgorithmInfo,
}

/// The output of `encode_serialized_payload` (§9.5.1.10.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPayload {
    /// `encoded_buffer`: the `CryptoContent` that replaces the
    /// `SerializedPayload`.
    pub encoded_buffer: Vec<u8>,
    /// `extra_inline_qos`: a `ParameterList` to add to the `inlineQos` of the
    /// Data or DataFrag submessage; empty when there is nothing to add.
    pub extra_inline_qos: Vec<u8>,
}

/// The output of `preprocess_secure_submsg` (§9.5.1.10.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreprocessedSubmessage {
    /// `datawriter_crypto`: the handle of the DataWriter of the pair the
    /// submessage belongs to; `HandleNIL` for a category without one.
    pub datawriter_crypto: DatawriterCryptoHandle,
    /// `datareader_crypto`: the handle of the DataReader of the pair.
    pub datareader_crypto: DatareaderCryptoHandle,
    /// `secure_submessage_category`.
    pub secure_submessage_category: SecureSubmessageCategory,
}

/// `CryptoKeyFactory` (§9.5.1.8, Table 42): creates the key material.
pub trait CryptoKeyFactory {
    /// `register_local_participant` (§9.5.1.8.1): creates the key material to
    /// encrypt and sign the messages to other participants and returns the
    /// handle with the `adjusted_algorithm_info`.
    ///
    /// `participant_properties` are the `dds.sec.crypto.` properties of the
    /// participant. A nil `participant_identity` or `participant_permissions`
    /// is `HandleNIL` with an exception.
    ///
    /// # Errors
    /// The `SecurityException`: a nil identity or permissions, an unknown
    /// value of a property, or a plugin failure.
    fn register_local_participant(
        &mut self,
        participant_identity: IdentityHandle,
        participant_permissions: PermissionsHandle,
        participant_properties: &[Property],
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<(ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo)>;

    /// `register_matched_remote_participant` (§9.5.1.8.2).
    ///
    /// # Errors
    /// The `SecurityException`: an unknown local handle or a nil identity.
    fn register_matched_remote_participant(
        &mut self,
        local_participant_crypto_handle: ParticipantCryptoHandle,
        remote_participant_identity: IdentityHandle,
        remote_participant_permissions: PermissionsHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle>;

    /// `register_local_datawriter` (§9.5.1.8.3).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn register_local_datawriter(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        datawriter_properties: &[Property],
        datawriter_security_config: &EndpointSecurityConfig,
        endpoint_guid: GuidT,
    ) -> SecurityResult<(DatawriterCryptoHandle, EndpointSecurityAlgorithmInfo)>;

    /// `register_matched_remote_datareader` (§9.5.1.8.4). `relay_only`
    /// is "associated with the returned DatareaderCryptoHandle" and decides
    /// which key material the writer's tokens carry.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn register_matched_remote_datareader(
        &mut self,
        local_datawriter_crypto_handle: DatawriterCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        shared_secret: SharedSecretHandle,
        relay_only: bool,
    ) -> SecurityResult<DatareaderCryptoHandle>;

    /// `register_local_datareader` (§9.5.1.8.5).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn register_local_datareader(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        datareader_properties: &[Property],
        datareader_security_config: &EndpointSecurityConfig,
        endpoint_guid: GuidT,
    ) -> SecurityResult<(DatareaderCryptoHandle, EndpointSecurityAlgorithmInfo)>;

    /// `register_matched_remote_datawriter` (§9.5.1.8.6).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn register_matched_remote_datawriter(
        &mut self,
        local_datareader_crypto_handle: DatareaderCryptoHandle,
        remote_participant_crypt: ParticipantCryptoHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<DatawriterCryptoHandle>;

    /// `revise_local_entity_keys` (§9.5.1.8.7): creates a revision of the key
    /// material of the participant and of its DataWriters and DataReaders and
    /// returns its number. A plugin without revisions returns `0`.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn revise_local_entity_keys(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<CryptoTransformKeyRevisionIntHolder>;

    /// `activate_key_revision` (§9.5.1.8.8): from now on the encode
    /// operations use the key material of `key_revision`.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn activate_key_revision(
        &mut self,
        local_participant_crypto_handle: ParticipantCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<bool>;

    /// `unregister_participant` (§9.5.1.8.9).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn unregister_participant(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<bool>;

    /// `unregister_datawriter` (§9.5.1.8.10).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn unregister_datawriter(
        &mut self,
        datawriter_crypto_handle: DatawriterCryptoHandle,
    ) -> SecurityResult<bool>;

    /// `unregister_datareader` (§9.5.1.8.11).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn unregister_datareader(
        &mut self,
        datareader_crypto_handle: DatareaderCryptoHandle,
    ) -> SecurityResult<bool>;
}

/// `CryptoKeyExchange` (§9.5.1.9, Table 43): the key tokens.
pub trait CryptoKeyExchange {
    /// `create_local_participant_crypto_tokens` (§9.5.1.9.1). Returns
    /// `local_participant_crypto_tokens`; `Ok(None)` is FALSE without an
    /// exception (a plugin whose keys are not exchanged, Table 88).
    /// `key_revision` has to be one a prior `revise_local_entity_keys`
    /// returned (or the active one).
    ///
    /// # Errors
    /// The `SecurityException`: an unknown handle or revision.
    fn create_local_participant_crypto_tokens(
        &mut self,
        local_participant_crypto: ParticipantCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>>;

    /// `set_remote_participant_crypto_tokens` (§9.5.1.9.2).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn set_remote_participant_crypto_tokens(
        &mut self,
        local_participant_crypto: ParticipantCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        remote_participant_tokens: &[CryptoToken],
    ) -> SecurityResult<bool>;

    /// `create_local_datawriter_crypto_tokens` (§9.5.1.9.3).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn create_local_datawriter_crypto_tokens(
        &mut self,
        local_datawriter_crypto: DatawriterCryptoHandle,
        remote_datareader_crypto: DatareaderCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>>;

    /// `set_remote_datawriter_crypto_tokens` (§9.5.1.9.4).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn set_remote_datawriter_crypto_tokens(
        &mut self,
        local_datareader_crypto: DatareaderCryptoHandle,
        remote_datawriter_crypto: DatawriterCryptoHandle,
        remote_datawriter_tokens: &[CryptoToken],
    ) -> SecurityResult<bool>;

    /// `create_local_datareader_crypto_tokens` (§9.5.1.9.5).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn create_local_datareader_crypto_tokens(
        &mut self,
        local_datareader_crypto: DatareaderCryptoHandle,
        remote_datawriter_crypto: DatawriterCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>>;

    /// `set_remote_datareader_crypto_tokens` (§9.5.1.9.6).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn set_remote_datareader_crypto_tokens(
        &mut self,
        local_datawriter_crypto: DatawriterCryptoHandle,
        remote_datareader_crypto: DatareaderCryptoHandle,
        remote_datareader_tokens: &[CryptoToken],
    ) -> SecurityResult<bool>;

    /// `return_crypto_tokens` (§9.5.1.9.7): returns tokens the plugin issued
    /// so that it releases what it holds for them.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn return_crypto_tokens(&mut self, crypto_tokens: Vec<CryptoToken>) -> SecurityResult<bool>;
}

/// `CryptoTransform` (§9.5.1.10, Table 44): encrypts, decrypts, signs and
/// verifies.
pub trait CryptoTransform {
    /// `encode_serialized_payload` (§9.5.1.10.1).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn encode_serialized_payload(
        &self,
        plain_buffer: &[u8],
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<EncodedPayload>>;

    /// `encode_datawriter_submessage` (§9.5.1.10.2). With
    /// `receiving_datareader_crypto_list_index` 0, `encoded_rtps_submessage`
    /// is filled from `plain_rtps_submessage`; with a non-zero index
    /// `plain_rtps_submessage` is empty and `encoded_rtps_submessage` the one
    /// an earlier call returned, whose receiver-specific parts the call
    /// replaces. The index is set to the next one; the length of the list
    /// ends the iteration.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn encode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &mut Vec<u8>,
        plain_rtps_submessage: &[u8],
        sending_datawriter_crypto: DatawriterCryptoHandle,
        receiving_datareader_crypto_list: &[DatareaderCryptoHandle],
        receiving_datareader_crypto_list_index: &mut usize,
    ) -> SecurityResult<bool>;

    /// `encode_datareader_submessage` (§9.5.1.10.3).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn encode_datareader_submessage(
        &self,
        plain_rtps_submessage: &[u8],
        sending_datareader_crypto: DatareaderCryptoHandle,
        receiving_datawriter_crypto_list: &[DatawriterCryptoHandle],
    ) -> SecurityResult<Option<Vec<u8>>>;

    /// `encode_rtps_message` (§9.5.1.10.4). The index works as in
    /// [`Self::encode_datawriter_submessage`]. `Ok(false)` leaves the message
    /// as it is (no transformation, no exception).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn encode_rtps_message(
        &self,
        encoded_rtps_message: &mut Vec<u8>,
        plain_rtps_message: &[u8],
        sending_participant_crypto: ParticipantCryptoHandle,
        receiving_participant_crypto_list: &[ParticipantCryptoHandle],
        receiving_participant_crypto_list_index: &mut usize,
        transform_with_psk: bool,
    ) -> SecurityResult<bool>;

    /// `decode_rtps_message` (§9.5.1.10.5).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_rtps_message(
        &self,
        encoded_buffer: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>>;

    /// `preprocess_secure_submsg` (§9.5.1.10.6).
    ///
    /// # Errors
    /// The `SecurityException`: a transformation kind the plugin does not
    /// know, or a key no handle of the sender is associated with.
    fn preprocess_secure_submsg(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<PreprocessedSubmessage>>;

    /// `decode_datawriter_submessage` (§9.5.1.10.7).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_datareader_crypto: DatareaderCryptoHandle,
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>>;

    /// `decode_datareader_submessage` (§9.5.1.10.8).
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_datareader_submessage(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_datawriter_crypto: DatawriterCryptoHandle,
        sending_datareader_crypto: DatareaderCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>>;

    /// `decode_serialized_payload` (§9.5.1.10.9). `inline_qos` is the
    /// `InlineQos` of the Data submessage that carried the payload.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_serialized_payload(
        &self,
        encoded_buffer: &[u8],
        inline_qos: &[u8],
        receiving_datareader_crypto: DatareaderCryptoHandle,
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>>;
}

/// Operations a vendor's way of protecting needs and the specification does
/// not describe. They are not part of Tables 42 to 44; a plugin offers them
/// through [`CryptographicPluginInterfaces::vendor_forms`], and the middleware
/// uses them only where a named setting asks for the vendor form
/// (`org.zerodds.sec.kx_key_fallback=accept`) or a sender protects with the key
/// of its participant.
pub trait VendorFormCrypto {
    /// Protects a submessage of an endpoint that has no key material of its
    /// own with the key of the local participant `local`.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn encode_submessage_with_participant_key(
        &self,
        local: ParticipantCryptoHandle,
        plain_rtps_submessage: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    /// Opens a submessage with the key of the remote participant `remote`
    /// (the one of its `ParticipantCryptoToken`) instead of the key of the
    /// endpoint that sent it.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_submessage_with_participant_key(
        &self,
        remote: ParticipantCryptoHandle,
        encoded_rtps_submessage: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    /// Opens an encrypted serialized payload with the key of the remote
    /// participant `remote`.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_serialized_payload_with_participant_key(
        &self,
        remote: ParticipantCryptoHandle,
        encoded_buffer: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    /// Opens an encrypted serialized payload with the Kx key of the remote
    /// participant `remote` (§10.5.2.1.2), which a vendor may protect it with.
    ///
    /// # Errors
    /// The `SecurityException`.
    fn decode_serialized_payload_with_kx_key(
        &self,
        remote: ParticipantCryptoHandle,
        encoded_buffer: &[u8],
    ) -> SecurityResult<Vec<u8>>;
}

/// A Cryptographic plugin: the three interfaces of §9.5.1 in one object. The
/// middleware (`SharedSecurityGate`) calls nothing else.
pub trait CryptographicPluginInterfaces:
    CryptoKeyFactory + CryptoKeyExchange + CryptoTransform + Send + Sync
{
    /// The vendor forms the plugin offers, `None` for none.
    fn vendor_forms(&self) -> Option<&dyn VendorFormCrypto> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_nil_is_zero_for_the_three_handle_types() {
        assert!(ParticipantCryptoHandle::NIL.is_nil());
        assert!(DatawriterCryptoHandle::NIL.is_nil());
        assert!(DatareaderCryptoHandle::NIL.is_nil());
        assert!(!ParticipantCryptoHandle(1).is_nil());
        assert!(!DatawriterCryptoHandle(1).is_nil());
        assert!(!DatareaderCryptoHandle(1).is_nil());
    }

    #[test]
    fn the_plugin_attribute_bits_are_those_of_tables_67_and_69() {
        assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED, 0x01);
        assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ENCRYPTED, 0x02);
        assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ENCRYPTED, 0x04);
        assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED, 0x08);
        assert_eq!(
            PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ORIGIN_AUTHENTICATED,
            0x10
        );
        assert_eq!(
            PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ORIGIN_AUTHENTICATED,
            0x20
        );
        assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED, 0x40);
        assert_eq!(PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED, 0x01);
        assert_eq!(PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, 0x02);
        assert_eq!(
            PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
            0x04
        );
    }

    #[test]
    fn the_three_categories_of_table_41_are_distinct() {
        assert_ne!(
            SecureSubmessageCategory::InfoSubmessage,
            SecureSubmessageCategory::DatawriterSubmessage
        );
        assert_ne!(
            SecureSubmessageCategory::DatawriterSubmessage,
            SecureSubmessageCategory::DatareaderSubmessage
        );
    }
}
