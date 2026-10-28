// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The types and constants of the cryptographic algorithms of DDS-Security 1.2:
//! `CryptoAlgorithmId` (§7.3.7), `CryptoAlgorithmBit` (§7.3.8),
//! `CryptoAlgorithmSet` (§7.3.9), `CryptoTransformKeyRevision` (§7.3.17), and
//! the `CNAME_*`, `CID_*` and `CBIT_*` constants of §8.1 to §8.3.

/// `CryptoAlgorithmId` (§7.3.7): one octet; 0x01 to 0x7F are the
/// specification's, 0x80 to 0xFF are the vendor's.
pub type CryptoAlgorithmId = u8;

/// `CRYPTO_ALGORITHM_INVALID_ID` (§7.3.7).
pub const CRYPTO_ALGORITHM_INVALID_ID: CryptoAlgorithmId = 0x00;

/// `CryptoAlgorithmBit` (§7.3.8): a power of two.
pub type CryptoAlgorithmBit = u32;

/// `CRYPTO_ALGORITHM_COMPATIBILITY_MODE` (§7.3.8), the meaning of which
/// §7.3.10.1 defines.
pub const CRYPTO_ALGORITHM_COMPATIBILITY_MODE: CryptoAlgorithmBit = 0x8000_0000;

/// `CryptoAlgorithmSet` (§7.3.9): a bitmask of `CryptoAlgorithmBit`.
pub type CryptoAlgorithmSet = u32;

/// `CRYPTO_ALGORITHM_SET_ALL` (§7.3.9).
pub const CRYPTO_ALGORITHM_SET_ALL: CryptoAlgorithmSet = 0xffff_ffff;

/// `CRYPTO_ALGORITHM_SET_EMPTY` (§7.3.9).
pub const CRYPTO_ALGORITHM_SET_EMPTY: CryptoAlgorithmSet = 0x0000_0000;

/// `CryptoTransformKeyRevision` (§7.3.17): three octets.
pub type CryptoTransformKeyRevision = [u8; 3];

/// `CRYPTO_TRANSFORM_KEY_REVISION_NONE` (§7.3.17).
pub const CRYPTO_TRANSFORM_KEY_REVISION_NONE: CryptoTransformKeyRevision = [0, 0, 0];

/// `CryptoTransformKeyRevisionIntHolder` (§7.3.17).
pub type CryptoTransformKeyRevisionIntHolder = i32;

/// The int holder of a revision: `256*256*r[0] + 256*r[1] + r[2]` (§7.3.17).
#[must_use]
pub const fn key_revision_to_int(revision: CryptoTransformKeyRevision) -> i32 {
    (revision[0] as i32) * 256 * 256 + (revision[1] as i32) * 256 + revision[2] as i32
}

/// The revision of an int holder (the inverse of [`key_revision_to_int`]); the
/// value is taken modulo 2^24.
#[must_use]
pub const fn key_revision_from_int(value: i32) -> CryptoTransformKeyRevision {
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

// ---- §8.1 Table 22: symmetric cipher algorithms ----

/// `CNAME_AES128_GMAC` (§8.1; the specification gives it the same string as
/// `CNAME_AES128_GCM`: the name stands for the AEAD/MAC pair).
pub const CNAME_AES128_GMAC: &str = "AES128+GCM";
/// `CNAME_AES128_GCM` (§8.1).
pub const CNAME_AES128_GCM: &str = "AES128+GCM";
/// `CNAME_AES256_GMAC` (§8.1).
pub const CNAME_AES256_GMAC: &str = "AES256+GCM";
/// `CNAME_AES256_GCM` (§8.1).
pub const CNAME_AES256_GCM: &str = "AES256+GCM";

/// `CID_AES128_GMAC` (§8.1).
pub const CID_AES128_GMAC: CryptoAlgorithmId = 0x01;
/// `CID_AES128_GCM` (§8.1).
pub const CID_AES128_GCM: CryptoAlgorithmId = 0x02;
/// `CID_AES256_GMAC` (§8.1).
pub const CID_AES256_GMAC: CryptoAlgorithmId = 0x03;
/// `CID_AES256_GCM` (§8.1).
pub const CID_AES256_GCM: CryptoAlgorithmId = 0x04;

/// `CBIT_AES128_GMAC` (§8.1; the same bit as the GCM variant).
pub const CBIT_AES128_GMAC: CryptoAlgorithmBit = 1 << 0;
/// `CBIT_AES128_GCM` (§8.1).
pub const CBIT_AES128_GCM: CryptoAlgorithmBit = 1 << 0;
/// `CBIT_AES256_GMAC` (§8.1; the same bit as the GCM variant).
pub const CBIT_AES256_GMAC: CryptoAlgorithmBit = 1 << 1;
/// `CBIT_AES256_GCM` (§8.1).
pub const CBIT_AES256_GCM: CryptoAlgorithmBit = 1 << 1;

// ---- §8.2 Table 25: digital signature algorithms ----

/// `CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256` (§8.2).
pub const CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256: &str = "RSASSA-PSS-MGF1SHA256+2048+SHA256";
/// `CNAME_RSASSA_PKCS1_V15_2048_SHA256` (§8.2).
pub const CNAME_RSASSA_PKCS1_V15_2048_SHA256: &str = "RSASSA-PKCS1-V1_5+2048+SHA256";
/// `CNAME_ECDSA_P256_SHA256_NAME` (§8.2).
pub const CNAME_ECDSA_P256_SHA256_NAME: &str = "ECDSA+P256+SHA256";
/// `CNAME_ECDSA_P384_SHA384` (§8.2).
pub const CNAME_ECDSA_P384_SHA384: &str = "ECDSA+P384+SHA384";

/// `CID_RSASSA_PSS_MGF1SHA256_2048_SHA256` (§8.2).
pub const CID_RSASSA_PSS_MGF1SHA256_2048_SHA256: CryptoAlgorithmId = 0x10;
/// `CID_RSASSA_PKCS1_V15_2048_SHA256` (§8.2).
pub const CID_RSASSA_PKCS1_V15_2048_SHA256: CryptoAlgorithmId = 0x11;
/// `CID_ECDSA_P256_SHA256` (§8.2).
pub const CID_ECDSA_P256_SHA256: CryptoAlgorithmId = 0x12;
/// `CID_ECDSA_P384_SHA384` (§8.2).
pub const CID_ECDSA_P384_SHA384: CryptoAlgorithmId = 0x13;

/// `CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256` (§8.2).
pub const CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256: CryptoAlgorithmBit = 1 << 0;
/// `CBIT_RSASSA_PKCS1_V15_2048_SHA256` (§8.2).
pub const CBIT_RSASSA_PKCS1_V15_2048_SHA256: CryptoAlgorithmBit = 1 << 1;
/// `CBIT_ECDSA_P256_SHA256` (§8.2).
pub const CBIT_ECDSA_P256_SHA256: CryptoAlgorithmBit = 1 << 2;
/// `CBIT_ECDSA_P384_SHA384` (§8.2).
pub const CBIT_ECDSA_P384_SHA384: CryptoAlgorithmBit = 1 << 3;

// ---- §8.3 Table 26: key establishment algorithms ----

/// `CNAME_DHE_MODP_2048_256` (§8.3).
pub const CNAME_DHE_MODP_2048_256: &str = "DHE+MODP-2048-256";
/// `CNAME_ECDHE_CEUM_P256` (§8.3).
pub const CNAME_ECDHE_CEUM_P256: &str = "ECDHE-CEUM+P256";
/// `CNAME_ECDHE_CEUM_P384` (§8.3).
pub const CNAME_ECDHE_CEUM_P384: &str = "ECDHE-CEUM+P384";

/// `CID_DHE_MODP_2048_256` (§8.3).
pub const CID_DHE_MODP_2048_256: CryptoAlgorithmId = 0x20;
/// `CID_ECDHE_CEUM_P256` (§8.3).
pub const CID_ECDHE_CEUM_P256: CryptoAlgorithmId = 0x21;
/// `CID_ECDHE_CEUM_P384` (§8.3).
pub const CID_ECDHE_CEUM_P384: CryptoAlgorithmId = 0x22;

/// `CBIT_DHE_MODP_2048_256` (§8.3).
pub const CBIT_DHE_MODP_2048_256: CryptoAlgorithmBit = 1 << 0;
/// `CBIT_ECDHE_CEUM_P256` (§8.3).
pub const CBIT_ECDHE_CEUM_P256: CryptoAlgorithmBit = 1 << 1;
/// `CBIT_ECDHE_CEUM_P384` (§8.3).
pub const CBIT_ECDHE_CEUM_P384: CryptoAlgorithmBit = 1 << 2;

#[cfg(test)]
mod tests {
    use super::*;

    /// The values of §7.3.7 to §7.3.9, §8.1 to §8.3 as the specification prints
    /// them.
    #[test]
    fn the_constants_are_the_ones_of_the_specification() {
        assert_eq!(CRYPTO_ALGORITHM_INVALID_ID, 0x00);
        assert_eq!(CRYPTO_ALGORITHM_COMPATIBILITY_MODE, 0x8000_0000);
        assert_eq!(CRYPTO_ALGORITHM_SET_ALL, 0xffff_ffff);
        assert_eq!(CRYPTO_ALGORITHM_SET_EMPTY, 0);
        assert_eq!(
            [
                CID_AES128_GMAC,
                CID_AES128_GCM,
                CID_AES256_GMAC,
                CID_AES256_GCM
            ],
            [0x01, 0x02, 0x03, 0x04]
        );
        assert_eq!(CBIT_AES128_GMAC, CBIT_AES128_GCM);
        assert_eq!(CBIT_AES256_GMAC, CBIT_AES256_GCM);
        assert_eq!((CBIT_AES128_GCM, CBIT_AES256_GCM), (1, 2));
        assert_eq!(CNAME_AES128_GMAC, "AES128+GCM");
        assert_eq!(CNAME_AES256_GMAC, "AES256+GCM");
        assert_eq!(
            [
                CID_RSASSA_PSS_MGF1SHA256_2048_SHA256,
                CID_RSASSA_PKCS1_V15_2048_SHA256,
                CID_ECDSA_P256_SHA256,
                CID_ECDSA_P384_SHA384
            ],
            [0x10, 0x11, 0x12, 0x13]
        );
        assert_eq!(
            [
                CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256,
                CBIT_RSASSA_PKCS1_V15_2048_SHA256,
                CBIT_ECDSA_P256_SHA256,
                CBIT_ECDSA_P384_SHA384
            ],
            [1, 2, 4, 8]
        );
        assert_eq!(
            CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256,
            "RSASSA-PSS-MGF1SHA256+2048+SHA256"
        );
        assert_eq!(
            CNAME_RSASSA_PKCS1_V15_2048_SHA256,
            "RSASSA-PKCS1-V1_5+2048+SHA256"
        );
        assert_eq!(CNAME_ECDSA_P256_SHA256_NAME, "ECDSA+P256+SHA256");
        assert_eq!(CNAME_ECDSA_P384_SHA384, "ECDSA+P384+SHA384");
        assert_eq!(
            [
                CID_DHE_MODP_2048_256,
                CID_ECDHE_CEUM_P256,
                CID_ECDHE_CEUM_P384
            ],
            [0x20, 0x21, 0x22]
        );
        assert_eq!(
            [
                CBIT_DHE_MODP_2048_256,
                CBIT_ECDHE_CEUM_P256,
                CBIT_ECDHE_CEUM_P384
            ],
            [1, 2, 4]
        );
        assert_eq!(CNAME_DHE_MODP_2048_256, "DHE+MODP-2048-256");
        assert_eq!(CNAME_ECDHE_CEUM_P256, "ECDHE-CEUM+P256");
        assert_eq!(CNAME_ECDHE_CEUM_P384, "ECDHE-CEUM+P384");
    }

    /// §7.3.17: `int_holder_value = 256*256*r[0] + 256*r[1] + r[2]`.
    #[test]
    fn the_key_revision_int_holder_follows_the_formula() {
        assert_eq!(key_revision_to_int(CRYPTO_TRANSFORM_KEY_REVISION_NONE), 0);
        assert_eq!(key_revision_to_int([0, 0, 1]), 1);
        assert_eq!(key_revision_to_int([0, 1, 0]), 256);
        assert_eq!(key_revision_to_int([1, 2, 3]), 65_536 + 512 + 3);
        for r in [
            [0, 0, 0],
            [0, 0, 255],
            [0, 1, 0],
            [9, 8, 7],
            [255, 255, 255],
        ] {
            assert_eq!(key_revision_from_int(key_revision_to_int(r)), r);
        }
    }
}
