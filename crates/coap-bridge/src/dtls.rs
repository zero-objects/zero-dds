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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

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
