// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-keyexchange`. Safety classification: **SAFE** (a thin wrapper around `ring::agreement` + `ring::hkdf`).
//!
//! X25519 / P-256 ephemeral Diffie-Hellman for the
//! DDS-Security 1.1 authentication handshake (spec §8.3.2).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumed by `zerodds-security-pki`.
//!
//! ## Purpose
//!
//! The `Authentication` handshake (spec §8.3.2) needs a
//! `SharedSecret` at the end. The usual way is ephemeral DH: each side
//! generates a temporary key pair, exchanges the public keys,
//! and derives the shared secret from `x25519(priv, remote_pub)`.
//! HKDF-SHA256 pulls a 32-byte key from it.
//!
//! # API
//!
//! ```
//! use zerodds_security_keyexchange::KeyExchange;
//!
//! // Both sides generate ephemerals.
//! let alice = KeyExchange::new().expect("alice");
//! let bob = KeyExchange::new().expect("bob");
//!
//! // Exchange public keys (via the SPDP handshake token).
//! let a_pub = alice.public_key().to_vec();
//! let b_pub = bob.public_key().to_vec();
//!
//! // Each side derives the same 32-byte SharedSecret.
//! let s1 = alice.derive_shared_secret(&b_pub).expect("alice derive");
//! let s2 = bob.derive_shared_secret(&a_pub).expect("bob derive");
//! assert_eq!(s1, s2);
//! ```
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`KeyExchange`] + [`KxSuite::X25519`] / [`KxSuite::EcdhP256`] —
//!   ephemeral-DH roundtrip with deterministic SharedSecret derivation.
//!
//! ## Non-goals
//!
//! RSA-OAEP key transport (spec §8.3.2.11 as an optional alternative for
//! legacy vendors without ECDH/X25519) is explicitly not in RC1: all
//! relevant vendors (Cyclone DDS, FastDDS, RTI Connext) speak
//! ECDH or X25519, and `ring 0.17` exposes no RSA encrypt API.
//! If a concrete legacy use case appears, the path via
//! the `rsa` crate is reintroduced as a major-2.0 additive extension.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;

use ring::agreement::{self, ECDH_P256, EphemeralPrivateKey, PublicKey, X25519};
use ring::rand::SystemRandom;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

/// DH suite selection.
///
/// The default is `EcdhP256` (NIST P-256, `ECDH+prime256v1-CEUM`) — the
/// cross-vendor-interoperable key agreement mandated by the
/// DDS-Security-1.2 spec (§9.3.2.3.1) (cyclone/FastDDS/OpenDDS). `X25519` is **not** a spec algorithm,
/// but a ZeroDDS vendor extension — only opt-in between pure ZeroDDS peers
/// (via `with_suite`), NEVER as a cross-vendor default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KxSuite {
    /// X25519 (Curve25519). 32-byte public key. **Non-spec** — opt-in only.
    X25519,
    /// NIST P-256 ECDH. 65-byte uncompressed public key (0x04 || X || Y).
    /// Spec default (§9.3.2.3.1), cross-vendor.
    EcdhP256,
}

impl Default for KxSuite {
    fn default() -> Self {
        Self::EcdhP256
    }
}

impl KxSuite {
    /// Expected length of the public-key bytes.
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

/// Ephemeral DH key pair for a single handshake.
///
/// After `derive_shared_secret` the instance is consumed — by `ring`'s
/// API design the `EphemeralPrivateKey` allows only one
/// `agree_ephemeral` call, which enforces PFS (perfect forward secrecy).
pub struct KeyExchange {
    suite: KxSuite,
    private: EphemeralPrivateKey,
    public: PublicKey,
}

impl KeyExchange {
    /// Generates a fresh key pair with the default suite
    /// ([`KxSuite::EcdhP256`], the spec/cross-vendor choice).
    ///
    /// # Errors
    /// `CryptoFailed` if the system RNG is not available (e.g.
    /// no `/dev/urandom` in a broken-sandbox scenario).
    pub fn new() -> SecurityResult<Self> {
        Self::with_suite(KxSuite::default())
    }

    /// Generates a key pair for the chosen DH suite.
    ///
    /// # Errors
    /// see [`Self::new`].
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

    /// Returns the active DH suite.
    #[must_use]
    pub fn suite(&self) -> KxSuite {
        self.suite
    }

    /// Returns the local public key as a byte slice. The length is
    /// suite-dependent (see [`KxSuite::public_key_len`]).
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        self.public.as_ref()
    }

    /// Derives the SharedSecret from the remote public key. Consumes
    /// the local ephemeral key.
    ///
    /// # Errors
    /// * `BadArgument` if `remote_public_key.len() != suite.public_key_len()`.
    /// * `CryptoFailed` if `ring` rejects the agreement (e.g.
    ///   small-subgroup attack, identity point, off-curve point).
    pub fn derive_shared_secret(self, remote_public_key: &[u8]) -> SecurityResult<Vec<u8>> {
        if remote_public_key.len() != self.suite.public_key_len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!(
                    "keyexchange: {:?} public key must be {} bytes",
                    self.suite,
                    self.suite.public_key_len()
                ),
            ));
        }
        let peer = agreement::UnparsedPublicKey::new(self.suite.algorithm(), remote_public_key);
        agreement::agree_ephemeral(self.private, &peer, |raw_dh| {
            // Return the **raw** DH output (P-256: 32-byte X coordinate; X25519: 32-byte
            // shared secret) — NO additional HKDF. DDS-Security
            // §9.3.2.5 / cyclone `generate_shared_secret`: the SharedSecret is
            // `SHA256(raw_dh)` (in security-pki). An extra HKDF here would
            // corrupt the SharedSecret cross-vendor → VolatileSecure Kx key
            // mismatch → neither side can decode the other's SEC_* DATA.
            raw_dh.to_vec()
        })
        .map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: DH agreement rejected (invalid peer key?)",
            )
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn default_public_key_is_p256_65_bytes() {
        // The default suite is ECDH-P256 (spec, cross-vendor) → 65-byte
        // uncompressed public key (0x04 || X || Y).
        let kx = KeyExchange::new().unwrap();
        assert_eq!(kx.public_key().len(), 65);
        assert_eq!(kx.public_key()[0], 0x04);
    }

    #[test]
    fn x25519_opt_in_public_key_is_32_bytes() {
        let kx = KeyExchange::with_suite(KxSuite::X25519).unwrap();
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
        assert_eq!(s1, s2, "alice + bob must derive an identical secret");
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

        assert_ne!(s1, s2, "different ephemerals → different secret (PFS)");
    }

    #[test]
    fn wrong_length_public_key_rejected() {
        let alice = KeyExchange::new().unwrap();
        let err = alice.derive_shared_secret(&[0u8; 16]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn zero_public_key_rejected_by_ring() {
        // All-zero X25519 public key → the agreement must fail
        // (identity point / small-subgroup attack).
        let alice = KeyExchange::with_suite(KxSuite::X25519).unwrap();
        let err = alice.derive_shared_secret(&[0u8; 32]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    // -------------------------------------------------------------
    // P-256 ECDH
    // -------------------------------------------------------------

    #[test]
    fn default_suite_is_ecdh_p256() {
        // Spec default (§9.3.2.3.1), cross-vendor. X25519 is opt-in only.
        let kx = KeyExchange::new().unwrap();
        assert_eq!(kx.suite(), KxSuite::EcdhP256);
    }

    #[test]
    fn p256_public_key_is_65_bytes() {
        let kx = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        assert_eq!(kx.suite(), KxSuite::EcdhP256);
        assert_eq!(kx.public_key().len(), 65);
        // The uncompressed format starts with 0x04.
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
        // 65 bytes, starts with 0x04, but the rest is null bytes →
        // not a valid P-256 point.
        let alice = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        let mut bogus = [0u8; 65];
        bogus[0] = 0x04;
        let err = alice.derive_shared_secret(&bogus).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn x25519_and_p256_produce_different_public_key_lengths() {
        let a = KeyExchange::with_suite(KxSuite::X25519).unwrap();
        let b = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        assert_ne!(a.public_key().len(), b.public_key().len());
    }
}
