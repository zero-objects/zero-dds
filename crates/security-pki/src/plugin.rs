// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `AuthenticationPlugin` impl based on X.509 / rustls-webpki.
//!
//! Spec: OMG DDS-Security 1.2 §10.3.2.5 + §10.3.4. Tokens are encoded via
//! the `DataHolder` wire format from [`crate::handshake_token`].
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`webpki::EndEntityCert::verify_signature` takes
//! `&dyn SignatureVerificationAlgorithm` — that is a 3rd-party API,
//! not a ZeroDDS construction. We cannot convert it to concrete
//! generics without forking webpki.)

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::backend::digest;
use crate::backend::rand::{SecureRandom, SystemRandom};
use crate::backend::signature;
use rustls_pki_types::CertificateDer;
use zerodds_security::authentication::{
    AlgorithmMasks, AuthStatusKind, AuthenticationExtensions, AuthenticationListener,
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle, IssuedTokens,
    LocalIdentityValidation, ParticipantSecurityConfig, PeerAlgorithmNames,
    RemoteIdentityValidation, SharedSecretHandle, SharedSecretProvider, ValidationResult,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;
use zerodds_security_keyexchange::{KeyExchange, KxSuite};

use crate::handshake_token::{
    self as ht, FinalBuildInput, ReplyBuildInput, RequestBuildInput, ct_eq,
};
use crate::identity::{CertKeyAlgo, IdentityConfig, IdentitySigner, ParsedIdentity, PkiError};
use crate::x509v1_compat::{
    CertAcceptance, CertPolicy, LOG_CATEGORY_X509V1_DEVIATION, PROP_ACCEPT_X509V1_IDENTITY,
    SPEC_CLAUSE_IDENTITY_CERTIFICATE, property_enables_deviation,
};

/// Property keys (spec §8.3.2.7 / implementation-defined). We follow
/// the FastDDS convention so that users can reuse existing XML
/// configs.
mod keys {
    /// PEM-encoded identity certificate (directly in the property value,
    /// **not** as a file path — so the plugin caller can decide
    /// whether to load from the filesystem or from a secret manager).
    pub const IDENTITY_CERT: &str = "dds.sec.auth.identity_certificate";
    /// PEM-encoded CA bundle.
    pub const IDENTITY_CA: &str = "dds.sec.auth.identity_ca";
    /// PEM-encoded PKCS8 private key (matching the identity cert).
    pub const IDENTITY_KEY: &str = "dds.sec.auth.private_key";
    /// Vendor setting, **not** a spec property: how the algorithms are named
    /// in the IdentityToken and the handshake, see
    /// [`crate::handshake_token::AlgorithmNaming`].
    /// Vendor extension, **not** a spec property: accept X.509 v1 identity
    /// certificates. See `crate::x509v1_compat`.
    pub use crate::x509v1_compat::PROP_ACCEPT_X509V1_IDENTITY as ACCEPT_X509V1_IDENTITY;
}

/// Maximum storage for "recently seen" initiator challenges
/// (replay-detection cache per replier). 1024 entries × 32 bytes = 32 KiB.
const REPLAY_CACHE_CAP: usize = 1024;

/// The spelling of the algorithm names `c.dsign_algo` and `c.kagree_algo` an
/// initiator sends (DDS-Security 1.2 §8.2 Table 25, §8.3 Table 26).
///
/// Version 1.2 names the algorithms `ECDSA+P256+SHA256`,
/// `RSASSA-PSS-MGF1SHA256+2048+SHA256`, `ECDHE-CEUM+P256` and
/// `DHE+MODP-2048-256`, and marks the names of 1.1 (`ECDSA-SHA256`,
/// `RSASSA-PSS-SHA256`, `ECDH+prime256v1-CEUM`, `DH+MODP-2048-256`) "deprecated
/// in v 1.2". Both spellings are taken on receipt. The specification does not
/// make the sent spelling depend on a version the peer announced (the
/// handshake tokens keep the class `DDS:Auth:PKI-DH:1.0+...` in 1.2), so the
/// default is the spelling of Tables 25 and 26; the others are named settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlgorithmNames {
    /// The names of Tables 25 and 26 to every peer. The default.
    #[default]
    V1_2,
    /// The names of 1.2 to a peer whose IdentityToken names version 1.2, those
    /// of 1.1 to a peer of version 1.0 (`org.zerodds.sec.algorithm_names=auto`).
    Auto,
    /// The deprecated names of 1.1 to every peer
    /// (`org.zerodds.sec.algorithm_names=1.1`): what Cyclone DDS, Fast DDS and
    /// OpenDDS read.
    V1_1,
}

impl AlgorithmNames {
    /// Whether the names of 1.2 go to a peer whose IdentityToken names
    /// `peer_version`.
    #[must_use]
    pub fn uses_v1_2(self, peer_version: ht::ClassVersion) -> bool {
        match self {
            Self::Auto => peer_version == ht::ClassVersion::V1_2,
            Self::V1_2 => true,
            Self::V1_1 => false,
        }
    }

    /// Reads the value of `org.zerodds.sec.algorithm_names`: `auto`, `1.2` or
    /// `1.1`.
    ///
    /// # Errors
    /// `InvalidConfiguration` for any other value: a setting nobody can read
    /// must not choose the names silently.
    pub fn parse(value: &str) -> SecurityResult<Self> {
        match value.trim() {
            "auto" => Ok(Self::Auto),
            "1.2" => Ok(Self::V1_2),
            "1.1" => Ok(Self::V1_1),
            other => Err(SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!(
                    "pki: org.zerodds.sec.algorithm_names = '{other}' is auto, 1.2 or 1.1"
                ),
            )),
        }
    }
}

/// PKI/X.509-based `AuthenticationPlugin`. Verifies identity
/// certs against a given trust anchor and runs a
/// 3-round PKI-DH handshake (spec §10.3.2.6-8 Tab.56/57/58) with the peer.
///
/// Wire (C3.1): three `DataHolder` tokens
/// `DDS:Auth:PKI-DH:1.2+AuthReq` / `+AuthReply` / `+AuthFinal`. Both
/// sides echo `cert_der` + `dh*` + `challenge*` + hash bindings.
/// The replier signs (kagree || ch1 || dh1 || ch2 || dh2) with the
/// identity private key; the initiator signs (kagree || ch2 || dh2 ||
/// ch1 || dh1).
pub struct PkiAuthenticationPlugin {
    next_handle: AtomicU64,
    identities: BTreeMap<IdentityHandle, ParsedIdentity>,
    /// Initiator side: handshakes not yet completed.
    pending_initiator: BTreeMap<HandshakeHandle, InitiatorState>,
    /// Replier side: between reply-send and final-receive.
    pending_replier: BTreeMap<HandshakeHandle, ReplierState>,
    /// The PKI-DH `class_id` version each remote identity announced in
    /// its IdentityToken; a handshake this side initiates speaks it.
    remote_versions: BTreeMap<IdentityHandle, ht::ClassVersion>,
    /// The GUID each remote identity was validated under
    /// (`validate_remote_identity`), the one its participant is discovered
    /// with. The `c.pdata` of its handshake must name it (§7.4.3).
    remote_guids: BTreeMap<IdentityHandle, [u8; 16]>,
    /// Per remote identity, the NONCE this side uses as its challenge
    /// toward it (challenge1 as initiator, challenge2 as replier), made by
    /// `validate_remote_identity` and announced in the AuthRequestMessageToken
    /// (DDS-Security 1.2 §10.3.2.4 Table 55: "The value shall match what will
    /// be sent on the challenge1 property of the HandshakeRequestMessageToken
    /// or the challenge2 property of the HandshakeReplyMessageToken").
    local_futures: BTreeMap<IdentityHandle, [u8; 32]>,
    /// Per remote identity, the `future_challenge` of the
    /// AuthRequestMessageToken it sent that caused the authentication to
    /// begin; its challenge1 or challenge2 must match it (Table 59).
    remote_futures: BTreeMap<IdentityHandle, [u8; 32]>,
    /// Completed handshakes → SharedSecret handle.
    handshake_to_secret: BTreeMap<HandshakeHandle, SharedSecretHandle>,
    /// Materialized SharedSecrets (32 bytes SHA256(raw_dh)).
    secrets: BTreeMap<SharedSecretHandle, Vec<u8>>,
    /// Per SharedSecret the `(challenge1, challenge2)` of the handshake
    /// (initiator/replier), for the VolatileSecure key derivation §9.5.3.5.
    secret_challenges: BTreeMap<SharedSecretHandle, ([u8; 32], [u8; 32])>,
    /// Per local identity handle: the set of all initiator challenges
    /// already seen (replay detection). Capped.
    replay_cache: BTreeMap<IdentityHandle, BTreeSet<[u8; 32]>>,
    /// FIFO order of the replay-cache entries for cap eviction.
    replay_order: BTreeMap<IdentityHandle, Vec<[u8; 32]>>,
    /// Preferred key agreement as initiator. **Default = `EcdhP256`**
    /// (OMG spec `ECDHE+P-256+SHA-256`, cross-vendor-readable). X25519 is only
    /// allowed as an explicitly set vendor extension, NEVER the default. The replier
    /// always follows the `c.kagree_algo` announced by the initiator.
    preferred_kx_suite: KxSuite,
    /// The spelling of `c.dsign_algo` and `c.kagree_algo` as initiator.
    algorithm_names: AlgorithmNames,
    /// The form of `dds.cert.sn` and `dds.ca.sn` in the IdentityToken.
    subject_name_form: zerodds_security::SubjectNameForm,
    /// Cross-vendor quirk (transient, per-peer): if `true`, the
    /// next `c.dsign_algo`/`c.kagree_algo` are emitted + hashed WITH a trailing `\0`.
    /// OpenDDS compares them via `sizeof` (incl. NUL); FastDDS
    /// (#3803) needs them WITHOUT. The discovery layer sets this via
    /// [`AuthenticationPlugin::set_algo_nul_terminate`] based on the peer
    /// VendorId BEFORE each `begin_handshake_request`/`begin_handshake_reply`.
    algo_nul: bool,
    /// The `message_auth.supported_mask` of the peer of the next handshake
    /// (transient, per-peer, set like `algo_nul` through
    /// [`AuthenticationPlugin::set_remote_message_auth_supported`]).
    remote_message_auth: Option<u32>,
    /// Local CMS-signed permissions document (`.p7s` bytes) for the
    /// `c.perm` property in the handshake (spec §9.3.2.5.1). Empty = no
    /// AccessControl permissions; a foreign vendor with active AccessControl
    /// (governance) then rejects the handshake. Set by the SecurityProfile.
    local_permissions: Vec<u8>,
    /// The Table 24 bit of the signature algorithm of the local permissions
    /// document (§7.3.11: `trust_chain.required_mask` holds the algorithms of
    /// the signed documents this participant sends); 0 when not set.
    local_permissions_signature_bit: u32,
    /// Certificate-validation policy for every identity this plugin
    /// validates, local and remote. [`CertPolicy::spec`] by default —
    /// DDS-Security 1.2 §10.3.1.3, X.509 v3 only.
    cert_policy: CertPolicy,
    /// Security-event logger (spec §8.6). `None` = none installed; the
    /// X.509 v1 deviation then falls back to this crate's own stderr sink
    /// rather than accepting a certificate nobody is told about.
    ///
    /// zerodds-lint: allow no_dyn_in_safe
    /// (the §8.6 `LoggingPlugin` SPI is a plugin slot — the sink type is
    /// chosen by the deployment, not by this crate.)
    logger: Option<alloc::sync::Arc<dyn zerodds_security::logging::LoggingPlugin>>,
    /// The `c.pdata` the tests of this module give with the handshake
    /// operations (`serialized_local_participant_data`).
    #[cfg(test)]
    local_pdata: Vec<u8>,
    /// The PermissionsCredentialToken and PermissionsToken of each local
    /// identity (`set_permissions_credential_and_token`, §9.3.2.11.12).
    local_permissions_by_identity: BTreeMap<IdentityHandle, LocalPermissions>,
    /// Per remote identity: the local identity it was validated for and the
    /// `ValidationResult_t` of `validate_remote_identity`, which fixes who
    /// begins the handshake (Table 59).
    remote_roles: BTreeMap<IdentityHandle, (IdentityHandle, ValidationResult)>,
    /// Per handshake: the remote identity it was begun with.
    handshake_remote: BTreeMap<HandshakeHandle, IdentityHandle>,
    /// The handshakes `begin_handshake_request`/`begin_handshake_reply`
    /// issued and `return_handshake_handle` did not take back.
    handshakes_issued: BTreeSet<HandshakeHandle>,
    /// Per completed handshake, the AuthenticatedPeerCredentialToken content
    /// (Table 54): `c.id` and `c.perm` as the peer sent them.
    peer_credentials: BTreeMap<HandshakeHandle, PeerCredential>,
    /// The tokens handed out by `get_identity_token`,
    /// `get_identity_status_token` and
    /// `get_authenticated_peer_credential_token` and not yet returned, with
    /// how often.
    tokens_out: IssuedTokens,
    /// `set_listener`.
    ///
    /// zerodds-lint: allow no_dyn_in_safe
    /// (the §9.3.2.12 `AuthenticationListener` is a plugin slot.)
    listener: Option<Arc<dyn AuthenticationListener>>,
    /// The signature and key establishment algorithms `set_participant_security_config`
    /// limited the plugin to, per local identity.
    algorithm_limits: BTreeMap<IdentityHandle, AlgorithmMasks>,
    /// The names the peer of the next handshake reads for the algorithms
    /// (transient, per peer, set like `algo_nul`), see
    /// [`AuthenticationExtensions::set_peer_algorithm_names`].
    peer_algorithm_names: PeerAlgorithmNames,
    /// The last IdentityStatusToken content per local identity, to tell the
    /// listener when it changed.
    status_seen: BTreeMap<IdentityHandle, Vec<u8>>,
    /// Local identity validations that run in the background because they
    /// ask an OCSP responder (`VALIDATION_PENDING_RETRY`, §9.3.2.11.2), per
    /// candidate GUID.
    pending_local: BTreeMap<[u8; 16], PendingLocal>,
}

/// A validation of a local identity running on its own thread.
struct PendingLocal {
    domain_id: u32,
    /// The result, once the thread has one.
    slot: Arc<std::sync::Mutex<Option<Result<ParsedIdentity, PkiError>>>>,
}

/// What `set_permissions_credential_and_token` stored.
#[derive(Clone)]
struct LocalPermissions {
    /// `dds.perm.cert` of the PermissionsCredentialToken (Table 64): the
    /// permissions document, which the handshake sends as `c.perm`.
    permissions_document: Vec<u8>,
    /// The PermissionsToken, as CDR bytes.
    permissions_token: Vec<u8>,
}

/// `c.id` and `c.perm` of an authenticated peer (Table 54).
#[derive(Clone)]
struct PeerCredential {
    c_id: Vec<u8>,
    c_perm: Vec<u8>,
}

/// The tokens `tokens_out` counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum TokenKind {
    Identity,
    IdentityStatus,
    PeerCredential,
}

/// `dds.perm.cert` of the PermissionsCredentialToken of the builtin
/// DDS:Access:Permissions plugin (DDS-Security 1.2 §10.4.2.1 Table 64).
pub const PERMISSIONS_CREDENTIAL_CERT: &str = "dds.perm.cert";

struct InitiatorState {
    /// Local identity handle (for cert/key lookup).
    local: IdentityHandle,
    /// The replier's remote identity handle.
    remote: IdentityHandle,
    /// Ephemeral DH keypair (consumed at the REPLY).
    kx: Option<KeyExchange>,
    /// Bytes of the local DH public.
    dh1: Vec<u8>,
    /// Locally generated challenge.
    challenge1: [u8; 32],
    /// Hash binding of the initiator tuple.
    hash_c1: [u8; 32],
    /// Permissions document (echo). Consumed by the AccessControl plugin in the
    /// permissions-bind step; held here only for symmetry
    /// with the replier state.
    #[allow(dead_code)]
    permissions: Vec<u8>,
    /// Pdata (echo).
    #[allow(dead_code)]
    pdata: Vec<u8>,
    /// Our own `c.kagree_algo` (what we sent in the REQUEST).
    kagree_algo: alloc::string::String,
    /// Our own `c.dsign_algo`. Echoed by the replier and checked for
    /// plausibility on reply decode (no match comparison
    /// here, because the initiator sends the replier algo).
    #[allow(dead_code)]
    dsign_algo: alloc::string::String,
    /// The scheme the final token is signed with, the one `dsign_algo` names.
    scheme: DsignScheme,
}

struct ReplierState {
    /// Local identity handle.
    local: IdentityHandle,
    /// Initiator challenge.
    challenge1: [u8; 32],
    /// Replier challenge.
    challenge2: [u8; 32],
    /// Initiator DH (echo).
    dh1: Vec<u8>,
    /// Replier DH.
    dh2: Vec<u8>,
    /// hash_c1 (echo).
    hash_c1: [u8; 32],
    /// hash_c2 (replier tuple).
    hash_c2: [u8; 32],
    /// SharedSecret already derived.
    secret_handle: SharedSecretHandle,
    /// Initiator cert DER (for the final-signature check).
    initiator_cert_der: Vec<u8>,
    /// The scheme the initiator's `c.dsign_algo` named, which its final
    /// signature is checked with.
    initiator_scheme: DsignScheme,
    /// The initiator's `c.id` and `c.perm`: its AuthenticatedPeerCredentialToken
    /// once the final proved its key.
    peer: PeerCredential,
}

/// The fields of an [`InitiatorState`] the checks of a reply read, copied
/// so that the state stays in place until the reply has passed them.
struct InitiatorChecks {
    local: IdentityHandle,
    remote: IdentityHandle,
    dh1: Vec<u8>,
    challenge1: [u8; 32],
    hash_c1: [u8; 32],
    kagree_algo: alloc::string::String,
}

impl Default for PkiAuthenticationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PkiAuthenticationPlugin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            identities: BTreeMap::new(),
            pending_initiator: BTreeMap::new(),
            pending_replier: BTreeMap::new(),
            remote_versions: BTreeMap::new(),
            remote_guids: BTreeMap::new(),
            local_futures: BTreeMap::new(),
            remote_futures: BTreeMap::new(),
            handshake_to_secret: BTreeMap::new(),
            secrets: BTreeMap::new(),
            secret_challenges: BTreeMap::new(),
            replay_cache: BTreeMap::new(),
            replay_order: BTreeMap::new(),
            // Spec default: cross-vendor-readable ECDHE+P-256+SHA-256.
            preferred_kx_suite: KxSuite::EcdhP256,
            algorithm_names: AlgorithmNames::default(),
            subject_name_form: zerodds_security::SubjectNameForm::default(),
            algo_nul: false,
            remote_message_auth: None,
            local_permissions: Vec::new(),
            local_permissions_signature_bit: 0,
            cert_policy: CertPolicy::spec(),
            logger: None,
            #[cfg(test)]
            local_pdata: Vec::new(),
            local_permissions_by_identity: BTreeMap::new(),
            remote_roles: BTreeMap::new(),
            handshake_remote: BTreeMap::new(),
            handshakes_issued: BTreeSet::new(),
            peer_credentials: BTreeMap::new(),
            tokens_out: IssuedTokens::default(),
            listener: None,
            algorithm_limits: BTreeMap::new(),
            peer_algorithm_names: PeerAlgorithmNames::Spec,
            status_seen: BTreeMap::new(),
            pending_local: BTreeMap::new(),
        }
    }

    /// Switches the X.509 v1 identity-certificate deviation on or off for
    /// every identity this plugin validates afterwards, local and remote.
    ///
    /// Off is the default and is DDS-Security 1.2 §10.3.1.3. On, the
    /// version check is the only check that is relaxed — see
    /// [`crate::x509v1_compat`] — and every accepted v1 certificate
    /// produces a security-log event.
    ///
    /// The property-driven equivalent is
    /// [`PROP_ACCEPT_X509V1_IDENTITY`] on the participant's
    /// `PropertyQosPolicy`, which
    /// [`AuthenticationPlugin::validate_local_identity`] reads.
    pub fn set_accept_x509v1_identity_certificates(&mut self, accept: bool) {
        self.cert_policy.accept_x509v1_identity_certificate = accept;
    }

    /// The subject name every identity certificate must carry, when the
    /// deployment names one. `None` (the default) is what both validation
    /// paths do without it: no subject expectation.
    pub fn set_expected_certificate_subject(&mut self, subject: Option<alloc::string::String>) {
        self.cert_policy.expected_subject = subject;
    }

    /// The certificate revocation lists of the identity CA, DER, against
    /// which the local and every remote identity certificate are checked
    /// (DDS-Security 1.2 §10.3.3 Table 59). To be set before the identity is
    /// validated.
    pub fn set_crls(&mut self, crls_der: Vec<Vec<u8>>) {
        self.cert_policy.crls_der = crls_der;
    }

    /// What happens when the revocation status of an identity certificate
    /// cannot be determined. To be set before the identity is validated.
    pub fn set_ocsp_policy(&mut self, policy: crate::ocsp::OcspPolicy) {
        self.cert_policy.ocsp = policy;
    }

    /// The certificate-validation policy in force.
    #[must_use]
    pub fn cert_policy(&self) -> &CertPolicy {
        &self.cert_policy
    }

    /// Installs the DDS-Security §8.6 security-event logger this plugin
    /// writes its deviation events to.
    ///
    /// zerodds-lint: allow no_dyn_in_safe
    /// (the §8.6 `LoggingPlugin` SPI is a plugin slot.)
    pub fn set_security_logger(
        &mut self,
        logger: alloc::sync::Arc<dyn zerodds_security::logging::LoggingPlugin>,
    ) {
        self.logger = Some(logger);
    }

    /// Writes the security-log event for an accepted X.509 v1 identity
    /// certificate: DDS-Security 1.2 §9.6.2.2.2 `log`, level `Warning`,
    /// category [`LOG_CATEGORY_X509V1_DEVIATION`], message naming the
    /// certificate's subject and the clause being left.
    ///
    /// A [`CertAcceptance::SpecV3`] writes nothing — the spec path is not
    /// an event.
    ///
    /// With no logger installed the event goes to this crate's own stderr
    /// sink (the same `StderrLoggingPlugin` the §8.6 wiring uses, not a
    /// `println!`). An accepted deviation that produced no record would be
    /// the one outcome worse than refusing the certificate.
    /// Tells the security log that a certificate passed although its
    /// revocation status could not be determined
    /// ([`zerodds_security::logging::LogLevel::Warning`]); nothing for the
    /// other outcomes.
    fn log_ocsp_outcome(
        &self,
        outcome: &crate::ocsp::OcspOutcome,
        participant_guid: [u8; 16],
        role: &str,
    ) {
        use zerodds_security::logging::{LogLevel, LoggingPlugin};
        let crate::ocsp::OcspOutcome::AcceptedUndetermined(why) = outcome else {
            return;
        };
        let message = alloc::format!(
            "accepted the {role} identity certificate without a revocation status ({why}): \
             org.zerodds.sec.auth.ocsp_unreachable is accept (DDS-Security 1.2 §10.3.3 Table 59 \
             asks for the check)"
        );
        match &self.logger {
            Some(logger) => logger.log(
                LogLevel::Warning,
                participant_guid,
                "ocsp.unreachable",
                &message,
            ),
            None => zerodds_security_logging::StderrLoggingPlugin::new().log(
                LogLevel::Warning,
                participant_guid,
                "ocsp.unreachable",
                &message,
            ),
        }
    }

    fn log_cert_acceptance(
        &self,
        acceptance: &CertAcceptance,
        participant_guid: [u8; 16],
        role: &str,
    ) {
        use zerodds_security::logging::{LogLevel, LoggingPlugin};
        let CertAcceptance::X509V1Deviation { subject } = acceptance else {
            return;
        };
        let message = alloc::format!(
            "accepted X.509 v1 {role} identity certificate, subject '{subject}': \
             {SPEC_CLAUSE_IDENTITY_CERTIFICATE} requires an X.509 v3 certificate, and this one was \
             accepted only because '{PROP_ACCEPT_X509V1_IDENTITY}' is set. \
             Signature, validity period, chain to the identity CA and subject \
             were checked; a v1 certificate carries no extensions, so key usage \
             was not."
        );
        match &self.logger {
            Some(logger) => logger.log(
                LogLevel::Warning,
                participant_guid,
                LOG_CATEGORY_X509V1_DEVIATION,
                &message,
            ),
            None => zerodds_security_logging::StderrLoggingPlugin::new().log(
                LogLevel::Warning,
                participant_guid,
                LOG_CATEGORY_X509V1_DEVIATION,
                &message,
            ),
        }
    }

    /// Sets the signature algorithm (object identifier, dotted form) of the
    /// local permissions document; its Table 24 bit joins
    /// `trust_chain.required_mask` (DDS-Security 1.2 §7.3.11).
    pub fn set_local_permissions_signature_algorithm(&mut self, oid: &str) {
        self.local_permissions_signature_bit = signature_bit_of_oid(oid);
    }

    /// Sets the local CMS-signed permissions document (`.p7s` bytes) that
    /// is sent along as `c.perm` in the HandshakeRequest/Reply. Required for
    /// cross-vendor interop with active AccessControl (FastDDS/Cyclone/RTI
    /// validate the permissions signature against the permissions_ca).
    pub fn set_local_permissions(&mut self, permissions_p7s: Vec<u8>) {
        self.local_permissions = permissions_p7s;
    }

    /// Sets the preferred key agreement as initiator. Only for a deliberate
    /// vendor-extension choice (e.g. `KxSuite::X25519` between pure ZeroDDS
    /// peers) — the default `EcdhP256` is the spec/cross-vendor choice and
    /// should NOT be changed for interop.
    pub fn set_preferred_kx_suite(&mut self, suite: KxSuite) {
        self.preferred_kx_suite = suite;
    }

    /// Sets the form of the subject names in the IdentityToken (`dds.cert.sn`,
    /// `dds.ca.sn`), see [`zerodds_security::SubjectNameForm`]. RFC 4514 by
    /// default (DDS-Security 1.2 §9.4.1.2.3).
    pub fn set_subject_name_form(&mut self, form: zerodds_security::SubjectNameForm) {
        self.subject_name_form = form;
    }

    /// Sets the spelling of `c.dsign_algo` and `c.kagree_algo` this side sends
    /// as initiator (see [`AlgorithmNames`]). The replier answers in the
    /// spelling of the request either way.
    pub fn set_algorithm_names(&mut self, names: AlgorithmNames) {
        self.algorithm_names = names;
        self.cert_policy.algorithm_naming = match names {
            AlgorithmNames::V1_2 => ht::AlgorithmNaming::Spec,
            AlgorithmNames::V1_1 => ht::AlgorithmNaming::Legacy,
            AlgorithmNames::Auto => ht::AlgorithmNaming::Auto,
        };
    }

    /// For OpenDDS peers (see [`Self::algo_nul`]) appends a `\0` to an
    /// algorithm string — consistent for the wire AND the hash (both use
    /// `.as_bytes()`). Otherwise unchanged.
    fn algo_str(&self, s: &str) -> alloc::string::String {
        if self.algo_nul {
            alloc::format!("{s}\0")
        } else {
            s.into()
        }
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Maps a `c.kagree_algo` string to the KX suite. `None` =
    /// unsupported (the replier then rejects). Any trailing `\0`
    /// (OpenDDS sends the algorithm strings NUL-terminated) is removed before
    /// the comparison — the KX suite is independent of it.
    /// The KX suite of a `c.kagree_algo`: the names of DDS-Security 1.2 §8.3
    /// Table 26 and their deprecated names of 1.1 alike. `X25519`, a vendor
    /// extension, only where this side runs it itself (`allow_x25519`); it
    /// was taken from any initiator.
    fn kx_suite_for_algo(algo: &str, allow_x25519: bool) -> Option<KxSuite> {
        let algo = algo.trim_end_matches('\0');
        if algo == ht::algo::ECDHE_P256_SHA256 || algo == ht::algo::ECDHE_CEUM_P256_V1_2 {
            Some(KxSuite::EcdhP256)
        } else if algo == ht::algo::ECDHE_CEUM_P384 {
            Some(KxSuite::EcdhP384)
        } else if algo == ht::algo::DH_MODP_2048 || algo == ht::algo::DHE_MODP_2048_256 {
            Some(KxSuite::DhModp2048_256)
        } else if algo == ht::algo::X25519 && allow_x25519 {
            Some(KxSuite::X25519)
        } else {
            None
        }
    }

    /// `c.kagree_algo` string for a KX suite: the name of Table 26, or with
    /// `legacy` the one of 1.1 where there is one.
    fn kagree_algo_str(suite: KxSuite, legacy: bool) -> &'static str {
        match (suite, legacy) {
            (KxSuite::EcdhP256, true) => ht::algo::ECDHE_P256_SHA256,
            (KxSuite::EcdhP256, false) => ht::algo::ECDHE_CEUM_P256_V1_2,
            (KxSuite::EcdhP384, _) => ht::algo::ECDHE_CEUM_P384,
            (KxSuite::DhModp2048_256, true) => ht::algo::DH_MODP_2048,
            (KxSuite::DhModp2048_256, false) => ht::algo::DHE_MODP_2048_256,
            (KxSuite::X25519, _) => ht::algo::X25519,
        }
    }

    /// Whether the handshake with a peer whose IdentityToken names `version`
    /// names the algorithms as 1.1 does: the setting
    /// `org.zerodds.sec.algorithm_names` says (`1.2` by default); `auto` gives
    /// the names of 1.1 to a peer of version 1.0 and to a vendor the discovery
    /// layer knows to read them.
    fn legacy_names(&self, version: ht::ClassVersion) -> bool {
        match self.algorithm_names {
            AlgorithmNames::V1_2 => false,
            AlgorithmNames::V1_1 => true,
            AlgorithmNames::Auto => {
                version != ht::ClassVersion::V1_2
                    || self.peer_algorithm_names == PeerAlgorithmNames::Legacy
            }
        }
    }

    /// Programmatic variant: pass `IdentityConfig` directly instead of
    /// a `PropertyList`. Useful for tests + native
    /// Rust callers.
    ///
    /// # Errors
    /// See [`PkiError`].
    pub fn validate_with_config(
        &mut self,
        cfg: IdentityConfig,
        participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle> {
        let parsed = ParsedIdentity::from_config(&cfg, self.cert_policy.clone(), participant_guid)
            .map_err(pki_to_security)?;
        self.log_cert_acceptance(&parsed.acceptance, participant_guid, "local");
        if let Some(why) = &parsed.ocsp_note {
            self.log_ocsp_outcome(
                &crate::ocsp::OcspOutcome::AcceptedUndetermined(why.clone()),
                participant_guid,
                "local",
            );
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, parsed);
        Ok(handle)
    }

    /// [`Self::validate_with_config`] with a token-backed
    /// [`IdentitySigner`] in place of the PKCS#8 private key.
    ///
    /// The certificate and CA are read and validated exactly as in
    /// `validate_with_config`; the private key stays on the token, and the
    /// handshake signs through the signer. `cfg.identity_key_pem` must be
    /// `None`.
    ///
    /// # Errors
    /// See [`PkiError`].
    pub fn validate_with_config_and_signer(
        &mut self,
        cfg: IdentityConfig,
        participant_guid: [u8; 16],
        signer: Arc<dyn IdentitySigner>,
    ) -> SecurityResult<IdentityHandle> {
        let parsed = ParsedIdentity::from_config_with_signer(
            &cfg,
            self.cert_policy.clone(),
            participant_guid,
            signer,
        )
        .map_err(pki_to_security)?;
        self.log_cert_acceptance(&parsed.acceptance, participant_guid, "local");
        if let Some(why) = &parsed.ocsp_note {
            self.log_ocsp_outcome(
                &crate::ocsp::OcspOutcome::AcceptedUndetermined(why.clone()),
                participant_guid,
                "local",
            );
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, parsed);
        Ok(handle)
    }

    /// Reads the raw 32-byte SharedSecret bytes. Primarily for tests;
    /// in the production path the `SharedSecretHandle` is passed through to the
    /// CryptoPlugin without the caller seeing the bytes.
    #[must_use]
    pub fn secret_bytes(&self, handle: SharedSecretHandle) -> Option<&[u8]> {
        self.secrets.get(&handle).map(Vec::as_slice)
    }

    fn store_secret(
        &mut self,
        _h: HandshakeHandle,
        bytes: Vec<u8>,
        challenge1: [u8; 32],
        challenge2: [u8; 32],
    ) -> SharedSecretHandle {
        let handle = SharedSecretHandle(self.next_id());
        self.secrets.insert(handle, bytes);
        self.secret_challenges
            .insert(handle, (challenge1, challenge2));
        handle
    }

    /// `validate_local_identity` for a caller that holds the identity
    /// material itself (DDS-Security 1.2 §9.3.2.11.2, Table 59): validates
    /// the identity certificate against the CA and returns the handle with
    /// the `adjusted_participant_guid`, the GUID whose prefix is bound to the
    /// SubjectName of the certificate and to the candidate GUID. It waits for
    /// the OCSP responder; the operation of the interface returns
    /// `VALIDATION_PENDING_RETRY` instead.
    ///
    /// `signer` is a token-backed [`IdentitySigner`] in place of the PKCS#8
    /// private key (`cfg.identity_key_pem` must then be `None`).
    ///
    /// # Errors
    /// See [`PkiError`].
    pub fn validate_identity(
        &mut self,
        cfg: IdentityConfig,
        domain_id: u32,
        candidate_participant_guid: [u8; 16],
        signer: Option<Arc<dyn IdentitySigner>>,
    ) -> SecurityResult<LocalIdentityValidation> {
        let parsed = match signer {
            Some(signer) => ParsedIdentity::from_config_with_signer(
                &cfg,
                self.cert_policy.clone(),
                candidate_participant_guid,
                signer,
            ),
            None => ParsedIdentity::from_config(
                &cfg,
                self.cert_policy.clone(),
                candidate_participant_guid,
            ),
        }
        .map_err(pki_to_security)?;
        self.register_local_identity(parsed, domain_id, candidate_participant_guid)
    }

    /// The end of the validation of a local identity: the GUID is adjusted,
    /// the identity gets its handle.
    fn register_local_identity(
        &mut self,
        mut parsed: ParsedIdentity,
        domain_id: u32,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        // Table 59: the first bit set, the next 47 bits from the SHA-256 hash
        // of the SubjectName, the following 48 bits from the hash of the
        // candidate GUID; the EntityId stays.
        let prefix = crate::guid_adjust::adjust_participant_guid_prefix(
            &candidate_participant_guid,
            &parsed.cert_der,
        )
        .map_err(pki_to_security)?;
        let mut adjusted_participant_guid = candidate_participant_guid;
        adjusted_participant_guid[..12].copy_from_slice(&prefix);
        parsed.participant_guid = adjusted_participant_guid;
        parsed.domain_id = Some(domain_id);
        self.log_cert_acceptance(&parsed.acceptance, adjusted_participant_guid, "local");
        if let Some(why) = &parsed.ocsp_note {
            self.log_ocsp_outcome(
                &crate::ocsp::OcspOutcome::AcceptedUndetermined(why.clone()),
                adjusted_participant_guid,
                "local",
            );
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, parsed);
        Ok(LocalIdentityValidation {
            result: ValidationResult::Ok,
            identity: handle,
            adjusted_participant_guid,
        })
    }

    /// `validate_local_identity` of the interface: a certificate that names an
    /// OCSP responder is validated on a thread of its own, so that the
    /// operation does not wait for the responder: it returns
    /// `VALIDATION_PENDING_RETRY` and is "called again after a configurable
    /// delay to check the status of verification" until it returns
    /// `VALIDATION_OK` or fails (§9.3.2.11.2: "This approach allows
    /// non-blocking interactions with services whose verification may
    /// require invoking remote services").
    fn validate_local_identity_nonblocking(
        &mut self,
        cfg: IdentityConfig,
        domain_id: u32,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        let pending_result = || LocalIdentityValidation {
            result: ValidationResult::PendingRetry,
            identity: IdentityHandle::NIL,
            // Meaningful with VALIDATION_OK only.
            adjusted_participant_guid: candidate_participant_guid,
        };
        if let Some(pending) = self.pending_local.get(&candidate_participant_guid) {
            let finished = pending
                .slot
                .lock()
                .map_err(|_| {
                    SecurityError::new(SecurityErrorKind::Internal, "pki: validation thread lost")
                })?
                .take();
            let Some(finished) = finished else {
                return Ok(pending_result());
            };
            let domain_id = pending.domain_id;
            self.pending_local.remove(&candidate_participant_guid);
            let parsed = finished.map_err(pki_to_security)?;
            return self.register_local_identity(parsed, domain_id, candidate_participant_guid);
        }
        let asks_a_responder = crate::identity::first_cert_der(&cfg.identity_cert_pem)
            .map(|der| !crate::ocsp::responder_urls(&der).is_empty())
            .unwrap_or(false);
        if !asks_a_responder {
            return self.validate_identity(cfg, domain_id, candidate_participant_guid, None);
        }
        let slot = Arc::new(std::sync::Mutex::new(None));
        let (thread_slot, policy) = (Arc::clone(&slot), self.cert_policy.clone());
        std::thread::Builder::new()
            .name("zerodds-pki-validate-local".into())
            .spawn(move || {
                let parsed = ParsedIdentity::from_config(&cfg, policy, candidate_participant_guid);
                // A poisoned slot cannot be: nothing else holds it for long.
                if let Ok(mut slot) = thread_slot.lock() {
                    *slot = Some(parsed);
                }
            })
            .map_err(|e| {
                SecurityError::new(
                    SecurityErrorKind::Internal,
                    alloc::format!("pki: cannot start the validation thread: {e}"),
                )
            })?;
        self.pending_local
            .insert(candidate_participant_guid, PendingLocal { domain_id, slot });
        Ok(pending_result())
    }

    /// Revokes the identity `handle`, local or remote (DDS-Security 1.2
    /// §9.3.2.12.2): the handle becomes invalid, and the listener is told
    /// with `on_revoke_identity` so that the DDS implementation ends the
    /// communication with the participant.
    ///
    /// # Errors
    /// `BadArgument` for a handle the plugin did not issue.
    pub fn revoke_identity(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
        if !self.identities.contains_key(&handle) && !self.remote_guids.contains_key(&handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown IdentityHandle",
            ));
        }
        self.revoke(handle);
        Ok(())
    }

    fn revoke(&mut self, handle: IdentityHandle) {
        // The handshakes with the identity: their state and secrets go.
        let handshakes: Vec<HandshakeHandle> = self
            .handshake_remote
            .iter()
            .filter(|(_, remote)| **remote == handle)
            .map(|(handshake, _)| *handshake)
            .collect();
        for handshake in handshakes {
            self.pending_initiator.remove(&handshake);
            self.pending_replier.remove(&handshake);
            self.peer_credentials.remove(&handshake);
            self.handshake_remote.remove(&handshake);
            self.handshakes_issued.remove(&handshake);
            if let Some(secret) = self.handshake_to_secret.remove(&handshake) {
                self.secrets.remove(&secret);
                self.secret_challenges.remove(&secret);
            }
        }
        // A local identity takes its remote identities with it.
        let remotes: Vec<IdentityHandle> = self
            .remote_roles
            .iter()
            .filter(|(_, (local, _))| *local == handle)
            .map(|(remote, _)| *remote)
            .collect();
        for remote in remotes {
            self.revoke(remote);
        }
        self.identities.remove(&handle);
        self.remote_guids.remove(&handle);
        self.remote_versions.remove(&handle);
        self.local_futures.remove(&handle);
        self.remote_futures.remove(&handle);
        self.remote_roles.remove(&handle);
        self.replay_cache.remove(&handle);
        self.replay_order.remove(&handle);
        self.local_permissions_by_identity.remove(&handle);
        self.algorithm_limits.remove(&handle);
        self.status_seen.remove(&handle);
        if let Some(listener) = self.listener.clone() {
            if let Err(e) = listener.on_revoke_identity(handle) {
                self.log_listener_failure(handle, &e);
            }
        }
    }

    fn log_listener_failure(&self, handle: IdentityHandle, error: &SecurityError) {
        use zerodds_security::logging::{LogLevel, LoggingPlugin};
        let message = alloc::format!(
            "the listener refused on_revoke_identity for IdentityHandle {}: {error}",
            handle.0
        );
        match &self.logger {
            Some(logger) => logger.log(
                LogLevel::Error,
                [0; 16],
                "authentication.listener.failed",
                &message,
            ),
            None => zerodds_security_logging::StderrLoggingPlugin::new().log(
                LogLevel::Error,
                [0; 16],
                "authentication.listener.failed",
                &message,
            ),
        }
    }

    fn local_identity(&self, handle: IdentityHandle) -> SecurityResult<&ParsedIdentity> {
        self.identities.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: not an IdentityHandle returned by validate_local_identity",
            )
        })
    }

    /// The precondition of the handshake operations (Table 59): `remote` is
    /// the identity `validate_remote_identity` made for `local`, and it
    /// returned `expected`.
    fn check_role(
        &self,
        local: IdentityHandle,
        remote: IdentityHandle,
        expected: ValidationResult,
    ) -> SecurityResult<()> {
        self.local_identity(local)?;
        match self.remote_roles.get(&remote) {
            Some((validated_for, result)) if *validated_for == local && *result == expected => {
                Ok(())
            }
            Some((validated_for, result)) => Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!(
                    "pki: precondition: validate_remote_identity returned {result:?} for local \
                     identity {}, not {expected:?} for {}",
                    validated_for.0,
                    local.0
                ),
            )),
            None => Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: precondition: not an IdentityHandle returned by validate_remote_identity",
            )),
        }
    }

    /// `set_participant_security_config` limits the plugin "to only accept
    /// the resulting set of supported algorithms": the signature scheme (when
    /// given) and the key agreement of a handshake have to be among them.
    fn require_limits_allow(
        &self,
        local: IdentityHandle,
        scheme: Option<DsignScheme>,
        suite: KxSuite,
    ) -> SecurityResult<()> {
        let Some(limits) = self.algorithm_limits.get(&local) else {
            return Ok(());
        };
        if let Some(scheme) = scheme {
            if limits.message_auth.0 & scheme_bit(scheme) == 0 {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "pki: the digital signature algorithm is not in message_auth.supported_mask",
                ));
            }
        }
        let bit = suite_bit(suite);
        if bit != 0 && limits.key_establishment.0 & bit == 0 {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "pki: the key agreement algorithm is not in shared_secret.supported_mask",
            ));
        }
        Ok(())
    }

    /// The `c.perm` of the handshakes of `identity`: the permissions
    /// document of its PermissionsCredentialToken (Table 64
    /// `dds.perm.cert`), else the one set with [`Self::set_local_permissions`].
    /// A document in S/MIME text is an ASCII string, and the value counts
    /// its terminating NUL (DDS-Security 1.2 §7.3.3.1); a binary one goes
    /// as it is.
    fn c_perm_of(&self, identity: IdentityHandle) -> Vec<u8> {
        let mut document = match self.local_permissions_by_identity.get(&identity) {
            Some(stored) => stored.permissions_document.clone(),
            None => self.local_permissions.clone(),
        };
        if !document.is_empty() && document.is_ascii() && document.last() != Some(&0) {
            document.push(0);
        }
        document
    }

    fn issue(&mut self, kind: TokenKind, token: Vec<u8>) -> Vec<u8> {
        self.tokens_out.issue(kind as u8, token)
    }

    fn take_back(&mut self, kind: TokenKind, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(kind as u8, token)
    }

    /// Tests: the `serialized_local_participant_data` the handshake
    /// operations of the test are given.
    #[cfg(test)]
    fn set_local_participant_data(&mut self, pdata: Vec<u8>) {
        self.local_pdata = pdata;
    }

    /// Tests: see [`Self::set_local_participant_data`].
    #[cfg(test)]
    fn test_pdata(&self) -> Vec<u8> {
        self.local_pdata.clone()
    }

    fn record_challenge(&mut self, local: IdentityHandle, c: [u8; 32]) -> SecurityResult<()> {
        let cache = self.replay_cache.entry(local).or_default();
        if cache.contains(&c) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "pki: replayed challenge1 detected",
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
}

impl SharedSecretProvider for PkiAuthenticationPlugin {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        self.secrets.get(&handle).cloned()
    }

    fn get_shared_secret_challenges(
        &self,
        handle: SharedSecretHandle,
    ) -> Option<([u8; 32], [u8; 32])> {
        self.secret_challenges.get(&handle).copied()
    }
}

fn pki_to_security(e: PkiError) -> SecurityError {
    let kind = match &e {
        PkiError::InvalidPem(_) | PkiError::NoCertInPem | PkiError::NoPrivateKeyInPem => {
            SecurityErrorKind::BadArgument
        }
        PkiError::CertInvalid(_) | PkiError::CertVersionNotV3(_) => {
            SecurityErrorKind::AuthenticationFailed
        }
        PkiError::EmptyTrustAnchors => SecurityErrorKind::InvalidConfiguration,
        PkiError::SigningFailed(_) => SecurityErrorKind::CryptoFailed,
    };
    SecurityError::new(kind, alloc::format!("pki: {e}"))
}

fn random_challenge() -> SecurityResult<[u8; 32]> {
    let rng = SystemRandom::new();
    let mut buf = [0u8; 32];
    rng.fill(&mut buf).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            "pki: SystemRandom not available",
        )
    })?;
    Ok(buf)
}

/// The signature scheme of the handshake (DDS-Security 1.2 §8.2 Table 25),
/// named by `c.dsign_algo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DsignScheme {
    /// `ECDSA+P256+SHA256` (1.1: `ECDSA-SHA256`).
    EcdsaP256,
    /// `ECDSA+P384+SHA384`.
    EcdsaP384,
    /// `RSASSA-PSS-MGF1SHA256+2048+SHA256` (1.1: `RSASSA-PSS-SHA256`).
    RsaPss,
    /// `RSASSA-PKCS1-V1_5+2048+SHA256`.
    RsaPkcs1,
}

impl DsignScheme {
    /// The scheme a `c.dsign_algo` names, in the names of 1.2 and of 1.1
    /// (a trailing NUL, as OpenDDS sends it, is not part of the name).
    fn from_name(declared: &str) -> Option<Self> {
        let name = declared.trim_end_matches('\0');
        let is = |known: &str| name.eq_ignore_ascii_case(known);
        if is(ht::algo::ECDSA_SHA256) || is(ht::algo::ECDSA_P256_SHA256) {
            Some(Self::EcdsaP256)
        } else if is(ht::algo::ECDSA_P384_SHA384) {
            Some(Self::EcdsaP384)
        } else if is(ht::algo::RSASSA_PSS_SHA256) || is(ht::algo::RSASSA_PSS_MGF1SHA256_2048_SHA256)
        {
            Some(Self::RsaPss)
        } else if is(ht::algo::RSASSA_PKCS1_V15_2048_SHA256) {
            Some(Self::RsaPkcs1)
        } else {
            None
        }
    }

    /// The name of the scheme: that of 1.2 with `v1_2`, else the deprecated
    /// one of 1.1 (the one Cyclone, Fast DDS and OpenDDS send) where there is
    /// one; a scheme new in 1.2 has the name of 1.2 only.
    fn name(self, v1_2: bool) -> &'static str {
        match (self, v1_2) {
            (Self::EcdsaP256, false) => ht::algo::ECDSA_SHA256,
            (Self::EcdsaP256, true) => ht::algo::ECDSA_P256_SHA256,
            (Self::EcdsaP384, _) => ht::algo::ECDSA_P384_SHA384,
            (Self::RsaPss, false) => ht::algo::RSASSA_PSS_SHA256,
            (Self::RsaPss, true) => ht::algo::RSASSA_PSS_MGF1SHA256_2048_SHA256,
            (Self::RsaPkcs1, _) => ht::algo::RSASSA_PKCS1_V15_2048_SHA256,
        }
    }

    /// Whether the scheme works with a certificate of this key algorithm.
    fn fits(self, key: CertKeyAlgo) -> bool {
        matches!(
            (self, key),
            (Self::EcdsaP256, CertKeyAlgo::EcdsaP256Sha256)
                | (Self::EcdsaP384, CertKeyAlgo::EcdsaP384Sha384)
                | (Self::RsaPss | Self::RsaPkcs1, CertKeyAlgo::RsaPssSha256)
        )
    }
}

/// `CryptoAlgorithmBit` of the RSA schemes in the `message_auth` mask of a
/// peer (§8.2 Table 25).
const BIT_RSASSA_PSS: u32 = 1 << 0;
const BIT_RSASSA_PKCS1: u32 = 1 << 1;

/// The scheme this identity signs a handshake token with. An RSA key signs
/// RSASSA-PSS, unless the peer announced that it verifies RSASSA-PKCS1-v1_5
/// only (`peer_message_auth`, DDS-Security 1.2 §7.3.11); a key on a token
/// (PKCS#11) signs RSASSA-PSS.
fn signing_scheme(
    parsed: &ParsedIdentity,
    peer_message_auth: Option<u32>,
) -> SecurityResult<DsignScheme> {
    match parsed.key_algo {
        CertKeyAlgo::EcdsaP256Sha256 => Ok(DsignScheme::EcdsaP256),
        CertKeyAlgo::EcdsaP384Sha384 => Ok(DsignScheme::EcdsaP384),
        CertKeyAlgo::RsaPssSha256 => {
            let pkcs1_only = peer_message_auth
                .is_some_and(|mask| mask & BIT_RSASSA_PSS == 0 && mask & BIT_RSASSA_PKCS1 != 0);
            Ok(if pkcs1_only && parsed.signer.is_none() {
                DsignScheme::RsaPkcs1
            } else {
                DsignScheme::RsaPss
            })
        }
        CertKeyAlgo::Unknown => Err(SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            "pki: cert public-key algo unsupported",
        )),
    }
}

/// Whether `dsign_algo` is spelled as in DDS-Security 1.2 Table 25 rather
/// than as a deprecated name of 1.1: a replier answers an initiator in its
/// names.
fn is_v1_2_dsign_name(dsign_algo: &str) -> bool {
    let name = dsign_algo.trim_end_matches('\0');
    [
        ht::algo::ECDSA_P256_SHA256,
        ht::algo::ECDSA_P384_SHA384,
        ht::algo::RSASSA_PSS_MGF1SHA256_2048_SHA256,
        ht::algo::RSASSA_PKCS1_V15_2048_SHA256,
    ]
    .iter()
    .any(|known| name.eq_ignore_ascii_case(known))
}

/// The scheme `declared` names, which has to fit the key of the certificate
/// that came with it (`detected`).
fn check_dsign_matches(declared: &str, detected: CertKeyAlgo) -> SecurityResult<DsignScheme> {
    // DDS-Security 1.2 §8.2 Table 25 names each algorithm, and keeps the
    // names of 1.1 as deprecated ones; both are taken, OpenDDS's trailing
    // NUL is not part of the name.
    match DsignScheme::from_name(declared) {
        Some(scheme) if scheme.fits(detected) => Ok(scheme),
        _ => Err(SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            alloc::format!(
                "pki: c.dsign_algo {} doesn't match cert ({detected:?})",
                declared.trim_end_matches('\0')
            ),
        )),
    }
}

fn sign_with(
    key_algo: CertKeyAlgo,
    scheme: DsignScheme,
    pkcs8: &[u8],
    msg: &[u8],
) -> SecurityResult<Vec<u8>> {
    let rng = SystemRandom::new();
    let ecdsa = |alg: &'static signature::EcdsaSigningAlgorithm| -> SecurityResult<Vec<u8>> {
        let key = crate::compat::ecdsa_from_pkcs8(alg, pkcs8, &rng).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                alloc::format!("pki: ecdsa key-parse failed: {e}"),
            )
        })?;
        let sig = key.sign(&rng, msg).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                alloc::format!("pki: ecdsa sign failed: {e}"),
            )
        })?;
        Ok(sig.as_ref().to_vec())
    };
    match (key_algo, scheme) {
        (CertKeyAlgo::EcdsaP256Sha256, DsignScheme::EcdsaP256) => {
            ecdsa(&signature::ECDSA_P256_SHA256_ASN1_SIGNING)
        }
        (CertKeyAlgo::EcdsaP384Sha384, DsignScheme::EcdsaP384) => {
            ecdsa(&signature::ECDSA_P384_SHA384_ASN1_SIGNING)
        }
        (CertKeyAlgo::RsaPssSha256, DsignScheme::RsaPss | DsignScheme::RsaPkcs1) => {
            let key = signature::RsaKeyPair::from_pkcs8(pkcs8).map_err(|e| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    alloc::format!("pki: rsa key-parse failed: {e}"),
                )
            })?;
            let padding: &'static dyn signature::RsaEncoding = if scheme == DsignScheme::RsaPss {
                &signature::RSA_PSS_SHA256
            } else {
                &signature::RSA_PKCS1_SHA256
            };
            let mut out = alloc::vec![0u8; crate::compat::rsa_modulus_len(&key)];
            key.sign(padding, &rng, msg, &mut out).map_err(|e| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    alloc::format!("pki: rsa sign failed: {e}"),
                )
            })?;
            Ok(out)
        }
        _ => Err(SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            "pki: cannot sign — unsupported key algo",
        )),
    }
}

/// Signs the handshake token with the identity's private key, whichever
/// form it has: the in-memory PKCS#8 bytes, or a token-backed
/// [`IdentitySigner`] that keeps the key on a PKCS#11 token
/// (DDS-Security 1.2 §9.3.1 Table 44).
fn sign_with_identity(
    parsed: &ParsedIdentity,
    scheme: DsignScheme,
    msg: &[u8],
) -> SecurityResult<Vec<u8>> {
    if let Some(signer) = &parsed.signer {
        return signer.sign(msg).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                alloc::format!("pki: token sign failed: {e}"),
            )
        });
    }
    let pkcs8 = parsed.private_key_pkcs8_der.as_deref().ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            "pki: no private key configured (cannot sign)",
        )
    })?;
    sign_with(parsed.key_algo, scheme, pkcs8, msg)
}

/// Verifies a handshake signature with the public key of `cert_der`.
/// `accept_x509v1`: the certificate policy accepts X.509 v1 identity
/// certificates ([`crate::x509v1_compat`]); `webpki` parses v3 only, so a v1
/// certificate's signature is then verified with its public key directly.
/// The policy accepted or refused the certificate before; this changes how
/// it is read, not whether it is trusted.
fn verify_signature_with_cert(
    cert_der: &[u8],
    scheme: DsignScheme,
    msg: &[u8],
    sig: &[u8],
    accept_x509v1: bool,
) -> SecurityResult<()> {
    let cert = CertificateDer::from_slice(cert_der);
    let ee = match webpki::EndEntityCert::try_from(&cert) {
        Ok(ee) => Some(ee),
        Err(webpki::Error::UnsupportedCertVersion) if accept_x509v1 => None,
        Err(e) => {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("pki: peer cert parse failed: {e:?}"),
            ));
        }
    };
    // DDS-Security 1.2 §8.2 Table 25 (RSASSA-PSS-MGF1SHA256+2048+SHA256): "The
    // length of the salt is not specified. ... The validation of the
    // signature shall detect the salt length from the signature." The
    // verifiers of `webpki` take the length of the hash only.
    let alg: &dyn rustls_pki_types::SignatureVerificationAlgorithm = match scheme {
        #[cfg(not(feature = "aws-lc"))]
        DsignScheme::EcdsaP256 => webpki::ring::ECDSA_P256_SHA256,
        #[cfg(feature = "aws-lc")]
        DsignScheme::EcdsaP256 => webpki::aws_lc_rs::ECDSA_P256_SHA256,
        #[cfg(not(feature = "aws-lc"))]
        DsignScheme::EcdsaP384 => webpki::ring::ECDSA_P384_SHA384,
        #[cfg(feature = "aws-lc")]
        DsignScheme::EcdsaP384 => webpki::aws_lc_rs::ECDSA_P384_SHA384,
        // The salt length is read from the signature, which the verifiers
        // of `webpki` do not do.
        DsignScheme::RsaPss => return verify_rsa_pss_of_any_salt(cert_der, msg, sig),
        #[cfg(not(feature = "aws-lc"))]
        DsignScheme::RsaPkcs1 => webpki::ring::RSA_PKCS1_2048_8192_SHA256,
        #[cfg(feature = "aws-lc")]
        DsignScheme::RsaPkcs1 => webpki::aws_lc_rs::RSA_PKCS1_2048_8192_SHA256,
    };
    let verified = match ee {
        Some(ee) => ee
            .verify_signature(alg, msg, sig)
            .map_err(|e| alloc::format!("{e:?}")),
        None => alg
            .verify_signature(&subject_public_key(cert_der)?, msg, sig)
            .map_err(|e| alloc::format!("{e:?}")),
    };
    verified.map_err(|e| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("pki: signature verify failed: {e}"),
        )
    })
}

/// The `subjectPublicKey` of a certificate of any version, the bits a
/// verification algorithm of `webpki` takes.
fn subject_public_key(cert_der: &[u8]) -> SecurityResult<Vec<u8>> {
    use x509_cert::der::Decode;
    let cert = x509_cert::Certificate::from_der(cert_der).map_err(|e| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("pki: peer cert parse failed: {e}"),
        )
    })?;
    Ok(cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes()
        .to_vec())
}

/// Verifies an RSASSA-PSS (SHA-256, MGF1-SHA256) signature of any salt length
/// with the public key of `cert_der`.
fn verify_rsa_pss_of_any_salt(cert_der: &[u8], msg: &[u8], sig: &[u8]) -> SecurityResult<()> {
    use x509_cert::der::Decode;
    let cert = x509_cert::Certificate::from_der(cert_der).map_err(|e| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("pki: peer cert parse failed: {e}"),
        )
    })?;
    let key = cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    zerodds_security_crypto::rsa_pss::verify_rsassa_pss_sha256(key, msg, sig)
        .map(|_| ())
        .map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("pki: signature verify failed: {e}"),
            )
        })
}

/// Detect peer cert algo from DER (re-uses identity helper logic via
/// trial-parse; pragmatic — we duplicate the SPKI scan locally to avoid
/// re-exporting an internal helper).
fn detect_peer_algo(cert_der: &[u8]) -> CertKeyAlgo {
    crate::identity::detect_cert_algo_pub(cert_der)
}

/// Verifies a handshake signature over `msg` with the public key of
/// `cert_der`, detecting the certificate's key algorithm.
///
/// This is the check a peer runs on a received handshake token
/// (DDS-Security 1.2 §9.3.2.5.2). It is public so that a caller holding
/// only a certificate — a PKCS#11 test, for instance — can verify a
/// signature the token produced.
///
/// # Errors
/// [`SecurityError`] with kind `AuthenticationFailed` when the
/// certificate does not parse or the signature does not verify.
pub fn verify_der_signature(cert_der: &[u8], msg: &[u8], sig: &[u8]) -> SecurityResult<()> {
    let schemes: &[DsignScheme] = match detect_peer_algo(cert_der) {
        CertKeyAlgo::EcdsaP256Sha256 => &[DsignScheme::EcdsaP256],
        CertKeyAlgo::EcdsaP384Sha384 => &[DsignScheme::EcdsaP384],
        CertKeyAlgo::RsaPssSha256 => &[DsignScheme::RsaPss, DsignScheme::RsaPkcs1],
        CertKeyAlgo::Unknown => {
            return Err(SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: peer cert algo unsupported",
            ));
        }
    };
    let mut last = None;
    for scheme in schemes {
        match verify_signature_with_cert(cert_der, *scheme, msg, sig, false) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "pki: signature verify failed",
        )
    }))
}

/// PEM-encode a DER certificate. The handshake `c.id` carries the cert as
/// **PEM** (spec §9.3.2.5.2; cyclone/FastDDS/RTI parse c.id as PEM and
/// fail on raw DER with "PEM routines: no start line"). PEM is an ASCII
/// string, so the value counts its terminating NUL (DDS-Security 1.2
/// §7.3.3.1); RTI Connext 7.7.0 calls a `c.id` without it "not properly
/// null terminated". A certificate that does not encode goes as its DER.
fn der_to_pem(der: &[u8]) -> Vec<u8> {
    use x509_cert::Certificate;
    use x509_cert::der::{Decode, EncodePem, pem::LineEnding};
    match Certificate::from_der(der).and_then(|c| c.to_pem(LineEnding::LF)) {
        Ok(pem) => {
            let mut value = pem.into_bytes();
            value.push(0);
            value
        }
        Err(_) => der.to_vec(),
    }
}

/// Normalizes received `c.id` bytes to DER for the webpki cert crypto:
/// cyclone/FastDDS send PEM, ZeroDDS legacy/internal possibly raw DER. The
/// hash check (compute_hash_c) still runs over the RAW `c.id` bytes (as
/// on the wire), only the cryptographic verification needs DER.
fn cid_to_der(bytes: &[u8]) -> Vec<u8> {
    use x509_cert::Certificate;
    use x509_cert::der::{DecodePem, Encode};
    // OpenDDS appends a NUL byte to c.id (cert `original_bytes`)
    // (`Certificate::load_cert_bytes`: `length(i + 1)`), and §7.3.3.1 asks
    // for it. The PEM block is read without the trailing NULs; hash_c1 and
    // hash_c2 still use the RAW c.id bytes (with NUL), so the hash matches
    // OpenDDS byte for byte. DER is taken as it is: its last octet belongs
    // to the signature even when it is 0x00.
    // Find a PEM certificate block anywhere in c.id, not only at offset 0.
    // RTI, Cyclone and FastDDS send pure PEM; CoreDX 6.14 prepends the
    // `openssl x509 -text` human dump before the PEM (measured 2026-09-14 on
    // the OMG dds-security suite), so `-----BEGIN CERTIFICATE-----` is not the
    // first byte and a naive `starts_with` fed the text to the DER parser
    // (its first byte `C` = 0x43 read as an APPLICATION[3] tag). The hash
    // path still uses the RAW c.id bytes, so this only affects parsing.
    const PEM_BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
    let pem_start = bytes.windows(PEM_BEGIN.len()).position(|w| w == PEM_BEGIN);
    if let Some(start) = pem_start {
        let pem = &bytes[start..];
        let pem = match pem.iter().rposition(|&b| b != 0) {
            Some(last) => &pem[..=last],
            None => pem,
        };
        if let Ok(cert) = Certificate::from_pem(pem) {
            if let Ok(der) = cert.to_der() {
                return der;
            }
        }
    }
    bytes.to_vec()
}

/// Derives the 32-byte `SharedSecret` from the raw DH output.
///
/// DDS-Security §9.3.2.5 + cyclone `generate_shared_secret` (authentication.c):
/// `SharedSecret = SHA256(raw_dh)`. **No** HKDF, **no** challenge salt —
/// the earlier ZeroDDS variant (`HKDF(raw_dh, salt=ch1||ch2)`) produced a
/// different secret cross-vendor than cyclone/FastDDS and thereby broke the
/// downstream crypto/VolatileSecure key exchange (the handshake itself
/// verified, because the signature does not cover the SharedSecret). challenge1/
/// challenge2 are carried separately (by the caller) for the VolatileSecure key derivation
/// (§9.5.3.5), not mixed in here.
fn derive_shared_secret(raw_dh: &[u8]) -> SecurityResult<Vec<u8>> {
    let d = digest::digest(&digest::SHA256, raw_dh);
    Ok(d.as_ref().to_vec())
}

/// PID_PARTICIPANT_GUID (DDSI-RTPS 2.5 Table 9.13).
const PID_PARTICIPANT_GUID: u16 = 0x0050;
/// PID_SENTINEL.
const PID_SENTINEL: u16 = 0x0001;

/// The participant GUID in `c.pdata`, "the CDR Big Endian Serialization of
/// the ParticipantBuiltinTopicData" (§10.3.2.5.1): a big-endian parameter
/// list from the first octet, as Cyclone (`DDS_Security_Deserialize_
/// ParticipantBuiltinTopicData`) and Fast DDS (`read_guid_from_cdr_msg`
/// with `BIGEND`) read it. The encapsulation header ZeroDDS puts in front
/// (`00 02 00 00`) reads as a parameter 0x0002 of length 0 and is passed
/// over, as by both.
fn pdata_participant_guid(pdata: &[u8]) -> SecurityResult<[u8; 16]> {
    let fail =
        |what: &'static str| SecurityError::new(SecurityErrorKind::AuthenticationFailed, what);
    let u16_at = |at: usize| -> Option<u16> {
        let b = pdata.get(at..at + 2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    };
    let mut at = 0;
    while let (Some(pid), Some(len)) = (u16_at(at), u16_at(at + 2)) {
        if pid == PID_SENTINEL {
            break;
        }
        let value = at + 4;
        let end = value + usize::from(len);
        let Some(bytes) = pdata.get(value..end) else {
            return Err(fail("c.pdata: a parameter runs past the end"));
        };
        if pid == PID_PARTICIPANT_GUID {
            return bytes
                .get(..16)
                .and_then(|g| <[u8; 16]>::try_from(g).ok())
                .ok_or_else(|| fail("c.pdata: PID_PARTICIPANT_GUID shorter than a GUID"));
        }
        // Parameters start on a 4-octet boundary (DDSI-RTPS 2.5 §9.4.2.11).
        at = (end + 3) & !3;
    }
    Err(fail("c.pdata: no PID_PARTICIPANT_GUID"))
}

/// DDS-Security 1.2 §7.4.3 and §10.3.3 Table 59: the participant GUID in a
/// handshake's `c.pdata` is bound to the identity in its `c.id` — bit 0
/// set, bits 1 to 47 from the SHA-256 hash of the SubjectName — and is the
/// GUID the participant was discovered with (`discovered`, the prefix
/// compared). Without the second check a participant with a valid
/// certificate could announce another participant's GUID over SPDP and
/// hand in its own, correct one in `c.pdata` (the "squatter" attack of
/// §7.4.3).
fn check_pdata(pdata: &[u8], cert_der: &[u8], discovered: Option<&[u8; 16]>) -> SecurityResult<()> {
    let guid = pdata_participant_guid(pdata)?;
    let head = crate::guid_adjust::identity_prefix_head(cert_der).map_err(pki_to_security)?;
    if guid[..6] != head {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "c.pdata: the participant GUID is not derived from the SubjectName of c.id",
        ));
    }
    if let Some(discovered) = discovered {
        if guid[..12] != discovered[..12] {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "c.pdata: the participant GUID is not the one the participant was discovered with",
            ));
        }
    }
    Ok(())
}

/// `CryptoAlgorithmBit` of the digital signature algorithms (DDS-Security 1.2
/// §8.2 Table 25).
mod sig_bit {
    pub(super) const RSASSA_PSS_2048_SHA256: u32 = 1 << 0;
    pub(super) const RSASSA_PKCS1_V15_2048_SHA256: u32 = 1 << 1;
    pub(super) const ECDSA_P256_SHA256: u32 = 1 << 2;
    pub(super) const ECDSA_P384_SHA384: u32 = 1 << 3;
}

/// `CryptoAlgorithmBit` of the key establishment algorithms (§8.3 Table 26).
mod kx_bit {
    pub(super) const DHE_MODP_2048_256: u32 = 1 << 0;
    pub(super) const ECDHE_CEUM_P256: u32 = 1 << 1;
    pub(super) const ECDHE_CEUM_P384: u32 = 1 << 2;
}

/// The digital signature algorithm bit of the signature of a certificate
/// (§8.2): 0 for one that has none.
fn certificate_signature_bit(cert_der: &[u8]) -> u32 {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;
    let Ok(cert) = Certificate::from_der(cert_der) else {
        return 0;
    };
    signature_bit_of_oid(cert.signature_algorithm.oid.to_string().as_str())
}

/// The Table 24 bit of a signature algorithm object identifier (dotted form);
/// 0 for one the table has no bit for.
fn signature_bit_of_oid(oid: &str) -> u32 {
    match oid {
        "1.2.840.10045.4.3.2" => sig_bit::ECDSA_P256_SHA256,
        "1.2.840.10045.4.3.3" => sig_bit::ECDSA_P384_SHA384,
        // sha256WithRSAEncryption, or rsaEncryption as a CMS SignerInfo names
        // it (its digest algorithm is SHA-256).
        "1.2.840.113549.1.1.11" | "1.2.840.113549.1.1.1" => sig_bit::RSASSA_PKCS1_V15_2048_SHA256,
        "1.2.840.113549.1.1.10" => sig_bit::RSASSA_PSS_2048_SHA256,
        _ => 0,
    }
}

/// `notAfter` of a certificate, Unix seconds.
fn cert_not_after(cert_der: &[u8]) -> Option<u64> {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;
    let cert = Certificate::from_der(cert_der).ok()?;
    Some(
        cert.tbs_certificate
            .validity
            .not_after
            .to_unix_duration()
            .as_secs(),
    )
}

/// The content of the IdentityStatusToken of a local identity (DDS-Security
/// 1.2 §10.3.2.2 Table 53): class `DDS:Auth:PKI-DH:1.0` and, when an OCSP
/// responder showed the identity certificate good, that response as the
/// binary property `ocsp_status`.
fn identity_status_bytes(parsed: &ParsedIdentity) -> Vec<u8> {
    let mut token = ht::DataHolder::new(crate::identity_token::IDENTITY_TOKEN_CLASS_ID);
    if let Some(response) = parsed.ocsp_staple.as_ref().filter(|r| !r.is_empty()) {
        token.set_binary_property(ht::prop::OCSP_STATUS, response.clone());
    }
    token.to_cdr_le()
}

/// The `message_auth` bit of the algorithm of the identity key (§8.2).
fn key_algo_bit(key_algo: CertKeyAlgo) -> u32 {
    match key_algo {
        CertKeyAlgo::EcdsaP256Sha256 => sig_bit::ECDSA_P256_SHA256,
        CertKeyAlgo::EcdsaP384Sha384 => sig_bit::ECDSA_P384_SHA384,
        CertKeyAlgo::RsaPssSha256 => sig_bit::RSASSA_PSS_2048_SHA256,
        CertKeyAlgo::Unknown => 0,
    }
}

/// The bit of a signature scheme in a `message_auth` mask (§8.2 Table 25).
fn scheme_bit(scheme: DsignScheme) -> u32 {
    match scheme {
        DsignScheme::EcdsaP256 => sig_bit::ECDSA_P256_SHA256,
        DsignScheme::EcdsaP384 => sig_bit::ECDSA_P384_SHA384,
        DsignScheme::RsaPss => sig_bit::RSASSA_PSS_2048_SHA256,
        DsignScheme::RsaPkcs1 => sig_bit::RSASSA_PKCS1_V15_2048_SHA256,
    }
}

/// The bit of a key agreement in a `shared_secret` mask (§8.3 Table 26);
/// 0 for X25519, a vendor extension that has none.
fn suite_bit(suite: KxSuite) -> u32 {
    match suite {
        KxSuite::EcdhP256 => kx_bit::ECDHE_CEUM_P256,
        KxSuite::EcdhP384 => kx_bit::ECDHE_CEUM_P384,
        KxSuite::DhModp2048_256 => kx_bit::DHE_MODP_2048_256,
        KxSuite::X25519 => 0,
    }
}

impl AuthenticationExtensions for PkiAuthenticationPlugin {
    fn plugin_class_id(&self) -> &str {
        "DDS:Auth:PKI-DH:1.2"
    }

    fn set_algo_nul_terminate(&mut self, nul: bool) {
        self.algo_nul = nul;
    }

    fn set_remote_message_auth_supported(&mut self, supported: Option<u32>) {
        self.remote_message_auth = supported;
    }

    fn set_peer_algorithm_names(&mut self, names: PeerAlgorithmNames) {
        self.peer_algorithm_names = names;
    }

    fn get_permissions_token(&self) -> Vec<u8> {
        // The token the AccessControl plugin made, handed over with
        // `set_permissions_credential_and_token`.
        if let Some(stored) = self.local_permissions_by_identity.values().next() {
            return stored.permissions_token.clone();
        }
        // Without configured permissions AccessControl is inactive — then
        // the token is omitted (no empty permissions-match trigger).
        if self.local_permissions.is_empty() {
            return Vec::new();
        }
        // class_id-only PermissionsToken. The class is the one of DDS-Security
        // 1.2 Table 65, `DDS:Access:Permissions:1.2`; `1.0` is what Cyclone
        // DDS and Fast DDS announce and is the named setting
        // `org.zerodds.sec.access.permissions_token_class_id` of the
        // AccessControl plugin, whose token the runtime announces. The signed
        // permissions content travels in the handshake as `c.perm`, not in the
        // SPDP token.
        ht::DataHolder::new(zerodds_security::token::class_id::ACCESS_PERMISSIONS_V12).to_cdr_le()
    }

    fn check_identities(&mut self, now_unix_secs: u64) {
        let mut revoked: Vec<IdentityHandle> = Vec::new();
        // The local identities: an identity certificate that expired is an
        // identity that is revoked (§9.3.2.12).
        for (handle, parsed) in &self.identities {
            if cert_not_after(&parsed.cert_der).is_some_and(|end| end < now_unix_secs) {
                revoked.push(*handle);
            }
        }
        // The remote identities that completed a handshake: the certificate
        // expired, or a CRL set since lists it.
        for (handshake, remote) in &self.handshake_remote {
            let Some(credential) = self.peer_credentials.get(handshake) else {
                continue;
            };
            let der = cid_to_der(&credential.c_id);
            let expired = cert_not_after(&der).is_some_and(|end| end < now_unix_secs);
            let listed = self
                .remote_roles
                .get(remote)
                .and_then(|(local, _)| self.identities.get(local))
                .is_some_and(|local| local.recheck_remote_der(&der).is_err());
            if expired || listed {
                revoked.push(*remote);
            }
        }
        revoked.sort();
        revoked.dedup();
        for handle in revoked {
            self.revoke(handle);
        }
        // A change of the status token of a local identity.
        let changed: Vec<IdentityHandle> = self
            .status_seen
            .iter()
            .filter(|(handle, seen)| {
                self.identities
                    .get(handle)
                    .is_some_and(|parsed| &identity_status_bytes(parsed) != *seen)
            })
            .map(|(handle, _)| *handle)
            .collect();
        for handle in changed {
            if let Some(parsed) = self.identities.get(&handle) {
                self.status_seen
                    .insert(handle, identity_status_bytes(parsed));
            }
            if let Some(listener) = self.listener.clone() {
                listener.on_status_changed(handle, AuthStatusKind::IdentityStatus);
            }
        }
    }
}

impl AuthenticationPlugin for PkiAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        domain_id: u32,
        participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        let props = participant_qos;
        let cert = props.get(keys::IDENTITY_CERT).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: missing dds.sec.auth.identity_certificate",
            )
        })?;
        let ca = props.get(keys::IDENTITY_CA).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: missing dds.sec.auth.identity_ca",
            )
        })?;
        // Table 50: the property list, and nothing else, configures the
        // plugin. The X.509 v1 deviation is read here like every other
        // `dds.sec.auth.*` value — absent means off, which is
        // §10.3.1.3 behaviour.
        self.cert_policy.accept_x509v1_identity_certificate = props
            .get(keys::ACCEPT_X509V1_IDENTITY)
            .is_some_and(property_enables_deviation);
        let cfg = IdentityConfig {
            identity_cert_pem: cert.as_bytes().to_vec(),
            identity_ca_pem: ca.as_bytes().to_vec(),
            identity_key_pem: props.get(keys::IDENTITY_KEY).map(|s| s.as_bytes().to_vec()),
        };
        self.validate_local_identity_nonblocking(cfg, domain_id, candidate_participant_guid)
    }

    fn get_identity_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        let parsed = self.local_identity(handle)?;
        let ca_der = parsed.trust_anchors_der.first().ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "pki: no trust anchor")
        })?;
        let token = crate::identity_token::build_identity_token_from_der_with(
            &parsed.cert_der,
            ca_der,
            self.cert_policy.algorithm_naming == ht::AlgorithmNaming::Legacy,
            self.subject_name_form,
        )
        .map_err(pki_to_security)?;
        let bytes = token.encode();
        Ok(self.issue(TokenKind::Identity, bytes))
    }

    fn get_identity_status_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        let parsed = self.local_identity(handle)?;
        let bytes = identity_status_bytes(parsed);
        self.status_seen.insert(handle, bytes.clone());
        Ok(self.issue(TokenKind::IdentityStatus, bytes))
    }

    fn set_participant_security_config(
        &mut self,
        handle: IdentityHandle,
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<AlgorithmMasks> {
        let parsed = self.local_identity(handle)?;
        let given = participant_security_config.algorithm_info;
        let precondition =
            |what: &'static str| SecurityError::new(SecurityErrorKind::InvalidConfiguration, what);
        // What the plugin verifies and makes.
        let documents = sig_bit::RSASSA_PSS_2048_SHA256
            | sig_bit::RSASSA_PKCS1_V15_2048_SHA256
            | sig_bit::ECDSA_P256_SHA256
            | sig_bit::ECDSA_P384_SHA384;
        let key_agreements =
            kx_bit::DHE_MODP_2048_256 | kx_bit::ECDHE_CEUM_P256 | kx_bit::ECDHE_CEUM_P384;
        // Table 59: each supported_mask "includes at least one algorithm that
        // is supported by the AUTH plugin. Fail otherwise."
        if given.trust_chain.0 & documents == 0 {
            return Err(precondition(
                "pki: digital_signature.trust_chain.supported_mask has no algorithm of the plugin",
            ));
        }
        if given.message_auth.0 & documents == 0 {
            return Err(precondition(
                "pki: digital_signature.message_auth.supported_mask has no algorithm of the plugin",
            ));
        }
        if given.key_establishment.0 & key_agreements == 0 {
            return Err(precondition(
                "pki: key_establishment.shared_secret.supported_mask has no algorithm of the plugin",
            ));
        }
        // The adjusted info: a copy, the unsupported algorithms removed from
        // the supported masks, the algorithms the plugin uses added to the
        // required masks.
        let trust_supported = given.trust_chain.0 & documents;
        let auth_supported = given.message_auth.0 & documents;
        let kx_supported = given.key_establishment.0 & key_agreements;
        let signs_with = key_algo_bit(parsed.key_algo);
        let agrees_with = suite_bit(self.preferred_kx_suite);
        let adjusted = AlgorithmMasks {
            trust_chain: (
                trust_supported,
                given.trust_chain.1
                    | certificate_signature_bit(&parsed.cert_der)
                    | self.local_permissions_signature_bit,
            ),
            message_auth: (auth_supported, given.message_auth.1 | signs_with),
            key_establishment: (kx_supported, given.key_establishment.1 | agrees_with),
        };
        // "Check that this algorithm is present in the
        // digital_signature.trust_chain.supported_mask, fail otherwise."
        if signs_with != 0 && adjusted.trust_chain.0 & signs_with == 0 {
            return Err(precondition(
                "pki: the algorithm of the identity key is not in trust_chain.supported_mask",
            ));
        }
        // "Check that this algorithm is present in the key_establisment.
        // shared_secret.supported_mask, fail otherwise." X25519 is a vendor
        // extension that has no bit.
        if agrees_with != 0 && adjusted.key_establishment.0 & agrees_with == 0 {
            return Err(precondition(
                "pki: the key agreement of the initiator is not in shared_secret.supported_mask",
            ));
        }
        // "The operation shall configure the AUTH plugin to only accept the
        // resulting set of supported algorithms."
        self.algorithm_limits.insert(handle, adjusted);
        Ok(adjusted)
    }

    fn set_permissions_credential_and_token(
        &mut self,
        handle: IdentityHandle,
        permissions_credential_token: &[u8],
        permissions_token: &[u8],
    ) -> SecurityResult<()> {
        self.local_identity(handle)?;
        let bad = |what: &'static str| SecurityError::new(SecurityErrorKind::BadArgument, what);
        // Table 64: the PermissionsCredentialToken of DDS:Access:Permissions.
        let credential = ht::DataHolder::from_cdr_le(permissions_credential_token)
            .map_err(|_| bad("pki: the PermissionsCredentialToken is not a DataHolder"))?;
        if credential.class_id != zerodds_security::token::class_id::ACCESS_PERMISSIONS_CREDENTIAL {
            return Err(bad(
                "pki: the PermissionsCredentialToken is not of the class DDS:Access:PermissionsCredential",
            ));
        }
        let document = credential
            .property(PERMISSIONS_CREDENTIAL_CERT)
            .map(|v| v.as_bytes().to_vec())
            .or_else(|| {
                credential
                    .binary_property(PERMISSIONS_CREDENTIAL_CERT)
                    .map(<[u8]>::to_vec)
            })
            .filter(|document| !document.is_empty())
            .ok_or_else(|| {
                bad("pki: the PermissionsCredentialToken has no dds.perm.cert property")
            })?;
        ht::DataHolder::from_cdr_le(permissions_token)
            .map_err(|_| bad("pki: the PermissionsToken is not a DataHolder"))?;
        self.local_permissions_by_identity.insert(
            handle,
            LocalPermissions {
                permissions_document: document,
                permissions_token: permissions_token.to_vec(),
            },
        );
        Ok(())
    }

    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        remote_identity_token: &[u8],
        remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation> {
        let local_guid = self.local_identity(local)?.participant_guid;
        // §10.3.2.1, Table 51: the remote's IdentityToken is a DataHolder of
        // the class `DDS:Auth:PKI-DH:<Major>.<Minor>` (a minimal one, with
        // the class_id alone, is what Cyclone and Fast DDS announce). Table
        // 59: "The operation shall compare the class_id of the local
        // identity_token with that of the remote_identity_token. If the
        // PluginClassName or the MajorVersion are different, it shall return
        // VALIDATION_FAILED." The certificate is validated in the handshake
        // (`verify_remote_der` on `c.id`, where the real certificate
        // arrives).
        let holder = zerodds_security::token::DataHolder::from_cdr_le(remote_identity_token)
            .map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "pki: the remote identity token is not a DataHolder",
                )
            })?;
        if !zerodds_security::token::class_id::same_class_and_major_version(
            crate::identity_token::IDENTITY_TOKEN_CLASS_ID,
            &holder.class_id,
        ) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!(
                    "pki: the class_id '{}' of the remote identity token is not that of DDS:Auth:PKI-DH, major version 1",
                    holder.class_id
                ),
            ));
        }
        // The spelling the peer speaks; a minor version this plugin has no
        // spelling for is answered in the specification's own.
        let remote_version =
            ht::ClassVersion::of_class_id(&holder.class_id, "").unwrap_or_default();
        // Table 59: "compare lexicographically the remote_participant_guid
        // with the participant key obtained from the local_identity_handle.
        // If the remote_participant_guid > local_participant_guid, the
        // operation shall return VALIDATION_PENDING_HANDSHAKE_REQUEST. If the
        // remote_participant_guid < local_participant_guid, [...]
        // VALIDATION_PENDING_HANDSHAKE_MESSAGE."
        let result = match remote_participant_guid.cmp(&local_guid) {
            core::cmp::Ordering::Greater => ValidationResult::PendingHandshakeRequest,
            core::cmp::Ordering::Less => ValidationResult::PendingHandshakeMessage,
            core::cmp::Ordering::Equal => {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "pki: the remote participant GUID is the local one",
                ));
            }
        };
        // Table 59: "If the remote_auth_request_token is NIL, the operation
        // shall generate a local_auth_request_token ..., otherwise the
        // local_auth_request_token shall be set to TokenNIL." The NONCE is
        // this side's challenge toward the peer either way, as with Cyclone.
        let remote_future = remote_auth_request_token
            .map(ht::parse_auth_request_token)
            .transpose()?;
        let local_future = random_challenge()?;
        let handle = IdentityHandle(self.next_id());
        self.remote_versions.insert(handle, remote_version);
        self.remote_guids.insert(handle, remote_participant_guid);
        self.local_futures.insert(handle, local_future);
        self.remote_roles.insert(handle, (local, result));
        let local_auth_request_token = match remote_future {
            Some(future) => {
                self.remote_futures.insert(handle, future);
                None
            }
            None => Some(ht::build_auth_request_token(remote_version, &local_future)),
        };
        Ok(RemoteIdentityValidation {
            result,
            identity: handle,
            local_auth_request_token,
        })
    }

    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        replier: IdentityHandle,
        serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        // Table 59: the initiator_identity_handle is the local_identity_handle
        // of a validate_remote_identity that returned
        // VALIDATION_PENDING_HANDSHAKE_REQUEST, the replier_identity_handle
        // the remote_identity_handle it returned.
        self.check_role(
            initiator,
            replier,
            ValidationResult::PendingHandshakeRequest,
        )?;
        // The replier's own spelling, from its IdentityToken: RTI Connext
        // announces `DDS:Auth:PKI-DH:1.2` and speaks `1.2+Req`.
        let version = self
            .remote_versions
            .get(&replier)
            .copied()
            .unwrap_or_default();
        let parsed = self.local_identity(initiator)?;
        // c.id carries the cert as PEM (spec §9.3.2.5.2 / cross-vendor);
        // hash_c1 is computed in build_request_token consistently over the same
        // PEM bytes.
        let cert_der = der_to_pem(&parsed.cert_der);
        // The OCSP response that showed our certificate good, if we asked.
        let ocsp_status = parsed.ocsp_staple.clone().unwrap_or_default();
        let scheme = signing_scheme(parsed, self.remote_message_auth)?;
        // The name of Table 25; the one of 1.1 (which Cyclone, Fast DDS and
        // OpenDDS send and read) where the setting or the peer asks for it.
        let legacy = self.legacy_names(version);
        let dsign_algo = self.algo_str(scheme.name(!legacy));
        // Spec/cross-vendor default (EcdhP256) instead of the non-spec X25519
        // extension. The announced `c.kagree_algo` must match the actually
        // used suite.
        let suite = self.preferred_kx_suite;
        // set_participant_security_config: "configure the AUTH plugin to only
        // accept the resulting set of supported algorithms".
        self.require_limits_allow(initiator, Some(scheme), suite)?;
        let kagree_algo = self.algo_str(Self::kagree_algo_str(suite, legacy));

        let kx = KeyExchange::with_suite(suite)?;
        let dh1 = kx.public_key().to_vec();
        // Table 59: the challenge1 is the future_challenge of the
        // local_auth_request_token, made with the replier's identity.
        let challenge1 = match self.local_futures.get(&replier) {
            Some(future) => *future,
            None => random_challenge()?,
        };
        // c.perm: local CMS-signed permissions document (cross-vendor
        // AccessControl requirement). c.pdata: own ParticipantBuiltinTopicData
        // as PL_CDR (the replier deserializes it as a ParameterList).
        let permissions: Vec<u8> = self.c_perm_of(initiator);
        let pdata: Vec<u8> = serialized_local_participant_data.to_vec();

        let bytes = ht::build_request_token(&RequestBuildInput {
            version,
            cert_der: &cert_der,
            permissions: &permissions,
            pdata: &pdata,
            dsign_algo: &dsign_algo,
            kagree_algo: &kagree_algo,
            dh1: &dh1,
            challenge1: &challenge1,
            ocsp_status: &ocsp_status,
        })?;

        let hash_c1 =
            ht::compute_hash_c(&cert_der, &permissions, &pdata, &dsign_algo, &kagree_algo);

        let handle = HandshakeHandle(self.next_id());
        self.handshakes_issued.insert(handle);
        self.handshake_remote.insert(handle, replier);
        self.pending_initiator.insert(
            handle,
            InitiatorState {
                local: initiator,
                remote: replier,
                kx: Some(kx),
                dh1,
                challenge1,
                hash_c1,
                permissions,
                pdata,
                kagree_algo,
                dsign_algo,
                scheme,
            },
        );
        Ok((handle, HandshakeStepOutcome::SendMessage { token: bytes }))
    }

    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        initiator: IdentityHandle,
        request_token: &[u8],
        serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        // Table 59: the replier_identity_handle is the local_identity_handle
        // of a validate_remote_identity that returned
        // VALIDATION_PENDING_HANDSHAKE_MESSAGE, the initiator_identity_handle
        // the remote_identity_handle it returned; otherwise the
        // PreconditionError.
        self.check_role(
            replier,
            initiator,
            ValidationResult::PendingHandshakeMessage,
        )?;
        // 1) Parse + hash re-validation (happens in parse_request_token).
        let req = ht::parse_request_token(request_token)?;
        // Echo kagree for the reply: `req.kagree_algo` is NUL-stripped on
        // parsing; for OpenDDS peers (algo_nul) restore the NUL form
        // so that the wire (DiffieHellman::factory sizeof comparison) AND
        // hash_c2 are consistently NUL-terminated. For FastDDS/Cyclone NUL-free.
        let reply_kagree = self.algo_str(&req.kagree_algo);

        // 2) Validate the initiator cert against our trust store.
        let parsed = self.local_identity(replier)?;
        let (acceptance, ocsp) = parsed
            .verify_remote_der_checked(&cid_to_der(&req.cert_der), &req.ocsp_status)
            .map_err(pki_to_security)?;
        let replier_guid = parsed.participant_guid;
        self.log_cert_acceptance(&acceptance, replier_guid, "remote initiator");
        self.log_ocsp_outcome(&ocsp, replier_guid, "remote initiator");

        // 2b) Table 59: "The operation shall verify that the first bit of the
        //     participant_guid of the ParticipantBuiltinTopic data inside the
        //     'c.pdata' is set to 1 and that the following 47 bits match the
        //     first 47 bits of the SHA-256 hash of the SubjectName appearing
        //     in the IdentityCredential."
        check_pdata(
            &req.pdata,
            &cid_to_der(&req.cert_der),
            self.remote_guids.get(&initiator),
        )?;

        // 2c) Table 59: "If the [remote_auth_request_token] was different
        //     from TokenNIL, the operation shall verify that the property
        //     named future_challenge found in that token is the same value as
        //     the property named challenge1 found in the handshake_message_in".
        if let Some(future) = self.remote_futures.get(&initiator) {
            if !ct_eq(future, &req.challenge1) {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "request: challenge1 is not the future_challenge of the AuthRequest",
                ));
            }
        }

        // 4) The initiator `c.dsign_algo` must match its cert public key.
        let initiator_key_algo = detect_peer_algo(&cid_to_der(&req.cert_der));
        let initiator_scheme = check_dsign_matches(&req.dsign_algo, initiator_key_algo)?;

        // 5) Generate our own DH pair + challenge2. The KX suite follows the
        //    `c.kagree_algo` announced by the initiator (spec: the replier
        //    uses the same mechanism) — unsupported = reject.
        let suite =
            Self::kx_suite_for_algo(&req.kagree_algo, self.preferred_kx_suite == KxSuite::X25519)
                .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "pki: unsupported c.kagree_algo in the request",
                )
            })?;
        // set_participant_security_config: only the resulting set of
        // supported algorithms is accepted.
        self.require_limits_allow(replier, Some(initiator_scheme), suite)?;
        // 3) Replay detection; a request that is refused for its algorithms
        //    is not recorded, so that a corrected configuration takes it.
        self.record_challenge(replier, req.challenge1)?;
        let kx = KeyExchange::with_suite(suite)?;
        let dh2 = kx.public_key().to_vec();
        // Table 59: the challenge2 is the future_challenge of the
        // local_auth_request_token, made with the initiator's identity.
        let challenge2 = match self.local_futures.get(&initiator) {
            Some(future) => *future,
            None => random_challenge()?,
        };

        // 6) DH agreement → SharedSecret.
        let parsed = self.local_identity(replier)?;
        let replier_cert_der = der_to_pem(&parsed.cert_der);
        let replier_ocsp = parsed.ocsp_staple.clone().unwrap_or_default();
        // An RSA replier signs RSASSA-PKCS1-v1_5 when the initiator did.
        let replier_scheme = match signing_scheme(parsed, self.remote_message_auth)? {
            DsignScheme::RsaPss
                if initiator_scheme == DsignScheme::RsaPkcs1 && parsed.signer.is_none() =>
            {
                DsignScheme::RsaPkcs1
            }
            scheme => scheme,
        };
        let replier_dsign = self.algo_str(replier_scheme.name(is_v1_2_dsign_name(&req.dsign_algo)));

        // raw DH output (KeyExchange consumes self).
        let secret_bytes = kx.derive_shared_secret(&req.dh1)?;
        // HKDF-Re-Derive with challenges as salt for spec-aligned key.
        let final_secret = derive_shared_secret(&secret_bytes)?;

        // 7) Replier hash_c2 = over the replier tuple (echo kagree, own
        //    cert/perm/pdata/dsign). c.perm = local permissions document.
        //    Needed both in the reply token and in the signature content.
        let permissions: Vec<u8> = self.c_perm_of(replier);
        let pdata: Vec<u8> = serialized_local_participant_data.to_vec();
        let hash_c2 = ht::compute_hash_c(
            &replier_cert_der,
            &permissions,
            &pdata,
            &replier_dsign,
            &reply_kagree,
        );

        // 8) Reply signature (DDS-Security §9.3.2.5.2.2): the replier signs over
        //    BinaryPropertySeq{ hash_c2, ch2, dh2, ch1, dh1, hash_c1 }.
        let to_sign = ht::reply_signing_bytes(
            &hash_c2,
            &challenge2,
            &dh2,
            &req.challenge1,
            &req.dh1,
            &req.hash_c1,
        );
        let signature = {
            let parsed = self.local_identity(replier)?;
            sign_with_identity(parsed, replier_scheme, &to_sign)?
        };

        // 9) Build the reply token.
        let reply_bytes = ht::build_reply_token(&ReplyBuildInput {
            version: req.version,
            cert_der: &replier_cert_der,
            permissions: &permissions,
            pdata: &pdata,
            dsign_algo: &replier_dsign,
            kagree_algo: &reply_kagree,
            dh2: &dh2,
            challenge2: &challenge2,
            hash_c1: &req.hash_c1,
            dh1: &req.dh1,
            challenge1: &req.challenge1,
            ocsp_status: &replier_ocsp,
            signature: &signature,
        })?;

        // 10) Persist state — the final-receive needs it.
        // challenge1 = initiator (req), challenge2 = own replier value.
        // The SharedSecret is not handed out before the final: the
        // initiator proves the possession of its private key only with the
        // signature in the final (DDS-Security 1.2 §9.3.2.5.2.3), and
        // `shared_secret` is what tells the caller the handshake is done.
        let handle = HandshakeHandle(self.next_id());
        let secret_handle = self.store_secret(handle, final_secret, req.challenge1, challenge2);
        self.handshakes_issued.insert(handle);
        self.handshake_remote.insert(handle, initiator);
        self.pending_replier.insert(
            handle,
            ReplierState {
                local: replier,
                challenge1: req.challenge1,
                challenge2,
                dh1: req.dh1,
                dh2,
                hash_c1: req.hash_c1,
                hash_c2,
                secret_handle,
                // The initiator's identity and permissions document travelled
                // in the request; they are its credential once the final has
                // proved its key (Table 54).
                peer: PeerCredential {
                    c_id: req.cert_der.clone(),
                    c_perm: req.permissions.clone(),
                },
                initiator_cert_der: req.cert_der,
                initiator_scheme,
            },
        );

        Ok((
            handle,
            HandshakeStepOutcome::SendMessage { token: reply_bytes },
        ))
    }

    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        // The initiator side has a pending_initiator entry → token = REPLY.
        if self.pending_initiator.contains_key(&handshake) {
            return self.process_reply_on_initiator(handshake, token);
        }
        // The replier side has a pending_replier entry → token = FINAL.
        if self.pending_replier.contains_key(&handshake) {
            return self.process_final_on_replier(handshake, token);
        }
        Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "pki: unknown HandshakeHandle",
        ))
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        self.handshake_to_secret
            .get(&handshake)
            .copied()
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "pki: handshake handle unknown or not yet completed",
                )
            })
    }

    fn get_authenticated_peer_credential_token(
        &mut self,
        handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        // Table 54: class_id "DDS:Auth:PKI-DH:1.0", binary properties `c.id`
        // and `c.perm` "received from the peer DomainParticipant as part of
        // the authentication process".
        let credential = self.peer_credentials.get(&handshake).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: the handshake has not completed (no authenticated peer)",
            )
        })?;
        let bytes = ht::DataHolder::new(crate::identity_token::IDENTITY_TOKEN_CLASS_ID)
            .with_binary_property(ht::prop::C_ID, credential.c_id.clone())
            .with_binary_property(ht::prop::C_PERM, credential.c_perm.clone())
            .to_cdr_le();
        Ok(self.issue(TokenKind::PeerCredential, bytes))
    }

    fn set_listener(
        &mut self,
        listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()> {
        self.listener = listener;
        Ok(())
    }

    fn return_identity_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.take_back(TokenKind::Identity, token)
    }

    fn return_identity_status_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.take_back(TokenKind::IdentityStatus, token)
    }

    fn return_authenticated_peer_credential_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.take_back(TokenKind::PeerCredential, token)
    }

    fn return_handshake_handle(&mut self, handshake: HandshakeHandle) -> SecurityResult<()> {
        if !self.handshakes_issued.remove(&handshake) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown HandshakeHandle",
            ));
        }
        self.pending_initiator.remove(&handshake);
        self.pending_replier.remove(&handshake);
        self.handshake_to_secret.remove(&handshake);
        self.handshake_remote.remove(&handshake);
        self.peer_credentials.remove(&handshake);
        Ok(())
    }

    fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
        let local = self.identities.remove(&handle).is_some();
        self.replay_cache.remove(&handle);
        self.replay_order.remove(&handle);
        let remote = self.remote_guids.remove(&handle).is_some();
        self.remote_versions.remove(&handle);
        self.local_futures.remove(&handle);
        self.remote_futures.remove(&handle);
        self.remote_roles.remove(&handle);
        self.local_permissions_by_identity.remove(&handle);
        self.algorithm_limits.remove(&handle);
        self.status_seen.remove(&handle);
        if local || remote {
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown IdentityHandle",
            ))
        }
    }

    fn return_sharedsecret_handle(&mut self, handle: SharedSecretHandle) -> SecurityResult<()> {
        // Only a secret `shared_secret` can have handed out: that of a
        // handshake that completed.
        if !self.handshake_to_secret.values().any(|s| *s == handle) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown SharedSecretHandle",
            ));
        }
        self.handshake_to_secret.retain(|_, s| *s != handle);
        self.secrets.remove(&handle);
        self.secret_challenges.remove(&handle);
        Ok(())
    }
}

impl PkiAuthenticationPlugin {
    fn process_reply_on_initiator(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let reply = ht::parse_reply_token(token)?;

        // DDS-Security 1.2 §9.3.2.11.5: the state stays until the reply has
        // passed its checks. A forged reply taken out the state, and the
        // genuine one found none.
        let st = {
            let st = self.pending_initiator.get(&handshake).ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::BadArgument, "pki: initiator state gone")
            })?;
            InitiatorChecks {
                local: st.local,
                remote: st.remote,
                dh1: st.dh1.clone(),
                challenge1: st.challenge1,
                hash_c1: st.hash_c1,
                kagree_algo: st.kagree_algo.clone(),
            }
        };

        // a) Echo consistency: challenge1 must match 1:1. hash_c1/dh1 are
        //    optional echo fields — cyclone/FastDDS omit them; if
        //    present, check against our own state (cert-bind).
        // hash_c1 echo (§9.3.2.3.2, cert-bind): cyclone omits hash_c1 in the reply;
        // FastDDS mirrors its recompute of the request. Both compute
        // hash_c1 over the `c.*` properties in WIRE order — since ZeroDDS
        // emits the request in hash order (c.id, c.perm, c.pdata, c.dsign_algo,
        // c.kagree_algo), the echo matches st.hash_c1 again.
        if let Some(reply_hash_c1) = reply.hash_c1 {
            if !ct_eq(&reply_hash_c1, &st.hash_c1) {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "reply: hash_c1 echo mismatch (cert-bind broken)",
                ));
            }
        }
        if let Some(reply_dh1) = &reply.dh1 {
            if !ct_eq(reply_dh1, &st.dh1) {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "reply: dh1 echo mismatch",
                ));
            }
        }
        if !ct_eq(&reply.challenge1, &st.challenge1) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: challenge1 echo mismatch",
            ));
        }
        // Table 59 (process_handshake on the initiator): with a remote
        // AuthRequestMessageToken, its future_challenge "is the same value as
        // the property named challenge2 found in the handshake_message_in".
        if let Some(future) = self.remote_futures.get(&st.remote) {
            if !ct_eq(future, &reply.challenge2) {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "reply: challenge2 is not the future_challenge of the AuthRequest",
                ));
            }
        }
        // Echo comparison NUL-tolerant: `st.kagree_algo` keeps the (OpenDDS)
        // NUL for hash_c1 consistency, `reply.kagree_algo` is already
        // NUL-stripped on parsing (take_bin_string). The algorithm name
        // itself must match.
        if reply.kagree_algo.trim_end_matches('\0') != st.kagree_algo.trim_end_matches('\0') {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: kagree_algo mismatch",
            ));
        }

        // b) Validate the replier cert.
        // Snapshot of the local identity so that we can borrow it
        // mutably later (store_secret).
        let remote_acceptance = {
            let parsed = self.identities.get(&st.local).ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::Internal, "pki: initiator identity gone")
            })?;
            let (acceptance, ocsp) = parsed
                .verify_remote_der_checked(&cid_to_der(&reply.cert_der), &reply.ocsp_status)
                .map_err(pki_to_security)?;
            (acceptance, ocsp, parsed.participant_guid)
        };
        let (remote_acceptance, remote_ocsp, initiator_guid) = remote_acceptance;
        self.log_cert_acceptance(&remote_acceptance, initiator_guid, "remote replier");
        self.log_ocsp_outcome(&remote_ocsp, initiator_guid, "remote replier");
        // The replier's GUID is bound to its identity as the initiator's is
        // (§7.4.3); Cyclone checks the reply's `c.pdata` as the request's.
        check_pdata(
            &reply.pdata,
            &cid_to_der(&reply.cert_der),
            self.remote_guids.get(&st.remote),
        )?;
        let replier_key_algo = detect_peer_algo(&cid_to_der(&reply.cert_der));
        let replier_scheme = check_dsign_matches(&reply.dsign_algo, replier_key_algo)?;

        // c) Verify the replier signature (§9.3.2.5.2.2): BinaryPropertySeq{
        //    hash_c2, ch2, dh2, ch1, dh1, hash_c1 }. dh1/hash_c1 = the values
        //    sent by the initiator (= st.dh1/st.hash_c1); the replier
        //    signs over them, in the reply itself they are optionally omitted.
        //    hash_c2 is the value recomputed from the reply credentials.
        let to_verify = ht::reply_signing_bytes(
            &reply.hash_c2,
            &reply.challenge2,
            &reply.dh2,
            &reply.challenge1,
            &st.dh1,
            &st.hash_c1,
        );
        verify_signature_with_cert(
            &cid_to_der(&reply.cert_der),
            replier_scheme,
            &to_verify,
            &reply.signature,
            self.cert_policy.accept_x509v1_identity_certificate,
        )?;

        let st = self.pending_initiator.remove(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "pki: initiator state gone")
        })?;

        // d) DH agreement.
        let kx = st.kx.ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::Internal, "pki: ephemeral kx gone")
        })?;
        let raw = kx.derive_shared_secret(&reply.dh2)?;
        let final_secret = derive_shared_secret(&raw)?;

        // challenge1 = own initiator value (st), challenge2 = replier (reply).
        let secret_handle =
            self.store_secret(handshake, final_secret, st.challenge1, reply.challenge2);
        self.handshake_to_secret.insert(handshake, secret_handle);
        // The replier's identity and permissions document travelled in the
        // reply (Table 54).
        self.peer_credentials.insert(
            handshake,
            PeerCredential {
                c_id: reply.cert_der.clone(),
                c_perm: reply.permissions.clone(),
            },
        );

        // e) Build + sign the final token (§9.3.2.5.2.3): the initiator signs
        //    over BinaryPropertySeq{ hash_c1, ch1, dh1, ch2, dh2, hash_c2 }.
        let to_sign = ht::final_signing_bytes(
            &st.hash_c1,
            &reply.challenge1,
            &st.dh1,
            &reply.challenge2,
            &reply.dh2,
            &reply.hash_c2,
        );
        let signature = {
            let parsed = self.identities.get(&st.local).ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::Internal, "pki: initiator identity gone")
            })?;
            sign_with_identity(parsed, st.scheme, &to_sign)?
        };
        // The final token echoes hash_c1/dh1 with the OWN initiator values
        // (st), not the ones optionally omitted in the reply; hash_c2 is the
        // value recomputed from the reply.
        let final_token = ht::build_final_token(&FinalBuildInput {
            version: reply.version,
            hash_c1: &st.hash_c1,
            hash_c2: &reply.hash_c2,
            dh1: &st.dh1,
            dh2: &reply.dh2,
            challenge1: &reply.challenge1,
            challenge2: &reply.challenge2,
            ocsp_status: &[],
            signature: &signature,
        })?;

        // Table 59: "On success the operation shall return
        // VALIDATION_OK_FINAL_MESSAGE": the initiator sends the final token
        // and is complete.
        Ok(HandshakeStepOutcome::SendFinalMessage { token: final_token })
    }

    fn process_final_on_replier(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let final_tok = ht::parse_final_token(token)?;
        // The state stays until the final has passed its checks (§9.3.2.11.6):
        // a forged final took it out, and the genuine one found none. A
        // failed final hands out no SharedSecret.
        {
            let st = self.pending_replier.get(&handshake).ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::BadArgument, "pki: replier state gone")
            })?;
            Self::check_final(
                &final_tok,
                st,
                self.cert_policy.accept_x509v1_identity_certificate,
            )?;
        }
        let st = self.pending_replier.remove(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "pki: replier state gone")
        })?;
        self.handshake_to_secret.insert(handshake, st.secret_handle);
        // The initiator proved its key: its credential is the peer's now.
        self.peer_credentials.insert(handshake, st.peer);

        // We take the local handle only because of the `local` field, so the
        // lint does not complain; in production we could use it for
        // audit logging.
        let _ = st.local;
        Ok(HandshakeStepOutcome::Complete {
            secret: st.secret_handle,
        })
    }

    /// The replier's checks of the final token: the echoed values and the
    /// initiator's signature. `accept_x509v1` as in
    /// [`verify_signature_with_cert`].
    fn check_final(
        final_tok: &ht::FinalTokenView,
        st: &ReplierState,
        accept_x509v1: bool,
    ) -> SecurityResult<()> {
        // Echo consistency. hash_c1, hash_c2, dh1 and dh2 are optional in
        // the final (§10.3.2.5.3 Table 58); one that is sent must match.
        let echo = |sent: Option<&[u8]>, own: &[u8]| sent.is_none_or(|v| ct_eq(v, own));
        if !echo(final_tok.hash_c1.as_ref().map(|v| &v[..]), &st.hash_c1)
            || !echo(final_tok.hash_c2.as_ref().map(|v| &v[..]), &st.hash_c2)
            || !echo(final_tok.dh1.as_deref(), &st.dh1)
            || !echo(final_tok.dh2.as_deref(), &st.dh2)
            || !ct_eq(&final_tok.challenge1, &st.challenge1)
            || !ct_eq(&final_tok.challenge2, &st.challenge2)
        {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "final: echo mismatch",
            ));
        }

        // Initiator signature (§9.3.2.5.2.3): BinaryPropertySeq{ hash_c1, ch1,
        // dh1, ch2, dh2, hash_c2 }.
        let to_verify = ht::final_signing_bytes(
            &st.hash_c1,
            &st.challenge1,
            &st.dh1,
            &st.challenge2,
            &st.dh2,
            &st.hash_c2,
        );
        verify_signature_with_cert(
            &cid_to_der(&st.initiator_cert_der),
            st.initiator_scheme,
            &to_verify,
            &final_tok.signature,
            accept_x509v1,
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::properties::Property;

    /// `c.id` with the `openssl x509 -text` human dump prepended before the
    /// PEM (CoreDX 6.14's handshake form, measured 2026-09-14) yields the same
    /// DER as the pure-PEM form. Before the fix the leading text was fed to
    /// the DER parser and its first byte `C` (0x43) read as an APPLICATION[3]
    /// tag, failing every handshake against CoreDX.
    #[test]
    fn cid_to_der_extracts_a_pem_block_after_a_text_dump() {
        let (_ca, cert_pem, _key) = make_signed_cert_ca_key();
        let pure = cid_to_der(&cert_pem);
        assert!(
            !pure.is_empty() && pure[0] == 0x30,
            "pure PEM must yield DER"
        );
        let mut prefixed = b"Certificate:\n    Data:\n        Version: 1 (0x0)\n".to_vec();
        prefixed.extend_from_slice(&cert_pem);
        assert_eq!(
            cid_to_der(&prefixed),
            pure,
            "text-prefixed c.id yields the same DER"
        );
        // A trailing NUL (OpenDDS) on top of the text prefix still works.
        prefixed.push(0);
        assert_eq!(cid_to_der(&prefixed), pure, "trailing NUL tolerated");
    }

    /// Creates a CA + end-entity cert + the matching private key
    /// (PKCS8-PEM) — all for the rcgen default ECDSA-P256.
    fn make_signed_cert_ca_key() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use rcgen::{CertificateParams, KeyPair};

        let mut ca_params = CertificateParams::new(vec!["ZeroDDS Test CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem();

        let mut ee_params = CertificateParams::new(vec!["zerodds-node".into()]).unwrap();
        ee_params.is_ca = rcgen::IsCa::NoCa;
        let ee_key = KeyPair::generate().unwrap();
        let ee_cert = ee_params.signed_by(&ee_key, &ca_cert, &ca_key).unwrap();
        let ee_pem = ee_cert.pem();
        let ee_key_pem = ee_key.serialize_pem();

        (
            ee_pem.into_bytes(),
            ca_pem.into_bytes(),
            ee_key_pem.into_bytes(),
        )
    }

    fn make_cert_with_wrong_ca() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use rcgen::{CertificateParams, KeyPair};

        let mut trusted_ca_params = CertificateParams::new(vec!["Trusted CA".into()]).unwrap();
        trusted_ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let trusted_ca_key = KeyPair::generate().unwrap();
        let trusted_ca_cert = trusted_ca_params.self_signed(&trusted_ca_key).unwrap();

        let mut rogue_ca_params = CertificateParams::new(vec!["Rogue CA".into()]).unwrap();
        rogue_ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let rogue_ca_key = KeyPair::generate().unwrap();
        let rogue_ca_cert = rogue_ca_params.self_signed(&rogue_ca_key).unwrap();

        let mut ee_params = CertificateParams::new(vec!["impersonator".into()]).unwrap();
        ee_params.is_ca = rcgen::IsCa::NoCa;
        let ee_key = KeyPair::generate().unwrap();
        let ee_cert = ee_params
            .signed_by(&ee_key, &rogue_ca_cert, &rogue_ca_key)
            .unwrap();

        (
            ee_cert.pem().into_bytes(),
            trusted_ca_cert.pem().into_bytes(),
            ee_key.serialize_pem().into_bytes(),
        )
    }

    /// Alice and Bob with the request and the reply exchanged: Alice's
    /// handshake, Bob's, and the final token Alice sends, which Bob has not
    /// processed yet.
    fn up_to_the_final() -> (
        PkiAuthenticationPlugin,
        PkiAuthenticationPlugin,
        HandshakeHandle,
        HandshakeHandle,
        Vec<u8>,
    ) {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out1 else {
            panic!("expected SendMessage");
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out2 else {
            panic!("expected SendMessage");
        };
        let HandshakeStepOutcome::SendFinalMessage { token: final_token } =
            alice.process_handshake(alice_hs, &reply).unwrap()
        else {
            panic!("alice expected to send final token");
        };
        (alice, bob, alice_hs, bob_hs, final_token)
    }

    fn alice_bob() -> (
        PkiAuthenticationPlugin,
        PkiAuthenticationPlugin,
        IdentityHandle,
        IdentityHandle,
        IdentityHandle,
        IdentityHandle,
    ) {
        let (a_cert, ca, a_key) = make_signed_cert_ca_key();
        let (b_cert, _ca2, b_key) = make_signed_cert_ca_key();
        // Both use their own CAs, but we would have to pack both into the
        // peer's trust bundle for the cert chain to be
        // valid. Instead: both take Alice's CA as the
        // trust anchor and Bob's cert is faked under this CA.
        // Simpler: regenerate both with a shared CA.
        let _ = (b_cert, b_key);

        // Cleaner approach: shared CA.
        use rcgen::{CertificateParams, KeyPair};
        let mut ca_params = CertificateParams::new(vec!["Common CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem().into_bytes();

        let alice_key_pair = KeyPair::generate().unwrap();
        let bob_key_pair = KeyPair::generate().unwrap();
        let issue = |name: &str, key: &KeyPair| {
            let mut params = CertificateParams::new(vec![name.into()]).unwrap();
            // The SubjectName: the bound GUID prefix starts with its hash.
            let mut dn = rcgen::DistinguishedName::new();
            dn.push(rcgen::DnType::CommonName, name);
            params.distinguished_name = dn;
            params.is_ca = rcgen::IsCa::NoCa;
            let cert = params.signed_by(key, &ca_cert, &ca_key).unwrap();
            cert.pem().into_bytes()
        };
        // Table 59: the participant with the smaller GUID starts the
        // handshake; Alice is the one that is smaller.
        let names = if bound_guid(&issue("alice", &alice_key_pair))
            < bound_guid(&issue("bob", &bob_key_pair))
        {
            ("alice", "bob")
        } else {
            ("bob", "alice")
        };
        let alice_cert_pem = issue(names.0, &alice_key_pair);
        let alice_key_pem = alice_key_pair.serialize_pem().into_bytes();
        let bob_cert_pem = issue(names.1, &bob_key_pair);
        let bob_key_pem = bob_key_pair.serialize_pem().into_bytes();

        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        let alice_guid = bound_guid(&alice_cert_pem);
        let bob_guid = bound_guid(&bob_cert_pem);
        alice.set_local_participant_data(pdata_of(alice_guid));
        bob.set_local_participant_data(pdata_of(bob_guid));
        let alice_h = alice
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: alice_cert_pem.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(alice_key_pem),
                },
                alice_guid,
            )
            .unwrap();
        let alice_remote_for_bob = alice
            .validate_remote_identity(alice_h, bob_guid, &identity_token_of(&bob_cert_pem), None)
            .map(|v| v.identity)
            .unwrap();
        let bob_h = bob
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: bob_cert_pem.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(bob_key_pem),
                },
                bob_guid,
            )
            .unwrap();
        let bob_remote_for_alice = bob
            .validate_remote_identity(bob_h, alice_guid, &identity_token_of(&alice_cert_pem), None)
            .map(|v| v.identity)
            .unwrap();
        let _ = (a_cert, ca, a_key);
        (
            alice,
            bob,
            alice_h,
            alice_remote_for_bob,
            bob_h,
            bob_remote_for_alice,
        )
    }

    /// Two plugins that trust one CA, with the identity keys given.
    fn alice_bob_with_keys(
        alice_key: rcgen::KeyPair,
        bob_key: rcgen::KeyPair,
    ) -> (
        PkiAuthenticationPlugin,
        PkiAuthenticationPlugin,
        IdentityHandle,
        IdentityHandle,
        IdentityHandle,
        IdentityHandle,
    ) {
        use rcgen::{CertificateParams, KeyPair};
        let mut ca_params = CertificateParams::new(vec!["Common CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem().into_bytes();
        let issue = |name: &str, key: &KeyPair| {
            let mut params = CertificateParams::new(vec![name.into()]).unwrap();
            // The SubjectName: the bound GUID prefix starts with its hash.
            let mut dn = rcgen::DistinguishedName::new();
            dn.push(rcgen::DnType::CommonName, name);
            params.distinguished_name = dn;
            params.is_ca = rcgen::IsCa::NoCa;
            let cert = params.signed_by(key, &ca_cert, &ca_key).unwrap();
            (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
        };
        // Table 59: the participant with the smaller GUID starts the
        // handshake; Alice is the one that is smaller.
        let names =
            if bound_guid(&issue("alice", &alice_key).0) < bound_guid(&issue("bob", &bob_key).0) {
                ("alice", "bob")
            } else {
                ("bob", "alice")
            };
        let (alice_cert_pem, alice_key_pem) = issue(names.0, &alice_key);
        let (bob_cert_pem, bob_key_pem) = issue(names.1, &bob_key);
        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        let alice_guid = bound_guid(&alice_cert_pem);
        let bob_guid = bound_guid(&bob_cert_pem);
        alice.set_local_participant_data(pdata_of(alice_guid));
        bob.set_local_participant_data(pdata_of(bob_guid));
        let alice_h = alice
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: alice_cert_pem.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(alice_key_pem),
                },
                alice_guid,
            )
            .unwrap();
        let alice_remote_for_bob = alice
            .validate_remote_identity(alice_h, bob_guid, &identity_token_of(&bob_cert_pem), None)
            .map(|v| v.identity)
            .unwrap();
        let bob_h = bob
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: bob_cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(bob_key_pem),
                },
                bob_guid,
            )
            .unwrap();
        let bob_remote_for_alice = bob
            .validate_remote_identity(bob_h, alice_guid, &identity_token_of(&alice_cert_pem), None)
            .map(|v| v.identity)
            .unwrap();
        (
            alice,
            bob,
            alice_h,
            alice_remote_for_bob,
            bob_h,
            bob_remote_for_alice,
        )
    }

    /// Runs the three messages; returns the `c.dsign_algo` of the request
    /// and of the reply, and the two secrets.
    fn run_handshake_for(
        alice: &mut PkiAuthenticationPlugin,
        bob: &mut PkiAuthenticationPlugin,
        ids: (
            IdentityHandle,
            IdentityHandle,
            IdentityHandle,
            IdentityHandle,
        ),
    ) -> (String, String, Vec<u8>, Vec<u8>) {
        let (alice_h, alice_remote_bob, bob_h, bob_remote_alice) = ids;
        let dsign = |token: &[u8]| {
            let h = zerodds_security::token::DataHolder::from_cdr_le(token).unwrap();
            String::from_utf8_lossy(h.binary_property(ht::prop::C_DSIGN_ALGO).unwrap())
                .trim_end_matches('\0')
                .to_string()
        };
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out1 else {
            panic!("request");
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out2 else {
            panic!("reply");
        };
        let HandshakeStepOutcome::SendFinalMessage { token: fin } =
            alice.process_handshake(alice_hs, &reply).unwrap()
        else {
            panic!("final");
        };
        let HandshakeStepOutcome::Complete { secret } =
            bob.process_handshake(bob_hs, &fin).unwrap()
        else {
            panic!("complete");
        };
        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        (
            dsign(&req),
            dsign(&reply),
            alice.secret_bytes(alice_secret).unwrap().to_vec(),
            bob.secret_bytes(secret).unwrap().to_vec(),
        )
    }

    /// DDS-Security 1.2 §8.2 Table 25: `ECDSA+P384+SHA384` signs and verifies
    /// the handshake of a P-384 identity; the name is that of 1.2 only.
    #[test]
    fn a_p384_identity_authenticates_with_ecdsa_p384() {
        let key = || rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P384_SHA384).unwrap();
        let (mut alice, mut bob, ah, ar, bh, br) = alice_bob_with_keys(key(), key());
        let (req, reply, a, b) = run_handshake_for(&mut alice, &mut bob, (ah, ar, bh, br));
        assert_eq!(req, "ECDSA+P384+SHA384");
        assert_eq!(reply, "ECDSA+P384+SHA384");
        assert_eq!(a, b);
    }

    /// X.509 v1 identity certificates accepted under the deviation
    /// ([`crate::x509v1_compat`]; the OMG dds-security suite's certificates of
    /// RTI Connext and Fast DDS are v1) also verify the handshake signatures:
    /// the reply's at the initiator, the final's at the replier. The check
    /// parsed the certificate with `webpki`, which takes v3 only, and every
    /// handshake with RTI Connext 7.7.0 as initiator ended at the final with
    /// `UnsupportedCertVersion`. Without the deviation the check refuses a v1
    /// certificate as before.
    #[test]
    fn a_handshake_of_x509_v1_certificates_completes_under_the_deviation() {
        use crate::x509v1_compat::testonly_v1_certs::{ca, v1_end_entity};
        const YEAR_2000: u64 = 946_684_800;
        const YEAR_2100: u64 = 4_102_444_800;
        let ca = ca("ZeroDDS Test Identity CA");
        let first = v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);
        let second = v1_end_entity(&ca, &ca, "bob", YEAR_2000, YEAR_2100);
        // Table 59: the participant with the smaller GUID starts.
        let (a, b) = if bound_guid(&first.0) < bound_guid(&second.0) {
            (first, second)
        } else {
            (second, first)
        };
        let (a_guid, b_guid) = (bound_guid(&a.0), bound_guid(&b.0));
        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        alice.set_accept_x509v1_identity_certificates(true);
        bob.set_accept_x509v1_identity_certificates(true);
        alice.set_local_participant_data(pdata_of(a_guid));
        bob.set_local_participant_data(pdata_of(b_guid));
        let a_der = cert_der_from_pem(&a.0);
        let alice_h = alice
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a.0.clone(),
                    identity_ca_pem: ca.cert_pem.clone(),
                    identity_key_pem: Some(a.1),
                },
                a_guid,
            )
            .unwrap();
        let bob_h = bob
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: b.0.clone(),
                    identity_ca_pem: ca.cert_pem.clone(),
                    identity_key_pem: Some(b.1),
                },
                b_guid,
            )
            .unwrap();
        let alice_remote_bob = alice
            .validate_remote_identity(alice_h, b_guid, &identity_token_of(&b.0), None)
            .map(|v| v.identity)
            .unwrap();
        let bob_remote_alice = bob
            .validate_remote_identity(bob_h, a_guid, &identity_token_of(&a.0), None)
            .map(|v| v.identity)
            .unwrap();
        let (_, _, alice_secret, bob_secret) = run_handshake_for(
            &mut alice,
            &mut bob,
            (alice_h, alice_remote_bob, bob_h, bob_remote_alice),
        );
        assert_eq!(alice_secret, bob_secret);

        let refused =
            verify_signature_with_cert(&a_der, DsignScheme::EcdsaP256, b"m", &[0; 64], false)
                .unwrap_err();
        assert!(
            alloc::format!("{refused}").contains("UnsupportedCertVersion"),
            "{refused}"
        );
    }

    /// DDS-Security 1.2 §8.2 Table 25: the salt length of RSASSA-PSS "is not
    /// specified" and the validation "shall detect the salt length from the
    /// signature". The handshake signature of a peer that signs with a salt of
    /// 0, 20, 32, 64 or 222 octets (OpenSSL made them) verifies against its
    /// certificate; a signature over another message does not. `webpki`'s
    /// verifier takes 32 octets only.
    #[test]
    fn an_rsa_pss_handshake_signature_of_any_salt_length_verifies() {
        const CERT: &[u8] =
            include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/cert2048.der");
        const MSG: &[u8] = include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/msg.bin");
        let sigs: [(usize, &[u8]); 5] = [
            (
                0,
                include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_0.bin"),
            ),
            (
                20,
                include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_20.bin"),
            ),
            (
                32,
                include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_32.bin"),
            ),
            (
                64,
                include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_64.bin"),
            ),
            (
                222,
                include_bytes!("../../security-crypto/tests/fixtures/rsa_pss/sig2048_222.bin"),
            ),
        ];
        for (salt, sig) in sigs {
            verify_signature_with_cert(CERT, DsignScheme::RsaPss, MSG, sig, false)
                .unwrap_or_else(|e| panic!("salt {salt}: {e}"));
            assert!(
                verify_signature_with_cert(
                    CERT,
                    DsignScheme::RsaPss,
                    b"another message",
                    sig,
                    false
                )
                .is_err(),
                "salt {salt}"
            );
            verify_der_signature(CERT, MSG, sig).unwrap_or_else(|e| panic!("salt {salt}: {e}"));
        }
    }

    /// §8.2 Table 25: an RSA identity signs `RSASSA-PSS-SHA256` by default and
    /// `RSASSA-PKCS1-V1_5+2048+SHA256` when the peer announced that it
    /// verifies only that one (§7.3.11); the replier follows the initiator.
    #[test]
    fn an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that() {
        let rsa = || {
            rcgen::KeyPair::try_from(
                include_bytes!("../tests/fixtures/rsa_2048_test_pkcs8.der").as_slice(),
            )
            .unwrap()
        };
        let (mut alice, mut bob, ah, ar, bh, br) = alice_bob_with_keys(rsa(), rsa());
        let (req, reply, a, b) = run_handshake_for(&mut alice, &mut bob, (ah, ar, bh, br));
        assert_eq!(req, "RSASSA-PSS-MGF1SHA256+2048+SHA256");
        assert_eq!(reply, "RSASSA-PSS-MGF1SHA256+2048+SHA256");
        assert_eq!(a, b);
        // Towards a peer that reads the names of 1.1.
        let (mut alice, mut bob, ah, ar, bh, br) = alice_bob_with_keys(rsa(), rsa());
        alice.set_algorithm_names(AlgorithmNames::Auto);
        alice.set_peer_algorithm_names(PeerAlgorithmNames::Legacy);
        let (req, reply, a, b) = run_handshake_for(&mut alice, &mut bob, (ah, ar, bh, br));
        assert_eq!(req, "RSASSA-PSS-SHA256");
        assert_eq!(reply, "RSASSA-PSS-SHA256");
        assert_eq!(a, b);

        let (mut alice, mut bob, ah, ar, bh, br) = alice_bob_with_keys(rsa(), rsa());
        alice.set_remote_message_auth_supported(Some(BIT_RSASSA_PKCS1));
        let (req, reply, a, b) = run_handshake_for(&mut alice, &mut bob, (ah, ar, bh, br));
        assert_eq!(req, "RSASSA-PKCS1-V1_5+2048+SHA256");
        assert_eq!(reply, "RSASSA-PKCS1-V1_5+2048+SHA256");
        assert_eq!(a, b);
    }

    /// DDS-Security 1.2 §8.2 Table 25, §8.3 Table 26: the initiator sends the
    /// names of 1.2 (`ECDSA+P256+SHA256`, `ECDHE-CEUM+P256`,
    /// `DHE+MODP-2048-256`) to a peer whose IdentityToken names version 1.2,
    /// the deprecated names of 1.1 (`ECDSA-SHA256`, `ECDH+prime256v1-CEUM`,
    /// `DH+MODP-2048-256`) to a peer of 1.0 (Cyclone DDS, Fast DDS, OpenDDS) with `auto`, and
    /// whichever the setting forces; the reply echoes the request.
    #[test]
    fn the_algorithm_names_follow_the_version_of_the_peer_and_the_setting() {
        let names = |token: &[u8]| {
            let h = zerodds_security::token::DataHolder::from_cdr_le(token).unwrap();
            let get = |p: &str| {
                String::from_utf8_lossy(h.binary_property(p).unwrap())
                    .trim_end_matches('\0')
                    .to_string()
            };
            (get(ht::prop::C_DSIGN_ALGO), get(ht::prop::C_KAGREE_ALGO))
        };
        let v1_2 = ("ECDSA+P256+SHA256", "ECDHE-CEUM+P256");
        let v1_1 = ("ECDSA-SHA256", "ECDH+prime256v1-CEUM");
        for (peer, setting, expected) in [
            (ht::ClassVersion::V1_0, AlgorithmNames::default(), v1_2),
            (ht::ClassVersion::V1_2, AlgorithmNames::default(), v1_2),
            (ht::ClassVersion::V1_0, AlgorithmNames::Auto, v1_1),
            (ht::ClassVersion::V1_2, AlgorithmNames::Auto, v1_2),
            (ht::ClassVersion::V1_0, AlgorithmNames::V1_2, v1_2),
            (ht::ClassVersion::V1_2, AlgorithmNames::V1_1, v1_1),
        ] {
            let (mut alice, mut bob, ah, ar, bh, br) = alice_bob();
            alice.remote_versions.insert(ar, peer);
            alice.set_algorithm_names(setting);
            let (hs_a, out) = alice
                .begin_handshake_request(ah, ar, &alice.test_pdata())
                .unwrap();
            let HandshakeStepOutcome::SendMessage { token: req } = out else {
                panic!("request");
            };
            assert_eq!(
                names(&req),
                (expected.0.into(), expected.1.into()),
                "{peer:?} {setting:?}"
            );
            let (hs_b, out) = bob
                .begin_handshake_reply(bh, br, &req, &bob.test_pdata())
                .unwrap();
            let HandshakeStepOutcome::SendMessage { token: reply } = out else {
                panic!("reply");
            };
            assert_eq!(
                names(&reply).0,
                expected.0,
                "the replier follows the initiator"
            );
            assert_eq!(names(&reply).1, expected.1);
            let HandshakeStepOutcome::SendFinalMessage { token: fin } =
                alice.process_handshake(hs_a, &reply).unwrap()
            else {
                panic!("final");
            };
            assert!(matches!(
                bob.process_handshake(hs_b, &fin).unwrap(),
                HandshakeStepOutcome::Complete { .. }
            ));
        }
        // MODP: DHE+MODP-2048-256 to a peer of 1.2.
        let (mut alice, _bob, ah, ar, _, _) = alice_bob();
        alice.remote_versions.insert(ar, ht::ClassVersion::V1_2);
        alice.set_preferred_kx_suite(KxSuite::DhModp2048_256);
        let HandshakeStepOutcome::SendMessage { token: req } = alice
            .begin_handshake_request(ah, ar, &alice.test_pdata())
            .unwrap()
            .1
        else {
            panic!("request");
        };
        assert_eq!(names(&req).1, "DHE+MODP-2048-256");
        // The setting reads auto, 1.2 and 1.1 and refuses the rest.
        assert_eq!(AlgorithmNames::default(), AlgorithmNames::V1_2);
        assert_eq!(AlgorithmNames::parse("auto").unwrap(), AlgorithmNames::Auto);
        assert_eq!(AlgorithmNames::parse("1.2").unwrap(), AlgorithmNames::V1_2);
        assert_eq!(AlgorithmNames::parse("1.1").unwrap(), AlgorithmNames::V1_1);
        assert!(AlgorithmNames::parse("1.0").is_err());
        assert!(AlgorithmNames::parse("").is_err());
    }

    /// DDS-Security 1.2 §8.3 Table 26: `DHE+MODP-2048-256` runs the handshake
    /// to the same SharedSecret on both sides; the request names it (in the
    /// name of 1.1, as the other vendors do), the replier echoes it.
    #[test]
    fn the_handshake_runs_over_dhe_modp_2048_256() {
        let (mut alice, mut bob, ah, ar, bh, br) = alice_bob();
        alice.set_preferred_kx_suite(KxSuite::DhModp2048_256);
        // Towards a peer that reads the names of 1.1.
        alice.set_algorithm_names(AlgorithmNames::Auto);
        alice.set_peer_algorithm_names(PeerAlgorithmNames::Legacy);
        let kagree = |token: &[u8]| {
            let h = zerodds_security::token::DataHolder::from_cdr_le(token).unwrap();
            String::from_utf8_lossy(h.binary_property(ht::prop::C_KAGREE_ALGO).unwrap())
                .trim_end_matches('\0')
                .to_string()
        };
        let (hs_a, out) = alice
            .begin_handshake_request(ah, ar, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out else {
            panic!("request");
        };
        assert_eq!(kagree(&req), "DH+MODP-2048-256");
        let (hs_b, out) = bob
            .begin_handshake_reply(bh, br, &req, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out else {
            panic!("reply");
        };
        assert_eq!(kagree(&reply), "DH+MODP-2048-256");
        let HandshakeStepOutcome::SendFinalMessage { token: fin } =
            alice.process_handshake(hs_a, &reply).unwrap()
        else {
            panic!("final");
        };
        let HandshakeStepOutcome::Complete { secret } = bob.process_handshake(hs_b, &fin).unwrap()
        else {
            panic!("complete");
        };
        let a = alice.shared_secret(hs_a).unwrap();
        assert_eq!(
            alice.secret_bytes(a).unwrap(),
            bob.secret_bytes(secret).unwrap()
        );
    }

    /// The scheme a `c.dsign_algo` names has to fit the key of the
    /// certificate that came with it.
    #[test]
    fn a_dsign_algo_must_fit_the_certificate() {
        assert_eq!(
            check_dsign_matches("ECDSA+P384+SHA384", CertKeyAlgo::EcdsaP384Sha384).unwrap(),
            DsignScheme::EcdsaP384
        );
        assert!(check_dsign_matches("ECDSA+P384+SHA384", CertKeyAlgo::EcdsaP256Sha256).is_err());
        assert!(check_dsign_matches("ECDSA+P256+SHA256", CertKeyAlgo::EcdsaP384Sha384).is_err());
        assert!(
            check_dsign_matches(
                "RSASSA-PKCS1-V1_5+2048+SHA256",
                CertKeyAlgo::EcdsaP256Sha256
            )
            .is_err()
        );
        assert_eq!(
            check_dsign_matches("RSASSA-PKCS1-V1_5+2048+SHA256\0", CertKeyAlgo::RsaPssSha256)
                .unwrap(),
            DsignScheme::RsaPkcs1
        );
        assert!(check_dsign_matches("rot13", CertKeyAlgo::RsaPssSha256).is_err());
    }

    /// A participant GUID bound to the identity in `cert_pem` (§10.3.3
    /// Table 59, `validate_local_identity`).
    fn bound_guid(cert_pem: &[u8]) -> [u8; 16] {
        let prefix = crate::guid_adjust::adjust_participant_guid_prefix(
            &[0x01; 16],
            &cert_der_from_pem(cert_pem),
        )
        .unwrap();
        let mut guid = [0u8; 16];
        guid[..12].copy_from_slice(&prefix);
        guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
        guid
    }

    /// A `c.pdata`: PL_CDR_BE with PID_PARTICIPANT_GUID and PID_SENTINEL.
    fn pdata_of(guid: [u8; 16]) -> Vec<u8> {
        let mut pdata = vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x50, 0x00, 0x10];
        pdata.extend_from_slice(&guid);
        pdata.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        pdata
    }

    /// The IdentityToken a peer announces: a DataHolder of the class
    /// `DDS:Auth:PKI-DH:1.0` (DDS-Security 1.2 §10.3.2.1).
    fn identity_token_of(_cert_pem: &[u8]) -> Vec<u8> {
        zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.0").to_cdr_le()
    }

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

    #[test]
    fn plugin_class_id_matches_spec() {
        let p = PkiAuthenticationPlugin::new();
        assert_eq!(p.plugin_class_id(), "DDS:Auth:PKI-DH:1.2");
    }

    #[test]
    fn validate_local_identity_accepts_ca_signed_cert() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let cfg = IdentityConfig {
            identity_cert_pem: cert_pem,
            identity_ca_pem: ca_pem,
            identity_key_pem: Some(key_pem),
        };
        let handle = plugin
            .validate_with_config(cfg, [0xAA; 16])
            .expect("signed cert must validate");
        assert_eq!(handle, IdentityHandle(1));
    }

    #[test]
    fn validate_local_identity_rejects_wrong_ca() {
        let (cert_pem, trusted_ca_pem, key_pem) = make_cert_with_wrong_ca();
        let mut plugin = PkiAuthenticationPlugin::new();
        let cfg = IdentityConfig {
            identity_cert_pem: cert_pem,
            identity_ca_pem: trusted_ca_pem,
            identity_key_pem: Some(key_pem),
        };
        let err = plugin
            .validate_with_config(cfg, [0xAA; 16])
            .expect_err("rogue-CA cert must not validate");
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// DDS-Security 1.2 §7.3.11, §7.3.12, §8.2, §8.3: the plugin names what it
    /// can verify and what it uses. An ECDSA P-256 identity signed by an
    /// ECDSA P-256 CA requires ECDSA P-256 for the chain, the handshake
    /// signature and (as the default key agreement) ECDHE-CEUM P-256.
    #[test]
    fn the_algorithm_masks_follow_the_identity_and_the_key_agreement() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let handle = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let unrestricted = ParticipantSecurityConfig::unrestricted();
        let masks = plugin
            .set_participant_security_config(handle, &unrestricted)
            .expect("masks");
        let (pss, pkcs1, p256, p384) = (1u32, 2u32, 4u32, 8u32);
        assert_eq!(masks.trust_chain, (pss | pkcs1 | p256 | p384, p256));
        assert_eq!(masks.message_auth, (pss | pkcs1 | p256 | p384, p256));
        // Table 26: bit 0 is DHE+MODP-2048-256, bit 1 ECDHE-CEUM+P256, bit 2
        // ECDHE-CEUM+P384.
        assert_eq!(masks.key_establishment, (0b111, 0b010));
        plugin.set_preferred_kx_suite(KxSuite::EcdhP384);
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &unrestricted)
                .unwrap()
                .key_establishment,
            (0b111, 0b100)
        );
        plugin.set_preferred_kx_suite(KxSuite::DhModp2048_256);
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &unrestricted)
                .unwrap()
                .key_establishment,
            (0b111, 0b001)
        );
        // A vendor extension has no bit in Table 26.
        plugin.set_preferred_kx_suite(KxSuite::X25519);
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &unrestricted)
                .unwrap()
                .key_establishment,
            (0b111, 0)
        );
        assert!(
            plugin
                .set_participant_security_config(IdentityHandle(999), &unrestricted)
                .is_err()
        );
    }

    /// §7.3.11: `trust_chain.required_mask` holds the signature algorithm of
    /// the permissions document this participant sends as well.
    #[test]
    fn the_required_trust_chain_mask_holds_the_algorithm_of_the_permissions_document() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let handle = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let (p256, p384, pkcs1) = (4u32, 8u32, 2u32);
        plugin.set_local_permissions_signature_algorithm("1.2.840.10045.4.3.3");
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &ParticipantSecurityConfig::unrestricted())
                .unwrap()
                .trust_chain
                .1,
            p256 | p384
        );
        plugin.set_local_permissions_signature_algorithm("1.2.840.113549.1.1.11");
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &ParticipantSecurityConfig::unrestricted())
                .unwrap()
                .trust_chain
                .1,
            p256 | pkcs1
        );
        plugin.set_local_permissions_signature_algorithm("1.2.840.113549.1.1.1");
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &ParticipantSecurityConfig::unrestricted())
                .unwrap()
                .trust_chain
                .1,
            p256 | pkcs1
        );
        // An algorithm Table 24 has no bit for adds none.
        plugin.set_local_permissions_signature_algorithm("1.2.3.4");
        assert_eq!(
            plugin
                .set_participant_security_config(handle, &ParticipantSecurityConfig::unrestricted())
                .unwrap()
                .trust_chain
                .1,
            p256
        );
    }

    #[test]
    fn validate_local_identity_rejects_empty_trust_anchors() {
        let (cert_pem, _, _) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let cfg = IdentityConfig {
            identity_cert_pem: cert_pem,
            identity_ca_pem: b"".to_vec(),
            identity_key_pem: None,
        };
        let err = plugin.validate_with_config(cfg, [0xAA; 16]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    #[test]
    fn validate_local_identity_via_property_list() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let cert_str = std::str::from_utf8(&cert_pem).unwrap().to_owned();
        let ca_str = std::str::from_utf8(&ca_pem).unwrap().to_owned();
        let key_str = std::str::from_utf8(&key_pem).unwrap().to_owned();
        let props = PropertyList::new()
            .with(Property::local(
                "dds.sec.auth.identity_certificate",
                cert_str,
            ))
            .with(Property::local("dds.sec.auth.identity_ca", ca_str))
            .with(Property::local("dds.sec.auth.private_key", key_str));
        let handle = plugin
            .validate_local_identity(0, &props, [0xAA; 16])
            .map(|v| v.identity)
            .expect("validate via props");
        assert!(handle.0 >= 1);
    }

    /// Table 50 — the `dds.sec.auth.*` names are the configuration
    /// contract for the builtin authentication plugin. `identity_certificate`
    /// and `identity_ca` are mandatory: dropping or misspelling either one
    /// must fail the local identity rather than fall back to a default.
    /// `private_key` is optional at validation time (a listen-only
    /// participant has none) but is read under exactly that name — an
    /// identity built without it cannot start a handshake.
    #[test]
    fn auth_property_names_are_each_required_and_exact() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let cert = std::str::from_utf8(&cert_pem).unwrap().to_owned();
        let ca = std::str::from_utf8(&ca_pem).unwrap().to_owned();
        let key = std::str::from_utf8(&key_pem).unwrap().to_owned();

        let mandatory = [
            "dds.sec.auth.identity_certificate",
            "dds.sec.auth.identity_ca",
        ];
        let full = || {
            PropertyList::new()
                .with(Property::local(
                    "dds.sec.auth.identity_certificate",
                    cert.clone(),
                ))
                .with(Property::local("dds.sec.auth.identity_ca", ca.clone()))
                .with(Property::local("dds.sec.auth.private_key", key.clone()))
        };

        PkiAuthenticationPlugin::new()
            .validate_local_identity(0, &full(), [0xAA; 16])
            .map(|v| v.identity)
            .expect("spec-named properties configure the plugin");

        for name in mandatory {
            // Missing entirely.
            let mut missing = full();
            missing.remove(name);
            let err = PkiAuthenticationPlugin::new()
                .validate_local_identity(0, &missing, [0xAA; 16])
                .map(|v| v.identity)
                .expect_err("a missing mandatory property must fail");
            assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration, "{name}");

            // Present, but under a near-miss name — must not be picked up.
            let mut typo = full();
            let value = typo.get(name).expect("value present").to_owned();
            typo.remove(name);
            typo.set(Property::local(alloc::format!("{name}_x"), value));
            let err = PkiAuthenticationPlugin::new()
                .validate_local_identity(0, &typo, [0xAA; 16])
                .map(|v| v.identity)
                .expect_err("a misspelled mandatory property must fail");
            assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration, "{name}");
        }

        // The private key is read under its spec name only. Spelled
        // correctly the identity carries a signing key; spelled
        // differently the same PEM is ignored and the identity has none,
        // so every handshake step that signs will refuse
        // (`InvalidConfiguration`, see `begin_handshake_reply`).
        let mut plugin = PkiAuthenticationPlugin::new();
        let with_key = plugin
            .validate_local_identity(0, &full(), [0xAA; 16])
            .map(|v| v.identity)
            .expect("identity with a key");
        assert!(
            plugin.identities[&with_key].private_key_pkcs8_der.is_some(),
            "dds.sec.auth.private_key must be consumed",
        );

        let mut typo_key = full();
        typo_key.remove("dds.sec.auth.private_key");
        typo_key.set(Property::local("dds.sec.auth.privatekey", key.clone()));
        let without_key = plugin
            .validate_local_identity(0, &typo_key, [0xBB; 16])
            .map(|v| v.identity)
            .expect("cert and CA alone still yield an identity");
        assert!(
            plugin.identities[&without_key]
                .private_key_pkcs8_der
                .is_none(),
            "a misspelled private-key property must not be picked up",
        );
    }

    // -------------------------------------------------------------
    // C3.1 — spec-conformant handshake (Tab.56/57/58)
    // -------------------------------------------------------------

    #[test]
    fn full_three_round_handshake_alice_bob() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();

        // 1) Alice → REQUEST.
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req_token = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };
        assert!(req_token.len() > 100, "request token contains cert + DH");

        // 2) Bob → REPLY.
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req_token, &bob.test_pdata())
            .unwrap();
        let reply_token = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        // 3) Alice → FINAL.
        let out3 = alice.process_handshake(alice_hs, &reply_token).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!("alice expected to send final token"),
        };

        // 4) Bob processes FINAL → Complete.
        let out4 = bob.process_handshake(bob_hs, &final_token).unwrap();
        let bob_secret = match out4 {
            HandshakeStepOutcome::Complete { secret } => secret,
            _ => panic!("expected Complete"),
        };

        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        let a_bytes = alice.secret_bytes(alice_secret).unwrap();
        let b_bytes = bob.secret_bytes(bob_secret).unwrap();
        assert_eq!(a_bytes.len(), 32);
        assert_eq!(
            a_bytes, b_bytes,
            "alice + bob must derive an identical secret"
        );
    }

    /// A stand-in for a token key: signs from PKCS#8 bytes through the
    /// same `sign_with` the in-memory path uses. It exercises
    /// `validate_with_config_and_signer` and the `sign_with_identity`
    /// branch on both the replier and the initiator side.
    struct MockSigner {
        pkcs8: Vec<u8>,
    }

    impl IdentitySigner for MockSigner {
        fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, PkiError> {
            sign_with(
                CertKeyAlgo::EcdsaP256Sha256,
                DsignScheme::EcdsaP256,
                &self.pkcs8,
                msg,
            )
            .map_err(|e| PkiError::SigningFailed(e.to_string()))
        }
    }

    /// A full three-round handshake where neither side holds its private
    /// key as PKCS#8 — both sign through an [`IdentitySigner`], the way a
    /// PKCS#11 token does (DDS-Security 1.2 §9.3.1 Table 44). The peer's
    /// signature check is unchanged, so completion proves the signer path
    /// produces verifiable signatures on both the REPLY and the FINAL.
    /// DDS-Security 1.2 §10.3.3 Table 59, `begin_handshake_reply`: "verify
    /// that the first bit of the participant_guid of the
    /// ParticipantBuiltinTopic data inside the 'c.pdata' is set to 1 and
    /// that the following 47 bits match the first 47 bits of the SHA-256
    /// hash of the SubjectName". A request whose `c.pdata` names a GUID not
    /// derived from its certificate is refused. It was accepted.
    #[test]
    fn a_request_with_a_pdata_guid_not_bound_to_its_certificate_is_refused() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        alice.set_local_participant_data(pdata_of([0x01; 16]));
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token } = out else {
            panic!("expected SendMessage");
        };
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &token, &bob.test_pdata())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// §7.4.3: the GUID is "tied to the Identity". A `c.pdata` with the
    /// requester's own bound GUID, while the requester was discovered under
    /// another GUID, is refused — the "squatter" announces a foreign GUID
    /// over SPDP.
    #[test]
    fn a_request_from_a_participant_discovered_under_another_guid_is_refused() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let mut squatted = pdata_participant_guid(&alice.local_pdata).unwrap();
        squatted[6] ^= 0xFF;
        // Bob discovered the requester under `squatted`.
        bob.remote_guids.insert(bob_remote_alice, squatted);
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token } = out else {
            panic!("expected SendMessage");
        };
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &token, &bob.test_pdata())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// The initiator checks the reply's `c.pdata` as the replier the
    /// request's (§7.4.3; Cyclone `validate_pdata` for both).
    #[test]
    fn a_reply_with_a_pdata_guid_not_bound_to_its_certificate_is_refused() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out1 else {
            panic!("expected SendMessage");
        };
        bob.set_local_participant_data(pdata_of([0x02; 16]));
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out2 else {
            panic!("expected SendMessage");
        };
        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// `c.pdata` is read as a big-endian parameter list from the first octet
    /// (Cyclone, Fast DDS), and with ZeroDDS' encapsulation header in front;
    /// without PID_PARTICIPANT_GUID, or cut short, it is refused.
    #[test]
    fn the_pdata_guid_is_read_from_the_parameter_list() {
        let guid = [7u8; 16];
        // ZeroDDS: encapsulation header, then the list.
        assert_eq!(pdata_participant_guid(&pdata_of(guid)).unwrap(), guid);
        // Cyclone and Fast DDS: the bare list, another parameter first.
        let mut bare = vec![0x00, 0x15, 0x00, 0x04, 2, 5, 0, 0];
        bare.extend_from_slice(&[0x00, 0x50, 0x00, 0x10]);
        bare.extend_from_slice(&guid);
        bare.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        assert_eq!(pdata_participant_guid(&bare).unwrap(), guid);
        let sentinel_only = [0x00, 0x01, 0x00, 0x00];
        assert!(pdata_participant_guid(&sentinel_only).is_err());
        assert!(pdata_participant_guid(&[]).is_err());
        let cut = [0x00, 0x50, 0x00, 0x10, 1, 2, 3];
        assert!(pdata_participant_guid(&cut).is_err());
    }

    /// DDS-Security 1.2 §9.3.2.5.2.3: the initiator proves the possession of
    /// its private key with the signature in the final token. The replier
    /// hands out no SharedSecret before that final; one with a forged
    /// signature fails and leaves none. The replier gave its secret out
    /// when it sent the reply, and the caller took the handshake as done.
    #[test]
    fn the_replier_has_no_shared_secret_before_a_verified_final() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req_token } = out1 else {
            panic!("expected SendMessage");
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req_token, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply_token } = out2 else {
            panic!("expected SendMessage");
        };
        assert!(
            bob.shared_secret(bob_hs).is_err(),
            "the replier handed out its secret before the final"
        );
        let HandshakeStepOutcome::SendFinalMessage { token: final_token } =
            alice.process_handshake(alice_hs, &reply_token).unwrap()
        else {
            panic!("alice expected to send final token");
        };

        let genuine = ht::parse_final_token(&final_token).unwrap();
        let mut signature = genuine.signature.clone();
        signature[0] ^= 0xFF;
        let forged = ht::build_final_token(&ht::FinalBuildInput {
            version: ht::ClassVersion::default(),
            hash_c1: &genuine.hash_c1.expect("ZeroDDS sends hash_c1"),
            hash_c2: &genuine.hash_c2.expect("ZeroDDS sends hash_c2"),
            dh1: genuine.dh1.as_deref().expect("ZeroDDS sends dh1"),
            dh2: genuine.dh2.as_deref().expect("ZeroDDS sends dh2"),
            challenge1: &genuine.challenge1,
            challenge2: &genuine.challenge2,
            ocsp_status: &genuine.ocsp_status,
            signature: &signature,
        })
        .unwrap();
        assert!(bob.process_handshake(bob_hs, &forged).is_err());
        assert!(
            bob.shared_secret(bob_hs).is_err(),
            "a forged final left a secret"
        );
        // §9.3.2.11.6: the forged final did not take the handshake down;
        // the genuine one completes it.
        assert!(matches!(
            bob.process_handshake(bob_hs, &final_token).unwrap(),
            HandshakeStepOutcome::Complete { .. }
        ));
        assert!(bob.shared_secret(bob_hs).is_ok());
    }

    /// DDS-Security 1.2 §10.3.3 Table 59, §10.3.2.4 Table 55: without a
    /// remote AuthRequestMessageToken, validate_remote_identity makes a local
    /// one, and its future_challenge is the challenge1 of the request; with
    /// one, the local token is TokenNIL. Neither was made, and challenge1 was
    /// a fresh NONCE.
    #[test]
    fn the_auth_request_announces_the_challenge1_of_the_request() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let bob_guid = alice.remote_guids[&alice_remote_bob];
        let validated = alice
            .validate_remote_identity(alice_h, bob_guid, &identity_token_of(&[]), None)
            .unwrap();
        let alice_remote_bob = validated.identity;
        let token = validated
            .local_auth_request_token
            .expect("no remote AuthRequest: a local one");
        let future = ht::parse_auth_request_token(&token).unwrap();
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out else {
            panic!("expected SendMessage");
        };
        assert_eq!(ht::parse_request_token(&req).unwrap().challenge1, future);

        let alice_guid = bob.remote_guids[&bob_remote_alice];
        let descriptor =
            zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.0").to_cdr_le();
        let validated = bob
            .validate_remote_identity(bob_h, alice_guid, &descriptor, Some(&token))
            .unwrap();
        let with_request = validated.identity;
        assert!(
            validated.local_auth_request_token.is_none(),
            "with a remote AuthRequest the local one is TokenNIL"
        );
        assert!(
            bob.begin_handshake_reply(bob_h, with_request, &req, &bob.test_pdata())
                .is_ok()
        );
    }

    /// Table 59, begin_handshake_reply: with a remote AuthRequestMessageToken
    /// its future_challenge must be the request's challenge1.
    #[test]
    fn a_request_whose_challenge1_is_not_the_announced_one_is_refused() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out else {
            panic!("expected SendMessage");
        };
        let alice_guid = bob.remote_guids[&bob_remote_alice];
        let descriptor =
            zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.0").to_cdr_le();
        let other = ht::build_auth_request_token(ht::ClassVersion::default(), &[0x5A; 32]);
        let with_request = bob
            .validate_remote_identity(bob_h, alice_guid, &descriptor, Some(&other))
            .map(|v| v.identity)
            .unwrap();
        let err = bob
            .begin_handshake_reply(bob_h, with_request, &req, &bob.test_pdata())
            .unwrap_err();
        assert!(err.detail.contains("future_challenge"), "{err}");
    }

    /// Table 59: an AuthRequestMessageToken of another class or without a
    /// 32-octet future_challenge fails validate_remote_identity.
    #[test]
    fn a_malformed_auth_request_fails_validate_remote_identity() {
        let (_alice, mut bob, _alice_h, _alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let alice_guid = bob.remote_guids[&bob_remote_alice];
        let descriptor =
            zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.0").to_cdr_le();
        let mut short = zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.0+AuthReq");
        short.set_binary_property(ht::prop::FUTURE_CHALLENGE, vec![1, 2, 3]);
        assert!(
            bob.validate_remote_identity(bob_h, alice_guid, &descriptor, Some(&short.to_cdr_le()))
                .map(|v| v.identity)
                .is_err()
        );
    }

    /// DDS-Security 1.2 §8.2 Table 25, §8.3 Table 26: the names of 1.2 and
    /// their deprecated names of 1.1 are taken; `X25519`, a vendor
    /// extension, only where this side runs it. The names of 1.2 were
    /// refused, and `X25519` was taken from any initiator.
    #[test]
    fn the_algorithm_names_of_1_2_and_1_1_are_taken() {
        for name in [
            "ECDH+prime256v1-CEUM",
            "ECDHE-CEUM+P256",
            "ECDHE-CEUM+P256\0",
        ] {
            assert_eq!(
                PkiAuthenticationPlugin::kx_suite_for_algo(name, false),
                Some(KxSuite::EcdhP256),
                "{name:?}"
            );
        }
        assert_eq!(
            PkiAuthenticationPlugin::kx_suite_for_algo("X25519", false),
            None
        );
        assert_eq!(
            PkiAuthenticationPlugin::kx_suite_for_algo("X25519", true),
            Some(KxSuite::X25519)
        );
        for name in ["ECDSA-SHA256", "ECDSA+P256+SHA256", "ecdsa+p256+sha256\0"] {
            assert!(
                check_dsign_matches(name, CertKeyAlgo::EcdsaP256Sha256).is_ok(),
                "{name:?}"
            );
        }
        assert!(check_dsign_matches("RSASSA-PSS-SHA256", CertKeyAlgo::EcdsaP256Sha256).is_err());
        assert!(
            check_dsign_matches(
                "RSASSA-PSS-MGF1SHA256+2048+SHA256",
                CertKeyAlgo::RsaPssSha256
            )
            .is_ok()
        );
    }

    /// A replier answers an initiator in its names: to a request with the
    /// names of 1.2 the reply's `c.dsign_algo` is one of 1.2.
    #[test]
    fn a_replier_answers_in_the_names_of_the_request() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out else {
            panic!("expected SendMessage");
        };
        let v = ht::parse_request_token(&req).unwrap();
        let renamed = ht::build_request_token(&ht::RequestBuildInput {
            version: v.version,
            cert_der: &v.cert_der,
            permissions: &v.permissions,
            pdata: &v.pdata,
            dsign_algo: ht::algo::ECDSA_P256_SHA256,
            kagree_algo: ht::algo::ECDHE_CEUM_P256_V1_2,
            dh1: &v.dh1,
            challenge1: &v.challenge1,
            ocsp_status: &v.ocsp_status,
        })
        .unwrap();
        let (_, out) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &renamed, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out else {
            panic!("expected SendMessage");
        };
        let h = zerodds_security::token::DataHolder::from_cdr_le(&reply).unwrap();
        let dsign = h.binary_property(ht::prop::C_DSIGN_ALGO).unwrap();
        assert_eq!(
            String::from_utf8_lossy(dsign).trim_end_matches('\0'),
            ht::algo::ECDSA_P256_SHA256
        );
    }

    /// DDS-Security 1.2 §9.3.2.11.18: a returned IdentityHandle is disposed
    /// of; its GUID binding goes with it, and a second return is refused.
    #[test]
    fn a_returned_identity_handle_is_disposed_of() {
        let (mut alice, _bob, _alice_h, alice_remote_bob, _bob_h, _bob_remote_alice) = alice_bob();
        assert!(alice.remote_guids.contains_key(&alice_remote_bob));
        alice.return_identity_handle(alice_remote_bob).unwrap();
        assert!(!alice.remote_guids.contains_key(&alice_remote_bob));
        assert!(!alice.remote_versions.contains_key(&alice_remote_bob));
        assert!(alice.return_identity_handle(alice_remote_bob).is_err());
    }

    /// DDS-Security 1.2 §10.3.2.5.3 Table 58: `hash_c1`, `hash_c2`, `dh1`
    /// and `dh2` are optional in the final; the signature covers them all
    /// the same. A final with only `challenge1`, `challenge2` and
    /// `signature`, as Cyclone sends it without `IncludeOptionalFields`,
    /// completes the replier with the initiator's secret. The parser
    /// wanted all four ("missing binary property: hash_c1"), and since the
    /// replier hands out its secret only after the final, no handshake
    /// with Cyclone or OpenDDS as initiator completed.
    #[test]
    fn a_final_without_the_optional_echoes_completes_the_replier() {
        use zerodds_security::token::DataHolder;
        let (alice, mut bob, alice_hs, bob_hs, final_token) = up_to_the_final();
        let full = DataHolder::from_cdr_le(&final_token).unwrap();
        let mut bare = DataHolder::new(full.class_id.clone());
        for key in [
            ht::prop::CHALLENGE1,
            ht::prop::CHALLENGE2,
            ht::prop::SIGNATURE,
        ] {
            bare.set_binary_property(key, full.binary_property(key).unwrap().to_vec());
        }
        let bare = bare.to_cdr_le();
        let view = ht::parse_final_token(&bare).unwrap();
        assert!(view.hash_c1.is_none() && view.dh1.is_none());

        assert!(matches!(
            bob.process_handshake(bob_hs, &bare).unwrap(),
            HandshakeStepOutcome::Complete { .. }
        ));
        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        let bob_secret = bob.shared_secret(bob_hs).unwrap();
        assert_eq!(
            alice.secret_bytes(alice_secret).unwrap().to_vec(),
            bob.secret_bytes(bob_secret).unwrap()
        );
    }

    /// Table 58: an optional echo that is sent must match. A final whose
    /// `dh1` differs from the request's fails.
    #[test]
    fn a_final_with_a_wrong_optional_echo_fails() {
        use zerodds_security::token::DataHolder;
        let (_alice, mut bob, _alice_hs, bob_hs, final_token) = up_to_the_final();
        let mut h = DataHolder::from_cdr_le(&final_token).unwrap();
        let mut dh1 = h.binary_property(ht::prop::DH1).unwrap().to_vec();
        dh1[0] ^= 0xFF;
        h.set_binary_property(ht::prop::DH1, dh1);
        let err = bob.process_handshake(bob_hs, &h.to_cdr_le()).unwrap_err();
        assert!(err.detail.contains("echo mismatch"), "{err}");
        assert!(bob.shared_secret(bob_hs).is_err());
    }

    /// §9.3.2.11.5: a reply with a forged signature fails and leaves the
    /// initiator's state in place; the genuine reply completes the
    /// handshake. The forged reply removed the state, and the genuine one
    /// found none.
    #[test]
    fn a_forged_reply_does_not_take_the_handshake_down() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out1 else {
            panic!("expected SendMessage");
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out2 else {
            panic!("expected SendMessage");
        };
        let genuine = ht::parse_reply_token(&reply).unwrap();
        let mut signature = genuine.signature.clone();
        signature[0] ^= 0xFF;
        let forged = ht::build_reply_token(&ReplyBuildInput {
            version: genuine.version,
            cert_der: &genuine.cert_der,
            permissions: &genuine.permissions,
            pdata: &genuine.pdata,
            dsign_algo: &genuine.dsign_algo,
            kagree_algo: &genuine.kagree_algo,
            dh2: &genuine.dh2,
            challenge2: &genuine.challenge2,
            hash_c1: &genuine.hash_c1.unwrap_or([0; 32]),
            dh1: genuine.dh1.as_deref().unwrap_or(&[]),
            challenge1: &genuine.challenge1,
            ocsp_status: &[],
            signature: &signature,
        })
        .unwrap();
        assert!(alice.process_handshake(alice_hs, &forged).is_err());
        assert!(matches!(
            alice.process_handshake(alice_hs, &reply).unwrap(),
            HandshakeStepOutcome::SendFinalMessage { .. }
        ));
        assert!(alice.shared_secret(alice_hs).is_ok());
    }

    #[test]
    fn a_token_signer_carries_a_full_handshake_on_both_sides() {
        use rcgen::{CertificateParams, KeyPair};

        let mut ca_params = CertificateParams::new(vec!["Common CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem().into_bytes();

        let identity = |name: &str| {
            let mut params = CertificateParams::new(vec![name.into()]).unwrap();
            let mut dn = rcgen::DistinguishedName::new();
            dn.push(rcgen::DnType::CommonName, name);
            params.distinguished_name = dn;
            params.is_ca = rcgen::IsCa::NoCa;
            let key = KeyPair::generate().unwrap();
            let cert = params.signed_by(&key, &ca_cert, &ca_key).unwrap();
            (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
        };
        let (mut alice_cert, mut alice_key) = identity("alice");
        let (mut bob_cert, mut bob_key) = identity("bob");
        // The participant with the smaller GUID starts the handshake.
        if bound_guid(&alice_cert) > bound_guid(&bob_cert) {
            core::mem::swap(&mut alice_cert, &mut bob_cert);
            core::mem::swap(&mut alice_key, &mut bob_key);
        }

        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        let alice_guid = bound_guid(&alice_cert);
        let bob_guid = bound_guid(&bob_cert);
        alice.set_local_participant_data(pdata_of(alice_guid));
        bob.set_local_participant_data(pdata_of(bob_guid));
        let alice_h = alice
            .validate_with_config_and_signer(
                IdentityConfig {
                    identity_cert_pem: alice_cert.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: None,
                },
                alice_guid,
                Arc::new(MockSigner {
                    pkcs8: crate::private_key_pkcs8_der(&alice_key).unwrap(),
                }),
            )
            .unwrap();
        let alice_remote_bob = alice
            .validate_remote_identity(alice_h, bob_guid, &identity_token_of(&bob_cert), None)
            .map(|v| v.identity)
            .unwrap();
        let bob_h = bob
            .validate_with_config_and_signer(
                IdentityConfig {
                    identity_cert_pem: bob_cert.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: None,
                },
                bob_guid,
                Arc::new(MockSigner {
                    pkcs8: crate::private_key_pkcs8_der(&bob_key).unwrap(),
                }),
            )
            .unwrap();
        let bob_remote_alice = bob
            .validate_remote_identity(bob_h, alice_guid, &identity_token_of(&alice_cert), None)
            .map(|v| v.identity)
            .unwrap();

        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };
        let out4 = bob.process_handshake(bob_hs, &final_token).unwrap();
        let bob_secret = match out4 {
            HandshakeStepOutcome::Complete { secret } => secret,
            _ => panic!("expected Complete"),
        };
        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        assert_eq!(
            alice.secret_bytes(alice_secret),
            bob.secret_bytes(bob_secret),
            "both sides derive the same secret through their token signers"
        );
    }

    #[test]
    fn request_token_has_spec_class_id_and_properties() {
        let (mut alice, _bob, alice_h, alice_remote_bob, _, _) = alice_bob();
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let token = match out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let parsed = ht::DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(parsed.class_id, "DDS:Auth:PKI-DH:1.0+Req");
        // c.dsign_algo / c.kagree_algo are BINARY properties (§9.3.2.5.2.1).
        assert!(parsed.binary_property("c.dsign_algo").is_some());
        assert!(parsed.binary_property("c.kagree_algo").is_some());
        // ocsp_status is spec-optional and is NOT emitted cross-vendor
        // (FastDDS does not send it; see build_request_token).
        for k in ["c.id", "c.perm", "c.pdata", "hash_c1", "dh1", "challenge1"] {
            assert!(
                parsed.binary_property(k).is_some(),
                "missing binary prop: {k}"
            );
        }
        // §7.3.3.1: the PEM of c.id is an ASCII string, and the value counts
        // its terminating NUL.
        let c_id = parsed.binary_property("c.id").unwrap();
        assert!(c_id.starts_with(b"-----BEGIN CERTIFICATE-----"));
        assert!(
            c_id.ends_with(b"-----END CERTIFICATE-----\n\0"),
            "c.id ends {:02x?}",
            &c_id[c_id.len().saturating_sub(4)..]
        );
    }

    #[test]
    fn cert_bind_replier_modifies_initiator_cert_in_reply_rejected() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let mut reply_token = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };

        // Tamper: flip a bit in hash_c1 → echo mismatch at the initiator.
        let mut h = ht::DataHolder::from_cdr_le(&reply_token).unwrap();
        let mut hash_c1 = h.binary_property("hash_c1").unwrap().to_vec();
        hash_c1[0] ^= 0x01;
        h.set_binary_property("hash_c1", hash_c1);
        reply_token = h.to_cdr_le();

        let err = alice.process_handshake(alice_hs, &reply_token).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn signature_tamper_rejected_by_initiator() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let mut reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };

        let mut h = ht::DataHolder::from_cdr_le(&reply).unwrap();
        let mut sig = h.binary_property("signature").unwrap().to_vec();
        sig[0] ^= 0x01;
        h.set_binary_property("signature", sig);
        reply = h.to_cdr_le();

        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn dh_tamper_in_reply_breaks_final_signature() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let mut reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };

        // Flips dh2 → the initiator sig verify fails (the signature
        // covers dh2). If this slipped through, the final
        // sig would diverge.
        let mut h = ht::DataHolder::from_cdr_le(&reply).unwrap();
        let mut dh2 = h.binary_property("dh2").unwrap().to_vec();
        dh2[0] ^= 0x01;
        h.set_binary_property("dh2", dh2);
        reply = h.to_cdr_le();

        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
        let _ = bob_hs;
    }

    #[test]
    fn wrong_ca_initiator_rejected_by_replier() {
        let (rogue_cert, _trusted_ca, rogue_key) = make_cert_with_wrong_ca();
        // Bob has the trusted CA, but Alice's cert is from a rogue CA.
        // We mount Bob with the trusted CA, Alice with her rogue CA.

        // Alice needs a CA that accepts her cert — so we take
        // the path: Alice validate-with-config loads with the rogue CA
        // (the same rogue ca from the helper). But make_cert_with_wrong_ca
        // returns only the EE cert + the trusted CA. We would have to
        // rebuild the rogue CA ourselves — more pragmatic: Alice+Bob with
        // the same shared CA, but Bob sets his trust anchor
        // to a *different* CA → the initiator cert is rejected at the reply
        // step.

        use rcgen::{CertificateParams, KeyPair};
        // Common CA for Alice's identity:
        let mut alice_ca_params = CertificateParams::new(vec!["AliceCA".into()]).unwrap();
        alice_ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let alice_ca_key = KeyPair::generate().unwrap();
        let alice_ca_cert = alice_ca_params.self_signed(&alice_ca_key).unwrap();
        let alice_ca_pem = alice_ca_cert.pem();

        let mut alice_params = CertificateParams::new(vec!["alice".into()]).unwrap();
        alice_params.is_ca = rcgen::IsCa::NoCa;
        let alice_key = KeyPair::generate().unwrap();
        let alice_cert = alice_params
            .signed_by(&alice_key, &alice_ca_cert, &alice_ca_key)
            .unwrap();
        let alice_cert_pem = alice_cert.pem().into_bytes();
        let alice_key_pem = alice_key.serialize_pem().into_bytes();

        // Bob with a completely different CA (and own cert):
        let mut bob_ca_params = CertificateParams::new(vec!["BobCA".into()]).unwrap();
        bob_ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let bob_ca_key = KeyPair::generate().unwrap();
        let bob_ca_cert = bob_ca_params.self_signed(&bob_ca_key).unwrap();
        let bob_ca_pem = bob_ca_cert.pem();
        let mut bob_params = CertificateParams::new(vec!["bob".into()]).unwrap();
        bob_params.is_ca = rcgen::IsCa::NoCa;
        let bob_key = KeyPair::generate().unwrap();
        let bob_cert = bob_params
            .signed_by(&bob_key, &bob_ca_cert, &bob_ca_key)
            .unwrap();
        let bob_cert_pem = bob_cert.pem().into_bytes();
        let bob_key_pem = bob_key.serialize_pem().into_bytes();

        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        let alice_h = alice
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: alice_cert_pem.clone(),
                    identity_ca_pem: alice_ca_pem.into_bytes(),
                    identity_key_pem: Some(alice_key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let bob_h = bob
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: bob_cert_pem.clone(),
                    identity_ca_pem: bob_ca_pem.into_bytes(),
                    identity_key_pem: Some(bob_key_pem),
                },
                [0xBB; 16],
            )
            .unwrap();

        // Alice does not know Bob (different CA), but she still tries
        // to start a handshake. We cannot get alice_remote_bob
        // via validate_remote_identity — so we fake it with
        // a valid stub handle. The test only checks the
        // replier path.
        let alice_remote_bob = alice
            .validate_remote_identity(alice_h, [0xBB; 16], &identity_token_of(&bob_cert_pem), None)
            .map(|v| v.identity)
            .unwrap();
        let bob_remote_alice = bob
            .validate_remote_identity(bob_h, [0xAA; 16], &identity_token_of(&alice_cert_pem), None)
            .map(|v| v.identity)
            .unwrap();
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // Bob tries to reply → Alice's cert is not from BobCA → reject.
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
        let _ = (rogue_cert, rogue_key);
    }

    #[test]
    fn replay_initiator_request_rejected_second_time() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // First reply: ok.
        let _ = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        // Second reply with the *same* request → replay → reject.
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn truncated_request_token_rejected() {
        let (_alice, mut bob, _alice_h, _, bob_h, bob_remote_alice) = alice_bob();
        let err = bob
            .begin_handshake_reply(
                bob_h,
                bob_remote_alice,
                &[0u8, 1u8, 2u8, 3u8, 4u8],
                &bob.test_pdata(),
            )
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn hash_c1_mismatch_in_request_rejected() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // Tamper with hash_c1 → parse_request_token rejected.
        let mut h = ht::DataHolder::from_cdr_le(&req).unwrap();
        let mut hc = h.binary_property("hash_c1").unwrap().to_vec();
        hc[5] ^= 0xFF;
        h.set_binary_property("hash_c1", hc);
        let bad = h.to_cdr_le();
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &bad, &bob.test_pdata())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// DDS-Security 1.2 §8.3 Table 26 gives `ECDHE-CEUM+P384` the
    /// `c.kagree_algo` string `"ECDHE-CEUM+P384"`. The mapping in both
    /// directions, alongside the two suites that were already there, and
    /// an unknown string still refused.
    #[test]
    fn kagree_algo_string_round_trips_for_every_supported_suite() {
        for suite in [
            KxSuite::EcdhP256,
            KxSuite::EcdhP384,
            KxSuite::DhModp2048_256,
            KxSuite::X25519,
        ] {
            for legacy in [false, true] {
                let s = PkiAuthenticationPlugin::kagree_algo_str(suite, legacy);
                assert_eq!(
                    PkiAuthenticationPlugin::kx_suite_for_algo(s, true),
                    Some(suite),
                    "kagree_algo {s:?}"
                );
            }
        }
        assert_eq!(
            PkiAuthenticationPlugin::kagree_algo_str(KxSuite::EcdhP384, false),
            "ECDHE-CEUM+P384"
        );
        // OpenDDS NUL-terminates the strings; the P-384 name must survive
        // that as the others do.
        assert_eq!(
            PkiAuthenticationPlugin::kx_suite_for_algo("ECDHE-CEUM+P384\0", false),
            Some(KxSuite::EcdhP384)
        );
        // DHE+MODP-2048-256 (Table 26) in both its names.
        for name in ["DH+MODP-2048-256", "DHE+MODP-2048-256"] {
            assert_eq!(
                PkiAuthenticationPlugin::kx_suite_for_algo(name, false),
                Some(KxSuite::DhModp2048_256)
            );
        }
        // Table 26 names, and the names of 1.1.
        for (suite, spec, legacy) in [
            (KxSuite::EcdhP256, "ECDHE-CEUM+P256", "ECDH+prime256v1-CEUM"),
            (KxSuite::EcdhP384, "ECDHE-CEUM+P384", "ECDHE-CEUM+P384"),
            (
                KxSuite::DhModp2048_256,
                "DHE+MODP-2048-256",
                "DH+MODP-2048-256",
            ),
        ] {
            assert_eq!(PkiAuthenticationPlugin::kagree_algo_str(suite, false), spec);
            assert_eq!(
                PkiAuthenticationPlugin::kagree_algo_str(suite, true),
                legacy
            );
        }
    }

    #[test]
    fn cross_algorithm_dsign_mismatch_rejected() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let mut req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // The token says "RSASSA-PSS-SHA256" although the cert is ECDSA.
        // But: hash_c1 contains the OLD algo → if we only swap dsign_algo,
        // hash_c1 will mismatch. So we must also
        // recompute hash_c1 and change the `c.dsign_algo`, then
        // the algo cross-check should fire at the replier step.
        let mut h = ht::DataHolder::from_cdr_le(&req).unwrap();
        // c.dsign_algo / c.kagree_algo are null-terminated BINARY properties
        // (§9.3.2.5.2.1).
        // NUL-free: c.dsign_algo + hash_c1 must be consistent (parse_request_token
        // recomputes hash_c1 over the RAW wire bytes). With a NUL in the wire value, but
        // without one in the hash, the hash check (AuthenticationFailed) would fire BEFORE the
        // algo cross-check (InvalidConfiguration). Here we want to test the algo check.
        h.set_binary_property("c.dsign_algo", b"RSASSA-PSS-SHA256".to_vec());
        let cert_der = h.binary_property("c.id").unwrap().to_vec();
        let perm = h.binary_property("c.perm").unwrap().to_vec();
        let pdata = h.binary_property("c.pdata").unwrap().to_vec();
        let mut kagree_bytes = h.binary_property("c.kagree_algo").unwrap().to_vec();
        if kagree_bytes.last() == Some(&0) {
            kagree_bytes.pop();
        }
        let kagree = String::from_utf8(kagree_bytes).unwrap();
        let new_hash = ht::compute_hash_c(&cert_der, &perm, &pdata, "RSASSA-PSS-SHA256", &kagree);
        h.set_binary_property("hash_c1", new_hash.to_vec());
        req = h.to_cdr_le();
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    #[test]
    fn extra_unknown_properties_in_reply_accepted_forward_compat() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // Adds an unknown property.
        let mut h = ht::DataHolder::from_cdr_le(&reply).unwrap();
        h.set_property("org.zerodds.future.feature", "yes");
        h.set_binary_property("org.zerodds.future.opaque", alloc::vec![0xFF; 8]);
        // hash_c2 is over the ORIGINAL properties. Since we only add EXTRA
        // fields, the hash inputs stay the same → ok.
        let new_reply = h.to_cdr_le();
        let res = alice.process_handshake(alice_hs, &new_reply);
        assert!(res.is_ok(), "forward-compat extra props must be accepted");
    }

    #[test]
    fn empty_permissions_accepted_in_phase3_mvp() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // alice sets no permissions in this test (no
        // AccessControl bind in the authentication-only roundtrip).
        let h = ht::DataHolder::from_cdr_le(&req).unwrap();
        assert_eq!(h.binary_property("c.perm").unwrap().len(), 0);
        let _ = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let _ = alice_hs;
    }

    #[test]
    fn shared_secret_returns_bad_argument_for_unknown_handle() {
        let plugin = PkiAuthenticationPlugin::new();
        let err = plugin.shared_secret(HandshakeHandle(42)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn permissions_token_announces_spec_class_id_when_permissions_set() {
        let mut p = PkiAuthenticationPlugin::new();
        // Without configured permissions: no token (AccessControl
        // inactive) — otherwise a secure remote would reject us with an empty
        // permissions match.
        assert!(
            p.get_permissions_token().is_empty(),
            "without permissions no PermissionsToken may be announced"
        );
        // With permissions: class_id-only PermissionsToken of Table 65,
        // `DDS:Access:Permissions:1.2` (the 1.0 of Cyclone DDS and Fast DDS is
        // a setting of the AccessControl plugin).
        p.set_local_permissions(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let token = p.get_permissions_token();
        assert!(
            !token.is_empty(),
            "PermissionsToken missing despite set permissions"
        );
        let parsed = ht::DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(parsed.class_id, "DDS:Access:Permissions:1.2");
        assert!(
            parsed.properties.is_empty(),
            "the announce without a plugin token carries no properties"
        );
    }

    /// DDS-Security 1.2 §10.3.2.1, Table 51: the remote IdentityToken is a
    /// DataHolder of the class `DDS:Auth:PKI-DH:<Major>.<Minor>`. A
    /// certificate in DER, a DataHolder of another class, and a class that
    /// only starts like it (`DDS:Auth:PKI-DHX:1.0`) are refused; they were
    /// taken (the last two by `starts_with`) or checked as a certificate.
    #[test]
    fn validate_remote_identity_takes_only_a_pki_dh_identity_token() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let holder = |class: &str| zerodds_security::token::DataHolder::new(class).to_cdr_le();
        // Table 59: only the PluginClassName and the MajorVersion are compared.
        for class in [
            "DDS:Auth:PKI-DH:1.0",
            "DDS:Auth:PKI-DH:1.1",
            "DDS:Auth:PKI-DH:1.2",
            "DDS:Auth:PKI-DH:1.9",
        ] {
            let remote = plugin
                .validate_remote_identity(local, [0xBB; 16], &holder(class), None)
                .map(|v| v.identity)
                .unwrap_or_else(|e| panic!("{class}: {e:?}"));
            assert_ne!(remote, local);
        }
        for refused in [
            cert_der_from_pem(&cert_pem),
            holder("DDS:Auth:PKI-DHX:1.0"),
            holder("DDS:Auth:PKI-DH"),
            holder("DDS:Auth:PKI-DH:2.0"),
            holder("DDS:Auth:PSK:1.2"),
            holder("DDS:Access:Permissions:1.0"),
        ] {
            assert!(
                plugin
                    .validate_remote_identity(local, [0xBB; 16], &refused, None)
                    .map(|v| v.identity)
                    .is_err()
            );
        }
    }

    // -------------------------------------------------------------
    // FU2 Gap 7b — spec IdentityToken descriptor (SPDP announce)
    // -------------------------------------------------------------

    #[test]
    fn get_identity_token_returns_decodable_spec_descriptor() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let token = plugin.get_identity_token(local).expect("identity token");
        assert!(!token.is_empty(), "no empty default token anymore");
        let decoded = crate::identity_token::IdentityToken::decode(&token)
            .expect("must be decodable as a spec descriptor");
        assert!(!decoded.cert_sn.is_empty(), "cert subject in the token");
        assert!(!decoded.ca_sn.is_empty(), "ca subject in the token");
    }

    /// DDS-Security 1.2 §10.3.2.2 Table 53: the IdentityStatusToken has the
    /// class_id "DDS:Auth:PKI-DH:1.0"; without an OCSP response it has no
    /// `ocsp_status`.
    #[test]
    fn the_identity_status_token_has_the_class_of_table_53() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let token = plugin.get_identity_status_token(local).unwrap();
        let holder = ht::DataHolder::from_cdr_le(&token).expect("a DataHolder");
        assert_eq!(holder.class_id, "DDS:Auth:PKI-DH:1.0");
        assert!(holder.binary_property("ocsp_status").is_none());
        assert!(
            plugin
                .get_identity_status_token(IdentityHandle(999))
                .is_err()
        );
    }

    #[test]
    fn validate_remote_identity_accepts_spec_descriptor_deferring_cert_check() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        // The peer announces a spec descriptor (no cert DER). The
        // cert validation happens later in the handshake (verify_remote_der
        // on c.id) — here the descriptor path accepts.
        let descriptor = crate::identity_token::build_identity_token_from_pem(&cert_pem, &ca_pem)
            .unwrap()
            .encode();
        let remote = plugin
            .validate_remote_identity(local, [0xBB; 16], &descriptor, None)
            .map(|v| v.identity)
            .expect("descriptor must be accepted");
        assert_ne!(remote, local);
    }

    #[test]
    fn validate_remote_identity_accepts_minimal_token_without_cert_properties() {
        // cyclone/FastDDS announce a MINIMAL IdentityToken in SPDP:
        // only class_id "DDS:Auth:PKI-DH:1.0", EMPTY properties (the cert-SN/
        // algo properties are optional per spec). validate_remote_identity
        // MUST recognize that as a spec descriptor (by class_id) and defer the
        // cert check to the handshake — NOT try to parse the 32 token bytes
        // as cert DER (that gave AuthenticationFailed
        // "TrailingData(SignedData)" → ZeroDDS sent no AUTH_REQUEST
        // cross-vendor).
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let minimal = zerodds_security::token::DataHolder::new(
            crate::identity_token::IDENTITY_TOKEN_CLASS_ID,
        )
        .to_cdr_le();
        let remote = plugin
            .validate_remote_identity(local, [0xBB; 16], &minimal, None)
            .map(|v| v.identity)
            .expect("minimal cyclone token (only class_id) must be accepted");
        assert_ne!(remote, local);
    }

    // -------------------------------------------------------------
    // Mutation killers (2026-05-01)
    // -------------------------------------------------------------

    /// Catches mutation `>` -> `>=` and `>` -> `==` on the replay-cache
    /// eviction boundary. The cache MUST be able to hold EXACTLY REPLAY_CACHE_CAP
    /// entries — eviction only happens at the CAP+1-th.
    ///
    /// Test strategy: after EXACTLY CAP unique inserts check whether the
    /// first is still in the cache (= replay reject).
    /// * Original `>`: 1024 > 1024 false → no eviction → 0 still in → reject.
    /// * Mutation `==`: 1024 == 1024 true → 0 evicted → no reject.
    /// * Mutation `>=`: likewise evicted at 1024 → no reject.
    #[test]
    fn replay_cache_holds_exactly_cap_entries_before_eviction() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        // Exactly CAP unique. The first (i=0) MUST still be in → replay reject.
        let mut c_first = [0u8; 32];
        c_first[0..8].copy_from_slice(&0u64.to_le_bytes());
        let err = alice.record_challenge(alice_h, c_first).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// Eviction happens at the CAP+1-th entry — test of the
    /// eviction path itself (FIFO: the oldest entry is evicted).
    #[test]
    fn replay_cache_evicts_oldest_at_cap_plus_one() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        // CAP+1: eviction triggers on the oldest (i=0).
        let mut c_extra = [0u8; 32];
        c_extra[0..8].copy_from_slice(&(REPLAY_CACHE_CAP as u64).to_le_bytes());
        alice.record_challenge(alice_h, c_extra).unwrap();

        // The first (i=0) should now be evicted → re-insert succeeds.
        let mut c_first = [0u8; 32];
        c_first[0..8].copy_from_slice(&0u64.to_le_bytes());
        alice
            .record_challenge(alice_h, c_first)
            .expect("after CAP+1 inserts, oldest must be evicted");

        // A young entry (e.g. the CAP-th) must still be in.
        // After inserting 0, 1 was evicted (FIFO), so we test
        // an entry that is surely still present — the CAP-th (=1024).
        let mut c_recent = [0u8; 32];
        c_recent[0..8].copy_from_slice(&(REPLAY_CACHE_CAP as u64).to_le_bytes());
        let err = alice.record_challenge(alice_h, c_recent).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// Catches mutation `get_shared_secret -> None / Some(vec![]) / Some(vec![0]) / Some(vec![1])`.
    /// After a successful handshake `get_shared_secret` must return the
    /// stored 32-byte secret, not None or a
    /// blanket value.
    #[test]
    fn get_shared_secret_returns_stored_value() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();

        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!(),
        };
        let out4 = bob.process_handshake(bob_hs, &final_token).unwrap();
        let bob_handle = match out4 {
            HandshakeStepOutcome::Complete { secret } => secret,
            _ => panic!(),
        };
        let alice_handle = alice.shared_secret(alice_hs).unwrap();

        // get_shared_secret must return concrete 32 bytes:
        let alice_bytes = SharedSecretProvider::get_shared_secret(&alice, alice_handle).unwrap();
        let bob_bytes = SharedSecretProvider::get_shared_secret(&bob, bob_handle).unwrap();
        // Mutation `None`: unwrap panicked → test fails.
        // Mutation `Some(vec![])`: len=0 != 32.
        // Mutation `Some(vec![0])` / `Some(vec![1])`: len=1 != 32.
        assert_eq!(alice_bytes.len(), 32);
        assert_eq!(bob_bytes.len(), 32);
        assert_eq!(
            alice_bytes, bob_bytes,
            "shared secrets must be identical + non-trivial"
        );
        assert!(alice_bytes.iter().any(|&b| b != 0));
        assert!(alice_bytes.iter().any(|&b| b != 1));

        // get_shared_secret with an unknown handle must return None.
        let bogus_handle = zerodds_security::authentication::SharedSecretHandle(0xDEAD_BEEF);
        assert!(SharedSecretProvider::get_shared_secret(&alice, bogus_handle).is_none());
    }

    /// Catches mutations `||` -> `&&` on each of the 6 echo checks in
    /// `process_final_on_replier`. One test per field — if ONE
    /// field is wrong, bob must reject the final token.
    /// With `&&`: only ALL 6 mismatches would reject.
    fn run_handshake_tampered_final<M>(mutate: M)
    where
        M: FnOnce(&mut ht::DataHolder),
    {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            _ => panic!(),
        };

        let mut h = ht::DataHolder::from_cdr_le(&final_token).unwrap();
        mutate(&mut h);
        let tampered = h.to_cdr_le();
        let err = bob.process_handshake(bob_hs, &tampered).unwrap_err();
        assert_eq!(
            err.kind,
            SecurityErrorKind::AuthenticationFailed,
            "tampered final token must return AuthFailed"
        );
    }

    fn flip_first_byte(h: &mut ht::DataHolder, prop: &str) {
        let mut v = h.binary_property(prop).unwrap().to_vec();
        v[0] ^= 0x01;
        h.set_binary_property(prop, v);
    }

    #[test]
    fn final_token_hash_c1_tamper_rejected() {
        run_handshake_tampered_final(|h| flip_first_byte(h, "hash_c1"));
    }
    #[test]
    fn final_token_hash_c2_tamper_rejected() {
        run_handshake_tampered_final(|h| flip_first_byte(h, "hash_c2"));
    }
    #[test]
    fn final_token_dh1_tamper_rejected() {
        run_handshake_tampered_final(|h| flip_first_byte(h, "dh1"));
    }
    #[test]
    fn final_token_dh2_tamper_rejected() {
        run_handshake_tampered_final(|h| flip_first_byte(h, "dh2"));
    }
    #[test]
    fn final_token_challenge1_tamper_rejected() {
        run_handshake_tampered_final(|h| flip_first_byte(h, "challenge1"));
    }
    #[test]
    fn final_token_challenge2_tamper_rejected() {
        run_handshake_tampered_final(|h| flip_first_byte(h, "challenge2"));
    }

    // ================================================================
    // DDS-Security 1.2 §9.3.2.11 Table 29 and §10.3.3 Table 59
    // ================================================================

    /// The `dds.sec.auth.*` properties of an identity.
    fn auth_props(cert_pem: &[u8], ca_pem: &[u8], key_pem: &[u8]) -> PropertyList {
        let text = |b: &[u8]| std::str::from_utf8(b).unwrap().to_owned();
        PropertyList::new()
            .with(Property::local(
                "dds.sec.auth.identity_certificate",
                text(cert_pem),
            ))
            .with(Property::local("dds.sec.auth.identity_ca", text(ca_pem)))
            .with(Property::local("dds.sec.auth.private_key", text(key_pem)))
    }

    /// SHA-256 of `bytes`, with the digest of the backend.
    fn sha256(bytes: &[u8]) -> Vec<u8> {
        digest::digest(&digest::SHA256, bytes).as_ref().to_vec()
    }

    /// Table 59, validate_local_identity: "The operation shall return the
    /// 16-byte adjusted_participant_guid GUID consisting of the same
    /// EntityId_t and a GuidPrefix_t computed as follows: The first bit (bit
    /// 0) shall be set to 1. The 47 bits following the first bit (bits 1 to
    /// 47) shall be set to the 47 first bits of the SHA-256 hash of the ASN.1
    /// DER encoding of the SubjectName [...]. The following 48 bits (bits 48
    /// to 95) shall be set to the first 48 bits of the SHA-256 hash of the
    /// candidate_participant_guid." The expected value is computed here from
    /// that text; `domain_id` is kept with the identity.
    #[test]
    fn validate_local_identity_returns_the_adjusted_guid_of_table_59() {
        use x509_cert::Certificate;
        use x509_cert::der::{Decode, Encode};
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let candidate = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x00, 0x00,
            0x01, 0xC1,
        ];
        let v = plugin
            .validate_local_identity(7, &auth_props(&cert_pem, &ca_pem, &key_pem), candidate)
            .unwrap();
        assert_eq!(v.result, ValidationResult::Ok);
        assert_ne!(v.identity, IdentityHandle::NIL);
        // The same EntityId.
        assert_eq!(v.adjusted_participant_guid[12..], candidate[12..]);
        // From the text of Table 59.
        let cert = Certificate::from_der(&cert_der_from_pem(&cert_pem)).unwrap();
        let subject_hash = sha256(&cert.tbs_certificate.subject.to_der().unwrap());
        let first_48 = subject_hash[..6]
            .iter()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
        // Bit 0 is 1, bits 1 to 47 are the first 47 bits of the hash.
        let head = (1u64 << 47) | (first_48 >> 1);
        let mut expected_prefix = head.to_be_bytes()[2..].to_vec();
        expected_prefix.extend_from_slice(&sha256(&candidate)[..6]);
        assert_eq!(v.adjusted_participant_guid[..12], expected_prefix[..]);
        assert_eq!(v.adjusted_participant_guid[0] & 0x80, 0x80);
        // The identity keeps the domain and the adjusted GUID.
        let kept = &plugin.identities[&v.identity];
        assert_eq!(kept.domain_id, Some(7));
        assert_eq!(kept.participant_guid, v.adjusted_participant_guid);
    }

    /// Table 59: a certificate the configured CA did not issue is
    /// VALIDATION_FAILED, and no identity is made.
    #[test]
    fn validate_local_identity_with_a_foreign_ca_fails_without_a_handle() {
        let (cert_pem, ca_pem, key_pem) = make_cert_with_wrong_ca();
        let mut plugin = PkiAuthenticationPlugin::new();
        let err = plugin
            .validate_local_identity(0, &auth_props(&cert_pem, &ca_pem, &key_pem), [1; 16])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
        assert!(plugin.identities.is_empty());
    }

    /// Table 59, get_identity_token and get_identity_status_token: "shall
    /// receive the handle corresponding to the one returned by a successful
    /// previous call to validate_local_identity. If the above condition is
    /// not met, the operation shall return the exception".
    #[test]
    fn the_tokens_of_a_local_identity_need_a_local_identity_handle() {
        let (mut alice, _bob, alice_h, alice_remote_bob, _bob_h, _) = alice_bob();
        assert!(alice.get_identity_token(alice_h).is_ok());
        assert!(alice.get_identity_status_token(alice_h).is_ok());
        for handle in [alice_remote_bob, IdentityHandle(999), IdentityHandle::NIL] {
            assert!(alice.get_identity_token(handle).is_err(), "{handle:?}");
            assert!(
                alice.get_identity_status_token(handle).is_err(),
                "{handle:?}"
            );
            assert!(
                alice
                    .set_participant_security_config(
                        handle,
                        &ParticipantSecurityConfig::unrestricted()
                    )
                    .is_err(),
                "{handle:?}"
            );
            assert!(
                alice
                    .set_permissions_credential_and_token(handle, &[], &[])
                    .is_err(),
                "{handle:?}"
            );
        }
    }

    /// Table 53: the IdentityStatusToken has the class `DDS:Auth:PKI-DH:1.0`
    /// and, when there is an OCSP response for the identity certificate, it
    /// as the binary property `ocsp_status` (DER).
    #[test]
    fn the_identity_status_token_carries_the_ocsp_response() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        let bare = alice.get_identity_status_token(alice_h).unwrap();
        let bare = ht::DataHolder::from_cdr_le(&bare).unwrap();
        assert_eq!(bare.class_id, "DDS:Auth:PKI-DH:1.0");
        assert!(bare.properties.is_empty() && bare.binary_properties.is_empty());
        let response = vec![0x30, 0x03, 0x0A, 0x01, 0x00];
        alice.identities.get_mut(&alice_h).unwrap().ocsp_staple = Some(response.clone());
        let with = alice.get_identity_status_token(alice_h).unwrap();
        let with = ht::DataHolder::from_cdr_le(&with).unwrap();
        assert_eq!(with.class_id, "DDS:Auth:PKI-DH:1.0");
        assert_eq!(with.binary_property("ocsp_status"), Some(&response[..]));
    }

    /// §9.3.2.12.1, Table 32: a change of the IdentityStatusToken is
    /// reported to the listener as IDENTITY_STATUS.
    #[test]
    fn a_changed_status_token_is_reported_as_identity_status() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        let recorder = Arc::new(Recorder::default());
        alice.set_listener(Some(recorder.clone())).unwrap();
        let first = alice.get_identity_status_token(alice_h).unwrap();
        alice.return_identity_status_token(&first).unwrap();
        alice.check_identities(1);
        assert!(recorder.status.lock().unwrap().is_empty(), "no change");
        alice.identities.get_mut(&alice_h).unwrap().ocsp_staple = Some(vec![0x30, 0x00]);
        alice.check_identities(1);
        assert_eq!(
            *recorder.status.lock().unwrap(),
            [(alice_h, AuthStatusKind::IdentityStatus)]
        );
        alice.check_identities(1);
        assert_eq!(recorder.status.lock().unwrap().len(), 1, "reported once");
    }

    /// A listener that notes what it is told.
    #[derive(Default)]
    struct Recorder {
        revoked: std::sync::Mutex<Vec<IdentityHandle>>,
        status: std::sync::Mutex<Vec<(IdentityHandle, AuthStatusKind)>>,
    }

    impl AuthenticationListener for Recorder {
        fn on_revoke_identity(&self, handle: IdentityHandle) -> SecurityResult<()> {
            self.revoked.lock().unwrap().push(handle);
            Ok(())
        }
        fn on_status_changed(&self, handle: IdentityHandle, status_kind: AuthStatusKind) {
            self.status.lock().unwrap().push((handle, status_kind));
        }
    }

    /// Table 59, set_participant_security_config: the supported masks have to
    /// include an algorithm of the plugin, in each of the three kinds.
    #[test]
    fn set_participant_security_config_fails_without_an_algorithm_of_the_plugin() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        let ok = ParticipantSecurityConfig::unrestricted();
        assert!(alice.set_participant_security_config(alice_h, &ok).is_ok());
        // Bit 8 and up are no algorithm of Table 25, bit 3 and up no
        // algorithm of Table 26.
        for broken in [
            AlgorithmMasks {
                trust_chain: (0xF0, 0),
                ..ok.algorithm_info
            },
            AlgorithmMasks {
                message_auth: (0xF0, 0),
                ..ok.algorithm_info
            },
            AlgorithmMasks {
                key_establishment: (0xF8, 0),
                ..ok.algorithm_info
            },
        ] {
            let config = ParticipantSecurityConfig {
                algorithm_info: broken,
            };
            let err = alice
                .set_participant_security_config(alice_h, &config)
                .unwrap_err();
            assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
        }
    }

    /// Table 59: the adjusted info starts as a copy of the given one; the
    /// algorithms the plugin does not support go out of the supported masks,
    /// the ones it uses (the identity certificate's, the key's, the key
    /// agreement of the initiator) go into the required masks.
    #[test]
    fn set_participant_security_config_adjusts_a_copy_of_the_given_info() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        let (pss, pkcs1, p256, p384) = (1u32, 2u32, 4u32, 8u32);
        let config = ParticipantSecurityConfig {
            algorithm_info: AlgorithmMasks {
                // PSS, P-256, plus bit 10 (no algorithm of Table 25).
                trust_chain: (pss | p256 | 0x400, 0x400),
                message_auth: (p256 | p384 | 0x400, 0),
                // The DH group and ECDHE P-256, plus bit 9.
                key_establishment: (0b011 | 0x200, 0),
            },
        };
        let adjusted = alice
            .set_participant_security_config(alice_h, &config)
            .unwrap();
        // The unsupported bit is gone from the supported mask, the required
        // mask the caller gave stays, the certificate's algorithm is added.
        assert_eq!(adjusted.trust_chain, (pss | p256, 0x400 | p256));
        assert_eq!(adjusted.message_auth, (p256 | p384, p256));
        assert_eq!(adjusted.key_establishment, (0b011, 0b010));
        let _ = pkcs1;
    }

    /// Table 59: the algorithm of the identity key "is present in the
    /// digital_signature.trust_chain.supported_mask, fail otherwise", as is
    /// the key agreement of the initiator in the shared_secret mask.
    #[test]
    fn set_participant_security_config_fails_when_a_used_algorithm_is_not_supported() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        // The key is ECDSA P-256 (bit 2); only P-384 (bit 3) is supported.
        let only_p384 = ParticipantSecurityConfig {
            algorithm_info: AlgorithmMasks {
                trust_chain: (8, 0),
                ..ParticipantSecurityConfig::unrestricted().algorithm_info
            },
        };
        assert!(
            alice
                .set_participant_security_config(alice_h, &only_p384)
                .is_err()
        );
        // The initiator agrees with ECDHE P-256 (bit 1); only the DH group
        // (bit 0) is supported.
        let only_dh = ParticipantSecurityConfig {
            algorithm_info: AlgorithmMasks {
                key_establishment: (1, 0),
                ..ParticipantSecurityConfig::unrestricted().algorithm_info
            },
        };
        assert!(
            alice
                .set_participant_security_config(alice_h, &only_dh)
                .is_err()
        );
    }

    /// Table 59: "The operation shall configure the AUTH plugin to only accept
    /// the resulting set of supported algorithms": a replier restricted to
    /// ECDHE P-384 refuses a request that agrees with ECDHE P-256, and one
    /// restricted to P-256 takes it.
    #[test]
    fn the_configured_algorithms_bind_the_handshake() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out else {
            panic!("expected SendMessage");
        };
        let only = |bit: u32| ParticipantSecurityConfig {
            algorithm_info: AlgorithmMasks {
                key_establishment: (bit, 0),
                ..ParticipantSecurityConfig::unrestricted().algorithm_info
            },
        };
        // bit 2 is ECDHE P-384.
        bob.set_preferred_kx_suite(KxSuite::EcdhP384);
        bob.set_participant_security_config(bob_h, &only(0b100))
            .unwrap();
        assert!(
            bob.begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
                .is_err(),
            "ECDHE P-256 is not in the supported mask"
        );
        bob.set_preferred_kx_suite(KxSuite::EcdhP256);
        bob.set_participant_security_config(bob_h, &only(0b010))
            .unwrap();
        assert!(
            bob.begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
                .is_ok()
        );
    }

    /// A permissions credential token of DDS:Access:Permissions (Table 64)
    /// and a permissions token.
    fn permissions_tokens(document: &str) -> (Vec<u8>, Vec<u8>) {
        (
            ht::DataHolder::new("DDS:Access:PermissionsCredential")
                .with_property(PERMISSIONS_CREDENTIAL_CERT, document)
                .to_cdr_le(),
            ht::DataHolder::new("DDS:Access:Permissions:1.2")
                .with_property("dds.perm_ca.sn", "CN=perm-ca")
                .to_cdr_le(),
        )
    }

    /// Table 59, set_permissions_credential_and_token: "shall store the
    /// PermissionsCredentialToken and the PermissionsToken internally to the
    /// plugin and associate them with the DomainParticipant represented by the
    /// IdentityHandle". The document of the credential (Table 64
    /// `dds.perm.cert`) is the `c.perm` of its handshakes, with the NUL
    /// that ends an ASCII value (§7.3.3.1), and the token is the one the
    /// plugin announces.
    #[test]
    fn the_permissions_credential_is_what_the_handshake_sends_as_c_perm() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        assert!(alice.get_permissions_token().is_empty());
        let (credential, token) = permissions_tokens("PERMISSIONS-OF-ALICE");
        alice
            .set_permissions_credential_and_token(alice_h, &credential, &token)
            .unwrap();
        assert_eq!(alice.get_permissions_token(), token);
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: req } = out else {
            panic!("expected SendMessage");
        };
        assert_eq!(
            ht::parse_request_token(&req).unwrap().permissions,
            b"PERMISSIONS-OF-ALICE\0"
        );
        // The replier's reply carries the document of its own credential.
        let (credential, token) = permissions_tokens("PERMISSIONS-OF-BOB");
        bob.set_permissions_credential_and_token(bob_h, &credential, &token)
            .unwrap();
        let (_, out) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req, &bob.test_pdata())
            .unwrap();
        let HandshakeStepOutcome::SendMessage { token: reply } = out else {
            panic!("expected SendMessage");
        };
        assert_eq!(
            ht::parse_reply_token(&reply).unwrap().permissions,
            b"PERMISSIONS-OF-BOB\0"
        );
    }

    /// Table 29: a credential that is not that of DDS:Access:Permissions, one
    /// without the document, and a token that is no DataHolder are refused.
    #[test]
    fn a_permissions_credential_that_is_none_is_refused() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        let (credential, token) = permissions_tokens("DOC");
        let wrong_class = ht::DataHolder::new("DDS:Access:PSK:PermissionsCredential")
            .with_property(PERMISSIONS_CREDENTIAL_CERT, "DOC")
            .to_cdr_le();
        let no_document = ht::DataHolder::new("DDS:Access:PermissionsCredential").to_cdr_le();
        assert!(
            alice
                .set_permissions_credential_and_token(alice_h, &wrong_class, &token)
                .is_err()
        );
        assert!(
            alice
                .set_permissions_credential_and_token(alice_h, &no_document, &token)
                .is_err()
        );
        assert!(
            alice
                .set_permissions_credential_and_token(alice_h, b"junk", &token)
                .is_err()
        );
        assert!(
            alice
                .set_permissions_credential_and_token(alice_h, &credential, b"junk")
                .is_err()
        );
        assert!(alice.local_permissions_by_identity.is_empty());
    }

    /// Table 59, validate_remote_identity: "compare lexicographically the
    /// remote_participant_guid with the participant key obtained from the
    /// local_identity_handle. If the remote_participant_guid >
    /// local_participant_guid, the operation shall return
    /// VALIDATION_PENDING_HANDSHAKE_REQUEST. If [...] < [...],
    /// VALIDATION_PENDING_HANDSHAKE_MESSAGE."
    #[test]
    fn validate_remote_identity_orders_the_guids_like_table_59() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local_guid = [0x50u8; 16];
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                local_guid,
            )
            .unwrap();
        let token = identity_token_of(&[]);
        let result = |plugin: &mut PkiAuthenticationPlugin, guid: [u8; 16]| {
            plugin
                .validate_remote_identity(local, guid, &token, None)
                .map(|v| v.result)
        };
        assert_eq!(
            result(&mut plugin, [0x60; 16]).unwrap(),
            ValidationResult::PendingHandshakeRequest
        );
        assert_eq!(
            result(&mut plugin, [0x40; 16]).unwrap(),
            ValidationResult::PendingHandshakeMessage
        );
        // Lexicographic: the first octet decides, whatever follows.
        let mut greater = local_guid;
        greater[0] = 0x51;
        greater[15] = 0x00;
        assert_eq!(
            result(&mut plugin, greater).unwrap(),
            ValidationResult::PendingHandshakeRequest
        );
        let mut smaller = local_guid;
        smaller[15] = 0x4F;
        assert_eq!(
            result(&mut plugin, smaller).unwrap(),
            ValidationResult::PendingHandshakeMessage
        );
        // The participant itself is no remote participant.
        assert!(result(&mut plugin, local_guid).is_err());
    }

    /// Table 59: "If the PluginClassName or the MajorVersion are different, it
    /// shall return VALIDATION_FAILED."; a different MinorVersion is not
    /// different.
    #[test]
    fn validate_remote_identity_compares_the_plugin_name_and_the_major_version() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0x50; 16],
            )
            .unwrap();
        let holder = |class: &str| ht::DataHolder::new(class).to_cdr_le();
        for accepted in [
            "DDS:Auth:PKI-DH:1.0",
            "DDS:Auth:PKI-DH:1.2",
            "DDS:Auth:PKI-DH:1.77",
        ] {
            assert!(
                plugin
                    .validate_remote_identity(local, [0x60; 16], &holder(accepted), None)
                    .is_ok(),
                "{accepted}"
            );
        }
        for refused in [
            "DDS:Auth:PKI-DH:2.0",
            "DDS:Auth:PKI-DH:0.9",
            "DDS:Auth:PKI-DHX:1.0",
            "DDS:Auth:PSK:1.2",
            "DDS:Auth:PKI-DH",
            "",
        ] {
            assert!(
                plugin
                    .validate_remote_identity(local, [0x60; 16], &holder(refused), None)
                    .is_err(),
                "{refused}"
            );
        }
        // Not a DataHolder at all.
        assert!(
            plugin
                .validate_remote_identity(local, [0x60; 16], b"junk", None)
                .is_err()
        );
    }

    /// Table 59 and Table 55: "If the remote_auth_request_token is NIL, the
    /// operation shall generate a local_auth_request_token [...] otherwise
    /// the local_auth_request_token shall be set to TokenNIL." The token is
    /// of the class `DDS:Auth:PKI-DH:1.0+AuthReq` and holds a 256-bit
    /// `future_challenge`.
    #[test]
    fn the_local_auth_request_token_is_made_for_a_nil_remote_one_only() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let local = plugin
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: cert_pem,
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(key_pem),
                },
                [0x50; 16],
            )
            .unwrap();
        let token = identity_token_of(&[]);
        let nil = plugin
            .validate_remote_identity(local, [0x60; 16], &token, None)
            .unwrap();
        let made = ht::DataHolder::from_cdr_le(&nil.local_auth_request_token.unwrap()).unwrap();
        assert_eq!(made.class_id, "DDS:Auth:PKI-DH:1.0+AuthReq");
        assert_eq!(made.binary_property("future_challenge").unwrap().len(), 32);
        assert!(made.properties.is_empty());
        let remote_token = ht::build_auth_request_token(ht::ClassVersion::default(), &[7; 32]);
        let given = plugin
            .validate_remote_identity(local, [0x60; 16], &token, Some(&remote_token))
            .unwrap();
        assert!(given.local_auth_request_token.is_none(), "TokenNIL");
        // A token of another class, or without the challenge, fails.
        for bad in [
            ht::DataHolder::new("DDS:Auth:PKI-DH:1.0+Req").to_cdr_le(),
            ht::DataHolder::new("DDS:Auth:PKI-DH:1.0+AuthReq").to_cdr_le(),
        ] {
            assert!(
                plugin
                    .validate_remote_identity(local, [0x60; 16], &token, Some(&bad))
                    .is_err()
            );
        }
        // Two NONCEs are two NONCEs.
        let other = plugin
            .validate_remote_identity(local, [0x61; 16], &token, None)
            .unwrap();
        let other = ht::DataHolder::from_cdr_le(&other.local_auth_request_token.unwrap()).unwrap();
        assert_ne!(
            made.binary_property("future_challenge"),
            other.binary_property("future_challenge")
        );
    }

    /// Table 59, begin_handshake_request and begin_handshake_reply: the
    /// handles come from a validate_remote_identity that returned
    /// VALIDATION_PENDING_HANDSHAKE_REQUEST (request) or
    /// VALIDATION_PENDING_HANDSHAKE_MESSAGE (reply), the local identity being
    /// the one that call was made for; otherwise the PreconditionError.
    #[test]
    fn the_handshake_operations_need_the_result_of_validate_remote_identity() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        // Alice has the smaller GUID: she begins, and cannot reply.
        assert!(
            alice
                .begin_handshake_reply(alice_h, alice_remote_bob, &[], &alice.test_pdata())
                .is_err()
        );
        // Bob is asked to reply, and cannot begin.
        assert!(
            bob.begin_handshake_request(bob_h, bob_remote_alice, &bob.test_pdata())
                .is_err()
        );
        // A handle that validate_remote_identity did not make, a local handle
        // in its place, and the handle of another local identity.
        for replier in [IdentityHandle(999), alice_h, IdentityHandle::NIL] {
            assert!(
                alice
                    .begin_handshake_request(alice_h, replier, &alice.test_pdata())
                    .is_err(),
                "{replier:?}"
            );
        }
        // And the right ones work.
        assert!(
            alice
                .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
                .is_ok()
        );
    }

    /// Table 59: the initiator's process_handshake of the reply returns
    /// VALIDATION_OK_FINAL_MESSAGE (and carries the final message), the
    /// replier's process_handshake of the final VALIDATION_OK;
    /// begin_handshake_request and begin_handshake_reply return
    /// VALIDATION_PENDING_HANDSHAKE_MESSAGE.
    #[test]
    fn the_handshake_returns_the_validation_results_of_table_59() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, request) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        assert_eq!(
            request.validation_result(),
            ValidationResult::PendingHandshakeMessage
        );
        let (bob_hs, reply) = bob
            .begin_handshake_reply(
                bob_h,
                bob_remote_alice,
                request.message_out().unwrap(),
                &bob.test_pdata(),
            )
            .unwrap();
        assert_eq!(
            reply.validation_result(),
            ValidationResult::PendingHandshakeMessage
        );
        let last = alice
            .process_handshake(alice_hs, reply.message_out().unwrap())
            .unwrap();
        assert_eq!(last.validation_result(), ValidationResult::OkFinalMessage);
        let done = bob
            .process_handshake(bob_hs, last.message_out().unwrap())
            .unwrap();
        assert_eq!(done.validation_result(), ValidationResult::Ok);
        assert!(done.message_out().is_none(), "the replier sends nothing");
    }

    /// Runs the handshake of `alice_bob()` to the end and returns the two
    /// handshake handles.
    fn complete_alice_bob(
        alice: &mut PkiAuthenticationPlugin,
        bob: &mut PkiAuthenticationPlugin,
        ids: (
            IdentityHandle,
            IdentityHandle,
            IdentityHandle,
            IdentityHandle,
        ),
    ) -> (HandshakeHandle, HandshakeHandle) {
        let (alice_h, alice_remote_bob, bob_h, bob_remote_alice) = ids;
        let (alice_hs, request) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        let (bob_hs, reply) = bob
            .begin_handshake_reply(
                bob_h,
                bob_remote_alice,
                request.message_out().unwrap(),
                &bob.test_pdata(),
            )
            .unwrap();
        let last = alice
            .process_handshake(alice_hs, reply.message_out().unwrap())
            .unwrap();
        bob.process_handshake(bob_hs, last.message_out().unwrap())
            .unwrap();
        (alice_hs, bob_hs)
    }

    /// Table 54: the AuthenticatedPeerCredentialToken has the class
    /// `DDS:Auth:PKI-DH:1.0`, the binary property `c.id` (the certificate the
    /// peer sent) and `c.perm` (the permissions document the peer sent), for
    /// the initiator and the replier alike; before the handshake completed
    /// there is none (§9.3.2.11.8: "after the last handshake operation [...]
    /// returns VALIDATION_OK or VALIDATION_OK_FINAL_MESSAGE").
    #[test]
    fn the_authenticated_peer_credential_token_holds_c_id_and_c_perm() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let alice_cert = der_to_pem(&alice.identities[&alice_h].cert_der);
        let bob_cert = der_to_pem(&bob.identities[&bob_h].cert_der);
        let (credential, token) = permissions_tokens("PERM-A");
        alice
            .set_permissions_credential_and_token(alice_h, &credential, &token)
            .unwrap();
        let (credential, token) = permissions_tokens("PERM-B");
        bob.set_permissions_credential_and_token(bob_h, &credential, &token)
            .unwrap();
        let (alice_hs, request) = alice
            .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
            .unwrap();
        assert!(
            alice
                .get_authenticated_peer_credential_token(alice_hs)
                .is_err(),
            "the initiator has not completed"
        );
        let (bob_hs, reply) = bob
            .begin_handshake_reply(
                bob_h,
                bob_remote_alice,
                request.message_out().unwrap(),
                &bob.test_pdata(),
            )
            .unwrap();
        assert!(
            bob.get_authenticated_peer_credential_token(bob_hs).is_err(),
            "the replier has not completed: its secret, and the peer, wait for the final"
        );
        let last = alice
            .process_handshake(alice_hs, reply.message_out().unwrap())
            .unwrap();
        // The initiator is complete with the final message.
        let at_alice = alice
            .get_authenticated_peer_credential_token(alice_hs)
            .unwrap();
        let at_alice = ht::DataHolder::from_cdr_le(&at_alice).unwrap();
        assert_eq!(at_alice.class_id, "DDS:Auth:PKI-DH:1.0");
        assert_eq!(at_alice.binary_property("c.id"), Some(&bob_cert[..]));
        assert_eq!(at_alice.binary_property("c.perm"), Some(&b"PERM-B\0"[..]));
        assert!(
            bob.get_authenticated_peer_credential_token(bob_hs).is_err(),
            "still waiting for the final"
        );
        bob.process_handshake(bob_hs, last.message_out().unwrap())
            .unwrap();
        let at_bob = bob.get_authenticated_peer_credential_token(bob_hs).unwrap();
        let at_bob = ht::DataHolder::from_cdr_le(&at_bob).unwrap();
        assert_eq!(at_bob.binary_property("c.id"), Some(&alice_cert[..]));
        assert_eq!(at_bob.binary_property("c.perm"), Some(&b"PERM-A\0"[..]));
    }

    /// §9.3.2.11.14 to §9.3.2.11.19 and Table 59: the return operations take
    /// back what the plugin issued — once — and release the state behind it.
    #[test]
    fn the_return_operations_take_back_what_was_issued() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let identity = alice.get_identity_token(alice_h).unwrap();
        let status = alice.get_identity_status_token(alice_h).unwrap();
        assert!(
            alice.return_identity_status_token(&identity).is_err(),
            "an identity token is no status token"
        );
        alice.return_identity_token(&identity).unwrap();
        assert!(alice.return_identity_token(&identity).is_err(), "twice");
        alice.return_identity_status_token(&status).unwrap();
        assert!(alice.return_identity_status_token(&status).is_err());
        assert!(alice.return_identity_token(b"not issued").is_err());

        let (alice_hs, bob_hs) = complete_alice_bob(
            &mut alice,
            &mut bob,
            (alice_h, alice_remote_bob, bob_h, bob_remote_alice),
        );
        let credential = alice
            .get_authenticated_peer_credential_token(alice_hs)
            .unwrap();
        assert!(
            alice.return_identity_token(&credential).is_err(),
            "a peer credential token is no identity token"
        );
        alice
            .return_authenticated_peer_credential_token(&credential)
            .unwrap();
        assert!(
            alice
                .return_authenticated_peer_credential_token(&credential)
                .is_err()
        );

        // The shared secret goes back, and its bytes with it.
        let secret = alice.shared_secret(alice_hs).unwrap();
        assert!(alice.secret_bytes(secret).is_some());
        alice.return_sharedsecret_handle(secret).unwrap();
        assert!(alice.secret_bytes(secret).is_none());
        assert!(alice.return_sharedsecret_handle(secret).is_err(), "twice");
        assert!(alice.shared_secret(alice_hs).is_err(), "no association");

        // The handshake goes back, and its state with it.
        alice.return_handshake_handle(alice_hs).unwrap();
        assert!(alice.return_handshake_handle(alice_hs).is_err(), "twice");
        assert!(
            alice
                .get_authenticated_peer_credential_token(alice_hs)
                .is_err()
        );
        assert!(alice.process_handshake(alice_hs, &[]).is_err());
        assert!(!alice.handshake_remote.contains_key(&alice_hs));
        // A handle no begin_handshake issued is not taken back.
        assert!(alice.return_handshake_handle(HandshakeHandle(999)).is_err());
        // The replier's side, the same.
        bob.return_handshake_handle(bob_hs).unwrap();
        assert!(bob.shared_secret(bob_hs).is_err());
        // A secret that no completed handshake holds is not one of the
        // plugin's to take back.
        assert!(
            alice
                .return_sharedsecret_handle(SharedSecretHandle(999))
                .is_err()
        );
    }

    /// §9.3.2.12.2: revoking an identity invalidates its handle and tells the
    /// listener; what hangs on it (the handshake, its secret) goes with it.
    #[test]
    fn revoking_an_identity_tells_the_listener_and_invalidates_the_handle() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let recorder = Arc::new(Recorder::default());
        alice.set_listener(Some(recorder.clone())).unwrap();
        let (alice_hs, _) = complete_alice_bob(
            &mut alice,
            &mut bob,
            (alice_h, alice_remote_bob, bob_h, bob_remote_alice),
        );
        let secret = alice.shared_secret(alice_hs).unwrap();
        alice.revoke_identity(alice_remote_bob).unwrap();
        assert_eq!(*recorder.revoked.lock().unwrap(), [alice_remote_bob]);
        // The handle is invalid: the operations on it fail.
        assert!(alice.return_identity_handle(alice_remote_bob).is_err());
        assert!(
            alice
                .begin_handshake_request(alice_h, alice_remote_bob, &alice.test_pdata())
                .is_err()
        );
        // No shared secret survives, and no authenticated peer.
        assert!(alice.shared_secret(alice_hs).is_err());
        assert!(alice.secret_bytes(secret).is_none());
        assert!(
            alice
                .get_authenticated_peer_credential_token(alice_hs)
                .is_err()
        );
        // Revoking again is no revocation.
        assert!(alice.revoke_identity(alice_remote_bob).is_err());
        assert_eq!(recorder.revoked.lock().unwrap().len(), 1);
        // The local identity can be revoked too: its remote identities go
        // with it.
        let (alice2, _bob2, alice2_h, alice2_remote, _, _) = alice_bob();
        let mut alice2 = alice2;
        let recorder = Arc::new(Recorder::default());
        alice2.set_listener(Some(recorder.clone())).unwrap();
        alice2.revoke_identity(alice2_h).unwrap();
        let told = recorder.revoked.lock().unwrap().clone();
        assert!(
            told.contains(&alice2_h) && told.contains(&alice2_remote),
            "{told:?}"
        );
        assert!(alice2.get_identity_token(alice2_h).is_err());
    }

    /// §9.3.2.12: "identity certificates can expire; in this situation, the
    /// AuthenticationPlugin shall call the AuthenticationListener to notify
    /// the DDS implementation that the identity of a specific
    /// DomainParticipant is being revoked."
    #[test]
    fn an_expired_identity_certificate_is_revoked() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let recorder = Arc::new(Recorder::default());
        alice.set_listener(Some(recorder.clone())).unwrap();
        let (alice_hs, _) = complete_alice_bob(
            &mut alice,
            &mut bob,
            (alice_h, alice_remote_bob, bob_h, bob_remote_alice),
        );
        let not_after = cert_not_after(&alice.identities[&alice_h].cert_der).unwrap();
        // Before the end of the certificates nothing happens.
        alice.check_identities(1_000_000_000);
        assert!(recorder.revoked.lock().unwrap().is_empty());
        assert!(alice.shared_secret(alice_hs).is_ok());
        // After it: the peer's certificate and the local one expired.
        alice.check_identities(not_after + 1);
        let told = recorder.revoked.lock().unwrap().clone();
        assert!(told.contains(&alice_remote_bob), "{told:?}");
        assert!(told.contains(&alice_h), "{told:?}");
        assert!(alice.shared_secret(alice_hs).is_err());
    }

    /// §9.3.2.12.2: a CRL that lists the certificate of an authenticated peer
    /// revokes its identity (Table 59: "The operation shall check a CRL").
    #[test]
    fn a_crl_that_lists_the_peer_revokes_its_identity() {
        use rcgen::{
            CertificateParams, CertificateRevocationListParams, KeyIdMethod, KeyPair,
            RevocationReason, RevokedCertParams, SerialNumber,
        };
        let mut ca_params = CertificateParams::new(vec!["CRL CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem().into_bytes();
        let issue = |name: &str, serial: u64| {
            let mut params = CertificateParams::new(vec![name.into()]).unwrap();
            let mut dn = rcgen::DistinguishedName::new();
            dn.push(rcgen::DnType::CommonName, name);
            params.distinguished_name = dn;
            params.serial_number = Some(SerialNumber::from(serial));
            let key = KeyPair::generate().unwrap();
            let cert = params.signed_by(&key, &ca_cert, &ca_key).unwrap();
            (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
        };
        // Alice is the smaller GUID.
        let (mut a, mut b) = (issue("alice", 11), issue("bob", 22));
        let mut serials = (11u64, 22u64);
        if bound_guid(&a.0) > bound_guid(&b.0) {
            core::mem::swap(&mut a, &mut b);
            core::mem::swap(&mut serials.0, &mut serials.1);
        }
        let (a_guid, b_guid) = (bound_guid(&a.0), bound_guid(&b.0));
        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        alice.set_local_participant_data(pdata_of(a_guid));
        bob.set_local_participant_data(pdata_of(b_guid));
        let config = |cert: &[u8], key: &[u8]| IdentityConfig {
            identity_cert_pem: cert.to_vec(),
            identity_ca_pem: ca_pem.clone(),
            identity_key_pem: Some(key.to_vec()),
        };
        let alice_h = alice
            .validate_with_config(config(&a.0, &a.1), a_guid)
            .unwrap();
        let bob_h = bob
            .validate_with_config(config(&b.0, &b.1), b_guid)
            .unwrap();
        let alice_remote_bob = alice
            .validate_remote_identity(alice_h, b_guid, &identity_token_of(&[]), None)
            .map(|v| v.identity)
            .unwrap();
        let bob_remote_alice = bob
            .validate_remote_identity(bob_h, a_guid, &identity_token_of(&[]), None)
            .map(|v| v.identity)
            .unwrap();
        let recorder = Arc::new(Recorder::default());
        alice.set_listener(Some(recorder.clone())).unwrap();
        let (alice_hs, _) = complete_alice_bob(
            &mut alice,
            &mut bob,
            (alice_h, alice_remote_bob, bob_h, bob_remote_alice),
        );
        alice.check_identities(1_000_000_000);
        assert!(recorder.revoked.lock().unwrap().is_empty());
        // The CA publishes a CRL that lists Bob's certificate.
        let now = rcgen::date_time_ymd(2026, 1, 1);
        let crl = CertificateRevocationListParams {
            this_update: now,
            next_update: rcgen::date_time_ymd(2090, 1, 1),
            crl_number: SerialNumber::from(1),
            issuing_distribution_point: None,
            revoked_certs: vec![RevokedCertParams {
                serial_number: SerialNumber::from(serials.1),
                revocation_time: now,
                reason_code: Some(RevocationReason::KeyCompromise),
                invalidity_date: None,
            }],
            key_identifier_method: KeyIdMethod::Sha256,
        }
        .signed_by(&ca_cert, &ca_key)
        .unwrap();
        alice.set_crls(vec![crl.der().to_vec()]);
        // The policy of the identity is the one it was accepted with:
        // validate_local_identity made it, so it takes the CRL on the next
        // validation; the policy of the plugin holds it for the check.
        alice
            .identities
            .get_mut(&alice_h)
            .unwrap()
            .policy
            .crls_der
            .push(crl.der().to_vec());
        alice.check_identities(1_000_000_000);
        assert_eq!(*recorder.revoked.lock().unwrap(), [alice_remote_bob]);
        assert!(alice.shared_secret(alice_hs).is_err());
    }

    /// Table 51 and §10.3.2.1: the IdentityToken the plugin announces has the
    /// names of Table 25 in `dds.cert.algo` and `dds.ca.algo`, and the names
    /// of 1.1 with the setting that OpenDDS needs.
    #[test]
    fn the_identity_token_names_the_algorithms_of_table_25() {
        let (cert_pem, ca_pem, key_pem) = make_signed_cert_ca_key();
        let mut plugin = PkiAuthenticationPlugin::new();
        let handle = plugin
            .validate_local_identity(0, &auth_props(&cert_pem, &ca_pem, &key_pem), [1; 16])
            .unwrap()
            .identity;
        let token = plugin.get_identity_token(handle).unwrap();
        let holder = ht::DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(holder.class_id, "DDS:Auth:PKI-DH:1.0");
        assert_eq!(holder.property("dds.cert.algo"), Some("ECDSA+P256+SHA256"));
        assert_eq!(holder.property("dds.ca.algo"), Some("ECDSA+P256+SHA256"));
        assert!(holder.property("dds.cert.sn").is_some());
        assert!(holder.property("dds.ca.sn").is_some());
        // The setting of 1.1.
        plugin.set_algorithm_names(AlgorithmNames::V1_1);
        let legacy = plugin
            .validate_local_identity(0, &auth_props(&cert_pem, &ca_pem, &key_pem), [2; 16])
            .unwrap()
            .identity;
        let token = plugin.get_identity_token(legacy).unwrap();
        let holder = ht::DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(holder.property("dds.cert.algo"), Some("EC-prime256v1"));
        assert_eq!(holder.property("dds.ca.algo"), Some("EC-prime256v1"));
    }

    /// Table 56, Table 57: `c.dsign_algo` and `c.kagree_algo` carry the
    /// `CryptoAlgorithmName` of Table 25 and Table 26. The names of 1.1, which
    /// Cyclone, Fast DDS and OpenDDS read, go to a peer that is known to read
    /// those under `auto`, or when the setting
    /// `org.zerodds.sec.algorithm_names` says `1.1`; `1.2` (the default) keeps
    /// the names of 1.2 for every peer.
    #[test]
    fn the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1() {
        let names = |token: &[u8]| {
            let h = ht::DataHolder::from_cdr_le(token).unwrap();
            let text = |p: &str| {
                String::from_utf8_lossy(h.binary_property(p).unwrap())
                    .trim_end_matches('\0')
                    .to_string()
            };
            (text(ht::prop::C_DSIGN_ALGO), text(ht::prop::C_KAGREE_ALGO))
        };
        let request = |configure: &dyn Fn(&mut PkiAuthenticationPlugin)| {
            let (mut alice, _bob, ah, ar, _, _) = alice_bob();
            configure(&mut alice);
            let (_, out) = alice
                .begin_handshake_request(ah, ar, &alice.test_pdata())
                .unwrap();
            names(out.message_out().unwrap())
        };
        let spec = (
            "ECDSA+P256+SHA256".to_string(),
            "ECDHE-CEUM+P256".to_string(),
        );
        let legacy = (
            "ECDSA-SHA256".to_string(),
            "ECDH+prime256v1-CEUM".to_string(),
        );
        // The default (1.2) with no knowledge of the peer, and with a peer
        // that reads the names of 1.1: Table 25 and 26.
        assert_eq!(request(&|_| {}), spec);
        assert_eq!(
            request(&|p| p.set_peer_algorithm_names(PeerAlgorithmNames::Legacy)),
            spec
        );
        // `auto` with a peer that reads the names of 1.1, and without.
        assert_eq!(
            request(&|p| {
                p.set_algorithm_names(AlgorithmNames::Auto);
                p.set_peer_algorithm_names(PeerAlgorithmNames::Legacy);
            }),
            legacy
        );
        // `spec` ignores what is known of the peer, `legacy` ignores it too.
        assert_eq!(
            request(&|p| {
                p.set_algorithm_names(AlgorithmNames::V1_2);
                p.set_peer_algorithm_names(PeerAlgorithmNames::Legacy);
            }),
            spec
        );
        assert_eq!(
            request(&|p| {
                p.set_algorithm_names(AlgorithmNames::V1_1);
                p.set_peer_algorithm_names(PeerAlgorithmNames::Spec);
            }),
            legacy
        );
    }
}
