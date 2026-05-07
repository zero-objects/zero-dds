// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `AuthenticationPlugin`-Impl auf Basis von X.509 / rustls-webpki.
//!
//! Spec: OMG DDS-Security 1.2 §10.3.2.6-8 + §10.3.4. Tokens werden ueber
//! das `DataHolder`-Wire-Format aus [`crate::handshake_token`] kodiert.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`webpki::EndEntityCert::verify_signature` nimmt
//! `&dyn SignatureVerificationAlgorithm` — das ist ein 3rd-party-API,
//! nicht ZeroDDS-Eigenkonstruktion. Wir koennen das nicht zu konkreten
//! Generics konvertieren ohne webpki-Fork.)

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use ring::rand::{SecureRandom, SystemRandom};
use ring::signature;
use rustls_pki_types::CertificateDer;
use zerodds_security::authentication::{
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle,
    SharedSecretHandle, SharedSecretProvider,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;
use zerodds_security_keyexchange::KeyExchange;

use crate::handshake_token::{
    self as ht, FinalBuildInput, ReplyBuildInput, RequestBuildInput, ct_eq, signing_bytes,
};
use crate::identity::{CertKeyAlgo, IdentityConfig, ParsedIdentity, PkiError};

/// Property-Keys (Spec §8.3.2.7 / implementation-defined). Wir folgen
/// der Fast-DDS-Konvention, damit Nutzer vorhandene XML-Configs
/// uebernehmen koennen.
mod keys {
    /// PEM-kodiertes Identity-Zertifikat (im Property-Value direkt,
    /// **nicht** als Dateipfad — so kann der Plugin-Caller entscheiden,
    /// ob er vom Filesystem laedt oder aus einem Secret-Manager).
    pub const IDENTITY_CERT: &str = "dds.sec.auth.identity_certificate";
    /// PEM-kodiertes CA-Bundle.
    pub const IDENTITY_CA: &str = "dds.sec.auth.identity_ca";
    /// PEM-kodierter PKCS8-Private-Key (passend zum Identity-Cert).
    pub const IDENTITY_KEY: &str = "dds.sec.auth.private_key";
}

/// Maximaler Speicher fuer "kuerzlich gesehene" Initiator-Challenges
/// (Replay-Detection-Cache pro Replier). 1024 entries × 32 byte = 32 KiB.
const REPLAY_CACHE_CAP: usize = 1024;

/// PKI/X.509-basierter `AuthenticationPlugin`. Verifiziert Identity-
/// Certs gegen einen vorgegebenen Trust-Anchor und fuehrt einen
/// 3-Round PKI-DH-Handshake (Spec §10.3.2.6-8 Tab.56/57/58) zum Peer.
///
/// Wire (C3.1): drei `DataHolder`-Tokens
/// `DDS:Auth:PKI-DH:1.2+AuthReq` / `+AuthReply` / `+AuthFinal`. Beide
/// Seiten echo'n `cert_der` + `dh*` + `challenge*` + Hash-Bindings.
/// Replier signiert (kagree || ch1 || dh1 || ch2 || dh2) mit
/// Identity-Private-Key; Initiator signiert (kagree || ch2 || dh2 ||
/// ch1 || dh1).
pub struct PkiAuthenticationPlugin {
    next_handle: AtomicU64,
    identities: BTreeMap<IdentityHandle, ParsedIdentity>,
    /// Initiator-Side: noch nicht abgeschlossene Handshakes.
    pending_initiator: BTreeMap<HandshakeHandle, InitiatorState>,
    /// Replier-Side: zwischen Reply-Send und Final-Empfang.
    pending_replier: BTreeMap<HandshakeHandle, ReplierState>,
    /// Abgeschlossene Handshakes → SharedSecret-Handle.
    handshake_to_secret: BTreeMap<HandshakeHandle, SharedSecretHandle>,
    /// Materialisierte SharedSecrets (32 byte HKDF-SHA256-Output).
    secrets: BTreeMap<SharedSecretHandle, Vec<u8>>,
    /// Pro lokalem Identity-Handle: Set aller bereits gesehenen
    /// Initiator-Challenges (Replay-Detection). Wird gecappt.
    replay_cache: BTreeMap<IdentityHandle, BTreeSet<[u8; 32]>>,
    /// FIFO-Order der replay-cache Entries fuer Cap-Eviction.
    replay_order: BTreeMap<IdentityHandle, Vec<[u8; 32]>>,
}

struct InitiatorState {
    /// Lokaler Identity-Handle (fuer Cert/Key-Lookup).
    local: IdentityHandle,
    /// Ephemerales DH-Keypair (verbraucht beim REPLY).
    kx: Option<KeyExchange>,
    /// Bytes des lokalen DH-Public.
    dh1: Vec<u8>,
    /// Lokal generierte Challenge.
    challenge1: [u8; 32],
    /// Hash-Bindung des Initiator-Tupels.
    hash_c1: [u8; 32],
    /// Permissions-Document (echo). Wird vom AccessControl-Plugin im
    /// Permissions-Bind-Schritt konsumiert; hier nur fuer Symmetrie
    /// zum Replier-State gehalten.
    #[allow(dead_code)]
    permissions: Vec<u8>,
    /// Pdata (echo).
    #[allow(dead_code)]
    pdata: Vec<u8>,
    /// Eigenes `c.kagree_algo` (was wir im REQUEST geschickt haben).
    kagree_algo: alloc::string::String,
    /// Eigenes `c.dsign_algo`. Wird vom Replier echoed und beim
    /// Reply-Decode auf Plausibilitaet geprueft (kein Match-Vergleich
    /// hier, weil der Initiator den Replier-Algo schickt).
    #[allow(dead_code)]
    dsign_algo: alloc::string::String,
}

struct ReplierState {
    /// Lokaler Identity-Handle.
    local: IdentityHandle,
    /// Vom Replier akzeptierter `c.kagree_algo` (vom Initiator uebernommen).
    kagree_algo: alloc::string::String,
    /// Initiator-Challenge.
    challenge1: [u8; 32],
    /// Replier-Challenge.
    challenge2: [u8; 32],
    /// Initiator-DH (echo).
    dh1: Vec<u8>,
    /// Replier-DH.
    dh2: Vec<u8>,
    /// hash_c1 (echo).
    hash_c1: [u8; 32],
    /// hash_c2 (replier-Tupel).
    hash_c2: [u8; 32],
    /// Bereits abgeleitetes SharedSecret.
    secret_handle: SharedSecretHandle,
    /// Initiator-Cert-DER (zur Final-Signature-Pruefung).
    initiator_cert_der: Vec<u8>,
    /// Detektierter Initiator-Cert-Algo.
    initiator_key_algo: CertKeyAlgo,
}

impl Default for PkiAuthenticationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PkiAuthenticationPlugin {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            identities: BTreeMap::new(),
            pending_initiator: BTreeMap::new(),
            pending_replier: BTreeMap::new(),
            handshake_to_secret: BTreeMap::new(),
            secrets: BTreeMap::new(),
            replay_cache: BTreeMap::new(),
            replay_order: BTreeMap::new(),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Programmatische Variante: direkt `IdentityConfig` statt
    /// `PropertyList` uebergeben. Nuetzlich fuer Tests + native
    /// Rust-Caller.
    ///
    /// # Errors
    /// Siehe [`PkiError`].
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

    /// Liest die rohen 32-byte SharedSecret-Bytes. Primär fuer Tests;
    /// im Produktions-Pfad wird der `SharedSecretHandle` an den
    /// CryptoPlugin durchgereicht, ohne dass der Caller die Bytes
    /// sieht.
    #[must_use]
    pub fn secret_bytes(&self, handle: SharedSecretHandle) -> Option<&[u8]> {
        self.secrets.get(&handle).map(Vec::as_slice)
    }

    fn store_secret(&mut self, _h: HandshakeHandle, bytes: Vec<u8>) -> SharedSecretHandle {
        let handle = SharedSecretHandle(self.next_id());
        self.secrets.insert(handle, bytes);
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
            let key = signature::EcdsaKeyPair::from_pkcs8(
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
            let mut out = alloc::vec![0u8; key.public().modulus_len()];
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
        CertKeyAlgo::EcdsaP256Sha256 => webpki::ring::ECDSA_P256_SHA256,
        CertKeyAlgo::RsaPssSha256 => webpki::ring::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
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

/// Leitet 32-byte SharedSecret aus DH + Challenges via HKDF-SHA256 ab.
/// Salt = (challenge1 || challenge2). Info = ZeroDDS-Domain-Separator.
fn derive_shared_secret(
    raw_dh: &[u8],
    challenge1: &[u8; 32],
    challenge2: &[u8; 32],
) -> SecurityResult<Vec<u8>> {
    use ring::hkdf;
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(challenge1);
    salt[32..].copy_from_slice(challenge2);
    let salt_obj = hkdf::Salt::new(hkdf::HKDF_SHA256, &salt);
    let prk = salt_obj.extract(raw_dh);
    let info = [b"DDS-Security-1.2-SharedSecret".as_slice()];
    let okm = prk.expand(&info, hkdf::HKDF_SHA256).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "pki: HKDF expand failed")
    })?;
    let mut out = [0u8; 32];
    okm.fill(&mut out).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "pki: HKDF fill failed")
    })?;
    Ok(out.to_vec())
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
                "pki: fehlt dds.sec.auth.identity_certificate",
            )
        })?;
        let ca = props.get(keys::IDENTITY_CA).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: fehlt dds.sec.auth.identity_ca",
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
                "pki: unbekannter lokaler IdentityHandle",
            )
        })?;
        parsed
            .verify_remote_der(remote_auth_token)
            .map_err(pki_to_security)?;
        let handle = IdentityHandle(self.next_id());
        Ok(handle)
    }

    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        _replier: IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let parsed = self.identities.get(&initiator).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unbekannter Initiator-IdentityHandle",
            )
        })?;
        let cert_der = parsed.cert_der.clone();
        let key_algo = parsed.key_algo;
        let dsign_algo = algo_for(key_algo)?.to_owned();
        let kagree_algo = ht::algo::X25519.to_owned();

        let kx = KeyExchange::new()?;
        let dh1 = kx.public_key().to_vec();
        let challenge1 = random_challenge()?;
        let permissions: Vec<u8> = Vec::new();
        let pdata: Vec<u8> = Vec::new();

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
        // 1) Parse + Hash-Re-Validation (passiert in parse_request_token).
        let req = ht::parse_request_token(request_token)?;

        // 2) Initiator-Cert gegen unseren Trust-Store validieren.
        let parsed = self.identities.get(&replier).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "pki: unbekannter Replier-IdentityHandle",
            )
        })?;
        parsed
            .verify_remote_der(&req.cert_der)
            .map_err(pki_to_security)?;

        // 3) Replay-Detection.
        self.record_challenge(replier, req.challenge1)?;

        // 4) Initiator-`c.dsign_algo` muss zu seinem Cert-Public-Key passen.
        let initiator_key_algo = detect_peer_algo(&req.cert_der);
        check_dsign_matches(&req.dsign_algo, initiator_key_algo)?;

        // 5) Eigenes DH-Pair generieren + challenge2.
        let kx = KeyExchange::new()?;
        let dh2 = kx.public_key().to_vec();
        let challenge2 = random_challenge()?;

        // 6) DH-Agreement → SharedSecret.
        let parsed = self.identities.get(&replier).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::Internal, "pki: replier identity gone")
        })?;
        // Sign with Replier-Key
        let priv_key = parsed.private_key_pkcs8_der.clone().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "pki: replier hat keinen private-key konfiguriert (kann nicht signieren)",
            )
        })?;
        let replier_cert_der = parsed.cert_der.clone();
        let replier_dsign = algo_for(parsed.key_algo)?.to_owned();
        let replier_key_algo = parsed.key_algo;

        // raw DH output (KeyExchange consumes self).
        let secret_bytes = kx.derive_shared_secret(&req.dh1)?;
        // HKDF-Re-Derive with challenges as salt for spec-aligned key.
        let final_secret = derive_shared_secret(&secret_bytes, &req.challenge1, &challenge2)?;

        // 7) Signatur ueber (kagree || ch1 || dh1 || ch2 || dh2).
        let to_sign = signing_bytes(
            &req.kagree_algo,
            &req.challenge1,
            &req.dh1,
            &challenge2,
            &dh2,
        );
        let signature = sign_with(replier_key_algo, &priv_key, &to_sign)?;

        // 8) Replier-hash_c2 = ueber Replier-Tupel (echo kagree, eigenes
        //    cert/perm/pdata/dsign).
        let permissions: Vec<u8> = Vec::new();
        let pdata: Vec<u8> = Vec::new();
        let hash_c2 = ht::compute_hash_c(
            &replier_cert_der,
            &permissions,
            &pdata,
            &replier_dsign,
            &req.kagree_algo,
        );

        // 9) Reply-Token bauen.
        let reply_bytes = ht::build_reply_token(&ReplyBuildInput {
            cert_der: &replier_cert_der,
            permissions: &permissions,
            pdata: &pdata,
            dsign_algo: &replier_dsign,
            kagree_algo: &req.kagree_algo,
            dh2: &dh2,
            challenge2: &challenge2,
            hash_c1: &req.hash_c1,
            dh1: &req.dh1,
            challenge1: &req.challenge1,
            ocsp_status: &[],
            signature: &signature,
        })?;

        // 10) State persistieren — Final-Empfang braucht das.
        let handle = HandshakeHandle(self.next_id());
        let secret_handle = self.store_secret(handle, final_secret);
        self.handshake_to_secret.insert(handle, secret_handle);
        self.pending_replier.insert(
            handle,
            ReplierState {
                local: replier,
                kagree_algo: req.kagree_algo,
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
        // Initiator-Seite hat einen pending_initiator-Eintrag → Token = REPLY.
        if self.pending_initiator.contains_key(&handshake) {
            return self.process_reply_on_initiator(handshake, token);
        }
        // Replier-Seite hat einen pending_replier-Eintrag → Token = FINAL.
        if self.pending_replier.contains_key(&handshake) {
            return self.process_final_on_replier(handshake, token);
        }
        Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "pki: unbekannter HandshakeHandle",
        ))
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        self.handshake_to_secret
            .get(&handshake)
            .copied()
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "pki: handshake-handle unbekannt oder noch nicht completed",
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

        // a) Echo-Konsistenz: hash_c1, dh1, challenge1 muessen 1:1 stimmen.
        if !ct_eq(&reply.hash_c1, &st.hash_c1) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: hash_c1 echo mismatch (cert-bind broken)",
            ));
        }
        if !ct_eq(&reply.dh1, &st.dh1) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: dh1 echo mismatch",
            ));
        }
        if !ct_eq(&reply.challenge1, &st.challenge1) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: challenge1 echo mismatch",
            ));
        }
        if reply.kagree_algo != st.kagree_algo {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: kagree_algo mismatch",
            ));
        }

        // b) Replier-Cert validieren.
        // Snapshot der lokalen Identity, damit wir spaeter mutable
        // borrowen koennen (store_secret).
        let (priv_key, initiator_key_algo) = {
            let parsed = self.identities.get(&st.local).ok_or_else(|| {
                SecurityError::new(SecurityErrorKind::Internal, "pki: initiator identity gone")
            })?;
            parsed
                .verify_remote_der(&reply.cert_der)
                .map_err(pki_to_security)?;
            let pk = parsed.private_key_pkcs8_der.clone().ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::InvalidConfiguration,
                    "pki: initiator hat keinen private-key (final-sign nicht moeglich)",
                )
            })?;
            (pk, parsed.key_algo)
        };
        let replier_key_algo = detect_peer_algo(&reply.cert_der);
        check_dsign_matches(&reply.dsign_algo, replier_key_algo)?;

        // c) Replier-Signature pruefen.
        let to_verify = signing_bytes(
            &reply.kagree_algo,
            &reply.challenge1,
            &reply.dh1,
            &reply.challenge2,
            &reply.dh2,
        );
        verify_signature_with_cert(
            &reply.cert_der,
            replier_key_algo,
            &to_verify,
            &reply.signature,
        )?;

        // d) DH-Agreement.
        let kx = st.kx.ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::Internal, "pki: ephemeral kx gone")
        })?;
        let raw = kx.derive_shared_secret(&reply.dh2)?;
        let final_secret = derive_shared_secret(&raw, &st.challenge1, &reply.challenge2)?;

        let secret_handle = self.store_secret(handshake, final_secret);
        self.handshake_to_secret.insert(handshake, secret_handle);

        // e) Final-Token bauen + signieren.
        let to_sign = signing_bytes(
            &reply.kagree_algo,
            &reply.challenge2,
            &reply.dh2,
            &reply.challenge1,
            &reply.dh1,
        );
        let signature = sign_with(initiator_key_algo, &priv_key, &to_sign)?;
        let final_token = ht::build_final_token(&FinalBuildInput {
            hash_c1: &reply.hash_c1,
            hash_c2: &reply.hash_c2,
            dh1: &reply.dh1,
            dh2: &reply.dh2,
            challenge1: &reply.challenge1,
            challenge2: &reply.challenge2,
            ocsp_status: &[],
            signature: &signature,
        })?;

        // Spec §10.3.2.10: Initiator schickt `final_token` UND ist
        // **complete**. Wir tunneln beides via `SendMessage` + Lookup
        // ueber `shared_secret()`. Damit die Wire-Schicht den Token
        // sieht, returnen wir SendMessage; die DCPS-Runtime ruft danach
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

        // Echo-Konsistenz.
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

        // Initiator-Signature ueber (kagree || ch2 || dh2 || ch1 || dh1).
        let to_verify = signing_bytes(
            &st.kagree_algo,
            &st.challenge2,
            &st.dh2,
            &st.challenge1,
            &st.dh1,
        );
        verify_signature_with_cert(
            &st.initiator_cert_der,
            st.initiator_key_algo,
            &to_verify,
            &final_tok.signature,
        )?;

        // Wir nehmen den Local-Handle nur wegen `local`-Field, damit der
        // Lint nicht meckert; produktiv koennten wir es fuer
        // Audit-Logging nutzen.
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

    /// Erzeugt ein CA + End-Entity-Cert + dazugehoerigen Private-Key
    /// (PKCS8-PEM) — alles fuer rcgen-Default ECDSA-P256.
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
        // Beide nutzen ihre eigenen CAs, aber wir muessen beide in das
        // Trust-Bundle der Gegenseite reinpacken, damit die Cert-Chain
        // valide ist. Stattdessen: beide nehmen die Alice-CA als
        // Trust-Anchor und Bob's Cert wird unter dieser CA gefaked.
        // Einfacher: beide regenerieren mit gemeinsamer CA.
        let _ = (b_cert, b_key);

        // Cleaner approach: gemeinsame CA.
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
    // C3.1 — Spec-konformer Handshake (Tab.56/57/58)
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
        assert!(req_token.len() > 100, "request token enthaelt cert + DH");

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
            "alice + bob muessen identisches secret ableiten"
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
        assert_eq!(parsed.class_id, "DDS:Auth:PKI-DH:1.2+AuthReq");
        assert!(parsed.property("c.dsign_algo").is_some());
        assert!(parsed.property("c.kagree_algo").is_some());
        for k in [
            "c.id",
            "c.perm",
            "c.pdata",
            "hash_c1",
            "dh1",
            "challenge1",
            "ocsp_status",
        ] {
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

        // Tamper: kippe ein bit in hash_c1 → Echo-Mismatch beim Initiator.
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

        // Kippt dh2 → Initiator-Sig-Verify schlaegt fehl (signature
        // covers dh2). Wenn das durchrutschen wuerde, wuerde die Final-
        // Sig divergieren.
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
        // Bob hat die Trusted-CA, aber Alice's cert ist von Rogue-CA.
        // Wir mounten Bob mit der Trusted-CA, Alice mit ihrer Rogue-CA.

        // Alice braucht eine CA die ihr Cert akzeptiert — also nehmen
        // wir den Pfad: Alice valide-with-config laed mit der Rogue-CA
        // (selbe rogue-ca aus der Helper). Aber make_cert_with_wrong_ca
        // gibt nur das EE-Cert + die Trusted-CA zurück. Wir muessen die
        // Rogue-CA selbst rebuilden — pragmatischer: Alice+Bob mit
        // derselben gemeinsamen CA, aber Bob setzt seinen Trust-Anchor
        // auf eine *andere* CA → Initiator-Cert wird beim Reply-Schritt
        // rejected.

        use rcgen::{CertificateParams, KeyPair};
        // Common CA fuer Alice's identity:
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

        // Bob mit completely different CA (and own cert):
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

        // Alice kennt Bob nicht (anderes CA), aber sie versucht trotzdem
        // einen Handshake zu starten. Wir koennen alice_remote_bob nicht
        // ueber validate_remote_identity holen — also faken wir mit
        // einem gueltigen Stub-Handle. Der Test prueft nur den
        // Replier-Path.
        let (_alice_hs, out1) = alice
            .begin_handshake_request(alice_h, IdentityHandle(99))
            .unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        // Bob versucht zu replyen → Cert von Alice ist nicht von BobCA → reject.
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
        // Erstes Reply: ok.
        let _ = bob
            .begin_handshake_reply(bob_h, bob_remote_alice, &req)
            .unwrap();
        // Zweites Reply mit *gleichem* request → replay → reject.
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
        // Manipuliere hash_c1 → parse_request_token rejected.
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
        // Der Token sagt "RSASSA-PSS-SHA256" obwohl das Cert ECDSA ist.
        // Aber: hash_c1 enthaelt das ALTE algo → wenn wir nur dsign_algo
        // tauschen, wird hash_c1 mismatchen. Wir muessen also auch
        // hash_c1 neu berechnen und das `c.dsign_algo` aendern, dann
        // sollte die Algo-Cross-Check beim Replier-Step greifen.
        let mut h = ht::DataHolder::from_cdr_le(&req).unwrap();
        h.set_property("c.dsign_algo", "RSASSA-PSS-SHA256");
        let cert_der = h.binary_property("c.id").unwrap().to_vec();
        let perm = h.binary_property("c.perm").unwrap().to_vec();
        let pdata = h.binary_property("c.pdata").unwrap().to_vec();
        let kagree = h.property("c.kagree_algo").unwrap().to_owned();
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
        // Fuegt ein unbekanntes Property hinzu.
        let mut h = ht::DataHolder::from_cdr_le(&reply).unwrap();
        h.set_property("zerodds.future.feature", "yes");
        h.set_binary_property("zerodds.future.opaque", alloc::vec![0xFF; 8]);
        // hash_c2 ist ueber die ORIGINAL-Properties. Da wir nur EXTRA-
        // Felder hinzufuegen, bleiben die hash-Inputs gleich → ok.
        let new_reply = h.to_cdr_le();
        let res = alice.process_handshake(alice_hs, &new_reply);
        assert!(
            res.is_ok(),
            "forward-compat extra props muessen akzeptiert werden"
        );
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
        // alice setzt keine permissions in diesem Test (kein
        // AccessControl-Bind im Authentication-only-Roundtrip).
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
    // Mutation-Killer (2026-05-01)
    // -------------------------------------------------------------

    /// Faengt Mutation `>` -> `>=` und `>` -> `==` auf der replay-cache-
    /// Eviction-Boundary. Der Cache MUSS GENAU REPLAY_CACHE_CAP
    /// Eintraege halten koennen — Eviction tritt erst beim CAP+1-ten ein.
    ///
    /// Test-Strategie: nach EXAKT CAP unique Inserts pruefen, ob der
    /// erste noch im Cache ist (= Replay-Reject).
    /// * Original `>`: 1024 > 1024 false → keine Eviction → 0 noch drin → Reject.
    /// * Mutation `==`: 1024 == 1024 true → 0 evicted → kein Reject.
    /// * Mutation `>=`: gleichfalls evicted bei 1024 → kein Reject.
    #[test]
    fn replay_cache_holds_exactly_cap_entries_before_eviction() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        // Genau CAP unique. Der erste (i=0) MUSS noch drin sein → Replay-Reject.
        let mut c_first = [0u8; 32];
        c_first[0..8].copy_from_slice(&0u64.to_le_bytes());
        let err = alice.record_challenge(alice_h, c_first).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// Eviction tritt beim CAP+1-ten Eintrag ein — Test des
    /// Eviction-Pfads selbst (FIFO: aelteste Eintragung wird evicted).
    #[test]
    fn replay_cache_evicts_oldest_at_cap_plus_one() {
        let (mut alice, _bob, alice_h, _, _, _) = alice_bob();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        // CAP+1: Eviction triggert auf dem aeltesten (i=0).
        let mut c_extra = [0u8; 32];
        c_extra[0..8].copy_from_slice(&(REPLAY_CACHE_CAP as u64).to_le_bytes());
        alice.record_challenge(alice_h, c_extra).unwrap();

        // Der erste (i=0) sollte jetzt evicted sein → re-insert erfolgreich.
        let mut c_first = [0u8; 32];
        c_first[0..8].copy_from_slice(&0u64.to_le_bytes());
        alice
            .record_challenge(alice_h, c_first)
            .expect("after CAP+1 inserts, oldest must be evicted");

        // Ein junger Eintrag (z.B. der CAP-te) muss noch drin sein.
        // Nach insert von 0 wurde 1 evicted (FIFO), also testen wir
        // einen sicher noch-vorhandenen Eintrag — der CAP-te (=1024).
        let mut c_recent = [0u8; 32];
        c_recent[0..8].copy_from_slice(&(REPLAY_CACHE_CAP as u64).to_le_bytes());
        let err = alice.record_challenge(alice_h, c_recent).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// Faengt Mutation `get_shared_secret -> None / Some(vec![]) / Some(vec![0]) / Some(vec![1])`.
    /// Nach erfolgreichem Handshake muss `get_shared_secret` das
    /// gespeicherte 32-Byte-Secret zurueckgeben, nicht None oder einen
    /// pauschalen Wert.
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

        // get_shared_secret muss konkrete 32 Byte zurueckgeben:
        let alice_bytes = SharedSecretProvider::get_shared_secret(&alice, alice_handle).unwrap();
        let bob_bytes = SharedSecretProvider::get_shared_secret(&bob, bob_handle).unwrap();
        // Mutation `None`: unwrap panicked → test fails.
        // Mutation `Some(vec![])`: len=0 != 32.
        // Mutation `Some(vec![0])` / `Some(vec![1])`: len=1 != 32.
        assert_eq!(alice_bytes.len(), 32);
        assert_eq!(bob_bytes.len(), 32);
        assert_eq!(
            alice_bytes, bob_bytes,
            "shared secrets muessen identisch + nicht trivial sein"
        );
        assert!(alice_bytes.iter().any(|&b| b != 0));
        assert!(alice_bytes.iter().any(|&b| b != 1));

        // get_shared_secret mit unbekanntem Handle muss None liefern.
        let bogus_handle = zerodds_security::authentication::SharedSecretHandle(0xDEAD_BEEF);
        assert!(SharedSecretProvider::get_shared_secret(&alice, bogus_handle).is_none());
    }

    /// Faengt Mutationen `||` -> `&&` auf jedem der 6 Echo-Checks in
    /// `process_final_on_replier`. Pro Field je ein Test — wenn EIN
    /// Field nicht stimmt, muss bob den Final-Token rejecten.
    /// Mit `&&`: nur ALLE 6 Mismatches wuerden rejecten.
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
            "tampered final-token muss AuthFailed liefern"
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
