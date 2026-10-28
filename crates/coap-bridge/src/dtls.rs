// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP DTLS mode per RFC 7252 §9.
//!
//! The spec defines four security modes:
//! - **NoSec** (§9.1): no encryption; default port 5683.
//! - **PreSharedKey** (§9.1.1): symmetric PSK path.
//! - **RawPublicKey** (§9.1.2): asymmetric curve keys without a cert chain.
//! - **Certificate** (§9.1.3): X.509 chain validation.
//!
//! Here we provide the configuration layer + profile selection. The
//! actual DTLS record-layer implementation is on the caller side
//! (e.g. via `tokio-rustls` or another DTLS crate); ZeroDDS
//! provides the identity and key data structures
//! (reusing the `security-pki` + `security-crypto` building blocks).

use alloc::string::String;
use alloc::vec::Vec;

/// Spec §9.1 — DTLS mode per connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DtlsMode {
    /// `NoSec` — plaintext CoAP on port 5683.
    NoSec,
    /// `PreSharedKey` — PSK identity + key.
    PreSharedKey {
        /// Identity the client sends (`psk_identity_hint`).
        identity: String,
        /// Pre-shared key (typically 16-32 bytes).
        key: Vec<u8>,
    },
    /// `RawPublicKey` — curve point without a cert.
    RawPublicKey {
        /// Public-key bytes (e.g. ECDSA-P256, 64 bytes uncompressed).
        public_key: Vec<u8>,
        /// Spec §9.1.2: subject-public-key-info ASN.1-DER.
        spki_der: Vec<u8>,
    },
    /// `Certificate` — X.509 chain.
    Certificate {
        /// Own cert chain (DER-encoded, leaf first).
        cert_chain: Vec<Vec<u8>>,
        /// Trust anchors for peer validation (DER-encoded).
        trust_anchors: Vec<Vec<u8>>,
    },
}

impl DtlsMode {
    /// Spec §9.1 — `true` if the mode enables DTLS (everything except
    /// `NoSec`).
    #[must_use]
    pub fn is_secure(&self) -> bool {
        !matches!(self, Self::NoSec)
    }

    /// Spec §6.2 / §9 — default port by mode: 5683 (NoSec) or
    /// 5684 (DTLS).
    #[must_use]
    pub fn default_port(&self) -> u16 {
        if self.is_secure() { 5684 } else { 5683 }
    }

    /// Mode name for logging/debug.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoSec => "NoSec",
            Self::PreSharedKey { .. } => "PreSharedKey",
            Self::RawPublicKey { .. } => "RawPublicKey",
            Self::Certificate { .. } => "Certificate",
        }
    }
}

/// DTLS configuration errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DtlsConfigError {
    /// PSK key is empty or too short (spec recommends ≥ 16 bytes).
    PskTooShort,
    /// Identity is empty.
    EmptyIdentity,
    /// Cert chain is empty.
    EmptyCertChain,
    /// Trust-anchor list is empty (no validation possible).
    NoTrustAnchors,
    /// Invalid public-key format.
    InvalidPublicKey,
}

impl core::fmt::Display for DtlsConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PskTooShort => write!(f, "PskTooShort"),
            Self::EmptyIdentity => write!(f, "EmptyIdentity"),
            Self::EmptyCertChain => write!(f, "EmptyCertChain"),
            Self::NoTrustAnchors => write!(f, "NoTrustAnchors"),
            Self::InvalidPublicKey => write!(f, "InvalidPublicKey"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DtlsConfigError {}

/// Spec §9.1 — validates a DtlsMode configuration.
///
/// # Errors
/// See [`DtlsConfigError`].
pub fn validate_dtls_mode(mode: &DtlsMode) -> Result<(), DtlsConfigError> {
    match mode {
        DtlsMode::NoSec => Ok(()),
        DtlsMode::PreSharedKey { identity, key } => {
            if identity.is_empty() {
                return Err(DtlsConfigError::EmptyIdentity);
            }
            if key.len() < 16 {
                return Err(DtlsConfigError::PskTooShort);
            }
            Ok(())
        }
        DtlsMode::RawPublicKey {
            public_key,
            spki_der,
        } => {
            if public_key.is_empty() || spki_der.is_empty() {
                return Err(DtlsConfigError::InvalidPublicKey);
            }
            Ok(())
        }
        DtlsMode::Certificate {
            cert_chain,
            trust_anchors,
        } => {
            if cert_chain.is_empty() {
                return Err(DtlsConfigError::EmptyCertChain);
            }
            if trust_anchors.is_empty() {
                return Err(DtlsConfigError::NoTrustAnchors);
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Bridge-Spec §7.1 — cipher suites, PSK store and the credential set the
// `coaps://` listener is configured with.
// ---------------------------------------------------------------------------

/// The cipher suites Bridge-Spec §7.1 names for `coaps://`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtlsCipherSuite {
    /// `TLS_PSK_WITH_AES_128_CCM_8` — PSK mode (RFC 7252 §9.1.3 / RFC 6655).
    PskWithAes128Ccm8,
    /// `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` — certificate mode.
    EcdheEcdsaWithAes128Ccm8,
    /// `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` — hybrid mode.
    EcdhePskWithAes128CbcSha256,
}

impl DtlsCipherSuite {
    /// The IANA TLS cipher-suite code point.
    #[must_use]
    pub const fn code_point(self) -> u16 {
        match self {
            Self::PskWithAes128Ccm8 => 0xc0a8,
            Self::EcdheEcdsaWithAes128Ccm8 => 0xc0ae,
            Self::EcdhePskWithAes128CbcSha256 => 0xc037,
        }
    }

    /// The IANA name, as Bridge-Spec §7.1 spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PskWithAes128Ccm8 => "TLS_PSK_WITH_AES_128_CCM_8",
            Self::EcdheEcdsaWithAes128Ccm8 => "TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8",
            Self::EcdhePskWithAes128CbcSha256 => "TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256",
        }
    }

    /// Parses an IANA cipher-suite name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::PskWithAes128Ccm8,
            Self::EcdheEcdsaWithAes128Ccm8,
            Self::EcdhePskWithAes128CbcSha256,
        ]
        .into_iter()
        .find(|s| s.name() == name)
    }

    /// `true` if the suite authenticates with a pre-shared key.
    #[must_use]
    pub const fn is_psk(self) -> bool {
        matches!(
            self,
            Self::PskWithAes128Ccm8 | Self::EcdhePskWithAes128CbcSha256
        )
    }
}

/// One `identity` → `secret` entry of the `coap.dtls.psk` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PskEntry {
    /// PSK identity the client sends.
    pub identity: String,
    /// The pre-shared secret.
    pub secret: Vec<u8>,
}

/// The `coap.dtls.psk` list: the identities the bridge accepts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PskStore {
    entries: Vec<PskEntry>,
}

impl PskStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an identity/secret pair.
    ///
    /// # Errors
    /// [`DtlsConfigError`] for an empty identity or a secret shorter than the
    /// 16 bytes [`validate_dtls_mode`] requires.
    pub fn add(&mut self, identity: &str, secret: Vec<u8>) -> Result<&mut Self, DtlsConfigError> {
        validate_dtls_mode(&DtlsMode::PreSharedKey {
            identity: String::from(identity),
            key: secret.clone(),
        })?;
        self.entries.push(PskEntry {
            identity: String::from(identity),
            secret,
        });
        Ok(self)
    }

    /// The secret for `identity`, or `None` if the bridge does not know it.
    #[must_use]
    pub fn secret_for(&self, identity: &[u8]) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|e| e.identity.as_bytes() == identity)
            .map(|e| e.secret.as_slice())
    }

    /// The identity hint the server sends — the first configured identity
    /// (RFC 4279 §5.2: the hint is advisory).
    #[must_use]
    pub fn identity_hint(&self) -> Option<&str> {
        self.entries.first().map(|e| e.identity.as_str())
    }

    /// The configured entries.
    #[must_use]
    pub fn entries(&self) -> &[PskEntry] {
        &self.entries
    }

    /// `true` if no identity is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Decodes a `secret_hex` value from the `coap.dtls.psk` list.
///
/// # Errors
/// [`DtlsConfigError::PskTooShort`] for an odd length or a non-hex digit — a
/// secret that cannot be decoded is refused rather than silently truncated.
pub fn parse_secret_hex(text: &str) -> Result<Vec<u8>, DtlsConfigError> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() || clean.len() % 2 != 0 {
        return Err(DtlsConfigError::PskTooShort);
    }
    let bytes = clean.as_bytes();
    let mut out = Vec::with_capacity(clean.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_digit(pair[0]).ok_or(DtlsConfigError::PskTooShort)?;
        let lo = hex_digit(pair[1]).ok_or(DtlsConfigError::PskTooShort)?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

const fn hex_digit(b: u8) -> Option<u8> {
    Some(match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => return None,
    })
}

/// The `coap.dtls` credential set (Bridge-Spec §7.1): the server certificate
/// and key, the CA that validates client certificates, and the PSK identities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DtlsCredentials {
    /// `cert_file` — the server certificate chain, PEM.
    pub cert_file: Option<String>,
    /// `key_file` — the server private key, PEM (PKCS#8).
    pub key_file: Option<String>,
    /// `ca_file` — the trust anchor for client certificates, PEM.
    pub ca_file: Option<String>,
    /// `psk` — the accepted identities.
    pub psk: PskStore,
    /// The cipher suites to offer, in preference order. Empty selects the
    /// §7.1 default for the configured credentials.
    pub cipher_suites: Vec<DtlsCipherSuite>,
}

impl DtlsCredentials {
    /// The cipher suites to offer: the configured list, or the §7.1 default
    /// for what is configured — the PSK suite when PSK identities exist, the
    /// certificate suite when a certificate does.
    #[must_use]
    pub fn effective_cipher_suites(&self) -> Vec<DtlsCipherSuite> {
        if !self.cipher_suites.is_empty() {
            return self.cipher_suites.clone();
        }
        if !self.psk.is_empty() {
            return alloc::vec![DtlsCipherSuite::PskWithAes128Ccm8];
        }
        alloc::vec![DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8]
    }

    /// `true` if the bridge should run the PSK handshake rather than the
    /// certificate one. Bridge-Spec §7.1 keeps the two modes apart; PSK wins
    /// when identities are configured, because a PSK client cannot fall back
    /// to a certificate it has no trust anchor for.
    #[must_use]
    pub fn uses_psk(&self) -> bool {
        !self.psk.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn cipher_suite_names_and_code_points_match_iana() {
        // Bridge-Spec §7.1 names all three; the code points are the IANA
        // TLS Cipher Suite registry values.
        assert_eq!(DtlsCipherSuite::PskWithAes128Ccm8.code_point(), 0xc0a8);
        assert_eq!(
            DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8.code_point(),
            0xc0ae
        );
        assert_eq!(
            DtlsCipherSuite::EcdhePskWithAes128CbcSha256.code_point(),
            0xc037
        );
        for s in [
            DtlsCipherSuite::PskWithAes128Ccm8,
            DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8,
            DtlsCipherSuite::EcdhePskWithAes128CbcSha256,
        ] {
            assert_eq!(DtlsCipherSuite::from_name(s.name()), Some(s));
        }
        assert_eq!(DtlsCipherSuite::from_name("TLS_NULL_WITH_NULL_NULL"), None);
        assert!(DtlsCipherSuite::PskWithAes128Ccm8.is_psk());
        assert!(!DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8.is_psk());
    }

    #[test]
    fn psk_store_looks_up_by_identity_and_refuses_short_secrets() {
        let mut store = PskStore::new();
        store
            .add("device-001", alloc::vec![7u8; 16])
            .expect("add")
            .add("device-002", alloc::vec![9u8; 32])
            .expect("add");
        assert_eq!(store.secret_for(b"device-001"), Some(&[7u8; 16][..]));
        assert_eq!(store.secret_for(b"device-002"), Some(&[9u8; 32][..]));
        assert_eq!(store.secret_for(b"unknown"), None);
        assert_eq!(store.identity_hint(), Some("device-001"));
        assert_eq!(store.entries().len(), 2);
        assert_eq!(
            PskStore::new().add("short", alloc::vec![1u8; 8]),
            Err(DtlsConfigError::PskTooShort)
        );
    }

    #[test]
    fn secret_hex_round_trips_and_rejects_garbage() {
        assert_eq!(
            parse_secret_hex("deadbeefcafebabe").expect("hex"),
            alloc::vec![0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe]
        );
        assert_eq!(
            parse_secret_hex("DEADBEEF").expect("hex"),
            alloc::vec![0xde, 0xad, 0xbe, 0xef]
        );
        for bad in ["", "abc", "zz", "de ad be ef q"] {
            assert_eq!(parse_secret_hex(bad), Err(DtlsConfigError::PskTooShort));
        }
    }

    #[test]
    fn credentials_pick_the_mode_they_were_configured_for() {
        let mut certs = DtlsCredentials {
            cert_file: Some(String::from("/etc/zerodds/coap-cert.pem")),
            key_file: Some(String::from("/etc/zerodds/coap-key.pem")),
            ..DtlsCredentials::default()
        };
        assert!(!certs.uses_psk());
        assert_eq!(
            certs.effective_cipher_suites(),
            alloc::vec![DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8]
        );
        certs
            .psk
            .add("device-001", alloc::vec![1u8; 16])
            .expect("add");
        assert!(certs.uses_psk());
        assert_eq!(
            certs.effective_cipher_suites(),
            alloc::vec![DtlsCipherSuite::PskWithAes128Ccm8]
        );
        certs.cipher_suites = alloc::vec![DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8];
        assert_eq!(
            certs.effective_cipher_suites(),
            alloc::vec![DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8]
        );
    }

    #[test]
    fn nosec_is_not_secure() {
        assert!(!DtlsMode::NoSec.is_secure());
        assert_eq!(DtlsMode::NoSec.default_port(), 5683);
        assert_eq!(DtlsMode::NoSec.name(), "NoSec");
    }

    #[test]
    fn psk_is_secure() {
        let m = DtlsMode::PreSharedKey {
            identity: "client-1".into(),
            key: alloc::vec![0; 32],
        };
        assert!(m.is_secure());
        assert_eq!(m.default_port(), 5684);
        assert_eq!(m.name(), "PreSharedKey");
    }

    #[test]
    fn raw_public_key_is_secure() {
        let m = DtlsMode::RawPublicKey {
            public_key: alloc::vec![0; 64],
            spki_der: alloc::vec![0; 91],
        };
        assert!(m.is_secure());
        assert_eq!(m.name(), "RawPublicKey");
    }

    #[test]
    fn certificate_is_secure() {
        let m = DtlsMode::Certificate {
            cert_chain: alloc::vec![alloc::vec![0; 100]],
            trust_anchors: alloc::vec![alloc::vec![0; 100]],
        };
        assert!(m.is_secure());
        assert_eq!(m.default_port(), 5684);
        assert_eq!(m.name(), "Certificate");
    }

    #[test]
    fn validate_psk_short_key_rejected() {
        let m = DtlsMode::PreSharedKey {
            identity: "a".into(),
            key: alloc::vec![0; 8],
        };
        assert_eq!(validate_dtls_mode(&m), Err(DtlsConfigError::PskTooShort));
    }

    #[test]
    fn validate_psk_empty_identity_rejected() {
        let m = DtlsMode::PreSharedKey {
            identity: "".into(),
            key: alloc::vec![0; 16],
        };
        assert_eq!(validate_dtls_mode(&m), Err(DtlsConfigError::EmptyIdentity));
    }

    #[test]
    fn validate_psk_valid() {
        let m = DtlsMode::PreSharedKey {
            identity: "client-1".into(),
            key: alloc::vec![0; 16],
        };
        assert!(validate_dtls_mode(&m).is_ok());
    }

    #[test]
    fn validate_certificate_empty_chain_rejected() {
        let m = DtlsMode::Certificate {
            cert_chain: alloc::vec![],
            trust_anchors: alloc::vec![alloc::vec![0; 100]],
        };
        assert_eq!(validate_dtls_mode(&m), Err(DtlsConfigError::EmptyCertChain));
    }

    #[test]
    fn validate_certificate_no_trust_anchors_rejected() {
        let m = DtlsMode::Certificate {
            cert_chain: alloc::vec![alloc::vec![0; 100]],
            trust_anchors: alloc::vec![],
        };
        assert_eq!(validate_dtls_mode(&m), Err(DtlsConfigError::NoTrustAnchors));
    }

    #[test]
    fn validate_raw_public_key_empty_rejected() {
        let m = DtlsMode::RawPublicKey {
            public_key: alloc::vec![],
            spki_der: alloc::vec![0; 91],
        };
        assert_eq!(
            validate_dtls_mode(&m),
            Err(DtlsConfigError::InvalidPublicKey)
        );
    }

    #[test]
    fn validate_nosec_passes() {
        assert!(validate_dtls_mode(&DtlsMode::NoSec).is_ok());
    }
}
