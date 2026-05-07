// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-keyexchange`. Safety classification: **SAFE** (duenner Wrapper um `ring::agreement` + `ring::hkdf`).
//!
//! X25519 / P-256 Ephemeral-Diffie-Hellman fuer den
//! DDS-Security 1.1 Authentication-Handshake (Spec §8.3.2).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumiert von `zerodds-security-pki`.
//!
//! ## Zweck
//!
//! Der `Authentication`-Handshake (Spec §8.3.2) braucht am Ende einen
//! `SharedSecret`. Der uebliche Weg ist ephemeral-DH: jede Seite
//! erzeugt ein temporaeres Schluesselpaar, tauscht die Public-Keys
//! aus, und leitet das Shared-Secret aus `x25519(priv, remote_pub)`
//! ab. HKDF-SHA256 zieht daraus einen 32-byte Key.
//!
//! # API
//!
//! ```
//! use zerodds_security_keyexchange::KeyExchange;
//!
//! // Beide Seiten erzeugen ephemerals.
//! let alice = KeyExchange::new().expect("alice");
//! let bob = KeyExchange::new().expect("bob");
//!
//! // Public-Keys tauschen (ueber den SPDP-Handshake-Token).
//! let a_pub = alice.public_key().to_vec();
//! let b_pub = bob.public_key().to_vec();
//!
//! // Jede Seite leitet den gleichen 32-byte SharedSecret ab.
//! let s1 = alice.derive_shared_secret(&b_pub).expect("alice derive");
//! let s2 = bob.derive_shared_secret(&a_pub).expect("bob derive");
//! assert_eq!(s1, s2);
//! ```
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`KeyExchange`] + [`KxSuite::X25519`] / [`KxSuite::EcdhP256`] —
//!   Ephemeral-DH-Roundtrip mit deterministischer SharedSecret-Ableitung.
//!
//! ## Nicht-Ziele
//!
//! RSA-OAEP-Key-Transport (Spec §8.3.2.11 als optionale Alternative fuer
//! Legacy-Vendors ohne ECDH/X25519) ist explizit nicht in RC1: alle
//! relevanten Vendoren (Cyclone DDS, FastDDS, RTI Connext) sprechen
//! ECDH oder X25519, und `ring 0.17` exponiert keine RSA-Encrypt-API.
//! Falls ein konkreter Legacy-Use-Case auftaucht, wird der Pfad ueber
//! die `rsa`-Crate als Major-2.0-additive-Erweiterung wieder eingefuehrt.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;

use ring::agreement::{self, ECDH_P256, EphemeralPrivateKey, PublicKey, X25519};
use ring::hkdf;
use ring::rand::SystemRandom;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

/// DH-Suite-Auswahl.
///
/// Default ist `X25519` — moderner Standard, 32-byte Public-Key.
/// `EcdhP256` fuer Interop mit Vendors die **kein** X25519 kennen
/// (RTI-Connext-Legacy, Fast-DDS < 2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KxSuite {
    /// X25519 (Curve25519). 32-byte Public-Key.
    X25519,
    /// NIST P-256 ECDH. 65-byte unkomprimierter Public-Key (0x04 || X || Y).
    EcdhP256,
}

impl Default for KxSuite {
    fn default() -> Self {
        Self::X25519
    }
}

impl KxSuite {
    /// Erwartete Laenge des Public-Key-Bytes.
    #[must_use]
    pub const fn public_key_len(self) -> usize {
        match self {
            Self::X25519 => 32,
            Self::EcdhP256 => 65,
        }
    }

    fn algorithm(self) -> &'static agreement::Algorithm {
        match self {
            Self::X25519 => &X25519,
            Self::EcdhP256 => &ECDH_P256,
        }
    }
}

/// Ephemerales DH-Schluesselpaar fuer einen einzelnen Handshake.
///
/// Nach `derive_shared_secret` ist die Instance verbraucht — das
/// `EphemeralPrivateKey` erlaubt nach API-Design von `ring` nur einen
/// `agree_ephemeral`-Call, was PFS (Perfect Forward Secrecy) erzwingt.
pub struct KeyExchange {
    suite: KxSuite,
    private: EphemeralPrivateKey,
    public: PublicKey,
}

impl KeyExchange {
    /// Erzeugt ein frisches X25519-Schluesselpaar (Default).
    ///
    /// # Errors
    /// `CryptoFailed` wenn die System-RNG nicht verfuegbar ist (z.B.
    /// kein `/dev/urandom` in einem broken-Sandbox-Szenario).
    pub fn new() -> SecurityResult<Self> {
        Self::with_suite(KxSuite::X25519)
    }

    /// Erzeugt ein Schluesselpaar fuer die gewaehlte DH-Suite.
    ///
    /// # Errors
    /// siehe [`Self::new`].
    pub fn with_suite(suite: KxSuite) -> SecurityResult<Self> {
        let rng = SystemRandom::new();
        let private = EphemeralPrivateKey::generate(suite.algorithm(), &rng).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: ephemeral-key generation failed",
            )
        })?;
        let public = private.compute_public_key().map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: public-key derivation failed",
            )
        })?;
        Ok(Self {
            suite,
            private,
            public,
        })
    }

    /// Liefert die aktive DH-Suite.
    #[must_use]
    pub fn suite(&self) -> KxSuite {
        self.suite
    }

    /// Liefert den lokalen Public-Key als Byte-Slice. Laenge ist
    /// suite-abhaengig (siehe [`KxSuite::public_key_len`]).
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        self.public.as_ref()
    }

    /// Leitet das SharedSecret aus dem Remote-Public-Key ab. Verbraucht
    /// das lokale ephemerale Key.
    ///
    /// # Errors
    /// * `BadArgument` wenn `remote_public_key.len() != suite.public_key_len()`.
    /// * `CryptoFailed` wenn `ring` das Agreement ablehnt (z.B.
    ///   kleiner-Untergruppen-Angriff, Identity-Punkt, Off-Curve-Point).
    pub fn derive_shared_secret(self, remote_public_key: &[u8]) -> SecurityResult<Vec<u8>> {
        if remote_public_key.len() != self.suite.public_key_len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!(
                    "keyexchange: {:?} public-key muss {} byte sein",
                    self.suite,
                    self.suite.public_key_len()
                ),
            ));
        }
        let peer = agreement::UnparsedPublicKey::new(self.suite.algorithm(), remote_public_key);
        agreement::agree_ephemeral(self.private, &peer, |raw_dh| {
            // HKDF-SHA256 über den rohen DH-Output erzwingt eine
            // uniforme Verteilung + erlaubt domain-separation.
            let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"zerodds-security-v1/shared-secret");
            let prk = salt.extract(raw_dh);
            let info_parts = [b"DDS:Auth:PKI-DH:secret".as_slice()];
            let okm = prk.expand(&info_parts, hkdf::HKDF_SHA256).map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "keyexchange: HKDF expand failed",
                )
            })?;
            let mut out = [0u8; 32];
            okm.fill(&mut out).map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "keyexchange: HKDF fill failed",
                )
            })?;
            Ok(out.to_vec())
        })
        .map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: DH agreement rejected (invalid peer key?)",
            )
        })
        .and_then(|r| r)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn public_key_is_32_bytes() {
        let kx = KeyExchange::new().unwrap();
        assert_eq!(kx.public_key().len(), 32);
    }

    #[test]
    fn two_parties_derive_identical_secret() {
        let alice = KeyExchange::new().unwrap();
        let bob = KeyExchange::new().unwrap();

        let a_pub = alice.public_key().to_vec();
        let b_pub = bob.public_key().to_vec();

        let s1 = alice.derive_shared_secret(&b_pub).unwrap();
        let s2 = bob.derive_shared_secret(&a_pub).unwrap();

        assert_eq!(s1.len(), 32);
        assert_eq!(s1, s2, "alice + bob muessen identisches secret ableiten");
    }

    #[test]
    fn different_pairs_produce_different_secrets() {
        let alice = KeyExchange::new().unwrap();
        let bob = KeyExchange::new().unwrap();
        let b_pub = bob.public_key().to_vec();
        let s1 = alice.derive_shared_secret(&b_pub).unwrap();

        let alice2 = KeyExchange::new().unwrap();
        let bob2 = KeyExchange::new().unwrap();
        let b2_pub = bob2.public_key().to_vec();
        let s2 = alice2.derive_shared_secret(&b2_pub).unwrap();

        assert_ne!(s1, s2, "andere ephemerals → anderes secret (PFS)");
    }

    #[test]
    fn wrong_length_public_key_rejected() {
        let alice = KeyExchange::new().unwrap();
        let err = alice.derive_shared_secret(&[0u8; 16]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn zero_public_key_rejected_by_ring() {
        // All-zero X25519 public key → agreement muss failen
        // (Identity-Punkt / kleines-Untergruppen-Angriff).
        let alice = KeyExchange::new().unwrap();
        let err = alice.derive_shared_secret(&[0u8; 32]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    // -------------------------------------------------------------
    // P-256 ECDH
    // -------------------------------------------------------------

    #[test]
    fn default_suite_is_x25519() {
        let kx = KeyExchange::new().unwrap();
        assert_eq!(kx.suite(), KxSuite::X25519);
        assert_eq!(kx.public_key().len(), 32);
    }

    #[test]
    fn p256_public_key_is_65_bytes() {
        let kx = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        assert_eq!(kx.suite(), KxSuite::EcdhP256);
        assert_eq!(kx.public_key().len(), 65);
        // Unkomprimiertes Format startet mit 0x04.
        assert_eq!(kx.public_key()[0], 0x04);
    }

    #[test]
    fn p256_two_parties_derive_identical_secret() {
        let alice = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        let bob = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        let a_pub = alice.public_key().to_vec();
        let b_pub = bob.public_key().to_vec();
        let s1 = alice.derive_shared_secret(&b_pub).unwrap();
        let s2 = bob.derive_shared_secret(&a_pub).unwrap();
        assert_eq!(s1.len(), 32);
        assert_eq!(s1, s2);
    }

    #[test]
    fn p256_rejects_wrong_length_public_key() {
        let alice = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        let err = alice.derive_shared_secret(&[0u8; 32]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn p256_rejects_off_curve_point() {
        // 65-byte, beginnt mit 0x04, aber der Rest ist Nullbytes →
        // keine gueltige P-256-Punkt.
        let alice = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        let mut bogus = [0u8; 65];
        bogus[0] = 0x04;
        let err = alice.derive_shared_secret(&bogus).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn x25519_and_p256_produce_different_public_key_lengths() {
        let a = KeyExchange::new().unwrap();
        let b = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        assert_ne!(a.public_key().len(), b.public_key().len());
    }
}
