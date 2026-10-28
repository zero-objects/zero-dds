// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DHE+MODP-2048-256` (DDS-Security 1.2 §8.3 Table 26, `CryptoAlgorithm Id
//! 0x20`): finite-field Diffie-Hellman over the 2048-bit MODP group with a
//! 256-bit prime order subgroup of RFC 5114 §2.3, the "dhEphem, C(2e, 0s, FFC
//! DH) Scheme" of NIST SP 800-56A Rev. 2 §6.1.2.1.
//!
//! The public key is the big-endian representation of the number without
//! leading zeros (Table 26: `BN_bn2bin`), the shared secret the same
//! representation of `peer^x mod p` (OpenSSL `DH_compute_key`, which Cyclone
//! DDS calls). The peer's key is checked as SP 800-56A §5.6.2.3.1 asks: in
//! `[2, p-2]` and in the subgroup of order `q`.
//!
//! The exponentiation runs in Montgomery form of `crypto-bigint`, whose `pow`
//! takes the same time for every exponent of a width; the private exponent is
//! a 256-bit number that is zeroed when the key is dropped.

use alloc::vec::Vec;

use crypto_bigint::modular::{FixedMontyForm, FixedMontyParams};
use crypto_bigint::{Odd, U256, U2048};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zeroize::Zeroize;

/// Prime `p` of RFC 5114 §2.3.
pub(crate) const P_HEX: &str = concat!(
    "87a8e61db4b6663cffbbd19c651959998ceef608660dd0f25d2ceed4435e3b00",
    "e00df8f1d61957d4faf7df4561b2aa3016c3d91134096faa3bf4296d830e9a7c",
    "209e0c6497517abd5a8a9d306bcf67ed91f9e6725b4758c022e0b1ef4275bf7b",
    "6c5bfc11d45f9088b941f54eb1e59bb8bc39a0bf12307f5c4fdb70c581b23f76",
    "b63acae1caa6b7902d52526735488a0ef13c6d9a51bfa4ab3ad8347796524d8e",
    "f6a167b5a41825d967e144e5140564251ccacb83e6b486f6b3ca3f7971506026",
    "c0b857f689962856ded4010abd0be621c3a3960a54e710c375f26375d7014103",
    "a4b54330c198af126116d2276e11715f693877fad7ef09cadb094ae91e1a1597",
);
/// Generator `g` of RFC 5114 §2.3.
pub(crate) const G_HEX: &str = concat!(
    "3fb32c9b73134d0b2e77506660edbd484ca7b18f21ef205407f4793a1a0ba125",
    "10dbc15077be463fff4fed4aac0bb555be3a6c1b0c6b47b1bc3773bf7e8c6f62",
    "901228f8c28cbb18a55ae31341000a650196f931c77a57f2ddf463e5e9ec144b",
    "777de62aaab8a8628ac376d282d6ed3864e67982428ebc831d14348f6f2f9193",
    "b5045af2767164e1dfc967c1fb3f2e55a4bd1bffe83b9c80d052b985d182ea0a",
    "db2a3b7313d3fe14c8484b1e052588b9b7d2bbd2df016199ecd06e1557cd0915",
    "b3353bbb64e0ec377fd028370df92b52c7891428cdc67eb6184b523d1db246c3",
    "2f63078490f00ef8d647d148d47954515e2327cfef98c582664b4c0f6cc41659",
);
/// Order `q` of the subgroup (256 bits) of RFC 5114 §2.3.
pub(crate) const Q_HEX: &str = "8cf83642a709a097b447997640129da299b1a47d1eb3750ba308b0fe64f5fbd3";

/// Octets of the group (the largest public key).
pub(crate) const MODULUS_LEN: usize = 256;

fn bad(message: &'static str) -> SecurityError {
    SecurityError::new(SecurityErrorKind::CryptoFailed, message)
}

fn modulus() -> U2048 {
    U2048::from_be_hex(P_HEX)
}

fn order() -> U256 {
    U256::from_be_hex(Q_HEX)
}

/// `base^exponent mod p` in Montgomery form. The parameters are built in
/// constant time and the exponentiation is constant-time in the exponent,
/// the private key `x` (crypto-bigint 0.7.6 fixed the secret-dependent
/// branches of RUSTSEC-2026-0387).
fn pow_mod_p<const E: usize>(base: &U2048, exponent: &crypto_bigint::Uint<E>) -> U2048 {
    let params = FixedMontyParams::new(Odd::<U2048>::from_be_hex(P_HEX));
    FixedMontyForm::new(base, &params).pow(exponent).retrieve()
}

/// The octets of `n` without leading zeros.
fn trimmed(n: &U2048) -> Vec<u8> {
    let bytes = n.to_be_bytes();
    let first = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
    bytes[first..].to_vec()
}

/// The ephemeral private key `x` and the public key `g^x mod p`.
pub(crate) struct ModpKey {
    x: U256,
    public: Vec<u8>,
}

impl Drop for ModpKey {
    fn drop(&mut self) {
        self.x.zeroize();
    }
}

impl ModpKey {
    /// Takes `x` from `random` (32 octets of the system RNG) if it is in
    /// `[1, q-1]`; `None` otherwise, and the caller draws again (rejection
    /// sampling keeps `x` uniform; about 0.55 of the draws pass). Computes the
    /// public key.
    pub(crate) fn from_random(random: &[u8; 32]) -> Option<Self> {
        let x = U256::from_be_slice(random);
        if x == U256::ZERO || x >= order() {
            return None;
        }
        let g = U2048::from_be_hex(G_HEX);
        let public = trimmed(&pow_mod_p(&g, &x));
        Some(Self { x, public })
    }

    /// The public key, big-endian without leading zeros.
    pub(crate) fn public(&self) -> &[u8] {
        &self.public
    }

    /// `peer^x mod p` for a public key of the peer that passes the checks of
    /// SP 800-56A §5.6.2.3.1.
    pub(crate) fn agree(&self, peer: &[u8]) -> SecurityResult<Vec<u8>> {
        if peer.is_empty() || peer.len() > MODULUS_LEN {
            return Err(bad("keyexchange: MODP public key has the wrong length"));
        }
        let mut padded = [0u8; MODULUS_LEN];
        padded[MODULUS_LEN - peer.len()..].copy_from_slice(peer);
        let y = U2048::from_be_slice(&padded);
        let p = modulus();
        // [2, p-2]
        let p_minus_2 = p.wrapping_sub(&U2048::from_u8(2));
        if y < U2048::from_u8(2) || y > p_minus_2 {
            return Err(bad("keyexchange: MODP public key outside [2, p-2]"));
        }
        if pow_mod_p(&y, &order()) != U2048::ONE {
            return Err(bad(
                "keyexchange: MODP public key is not in the subgroup of order q",
            ));
        }
        let secret = trimmed(&pow_mod_p(&y, &self.x));
        if secret.is_empty() {
            return Err(bad("keyexchange: MODP agreement produced zero"));
        }
        Ok(secret)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The hexadecimal values of `p`, `g` and `q` as RFC 5114 §2.3 prints them
    /// (`tests/fixtures/rfc5114_2_3.txt` is that text, from the RFC), read
    /// group by group.
    fn rfc_value(name: &str) -> alloc::string::String {
        let text = include_str!("../tests/fixtures/rfc5114_2_3.txt");
        let mut out = alloc::string::String::new();
        let mut inside = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix(&alloc::format!("{name} = ")) {
                inside = true;
                out.push_str(&rest.replace(' ', "").to_lowercase());
            } else if inside {
                let is_group_line = !trimmed.is_empty()
                    && trimmed
                        .split(' ')
                        .all(|g| g.len() == 8 && g.chars().all(|c| c.is_ascii_hexdigit()));
                if is_group_line {
                    out.push_str(&trimmed.replace(' ', "").to_lowercase());
                } else {
                    break;
                }
            }
        }
        out
    }

    /// The constants are the ones of RFC 5114 §2.3, byte for byte: the text
    /// of the RFC is the oracle.
    #[test]
    fn the_constants_are_the_ones_printed_in_rfc_5114_2_3() {
        assert_eq!(P_HEX, rfc_value("p"));
        assert_eq!(G_HEX, rfc_value("g"));
        assert_eq!(Q_HEX, rfc_value("q"));
        assert_eq!(P_HEX.len(), 512);
        assert_eq!(Q_HEX.len(), 64);
    }

    /// RFC 5114 §2.3: `p` has 2048 bits, `q` has 256 bits and divides `p - 1`,
    /// and `g` has order `q` (`g^q mod p == 1`, `g != 1`).
    #[test]
    fn the_group_has_the_structure_of_rfc_5114_2_3() {
        let (p, g, q) = (modulus(), U2048::from_be_hex(G_HEX), order());
        assert_eq!(p.bits_vartime(), 2048);
        assert_eq!(q.bits_vartime(), 256);
        let p_minus_1 = p.wrapping_sub(&U2048::ONE);
        let q_wide = U2048::from_be_slice(&{
            let mut b = [0u8; MODULUS_LEN];
            b[MODULUS_LEN - 32..].copy_from_slice(&q.to_be_bytes());
            b
        });
        assert_eq!(
            p_minus_1.rem(&crypto_bigint::NonZero::new(q_wide).unwrap()),
            U2048::ZERO
        );
        assert_ne!(g, U2048::ONE);
        assert_eq!(pow_mod_p(&g, &q), U2048::ONE);
    }

    #[test]
    fn two_parties_agree() {
        let a = ModpKey::from_random(&[0x11; 32]).unwrap();
        let b = ModpKey::from_random(&[0x22; 32]).unwrap();
        assert_ne!(a.public(), b.public());
        assert_eq!(a.agree(b.public()).unwrap(), b.agree(a.public()).unwrap());
    }

    /// A private exponent outside `[1, q-1]` is not taken.
    #[test]
    fn an_exponent_outside_the_range_is_rejected() {
        assert!(ModpKey::from_random(&[0; 32]).is_none());
        assert!(ModpKey::from_random(&[0xFF; 32]).is_none());
        let q: [u8; 32] = order().to_be_bytes().as_ref().try_into().unwrap();
        assert!(ModpKey::from_random(&q).is_none());
    }

    #[test]
    fn a_peer_key_outside_the_group_is_refused() {
        let a = ModpKey::from_random(&[0x33; 32]).unwrap();
        // 0, 1, p-1 and p are no public keys; 2 is outside the subgroup of q.
        let p = modulus();
        for bad in [
            alloc::vec![0u8],
            alloc::vec![1u8],
            trimmed(&p.wrapping_sub(&U2048::ONE)),
            trimmed(&p),
            alloc::vec![2u8],
            alloc::vec![],
            alloc::vec![0xFF; MODULUS_LEN + 1],
        ] {
            assert!(a.agree(&bad).is_err(), "{bad:02x?}");
        }
    }
}
