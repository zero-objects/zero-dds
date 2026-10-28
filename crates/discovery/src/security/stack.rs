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
use alloc::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "std")]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(feature = "std")]
use zerodds_security::authentication::{
    AuthStatusKind, AuthenticationListener, AuthenticationPlugin, HandshakeHandle,
    HandshakeStepOutcome, IdentityHandle, PeerAlgorithmNames, SharedSecretHandle, ValidationResult,
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

/// How the authentication of a remote participant stands (DDS-Security 1.2
/// §9.3.2.9 Figure 9, §9.3.2.10).
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerAuthentication {
    /// The authentication is running (validation, handshake or a retry).
    InProgress,
    /// The participant is authenticated.
    Authenticated,
    /// The participant is "Unauthenticated" (§9.3.2.10): whatever the reason
    /// is, "All these cases shall be treated the same", according to what
    /// the configuration of the participant says about unauthenticated
    /// participants.
    Unauthenticated(UnauthenticatedReason),
}

/// Why a participant is unauthenticated. §9.3.2.10 lists them: "they lack
/// support for the Authentication plugin being used, have incompatible
/// plugins, incompatible plugin configurations, or simply fail the
/// authentication protocol". The reason is for the log; the treatment of the
/// participant does not depend on it.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnauthenticatedReason {
    /// The participant announced no IdentityToken: no authentication
    /// plugin on its side.
    NoIdentityToken,
    /// `validate_remote_identity` refused its identity (another plugin, an
    /// incompatible plugin version, a certificate that does not validate).
    IdentityRejected(alloc::string::String),
    /// Its algorithms or its plugin configuration are incompatible with
    /// this participant's (§7.3.10.1).
    Incompatible(alloc::string::String),
    /// The handshake failed.
    HandshakeFailed(alloc::string::String),
    /// The handshake did not complete within the `authentication_timeout`.
    Timeout,
    /// Its identity was revoked (`AuthenticationListener::on_revoke_identity`).
    Revoked,
}

/// An event of the authentication plugin, collected by the stack's listener.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum AuthEvent {
    Revoked(IdentityHandle),
    StatusChanged(IdentityHandle, AuthStatusKind),
}

/// The `AuthenticationListener` the stack installs in its plugin: it queues
/// the events for the stack's tick, so that the plugin, which is locked when
/// it calls the listener, is not re-entered.
#[cfg(feature = "std")]
struct StackListener {
    events: Arc<Mutex<Vec<AuthEvent>>>,
}

#[cfg(feature = "std")]
impl StackListener {
    fn push(&self, event: AuthEvent) {
        // A poisoned queue still holds the events before the panic; the new
        // one is added to them.
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(poisoned) => poisoned.into_inner(),
        };
        events.push(event);
    }
}

#[cfg(feature = "std")]
impl AuthenticationListener for StackListener {
    fn on_revoke_identity(&self, handle: IdentityHandle) -> SecurityResult<()> {
        self.push(AuthEvent::Revoked(handle));
        Ok(())
    }

    fn on_status_changed(&self, handle: IdentityHandle, status_kind: AuthStatusKind) {
        self.push(AuthEvent::StatusChanged(handle, status_kind));
    }
}

/// What a plugin step that returned `VALIDATION_PENDING_RETRY` runs again
/// (§9.3.2.9 Figure 9: RetryWait1 to RetryWait5).
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum RetryAction {
    /// `validate_remote_identity`, with the token stored in the peer.
    ValidateRemote,
    /// `begin_handshake_request`.
    BeginRequest,
    /// `begin_handshake_reply` with the request and the message identity it
    /// came with.
    BeginReply {
        request: Vec<u8>,
        related: MessageIdentity,
    },
    /// `process_handshake` with the message and the identity it came with.
    Process {
        token: Vec<u8>,
        related: MessageIdentity,
    },
}

/// A retry that is due at `due` (the stack's clock).
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
struct ScheduledRetry {
    due: Duration,
    action: RetryAction,
}

/// Default of [`SecurityBuiltinStack::set_retry_timeout`]: how long the stack
/// waits for a reply before it sends the cached message again
/// (§9.3.2.9.1 rule 2), and before it calls an operation again that returned
/// `VALIDATION_PENDING_RETRY`.
#[cfg(feature = "std")]
pub const DEFAULT_RETRY_TIMEOUT: Duration = Duration::from_millis(500);

/// Default of [`SecurityBuiltinStack::set_authentication_timeout`]: the time
/// a handshake may take (AuthenticationTimeoutWait of Figure 9). The spec
/// leaves it to the implementation ("an implementation-specific timeout").
#[cfg(feature = "std")]
pub const DEFAULT_AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(60);

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
    /// Secret already reported to the caller — idempotency guard.
    secret: Option<SharedSecretHandle>,
    /// FU2 S3: last sent initiator message (AUTH_REQUEST). Re-emitted on
    /// every repeated `begin_handshake_with` (periodic SPDP beacon) as
    /// long as the secret is missing — makes the best-effort stateless
    /// handshake robust against lost initial messages.
    last_request: Option<ParticipantGenericMessage>,
    /// The peer's permissions document (`c.perm`) captured from the handshake,
    /// forwarded to the access-control gate for the remote endpoint checks.
    permissions: Option<Vec<u8>>,
    /// The DER certificate the peer authenticated with, once the handshake
    /// completed; its SubjectName selects the peer's grant.
    identity_certificate: Option<Vec<u8>>,
    /// The IdentityToken `validate_remote_identity` was given (kept for the
    /// operation to be called again, §9.3.2.9.1 rule 1 and a retry).
    remote_identity_token: Vec<u8>,
    /// The stack's clock when the authentication of the peer began.
    started_at: Duration,
    /// The operation to call again, when the plugin asked for it.
    retry: Option<ScheduledRetry>,
    /// `validate_remote_identity` returned `VALIDATION_OK`: authenticated
    /// without a handshake and without a shared secret.
    authenticated: bool,
}

#[cfg(feature = "std")]
impl PeerHandshake {
    fn new(
        remote_identity: IdentityHandle,
        remote_guid: [u8; 16],
        remote_identity_token: Vec<u8>,
        started_at: Duration,
    ) -> Self {
        Self {
            remote_identity,
            remote_guid,
            handshake: None,
            secret: None,
            last_request: None,
            permissions: None,
            identity_certificate: None,
            remote_identity_token,
            started_at,
            retry: None,
            authenticated: false,
        }
    }
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
    /// Local 16-byte participant GUID.
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
    /// The `message_auth.supported_mask` of each remote participant's
    /// signature algorithm info (DDS-Security 1.2 §7.3.11), via
    /// [`Self::note_remote_message_auth_supported`].
    #[cfg(feature = "std")]
    remote_message_auth: BTreeMap<GuidPrefix, u32>,
    /// The class id of each remote participant's SPDP permissions_token
    /// (`PID_PERMISSIONS_TOKEN`), which DDS-Security 1.2 Table 70 compares
    /// with the local one. Maintained via
    /// [`Self::note_remote_permissions_token`].
    #[cfg(feature = "std")]
    remote_permissions_tokens: BTreeMap<GuidPrefix, alloc::string::String>,
    /// The AuthRequestMessageToken a participant sent before it was
    /// discovered, per prefix, for the `validate_remote_identity` its
    /// discovery causes (DDS-Security 1.2 §9.3.2.11.3). Bounded by
    /// [`MAX_PENDING_AUTH_REQUESTS`].
    #[cfg(feature = "std")]
    pending_auth_requests: BTreeMap<GuidPrefix, Vec<u8>>,
    /// Timestamp of the last handshake resend (throttle). The
    /// `DCPSParticipantStatelessMessage` writer is per spec (DDS-Security
    /// §7.4.4) BEST_EFFORT; the reliability of the 3-message handshake
    /// comes from periodic re-send of the pending message until the secret
    /// is established. That is the spec protocol, NOT a workaround for the
    /// (separately fixed) discovery coupling.
    #[cfg(feature = "std")]
    last_handshake_resend: Duration,
    /// The local `ParticipantBuiltinTopicData` as PL_CDR in big endian, the
    /// `serialized_local_participant_data` of `begin_handshake_request` and
    /// `begin_handshake_reply` (§9.3.2.11.4, §9.3.2.11.5).
    #[cfg(feature = "std")]
    local_pdata: Vec<u8>,
    /// The stack's clock: the `now` of the last `poll`.
    #[cfg(feature = "std")]
    clock: Duration,
    /// How long to wait for a reply before sending the cached message again.
    #[cfg(feature = "std")]
    retry_timeout: Duration,
    /// How long a handshake may take.
    #[cfg(feature = "std")]
    authentication_timeout: Duration,
    /// Why the participants that are not authenticated are not (§9.3.2.10).
    #[cfg(feature = "std")]
    unauthenticated: BTreeMap<GuidPrefix, UnauthenticatedReason>,
    /// The participants whose identity was revoked and that are not
    /// authenticated again while they are known.
    #[cfg(feature = "std")]
    revoked: BTreeSet<GuidPrefix>,
    /// The revoked participants the DDS implementation has not been told of.
    #[cfg(feature = "std")]
    revoked_peers: Vec<GuidPrefix>,
    /// Handshake failures the tick found (no caller to return them to); the
    /// DDS implementation takes them with [`Self::take_handshake_failures`].
    #[cfg(feature = "std")]
    failures: Vec<(GuidPrefix, SecurityError)>,
    /// A message to send again at the next poll (§9.3.2.9.1 rule 3), per peer.
    #[cfg(feature = "std")]
    resend_now: BTreeMap<GuidPrefix, ParticipantGenericMessage>,
    /// The queue of the listener installed in the plugin.
    #[cfg(feature = "std")]
    auth_events: Arc<Mutex<Vec<AuthEvent>>>,
    /// The identity status of the local participant changed
    /// (`AuthStatusKind::IdentityStatus`).
    #[cfg(feature = "std")]
    identity_status_changed: bool,
    /// The local identity was revoked.
    #[cfg(feature = "std")]
    local_identity_revoked: bool,
    /// The error of installing the listener in the plugin, if any.
    #[cfg(feature = "std")]
    listener_error: Option<SecurityError>,
    /// Handshakes that completed in a retried step.
    #[cfg(feature = "std")]
    completed_by_tick: Vec<(GuidPrefix, IdentityHandle, SharedSecretHandle)>,
    /// The second of the plugin's last `check_identities`.
    #[cfg(feature = "std")]
    last_identity_check_secs: u64,
    /// How the algorithm names of a handshake token are written
    /// (DDS-Security 1.2 §7.3.3.1).
    algo_name_nul: zerodds_security::AlgoNameNul,
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
            remote_message_auth: BTreeMap::new(),
            #[cfg(feature = "std")]
            remote_permissions_tokens: BTreeMap::new(),
            #[cfg(feature = "std")]
            pending_auth_requests: BTreeMap::new(),
            #[cfg(feature = "std")]
            last_handshake_resend: Duration::ZERO,
            #[cfg(feature = "std")]
            local_pdata: Vec::new(),
            #[cfg(feature = "std")]
            clock: Duration::ZERO,
            #[cfg(feature = "std")]
            retry_timeout: DEFAULT_RETRY_TIMEOUT,
            #[cfg(feature = "std")]
            authentication_timeout: DEFAULT_AUTHENTICATION_TIMEOUT,
            #[cfg(feature = "std")]
            unauthenticated: BTreeMap::new(),
            #[cfg(feature = "std")]
            revoked: BTreeSet::new(),
            #[cfg(feature = "std")]
            revoked_peers: Vec::new(),
            #[cfg(feature = "std")]
            failures: Vec::new(),
            #[cfg(feature = "std")]
            resend_now: BTreeMap::new(),
            #[cfg(feature = "std")]
            auth_events: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            identity_status_changed: false,
            #[cfg(feature = "std")]
            local_identity_revoked: false,
            #[cfg(feature = "std")]
            listener_error: None,
            #[cfg(feature = "std")]
            completed_by_tick: Vec::new(),
            #[cfg(feature = "std")]
            last_identity_check_secs: 0,
            algo_name_nul: zerodds_security::AlgoNameNul::default(),
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
        let mut stack = Self::new(local_prefix, vendor_id);
        stack.auth = Some(auth);
        stack.local_identity = Some(local_identity);
        stack.local_guid = local_guid;
        stack.install_listener();
        stack
    }

    /// Installs the stack's `AuthenticationListener` in the plugin
    /// (`set_listener`, §9.3.2.11.13), so that it learns of revoked
    /// identities and changes of the identity status. An error of the plugin
    /// is kept in [`Self::listener_error`].
    #[cfg(feature = "std")]
    fn install_listener(&mut self) {
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let listener: Arc<dyn AuthenticationListener> = Arc::new(StackListener {
            events: Arc::clone(&self.auth_events),
        });
        self.listener_error = lock_auth(&auth)
            .and_then(|mut plugin| plugin.set_listener(Some(listener)))
            .err();
    }

    /// The error of `set_listener` on the plugin, when it refused the
    /// listener of the stack: then the stack does not learn of revoked
    /// identities.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn listener_error(&self) -> Option<&SecurityError> {
        self.listener_error.as_ref()
    }

    /// Sets the local `ParticipantBuiltinTopicData` as PL_CDR in big endian:
    /// the `serialized_local_participant_data` the stack gives
    /// `begin_handshake_request` and `begin_handshake_reply` (§9.3.2.11.4,
    /// §9.3.2.11.5), which the builtin plugin sends as `c.pdata`.
    #[cfg(feature = "std")]
    pub fn set_local_participant_data(&mut self, pdata: Vec<u8>) {
        self.local_pdata = pdata;
    }

    /// How long the stack waits for a reply before it sends the cached
    /// message again (§9.3.2.9.1 rule 2), and before it calls an operation
    /// again that returned `VALIDATION_PENDING_RETRY`.
    #[cfg(feature = "std")]
    pub fn set_retry_timeout(&mut self, timeout: Duration) {
        self.retry_timeout = timeout;
    }

    /// How long a handshake may take; a peer that has not authenticated by
    /// then is unauthenticated with the reason
    /// [`UnauthenticatedReason::Timeout`] (AuthenticationTimeoutWait of
    /// Figure 9).
    #[cfg(feature = "std")]
    pub fn set_authentication_timeout(&mut self, timeout: Duration) {
        self.authentication_timeout = timeout;
    }

    /// How the algorithm names `c.dsign_algo` and `c.kagree_algo` are written
    /// (DDS-Security 1.2 §7.3.3.1); the default is the spec form with the NUL.
    pub fn set_algo_name_nul(&mut self, form: zerodds_security::AlgoNameNul) {
        self.algo_name_nul = form;
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
        self.handle_remote_stateless_endpoints(peer);
        self.handle_remote_volatile_endpoints(peer);
    }

    /// The ParticipantStatelessMessage endpoints of `peer`: what a
    /// participant matches of one that is to authenticate, "to allow the
    /// authentication handshake to proceed" (DDS-Security 1.2 §9.8.3.2).
    pub fn handle_remote_stateless_endpoints(&mut self, peer: &DiscoveredParticipant) {
        if peer.sender_prefix == self.local_prefix {
            return;
        }
        let caps = PeerCapabilities::from_bits(peer.data.builtin_endpoint_set);
        let unicast: Vec<Locator> = peer.data.metatraffic_or_default_unicast_locators();
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
                unicast,
                Vec::new(),
                false,
            ));
        }
    }

    /// The ParticipantVolatileMessageSecure endpoints of `peer`, a secure
    /// builtin endpoint matched once the peer is authenticated (DDS-Security
    /// 1.2 §9.8.3); its crypto tokens travel over it.
    pub fn handle_remote_volatile_endpoints(&mut self, peer: &DiscoveredParticipant) {
        if peer.sender_prefix == self.local_prefix {
            return;
        }
        let caps = PeerCapabilities::from_bits(peer.data.builtin_endpoint_set);
        let unicast: Vec<Locator> = peer.data.metatraffic_or_default_unicast_locators();
        let remote_prefix = peer.sender_prefix;
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
        // The identity handle the plugin made for this peer is returned
        // (§9.3.2.11.18): its state, the remote's token version among it,
        // was never released.
        #[cfg(feature = "std")]
        {
            self.unauthenticated.remove(&prefix);
            self.revoked.remove(&prefix);
            self.resend_now.remove(&prefix);
            self.pending_auth_requests.remove(&prefix);
            if let Some(peer) = self.handshakes.remove(&prefix) {
                if let (Some(auth), Some(local_identity)) = (self.auth.clone(), self.local_identity)
                {
                    self.release_peer_handles(&auth, local_identity, prefix, &peer);
                }
            }
        }
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

    /// Tick over all endpoints: [`Self::poll_volatile`] followed by
    /// [`Self::poll_stateless`].
    ///
    /// # Errors
    /// Wire encode errors from the reliable layer.
    pub fn poll(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = self.poll_volatile(now)?;
        out.extend(self.poll_stateless(now)?);
        Ok(out)
    }

    /// The ParticipantVolatileMessageSecure part of [`Self::poll`]: the
    /// writer's HEARTBEATs and repairs, the reader's ACKNACK/NACK_FRAG. Each
    /// datagram is addressed to one peer with an INFO_DST, and is protected
    /// with that peer's key before it goes out (§7.5.4).
    ///
    /// # Errors
    /// Wire encode errors from the reliable layer.
    pub fn poll_volatile(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        let mut out = self.volatile_writer.tick(now)?;
        out.extend(self.volatile_reader.tick_outbound(now)?);
        Ok(out)
    }

    /// The ParticipantStatelessMessage part of [`Self::poll`]: the pending
    /// handshake messages sent again. Not protected (§7.4.3).
    ///
    /// # Errors
    /// Wire encode errors.
    pub fn poll_stateless(&mut self, now: Duration) -> Result<Vec<OutboundDatagram>, WireError> {
        #[allow(unused_mut)]
        let mut out = Vec::new();
        #[cfg(not(feature = "std"))]
        let _ = now;
        // DDS-Security §7.4.4: the stateless handshake channel is BEST_EFFORT;
        // reliability comes from periodically re-sending the pending handshake
        // message (initiator: AUTH_REQUEST, replier: cached reply) until the
        // secret is derived. The receiver is idempotent (duplicate request →
        // cached reply, no DH regeneration). Without this re-send, reply/final
        // are lost over the lossy channel and the handshake stalls —
        // independent of the separately fixed discovery coupling.
        #[cfg(feature = "std")]
        {
            self.clock = now;
            self.drain_auth_events();
            self.check_identities_once_a_second();
            self.expire_handshakes(now);
            let retried = self.run_retries(now);
            if !retried.is_empty() {
                // A message that was just sent starts the timer of the resend
                // (§9.3.2.9.1 rule 2).
                self.last_handshake_resend = now;
            }
            out.extend(retried);
            // §9.3.2.9.1 rule 3: the message a refused reply or final was the
            // answer to goes out again.
            for msg in core::mem::take(&mut self.resend_now).into_values() {
                out.extend(self.stateless_writer.write(&msg)?);
            }
        }
        #[cfg(feature = "std")]
        if now.saturating_sub(self.last_handshake_resend) >= self.retry_timeout {
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

    /// Takes the events the plugin reported to the stack's listener
    /// (§9.3.2.12): a revoked identity ends the authentication of the
    /// participant (§9.3.2.12.2), a changed identity status is to be announced
    /// (§9.3.2.12.1).
    #[cfg(feature = "std")]
    fn drain_auth_events(&mut self) {
        let events = match self.auth_events.lock() {
            Ok(mut events) => core::mem::take(&mut *events),
            Err(poisoned) => core::mem::take(&mut *poisoned.into_inner()),
        };
        for event in events {
            match event {
                AuthEvent::Revoked(handle) => {
                    if Some(handle) == self.local_identity {
                        // The local identity is gone: nobody is authenticated
                        // with it any more.
                        self.local_identity_revoked = true;
                        let prefixes: Vec<GuidPrefix> = self.handshakes.keys().copied().collect();
                        for prefix in prefixes {
                            self.end_authentication(prefix, UnauthenticatedReason::Revoked);
                        }
                        continue;
                    }
                    let prefix = self
                        .handshakes
                        .iter()
                        .find(|(_, peer)| peer.remote_identity == handle)
                        .map(|(prefix, _)| *prefix);
                    if let Some(prefix) = prefix {
                        self.end_authentication(prefix, UnauthenticatedReason::Revoked);
                    }
                }
                AuthEvent::StatusChanged(handle, AuthStatusKind::IdentityStatus) => {
                    if Some(handle) == self.local_identity {
                        self.identity_status_changed = true;
                    }
                }
                AuthEvent::StatusChanged(..) => {}
            }
        }
    }

    /// Ends the authentication of `prefix` for good: its state goes (the
    /// plugin already took the revoked identity back) and, for a revoked
    /// identity, it is not authenticated again while it is known.
    #[cfg(feature = "std")]
    fn end_authentication(&mut self, prefix: GuidPrefix, reason: UnauthenticatedReason) {
        self.handshakes.remove(&prefix);
        self.resend_now.remove(&prefix);
        if reason == UnauthenticatedReason::Revoked {
            self.revoked.insert(prefix);
            self.revoked_peers.push(prefix);
        }
        self.unauthenticated.insert(prefix, reason);
    }

    /// Calls `check_identities` of the plugin with the time, at most once a
    /// second: the plugin finds out which identity expired or was revoked.
    #[cfg(feature = "std")]
    fn check_identities_once_a_second(&mut self) {
        let Some(auth) = self.auth.clone() else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if now == self.last_identity_check_secs {
            return;
        }
        self.last_identity_check_secs = now;
        if let Ok(mut plugin) = lock_auth(&auth) {
            plugin.check_identities(now);
        }
    }

    /// A handshake that is not complete after the authentication timeout
    /// ends (AuthenticationTimeoutWait of Figure 9): the participant is
    /// unauthenticated; the next SPDP announcement starts it again.
    #[cfg(feature = "std")]
    fn expire_handshakes(&mut self, now: Duration) {
        let expired: Vec<GuidPrefix> = self
            .handshakes
            .iter()
            .filter(|(_, peer)| {
                peer.secret.is_none()
                    && !peer.authenticated
                    && now.saturating_sub(peer.started_at) >= self.authentication_timeout
            })
            .map(|(prefix, _)| *prefix)
            .collect();
        for prefix in expired {
            let Some(peer) = self.handshakes.remove(&prefix) else {
                continue;
            };
            if let (Some(auth), Some(local_identity)) = (self.auth.clone(), self.local_identity) {
                self.release_peer_handles(&auth, local_identity, prefix, &peer);
            }
            self.resend_now.remove(&prefix);
            self.unauthenticated
                .insert(prefix, UnauthenticatedReason::Timeout);
            self.failures.push((
                prefix,
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "authentication timeout: the handshake did not complete",
                ),
            ));
        }
    }

    /// Whether the ParticipantVolatileMessageSecure reader of
    /// `remote_prefix` is matched with this side's writer.
    #[must_use]
    pub fn volatile_matched(&self, remote_prefix: GuidPrefix) -> bool {
        let reader = Guid::new(
            remote_prefix,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
        );
        self.volatile_writer
            .inner()
            .reader_proxies()
            .iter()
            .any(|p| p.remote_reader_guid == reader)
    }

    /// Returns the `SharedSecretHandle` of a peer once the handshake is
    /// complete on this side (otherwise `None`). FU2: lets the DCPS layer
    /// (and tests) check whether a peer has been authenticated.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_secret(&self, remote_prefix: GuidPrefix) -> Option<SharedSecretHandle> {
        self.handshakes.get(&remote_prefix).and_then(|p| p.secret)
    }

    /// The `IdentityHandle` the plugin made for the participant `remote_prefix`
    /// (`validate_remote_identity`, §9.3.2.11.3), while it is known.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_identity(&self, remote_prefix: GuidPrefix) -> Option<IdentityHandle> {
        self.handshakes
            .get(&remote_prefix)
            .map(|p| p.remote_identity)
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
    /// discovered peer (Spec §9.3.2). Called from the SPDP hot path after
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

    /// Records the `message_auth.supported_mask` a remote participant announced
    /// (or the default of the specification), the signatures it verifies in
    /// the handshake; the authentication plugin signs with one of them.
    #[cfg(feature = "std")]
    pub fn note_remote_message_auth_supported(&mut self, remote_prefix: GuidPrefix, mask: u32) {
        self.remote_message_auth.insert(remote_prefix, mask);
    }

    /// Records the class id of the permissions_token a remote participant
    /// announced in SPDP (`PID_PERMISSIONS_TOKEN`, a CDR DataHolder). A
    /// missing or unreadable token leaves none recorded.
    #[cfg(feature = "std")]
    pub fn note_remote_permissions_token(
        &mut self,
        remote_prefix: GuidPrefix,
        token: Option<&[u8]>,
    ) {
        match token.and_then(|blob| DataHolder::from_cdr_le(blob).ok()) {
            Some(holder) => {
                self.remote_permissions_tokens
                    .insert(remote_prefix, holder.class_id);
            }
            None => {
                self.remote_permissions_tokens.remove(&remote_prefix);
            }
        }
    }

    /// The class id of the SPDP permissions_token of the peer at
    /// `remote_prefix`, see [`Self::note_remote_permissions_token`].
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_permissions_token_class(
        &self,
        remote_prefix: GuidPrefix,
    ) -> Option<alloc::string::String> {
        self.remote_permissions_tokens.get(&remote_prefix).cloned()
    }

    /// Starts the authentication of a discovered participant
    /// (`validate_remote_identity`, §9.3.2.11.3) and, as the plugin's result
    /// says, the handshake: `VALIDATION_PENDING_HANDSHAKE_REQUEST` — this
    /// side sends the request; `VALIDATION_PENDING_HANDSHAKE_MESSAGE` — it
    /// waits for the request of the peer; `VALIDATION_PENDING_RETRY` — it
    /// calls the operation again after the retry timeout; `VALIDATION_OK` —
    /// the peer is authenticated without a handshake.
    ///
    /// # Errors
    /// `SecurityError` on failed remote identity validation or wire
    /// encode of the request token. The participant is unauthenticated then
    /// ([`Self::authentication_status`]); the next call tries again.
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
        // A participant whose identity was revoked is not authenticated
        // again (§9.3.2.12.2).
        if self.revoked.contains(&remote_prefix) {
            return Ok(Vec::new());
        }
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
        self.unauthenticated.remove(&remote_prefix);
        let validation = {
            let mut plugin = lock_auth(&auth)?;
            self.prime_plugin(&mut *plugin, remote_prefix);
            // §9.3.2.11.3: the AuthRequestMessageToken the participant sent
            // before its discovery is the one that caused the authentication
            // to begin.
            let remote_auth_request = self.pending_auth_requests.remove(&remote_prefix);
            plugin.validate_remote_identity(
                local_identity,
                remote_guid,
                remote_identity_token,
                remote_auth_request.as_deref(),
            )
        };
        let validation = match validation {
            Ok(validation) => validation,
            Err(e) => {
                // §9.3.2.10: whatever the reason, the participant is treated
                // as unauthenticated.
                self.unauthenticated.insert(
                    remote_prefix,
                    UnauthenticatedReason::IdentityRejected(alloc::format!("{e}")),
                );
                return Err(e);
            }
        };

        let mut peer = PeerHandshake::new(
            validation.identity,
            remote_guid,
            remote_identity_token.to_vec(),
            self.clock,
        );

        let mut datagrams = Vec::new();
        // §9.3.2.11.3: a local_auth_request_token other than TokenNIL is sent
        // on the stateless writer, ahead of a request; the challenge it
        // announces is the one this side then uses (Table 55).
        if let Some(token) = validation.local_auth_request_token {
            let sent = peer
                .build_message(
                    self.stateless_writer.next_message_identity(),
                    class_id::AUTH_REQUEST,
                    token,
                    MessageIdentity::default(),
                )
                .and_then(|msg| self.stateless_writer.write(&msg).map_err(wire_to_security));
            match sent {
                Ok(sent) => datagrams = sent,
                Err(e) => {
                    self.fail_peer(&auth, local_identity, remote_prefix, &peer, &e);
                    return Err(e);
                }
            }
        }
        if let Err(e) = self.continue_validation(
            &auth,
            local_identity,
            remote_prefix,
            &mut peer,
            validation.result,
            &mut datagrams,
        ) {
            self.fail_peer(&auth, local_identity, remote_prefix, &peer, &e);
            return Err(e);
        }
        self.handshakes.insert(remote_prefix, peer);
        Ok(datagrams)
    }

    /// The settings the plugin takes per peer before a step with it (the
    /// vendor of the peer decides how the algorithm names are written, and the
    /// peer announced which signatures it verifies).
    #[cfg(feature = "std")]
    fn prime_plugin(&self, plugin: &mut dyn AuthenticationPlugin, remote_prefix: GuidPrefix) {
        // Vendor quirk: OpenDDS requires NUL-terminated
        // c.dsign_algo/c.kagree_algo (sizeof comparison).
        plugin.set_algo_nul_terminate(
            self.algo_name_nul
                .nul_for(self.remote_vendors.get(&remote_prefix) == Some(&VendorId::OPENDDS)),
        );
        plugin.set_remote_message_auth_supported(
            self.remote_message_auth.get(&remote_prefix).copied(),
        );
        // Cyclone, Fast DDS and OpenDDS read the algorithm names of DDS-Security
        // 1.1 (`ECDSA-SHA256`, `ECDH+prime256v1-CEUM`); every other peer is
        // spoken to in the names of 1.2 (Table 25, Table 26).
        let legacy = matches!(
            self.remote_vendors.get(&remote_prefix),
            Some(&VendorId::CYCLONE | &VendorId::FASTDDS | &VendorId::OPENDDS)
        );
        plugin.set_peer_algorithm_names(if legacy {
            PeerAlgorithmNames::Legacy
        } else {
            PeerAlgorithmNames::Spec
        });
    }

    /// What follows `validate_remote_identity` (Figure 9) for the `result`
    /// the plugin returned.
    #[cfg(feature = "std")]
    fn continue_validation(
        &mut self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        local_identity: IdentityHandle,
        remote_prefix: GuidPrefix,
        peer: &mut PeerHandshake,
        result: ValidationResult,
        datagrams: &mut Vec<OutboundDatagram>,
    ) -> SecurityResult<()> {
        match result {
            ValidationResult::PendingHandshakeRequest => {
                let stepped = {
                    let mut plugin = lock_auth(auth)?;
                    self.prime_plugin(&mut *plugin, remote_prefix);
                    plugin.begin_handshake_request(
                        local_identity,
                        peer.remote_identity,
                        &self.local_pdata,
                    )
                };
                let (handle, outcome) = stepped?;
                peer.handshake = Some(handle);
                // §9.3.2.11.4: the related_message_identity of a request is
                // GUID_UNKNOWN with sequence number zero.
                self.apply_outcome(
                    peer,
                    outcome,
                    RetryAction::BeginRequest,
                    MessageIdentity::default(),
                    datagrams,
                )
            }
            // The request of the peer starts the handshake.
            ValidationResult::PendingHandshakeMessage => Ok(()),
            ValidationResult::Ok => {
                peer.authenticated = true;
                Ok(())
            }
            ValidationResult::PendingRetry => {
                peer.retry = Some(ScheduledRetry {
                    due: self.clock.saturating_add(self.retry_timeout),
                    action: RetryAction::ValidateRemote,
                });
                Ok(())
            }
            other => Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("validate_remote_identity returned {other:?}"),
            )),
        }
    }

    /// Acts on the outcome of a handshake step (§9.3.2.11.4 to §9.3.2.11.6):
    /// the message to send is wrapped, cached for the resend (§9.3.2.9.1 rule
    /// 2) and written; `VALIDATION_PENDING_RETRY` schedules `again`.
    #[cfg(feature = "std")]
    fn apply_outcome(
        &mut self,
        peer: &mut PeerHandshake,
        outcome: HandshakeStepOutcome,
        again: RetryAction,
        related: MessageIdentity,
        datagrams: &mut Vec<OutboundDatagram>,
    ) -> SecurityResult<()> {
        match outcome {
            HandshakeStepOutcome::SendMessage { token }
            | HandshakeStepOutcome::SendFinalMessage { token } => {
                // OMG DDS-Security §7.4.4: ALL handshake tokens (request/reply/
                // final) travel in the message_class_id `dds.sec.auth`; only the
                // AuthRequestMessageToken uses `dds.sec.auth_request`. The
                // HandshakeRequest has related_message_identity = NIL
                // (= MessageIdentity::default()). Confirmed on the wire against
                // FastDDS/Cyclone.
                let msg = peer.build_message(
                    self.stateless_writer.next_message_identity(),
                    class_id::AUTH,
                    token,
                    related,
                )?;
                // Remember for the resend path (periodic beacon cadence).
                peer.last_request = Some(msg.clone());
                datagrams.extend(
                    self.stateless_writer
                        .write(&msg)
                        .map_err(wire_to_security)?,
                );
                peer.retry = None;
                Ok(())
            }
            HandshakeStepOutcome::Complete { .. } | HandshakeStepOutcome::WaitingForPeer => {
                peer.retry = None;
                Ok(())
            }
            HandshakeStepOutcome::PendingRetry => {
                peer.retry = Some(ScheduledRetry {
                    due: self.clock.saturating_add(self.retry_timeout),
                    action: again,
                });
                Ok(())
            }
            other => Err(SecurityError::new(
                SecurityErrorKind::Internal,
                alloc::format!("handshake step returned {other:?}"),
            )),
        }
    }

    /// The handshake with `peer` failed: its identity goes back to the plugin
    /// (§9.3.2.11.18) and the participant is unauthenticated (§9.3.2.10).
    #[cfg(feature = "std")]
    fn fail_peer(
        &mut self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        local_identity: IdentityHandle,
        remote_prefix: GuidPrefix,
        peer: &PeerHandshake,
        error: &SecurityError,
    ) {
        self.release_peer_handles(auth, local_identity, remote_prefix, peer);
        self.unauthenticated.insert(
            remote_prefix,
            UnauthenticatedReason::HandshakeFailed(alloc::format!("{error}")),
        );
    }

    /// Gives the handles of `peer` back to the plugin: its handshake, its
    /// shared secret and its identity (§9.3.2.11.17, §9.3.2.11.19,
    /// §9.3.2.11.18). A refusal of the plugin is reported with the handshake
    /// failures ([`Self::take_handshake_failures`]).
    #[cfg(feature = "std")]
    fn release_peer_handles(
        &mut self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        local_identity: IdentityHandle,
        remote_prefix: GuidPrefix,
        peer: &PeerHandshake,
    ) {
        let mut refused: Vec<SecurityError> = Vec::new();
        match lock_auth(auth) {
            Ok(mut plugin) => {
                if let Some(secret) = peer.secret {
                    refused.extend(plugin.return_sharedsecret_handle(secret).err());
                }
                if let Some(handshake) = peer.handshake {
                    refused.extend(plugin.return_handshake_handle(handshake).err());
                }
                if peer.remote_identity != local_identity
                    && peer.remote_identity != IdentityHandle::NIL
                {
                    refused.extend(plugin.return_identity_handle(peer.remote_identity).err());
                }
            }
            Err(e) => refused.push(e),
        }
        self.failures
            .extend(refused.into_iter().map(|e| (remote_prefix, e)));
    }

    /// The state of the authentication of the participant `remote_prefix`.
    /// A participant that is not authenticated, whatever the reason
    /// ([`UnauthenticatedReason`]), is one case (§9.3.2.10).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn authentication_status(&self, remote_prefix: GuidPrefix) -> PeerAuthentication {
        if let Some(peer) = self.handshakes.get(&remote_prefix) {
            return if peer.secret.is_some() || peer.authenticated {
                PeerAuthentication::Authenticated
            } else {
                PeerAuthentication::InProgress
            };
        }
        match self.unauthenticated.get(&remote_prefix) {
            Some(reason) => PeerAuthentication::Unauthenticated(reason.clone()),
            None => PeerAuthentication::Unauthenticated(UnauthenticatedReason::NoIdentityToken),
        }
    }

    /// Whether the participant `remote_prefix` is authenticated.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn is_authenticated(&self, remote_prefix: GuidPrefix) -> bool {
        self.authentication_status(remote_prefix) == PeerAuthentication::Authenticated
    }

    /// Records that the participant `remote_prefix` is unauthenticated for
    /// `reason`, without the stack having tried to authenticate it: it
    /// announced no IdentityToken, or its plugin configuration is
    /// incompatible with this one's (§9.3.2.10). It is one case with the
    /// participants whose handshake failed.
    #[cfg(feature = "std")]
    pub fn mark_unauthenticated(
        &mut self,
        remote_prefix: GuidPrefix,
        reason: UnauthenticatedReason,
    ) {
        if !self.handshakes.contains_key(&remote_prefix) {
            self.unauthenticated.insert(remote_prefix, reason);
        }
    }

    /// The participants whose identity was revoked since the last call
    /// (`AuthenticationListener::on_revoke_identity`, §9.3.2.12.2): "the DDS
    /// middleware shall terminate any communications" with them. The DDS
    /// implementation ends the communication and makes new key material.
    #[cfg(feature = "std")]
    pub fn take_revoked_peers(&mut self) -> Vec<GuidPrefix> {
        core::mem::take(&mut self.revoked_peers)
    }

    /// The current IdentityStatusToken of the local participant
    /// (`get_identity_status_token`, §9.3.2.11.10; empty is TokenNIL). The
    /// token the plugin issued goes back to it (§9.3.2.11.15).
    ///
    /// # Errors
    /// The plugin's error; `BadArgument` without a plugin.
    #[cfg(feature = "std")]
    pub fn identity_status_token(&mut self) -> SecurityResult<Vec<u8>> {
        let (Some(auth), Some(local_identity)) = (self.auth.clone(), self.local_identity) else {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "no authentication plugin",
            ));
        };
        let mut plugin = lock_auth(&auth)?;
        let token = plugin.get_identity_status_token(local_identity)?;
        plugin.return_identity_status_token(&token)?;
        Ok(token)
    }

    /// Whether the IdentityStatusToken of the local participant changed
    /// since the last call (`AuthStatusKind::IdentityStatus`, §9.3.2.12.1):
    /// the DDS implementation then announces the new token in
    /// DCPSParticipantsSecure.
    #[cfg(feature = "std")]
    pub fn take_identity_status_changed(&mut self) -> bool {
        core::mem::take(&mut self.identity_status_changed)
    }

    /// Whether the identity of the local participant was revoked.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn local_identity_revoked(&self) -> bool {
        self.local_identity_revoked
    }

    /// The handshake failures found by [`Self::poll`] (a timeout, an
    /// operation that failed on a retry), with the peer.
    #[cfg(feature = "std")]
    pub fn take_handshake_failures(&mut self) -> Vec<(GuidPrefix, SecurityError)> {
        core::mem::take(&mut self.failures)
    }

    /// Number of remote peers with established handshake state (initiator
    /// as well as replier). Read-only observability for the discovery
    /// trigger.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn handshake_peer_count(&self) -> usize {
        self.handshakes.len()
    }

    /// The permissions document (`c.perm`) the peer presented in its
    /// handshake, if captured: from its AuthenticatedPeerCredentialToken
    /// (§10.3.2.3 Table 54). The DCPS layer forwards it to the
    /// access-control gate (`set_remote_permissions`) for the remote
    /// endpoint checks (DDS-Security 1.2 §9.4.2.9.10 / §9.4.2.9.11).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_permissions(&self, remote_prefix: GuidPrefix) -> Option<Vec<u8>> {
        self.handshakes
            .get(&remote_prefix)
            .and_then(|p| p.permissions.clone())
    }

    /// The DER certificate the peer at `remote_prefix` authenticated with,
    /// once its handshake completed (`c.id` of its
    /// AuthenticatedPeerCredentialToken). Its SubjectName binds the peer's
    /// permissions (DDS-Security 1.2 §10.4.1.5.3.1).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_identity_certificate(&self, remote_prefix: GuidPrefix) -> Option<Vec<u8>> {
        self.handshakes
            .get(&remote_prefix)
            .and_then(|p| p.identity_certificate.clone())
    }

    /// The `AuthenticatedPeerCredentialToken` of the peer at `remote_prefix`
    /// (DDS-Security 1.2 §10.3.2.3, Table 54): the DER certificate it
    /// authenticated with (`c.id`) and the permissions document it presented
    /// (`c.perm`). The AccessControl plugin reads both in
    /// `validate_remote_permissions` (§9.4.2.9.2). `None` until the handshake
    /// completed and when the peer presented no `c.perm`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_credential_token(&self, remote_prefix: GuidPrefix) -> Option<DataHolder> {
        let peer = self.handshakes.get(&remote_prefix)?;
        let certificate = peer.identity_certificate.clone()?;
        let permissions = peer.permissions.clone()?;
        Some(
            DataHolder::new("DDS:Auth:PKI-DH:1.0")
                .with_binary_property("c.id", certificate)
                .with_binary_property("c.perm", permissions),
        )
    }

    /// The `IdentityHandle` that `validate_remote_identity` gave the peer at
    /// `remote_prefix`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn peer_identity_handle(&self, remote_prefix: GuidPrefix) -> Option<IdentityHandle> {
        self.handshakes
            .get(&remote_prefix)
            .map(|p| p.remote_identity)
    }

    /// The `IdentityHandle` of the local participant, once the stack has one.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn local_identity_handle(&self) -> Option<IdentityHandle> {
        self.local_identity
    }

    /// FU2 handshake driver: processes an incoming stateless auth
    /// message (Spec §7.5.3). Dispatch by `message_class_id`:
    ///
    /// - `AUTH_REQUEST`: the AuthRequestMessageToken, passed to the plugin
    ///   (§9.3.2.9.1 rule 1) — with `validate_remote_identity`.
    /// - `AUTH` without `related_message_identity` (replier side) →
    ///   `begin_handshake_reply`, sends the reply token back.
    /// - otherwise (`AUTH` = reply at the initiator / final at the
    ///   replier) → `process_handshake`; a message the plugin refuses leaves
    ///   the handshake as it was and the cached message goes out again
    ///   (§9.3.2.9.1 rule 3).
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
        // DDS-Security 1.2 §7.5.3.4: a message whose
        // destination_participant_guid names another participant is not for
        // this reader ("destination_participant_guid == GUID_UNKNOWN ||
        // destination_participant_guid == ...participant.guid").
        if msg.destination_participant_key != [0; 16]
            && msg.destination_participant_key != local_guid
        {
            return Ok((Vec::new(), None));
        }
        // §7.5.3.4: a `destination_endpoint_guid` other than GUID_UNKNOWN
        // targets that endpoint of the destination participant; the only one
        // this stack reads is its BuiltinParticipantStatelessMessageReader.
        if msg.destination_endpoint_key != [0; 16]
            && msg.destination_endpoint_key != self.stateless_reader.guid().to_bytes()
        {
            return Ok((Vec::new(), None));
        }
        // §9.3.2.11.5, §9.3.2.11.6: a handshake message is taken from the
        // participant its message_identity names (the participant or its
        // stateless writer, §7.5.3.3: one prefix).
        if msg.message_identity.source_guid[..12] != remote_prefix.to_bytes() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "stateless: message_identity.source_guid is not the sender's",
            ));
        }
        // A participant whose identity was revoked is not heard
        // (§9.3.2.12.2: "terminate any communications").
        if self.revoked.contains(&remote_prefix) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "stateless: the identity of the sender was revoked",
            ));
        }
        if msg.message_class_id == class_id::AUTH_REQUEST {
            return self
                .on_auth_request(&auth, local_identity, remote_prefix, msg)
                .map(|()| (Vec::new(), None));
        }
        // OMG DDS-Security §7.4.4: a HandshakeRequest is message_class
        // `dds.sec.auth` WITH related_message_identity == NIL. Reply/final are
        // also `dds.sec.auth`, but have related != NIL. (The separate
        // AuthRequestMessageToken uses `dds.sec.auth_request` — it is not
        // treated as a handshake request here.) This replaces the earlier
        // dispatch on the message class, which was cross-vendor incorrect.
        let is_request = msg.message_class_id == class_id::AUTH
            && msg.related_message_identity == MessageIdentity::default();
        let known = self.handshakes.contains_key(&remote_prefix);
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
                // The peer is authenticated under the prefix of the RTPS
                // header it sent from, so the `c.pdata` GUID of a PKI-DH
                // request is bound to that prefix, as to the one it is
                // discovered with (DDS-Security 1.2 §7.4.3, §10.3.3 Table
                // 59). The identity of an undiscovered peer carries it; it
                // took the local handle as a placeholder, and
                // `begin_handshake_reply` had no GUID to compare: a
                // participant with a valid certificate could authenticate
                // under another's prefix before that one's SPDP arrived. The
                // descriptor is the request's class without `+Req`, so the
                // reply speaks its version. Other classes (PSK) bind no GUID
                // and keep the placeholder.
                let Some(request) = msg.message_data.first() else {
                    return Ok((Vec::new(), None));
                };
                let remote_guid = Guid::new(remote_prefix, EntityId::PARTICIPANT).to_bytes();
                let (remote_identity, remote_identity_token) = match request
                    .class_id
                    .strip_suffix("+Req")
                    .filter(|class| class.starts_with("DDS:Auth:PKI-DH:"))
                {
                    // An AuthRequest it sent before is the one to check its
                    // challenge1 against; none goes out, the reply follows
                    // at once.
                    Some(class) => {
                        let token = DataHolder::new(class).to_cdr_le();
                        let validated = lock_auth(&auth)?.validate_remote_identity(
                            local_identity,
                            remote_guid,
                            &token,
                            self.pending_auth_requests.remove(&remote_prefix).as_deref(),
                        );
                        match validated {
                            Ok(validated) => (validated.identity, token),
                            Err(e) => {
                                self.unauthenticated.insert(
                                    remote_prefix,
                                    UnauthenticatedReason::IdentityRejected(alloc::format!("{e}")),
                                );
                                return Err(e);
                            }
                        }
                    }
                    // Ignored by the plugins that bind no GUID; after
                    // completion only passed to the secret-based crypto
                    // register (the Kx key comes from HKDF(shared_secret)).
                    None => (local_identity, Vec::new()),
                };
                PeerHandshake::new(
                    remote_identity,
                    // Destination of the reply tokens: the participant GUID
                    // (§9.3.2.11.5, "destination_participant_guid shall be set
                    // to match the DDS GUID_t of the destination
                    // DomainParticipant"). The request's source_guid may be its
                    // stateless writer's (Fast DDS), and Fast DDS drops a
                    // message addressed to that.
                    remote_guid,
                    remote_identity_token,
                    self.clock,
                )
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

        // The mirror case: a reply after our side is complete means the
        // replier did not get our final and is re-sending its reply. Re-send
        // the cached final. `process_handshake` cannot answer it — the plugin
        // dropped the handshake state on completion ("unknown
        // HandshakeHandle") — and without the final the replier waits for
        // ever while this side counts the peer as authenticated.
        //
        // A final after completion is the replier's case, and it asks for
        // nothing: the initiator re-sent it, or a copy arrived over a second
        // locator. Answering it with the cached reply made the initiator
        // answer with its final again, and two ZeroDDS participants traded
        // reply and final without end — 143 262 replies and 162 991 finals
        // in one secured bench cell, which crowded out their discovery.
        let is_final = msg
            .message_data
            .first()
            .is_some_and(|dh| dh.class_id.ends_with("+Final"));
        if !is_request && is_final && peer.secret.is_some() {
            self.handshakes.insert(remote_prefix, peer);
            return Ok((Vec::new(), None));
        }
        if !is_request && peer.secret.is_some() {
            let resend = peer.last_request.clone();
            self.handshakes.insert(remote_prefix, peer);
            return match resend {
                Some(f) => self
                    .stateless_writer
                    .write(&f)
                    .map(|d| (d, None))
                    .map_err(wire_to_security),
                None => Ok((Vec::new(), None)),
            };
        }

        // §9.3.2.11.6: process_handshake is called for a message whose
        // related_message_identity matches "the message_identity of the last
        // ParticipantStatelessMessage sent to the peer". A message that does
        // not is none for the handshake; it leaves it as it was.
        if !is_request && peer.handshake.is_some() {
            let last_sent = peer.last_request.as_ref().map(|m| &m.message_identity);
            if last_sent != Some(&msg.related_message_identity) {
                self.handshakes.insert(remote_prefix, peer);
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "stateless: related_message_identity is not the last message sent",
                ));
            }
        }
        if !is_request && peer.handshake.is_none() {
            self.handshakes.insert(remote_prefix, peer);
            return Ok((Vec::new(), None));
        }

        let mut datagrams = Vec::new();
        let stepped = {
            let mut plugin = match lock_auth(&auth) {
                Ok(p) => p,
                Err(e) => {
                    self.handshakes.insert(remote_prefix, peer);
                    return Err(e);
                }
            };
            self.prime_plugin(&mut *plugin, remote_prefix);
            // The setting `mirror`: the replier answers in the form of the
            // request. A request whose algorithm names have no NUL (Fast DDS)
            // comes from a peer that compares the names it gets back with its
            // own. The default (`spec`) answers with the NUL (§7.3.3.1).
            if is_request && self.algo_name_nul == zerodds_security::AlgoNameNul::Mirror {
                let request_without_nul = msg
                    .message_data
                    .first()
                    .and_then(|dh| dh.binary_property("c.dsign_algo"))
                    .is_some_and(|name| name.last() != Some(&0));
                if request_without_nul {
                    plugin.set_algo_nul_terminate(false);
                }
            }
            if is_request {
                plugin
                    .begin_handshake_reply(
                        local_identity,
                        peer.remote_identity,
                        &token,
                        &self.local_pdata,
                    )
                    .map(|(handle, outcome)| {
                        peer.handshake = Some(handle);
                        outcome
                    })
            } else if let Some(handle) = peer.handshake {
                plugin.process_handshake(handle, &token)
            } else {
                Ok(HandshakeStepOutcome::WaitingForPeer)
            }
        };
        // A refused step leaves the handshake as it was: the plugin keeps its
        // state over a forged reply or final (§9.3.2.11.5, §9.3.2.11.6), and
        // the genuine one that follows needs this peer to reach it. A peer
        // first seen with a refused request is not kept.
        // §9.3.2.9.1 rule 3: the state machine goes back to the state that
        // sent the message and sends it again.
        let outcome = match stepped {
            Ok(outcome) => outcome,
            Err(e) => {
                if known || !is_request {
                    if let Some(cached) = peer.last_request.clone() {
                        self.resend_now.insert(remote_prefix, cached);
                    }
                    self.handshakes.insert(remote_prefix, peer);
                } else {
                    // The identity made for it goes back (§9.3.2.11.18);
                    // the step's error is the one reported.
                    self.fail_peer(&auth, local_identity, remote_prefix, &peer, &e);
                }
                return Err(e);
            }
        };

        let again = if is_request {
            RetryAction::BeginReply {
                request: token.clone(),
                related: msg.message_identity.clone(),
            }
        } else {
            RetryAction::Process {
                token: token.clone(),
                related: msg.message_identity.clone(),
            }
        };
        if let Err(e) = self.apply_outcome(
            &mut peer,
            outcome,
            again,
            msg.message_identity.clone(),
            &mut datagrams,
        ) {
            self.fail_peer(&auth, local_identity, remote_prefix, &peer, &e);
            return Err(e);
        }

        let completed = match self.collect_completion(&auth, &mut peer) {
            Ok(completed) => completed,
            Err(e) => {
                self.fail_peer(&auth, local_identity, remote_prefix, &peer, &e);
                return Err(e);
            }
        };
        self.handshakes.insert(remote_prefix, peer);
        Ok((datagrams, completed))
    }

    /// Secret extraction: after each step, check whether the plugin can
    /// now derive the SharedSecret. The initiator obtains it after the
    /// reply (`VALIDATION_OK_FINAL_MESSAGE`), the replier on `VALIDATION_OK`
    /// — `get_shared_secret` covers both cases (§9.3.2.11.7). Then the
    /// AuthenticatedPeerCredentialToken is taken (§9.3.2.11.8): the peer's
    /// permissions document and certificate, for the access-control gate's
    /// remote endpoint checks.
    #[cfg(feature = "std")]
    fn collect_completion(
        &self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        peer: &mut PeerHandshake,
    ) -> SecurityResult<Option<(IdentityHandle, SharedSecretHandle)>> {
        if peer.secret.is_some() {
            return Ok(None);
        }
        let Some(handle) = peer.handshake else {
            return Ok(None);
        };
        let mut plugin = lock_auth(auth)?;
        let Ok(secret) = plugin.shared_secret(handle) else {
            return Ok(None);
        };
        let token = plugin.get_authenticated_peer_credential_token(handle)?;
        let credential = DataHolder::from_cdr_le(&token)?;
        plugin.return_authenticated_peer_credential_token(&token)?;
        peer.permissions = credential
            .binary_property("c.perm")
            .filter(|perm| !perm.is_empty())
            .map(<[u8]>::to_vec);
        peer.identity_certificate = credential.binary_property("c.id").and_then(certificate_der);
        peer.secret = Some(secret);
        Ok(Some((peer.remote_identity, secret)))
    }

    /// An AuthRequestMessageToken (`dds.sec.auth_request`, §9.3.2.5): the
    /// peer announces the challenge it will use. §9.3.2.9.1 rule 1: "The
    /// DDS security implementation shall pass to the AuthenticationPlugin any
    /// message received [...] that has a GenericMessageClassId set to
    /// GMCLASSID_SECURITY_AUTH_REQUEST". The plugin takes it as the
    /// `remote_auth_request_token` of `validate_remote_identity`, §9.3.2.11.3:
    /// the token "that caused the authentication to begin". A participant
    /// not yet discovered has none to validate: the token is kept for the
    /// validation its discovery causes. For a participant whose handshake
    /// did not begin yet, the identity is validated again with the token and
    /// replaces the first one; after the handshake began, the plugin is
    /// given the token (it checks it) and the identity this made is given
    /// back, since the challenges of the handshake are fixed.
    #[cfg(feature = "std")]
    fn on_auth_request(
        &mut self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        local_identity: IdentityHandle,
        remote_prefix: GuidPrefix,
        msg: &ParticipantGenericMessage,
    ) -> SecurityResult<()> {
        let Some(token) = msg.message_data.first() else {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "stateless: an AuthRequest without a token",
            ));
        };
        let token = token.to_cdr_le();
        let Some(mut peer) = self.handshakes.remove(&remote_prefix) else {
            if self.pending_auth_requests.len() >= MAX_PENDING_AUTH_REQUESTS
                && !self.pending_auth_requests.contains_key(&remote_prefix)
            {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "stateless: too many AuthRequests of undiscovered participants",
                ));
            }
            self.pending_auth_requests.insert(remote_prefix, token);
            return Ok(());
        };
        let validated = {
            let mut plugin = match lock_auth(auth) {
                Ok(plugin) => plugin,
                Err(e) => {
                    self.handshakes.insert(remote_prefix, peer);
                    return Err(e);
                }
            };
            self.prime_plugin(&mut *plugin, remote_prefix);
            let validated = plugin.validate_remote_identity(
                local_identity,
                peer.remote_guid,
                &peer.remote_identity_token,
                Some(&token),
            );
            match validated {
                Ok(validated) if peer.handshake.is_none() && peer.secret.is_none() => {
                    // The identity made for the discovery gives way to the
                    // one made with the token.
                    let first = peer.remote_identity;
                    peer.remote_identity = validated.identity;
                    if first != local_identity && first != IdentityHandle::NIL {
                        plugin.return_identity_handle(first)
                    } else {
                        Ok(())
                    }
                }
                Ok(validated) => plugin.return_identity_handle(validated.identity),
                Err(e) => Err(e),
            }
        };
        self.handshakes.insert(remote_prefix, peer);
        validated
    }

    /// Runs the operations that returned `VALIDATION_PENDING_RETRY` and are
    /// due (Figure 9, RetryWait1 to RetryWait5).
    #[cfg(feature = "std")]
    fn run_retries(&mut self, now: Duration) -> Vec<OutboundDatagram> {
        let (Some(auth), Some(local_identity)) = (self.auth.clone(), self.local_identity) else {
            return Vec::new();
        };
        let due: Vec<GuidPrefix> = self
            .handshakes
            .iter()
            .filter(|(_, peer)| peer.retry.as_ref().is_some_and(|retry| retry.due <= now))
            .map(|(prefix, _)| *prefix)
            .collect();
        let mut datagrams = Vec::new();
        for prefix in due {
            let Some(mut peer) = self.handshakes.remove(&prefix) else {
                continue;
            };
            let Some(retry) = peer.retry.take() else {
                self.handshakes.insert(prefix, peer);
                continue;
            };
            let result = self.retry_step(
                &auth,
                local_identity,
                prefix,
                &mut peer,
                retry.action,
                &mut datagrams,
            );
            match result {
                Ok(()) => {
                    self.handshakes.insert(prefix, peer);
                }
                Err(e) => {
                    self.fail_peer(&auth, local_identity, prefix, &peer, &e);
                    self.failures.push((prefix, e));
                }
            }
        }
        datagrams
    }

    /// One retried operation.
    #[cfg(feature = "std")]
    fn retry_step(
        &mut self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        local_identity: IdentityHandle,
        prefix: GuidPrefix,
        peer: &mut PeerHandshake,
        action: RetryAction,
        datagrams: &mut Vec<OutboundDatagram>,
    ) -> SecurityResult<()> {
        match action {
            RetryAction::ValidateRemote => {
                let validated = {
                    let mut plugin = lock_auth(auth)?;
                    self.prime_plugin(&mut *plugin, prefix);
                    plugin.validate_remote_identity(
                        local_identity,
                        peer.remote_guid,
                        &peer.remote_identity_token,
                        None,
                    )?
                };
                peer.remote_identity = validated.identity;
                if let Some(token) = validated.local_auth_request_token {
                    let msg = peer.build_message(
                        self.stateless_writer.next_message_identity(),
                        class_id::AUTH_REQUEST,
                        token,
                        MessageIdentity::default(),
                    )?;
                    datagrams.extend(
                        self.stateless_writer
                            .write(&msg)
                            .map_err(wire_to_security)?,
                    );
                }
                self.continue_validation(
                    auth,
                    local_identity,
                    prefix,
                    peer,
                    validated.result,
                    datagrams,
                )
            }
            RetryAction::BeginRequest => {
                let stepped = {
                    let mut plugin = lock_auth(auth)?;
                    self.prime_plugin(&mut *plugin, prefix);
                    // The handshake of the step that is retried gives way to
                    // the one this call makes.
                    if let Some(first) = peer.handshake.take() {
                        plugin.return_handshake_handle(first)?;
                    }
                    plugin.begin_handshake_request(
                        local_identity,
                        peer.remote_identity,
                        &self.local_pdata,
                    )?
                };
                peer.handshake = Some(stepped.0);
                self.apply_outcome(
                    peer,
                    stepped.1,
                    RetryAction::BeginRequest,
                    MessageIdentity::default(),
                    datagrams,
                )
            }
            RetryAction::BeginReply { request, related } => {
                let stepped = {
                    let mut plugin = lock_auth(auth)?;
                    self.prime_plugin(&mut *plugin, prefix);
                    if let Some(first) = peer.handshake.take() {
                        plugin.return_handshake_handle(first)?;
                    }
                    plugin.begin_handshake_reply(
                        local_identity,
                        peer.remote_identity,
                        &request,
                        &self.local_pdata,
                    )?
                };
                peer.handshake = Some(stepped.0);
                self.apply_outcome(
                    peer,
                    stepped.1,
                    RetryAction::BeginReply {
                        request,
                        related: related.clone(),
                    },
                    related,
                    datagrams,
                )?;
                self.finish_retried(auth, prefix, peer)
            }
            RetryAction::Process { token, related } => {
                let handle = peer.handshake.ok_or_else(|| {
                    SecurityError::new(SecurityErrorKind::Internal, "retry: no handshake handle")
                })?;
                let outcome = {
                    let mut plugin = lock_auth(auth)?;
                    self.prime_plugin(&mut *plugin, prefix);
                    plugin.process_handshake(handle, &token)?
                };
                self.apply_outcome(
                    peer,
                    outcome,
                    RetryAction::Process {
                        token,
                        related: related.clone(),
                    },
                    related,
                    datagrams,
                )?;
                self.finish_retried(auth, prefix, peer)
            }
        }
    }

    /// A retried step completed the handshake: the stack has no caller to
    /// return the `(identity, secret)` pair to; the secret is in the peer
    /// state, which [`Self::peer_secret`] reports, and the pair is queued for
    /// [`Self::take_completed`].
    #[cfg(feature = "std")]
    fn finish_retried(
        &mut self,
        auth: &Arc<Mutex<dyn AuthenticationPlugin + 'static>>,
        prefix: GuidPrefix,
        peer: &mut PeerHandshake,
    ) -> SecurityResult<()> {
        if let Some((identity, secret)) = self.collect_completion(auth, peer)? {
            self.completed_by_tick.push((prefix, identity, secret));
        }
        Ok(())
    }

    /// The handshakes that completed in a step the tick ran again after
    /// `VALIDATION_PENDING_RETRY`: `(peer, remote identity, shared secret)`,
    /// for the same registration as the `(identity, secret)` pair that
    /// [`Self::on_stateless_message`] returns.
    #[cfg(feature = "std")]
    pub fn take_completed(&mut self) -> Vec<(GuidPrefix, IdentityHandle, SharedSecretHandle)> {
        core::mem::take(&mut self.completed_by_tick)
    }
}

/// The AuthRequestMessageTokens of undiscovered participants the stack
/// keeps at most; one more is refused. Bounds what spoofed prefixes can
/// make it hold.
#[cfg(feature = "std")]
pub const MAX_PENDING_AUTH_REQUESTS: usize = 256;

/// `WireError` → `SecurityError` adapter for the handshake driver.
#[cfg(feature = "std")]
fn wire_to_security(_e: WireError) -> zerodds_security::error::SecurityError {
    zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::BadArgument,
        "stateless handshake: wire encode failed",
    )
}

/// The DER of the certificate in a `c.id`: the PEM block as it travels in
/// the handshake (what Cyclone, Fast DDS and ZeroDDS send, a trailing NUL of
/// OpenDDS and text before the block tolerated), or DER, taken as it is: its
/// last octet belongs to the signature even when it is 0x00.
#[cfg(feature = "std")]
fn certificate_der(c_id: &[u8]) -> Option<Vec<u8>> {
    const BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
    const END: &[u8] = b"-----END CERTIFICATE-----";
    let Some(start) = c_id.windows(BEGIN.len()).position(|w| w == BEGIN) else {
        return (!c_id.is_empty()).then(|| c_id.to_vec());
    };
    let body = &c_id[start + BEGIN.len()..];
    let end = body.windows(END.len()).position(|w| w == END)?;
    base64_decode(&body[..end])
}

/// RFC 4648 base64, with white space between the characters.
#[cfg(feature = "std")]
fn base64_decode(text: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for &c in text {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            _ => return None,
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(accumulator >> bits).ok()?);
            accumulator &= (1u32 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(feature = "std")]
impl PeerHandshake {
    /// Wraps a handshake token into a `ParticipantGenericMessage` with a
    /// running `message_identity` and a referenced
    /// `related_message_identity` (NIL for the initial request).
    fn build_message(
        &mut self,
        message_identity: MessageIdentity,
        message_class: &str,
        token: Vec<u8>,
        related: MessageIdentity,
    ) -> SecurityResult<ParticipantGenericMessage> {
        // The token IS a CDR-LE-serialized DataHolder
        // (build_*_token = DataHolder::to_cdr_le); read it back in so that
        // the generic-message encoding re-serializes it cleanly.
        let holder = DataHolder::from_cdr_le(&token)?;
        Ok(ParticipantGenericMessage {
            message_identity,
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
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A `c.id` in DER whose last octet is 0x00 is taken whole, and its PEM
    /// with the NUL of DDS-Security 1.2 §7.3.3.1 gives the same DER. Cutting
    /// the zeros off DER handed the AccessControl plugin a certificate one
    /// octet short, and it refused the peer's permissions.
    #[cfg(feature = "std")]
    #[test]
    fn a_der_certificate_that_ends_in_zero_is_taken_whole() {
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
        assert_eq!(certificate_der(&der), Some(der.clone()));
        let mut c_id = pem.into_bytes();
        c_id.push(0);
        assert_eq!(certificate_der(&c_id), Some(der));
    }
    use alloc::string::{String, ToString};
    use alloc::vec;
    use zerodds_rtps::participant_data::{
        Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
    };
    use zerodds_rtps::wire_types::ProtocolVersion;
    use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
    use zerodds_security::token::DataHolder;

    // Local node (A) carries the SMALLER GUID, making it the initiator
    // under the cyclone convention (smaller GUID = initiator) — so the
    // "A sends" role assertions stay valid.
    /// The datagrams of `out` with a handshake message (`dds.sec.auth`),
    /// without the AuthRequestMessageToken (`dds.sec.auth_request`) that
    /// goes out ahead of them on discovery.
    fn handshake_only(out: Vec<OutboundDatagram>) -> Vec<OutboundDatagram> {
        out.into_iter()
            .filter(|d| class_of(d) == class_id::AUTH)
            .collect()
    }

    /// The `message_class_id` of the one stateless message `d` carries.
    fn class_of(d: &OutboundDatagram) -> alloc::string::String {
        StatelessMessageReader::new(GuidPrefix::from_bytes([0xEE; 12]), VendorId::ZERODDS)
            .handle_datagram(&d.bytes)
            .unwrap()[0]
            .message_class_id
            .clone()
    }

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
                default_unicast_locators: vec![Locator::udp_v4([127, 0, 0, 99], 7411)],
                default_multicast_locators: Vec::new(),
                metatraffic_unicast_locators: Vec::new(),
                metatraffic_multicast_locators: Vec::new(),
                domain_id: None,
                domain_tag: Default::default(),
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
                available_builtin_endpoints_ext: 0,
                participant_security_info: None,
                expects_inline_qos: false,
                manual_liveliness_count: None,
                builtin_endpoint_qos: None,
                entity_name: None,
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
            destination_participant_key: Guid::new(remote_prefix(), EntityId::PARTICIPANT)
                .to_bytes(),
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
            // The SubjectName, not only the alternative name: the bound GUID
            // prefix starts with its hash (§10.3.3 Table 59), and two leafs
            // with the default subject were bound to one GUID.
            let mut dn = rcgen::DistinguishedName::new();
            dn.push(rcgen::DnType::CommonName, name);
            params.distinguished_name = dn;
            params.is_ca = rcgen::IsCa::NoCa;
            let key = KeyPair::generate().unwrap();
            let cert = params.signed_by(&key, &ca_cert, &ca_key).unwrap();
            (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
        };
        (ca_pem, mint_leaf("alice"), mint_leaf("bob"))
    }

    /// A participant GUID bound to the identity in `cert_pem` (§10.3.3
    /// Table 59).
    #[cfg(feature = "std")]
    fn bound_guid(cert_pem: &[u8]) -> [u8; 16] {
        let prefix = zerodds_security_pki::adjust_participant_guid_prefix(
            &[0x01; 16],
            &cert_der_from_pem(cert_pem),
        )
        .unwrap();
        Guid::new(GuidPrefix::from_bytes(prefix), EntityId::PARTICIPANT).to_bytes()
    }

    /// A `c.pdata`: PL_CDR_BE with PID_PARTICIPANT_GUID and PID_SENTINEL.
    #[cfg(feature = "std")]
    fn pdata_of(guid: [u8; 16]) -> Vec<u8> {
        let mut pdata = std::vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x50, 0x00, 0x10];
        pdata.extend_from_slice(&guid);
        pdata.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        pdata
    }

    /// The IdentityToken a peer announces: a DataHolder of the class
    /// `DDS:Auth:PKI-DH:1.0` (DDS-Security 1.2 §10.3.2.1); its certificate
    /// travels in the handshake.
    #[cfg(feature = "std")]
    fn identity_token_of(_cert_pem: &[u8]) -> Vec<u8> {
        DataHolder::new("DDS:Auth:PKI-DH:1.0").to_cdr_le()
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
        let mut s = two_authenticating_stacks();

        // Both discover each other and call begin_handshake_with. Only
        // the initiator (smaller prefix → A, cyclone convention) sends.
        let in_flight = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        let from_b = handshake_only(
            s.b.begin_handshake_with(s.a_prefix, s.a_guid, &identity_token_of(&s.a_cert))
                .unwrap(),
        );
        assert!(
            from_b.is_empty(),
            "B is the replier (larger prefix), must not initiate"
        );
        assert_eq!(in_flight.len(), 1, "A sends exactly one AUTH_REQUEST");

        // Ping-pong until both secrets are present; the first datagram goes
        // from A to B.
        let (a_secret, b_secret) = s.exchange(in_flight, true);
        let (a_bytes, b_bytes) = s.secret_bytes(
            a_secret.expect("A must derive a secret"),
            b_secret.expect("B must derive a secret"),
        );
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
        let from_small = handshake_only(
            small
                .begin_handshake_with(big_prefix, big_guid, &identity_token_of(&big_cert))
                .unwrap(),
        );
        assert_eq!(
            from_small.len(),
            1,
            "smaller local GUID MUST send AUTH_REQUEST (initiator, cyclone convention)"
        );
        // §7.5.3.3: the message identity names the stateless writer, and
        // its sequence number starts at one.
        let request =
            StatelessMessageReader::new(GuidPrefix::from_bytes([0xEE; 12]), VendorId::ZERODDS)
                .handle_datagram(&from_small[0].bytes)
                .unwrap()
                .remove(0);
        assert_eq!(
            request.message_identity.source_guid,
            Guid::new(
                small_prefix,
                EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
            )
            .to_bytes()
        );
        assert!(request.message_identity.sequence_number >= 1);

        // Large node: local > remote ⇒ MUST wait (replier).
        let mut big = mk(big_cert, big_key, big_prefix, big_guid);
        big.handle_remote_endpoints(&remote_with_prefix(small_prefix, flags));
        let from_big = handshake_only(
            big.begin_handshake_with(small_prefix, small_guid, &identity_token_of(&small_cert))
                .unwrap(),
        );
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
        let mut s = two_authenticating_stacks();
        let b_token = identity_token_of(&s.b_cert);
        let first = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &b_token)
                .unwrap(),
        );
        assert_eq!(first.len(), 1, "first beacon: AUTH_REQUEST sent");

        // Second beacon, handshake still incomplete (no secret) →
        // RESEND instead of idempotent-empty.
        let resend = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &b_token)
                .unwrap(),
        );
        assert_eq!(
            resend.len(),
            1,
            "second beacon: AUTH_REQUEST must be sent again (resend, not idempotent-empty)"
        );
    }

    #[test]
    fn a_duplicate_reply_after_completion_is_answered_with_the_final() {
        // The initiator completes on the reply and sends its final. A replier
        // that waits for the final — Cyclone's does; ZeroDDS's own derives its
        // secret with the reply — re-sends the reply when the final is lost.
        // The initiator must answer that duplicate with the same final. Before,
        // `process_handshake` answered it with "unknown HandshakeHandle" (the
        // plugin drops the state on completion) and nothing was sent, so such
        // a replier waited for ever while the initiator counted the peer as
        // authenticated.
        let TwoStacks {
            mut a,
            mut b,
            a_prefix,
            b_prefix,
            a_guid,
            b_guid,
            a_cert,
            b_cert,
            ..
        } = two_authenticating_stacks();
        assert!(
            handshake_only(
                b.begin_handshake_with(a_prefix, a_guid, &identity_token_of(&a_cert))
                    .unwrap()
            )
            .is_empty()
        );

        // 1. request A -> B, 2. reply B -> A, 3. A completes and sends the final.
        let req = handshake_only(
            a.begin_handshake_with(b_prefix, b_guid, &identity_token_of(&b_cert))
                .unwrap(),
        );
        let m = b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let (reply, _) = b.on_stateless_message(a_prefix, &m[0]).unwrap();
        let m = a.stateless_reader.handle_datagram(&reply[0].bytes).unwrap();
        let (_lost_final, a_done) = a.on_stateless_message(b_prefix, &m[0]).unwrap();
        assert!(a_done.is_some(), "the initiator completes on the reply");
        assert_eq!(_lost_final.len(), 1, "and sends its final");

        // 4. The replier re-sends the same reply; the initiator answers with
        //    the same final.
        let m = a.stateless_reader.handle_datagram(&reply[0].bytes).unwrap();
        let (final_again, again) = a.on_stateless_message(b_prefix, &m[0]).unwrap();
        assert!(again.is_none(), "no second completion");
        assert_eq!(
            final_again.len(),
            1,
            "a duplicate reply is answered with the final"
        );
        let original = a_final_payload(&_lost_final[0].bytes);
        assert_eq!(
            a_final_payload(&final_again[0].bytes),
            original,
            "the cached final, not a new one"
        );
    }

    /// Two PKI-authenticating stacks, A at `a_prefix` and B at
    /// `b_prefix`, each with the other's stateless endpoints, and the
    /// plugins that hold their secrets.
    struct TwoStacks {
        a: SecurityBuiltinStack,
        b: SecurityBuiltinStack,
        a_prefix: GuidPrefix,
        b_prefix: GuidPrefix,
        a_guid: [u8; 16],
        b_guid: [u8; 16],
        a_cert: Vec<u8>,
        b_cert: Vec<u8>,
        a_pki: alloc::sync::Arc<std::sync::Mutex<zerodds_security_pki::PkiAuthenticationPlugin>>,
        b_pki: alloc::sync::Arc<std::sync::Mutex<zerodds_security_pki::PkiAuthenticationPlugin>>,
    }

    impl TwoStacks {
        /// Delivers `in_flight` to the other side — the first datagram to B
        /// when `to_b` — and each answer back, until nothing is in flight or
        /// six hops passed. Returns the secrets A and B completed with.
        fn exchange(
            &mut self,
            mut in_flight: Vec<OutboundDatagram>,
            mut to_b: bool,
        ) -> (Option<SharedSecretHandle>, Option<SharedSecretHandle>) {
            let (mut a_secret, mut b_secret) = (None, None);
            for _ in 0..6 {
                if in_flight.is_empty() {
                    break;
                }
                let datagram = in_flight.remove(0);
                let (target, from) = if to_b {
                    (&mut self.b, self.a_prefix)
                } else {
                    (&mut self.a, self.b_prefix)
                };
                let msgs = target
                    .stateless_reader
                    .handle_datagram(&datagram.bytes)
                    .unwrap();
                assert_eq!(msgs.len(), 1, "one generic message per datagram");
                let (out, completed) = target.on_stateless_message(from, &msgs[0]).unwrap();
                if let Some((_id, secret)) = completed {
                    if to_b {
                        b_secret = Some(secret);
                    } else {
                        a_secret = Some(secret);
                    }
                }
                in_flight = out;
                to_b = !to_b;
            }
            (a_secret, b_secret)
        }

        /// The bytes of the secrets A and B derived.
        fn secret_bytes(&self, a: SharedSecretHandle, b: SharedSecretHandle) -> (Vec<u8>, Vec<u8>) {
            let a_bytes = self.a_pki.lock().unwrap().secret_bytes(a).unwrap().to_vec();
            let b_bytes = self.b_pki.lock().unwrap().secret_bytes(b).unwrap().to_vec();
            (a_bytes, b_bytes)
        }
    }

    fn two_authenticating_stacks() -> TwoStacks {
        use alloc::sync::Arc;
        use std::sync::Mutex;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        let (ca_pem, mut a_leaf, mut b_leaf) = mint_ca_and_two_leafs();
        // DDS-Security 1.2 §10.3.3 Table 59: the participant GUIDs are bound
        // to the identities, and the handshakes carry them in `c.pdata`. A
        // is the one with the smaller GUID, the initiator.
        if bound_guid(&a_leaf.0) > bound_guid(&b_leaf.0) {
            core::mem::swap(&mut a_leaf, &mut b_leaf);
        }
        let ((a_cert, a_key), (b_cert, b_key)) = (a_leaf, b_leaf);
        let a_guid = bound_guid(&a_cert);
        let b_guid = bound_guid(&b_cert);
        // The RTPS prefixes are those of the bound GUIDs, as for any
        // participant: the replier binds an undiscovered peer's c.pdata GUID
        // to the prefix it sent from.
        let prefix_of = |guid: [u8; 16]| {
            let mut p = [0u8; 12];
            p.copy_from_slice(&guid[..12]);
            GuidPrefix::from_bytes(p)
        };
        let (a_prefix, b_prefix) = (prefix_of(a_guid), prefix_of(b_guid));
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
        let mut a =
            SecurityBuiltinStack::with_auth(a_prefix, VendorId::ZERODDS, a_auth, a_local, a_guid);
        let mut b =
            SecurityBuiltinStack::with_auth(b_prefix, VendorId::ZERODDS, b_auth, b_local, b_guid);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        a.set_local_participant_data(pdata_of(a_guid));
        b.set_local_participant_data(pdata_of(b_guid));
        a.handle_remote_endpoints(&remote_with_prefix(b_prefix, flags));
        b.handle_remote_endpoints(&remote_with_prefix(a_prefix, flags));
        TwoStacks {
            a,
            b,
            a_prefix,
            b_prefix,
            a_guid,
            b_guid,
            a_cert,
            b_cert,
            a_pki,
            b_pki,
        }
    }

    /// DDS-Security 1.2 §7.5.3.3, §7.5.3.6.1, §7.5.3.6.2: every message of the
    /// authentication names the participant at its destination
    /// (`destination_participant_guid`), leaves `destination_endpoint_guid` and
    /// `source_endpoint_guid` at GUID_UNKNOWN, and relates to the message it
    /// answers: the AUTH_REQUEST and the handshake request relate to nothing
    /// ({GUID_UNKNOWN, 0}), the reply to the request, the final to the reply.
    /// The AUTH_REQUEST carries one AuthRequestMessageToken.
    #[cfg(feature = "std")]
    #[test]
    fn handshake_messages_carry_the_fields_of_the_spec() {
        use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage};
        let mut ts = two_authenticating_stacks();
        let (a_prefix, b_prefix, a_guid, b_guid) = (ts.a_prefix, ts.b_prefix, ts.a_guid, ts.b_guid);
        let decode = |datagrams: &[OutboundDatagram]| -> Vec<ParticipantGenericMessage> {
            datagrams
                .iter()
                .flat_map(|d| {
                    StatelessMessageReader::new(
                        GuidPrefix::from_bytes([0xEE; 12]),
                        VendorId::ZERODDS,
                    )
                    .handle_datagram(&d.bytes)
                    .unwrap()
                })
                .collect()
        };
        let unknown = [0u8; 16];
        let nil = MessageIdentity::default();
        // B (the replier) may send an AUTH_REQUEST; A sends its AUTH_REQUEST
        // and the handshake request.
        let from_b = decode(
            &ts.b
                .begin_handshake_with(a_prefix, a_guid, &identity_token_of(&ts.a_cert))
                .unwrap(),
        );
        let from_a_datagrams =
            ts.a.begin_handshake_with(b_prefix, b_guid, &identity_token_of(&ts.b_cert))
                .unwrap();
        let from_a = decode(&from_a_datagrams);
        for (messages, destination) in [(&from_b, a_guid), (&from_a, b_guid)] {
            for m in messages {
                assert_eq!(m.destination_participant_key, destination);
                assert_eq!(m.destination_endpoint_key, unknown);
                assert_eq!(m.source_endpoint_key, unknown);
                assert_eq!(m.related_message_identity, nil);
                if m.message_class_id == class_id::AUTH_REQUEST {
                    assert_eq!(m.message_data.len(), 1, "one AuthRequestMessageToken");
                }
            }
        }
        assert!(
            from_a
                .iter()
                .any(|m| m.message_class_id == class_id::AUTH_REQUEST),
            "the AUTH_REQUEST is sent"
        );
        let request = from_a
            .iter()
            .find(|m| m.message_class_id == class_id::AUTH)
            .expect("the handshake request")
            .clone();
        // The request goes to B, the reply back to A.
        let (reply_datagrams, _) = ts.b.on_stateless_message(a_prefix, &request).unwrap();
        let reply = decode(&reply_datagrams).remove(0);
        assert_eq!(reply.message_class_id, class_id::AUTH);
        assert_eq!(reply.destination_participant_key, a_guid);
        assert_eq!(reply.destination_endpoint_key, unknown);
        assert_eq!(reply.source_endpoint_key, unknown);
        assert_eq!(reply.related_message_identity, request.message_identity);
        let (final_datagrams, _) = ts.a.on_stateless_message(b_prefix, &reply).unwrap();
        let last = decode(&final_datagrams).remove(0);
        assert_eq!(last.message_class_id, class_id::AUTH);
        assert_eq!(last.destination_participant_key, b_guid);
        assert_eq!(last.destination_endpoint_key, unknown);
        assert_eq!(last.source_endpoint_key, unknown);
        assert_eq!(last.related_message_identity, reply.message_identity);
    }

    /// DDS-Security 1.2 §7.5.3.4: a message with a `destination_endpoint_guid`
    /// other than GUID_UNKNOWN targets that endpoint of the destination
    /// participant. The stateless reader takes it when it names the
    /// BuiltinParticipantStatelessMessageReader and leaves it alone when it
    /// names another endpoint.
    #[cfg(feature = "std")]
    #[test]
    fn a_stateless_message_for_another_endpoint_is_left_alone() {
        let mut ts = two_authenticating_stacks();
        let (a_prefix, b_prefix, a_guid, b_guid) = (ts.a_prefix, ts.b_prefix, ts.a_guid, ts.b_guid);
        let _ =
            ts.b.begin_handshake_with(a_prefix, a_guid, &identity_token_of(&ts.a_cert))
                .unwrap();
        let datagrams =
            ts.a.begin_handshake_with(b_prefix, b_guid, &identity_token_of(&ts.b_cert))
                .unwrap();
        let request = datagrams
            .iter()
            .flat_map(|d| {
                StatelessMessageReader::new(GuidPrefix::from_bytes([0xEE; 12]), VendorId::ZERODDS)
                    .handle_datagram(&d.bytes)
                    .unwrap()
            })
            .find(|m| m.message_class_id == class_id::AUTH)
            .expect("the handshake request");
        let mut elsewhere = request.clone();
        elsewhere.destination_endpoint_key =
            Guid::new(b_prefix, EntityId::user_reader_with_key([1, 2, 3])).to_bytes();
        let (out, completed) = ts.b.on_stateless_message(a_prefix, &elsewhere).unwrap();
        assert!(out.is_empty() && completed.is_none());
        let mut at_reader = request;
        at_reader.destination_endpoint_key = Guid::new(
            b_prefix,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
        )
        .to_bytes();
        let (out, _) = ts.b.on_stateless_message(a_prefix, &at_reader).unwrap();
        assert_eq!(out.len(), 1, "the reply to a request that names the reader");
    }

    /// DDS-Security 1.2 §7.3.3.1: the name "ECDSA+P256+SHA256" is an octet
    /// sequence of 18 octets, the NUL included. That is the default for every peer;
    /// `omit` (Fast DDS) writes 17, `by_vendor` writes 18 for an OpenDDS peer
    /// and 17 for any other.
    #[cfg(feature = "std")]
    #[test]
    fn the_algorithm_names_follow_the_setting() {
        use zerodds_security::AlgoNameNul;
        let length_of_dsign_algo = |form: Option<AlgoNameNul>, vendor: VendorId| {
            let mut ts = two_authenticating_stacks();
            if let Some(form) = form {
                ts.a.set_algo_name_nul(form);
            }
            ts.a.note_remote_vendor(ts.b_prefix, vendor);
            let datagrams =
                ts.a.begin_handshake_with(ts.b_prefix, ts.b_guid, &identity_token_of(&ts.b_cert))
                    .unwrap();
            let request = datagrams
                .iter()
                .flat_map(|d| {
                    StatelessMessageReader::new(
                        GuidPrefix::from_bytes([0xEE; 12]),
                        VendorId::ZERODDS,
                    )
                    .handle_datagram(&d.bytes)
                    .unwrap()
                })
                .find(|m| m.message_class_id == class_id::AUTH)
                .expect("the handshake request");
            let dsign = request.message_data[0]
                .binary_property("c.dsign_algo")
                .expect("c.dsign_algo")
                .to_vec();
            assert!(dsign.starts_with(b"ECDSA+P256+SHA256"));
            dsign.len()
        };
        assert_eq!(length_of_dsign_algo(None, VendorId::ZERODDS), 18, "default");
        assert_eq!(length_of_dsign_algo(None, VendorId::OPENDDS), 18);
        assert_eq!(
            length_of_dsign_algo(Some(AlgoNameNul::Omit), VendorId::OPENDDS),
            17
        );
        assert_eq!(
            length_of_dsign_algo(Some(AlgoNameNul::ByVendor), VendorId::ZERODDS),
            17
        );
        assert_eq!(
            length_of_dsign_algo(Some(AlgoNameNul::ByVendor), VendorId::OPENDDS),
            18
        );
    }

    /// The setting `mirror`: the replier answers in the form of the request,
    /// names without the NUL (Fast DDS) are answered without it. The default
    /// answers with the NUL whatever the request has.
    #[cfg(feature = "std")]
    #[test]
    fn the_reply_has_the_form_of_the_request() {
        use zerodds_security::AlgoNameNul;
        let reply_dsign_len = |initiator_form: AlgoNameNul, replier_form: AlgoNameNul| {
            let mut ts = two_authenticating_stacks();
            ts.a.set_algo_name_nul(initiator_form);
            ts.b.set_algo_name_nul(replier_form);
            let (a_prefix, b_prefix, a_guid, b_guid) =
                (ts.a_prefix, ts.b_prefix, ts.a_guid, ts.b_guid);
            let _ =
                ts.b.begin_handshake_with(a_prefix, a_guid, &identity_token_of(&ts.a_cert))
                    .unwrap();
            let datagrams =
                ts.a.begin_handshake_with(b_prefix, b_guid, &identity_token_of(&ts.b_cert))
                    .unwrap();
            let decode = |datagrams: &[zerodds_rtps::message_builder::OutboundDatagram]| {
                datagrams
                    .iter()
                    .flat_map(|d| {
                        StatelessMessageReader::new(
                            GuidPrefix::from_bytes([0xEE; 12]),
                            VendorId::ZERODDS,
                        )
                        .handle_datagram(&d.bytes)
                        .unwrap()
                    })
                    .find(|m| m.message_class_id == class_id::AUTH)
                    .expect("a handshake message")
            };
            let request = decode(&datagrams);
            let (out, _) = ts.b.on_stateless_message(a_prefix, &request).unwrap();
            let reply = decode(&out);
            reply.message_data[0]
                .binary_property("c.dsign_algo")
                .expect("c.dsign_algo")
                .len()
        };
        // The default answers with the NUL, also to a request without it.
        assert_eq!(reply_dsign_len(AlgoNameNul::Omit, AlgoNameNul::Spec), 18);
        assert_eq!(reply_dsign_len(AlgoNameNul::Spec, AlgoNameNul::Mirror), 18);
        assert_eq!(reply_dsign_len(AlgoNameNul::Omit, AlgoNameNul::Mirror), 17);
    }

    #[test]
    fn a_duplicate_final_after_completion_is_not_answered() {
        // A final the replier gets after completion asks for nothing. The
        // replier answered it with its cached reply, the initiator answered
        // that with its final, and two ZeroDDS participants traded the two
        // without end.
        let TwoStacks {
            mut a,
            mut b,
            a_prefix,
            b_prefix,
            a_guid,
            b_guid,
            a_cert,
            b_cert,
            ..
        } = two_authenticating_stacks();
        assert!(
            handshake_only(
                b.begin_handshake_with(a_prefix, a_guid, &identity_token_of(&a_cert))
                    .unwrap()
            )
            .is_empty()
        );
        let req = handshake_only(
            a.begin_handshake_with(b_prefix, b_guid, &identity_token_of(&b_cert))
                .unwrap(),
        );
        let m = b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let (reply, _) = b.on_stateless_message(a_prefix, &m[0]).unwrap();
        let m = a.stateless_reader.handle_datagram(&reply[0].bytes).unwrap();
        let (fin, _) = a.on_stateless_message(b_prefix, &m[0]).unwrap();
        // DDS-Security 1.2 §10.3.3 Table 59: the replier has its secret
        // once the final is verified, so it completes with the final; it
        // completed with its reply, before the final proved the initiator.
        let m = b.stateless_reader.handle_datagram(&fin[0].bytes).unwrap();
        let (answer, b_done) = b.on_stateless_message(a_prefix, &m[0]).unwrap();
        assert!(b_done.is_some(), "the replier completes with the final");
        assert!(answer.is_empty(), "a final is not answered");

        let m = b.stateless_reader.handle_datagram(&fin[0].bytes).unwrap();
        let (answer, again) = b.on_stateless_message(a_prefix, &m[0]).unwrap();
        assert!(again.is_none(), "no second completion");
        assert!(
            answer.is_empty(),
            "a duplicate final is not answered either"
        );
    }

    #[test]
    fn a_forged_final_does_not_take_the_handshake_down() {
        // DDS-Security 1.2 §9.3.2.11.6: the plugin keeps its state over a
        // final whose signature fails, and the genuine final completes the
        // handshake. The stack took the peer out of its table for the step
        // and did not put it back on the error, so the genuine final found
        // no peer and was dropped.
        let TwoStacks {
            mut a,
            mut b,
            a_prefix,
            b_prefix,
            a_guid,
            b_guid,
            a_cert,
            b_cert,
            ..
        } = two_authenticating_stacks();
        assert!(
            handshake_only(
                b.begin_handshake_with(a_prefix, a_guid, &identity_token_of(&a_cert))
                    .unwrap()
            )
            .is_empty()
        );
        let req = handshake_only(
            a.begin_handshake_with(b_prefix, b_guid, &identity_token_of(&b_cert))
                .unwrap(),
        );
        let m = b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let (reply, _) = b.on_stateless_message(a_prefix, &m[0]).unwrap();
        let m = a.stateless_reader.handle_datagram(&reply[0].bytes).unwrap();
        let (fin, _) = a.on_stateless_message(b_prefix, &m[0]).unwrap();
        let genuine = b.stateless_reader.handle_datagram(&fin[0].bytes).unwrap();

        let mut forged = genuine[0].clone();
        let token = &mut forged.message_data[0];
        let mut signature = token.binary_property("signature").unwrap().to_vec();
        signature[0] ^= 0xFF;
        token.set_binary_property("signature", signature);
        assert!(b.on_stateless_message(a_prefix, &forged).is_err());
        assert!(b.peer_secret(a_prefix).is_none());

        let (_, b_done) = b.on_stateless_message(a_prefix, &genuine[0]).unwrap();
        assert!(b_done.is_some(), "the genuine final completes the replier");
    }

    /// The serialized payload of the one DATA a stateless datagram carries.
    fn a_final_payload(bytes: &[u8]) -> Vec<u8> {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        decode_datagram(bytes)
            .unwrap()
            .submessages
            .into_iter()
            .find_map(|s| match s.submessage {
                ParsedSubmessage::Data(d) => Some(d.serialized_payload.to_vec()),
                _ => None,
            })
            .expect("a DATA")
    }

    #[test]
    fn lost_reply_recovered_by_resend_yields_matching_secret() {
        // FU2 S3: B's reply is lost → A re-sends AUTH_REQUEST → B MUST
        // re-send the CACHED reply (not regenerate it), otherwise the DH
        // secrets diverge. Proof: after loss+resend both sides derive the
        // SAME secret.
        let mut s = two_authenticating_stacks();

        // Both discover each other; B (replier) creates its peer state for A
        // (sends nothing itself).
        let from_b = handshake_only(
            s.b.begin_handshake_with(s.a_prefix, s.a_guid, &identity_token_of(&s.a_cert))
                .unwrap(),
        );
        assert!(from_b.is_empty(), "B is the replier, does not initiate");

        // 1. A → AUTH_REQUEST. 2. B processes → reply LOST (discarded).
        let req = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        let m = s.b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let (_lost, b_c0) = s.b.on_stateless_message(s.a_prefix, &m[0]).unwrap();

        // 3. A's next beacon → resend of the same AUTH_REQUEST.
        let req2 = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        assert_eq!(req2.len(), 1, "AUTH_REQUEST must be resent");

        // 4. B sees the duplicate → re-sends the CACHED reply. Ping-pong from
        //    here; the reply goes to A first.
        let m2 =
            s.b.stateless_reader
                .handle_datagram(&req2[0].bytes)
                .unwrap();
        let (reply, b_c1) = s.b.on_stateless_message(s.a_prefix, &m2[0]).unwrap();
        assert_eq!(reply.len(), 1, "B resends the cached reply");
        let b_earlier = b_c0.or(b_c1).map(|x| x.1);

        let (a_secret, b_secret) = s.exchange(reply, false);
        let (a_bytes, b_bytes) = s.secret_bytes(
            a_secret.expect("A must derive a secret"),
            b_secret.or(b_earlier).expect("B must derive a secret"),
        );
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
        let mut s = two_authenticating_stacks();

        // A initiates. B DELIBERATELY does NOT call `begin_handshake_with` —
        // so B has NOT discovered A via SPDP (= the cross-process race case).
        let in_flight = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        assert_eq!(in_flight.len(), 1, "A sends AUTH_REQUEST");

        let (a_secret, b_secret) = s.exchange(in_flight, true);
        let (a_bytes, b_bytes) = s.secret_bytes(
            a_secret.expect("A derives a secret"),
            b_secret.expect("B (without prior discovery) derives a secret"),
        );
        assert_eq!(
            a_bytes, b_bytes,
            "secret must match even though B processed the request without prior discovery"
        );
    }

    #[test]
    fn an_undiscovered_request_under_another_prefix_is_refused() {
        // DDS-Security 1.2 §7.4.3, §10.3.3 Table 59: the replier
        // authenticates the peer under the prefix it sent from, and the
        // c.pdata GUID must be that one. Without the SPDP of the initiator
        // the replier compared it with nothing, and a participant with a
        // valid certificate authenticated under another participant's
        // prefix.
        let mut s = two_authenticating_stacks();
        let req = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        let m = s.b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let squatted = GuidPrefix::from_bytes([7; 12]);
        // The squatter sends from the other prefix and names it as source.
        let mut from_squatted = m[0].clone();
        from_squatted.message_identity.source_guid[..12].copy_from_slice(&squatted.to_bytes());
        let err =
            s.b.on_stateless_message(squatted, &from_squatted)
                .unwrap_err();
        assert!(
            err.detail
                .contains("not the one the participant was discovered with"),
            "{err}"
        );
        assert_eq!(
            s.b.handshake_peer_count(),
            0,
            "a refused first request keeps no peer"
        );

        // Under its own prefix the same request is answered.
        let (reply, _) = s.b.on_stateless_message(s.a_prefix, &m[0]).unwrap();
        assert_eq!(reply.len(), 1, "B answers A");
    }

    #[test]
    fn a_handshake_message_is_taken_only_as_the_spec_addresses_it() {
        // DDS-Security 1.2 §7.5.3.4: a message for another participant is
        // not for this one. §9.3.2.11.5/.6: the source is the sender, and a
        // reply or final relates to the last message sent to the peer. All
        // three were taken as they came.
        let mut s = two_authenticating_stacks();
        assert!(
            handshake_only(
                s.b.begin_handshake_with(s.a_prefix, s.a_guid, &identity_token_of(&s.a_cert))
                    .unwrap()
            )
            .is_empty()
        );
        let req = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        let m = s.b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();

        let mut elsewhere = m[0].clone();
        elsewhere.destination_participant_key = [9; 16];
        let (out, done) = s.b.on_stateless_message(s.a_prefix, &elsewhere).unwrap();
        assert!(out.is_empty() && done.is_none(), "not for B");

        let mut foreign = m[0].clone();
        foreign.message_identity.source_guid[0] ^= 0xFF;
        let err = s.b.on_stateless_message(s.a_prefix, &foreign).unwrap_err();
        assert!(err.detail.contains("source_guid"), "{err}");

        let (reply, _) = s.b.on_stateless_message(s.a_prefix, &m[0]).unwrap();
        let r =
            s.a.stateless_reader
                .handle_datagram(&reply[0].bytes)
                .unwrap();
        let mut unrelated = r[0].clone();
        unrelated.related_message_identity.sequence_number += 7;
        let err =
            s.a.on_stateless_message(s.b_prefix, &unrelated)
                .unwrap_err();
        assert!(err.detail.contains("related_message_identity"), "{err}");

        let (fin, a_done) = s.a.on_stateless_message(s.b_prefix, &r[0]).unwrap();
        assert!(
            a_done.is_some(),
            "the related reply completes the initiator"
        );
        let f = s.b.stateless_reader.handle_datagram(&fin[0].bytes).unwrap();
        let (_, b_done) = s.b.on_stateless_message(s.a_prefix, &f[0]).unwrap();
        assert!(b_done.is_some(), "the related final completes the replier");
    }

    #[test]
    fn the_reply_to_an_undiscovered_initiator_is_addressed_to_its_participant() {
        // §9.3.2.11.5: "destination_participant_guid shall be set to match
        // the DDS GUID_t of the destination DomainParticipant". The reply
        // to a request from an undiscovered peer took the request's
        // source_guid, which Fast DDS sets to its stateless writer, and Fast
        // DDS drops a message addressed to that.
        let mut s = two_authenticating_stacks();
        let req = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        let m = s.b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let mut from_writer = m[0].clone();
        from_writer.message_identity.source_guid[12..]
            .copy_from_slice(&EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER.to_bytes());
        let (reply, _) = s.b.on_stateless_message(s.a_prefix, &from_writer).unwrap();
        let r =
            s.a.stateless_reader
                .handle_datagram(&reply[0].bytes)
                .unwrap();
        assert_eq!(r[0].destination_participant_key, s.a_guid);
    }

    #[test]
    fn an_auth_request_goes_out_on_discovery_and_binds_the_challenge() {
        // DDS-Security 1.2 §9.3.2.11.3, §10.3.2.4 Table 55, Table 59: the
        // validate_remote_identity of a discovery without a remote
        // AuthRequestMessageToken makes one, and it is sent; its
        // future_challenge is the challenge its sender uses. A participant
        // that got the peer's AuthRequest before the discovery hands it in
        // and sends none. The stack sent no AuthRequest and dropped the
        // ones it got.
        let mut s = two_authenticating_stacks();
        let from_b =
            s.b.begin_handshake_with(s.a_prefix, s.a_guid, &identity_token_of(&s.a_cert))
                .unwrap();
        assert_eq!(from_b.len(), 1, "B, the replier, sends its AuthRequest");
        assert_eq!(class_of(&from_b[0]), class_id::AUTH_REQUEST);
        let ar =
            s.a.stateless_reader
                .handle_datagram(&from_b[0].bytes)
                .unwrap();
        let (out, done) = s.a.on_stateless_message(s.b_prefix, &ar[0]).unwrap();
        assert!(out.is_empty() && done.is_none());

        let from_a =
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap();
        assert_eq!(
            from_a.iter().map(class_of).collect::<Vec<_>>(),
            vec![alloc::string::String::from(class_id::AUTH)],
            "A got B's AuthRequest first: its own is TokenNIL"
        );
        let (a_secret, b_secret) = s.exchange(from_a, true);
        assert!(
            a_secret.is_some() && b_secret.is_some(),
            "B's challenge2 is the future_challenge it announced"
        );
    }

    #[test]
    fn a_reply_whose_challenge2_is_not_the_announced_one_is_refused() {
        // Table 59, process_handshake on the initiator: with a remote
        // AuthRequestMessageToken its future_challenge must be the reply's
        // challenge2.
        let mut s = two_authenticating_stacks();
        let from_b =
            s.b.begin_handshake_with(s.a_prefix, s.a_guid, &identity_token_of(&s.a_cert))
                .unwrap();
        let mut ar =
            s.a.stateless_reader
                .handle_datagram(&from_b[0].bytes)
                .unwrap();
        ar[0].message_data[0].set_binary_property("future_challenge", vec![0x5A; 32]);
        s.a.on_stateless_message(s.b_prefix, &ar[0]).unwrap();

        let req = handshake_only(
            s.a.begin_handshake_with(s.b_prefix, s.b_guid, &identity_token_of(&s.b_cert))
                .unwrap(),
        );
        let m = s.b.stateless_reader.handle_datagram(&req[0].bytes).unwrap();
        let (reply, _) = s.b.on_stateless_message(s.a_prefix, &m[0]).unwrap();
        let r =
            s.a.stateless_reader
                .handle_datagram(&reply[0].bytes)
                .unwrap();
        let err = s.a.on_stateless_message(s.b_prefix, &r[0]).unwrap_err();
        assert!(err.detail.contains("future_challenge"), "{err}");
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
        // Addressed to B, as a crypto-token message is (§7.5.4.4).
        msg.destination_participant_key =
            Guid::new(remote_prefix(), EntityId::PARTICIPANT).to_bytes();

        let dgs = a.volatile_writer.write(&msg).unwrap();
        assert_eq!(dgs.len(), 1, "ein Datagram pro Reader-Proxy");
        // Wire-decode + dispatch into B's volatile reader
        let parsed = zerodds_rtps::datagram::decode_datagram(&dgs[0].bytes).unwrap();
        let mut received_msgs = Vec::new();
        for sub in parsed.submessages {
            if let zerodds_rtps::datagram::ParsedSubmessage::Data(d) = sub.submessage {
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
        // §7.5.4.3 (Contents of the ParticipantVolatileMessageSecure, DDS-Security 1.2):
        // message_identity.source_guid is the GUID_t of the
        // BuiltinParticipantVolatileMessageSecureWriter and the
        // sequence_number starts at one.
        let mut expected = msg.clone();
        expected.message_identity = MessageIdentity {
            source_guid: Guid::new(
                local_prefix(),
                EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            )
            .to_bytes(),
            sequence_number: 1,
        };
        assert_eq!(received_msgs[0], expected);

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

    // ================================================================
    // The state machine of DDS-Security 1.2 §9.3.2.9 (Figure 9) with a
    // plugin whose answers the test scripts.
    // ================================================================

    use std::collections::VecDeque;
    use zerodds_security::authentication::{
        AlgorithmMasks, AuthenticationExtensions, LocalIdentityValidation,
        ParticipantSecurityConfig, RemoteIdentityValidation,
    };

    /// What the scripted plugin does, and what it was asked.
    #[derive(Default)]
    struct Script {
        /// Operations called, in order, with their relevant arguments.
        calls: Vec<String>,
        validate_remote: VecDeque<SecurityResult<RemoteIdentityValidation>>,
        begin_request: VecDeque<SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>>,
        begin_reply: VecDeque<SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>>,
        process: VecDeque<SecurityResult<HandshakeStepOutcome>>,
        /// The `remote_auth_request_token` of each `validate_remote_identity`.
        remote_auth_requests: Vec<Option<Vec<u8>>>,
        returned_identities: Vec<IdentityHandle>,
        returned_handshakes: Vec<HandshakeHandle>,
        /// The secret `shared_secret` hands out once set.
        secret: Option<SharedSecretHandle>,
        /// `c.id` and `c.perm` of the credential token.
        credential: Option<(Vec<u8>, Vec<u8>)>,
        listener: Option<Arc<dyn AuthenticationListener>>,
        next_identity: u64,
    }

    struct ScriptedAuth(Arc<Mutex<Script>>);

    impl ScriptedAuth {
        fn script(&self) -> std::sync::MutexGuard<'_, Script> {
            self.0.lock().unwrap()
        }
    }

    fn token(class: &str) -> Vec<u8> {
        DataHolder::new(class).to_cdr_le()
    }

    impl AuthenticationExtensions for ScriptedAuth {
        fn plugin_class_id(&self) -> &str {
            "TEST:Auth:Scripted:1.0"
        }
    }

    impl AuthenticationPlugin for ScriptedAuth {
        fn validate_local_identity(
            &mut self,
            _: u32,
            _: &zerodds_security::properties::PropertyList,
            guid: [u8; 16],
        ) -> SecurityResult<LocalIdentityValidation> {
            Ok(LocalIdentityValidation {
                result: ValidationResult::Ok,
                identity: IdentityHandle(1),
                adjusted_participant_guid: guid,
            })
        }
        fn get_identity_token(&mut self, _: IdentityHandle) -> SecurityResult<Vec<u8>> {
            self.script().calls.push("get_identity_token".into());
            Ok(token("TEST:Auth:Scripted:1.0"))
        }
        fn get_identity_status_token(&mut self, _: IdentityHandle) -> SecurityResult<Vec<u8>> {
            self.script().calls.push("get_identity_status_token".into());
            Ok(token("TEST:Auth:Status:1.0"))
        }
        fn set_participant_security_config(
            &mut self,
            _: IdentityHandle,
            config: &ParticipantSecurityConfig,
        ) -> SecurityResult<AlgorithmMasks> {
            Ok(config.algorithm_info)
        }
        fn set_permissions_credential_and_token(
            &mut self,
            _: IdentityHandle,
            _: &[u8],
            _: &[u8],
        ) -> SecurityResult<()> {
            Ok(())
        }
        fn validate_remote_identity(
            &mut self,
            _: IdentityHandle,
            _: [u8; 16],
            _: &[u8],
            remote_auth_request_token: Option<&[u8]>,
        ) -> SecurityResult<RemoteIdentityValidation> {
            let mut script = self.script();
            script.calls.push("validate_remote_identity".into());
            script
                .remote_auth_requests
                .push(remote_auth_request_token.map(<[u8]>::to_vec));
            script.next_identity += 1;
            let identity = IdentityHandle(100 + script.next_identity);
            script
                .validate_remote
                .pop_front()
                .unwrap_or(Ok(RemoteIdentityValidation {
                    result: ValidationResult::PendingHandshakeRequest,
                    identity,
                    local_auth_request_token: None,
                }))
        }
        fn begin_handshake_request(
            &mut self,
            _: IdentityHandle,
            replier: IdentityHandle,
            _: &[u8],
        ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
            let mut script = self.script();
            script.calls.push("begin_handshake_request".into());
            script.begin_request.pop_front().unwrap_or(Ok((
                HandshakeHandle(replier.0),
                HandshakeStepOutcome::SendMessage {
                    token: token("TEST:Auth:Scripted:1.0+Req"),
                },
            )))
        }
        fn begin_handshake_reply(
            &mut self,
            _: IdentityHandle,
            initiator: IdentityHandle,
            _: &[u8],
            _: &[u8],
        ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
            let mut script = self.script();
            script.calls.push("begin_handshake_reply".into());
            script.begin_reply.pop_front().unwrap_or(Ok((
                HandshakeHandle(initiator.0),
                HandshakeStepOutcome::SendMessage {
                    token: token("TEST:Auth:Scripted:1.0+Reply"),
                },
            )))
        }
        fn process_handshake(
            &mut self,
            _: HandshakeHandle,
            _: &[u8],
        ) -> SecurityResult<HandshakeStepOutcome> {
            let mut script = self.script();
            script.calls.push("process_handshake".into());
            script
                .process
                .pop_front()
                .unwrap_or(Ok(HandshakeStepOutcome::WaitingForPeer))
        }
        fn shared_secret(&self, _: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
            self.script().secret.ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::BadArgument, "scripted: not complete")
            })
        }
        fn get_authenticated_peer_credential_token(
            &mut self,
            _: HandshakeHandle,
        ) -> SecurityResult<Vec<u8>> {
            let mut script = self.script();
            script
                .calls
                .push("get_authenticated_peer_credential_token".into());
            let (c_id, c_perm) = script.credential.clone().unwrap_or_default();
            Ok(DataHolder::new("TEST:Credential")
                .with_binary_property("c.id", c_id)
                .with_binary_property("c.perm", c_perm)
                .to_cdr_le())
        }
        fn set_listener(
            &mut self,
            listener: Option<Arc<dyn AuthenticationListener>>,
        ) -> SecurityResult<()> {
            self.script().listener = listener;
            Ok(())
        }
        fn return_identity_token(&mut self, _: &[u8]) -> SecurityResult<()> {
            Ok(())
        }
        fn return_identity_status_token(&mut self, _: &[u8]) -> SecurityResult<()> {
            self.script()
                .calls
                .push("return_identity_status_token".into());
            Ok(())
        }
        fn return_authenticated_peer_credential_token(&mut self, _: &[u8]) -> SecurityResult<()> {
            self.script()
                .calls
                .push("return_authenticated_peer_credential_token".into());
            Ok(())
        }
        fn return_handshake_handle(&mut self, handshake: HandshakeHandle) -> SecurityResult<()> {
            self.script().returned_handshakes.push(handshake);
            Ok(())
        }
        fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
            self.script().returned_identities.push(handle);
            Ok(())
        }
        fn return_sharedsecret_handle(&mut self, _: SharedSecretHandle) -> SecurityResult<()> {
            Ok(())
        }
    }

    /// A stack with the scripted plugin, the initiator of the peer
    /// `remote_prefix()` is the local side when its prefix is the smaller one.
    fn scripted_stack(local: GuidPrefix) -> (SecurityBuiltinStack, Arc<Mutex<Script>>) {
        let script = Arc::new(Mutex::new(Script::default()));
        let auth: Arc<Mutex<dyn AuthenticationPlugin>> =
            Arc::new(Mutex::new(ScriptedAuth(Arc::clone(&script))));
        let guid = Guid::new(local, EntityId::PARTICIPANT).to_bytes();
        let mut stack = SecurityBuiltinStack::with_auth(
            local,
            VendorId::ZERODDS,
            auth,
            IdentityHandle(1),
            guid,
        );
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        for peer in [2u8, 3, 4] {
            stack.handle_remote_endpoints(&remote_with_prefix(
                GuidPrefix::from_bytes([peer; 12]),
                flags,
            ));
        }
        (stack, script)
    }

    fn peer_prefix(n: u8) -> GuidPrefix {
        GuidPrefix::from_bytes([n; 12])
    }

    fn peer_guid(n: u8) -> [u8; 16] {
        Guid::new(peer_prefix(n), EntityId::PARTICIPANT).to_bytes()
    }

    fn calls(script: &Arc<Mutex<Script>>) -> Vec<String> {
        script.lock().unwrap().calls.clone()
    }

    /// The generic messages in `datagrams`.
    fn messages_of(datagrams: &[OutboundDatagram]) -> Vec<ParticipantGenericMessage> {
        let mut reader =
            StatelessMessageReader::new(GuidPrefix::from_bytes([0xEE; 12]), VendorId::ZERODDS);
        datagrams
            .iter()
            .flat_map(|d| reader.handle_datagram(&d.bytes).unwrap())
            .collect()
    }

    /// A message of the handshake from the peer `n`.
    fn handshake_from(
        n: u8,
        local: GuidPrefix,
        class: &str,
        related: MessageIdentity,
    ) -> ParticipantGenericMessage {
        ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: peer_guid(n),
                sequence_number: 1,
            },
            related_message_identity: related,
            destination_participant_key: Guid::new(local, EntityId::PARTICIPANT).to_bytes(),
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0; 16],
            message_class_id: class_id::AUTH.into(),
            message_data: vec![DataHolder::new(class)],
        }
    }

    /// §9.3.2.11.4, §9.3.2.11.5, §9.3.2.11.6: "the sequence_number to the
    /// value in the previous message sent by the
    /// BuiltinParticipantMessageWriter, incremented by one": the numbers
    /// count the messages of the writer, not those to one peer.
    #[test]
    fn the_sequence_number_counts_the_messages_of_the_writer() {
        let (mut stack, _script) = scripted_stack(local_prefix());
        let mut numbers = Vec::new();
        for n in [2u8, 3, 4] {
            let out = stack
                .begin_handshake_with(
                    peer_prefix(n),
                    peer_guid(n),
                    &token("TEST:Auth:Scripted:1.0"),
                )
                .unwrap();
            for m in messages_of(&out) {
                assert_eq!(
                    m.message_identity.source_guid,
                    Guid::new(
                        local_prefix(),
                        EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                    )
                    .to_bytes()
                );
                numbers.push(m.message_identity.sequence_number);
            }
        }
        assert_eq!(numbers, [1, 2, 3], "one writer: 1, 2, 3 for three peers");
    }

    /// §9.3.2.9 Figure 9, RetryWait1: `validate_remote_identity` returned
    /// VALIDATION_PENDING_RETRY: the operation is called again after the retry
    /// timeout, and the handshake follows its result.
    #[test]
    fn a_pending_validation_is_retried_after_the_retry_timeout() {
        let (mut stack, script) = scripted_stack(local_prefix());
        stack.set_retry_timeout(Duration::from_millis(100));
        script
            .lock()
            .unwrap()
            .validate_remote
            .push_back(Ok(RemoteIdentityValidation {
                result: ValidationResult::PendingRetry,
                identity: IdentityHandle::NIL,
                local_auth_request_token: None,
            }));
        let out = stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        assert!(
            out.is_empty(),
            "nothing to send while the validation is pending"
        );
        assert_eq!(calls(&script), ["validate_remote_identity"]);
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::InProgress
        );
        // Before the timeout: not called again.
        assert!(stack.poll(Duration::from_millis(50)).unwrap().is_empty());
        assert_eq!(calls(&script), ["validate_remote_identity"]);
        // After it: validated again, and the request goes out.
        let out = stack.poll(Duration::from_millis(150)).unwrap();
        assert_eq!(
            calls(&script),
            [
                "validate_remote_identity",
                "validate_remote_identity",
                "begin_handshake_request"
            ]
        );
        let sent = messages_of(&out);
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0].message_data[0].class_id,
            "TEST:Auth:Scripted:1.0+Req"
        );
    }

    /// RetryWait2: `begin_handshake_request` returned PENDING_RETRY.
    #[test]
    fn a_pending_request_is_retried_after_the_retry_timeout() {
        let (mut stack, script) = scripted_stack(local_prefix());
        stack.set_retry_timeout(Duration::from_millis(100));
        script
            .lock()
            .unwrap()
            .begin_request
            .push_back(Ok((HandshakeHandle(5), HandshakeStepOutcome::PendingRetry)));
        let out = stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        assert!(messages_of(&out).is_empty());
        let out = stack.poll(Duration::from_millis(150)).unwrap();
        assert_eq!(
            calls(&script),
            [
                "validate_remote_identity",
                "begin_handshake_request",
                "begin_handshake_request"
            ]
        );
        assert_eq!(messages_of(&out).len(), 1);
    }

    /// RetryWait3 and RetryWait4: `begin_handshake_reply` and
    /// `process_handshake` returned PENDING_RETRY; the message that caused
    /// the call is the one they are called with again, and a handshake a
    /// retried step completes is reported to the DDS implementation.
    #[test]
    fn a_pending_reply_and_a_pending_process_are_retried_with_their_message() {
        // The replier: the peer has the smaller prefix.
        let local = peer_prefix(9);
        let (mut stack, script) = scripted_stack(local);
        stack.set_retry_timeout(Duration::from_millis(100));
        script
            .lock()
            .unwrap()
            .begin_reply
            .push_back(Ok((HandshakeHandle(5), HandshakeStepOutcome::PendingRetry)));
        let request = handshake_from(
            2,
            local,
            "TEST:Auth:Scripted:1.0+Req",
            MessageIdentity::default(),
        );
        let (out, completed) = stack
            .on_stateless_message(peer_prefix(2), &request)
            .unwrap();
        assert!(out.is_empty() && completed.is_none());
        let out = stack.poll(Duration::from_millis(150)).unwrap();
        let reply = messages_of(&out);
        assert_eq!(reply.len(), 1, "the reply goes out after the retry");
        assert_eq!(
            reply[0].related_message_identity, request.message_identity,
            "the retried reply still answers the request"
        );
        // The final arrives, process_handshake is pending, then completes.
        {
            let mut s = script.lock().unwrap();
            s.process.push_back(Ok(HandshakeStepOutcome::PendingRetry));
            s.process.push_back(Ok(HandshakeStepOutcome::Complete {
                secret: SharedSecretHandle(7),
            }));
        }
        let last = handshake_from(
            2,
            local,
            "TEST:Auth:Scripted:1.0+Final",
            reply[0].message_identity.clone(),
        );
        let (_, completed) = stack.on_stateless_message(peer_prefix(2), &last).unwrap();
        assert!(completed.is_none(), "pending");
        assert!(stack.take_completed().is_empty());
        script.lock().unwrap().secret = Some(SharedSecretHandle(7));
        let _ = stack.poll(Duration::from_millis(300)).unwrap();
        assert_eq!(
            stack.take_completed(),
            [(peer_prefix(2), IdentityHandle(1), SharedSecretHandle(7))]
        );
        assert_eq!(
            stack.peer_secret(peer_prefix(2)),
            Some(SharedSecretHandle(7))
        );
        assert!(
            calls(&script)
                .iter()
                .filter(|c| *c == "process_handshake")
                .count()
                == 2
        );
    }

    /// Figure 9, AuthenticationTimeoutWait: a handshake that has not completed
    /// within the authentication timeout ends: the participant is
    /// unauthenticated, the handles go back to the plugin, the failure is
    /// reported; the next announcement starts the authentication again.
    #[test]
    fn a_handshake_that_does_not_complete_ends_with_the_authentication_timeout() {
        let (mut stack, script) = scripted_stack(local_prefix());
        stack.set_authentication_timeout(Duration::from_secs(10));
        stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        let _ = stack.poll(Duration::from_secs(1)).unwrap();
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::InProgress
        );
        let _ = stack.poll(Duration::from_secs(11)).unwrap();
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::Unauthenticated(UnauthenticatedReason::Timeout)
        );
        assert_eq!(stack.handshake_peer_count(), 0);
        let failures = stack.take_handshake_failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].0, peer_prefix(2));
        let returned = script.lock().unwrap().returned_identities.clone();
        assert_eq!(returned, [IdentityHandle(101)], "the identity went back");
        assert_eq!(
            script.lock().unwrap().returned_handshakes,
            [HandshakeHandle(101)]
        );
        // The next beacon starts it again.
        let out = stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        assert_eq!(messages_of(&out).len(), 1);
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::InProgress
        );
        // A completed handshake does not time out.
        let request = messages_of(&out).remove(0);
        {
            let mut s = script.lock().unwrap();
            s.process.push_back(Ok(HandshakeStepOutcome::Complete {
                secret: SharedSecretHandle(7),
            }));
            s.secret = Some(SharedSecretHandle(7));
        }
        let reply = handshake_from(
            2,
            local_prefix(),
            "TEST:Auth:Scripted:1.0+Reply",
            request.message_identity,
        );
        let (_, completed) = stack.on_stateless_message(peer_prefix(2), &reply).unwrap();
        assert_eq!(
            completed,
            Some((IdentityHandle(102), SharedSecretHandle(7)))
        );
        let _ = stack.poll(Duration::from_secs(1000)).unwrap();
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::Authenticated
        );
        assert!(stack.take_handshake_failures().is_empty());
    }

    /// §9.3.2.9.1 rule 3: "If the plugin operation returns VALIDATION_NOT_OK,
    /// the implementation transitions back to the previous state that caused
    /// the message to be sent and resends the same message." A forged reply
    /// leaves the handshake as it was, and the request goes out again.
    #[test]
    fn a_refused_reply_makes_the_stack_send_the_request_again() {
        let (mut stack, script) = scripted_stack(local_prefix());
        let first = stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        let request = messages_of(&first).remove(0);
        script
            .lock()
            .unwrap()
            .process
            .push_back(Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "forged",
            )));
        let forged = handshake_from(
            2,
            local_prefix(),
            "TEST:Auth:Scripted:1.0+Reply",
            request.message_identity.clone(),
        );
        let err = stack
            .on_stateless_message(peer_prefix(2), &forged)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
        assert_eq!(
            stack.handshake_peer_count(),
            1,
            "the handshake is still there"
        );
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::InProgress
        );
        // The same message goes out at the next poll, not at the next resend
        // period: the clock has not moved.
        let again = messages_of(&stack.poll(Duration::ZERO).unwrap());
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].message_data, request.message_data);
        assert_eq!(again[0].message_identity, request.message_identity);
        // And the genuine reply is still taken.
        script
            .lock()
            .unwrap()
            .process
            .push_back(Ok(HandshakeStepOutcome::WaitingForPeer));
        let genuine = handshake_from(
            2,
            local_prefix(),
            "TEST:Auth:Scripted:1.0+Reply",
            request.message_identity,
        );
        assert!(stack.on_stateless_message(peer_prefix(2), &genuine).is_ok());
    }

    /// §9.3.2.9.1 rule 1: "The DDS security implementation shall pass to the
    /// AuthenticationPlugin any message received by the
    /// BuiltinParticipantStatelessMessageReader that has a GenericMessageClassId
    /// set to GMCLASSID_SECURITY_AUTH_REQUEST or
    /// GMCLASSID_SECURITY_AUTH_HANDSHAKE." An AuthRequest of a participant
    /// whose handshake did not begin replaces the identity made at its
    /// discovery with one made with the token; one that arrives later is passed
    /// to the plugin as well, and the identity this makes goes back.
    #[test]
    fn an_auth_request_is_passed_to_the_plugin_whenever_it_arrives() {
        let local = peer_prefix(9);
        let (mut stack, script) = scripted_stack(local);
        // The replier: validation says to wait for the request.
        script
            .lock()
            .unwrap()
            .validate_remote
            .push_back(Ok(RemoteIdentityValidation {
                result: ValidationResult::PendingHandshakeMessage,
                identity: IdentityHandle(500),
                local_auth_request_token: None,
            }));
        stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        let auth_request = ParticipantGenericMessage {
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: vec![
                DataHolder::new("TEST:Auth:Scripted:1.0+AuthReq")
                    .with_binary_property("future_challenge", vec![9; 32]),
            ],
            ..handshake_from(2, local, "x", MessageIdentity::default())
        };
        stack
            .on_stateless_message(peer_prefix(2), &auth_request)
            .unwrap();
        {
            let s = script.lock().unwrap();
            assert_eq!(s.remote_auth_requests.len(), 2);
            assert_eq!(s.remote_auth_requests[0], None);
            let given =
                DataHolder::from_cdr_le(s.remote_auth_requests[1].as_ref().unwrap()).unwrap();
            assert_eq!(
                given.binary_property("future_challenge"),
                Some(&[9u8; 32][..])
            );
            assert_eq!(
                s.returned_identities,
                [IdentityHandle(500)],
                "the first identity went back"
            );
        }
        // The request arrives and the handshake begins; a second AuthRequest
        // is still passed to the plugin.
        let request = handshake_from(
            2,
            local,
            "TEST:Auth:Scripted:1.0+Req",
            MessageIdentity::default(),
        );
        stack
            .on_stateless_message(peer_prefix(2), &request)
            .unwrap();
        stack
            .on_stateless_message(peer_prefix(2), &auth_request)
            .unwrap();
        let s = script.lock().unwrap();
        assert_eq!(s.remote_auth_requests.len(), 3, "passed again");
        assert_eq!(
            s.returned_identities.len(),
            2,
            "the identity of a late AuthRequest goes back at once"
        );
    }

    /// §9.3.2.10: whatever the reason is, the participant is unauthenticated,
    /// and "All these cases shall be treated the same": a participant without
    /// an IdentityToken, one whose identity is refused, one whose handshake
    /// fails, one whose configuration is incompatible and one that times out
    /// are all not authenticated, have no secret, and are told apart for the
    /// log only.
    #[test]
    fn every_reason_for_being_unauthenticated_is_one_case() {
        let (mut stack, script) = scripted_stack(local_prefix());
        stack.set_authentication_timeout(Duration::from_secs(5));
        // 2: no IdentityToken.
        stack.mark_unauthenticated(peer_prefix(2), UnauthenticatedReason::NoIdentityToken);
        // 3: the identity is refused.
        script
            .lock()
            .unwrap()
            .validate_remote
            .push_back(Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "another plugin",
            )));
        assert!(
            stack
                .begin_handshake_with(peer_prefix(3), peer_guid(3), &token("OTHER:Auth:1.0"))
                .is_err()
        );
        // 4: the handshake fails at the first step.
        script
            .lock()
            .unwrap()
            .begin_request
            .push_back(Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "no key",
            )));
        assert!(
            stack
                .begin_handshake_with(
                    peer_prefix(4),
                    peer_guid(4),
                    &token("TEST:Auth:Scripted:1.0")
                )
                .is_err()
        );
        // 5: an incompatible configuration; 6: a timeout.
        stack.mark_unauthenticated(
            peer_prefix(5),
            UnauthenticatedReason::Incompatible("masks".into()),
        );
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        stack.handle_remote_endpoints(&remote_with_prefix(peer_prefix(6), flags));
        stack
            .begin_handshake_with(
                peer_prefix(6),
                peer_guid(6),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        let _ = stack.poll(Duration::from_secs(60)).unwrap();
        let reasons: Vec<_> = (2u8..=6)
            .map(|n| match stack.authentication_status(peer_prefix(n)) {
                PeerAuthentication::Unauthenticated(reason) => reason,
                other => panic!("peer {n}: {other:?}"),
            })
            .collect();
        assert!(matches!(reasons[0], UnauthenticatedReason::NoIdentityToken));
        assert!(matches!(
            reasons[1],
            UnauthenticatedReason::IdentityRejected(_)
        ));
        assert!(matches!(
            reasons[2],
            UnauthenticatedReason::HandshakeFailed(_)
        ));
        assert!(matches!(reasons[3], UnauthenticatedReason::Incompatible(_)));
        assert!(matches!(reasons[4], UnauthenticatedReason::Timeout));
        for n in 2u8..=6 {
            assert!(!stack.is_authenticated(peer_prefix(n)), "peer {n}");
            assert!(stack.peer_secret(peer_prefix(n)).is_none(), "peer {n}");
        }
    }

    /// §9.3.2.12.2: when the plugin revokes an identity, the stack ends the
    /// authentication of the participant, reports it, does not authenticate it
    /// again while it is known, and takes no message from it.
    #[test]
    fn a_revoked_identity_ends_the_authentication_of_its_participant() {
        let (mut stack, script) = scripted_stack(local_prefix());
        stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        let listener = script
            .lock()
            .unwrap()
            .listener
            .clone()
            .expect("the stack installed its listener");
        // The remote identity of the peer is IdentityHandle(101).
        listener.on_revoke_identity(IdentityHandle(101)).unwrap();
        assert!(
            stack.take_revoked_peers().is_empty(),
            "reported at the tick"
        );
        let _ = stack.poll(Duration::ZERO).unwrap();
        assert_eq!(stack.take_revoked_peers(), [peer_prefix(2)]);
        assert!(stack.take_revoked_peers().is_empty(), "once");
        assert_eq!(
            stack.authentication_status(peer_prefix(2)),
            PeerAuthentication::Unauthenticated(UnauthenticatedReason::Revoked)
        );
        assert_eq!(stack.handshake_peer_count(), 0);
        // Not authenticated again, no message taken.
        let calls_before = calls(&script).len();
        assert!(
            stack
                .begin_handshake_with(
                    peer_prefix(2),
                    peer_guid(2),
                    &token("TEST:Auth:Scripted:1.0")
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            calls(&script).len(),
            calls_before,
            "the plugin was not asked"
        );
        let message = handshake_from(
            2,
            local_prefix(),
            "TEST:Auth:Scripted:1.0+Req",
            MessageIdentity::default(),
        );
        assert!(
            stack
                .on_stateless_message(peer_prefix(2), &message)
                .is_err()
        );
        // When the participant is gone, a new one can authenticate.
        stack.on_participant_lost(peer_prefix(2));
        stack.handle_remote_endpoints(&remote_with_prefix(
            peer_prefix(2),
            endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
                | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER,
        ));
        assert!(
            !stack
                .begin_handshake_with(
                    peer_prefix(2),
                    peer_guid(2),
                    &token("TEST:Auth:Scripted:1.0")
                )
                .unwrap()
                .is_empty()
        );
    }

    /// §9.3.2.12.1, §9.3.2.12.3: a change of the identity status is learned
    /// from `on_status_changed`; the stack takes the new token.
    #[test]
    fn a_status_change_is_passed_on_with_the_new_token() {
        let (mut stack, script) = scripted_stack(local_prefix());
        let listener = script.lock().unwrap().listener.clone().unwrap();
        assert!(!stack.take_identity_status_changed());
        listener.on_status_changed(IdentityHandle(1), AuthStatusKind::IdentityStatus);
        let _ = stack.poll(Duration::ZERO).unwrap();
        assert!(stack.take_identity_status_changed());
        assert!(!stack.take_identity_status_changed(), "once");
        let token = stack.identity_status_token().unwrap();
        assert_eq!(
            DataHolder::from_cdr_le(&token).unwrap().class_id,
            "TEST:Auth:Status:1.0"
        );
        assert!(
            calls(&script).contains(&"return_identity_status_token".to_string()),
            "the token goes back to the plugin"
        );
    }

    /// §9.3.2.11.8, Table 54: when the handshake completes the stack asks for
    /// the AuthenticatedPeerCredentialToken, takes `c.perm` and `c.id` from it
    /// (the certificate as the PEM that travelled, as DER for the access
    /// control) and gives the token back.
    #[test]
    fn the_peer_credential_token_gives_the_permissions_and_the_certificate() {
        let local = peer_prefix(9);
        let (mut stack, script) = scripted_stack(local);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = rcgen::CertificateParams::new(vec!["peer".to_string()])
            .unwrap()
            .self_signed(&key)
            .unwrap();
        {
            let mut s = script.lock().unwrap();
            s.validate_remote.push_back(Ok(RemoteIdentityValidation {
                result: ValidationResult::PendingHandshakeMessage,
                identity: IdentityHandle(500),
                local_auth_request_token: None,
            }));
            s.begin_reply.push_back(Ok((
                HandshakeHandle(5),
                HandshakeStepOutcome::SendMessage {
                    token: token("TEST:Auth:Scripted:1.0+Reply"),
                },
            )));
            s.process.push_back(Ok(HandshakeStepOutcome::Complete {
                secret: SharedSecretHandle(7),
            }));
            s.credential = Some((cert.pem().into_bytes(), b"PERMISSIONS".to_vec()));
        }
        stack
            .begin_handshake_with(
                peer_prefix(2),
                peer_guid(2),
                &token("TEST:Auth:Scripted:1.0"),
            )
            .unwrap();
        let request = handshake_from(
            2,
            local,
            "TEST:Auth:Scripted:1.0+Req",
            MessageIdentity::default(),
        );
        let (out, _) = stack
            .on_stateless_message(peer_prefix(2), &request)
            .unwrap();
        let reply = messages_of(&out).remove(0);
        // Not complete: no credential yet.
        assert!(stack.peer_permissions(peer_prefix(2)).is_none());
        script.lock().unwrap().secret = Some(SharedSecretHandle(7));
        let last = handshake_from(
            2,
            local,
            "TEST:Auth:Scripted:1.0+Final",
            reply.message_identity,
        );
        let (_, completed) = stack.on_stateless_message(peer_prefix(2), &last).unwrap();
        assert_eq!(
            completed,
            Some((IdentityHandle(500), SharedSecretHandle(7)))
        );
        assert_eq!(
            stack.peer_permissions(peer_prefix(2)),
            Some(b"PERMISSIONS".to_vec())
        );
        assert_eq!(
            stack.peer_identity_certificate(peer_prefix(2)),
            Some(cert.der().to_vec())
        );
        let calls = calls(&script);
        assert!(calls.contains(&"get_authenticated_peer_credential_token".to_string()));
        assert!(calls.contains(&"return_authenticated_peer_credential_token".to_string()));
    }
    /// DDS-Security 1.2 §9.8.3 Figure 23: both sides call
    /// `validate_remote_identity`, the initiator `begin_handshake_request`,
    /// the replier `begin_handshake_reply`, both `process_handshake`, and
    /// after it `get_shared_secret` and
    /// `get_authenticated_peer_credential_token` (steps 15 to 18), which they
    /// give back with `return_authenticated_peer_credential_token`.
    #[cfg(feature = "std")]
    #[test]
    fn figure_23_calls_the_operations_of_both_plugins_in_order() {
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security::mock::MockAuthenticationPlugin;
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        // The mock starts the handshake at the smaller GUID (Table 59).
        let (a_prefix, b_prefix) = (peer_prefix(0x01), peer_prefix(0x90));
        let (a_guid, b_guid) = (peer_guid(0x01), peer_guid(0x90));
        let make = |prefix: GuidPrefix, guid: [u8; 16]| {
            let mock = MockAuthenticationPlugin::new();
            let log = mock.calls();
            let auth: Arc<Mutex<dyn AuthenticationPlugin>> = Arc::new(Mutex::new(mock));
            let local = auth
                .lock()
                .unwrap()
                .validate_local_identity(
                    0,
                    &zerodds_security::properties::PropertyList::new(),
                    guid,
                )
                .unwrap()
                .identity;
            let mut stack =
                SecurityBuiltinStack::with_auth(prefix, VendorId::ZERODDS, auth, local, guid);
            stack.set_local_participant_data(pdata_of(guid));
            (stack, log)
        };
        let (mut a, a_log) = make(a_prefix, a_guid);
        let (mut b, b_log) = make(b_prefix, b_guid);
        a.handle_remote_endpoints(&remote_with_prefix(b_prefix, flags));
        b.handle_remote_endpoints(&remote_with_prefix(a_prefix, flags));

        let mut in_flight = handshake_only(
            a.begin_handshake_with(b_prefix, b_guid, &token("DDS:Auth:Mock"))
                .unwrap(),
        );
        assert!(
            handshake_only(
                b.begin_handshake_with(a_prefix, a_guid, &token("DDS:Auth:Mock"))
                    .unwrap()
            )
            .is_empty(),
            "the replier does not initiate"
        );
        let mut to_b = true;
        let (mut a_done, mut b_done) = (false, false);
        for _ in 0..6 {
            if in_flight.is_empty() {
                break;
            }
            let datagram = in_flight.remove(0);
            let (target, from) = if to_b {
                (&mut b, a_prefix)
            } else {
                (&mut a, b_prefix)
            };
            let messages = target
                .stateless_reader
                .handle_datagram(&datagram.bytes)
                .unwrap();
            let (out, completed) = target
                .on_stateless_message(from, &messages[0])
                .unwrap_or_else(|e| {
                    panic!(
                        "hop to_b={to_b} class={} data={}: {e:?}",
                        messages[0].message_class_id,
                        messages[0].message_data.len()
                    )
                });
            if completed.is_some() {
                if to_b {
                    b_done = true;
                } else {
                    a_done = true;
                }
            }
            in_flight = out;
            to_b = !to_b;
        }
        assert!(a_done && b_done, "both sides completed");

        let order = |log: &Arc<Mutex<Vec<String>>>, begin: &str| {
            let calls = log.lock().unwrap().clone();
            let first = |name: &str| {
                calls
                    .iter()
                    .position(|c| c == name)
                    .unwrap_or_else(|| panic!("{name} was not called: {calls:?}"))
            };
            let chain = [
                first("validate_remote_identity"),
                first(begin),
                first("process_handshake"),
                first("get_authenticated_peer_credential_token"),
                first("return_authenticated_peer_credential_token"),
            ];
            assert!(chain.windows(2).all(|w| w[0] < w[1]), "{calls:?}");
            // `shared_secret` is asked after each step; the step that gives
            // it comes before the credential token.
            let secret = calls.iter().rposition(|c| c == "shared_secret").unwrap();
            assert!(
                secret < first("get_authenticated_peer_credential_token"),
                "{calls:?}"
            );
            let only_once = |name: &str| calls.iter().filter(|c| *c == name).count() == 1;
            assert!(
                only_once("get_authenticated_peer_credential_token"),
                "{calls:?}"
            );
            assert!(!calls.iter().any(|c| c
                == if begin == "begin_handshake_request" {
                    "begin_handshake_reply"
                } else {
                    "begin_handshake_request"
                }));
        };
        order(&a_log, "begin_handshake_request");
        order(&b_log, "begin_handshake_reply");
    }
}
