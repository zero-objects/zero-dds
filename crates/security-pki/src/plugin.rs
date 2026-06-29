// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `AuthenticationPlugin` impl based on X.509 / rustls-webpki.
//!
//! Spec: OMG DDS-Security 1.2 §10.3.2.6-8 + §10.3.4. Tokens are encoded via
//! the `DataHolder` wire format from [`crate::handshake_token`].
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`webpki::EndEntityCert::verify_signature` takes
//! `&dyn SignatureVerificationAlgorithm` — that is a 3rd-party API,
//! not a ZeroDDS construction. We cannot convert it to concrete
//! generics without forking webpki.)

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::backend::digest;
use crate::backend::rand::{SecureRandom, SystemRandom};
use crate::backend::signature;
use rustls_pki_types::CertificateDer;
use zerodds_security::authentication::{
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle,
    SharedSecretHandle, SharedSecretProvider,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;
use zerodds_security_keyexchange::{KeyExchange, KxSuite};

use crate::handshake_token::{
    self as ht, FinalBuildInput, ReplyBuildInput, RequestBuildInput, ct_eq,
};
use crate::identity::{CertKeyAlgo, IdentityConfig, ParsedIdentity, PkiError};

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
}

/// Maximum storage for "recently seen" initiator challenges
/// (replay-detection cache per replier). 1024 entries × 32 bytes = 32 KiB.
const REPLAY_CACHE_CAP: usize = 1024;

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
    /// Cross-vendor quirk (transient, per-peer): if `true`, the
    /// next `c.dsign_algo`/`c.kagree_algo` are emitted + hashed WITH a trailing `\0`.
    /// OpenDDS compares them via `sizeof` (incl. NUL); FastDDS
    /// (#3803) needs them WITHOUT. The discovery layer sets this via
    /// [`AuthenticationPlugin::set_algo_nul_terminate`] based on the peer
    /// VendorId BEFORE each `begin_handshake_request`/`begin_handshake_reply`.
    algo_nul: bool,
    /// Local CMS-signed permissions document (`.p7s` bytes) for the
    /// `c.perm` property in the handshake (spec §9.3.2.5.1). Empty = no
    /// AccessControl permissions; a foreign vendor with active AccessControl
    /// (governance) then rejects the handshake. Set by the SecurityProfile.
    local_permissions: Vec<u8>,
    /// Local `ParticipantBuiltinTopicData` as PL_CDR bytes — sent along in the
    /// handshake as `c.pdata` (spec §9.3.2.5.2). The replier
    /// (e.g. cyclone) deserializes c.pdata as a ParameterList; empty leads
    /// cross-vendor to "Deserialize parameter header failed".
    local_pdata: Vec<u8>,
}

struct InitiatorState {
    /// Local identity handle (for cert/key lookup).
    local: IdentityHandle,
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
    /// Detected initiator cert algo.
    initiator_key_algo: CertKeyAlgo,
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
            handshake_to_secret: BTreeMap::new(),
            secrets: BTreeMap::new(),
            secret_challenges: BTreeMap::new(),
            replay_cache: BTreeMap::new(),
            replay_order: BTreeMap::new(),
            // Spec default: cross-vendor-readable ECDHE+P-256+SHA-256.
            preferred_kx_suite: KxSuite::EcdhP256,
            algo_nul: false,
            local_permissions: Vec::new(),
            local_pdata: Vec::new(),
        }
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
    fn kx_suite_for_algo(algo: &str) -> Option<KxSuite> {
        let algo = algo.trim_end_matches('\0');
        if algo == ht::algo::ECDHE_P256_SHA256 {
            Some(KxSuite::EcdhP256)
        } else if algo == ht::algo::X25519 {
            Some(KxSuite::X25519)
        } else {
            None
        }
    }

    /// `c.kagree_algo` string for a KX suite.
    fn kagree_algo_str(suite: KxSuite) -> &'static str {
        match suite {
            KxSuite::EcdhP256 => ht::algo::ECDHE_P256_SHA256,
            KxSuite::X25519 => ht::algo::X25519,
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
        _participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle> {
        let parsed = ParsedIdentity::from_config(&cfg).map_err(pki_to_security)?;
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
        PkiError::InvalidPem(_) | PkiError::NoCertInPem => SecurityErrorKind::BadArgument,
        PkiError::CertInvalid(_) => SecurityErrorKind::AuthenticationFailed,
        PkiError::EmptyTrustAnchors => SecurityErrorKind::InvalidConfiguration,
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

fn algo_for(key_algo: CertKeyAlgo) -> SecurityResult<&'static str> {
    match key_algo {
        CertKeyAlgo::EcdsaP256Sha256 => Ok(ht::algo::ECDSA_SHA256),
        CertKeyAlgo::RsaPssSha256 => Ok(ht::algo::RSASSA_PSS_SHA256),
        CertKeyAlgo::Unknown => Err(SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            "pki: cert public-key algo unsupported",
        )),
    }
}

fn check_dsign_matches(declared: &str, detected: CertKeyAlgo) -> SecurityResult<()> {
    let expected = algo_for(detected)?;
    // OpenDDS sends c.dsign_algo NUL-terminated (sizeof convention) — the
    // trailing `\0` is not part of the algorithm name.
    let declared = declared.trim_end_matches('\0');
    if !declared.eq_ignore_ascii_case(expected) {
        return Err(SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            alloc::format!("pki: c.dsign_algo {declared} doesn't match cert (expected {expected})"),
        ));
    }
    Ok(())
}

fn sign_with(key_algo: CertKeyAlgo, pkcs8: &[u8], msg: &[u8]) -> SecurityResult<Vec<u8>> {
    let rng = SystemRandom::new();
    match key_algo {
        CertKeyAlgo::EcdsaP256Sha256 => {
            let key = crate::compat::ecdsa_from_pkcs8(
                &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                pkcs8,
                &rng,
            )
            .map_err(|e| {
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
        }
        CertKeyAlgo::RsaPssSha256 => {
            let key = signature::RsaKeyPair::from_pkcs8(pkcs8).map_err(|e| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    alloc::format!("pki: rsa key-parse failed: {e}"),
                )
            })?;
            let mut out = alloc::vec![0u8; crate::compat::rsa_modulus_len(&key)];
            key.sign(&signature::RSA_PSS_SHA256, &rng, msg, &mut out)
                .map_err(|e| {
                    SecurityError::new(
                        SecurityErrorKind::CryptoFailed,
                        alloc::format!("pki: rsa sign failed: {e}"),
                    )
                })?;
            Ok(out)
        }
        CertKeyAlgo::Unknown => Err(SecurityError::new(
            SecurityErrorKind::InvalidConfiguration,
            "pki: cannot sign — unsupported key algo",
        )),
    }
}

fn verify_signature_with_cert(
    cert_der: &[u8],
    key_algo: CertKeyAlgo,
    msg: &[u8],
    sig: &[u8],
) -> SecurityResult<()> {
    let cert = CertificateDer::from_slice(cert_der);
    let ee = webpki::EndEntityCert::try_from(&cert).map_err(|e| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("pki: peer cert parse failed: {e:?}"),
        )
    })?;
    let alg: &dyn rustls_pki_types::SignatureVerificationAlgorithm = match key_algo {
        #[cfg(not(feature = "aws-lc"))]
        CertKeyAlgo::EcdsaP256Sha256 => webpki::ring::ECDSA_P256_SHA256,
        #[cfg(feature = "aws-lc")]
        CertKeyAlgo::EcdsaP256Sha256 => webpki::aws_lc_rs::ECDSA_P256_SHA256,
        #[cfg(not(feature = "aws-lc"))]
        CertKeyAlgo::RsaPssSha256 => webpki::ring::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
        #[cfg(feature = "aws-lc")]
        CertKeyAlgo::RsaPssSha256 => webpki::aws_lc_rs::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
        CertKeyAlgo::Unknown => {
            return Err(SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: peer cert algo unsupported",
            ));
        }
    };
    ee.verify_signature(alg, msg, sig).map_err(|e| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("pki: signature verify failed: {e:?}"),
        )
    })
}

/// Detect peer cert algo from DER (re-uses identity helper logic via
/// trial-parse; pragmatic — we duplicate the SPKI scan locally to avoid
/// re-exporting an internal helper).
fn detect_peer_algo(cert_der: &[u8]) -> CertKeyAlgo {
    crate::identity::detect_cert_algo_pub(cert_der)
}

/// PEM-encode a DER certificate. The handshake `c.id` carries the cert as
/// **PEM** (spec §9.3.2.5.2; cyclone/FastDDS/RTI parse c.id as PEM and
/// fail on raw DER with "PEM routines: no start line").
fn der_to_pem(der: &[u8]) -> Vec<u8> {
    use x509_cert::Certificate;
    use x509_cert::der::{Decode, EncodePem, pem::LineEnding};
    match Certificate::from_der(der).and_then(|c| c.to_pem(LineEnding::LF)) {
        Ok(pem) => pem.into_bytes(),
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
    // (`Certificate::load_cert_bytes`: `length(i + 1)`). webpki rejects that
    // on the DER side as `TrailingData(SignedData)` — strip trailing NULs.
    // ONLY for parsing: hash_c1/hash_c2 still use the RAW c.id bytes
    // (with NUL), so the hash matches OpenDDS byte for byte.
    let bytes = match bytes.iter().rposition(|&b| b != 0) {
        Some(i) => &bytes[..=i],
        None => bytes,
    };
    if bytes.starts_with(b"-----") {
        if let Ok(cert) = Certificate::from_pem(bytes) {
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

impl AuthenticationPlugin for PkiAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        props: &PropertyList,
        participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle> {
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
        let cfg = IdentityConfig {
            identity_cert_pem: cert.as_bytes().to_vec(),
            identity_ca_pem: ca.as_bytes().to_vec(),
            identity_key_pem: props.get(keys::IDENTITY_KEY).map(|s| s.as_bytes().to_vec()),
        };
        self.validate_with_config(cfg, participant_guid)
    }

    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        _remote_participant_guid: [u8; 16],
        remote_auth_token: &[u8],
    ) -> SecurityResult<IdentityHandle> {
        let parsed = self.identities.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown local IdentityHandle",
            )
        })?;
        // Two accepted token forms:
        // 1. Spec `IdentityToken` descriptor (PID_IDENTITY_TOKEN from SPDP,
        //    cross-vendor): only cert SN/algo, no cert. Acceptance gate —
        //    the cryptographic cert-chain validation happens in the
        //    handshake (verify_remote_der on `c.id`, where the real cert
        //    arrives). Both handshake sides already validate.
        // 2. Raw cert DER (ZeroDDS-internal/legacy caller): check immediately against
        //    the local trust anchors.
        // A spec IdentityToken descriptor is recognized by its class_id
        // (DataHolder), NOT by the optional cert properties: cyclone/
        // FastDDS send a MINIMAL token in SPDP (only class_id
        // "DDS:Auth:PKI-DH:x.y", empty properties). Only if it is NOT such a
        // descriptor (= raw cert DER from ZeroDDS-internal/legacy
        // callers) check immediately against the local trust anchors. The
        // cryptographic cert-chain validation of the remote happens anyway
        // only in the handshake (verify_remote_der on `c.id`, where the real cert
        // arrives). Previously there was IdentityToken::decode here (requires ALL 4
        // cert properties) — that threw cyclone's minimal token into the
        // cert-DER path → AuthenticationFailed "TrailingData(SignedData)" →
        // ZeroDDS sent no AUTH_REQUEST cross-vendor.
        let is_spec_descriptor =
            zerodds_security::token::DataHolder::from_cdr_le(remote_auth_token)
                .map(|dh| dh.class_id.starts_with("DDS:Auth:PKI-DH"))
                .unwrap_or(false);
        if !is_spec_descriptor {
            parsed
                .verify_remote_der(remote_auth_token)
                .map_err(pki_to_security)?;
        }
        let handle = IdentityHandle(self.next_id());
        Ok(handle)
    }

    fn get_identity_token(&self, local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        let parsed = self.identities.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown local IdentityHandle",
            )
        })?;
        let ca_der = parsed.trust_anchors_der.first().ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "pki: no trust anchor")
        })?;
        let token = crate::identity_token::build_identity_token_from_der(&parsed.cert_der, ca_der)
            .map_err(pki_to_security)?;
        Ok(token.encode())
    }

    fn get_permissions_token(&self) -> Vec<u8> {
        // Without configured permissions AccessControl is inactive — then
        // the token is omitted (no empty permissions-match trigger).
        if self.local_permissions.is_empty() {
            return Vec::new();
        }
        // class_id-only PermissionsToken (spec §7.2.4). Version string
        // `1.0` + empty properties mirror what cyclone/FastDDS announce
        // in SPDP (`permissions_token="DDS:Access:Permissions:1.0"
        // :{}:{}`). The actual signed permissions content travels
        // in the handshake as `c.perm`, not in the SPDP token.
        ht::DataHolder::new("DDS:Access:Permissions:1.0").to_cdr_le()
    }

    fn set_local_participant_data(&mut self, pdata: Vec<u8>) {
        self.local_pdata = pdata;
    }

    fn set_algo_nul_terminate(&mut self, nul: bool) {
        self.algo_nul = nul;
    }

    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        _replier: IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let parsed = self.identities.get(&initiator).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown initiator IdentityHandle",
            )
        })?;
        // c.id carries the cert as PEM (spec §9.3.2.5.2 / cross-vendor);
        // hash_c1 is computed in build_request_token consistently over the same
        // PEM bytes.
        let cert_der = der_to_pem(&parsed.cert_der);
        let key_algo = parsed.key_algo;
        let dsign_algo = self.algo_str(algo_for(key_algo)?);
        // Spec/cross-vendor default (EcdhP256) instead of the non-spec X25519
        // extension. The announced `c.kagree_algo` must match the actually
        // used suite.
        let suite = self.preferred_kx_suite;
        let kagree_algo = self.algo_str(Self::kagree_algo_str(suite));

        let kx = KeyExchange::with_suite(suite)?;
        let dh1 = kx.public_key().to_vec();
        let challenge1 = random_challenge()?;
        // c.perm: local CMS-signed permissions document (cross-vendor
        // AccessControl requirement). c.pdata: own ParticipantBuiltinTopicData
        // as PL_CDR (the replier deserializes it as a ParameterList).
        let permissions: Vec<u8> = self.local_permissions.clone();
        let pdata: Vec<u8> = self.local_pdata.clone();

        let bytes = ht::build_request_token(&RequestBuildInput {
            cert_der: &cert_der,
            permissions: &permissions,
            pdata: &pdata,
            dsign_algo: &dsign_algo,
            kagree_algo: &kagree_algo,
            dh1: &dh1,
            challenge1: &challenge1,
            ocsp_status: &[],
        })?;

        let hash_c1 =
            ht::compute_hash_c(&cert_der, &permissions, &pdata, &dsign_algo, &kagree_algo);

        let handle = HandshakeHandle(self.next_id());
        self.pending_initiator.insert(
            handle,
            InitiatorState {
                local: initiator,
                kx: Some(kx),
                dh1,
                challenge1,
                hash_c1,
                permissions,
                pdata,
                kagree_algo,
                dsign_algo,
            },
        );
        Ok((handle, HandshakeStepOutcome::SendMessage { token: bytes }))
    }

    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        _initiator: IdentityHandle,
        request_token: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        // 1) Parse + hash re-validation (happens in parse_request_token).
        let req = ht::parse_request_token(request_token)?;
        // Echo kagree for the reply: `req.kagree_algo` is NUL-stripped on
        // parsing; for OpenDDS peers (algo_nul) restore the NUL form
        // so that the wire (DiffieHellman::factory sizeof comparison) AND
        // hash_c2 are consistently NUL-terminated. For FastDDS/Cyclone NUL-free.
        let reply_kagree = self.algo_str(&req.kagree_algo);

        // 2) Validate the initiator cert against our trust store.
        let parsed = self.identities.get(&replier).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unknown replier IdentityHandle",
            )
        })?;
        parsed
            .verify_remote_der(&cid_to_der(&req.cert_der))
            .map_err(pki_to_security)?;

        // 3) Replay detection.
        self.record_challenge(replier, req.challenge1)?;

        // 4) The initiator `c.dsign_algo` must match its cert public key.
        let initiator_key_algo = detect_peer_algo(&cid_to_der(&req.cert_der));
        check_dsign_matches(&req.dsign_algo, initiator_key_algo)?;

        // 5) Generate our own DH pair + challenge2. The KX suite follows the
        //    `c.kagree_algo` announced by the initiator (spec: the replier
        //    uses the same mechanism) — unsupported = reject.
        let suite = Self::kx_suite_for_algo(&req.kagree_algo).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "pki: unsupported c.kagree_algo in the request",
            )
        })?;
        let kx = KeyExchange::with_suite(suite)?;
        let dh2 = kx.public_key().to_vec();
        let challenge2 = random_challenge()?;

        // 6) DH agreement → SharedSecret.
        let parsed = self.identities.get(&replier).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::Internal, "pki: replier identity gone")
        })?;
        // Sign with the replier key
        let priv_key = parsed.private_key_pkcs8_der.clone().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: replier has no private key configured (cannot sign)",
            )
        })?;
        let replier_cert_der = der_to_pem(&parsed.cert_der);
        let replier_dsign = self.algo_str(algo_for(parsed.key_algo)?);
        let replier_key_algo = parsed.key_algo;

        // raw DH output (KeyExchange consumes self).
        let secret_bytes = kx.derive_shared_secret(&req.dh1)?;
        // HKDF-Re-Derive with challenges as salt for spec-aligned key.
        let final_secret = derive_shared_secret(&secret_bytes)?;

        // 7) Replier hash_c2 = over the replier tuple (echo kagree, own
        //    cert/perm/pdata/dsign). c.perm = local permissions document.
        //    Needed both in the reply token and in the signature content.
        let permissions: Vec<u8> = self.local_permissions.clone();
        let pdata: Vec<u8> = self.local_pdata.clone();
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
        let signature = sign_with(replier_key_algo, &priv_key, &to_sign)?;

        // 9) Build the reply token.
        let reply_bytes = ht::build_reply_token(&ReplyBuildInput {
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
            ocsp_status: &[],
            signature: &signature,
        })?;

        // 10) Persist state — the final-receive needs it.
        // challenge1 = initiator (req), challenge2 = own replier value.
        let handle = HandshakeHandle(self.next_id());
        let secret_handle = self.store_secret(handle, final_secret, req.challenge1, challenge2);
        self.handshake_to_secret.insert(handle, secret_handle);
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
                initiator_cert_der: req.cert_der,
                initiator_key_algo,
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

    fn plugin_class_id(&self) -> &str {
        "DDS:Auth:PKI-DH:1.2"
    }
}

impl PkiAuthenticationPlugin {
    fn process_reply_on_initiator(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let reply = ht::parse_reply_token(token)?;

        let st = self.pending_initiator.remove(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "pki: initiator state gone")
        })?;

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
        let (priv_key, initiator_key_algo) = {
            let parsed = self.identities.get(&st.local).ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::Internal, "pki: initiator identity gone")
            })?;
            parsed
                .verify_remote_der(&cid_to_der(&reply.cert_der))
                .map_err(pki_to_security)?;
            let pk = parsed.private_key_pkcs8_der.clone().ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::InvalidConfiguration,
                    "pki: initiator has no private key (final-sign not possible)",
                )
            })?;
            (pk, parsed.key_algo)
        };
        let replier_key_algo = detect_peer_algo(&cid_to_der(&reply.cert_der));
        check_dsign_matches(&reply.dsign_algo, replier_key_algo)?;

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
            replier_key_algo,
            &to_verify,
            &reply.signature,
        )?;

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
        let signature = sign_with(initiator_key_algo, &priv_key, &to_sign)?;
        // The final token echoes hash_c1/dh1 with the OWN initiator values
        // (st), not the ones optionally omitted in the reply; hash_c2 is the
        // value recomputed from the reply.
        let final_token = ht::build_final_token(&FinalBuildInput {
            hash_c1: &st.hash_c1,
            hash_c2: &reply.hash_c2,
            dh1: &st.dh1,
            dh2: &reply.dh2,
            challenge1: &reply.challenge1,
            challenge2: &reply.challenge2,
            ocsp_status: &[],
            signature: &signature,
        })?;

        // Spec §10.3.2.10: the initiator sends `final_token` AND is
        // **complete**. We tunnel both via `SendMessage` + lookup
        // via `shared_secret()`. So that the wire layer sees the token,
        // we return SendMessage; the DCPS runtime then calls
        // `shared_secret()`.
        Ok(HandshakeStepOutcome::SendMessage { token: final_token })
    }

    fn process_final_on_replier(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let final_tok = ht::parse_final_token(token)?;
        let st = self.pending_replier.remove(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "pki: replier state gone")
        })?;

        // Echo consistency.
        if !ct_eq(&final_tok.hash_c1, &st.hash_c1)
            || !ct_eq(&final_tok.hash_c2, &st.hash_c2)
            || !ct_eq(&final_tok.dh1, &st.dh1)
            || !ct_eq(&final_tok.dh2, &st.dh2)
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
            st.initiator_key_algo,
            &to_verify,
            &final_tok.signature,
        )?;

        // We take the local handle only because of the `local` field, so the
        // lint does not complain; in production we could use it for
        // audit logging.
        let _ = st.local;
        Ok(HandshakeStepOutcome::Complete {
            secret: st.secret_handle,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::properties::Property;

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

        let mut alice_params = CertificateParams::new(vec!["alice".into()]).unwrap();
        alice_params.is_ca = rcgen::IsCa::NoCa;
        let alice_key_pair = KeyPair::generate().unwrap();
        let alice_cert = alice_params
            .signed_by(&alice_key_pair, &ca_cert, &ca_key)
            .unwrap();
        let alice_cert_pem = alice_cert.pem().into_bytes();
        let alice_key_pem = alice_key_pair.serialize_pem().into_bytes();

        let mut bob_params = CertificateParams::new(vec!["bob".into()]).unwrap();
        bob_params.is_ca = rcgen::IsCa::NoCa;
        let bob_key_pair = KeyPair::generate().unwrap();
        let bob_cert = bob_params
            .signed_by(&bob_key_pair, &ca_cert, &ca_key)
            .unwrap();
        let bob_cert_pem = bob_cert.pem().into_bytes();
        let bob_key_pem = bob_key_pair.serialize_pem().into_bytes();

        let mut alice = PkiAuthenticationPlugin::new();
        let mut bob = PkiAuthenticationPlugin::new();
        let alice_h = alice
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: alice_cert_pem.clone(),
                    identity_ca_pem: ca_pem.clone(),
                    identity_key_pem: Some(alice_key_pem),
                },
                [0xAA; 16],
            )
            .unwrap();
        let alice_remote_for_bob = alice
            .validate_remote_identity(alice_h, [0xBB; 16], &cert_der_from_pem(&bob_cert_pem))
            .unwrap();
        let bob_h = bob
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: bob_cert_pem.clone(),
                    identity_ca_pem: ca_pem,
                    identity_key_pem: Some(bob_key_pem),
                },
                [0xBB; 16],
            )
            .unwrap();
        let bob_remote_for_alice = bob
            .validate_remote_identity(bob_h, [0xAA; 16], &cert_der_from_pem(&alice_cert_pem))
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
            .validate_local_identity(&props, [0xAA; 16])
            .expect("validate via props");
        assert!(handle.0 >= 1);
    }

    // -------------------------------------------------------------
    // C3.1 — spec-conformant handshake (Tab.56/57/58)
    // -------------------------------------------------------------

    #[test]
    fn full_three_round_handshake_alice_bob() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();

        // 1) Alice → REQUEST.
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req_token = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };
        assert!(req_token.len() > 100, "request token contains cert + DH");

        // 2) Bob → REPLY.
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req_token)
            .unwrap();
        let reply_token = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        // 3) Alice → FINAL.
        let out3 = alice.process_handshake(alice_hs, &reply_token).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
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

    #[test]
    fn request_token_has_spec_class_id_and_properties() {
        let (mut alice, _bob, alice_h, alice_remote_bob, _, _) = alice_bob();
        let (_, out) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
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
    }

    #[test]
    fn cert_bind_replier_modifies_initiator_cert_in_reply_rejected() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
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
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
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
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
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
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, IdentityHandle(99))
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // Bob tries to reply → Alice's cert is not from BobCA → reject.
        let err = bob
            .begin_handshake_reply(bob_h, IdentityHandle(99), &req)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
        let _ = (rogue_cert, rogue_key);
    }

    #[test]
    fn replay_initiator_request_rejected_second_time() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // First reply: ok.
        let _ = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap();
        // Second reply with the *same* request → replay → reject.
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn truncated_request_token_rejected() {
        let (_alice, mut bob, _alice_h, _, bob_h, bob_remote_alice) = alice_bob();
        let err = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &[0u8, 1u8, 2u8, 3u8, 4u8])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn hash_c1_mismatch_in_request_rejected() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
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
            .begin_handshake_reply(bob_h, bob_remote_alice, &bad)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn cross_algorithm_dsign_mismatch_rejected() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
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
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    #[test]
    fn extra_unknown_properties_in_reply_accepted_forward_compat() {
        let (mut alice, mut bob, alice_h, alice_remote_bob, bob_h, bob_remote_alice) = alice_bob();
        let (alice_hs, out1) = alice
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // Adds an unknown property.
        let mut h = ht::DataHolder::from_cdr_le(&reply).unwrap();
        h.set_property("zerodds.future.feature", "yes");
        h.set_binary_property("zerodds.future.opaque", alloc::vec![0xFF; 8]);
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
            .begin_handshake_request(alice_h, alice_remote_bob)
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
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
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
        // With permissions: class_id-only PermissionsToken, spec-1.0 string
        // (cyclone/FastDDS announce exactly `DDS:Access:Permissions:1.0`
        // with empty properties — we mirror that byte-structurally).
        p.set_local_permissions(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let token = p.get_permissions_token();
        assert!(
            !token.is_empty(),
            "PermissionsToken missing despite set permissions"
        );
        let parsed = ht::DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(parsed.class_id, "DDS:Access:Permissions:1.0");
        assert!(
            parsed.properties.is_empty(),
            "PermissionsToken announce carries no properties (cyclone mirror)"
        );
    }

    #[test]
    fn validate_remote_identity_accepts_trusted_cert_der() {
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

        let remote_der = cert_der_from_pem(&cert_pem);
        let remote = plugin
            .validate_remote_identity(local, [0xBB; 16], &remote_der)
            .expect("trusted remote must be accepted");
        assert_ne!(remote, local);
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
            .validate_remote_identity(local, [0xBB; 16], &descriptor)
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
            .validate_remote_identity(local, [0xBB; 16], &minimal)
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
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
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
            .begin_handshake_request(alice_h, alice_remote_bob)
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_token = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
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
}
