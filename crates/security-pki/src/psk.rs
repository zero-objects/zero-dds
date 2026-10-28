// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Pre-shared-key authentication: the spec plugin `DDS:Auth:PSK` and a
//! handshake of ZeroDDS next to it.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`SharedSecretProvider` is a plugin SPI; the `&dyn` test bridge demonstrates
//! the substitution path to the crypto plugin.)
//!
//! # `DDS:Auth:PSK` (DDS-Security 1.2 §10.7)
//!
//! [`PskAuthenticationPlugin`] is what the clause describes: "mostly a
//! 'NOOP' plugin". It authenticates nobody ("treating all participants it
//! discovers as 'Unauthenticated' Participants"), `validate_remote_identity`
//! always fails, and no handshake token exists. Its IdentityToken has the
//! `class_id` `DDS:Auth:PSK:1.2`. It is meant to run with `DDS:Access:PSK`
//! and `DDS:Crypto:PSK`, where the shared key is what keeps outsiders out.
//!
//! # A handshake of ZeroDDS: `PskHandshakeAuthenticationPlugin`
//!
//! [`PskHandshakeAuthenticationPlugin`] is not in the specification. It
//! authenticates two participants that hold the same pre-shared key by a
//! three-message handshake and derives a `SharedSecret` from it, for the
//! crypto plugin that takes one. Its tokens carry `class_id`s under
//! `org.zerodds.auth.psk-handshake`, not under the name the specification gave
//! to the plugin above; the two are different mechanisms and a peer must
//! not mistake one for the other.
//!
//! ## Wire layout
//!
//! Three tokens, all as `DataHolder` on the wire:
//!
//! | Token  | class_id                                        | properties / binary properties |
//! |--------|-------------------------------------------------|--------------------------------|
//! | Req    | `org.zerodds.auth.psk-handshake:1.0+AuthReq`        | `psk.id`, `challenge1`, `c.kagree_algo="PSK"` |
//! | Reply  | `org.zerodds.auth.psk-handshake:1.0+AuthReply`      | `psk.id`, `challenge1`, `challenge2`, `hmac` |
//! | Final  | `org.zerodds.auth.psk-handshake:1.0+AuthFinal`      | `challenge1`, `challenge2`, `hmac` |
//!
//! The HMAC is `HMAC-SHA256(pre_shared_key, length_prefixed(psk.id ||
//! challenge1 || challenge2))`. The `SharedSecret` output is 32 bytes
//! HKDF-SHA256(pre_shared_key, salt=challenge1||challenge2,
//! info="DDS-Security-1.2-PSK").

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::backend::hkdf;
use crate::backend::hmac;
use crate::backend::rand::{SecureRandom, SystemRandom};
use alloc::sync::Arc;
use zerodds_security::authentication::{
    AlgorithmMasks, AuthenticationExtensions, AuthenticationListener, AuthenticationPlugin,
    HandshakeHandle, HandshakeStepOutcome, IdentityHandle, IssuedTokens, LocalIdentityValidation,
    ParticipantSecurityConfig, RemoteIdentityValidation, SharedSecretHandle, SharedSecretProvider,
    ValidationResult,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;
use zerodds_security::token::DataHolder;

/// Class-ID strings of the handshake of ZeroDDS (not specified; the class
/// the specification gives to `DDS:Auth:PSK` is [`SPEC_CLASS_ID`]).
pub mod class_id {
    /// Plugin class ID of the PSK handshake authentication.
    pub const PSK: &str = "org.zerodds.auth.psk-handshake:1.0";
    /// Request token.
    pub const REQUEST: &str = "org.zerodds.auth.psk-handshake:1.0+AuthReq";
    /// Reply token.
    pub const REPLY: &str = "org.zerodds.auth.psk-handshake:1.0+AuthReply";
    /// Final token.
    pub const FINAL: &str = "org.zerodds.auth.psk-handshake:1.0+AuthFinal";
}

/// The `class_id` of the IdentityToken of `DDS:Auth:PSK`
/// (DDS-Security 1.2 §10.7.2.1).
pub const SPEC_CLASS_ID: &str = "DDS:Auth:PSK:1.2";

/// The builtin authentication plugin `DDS:Auth:PSK` (DDS-Security 1.2
/// §10.7), "mostly a 'NOOP' plugin". Table 83:
///
/// * `validate_local_identity` "shall always return VALIDATION_OK" and leaves
///   the participant GUID as it is;
/// * `get_identity_token` returns the token of §10.7.2.1;
/// * `get_identity_status_token` returns TokenNIL;
/// * `validate_remote_identity` "shall always return VALIDATION_FAILED": the
///   participants it discovers are unauthenticated, and no handshake step is
///   ever taken, so there are no handshake tokens and no shared secret.
#[derive(Default)]
pub struct PskAuthenticationPlugin {
    next_handle: AtomicU64,
    identities: BTreeSet<IdentityHandle>,
    /// The tokens handed out and not given back.
    tokens_out: IssuedTokens,
    /// `set_listener`: kept, and never called: the plugin authenticates
    /// nobody and revokes nothing.
    ///
    /// zerodds-lint: allow no_dyn_in_safe
    /// (the §9.3.2.12 `AuthenticationListener` is a plugin slot.)
    listener: Option<Arc<dyn AuthenticationListener>>,
}

impl core::fmt::Debug for PskAuthenticationPlugin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PskAuthenticationPlugin")
            .field("identities", &self.identities)
            .field("listener", &self.listener.is_some())
            .finish_non_exhaustive()
    }
}

/// Kinds of token `PskAuthenticationPlugin` hands out.
const TOKEN_IDENTITY: u8 = 0;
const TOKEN_IDENTITY_STATUS: u8 = 1;

impl PskAuthenticationPlugin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `set_listener` installed a listener.
    #[must_use]
    pub fn has_listener(&self) -> bool {
        self.listener.is_some()
    }
}

fn psk_not_used(what: &'static str) -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::AuthenticationFailed,
        match what {
            "remote" => {
                "psk: DDS:Auth:PSK authenticates no participant (VALIDATION_FAILED, §10.7.3)"
            }
            _ => "psk: DDS:Auth:PSK has no handshake (§10.7.3)",
        },
    )
}

impl AuthenticationExtensions for PskAuthenticationPlugin {
    fn plugin_class_id(&self) -> &str {
        SPEC_CLASS_ID
    }
}

impl AuthenticationPlugin for PskAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        _domain_id: u32,
        _participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        // Table 83: "The operation shall always return VALIDATION_OK. The
        // operation shall set the output 16-byte adjusted_participant_guid
        // GUID to the same value as the input participant_guid."
        let identity = IdentityHandle(self.next_handle.fetch_add(1, Ordering::Relaxed) + 1);
        self.identities.insert(identity);
        Ok(LocalIdentityValidation {
            result: ValidationResult::Ok,
            identity,
            adjusted_participant_guid: candidate_participant_guid,
        })
    }

    fn get_identity_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        if !self.identities.contains(&handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: unknown IdentityHandle",
            ));
        }
        Ok(self
            .tokens_out
            .issue(TOKEN_IDENTITY, DataHolder::new(SPEC_CLASS_ID).to_cdr_le()))
    }

    fn get_identity_status_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        if !self.identities.contains(&handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: unknown IdentityHandle",
            ));
        }
        // Table 83: "This operation shall return TokenNIL."
        Ok(self.tokens_out.issue(TOKEN_IDENTITY_STATUS, Vec::new()))
    }

    fn set_participant_security_config(
        &mut self,
        _handle: IdentityHandle,
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<AlgorithmMasks> {
        // Table 83: "This operation shall do nothing and return TRUE": the
        // adjusted algorithm info is the one given.
        Ok(participant_security_config.algorithm_info)
    }

    fn set_permissions_credential_and_token(
        &mut self,
        _handle: IdentityHandle,
        _permissions_credential_token: &[u8],
        _permissions_token: &[u8],
    ) -> SecurityResult<()> {
        // Table 83: "This operation shall do nothing and return TRUE."
        Ok(())
    }

    fn validate_remote_identity(
        &mut self,
        _local: IdentityHandle,
        _remote_participant_guid: [u8; 16],
        _remote_identity_token: &[u8],
        _remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation> {
        // Table 83: "The operation shall always return VALIDATION_FAILED."
        Err(psk_not_used("remote"))
    }

    fn begin_handshake_request(
        &mut self,
        _initiator: IdentityHandle,
        _replier: IdentityHandle,
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        Err(psk_not_used("handshake"))
    }

    fn begin_handshake_reply(
        &mut self,
        _replier: IdentityHandle,
        _initiator: IdentityHandle,
        _request_token: &[u8],
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        Err(psk_not_used("handshake"))
    }

    fn process_handshake(
        &mut self,
        _handshake: HandshakeHandle,
        _token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        Err(psk_not_used("handshake"))
    }

    fn shared_secret(&self, _handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        Err(psk_not_used("handshake"))
    }

    fn get_authenticated_peer_credential_token(
        &mut self,
        _handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        Err(psk_not_used("handshake"))
    }

    fn set_listener(
        &mut self,
        listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()> {
        // Table 83: "This operation shall save a reference to the listener
        // object".
        self.listener = listener;
        Ok(())
    }

    fn return_identity_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(TOKEN_IDENTITY, token)
    }

    fn return_identity_status_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(TOKEN_IDENTITY_STATUS, token)
    }

    fn return_authenticated_peer_credential_token(&mut self, _token: &[u8]) -> SecurityResult<()> {
        // The plugin issues none.
        Err(psk_not_used("handshake"))
    }

    fn return_handshake_handle(&mut self, _handshake: HandshakeHandle) -> SecurityResult<()> {
        Err(psk_not_used("handshake"))
    }

    fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
        if self.identities.remove(&handle) {
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: unknown IdentityHandle",
            ))
        }
    }

    fn return_sharedsecret_handle(&mut self, _handle: SharedSecretHandle) -> SecurityResult<()> {
        Err(psk_not_used("handshake"))
    }
}

/// Property keys in the handshake token (spec §10.7.2).
pub mod prop {
    /// PSK identity (UTF-8 string).
    pub const PSK_ID: &str = "psk.id";
    /// Initiator challenge (32 bytes binary).
    pub const CHALLENGE1: &str = "challenge1";
    /// Replier challenge (32 bytes binary).
    pub const CHALLENGE2: &str = "challenge2";
    /// HMAC-SHA256 tag (32 bytes binary).
    pub const HMAC: &str = "hmac";
    /// Key agreement algorithm (always `"PSK"` for this plugin).
    pub const KAGREE_ALGO: &str = "c.kagree_algo";
}

/// PropertyList key for the local PSK identifier.
pub const PROP_PSK_ID: &str = "dds.psk.identity_id";
/// PropertyList key for the local PSK material (hex-encoded).
pub const PROP_PSK_KEY_HEX: &str = "dds.psk.pre_shared_key_hex";

/// Replay cache per local identity (DoS cap analogous to PKI).
const REPLAY_CACHE_CAP: usize = 1024;

/// HKDF info string for SharedSecret derivation. Spec domain
/// separator (§10.7.3).
pub const HKDF_INFO_SHARED_SECRET: &[u8] = b"DDS-Security-1.2-PSK";

/// The pre-shared-key handshake of ZeroDDS (not DDS-Security 1.2 §10.7, see
/// the module documentation).
pub struct PskHandshakeAuthenticationPlugin {
    next_handle: AtomicU64,
    /// Configured PSKs: identity string → pre-shared-key bytes.
    psks: BTreeMap<String, Vec<u8>>,
    /// Locally validated identities: handle → identity string.
    identities: BTreeMap<IdentityHandle, String>,
    /// The participant GUID each local identity was validated for.
    local_guids: BTreeMap<IdentityHandle, [u8; 16]>,
    /// Per remote identity: the local identity it was validated for and the
    /// `ValidationResult_t` of `validate_remote_identity`.
    remote_roles: BTreeMap<IdentityHandle, (IdentityHandle, ValidationResult)>,
    /// Initiator state between request and reply.
    pending_initiator: BTreeMap<HandshakeHandle, InitiatorState>,
    /// Replier state between reply and final.
    pending_replier: BTreeMap<HandshakeHandle, ReplierState>,
    /// Completed handshakes → SharedSecret handle.
    handshake_to_secret: BTreeMap<HandshakeHandle, SharedSecretHandle>,
    /// The handshakes issued and not given back.
    handshakes_issued: BTreeSet<HandshakeHandle>,
    /// Per completed handshake, the `psk.id` of the peer (its
    /// AuthenticatedPeerCredentialToken).
    peer_psk_ids: BTreeMap<HandshakeHandle, String>,
    /// Materialized SharedSecrets (32 bytes HKDF output).
    secrets: BTreeMap<SharedSecretHandle, Vec<u8>>,
    /// Replay cache: per local identity the already-seen
    /// `challenge1` values (replier view).
    replay_cache: BTreeMap<IdentityHandle, BTreeSet<[u8; 32]>>,
    /// FIFO order of the replay-cache entries for cap eviction.
    replay_order: BTreeMap<IdentityHandle, Vec<[u8; 32]>>,
    /// The tokens handed out and not given back.
    tokens_out: IssuedTokens,
    /// `set_listener`.
    ///
    /// zerodds-lint: allow no_dyn_in_safe
    /// (the §9.3.2.12 `AuthenticationListener` is a plugin slot.)
    listener: Option<Arc<dyn AuthenticationListener>>,
}

struct InitiatorState {
    local: IdentityHandle,
    psk_id: String,
    challenge1: [u8; 32],
}

/// Kinds of token `PskHandshakeAuthenticationPlugin` hands out.
const HS_TOKEN_IDENTITY: u8 = 0;
const HS_TOKEN_STATUS: u8 = 1;
const HS_TOKEN_PEER_CREDENTIAL: u8 = 2;

struct ReplierState {
    local: IdentityHandle,
    psk_id: String,
    challenge1: [u8; 32],
    challenge2: [u8; 32],
    secret_handle: SharedSecretHandle,
}

impl Default for PskHandshakeAuthenticationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PskHandshakeAuthenticationPlugin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            psks: BTreeMap::new(),
            identities: BTreeMap::new(),
            local_guids: BTreeMap::new(),
            remote_roles: BTreeMap::new(),
            pending_initiator: BTreeMap::new(),
            pending_replier: BTreeMap::new(),
            handshake_to_secret: BTreeMap::new(),
            handshakes_issued: BTreeSet::new(),
            peer_psk_ids: BTreeMap::new(),
            secrets: BTreeMap::new(),
            replay_cache: BTreeMap::new(),
            replay_order: BTreeMap::new(),
            tokens_out: IssuedTokens::default(),
            listener: None,
        }
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Registers a pre-shared key for an identity. Replace
    /// semantics: a second registration with the same ID overwrites.
    ///
    /// # Errors
    /// `BadArgument` if `id` is empty or `key` is empty.
    pub fn register_psk(&mut self, id: String, key: Vec<u8>) -> SecurityResult<()> {
        if id.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: identity-id empty",
            ));
        }
        if key.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: pre-shared-key empty",
            ));
        }
        self.psks.insert(id, key);
        Ok(())
    }

    /// [`Self::validate_local_psk_identity`] for the participant `guid`: the
    /// identity can then validate remote identities, which orders the two
    /// participants by their GUIDs (`validate_remote_identity`, Table 59
    /// analogously).
    ///
    /// # Errors
    /// `BadArgument` if `identity_id` is not registered.
    pub fn validate_local_psk_identity_for(
        &mut self,
        identity_id: &str,
        guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle> {
        let handle = self.validate_local_psk_identity(identity_id)?;
        self.local_guids.insert(handle, guid);
        Ok(handle)
    }

    /// Validates a local PSK identity and returns a handle. Without the
    /// participant GUID ([`Self::validate_local_psk_identity_for`]) the
    /// identity cannot validate remote identities.
    ///
    /// # Errors
    /// `BadArgument` if `identity_id` is not registered.
    pub fn validate_local_psk_identity(
        &mut self,
        identity_id: &str,
    ) -> SecurityResult<IdentityHandle> {
        if !self.psks.contains_key(identity_id) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("psk: unknown identity-id '{identity_id}'"),
            ));
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, identity_id.to_string());
        Ok(handle)
    }

    /// Validates a remote PSK identity (from the propagated
    /// IdentityToken). Check: the claimed `psk.id` must be present in
    /// our local PSK map.
    ///
    /// # Errors
    /// `BadArgument` if the token has a format error or the
    /// `psk.id` is not in the local map.
    pub fn validate_remote_psk_identity(
        &mut self,
        remote_token: &[u8],
    ) -> SecurityResult<IdentityHandle> {
        let dh = DataHolder::from_cdr_le(remote_token)?;
        if dh.class_id != class_id::PSK {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!(
                    "psk: remote IdentityToken has wrong class_id '{}'",
                    dh.class_id
                ),
            ));
        }
        let id = dh.property(PROP_PSK_ID).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: IdentityToken without psk.id",
            )
        })?;
        if !self.psks.contains_key(id) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: remote psk.id '{id}' not in the local trust store"),
            ));
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, id.to_string());
        Ok(handle)
    }

    /// Creates the wire IdentityToken (`org.zerodds.auth.psk-handshake:1.0`) for a
    /// local identity.
    ///
    /// # Errors
    /// `BadArgument` if the handle is unknown.
    pub fn build_identity_token(&self, local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        let id = self.identities.get(&local).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "psk: unknown handle")
        })?;
        let dh = DataHolder::new(class_id::PSK).with_property(PROP_PSK_ID, id.clone());
        Ok(dh.to_cdr_le())
    }

    /// Returns the raw SharedSecret (32 bytes) — primarily for tests.
    #[must_use]
    pub fn secret_bytes(&self, handle: SharedSecretHandle) -> Option<&[u8]> {
        self.secrets.get(&handle).map(Vec::as_slice)
    }

    fn store_secret(&mut self, bytes: Vec<u8>) -> SharedSecretHandle {
        let handle = SharedSecretHandle(self.next_id());
        self.secrets.insert(handle, bytes);
        handle
    }

    fn record_challenge(&mut self, local: IdentityHandle, c: [u8; 32]) -> SecurityResult<()> {
        let cache = self.replay_cache.entry(local).or_default();
        if cache.contains(&c) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: replayed challenge1 detected",
            ));
        }
        cache.insert(c);
        let order = self.replay_order.entry(local).or_default();
        order.push(c);
        if order.len() > REPLAY_CACHE_CAP {
            let dropped = order.remove(0);
            cache.remove(&dropped);
        }
        Ok(())
    }

    fn lookup_psk(&self, id: &str) -> SecurityResult<&[u8]> {
        self.psks.get(id).map(Vec::as_slice).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: unknown identity-id '{id}'"),
            )
        })
    }
}

impl SharedSecretProvider for PskHandshakeAuthenticationPlugin {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        self.secrets.get(&handle).cloned()
    }
}

/// Random 32-byte challenge.
fn random_challenge() -> SecurityResult<[u8; 32]> {
    let rng = SystemRandom::new();
    let mut buf = [0u8; 32];
    rng.fill(&mut buf).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            "psk: SystemRandom not available",
        )
    })?;
    Ok(buf)
}

/// HMAC input per spec §10.7.2. Length-prefixed concatenation
/// (`u32-LE len || bytes`) prevents cross-field tampering.
fn hmac_input(psk_id: &str, ch1: &[u8; 32], ch2: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + psk_id.len() + 4 + 32 + 4 + 32);
    out.extend_from_slice(&(psk_id.len() as u32).to_le_bytes());
    out.extend_from_slice(psk_id.as_bytes());
    out.extend_from_slice(&(32u32).to_le_bytes());
    out.extend_from_slice(ch1);
    out.extend_from_slice(&(32u32).to_le_bytes());
    out.extend_from_slice(ch2);
    out
}

fn hmac_sign(psk: &[u8], psk_id: &str, ch1: &[u8; 32], ch2: &[u8; 32]) -> [u8; 32] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, psk);
    let tag = hmac::sign(&key, &hmac_input(psk_id, ch1, ch2));
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

fn hmac_verify(
    psk: &[u8],
    psk_id: &str,
    ch1: &[u8; 32],
    ch2: &[u8; 32],
    tag: &[u8],
) -> SecurityResult<()> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, psk);
    hmac::verify(&key, &hmac_input(psk_id, ch1, ch2), tag).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "psk: hmac verify failed",
        )
    })
}

/// Spec §10.7.3 — SharedSecret = HKDF-SHA256(psk, salt=ch1||ch2,
/// info="DDS-Security-1.2-PSK"). Output: 32 bytes.
pub fn derive_psk_shared_secret(
    psk: &[u8],
    ch1: &[u8; 32],
    ch2: &[u8; 32],
) -> SecurityResult<[u8; 32]> {
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(ch1);
    salt[32..].copy_from_slice(ch2);
    let salt_obj = hkdf::Salt::new(hkdf::HKDF_SHA256, &salt);
    let prk = salt_obj.extract(psk);
    let info = [HKDF_INFO_SHARED_SECRET];
    let okm = prk.expand(&info, hkdf::HKDF_SHA256).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "psk: HKDF expand failed")
    })?;
    let mut out = [0u8; 32];
    okm.fill(&mut out).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "psk: HKDF fill failed")
    })?;
    Ok(out)
}

fn read_32(dh: &DataHolder, name: &str) -> SecurityResult<[u8; 32]> {
    let bytes = dh.binary_property(name).ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("psk: missing binary property '{name}'"),
        )
    })?;
    if bytes.len() != 32 {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("psk: '{name}' must be 32 bytes"),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

impl AuthenticationExtensions for PskHandshakeAuthenticationPlugin {
    fn plugin_class_id(&self) -> &str {
        class_id::PSK
    }
}

impl AuthenticationPlugin for PskHandshakeAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        _domain_id: u32,
        participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        let props = participant_qos;
        let id = props.get(PROP_PSK_ID).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "psk: missing dds.psk.identity_id",
            )
        })?;
        // Optional: load PSK material directly from properties (hex).
        if let Some(hex) = props.get(PROP_PSK_KEY_HEX) {
            let bytes = hex_decode(hex)?;
            self.register_psk(id.to_string(), bytes)?;
        }
        let identity = self.validate_local_psk_identity(id)?;
        self.local_guids
            .insert(identity, candidate_participant_guid);
        Ok(LocalIdentityValidation {
            result: ValidationResult::Ok,
            identity,
            // The handshake binds no GUID to the identity.
            adjusted_participant_guid: candidate_participant_guid,
        })
    }

    /// Returns the `IdentityToken` announced in the SPDP beacon (PID_IDENTITY_TOKEN).
    /// Without it no token would be on the beacon, and the peer would never
    /// start `begin_handshake_with`.
    fn get_identity_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        let token = self.build_identity_token(handle)?;
        Ok(self.tokens_out.issue(HS_TOKEN_IDENTITY, token))
    }

    fn get_identity_status_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        if !self.local_guids.contains_key(&handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: not an IdentityHandle returned by validate_local_identity",
            ));
        }
        // The pre-shared key has no status of its own: TokenNIL.
        Ok(self.tokens_out.issue(HS_TOKEN_STATUS, Vec::new()))
    }

    fn set_participant_security_config(
        &mut self,
        handle: IdentityHandle,
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<AlgorithmMasks> {
        if !self.local_guids.contains_key(&handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: not an IdentityHandle returned by validate_local_identity",
            ));
        }
        // The handshake uses HMAC-SHA256 and HKDF-SHA256 over a pre-shared
        // key; none of the digital signature or key establishment algorithms
        // of Table 25 and 26. The configuration is not restricted by it, and
        // the plugin adds no algorithm to the required masks.
        Ok(participant_security_config.algorithm_info)
    }

    fn set_permissions_credential_and_token(
        &mut self,
        handle: IdentityHandle,
        _permissions_credential_token: &[u8],
        _permissions_token: &[u8],
    ) -> SecurityResult<()> {
        // The handshake carries no permissions document.
        if self.local_guids.contains_key(&handle) {
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: not an IdentityHandle returned by validate_local_identity",
            ))
        }
    }

    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        remote_identity_token: &[u8],
        _remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation> {
        let local_guid = self.local_guids.get(&local).copied().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: not an IdentityHandle returned by validate_local_identity \
                 (or validate_local_psk_identity_for)",
            )
        })?;
        let result = match remote_participant_guid.cmp(&local_guid) {
            core::cmp::Ordering::Greater => ValidationResult::PendingHandshakeRequest,
            core::cmp::Ordering::Less => ValidationResult::PendingHandshakeMessage,
            core::cmp::Ordering::Equal => {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "psk: the remote participant GUID is the local one",
                ));
            }
        };
        let identity = self.validate_remote_psk_identity(remote_identity_token)?;
        self.remote_roles.insert(identity, (local, result));
        // The handshake sends no AuthRequestMessageToken (TokenNIL).
        Ok(RemoteIdentityValidation {
            result,
            identity,
            local_auth_request_token: None,
        })
    }

    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        _replier: IdentityHandle,
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let psk_id = self
            .identities
            .get(&initiator)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk: unknown initiator IdentityHandle",
                )
            })?
            .clone();
        let challenge1 = random_challenge()?;

        let token = DataHolder::new(class_id::REQUEST)
            .with_property(prop::PSK_ID, psk_id.clone())
            .with_property(prop::KAGREE_ALGO, "PSK")
            .with_binary_property(prop::CHALLENGE1, challenge1.to_vec())
            .to_cdr_le();

        let handle = HandshakeHandle(self.next_id());
        self.handshakes_issued.insert(handle);
        self.pending_initiator.insert(
            handle,
            InitiatorState {
                local: initiator,
                psk_id,
                challenge1,
            },
        );
        Ok((handle, HandshakeStepOutcome::SendMessage { token }))
    }

    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        _initiator: IdentityHandle,
        request_token: &[u8],
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let dh = DataHolder::from_cdr_le(request_token)?;
        if dh.class_id != class_id::REQUEST {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!(
                    "psk: reply expected request, got class_id '{}'",
                    dh.class_id
                ),
            ));
        }
        let psk_id = dh
            .property(prop::PSK_ID)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "psk: request missing psk.id",
                )
            })?
            .to_string();
        let challenge1 = read_32(&dh, prop::CHALLENGE1)?;

        // Replay detection (replier side).
        self.record_challenge(replier, challenge1)?;

        // PSK lookup against the local trust store.
        let psk = self.lookup_psk(&psk_id)?.to_vec();

        let challenge2 = random_challenge()?;
        let hmac = hmac_sign(&psk, &psk_id, &challenge1, &challenge2);
        let secret = derive_psk_shared_secret(&psk, &challenge1, &challenge2)?;

        let token = DataHolder::new(class_id::REPLY)
            .with_property(prop::PSK_ID, psk_id.clone())
            .with_property(prop::KAGREE_ALGO, "PSK")
            .with_binary_property(prop::CHALLENGE1, challenge1.to_vec())
            .with_binary_property(prop::CHALLENGE2, challenge2.to_vec())
            .with_binary_property(prop::HMAC, hmac.to_vec())
            .to_cdr_le();

        // The shared secret is handed out with the final, after the
        // initiator proved the key.
        let secret_handle = self.store_secret(secret.to_vec());
        let handle = HandshakeHandle(self.next_id());
        self.handshakes_issued.insert(handle);
        self.pending_replier.insert(
            handle,
            ReplierState {
                local: replier,
                psk_id,
                challenge1,
                challenge2,
                secret_handle,
            },
        );
        Ok((handle, HandshakeStepOutcome::SendMessage { token }))
    }

    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        if self.pending_initiator.contains_key(&handshake) {
            return self.process_reply_on_initiator(handshake, token);
        }
        if self.pending_replier.contains_key(&handshake) {
            return self.process_final_on_replier(handshake, token);
        }
        Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk: unknown HandshakeHandle",
        ))
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        self.handshake_to_secret
            .get(&handshake)
            .copied()
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk: handshake handle unknown or not yet completed",
                )
            })
    }

    fn get_authenticated_peer_credential_token(
        &mut self,
        handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        let psk_id = self.peer_psk_ids.get(&handshake).cloned().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: the handshake has not completed (no authenticated peer)",
            )
        })?;
        let token = DataHolder::new(class_id::PSK)
            .with_property(PROP_PSK_ID, psk_id)
            .to_cdr_le();
        Ok(self.tokens_out.issue(HS_TOKEN_PEER_CREDENTIAL, token))
    }

    fn set_listener(
        &mut self,
        listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()> {
        self.listener = listener;
        Ok(())
    }

    fn return_identity_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(HS_TOKEN_IDENTITY, token)
    }

    fn return_identity_status_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(HS_TOKEN_STATUS, token)
    }

    fn return_authenticated_peer_credential_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(HS_TOKEN_PEER_CREDENTIAL, token)
    }

    fn return_handshake_handle(&mut self, handshake: HandshakeHandle) -> SecurityResult<()> {
        if !self.handshakes_issued.remove(&handshake) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: unknown HandshakeHandle",
            ));
        }
        self.pending_initiator.remove(&handshake);
        self.pending_replier.remove(&handshake);
        self.handshake_to_secret.remove(&handshake);
        self.peer_psk_ids.remove(&handshake);
        Ok(())
    }

    fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
        self.replay_cache.remove(&handle);
        self.replay_order.remove(&handle);
        self.local_guids.remove(&handle);
        self.remote_roles.remove(&handle);
        self.identities.remove(&handle).map(|_| ()).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: unknown IdentityHandle",
            )
        })
    }

    fn return_sharedsecret_handle(&mut self, handle: SharedSecretHandle) -> SecurityResult<()> {
        if !self.handshake_to_secret.values().any(|s| *s == handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: unknown SharedSecretHandle",
            ));
        }
        self.handshake_to_secret.retain(|_, s| *s != handle);
        self.secrets.remove(&handle);
        Ok(())
    }
}

impl PskHandshakeAuthenticationPlugin {
    fn process_reply_on_initiator(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let dh = DataHolder::from_cdr_le(token)?;
        if dh.class_id != class_id::REPLY {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: process expected reply, got '{}'", dh.class_id),
            ));
        }
        // The state stays until the reply has passed its checks
        // (§9.3.2.9.1 rule 3): a forged reply must not end the handshake.
        let st = self.pending_initiator.get(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "psk: initiator state gone")
        })?;
        let psk_id_in = dh
            .property(prop::PSK_ID)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "psk: reply missing psk.id",
                )
            })?
            .to_string();
        if psk_id_in != st.psk_id {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: psk.id echo mismatch in reply",
            ));
        }
        let ch1 = read_32(&dh, prop::CHALLENGE1)?;
        if ch1 != st.challenge1 {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: challenge1 echo mismatch",
            ));
        }
        let ch2 = read_32(&dh, prop::CHALLENGE2)?;
        let hmac = dh.binary_property(prop::HMAC).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: reply missing hmac",
            )
        })?;

        let (psk_id, local) = (st.psk_id.clone(), st.local);
        if !self.identities.contains_key(&local) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: the local identity of the handshake was returned",
            ));
        }
        let psk = self.lookup_psk(&psk_id)?.to_vec();
        hmac_verify(&psk, &psk_id, &ch1, &ch2, hmac)?;
        self.pending_initiator.remove(&handshake);

        // Own HMAC for the final token.
        let final_hmac = hmac_sign(&psk, &psk_id, &ch1, &ch2);
        let secret = derive_psk_shared_secret(&psk, &ch1, &ch2)?;
        let secret_handle = self.store_secret(secret.to_vec());
        self.handshake_to_secret.insert(handshake, secret_handle);
        self.peer_psk_ids.insert(handshake, psk_id);

        let final_token = DataHolder::new(class_id::FINAL)
            .with_binary_property(prop::CHALLENGE1, ch1.to_vec())
            .with_binary_property(prop::CHALLENGE2, ch2.to_vec())
            .with_binary_property(prop::HMAC, final_hmac.to_vec())
            .to_cdr_le();

        Ok(HandshakeStepOutcome::SendFinalMessage { token: final_token })
    }

    fn process_final_on_replier(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let dh = DataHolder::from_cdr_le(token)?;
        if dh.class_id != class_id::FINAL {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: process expected final, got '{}'", dh.class_id),
            ));
        }
        let st = self.pending_replier.get(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "psk: replier state gone")
        })?;
        let ch1 = read_32(&dh, prop::CHALLENGE1)?;
        let ch2 = read_32(&dh, prop::CHALLENGE2)?;
        if ch1 != st.challenge1 || ch2 != st.challenge2 {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: final challenge echo mismatch",
            ));
        }
        let hmac = dh.binary_property(prop::HMAC).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: final missing hmac",
            )
        })?;
        let psk = self.lookup_psk(&st.psk_id)?.to_vec();
        hmac_verify(&psk, &st.psk_id, &ch1, &ch2, hmac)?;
        let (secret_handle, psk_id) = (st.secret_handle, st.psk_id.clone());
        let _ = st.local;
        self.pending_replier.remove(&handshake);
        // The initiator proved the key: the secret is the handshake's now.
        self.handshake_to_secret.insert(handshake, secret_handle);
        self.peer_psk_ids.insert(handshake, psk_id);
        Ok(HandshakeStepOutcome::Complete {
            secret: secret_handle,
        })
    }
}

fn hex_decode(s: &str) -> SecurityResult<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk: hex string with odd length",
        ));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for chunk in bytes.chunks(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        // Arithmetic form instead of `(hi << 4) | lo`: for nibble values
        // (0..=15) the bits do not overlap, so it is identical.
        // mutation-detection-friendly: `*` and `+` are not
        // equivalent to each other.
        out.push(hi * 16 + lo);
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> SecurityResult<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk: invalid hex nibble",
        )),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::properties::Property;

    fn alice_bob_with_shared_psk() -> (
        PskHandshakeAuthenticationPlugin,
        PskHandshakeAuthenticationPlugin,
        IdentityHandle,
        IdentityHandle,
    ) {
        let psk = alloc::vec![0xA5u8; 32];
        let mut alice = PskHandshakeAuthenticationPlugin::new();
        let mut bob = PskHandshakeAuthenticationPlugin::new();
        alice.register_psk("alice-bob".into(), psk.clone()).unwrap();
        bob.register_psk("alice-bob".into(), psk).unwrap();
        let alice_h = alice.validate_local_psk_identity("alice-bob").unwrap();
        let bob_h = bob.validate_local_psk_identity("alice-bob").unwrap();
        (alice, bob, alice_h, bob_h)
    }

    /// The handshake of ZeroDDS does not use the class the specification
    /// gives to `DDS:Auth:PSK`.
    #[test]
    fn the_handshake_classes_are_not_the_spec_class() {
        let p = PskHandshakeAuthenticationPlugin::new();
        assert_eq!(p.plugin_class_id(), "org.zerodds.auth.psk-handshake:1.0");
        assert_eq!(class_id::PSK, "org.zerodds.auth.psk-handshake:1.0");
        assert_eq!(
            class_id::REQUEST,
            "org.zerodds.auth.psk-handshake:1.0+AuthReq"
        );
        assert_eq!(
            class_id::REPLY,
            "org.zerodds.auth.psk-handshake:1.0+AuthReply"
        );
        assert_eq!(
            class_id::FINAL,
            "org.zerodds.auth.psk-handshake:1.0+AuthFinal"
        );
        assert_ne!(class_id::PSK, SPEC_CLASS_ID);
    }

    /// DDS-Security 1.2 §10.7.3 Table 83: validate_local_identity is always
    /// OK, the IdentityToken has the class `DDS:Auth:PSK:1.2` and no other
    /// attribute, the status token is TokenNIL, validate_remote_identity
    /// always fails, and there is no handshake and no shared secret.
    #[test]
    fn the_spec_plugin_authenticates_nobody() {
        let mut p = PskAuthenticationPlugin::new();
        assert_eq!(p.plugin_class_id(), "DDS:Auth:PSK:1.2");
        let local = p
            .validate_local_identity(0, &PropertyList::default(), [7; 16])
            .unwrap()
            .identity;
        let token_bytes = p.get_identity_token(local).unwrap();
        let token = DataHolder::from_cdr_le(&token_bytes).unwrap();
        assert_eq!(token.class_id, "DDS:Auth:PSK:1.2");
        assert!(token.properties.is_empty() && token.binary_properties.is_empty());
        assert!(p.get_identity_status_token(local).unwrap().is_empty());
        // Whatever the remote presents, even a token of this very plugin.
        for presented in [
            DataHolder::new("DDS:Auth:PSK:1.2").to_cdr_le(),
            DataHolder::new(class_id::PSK).to_cdr_le(),
            Vec::new(),
        ] {
            let err = p
                .validate_remote_identity(local, [8; 16], &presented, None)
                .unwrap_err();
            assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
        }
        assert!(p.begin_handshake_request(local, local, &[]).is_err());
        assert!(p.begin_handshake_reply(local, local, &[], &[]).is_err());
        assert!(p.process_handshake(HandshakeHandle(1), &[]).is_err());
        assert!(p.shared_secret(HandshakeHandle(1)).is_err());
        assert!(
            p.get_authenticated_peer_credential_token(HandshakeHandle(1))
                .is_err()
        );
        assert!(p.return_identity_handle(local).is_ok());
        assert!(p.return_identity_handle(local).is_err(), "taken back once");
    }

    /// Table 83, validate_local_identity: "shall always return VALIDATION_OK.
    /// The operation shall set the output 16-byte adjusted_participant_guid
    /// GUID to the same value as the input participant_guid."
    #[test]
    fn table_83_validate_local_identity_returns_ok_and_the_input_guid() {
        let mut p = PskAuthenticationPlugin::new();
        for guid in [[7u8; 16], [0u8; 16], [0xFF; 16]] {
            let v = p
                .validate_local_identity(3, &PropertyList::default(), guid)
                .unwrap();
            assert_eq!(v.result, ValidationResult::Ok);
            assert_eq!(v.adjusted_participant_guid, guid);
            assert_ne!(v.identity, IdentityHandle::NIL);
        }
    }

    /// Table 83, set_participant_security_config and
    /// set_permissions_credential_and_token: "shall do nothing and return
    /// TRUE": the adjusted info is the one given, and nothing the credential
    /// carries is kept or checked.
    #[test]
    fn table_83_set_participant_security_config_and_credential_do_nothing() {
        let mut p = PskAuthenticationPlugin::new();
        let local = p
            .validate_local_identity(0, &PropertyList::default(), [1; 16])
            .unwrap()
            .identity;
        let config = ParticipantSecurityConfig {
            algorithm_info: AlgorithmMasks {
                trust_chain: (0x0F, 0x01),
                message_auth: (0x03, 0x02),
                key_establishment: (0x07, 0x04),
            },
        };
        assert_eq!(
            p.set_participant_security_config(local, &config).unwrap(),
            config.algorithm_info,
            "adjusted_algorithm_info is the given one"
        );
        p.set_permissions_credential_and_token(local, b"credential", b"token")
            .unwrap();
        p.set_permissions_credential_and_token(local, &[], &[])
            .unwrap();
        assert!(
            p.get_permissions_token().is_empty(),
            "no PermissionsToken kept"
        );
    }

    /// Table 83, set_listener: "shall save a reference to the listener".
    #[test]
    fn table_83_set_listener_saves_the_listener() {
        struct Quiet;
        impl AuthenticationListener for Quiet {
            fn on_revoke_identity(&self, _handle: IdentityHandle) -> SecurityResult<()> {
                Ok(())
            }
            fn on_status_changed(
                &self,
                _handle: IdentityHandle,
                _status_kind: zerodds_security::authentication::AuthStatusKind,
            ) {
            }
        }
        let mut p = PskAuthenticationPlugin::new();
        assert!(!p.has_listener());
        p.set_listener(Some(Arc::new(Quiet))).unwrap();
        assert!(p.has_listener());
        p.set_listener(None).unwrap();
        assert!(!p.has_listener(), "nil means no listener");
    }

    /// Table 83, return_identity_token, return_identity_status_token and
    /// return_identity_handle behave as §9.3.2.11.14, .15 and .18: a token the
    /// plugin issued goes back once, one it did not issue is refused.
    #[test]
    fn table_83_the_return_operations_take_back_what_was_issued() {
        let mut p = PskAuthenticationPlugin::new();
        let local = p
            .validate_local_identity(0, &PropertyList::default(), [1; 16])
            .unwrap()
            .identity;
        let identity = p.get_identity_token(local).unwrap();
        let status = p.get_identity_status_token(local).unwrap();
        assert!(
            p.return_identity_status_token(&identity).is_err(),
            "an identity token is no status token"
        );
        p.return_identity_token(&identity).unwrap();
        assert!(p.return_identity_token(&identity).is_err(), "twice");
        p.return_identity_status_token(&status).unwrap();
        assert!(p.return_identity_status_token(&status).is_err(), "twice");
        assert!(p.return_identity_token(b"other").is_err(), "not issued");
        // A token handed out twice goes back twice.
        let a = p.get_identity_token(local).unwrap();
        let b = p.get_identity_token(local).unwrap();
        p.return_identity_token(&a).unwrap();
        p.return_identity_token(&b).unwrap();
        assert!(p.return_identity_token(&a).is_err());
        p.return_identity_handle(local).unwrap();
        assert!(p.get_identity_token(local).is_err(), "the handle is gone");
    }

    #[test]
    fn register_psk_then_validate_local_happy_path() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        p.register_psk("client-1".into(), alloc::vec![0x11; 32])
            .unwrap();
        let h = p.validate_local_psk_identity("client-1").unwrap();
        assert!(h.0 >= 1);
    }

    #[test]
    fn validate_local_unknown_id_rejected() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        let err = p.validate_local_psk_identity("ghost").unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_rejects_empty_key() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        let err = p.register_psk("x".into(), Vec::new()).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_rejects_empty_id() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        let err = p
            .register_psk(String::new(), alloc::vec![1, 2, 3])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_replace_semantics_last_wins() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        p.register_psk("k".into(), alloc::vec![1; 32]).unwrap();
        p.register_psk("k".into(), alloc::vec![2; 32]).unwrap();
        let key = p.psks.get("k").unwrap();
        assert_eq!(key, &alloc::vec![2u8; 32]);
    }

    #[test]
    fn full_three_round_handshake_alice_bob() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();

        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        let out4 = bob.process_handshake(bob_hs, &final_tok).unwrap();
        let bob_secret = match out4 {
            HandshakeStepOutcome::Complete { secret } => secret,
            _ => panic!("expected Complete"),
        };

        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        let a_bytes = alice.secret_bytes(alice_secret).unwrap();
        let b_bytes = bob.secret_bytes(bob_secret).unwrap();
        assert_eq!(a_bytes.len(), 32);
        assert_eq!(a_bytes, b_bytes, "alice + bob must have the same secret");
    }

    #[test]
    fn tampered_reply_hmac_rejected_by_initiator() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let mut reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let mut h = DataHolder::from_cdr_le(&reply).unwrap();
        let mut hmac = h.binary_property(prop::HMAC).unwrap().to_vec();
        hmac[0] ^= 0x01;
        h.set_binary_property(prop::HMAC, hmac);
        reply = h.to_cdr_le();
        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn replay_initiator_request_rejected_second_time() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (_alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        bob.begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let err = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn wrong_psk_on_replier_breaks_hmac_on_initiator() {
        let mut alice = PskHandshakeAuthenticationPlugin::new();
        let mut bob = PskHandshakeAuthenticationPlugin::new();
        alice
            .register_psk("k".into(), alloc::vec![0xAAu8; 32])
            .unwrap();
        // Bob has a different key for the same ID.
        bob.register_psk("k".into(), alloc::vec![0xBBu8; 32])
            .unwrap();
        let alice_h = alice.validate_local_psk_identity("k").unwrap();
        let bob_h = bob.validate_local_psk_identity("k").unwrap();

        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn unknown_psk_id_in_request_rejected_by_replier() {
        let mut bob = PskHandshakeAuthenticationPlugin::new();
        bob.register_psk("known".into(), alloc::vec![0x11; 32])
            .unwrap();
        let bob_h = bob.validate_local_psk_identity("known").unwrap();
        // Alice sends a request with an "unknown" id (hand-built).
        let req = DataHolder::new(class_id::REQUEST)
            .with_property(prop::PSK_ID, "unknown")
            .with_property(prop::KAGREE_ALGO, "PSK")
            .with_binary_property(prop::CHALLENGE1, alloc::vec![0u8; 32])
            .to_cdr_le();
        let err = bob
            .begin_handshake_reply(bob_h, IdentityHandle(99), &req, &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn truncated_request_rejected() {
        let mut bob = PskHandshakeAuthenticationPlugin::new();
        bob.register_psk("k".into(), alloc::vec![0x11; 32]).unwrap();
        let bob_h = bob.validate_local_psk_identity("k").unwrap();
        let err = bob
            .begin_handshake_reply(bob_h, IdentityHandle(99), &[0u8, 1, 2], &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn validate_remote_token_happy_path() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        p.register_psk("peer-1".into(), alloc::vec![0xCCu8; 32])
            .unwrap();
        let local = p.validate_local_psk_identity("peer-1").unwrap();
        let token = p.build_identity_token(local).unwrap();
        let remote = p.validate_remote_psk_identity(&token).unwrap();
        assert_ne!(remote, local);
    }

    #[test]
    fn validate_remote_token_rejects_unknown_id() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        p.register_psk("known".into(), alloc::vec![0xCCu8; 32])
            .unwrap();
        let token = DataHolder::new(class_id::PSK)
            .with_property(PROP_PSK_ID, "stranger")
            .to_cdr_le();
        let err = p.validate_remote_psk_identity(&token).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn validate_remote_token_rejects_wrong_class_id() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        p.register_psk("k".into(), alloc::vec![0x1u8; 32]).unwrap();
        let token = DataHolder::new("DDS:Auth:PKI-DH:1.2")
            .with_property(PROP_PSK_ID, "k")
            .to_cdr_le();
        let err = p.validate_remote_psk_identity(&token).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn cross_plugin_psk_vs_pki_mismatch_class_id() {
        // Token from the PSK plugin → classified as different by the
        // PKI IdentityToken decoder (different class_ids).
        let mut psk = PskHandshakeAuthenticationPlugin::new();
        psk.register_psk("k".into(), alloc::vec![0xAA; 32]).unwrap();
        let h = psk.validate_local_psk_identity("k").unwrap();
        let psk_token = psk.build_identity_token(h).unwrap();
        let dh = DataHolder::from_cdr_le(&psk_token).unwrap();
        assert_eq!(dh.class_id, "org.zerodds.auth.psk-handshake:1.0");
        assert_ne!(dh.class_id, "DDS:Auth:PKI-DH:1.2");
    }

    #[test]
    fn token_roundtrip_via_data_holder_codec() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        p.register_psk("alpha".into(), alloc::vec![0xBE; 32])
            .unwrap();
        let h = p.validate_local_psk_identity("alpha").unwrap();
        let token = p.build_identity_token(h).unwrap();
        let dh = DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(dh.class_id, class_id::PSK);
        assert_eq!(dh.property(PROP_PSK_ID), Some("alpha"));
    }

    #[test]
    fn validate_local_via_property_list_with_inline_hex_key() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        let key_hex: String = (0..32).map(|_| "ab").collect();
        let props = PropertyList::new()
            .with(Property::local(PROP_PSK_ID, "node-1"))
            .with(Property::local(PROP_PSK_KEY_HEX, key_hex));
        let h = p
            .validate_local_identity(0, &props, [0xAA; 16])
            .unwrap()
            .identity;
        assert!(h.0 >= 1);
    }

    #[test]
    fn validate_local_via_property_list_missing_id_rejected() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        let props = PropertyList::new();
        let err = p
            .validate_local_identity(0, &props, [0xAA; 16])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    #[test]
    fn shared_secret_returns_bad_argument_for_unknown_handle() {
        let p = PskHandshakeAuthenticationPlugin::new();
        let err = p.shared_secret(HandshakeHandle(42)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn hkdf_test_vector_rfc5869_ish_is_deterministic() {
        // Cross-validation: same PSK + same challenges → bit-identical secret.
        let psk = alloc::vec![0x0bu8; 22];
        let ch1 = [0x01u8; 32];
        let ch2 = [0x02u8; 32];
        let s1 = derive_psk_shared_secret(&psk, &ch1, &ch2).unwrap();
        let s2 = derive_psk_shared_secret(&psk, &ch1, &ch2).unwrap();
        assert_eq!(s1, s2);
        // Different challenges → different secret.
        let s3 = derive_psk_shared_secret(&psk, &ch2, &ch1).unwrap();
        assert_ne!(s1, s3);
    }

    #[test]
    fn shared_secret_is_32_bytes() {
        let psk = alloc::vec![0xFFu8; 16];
        let s = derive_psk_shared_secret(&psk, &[0u8; 32], &[1u8; 32]).unwrap();
        assert_eq!(s.len(), 32);
    }

    #[test]
    fn process_handshake_unknown_handle_rejected() {
        let mut p = PskHandshakeAuthenticationPlugin::new();
        let err = p.process_handshake(HandshakeHandle(999), &[]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn final_token_validates_initiator_hmac_on_replier() {
        // If the initiator HMAC in the final token is broken,
        // process_final_on_replier fails with AuthenticationFailed.
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, req_out) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match req_out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, reply_out) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let reply = match reply_out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let final_out = alice.process_handshake(alice_hs, &reply).unwrap();
        let mut final_tok = match final_out {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!(),
        };
        let mut h = DataHolder::from_cdr_le(&final_tok).unwrap();
        let mut hm = h.binary_property(prop::HMAC).unwrap().to_vec();
        hm[5] ^= 0xFF;
        h.set_binary_property(prop::HMAC, hm);
        final_tok = h.to_cdr_le();
        let err = bob.process_handshake(bob_hs, &final_tok).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn request_token_carries_kagree_psk() {
        let (mut alice, _bob, alice_h, _bob_h) = alice_bob_with_shared_psk();
        let (_, out) = alice
            .begin_handshake_request(alice_h, IdentityHandle(99), &[])
            .unwrap();
        let token = match out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let dh = DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(dh.class_id, class_id::REQUEST);
        assert_eq!(dh.property(prop::KAGREE_ALGO), Some("PSK"));
    }

    #[test]
    fn shared_secret_provider_returns_bytes_after_handshake() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!(),
        };
        bob.process_handshake(bob_hs, &final_tok).unwrap();

        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        let provider: &dyn SharedSecretProvider = &alice;
        let bytes = provider.get_shared_secret(alice_secret).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn hex_decode_round_trips_simple_input() {
        let v = hex_decode("0a0b").unwrap();
        assert_eq!(v, alloc::vec![0x0a, 0x0b]);
    }

    #[test]
    fn hex_decode_rejects_odd_len() {
        let err = hex_decode("abc").unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn hex_decode_rejects_non_hex() {
        let err = hex_decode("zz").unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    // -------------------------------------------------------------
    // Mutation killers (2026-05-01)
    // -------------------------------------------------------------

    /// Replay-cache CAP boundary (analogous to plugin.rs).
    /// Catches `>` -> `==`/`>=` on record_challenge eviction.
    #[test]
    fn psk_replay_cache_holds_exactly_cap() {
        let (mut alice, _, alice_h, _) = alice_bob_with_shared_psk();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        let mut c_first = [0u8; 32];
        c_first[0..8].copy_from_slice(&0u64.to_le_bytes());
        let err = alice.record_challenge(alice_h, c_first).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn psk_replay_cache_evicts_at_cap_plus_one() {
        let (mut alice, _, alice_h, _) = alice_bob_with_shared_psk();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        let mut c_extra = [0u8; 32];
        c_extra[0..8].copy_from_slice(&(REPLAY_CACHE_CAP as u64).to_le_bytes());
        alice.record_challenge(alice_h, c_extra).unwrap();
        let mut c0 = [0u8; 32];
        c0[0..8].copy_from_slice(&0u64.to_le_bytes());
        alice
            .record_challenge(alice_h, c0)
            .expect("oldest should be evicted");
    }

    /// hmac_input returns a specific length-prefixed concatenation.
    /// Catches mutations `vec![]`/`vec![0]`/`vec![1]`.
    #[test]
    fn hmac_input_length_prefixed_concatenation() {
        let psk_id = "abc";
        let ch1 = [0x11u8; 32];
        let ch2 = [0x22u8; 32];
        let out = hmac_input(psk_id, &ch1, &ch2);
        // Layout: u32-LE psk_id_len + psk_id + u32-LE 32 + ch1 + u32-LE 32 + ch2
        // = 4 + 3 + 4 + 32 + 4 + 32 = 79
        assert_eq!(out.len(), 4 + 3 + 4 + 32 + 4 + 32);
        // First 4 bytes: 3 as u32-LE
        assert_eq!(&out[0..4], &3u32.to_le_bytes());
        // psk_id ascii
        assert_eq!(&out[4..7], b"abc");
        // ch1 len prefix
        assert_eq!(&out[7..11], &32u32.to_le_bytes());
        // ch1 bytes
        assert_eq!(&out[11..43], &ch1[..]);
        // ch2 len prefix
        assert_eq!(&out[43..47], &32u32.to_le_bytes());
        // ch2 bytes
        assert_eq!(&out[47..79], &ch2[..]);

        // Different inputs => different outputs
        let out_b = hmac_input("xyz", &ch1, &ch2);
        assert_ne!(out, out_b);
    }

    /// hex_nibble: cover all three branches a..f, A..F, 0..9.
    /// Catches `delete arm` and `+ -> -` mutations.
    #[test]
    fn hex_nibble_all_three_ranges() {
        for (c, expected) in [
            (b'0', 0u8),
            (b'5', 5),
            (b'9', 9),
            (b'a', 10),
            (b'c', 12),
            (b'f', 15),
            (b'A', 10),
            (b'C', 12),
            (b'F', 15),
        ] {
            assert_eq!(hex_nibble(c).unwrap(), expected, "char {c:#x}");
        }
        assert!(hex_nibble(b'g').is_err());
        assert!(hex_nibble(b'G').is_err());
        assert!(hex_nibble(b'@').is_err());
    }

    /// hex_decode with concrete value — catches `*` -> `+` and arith.
    /// mutations on the nibble accumulation.
    #[test]
    fn hex_decode_specific_byte_values() {
        // "ab" = 0xAB; with `*` mutated the result would be 10+11=21 != 171.
        assert_eq!(hex_decode("ab").unwrap(), vec![0xAB]);
        // "F0" = 0xF0; mutated: 15+0=15 vs 15*16=240.
        assert_eq!(hex_decode("F0").unwrap(), vec![0xF0]);
        assert_eq!(hex_decode("0F").unwrap(), vec![0x0F]);
        assert_eq!(
            hex_decode("DEADBEEF").unwrap(),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    /// Final-token tampering: a single field mismatch must reject.
    /// Catches `||` -> `&&` (line 624).
    fn run_psk_final_tampered<M>(mutate: M)
    where
        M: FnOnce(&mut DataHolder),
    {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!(),
        };
        let mut h = DataHolder::from_cdr_le(&final_tok).unwrap();
        mutate(&mut h);
        let tampered = h.to_cdr_le();
        let err = bob.process_handshake(bob_hs, &tampered).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn psk_final_challenge1_tamper_rejected() {
        run_psk_final_tampered(|h| {
            let mut v = h.binary_property("challenge1").unwrap().to_vec();
            v[0] ^= 0x01;
            h.set_binary_property("challenge1", v);
        });
    }
    #[test]
    fn psk_final_challenge2_tamper_rejected() {
        run_psk_final_tampered(|h| {
            let mut v = h.binary_property("challenge2").unwrap().to_vec();
            v[0] ^= 0x01;
            h.set_binary_property("challenge2", v);
        });
    }

    /// Catches `||` -> `&&` (line 624) in process_final_on_replier
    /// echo check: ch1 != local AND ch2 == local must reject.
    ///
    /// Naive tamper tests do not catch this, because the HMAC verify ALSO
    /// catches the mismatch. Here the HMAC is recomputed over the tampered
    /// values, so that the mutation to `&&` leads to replier
    /// acceptance while the original `||` leads to an echo reject.
    #[test]
    fn psk_final_only_ch1_tamper_with_recomputed_hmac_rejected() {
        let psk_bytes = alloc::vec![0xA5u8; 32];
        let psk_id = "alice-bob";

        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, &req, &[])
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!(),
        };

        // Tamper challenge1 + recompute HMAC over the tampered values.
        let mut h = DataHolder::from_cdr_le(&final_tok).unwrap();
        let mut new_ch1 = h.binary_property("challenge1").unwrap().to_vec();
        new_ch1[0] ^= 0x01;
        let ch2_vec = h.binary_property("challenge2").unwrap().to_vec();

        let mut ch1_arr = [0u8; 32];
        ch1_arr.copy_from_slice(&new_ch1);
        let mut ch2_arr = [0u8; 32];
        ch2_arr.copy_from_slice(&ch2_vec);
        let new_hmac = hmac_sign(&psk_bytes, psk_id, &ch1_arr, &ch2_arr);

        h.set_binary_property("challenge1", new_ch1);
        h.set_binary_property("hmac", new_hmac.to_vec());
        let tampered = h.to_cdr_le();

        // Original (`||`): echo mismatch of ch1 => AuthFailed.
        // Mutation (`&&`): ch1 mismatched but ch2 ok => the echo check
        // passes, the HMAC verify passes (recomputed) => success.
        let err = bob.process_handshake(bob_hs, &tampered).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    // ---------------------------------------------------------------
    // The operations of Table 29 for the handshake of ZeroDDS
    // ---------------------------------------------------------------

    /// Two plugins with the same key whose identities know their GUIDs: Alice
    /// has the smaller one.
    fn guided_pair() -> (
        PskHandshakeAuthenticationPlugin,
        PskHandshakeAuthenticationPlugin,
        IdentityHandle,
        IdentityHandle,
    ) {
        let mut alice = PskHandshakeAuthenticationPlugin::new();
        let mut bob = PskHandshakeAuthenticationPlugin::new();
        for plugin in [&mut alice, &mut bob] {
            plugin.register_psk("k".into(), vec![0x33; 32]).unwrap();
        }
        let alice_h = alice
            .validate_local_psk_identity_for("k", [0x10; 16])
            .unwrap();
        let bob_h = bob
            .validate_local_psk_identity_for("k", [0x20; 16])
            .unwrap();
        (alice, bob, alice_h, bob_h)
    }

    /// Table 59 (analogous): the smaller GUID starts; a local identity that
    /// does not know its GUID cannot validate a remote one.
    #[test]
    fn the_handshake_plugin_orders_remote_identities_by_guid() {
        let (mut alice, mut bob, alice_h, bob_h) = guided_pair();
        let token = |p: &PskHandshakeAuthenticationPlugin, h| p.build_identity_token(h).unwrap();
        let (alice_token, bob_token) = (token(&alice, alice_h), token(&bob, bob_h));
        let seen_by_alice = alice
            .validate_remote_identity(alice_h, [0x20; 16], &bob_token, None)
            .unwrap();
        assert_eq!(
            seen_by_alice.result,
            ValidationResult::PendingHandshakeRequest
        );
        assert!(seen_by_alice.local_auth_request_token.is_none());
        let seen_by_bob = bob
            .validate_remote_identity(bob_h, [0x10; 16], &alice_token, None)
            .unwrap();
        assert_eq!(
            seen_by_bob.result,
            ValidationResult::PendingHandshakeMessage
        );
        assert!(
            alice
                .validate_remote_identity(alice_h, [0x10; 16], &bob_token, None)
                .is_err(),
            "the participant itself"
        );
        // Without the GUID of the local participant there is no order.
        let blind = alice.validate_local_psk_identity("k").unwrap();
        assert!(
            alice
                .validate_remote_identity(blind, [0x20; 16], &bob_token, None)
                .is_err()
        );
    }

    /// §9.3.2.11.8: after the handshake the peer's credential token holds its
    /// `psk.id`; before, there is none. The return operations take back what
    /// was issued (§9.3.2.11.14 to .19), once.
    #[test]
    fn the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued() {
        let (mut alice, mut bob, alice_h, bob_h) = guided_pair();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        assert!(
            alice
                .get_authenticated_peer_credential_token(alice_hs)
                .is_err()
        );
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, out1.message_out().unwrap(), &[])
            .unwrap();
        let out3 = alice
            .process_handshake(alice_hs, out2.message_out().unwrap())
            .unwrap();
        assert_eq!(out3.validation_result(), ValidationResult::OkFinalMessage);
        // The replier is complete with the final, not before.
        assert!(bob.shared_secret(bob_hs).is_err());
        assert!(bob.get_authenticated_peer_credential_token(bob_hs).is_err());
        let out4 = bob
            .process_handshake(bob_hs, out3.message_out().unwrap())
            .unwrap();
        assert_eq!(out4.validation_result(), ValidationResult::Ok);
        for (plugin, hs) in [(&mut alice, alice_hs), (&mut bob, bob_hs)] {
            let credential = plugin.get_authenticated_peer_credential_token(hs).unwrap();
            let holder = DataHolder::from_cdr_le(&credential).unwrap();
            assert_eq!(holder.class_id, class_id::PSK);
            assert_eq!(holder.property(PROP_PSK_ID), Some("k"));
            plugin
                .return_authenticated_peer_credential_token(&credential)
                .unwrap();
            assert!(
                plugin
                    .return_authenticated_peer_credential_token(&credential)
                    .is_err()
            );
            let secret = plugin.shared_secret(hs).unwrap();
            plugin.return_sharedsecret_handle(secret).unwrap();
            assert!(plugin.secret_bytes(secret).is_none());
            assert!(plugin.return_sharedsecret_handle(secret).is_err());
            plugin.return_handshake_handle(hs).unwrap();
            assert!(plugin.return_handshake_handle(hs).is_err());
            assert!(plugin.shared_secret(hs).is_err());
        }
        // Identity and status tokens, and the listener.
        let identity = alice.get_identity_token(alice_h).unwrap();
        let status = alice.get_identity_status_token(alice_h).unwrap();
        assert!(status.is_empty(), "the key has no status: TokenNIL");
        alice.return_identity_token(&identity).unwrap();
        assert!(alice.return_identity_token(&identity).is_err());
        alice.return_identity_status_token(&status).unwrap();
        assert!(
            alice
                .set_participant_security_config(
                    alice_h,
                    &ParticipantSecurityConfig::unrestricted()
                )
                .is_ok()
        );
        assert!(alice.set_listener(None).is_ok());
        assert!(
            alice
                .set_permissions_credential_and_token(alice_h, &[], &[])
                .is_ok()
        );
        assert!(
            alice
                .set_permissions_credential_and_token(IdentityHandle(99), &[], &[])
                .is_err()
        );
    }

    /// §9.3.2.9.1 rule 3 for the handshake of ZeroDDS: a forged reply or
    /// final leaves the handshake in place, so the genuine message completes it.
    #[test]
    fn a_forged_message_leaves_the_handshake_in_place() {
        let (mut alice, mut bob, alice_h, bob_h) = guided_pair();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h, &[]).unwrap();
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, alice_h, out1.message_out().unwrap(), &[])
            .unwrap();
        let reply = out2.message_out().unwrap().to_vec();
        let mut forged = DataHolder::from_cdr_le(&reply).unwrap();
        forged.set_binary_property(prop::HMAC, vec![0u8; 32]);
        assert!(
            alice
                .process_handshake(alice_hs, &forged.to_cdr_le())
                .is_err()
        );
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        assert_eq!(out3.validation_result(), ValidationResult::OkFinalMessage);
        let genuine = out3.message_out().unwrap().to_vec();
        let mut forged = DataHolder::from_cdr_le(&genuine).unwrap();
        forged.set_binary_property(prop::HMAC, vec![0u8; 32]);
        assert!(bob.process_handshake(bob_hs, &forged.to_cdr_le()).is_err());
        assert!(
            bob.shared_secret(bob_hs).is_err(),
            "a forged final hands out no secret"
        );
        assert!(bob.process_handshake(bob_hs, &genuine).is_ok());
        assert!(bob.shared_secret(bob_hs).is_ok());
    }
}
