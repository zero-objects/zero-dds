// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Delegation-Link / Delegation-Chain.
//!
//! Datenmodell + Sign/Verify fuer kryptographische Delegations-Ketten.
//! Architektur-Referenz: `docs/architecture/09_delegation.md` §5.
//!
//! Ein [`DelegationLink`] traegt eine signierte Aussage:
//! "Delegator (mit X.509-Cert) erlaubt Delegatee (identifiziert via
//! `delegatee_guid`), in einem definierten Zeit- und Scope-Fenster
//! Samples zu schreiben/lesen."
//!
//! Eine [`DelegationChain`] verkettet 1..N solcher Links zu einer
//! Beweisreihe Origin → Edge. Jeder Link wird vom **vorigen
//! Delegatee** signiert (Initial-Link vom Trust-Anchor selbst).
//!
//! # Layout (deterministisch fuer Signing-Input)
//!
//! ```text
//! magic        = b"ZERODDSD" (8 byte)
//! version      = 1 (u8)
//! delegator    = 16 byte GUID
//! delegatee    = 16 byte GUID
//! not_before   = i64 big-endian (Unix-Sekunden)
//! not_after    = i64 big-endian
//! algorithm    = u8 (siehe SignatureAlgorithm::wire_id)
//! n_topics     = u32 big-endian
//! [u32_be len + utf8_bytes] * n_topics
//! n_partitions = u32 big-endian
//! [u32_be len + utf8_bytes] * n_partitions
//! ```
//!
//! Signature-Layout: derselbe Input wie oben (ohne `signature`-Suffix),
//! Hash impliziert durch `SignatureAlgorithm` (SHA-256 fuer P-256 +
//! Ed25519, SHA-384 fuer P-384, SHA-256 fuer RSA-PSS-2048).
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Cert-Validation via `rustls-webpki` benoetigt object-safe Trait-Refs.)

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use ring::rand::SystemRandom;
use ring::signature::{
    self, ECDSA_P256_SHA256_FIXED, ECDSA_P384_SHA384_FIXED, ED25519, RSA_PSS_2048_8192_SHA256,
    UnparsedPublicKey,
};

/// Signatur-Algorithmus fuer Delegation-Links.
///
/// Default-Empfehlung: [`SignatureAlgorithm::EcdsaP256`] — kompakte
/// Signatur (~64 byte), schnelle Verify auf Edge-Hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SignatureAlgorithm {
    /// ECDSA mit P-256 + SHA-256, fixed-length encoding (64 byte sig).
    EcdsaP256,
    /// ECDSA mit P-384 + SHA-384, fixed-length encoding (96 byte sig).
    EcdsaP384,
    /// RSA-PSS mit 2048-bit Key + SHA-256 (256 byte sig).
    RsaPss2048,
    /// Ed25519 (64 byte sig).
    Ed25519,
}

impl SignatureAlgorithm {
    /// Wire-Id fuer das deterministische Sign-Input-Layout.
    #[must_use]
    pub const fn wire_id(self) -> u8 {
        match self {
            Self::EcdsaP256 => 1,
            Self::EcdsaP384 => 2,
            Self::RsaPss2048 => 3,
            Self::Ed25519 => 4,
        }
    }

    /// Decode aus Wire-Id; `None` bei unknown-Algorithm.
    #[must_use]
    pub const fn from_wire_id(id: u8) -> Option<Self> {
        match id {
            1 => Some(Self::EcdsaP256),
            2 => Some(Self::EcdsaP384),
            3 => Some(Self::RsaPss2048),
            4 => Some(Self::Ed25519),
            _ => None,
        }
    }

    /// Erwartete Signatur-Laenge in Bytes (fixed) oder `None` fuer
    /// variabel (RSA-PSS abhaengig von Key-Size — wir fixieren auf 2048
    /// und damit 256 byte).
    #[must_use]
    pub const fn expected_signature_len(self) -> Option<usize> {
        match self {
            Self::EcdsaP256 | Self::Ed25519 => Some(64),
            Self::EcdsaP384 => Some(96),
            Self::RsaPss2048 => Some(256),
        }
    }
}

/// Magic-Bytes fuer das Sign-Input-Layout (8 byte).
pub const DELEGATION_MAGIC: &[u8; 8] = b"ZERODDSD";

/// Wire-Layout-Version.
pub const DELEGATION_VERSION: u8 = 1;

/// Maximale Anzahl Topic-Patterns pro Link (DoS-Cap).
pub const MAX_TOPIC_PATTERNS: usize = 64;
/// Maximale Anzahl Partition-Patterns pro Link (DoS-Cap).
pub const MAX_PARTITION_PATTERNS: usize = 64;
/// Maximale Pattern-String-Laenge in Bytes.
pub const MAX_PATTERN_LEN: usize = 256;

/// Errors aus dem Delegation-Modul.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DelegationError {
    /// Pattern-Liste ist groesser als der DoS-Cap.
    TooManyPatterns {
        /// Welche Liste — `"topic"` oder `"partition"`.
        kind: &'static str,
        /// Tatsaechliche Anzahl.
        count: usize,
        /// Erlaubtes Maximum.
        max: usize,
    },
    /// Pattern ist laenger als der String-Cap.
    PatternTooLong {
        /// Tatsaechliche Laenge.
        len: usize,
        /// Erlaubtes Maximum.
        max: usize,
    },
    /// Sign-Operation fehlgeschlagen (z.B. ungueltiger Key).
    SignFailed(String),
    /// Verify-Operation fehlgeschlagen.
    VerifyFailed(String),
    /// Wire-Bytes konnten nicht decodiert werden.
    Malformed(String),
    /// Unsupported Wire-Algorithm-Id.
    UnknownAlgorithm(u8),
    /// `not_before > not_after`.
    InvalidTimeWindow,
    /// Magic-Bytes stimmen nicht.
    BadMagic,
    /// Wire-Layout-Version unbekannt.
    UnsupportedVersion(u8),
}

impl core::fmt::Display for DelegationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooManyPatterns { kind, count, max } => {
                write!(f, "{kind} patterns: {count} > max {max}")
            }
            Self::PatternTooLong { len, max } => {
                write!(f, "pattern length {len} > max {max}")
            }
            Self::SignFailed(s) => write!(f, "sign failed: {s}"),
            Self::VerifyFailed(s) => write!(f, "verify failed: {s}"),
            Self::Malformed(s) => write!(f, "malformed delegation: {s}"),
            Self::UnknownAlgorithm(id) => write!(f, "unknown algorithm id: {id}"),
            Self::InvalidTimeWindow => write!(f, "not_before > not_after"),
            Self::BadMagic => write!(f, "bad magic bytes"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported version: {v}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DelegationError {}

/// Result-Alias fuer Delegation-Operationen.
pub type DelegationResult<T> = Result<T, DelegationError>;

/// Eine einzelne Delegation-Aussage (signiert vom Delegator).
///
/// Architektur-Referenz: `09_delegation.md` §5.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationLink {
    /// 16-byte Participant-GUID des Delegators (= Eigentuemer des
    /// Signing-Keys, gleichzeitig Subject des X.509-Certs, das den
    /// Verify-Key liefert).
    pub delegator_guid: [u8; 16],
    /// 16-byte Participant-GUID des Delegatees (= Edge-Peer, der ueber
    /// diesen Link Berechtigung erbt).
    pub delegatee_guid: [u8; 16],
    /// Erlaubte Topic-Glob-Patterns. Leere Liste = "alle" (nur in
    /// Initial-Link, sonst Validation-Fail).
    pub allowed_topic_patterns: Vec<String>,
    /// Erlaubte Partition-Patterns. Leere Liste = Default-Partition.
    pub allowed_partition_patterns: Vec<String>,
    /// Unix-Sekunden ab wann der Link gilt.
    pub not_before: i64,
    /// Unix-Sekunden bis wann der Link gilt.
    pub not_after: i64,
    /// Signatur-Algorithmus, mit dem der Link signiert ist.
    pub algorithm: SignatureAlgorithm,
    /// Signatur-Bytes (Layout abhaengig von `algorithm`).
    pub signature: Vec<u8>,
}

impl DelegationLink {
    /// Erzeugt einen unsigned Link (signature leer). Nutze
    /// [`Self::sign`] zum Signieren.
    ///
    /// # Errors
    /// * [`DelegationError::TooManyPatterns`] wenn `topic_patterns` oder
    ///   `partition_patterns` den DoS-Cap ueberschreiten.
    /// * [`DelegationError::PatternTooLong`] wenn ein Einzel-Pattern den
    ///   String-Cap ueberschreitet.
    /// * [`DelegationError::InvalidTimeWindow`] wenn
    ///   `not_before > not_after`.
    pub fn new(
        delegator_guid: [u8; 16],
        delegatee_guid: [u8; 16],
        allowed_topic_patterns: Vec<String>,
        allowed_partition_patterns: Vec<String>,
        not_before: i64,
        not_after: i64,
        algorithm: SignatureAlgorithm,
    ) -> DelegationResult<Self> {
        if allowed_topic_patterns.len() > MAX_TOPIC_PATTERNS {
            return Err(DelegationError::TooManyPatterns {
                kind: "topic",
                count: allowed_topic_patterns.len(),
                max: MAX_TOPIC_PATTERNS,
            });
        }
        if allowed_partition_patterns.len() > MAX_PARTITION_PATTERNS {
            return Err(DelegationError::TooManyPatterns {
                kind: "partition",
                count: allowed_partition_patterns.len(),
                max: MAX_PARTITION_PATTERNS,
            });
        }
        for p in allowed_topic_patterns
            .iter()
            .chain(allowed_partition_patterns.iter())
        {
            if p.len() > MAX_PATTERN_LEN {
                return Err(DelegationError::PatternTooLong {
                    len: p.len(),
                    max: MAX_PATTERN_LEN,
                });
            }
        }
        if not_before > not_after {
            return Err(DelegationError::InvalidTimeWindow);
        }
        Ok(Self {
            delegator_guid,
            delegatee_guid,
            allowed_topic_patterns,
            allowed_partition_patterns,
            not_before,
            not_after,
            algorithm,
            signature: Vec::new(),
        })
    }

    /// Sign-Input (deterministisch). Identisch fuer Sign + Verify.
    #[must_use]
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64 + 32 * self.allowed_topic_patterns.len());
        buf.extend_from_slice(DELEGATION_MAGIC);
        buf.push(DELEGATION_VERSION);
        buf.extend_from_slice(&self.delegator_guid);
        buf.extend_from_slice(&self.delegatee_guid);
        buf.extend_from_slice(&self.not_before.to_be_bytes());
        buf.extend_from_slice(&self.not_after.to_be_bytes());
        buf.push(self.algorithm.wire_id());

        // Topic-Patterns.
        let n_topic = u32::try_from(self.allowed_topic_patterns.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&n_topic.to_be_bytes());
        for p in &self.allowed_topic_patterns {
            let len = u32::try_from(p.len()).unwrap_or(u32::MAX);
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(p.as_bytes());
        }

        // Partition-Patterns.
        let n_part = u32::try_from(self.allowed_partition_patterns.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&n_part.to_be_bytes());
        for p in &self.allowed_partition_patterns {
            let len = u32::try_from(p.len()).unwrap_or(u32::MAX);
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(p.as_bytes());
        }

        buf
    }

    /// Signiert den Link mit `signing_key`. `signing_key`-Format:
    /// * `EcdsaP256` / `EcdsaP384` — PKCS#8-DER ECDSA-Privatekey.
    /// * `RsaPss2048` — PKCS#8-DER RSA-Privatekey.
    /// * `Ed25519` — PKCS#8-DER Ed25519-Privatekey (rfc8410-Format).
    ///
    /// Setzt `self.signature` auf den Sign-Output.
    ///
    /// # Errors
    /// [`DelegationError::SignFailed`] bei Key-Parsing-Fehler oder
    /// `ring`-internem Fehlschlag.
    pub fn sign(&mut self, signing_key_pkcs8: &[u8]) -> DelegationResult<()> {
        let input = self.signing_bytes();
        let sig = match self.algorithm {
            SignatureAlgorithm::EcdsaP256 => sign_ecdsa(
                &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                signing_key_pkcs8,
                &input,
            )?,
            SignatureAlgorithm::EcdsaP384 => sign_ecdsa(
                &signature::ECDSA_P384_SHA384_FIXED_SIGNING,
                signing_key_pkcs8,
                &input,
            )?,
            SignatureAlgorithm::RsaPss2048 => sign_rsa_pss(signing_key_pkcs8, &input)?,
            SignatureAlgorithm::Ed25519 => sign_ed25519(signing_key_pkcs8, &input)?,
        };
        self.signature = sig;
        Ok(())
    }

    /// Verifiziert den Link gegen einen **Public-Key-DER**, dessen
    /// Format pro Algorithmus unterschiedlich ist:
    /// * `EcdsaP256`/`EcdsaP384` — `SubjectPublicKeyInfo`-Bytes
    ///   (Uncompressed-Point-Encoding nach SEC1 §2.3.3).
    /// * `RsaPss2048` — `SubjectPublicKeyInfo`-Bytes (RFC 8017).
    /// * `Ed25519` — 32 byte raw-Public-Key.
    ///
    /// In der Praxis wird der Verify-Key aus dem **Delegator-X.509-Cert**
    /// extrahiert (siehe `delegation_check.rs` in delegation_check (security-permissions)).
    ///
    /// # Errors
    /// * [`DelegationError::VerifyFailed`] bei Signatur-Mismatch oder
    ///   Public-Key-Parse-Fehler.
    /// * [`DelegationError::SignFailed`] wenn `signature` leer ist.
    pub fn verify(&self, verify_public_key: &[u8]) -> DelegationResult<()> {
        if self.signature.is_empty() {
            return Err(DelegationError::SignFailed("empty signature".to_string()));
        }
        if let Some(expected_len) = self.algorithm.expected_signature_len() {
            if self.signature.len() != expected_len {
                return Err(DelegationError::VerifyFailed(alloc::format!(
                    "sig len {} != expected {}",
                    self.signature.len(),
                    expected_len
                )));
            }
        }
        let input = self.signing_bytes();
        let alg: &dyn signature::VerificationAlgorithm = match self.algorithm {
            SignatureAlgorithm::EcdsaP256 => &ECDSA_P256_SHA256_FIXED,
            SignatureAlgorithm::EcdsaP384 => &ECDSA_P384_SHA384_FIXED,
            SignatureAlgorithm::RsaPss2048 => &RSA_PSS_2048_8192_SHA256,
            SignatureAlgorithm::Ed25519 => &ED25519,
        };
        let pk = UnparsedPublicKey::new(alg, verify_public_key);
        pk.verify(&input, &self.signature)
            .map_err(|_| DelegationError::VerifyFailed("ring::verify".to_string()))
    }

    /// Wire-Encoding eines kompletten Links (Sign-Input + Signatur-Suffix).
    ///
    /// Layout = `signing_bytes()` || `u16_be(sig_len)` || `signature`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = self.signing_bytes();
        let sig_len = u16::try_from(self.signature.len()).unwrap_or(u16::MAX);
        buf.extend_from_slice(&sig_len.to_be_bytes());
        buf.extend_from_slice(&self.signature);
        buf
    }

    /// Decode eines kompletten Links.
    ///
    /// Konsumiert exakt soviele Bytes wie der Link gross ist; gibt den
    /// Rest des Slices als `tail` zurueck (Chain-Decoder reicht das
    /// weiter an den naechsten Link).
    ///
    /// # Errors
    /// [`DelegationError::Malformed`] wenn das Layout nicht stimmt.
    pub fn decode(buf: &[u8]) -> DelegationResult<(Self, &[u8])> {
        let mut p = 0usize;
        let need = |needed: usize, p: usize, buf: &[u8]| -> DelegationResult<()> {
            if buf.len() < p + needed {
                return Err(DelegationError::Malformed(alloc::format!(
                    "truncated at offset {p}, needed {needed} bytes"
                )));
            }
            Ok(())
        };

        need(8, p, buf)?;
        if &buf[p..p + 8] != DELEGATION_MAGIC {
            return Err(DelegationError::BadMagic);
        }
        p += 8;

        need(1, p, buf)?;
        let version = buf[p];
        if version != DELEGATION_VERSION {
            return Err(DelegationError::UnsupportedVersion(version));
        }
        p += 1;

        need(16, p, buf)?;
        let mut delegator_guid = [0u8; 16];
        delegator_guid.copy_from_slice(&buf[p..p + 16]);
        p += 16;

        need(16, p, buf)?;
        let mut delegatee_guid = [0u8; 16];
        delegatee_guid.copy_from_slice(&buf[p..p + 16]);
        p += 16;

        need(8, p, buf)?;
        let not_before = i64::from_be_bytes(buf[p..p + 8].try_into().unwrap_or([0u8; 8]));
        p += 8;

        need(8, p, buf)?;
        let not_after = i64::from_be_bytes(buf[p..p + 8].try_into().unwrap_or([0u8; 8]));
        p += 8;

        need(1, p, buf)?;
        let algo_id = buf[p];
        p += 1;
        let algorithm = SignatureAlgorithm::from_wire_id(algo_id)
            .ok_or(DelegationError::UnknownAlgorithm(algo_id))?;

        // Topic-Patterns.
        need(4, p, buf)?;
        let n_topic = u32::from_be_bytes(buf[p..p + 4].try_into().unwrap_or([0u8; 4])) as usize;
        p += 4;
        if n_topic > MAX_TOPIC_PATTERNS {
            return Err(DelegationError::TooManyPatterns {
                kind: "topic",
                count: n_topic,
                max: MAX_TOPIC_PATTERNS,
            });
        }
        let mut allowed_topic_patterns = Vec::with_capacity(n_topic);
        for _ in 0..n_topic {
            need(4, p, buf)?;
            let len = u32::from_be_bytes(buf[p..p + 4].try_into().unwrap_or([0u8; 4])) as usize;
            p += 4;
            if len > MAX_PATTERN_LEN {
                return Err(DelegationError::PatternTooLong {
                    len,
                    max: MAX_PATTERN_LEN,
                });
            }
            need(len, p, buf)?;
            let s = core::str::from_utf8(&buf[p..p + len])
                .map_err(|e| DelegationError::Malformed(alloc::format!("utf8 topic: {e}")))?;
            allowed_topic_patterns.push(s.to_string());
            p += len;
        }

        // Partition-Patterns.
        need(4, p, buf)?;
        let n_part = u32::from_be_bytes(buf[p..p + 4].try_into().unwrap_or([0u8; 4])) as usize;
        p += 4;
        if n_part > MAX_PARTITION_PATTERNS {
            return Err(DelegationError::TooManyPatterns {
                kind: "partition",
                count: n_part,
                max: MAX_PARTITION_PATTERNS,
            });
        }
        let mut allowed_partition_patterns = Vec::with_capacity(n_part);
        for _ in 0..n_part {
            need(4, p, buf)?;
            let len = u32::from_be_bytes(buf[p..p + 4].try_into().unwrap_or([0u8; 4])) as usize;
            p += 4;
            if len > MAX_PATTERN_LEN {
                return Err(DelegationError::PatternTooLong {
                    len,
                    max: MAX_PATTERN_LEN,
                });
            }
            need(len, p, buf)?;
            let s = core::str::from_utf8(&buf[p..p + len])
                .map_err(|e| DelegationError::Malformed(alloc::format!("utf8 part: {e}")))?;
            allowed_partition_patterns.push(s.to_string());
            p += len;
        }

        // Signatur.
        need(2, p, buf)?;
        let sig_len = u16::from_be_bytes(buf[p..p + 2].try_into().unwrap_or([0u8; 2])) as usize;
        p += 2;
        need(sig_len, p, buf)?;
        let signature = buf[p..p + sig_len].to_vec();
        p += sig_len;

        let link = Self {
            delegator_guid,
            delegatee_guid,
            allowed_topic_patterns,
            allowed_partition_patterns,
            not_before,
            not_after,
            algorithm,
            signature,
        };
        Ok((link, &buf[p..]))
    }
}

/// Verkettete Delegation: Origin (Trust-Anchor) → ... → Edge.
///
/// Architektur-Referenz: `09_delegation.md` §5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationChain {
    /// 16-byte GUID des Origin-Participants. Bei 1-Hop = Delegator des
    /// einzigen Links; bei N-Hop = Delegator des **ersten** Links.
    pub origin_guid: [u8; 16],
    /// Links in Order: links[0] = Origin → erste Zwischenstufe,
    /// links[N-1] = letzte Zwischenstufe → Edge.
    pub links: Vec<DelegationLink>,
}

/// Maximale Chain-Tiefe (DoS-Cap, hart). Profile-Override siehe
/// `DelegationProfile::max_chain_depth`.
pub const MAX_CHAIN_DEPTH_HARD_CAP: usize = 8;

impl DelegationChain {
    /// Konstruktor, prueft Hop-Cap.
    ///
    /// # Errors
    /// [`DelegationError::TooManyPatterns`] mit `kind = "chain"` wenn
    /// `links.len() > MAX_CHAIN_DEPTH_HARD_CAP`.
    pub fn new(origin_guid: [u8; 16], links: Vec<DelegationLink>) -> DelegationResult<Self> {
        if links.len() > MAX_CHAIN_DEPTH_HARD_CAP {
            return Err(DelegationError::TooManyPatterns {
                kind: "chain",
                count: links.len(),
                max: MAX_CHAIN_DEPTH_HARD_CAP,
            });
        }
        Ok(Self { origin_guid, links })
    }

    /// Anzahl Links (Chain-Tiefe).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.links.len()
    }

    /// GUID des Edge-Peers (letzter Delegatee).
    #[must_use]
    pub fn edge_guid(&self) -> Option<[u8; 16]> {
        self.links.last().map(|l| l.delegatee_guid)
    }

    /// Wire-Encoding der Chain.
    ///
    /// Layout:
    /// ```text
    /// version       = 1 (u8)
    /// origin_guid   = 16 byte
    /// n_links       = u8 (max 255 — durch HARD_CAP=8 ohnehin <)
    /// [encoded link]*
    /// ```
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32 + 256 * self.links.len());
        buf.push(DELEGATION_VERSION);
        buf.extend_from_slice(&self.origin_guid);
        let n = u8::try_from(self.links.len()).unwrap_or(u8::MAX);
        buf.push(n);
        for link in &self.links {
            buf.extend_from_slice(&link.encode());
        }
        buf
    }

    /// Decode aus Wire-Bytes.
    ///
    /// # Errors
    /// [`DelegationError::Malformed`] bei Layout-Verletzung.
    pub fn decode(buf: &[u8]) -> DelegationResult<Self> {
        if buf.len() < 1 + 16 + 1 {
            return Err(DelegationError::Malformed(
                "chain header truncated".to_string(),
            ));
        }
        let version = buf[0];
        if version != DELEGATION_VERSION {
            return Err(DelegationError::UnsupportedVersion(version));
        }
        let mut origin_guid = [0u8; 16];
        origin_guid.copy_from_slice(&buf[1..17]);
        let n_links = buf[17] as usize;
        if n_links > MAX_CHAIN_DEPTH_HARD_CAP {
            return Err(DelegationError::TooManyPatterns {
                kind: "chain",
                count: n_links,
                max: MAX_CHAIN_DEPTH_HARD_CAP,
            });
        }
        let mut tail = &buf[18..];
        let mut links = Vec::with_capacity(n_links);
        for _ in 0..n_links {
            let (link, rest) = DelegationLink::decode(tail)?;
            links.push(link);
            tail = rest;
        }
        Ok(Self { origin_guid, links })
    }
}

// ---------- private signing helpers ----------

fn sign_ecdsa(
    alg: &'static signature::EcdsaSigningAlgorithm,
    pkcs8: &[u8],
    input: &[u8],
) -> DelegationResult<Vec<u8>> {
    let rng = SystemRandom::new();
    let key_pair = signature::EcdsaKeyPair::from_pkcs8(alg, pkcs8, &rng)
        .map_err(|e| DelegationError::SignFailed(alloc::format!("ecdsa key parse: {e}")))?;
    let sig = key_pair
        .sign(&rng, input)
        .map_err(|e| DelegationError::SignFailed(alloc::format!("ecdsa sign: {e}")))?;
    Ok(sig.as_ref().to_vec())
}

fn sign_rsa_pss(pkcs8: &[u8], input: &[u8]) -> DelegationResult<Vec<u8>> {
    let key_pair = signature::RsaKeyPair::from_pkcs8(pkcs8)
        .map_err(|e| DelegationError::SignFailed(alloc::format!("rsa key parse: {e}")))?;
    if key_pair.public().modulus_len() != 256 {
        return Err(DelegationError::SignFailed(alloc::format!(
            "rsa key is {} bits, expected 2048",
            key_pair.public().modulus_len() * 8
        )));
    }
    let mut sig = alloc::vec![0u8; key_pair.public().modulus_len()];
    let rng = SystemRandom::new();
    key_pair
        .sign(&signature::RSA_PSS_SHA256, &rng, input, &mut sig)
        .map_err(|e| DelegationError::SignFailed(alloc::format!("rsa sign: {e}")))?;
    Ok(sig)
}

fn sign_ed25519(pkcs8: &[u8], input: &[u8]) -> DelegationResult<Vec<u8>> {
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|e| DelegationError::SignFailed(alloc::format!("ed25519 key parse: {e}")))?;
    let sig = key_pair.sign(input);
    Ok(sig.as_ref().to_vec())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{
        ECDSA_P256_SHA256_FIXED_SIGNING, ECDSA_P384_SHA384_FIXED_SIGNING, EcdsaKeyPair,
        Ed25519KeyPair, KeyPair,
    };

    fn link_skeleton() -> DelegationLink {
        DelegationLink::new(
            [0xAA; 16],
            [0xBB; 16],
            alloc::vec!["sensor/*".to_string()],
            alloc::vec!["public".to_string()],
            1_700_000_000,
            1_800_000_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .expect("valid skeleton")
    }

    fn ecdsa_key(alg: &'static signature::EcdsaSigningAlgorithm) -> (Vec<u8>, Vec<u8>) {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(alg, &rng).expect("gen ecdsa");
        let pkcs8_vec = pkcs8.as_ref().to_vec();
        let key = EcdsaKeyPair::from_pkcs8(alg, &pkcs8_vec, &rng).expect("parse");
        let pub_key = key.public_key().as_ref().to_vec();
        (pkcs8_vec, pub_key)
    }

    fn ed25519_key() -> (Vec<u8>, Vec<u8>) {
        let rng = SystemRandom::new();
        let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng).expect("gen");
        let kp = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).expect("parse");
        (
            pkcs8_bytes.as_ref().to_vec(),
            kp.public_key().as_ref().to_vec(),
        )
    }

    #[test]
    fn signing_bytes_deterministic() {
        let l = link_skeleton();
        let a = l.signing_bytes();
        let b = l.signing_bytes();
        assert_eq!(a, b);
        // Magic + version sind die ersten 9 byte.
        assert_eq!(&a[..8], DELEGATION_MAGIC);
        assert_eq!(a[8], DELEGATION_VERSION);
    }

    #[test]
    fn ecdsa_p256_sign_verify_roundtrip() {
        let (pkcs8, pub_key) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let mut l = link_skeleton();
        l.algorithm = SignatureAlgorithm::EcdsaP256;
        l.sign(&pkcs8).expect("sign");
        l.verify(&pub_key).expect("verify");
        assert_eq!(l.signature.len(), 64);
    }

    #[test]
    fn ecdsa_p384_sign_verify_roundtrip() {
        let (pkcs8, pub_key) = ecdsa_key(&ECDSA_P384_SHA384_FIXED_SIGNING);
        let mut l = link_skeleton();
        l.algorithm = SignatureAlgorithm::EcdsaP384;
        l.sign(&pkcs8).expect("sign");
        l.verify(&pub_key).expect("verify");
        assert_eq!(l.signature.len(), 96);
    }

    #[test]
    fn ed25519_sign_verify_roundtrip() {
        let (pkcs8, pub_key) = ed25519_key();
        let mut l = link_skeleton();
        l.algorithm = SignatureAlgorithm::Ed25519;
        l.sign(&pkcs8).expect("sign");
        l.verify(&pub_key).expect("verify");
        assert_eq!(l.signature.len(), 64);
    }

    /// PKCS#8-DER eines mit `openssl genpkey -algorithm RSA
    /// -pkeyopt rsa_keygen_bits:2048` erzeugten Test-Schluessels (1192 byte,
    /// committet in `tests/fixtures/`).
    /// Nicht-sensitiv: Test-Vector, niemals fuer echte Signaturen.
    const TEST_RSA_2048_PKCS8: &[u8] = include_bytes!("../tests/fixtures/rsa_2048_test_pkcs8.der");

    /// RSA-PSS-2048-Sign-Test mit hardcoded PKCS#8-DER Test-Vector.
    /// Faengt die `modulus_len != 256` -> `==` Mutation in
    /// `sign_rsa_pss` (Zeile 637): mit `==` wuerde JEDER 2048-bit
    /// RSA-Key zur SignFailed fuehren.
    #[test]
    fn rsa_pss_2048_sign_succeeds_with_2048_bit_key() {
        let mut l = link_skeleton();
        l.algorithm = SignatureAlgorithm::RsaPss2048;
        l.sign(TEST_RSA_2048_PKCS8).expect("RSA-PSS-2048 sign");
        // 2048-bit Modulus = 256 byte Signatur (RFC 8017).
        assert_eq!(l.signature.len(), 256);
    }

    #[test]
    fn tampered_byte_breaks_verify() {
        let (pkcs8, pub_key) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let mut l = link_skeleton();
        l.sign(&pkcs8).expect("sign");
        // Veraendere ein Topic-Pattern — Sign-Input aendert sich, alte
        // Sig wird invalid.
        l.allowed_topic_patterns[0] = "/different/*".to_string();
        let err = l.verify(&pub_key).expect_err("must fail");
        assert!(matches!(err, DelegationError::VerifyFailed(_)));
    }

    #[test]
    fn wrong_pubkey_breaks_verify() {
        let (pkcs8_a, _pub_a) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let (_pkcs8_b, pub_b) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let mut l = link_skeleton();
        l.sign(&pkcs8_a).expect("sign");
        let err = l.verify(&pub_b).expect_err("must fail");
        assert!(matches!(err, DelegationError::VerifyFailed(_)));
    }

    #[test]
    fn empty_signature_rejects_verify() {
        let (_pkcs8, pub_key) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let l = link_skeleton(); // unsigned
        let err = l.verify(&pub_key).expect_err("must fail");
        assert!(matches!(err, DelegationError::SignFailed(_)));
    }

    #[test]
    fn link_encode_decode_roundtrip() {
        let (pkcs8, _) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let mut l = link_skeleton();
        l.sign(&pkcs8).expect("sign");
        let wire = l.encode();
        let (decoded, tail) = DelegationLink::decode(&wire).expect("decode");
        assert!(tail.is_empty());
        assert_eq!(decoded, l);
    }

    #[test]
    fn link_decode_bad_magic_rejects() {
        let mut bad = alloc::vec![0u8; 64];
        bad[..8].copy_from_slice(b"NOTMAGIC");
        let err = DelegationLink::decode(&bad).expect_err("must fail");
        assert!(matches!(err, DelegationError::BadMagic));
    }

    #[test]
    fn link_decode_bad_version_rejects() {
        let mut wire = link_skeleton().encode();
        wire[8] = 99; // Version-Byte
        let err = DelegationLink::decode(&wire).expect_err("must fail");
        assert!(matches!(err, DelegationError::UnsupportedVersion(99)));
    }

    #[test]
    fn link_decode_unknown_algorithm_rejects() {
        let mut l = link_skeleton();
        l.signature = alloc::vec![0u8; 64];
        let mut wire = l.encode();
        // Algorithm-Byte sitzt nach magic(8) + version(1) + delegator(16)
        // + delegatee(16) + not_before(8) + not_after(8) = offset 57.
        wire[57] = 99;
        let err = DelegationLink::decode(&wire).expect_err("must fail");
        assert!(matches!(err, DelegationError::UnknownAlgorithm(99)));
    }

    #[test]
    fn link_new_rejects_too_many_topics() {
        let topics = (0..MAX_TOPIC_PATTERNS + 1)
            .map(|i| alloc::format!("topic_{i}"))
            .collect();
        let err = DelegationLink::new(
            [0; 16],
            [0; 16],
            topics,
            alloc::vec![],
            0,
            1,
            SignatureAlgorithm::EcdsaP256,
        )
        .expect_err("must fail");
        assert!(matches!(
            err,
            DelegationError::TooManyPatterns { kind: "topic", .. }
        ));
    }

    #[test]
    fn link_new_rejects_inverted_window() {
        let err = DelegationLink::new(
            [0; 16],
            [0; 16],
            alloc::vec![],
            alloc::vec![],
            100,
            50,
            SignatureAlgorithm::EcdsaP256,
        )
        .expect_err("must fail");
        assert!(matches!(err, DelegationError::InvalidTimeWindow));
    }

    #[test]
    fn chain_encode_decode_roundtrip() {
        let (pkcs8, _) = ecdsa_key(&ECDSA_P256_SHA256_FIXED_SIGNING);
        let mut l1 = link_skeleton();
        l1.sign(&pkcs8).expect("sign1");
        let mut l2 = DelegationLink::new(
            [0xBB; 16],
            [0xCC; 16],
            alloc::vec!["sensor/lidar".to_string()],
            alloc::vec![],
            1_700_000_000,
            1_800_000_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .expect("l2 new");
        l2.sign(&pkcs8).expect("sign2");

        let chain =
            DelegationChain::new([0xAA; 16], alloc::vec![l1.clone(), l2.clone()]).expect("chain");
        assert_eq!(chain.depth(), 2);
        assert_eq!(chain.edge_guid(), Some([0xCC; 16]));
        let wire = chain.encode();
        let decoded = DelegationChain::decode(&wire).expect("decode");
        assert_eq!(decoded, chain);
    }

    #[test]
    fn chain_new_rejects_too_deep() {
        let dummy = link_skeleton();
        let too_deep = alloc::vec![dummy; MAX_CHAIN_DEPTH_HARD_CAP + 1];
        let err = DelegationChain::new([0; 16], too_deep).expect_err("must fail");
        assert!(matches!(
            err,
            DelegationError::TooManyPatterns { kind: "chain", .. }
        ));
    }

    #[test]
    fn algorithm_wire_id_roundtrip() {
        for a in [
            SignatureAlgorithm::EcdsaP256,
            SignatureAlgorithm::EcdsaP384,
            SignatureAlgorithm::RsaPss2048,
            SignatureAlgorithm::Ed25519,
        ] {
            let id = a.wire_id();
            assert_eq!(SignatureAlgorithm::from_wire_id(id), Some(a));
        }
        assert_eq!(SignatureAlgorithm::from_wire_id(0), None);
        assert_eq!(SignatureAlgorithm::from_wire_id(255), None);
    }

    // -------------------------------------------------------------
    // Mutation-Killer (2026-05-01)
    // -------------------------------------------------------------

    /// expected_signature_len pro Algorithmus liefert spezifischen Wert.
    /// Faengt `expected_signature_len -> None`-Mutation.
    #[test]
    fn expected_signature_len_per_algorithm() {
        assert_eq!(
            SignatureAlgorithm::EcdsaP256.expected_signature_len(),
            Some(64)
        );
        assert_eq!(
            SignatureAlgorithm::EcdsaP384.expected_signature_len(),
            Some(96)
        );
        assert_eq!(
            SignatureAlgorithm::Ed25519.expected_signature_len(),
            Some(64)
        );
        assert_eq!(
            SignatureAlgorithm::RsaPss2048.expected_signature_len(),
            Some(256)
        );
    }

    /// Display-Format aller DelegationError-Variants.
    /// Faengt `Display::fmt -> Ok(Default)` Mutation.
    #[test]
    fn delegation_error_display_messages_specific() {
        assert_eq!(
            alloc::format!(
                "{}",
                DelegationError::TooManyPatterns {
                    kind: "topic",
                    count: 100,
                    max: 64
                }
            ),
            "topic patterns: 100 > max 64"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::PatternTooLong { len: 500, max: 256 }),
            "pattern length 500 > max 256"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::SignFailed("bad".into())),
            "sign failed: bad"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::VerifyFailed("nope".into())),
            "verify failed: nope"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::Malformed("hdr".into())),
            "malformed delegation: hdr"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::UnknownAlgorithm(42)),
            "unknown algorithm id: 42"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::InvalidTimeWindow),
            "not_before > not_after"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::BadMagic),
            "bad magic bytes"
        );
        assert_eq!(
            alloc::format!("{}", DelegationError::UnsupportedVersion(2)),
            "unsupported version: 2"
        );
    }

    // ---- Cap-Boundary-Tests fuer DelegationLink::decode ----

    /// Baut einen minimal-validen Link-Wire mit n_topic Topic-Patterns
    /// und n_part Partition-Patterns; jedes Pattern leerer string.
    fn build_link_wire(
        n_topic: u32,
        n_part: u32,
        topic_lens: &[u32],
        part_lens: &[u32],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(DELEGATION_MAGIC);
        buf.push(DELEGATION_VERSION);
        buf.extend_from_slice(&[0u8; 16]); // delegator
        buf.extend_from_slice(&[0u8; 16]); // delegatee
        buf.extend_from_slice(&0i64.to_be_bytes()); // not_before
        buf.extend_from_slice(&0i64.to_be_bytes()); // not_after
        buf.push(1); // EcdsaP256
        // Topic
        buf.extend_from_slice(&n_topic.to_be_bytes());
        for &len in topic_lens {
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend(std::iter::repeat_n(b'a', len as usize));
        }
        // Partition
        buf.extend_from_slice(&n_part.to_be_bytes());
        for &len in part_lens {
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend(std::iter::repeat_n(b'b', len as usize));
        }
        // sig_len + sig
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf
    }

    /// n_topic == MAX_TOPIC_PATTERNS muss durchgehen.
    /// n_topic > MAX muss erroren.
    #[test]
    fn link_decode_n_topic_at_and_over_cap() {
        let topic_lens = vec![1u32; MAX_TOPIC_PATTERNS];
        let wire = build_link_wire(MAX_TOPIC_PATTERNS as u32, 0, &topic_lens, &[]);
        let res = DelegationLink::decode(&wire);
        assert!(res.is_ok(), "n_topic=MAX must succeed, got {res:?}");

        let topic_lens_over = vec![1u32; MAX_TOPIC_PATTERNS + 1];
        let wire_over = build_link_wire((MAX_TOPIC_PATTERNS + 1) as u32, 0, &topic_lens_over, &[]);
        let err = DelegationLink::decode(&wire_over).unwrap_err();
        assert!(matches!(err, DelegationError::TooManyPatterns { .. }));
    }

    /// Topic-pattern-len == MAX_PATTERN_LEN durchgehen, > MAX erroren.
    #[test]
    fn link_decode_topic_pattern_len_at_and_over_cap() {
        let wire = build_link_wire(1, 0, &[MAX_PATTERN_LEN as u32], &[]);
        assert!(DelegationLink::decode(&wire).is_ok());

        let wire_over = build_link_wire(1, 0, &[(MAX_PATTERN_LEN + 1) as u32], &[]);
        let err = DelegationLink::decode(&wire_over).unwrap_err();
        assert!(matches!(err, DelegationError::PatternTooLong { .. }));
    }

    /// n_part == MAX, > MAX.
    #[test]
    fn link_decode_n_part_at_and_over_cap() {
        let part_lens = vec![1u32; MAX_PARTITION_PATTERNS];
        let wire = build_link_wire(0, MAX_PARTITION_PATTERNS as u32, &[], &part_lens);
        assert!(DelegationLink::decode(&wire).is_ok());

        let part_lens_over = vec![1u32; MAX_PARTITION_PATTERNS + 1];
        let wire_over =
            build_link_wire(0, (MAX_PARTITION_PATTERNS + 1) as u32, &[], &part_lens_over);
        let err = DelegationLink::decode(&wire_over).unwrap_err();
        assert!(matches!(err, DelegationError::TooManyPatterns { .. }));
    }

    /// Part-pattern-len cap.
    #[test]
    fn link_decode_part_pattern_len_at_and_over_cap() {
        let wire = build_link_wire(0, 1, &[], &[MAX_PATTERN_LEN as u32]);
        assert!(DelegationLink::decode(&wire).is_ok());

        let wire_over = build_link_wire(0, 1, &[], &[(MAX_PATTERN_LEN + 1) as u32]);
        let err = DelegationLink::decode(&wire_over).unwrap_err();
        assert!(matches!(err, DelegationError::PatternTooLong { .. }));
    }

    // ---- DelegationChain::decode header-len boundary ----

    /// Faengt Mutationen `<` -> `==`/`<=` und `+` -> `-`/`*` auf
    /// `if buf.len() < 1 + 16 + 1` Header-Check.
    /// Layout: 1 byte version + 16 byte origin_guid + 1 byte n_links = 18.
    /// Buffer EXAKT 18 byte (mit n_links=0): muss durchgehen.
    /// Buffer 17 byte: muss erroren (Truncated).
    #[test]
    fn chain_decode_header_at_minimum_size() {
        let mut buf = Vec::new();
        buf.push(DELEGATION_VERSION); // 1
        buf.extend_from_slice(&[0u8; 16]); // 16 — origin_guid
        buf.push(0); // 1 — n_links=0
        assert_eq!(buf.len(), 18);
        let chain = DelegationChain::decode(&buf).expect("18-byte header must decode");
        assert_eq!(chain.links.len(), 0);
    }

    #[test]
    fn chain_decode_header_one_byte_too_short() {
        let mut buf = Vec::new();
        buf.push(DELEGATION_VERSION);
        buf.extend_from_slice(&[0u8; 16]);
        // Missing n_links byte: total 17.
        assert_eq!(buf.len(), 17);
        let err = DelegationChain::decode(&buf).unwrap_err();
        assert!(matches!(err, DelegationError::Malformed(_)));
    }

    /// n_links == MAX_CHAIN_DEPTH_HARD_CAP muss durchgehen (mit korrekt
    /// vielen Links danach), n_links > MAX muss erroren.
    /// Kein Versuch alle Links zu encoden — nur header check.
    #[test]
    fn chain_decode_n_links_over_hard_cap_rejected() {
        let mut buf = Vec::new();
        buf.push(DELEGATION_VERSION);
        buf.extend_from_slice(&[0u8; 16]);
        buf.push((MAX_CHAIN_DEPTH_HARD_CAP + 1) as u8);
        let err = DelegationChain::decode(&buf).unwrap_err();
        assert!(matches!(
            err,
            DelegationError::TooManyPatterns { kind: "chain", .. }
        ));
    }

    /// n_links == MAX_CHAIN_DEPTH_HARD_CAP MUSS den Cap-Check passieren
    /// (Original `>` ist false). Faengt `>` -> `>=` (an MAX wuerde
    /// Mutation TooManyPatterns liefern statt Malformed-aus-Loop).
    /// Wir liefern bewusst KEINE Link-Bodies — Original geht in Loop und
    /// liefert dort einen anderen Error (Malformed); Mutation feuert die
    /// Cap-Branch.
    #[test]
    fn chain_decode_n_links_at_hard_cap_passes_cap_check() {
        let mut buf = Vec::new();
        buf.push(DELEGATION_VERSION);
        buf.extend_from_slice(&[0u8; 16]);
        buf.push(MAX_CHAIN_DEPTH_HARD_CAP as u8);
        let err = DelegationChain::decode(&buf).unwrap_err();
        // Original `>`: cap check geht durch, dann Loop scheitert auf
        // fehlenden Link-Bytes -> Malformed.
        // Mutation `>=`: cap check wuerde TooManyPatterns liefern.
        assert!(
            matches!(err, DelegationError::Malformed(_)),
            "expected Malformed (loop failure), got {err:?}"
        );
    }
}
