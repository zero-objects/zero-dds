// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Thread-safe wrapper around [`SecurityGate`] for multi-thread use.
//!
//! The reference-based `SecurityGate<'c, P>` is meant for single-thread
//! use — but several runtime threads (SPDP loop, user RX,
//! event loop) need synchronized access.
//!
//! [`SharedSecurityGate`] encapsulates:
//! * `Governance` (immutable per participant — cloneable).
//! * `Box<dyn CryptographicPlugin>` (mutable on key registration).
//! * Cache of the local CryptoHandle.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (the plugin is held via `Box<dyn CryptographicPlugin>` so that
//! the user can freely choose the backend.)

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use std::sync::{Arc, Mutex, PoisonError};

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security_permissions::{Governance, ProtectionKind};
use zerodds_security_rtps::{
    RTPS_HEADER_LEN, SRTPS_PREFIX, decode_secured_submessage_multi, encode_secured_submessage_multi,
};

use crate::gate::SecurityGateError;
use crate::policy::{NetInterface, ProtectionLevel};

// ============================================================================
// Inbound verdict
// ============================================================================

/// Result of a `classify_inbound` decision.
///
/// The enum variants cleanly separate the possible reasons, so the
/// caller (dcps runtime) can pass a suitable `LogLevel` per reason to the
/// [`zerodds_security::logging::LoggingPlugin`].
///
/// The interface context (`NetInterface`) is passed along by the caller
/// and reappears in [`InboundVerdict::iface`] —
/// so log events are attributable per interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundVerdict {
    /// The packet is admissible — `bytes` is the decoded RTPS datagram
    /// passed on to the SPDP/SEDP/user dispatch.
    Accept(Vec<u8>),
    /// The packet is too short for an RTPS header (< 20 bytes). This is
    /// really a transport bug or a fuzz probe. Severity
    /// should be `Error`.
    Malformed,
    /// The packet came from an **unauth peer on a domain that
    /// requires authentication** (`allow_unauthenticated_participants =
    /// false`). Severity should be `Error`.
    LegacyBlocked,
    /// Policy violation: the domain requires protection but the packet
    /// is plain (or vice versa). Severity should be `Warning`
    /// — possibly a tampering attempt.
    PolicyViolation(String),
    /// Cryptographic error on unwrap (tag mismatch, wrong
    /// key, replay attack, etc.). Severity `Warning`.
    CryptoError(String),
}

impl InboundVerdict {
    /// Shorthand: `true` if the packet is passed on.
    #[must_use]
    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::Accept(_))
    }

    /// Log category (OMG §8.6.3) — free string that identifies the drop
    /// reason. Helpful when a LoggingPlugin filters by category.
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::Accept(_) => "inbound.accept",
            Self::Malformed => "inbound.malformed",
            Self::LegacyBlocked => "inbound.legacy_blocked",
            Self::PolicyViolation(_) => "inbound.policy_violation",
            Self::CryptoError(_) => "inbound.crypto_error",
        }
    }
}

/// Opaque peer identifier. In RTPS environments the caller typically maps
/// `GuidPrefix` (12 bytes) onto it — `[u8; 12]` fits exactly.
pub type PeerKey = [u8; 12];

/// Thread-safe security gate. Clone gives a second reference to
/// the same plugin instance — all clones operate on the same
/// key store.
#[derive(Clone)]
pub struct SharedSecurityGate {
    inner: Arc<Mutex<GateInner>>,
}

impl core::fmt::Debug for SharedSecurityGate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Plugin + keys never in debug output — only metadata.
        match self.inner.lock() {
            Ok(g) => write!(
                f,
                "SharedSecurityGate {{ domain_id: {}, peers: {}, local_registered: {} }}",
                g.domain_id,
                g.peers.len(),
                g.local.is_some()
            ),
            Err(_) => write!(f, "SharedSecurityGate {{ <poisoned> }}"),
        }
    }
}

struct GateInner {
    domain_id: u32,
    governance: Governance,
    crypto: Box<dyn CryptographicPlugin>,
    local: Option<CryptoHandle>,
    /// Peer-key → CryptoHandle mapping. Filled by
    /// `register_remote_by_guid`; `transform_inbound_from`
    /// looks here.
    peers: BTreeMap<PeerKey, CryptoHandle>,
}

/// Derives a deterministic 4-byte `key_id` from a
/// 12-byte [`PeerKey`] (GuidPrefix). Both communication partners
/// compute it identically without a handshake, i.e. the `key_id` in the
/// wire-format SEC_POSTFIX is synchronizable across
/// platforms.
///
/// Usage: low 32 bits of the first 4 bytes of the GuidPrefix.
#[must_use]
pub fn peer_key_to_id(pk: &PeerKey) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&pk[..4]);
    u32::from_le_bytes(buf)
}

fn poisoned<T>(_: PoisonError<T>) -> SecurityGateError {
    SecurityGateError::Crypto(zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::Internal,
        "security-runtime: mutex poisoned",
    ))
}

impl SharedSecurityGate {
    /// Constructor. The plugin is **owned** by the gate (Box).
    #[must_use]
    pub fn new(
        domain_id: u32,
        governance: Governance,
        crypto: Box<dyn CryptographicPlugin>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GateInner {
                domain_id,
                governance,
                crypto,
                local: None,
                peers: BTreeMap::new(),
            })),
        }
    }

    fn with_inner<R>(
        &self,
        f: impl FnOnce(&mut GateInner) -> Result<R, SecurityGateError>,
    ) -> Result<R, SecurityGateError> {
        let mut g = self.inner.lock().map_err(poisoned)?;
        f(&mut g)
    }

    /// Returns the domain id the gate runs for.
    pub fn domain_id(&self) -> Result<u32, SecurityGateError> {
        self.with_inner(|g| Ok(g.domain_id))
    }

    /// `ProtectionKind` for the message level — derived from the first
    /// matching `<domain_rule>`.
    pub fn message_protection(&self) -> Result<ProtectionKind, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.rtps_protection_kind)
                .unwrap_or(ProtectionKind::None))
        })
    }

    /// `ProtectionLevel` for user DATA — derived from the
    /// `data_protection_kind` of the domain's first `<topic_rule>`
    /// (FU2 S3). Used by the user outbound path as a fallback when
    /// the matched reader has not announced an explicit `security_info` level via
    /// SEDP — this way ZeroDDS↔ZeroDDS encrypts user data
    /// according to its own governance, while SPDP/SEDP metatraffic
    /// bootstraps plaintext via `rtps_protection_kind=NONE`.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn data_protection(&self) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .find_domain_rule(g.domain_id)
                .and_then(|r| r.topic_rules.first())
                .map(|t| t.data_protection_kind)
                .unwrap_or(ProtectionKind::None);
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// `ProtectionLevel` for submessage metadata — from the
    /// `metadata_protection_kind` of the domain's first `<topic_rule>`.
    /// Controls (together with [`Self::data_protection`]) the
    /// `endpoint_security_attributes` that ZeroDDS announces via SEDP —
    /// cross-vendor the mask must match cyclone/OpenDDS byte-exactly
    /// (otherwise "security_attributes mismatch" → no endpoint match).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn metadata_protection(&self) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .find_domain_rule(g.domain_id)
                .and_then(|r| r.topic_rules.first())
                .map(|t| t.metadata_protection_kind)
                .unwrap_or(ProtectionKind::None);
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// `ProtectionLevel` for protected discovery — from the DOMAIN-wide
    /// `discovery_protection_kind` (DDS-Security §8.4.2.4 `is_discovery_
    /// protected`). `!= None` ⟹ secured endpoints are announced via the secure SEDP
    /// (DCPSPublicationsSecure/DCPSSubscriptionsSecure) instead of plaintext SEDP;
    /// the DATA/HEARTBEAT/GAP of the secure-SEDP writers are protected with the
    /// participant data key via `encode_datawriter_submessage`.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn discovery_protection(&self) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.discovery_protection_kind)
                .unwrap_or(ProtectionKind::None);
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// `true` if the domain's first `<topic_rule>` sets
    /// `enable_discovery_protection`. Controls the endpoint bit
    /// `IS_DISCOVERY_PROTECTED` of the `EndpointSecurityAttributes`
    /// (§9.4.2.4) — this is a TOPIC-level flag, NOT the domain-wide
    /// [`Self::discovery_protection`] (= `discovery_protection_kind`, which
    /// only protects the SEDP channel). cyclone derives the endpoint mask from
    /// this boolean; ZeroDDS must announce byte-exactly the same,
    /// otherwise "security_attributes mismatch" → no endpoint match.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn topic_discovery_protected(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .and_then(|r| r.topic_rules.first())
                .map(|t| t.enable_discovery_protection)
                .unwrap_or(false))
        })
    }

    /// `true` if the domain's first `<topic_rule>` sets
    /// `enable_read_access_control` → endpoint bit `IS_READ_PROTECTED`
    /// (0x01) of the `EndpointSecurityAttributes` (§9.4.2.4). cyclone sets it for
    /// access-control topics; ZeroDDS must announce byte-exactly the same mask.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn topic_read_protected(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .and_then(|r| r.topic_rules.first())
                .map(|t| t.enable_read_access_control)
                .unwrap_or(false))
        })
    }

    /// `true` if the domain's first `<topic_rule>` sets
    /// `enable_write_access_control` → endpoint bit `IS_WRITE_PROTECTED`
    /// (0x02) of the `EndpointSecurityAttributes` (§9.4.2.4).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn topic_write_protected(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .and_then(|r| r.topic_rules.first())
                .map(|t| t.enable_write_access_control)
                .unwrap_or(false))
        })
    }

    /// `ProtectionLevel` for the whole RTPS message — DOMAIN-wide
    /// `rtps_protection_kind` (§8.4.2.4 `is_rtps_protected`). Flows into the
    /// `ParticipantSecurityAttributes` mask (§9.4.2.4) that ZeroDDS announces via
    /// SPDP.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn rtps_protection(&self) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.rtps_protection_kind)
                .unwrap_or(ProtectionKind::None);
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// `ProtectionLevel` for liveliness (BuiltinParticipantMessageSecure) —
    /// DOMAIN-wide `liveliness_protection_kind` (§8.4.2.4).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn liveliness_protection(&self) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.liveliness_protection_kind)
                .unwrap_or(ProtectionKind::None);
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// Registers the local participant with the crypto plugin (idempotent).
    ///
    /// # Errors
    /// `CryptoSetup` if the plugin does not accept the identity handle.
    pub fn ensure_local(&self) -> Result<CryptoHandle, SecurityGateError> {
        self.with_inner(|g| {
            if let Some(h) = g.local {
                return Ok(h);
            }
            let h = g
                .crypto
                .register_local_participant(IdentityHandle(1), &[])
                .map_err(SecurityGateError::CryptoSetup)?;
            g.local = Some(h);
            Ok(h)
        })
    }

    /// Token of the local participant (to be sent to the remote via SEDP).
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn local_token(&self) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .create_local_participant_crypto_tokens(local, CryptoHandle(0))
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Registers a remote peer and installs its token.
    /// The returned `CryptoHandle` identifies the slot in
    /// which the remote key is stored — needed again at `decode_inbound_message`.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn register_remote_with_token(
        &self,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
        token: &[u8],
    ) -> Result<CryptoHandle, SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            let slot = g
                .crypto
                .register_matched_remote_participant(local, remote_identity, shared_secret)
                .map_err(SecurityGateError::CryptoSetup)?;
            g.crypto
                .set_remote_participant_crypto_tokens(local, slot, token)
                .map_err(SecurityGateError::Crypto)?;
            Ok(slot)
        })
    }

    /// Registers a remote peer **with peer-key mapping**. After
    /// this call [`Self::transform_inbound_from`] can find the peer
    /// by its [`PeerKey`] (GuidPrefix).
    ///
    /// Idempotent: a repeated call with the same key does not overwrite the
    /// old slot — the caller must explicitly call
    /// [`Self::forget_remote`] to be able to rotate.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn register_remote_by_guid(
        &self,
        peer_key: PeerKey,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
        token: &[u8],
    ) -> Result<CryptoHandle, SecurityGateError> {
        // Idempotency check: if already registered, return the existing
        // handle.
        {
            let g = self.inner.lock().map_err(poisoned)?;
            if let Some(h) = g.peers.get(&peer_key) {
                return Ok(*h);
            }
        }
        let slot = self.register_remote_with_token(remote_identity, shared_secret, token)?;
        self.with_inner(|g| {
            g.peers.insert(peer_key, slot);
            Ok(())
        })?;
        Ok(slot)
    }

    /// FU2 S1.2: Registers a remote peer **only with the Kx key**
    /// (from the handshake `SharedSecret`), WITHOUT a data token. The data
    /// key is installed later via [`Self::set_remote_data_token_by_guid`] from
    /// the received ParticipantCryptoToken.
    ///
    /// Maps `peer_key` → slot; idempotent (a repeated call with the
    /// same key returns the existing slot).
    ///
    /// # Errors
    /// `CryptoSetup` if the plugin does not accept the secret.
    pub fn register_remote_by_guid_from_secret(
        &self,
        peer_key: PeerKey,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> Result<CryptoHandle, SecurityGateError> {
        {
            let g = self.inner.lock().map_err(poisoned)?;
            if let Some(h) = g.peers.get(&peer_key) {
                return Ok(*h);
            }
        }
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            let slot = g
                .crypto
                .register_matched_remote_participant(local, remote_identity, shared_secret)
                .map_err(SecurityGateError::CryptoSetup)?;
            g.peers.insert(peer_key, slot);
            Ok(slot)
        })
    }

    /// FU2 S1.2: Installs the **data key** of a peer already registered via
    /// [`Self::register_remote_by_guid_from_secret`]
    /// from its received ParticipantCryptoToken (token exchange).
    ///
    /// # Errors
    /// `PolicyViolation` if the peer is not registered; `Crypto`
    /// on an invalid token.
    pub fn set_remote_data_token_by_guid(
        &self,
        peer_key: &PeerKey,
        token: &[u8],
    ) -> Result<(), SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "set_remote_data_token: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .set_remote_participant_crypto_tokens(local, slot, token)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// FU2 S1.2: Encrypts a VolatileSecure payload (e.g. a
    /// ParticipantCryptoToken) with the peer's **Kx key**
    /// (`encode_kx_submessage`). The peer must be registered via
    /// [`Self::register_remote_by_guid_from_secret`].
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto`.
    pub fn transform_kx_outbound_for(
        &self,
        peer_key: &PeerKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "transform_kx_outbound: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .encode_kx_submessage(slot, plaintext, &[])
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// FU2 S1.2: Decrypts a Kx-protected VolatileSecure
    /// payload of a peer (`decode_kx_submessage`).
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (tag mismatch).
    pub fn transform_kx_inbound_from(
        &self,
        peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "transform_kx_inbound: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .decode_kx_submessage(slot, wire, &[])
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Cross-vendor VolatileSecure: protects a **DATA submessage** as a
    /// cyclone-conform `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX` sequence with the
    /// peer's Kx key (`encode_kx_datawriter_submessage`).
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto`.
    pub fn encode_kx_datawriter_for(
        &self,
        peer_key: &PeerKey,
        data_submessage: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "encode_kx_datawriter: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .encode_kx_datawriter_submessage(slot, data_submessage)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Counterpart: decodes a peer's `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX` sequence
    /// back to the original DATA submessage (`decode_kx_datawriter_submessage`).
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (tag mismatch).
    pub fn decode_kx_datawriter_from(
        &self,
        peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "decode_kx_datawriter: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .decode_kx_datawriter_submessage(slot, wire)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Cross-vendor user DATA (send): protects a DATA submessage with the
    /// **local data key** as a cyclone-conform SEC_PREFIX/BODY/POSTFIX sequence
    /// (`encode_data_datawriter_submessage`). cyclone decodes it with the
    /// (= local) key sent by ZeroDDS via `datawriter_crypto_tokens`.
    ///
    /// # Errors
    /// `CryptoSetup` (no local slot) / `Crypto`.
    pub fn encode_data_datawriter_local(
        &self,
        data_submessage: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .encode_data_datawriter_submessage(local, data_submessage)
                .map_err(SecurityGateError::Crypto)
        })
    }

    // -------- per-endpoint crypto (DDS-Security §8.5 CryptoKeyFactory + §9.5.3.3) --------
    //
    // Instead of a flat participant key for ALL endpoints, each
    // local DataWriter/DataReader (incl. the secure builtin discovery endpoints)
    // gets its OWN key material. On endpoint match the per-endpoint token is
    // exchanged (deterministically, over the reliable VolatileSecure), so that
    // the keys stand BEFORE the data — no fail-first decode retry. cyclone/
    // FastDDS work identically.

    /// Registers a local endpoint crypto slot with its own key material.
    /// `is_writer` = DataWriter (else DataReader). Returns the endpoint handle.
    ///
    /// # Errors
    /// `CryptoSetup` (no local participant) / plugin error.
    pub fn register_local_endpoint(
        &self,
        is_writer: bool,
    ) -> Result<CryptoHandle, SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .register_local_endpoint(local, is_writer, &[])
                .map_err(SecurityGateError::CryptoSetup)
        })
    }

    /// Serializes the per-endpoint `datawriter`/`datareader_crypto_token`
    /// (the key material of the local endpoint slot), to be sent to the matched
    /// remote endpoint via VolatileSecure.
    ///
    /// # Errors
    /// `Crypto` (unknown endpoint handle).
    pub fn create_endpoint_token(
        &self,
        endpoint: CryptoHandle,
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            g.crypto
                .create_local_participant_crypto_tokens(endpoint, CryptoHandle(0))
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Installs a received per-endpoint token. The key material is
    /// indexed via its `transformation_key_id` (`remote_by_key_id`), so that
    /// [`Self::decode_data_by_key_id`] maps the submessages of this remote endpoint
    /// — independent of the participant key.
    ///
    /// # Errors
    /// `CryptoSetup` (no local slot) / `Crypto`.
    /// Optional second (payload) token of a DataWriter endpoint for the
    /// cyclone dual-key model (metadata != data, e.g. meta-sign-data). `None`
    /// = single key (all other profiles).
    #[must_use]
    pub fn endpoint_payload_token(&self, endpoint: CryptoHandle) -> Option<alloc::vec::Vec<u8>> {
        self.with_inner(|g| Ok(g.crypto.endpoint_payload_token(endpoint)))
            .ok()
            .flatten()
    }

    /// Installs a remote endpoint crypto token (volatile key exchange):
    /// passes the received token via `set_remote_participant_crypto_tokens` to
    /// the crypto plugin (participant handle `CryptoHandle(0)` = per-endpoint).
    pub fn install_remote_endpoint_token(&self, token: &[u8]) -> Result<(), SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .set_remote_participant_crypto_tokens(local, CryptoHandle(0), token)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Encodes a DATA submessage with the **per-endpoint** key (not the
    /// participant). Wire-identical to cyclone `encode_datawriter_submessage`.
    ///
    /// # Errors
    /// `Crypto` (unknown endpoint handle / encode error).
    pub fn encode_data_datawriter_by_handle(
        &self,
        endpoint: CryptoHandle,
        data_submessage: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            g.crypto
                .encode_data_datawriter_submessage(endpoint, data_submessage)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Cross-vendor user DATA (recv): decodes a SEC_* sequence with the
    /// **sender's data key** (peer slot, filled via token exchange).
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (tag mismatch).
    pub fn decode_data_datawriter_from(
        &self,
        peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "decode_data_datawriter: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .decode_data_datawriter_submessage(slot, wire)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Cross-vendor (recv) by **key_id**: decodes a SEC_* submessage without
    /// knowing the endpoint handle — the plugin looks up the remote key material
    /// by the `transformation_key_id` in the CryptoHeader (§9.5.2.1.1). Needed
    /// for protected discovery, where a peer has its own key per secure builtin
    /// endpoint.
    ///
    /// # Errors
    /// `Crypto` (no key for the key_id / tag mismatch).
    pub fn decode_data_by_key_id(&self, wire: &[u8]) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            g.crypto
                .decode_data_by_key_id(wire)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// §9.5.3.3.1 data_protection (send): encrypts the SerializedPayload
    /// of ONE endpoint (per-endpoint writer key) as the inner layer. Cross-vendor
    /// needed: with `data_protection_kind=ENCRYPT` cyclone expects the
    /// encrypted payload (`decode_serialized_payload`).
    ///
    /// # Errors
    /// `Crypto` (no key / seal error).
    pub fn encode_serialized_payload(
        &self,
        endpoint: CryptoHandle,
        payload: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            g.crypto
                .encode_serialized_payload(endpoint, payload)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// §9.5.3.3.1 data_protection (recv): key via `transformation_key_id`.
    ///
    /// # Errors
    /// `Crypto` (no key for key_id / tag mismatch).
    pub fn decode_serialized_payload(&self, encoded: &[u8]) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            g.crypto
                .decode_serialized_payload(encoded)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Prefixes of all peers with which a data key is installed (participant
    /// slot table). STABLE — unlike `completed_peer_prefixes` (derived from the
    /// handshake entries GC'd after completion), this entry persists. Source for
    /// re-sending per-endpoint crypto tokens to
    /// late-matching user endpoints.
    pub fn authenticated_peer_prefixes(&self) -> Vec<PeerKey> {
        self.with_inner(|g| Ok(g.peers.keys().copied().collect()))
            .unwrap_or_default()
    }

    /// §9.5.3.3.1 data_protection (recv) fallback: key via the **sender prefix**
    /// (GuidPrefix slot table, token exchange) instead of key_id. zero↔zero indexes
    /// the remote key material via the peer slot, not necessarily via a
    /// unique `transformation_key_id` — analogous to `decode_data_datawriter_from`.
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (tag mismatch).
    pub fn decode_serialized_payload_from(
        &self,
        peer_key: &PeerKey,
        encoded: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "decode_serialized_payload: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .decode_serialized_payload_with(slot, encoded)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// §9.5.3.3.1 data_protection (recv) fallback with the **Kx/participant key**
    /// of the peer slot (transformation_key_id=0). cyclone encrypts the
    /// data_protection payload with the SharedSecret-derived participant key
    /// instead of the per-endpoint DataWriter key — the same handle indexes the
    /// Kx key material in `kx_slots` (§10.5.2.1.2 Tab. 73).
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (tag mismatch).
    pub fn decode_serialized_payload_kx(
        &self,
        peer_key: &PeerKey,
        encoded: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let slot = g.peers.get(peer_key).copied().ok_or_else(|| {
                SecurityGateError::PolicyViolation(alloc::format!(
                    "decode_serialized_payload_kx: peer {peer_key:?} not registered"
                ))
            })?;
            g.crypto
                .decode_serialized_payload_kx(slot, encoded)
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Removes the peer-key → slot mapping. The slot itself stays
    /// in the plugin (key cleanup is the plugin's job on re-register).
    pub fn forget_remote(&self, peer_key: &PeerKey) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.peers.remove(peer_key);
            Ok(())
        })
    }

    /// Returns the slot for a peer key, if registered.
    pub fn slot_for(&self, peer_key: &PeerKey) -> Result<Option<CryptoHandle>, SecurityGateError> {
        self.with_inner(|g| Ok(g.peers.get(peer_key).copied()))
    }

    /// Inbound transform with **peer-key lookup**. The RTPS header
    /// contains the sender's GuidPrefix on bytes 8..20 — the
    /// caller must pass this here as `peer_key`.
    ///
    /// If the peer is not registered **and** the message
    /// looks encrypted: `PolicyViolation` (unknown sender
    /// sends SRTPS).
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn transform_inbound_from(
        &self,
        peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let looks_secured = wire.len() > RTPS_HEADER_LEN && wire[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = self.message_protection()?;
        if !looks_secured {
            // Passthrough or policy violation — same logic as in the
            // slot-based path.
            return if matches!(kind, ProtectionKind::None) {
                Ok(wire.to_vec())
            } else {
                Err(SecurityGateError::PolicyViolation(alloc::format!(
                    "domain requires {kind:?}, got a plain rtps message"
                )))
            };
        }
        let slot = self.slot_for(peer_key)?.ok_or_else(|| {
            SecurityGateError::PolicyViolation(alloc::format!(
                "unknown peer {peer_key:?} sends SRTPS_PREFIX"
            ))
        })?;
        self.transform_inbound(slot, wire)
    }

    /// Outbound message: wrap if governance requires message protection.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn transform_outbound(&self, message: &[u8]) -> Result<Vec<u8>, SecurityGateError> {
        match self.message_protection()? {
            ProtectionKind::None => Ok(message.to_vec()),
            _ => {
                let local = self.ensure_local()?;
                // Cyclone-conform message-level SRTPS: CryptoHeader with
                // transformation_key_id in the SRTPS_PREFIX (cross-vendor decodable).
                self.with_inner(|g| {
                    g.crypto
                        .encode_rtps_message_cyclone(local, message)
                        .map_err(SecurityGateError::Crypto)
                })
            }
        }
    }

    /// Group outbound with receiver-specific MACs.
    ///
    /// Uses [`encode_secured_submessage_multi`] when all receivers
    /// are already registered in the gate via peer keys. Returns a
    /// **single** wire datagram with a multi-MAC SEC_POSTFIX; the
    /// caller can unicast the same datagram to all matched readers.
    ///
    /// The resulting wire is NOT an RTPS message-level wrapper —
    /// it is a submessage sequence (SEC_PREFIX/BODY/POSTFIX). The
    /// caller must put the RTPS header in front itself or use the
    /// `transform_outbound_multi_wrapped` variant (follows if
    /// needed — for stage 7 the submessage sequence suffices).
    ///
    /// # Errors
    /// * `Crypto` passed through.
    /// * `PolicyViolation` if a peer key is not registered.
    pub fn transform_outbound_group(
        &self,
        peer_keys: &[PeerKey],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        // Resolve all PeerKeys to (CryptoHandle, key_id) pairs. We
        // derive the key_id deterministically from the PeerKey prefix
        // (low 32 bits of the GuidPrefix) — both sides (sender +
        // receiver) can compute it without an additional handshake.
        // The caller must have created a slot per PeerKey beforehand via
        // register_remote_by_guid.
        let bindings: Vec<(CryptoHandle, u32)> = self.with_inner(|g| {
            let mut out = Vec::with_capacity(peer_keys.len());
            for pk in peer_keys {
                let h = g.peers.get(pk).copied().ok_or_else(|| {
                    SecurityGateError::PolicyViolation(alloc::format!(
                        "transform_outbound_group: peer {pk:?} not registered"
                    ))
                })?;
                out.push((h, peer_key_to_id(pk)));
            }
            Ok(out)
        })?;
        self.with_inner(|g| {
            encode_secured_submessage_multi(&*g.crypto, local, &bindings, plaintext)
                .map_err(SecurityGateError::from)
        })
    }

    /// Group inbound: decodes a multi-MAC submessage sequence and
    /// validates **our own** MAC.
    ///
    /// `sender_peer_key` is the sender's GuidPrefix (as registered in
    /// [`Self::register_remote_by_guid`]). The receiver
    /// MAC key comes from `ensure_local()` — at encoding the sender set
    /// exactly this slot handle as a `remote_list` entry
    /// (via `register_remote_by_guid(our_prefix, our_local_token)`).
    ///
    /// # Errors
    /// * `PolicyViolation` if `sender_peer_key` is not registered.
    /// * `Crypto` / `Wrapper` on a tag/MAC mismatch.
    pub fn transform_inbound_group(
        &self,
        sender_peer_key: &PeerKey,
        own_peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let sender_slot = self.slot_for(sender_peer_key)?.ok_or_else(|| {
            SecurityGateError::PolicyViolation(alloc::format!(
                "transform_inbound_group: unknown sender {sender_peer_key:?}"
            ))
        })?;
        // The MAC-key slot source is our own local slot (when the
        // sender registered our PeerKey it stored exactly
        // our local_token → same master key).
        let own_local = self.ensure_local()?;
        let own_id = peer_key_to_id(own_peer_key);
        self.with_inner(|g| {
            decode_secured_submessage_multi(
                &*g.crypto,
                sender_slot,
                sender_slot,
                own_id,
                own_local,
                wire,
            )
            .map_err(SecurityGateError::from)
        })
    }

    /// Peer-specific outbound transform.
    ///
    /// Unlike [`Self::transform_outbound`], this
    /// method ignores the domain rule and **instead** applies the
    /// caller-given [`ProtectionLevel`]. This is the API that
    /// the heterogeneous-security writer tick (per-reader serializer)
    /// calls per matched reader.
    ///
    /// * `ProtectionLevel::None`    → plaintext passthrough
    /// * `ProtectionLevel::Sign`    → RTPS message wrap (HMAC/GCM tag)
    /// * `ProtectionLevel::Encrypt` → RTPS message wrap (AEAD ciphertext)
    ///
    /// The sign/encrypt distinction today uses the same encoder
    /// as [`Self::transform_outbound`] — the current
    /// `AesGcmCryptoPlugin` does not differentiate. Receiver-specific MACs
    /// and further extensions retrofit the distinction. The
    /// `peer_key` is carried along (for future per-peer crypto
    /// handles) but not yet passed to the plugin.
    ///
    /// # Errors
    /// See [`SecurityGateError`]. Never an error in the `None` path.
    pub fn transform_outbound_for(
        &self,
        _peer_key: &PeerKey,
        message: &[u8],
        level: ProtectionLevel,
    ) -> Result<Vec<u8>, SecurityGateError> {
        match level {
            ProtectionLevel::None => Ok(message.to_vec()),
            ProtectionLevel::Sign | ProtectionLevel::Encrypt => {
                let local = self.ensure_local()?;
                // Cyclone-conform message-level SRTPS (CryptoHeader with
                // transformation_key_id in the SRTPS_PREFIX) — identical to the
                // generic transform_outbound. The peer resolves the key by
                // key_id, so NO peer-specific keymat is needed.
                // (Previously: encode_secured_rtps_message = 16-byte null prefix,
                // which decode_rtps_message_cyclone rejects as !=20 -> the asymmetry
                // broke cross-instance/cross-vendor user DATA + 3 unit tests.)
                self.with_inner(|g| {
                    g.crypto
                        .encode_rtps_message_cyclone(local, message)
                        .map_err(SecurityGateError::Crypto)
                })
            }
        }
    }

    /// Returns the `allow_unauthenticated_participants` flag from the
    /// domain rule. Default `false` if no rule exists for the domain
    /// — conservative-safe stance.
    pub fn allow_unauthenticated(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.allow_unauthenticated_participants)
                .unwrap_or(false))
        })
    }

    /// Classifies an incoming RTPS datagram against the domain rule,
    /// peer registration, wire format and network interface
    ///.
    ///
    /// Decision matrix:
    ///
    /// 1. `bytes.len() < 20` → `Malformed`.
    /// 2. Extract `peer_key` from `bytes[8..20]`.
    /// 3. If the packet is SRTPS-wrapped → standard unwrap path
    ///    (`transform_inbound_from`). On a crypto error `CryptoError`,
    ///    on an unknown peer `PolicyViolation`.
    /// 4. If the packet is plain AND the domain requires ProtectionKind::None
    ///    → `Accept`.
    /// 5. If the packet is plain AND the domain requires protection:
    ///    * the interface is `Loopback` or `LocalHost` → `Accept`
    ///      (bytes do not leave the host kernel — spec-conform
    ///      plaintext on host-local transport)
    ///    * `allow_unauthenticated_participants = true` → `Accept`
    ///    * otherwise → `LegacyBlocked`
    ///
    /// The `iface` context is currently consulted in the rules only for the
    /// loopback fast path; the finer per-interface peer/topic
    /// classification is handled by the `PolicyEngine`
    /// from stage 8 on (governance-XML `<interface_bindings>`).
    #[must_use]
    pub fn classify_inbound(&self, bytes: &[u8], iface: &NetInterface) -> InboundVerdict {
        if bytes.len() < RTPS_HEADER_LEN + 8 {
            return InboundVerdict::Malformed;
        }
        let mut peer_key = [0u8; 12];
        peer_key.copy_from_slice(&bytes[8..20]);

        let looks_secured = bytes.len() > RTPS_HEADER_LEN && bytes[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = match self.message_protection() {
            Ok(k) => k,
            Err(e) => {
                return InboundVerdict::CryptoError(alloc::format!("gate lookup failed: {e:?}"));
            }
        };

        if looks_secured {
            return match self.transform_inbound_from(&peer_key, bytes) {
                Ok(clear) => InboundVerdict::Accept(clear),
                Err(SecurityGateError::PolicyViolation(msg)) => {
                    InboundVerdict::PolicyViolation(msg)
                }
                Err(e) => InboundVerdict::CryptoError(alloc::format!("{e:?}")),
            };
        }

        // A plain packet came in.
        if matches!(kind, ProtectionKind::None) {
            return InboundVerdict::Accept(bytes.to_vec());
        }
        // Loopback / LocalHost: bytes do not leave the host, so
        // plain is functionally OK — matches arch doc §2.1 "intra-
        // host loopback: plain (does not leave the network)".
        if matches!(iface, NetInterface::Loopback | NetInterface::LocalHost) {
            return InboundVerdict::Accept(bytes.to_vec());
        }
        // The domain requires protection, the peer sent plain on a remote
        // interface.
        match self.allow_unauthenticated() {
            Ok(true) => InboundVerdict::Accept(bytes.to_vec()),
            Ok(false) => InboundVerdict::LegacyBlocked,
            Err(e) => InboundVerdict::CryptoError(alloc::format!("gate lookup failed: {e:?}")),
        }
    }

    /// Inbound message: unwrap if SRTPS_PREFIX is detected, otherwise
    /// passthrough or PolicyViolation.
    ///
    /// `remote_slot` points to the slot in which the sender key
    /// is stored (from [`Self::register_remote_with_token`]).
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn transform_inbound(
        &self,
        remote_slot: CryptoHandle,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let looks_secured = wire.len() > RTPS_HEADER_LEN && wire[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = self.message_protection()?;
        match (kind, looks_secured) {
            (ProtectionKind::None, false) => Ok(wire.to_vec()),
            (_, true) => self.with_inner(|g| {
                // key_id-based SRTPS decode (cyclone-conform); remote_slot
                // stays for the signature contract, the key comes via key_id.
                let _ = remote_slot;
                g.crypto
                    .decode_rtps_message_cyclone(wire)
                    .map_err(SecurityGateError::Crypto)
            }),
            (_, false) => Err(SecurityGateError::PolicyViolation(alloc::format!(
                "domain requires {kind:?}, got a plain rtps message"
            ))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::thread;
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;

    const GOV_RTPS: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;

    fn fake_msg(body: &[u8]) -> Vec<u8> {
        let mut m = Vec::with_capacity(20 + body.len());
        m.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        m.extend_from_slice(&[0u8; 12]);
        m.extend_from_slice(body);
        m
    }

    #[test]
    fn outbound_none_is_passthrough() {
        let gov = parse_governance_xml(
            r#"<domain_access_rules><domain_rule><domains><id>0</id></domains><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules>"#,
        )
        .unwrap();
        let gate = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));
        let msg = fake_msg(b"x");
        assert_eq!(gate.transform_outbound(&msg).unwrap(), msg);
    }

    #[test]
    fn per_endpoint_datawriter_token_roundtrips_by_key_id() {
        // §9.5.3.3: each DataWriter has its OWN key material; the matched
        // reader installs it via datawriter_crypto_tokens + decodes the
        // submessage via the transformation_key_id — NOT via the
        // participant key. Deterministic per-endpoint crypto path (replaces
        // the flat participant dump that caused the keys-before-data race).
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let bob = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );

        // Alice: local writer endpoint with its own key (NOT participant).
        let aw = alice.register_local_endpoint(true).unwrap();
        // Alice → Bob: the per-endpoint datawriter_crypto_token.
        let token = alice.create_endpoint_token(aw).unwrap();
        bob.install_remote_endpoint_token(&token).unwrap();

        // Alice encodes a DATA submessage with the ENDPOINT key.
        let plain = fake_msg(b"[DATA:per-endpoint]");
        let wire = alice.encode_data_datawriter_by_handle(aw, &plain).unwrap();
        // Bob decodes it exclusively via the key_id in the CryptoHeader.
        let back = bob.decode_data_by_key_id(&wire).unwrap();
        assert_eq!(back, plain, "per-endpoint key round-trips via key_id");
    }

    #[test]
    fn topic_discovery_protected_reads_topic_rule_flag() {
        // §9.4.2.4: the endpoint bit IS_DISCOVERY_PROTECTED comes from the
        // TOPIC rule `enable_discovery_protection` (boolean) — NOT from the
        // domain-wide discovery_protection_kind. cyclone sets it for
        // enable_discovery_protection=true; ZeroDDS must announce the same mask
        // (otherwise "security_attributes mismatch").
        const GOV_DISC: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
    <topic_access_rules><topic_rule>
      <topic_expression>*</topic_expression>
      <enable_discovery_protection>true</enable_discovery_protection>
    </topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let on = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_DISC).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(
            on.topic_discovery_protected().unwrap(),
            "enable_discovery_protection=true ⟹ endpoint IS_DISCOVERY_PROTECTED"
        );
        // GOV_RTPS has NO enable_discovery_protection → false.
        let off = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!off.topic_discovery_protected().unwrap());
    }

    #[test]
    fn e2e_alice_bob_with_shared_gate() {
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let bob = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let alice_token = alice.local_token().unwrap();
        let bob_view_of_alice = bob
            .register_remote_with_token(IdentityHandle(1), SharedSecretHandle(1), &alice_token)
            .unwrap();

        let plain = fake_msg(b"[DATA:shared]");
        let wire = alice.transform_outbound(&plain).unwrap();
        let back = bob.transform_inbound(bob_view_of_alice, &wire).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn clone_shares_same_plugin_instance() {
        // Clone creates a second gate handle on THE SAME plugin.
        // `ensure_local` by clone1 creates the local slot; clone2
        // sees the same session ID.
        let gate1 = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let gate2 = gate1.clone();
        let t1 = gate1.local_token().unwrap();
        let t2 = gate2.local_token().unwrap();
        assert_eq!(t1, t2, "both clones see the same local slot");
    }

    #[test]
    fn concurrent_transform_is_thread_safe() {
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let mut handles = Vec::new();
        for i in 0..8 {
            let g = alice.clone();
            handles.push(thread::spawn(move || {
                let m = fake_msg(alloc::format!("[DATA:{i}]").as_bytes());
                // Must serialize WITHOUT panic — the nonce counter stays
                // thread-safe (AtomicU64 in the key material).
                let _ = g.transform_outbound(&m).unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn plain_inbound_on_protected_domain_is_policy_violation() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let plain = fake_msg(b"nope");
        let err = gate
            .transform_inbound(CryptoHandle(99), &plain)
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn domain_id_reflects_constructor() {
        let gate = SharedSecurityGate::new(
            7,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert_eq!(gate.domain_id().unwrap(), 7);
    }

    // -------------------------------------------------------------
    // RC1.4 preparation — peer-key mapping
    // -------------------------------------------------------------

    fn build_pair() -> (SharedSecurityGate, SharedSecurityGate) {
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let bob = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        (alice, bob)
    }

    #[test]
    fn register_remote_by_guid_is_idempotent() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        let atoken = alice.local_token().unwrap();
        let slot1 = bob
            .register_remote_by_guid(
                alice_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &atoken,
            )
            .unwrap();
        let slot2 = bob
            .register_remote_by_guid(
                alice_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &atoken,
            )
            .unwrap();
        assert_eq!(slot1, slot2, "idempotent: same guid prefix → same slot");
    }

    #[test]
    fn transform_inbound_from_looks_up_slot_by_guid() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        let atoken = alice.local_token().unwrap();
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &atoken,
        )
        .unwrap();

        let msg = fake_msg(b"[DATA:guid-lookup]");
        let wire = alice.transform_outbound(&msg).unwrap();
        let back = bob.transform_inbound_from(&alice_prefix, &wire).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn transform_inbound_from_unknown_peer_is_policy_violation() {
        let (alice, bob) = build_pair();
        // Alice does NOT register with Bob.
        let msg = fake_msg(b"x");
        let wire = alice.transform_outbound(&msg).unwrap();
        let err = bob.transform_inbound_from(&[0xCC; 12], &wire).unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn multi_peer_mapping_routes_correctly() {
        // Alice + Charlie send to Bob. Bob must distinguish the two by
        // GuidPrefix.
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let alice = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let charlie = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let bob = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [1u8; 12];
        let charlie_prefix: PeerKey = [3u8; 12];

        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        bob.register_remote_by_guid(
            charlie_prefix,
            IdentityHandle(3),
            SharedSecretHandle(3),
            &charlie.local_token().unwrap(),
        )
        .unwrap();

        let m_alice = fake_msg(b"from-alice");
        let m_charlie = fake_msg(b"from-charlie");
        let w_alice = alice.transform_outbound(&m_alice).unwrap();
        let w_charlie = charlie.transform_outbound(&m_charlie).unwrap();

        assert_eq!(
            bob.transform_inbound_from(&alice_prefix, &w_alice).unwrap(),
            m_alice
        );
        assert_eq!(
            bob.transform_inbound_from(&charlie_prefix, &w_charlie)
                .unwrap(),
            m_charlie
        );
    }

    #[test]
    fn tampered_wire_fails_tag_verify() {
        // Since the per-key_id resolution (transform_inbound resolves the key
        // via the transformation_key_id in the CryptoHeader, NOT via the
        // prefix-chosen slot) a wrong-prefix-but-valid wire decodes
        // correctly — Bob HAS Alice's key, the key_id points to it. This is intended
        // (sender attribution by crypto identity, not by a header prefix hint).
        // The SECURITY PROPERTY is GCM tag integrity: a tampered
        // wire MUST fail. That is exactly what this test verifies.
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [1u8; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();

        let msg = fake_msg(b"from-alice");
        let wire = alice.transform_outbound(&msg).unwrap();
        // Legit: an unchanged wire decodes correctly.
        assert_eq!(
            bob.transform_inbound_from(&alice_prefix, &wire).unwrap(),
            msg
        );

        // Tampered: flip one byte in the common_mac (GCM tag, in the SRTPS_POSTFIX).
        // The last 4 bytes are receiver_specific_macs._length(=0); the 16-byte
        // common_mac lies before that -> len-6 hits the tag. AES-GCM open MUST then
        // abort with a tag mismatch (Crypto) or a structure error (Wrapper).
        assert!(
            wire.len() > 30,
            "SRTPS wire too short for the tamper offset"
        );
        let mut tampered = wire.clone();
        let idx = tampered.len() - 6;
        tampered[idx] ^= 0xff;
        let err = bob
            .transform_inbound_from(&alice_prefix, &tampered)
            .unwrap_err();
        assert!(matches!(
            err,
            SecurityGateError::Wrapper(_) | SecurityGateError::Crypto(_)
        ));
    }

    // -------------------------------------------------------------
    // RC1 stage 7 — receiver-specific MACs in the gate E2E
    // -------------------------------------------------------------

    #[test]
    fn group_transform_one_ciphertext_three_macs_each_reader_decodes() {
        // DoD §stage 7 verbatim: 1 writer, 3 readers same suite,
        // different tokens → one ciphertext + 3 MACs; each
        // reader validates its own MAC.
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let alice = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let bob = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let charlie = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let dave = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let bob_prefix: PeerKey = [0xB1; 12];
        let charlie_prefix: PeerKey = [0xC1; 12];
        let dave_prefix: PeerKey = [0xD1; 12];
        let alice_prefix: PeerKey = [0xA1; 12];

        // Alice registers all three receivers with their **own**
        // session keys (this is the per-reader SharedSecret).
        alice
            .register_remote_by_guid(
                bob_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &bob.local_token().unwrap(),
            )
            .unwrap();
        alice
            .register_remote_by_guid(
                charlie_prefix,
                IdentityHandle(2),
                SharedSecretHandle(2),
                &charlie.local_token().unwrap(),
            )
            .unwrap();
        alice
            .register_remote_by_guid(
                dave_prefix,
                IdentityHandle(3),
                SharedSecretHandle(3),
                &dave.local_token().unwrap(),
            )
            .unwrap();

        // Each receiver registers Alice under her Alice prefix —
        // so `transform_inbound_group` finds the sender side
        // for the AES-GCM unwrap.
        for recv in [&bob, &charlie, &dave] {
            recv.register_remote_by_guid(
                alice_prefix,
                IdentityHandle(10),
                SharedSecretHandle(10),
                &alice.local_token().unwrap(),
            )
            .unwrap();
        }

        // Encode: 1 Ciphertext + 3 MACs.
        let plain = b"hetero-broadcast-e2e";
        let wire = alice
            .transform_outbound_group(&[bob_prefix, charlie_prefix, dave_prefix], plain)
            .unwrap();

        // Each receiver decodes its variant identically — with
        // its own PeerKey as the match ID.
        let out_bob = bob
            .transform_inbound_group(&alice_prefix, &bob_prefix, &wire)
            .unwrap();
        let out_charlie = charlie
            .transform_inbound_group(&alice_prefix, &charlie_prefix, &wire)
            .unwrap();
        let out_dave = dave
            .transform_inbound_group(&alice_prefix, &dave_prefix, &wire)
            .unwrap();
        assert_eq!(out_bob, plain);
        assert_eq!(out_charlie, plain);
        assert_eq!(out_dave, plain);
    }

    #[test]
    fn group_transform_rogue_receiver_without_mac_rejects() {
        // A 4th receiver (Eve) is NOT in Alice's MAC list.
        // Eve obtained Alice's token via a side channel (or
        // eavesdropping) and tries to decode. Her own HMAC
        // key in Eve's `ensure_local()` slot matches no MAC
        // entry → crypto fail.
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let alice = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let bob = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let eve = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [0xA2; 12];
        let bob_prefix: PeerKey = [0xB2; 12];
        alice
            .register_remote_by_guid(
                bob_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &bob.local_token().unwrap(),
            )
            .unwrap();
        // Eve knows Alice's token (attacker scenario), registers her.
        eve.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(10),
            SharedSecretHandle(10),
            &alice.local_token().unwrap(),
        )
        .unwrap();

        let wire = alice
            .transform_outbound_group(&[bob_prefix], b"confidential")
            .unwrap();

        let eve_prefix: PeerKey = [0xEE; 12];
        let err = eve
            .transform_inbound_group(&alice_prefix, &eve_prefix, &wire)
            .unwrap_err();
        assert!(
            matches!(
                err,
                SecurityGateError::Crypto(_) | SecurityGateError::Wrapper(_)
            ),
            "Eve without a MAC entry must drop, got: {err:?}"
        );
    }

    #[test]
    fn group_transform_unknown_peer_is_policy_violation() {
        // Alice tries to encode for an unregistered peer
        // → PolicyViolation.
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let unregistered: PeerKey = [0x99; 12];
        let err = alice
            .transform_outbound_group(&[unregistered], b"x")
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn forget_remote_removes_mapping() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        assert!(bob.slot_for(&alice_prefix).unwrap().is_some());
        bob.forget_remote(&alice_prefix).unwrap();
        assert!(bob.slot_for(&alice_prefix).unwrap().is_none());
    }

    // -------------------------------------------------------------
    // RC1 stage 4a — transform_outbound_for
    // -------------------------------------------------------------

    #[test]
    fn transform_outbound_for_none_is_passthrough_even_on_protected_domain() {
        // The domain is ENCRYPT per governance, but the caller requests
        // per-reader None — must deliver plaintext (this is the
        // heterogeneous case).
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let peer_key: PeerKey = [0xBB; 12];
        let msg = fake_msg(b"[plain-for-legacy]");
        let out = gate
            .transform_outbound_for(&peer_key, &msg, ProtectionLevel::None)
            .unwrap();
        assert_eq!(out, msg, "None level must passthrough byte-identical");
    }

    #[test]
    fn transform_outbound_for_encrypt_produces_srtps_wire() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let peer_key: PeerKey = [0xCC; 12];
        let msg = fake_msg(b"[enc-for-secure]");
        let wire = gate
            .transform_outbound_for(&peer_key, &msg, ProtectionLevel::Encrypt)
            .unwrap();
        // The output must be longer than plain (SRTPS overhead) and start at the
        // SRTPS_PREFIX byte after the RTPS header.
        assert!(wire.len() > msg.len());
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
    }

    #[test]
    fn transform_outbound_for_sign_also_uses_srtps_encoder() {
        // The sign level today uses the same encoder as Encrypt (v1.4
        // plugin status). All that matters: the output is NOT plaintext.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let peer_key: PeerKey = [0xDD; 12];
        let msg = fake_msg(b"[sig-for-fast]");
        let wire = gate
            .transform_outbound_for(&peer_key, &msg, ProtectionLevel::Sign)
            .unwrap();
        assert_ne!(wire, msg, "Sign must not be byte-identical to plain");
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
    }

    #[test]
    fn transform_outbound_for_heterogeneous_three_readers() {
        // 1 writer → 3 readers (legacy/sign/encrypt). Each output is
        // individually different — that is the core of RC1.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[broadcast]");
        let legacy = gate
            .transform_outbound_for(&[1; 12], &msg, ProtectionLevel::None)
            .unwrap();
        let fast = gate
            .transform_outbound_for(&[2; 12], &msg, ProtectionLevel::Sign)
            .unwrap();
        let secure = gate
            .transform_outbound_for(&[3; 12], &msg, ProtectionLevel::Encrypt)
            .unwrap();
        assert_eq!(legacy, msg, "the legacy reader gets plain");
        assert_ne!(fast, msg, "the fast reader gets SRTPS-wrapped");
        assert_ne!(secure, msg, "the secure reader gets SRTPS-wrapped");
        // Sign and encrypt packets are not byte-identical — even
        // when the same encoder is used, each encode uses a
        // fresh nonce counter.
        assert_ne!(fast, secure, "per-reader encoding must differ each time");
    }

    // -------------------------------------------------------------
    // RC1 stage 5 — classify_inbound + allow_unauthenticated
    // -------------------------------------------------------------

    const GOV_NONE: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>NONE</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
    const GOV_ENCRYPT_ALLOW_UNAUTH: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <allow_unauthenticated_participants>TRUE</allow_unauthenticated_participants>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;

    #[test]
    fn allow_unauthenticated_default_false_without_element() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!gate.allow_unauthenticated().unwrap());
    }

    #[test]
    fn allow_unauthenticated_reads_true_when_set() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_ENCRYPT_ALLOW_UNAUTH).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(gate.allow_unauthenticated().unwrap());
    }

    #[test]
    fn allow_unauthenticated_defaults_false_for_unknown_domain() {
        let gate = SharedSecurityGate::new(
            99,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!gate.allow_unauthenticated().unwrap());
    }

    #[test]
    fn classify_inbound_rejects_truncated_datagram() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_NONE).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let verdict = gate.classify_inbound(&[0u8; 10], &NetInterface::Wan);
        assert_eq!(verdict, InboundVerdict::Malformed);
        assert_eq!(verdict.category(), "inbound.malformed");
    }

    #[test]
    fn classify_inbound_plain_on_none_domain_accepts() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_NONE).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[plain-hello]");
        match gate.classify_inbound(&msg, &NetInterface::Wan) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Accept, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_plain_on_protected_domain_is_legacy_blocked() {
        // The domain requires ENCRYPT, allow_unauth = false (default).
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[legacy-on-encrypted]");
        let verdict = gate.classify_inbound(&msg, &NetInterface::Wan);
        assert_eq!(verdict, InboundVerdict::LegacyBlocked);
        assert_eq!(verdict.category(), "inbound.legacy_blocked");
        assert!(!verdict.is_accept());
    }

    #[test]
    fn classify_inbound_plain_on_protected_domain_with_allow_unauth_accepts() {
        // DoD test: a legacy peer is accepted when governance
        // explicitly allows it.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_ENCRYPT_ALLOW_UNAUTH).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[legacy-allowed]");
        match gate.classify_inbound(&msg, &NetInterface::Wan) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Accept (allow_unauthenticated=true), got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_plain_on_loopback_accepts_even_on_protected_domain() {
        // Arch doc §2.1: "intra-host loopback: plain (does not leave the
        // network)". Protected domain, but interface=Loopback →
        // plaintext is accepted spec-conform.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[loopback-plain]");
        match gate.classify_inbound(&msg, &NetInterface::Loopback) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Loopback-Accept, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_srtps_from_unknown_peer_is_policy_violation() {
        // No register_remote_by_guid — the peer is unknown.
        let (alice, bob) = build_pair();
        // Alice sends us an SRTPS wrapper, but bob has not registered
        // alice → classify must report PolicyViolation.
        let msg = fake_msg(b"[from-unknown]");
        let wire = alice.transform_outbound(&msg).unwrap();
        let verdict = bob.classify_inbound(&wire, &NetInterface::Wan);
        assert!(
            matches!(verdict, InboundVerdict::PolicyViolation(_)),
            "expected PolicyViolation, got {verdict:?}"
        );
        assert_eq!(verdict.category(), "inbound.policy_violation");
    }

    #[test]
    fn classify_inbound_srtps_from_known_peer_accepts() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        let msg = fake_msg(b"[authed-peer]");
        // The sender must carry the same GuidPrefix in the header so
        // classify_inbound finds the peer key.
        let mut hdr_msg = Vec::with_capacity(msg.len());
        hdr_msg.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        hdr_msg.extend_from_slice(&alice_prefix);
        hdr_msg.extend_from_slice(b"payload-body");
        let wire = alice.transform_outbound(&hdr_msg).unwrap();
        match bob.classify_inbound(&wire, &NetInterface::Wan) {
            InboundVerdict::Accept(_) => {}
            other => panic!("expected Accept, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_srtps_with_wrong_key_is_crypto_error() {
        // Alice + Charlie encode with different keys; Bob has
        // Alice registered but gets Charlie's bytes under Alice's
        // peer_key → crypto tag mismatch.
        let (alice, bob) = build_pair();
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let charlie = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();

        // Charlie encodes with Alice's prefix in the header (MITM simulation).
        let mut body = Vec::new();
        body.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        body.extend_from_slice(&alice_prefix);
        body.extend_from_slice(b"mitm-try");
        let spoofed = charlie.transform_outbound(&body).unwrap();

        let verdict = bob.classify_inbound(&spoofed, &NetInterface::Wan);
        assert!(
            matches!(verdict, InboundVerdict::CryptoError(_)),
            "expected CryptoError, got {verdict:?}"
        );
        assert_eq!(verdict.category(), "inbound.crypto_error");
    }

    #[test]
    fn transform_outbound_for_is_decodable_with_registered_token() {
        // E2E: Alice serializes via `transform_outbound_for`, Bob
        // registers Alice's token and decodes successfully.
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        let msg = fake_msg(b"[hetero-e2e]");
        let wire = alice
            .transform_outbound_for(&[9; 12], &msg, ProtectionLevel::Encrypt)
            .unwrap();
        let back = bob.transform_inbound_from(&alice_prefix, &wire).unwrap();
        assert_eq!(back, msg);
    }

    // -------------------------------------------------------------
    // FU2 S1.2 — dual-key at the gate: Kx channel (VolatileSecure) +
    // data key via token. Both from the same handshake secret.
    // -------------------------------------------------------------

    struct FixedSecret;
    impl zerodds_security::authentication::SharedSecretProvider for FixedSecret {
        fn get_shared_secret(
            &self,
            _h: zerodds_security::authentication::SharedSecretHandle,
        ) -> Option<Vec<u8>> {
            Some(alloc::vec![0x77u8; 32])
        }
    }

    fn build_kx_pair() -> (SharedSecurityGate, SharedSecurityGate) {
        let mk = || {
            SharedSecurityGate::new(
                0,
                parse_governance_xml(GOV_RTPS).unwrap(),
                Box::new(AesGcmCryptoPlugin::with_secret_provider(
                    zerodds_security_crypto::Suite::Aes128Gcm,
                    Arc::new(FixedSecret)
                        as Arc<dyn zerodds_security::authentication::SharedSecretProvider>,
                )),
            )
        };
        (mk(), mk())
    }

    #[test]
    fn kx_channel_round_trips_through_gate() {
        let (alice, bob) = build_kx_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        let bob_prefix: PeerKey = [0xBB; 12];
        alice
            .register_remote_by_guid_from_secret(
                bob_prefix,
                IdentityHandle(2),
                SharedSecretHandle(1),
            )
            .unwrap();
        bob.register_remote_by_guid_from_secret(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
        )
        .unwrap();
        // VolatileSecure payload (ParticipantCryptoToken) Kx-protected.
        let token_blob = b"participant-crypto-token-payload";
        let wire = alice
            .transform_kx_outbound_for(&bob_prefix, token_blob)
            .unwrap();
        let back = bob.transform_kx_inbound_from(&alice_prefix, &wire).unwrap();
        assert_eq!(back, token_blob);
    }

    #[test]
    fn data_round_trips_via_token_after_kx_register() {
        let (alice, bob) = build_kx_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        let bob_prefix: PeerKey = [0xBB; 12];
        // Both register the Kx key (no token).
        alice
            .register_remote_by_guid_from_secret(
                bob_prefix,
                IdentityHandle(2),
                SharedSecretHandle(1),
            )
            .unwrap();
        bob.register_remote_by_guid_from_secret(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
        )
        .unwrap();
        // Data token exchange: bob installs alice's local data key.
        bob.set_remote_data_token_by_guid(&alice_prefix, &alice.local_token().unwrap())
            .unwrap();
        // Secured DATA: alice encrypts (local key), bob decrypts (token key).
        let msg = fake_msg(b"[secured-user-data]");
        let wire = alice
            .transform_outbound_for(&bob_prefix, &msg, ProtectionLevel::Encrypt)
            .unwrap();
        let back = bob.transform_inbound_from(&alice_prefix, &wire).unwrap();
        assert_eq!(back, msg);
    }
}
