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
use alloc::string::String;
use alloc::vec::Vec;
use std::sync::{Arc, Mutex, PoisonError};

use zerodds_security::access_control::{
    AccessControlListener, AccessControlPlugin, AccessDecision, EndpointRequest,
    EndpointSecurityConfig, ParticipantRequest, ParticipantSecurityConfig, PermissionsHandle,
    QosEntries, RelayRole, RemoteReaderDecision, TopicRequest, TopicSecurityConfig,
};
use zerodds_security::authentication::{AlgorithmMasks, IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::CryptoHandle;
use zerodds_security::cryptographic::CryptographicPluginInterfaces;
use zerodds_security::properties::PropertyList;
use zerodds_security::token::DataHolder;

use zerodds_security_permissions::{
    Governance, Permissions, PermissionsAccessControl, ProtectionKind, minor_code,
};
use zerodds_security_rtps::{RTPS_HEADER_LEN, SRTPS_PREFIX};

use crate::gate::SecurityGateError;
use crate::gate_crypto::{CryptoLayer, LocalParams};
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
/// The verdict carries no interface; the interface is the `NetInterface`
/// the caller passed to [`SharedSecurityGate::classify_inbound`].
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

/// Outcome of the DDS-Security 1.2 §9.4.2.9.4 / §9.4.2.9.5 endpoint-create
/// check.
///
/// Denial has two distinct causes and they are kept apart, because they need
/// different operator action: a topic the permissions do not grant is a
/// permissions problem, a gate with no permissions at all is a wiring
/// problem. Neither is ever silently a permit — a governance with
/// `<enable_write_access_control>TRUE</enable_write_access_control>` that
/// creates the writer anyway is the failure class
/// `docs/silent-drop-sweep.md` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointAccess {
    /// The topic is not access-controlled by the governance, or a grant
    /// permits the operation.
    Permit,
    /// The governance access-controls the topic and no grant permits it.
    Deny,
    /// The governance access-controls the topic and the gate carries no
    /// permissions document to consult
    /// ([`SharedSecurityGate::set_permissions`] was never called).
    NoPermissions,
    /// A remote reader whose grant allows only relaying the topic
    /// (DDS-Security 1.2 §9.4.2.9.11, `allow_relay_only`).
    RelayOnly,
}

/// What [`SharedSecurityGate::set_remote_permissions`] made of a peer's
/// permissions document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePermissions {
    /// Signed by the permissions CA, with a grant for the peer's
    /// SubjectName: stored, and the peer's endpoints are checked against it.
    Granted,
    /// No permissions CA is armed: the document cannot be authenticated.
    NoVerifier,
    /// The signature does not verify against the permissions CA.
    NotVerified,
    /// The peer announced no permissions_token, or one of another plugin
    /// class or major version (DDS-Security 1.2 Table 70).
    IncompatibleToken,
    /// No grant of the document names the peer's SubjectName (DDS-Security
    /// 1.2 §10.4.1.5.3.1).
    NoGrantForSubject,
}

impl EndpointAccess {
    /// `true` only for [`Self::Permit`].
    #[must_use]
    pub fn is_permitted(self) -> bool {
        matches!(self, Self::Permit)
    }
}

/// The documents [`SharedSecurityGate::set_builtin_access_control`] builds the
/// builtin AccessControl plugin from (DDS-Security 1.2 §10.4.3, Table 70
/// `validate_local_permissions`).
#[derive(Debug, Clone)]
pub struct SignedDocuments<'a> {
    /// The permissions, reduced to the grant of the participant's subject
    /// (read by the profile; the plugin reads the signed document itself when
    /// all documents are text).
    pub permissions: Permissions,
    /// The signed permissions document as it was read.
    pub signed_permissions: &'a [u8],
    /// The signed governance document as it was read.
    pub signed_governance: &'a [u8],
    /// The identity certificate of the participant, PEM.
    pub identity_certificate_pem: &'a [u8],
    /// The Permissions CA, PEM.
    pub permissions_ca_pem: &'a [u8],
}

/// Thread-safe security gate. Clone gives a second reference to
/// the same plugin instance — all clones operate on the same
/// key store.
#[derive(Clone)]
pub struct SharedSecurityGate {
    inner: Arc<Mutex<GateInner>>,
    /// The pre-shared key that protects RTPS messages (`crate::psk_gate`).
    pub(crate) psk: Arc<Mutex<crate::psk_gate::PskState>>,
}

impl core::fmt::Debug for SharedSecurityGate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Plugin + keys never in debug output — only metadata.
        match self.inner.lock() {
            Ok(g) => write!(
                f,
                "SharedSecurityGate {{ domain_id: {}, peers: {}, local_registered: {} }}",
                g.domain_id,
                g.crypto.peer_keys().len(),
                g.local.is_some()
            ),
            Err(_) => write!(f, "SharedSecurityGate {{ <poisoned> }}"),
        }
    }
}

struct GateInner {
    domain_id: u32,
    /// The DomainTag of the participant (DDS-Security 1.2 §10.4.1.2.5.1);
    /// `""` for none.
    domain_tag: String,
    /// Plaintext RTPS messages over loopback are taken on an RTPS-protected
    /// domain (`org.zerodds.sec.plaintext_loopback`); off by default.
    plaintext_loopback: bool,
    /// The key width of the crypto plugin in bits (128 or 256), announced in
    /// the symmetric cipher info of the participant (§7.3.13).
    symmetric_cipher_bits: u16,
    /// Participants and endpoints with an incompatible configuration are not
    /// authenticated or matched (DDS-Security 1.2 §7.3.10 to §7.3.24); `false`
    /// is the named setting `org.zerodds.sec.compatibility_check=ignore`.
    compatibility_checks: bool,
    /// A user or discovery submessage or an encrypted payload that no key of
    /// its sender opens is tried with the key derived from the SharedSecret
    /// (the Kx key) as well: what some vendors protect with
    /// (`org.zerodds.sec.kx_key_fallback=accept`). DDS-Security 1.2 §10.5.2.1.2
    /// gives that key to the volatile channel only.
    kx_key_fallback: bool,
    /// Under origin authentication, an RTPS message or submessage under a key
    /// without a receiver-specific key is verified with the common MAC alone
    /// (`org.zerodds.sec.origin_authentication_fallback=accept`); the crypto
    /// plugin gets it as `org.zerodds.crypto.origin_authentication_fallback`
    /// when the local participant is registered.
    origin_authentication_fallback: bool,
    /// How the algorithm names of a handshake token are written (DDS-Security
    /// 1.2 §7.3.3.1); `org.zerodds.sec.algo_name_nul`.
    algo_name_nul: zerodds_security::AlgoNameNul,
    governance: Governance,
    /// The AccessControl plugin every access decision goes through
    /// (DDS-Security 1.2 §9.4.2.9). `None` until
    /// [`SharedSecurityGate::set_access_control`] or
    /// [`SharedSecurityGate::set_permissions`] arms the gate — see
    /// [`EndpointAccess::NoPermissions`].
    access: Option<AccessState>,
    /// What the builtin plugin is built from; kept so that a change of the
    /// governance, the DomainTag or the permissions CA rebuilds it. `None`
    /// until a permissions document is given, and unused with a plugin of
    /// the application.
    builtin: BuiltinInputs,
    /// The `IdentityHandle` of the local participant that the AccessControl
    /// operations are called with.
    local_identity: IdentityHandle,
    /// The GUID of the local participant.
    local_guid: [u8; 16],
    /// The participant announces `participant_security_optional_attributes`
    /// (DDS-Security 1.2 §7.3.23, Table 35); `false` is the named setting
    /// `org.zerodds.sec.optional_attributes=omit`.
    send_optional_attributes: bool,
    crypto: CryptoLayer,
    local: Option<CryptoHandle>,
}

/// The documents the gate builds the builtin AccessControl plugin from.
#[derive(Default)]
struct BuiltinInputs {
    permissions: Option<Permissions>,
    signed_permissions: Option<Vec<u8>>,
    signed_governance: Option<Vec<u8>>,
    identity_certificate_pem: Option<Vec<u8>>,
    permissions_ca_pem: Option<Vec<u8>>,
    identity_algorithms: Option<AlgorithmMasks>,
    /// The `class_id` of the PermissionsToken the builtin plugin sends
    /// (`org.zerodds.sec.access.permissions_token_class_id`); `None` for the
    /// default of Table 65.
    token_class_id: Option<String>,
    /// The form of `dds.perm_ca.sn` in the PermissionsToken
    /// (`org.zerodds.sec.subject_name_form`).
    subject_name_form: zerodds_security::SubjectNameForm,
    /// The names of `dds.perm_ca.algo` in the PermissionsToken
    /// (`org.zerodds.sec.algorithm_names`).
    algorithm_names: zerodds_security_pki::AlgorithmNames,
    /// A plugin of the application replaced the builtin one.
    replaced: bool,
}

/// The AccessControl plugin of the gate with the handles it gave.
struct AccessState {
    plugin: Box<dyn AccessControlPlugin>,
    /// The `PermissionsHandle` of the local participant.
    local: PermissionsHandle,
    /// The `PermissionsHandle` of each remote participant that was
    /// validated (`validate_remote_permissions`, §9.4.2.9.2). A peer without
    /// one is checked with [`PermissionsHandle::NIL`].
    remote: BTreeMap<PeerKey, PermissionsHandle>,
    /// `get_participant_security_config` of the local participant, once
    /// asked for.
    participant_config: Option<ParticipantSecurityConfig>,
    /// The handles the plugin revoked (§9.4.2.10).
    revoked: Arc<RevocationQueue>,
}

impl Drop for AccessState {
    fn drop(&mut self) {
        if let Some(config) = self.participant_config.take() {
            // The gate is gone; whether the plugin knew the config is its
            // own matter.
            let _ = self.plugin.return_participant_security_config(config);
        }
    }
}

/// The [`AccessControlListener`] of the gate: it queues the handles the
/// plugin revokes (§9.4.2.10) for [`SharedSecurityGate::take_revoked`].
#[derive(Default)]
struct RevocationQueue {
    handles: Mutex<Vec<PermissionsHandle>>,
}

impl AccessControlListener for RevocationQueue {
    fn on_revoke_permissions(&self, _plugin: &str, handle: PermissionsHandle) -> bool {
        match self.handles.lock() {
            Ok(mut queue) => {
                queue.push(handle);
                true
            }
            Err(_) => false,
        }
    }
}

/// The permissions the gate lost, see [`SharedSecurityGate::take_revoked`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RevokedPermissions {
    /// The remote participants whose permissions were revoked: the gate
    /// forgot their handle, they have none now.
    pub peers: Vec<PeerKey>,
    /// The permissions of the local participant were revoked.
    pub local: bool,
}

/// Topic name to pass to the per-topic gate accessors when the caller has
/// no user topic: the builtin endpoints, and the participant-wide default a
/// crypto plugin is armed with before any endpoint exists.
///
/// The empty name matches a `<topic_expression>*` catch-all rule and nothing
/// else, so a single-rule governance keeps applying its one rule
/// participant-wide, while a governance whose rules name concrete topics
/// contributes no participant-wide default — instead of the first rule in
/// document order standing in for every topic.
pub const PARTICIPANT_WIDE_TOPIC: &str = "";

/// The name of the property the middleware adds to the `Property_t` it passes
/// to `register_local_datawriter` and `register_local_datareader` for the
/// builtin volatile message endpoints (DDS-Security 1.2 §9.8.9.1).
pub const PROP_BUILTIN_ENDPOINT_NAME: &str = "dds.sec.builtin_endpoint_name";

/// Its value for the BuiltinParticipantVolatileMessageSecureWriter.
pub const BUILTIN_VOLATILE_MESSAGE_WRITER: &str = "BuiltinParticipantVolatileMessageSecureWriter";

/// Its value for the BuiltinParticipantVolatileMessageSecureReader.
pub const BUILTIN_VOLATILE_MESSAGE_READER: &str = "BuiltinParticipantVolatileMessageSecureReader";

/// Which create check of the local participant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LocalCheck {
    Writer,
    Reader,
    RelayReader,
}

impl From<AccessDecision> for EndpointAccess {
    fn from(decision: AccessDecision) -> Self {
        match decision {
            AccessDecision::Permit => Self::Permit,
            AccessDecision::Deny => Self::Deny,
        }
    }
}

impl From<RemoteReaderDecision> for EndpointAccess {
    fn from(decision: RemoteReaderDecision) -> Self {
        match decision {
            RemoteReaderDecision::Permit => Self::Permit,
            RemoteReaderDecision::RelayOnly => Self::RelayOnly,
            RemoteReaderDecision::Deny => Self::Deny,
        }
    }
}

impl AccessState {
    /// The handle of the participant at `peer`; [`PermissionsHandle::NIL`]
    /// for one the plugin did not validate.
    fn remote_handle(&self, peer: &PeerKey) -> PermissionsHandle {
        self.remote
            .get(peer)
            .copied()
            .unwrap_or(PermissionsHandle::NIL)
    }
}

fn no_access_control() -> SecurityGateError {
    SecurityGateError::AccessControl(zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::InvalidConfiguration,
        "security-runtime: the gate has no AccessControl plugin",
    ))
}

fn poisoned_queue() -> SecurityGateError {
    SecurityGateError::AccessControl(zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::Internal,
        "security-runtime: the revocation queue is poisoned",
    ))
}

impl GateInner {
    fn domain(&self) -> zerodds_security_permissions::Domain<'_> {
        zerodds_security_permissions::Domain {
            id: self.domain_id,
            tag: &self.domain_tag,
        }
    }

    /// Installs `plugin` with the local handle `local` and the gate's
    /// listener.
    fn install(
        &mut self,
        mut plugin: Box<dyn AccessControlPlugin>,
        local: PermissionsHandle,
    ) -> Result<(), SecurityGateError> {
        let revoked = Arc::new(RevocationQueue::default());
        plugin
            .set_listener(revoked.clone())
            .map_err(SecurityGateError::AccessControl)?;
        self.access = Some(AccessState {
            plugin,
            local,
            remote: BTreeMap::new(),
            participant_config: None,
            revoked,
        });
        Ok(())
    }

    /// Builds the builtin plugin `DDS:Access:Permissions` from the
    /// governance and the permissions the gate was given, unless an
    /// application plugin replaced it or no permissions were given yet.
    fn rebuild_builtin(&mut self) -> Result<(), SecurityGateError> {
        if self.builtin.replaced {
            return Ok(());
        }
        let Some(permissions) = self.builtin.permissions.clone() else {
            return Ok(());
        };
        let props = PropertyList::new();
        let participant = ParticipantRequest {
            domain_id: self.domain_id,
            domain_tag: &self.domain_tag,
            participant_guid: self.local_guid,
            properties: &props,
            identity_algorithms: self.builtin.identity_algorithms,
        };
        let mut plugin = PermissionsAccessControl::new().with_governance(self.governance.clone());
        if let Some(class) = &self.builtin.token_class_id {
            plugin
                .set_token_class_id(class)
                .map_err(SecurityGateError::AccessControl)?;
        }
        plugin.set_subject_name_form(self.builtin.subject_name_form);
        plugin.set_algorithm_names(self.builtin.algorithm_names);
        let local = match self.builtin_document_texts() {
            // §9.4.2.9.1: the middleware calls `validate_local_permissions`
            // with the documents; the plugin verifies and binds them (Table
            // 70). That takes them as property values, so as text.
            Some(properties) => {
                let request = ParticipantRequest {
                    properties: &properties,
                    ..participant
                };
                let handle = plugin
                    .validate_local_permissions(self.local_identity, &request)
                    .map_err(SecurityGateError::AccessControl)?;
                // The governance of the gate carries the settings made on it
                // (the partition matching).
                plugin.set_governance(self.governance.clone());
                handle
            }
            // A binary document is registered as the profile verified it.
            None => match (
                &self.builtin.signed_permissions,
                &self.builtin.permissions_ca_pem,
            ) {
                (Some(signed), Some(ca)) => plugin
                    .register_local(
                        self.governance.clone(),
                        permissions,
                        signed.clone(),
                        ca,
                        &participant,
                    )
                    .map_err(SecurityGateError::AccessControl)?,
                (_, ca) => {
                    if let Some(ca) = ca {
                        plugin.arm_remote_permissions_verifier(ca).map_err(|e| {
                            SecurityGateError::AccessControl(
                                zerodds_security::error::SecurityError::new(
                                    zerodds_security::error::SecurityErrorKind::InvalidConfiguration,
                                    alloc::format!("permissions CA: {e}"),
                                ),
                            )
                        })?;
                    }
                    plugin.register_local_parsed(permissions, &participant)
                }
            },
        };
        self.install(Box::new(plugin), local)
    }

    /// The properties of `validate_local_permissions` (Table 63, Table 50)
    /// when the gate holds all four documents as text.
    fn builtin_document_texts(&self) -> Option<PropertyList> {
        use zerodds_security::properties::Property;
        let text = |bytes: &Option<Vec<u8>>| -> Option<String> {
            bytes
                .as_ref()
                .and_then(|b| core::str::from_utf8(b).ok())
                .map(String::from)
        };
        Some(
            PropertyList::new()
                .with(Property::local(
                    zerodds_security_permissions::PROP_PERMISSIONS_XML,
                    text(&self.builtin.signed_permissions)?,
                ))
                .with(Property::local(
                    zerodds_security_permissions::PROP_GOVERNANCE,
                    text(&self.builtin.signed_governance)?,
                ))
                .with(Property::local(
                    zerodds_security_permissions::PROP_PERMISSIONS_CA,
                    text(&self.builtin.permissions_ca_pem)?,
                ))
                .with(Property::local(
                    zerodds_security_permissions::PROP_IDENTITY_CERTIFICATE,
                    text(&self.builtin.identity_certificate_pem)?,
                )),
        )
    }

    /// The ParticipantRequest for `guid` with the QoS `properties`.
    fn participant_request<'a>(
        &'a self,
        guid: [u8; 16],
        properties: &'a PropertyList,
    ) -> ParticipantRequest<'a> {
        ParticipantRequest {
            domain_id: self.domain_id,
            domain_tag: &self.domain_tag,
            participant_guid: guid,
            properties,
            identity_algorithms: self.builtin.identity_algorithms,
        }
    }

    /// The config of the local participant, asked of the plugin once.
    fn cached_participant_config(
        &mut self,
    ) -> Result<&ParticipantSecurityConfig, SecurityGateError> {
        let access = self.access.as_mut().ok_or_else(no_access_control)?;
        if access.participant_config.is_none() {
            access.participant_config = Some(
                access
                    .plugin
                    .get_participant_security_config(access.local)
                    .map_err(SecurityGateError::AccessControl)?,
            );
        }
        access
            .participant_config
            .as_ref()
            .ok_or_else(no_access_control)
    }

    fn participant_config(&mut self) -> Result<ParticipantSecurityConfig, SecurityGateError> {
        self.cached_participant_config().cloned()
    }

    /// What the governance alone says when there is no plugin: no topic rule
    /// refuses, an open topic permits, and a controlled one has nobody to
    /// decide ([`EndpointAccess::NoPermissions`] for a local endpoint,
    /// `Deny` for a remote one). `write` is the direction (`None`: a topic,
    /// open if either direction is).
    fn without_plugin(&self, write: Option<bool>, topic: &str, remote: bool) -> EndpointAccess {
        let domain = self.domain();
        if !self.governance.has_topic_rule(domain, topic) {
            return EndpointAccess::Deny;
        }
        let write_open = !self.governance.write_access_controlled(domain, topic);
        let read_open = !self.governance.read_access_controlled(domain, topic);
        let open = match write {
            Some(true) => write_open,
            Some(false) => read_open,
            None => write_open || read_open,
        };
        if open {
            EndpointAccess::Permit
        } else if remote {
            EndpointAccess::Deny
        } else {
            EndpointAccess::NoPermissions
        }
    }

    fn check_local_endpoint(
        &self,
        kind: LocalCheck,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        let request = EndpointRequest {
            domain_id: self.domain_id,
            ..*request
        };
        let Some(access) = self.access.as_ref() else {
            return Ok(self.without_plugin(
                Some(kind == LocalCheck::Writer),
                request.topic_name,
                false,
            ));
        };
        let decision = match kind {
            LocalCheck::Writer => access
                .plugin
                .check_create_datawriter(access.local, &request),
            LocalCheck::Reader => access
                .plugin
                .check_create_datareader(access.local, &request),
            LocalCheck::RelayReader => {
                access
                    .plugin
                    .check_relay(access.local, RelayRole::Reader, &request)
            }
        };
        decision
            .map(EndpointAccess::from)
            .map_err(SecurityGateError::AccessControl)
    }

    fn check_remote_endpoint(
        &self,
        peer: PeerKey,
        reader: bool,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        let request = EndpointRequest {
            domain_id: self.domain_id,
            ..*request
        };
        let Some(access) = self.access.as_ref() else {
            return Ok(self.without_plugin(Some(!reader), request.topic_name, true));
        };
        let handle = access.remote_handle(&peer);
        if reader {
            access
                .plugin
                .check_remote_datareader(handle, &request)
                .map(EndpointAccess::from)
        } else {
            access
                .plugin
                .check_remote_datawriter(handle, &request)
                .map(EndpointAccess::from)
        }
        .map_err(SecurityGateError::AccessControl)
    }
}

fn poisoned<T>(_: PoisonError<T>) -> SecurityGateError {
    SecurityGateError::Crypto(zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::Internal,
        "security-runtime: mutex poisoned",
    ))
}

/// Whether `kind` is `SIGN_WITH_ORIGIN_AUTHENTICATION` or
/// `ENCRYPT_WITH_ORIGIN_AUTHENTICATION` (DDS-Security 1.2 §10.4.1.2.1).
fn is_origin_authenticated(kind: ProtectionKind) -> bool {
    matches!(
        kind,
        ProtectionKind::SignWithOriginAuthentication
            | ProtectionKind::EncryptWithOriginAuthentication
    )
}

impl SharedSecurityGate {
    /// Constructor. The plugin is **owned** by the gate (Box).
    #[must_use]
    pub fn new(
        domain_id: u32,
        governance: Governance,
        crypto: Box<dyn CryptographicPluginInterfaces>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GateInner {
                domain_id,
                domain_tag: String::new(),
                plaintext_loopback: false,
                symmetric_cipher_bits: 256,
                compatibility_checks: true,
                kx_key_fallback: false,
                origin_authentication_fallback: false,
                algo_name_nul: zerodds_security::AlgoNameNul::default(),
                governance,
                access: None,
                builtin: BuiltinInputs::default(),
                local_identity: IdentityHandle(1),
                local_guid: [0; 16],
                send_optional_attributes: true,
                crypto: CryptoLayer::new(crypto),
                local: None,
            })),
            psk: Arc::new(Mutex::new(crate::psk_gate::PskState::default())),
        }
    }

    fn with_inner<R>(
        &self,
        f: impl FnOnce(&mut GateInner) -> Result<R, SecurityGateError>,
    ) -> Result<R, SecurityGateError> {
        let mut g = self.inner.lock().map_err(poisoned)?;
        f(&mut g)
    }

    /// The DomainTag of the participant (`""` for none).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn domain_tag(&self) -> Result<String, SecurityGateError> {
        self.with_inner(|g| Ok(g.domain_tag.clone()))
    }

    /// Sets the DomainTag of the participant; the domain rules of the
    /// governance and the rules of the permissions match on it
    /// (§10.4.1.2.5.1). The tags were not read.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_domain_tag(&self, tag: &str) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.domain_tag = tag.to_string();
            g.rebuild_builtin()
        })
    }

    /// Takes plaintext RTPS messages arriving over loopback on an
    /// RTPS-protected domain. DDS-Security 1.2 §9.4.2.4 Table 33 makes no
    /// exception for loopback; they were taken unconditionally.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_plaintext_loopback(&self, allow: bool) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.plaintext_loopback = allow;
            Ok(())
        })
    }

    /// Sets the key width the crypto plugin runs with (128 or 256 bits).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_symmetric_cipher_bits(&self, bits: u16) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.symmetric_cipher_bits = bits;
            Ok(())
        })
    }

    /// The key width the crypto plugin runs with, 128 or 256 bits.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn symmetric_cipher_bits(&self) -> Result<u16, SecurityGateError> {
        self.with_inner(|g| Ok(g.symmetric_cipher_bits))
    }

    /// Whether participants and endpoints of an incompatible configuration
    /// are kept apart (the default, DDS-Security 1.2 §7.3.23, §7.3.24).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_compatibility_checks(&self, on: bool) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.compatibility_checks = on;
            Ok(())
        })
    }

    /// Whether a submessage or payload that no key of its sender opens is
    /// also tried with the Kx key (`org.zerodds.sec.kx_key_fallback=accept`).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_kx_key_fallback(&self, on: bool) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.kx_key_fallback = on;
            Ok(())
        })
    }

    /// See [`Self::set_kx_key_fallback`].
    ///
    /// # Errors
    /// Mutex poison.
    pub fn kx_key_fallback(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| Ok(g.kx_key_fallback))
    }

    /// Whether, under origin authentication, an RTPS message or submessage
    /// under a key without a receiver-specific key is verified with the common
    /// MAC alone (`org.zerodds.sec.origin_authentication_fallback=accept`)
    /// instead of refused (DDS-Security 1.2 §10.5.3.3 Table 80
    /// `decode_rtps_message`). Takes effect for the local participant
    /// registered after the call.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_origin_authentication_fallback(&self, on: bool) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.origin_authentication_fallback = on;
            Ok(())
        })
    }

    /// See [`Self::set_origin_authentication_fallback`].
    ///
    /// # Errors
    /// Mutex poison.
    pub fn origin_authentication_fallback(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| Ok(g.origin_authentication_fallback))
    }

    /// How the algorithm names of a handshake token are written
    /// (`org.zerodds.sec.algo_name_nul`, DDS-Security 1.2 §7.3.3.1).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_algo_name_nul(
        &self,
        form: zerodds_security::AlgoNameNul,
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.algo_name_nul = form;
            Ok(())
        })
    }

    /// See [`Self::set_algo_name_nul`].
    ///
    /// # Errors
    /// Mutex poison.
    pub fn algo_name_nul(&self) -> Result<zerodds_security::AlgoNameNul, SecurityGateError> {
        self.with_inner(|g| Ok(g.algo_name_nul))
    }

    /// See [`Self::set_compatibility_checks`].
    ///
    /// # Errors
    /// Mutex poison.
    pub fn compatibility_checks(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| Ok(g.compatibility_checks))
    }

    /// Selects the "legacy matching" of the partitions of a permissions
    /// allow rule (§10.4.1.5.3.4.1.4), or the default subset matching.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_partition_matching(
        &self,
        matching: zerodds_security_permissions::PartitionMatching,
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.governance.set_partition_matching(matching);
            g.rebuild_builtin()
        })
    }

    /// Returns the domain id the gate runs for.
    pub fn domain_id(&self) -> Result<u32, SecurityGateError> {
        self.with_inner(|g| Ok(g.domain_id))
    }

    /// Arms the gate with the AccessControl plugin `plugin` of the
    /// application, replacing the builtin one: every access decision of the
    /// gate (DDS-Security 1.2 §9.4.2.9) then goes through it. `local` is the
    /// `PermissionsHandle` its `validate_local_permissions` returned. The
    /// gate listens for the permissions the plugin revokes
    /// ([`Self::take_revoked`]).
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] when the plugin does not take
    /// the listener; mutex poison.
    pub fn set_access_control(
        &self,
        plugin: Box<dyn AccessControlPlugin>,
        local: PermissionsHandle,
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.replaced = true;
            g.install(plugin, local)
        })
    }

    /// Whether the participant announces
    /// `participant_security_optional_attributes` in its
    /// `PID_PARTICIPANT_SECURITY_INFO` (DDS-Security 1.2 §7.3.23, Table 35):
    /// on by default, off is the named setting
    /// `org.zerodds.sec.optional_attributes=omit`, for a peer that reads only the
    /// two masks of version 1.1 and refuses a longer parameter.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_optional_attributes(&self, send: bool) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.send_optional_attributes = send;
            Ok(())
        })
    }

    /// See [`Self::set_optional_attributes`].
    ///
    /// # Errors
    /// Mutex poison.
    pub fn sends_optional_attributes(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| Ok(g.send_optional_attributes))
    }

    /// Sets the identity and the GUID of the local participant that the
    /// AccessControl operations are called with
    /// (`validate_remote_permissions` takes the local identity,
    /// `check_create_participant` the participant's GUID).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_local_participant(
        &self,
        identity: IdentityHandle,
        guid: [u8; 16],
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.local_identity = identity;
            g.local_guid = guid;
            g.rebuild_builtin()
        })
    }

    /// Arms the DDS-Security 1.2 §9.4.2.9.4 / §9.4.2.9.5 endpoint-create
    /// checks with the participant's own permissions document: the gate
    /// builds the builtin plugin `DDS:Access:Permissions` from its
    /// governance and `permissions` (reduced to the grant of the
    /// participant's subject).
    ///
    /// Until this is called, [`Self::check_create_datawriter`] and
    /// [`Self::check_create_datareader`] answer
    /// [`EndpointAccess::NoPermissions`] for every access-controlled topic —
    /// the gate says that it cannot decide instead of permitting, because a
    /// governance that asks for access control and gets none is a
    /// configuration error and not an open domain.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_permissions(&self, permissions: Permissions) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.permissions = Some(permissions);
            g.builtin.signed_permissions = None;
            g.rebuild_builtin()
        })
    }

    /// As [`Self::set_permissions`], with what the builtin plugin needs to
    /// validate the participant (`validate_local_permissions`, Table 70) and
    /// to stand for it on the network: the signed documents, the Permissions
    /// CA, the identity certificate ([`SignedDocuments`]) and the algorithms
    /// of the identity certificate (for `get_participant_security_config`).
    /// The plugin verifies the signatures itself, and binds the grant to the
    /// subject of the certificate.
    /// The participant's config is read at once: a governance that does not
    /// allow the identity certificate's algorithms is an error here
    /// (Table 70).
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] for a CA that does not parse and
    /// for a config the plugin refuses; mutex poison.
    pub fn set_builtin_access_control(
        &self,
        documents: SignedDocuments<'_>,
        identity_algorithms: Option<AlgorithmMasks>,
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.permissions = Some(documents.permissions);
            g.builtin.signed_permissions = Some(documents.signed_permissions.to_vec());
            g.builtin.signed_governance = Some(documents.signed_governance.to_vec());
            g.builtin.identity_certificate_pem = Some(documents.identity_certificate_pem.to_vec());
            g.builtin.permissions_ca_pem = Some(documents.permissions_ca_pem.to_vec());
            g.builtin.identity_algorithms = identity_algorithms;
            g.rebuild_builtin()?;
            let config = g.participant_config()?;
            // The key width of the Cryptographic plugin, and the AES256 of
            // the Kx endpoints (§10.5.2.1.2), must be among the symmetric
            // ciphers the governance allows.
            let width = if g.symmetric_cipher_bits == 128 {
                zerodds_security::algorithm::CBIT_AES128_GCM
            } else {
                zerodds_security::algorithm::CBIT_AES256_GCM
            };
            let needed = width | zerodds_security::algorithm::CBIT_AES256_GCM;
            if needed & !config.algorithm_info.symmetric_cipher.supported_mask != 0 {
                return Err(SecurityGateError::AccessControl(
                    zerodds_security::error::SecurityError::new(
                        zerodds_security::error::SecurityErrorKind::AccessDenied,
                        "security-runtime: <symmetric_cipher> of the governance does not allow \
                         the key width of the Cryptographic plugin and AES256 of the Kx endpoints",
                    ),
                ));
            }
            Ok(())
        })
    }

    /// Names the `class_id` of the PermissionsToken the builtin plugin sends
    /// (`org.zerodds.sec.access.permissions_token_class_id`): the default is the
    /// `DDS:Access:Permissions:1.2` of Table 65, `DDS:Access:Permissions:1.0`
    /// is what other implementations send.
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] for a class of another plugin or
    /// major version; mutex poison.
    pub fn set_permissions_token_class(&self, class: &str) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.token_class_id = Some(class.to_string());
            g.rebuild_builtin()
        })
    }

    /// Names the form of `dds.perm_ca.sn` in the PermissionsToken the builtin
    /// plugin sends (`org.zerodds.sec.subject_name_form`): RFC 4514 by default
    /// (DDS-Security 1.2 §9.4.1.2.3).
    ///
    /// # Errors
    /// Mutex poison; a rebuild of the plugin that fails.
    pub fn set_subject_name_form(
        &self,
        form: zerodds_security::SubjectNameForm,
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.subject_name_form = form;
            g.rebuild_builtin()
        })
    }

    /// Names the algorithm names of `dds.perm_ca.algo` in the PermissionsToken
    /// the builtin plugin sends (`org.zerodds.sec.algorithm_names`): those of
    /// DDS-Security 1.2 §8.2 Table 25 by default, those of 1.1 with
    /// [`zerodds_security_pki::AlgorithmNames::V1_1`].
    ///
    /// # Errors
    /// Mutex poison; a rebuild of the plugin that fails.
    pub fn set_algorithm_names(
        &self,
        names: zerodds_security_pki::AlgorithmNames,
    ) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.algorithm_names = names;
            g.rebuild_builtin()
        })
    }

    /// Stores a remote participant's permissions document (from its handshake
    /// `c.perm`), keyed by its `PeerKey`, through the plugin's
    /// `validate_remote_permissions` (§9.4.2.9.2): `credential` is the
    /// `AuthenticatedPeerCredentialToken` of the peer (`c.id`, `c.perm`),
    /// `permissions_token_class` the class id of its SPDP `PID_PERMISSIONS_TOKEN`.
    /// The remote endpoint access checks (§9.4.2.9.10 / §9.4.2.9.11) then
    /// consult the handle; a peer without one is checked with
    /// [`PermissionsHandle::NIL`] and denied on every access-controlled
    /// topic. A document that is not authenticated, or without a grant for
    /// the subject, leaves the peer without a handle, also when an earlier
    /// document had given one. So does a peer whose permissions_token is
    /// missing or differs from the plugin's in PluginClassName or
    /// MajorVersion (Table 70).
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] for a refusal the outcomes of
    /// [`RemotePermissions`] do not name; mutex poison.
    pub fn set_remote_permissions(
        &self,
        peer: PeerKey,
        remote_identity: IdentityHandle,
        permissions_token_class: Option<&str>,
        credential: &DataHolder,
    ) -> Result<RemotePermissions, SecurityGateError> {
        self.with_inner(|g| {
            let local_identity = g.local_identity;
            let Some(access) = g.access.as_mut() else {
                return Ok(RemotePermissions::NoVerifier);
            };
            access.remote.remove(&peer);
            let Some(class) = permissions_token_class.filter(|class| {
                zerodds_security::token::class_id::same_class_and_major_version(
                    class,
                    access.plugin.plugin_class_id(),
                )
            }) else {
                return Ok(RemotePermissions::IncompatibleToken);
            };
            let token = DataHolder::new(class);
            match access.plugin.validate_remote_permissions(
                local_identity,
                remote_identity,
                &token,
                credential,
            ) {
                Ok(handle) => {
                    access.remote.insert(peer, handle);
                    Ok(RemotePermissions::Granted)
                }
                Err(e) if e.kind == zerodds_security::error::SecurityErrorKind::AccessDenied => {
                    match e.minor_code {
                        minor_code::NO_VERIFIER => Ok(RemotePermissions::NoVerifier),
                        minor_code::NOT_VERIFIED => Ok(RemotePermissions::NotVerified),
                        minor_code::NO_GRANT_FOR_SUBJECT => {
                            Ok(RemotePermissions::NoGrantForSubject)
                        }
                        _ => Err(SecurityGateError::AccessControl(e)),
                    }
                }
                Err(e) => Err(SecurityGateError::AccessControl(e)),
            }
        })
    }

    /// Arms the Permissions CA (PEM bundle) that a peer's signed `c.perm` is
    /// verified against by the builtin plugin
    /// ([`Self::set_remote_permissions`]); usually the same as the identity
    /// CA.
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] for a CA that does not parse;
    /// mutex poison.
    pub fn arm_remote_permissions_verifier(&self, ca_pem: &[u8]) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.builtin.permissions_ca_pem = Some(ca_pem.to_vec());
            g.rebuild_builtin()
        })
    }

    /// The permissions the plugin revoked since the last call (§9.4.2.10
    /// `on_revoke_permissions`): the gate forgets the handles of the peers
    /// and the caller ignores those participants, as for a participant that
    /// fails `check_remote_participant` (§9.4.2.9.9: "shall be considered
    /// invalid [...] un-matching the remote DomainParticipant").
    ///
    /// # Errors
    /// Mutex poison.
    pub fn take_revoked(&self) -> Result<RevokedPermissions, SecurityGateError> {
        self.with_inner(|g| {
            let mut out = RevokedPermissions::default();
            let Some(access) = g.access.as_mut() else {
                return Ok(out);
            };
            let handles: Vec<PermissionsHandle> = match access.revoked.handles.lock() {
                Ok(mut queue) => core::mem::take(&mut *queue),
                Err(_) => return Err(poisoned_queue()),
            };
            for handle in handles {
                if handle == access.local {
                    out.local = true;
                }
                access.remote.retain(|peer, h| {
                    if *h == handle {
                        out.peers.push(*peer);
                        false
                    } else {
                        true
                    }
                });
            }
            Ok(out)
        })
    }

    /// `true` once the gate has an AccessControl plugin
    /// ([`Self::set_access_control`], [`Self::set_permissions`]).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn has_permissions(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| Ok(g.access.is_some()))
    }

    /// DDS-Security 1.2 §9.4.2.9.3 `check_create_participant`: may the local
    /// participant join the domain with the QoS `properties`? Asked when the
    /// participant is created and at `set_qos`. Without a plugin the domain
    /// must exist in the governance and not require join access control.
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] when the plugin fails; mutex
    /// poison.
    pub fn check_create_participant(
        &self,
        properties: &PropertyList,
    ) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            let Some(access) = g.access.as_ref() else {
                return Ok(g
                    .governance
                    .find_domain_rule(g.domain())
                    .is_some_and(|rule| !rule.enable_join_access_control));
            };
            let request = g.participant_request(g.local_guid, properties);
            access
                .plugin
                .check_create_participant(access.local, &request)
                .map(AccessDecision::is_permitted)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.4 `check_create_datawriter` — may a
    /// DataWriter be created for the topic, partitions, data tags and QoS of
    /// `request`? (The domain of the request is the gate's.)
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] when the plugin fails; mutex
    /// poison.
    pub fn check_create_datawriter(
        &self,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| g.check_local_endpoint(LocalCheck::Writer, request))
    }

    /// DDS-Security 1.2 §9.4.2.9.5 `check_create_datareader` — the mirror of
    /// [`Self::check_create_datawriter`].
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn check_create_datareader(
        &self,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| g.check_local_endpoint(LocalCheck::Reader, request))
    }

    /// May this participant create a relay reader for the topic of
    /// `request`: one that receives and stores the topic for forwarding
    /// without the key to its data (DDS-Security 1.2 §7.1.1.4, the
    /// `<relay>` action of §10.4.1.5.3.4.1.2). The mirror of
    /// [`Self::check_create_datareader`] over `<relay>` instead of
    /// `<subscribe>`.
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn check_create_relay_datareader(
        &self,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| g.check_local_endpoint(LocalCheck::RelayReader, request))
    }

    /// DDS-Security 1.2 §9.4.2.9.6 `check_create_topic`.
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn check_create_topic(
        &self,
        topic_name: &str,
        qos: QosEntries<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| {
            let request = TopicRequest {
                domain_id: g.domain_id,
                topic_name,
                qos,
            };
            let Some(access) = g.access.as_ref() else {
                return Ok(g.without_plugin(None, topic_name, false));
            };
            access
                .plugin
                .check_create_topic(access.local, &request)
                .map(EndpointAccess::from)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// `true` when the instance-level operations of the plugin are worth
    /// calling: the plugin is one of the application (`set_access_control`).
    /// The builtin plugins return TRUE for them (Table 70, Table 85), and
    /// the gate does not take the lock, build the key and ask for each
    /// instance of each writer to be told so.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn application_access_control(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| Ok(g.builtin.replaced && g.access.is_some()))
    }

    /// DDS-Security 1.2 §9.4.2.9.7 `check_local_datawriter_register_instance`
    /// and §9.4.2.9.8 `..._dispose_instance` (`dispose`): may the local
    /// DataWriter `writer` register or dispose the instance with the
    /// serialized key `key`? Without a plugin nothing restricts it.
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] when the plugin fails; mutex
    /// poison.
    pub fn check_local_datawriter_instance(
        &self,
        writer: [u8; 16],
        key: &[u8],
        dispose: bool,
    ) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            let Some(access) = g.access.as_ref() else {
                return Ok(true);
            };
            let decision = if dispose {
                access
                    .plugin
                    .check_local_datawriter_dispose_instance(access.local, writer, key)
            } else {
                access
                    .plugin
                    .check_local_datawriter_register_instance(access.local, writer, key)
            };
            decision
                .map(AccessDecision::is_permitted)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.15 `check_remote_datawriter_register_instance`
    /// and §9.4.2.9.16 `..._dispose_instance` (`dispose`): may the remote
    /// DataWriter `publication` of `peer` register or dispose the instance
    /// with the key `key`, as seen by the local DataReader `reader`?
    ///
    /// # Errors
    /// See [`Self::check_local_datawriter_instance`].
    pub fn check_remote_datawriter_instance(
        &self,
        peer: PeerKey,
        reader: [u8; 16],
        publication: [u8; 16],
        key: &[u8],
        dispose: bool,
    ) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            let Some(access) = g.access.as_ref() else {
                return Ok(true);
            };
            let handle = access.remote_handle(&peer);
            let decision = if dispose {
                access.plugin.check_remote_datawriter_dispose_instance(
                    handle,
                    reader,
                    publication,
                    key,
                )
            } else {
                access.plugin.check_remote_datawriter_register_instance(
                    handle,
                    reader,
                    publication,
                    key,
                )
            };
            decision
                .map(AccessDecision::is_permitted)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.13 `check_local_datawriter_match` (and, with
    /// `reader_is_local`, §9.4.2.9.14 `check_local_datareader_match` for a
    /// local DataReader and the DataWriter it matches): may the DataWriter
    /// `publication` and the DataReader `subscription` match, as far as the
    /// data tags decide? `peer` is the remote participant; the endpoints of
    /// the remote side are checked before with
    /// [`Self::check_remote_datawriter`]/[`Self::check_remote_datareader`].
    /// A match with an endpoint of the local participant is
    /// `peer == None`.
    ///
    /// # Errors
    /// See [`Self::check_local_datawriter_instance`].
    pub fn check_match(
        &self,
        local_is_writer: bool,
        peer: Option<PeerKey>,
        publication: &EndpointRequest<'_>,
        subscription: &EndpointRequest<'_>,
    ) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            let Some(access) = g.access.as_ref() else {
                return Ok(true);
            };
            let local = access.local;
            let other = peer.map_or(local, |p| access.remote_handle(&p));
            let publication = EndpointRequest {
                domain_id: g.domain_id,
                ..*publication
            };
            let subscription = EndpointRequest {
                domain_id: g.domain_id,
                ..*subscription
            };
            let decision = if local_is_writer {
                access.plugin.check_local_datawriter_match(
                    local,
                    other,
                    &publication,
                    &subscription,
                )
            } else {
                access.plugin.check_local_datareader_match(
                    other,
                    local,
                    &subscription,
                    &publication,
                )
            };
            decision
                .map(AccessDecision::is_permitted)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.12 `check_remote_topic`: may `peer` create
    /// the topic (with the QoS of its topic data)?
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn check_remote_topic(
        &self,
        peer: PeerKey,
        topic_name: &str,
        qos: QosEntries<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| {
            let request = TopicRequest {
                domain_id: g.domain_id,
                topic_name,
                qos,
            };
            let Some(access) = g.access.as_ref() else {
                return Ok(g.without_plugin(None, topic_name, true));
            };
            access
                .plugin
                .check_remote_topic(access.remote_handle(&peer), &request)
                .map(EndpointAccess::from)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.10 `check_remote_datawriter` — may the
    /// remote writer at `peer` publish the topic of `request`, in its
    /// partitions and with its data tags and QoS?
    ///
    /// [`EndpointAccess::Permit`] when the topic is not write-access-controlled
    /// by the governance, or the peer's grant permits it.
    /// [`EndpointAccess::Deny`] when the governance access-controls the topic
    /// and the peer has no handle ([`Self::set_remote_permissions`] did not
    /// give one) or its grant does not permit it. A peer whose grant is not
    /// known yet is denied; the runtime matches again once it is.
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn check_remote_datawriter(
        &self,
        peer: PeerKey,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| g.check_remote_endpoint(peer, false, request))
    }

    /// DDS-Security 1.2 §9.4.2.9.11 `check_remote_datareader` — the mirror of
    /// [`Self::check_remote_datawriter`] for a remote reader; a grant that
    /// allows only `<relay>` gives [`EndpointAccess::RelayOnly`].
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn check_remote_datareader(
        &self,
        peer: PeerKey,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointAccess, SecurityGateError> {
        self.with_inner(|g| g.check_remote_endpoint(peer, true, request))
    }

    /// DDS-Security 1.2 §7.1.1.4: may the remote participant `peer` forward
    /// the topic of `request` on behalf of other writers — the `<relay>`
    /// action in its grant where the topic is write-access-controlled. A
    /// reader takes the original writer's identity from a sample of a writer
    /// of `peer` only then.
    ///
    /// # Errors
    /// See [`Self::check_create_datawriter`].
    pub fn remote_may_relay(
        &self,
        peer: PeerKey,
        request: &EndpointRequest<'_>,
    ) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            let request = EndpointRequest {
                domain_id: g.domain_id,
                ..*request
            };
            let Some(access) = g.access.as_ref() else {
                return Ok(g.without_plugin(Some(true), request.topic_name, true)
                    == EndpointAccess::Permit);
            };
            access
                .plugin
                .check_relay(access.remote_handle(&peer), RelayRole::Writer, &request)
                .map(AccessDecision::is_permitted)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.22 `get_participant_security_config` of the
    /// local participant, as the plugin gave it (the gate keeps it).
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] without a plugin or when the
    /// plugin refuses; mutex poison.
    pub fn participant_security_config(
        &self,
    ) -> Result<ParticipantSecurityConfig, SecurityGateError> {
        self.with_inner(GateInner::participant_config)
    }

    /// DDS-Security 1.2 §9.4.2.9.23 `get_topic_security_config` for
    /// `topic_name`.
    ///
    /// # Errors
    /// See [`Self::participant_security_config`].
    pub fn topic_security_config(
        &self,
        topic_name: &str,
    ) -> Result<TopicSecurityConfig, SecurityGateError> {
        self.with_inner(|g| {
            let access = g.access.as_mut().ok_or_else(no_access_control)?;
            let config = access
                .plugin
                .get_topic_security_config(access.local, topic_name)
                .map_err(SecurityGateError::AccessControl)?;
            access
                .plugin
                .return_topic_security_config(config)
                .map_err(SecurityGateError::AccessControl)?;
            Ok(config)
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.24 `get_datawriter_security_config`
    /// (`reader` false) or §9.4.2.9.25 `get_datareader_security_config`
    /// (`reader` true) for the endpoint of `request`; called after the
    /// create check.
    ///
    /// # Errors
    /// See [`Self::participant_security_config`].
    pub fn endpoint_security_config(
        &self,
        reader: bool,
        request: &EndpointRequest<'_>,
    ) -> Result<EndpointSecurityConfig, SecurityGateError> {
        self.with_inner(|g| {
            let request = EndpointRequest {
                domain_id: g.domain_id,
                ..*request
            };
            let access = g.access.as_mut().ok_or_else(no_access_control)?;
            if reader {
                let config = access
                    .plugin
                    .get_datareader_security_config(access.local, &request)
                    .map_err(SecurityGateError::AccessControl)?;
                access
                    .plugin
                    .return_datareader_security_config(config.clone())
                    .map_err(SecurityGateError::AccessControl)?;
                Ok(config)
            } else {
                let config = access
                    .plugin
                    .get_datawriter_security_config(access.local, &request)
                    .map_err(SecurityGateError::AccessControl)?;
                access
                    .plugin
                    .return_datawriter_security_config(config.clone())
                    .map_err(SecurityGateError::AccessControl)?;
                Ok(config)
            }
        })
    }

    /// The `PermissionsToken` of the local participant for its SPDP
    /// announcement (§9.4.2.9.17 `get_permissions_token`), CDR little
    /// endian; the gate returns it to the plugin
    /// (`return_permissions_token`).
    ///
    /// # Errors
    /// See [`Self::participant_security_config`].
    pub fn local_permissions_token(&self) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let access = g.access.as_mut().ok_or_else(no_access_control)?;
            let token = access
                .plugin
                .get_permissions_token(access.local)
                .map_err(SecurityGateError::AccessControl)?;
            let bytes = token.to_cdr_le();
            access
                .plugin
                .return_permissions_token(token)
                .map_err(SecurityGateError::AccessControl)?;
            Ok(bytes)
        })
    }

    /// The `PermissionsCredentialToken` of the local participant
    /// (§9.4.2.9.18 `get_permissions_credential_token`), for the
    /// authentication plugin's `set_permissions_credential_and_token`; the
    /// gate returns it to the plugin.
    ///
    /// # Errors
    /// See [`Self::participant_security_config`]; a plugin without a signed
    /// document for the participant refuses.
    pub fn local_permissions_credential_token(&self) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| {
            let access = g.access.as_mut().ok_or_else(no_access_control)?;
            let token = access
                .plugin
                .get_permissions_credential_token(access.local)
                .map_err(SecurityGateError::AccessControl)?;
            let bytes = token.to_cdr_le();
            access
                .plugin
                .return_permissions_credential_token(token)
                .map_err(SecurityGateError::AccessControl)?;
            Ok(bytes)
        })
    }

    /// `ProtectionKind` for the message level — derived from the first
    /// matching `<domain_rule>`.
    pub fn message_protection(&self) -> Result<ProtectionKind, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain())
                .map(|r| r.rtps_protection_kind)
                .unwrap_or(ProtectionKind::None))
        })
    }

    /// `ProtectionLevel` for user DATA on `topic_name` — the
    /// `data_protection_kind` of the `<topic_rule>` whose
    /// `<topic_expression>` matches that topic (DDS-Security 1.2 §10.4.1.2.6,
    /// Table 41). Used by the user outbound path as a fallback when
    /// the matched reader has not announced an explicit `security_info` level via
    /// SEDP — this way ZeroDDS↔ZeroDDS encrypts user data
    /// according to its own governance, while SPDP/SEDP metatraffic
    /// bootstraps plaintext via `rtps_protection_kind=NONE`.
    ///
    /// Every protection attribute is a property of the topic, not of the
    /// participant: a governance with several `<topic_rule>` elements
    /// protects `PD_RWA_EM_ED` and leaves `OD_OA_OM_OD` in the clear, in
    /// one and the same domain. [`PARTICIPANT_WIDE_TOPIC`] is the name to
    /// pass where no topic exists.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn data_protection(&self, topic_name: &str) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .effective_topic_rule(g.domain(), topic_name)
                .data_protection_kind;
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// `ProtectionLevel` for submessage metadata on `topic_name` — from the
    /// `metadata_protection_kind` of the matching `<topic_rule>`.
    /// Controls (together with [`Self::data_protection`]) the
    /// `endpoint_security_attributes` that ZeroDDS announces via SEDP —
    /// cross-vendor the mask must match cyclone/OpenDDS byte-exactly
    /// (otherwise "security_attributes mismatch" → no endpoint match).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn metadata_protection(
        &self,
        topic_name: &str,
    ) -> Result<ProtectionLevel, SecurityGateError> {
        self.with_inner(|g| {
            let kind = g
                .governance
                .effective_topic_rule(g.domain(), topic_name)
                .metadata_protection_kind;
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// Whether the Metadata Protection Kind of the rule of `topic_name` is a
    /// `*_WITH_ORIGIN_AUTHENTICATION` kind: the key material to a remote
    /// endpoint then carries a receiver-specific key and the submessages a
    /// MAC for it (DDS-Security 1.2 §10.4.1.2.6.6,
    /// `is_submessage_origin_authenticated`).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn metadata_origin_authenticated(
        &self,
        topic_name: &str,
    ) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(is_origin_authenticated(
                g.governance
                    .effective_topic_rule(g.domain(), topic_name)
                    .metadata_protection_kind,
            ))
        })
    }

    /// Whether the `discovery_protection_kind` of the domain is a
    /// `*_WITH_ORIGIN_AUTHENTICATION` kind (§10.4.1.2.5.5,
    /// `is_discovery_origin_authenticated`).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn discovery_origin_authenticated(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(is_origin_authenticated(
                g.governance
                    .find_domain_rule(g.domain())
                    .map(|r| r.discovery_protection_kind)
                    .unwrap_or_default(),
            ))
        })
    }

    /// Whether the `liveliness_protection_kind` of the domain is a
    /// `*_WITH_ORIGIN_AUTHENTICATION` kind (§10.4.1.2.5.6,
    /// `is_liveliness_origin_authenticated`).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn liveliness_origin_authenticated(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(is_origin_authenticated(
                g.governance
                    .find_domain_rule(g.domain())
                    .map(|r| r.liveliness_protection_kind)
                    .unwrap_or_default(),
            ))
        })
    }

    /// Whether the `rtps_protection_kind` of the domain is a
    /// `*_WITH_ORIGIN_AUTHENTICATION` kind (§10.4.1.2.5.7,
    /// `is_rtps_origin_authenticated`).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn rtps_origin_authenticated(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(is_origin_authenticated(
                g.governance
                    .find_domain_rule(g.domain())
                    .map(|r| r.rtps_protection_kind)
                    .unwrap_or_default(),
            ))
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
                .find_domain_rule(g.domain())
                .map(|r| r.discovery_protection_kind)
                .unwrap_or(ProtectionKind::None);
            Ok(ProtectionLevel::from_protection_kind(kind))
        })
    }

    /// `true` if the `<topic_rule>` matching `topic_name` sets
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
    pub fn topic_discovery_protected(&self, topic_name: &str) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .effective_topic_rule(g.domain(), topic_name)
                .enable_discovery_protection)
        })
    }

    /// `true` if the `<topic_rule>` matching `topic_name` sets
    /// `enable_liveliness_protection` (`is_liveliness_protected` of the
    /// TopicSecurityConfig, DDS-Security 1.2 §10.4.1.2.6.3, Table 36): the
    /// liveliness of a writer of this topic goes over the secure
    /// DCPSParticipantMessageSecure topic (§7.5.2).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn topic_liveliness_protected(&self, topic_name: &str) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .effective_topic_rule(g.domain(), topic_name)
                .enable_liveliness_protection)
        })
    }

    /// `true` if the `<topic_rule>` matching `topic_name` sets
    /// `enable_read_access_control` → endpoint bit `IS_READ_PROTECTED`
    /// (0x01) of the `EndpointSecurityAttributes` (§9.4.2.4). cyclone sets it for
    /// access-control topics; ZeroDDS must announce byte-exactly the same mask.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn topic_read_protected(&self, topic_name: &str) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .effective_topic_rule(g.domain(), topic_name)
                .enable_read_access_control)
        })
    }

    /// `true` if the `<topic_rule>` matching `topic_name` sets
    /// `enable_write_access_control` → endpoint bit `IS_WRITE_PROTECTED`
    /// (0x02) of the `EndpointSecurityAttributes` (§9.4.2.4).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn topic_write_protected(&self, topic_name: &str) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .effective_topic_rule(g.domain(), topic_name)
                .enable_write_access_control)
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
                .find_domain_rule(g.domain())
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
                .find_domain_rule(g.domain())
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
        let params = self.with_inner(|g| {
            // §10.5.3.1 register_local_participant: GCM or GMAC "according to
            // the setting of the RTPS Protection Kind", and origin
            // authentication when it specifies it.
            let kind = g
                .governance
                .find_domain_rule(g.domain())
                .map_or(ProtectionKind::None, |r| r.rtps_protection_kind);
            Ok(LocalParams {
                encrypt: !matches!(
                    kind,
                    ProtectionKind::Sign | ProtectionKind::SignWithOriginAuthentication
                ),
                origin: is_origin_authenticated(kind),
                bits: g.symmetric_cipher_bits,
                config: g.cached_participant_config().ok().cloned(),
                // Without an AccessControl plugin the Cryptographic plugin gets
                // a placeholder: it refuses the NIL handle.
                permissions: g
                    .access
                    .as_ref()
                    .map(|a| a.local)
                    .filter(|h| *h != PermissionsHandle::NIL)
                    .unwrap_or(PermissionsHandle(1)),
                // §9.4.2.4 `ac_participant_properties`: what the AccessControl
                // plugin adds to the `participant_properties` of
                // `register_local_participant`.
                properties: g
                    .cached_participant_config()
                    .map_or_else(
                        |_| Vec::new(),
                        |config| {
                            config
                                .ac_participant_properties
                                .iter()
                                .map(|p| (p.name.to_string(), p.value.to_string()))
                                .collect()
                        },
                    )
                    .into_iter()
                    .chain(g.origin_authentication_fallback.then(|| {
                        (
                            String::from(
                                zerodds_security_crypto::PROP_ORIGIN_AUTHENTICATION_FALLBACK,
                            ),
                            String::from("accept"),
                        )
                    }))
                    .collect(),
            })
        })?;
        self.with_inner(|g| {
            if let Some(h) = g.local {
                return Ok(h);
            }
            let h = g.crypto.ensure_local(params)?;
            g.local = Some(h);
            Ok(h)
        })
    }

    /// Token of the local participant (to be sent to the remote via SEDP).
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    pub fn local_token(&self) -> Result<Vec<u8>, SecurityGateError> {
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.participant_token(None))
    }

    /// The token of the local participant for the registered peer
    /// `peer_key`: the participant key and, with origin authentication in the
    /// RTPS Protection Kind, the receiver-specific key of that one peer
    /// (`Participant2ParticipantKeyMaterial`, DDS-Security 1.2 §10.5.3.1).
    ///
    /// # Errors
    /// `PolicyViolation` if the peer is not registered; `Crypto`.
    pub fn local_token_for(&self, peer_key: &PeerKey) -> Result<Vec<u8>, SecurityGateError> {
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.participant_token(Some(peer_key)))
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
        self.ensure_local()?;
        self.with_inner(|g| {
            // No peer key was given: the peer gets one of its own, so that
            // the handle it returns names it.
            let mut n = g.crypto.peer_keys().len() as u64;
            let key = loop {
                let mut key = [0xEE; 12];
                key[4..].copy_from_slice(&n.to_be_bytes());
                if g.crypto.peer_handle(&key).is_none() {
                    break key;
                }
                n += 1;
            };
            let slot = g
                .crypto
                .register_peer(key, remote_identity, shared_secret)?;
            if let Err(e) = g.crypto.set_participant_token(&key, token) {
                g.crypto.forget_peer(&key)?;
                return Err(e);
            }
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
        {
            let g = self.inner.lock().map_err(poisoned)?;
            if let Some(h) = g.crypto.peer_handle(&peer_key) {
                return Ok(h);
            }
        }
        self.ensure_local()?;
        self.with_inner(|g| {
            let slot = g
                .crypto
                .register_peer(peer_key, remote_identity, shared_secret)?;
            if let Err(e) = g.crypto.set_participant_token(&peer_key, token) {
                g.crypto.forget_peer(&peer_key)?;
                return Err(e);
            }
            Ok(slot)
        })
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
            if let Some(h) = g.crypto.peer_handle(&peer_key) {
                return Ok(h);
            }
        }
        self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .register_peer(peer_key, remote_identity, shared_secret)
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
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.set_participant_token(peer_key, token))
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
        self.with_inner(|g| g.crypto.encode_kx(peer_key, data_submessage))
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
        self.with_inner(|g| g.crypto.decode_kx(peer_key, wire))
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
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.encode_with_participant_key(data_submessage))
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
        let (metadata, data) =
            self.endpoint_protection_kinds(Some(PARTICIPANT_WIDE_TOPIC), false)?;
        self.register_local_endpoint_with_properties(is_writer, metadata, data, None)
    }

    /// The Metadata and the Data Protection Kind of a local endpoint: those
    /// of the `<topic_rule>` of `topic` (DDS-Security 1.2 §10.4.1.2.6.6,
    /// §10.4.1.2.6.7), none without one; for a secure builtin endpoint
    /// (`topic` `None`) the `discovery_protection_kind` or, with
    /// `liveliness`, the `liveliness_protection_kind` of the domain, with no
    /// payload protection (§10.4.1.2.5).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn endpoint_protection_kinds(
        &self,
        topic: Option<&str>,
        liveliness: bool,
    ) -> Result<(ProtectionKind, ProtectionKind), SecurityGateError> {
        self.with_inner(|g| {
            Ok(match topic {
                // The rule that decides the topic, builtin rules included
                // (DDS-Security 1.2 §10.6: the BuiltinLoggingTopic is
                // `SIGN`/`ENCRYPT` whatever the governance names).
                Some(topic) => {
                    let rule = g.governance.effective_topic_rule(g.domain(), topic);
                    (rule.metadata_protection_kind, rule.data_protection_kind)
                }
                None => {
                    let rule = g.governance.find_domain_rule(g.domain());
                    let kind = rule
                        .map(|r| {
                            if liveliness {
                                r.liveliness_protection_kind
                            } else {
                                r.discovery_protection_kind
                            }
                        })
                        .unwrap_or_default();
                    (kind, ProtectionKind::None)
                }
            })
        })
    }

    /// Creates the key material of a local endpoint for its Metadata and
    /// Data Protection Kind (§10.5.3.1 Table 78), from
    /// [`Self::endpoint_protection_kinds`]. [`Self::register_local_endpoint`]
    /// gave every endpoint the suites of the rule for the empty topic name.
    ///
    /// # Errors
    /// `CryptoSetup`.
    pub fn register_local_endpoint_with_kinds(
        &self,
        is_writer: bool,
        metadata: ProtectionKind,
        data: ProtectionKind,
    ) -> Result<CryptoHandle, SecurityGateError> {
        self.register_local_endpoint_with_properties(is_writer, metadata, data, None)
    }

    /// The Metadata and Data Protection Kind of the secure TypeLookup
    /// endpoints: `is_submessage_protected` TRUE, `is_payload_protected`
    /// FALSE (DDS-Security 1.2 §7.5.13 Table 21), submessages encrypted.
    #[must_use]
    pub fn secure_type_lookup_kinds() -> (ProtectionKind, ProtectionKind) {
        (ProtectionKind::Encrypt, ProtectionKind::None)
    }

    /// [`Self::register_local_endpoint_with_kinds`] with the
    /// `dds.sec.crypto.symmetric_cipher_algorithm` of the endpoint's own
    /// `PropertyQosPolicy` (DDS-Security 1.2 §7.3.21, §10.5.1.1): the key
    /// width of this endpoint. `None`, or `AUTO`, leaves the default of the
    /// DomainParticipant.
    ///
    /// # Errors
    /// `CryptoSetup`, among them a value that names no algorithm of §8.1.
    pub fn register_local_endpoint_with_properties(
        &self,
        is_writer: bool,
        metadata: ProtectionKind,
        data: ProtectionKind,
        symmetric_cipher_algorithm: Option<&str>,
    ) -> Result<CryptoHandle, SecurityGateError> {
        self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .register_endpoint(is_writer, metadata, data, symmetric_cipher_algorithm, &[])
        })
    }

    /// Creates the key material of a local endpoint with the properties the
    /// AccessControl plugin gave for it (`ac_endpoint_properties` of its
    /// `EndpointSecurityConfig`, DDS-Security 1.2 §9.4.2.7 Table 37) and the
    /// `dds.sec.crypto.symmetric_cipher_algorithm` of the endpoint's own
    /// `PropertyQosPolicy`, if any (§10.5.1.1).
    ///
    /// # Errors
    /// `CryptoSetup`, among them a cipher value that names no algorithm of
    /// §8.1.
    pub fn register_local_endpoint_with_config(
        &self,
        is_writer: bool,
        metadata: ProtectionKind,
        data: ProtectionKind,
        ac_endpoint_properties: &[zerodds_security::properties::Property],
        symmetric_cipher_algorithm: Option<&str>,
    ) -> Result<CryptoHandle, SecurityGateError> {
        self.ensure_local()?;
        let properties: Vec<(&str, &str)> = ac_endpoint_properties
            .iter()
            .map(|p| (p.name.as_ref(), p.value.as_ref()))
            .collect();
        self.with_inner(|g| {
            g.crypto.register_endpoint(
                is_writer,
                metadata,
                data,
                symmetric_cipher_algorithm,
                &properties,
            )
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
        self.with_inner(|g| g.crypto.endpoint_token(endpoint, None))
    }

    /// The token of the local endpoint `endpoint` for the remote endpoint
    /// `remote_endpoint`: the key material of the endpoint and, when its
    /// Metadata Protection Kind specifies origin authentication, the
    /// receiver-specific key of that one remote endpoint (DDS-Security 1.2
    /// §10.5.3.1 `register_matched_remote_datareader` / `_datawriter`,
    /// §10.5.2.1.1).
    ///
    /// # Errors
    /// `Crypto` (unknown endpoint handle).
    pub fn create_endpoint_token_for(
        &self,
        endpoint: CryptoHandle,
        remote_endpoint: [u8; 16],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| g.crypto.endpoint_token(endpoint, Some(remote_endpoint)))
    }

    /// Installs a received per-endpoint token. The key material is
    /// indexed via its `transformation_key_id` (`remote_by_key_id`), so that
    /// [`Self::decode_data_by_key_id`] maps the submessages of this remote endpoint
    /// — independent of the participant key.
    ///
    /// # Errors
    /// `CryptoSetup` (no local slot) / `Crypto`.
    /// Optional second (payload) token of a DataWriter endpoint whose
    /// submessages and payload have keys of their own. `None` = one key.
    #[must_use]
    pub fn endpoint_payload_token(&self, endpoint: CryptoHandle) -> Option<alloc::vec::Vec<u8>> {
        self.with_inner(|g| Ok(g.crypto.payload_token(endpoint)))
            .ok()
            .flatten()
    }

    /// The DataWriter token for a relay-only reader (DDS-Security 1.2
    /// §9.5.1.9.5): the submessage key material alone. `None` when the
    /// writer has no key that protects its submessages alone.
    #[must_use]
    pub fn endpoint_relay_token(&self, endpoint: CryptoHandle) -> Option<alloc::vec::Vec<u8>> {
        self.with_inner(|g| Ok(g.crypto.relay_token(endpoint)))
            .ok()
            .flatten()
    }

    /// Installs a remote endpoint crypto token of the peer `peer_key`
    /// (volatile key exchange) via `set_remote_endpoint_crypto_tokens`, under
    /// the peer's slot: its `transformation_key_id` identifies the key only
    /// together with the peer (DDS-Security 1.2 §9.5.2.1.1). It was indexed
    /// by the key id alone, so that a second peer with the same key id
    /// replaced the first one's key.
    ///
    /// # Errors
    /// `PolicyViolation` if the peer is not registered; `Crypto` on an
    /// invalid token.
    pub fn install_remote_endpoint_token(
        &self,
        peer_key: &PeerKey,
        token: &[u8],
    ) -> Result<(), SecurityGateError> {
        self.install_remote_endpoint_token_from(peer_key, token, false)
    }

    /// [`Self::install_remote_endpoint_token`] for the key of a remote DataReader
    /// (`from_reader`, `set_remote_datareader_crypto_tokens`) or of a remote
    /// DataWriter (`set_remote_datawriter_crypto_tokens`).
    ///
    /// # Errors
    /// `PolicyViolation` if the peer is not registered; `Crypto` on an
    /// invalid token.
    pub fn install_remote_endpoint_token_from(
        &self,
        peer_key: &PeerKey,
        token: &[u8],
        from_reader: bool,
    ) -> Result<(), SecurityGateError> {
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.set_endpoint_token(peer_key, token, from_reader))
    }

    /// `revise_local_entity_keys` (DDS-Security 1.2 §9.5.1.8.7): a new
    /// revision of the key material of the participant and its endpoints;
    /// the tokens made for it are sent, then [`Self::activate_key_revision`].
    ///
    /// # Errors
    /// `Crypto`.
    pub fn revise_local_entity_keys(&self) -> Result<i32, SecurityGateError> {
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.revise())
    }

    /// The `required_mask` of the `adjusted_algorithm_info` the Cryptographic
    /// plugin returned for the local endpoint `endpoint` (Table 42): what the
    /// endpoint announces as `PID_ENDPOINT_SECURITY_SYMMETRIC_CIPHER` (§7.3.15).
    #[must_use]
    pub fn endpoint_required_mask(&self, endpoint: CryptoHandle) -> Option<u32> {
        self.with_inner(|g| Ok(g.crypto.endpoint_required_mask(endpoint)))
            .ok()
            .flatten()
    }

    /// `is_key_revision_enabled` (DDS-Security 1.2 §9.4.2.4 Table 33): the
    /// `<enable_key_revision>` of the domain rule. The participant revises
    /// its key material on an event (a revoked identity) only if it is set.
    ///
    /// # Errors
    /// Mutex poison.
    pub fn key_revision_enabled(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain())
                .is_some_and(|r| r.enable_key_revision))
        })
    }

    /// `activate_key_revision` (§9.5.1.8.8).
    ///
    /// # Errors
    /// `Crypto`.
    pub fn activate_key_revision(&self, revision: i32) -> Result<bool, SecurityGateError> {
        self.ensure_local()?;
        self.with_inner(|g| g.crypto.activate(revision))
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
                .encode_endpoint_submessage(endpoint, data_submessage)
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
        self.with_inner(|g| g.crypto.decode_with_participant_key(peer_key, wire))
    }

    /// Cross-vendor (recv) by **key_id**: decodes a SEC_* submessage from
    /// the peer `peer_key` without knowing the endpoint handle — the plugin
    /// looks up the peer's key material by the `transformation_key_id` in the
    /// CryptoHeader (§9.5.2.1.1). Needed for protected discovery, where a
    /// peer has its own key per secure builtin endpoint. Only the sender's
    /// keys decode (§10.5.3.3.1): a key of another participant does not.
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (no key of the peer for
    /// the key_id / tag mismatch).
    pub fn decode_data_by_key_id(
        &self,
        peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| g.crypto.decode_submessage(peer_key, wire))
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
        self.with_inner(|g| g.crypto.encode_payload(endpoint, payload))
    }

    /// §9.5.3.3.1 data_protection (recv): key via `transformation_key_id`,
    /// among the keys of the writer's participant `peer_key`.
    ///
    /// # Errors
    /// `PolicyViolation` (unknown peer) / `Crypto` (no key of the peer for
    /// the key_id / tag mismatch).
    pub fn decode_serialized_payload(
        &self,
        peer_key: &PeerKey,
        encoded: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.with_inner(|g| g.crypto.decode_payload(peer_key, encoded))
    }

    /// Prefixes of all peers with which a data key is installed (participant
    /// slot table). STABLE — unlike `completed_peer_prefixes` (derived from the
    /// handshake entries GC'd after completion), this entry persists. Source for
    /// re-sending per-endpoint crypto tokens to
    /// late-matching user endpoints.
    pub fn authenticated_peer_prefixes(&self) -> Vec<PeerKey> {
        self.with_inner(|g| Ok(g.crypto.peer_keys()))
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
            g.crypto
                .decode_payload_with_participant_key(peer_key, encoded)
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
        self.with_inner(|g| g.crypto.decode_payload_with_kx_key(peer_key, encoded))
    }

    /// Forgets a peer that is gone: the peer-key → slot mapping, the slot,
    /// its Kx key and the keys of its endpoints in the plugin, and the
    /// permissions document it presented.
    pub fn forget_remote(&self, peer_key: &PeerKey) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            if let Some(access) = g.access.as_mut() {
                access.remote.remove(peer_key);
            }
            g.crypto.forget_peer(peer_key)
        })
    }

    /// Releases the key material of a local endpoint that was deleted
    /// (DDS-Security 1.2 §9.5.1.8 `unregister_local_endpoint`).
    ///
    /// # Errors
    /// `Crypto` from the plugin.
    pub fn unregister_local_endpoint(&self, handle: CryptoHandle) -> Result<(), SecurityGateError> {
        self.with_inner(|g| g.crypto.unregister_endpoint(handle))
    }

    /// Returns the slot for a peer key, if registered.
    pub fn slot_for(&self, peer_key: &PeerKey) -> Result<Option<CryptoHandle>, SecurityGateError> {
        self.with_inner(|g| Ok(g.crypto.peer_handle(peer_key)))
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
                self.ensure_local()?;
                // Message-level SRTPS (`encode_rtps_message`): the
                // CryptoHeader names the key, so the peer finds it by its
                // transformation_key_id.
                self.with_inner(|g| g.crypto.encode_message(message, None))
            }
        }
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
        peer_key: &PeerKey,
        message: &[u8],
        level: ProtectionLevel,
    ) -> Result<Vec<u8>, SecurityGateError> {
        match level {
            ProtectionLevel::None => Ok(message.to_vec()),
            ProtectionLevel::Sign | ProtectionLevel::Encrypt => {
                self.ensure_local()?;
                // Cyclone-conform message-level SRTPS (CryptoHeader with
                // transformation_key_id in the SRTPS_PREFIX) — identical to the
                // generic transform_outbound. The peer resolves the key by
                // key_id, so NO peer-specific keymat is needed.
                // (Previously: encode_secured_rtps_message = 16-byte null prefix,
                // which decode_rtps_message_cyclone rejects as !=20 -> the asymmetry
                // broke cross-instance/cross-vendor user DATA + 3 unit tests.)
                self.with_inner(|g| g.crypto.encode_message(message, Some(peer_key)))
            }
        }
    }

    /// `is_access_protected` of the ParticipantSecurityConfig
    /// (DDS-Security 1.2 §9.4.2.4 Table 33): with the builtin plugin the domain
    /// rule's `enable_join_access_control`; a plugin of the application says
    /// it in its config. Without a plugin, or when the plugin cannot give the
    /// config, `true` if there is no rule for the domain: a participant
    /// there is refused anyway (§10.4.1.2.7).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn is_access_protected(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            if g.access.is_some() {
                if let Ok(config) = g.cached_participant_config() {
                    return Ok(config.is_access_protected);
                }
            }
            Ok(g.governance
                .find_domain_rule(g.domain())
                .is_none_or(|r| r.enable_join_access_control))
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.9 `check_remote_participant` (Table 70):
    /// asked of the plugin for the participant at `peer` (its handle, or
    /// [`PermissionsHandle::NIL`] for none). The builtin plugin answers
    /// `true` when the domain is not access protected; otherwise only when
    /// the peer's grant ([`Self::set_remote_permissions`]) allows joining the
    /// domain. A peer without a grant is `false`. Without a plugin the
    /// governance decides: not access protected.
    ///
    /// # Errors
    /// [`SecurityGateError::AccessControl`] when the plugin fails; mutex
    /// poison.
    pub fn check_remote_participant(&self, peer: PeerKey) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            let Some(access) = g.access.as_ref() else {
                return Ok(g
                    .governance
                    .find_domain_rule(g.domain())
                    .is_some_and(|rule| !rule.enable_join_access_control));
            };
            let mut guid = [0u8; 16];
            guid[..12].copy_from_slice(&peer);
            // The participant entity id 0x000001c1 (DDSI-RTPS 2.5 §9.3.1.2).
            guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xc1]);
            let props = PropertyList::new();
            let request = g.participant_request(guid, &props);
            access
                .plugin
                .check_remote_participant(access.remote_handle(&peer), &request)
                .map(AccessDecision::is_permitted)
                .map_err(SecurityGateError::AccessControl)
        })
    }

    /// Returns the `allow_unauthenticated_participants` flag from the
    /// domain rule (the config of the plugin, when there is one). Default
    /// `false` if no rule exists for the domain — conservative-safe stance.
    pub fn allow_unauthenticated(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            if g.access.is_some() {
                if let Ok(config) = g.cached_participant_config() {
                    return Ok(config.allow_unauthenticated_participants);
                }
            }
            Ok(g.governance
                .find_domain_rule(g.domain())
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
    /// 1. `bytes.len() < 20` → `Malformed`: no full RTPS header (DDSI-RTPS
    ///    2.5 §8.3.6.3). Exactly 20 is a message without submessages,
    ///    `Accept`: it carries nothing to protect or to deliver.
    /// 2. Extract `peer_key` from `bytes[8..20]`.
    /// 3. If the packet is SRTPS-wrapped → standard unwrap path
    ///    (`transform_inbound_from`). On a crypto error `CryptoError`,
    ///    on an unknown peer `PolicyViolation`.
    /// 4. If the packet is plain AND the domain requires ProtectionKind::None
    ///    → `Accept`.
    /// 5. If the packet is plain AND the domain requires protection
    ///    (`is_rtps_axk_protected`, §9.4.2.4 Table 33): `LegacyBlocked`;
    ///    over `Loopback` or `LocalHost` `Accept` only with
    ///    [`Self::set_plaintext_loopback`]. Such a domain has
    ///    `allow_unauthenticated_participants` FALSE (the governance
    ///    refuses the other combination), so an unauthenticated sender's
    ///    plaintext is not taken. Bootstrap messages (SPDP, stateless) are
    ///    exempt before this check.
    ///
    /// The `iface` context is consulted only for the loopback setting; the
    /// finer per-interface classification is the `PolicyEngine`'s
    /// (governance-XML `<interface_bindings>`).
    #[must_use]
    pub fn classify_inbound(&self, bytes: &[u8], iface: &NetInterface) -> InboundVerdict {
        // DDSI-RTPS 2.5 §8.3.6.3: a header is invalid when "the Message has
        // less than the required number of octets to contain a full Header".
        // A header without submessages is a valid message with nothing in it;
        // Fast DDS sends such messages, and they were reported as malformed.
        if bytes.len() < RTPS_HEADER_LEN {
            return InboundVerdict::Malformed;
        }
        if bytes.len() == RTPS_HEADER_LEN {
            return InboundVerdict::Accept(bytes.to_vec());
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
        // Loopback plaintext only where the configuration allows it; it was
        // taken on every protected domain.
        if matches!(iface, NetInterface::Loopback | NetInterface::LocalHost) {
            match self.with_inner(|g| Ok(g.plaintext_loopback)) {
                Ok(true) => return InboundVerdict::Accept(bytes.to_vec()),
                Ok(false) => {}
                Err(e) => {
                    return InboundVerdict::CryptoError(alloc::format!(
                        "gate lookup failed: {e:?}"
                    ));
                }
            }
        }
        // The domain requires protection and the message is plain.
        InboundVerdict::LegacyBlocked
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
            (_, true) => self.with_inner(|g| g.crypto.decode_message(remote_slot, wire)),
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
    use zerodds_security::logging::{TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2};
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;

    const GOV_RTPS: &str = r#"
<dds><domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>
"#;

    /// Two topic rules; the FIRST is unprotected and the access-controlled
    /// one stands behind it — the shape of the OMG suite's governance.
    const GOV_AC: &str = r#"
<dds><domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants>
    <enable_join_access_control>TRUE</enable_join_access_control>
    <topic_access_rules>
      <topic_rule>
        <topic_expression>OD_OA_OM_OD</topic_expression>
        <enable_read_access_control>FALSE</enable_read_access_control>
        <enable_write_access_control>FALSE</enable_write_access_control>
      </topic_rule>
      <topic_rule>
        <topic_expression>PD_RWA_EM_ED</topic_expression>
        <enable_read_access_control>TRUE</enable_read_access_control>
        <enable_write_access_control>TRUE</enable_write_access_control>
        <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
        <data_protection_kind>ENCRYPT</data_protection_kind>
      </topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>
"#;

    /// An endpoint request for `topic`; the gate fills in its domain.
    fn req(topic: &str) -> EndpointRequest<'_> {
        EndpointRequest::new(0, topic)
    }

    fn ac_gate() -> SharedSecurityGate {
        SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_AC).expect("governance"),
            Box::new(AesGcmCryptoPlugin::new()),
        )
    }

    /// DDS-Security 1.2 §9.4.2.9.10/.11: on an access-controlled topic the
    /// endpoints of a peer without a grant are denied; the open topic is
    /// unaffected. A document the gate cannot authenticate stores no grant.
    /// The remote checks permitted every peer whose grant was not known.
    #[test]
    fn a_peer_without_a_grant_is_denied_on_an_access_controlled_topic() {
        let gate = ac_gate();
        let peer = [7u8; 12];
        assert_eq!(
            gate.check_remote_datawriter(peer, &req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Deny
        );
        assert_eq!(
            gate.check_remote_datareader(peer, &req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Deny
        );
        assert_eq!(
            gate.check_remote_datawriter(peer, &req("OD_OA_OM_OD"))
                .expect("lock"),
            EndpointAccess::Permit
        );
        let credential = DataHolder::new("DDS:Auth:PKI-DH:1.0")
            .with_binary_property("c.id", b"not a certificate".to_vec())
            .with_binary_property("c.perm", b"not a signed document".to_vec());
        assert_eq!(
            gate.set_remote_permissions(
                peer,
                IdentityHandle(2),
                Some("DDS:Access:Permissions:1.0"),
                &credential,
            )
            .expect("lock"),
            RemotePermissions::NoVerifier
        );
        assert_eq!(
            gate.check_remote_datareader(peer, &req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Deny
        );
        // §9.4.2.9.9: under join access control a peer without a grant may
        // not join the domain.
        assert!(gate.is_access_protected().expect("lock"));
        assert!(!gate.check_remote_participant(peer).expect("lock"));
    }

    /// A gate that was never armed with permissions answers
    /// `NoPermissions` — never `Permit` — for an access-controlled topic.
    /// Governance asking for access control and getting an endpoint anyway
    /// is the failure this check exists to stop.
    #[test]
    fn an_unarmed_gate_refuses_an_access_controlled_topic() {
        let gate = ac_gate();
        assert!(!gate.has_permissions().expect("lock"));
        assert_eq!(
            gate.check_create_datawriter(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::NoPermissions
        );
        assert_eq!(
            gate.check_create_datareader(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::NoPermissions
        );
        // §10.4.1.2.7: a topic no rule covers gets no endpoint, local or
        // remote. It was taken as unprotected and permitted.
        assert_eq!(
            gate.check_create_datawriter(&req("NoRuleForThis"))
                .expect("lock"),
            EndpointAccess::Deny
        );
        assert_eq!(
            gate.check_remote_datareader([1; 12], &req("NoRuleForThis"))
                .expect("lock"),
            EndpointAccess::Deny
        );
        // The uncontrolled topic is unaffected: no permissions are needed
        // where the governance asks for none.
        assert_eq!(
            gate.check_create_datawriter(&req("OD_OA_OM_OD"))
                .expect("lock"),
            EndpointAccess::Permit
        );
    }

    /// The §10.6 BuiltinLoggingTopic rule is builtin: its `SIGN`/`ENCRYPT`
    /// protection applies even though the governance does not name the topic.
    #[test]
    fn the_builtin_log_topic_rule_carries_its_protection() {
        let gate = ac_gate();
        assert_eq!(
            gate.data_protection(TOPIC_LOG_TOPIC_V2).expect("lock"),
            ProtectionLevel::from_protection_kind(ProtectionKind::Encrypt)
        );
        assert_eq!(
            gate.metadata_protection(TOPIC_LOG_TOPIC_V2).expect("lock"),
            ProtectionLevel::from_protection_kind(ProtectionKind::Sign)
        );
        assert_eq!(
            gate.data_protection(TOPIC_LOG_TOPIC).expect("lock"),
            ProtectionLevel::from_protection_kind(ProtectionKind::Encrypt)
        );
        // A topic the governance does not name and that is not builtin stays
        // unprotected.
        assert_eq!(
            gate.data_protection("Chatter").expect("lock"),
            ProtectionLevel::from_protection_kind(ProtectionKind::None)
        );
    }

    /// DDS-Security 1.2 §10.6: "any DomainParticipant with permission necessary
    /// to join the DDS Domain shall be allowed to write the
    /// BuiltinLoggingTopic but in order to read [it] the DomainParticipant
    /// needs to have a grant for the BuiltinLoggingTopic in its permissions
    /// document." The rule is built in and holds for both names of the topic,
    /// whatever the governance says.
    #[test]
    fn a_log_topic_is_written_without_a_grant_and_read_only_with_one() {
        let grant = |subscribe: &str| {
            zerodds_security_permissions::parse_permissions_xml(&format!(
                r#"<dds><permissions><grant name="g">
                     <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                     <allow_rule><domains><id>0</id></domains>
                       <publish><topics><topic>Other</topic></topics></publish>{subscribe}
                     </allow_rule>
                     <default>DENY</default>
                   </grant></permissions></dds>"#
            ))
            .expect("permissions")
        };
        let gate = ac_gate();
        gate.set_permissions(grant("")).expect("lock");
        for topic in [TOPIC_LOG_TOPIC_V2, TOPIC_LOG_TOPIC] {
            assert_eq!(
                gate.check_create_datawriter(&req(topic)).expect("lock"),
                EndpointAccess::Permit,
                "{topic}: written without a grant"
            );
            assert_eq!(
                gate.check_create_datareader(&req(topic)).expect("lock"),
                EndpointAccess::Deny,
                "{topic}: not read without one"
            );
        }
        gate.set_permissions(grant(&format!(
            "<subscribe><topics><topic>{TOPIC_LOG_TOPIC_V2}</topic><topic>{TOPIC_LOG_TOPIC}</topic></topics></subscribe>"
        )))
        .expect("lock");
        for topic in [TOPIC_LOG_TOPIC_V2, TOPIC_LOG_TOPIC] {
            assert_eq!(
                gate.check_create_datareader(&req(topic)).expect("lock"),
                EndpointAccess::Permit,
                "{topic}: read with a grant"
            );
        }
    }

    /// Armed with a publish-only grant, the gate permits the writer and
    /// denies the reader on the same topic.
    #[test]
    fn an_armed_gate_separates_publish_from_subscribe() {
        let gate = ac_gate();
        gate.set_permissions(
            zerodds_security_permissions::parse_permissions_xml(
                r#"<dds><permissions><grant name="w">
                     <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                     <allow_rule><domains><id>0</id></domains>
                       <publish><topics><topic>PD_RWA_EM_ED</topic></topics></publish>
                     </allow_rule>
                     <default>DENY</default>
                   </grant></permissions></dds>"#,
            )
            .expect("permissions"),
        )
        .expect("lock");
        assert!(gate.has_permissions().expect("lock"));
        assert_eq!(
            gate.check_create_datawriter(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_create_datareader(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Deny
        );
    }

    /// DDS-Security 1.2 §7.1.1.4: a relay reader needs `<relay>` on an
    /// access-controlled topic, a `<subscribe>` grant does not do; a peer
    /// without a grant forwards nothing there, and on the open topic every
    /// participant may relay.
    #[test]
    fn relaying_needs_the_relay_grant_on_an_access_controlled_topic() {
        let gate = ac_gate();
        assert_eq!(
            gate.check_create_relay_datareader(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::NoPermissions
        );
        assert_eq!(
            gate.check_create_relay_datareader(&req("OD_OA_OM_OD"))
                .expect("lock"),
            EndpointAccess::Permit
        );
        assert!(
            !gate
                .remote_may_relay([7; 12], &req("PD_RWA_EM_ED"))
                .expect("lock")
        );
        assert!(
            gate.remote_may_relay([7; 12], &req("OD_OA_OM_OD"))
                .expect("lock")
        );
        assert!(
            !gate
                .remote_may_relay([7; 12], &req("NoRuleForThis"))
                .expect("lock")
        );
        let grant = |action: &str| {
            zerodds_security_permissions::parse_permissions_xml(&alloc::format!(
                r#"<dds><permissions><grant name="t">
                     <subject_name>/C=DE/CN=T</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                     <allow_rule><domains><id>0</id></domains>
                       <{action}><topics><topic>PD_RWA_EM_ED</topic></topics></{action}>
                     </allow_rule>
                     <default>DENY</default>
                   </grant></permissions></dds>"#
            ))
            .expect("permissions")
        };
        gate.set_permissions(grant("subscribe")).expect("lock");
        assert_eq!(
            gate.check_create_relay_datareader(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Deny
        );
        gate.set_permissions(grant("relay")).expect("lock");
        assert_eq!(
            gate.check_create_relay_datareader(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_create_datareader(&req("PD_RWA_EM_ED"))
                .expect("lock"),
            EndpointAccess::Deny,
            "relaying is not subscribing"
        );
    }

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
            r#"<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#,
        )
        .unwrap();
        let gate = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));
        let msg = fake_msg(b"x");
        assert_eq!(gate.transform_outbound(&msg).unwrap(), msg);
    }

    /// DDS-Security 1.2 §10.4.1.2.1, §10.5.3.1, §10.5.3.3.6: the governance
    /// kinds `*_WITH_ORIGIN_AUTHENTICATION` give the key material to a
    /// remote endpoint or participant a receiver-specific key, and the
    /// protected submessages and RTPS messages a MAC for it that the
    /// receiver checks.
    #[test]
    fn origin_authentication_follows_the_governance_kinds() {
        const GOV_ORIGIN: &str = r#"
<dds><domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
    <discovery_protection_kind>SIGN_WITH_ORIGIN_AUTHENTICATION</discovery_protection_kind>
    <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
    <rtps_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</rtps_protection_kind>
    <topic_access_rules>
      <topic_rule>
        <topic_expression>Origin*</topic_expression>
        <metadata_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</metadata_protection_kind>
        <data_protection_kind>NONE</data_protection_kind>
      </topic_rule>
      <topic_rule>
        <topic_expression>*</topic_expression>
        <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
        <data_protection_kind>NONE</data_protection_kind>
      </topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>
"#;
        let gate = || {
            SharedSecurityGate::new(
                0,
                parse_governance_xml(GOV_ORIGIN).unwrap(),
                Box::new(AesGcmCryptoPlugin::new()),
            )
        };
        let (alice, bob, carol) = (gate(), gate(), gate());
        assert!(alice.rtps_origin_authenticated().unwrap());
        assert!(alice.discovery_origin_authenticated().unwrap());
        assert!(!alice.liveliness_origin_authenticated().unwrap());
        assert!(alice.metadata_origin_authenticated("OriginTrack").unwrap());
        assert!(!alice.metadata_origin_authenticated("Chatter").unwrap());

        // Endpoint level: a writer of an OriginTrack, and two readers.
        let kinds = |topic: &str| alice.endpoint_protection_kinds(Some(topic), false).unwrap();
        let (meta, data) = kinds("OriginTrack");
        let writer = alice
            .register_local_endpoint_with_kinds(true, meta, data)
            .unwrap();
        let (bob_reader, carol_reader) = ([0xB1u8; 16], [0xC1u8; 16]);
        let token_bob = alice.create_endpoint_token_for(writer, bob_reader).unwrap();
        let token_carol = alice
            .create_endpoint_token_for(writer, carol_reader)
            .unwrap();
        assert_ne!(token_bob, token_carol, "a receiver-specific key each");
        let alice_prefix: PeerKey = [0xAA; 12];
        for (gate, token) in [(&bob, &token_bob), (&carol, &token_carol)] {
            gate.register_remote_by_guid(
                alice_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &alice.local_token().unwrap(),
            )
            .unwrap();
            gate.install_remote_endpoint_token(&alice_prefix, token)
                .unwrap();
        }
        let plain = fake_msg(b"[DATA:origin]");
        let wire = alice
            .encode_data_datawriter_by_handle(writer, &plain)
            .unwrap();
        assert_eq!(
            bob.decode_data_by_key_id(&alice_prefix, &wire).unwrap(),
            plain
        );
        assert_eq!(
            carol.decode_data_by_key_id(&alice_prefix, &wire).unwrap(),
            plain
        );
        // The endpoint of an ordinary topic carries no receiver-specific MAC.
        let (meta, data) = kinds("Chatter");
        let ordinary = alice
            .register_local_endpoint_with_kinds(true, meta, data)
            .unwrap();
        let ordinary_token = alice
            .create_endpoint_token_for(ordinary, bob_reader)
            .unwrap();
        assert_eq!(
            ordinary_token,
            alice.create_endpoint_token(ordinary).unwrap(),
            "no receiver-specific key"
        );

        // RTPS level: Alice gives Bob a token with a receiver-specific key
        // for him; a message of Alice opens for Bob, and an altered MAC does
        // not.
        let bob_prefix: PeerKey = [0xB2; 12];
        alice
            .register_remote_by_guid(
                bob_prefix,
                IdentityHandle(2),
                SharedSecretHandle(2),
                &bob.local_token().unwrap(),
            )
            .unwrap();
        let rtps_bob = gate();
        rtps_bob
            .register_remote_by_guid(
                alice_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &alice.local_token_for(&bob_prefix).unwrap(),
            )
            .unwrap();
        let message = fake_msg(b"[SRTPS:origin]");
        let secured = alice.transform_outbound(&message).unwrap();
        assert_eq!(
            rtps_bob
                .transform_inbound_from(&alice_prefix, &secured)
                .unwrap(),
            message
        );
        let mut forged = secured.clone();
        let last = forged.len() - 1;
        forged[last] ^= 1;
        assert!(
            rtps_bob
                .transform_inbound_from(&alice_prefix, &forged)
                .is_err()
        );
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

        // Bob knows Alice as a peer.
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        // Alice: local writer endpoint with its own key (NOT participant).
        let aw = alice.register_local_endpoint(true).unwrap();
        // Alice → Bob: the per-endpoint datawriter_crypto_token.
        let token = alice.create_endpoint_token(aw).unwrap();
        bob.install_remote_endpoint_token(&alice_prefix, &token)
            .unwrap();

        // Alice encodes a DATA submessage with the ENDPOINT key.
        let plain = fake_msg(b"[DATA:per-endpoint]");
        let wire = alice.encode_data_datawriter_by_handle(aw, &plain).unwrap();
        // Bob decodes it exclusively via the key_id in the CryptoHeader.
        let back = bob.decode_data_by_key_id(&alice_prefix, &wire).unwrap();
        assert_eq!(back, plain, "per-endpoint key round-trips via key_id");
    }

    /// §9.5.2.1.1: a `transformation_key_id` is chosen by its sender, and
    /// two peers may choose the same one. Carol's endpoint key installed
    /// under the key id of Alice's leaves Alice's in place: Alice's data
    /// still decodes. The second token replaced the first one's key.
    #[test]
    fn two_peers_with_the_same_key_id_keep_their_keys() {
        let gate = || {
            SharedSecurityGate::new(
                0,
                parse_governance_xml(GOV_RTPS).unwrap(),
                Box::new(AesGcmCryptoPlugin::new()),
            )
        };
        let (alice, carol, bob) = (gate(), gate(), gate());
        let (alice_prefix, carol_prefix): (PeerKey, PeerKey) = ([0xAA; 12], [0xCC; 12]);
        for (prefix, peer, id) in [(alice_prefix, &alice, 1), (carol_prefix, &carol, 2)] {
            bob.register_remote_by_guid(
                prefix,
                IdentityHandle(id),
                SharedSecretHandle(id),
                &peer.local_token().unwrap(),
            )
            .unwrap();
        }
        let aw = alice.register_local_endpoint(true).unwrap();
        let alice_token = alice.create_endpoint_token(aw).unwrap();
        let cw = carol.register_local_endpoint(true).unwrap();
        let mut carol_token = carol.create_endpoint_token(cw).unwrap();
        // The token: kind(4) | salt length(4) | salt | key id(4) | …
        let salt_len = u32::from_be_bytes(alice_token[4..8].try_into().unwrap()) as usize;
        let key_id = 8 + salt_len..12 + salt_len;
        carol_token[key_id.clone()].copy_from_slice(&alice_token[key_id]);

        bob.install_remote_endpoint_token(&alice_prefix, &alice_token)
            .unwrap();
        bob.install_remote_endpoint_token(&carol_prefix, &carol_token)
            .unwrap();
        let plain = fake_msg(b"[DATA]");
        let from_alice = alice.encode_data_datawriter_by_handle(aw, &plain).unwrap();
        assert_eq!(
            bob.decode_data_by_key_id(&alice_prefix, &from_alice)
                .unwrap(),
            plain,
            "Carol's token with Alice's key id replaced Alice's key"
        );
    }

    /// DDS-Security 1.2 §10.5.3.3.1 and §9.5.2.1.1: a submessage decodes
    /// only with the keys of the participant it comes from. A submessage
    /// protected with Alice's endpoint key and arriving under Carol's prefix
    /// is refused, and Carol's own key decodes Carol's. A key id was looked
    /// up among the keys of every peer.
    #[test]
    fn a_key_of_another_peer_does_not_decode() {
        let gate = || {
            SharedSecurityGate::new(
                0,
                parse_governance_xml(GOV_RTPS).unwrap(),
                Box::new(AesGcmCryptoPlugin::new()),
            )
        };
        let (alice, carol, bob) = (gate(), gate(), gate());
        let (alice_prefix, carol_prefix): (PeerKey, PeerKey) = ([0xAA; 12], [0xCC; 12]);
        for (prefix, peer, id) in [(alice_prefix, &alice, 1), (carol_prefix, &carol, 2)] {
            bob.register_remote_by_guid(
                prefix,
                IdentityHandle(id),
                SharedSecretHandle(id),
                &peer.local_token().unwrap(),
            )
            .unwrap();
        }
        let aw = alice.register_local_endpoint(true).unwrap();
        bob.install_remote_endpoint_token(&alice_prefix, &alice.create_endpoint_token(aw).unwrap())
            .unwrap();
        let cw = carol.register_local_endpoint(true).unwrap();
        bob.install_remote_endpoint_token(&carol_prefix, &carol.create_endpoint_token(cw).unwrap())
            .unwrap();

        let plain = fake_msg(b"[DATA]");
        let from_alice = alice.encode_data_datawriter_by_handle(aw, &plain).unwrap();
        assert!(
            bob.decode_data_by_key_id(&carol_prefix, &from_alice)
                .is_err(),
            "Alice's key decoded what arrived from Carol"
        );
        assert_eq!(
            bob.decode_data_by_key_id(&alice_prefix, &from_alice)
                .unwrap(),
            plain
        );
        let from_carol = carol.encode_data_datawriter_by_handle(cw, &plain).unwrap();
        assert_eq!(
            bob.decode_data_by_key_id(&carol_prefix, &from_carol)
                .unwrap(),
            plain
        );
        // An endpoint token of a peer that is not registered is refused.
        assert!(
            bob.install_remote_endpoint_token(
                &[0xDD; 12],
                &alice.create_endpoint_token(aw).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn topic_discovery_protected_reads_topic_rule_flag() {
        // §9.4.2.4: the endpoint bit IS_DISCOVERY_PROTECTED comes from the
        // TOPIC rule `enable_discovery_protection` (boolean) — NOT from the
        // domain-wide discovery_protection_kind. cyclone sets it for
        // enable_discovery_protection=true; ZeroDDS must announce the same mask
        // (otherwise "security_attributes mismatch").
        const GOV_DISC: &str = r#"
<dds><domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
    <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
    <topic_access_rules><topic_rule>
      <topic_expression>*</topic_expression>
      <enable_discovery_protection>true</enable_discovery_protection>
    </topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>
"#;
        let on = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_DISC).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(
            on.topic_discovery_protected("Chatter").unwrap(),
            "enable_discovery_protection=true ⟹ endpoint IS_DISCOVERY_PROTECTED"
        );
        // GOV_RTPS has NO enable_discovery_protection → false.
        let off = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!off.topic_discovery_protected("Chatter").unwrap());
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
<dds><domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
    <rtps_protection_kind>NONE</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>
"#;
    /// No RTPS protection, unauthenticated participants allowed: the one
    /// form §9.4.2.4 Table 33 admits for allow_unauthenticated TRUE.
    const GOV_OPEN_ALLOW_UNAUTH: &str = r#"
<dds><domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains><enable_join_access_control>false</enable_join_access_control>
    <allow_unauthenticated_participants>TRUE</allow_unauthenticated_participants>
    <rtps_protection_kind>NONE</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>
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
            parse_governance_xml(GOV_OPEN_ALLOW_UNAUTH).unwrap(),
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

    /// DDS-Security 1.2 §10.4.1.2.6.6, §10.4.1.2.6.7: the protection kinds
    /// of a local endpoint are those of the rule of its topic; a secure
    /// builtin endpoint has the discovery or the liveliness kind of the
    /// domain and no payload protection (§10.4.1.2.5). Every endpoint had
    /// the kinds of the rule for the empty topic name.
    #[test]
    fn the_protection_kinds_of_an_endpoint_come_from_its_topic_rule() {
        const GOV: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><discovery_protection_kind>ENCRYPT</discovery_protection_kind><liveliness_protection_kind>SIGN</liveliness_protection_kind><rtps_protection_kind>NONE</rtps_protection_kind><topic_access_rules><topic_rule><topic_expression>Secure*</topic_expression><metadata_protection_kind>ENCRYPT</metadata_protection_kind><data_protection_kind>ENCRYPT</data_protection_kind></topic_rule><topic_rule><topic_expression>*</topic_expression><metadata_protection_kind>SIGN</metadata_protection_kind><data_protection_kind>SIGN</data_protection_kind></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert_eq!(
            gate.endpoint_protection_kinds(Some("SecureTrack"), false)
                .unwrap(),
            (ProtectionKind::Encrypt, ProtectionKind::Encrypt)
        );
        assert_eq!(
            gate.endpoint_protection_kinds(Some("Chatter"), false)
                .unwrap(),
            (ProtectionKind::Sign, ProtectionKind::Sign)
        );
        assert_eq!(
            gate.endpoint_protection_kinds(None, false).unwrap(),
            (ProtectionKind::Encrypt, ProtectionKind::None),
            "secure discovery endpoints"
        );
        assert_eq!(
            gate.endpoint_protection_kinds(None, true).unwrap(),
            (ProtectionKind::Sign, ProtectionKind::None),
            "secure WLP"
        );
        assert!(
            gate.register_local_endpoint_with_kinds(
                true,
                ProtectionKind::Sign,
                ProtectionKind::Encrypt
            )
            .is_ok()
        );
    }

    /// DDS-Security 1.2 §10.4.1.2.1, §10.5.3.1 Table 78: the Basic Protection
    /// Kind of an endpoint decides GMAC (SIGN) or GCM (ENCRYPT) of its key
    /// material, the width the `symmetric_cipher_algorithm`; the
    /// `required_mask` of the plugin's `adjusted_algorithm_info` is the bit of
    /// that width (Table 22).
    #[test]
    fn the_basic_protection_kind_of_an_endpoint_decides_gmac_or_gcm() {
        use zerodds_rtps::security_algo_info::symmetric_bit;
        const GOV: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        // (metadata kind, data kind, cipher, transformation_kind id of the
        // submessage key, required_mask)
        let cases = [
            (
                ProtectionKind::Sign,
                ProtectionKind::Sign,
                None,
                3,
                symmetric_bit::AES256,
            ),
            (
                ProtectionKind::Encrypt,
                ProtectionKind::Encrypt,
                None,
                4,
                symmetric_bit::AES256,
            ),
            (
                ProtectionKind::Sign,
                ProtectionKind::Sign,
                Some("AES128+GCM"),
                1,
                symmetric_bit::AES128,
            ),
            (
                ProtectionKind::Encrypt,
                ProtectionKind::Encrypt,
                Some("AES128+GCM"),
                2,
                symmetric_bit::AES128,
            ),
            (
                ProtectionKind::Encrypt,
                ProtectionKind::None,
                Some("AES128-GMAC"),
                2,
                symmetric_bit::AES128,
            ),
            (
                ProtectionKind::None,
                ProtectionKind::Sign,
                None,
                3,
                symmetric_bit::AES256,
            ),
        ];
        for (is_writer, (meta, data, cipher, id, mask)) in
            cases.iter().enumerate().map(|(i, c)| (i % 2 == 0, *c))
        {
            let h = gate
                .register_local_endpoint_with_properties(is_writer, meta, data, cipher)
                .unwrap();
            assert_eq!(
                gate.create_endpoint_token(h).unwrap()[3],
                id,
                "{meta:?}/{data:?}/{cipher:?}"
            );
            assert_eq!(gate.endpoint_required_mask(h), Some(mask));
        }
        // A cipher outside Table 22 is refused.
        assert!(
            gate.register_local_endpoint_with_properties(
                true,
                ProtectionKind::Sign,
                ProtectionKind::Sign,
                Some("ROT13")
            )
            .is_err()
        );
    }

    /// DDSI-RTPS 2.5 §8.3.6.3: a message of a full header and no
    /// submessage is valid, and has nothing to protect; it was refused as
    /// malformed, also on a domain with rtps_protection.
    #[test]
    fn classify_inbound_takes_a_header_without_submessages() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let mut header = b"RTPS\x02\x05\x01\x0f".to_vec();
        header.extend_from_slice(&[0x44; 12]);
        assert_eq!(
            gate.classify_inbound(&header, &NetInterface::Wan),
            InboundVerdict::Accept(header.clone())
        );
        assert_eq!(
            gate.classify_inbound(&header[..19], &NetInterface::Wan),
            InboundVerdict::Malformed
        );
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

    /// §9.4.2.4 Table 33: a domain with RTPS protection has
    /// allow_unauthenticated_participants FALSE; a plaintext message from a
    /// legacy peer is blocked. On a domain without RTPS protection it is
    /// taken.
    #[test]
    fn classify_inbound_plain_from_a_legacy_peer_follows_the_rtps_protection() {
        let open = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_OPEN_ALLOW_UNAUTH).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[legacy]");
        assert!(matches!(
            open.classify_inbound(&msg, &NetInterface::Wan),
            InboundVerdict::Accept(_)
        ));
        let protected = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert_eq!(
            protected.classify_inbound(&msg, &NetInterface::Wan),
            InboundVerdict::LegacyBlocked
        );
    }

    /// §9.4.2.4 Table 33 makes no exception for loopback: on an
    /// RTPS-protected domain plaintext over loopback is blocked unless
    /// `org.zerodds.sec.plaintext_loopback` allows it. It was taken always.
    #[test]
    fn classify_inbound_plain_on_loopback_needs_the_setting() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[loopback-plain]");
        for iface in [NetInterface::Loopback, NetInterface::LocalHost] {
            assert_eq!(
                gate.classify_inbound(&msg, &iface),
                InboundVerdict::LegacyBlocked
            );
        }
        gate.set_plaintext_loopback(true).unwrap();
        match gate.classify_inbound(&msg, &NetInterface::Loopback) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Accept with the setting, got {other:?}"),
        }
        assert_eq!(
            gate.classify_inbound(&msg, &NetInterface::Wan),
            InboundVerdict::LegacyBlocked,
            "the setting is for loopback only"
        );
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
            .encode_kx_datawriter_for(&bob_prefix, token_blob)
            .unwrap();
        let back = bob.decode_kx_datawriter_from(&alice_prefix, &wire).unwrap();
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

    // ------------------------------------------------------------------
    // The gate decides through the AccessControl plugin (§9.4.2.9).
    // ------------------------------------------------------------------

    use zerodds_security::access_control::{
        AccessControlListener, PermissionsHandle, RemoteReaderDecision, TopicSecurityConfig,
    };
    use zerodds_security::mock::MockAccessControlPlugin;

    /// A gate with the mock plugin installed; `decide` answers every check.
    #[allow(clippy::type_complexity)]
    fn spied_gate(
        decide: impl Fn(&zerodds_security::mock::MockAccessCall) -> bool + Send + Sync + 'static,
        relay_only: bool,
    ) -> (
        SharedSecurityGate,
        Arc<Mutex<Vec<zerodds_security::mock::MockAccessCall>>>,
        Arc<dyn Fn() -> Option<Arc<dyn AccessControlListener>> + Send + Sync>,
    ) {
        let gate = ac_gate();
        let (mut plugin, calls) = MockAccessControlPlugin::deciding(decide);
        if relay_only {
            plugin = plugin.relaying_refused_readers();
        }
        // The listener the gate sets on the plugin is kept for the test.
        let slot: Arc<Mutex<Option<Arc<dyn AccessControlListener>>>> = Arc::new(Mutex::new(None));
        let slot_for_reader = Arc::clone(&slot);
        let wrapper = ListenerTap {
            inner: plugin,
            slot,
        };
        gate.set_access_control(Box::new(wrapper), PermissionsHandle(7))
            .expect("lock");
        (
            gate,
            calls,
            Arc::new(move || slot_for_reader.lock().unwrap().clone()),
        )
    }

    /// The mock plugin, remembering the listener the gate sets on it.
    struct ListenerTap {
        inner: MockAccessControlPlugin,
        slot: Arc<Mutex<Option<Arc<dyn AccessControlListener>>>>,
    }

    macro_rules! delegate {
        ($(fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {
            $(fn $name(&self $(, $arg: $ty)*) -> $ret {
                self.inner.$name($($arg),*)
            })*
        };
    }

    macro_rules! delegate_mut {
        ($(fn $name:ident(&mut self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {
            $(fn $name(&mut self $(, $arg: $ty)*) -> $ret {
                self.inner.$name($($arg),*)
            })*
        };
    }

    impl zerodds_security::access_control::AccessControlPlugin for ListenerTap {
        delegate! {
            fn check_create_participant(&self, permissions: PermissionsHandle, participant: &zerodds_security::access_control::ParticipantRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_create_datawriter(&self, permissions: PermissionsHandle, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_create_datareader(&self, permissions: PermissionsHandle, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_create_topic(&self, permissions: PermissionsHandle, request: &zerodds_security::access_control::TopicRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_local_datawriter_register_instance(&self, permissions: PermissionsHandle, writer: [u8; 16], key: &[u8]) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_local_datawriter_dispose_instance(&self, permissions: PermissionsHandle, writer: [u8; 16], key: &[u8]) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_remote_participant(&self, permissions: PermissionsHandle, participant: &zerodds_security::access_control::ParticipantRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_remote_datawriter(&self, permissions: PermissionsHandle, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_remote_datareader(&self, permissions: PermissionsHandle, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<RemoteReaderDecision>;
            fn check_remote_topic(&self, permissions: PermissionsHandle, request: &zerodds_security::access_control::TopicRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_local_datawriter_match(&self, writer_permissions: PermissionsHandle, reader_permissions: PermissionsHandle, publication: &EndpointRequest<'_>, subscription: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_local_datareader_match(&self, writer_permissions: PermissionsHandle, reader_permissions: PermissionsHandle, subscription: &EndpointRequest<'_>, publication: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_remote_datawriter_register_instance(&self, permissions: PermissionsHandle, reader: [u8; 16], publication: [u8; 16], key: &[u8]) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_remote_datawriter_dispose_instance(&self, permissions: PermissionsHandle, reader: [u8; 16], publication: [u8; 16], key: &[u8]) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn check_relay(&self, permissions: PermissionsHandle, role: zerodds_security::access_control::RelayRole, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::AccessDecision>;
            fn plugin_class_id(&self) -> &str;
        }

        delegate_mut! {
            fn validate_local_permissions(&mut self, identity: IdentityHandle, participant: &zerodds_security::access_control::ParticipantRequest<'_>) -> zerodds_security::error::SecurityResult<PermissionsHandle>;
            fn validate_remote_permissions(&mut self, local_identity: IdentityHandle, remote_identity: IdentityHandle, remote_permissions_token: &zerodds_security::token::PermissionsToken, remote_credential_token: &DataHolder) -> zerodds_security::error::SecurityResult<PermissionsHandle>;
            fn get_permissions_token(&mut self, handle: PermissionsHandle) -> zerodds_security::error::SecurityResult<zerodds_security::token::PermissionsToken>;
            fn get_permissions_credential_token(&mut self, handle: PermissionsHandle) -> zerodds_security::error::SecurityResult<zerodds_security::token::PermissionsCredentialToken>;
            fn return_permissions_token(&mut self, token: zerodds_security::token::PermissionsToken) -> zerodds_security::error::SecurityResult<()>;
            fn return_permissions_credential_token(&mut self, token: zerodds_security::token::PermissionsCredentialToken) -> zerodds_security::error::SecurityResult<()>;
            fn get_participant_security_config(&mut self, permissions: PermissionsHandle) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::ParticipantSecurityConfig>;
            fn get_topic_security_config(&mut self, permissions: PermissionsHandle, topic_name: &str) -> zerodds_security::error::SecurityResult<TopicSecurityConfig>;
            fn get_datawriter_security_config(&mut self, permissions: PermissionsHandle, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::EndpointSecurityConfig>;
            fn get_datareader_security_config(&mut self, permissions: PermissionsHandle, request: &EndpointRequest<'_>) -> zerodds_security::error::SecurityResult<zerodds_security::access_control::EndpointSecurityConfig>;
            fn return_participant_security_config(&mut self, config: zerodds_security::access_control::ParticipantSecurityConfig) -> zerodds_security::error::SecurityResult<()>;
            fn return_topic_security_config(&mut self, config: TopicSecurityConfig) -> zerodds_security::error::SecurityResult<()>;
            fn return_datawriter_security_config(&mut self, config: zerodds_security::access_control::EndpointSecurityConfig) -> zerodds_security::error::SecurityResult<()>;
            fn return_datareader_security_config(&mut self, config: zerodds_security::access_control::EndpointSecurityConfig) -> zerodds_security::error::SecurityResult<()>;
        }

        fn set_listener(
            &mut self,
            listener: Arc<dyn AccessControlListener>,
        ) -> zerodds_security::error::SecurityResult<()> {
            *self.slot.lock().unwrap() = Some(Arc::clone(&listener));
            self.inner.set_listener(listener)
        }
    }

    /// DDS-Security 1.2 §9.4.2.9 and §2.3 (the plugin model): every access
    /// decision of the gate is the answer of the AccessControl plugin
    /// installed in it — with the governance of the gate saying the opposite.
    #[test]
    fn a_plugin_of_the_application_decides_every_check() {
        // PD_RWA_EM_ED is read and write controlled by the governance; the
        // plugin permits it all, though no permissions document is armed.
        let (gate, calls, _) = spied_gate(|_| true, false);
        assert!(gate.has_permissions().expect("lock"));
        assert!(gate.application_access_control().expect("lock"));
        let topic = "PD_RWA_EM_ED";
        let peer = [3u8; 12];
        assert_eq!(
            gate.check_create_datawriter(&req(topic)).expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_create_datareader(&req(topic)).expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_create_relay_datareader(&req(topic))
                .expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_create_topic(topic, &[]).expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_remote_datawriter(peer, &req(topic))
                .expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_remote_datareader(peer, &req(topic))
                .expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_remote_topic(peer, topic, &[]).expect("lock"),
            EndpointAccess::Permit
        );
        assert!(gate.remote_may_relay(peer, &req(topic)).expect("lock"));
        assert!(
            gate.check_create_participant(&PropertyList::new())
                .expect("lock")
        );
        assert!(gate.check_remote_participant(peer).expect("lock"));
        for dispose in [false, true] {
            assert!(
                gate.check_local_datawriter_instance([1; 16], b"k", dispose)
                    .expect("lock")
            );
            assert!(
                gate.check_remote_datawriter_instance(peer, [1; 16], [2; 16], b"k", dispose)
                    .expect("lock")
            );
        }
        assert!(
            gate.check_match(true, Some(peer), &req(topic), &req(topic))
                .expect("lock")
        );
        assert!(
            gate.check_match(false, None, &req(topic), &req(topic))
                .expect("lock")
        );
        // The configs and tokens are the plugin's, and the gate gives each
        // back (§9.4.2.9.20 to §9.4.2.9.29).
        gate.participant_security_config().expect("lock");
        assert_eq!(
            gate.topic_security_config(topic).expect("lock"),
            TopicSecurityConfig::default()
        );
        gate.endpoint_security_config(false, &req(topic))
            .expect("lock");
        gate.endpoint_security_config(true, &req(topic))
            .expect("lock");
        let token = gate.local_permissions_token().expect("lock");
        assert_eq!(
            DataHolder::from_cdr_le(&token).expect("cdr").class_id,
            "DDS:Access:Mock:1.0"
        );
        gate.local_permissions_credential_token().expect("lock");
        let operations: Vec<&str> = calls.lock().unwrap().iter().map(|c| c.operation).collect();
        for operation in [
            "check_create_datawriter",
            "check_create_datareader",
            "check_relay",
            "check_create_topic",
            "check_remote_datawriter",
            "check_remote_datareader",
            "check_remote_topic",
            "check_create_participant",
            "check_remote_participant",
            "check_local_datawriter_register_instance",
            "check_local_datawriter_dispose_instance",
            "check_remote_datawriter_register_instance",
            "check_remote_datawriter_dispose_instance",
            "check_local_datawriter_match",
            "check_local_datareader_match",
            "get_participant_security_config",
            "get_topic_security_config",
            "return_topic_security_config",
            "get_datawriter_security_config",
            "return_datawriter_security_config",
            "get_datareader_security_config",
            "return_datareader_security_config",
            "get_permissions_token",
            "return_permissions_token",
            "get_permissions_credential_token",
            "return_permissions_credential_token",
        ] {
            assert!(
                operations.contains(&operation),
                "{operation} was not asked: {operations:?}"
            );
        }
    }

    /// The same plugin refusing refuses everything, on the topic the
    /// governance leaves open as well.
    #[test]
    fn a_refusing_plugin_of_the_application_refuses_every_check() {
        let (gate, _, _) = spied_gate(|_| false, true);
        let peer = [3u8; 12];
        for topic in ["PD_RWA_EM_ED", "OD_OA_OM_OD"] {
            assert_eq!(
                gate.check_create_datawriter(&req(topic)).expect("lock"),
                EndpointAccess::Deny,
                "{topic}"
            );
            assert_eq!(
                gate.check_remote_datawriter(peer, &req(topic))
                    .expect("lock"),
                EndpointAccess::Deny,
                "{topic}"
            );
            // The plugin's relay-only answer reaches the runtime as such.
            assert_eq!(
                gate.check_remote_datareader(peer, &req(topic))
                    .expect("lock"),
                EndpointAccess::RelayOnly,
                "{topic}"
            );
        }
        assert!(
            !gate
                .remote_may_relay(peer, &req("OD_OA_OM_OD"))
                .expect("lock")
        );
        assert!(!gate.check_remote_participant(peer).expect("lock"));
        assert!(
            !gate
                .check_local_datawriter_instance([1; 16], b"k", false)
                .expect("lock")
        );
    }

    /// The request of the runtime reaches the plugin whole: partitions, data
    /// tags and QoS entries (§9.4.2.9.4 `partition`, `data_tag`, `qos`), and
    /// the domain of the gate.
    #[test]
    fn the_plugin_gets_partitions_data_tags_and_qos() {
        let (gate, calls, _) = spied_gate(
            |call| call.data_tags.iter().all(|(_, value)| value != "forbidden"),
            false,
        );
        let tags = [("class", "secret")];
        let qos = [("reliability", "RELIABLE"), ("property.x", "y")];
        let request = EndpointRequest::new(99, "OD_OA_OM_OD")
            .in_partitions(&["P1", "P2"])
            .with_data_tags(&tags)
            .with_qos(&qos);
        assert_eq!(
            gate.check_create_datawriter(&request).expect("lock"),
            EndpointAccess::Permit
        );
        let call = calls
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.operation == "check_create_datawriter")
            .cloned()
            .expect("asked");
        assert_eq!(call.topic.as_deref(), Some("OD_OA_OM_OD"));
        assert_eq!(call.partitions, ["P1", "P2"]);
        assert_eq!(
            call.data_tags,
            [("class".to_string(), "secret".to_string())]
        );
        assert_eq!(call.qos.len(), 2);
        assert_eq!(call.domain_id, Some(0), "the domain is the gate's");
        let forbidden = [("class", "forbidden")];
        assert_eq!(
            gate.check_create_datareader(&request.with_data_tags(&forbidden))
                .expect("lock"),
            EndpointAccess::Deny
        );
    }

    /// §9.4.2.9.2: a remote participant's permissions come from the
    /// plugin's `validate_remote_permissions`, and the handle is the one the
    /// plugin gets for that peer's checks. §9.4.2.10: the plugin revokes it,
    /// the gate hands the peer to the runtime once.
    #[test]
    fn a_revoked_remote_handle_is_handed_to_the_runtime_once() {
        let (gate, _, listener) = spied_gate(|_| true, false);
        let peer = [4u8; 12];
        let credential = DataHolder::new("DDS:Auth:PKI-DH:1.0");
        assert_eq!(
            gate.set_remote_permissions(
                peer,
                IdentityHandle(5),
                Some("DDS:Access:Mock:1.2"),
                &credential
            )
            .expect("lock"),
            RemotePermissions::Granted
        );
        // A token of another plugin class is refused before the plugin.
        assert_eq!(
            gate.set_remote_permissions(
                [5u8; 12],
                IdentityHandle(6),
                Some("DDS:Access:Permissions:1.2"),
                &credential
            )
            .expect("lock"),
            RemotePermissions::IncompatibleToken
        );
        assert_eq!(
            gate.set_remote_permissions([6u8; 12], IdentityHandle(6), None, &credential)
                .expect("lock"),
            RemotePermissions::IncompatibleToken
        );
        assert_eq!(
            gate.take_revoked().expect("lock"),
            RevokedPermissions::default()
        );
        // The mock hands out 1 for the local... the local handle of the gate
        // is 7; the remote handle is the first the mock made: 1.
        let heard = listener().expect("the gate listens");
        assert!(heard.on_revoke_permissions("DDS:Access:Mock:1.0", PermissionsHandle(1)));
        assert!(heard.on_revoke_permissions("DDS:Access:Mock:1.0", PermissionsHandle(7)));
        assert_eq!(
            gate.take_revoked().expect("lock"),
            RevokedPermissions {
                peers: alloc::vec![peer],
                local: true
            }
        );
        assert_eq!(
            gate.take_revoked().expect("lock"),
            RevokedPermissions::default(),
            "handed over once"
        );
    }

    /// Without a plugin of the application the builtin plugin built from the
    /// governance and the permissions answers; the instance and match
    /// operations are not asked of it (Table 70: they are TRUE).
    #[test]
    fn the_builtin_plugin_is_not_asked_for_what_it_always_permits() {
        let gate = ac_gate();
        gate.set_permissions(
            zerodds_security_permissions::parse_permissions_xml(
                r#"<dds><permissions><grant name="w">
                     <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                     <allow_rule><domains><id>0</id></domains>
                       <publish><topics><topic>PD_RWA_EM_ED</topic></topics></publish>
                     </allow_rule>
                     <default>DENY</default>
                   </grant></permissions></dds>"#,
            )
            .expect("permissions"),
        )
        .expect("lock");
        assert!(!gate.application_access_control().expect("lock"));
        assert!(
            gate.check_local_datawriter_instance([1; 16], b"k", true)
                .expect("lock")
        );
        assert!(
            gate.check_match(true, None, &req("PD_RWA_EM_ED"), &req("PD_RWA_EM_ED"))
                .expect("lock")
        );
        // Table 70, check_create_topic: the grant publishes the topic.
        assert_eq!(
            gate.check_create_topic("PD_RWA_EM_ED", &[]).expect("lock"),
            EndpointAccess::Permit
        );
        assert_eq!(
            gate.check_create_topic("NoRuleForThis", &[]).expect("lock"),
            EndpointAccess::Deny
        );
    }
}
