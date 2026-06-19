// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds-lint: allow no_dyn_in_safe
// Rationale: the DDS-Security plugins (Authentication/AccessControl/Crypto)
// are chosen config-driven at runtime (PKI vs. stub, etc.) — that is
// inherently dynamic polymorphism. `dyn ...Plugin` behind Arc<Mutex<>> is
// here the spec-conformant plugin pattern, not replaceable by concrete generics.
//! `SecurityBuiltinStack` — bundles the two security builtin topic-
//! Endpoint pairs in one structure.
//!
//! - `DCPSParticipantStatelessMessage` (auth handshake, BestEffort).
//! - `DCPSParticipantVolatileMessageSecure` (crypto key exchange, Reliable).
//!
//! Instantiated by the participant wiring (DCPS layer) as soon as a
//! security plugin is registered and discovery bits 22..25 are announced
//! in the `BuiltinEndpointSet`. The stack maintains the reader/writer
//! proxies per remote participant — `handle_remote_endpoints` is called
//! from the SPDP hot path as soon as a peer with the corresponding bits
//! is discovered.

extern crate alloc;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::error::WireError;
use zerodds_rtps::message_builder::OutboundDatagram;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

use crate::capabilities::PeerCapabilities;
use crate::security::stateless::{StatelessMessageReader, StatelessMessageWriter};
use crate::security::volatile_secure::{VolatileSecureMessageReader, VolatileSecureMessageWriter};
use crate::spdp::DiscoveredParticipant;

#[cfg(feature = "std")]
use alloc::collections::BTreeMap;
#[cfg(feature = "std")]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(feature = "std")]
use zerodds_security::authentication::{
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle,
};
#[cfg(feature = "std")]
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
#[cfg(feature = "std")]
use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
#[cfg(feature = "std")]
use zerodds_security::token::DataHolder;

/// Result of a processed incoming handshake step: outbound datagrams
/// plus — once derived on this side — the `(remote_identity,
/// shared_secret)` tuple for the crypto gate.
#[cfg(feature = "std")]
pub type HandshakeStepResult = (
    Vec<OutboundDatagram>,
    Option<(IdentityHandle, SharedSecretHandle)>,
);

/// Locks the shared auth plugin; a poisoned mutex (panic in another
/// handshake thread) is reported as an `Internal` error instead of
/// panicking itself.
#[cfg(feature = "std")]
fn lock_auth<'a>(
    auth: &'a Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
) -> SecurityResult<std::sync::MutexGuard<'a, dyn AuthenticationPlugin + 'static>> {
    auth.lock()
        .map_err(|_| SecurityError::new(SecurityErrorKind::Internal, "auth mutex poisoned"))
}

/// Per-peer state of the auth handshake (FU2 driver).
///
/// Created as soon as a peer with stateless bits is discovered via SPDP
/// and `begin_handshake_with` has been called. The `handshake` handle
/// only exists after the first plugin step (initiator:
/// `begin_handshake_request`, replier: `begin_handshake_reply`).
#[cfg(feature = "std")]
#[derive(Debug)]
struct PeerHandshake {
    /// Validated remote identity (from `validate_remote_identity`).
    remote_identity: IdentityHandle,
    /// 16-byte remote participant GUID (destination of the tokens).
    remote_guid: [u8; 16],
    /// Running handshake in the plugin — `None` until the first step.
    handshake: Option<HandshakeHandle>,
    /// Sequence-number counter for outbound `message_identity`.
    next_sn: i64,
    /// Secret already reported to the caller — idempotency guard.
    secret: Option<SharedSecretHandle>,
    /// FU2 S3: last sent initiator message (AUTH_REQUEST). Re-emitted on
    /// every repeated `begin_handshake_with` (periodic SPDP beacon) as
    /// long as the secret is missing — makes the best-effort stateless
    /// handshake robust against lost initial messages.
    last_request: Option<ParticipantGenericMessage>,
}

/// Bundle of the four security builtin endpoints.
pub struct SecurityBuiltinStack {
    local_prefix: GuidPrefix,
    /// Stateless auth writer (Spec §7.4.4).
    pub stateless_writer: StatelessMessageWriter,
    /// Stateless auth reader.
    pub stateless_reader: StatelessMessageReader,
    /// Volatile-Secure writer (Spec §7.4.5).
    pub volatile_writer: VolatileSecureMessageWriter,
    /// Volatile-Secure reader.
    pub volatile_reader: VolatileSecureMessageReader,
    /// Auth plugin for the identity handshake. `None` = pure proxy
    /// plumbing without a handshake driver (backward-compat). Shared via
    /// `Arc<Mutex>` with the crypto plugin's `SharedSecretProvider`
    /// (security-runtime Gap 1), so that the secret derived after the
    /// handshake is resolvable there.
    #[cfg(feature = "std")]
    auth: Option<Arc<Mutex<dyn AuthenticationPlugin>>>,
    /// Local validated identity (from `validate_local_identity`).
    #[cfg(feature = "std")]
    local_identity: Option<IdentityHandle>,
    /// Local 16-byte participant GUID (source of the `message_identity`).
    #[cfg(feature = "std")]
    local_guid: [u8; 16],
    /// Handshake state per remote participant.
    #[cfg(feature = "std")]
    handshakes: BTreeMap<GuidPrefix, PeerHandshake>,
    /// Peer `VendorId` per remote prefix (from the SPDP RTPS header). Controls
    /// vendor-specific handshake quirks (e.g. OpenDDS' NUL-terminated
    /// `c.dsign_algo`/`c.kagree_algo`). Maintained by the discovery layer via
    /// [`Self::note_remote_vendor`].
    #[cfg(feature = "std")]
    remote_vendors: BTreeMap<GuidPrefix, VendorId>,
    /// Timestamp of the last handshake resend (throttle). The
    /// `DCPSParticipantStatelessMessage` writer is per spec (DDS-Security
    /// §7.4.4) BEST_EFFORT; the reliability of the 3-message handshake
    /// comes from periodic re-send of the pending message until the secret
    /// is established. That is the spec protocol, NOT a workaround for the
    /// (separately fixed) discovery coupling.
    #[cfg(feature = "std")]
    last_handshake_resend: Duration,
}

impl core::fmt::Debug for SecurityBuiltinStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut dbg = f.debug_struct("SecurityBuiltinStack");
        dbg.field("local_prefix", &self.local_prefix)
            .field("stateless_writer", &self.stateless_writer)
            .field("stateless_reader", &self.stateless_reader)
            .field("volatile_writer", &self.volatile_writer)
            .field("volatile_reader", &self.volatile_reader);
        #[cfg(feature = "std")]
        dbg.field("auth", &self.auth.is_some())
            .field("local_identity", &self.local_identity)
            .field("handshakes", &self.handshakes);
        dbg.finish()
    }
}

impl SecurityBuiltinStack {
    /// Creates a fresh stack without remote proxies and without a
    /// handshake driver (pure proxy plumbing, backward-compat).
    #[must_use]
    pub fn new(local_prefix: GuidPrefix, vendor_id: VendorId) -> Self {
        Self {
            local_prefix,
            stateless_writer: StatelessMessageWriter::new(local_prefix, vendor_id),
            stateless_reader: StatelessMessageReader::new(local_prefix, vendor_id),
            volatile_writer: VolatileSecureMessageWriter::new(local_prefix, vendor_id),
            volatile_reader: VolatileSecureMessageReader::new(local_prefix, vendor_id),
            #[cfg(feature = "std")]
            auth: None,
            #[cfg(feature = "std")]
            local_identity: None,
            #[cfg(feature = "std")]
            local_guid: [0; 16],
            #[cfg(feature = "std")]
            handshakes: BTreeMap::new(),
            #[cfg(feature = "std")]
            remote_vendors: BTreeMap::new(),
            #[cfg(feature = "std")]
            last_handshake_resend: Duration::ZERO,
        }
    }

    /// Creates a stack WITH a handshake driver (FU2). The `auth` plugin
    /// is shared via `Arc<Mutex>` — the same instance must be attached as
    /// the `SharedSecretProvider` on the crypto plugin (security-runtime
    /// Gap 1), so that the secret returned after `Complete` is resolvable
    /// there.
    ///
    /// `local_identity` comes from `validate_local_identity`,
    /// `local_guid` is the 16-byte participant GUID of this stack.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn with_auth(
        local_prefix: GuidPrefix,
        vendor_id: VendorId,
        auth: Arc<Mutex<dyn AuthenticationPlugin>>,
        local_identity: IdentityHandle,
        local_guid: [u8; 16],
    ) -> Self {
        Self {
            local_prefix,
            stateless_writer: StatelessMessageWriter::new(local_prefix, vendor_id),
            stateless_reader: StatelessMessageReader::new(local_prefix, vendor_id),
            volatile_writer: VolatileSecureMessageWriter::new(local_prefix, vendor_id),
            volatile_reader: VolatileSecureMessageReader::new(local_prefix, vendor_id),
            auth: Some(auth),
            local_identity: Some(local_identity),
            local_guid,
            handshakes: BTreeMap::new(),
            remote_vendors: BTreeMap::new(),
            last_handshake_resend: Duration::ZERO,
        }
    }

    /// Local GuidPrefix.
    #[must_use]
    pub fn local_prefix(&self) -> GuidPrefix {
        self.local_prefix
    }

    /// Wires reader/writer proxies based on the BuiltinEndpointSet bits
    /// announced by the peer (Spec §7.4.7.1):
    ///
    /// - Bits 22+23 (`PARTICIPANT_STATELESS_MESSAGE_*`) → stateless slot
    /// - Bits 24+25 (`PARTICIPANT_VOLATILE_MESSAGE_SECURE_*`) → volatile slot
    ///
    /// We route over `metatraffic_unicast_locator` (PID 0x0032),
    /// falling back to `default_unicast_locator`. Self-discovery
    /// (`peer.sender_prefix == self.local_prefix`) is ignored.
    pub fn handle_remote_endpoints(&mut self, peer: &DiscoveredParticipant) {
        if peer.sender_prefix == self.local_prefix {
            return;
        }
        let caps = PeerCapabilities::from_bits(peer.data.builtin_endpoint_set);
        if !caps.has_stateless_auth && !caps.has_volatile_secure {
            return;
        }
        let unicast: Vec<Locator> = peer
            .data
            .metatraffic_unicast_locator
            .or(peer.data.default_unicast_locator)
            .into_iter()
            .collect();
        let remote_prefix = peer.sender_prefix;

        if caps.has_stateless_auth {
            self.stateless_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
                ),
                unicast.clone(),
                Vec::new(),
                false,
            ));
            self.stateless_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
                ),
                unicast.clone(),
                Vec::new(),
                false,
            ));
        }

        if caps.has_volatile_secure {
            self.volatile_writer.add_reader_proxy(ReaderProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
                ),
                unicast.clone(),
                Vec::new(),
                true,
            ));
            self.volatile_reader.add_writer_proxy(WriterProxy::new(
                Guid::new(
                    remote_prefix,
                    EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
                ),
                unicast,
                Vec::new(),
                true,
            ));
        }
    }

    /// Cleanup after an SPDP lease timeout: removes all proxies of this
    /// prefix. Returns `(stateless_pairs_removed,
    /// volatile_pairs_removed)`.
    pub fn on_participant_lost(&mut self, prefix: GuidPrefix) -> (usize, usize) {
        let mut stateless = 0usize;
        let mut volatile = 0usize;
        #[cfg(feature = "std")]
        self.handshakes.remove(&prefix);
        if self
            .stateless_writer
            .remove_reader_proxy(Guid::new(
                prefix,
                EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            ))
            .is_some()
        {
            stateless += 1;
        }
        self.stateless_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
        ));
        if self
            .volatile_writer
            .remove_reader_proxy(Guid::new(
                prefix,
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            ))
            .is_some()
        {
            volatile += 1;
        }
        self.volatile_reader.remove_writer_proxy(Guid::new(
            prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
        ));
        (stateless, volatile)
    }

    /// Tick over all endpoints. Returns HEARTBEATs/resends from the
    /// volatile writer plus ACKNACK/NACK_FRAG from the volatile reader.
    /// Stateless has no tick (BestEffort, no resend state).
    ///
    /// # Errors
    /// Wire encode errors from the reliable layer.
    pub fn poll(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = Vec::new();
        out.extend(self.volatile_writer.tick(now)?);
        out.extend(self.volatile_reader.tick_outbound(now)?);
        // DDS-Security §7.4.4: the stateless handshake channel is BEST_EFFORT;
        // reliability comes from periodically re-sending the pending handshake
        // message (initiator: AUTH_REQUEST, replier: cached reply) until the
        // secret is derived. The receiver is idempotent (duplicate request →
        // cached reply, no DH regeneration). Without this re-send, reply/final
        // are lost over the lossy channel and the handshake stalls —
        // independent of the separately fixed discovery coupling.
        #[cfg(feature = "std")]
        if now.saturating_sub(self.last_handshake_resend) >= Duration::from_millis(500) {
            self.last_handshake_resend = now;
            let pending: Vec<ParticipantGenericMessage> = self
                .handshakes
                .values()
                .filter(|p| p.secret.is_none())
                .filter_map(|p| p.last_request.clone())
                .collect();
            for msg in &pending {
                out.extend(self.stateless_writer.write(msg)?);
            }
        }
        Ok(out)
    }

    /// Returns the `SharedSecretHandle` of a peer once the handshake is
    /// complete on this side (otherwise `None`). FU2: lets the DCPS layer
    /// (and tests) check whether a peer has been authenticated.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_secret(&self, remote_prefix: GuidPrefix) -> Option<SharedSecretHandle> {
        self.handshakes.get(&remote_prefix).and_then(|p| p.secret)
    }

    /// All peers whose handshake is complete on this side (`secret` set).
    /// Lets the DCPS tick send per-endpoint crypto tokens to every
    /// authenticated peer as soon as the local user endpoints exist (FU2
    /// step 6b — the handshake completes before the benchmark creates the
    /// user endpoints).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn completed_peer_prefixes(&self) -> Vec<GuidPrefix> {
        self.handshakes
            .iter()
            .filter(|(_, p)| p.secret.is_some())
            .map(|(prefix, _)| *prefix)
            .collect()
    }

    /// FU2 handshake driver: starts the auth handshake with a freshly
    /// discovered peer (Spec §8.3.2). Called from the SPDP hot path after
    /// `handle_remote_endpoints` as soon as the peer's
    /// `PID_IDENTITY_TOKEN` is available.
    ///
    /// Validates the remote identity, determines the role via GUID
    /// comparison (SMALLER local GUID = initiator) and — if initiator —
    /// sends the `AUTH_REQUEST` over the stateless writer. The replier
    /// only creates its peer state and waits for the request.
    ///
    /// Without a configured auth plugin (`new` instead of `with_auth`)
    /// this is a no-op (empty datagram list).
    ///
    /// The initiator convention (smaller local GUID ⇒ initiator) is
    /// cyclone-verified (c2c handshake FSM trace: the smaller GUID sends
    /// the request, the larger one replies). Both sides MUST choose the
    /// same direction, otherwise neither/both initiate.
    ///
    /// Records the `VendorId` of a remote participant (from the SPDP RTPS
    /// header) for vendor-specific handshake quirks (e.g. OpenDDS'
    /// NUL-terminated algorithm strings). Should be set BEFORE
    /// [`Self::begin_handshake_with`]/[`Self::on_stateless_message`];
    /// if it is not, the NUL-free spec/FastDDS/Cyclone default applies.
    #[cfg(feature = "std")]
    pub fn note_remote_vendor(&mut self, remote_prefix: GuidPrefix, vendor: VendorId) {
        self.remote_vendors.insert(remote_prefix, vendor);
    }

    /// # Errors
    /// `SecurityError` on failed remote identity validation or wire
    /// encode of the request token.
    #[cfg(feature = "std")]
    pub fn begin_handshake_with(
        &mut self,
        remote_prefix: GuidPrefix,
        remote_guid: [u8; 16],
        remote_identity_token: &[u8],
    ) -> SecurityResult<Vec<OutboundDatagram>> {
        let (auth, local_identity) = match (self.auth.clone(), self.local_identity) {
            (Some(a), Some(id)) => (a, id),
            _ => return Ok(Vec::new()),
        };
        // Peer already known: NO second handshake — but as long as the
        // secret is missing (handshake incomplete), re-send the last sent
        // initiator message. The periodic SPDP beacon thereby drives the
        // resend cadence; if the initial AUTH_REQUEST (or B's reply) is
        // lost, the next beacon recovers it instead of stalling
        // permanently. Without a pending message (replier, or already
        // done): no-op.
        if let Some(peer) = self.handshakes.get(&remote_prefix) {
            if peer.secret.is_none() {
                if let Some(req) = peer.last_request.clone() {
                    return self.stateless_writer.write(&req).map_err(wire_to_security);
                }
            }
            return Ok(Vec::new());
        }
        // SMALLER local GUID = initiator (cyclone-verified: in the c2c
        // handshake FSM trace the smaller GUID sends the AUTH_REQUEST, the
        // larger one replies). The inverse convention would make both sides
        // wait with cyclone (ZeroDDS < cyclone ⇒ both repliers) → deadlock.
        let is_initiator = self.local_guid < remote_guid;

        let (remote_identity, request) = {
            let mut plugin = lock_auth(&auth)?;
            // Vendor quirk for the initiator request: OpenDDS requires
            // NUL-terminated c.dsign_algo/c.kagree_algo (sizeof comparison).
            plugin.set_algo_nul_terminate(
                self.remote_vendors.get(&remote_prefix) == Some(&VendorId::OPENDDS),
            );
            let remote_identity = plugin.validate_remote_identity(
                local_identity,
                remote_guid,
                remote_identity_token,
            )?;
            let request = if is_initiator {
                Some(plugin.begin_handshake_request(local_identity, remote_identity)?)
            } else {
                None
            };
            (remote_identity, request)
        };

        let mut peer = PeerHandshake {
            remote_identity,
            remote_guid,
            handshake: None,
            next_sn: 1,
            secret: None,
            last_request: None,
        };

        let mut datagrams = Vec::new();
        if let Some((handle, outcome)) = request {
            peer.handshake = Some(handle);
            if let HandshakeStepOutcome::SendMessage { token } = outcome {
                // OMG DDS-Security §7.4.4: ALL handshake tokens (request/reply/
                // final) travel in the message_class_id `dds.sec.auth`; only the
                // AuthRequestMessageToken uses `dds.sec.auth_request`. The
                // HandshakeRequest has related_message_identity = NIL
                // (= MessageIdentity::default()). Confirmed on the wire against
                // FastDDS/Cyclone.
                let msg = peer.build_message(
                    self.local_guid,
                    class_id::AUTH,
                    token,
                    MessageIdentity::default(),
                )?;
                // Remember for the resend path (periodic beacon cadence).
                peer.last_request = Some(msg.clone());
                datagrams = self
                    .stateless_writer
                    .write(&msg)
                    .map_err(wire_to_security)?;
            }
        }
        self.handshakes.insert(remote_prefix, peer);
        Ok(datagrams)
    }

    /// Number of remote peers with established handshake state (initiator
    /// as well as replier). Read-only observability for the discovery
    /// trigger.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn handshake_peer_count(&self) -> usize {
        self.handshakes.len()
    }

    /// FU2 handshake driver: processes an incoming stateless auth
    /// message (Spec §8.3.2). Dispatch by `message_class_id`:
    ///
    /// - `AUTH_REQUEST` (replier side) → `begin_handshake_reply`,
    ///   sends the reply token back.
    /// - otherwise (`AUTH` = reply at the initiator / final at the
    ///   replier) → `process_handshake`; a `SendMessage` outcome
    ///   (initiator: final token) is sent.
    ///
    /// Returns `(outbound_datagrams, Option<(remote_identity,
    /// shared_secret)>)`. The secret tuple is `Some` as soon as the
    /// handshake derives the SharedSecret on this side (initiator after
    /// reply processing, replier on `Complete`). The DCPS caller passes
    /// it on to `gate.register_remote_with_token` —
    /// **important:** the auth lock held here is already released at
    /// return time (the crypto `SharedSecretProvider` takes the same
    /// mutex), otherwise deadlock.
    ///
    /// Without a configured auth plugin, a no-op.
    ///
    /// # Errors
    /// `SecurityError` from the plugin step or wire encode of the response.
    #[cfg(feature = "std")]
    pub fn on_stateless_message(
        &mut self,
        remote_prefix: GuidPrefix,
        msg: &ParticipantGenericMessage,
    ) -> SecurityResult<HandshakeStepResult> {
        let (auth, local_identity) = match (self.auth.clone(), self.local_identity) {
            (Some(a), Some(id)) => (a, id),
            _ => return Ok((Vec::new(), None)),
        };
        let local_guid = self.local_guid;
        // AuthRequestMessageToken (`dds.sec.auth_request`, +AuthReq): the
        // peer announces its `future_challenge` with it. As initiator we
        // do not need it — cyclone/FastDDS only check `challenge1` against
        // a future_challenge announced by the INITIATOR, which we
        // (deliberately) do not send, so the check is moot. IGNORE it here,
        // otherwise the message falls into the reply/final path and
        // parse_reply_token reports "reply: class_id mismatch".
        if msg.message_class_id == class_id::AUTH_REQUEST {
            return Ok((Vec::new(), None));
        }
        // OMG DDS-Security §7.4.4: a HandshakeRequest is message_class
        // `dds.sec.auth` WITH related_message_identity == NIL. Reply/final are
        // also `dds.sec.auth`, but have related != NIL. (The separate
        // AuthRequestMessageToken uses `dds.sec.auth_request` — it is not
        // treated as a handshake request here.) This replaces the earlier
        // dispatch on the message class, which was cross-vendor incorrect.
        let is_request = msg.message_class_id == class_id::AUTH
            && msg.related_message_identity == MessageIdentity::default();
        let mut peer = match self.handshakes.remove(&remote_prefix) {
            Some(p) => p,
            None => {
                // FU2 S4: the HandshakeRequest is SELF-CONTAINED — it carries
                // the initiator cert within it, and `begin_handshake_reply`
                // validates it internally (`verify_remote_der`) and ignores the
                // SPDP-derived identity handle. The replier therefore does NOT
                // need to have discovered the initiator beforehand via an SPDP
                // beacon (this was the true cause of the cross-process handshake
                // break). We create the replier state on the fly. Reply/final
                // without state = nothing to do.
                if !is_request {
                    return Ok((Vec::new(), None));
                }
                PeerHandshake {
                    // Placeholder: ignored by `begin_handshake_reply`; after
                    // completion only passed to the secret-based crypto register
                    // (the Kx key comes from HKDF(shared_secret), not from the
                    // identity handle).
                    remote_identity: local_identity,
                    // Destination of the reply tokens = source GUID of the request.
                    remote_guid: msg.message_identity.source_guid,
                    handshake: None,
                    next_sn: 1,
                    secret: None,
                    last_request: None,
                }
            }
        };
        let token = match msg.message_data.first() {
            Some(dh) => dh.to_cdr_le(),
            None => {
                self.handshakes.insert(remote_prefix, peer);
                return Ok((Vec::new(), None));
            }
        };

        // FU2 S3: duplicate request (the initiator re-sent because our reply
        // was lost) → re-send the cached reply, do NOT run
        // `begin_handshake_reply` again. Otherwise the replier would generate
        // fresh DH keys and the derived secret would diverge from the
        // initiator (silent decryption failure).
        if is_request && peer.handshake.is_some() {
            let resend = peer.last_request.clone();
            self.handshakes.insert(remote_prefix, peer);
            return match resend {
                Some(r) => self
                    .stateless_writer
                    .write(&r)
                    .map(|d| (d, None))
                    .map_err(wire_to_security),
                None => Ok((Vec::new(), None)),
            };
        }

        let outcome = {
            let mut plugin = lock_auth(&auth)?;
            // Vendor quirk for the replier (own c.dsign_algo; c.kagree_algo
            // is echoed from the request anyway): OpenDDS requires the NUL form.
            plugin.set_algo_nul_terminate(
                self.remote_vendors.get(&remote_prefix) == Some(&VendorId::OPENDDS),
            );
            if is_request {
                let (handle, outcome) =
                    plugin.begin_handshake_reply(local_identity, peer.remote_identity, &token)?;
                peer.handshake = Some(handle);
                outcome
            } else {
                match peer.handshake {
                    Some(handle) => plugin.process_handshake(handle, &token)?,
                    None => {
                        drop(plugin);
                        self.handshakes.insert(remote_prefix, peer);
                        return Ok((Vec::new(), None));
                    }
                }
            }
        };

        let mut datagrams = Vec::new();
        if let HandshakeStepOutcome::SendMessage { token: out_token } = outcome {
            let response = peer.build_message(
                local_guid,
                class_id::AUTH,
                out_token,
                msg.message_identity.clone(),
            )?;
            // Cache the replier reply (or initiator final) → idempotent
            // resend on a lost reply, without new DH generation.
            peer.last_request = Some(response.clone());
            datagrams = self
                .stateless_writer
                .write(&response)
                .map_err(wire_to_security)?;
        }

        // Secret extraction: after each step, check whether the plugin can
        // now derive the SharedSecret. The initiator obtains it after the
        // reply (outcome is SendMessage{Final}), the replier on Complete —
        // `shared_secret(handle)` covers both cases.
        let mut completed = None;
        if peer.secret.is_none() {
            if let Some(handle) = peer.handshake {
                let secret = lock_auth(&auth)?.shared_secret(handle).ok();
                if let Some(secret) = secret {
                    peer.secret = Some(secret);
                    completed = Some((peer.remote_identity, secret));
                }
            }
        }

        self.handshakes.insert(remote_prefix, peer);
        Ok((datagrams, completed))
    }
}

/// `WireError` → `SecurityError` adapter for the handshake driver.
#[cfg(feature = "std")]
fn wire_to_security(_e: WireError) -> zerodds_security::error::SecurityError {
    zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::BadArgument,
        "stateless handshake: wire encode failed",
    )
}

#[cfg(feature = "std")]
impl PeerHandshake {
    /// Wraps a handshake token into a `ParticipantGenericMessage` with a
    /// running `message_identity` and a referenced
    /// `related_message_identity` (NIL for the initial request).
    fn build_message(
        &mut self,
        local_guid: [u8; 16],
        message_class: &str,
        token: Vec<u8>,
        related: MessageIdentity,
    ) -> SecurityResult<ParticipantGenericMessage> {
        let sequence_number = self.next_sn;
        self.next_sn = self.next_sn.saturating_add(1);
        // The token IS a CDR-LE-serialized DataHolder
        // (build_*_token = DataHolder::to_cdr_le); read it back in so that
        // the generic-message encoding re-serializes it cleanly.
        let holder = DataHolder::from_cdr_le(&token)?;
        Ok(ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: local_guid,
                sequence_number,
            },
            related_message_identity: related,
            destination_participant_key: self.remote_guid,
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0; 16],
            message_class_id: message_class.into(),
            message_data: alloc::vec![holder],
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::participant_data::{
        Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
    };
    use zerodds_rtps::wire_types::ProtocolVersion;
    use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
    use zerodds_security::token::DataHolder;

    // Local node (A) carries the SMALLER GUID, making it the initiator
    // under the cyclone convention (smaller GUID = initiator) — so the
    // "A sends" role assertions stay valid.
    fn local_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([1; 12])
    }
    fn remote_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([2; 12])
    }

    fn remote_with(flags: u32) -> DiscoveredParticipant {
        DiscoveredParticipant {
            sender_prefix: remote_prefix(),
            sender_vendor: VendorId::ZERODDS,
            data: ParticipantBuiltinTopicData {
                guid: Guid::new(remote_prefix(), EntityId::PARTICIPANT),
                protocol_version: ProtocolVersion::V2_5,
                vendor_id: VendorId::ZERODDS,
                default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7411)),
                default_multicast_locator: None,
                metatraffic_unicast_locator: None,
                metatraffic_multicast_locator: None,
                domain_id: None,
                builtin_endpoint_set: flags,
                lease_duration: DdsDuration::from_secs(30),
                user_data: alloc::vec::Vec::new(),
                properties: Default::default(),
                identity_token: None,
                permissions_token: None,
                identity_status_token: None,
                sig_algo_info: None,
                kx_algo_info: None,
                sym_cipher_algo_info: None,
                participant_security_info: None,
            },
        }
    }

    fn sample_stateless_msg() -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xAA; 16],
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xBB; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xCC; 16],
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: alloc::vec![DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")],
        }
    }

    #[test]
    fn new_stack_has_zero_proxies_everywhere() {
        let s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
        assert_eq!(s.stateless_reader.writer_proxy_count(), 0);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_reader.writer_proxy_count(), 0);
        assert_eq!(s.local_prefix(), local_prefix());
    }

    #[test]
    fn handle_remote_endpoints_with_all_bits_wires_all_four() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
        assert_eq!(s.stateless_reader.writer_proxy_count(), 1);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 1);
        assert_eq!(s.volatile_reader.writer_proxy_count(), 1);
    }

    #[test]
    fn handle_remote_endpoints_with_only_stateless_bits_skips_volatile() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
        assert_eq!(s.stateless_reader.writer_proxy_count(), 1);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_reader.writer_proxy_count(), 0);
    }

    #[test]
    fn handle_remote_endpoints_with_no_security_bits_is_noop() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::ALL_STANDARD;
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
    }

    #[test]
    fn self_discovery_is_ignored() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let mut peer = remote_with(endpoint_flag::ALL_SECURE);
        peer.sender_prefix = local_prefix();
        s.handle_remote_endpoints(&peer);
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
    }

    #[test]
    fn handle_remote_endpoints_is_idempotent_on_repeat_announcement() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        s.handle_remote_endpoints(&remote_with(flags));
        assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
    }

    #[test]
    fn on_participant_lost_clears_proxies() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        s.handle_remote_endpoints(&remote_with(flags));
        let (sl, vol) = s.on_participant_lost(remote_prefix());
        assert_eq!(sl, 1);
        assert_eq!(vol, 1);
        assert_eq!(s.stateless_writer.reader_proxy_count(), 0);
        assert_eq!(s.volatile_writer.reader_proxy_count(), 0);
    }

    #[test]
    fn poll_on_empty_stack_returns_no_datagrams() {
        let mut s = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let dgs = s.poll(Duration::from_secs(1)).unwrap();
        assert!(dgs.is_empty());
    }

    #[test]
    fn end_to_end_stateless_message_loopback_between_stacks() {
        let mut a = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let mut b = SecurityBuiltinStack::new(remote_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        // A discovers B
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        // B discovers A
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        let msg = sample_stateless_msg();
        let dgs = a.stateless_writer.write(&msg).unwrap();
        assert_eq!(dgs.len(), 1);
        let received = b.stateless_reader.handle_datagram(&dgs[0].bytes).unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], msg);
    }

    fn remote_with_prefix(prefix: GuidPrefix, flags: u32) -> DiscoveredParticipant {
        let mut peer = remote_with(flags);
        peer.sender_prefix = prefix;
        peer.data.guid = Guid::new(prefix, EntityId::PARTICIPANT);
        peer
    }

    // ---------------------------------------------------------------
    // FU2/Gap-3: handshake driver — two stacks drive a full PKI 3-round
    // handshake over a stateless loopback and BOTH derive the same
    // SharedSecret.
    // ---------------------------------------------------------------
    #[cfg(feature = "std")]
    #[allow(clippy::type_complexity)]
    fn mint_ca_and_two_leafs() -> (Vec<u8>, (Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)) {
        use alloc::string::String;
        use rcgen::{CertificateParams, KeyPair};
        let mut ca_params = CertificateParams::new(std::vec![String::from("Common CA")]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem().into_bytes();

        let mint_leaf = |name: &str| -> (Vec<u8>, Vec<u8>) {
            let mut params = CertificateParams::new(std::vec![String::from(name)]).unwrap();
            params.is_ca = rcgen::IsCa::NoCa;
            let key = KeyPair::generate().unwrap();
            let cert = params.signed_by(&key, &ca_cert, &ca_key).unwrap();
            (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
        };
        (ca_pem, mint_leaf("alice"), mint_leaf("bob"))
    }

    #[cfg(feature = "std")]
    fn cert_der_from_pem(pem: &[u8]) -> Vec<u8> {
        use rustls_pki_types::CertificateDer;
        use rustls_pki_types::pem::PemObject;
        CertificateDer::pem_slice_iter(pem)
            .next()
            .unwrap()
            .unwrap()
            .as_ref()
            .to_vec()
    }

    #[cfg(feature = "std")]
    #[test]
    fn full_pki_handshake_between_two_stacks_yields_shared_secret() {
        use alloc::sync::Arc;
        use std::sync::Mutex;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        let (ca_pem, (a_cert, a_key), (b_cert, b_key)) = mint_ca_and_two_leafs();

        let a_guid = Guid::new(local_prefix(), EntityId::PARTICIPANT).to_bytes();
        let b_guid = Guid::new(remote_prefix(), EntityId::PARTICIPANT).to_bytes();

        // Validate local identities (own plugin per stack).
        let a_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let b_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let a_local = a_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a_cert.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(a_key),
                },
                a_guid,
            )
            .unwrap();
        let b_local = b_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: b_cert.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(b_key),
                },
                b_guid,
            )
            .unwrap();

        let a_auth: Arc<Mutex<dyn AuthenticationPlugin>> = a_pki.clone();
        let b_auth: Arc<Mutex<dyn AuthenticationPlugin>> = b_pki.clone();
        let mut a = SecurityBuiltinStack::with_auth(
            local_prefix(),
            VendorId::ZERODDS,
            a_auth,
            a_local,
            a_guid,
        );
        let mut b = SecurityBuiltinStack::with_auth(
            remote_prefix(),
            VendorId::ZERODDS,
            b_auth,
            b_local,
            b_guid,
        );

        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        // Both discover each other and call begin_handshake_with. Only
        // the initiator (smaller prefix → A, cyclone convention) sends.
        let mut in_flight = a
            .begin_handshake_with(remote_prefix(), b_guid, &cert_der_from_pem(&b_cert))
            .unwrap();
        let from_b = b
            .begin_handshake_with(local_prefix(), a_guid, &cert_der_from_pem(&a_cert))
            .unwrap();
        assert!(
            from_b.is_empty(),
            "B is the replier (larger prefix), must not initiate"
        );
        assert_eq!(in_flight.len(), 1, "A sends exactly one AUTH_REQUEST");

        // Ping-pong: deliver the datagram to the other side, whose response
        // becomes the next in_flight. Loop until both secrets are present.
        let mut a_secret = None;
        let mut b_secret = None;
        let mut deliver_to_b = true; // the first datagram goes from A to B
        for _ in 0..6 {
            if in_flight.is_empty() {
                break;
            }
            let datagram = in_flight.remove(0);
            let (target, target_prefix) = if deliver_to_b {
                (&mut b, local_prefix())
            } else {
                (&mut a, remote_prefix())
            };
            let msgs = target
                .stateless_reader
                .handle_datagram(&datagram.bytes)
                .unwrap();
            assert_eq!(msgs.len(), 1, "ein generic-message pro Datagram");
            let (out, completed) = target
                .on_stateless_message(target_prefix, &msgs[0])
                .unwrap();
            if let Some((_id, secret)) = completed {
                if deliver_to_b {
                    b_secret = Some(secret);
                } else {
                    a_secret = Some(secret);
                }
            }
            in_flight = out;
            deliver_to_b = !deliver_to_b;
        }

        let a_secret = a_secret.expect("A must derive a secret");
        let b_secret = b_secret.expect("B must derive a secret");
        let a_bytes = a_pki
            .lock()
            .unwrap()
            .secret_bytes(a_secret)
            .unwrap()
            .to_vec();
        let b_bytes = b_pki
            .lock()
            .unwrap()
            .secret_bytes(b_secret)
            .unwrap()
            .to_vec();
        assert_eq!(a_bytes.len(), 32);
        assert_eq!(a_bytes, b_bytes, "both stacks derive the same secret");
    }

    #[cfg(feature = "std")]
    #[test]
    fn smaller_local_guid_initiates_handshake_cyclone_compatible() {
        // cyclone-verified (c2c handshake FSM trace): the SMALLER GUID
        // sends the AUTH_REQUEST (initiator), the larger one replies. ZeroDDS
        // MUST choose the same direction — otherwise both sides wait with
        // cyclone (ZeroDDS < cyclone ⇒ both repliers) and the handshake stalls.
        use alloc::sync::Arc;
        use std::sync::Mutex;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        let (ca_pem, (big_cert, big_key), (small_cert, small_key)) = mint_ca_and_two_leafs();
        let big_prefix = GuidPrefix::from_bytes([0x09; 12]);
        let small_prefix = GuidPrefix::from_bytes([0x01; 12]);
        let big_guid = Guid::new(big_prefix, EntityId::PARTICIPANT).to_bytes();
        let small_guid = Guid::new(small_prefix, EntityId::PARTICIPANT).to_bytes();
        assert!(big_guid > small_guid, "test setup: big must be > small");

        let mk = |cert: Vec<u8>, key: Vec<u8>, prefix: GuidPrefix, guid: [u8; 16]| {
            let pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
            let local = pki
                .lock()
                .unwrap()
                .validate_with_config(
                    IdentityConfig {
                        identity_cert_pem: cert,
                        identity_ca_pem: ca_pem.clone(),
                        identity_key_pem: Some(key),
                    },
                    guid,
                )
                .unwrap();
            let auth: Arc<Mutex<dyn AuthenticationPlugin>> = pki.clone();
            SecurityBuiltinStack::with_auth(prefix, VendorId::ZERODDS, auth, local, guid)
        };

        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;

        // Small node: local < remote ⇒ MUST initiate.
        let mut small = mk(small_cert.clone(), small_key, small_prefix, small_guid);
        small.handle_remote_endpoints(&remote_with_prefix(big_prefix, flags));
        let from_small = small
            .begin_handshake_with(big_prefix, big_guid, &cert_der_from_pem(&big_cert))
            .unwrap();
        assert_eq!(
            from_small.len(),
            1,
            "smaller local GUID MUST send AUTH_REQUEST (initiator, cyclone convention)"
        );

        // Large node: local > remote ⇒ MUST wait (replier).
        let mut big = mk(big_cert, big_key, big_prefix, big_guid);
        big.handle_remote_endpoints(&remote_with_prefix(small_prefix, flags));
        let from_big = big
            .begin_handshake_with(small_prefix, small_guid, &cert_der_from_pem(&small_cert))
            .unwrap();
        assert!(
            from_big.is_empty(),
            "larger local GUID MUST wait (replier), must not initiate"
        );
    }

    #[test]
    fn initiator_resends_auth_request_on_repeated_begin_while_incomplete() {
        // FU2 S3: stateless auth is best-effort. If the initial
        // AUTH_REQUEST (or B's reply) is lost, the next SPDP-beacon-driven
        // `begin_handshake_with` call must re-send the AUTH_REQUEST as long
        // as the secret is missing — otherwise the handshake stalls
        // permanently (startup race).
        use alloc::sync::Arc;
        use std::sync::Mutex;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        let (ca_pem, (a_cert, a_key), (b_cert, _b_key)) = mint_ca_and_two_leafs();
        let a_guid = Guid::new(local_prefix(), EntityId::PARTICIPANT).to_bytes();
        let b_guid = Guid::new(remote_prefix(), EntityId::PARTICIPANT).to_bytes();

        let a_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let a_local = a_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a_cert.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(a_key),
                },
                a_guid,
            )
            .unwrap();
        let a_auth: Arc<Mutex<dyn AuthenticationPlugin>> = a_pki.clone();
        let mut a = SecurityBuiltinStack::with_auth(
            local_prefix(),
            VendorId::ZERODDS,
            a_auth,
            a_local,
            a_guid,
        );

        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));

        let b_token = cert_der_from_pem(&b_cert);
        let first = a
            .begin_handshake_with(remote_prefix(), b_guid, &b_token)
            .unwrap();
        assert_eq!(first.len(), 1, "first beacon: AUTH_REQUEST sent");

        // Second beacon, handshake still incomplete (no secret) →
        // RESEND instead of idempotent-empty.
        let resend = a
            .begin_handshake_with(remote_prefix(), b_guid, &b_token)
            .unwrap();
        assert_eq!(
            resend.len(),
            1,
            "second beacon: AUTH_REQUEST must be sent again (resend, not idempotent-empty)"
        );
    }

    #[test]
    fn lost_reply_recovered_by_resend_yields_matching_secret() {
        // FU2 S3: B's reply is lost → A re-sends AUTH_REQUEST → B MUST
        // re-send the CACHED reply (not regenerate it), otherwise the DH
        // secrets diverge. Proof: after loss+resend both sides derive the
        // SAME secret.
        use alloc::sync::Arc;
        use std::sync::Mutex;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        let (ca_pem, (a_cert, a_key), (b_cert, b_key)) = mint_ca_and_two_leafs();
        let a_guid = Guid::new(local_prefix(), EntityId::PARTICIPANT).to_bytes();
        let b_guid = Guid::new(remote_prefix(), EntityId::PARTICIPANT).to_bytes();
        let a_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let b_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let a_local = a_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a_cert.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(a_key),
                },
                a_guid,
            )
            .unwrap();
        let b_local = b_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: b_cert.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(b_key),
                },
                b_guid,
            )
            .unwrap();
        let a_auth: Arc<Mutex<dyn AuthenticationPlugin>> = a_pki.clone();
        let b_auth: Arc<Mutex<dyn AuthenticationPlugin>> = b_pki.clone();
        let mut a = SecurityBuiltinStack::with_auth(
            local_prefix(),
            VendorId::ZERODDS,
            a_auth,
            a_local,
            a_guid,
        );
        let mut b = SecurityBuiltinStack::with_auth(
            remote_prefix(),
            VendorId::ZERODDS,
            b_auth,
            b_local,
            b_guid,
        );
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        let mut a_secret = None;
        let mut b_secret = None;

        // Both discover each other; B (replier) creates its peer state for A
        // (sends nothing itself).
        let from_b = b
            .begin_handshake_with(local_prefix(), a_guid, &cert_der_from_pem(&a_cert))
            .unwrap();
        assert!(from_b.is_empty(), "B is the replier, does not initiate");

        // 1. A → AUTH_REQUEST. 2. B processes → reply LOST (discarded).
        let req = a
            .begin_handshake_with(remote_prefix(), b_guid, &cert_der_from_pem(&b_cert))
            .unwrap();
        let m = b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let (_lost, b_c0) = b.on_stateless_message(local_prefix(), &m[0]).unwrap();
        b_secret = b_secret.or(b_c0.map(|x| x.1));

        // 3. A's next beacon → resend of the same AUTH_REQUEST.
        let req2 = a
            .begin_handshake_with(remote_prefix(), b_guid, &cert_der_from_pem(&b_cert))
            .unwrap();
        assert_eq!(req2.len(), 1, "AUTH_REQUEST must be resent");

        // 4. B sees the duplicate → re-sends the CACHED reply. Ping-pong from here.
        let m2 = b.stateless_reader.handle_datagram(&req2[0].bytes).unwrap();
        let (reply, b_c1) = b.on_stateless_message(local_prefix(), &m2[0]).unwrap();
        b_secret = b_secret.or(b_c1.map(|x| x.1));
        assert_eq!(reply.len(), 1, "B resends the cached reply");

        let mut in_flight = reply;
        let mut deliver_to_b = false; // the reply goes to A first
        for _ in 0..6 {
            if in_flight.is_empty() {
                break;
            }
            let dg = in_flight.remove(0);
            let (target, target_prefix) = if deliver_to_b {
                (&mut b, local_prefix())
            } else {
                (&mut a, remote_prefix())
            };
            let msgs = target.stateless_reader.handle_datagram(&dg.bytes).unwrap();
            let (out, completed) = target
                .on_stateless_message(target_prefix, &msgs[0])
                .unwrap();
            if let Some((_id, secret)) = completed {
                if deliver_to_b {
                    b_secret = Some(secret);
                } else {
                    a_secret = Some(secret);
                }
            }
            in_flight = out;
            deliver_to_b = !deliver_to_b;
        }

        let a_secret = a_secret.expect("A must derive a secret");
        let b_secret = b_secret.expect("B must derive a secret");
        let a_bytes = a_pki
            .lock()
            .unwrap()
            .secret_bytes(a_secret)
            .unwrap()
            .to_vec();
        let b_bytes = b_pki
            .lock()
            .unwrap()
            .secret_bytes(b_secret)
            .unwrap()
            .to_vec();
        assert_eq!(
            a_bytes, b_bytes,
            "secret after reply-loss+resend must match (B must NOT have regenerated)"
        );
    }

    #[test]
    fn replier_handles_auth_request_without_prior_discovery() {
        // FU2 S4 (true cause of the cross-process break): the replier (B)
        // receives A's AUTH_REQUEST WITHOUT having called
        // `begin_handshake_with` beforehand (= without having processed A's
        // SPDP beacon). Since the request is self-contained, B MUST still
        // process it, reply, and ultimately derive the same secret as A — no
        // timer, no resend crutch needed.
        use alloc::sync::Arc;
        use std::sync::Mutex;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        let (ca_pem, (a_cert, a_key), (b_cert, b_key)) = mint_ca_and_two_leafs();
        let a_guid = Guid::new(local_prefix(), EntityId::PARTICIPANT).to_bytes();
        let b_guid = Guid::new(remote_prefix(), EntityId::PARTICIPANT).to_bytes();
        let a_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let b_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let a_local = a_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a_cert.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(a_key),
                },
                a_guid,
            )
            .unwrap();
        let b_local = b_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: b_cert.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(b_key),
                },
                b_guid,
            )
            .unwrap();
        let a_auth: Arc<Mutex<dyn AuthenticationPlugin>> = a_pki.clone();
        let b_auth: Arc<Mutex<dyn AuthenticationPlugin>> = b_pki.clone();
        let mut a = SecurityBuiltinStack::with_auth(
            local_prefix(),
            VendorId::ZERODDS,
            a_auth,
            a_local,
            a_guid,
        );
        let mut b = SecurityBuiltinStack::with_auth(
            remote_prefix(),
            VendorId::ZERODDS,
            b_auth,
            b_local,
            b_guid,
        );
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        // A initiates. B DELIBERATELY does NOT call `begin_handshake_with` —
        // so B has NOT discovered A via SPDP (= the cross-process race case).
        let mut in_flight = a
            .begin_handshake_with(remote_prefix(), b_guid, &cert_der_from_pem(&b_cert))
            .unwrap();
        assert_eq!(in_flight.len(), 1, "A sends AUTH_REQUEST");

        let mut a_secret = None;
        let mut b_secret = None;
        let mut deliver_to_b = true;
        for _ in 0..6 {
            if in_flight.is_empty() {
                break;
            }
            let dg = in_flight.remove(0);
            let (target, target_prefix) = if deliver_to_b {
                (&mut b, local_prefix())
            } else {
                (&mut a, remote_prefix())
            };
            let msgs = target.stateless_reader.handle_datagram(&dg.bytes).unwrap();
            let (out, completed) = target
                .on_stateless_message(target_prefix, &msgs[0])
                .unwrap();
            if let Some((_id, secret)) = completed {
                if deliver_to_b {
                    b_secret = Some(secret);
                } else {
                    a_secret = Some(secret);
                }
            }
            in_flight = out;
            deliver_to_b = !deliver_to_b;
        }

        let a_secret = a_secret.expect("A derives a secret");
        let b_secret = b_secret.expect("B (without prior discovery) derives a secret");
        let a_bytes = a_pki
            .lock()
            .unwrap()
            .secret_bytes(a_secret)
            .unwrap()
            .to_vec();
        let b_bytes = b_pki
            .lock()
            .unwrap()
            .secret_bytes(b_secret)
            .unwrap()
            .to_vec();
        assert_eq!(
            a_bytes, b_bytes,
            "secret must match even though B processed the request without prior discovery"
        );
    }

    #[test]
    fn end_to_end_volatile_secure_handshake_via_reliable_loop() {
        // A sends a crypto-token message over the VolatileSecure topic to
        // B. We simulate the reliable hop manually:
        // 1. A.write produces DATA + (an initial pre-emptive HEARTBEAT follows in the tick)
        // 2. B.handle_data decodes the message
        // 3. B.tick_outbound emits an ACKNACK
        // 4. A.handle_acknack accepts it
        let mut a = SecurityBuiltinStack::new(local_prefix(), VendorId::ZERODDS);
        let mut b = SecurityBuiltinStack::new(remote_prefix(), VendorId::ZERODDS);
        let flags = endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        a.handle_remote_endpoints(&remote_with_prefix(remote_prefix(), flags));
        b.handle_remote_endpoints(&remote_with_prefix(local_prefix(), flags));

        let mut msg = sample_stateless_msg();
        msg.message_class_id = class_id::PARTICIPANT_CRYPTO_TOKENS.into();

        let dgs = a.volatile_writer.write(&msg).unwrap();
        assert_eq!(dgs.len(), 1, "ein Datagram pro Reader-Proxy");
        // Wire-decode + dispatch into B's volatile reader
        let parsed = zerodds_rtps::datagram::decode_datagram(&dgs[0].bytes).unwrap();
        let mut received_msgs = Vec::new();
        for sub in parsed.submessages {
            if let zerodds_rtps::datagram::ParsedSubmessage::Data(d) = sub {
                if d.reader_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER {
                    received_msgs.extend(
                        b.volatile_reader
                            .handle_data(parsed.header.guid_prefix, &d)
                            .unwrap(),
                    );
                }
            }
        }
        assert_eq!(received_msgs.len(), 1);
        assert_eq!(received_msgs[0], msg);

        // ACKNACK flows back, A must accept handle_acknack
        let outbound = b
            .volatile_reader
            .tick_outbound(Duration::from_millis(500))
            .unwrap();
        // B knows A as a writer proxy → ACKNACK datagrams should exist
        assert!(
            !outbound.is_empty(),
            "reader should send an initial ACKNACK"
        );
    }
}
