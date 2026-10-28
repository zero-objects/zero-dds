// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Authentication plugin SPI (OMG DDS-Security 1.2 §9.3).
//!
//! Responsible for:
//! 1. **Identity validation** at participant start — validate the local
//!    identity (e.g. X.509 cert + private key) against the trust anchor.
//! 2. **Identity handshake** between two participants — challenge/
//!    response with signed nonces, spec §10.3.4.
//! 3. **SharedSecret creation** at the end of the handshake — input
//!    for the CryptographicPlugin for key derivation.
//!
//! The SPI is **state-machine-light** — the plugin holds the
//! handshake state itself. The caller (DCPS layer) drives the state machine
//! of §9.3.2.9 (Figure 9) with the [`ValidationResult`] of each operation:
//! `validate_remote_identity`, then `begin_handshake_request` or
//! `begin_handshake_reply`, then `process_handshake` until the handshake is
//! complete, then `get_shared_secret` and
//! `get_authenticated_peer_credential_token`.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn AuthenticationPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::error::{SecurityError, SecurityErrorKind, SecurityResult};
use crate::properties::PropertyList;

/// The digital signature and key establishment algorithms an authentication
/// plugin can verify and uses, as `CryptoAlgorithmBit` masks (DDS-Security 1.2
/// §8.2 Table 25, §8.3 Table 26): each pair is `(supported, required)` as in
/// `CryptoAlgorithmRequirements` (§7.3.10). They become the
/// `ParticipantSecurityAlgorithmInfo` of the participant (§7.3.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlgorithmMasks {
    /// `trust_chain`: the algorithms of signed documents the plugin
    /// validates, and those in the documents it sends (§7.3.11).
    pub trust_chain: (u32, u32),
    /// `message_auth`: the signatures the plugin verifies in the handshake,
    /// and the one it signs with (§7.3.11).
    pub message_auth: (u32, u32),
    /// `shared_secret`: the key agreements the plugin runs, and the one it
    /// uses (§7.3.12).
    pub key_establishment: (u32, u32),
}

/// Opaque handle for a validated identity. The plugin-internal
/// state (X.509 cert, keys) is not exposed outward through this
/// handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityHandle(pub u64);

impl IdentityHandle {
    /// The nil handle (the `NILHandle` of the IDL): no identity.
    pub const NIL: Self = Self(0);
}

/// Opaque handle for a running handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandshakeHandle(pub u64);

impl HandshakeHandle {
    /// The nil handle (the `NILHandle` of the IDL): no handshake.
    pub const NIL: Self = Self(0);
}

/// Opaque handle for a shared secret (output of a completed
/// handshake). Passed on to the `CryptographicPlugin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedSecretHandle(pub u64);

impl SharedSecretHandle {
    /// The nil handle (the `NILHandle` of the IDL): no shared secret.
    pub const NIL: Self = Self(0);
}

/// Lookup bridge between [`AuthenticationPlugin`] and
/// [`crate::crypto::CryptographicPlugin`].
///
/// After the handshake the authentication plugin produces a
/// [`SharedSecretHandle`] and knows the associated 32 bytes from
/// `x25519 + HKDF-SHA256`. The crypto plugin needs exactly these
/// bytes to derive its per-peer master key — instead of generating a
/// random key and routing it as an opaque token through the
/// governance.
///
/// Implementors must be **Send + Sync** so that the crypto
/// plugin can hold the provider via `Arc<dyn SharedSecretProvider>`.
pub trait SharedSecretProvider: Send + Sync {
    /// Returns the raw bytes of the shared key.
    /// `None` if the handle is unknown (handshake not yet
    /// completed or already discarded).
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<alloc::vec::Vec<u8>>;

    /// Returns `(challenge1, challenge2)` of the associated handshake
    /// (challenge1 = initiator, challenge2 = replier). These feed into the
    /// VolatileSecure key derivation (DDS-Security §10.5.2.1 Table 74, `calculate_kx_keys`)
    /// — both peers derive the same Kx key from `(SharedSecret, challenge1,
    /// challenge2)`. Default `None` for providers without challenge tracking
    /// (e.g. PSK/mock); their Kx path then uses the fallback.
    fn get_shared_secret_challenges(
        &self,
        _handle: SharedSecretHandle,
    ) -> Option<([u8; 32], [u8; 32])> {
        None
    }
}

/// `ValidationResult_t` (DDS-Security 1.2 §9.3.2.11.1 Table 30): the result of
/// the operations `validate_local_identity`, `validate_remote_identity`,
/// `begin_handshake_request`, `begin_handshake_reply` and `process_handshake`.
///
/// In the Rust API an operation that fails returns `Err(SecurityError)`, which
/// is `VALIDATION_FAILED` with the filled `SecurityException` ("If an error
/// occurs, this method shall return VALIDATION_FAILED and fill the
/// SecurityException", §9.3.2.11.2 to §9.3.2.11.6); [`Self::Failed`] names
/// that outcome for code that reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ValidationResult {
    /// `VALIDATION_OK`: the validation has succeeded.
    Ok,
    /// `VALIDATION_FAILED`: the validation has failed.
    Failed,
    /// `VALIDATION_PENDING_RETRY`: validation is still proceeding; the
    /// operation shall be retried at a later point in time.
    PendingRetry,
    /// `VALIDATION_PENDING_HANDSHAKE_REQUEST`: validation of the submitted
    /// IdentityToken requires sending a handshake message; the DDS
    /// implementation shall call `begin_handshake_request`.
    PendingHandshakeRequest,
    /// `VALIDATION_PENDING_HANDSHAKE_MESSAGE`: validation is still pending;
    /// the DDS implementation shall wait for a message of the peer and then
    /// call `process_handshake` (or `begin_handshake_reply`).
    PendingHandshakeMessage,
    /// `VALIDATION_OK_FINAL_MESSAGE`: validation has succeeded but the DDS
    /// implementation shall send a final message.
    OkFinalMessage,
}

impl ValidationResult {
    /// Whether the validation has succeeded (`VALIDATION_OK` or
    /// `VALIDATION_OK_FINAL_MESSAGE`): after the last handshake operation
    /// returned one of these, the handshake is complete and
    /// `get_shared_secret` and `get_authenticated_peer_credential_token` may
    /// be called on its handle (§9.3.2.11.7, §9.3.2.11.8).
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Ok | Self::OkFinalMessage)
    }
}

/// What `validate_local_identity` returns besides the result: the
/// `local_identity_handle` and the `adjusted_participant_guid` out parameters
/// (§9.3.2.11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalIdentityValidation {
    /// `VALIDATION_OK`, or `VALIDATION_PENDING_RETRY` when the verification
    /// has not finished and the operation "shall be called again after a
    /// configurable delay".
    pub result: ValidationResult,
    /// `local_identity_handle`: "will only be meaningful if the operation
    /// returns VALIDATION_OK"; [`IdentityHandle::NIL`] while pending.
    pub identity: IdentityHandle,
    /// `adjusted_participant_guid`: "the GUID_t that the DDS implementation
    /// shall use to uniquely identify the DomainParticipant on the network";
    /// the one that "eventually appears in the participant_guid attribute of
    /// the ParticipantBuiltinTopicData sent via discovery".
    pub adjusted_participant_guid: [u8; 16],
}

/// What `validate_remote_identity` returns besides the result: the
/// `remote_identity_handle` and `local_auth_request_token` out parameters
/// (§9.3.2.11.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteIdentityValidation {
    /// `VALIDATION_OK`, `VALIDATION_PENDING_HANDSHAKE_REQUEST`,
    /// `VALIDATION_PENDING_HANDSHAKE_MESSAGE` or `VALIDATION_PENDING_RETRY`.
    pub result: ValidationResult,
    /// `remote_identity_handle`: "will only be provided if the operation
    /// returns something other than VALIDATION_FAILED"; the nil handle
    /// while the validation is pending a retry.
    pub identity: IdentityHandle,
    /// `local_auth_request_token`: the AuthRequestMessageToken (a CDR
    /// DataHolder) "to be sent using the BuiltinParticipantStatelessMessageWriter";
    /// `None` is TokenNIL: "the AuthRequestMessageToken shall not be sent".
    pub local_auth_request_token: Option<Vec<u8>>,
}

/// Result of a handshake step: the `ValidationResult_t` of
/// `begin_handshake_request`, `begin_handshake_reply` or `process_handshake`
/// together with its `handshake_message_out` (§9.3.2.11.4 to §9.3.2.11.6);
/// see [`Self::validation_result`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HandshakeStepOutcome {
    /// `VALIDATION_PENDING_HANDSHAKE_MESSAGE` with a message: the plugin
    /// wants to send another message to the peer and then wait for its
    /// reply.
    SendMessage {
        /// Opaque handshake token as a byte blob. Goes 1:1 into the
        /// `AuthHandshake` built-in submessage (spec §10.3.2.5).
        token: Vec<u8>,
    },
    /// `VALIDATION_OK_FINAL_MESSAGE`: the validation has succeeded and the
    /// message is to be sent, without waiting for any reply. The
    /// `SharedSecretHandle` is available through `get_shared_secret`.
    SendFinalMessage {
        /// The final `handshake_message_out`.
        token: Vec<u8>,
    },
    /// `VALIDATION_OK`: handshake completed successfully — the
    /// `SharedSecretHandle` is usable.
    Complete {
        /// Shared key handle for the CryptographicPlugin.
        secret: SharedSecretHandle,
    },
    /// `VALIDATION_PENDING_HANDSHAKE_MESSAGE` without a message to send:
    /// the handshake still needs a message from the peer.
    WaitingForPeer,
    /// `VALIDATION_PENDING_RETRY`: the validation has not completed; the DDS
    /// implementation "shall call the operation again at a later point in
    /// time to check the validation status".
    PendingRetry,
}

impl HandshakeStepOutcome {
    /// The `ValidationResult_t` of this outcome (Table 30).
    #[must_use]
    pub const fn validation_result(&self) -> ValidationResult {
        match self {
            Self::SendMessage { .. } | Self::WaitingForPeer => {
                ValidationResult::PendingHandshakeMessage
            }
            Self::SendFinalMessage { .. } => ValidationResult::OkFinalMessage,
            Self::Complete { .. } => ValidationResult::Ok,
            Self::PendingRetry => ValidationResult::PendingRetry,
        }
    }

    /// The `handshake_message_out` of this outcome, if the DDS implementation
    /// is to send one.
    #[must_use]
    pub fn message_out(&self) -> Option<&[u8]> {
        match self {
            Self::SendMessage { token } | Self::SendFinalMessage { token } => Some(token),
            _ => None,
        }
    }
}

/// `AuthStatusKind` (DDS-Security 1.2 §9.3.2.12.1 Table 32): the kind of
/// change `AuthenticationListener::on_status_changed` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AuthStatusKind {
    /// `IDENTITY_STATUS`: "a change to an identity status"; the changed
    /// IdentityStatusToken, from `get_identity_status_token`, "shall be
    /// propagated by the DDS implementation to the other DomainParticipants
    /// using the DCPSParticipantsSecure builtin Topic".
    IdentityStatus,
}

/// `AuthenticationListener` (DDS-Security 1.2 §9.3.2.12 Table 31): how the
/// plugin notifies the DDS implementation of events relevant to the
/// authentication of DomainParticipants. Installed with
/// [`AuthenticationPlugin::set_listener`]. The `plugin` parameter of the IDL
/// operations is left out: the listener is installed on one plugin and
/// knows it.
pub trait AuthenticationListener: Send + Sync {
    /// `on_revoke_identity` (§9.3.2.12.2): "Revokes the identity of the
    /// participant identified by the IdentityHandle. The corresponding
    /// IdentityHandle becomes invalid. As a result of this, the DDS
    /// middleware shall terminate any communications with the
    /// DomainParticipant associated with that handle", and create a new
    /// revision of the key material it shared with it.
    ///
    /// # Errors
    /// An `Err` is the `false` of the IDL operation ("If an error occurs,
    /// this method shall return false").
    fn on_revoke_identity(&self, handle: IdentityHandle) -> SecurityResult<()>;

    /// `on_status_changed` (§9.3.2.12.3): a status of the plugin, or of an
    /// identity it manages, has changed.
    fn on_status_changed(&self, handle: IdentityHandle, status_kind: AuthStatusKind);
}

/// The part of the `ParticipantSecurityConfig` (DDS-Security 1.2 §9.4.2.4)
/// that the Authentication plugin reads in `set_participant_security_config`
/// (§9.3.2.11.11): its `algorithm_info`. The cryptographic algorithms of the
/// symmetric ciphers belong to the Cryptographic plugin and are not touched
/// here; the other members of the configuration are those of the
/// AccessControl plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantSecurityConfig {
    /// `algorithm_info` (`ParticipantSecurityAlgorithmInfo`, §7.3.14), the
    /// digital signature and key establishment part: the algorithms the
    /// participant supports and requires.
    pub algorithm_info: AlgorithmMasks,
}

impl ParticipantSecurityConfig {
    /// A configuration that supports every algorithm and requires none
    /// (`CRYPTO_ALGORITHM_SET_ALL` supported, `CRYPTO_ALGORITHM_SET_EMPTY`
    /// required, §7.3.9), which leaves the choice to the plugin.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            algorithm_info: AlgorithmMasks {
                trust_chain: (crate::algorithm::CRYPTO_ALGORITHM_SET_ALL, 0),
                message_auth: (crate::algorithm::CRYPTO_ALGORITHM_SET_ALL, 0),
                key_establishment: (crate::algorithm::CRYPTO_ALGORITHM_SET_ALL, 0),
            },
        }
    }
}

/// The tokens a plugin handed out and did not get back, for the `return_*_token`
/// operations (§9.3.2.11.14 to §9.3.2.11.16): a token is "returned to the
/// plugin so it can be disposed of" only if the plugin issued it. A plugin
/// holds one of these and calls [`Self::issue`] for each token it hands out
/// and [`Self::take_back`] in the matching `return_*` operation.
#[derive(Debug, Default, Clone)]
pub struct IssuedTokens {
    outstanding: BTreeMap<(u8, Vec<u8>), usize>,
}

impl IssuedTokens {
    /// Notes that a token of `kind` (a number the plugin chooses per kind of
    /// token) was handed out, and returns it.
    pub fn issue(&mut self, kind: u8, token: Vec<u8>) -> Vec<u8> {
        *self.outstanding.entry((kind, token.clone())).or_default() += 1;
        token
    }

    /// Takes a token of `kind` back.
    ///
    /// # Errors
    /// `BadArgument` for a token that was not issued, or was already taken
    /// back as often as it was issued.
    pub fn take_back(&mut self, kind: u8, token: &[u8]) -> SecurityResult<()> {
        let key = (kind, token.to_vec());
        match self.outstanding.get_mut(&key) {
            Some(count) if *count > 1 => {
                *count -= 1;
                Ok(())
            }
            Some(_) => {
                self.outstanding.remove(&key);
                Ok(())
            }
            None => Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "a token the plugin did not issue, or already took back",
            )),
        }
    }

    /// How many tokens are out.
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.outstanding.values().sum()
    }
}

/// The names a peer reads for the algorithms in the handshake
/// (`c.dsign_algo`, `c.kagree_algo`): the `CryptoAlgorithmName`s of DDS-Security
/// 1.2 §8.2 Table 25 and §8.3 Table 26, or the names of 1.1 that Cyclone, Fast
/// DDS and OpenDDS read (`ECDSA-SHA256`, `ECDH+prime256v1-CEUM`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PeerAlgorithmNames {
    /// The names of 1.2 (Table 25 and 26): the specification's.
    #[default]
    Spec,
    /// The names of 1.1, deprecated in 1.2.
    Legacy,
}

/// Operations of an authentication plugin beyond the Authentication
/// interface of DDS-Security 1.2 §9.3.2.11 Table 29, which
/// [`AuthenticationPlugin`] holds in full and alone. They are supertrait
/// operations so that the DDS implementation, which holds the plugin as
/// `dyn AuthenticationPlugin`, reaches them. Each has a body that does
/// nothing for a plugin to which it does not apply.
pub trait AuthenticationExtensions {
    /// The plugin class name with its version (e.g. "DDS:Auth:PKI-DH:1.2");
    /// the `class_id` family of the plugin's tokens.
    fn plugin_class_id(&self) -> &str;

    /// Interoperability with a peer vendor: whether the next handshake
    /// algorithm strings (`c.dsign_algo`/`c.kagree_algo`) are emitted and
    /// hashed NUL-terminated. OpenDDS compares them with `sizeof` (incl.
    /// `\0`) and needs the NUL form; Fast DDS (#3803) needs them without;
    /// Cyclone is tolerant. The handshake runs per peer, so the discovery
    /// layer calls this with the `VendorId` of the peer BEFORE
    /// `begin_handshake_request` or `begin_handshake_reply`. The default,
    /// NUL-free, is the specification's.
    fn set_algo_nul_terminate(&mut self, _nul: bool) {}

    /// The `message_auth.supported_mask` the remote participant announced
    /// (DDS-Security 1.2 §7.3.11), or the default of the specification
    /// (§7.3.11.2); `None` when unknown. The discovery layer calls this
    /// before `begin_handshake_request` or `begin_handshake_reply`, as
    /// [`Self::set_algo_nul_terminate`], so that the plugin signs with an
    /// algorithm the peer verifies.
    fn set_remote_message_auth_supported(&mut self, _supported: Option<u32>) {}

    /// Interoperability with a peer vendor: the names this peer reads for the
    /// algorithms of the handshake. The discovery layer calls this with what
    /// it knows of the peer (its `VendorId`) BEFORE `begin_handshake_request`
    /// or `begin_handshake_reply`, as [`Self::set_algo_nul_terminate`]. A
    /// plugin whose configuration names the spelling (`org.zerodds.sec.auth.
    /// algorithm_names`) follows that instead. The default is the
    /// specification's.
    fn set_peer_algorithm_names(&mut self, _names: PeerAlgorithmNames) {}

    /// The PermissionsToken the plugin was given with
    /// `set_permissions_credential_and_token`, or the one its AccessControl
    /// counterpart announces, for `PID_PERMISSIONS_TOKEN` of the SPDP
    /// announce; empty when no permissions are configured (AccessControl
    /// inactive, the token is omitted). Secure vendors (Cyclone, Fast DDS)
    /// validate a remote participant only if its SPDP carries both the
    /// identity and the permissions token.
    fn get_permissions_token(&self) -> Vec<u8> {
        Vec::new()
    }

    /// The clock of the plugin: the DDS implementation calls it from its
    /// periodic tick with the current time (seconds since the Unix epoch),
    /// so that the plugin finds out when an identity expired or was
    /// revoked and tells its listener ("identity certificates can expire; in
    /// this situation, the AuthenticationPlugin shall call the
    /// AuthenticationListener to notify the DDS implementation that the
    /// identity of a specific DomainParticipant is being revoked",
    /// §9.3.2.12). The interface of Table 29 has no operation that carries
    /// time.
    fn check_identities(&mut self, _now_unix_secs: u64) {}
}

/// Authentication plugin trait: the operations of the Authentication
/// interface, DDS-Security 1.2 §9.3.2.11 Table 29. In the Rust API a failing
/// operation returns `Err(SecurityError)` (`VALIDATION_FAILED` or `false`,
/// with the `SecurityException`); out parameters are returned.
pub trait AuthenticationPlugin: AuthenticationExtensions + Send + Sync {
    /// Validates the identity of the local DomainParticipant (§9.3.2.11.2).
    /// Called before the DomainParticipant is enabled. `participant_qos` is
    /// the `PropertyQosPolicy` of the `DomainParticipantQos` (the
    /// properties of §10.3.1 for the builtin plugin); `domain_id` is the
    /// DDS Domain Id and `candidate_participant_guid` "the GUID_t that the
    /// DDS implementation would have used to uniquely identify the
    /// DomainParticipant if the Security plugins were not enabled".
    ///
    /// Returns `VALIDATION_OK` with the handle and the
    /// `adjusted_participant_guid`, or `VALIDATION_PENDING_RETRY`, in which
    /// case the operation is "called again after a configurable delay"
    /// until it returns OK or fails.
    ///
    /// # Errors
    /// `VALIDATION_FAILED` with the exception.
    fn validate_local_identity(
        &mut self,
        domain_id: u32,
        participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation>;

    /// Retrieves the IdentityToken that represents the identity of the local
    /// DomainParticipant on the network (§9.3.2.11.9), as a CDR DataHolder.
    /// Published in the SPDP announce as `PID_IDENTITY_TOKEN` (0x1001). The
    /// token is given back with [`Self::return_identity_token`].
    ///
    /// # Errors
    /// A handle that `validate_local_identity` did not return.
    fn get_identity_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>>;

    /// Retrieves the IdentityStatusToken that represents the authentication
    /// state of the local DomainParticipant (§9.3.2.11.10), to be included
    /// in the DCPSParticipantsSecure builtin Topic. Empty is TokenNIL.
    /// Given back with [`Self::return_identity_status_token`].
    ///
    /// # Errors
    /// A handle that `validate_local_identity` did not return.
    fn get_identity_status_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>>;

    /// Configures the plugin with the `ParticipantSecurityConfig` of the
    /// AccessControl plugin and returns the `adjusted_algorithm_info`
    /// (§9.3.2.11.11): a copy of `participant_security_config.algorithm_info`
    /// with the algorithms the plugin does not support removed from the
    /// `supported_mask`s and the ones it uses added to the `required_mask`s.
    /// Called after `validate_local_identity` and
    /// `get_participant_security_config`.
    ///
    /// # Errors
    /// The plugin is not able to restrict the algorithms to those of the
    /// configuration, or the handle is not a local identity.
    fn set_participant_security_config(
        &mut self,
        handle: IdentityHandle,
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<AlgorithmMasks>;

    /// Associates the PermissionsCredentialToken of the AccessControl plugin
    /// and the PermissionsToken with the local DomainParticipant
    /// (§9.3.2.11.12). Called after `validate_local_identity` and before
    /// any `validate_remote_identity`. Both are CDR DataHolders.
    ///
    /// # Errors
    /// A handle that `validate_local_identity` did not return, or a token
    /// the plugin cannot read.
    fn set_permissions_credential_and_token(
        &mut self,
        handle: IdentityHandle,
        permissions_credential_token: &[u8],
        permissions_token: &[u8],
    ) -> SecurityResult<()>;

    /// Called as soon as a remote participant has been discovered via SPDP
    /// (§9.3.2.11.3): validates the identity represented by
    /// `remote_identity_token` (a CDR DataHolder) of the participant
    /// `remote_participant_guid`.
    ///
    /// `remote_auth_request_token` is "the AuthRequestMessageToken received
    /// from the remote DomainParticipant that caused the authentication to
    /// begin", `None` (TokenNIL) if it was not initiated by one.
    ///
    /// The result tells the DDS implementation what to do next:
    /// `PendingHandshakeRequest` — call `begin_handshake_request`;
    /// `PendingHandshakeMessage` — wait for the message of the peer and call
    /// `begin_handshake_reply`; `PendingRetry` — call again later; `Ok`.
    ///
    /// # Errors
    /// `VALIDATION_FAILED` with the exception.
    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        remote_identity_token: &[u8],
        remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation>;

    /// Starts the handshake as the initiator (§9.3.2.11.4); called solely
    /// after `validate_remote_identity` returned
    /// `PendingHandshakeRequest`. `serialized_local_participant_data` is the
    /// "CDR Big Endian Serialization for the ParticipantBuiltInTopicDataSecure
    /// object associated with the local DomainParticipant" (`c.pdata`). The
    /// outcome carries the `handshake_message_out` to send to the replier.
    ///
    /// # Errors
    /// `VALIDATION_FAILED` with the exception.
    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        replier: IdentityHandle,
        serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>;

    /// Peer side of the handshake start (§9.3.2.11.5); called solely after
    /// `validate_remote_identity` returned `PendingHandshakeMessage` and a
    /// message of the initiator arrived. `request_token` is the
    /// `handshake_message_in`; `replier` is the local, `initiator` the
    /// remote identity.
    ///
    /// # Errors
    /// `VALIDATION_FAILED` with the exception.
    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        initiator: IdentityHandle,
        request_token: &[u8],
        serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>;

    /// Continues a handshake with the message of the peer (§9.3.2.11.6).
    /// A message that fails the checks leaves the handshake in the state it
    /// was in (§9.3.2.9.1 rule 3), so the genuine message can still follow.
    ///
    /// # Errors
    /// `VALIDATION_FAILED` with the exception.
    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome>;

    /// `get_shared_secret` (§9.3.2.11.7): the SharedSecretHandle that
    /// resulted from a successfully completed handshake; called after the
    /// last handshake operation on `handshake` returned `Ok` or
    /// `OkFinalMessage`.
    ///
    /// # Errors
    /// The handshake has not completed (the NILHandle with the exception).
    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle>;

    /// `get_authenticated_peer_credential_token` (§9.3.2.11.8): the
    /// AuthenticatedPeerCredentialToken (a CDR DataHolder) that resulted from
    /// the authentication of the peer, for the AccessControl plugin; called
    /// after the handshake completed. Given back with
    /// [`Self::return_authenticated_peer_credential_token`].
    ///
    /// # Errors
    /// The handshake has not completed successfully.
    fn get_authenticated_peer_credential_token(
        &mut self,
        handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>>;

    /// `set_listener` (§9.3.2.11.13): the listener the plugin notifies of
    /// events relevant to the authentication of participants; `None` means
    /// there shall be no listener.
    ///
    /// # Errors
    /// The plugin cannot install the listener.
    fn set_listener(
        &mut self,
        listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()>;

    /// `return_identity_token` (§9.3.2.11.14): gives a token issued by
    /// `get_identity_token` back to the plugin "so it can be disposed of".
    ///
    /// # Errors
    /// A token the plugin did not issue, or already took back.
    fn return_identity_token(&mut self, token: &[u8]) -> SecurityResult<()>;

    /// `return_identity_status_token` (§9.3.2.11.15).
    ///
    /// # Errors
    /// A token the plugin did not issue, or already took back.
    fn return_identity_status_token(&mut self, token: &[u8]) -> SecurityResult<()>;

    /// `return_authenticated_peer_credential_token` (§9.3.2.11.16).
    ///
    /// # Errors
    /// A token the plugin did not issue, or already took back.
    fn return_authenticated_peer_credential_token(&mut self, token: &[u8]) -> SecurityResult<()>;

    /// `return_handshake_handle` (§9.3.2.11.17): gives a HandshakeHandle
    /// issued by `begin_handshake_request` or `begin_handshake_reply` back
    /// to the plugin; the state of the handshake, its shared secret
    /// association included, is released.
    ///
    /// # Errors
    /// A handle the plugin did not issue, or already took back.
    fn return_handshake_handle(&mut self, handshake: HandshakeHandle) -> SecurityResult<()>;

    /// `return_identity_handle` (§9.3.2.11.18): gives an `IdentityHandle`
    /// issued by `validate_local_identity` or `validate_remote_identity` back
    /// to the plugin "so it can be disposed of".
    ///
    /// # Errors
    /// `BadArgument` for a handle the plugin did not issue or already took
    /// back.
    fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()>;

    /// `return_sharedsecret_handle` (§9.3.2.11.19): gives a
    /// SharedSecretHandle issued through `get_shared_secret` back to the
    /// plugin; the secret is released.
    ///
    /// # Errors
    /// A handle the plugin did not issue, or already took back.
    fn return_sharedsecret_handle(&mut self, handle: SharedSecretHandle) -> SecurityResult<()>;
}

/// Factory alias — avoids `Box<dyn ...>` boilerplate at call sites.
pub type AuthPluginBox = Box<dyn AuthenticationPlugin>;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // Compile check: the stub impl demonstrates that the trait is object-safe
    // + Send+Sync satisfiable, and that Table 29 is the whole interface.
    struct StubAuth;
    impl AuthenticationExtensions for StubAuth {
        fn plugin_class_id(&self) -> &str {
            "DDS:Auth:Stub"
        }
    }
    impl AuthenticationPlugin for StubAuth {
        fn validate_local_identity(
            &mut self,
            _domain_id: u32,
            _qos: &PropertyList,
            guid: [u8; 16],
        ) -> SecurityResult<LocalIdentityValidation> {
            Ok(LocalIdentityValidation {
                result: ValidationResult::Ok,
                identity: IdentityHandle(1),
                adjusted_participant_guid: guid,
            })
        }
        fn get_identity_token(&mut self, _h: IdentityHandle) -> SecurityResult<Vec<u8>> {
            Ok(Vec::new())
        }
        fn get_identity_status_token(&mut self, _h: IdentityHandle) -> SecurityResult<Vec<u8>> {
            Ok(Vec::new())
        }
        fn set_participant_security_config(
            &mut self,
            _h: IdentityHandle,
            config: &ParticipantSecurityConfig,
        ) -> SecurityResult<AlgorithmMasks> {
            Ok(config.algorithm_info)
        }
        fn set_permissions_credential_and_token(
            &mut self,
            _h: IdentityHandle,
            _credential: &[u8],
            _token: &[u8],
        ) -> SecurityResult<()> {
            Ok(())
        }
        fn validate_remote_identity(
            &mut self,
            _l: IdentityHandle,
            _r: [u8; 16],
            _t: &[u8],
            _remote_auth_request_token: Option<&[u8]>,
        ) -> SecurityResult<RemoteIdentityValidation> {
            Ok(RemoteIdentityValidation {
                result: ValidationResult::PendingHandshakeRequest,
                identity: IdentityHandle(2),
                local_auth_request_token: None,
            })
        }
        fn begin_handshake_request(
            &mut self,
            _i: IdentityHandle,
            _r: IdentityHandle,
            _pdata: &[u8],
        ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
            Ok((HandshakeHandle(1), HandshakeStepOutcome::WaitingForPeer))
        }
        fn begin_handshake_reply(
            &mut self,
            _r: IdentityHandle,
            _i: IdentityHandle,
            _t: &[u8],
            _pdata: &[u8],
        ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
            Ok((HandshakeHandle(1), HandshakeStepOutcome::WaitingForPeer))
        }
        fn process_handshake(
            &mut self,
            _h: HandshakeHandle,
            _t: &[u8],
        ) -> SecurityResult<HandshakeStepOutcome> {
            Ok(HandshakeStepOutcome::Complete {
                secret: SharedSecretHandle(1),
            })
        }
        fn shared_secret(&self, _h: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
            Ok(SharedSecretHandle(1))
        }
        fn get_authenticated_peer_credential_token(
            &mut self,
            _h: HandshakeHandle,
        ) -> SecurityResult<Vec<u8>> {
            Ok(Vec::new())
        }
        fn set_listener(
            &mut self,
            _listener: Option<Arc<dyn AuthenticationListener>>,
        ) -> SecurityResult<()> {
            Ok(())
        }
        fn return_identity_token(&mut self, _t: &[u8]) -> SecurityResult<()> {
            Ok(())
        }
        fn return_identity_status_token(&mut self, _t: &[u8]) -> SecurityResult<()> {
            Ok(())
        }
        fn return_authenticated_peer_credential_token(&mut self, _t: &[u8]) -> SecurityResult<()> {
            Ok(())
        }
        fn return_handshake_handle(&mut self, _h: HandshakeHandle) -> SecurityResult<()> {
            Ok(())
        }
        fn return_identity_handle(&mut self, _h: IdentityHandle) -> SecurityResult<()> {
            Ok(())
        }
        fn return_sharedsecret_handle(&mut self, _h: SharedSecretHandle) -> SecurityResult<()> {
            Ok(())
        }
    }

    #[test]
    fn stub_can_be_boxed() {
        let _: AuthPluginBox = Box::new(StubAuth);
    }

    /// Table 30: each outcome of a handshake step is one ValidationResult_t;
    /// a message is carried by the two outcomes that send one.
    #[test]
    fn a_handshake_outcome_is_the_validation_result_of_table_30() {
        let token = alloc::vec![1u8, 2, 3];
        let cases = [
            (
                HandshakeStepOutcome::SendMessage {
                    token: token.clone(),
                },
                ValidationResult::PendingHandshakeMessage,
                Some(token.as_slice()),
            ),
            (
                HandshakeStepOutcome::SendFinalMessage {
                    token: token.clone(),
                },
                ValidationResult::OkFinalMessage,
                Some(token.as_slice()),
            ),
            (
                HandshakeStepOutcome::Complete {
                    secret: SharedSecretHandle(7),
                },
                ValidationResult::Ok,
                None,
            ),
            (
                HandshakeStepOutcome::WaitingForPeer,
                ValidationResult::PendingHandshakeMessage,
                None,
            ),
            (
                HandshakeStepOutcome::PendingRetry,
                ValidationResult::PendingRetry,
                None,
            ),
        ];
        for (outcome, result, message) in &cases {
            assert_eq!(outcome.validation_result(), *result, "{outcome:?}");
            assert_eq!(outcome.message_out(), *message, "{outcome:?}");
        }
    }

    /// §9.3.2.11.7: the shared secret is retrieved after VALIDATION_OK or
    /// VALIDATION_OK_FINAL_MESSAGE, and after nothing else.
    #[test]
    fn only_ok_and_ok_final_message_succeed() {
        let results = [
            (ValidationResult::Ok, true),
            (ValidationResult::OkFinalMessage, true),
            (ValidationResult::Failed, false),
            (ValidationResult::PendingRetry, false),
            (ValidationResult::PendingHandshakeRequest, false),
            (ValidationResult::PendingHandshakeMessage, false),
        ];
        for (result, success) in results {
            assert_eq!(result.is_success(), success, "{result:?}");
        }
    }

    /// The NILHandle is the handle no plugin issues (handles count from 1).
    #[test]
    fn the_nil_handles_are_zero() {
        assert_eq!(IdentityHandle::NIL, IdentityHandle(0));
        assert_eq!(HandshakeHandle::NIL, HandshakeHandle(0));
        assert_eq!(SharedSecretHandle::NIL, SharedSecretHandle(0));
    }

    /// §9.3.2.11.11: a configuration without restriction supports every
    /// algorithm and requires none (§7.3.9).
    #[test]
    fn the_unrestricted_configuration_supports_all_and_requires_none() {
        let info = ParticipantSecurityConfig::unrestricted().algorithm_info;
        for (supported, required) in [info.trust_chain, info.message_auth, info.key_establishment] {
            assert_eq!(supported, 0xffff_ffff);
            assert_eq!(required, 0);
        }
    }
}
