// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-keyexchange`. Safety classification: **SAFE** (a thin wrapper around `ring::agreement` + `ring::hkdf`).
//!
//! X25519 / P-256 ephemeral Diffie-Hellman for the
//! DDS-Security 1.2 authentication handshake (spec §10.3.4; key
//! establishment algorithms §8.3).
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
//! ## Public API
//!
//! - [`KeyExchange`] + [`KxSuite::EcdhP256`] (`ECDHE-CEUM+P256`,
//!   `CryptoAlgorithm Id 0x21`) / [`KxSuite::EcdhP384`]
//!   (`ECDHE-CEUM+P384`, `0x22`) / [`KxSuite::X25519`] — ephemeral-DH
//!   roundtrip with deterministic SharedSecret derivation.
//!
//! [`KxSuite::DhModp2048_256`] is `DHE+MODP-2048-256` (`CryptoAlgorithm Id
//! 0x20`, §8.3 Table 26): finite-field Diffie-Hellman over the RFC 5114 §2.3
//! group, in `modp` (`ring` has no finite-field DH).
//!
//! ## Non-goals
//!
//! There is no RSA key transport in DDS-Security 1.2: §8.3 Table 26
//! names the three DH suites above and nothing else. An earlier note
//! here claimed "RSA-OAEP, spec §8.3.2.11"; that clause does not exist
//! in the specification.

// Needs `std`: the key pairs are drawn from the backend's `SystemRandom`, an
// operating-system random source.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod modp;

// Crypto primitive backend: `ring` (default) or `aws-lc-rs` (FIPS, via `aws-lc`).
// ring-API-compatible; the agreement/rand calls below go through `backend::`.
#[cfg(feature = "aws-lc")]
pub(crate) use aws_lc_rs as backend;
#[cfg(all(
    feature = "ring-backend",
    not(any(feature = "aws-lc", feature = "wolfcrypt"))
))]
pub(crate) use ring as backend;
#[cfg(all(feature = "wolfcrypt", not(feature = "aws-lc")))]
pub(crate) use wolfcrypt_compat as backend;
#[cfg(not(any(feature = "ring-backend", feature = "aws-lc", feature = "wolfcrypt")))]
compile_error!(
    "enable exactly one crypto backend: `ring-backend` (default), `aws-lc`, or `wolfcrypt`"
);

use alloc::vec::Vec;

use crate::backend::agreement::{
    self, ECDH_P256, ECDH_P384, EphemeralPrivateKey, PublicKey, X25519,
};
use crate::backend::rand::SystemRandom;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

/// DH suite selection.
///
/// The default is `EcdhP256` (NIST P-256, `ECDH+prime256v1-CEUM`) — the
/// cross-vendor-interoperable key agreement mandated by the
/// DDS-Security-1.2 spec (§10.3.1.4, §8.3 Table 26) (cyclone/FastDDS/OpenDDS). `X25519` is **not** a spec algorithm,
/// but a ZeroDDS vendor extension — only opt-in between pure ZeroDDS peers
/// (via `with_suite`), NEVER as a cross-vendor default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KxSuite {
    /// X25519 (Curve25519). 32-byte public key. **Non-spec** — opt-in only.
    X25519,
    /// NIST P-256 ECDH. 65-byte uncompressed public key (0x04 || X || Y).
    /// Spec default (§9.3.2.3.1), cross-vendor.
    EcdhP256,
    /// `ECDHE-CEUM+P384` — DDS-Security 1.2 §8.3 Table 26,
    /// `CryptoAlgorithm Id 0x22`: the Cofactor Ephemeral Unified Model
    /// over NIST P-384 (secp384r1). 97-byte uncompressed public key
    /// (`0x04 || X || Y`).
    EcdhP384,
    /// `DHE+MODP-2048-256` — DDS-Security 1.2 §8.3 Table 26,
    /// `CryptoAlgorithm Id 0x20`: the Diffie-Hellman public key of the 2048-bit
    /// MODP group with 256-bit prime order subgroup of RFC 5114 §2.3, the
    /// big-endian representation of the number (up to 256 octets).
    DhModp2048_256,
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
            Self::EcdhP384 => 97,
            // The largest; the public key is the number without leading zeros.
            Self::DhModp2048_256 => modp::MODULUS_LEN,
        }
    }

    /// `CryptoAlgorithm Id` of the suite — DDS-Security 1.2 §8.3
    /// Table 26. `None` for [`Self::X25519`], which the table does not
    /// name and which therefore has no identifier to announce.
    #[must_use]
    pub const fn crypto_algorithm_id(self) -> Option<u8> {
        match self {
            Self::X25519 => None,
            Self::EcdhP256 => Some(0x21),
            Self::EcdhP384 => Some(0x22),
            Self::DhModp2048_256 => Some(0x20),
        }
    }

    /// The `ring` algorithm of an elliptic-curve suite; `None` for the
    /// finite-field one.
    fn algorithm(self) -> Option<&'static agreement::Algorithm> {
        match self {
            Self::X25519 => Some(&X25519),
            Self::EcdhP256 => Some(&ECDH_P256),
            Self::EcdhP384 => Some(&ECDH_P384),
            Self::DhModp2048_256 => None,
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
    key: Ephemeral,
}

/// The ephemeral key of a suite.
enum Ephemeral {
    /// `ring`: X25519, P-256, P-384.
    Curve {
        private: EphemeralPrivateKey,
        public: PublicKey,
    },
    /// `DHE+MODP-2048-256`.
    Modp(modp::ModpKey),
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
        let Some(algorithm) = suite.algorithm() else {
            // Rejection sampling: x uniform in [1, q-1].
            for _ in 0..64 {
                let mut random = [0u8; 32];
                crate::backend::rand::SecureRandom::fill(&rng, &mut random).map_err(|_| {
                    SecurityError::new(
                        SecurityErrorKind::CryptoFailed,
                        "keyexchange: system RNG unavailable",
                    )
                })?;
                let key = modp::ModpKey::from_random(&random);
                zeroize::Zeroize::zeroize(&mut random);
                if let Some(key) = key {
                    return Ok(Self {
                        suite,
                        key: Ephemeral::Modp(key),
                    });
                }
            }
            return Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: no private exponent in range after 64 draws",
            ));
        };
        let private = EphemeralPrivateKey::generate(algorithm, &rng).map_err(|_| {
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
            key: Ephemeral::Curve { private, public },
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
        match &self.key {
            Ephemeral::Curve { public, .. } => public.as_ref(),
            Ephemeral::Modp(key) => key.public(),
        }
    }

    /// Derives the SharedSecret from the remote public key. Consumes
    /// the local ephemeral key.
    ///
    /// # Errors
    /// * `BadArgument` if `remote_public_key.len() != suite.public_key_len()`.
    /// * `CryptoFailed` if `ring` rejects the agreement (e.g.
    ///   small-subgroup attack, identity point, off-curve point).
    pub fn derive_shared_secret(self, remote_public_key: &[u8]) -> SecurityResult<Vec<u8>> {
        let (private, algorithm) = match (self.key, self.suite.algorithm()) {
            (Ephemeral::Modp(key), _) => return key.agree(remote_public_key),
            (Ephemeral::Curve { private, .. }, Some(algorithm)) => (private, algorithm),
            (Ephemeral::Curve { .. }, None) => {
                return Err(SecurityError::new(
                    SecurityErrorKind::Internal,
                    "keyexchange: a curve key for a finite-field suite",
                ));
            }
        };
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
        let peer = agreement::UnparsedPublicKey::new(algorithm, remote_public_key);
        // Return the **raw** DH output (P-256: 32-byte X coordinate; X25519: 32-byte
        // shared secret) — NO additional HKDF. DDS-Security §9.3.2.5 / cyclone
        // `generate_shared_secret`: the SharedSecret is `SHA256(raw_dh)` (in
        // security-pki). An extra HKDF here would corrupt the SharedSecret
        // cross-vendor → VolatileSecure Kx key mismatch → neither side can decode
        // the other's SEC_* DATA.
        // ring 0.17 changed `agree_ephemeral`: 3 args + an infallible kdf
        // (`FnOnce(&[u8]) -> R`); aws-lc-rs (and wolfcrypt-ring-compat, which
        // mirrors aws-lc-rs' API) kept the older 4-arg form with an `error_value`
        // and a fallible kdf (`FnOnce(&[u8]) -> Result<R, E>`).
        #[cfg(not(any(feature = "aws-lc", feature = "wolfcrypt")))]
        let agreed = agreement::agree_ephemeral(private, &peer, |raw_dh| raw_dh.to_vec());
        #[cfg(any(feature = "aws-lc", feature = "wolfcrypt"))]
        let agreed = agreement::agree_ephemeral(private, peer, (), |raw_dh| {
            Ok::<Vec<u8>, ()>(raw_dh.to_vec())
        });
        let secret = agreed.map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: DH agreement rejected (invalid peer key?)",
            )
        })?;
        // Contributory-behaviour / small-subgroup guard (RFC 7748 §6.1). An
        // all-zero peer X25519 key (identity point) yields an all-zero shared
        // secret. ring and aws-lc-rs reject this at agreement time; wolfSSL
        // computes it silently — so we enforce the rejection ourselves and the
        // contract holds identically across every backend.
        if secret.iter().all(|&b| b == 0) {
            return Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "keyexchange: DH agreement produced an all-zero shared secret (degenerate peer key)",
            ));
        }
        Ok(secret)
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

    /// DDS-Security 1.2 §8.3 Table 26: `DHE+MODP-2048-256`, Id 0x20; the
    /// public key is a number of up to 256 octets, and the two parties
    /// derive the same secret.
    #[test]
    fn modp_2048_256_agrees_and_has_id_0x20() {
        assert_eq!(KxSuite::DhModp2048_256.crypto_algorithm_id(), Some(0x20));
        let alice = KeyExchange::with_suite(KxSuite::DhModp2048_256).unwrap();
        let bob = KeyExchange::with_suite(KxSuite::DhModp2048_256).unwrap();
        assert!(alice.public_key().len() <= 256 && alice.public_key().len() > 200);
        assert_ne!(alice.public_key()[0], 0, "no leading zero");
        let (a_pub, b_pub) = (alice.public_key().to_vec(), bob.public_key().to_vec());
        let s1 = alice.derive_shared_secret(&b_pub).unwrap();
        let s2 = bob.derive_shared_secret(&a_pub).unwrap();
        assert_eq!(s1, s2);
        let carol = KeyExchange::with_suite(KxSuite::DhModp2048_256).unwrap();
        assert!(
            carol.derive_shared_secret(&[1]).is_err(),
            "1 is no public key"
        );
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

    /// DDS-Security 1.2 §8.3 Table 26 names `ECDHE-CEUM+P384` under
    /// `CryptoAlgorithm Id 0x22`, with the Diffie-Hellman public key on
    /// NIST P-384 (secp384r1). Uncompressed point over a 384-bit curve
    /// is `0x04` plus two 48-octet coordinates = 97 octets, and two
    /// parties must land on the same secret.
    #[test]
    fn p384_matches_the_spec_table_and_derives_one_shared_secret() {
        assert_eq!(KxSuite::EcdhP384.crypto_algorithm_id(), Some(0x22));
        assert_eq!(KxSuite::EcdhP256.crypto_algorithm_id(), Some(0x21));
        assert_eq!(
            KxSuite::X25519.crypto_algorithm_id(),
            None,
            "X25519 is not in Table 26 and has no id to announce"
        );

        let alice = KeyExchange::with_suite(KxSuite::EcdhP384).unwrap();
        let bob = KeyExchange::with_suite(KxSuite::EcdhP384).unwrap();
        assert_eq!(alice.public_key().len(), 97);
        assert_eq!(alice.public_key()[0], 0x04, "uncompressed point");
        assert_eq!(KxSuite::EcdhP384.public_key_len(), 97);

        let a_pub = alice.public_key().to_vec();
        let b_pub = bob.public_key().to_vec();
        let s1 = alice.derive_shared_secret(&b_pub).unwrap();
        let s2 = bob.derive_shared_secret(&a_pub).unwrap();
        assert_eq!(s1, s2);
        assert_ne!(a_pub, b_pub);
    }

    /// A P-384 endpoint must not accept a P-256 public key: the point
    /// lies on a different curve and is the wrong length.
    #[test]
    fn p384_rejects_a_p256_public_key() {
        let alice = KeyExchange::with_suite(KxSuite::EcdhP384).unwrap();
        let bob = KeyExchange::with_suite(KxSuite::EcdhP256).unwrap();
        let err = alice.derive_shared_secret(bob.public_key()).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
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
