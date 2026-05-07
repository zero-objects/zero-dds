// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ShmLocator — Wire-Format fuer PID_SHM_LOCATOR (Spec §3.1).
//!
//! Layout (little-endian):
//!
//! ```text
//! +--- ShmLocator value ---+
//! | 0x00 | u32 | hostname_hash (FNV-1a)
//! | 0x04 | u32 | uid (POSIX uid_t)
//! | 0x08 | u32 | slot_count
//! | 0x0c | u32 | slot_size
//! | 0x10 |     | segment_path: u32 length + UTF-8 + 0 + pad to 4
//! +------------------------+
//! ```
//!
//! Caller-Layer (Discovery) sendet das via PID_SHM_LOCATOR=0x8001
//! im SEDP-Sample. Reader auf demselben Host matcht (siehe
//! `SameHostMatch`).

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// SHM-Locator: alle Daten die ein Same-Host-Reader braucht um zu
/// einem Writer-SHM-Segment zu attachen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShmLocator {
    /// FNV-1a Hash des Hostnamens. Same-Host-Match-Anker.
    pub hostname_hash: u32,
    /// POSIX-UID des Writer-Prozesses. Verhindert Cross-User-Attaches
    /// auf shared Hosts.
    pub uid: u32,
    /// Anzahl Slots im Segment.
    pub slot_count: u32,
    /// Slot-Total-Size (Header + Daten + Padding).
    pub slot_size: u32,
    /// SHM-Segment-Pfad (z.B. `/zddspub_<entity_id>`).
    pub segment_path: String,
}

/// Fehler beim Encode/Decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocatorError {
    /// Buffer zu kurz fuer den fixen Header (16 byte).
    TruncatedHeader,
    /// String-Length-Prefix passt nicht zum Buffer.
    TruncatedString,
    /// Path enthaelt Non-UTF-8.
    InvalidUtf8,
    /// Path zu lang (> 256 byte als DoS-Cap).
    PathTooLong,
}

impl core::fmt::Display for LocatorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TruncatedHeader => f.write_str("ShmLocator: truncated 16-byte header"),
            Self::TruncatedString => f.write_str("ShmLocator: string length out of buffer"),
            Self::InvalidUtf8 => f.write_str("ShmLocator: segment_path is not UTF-8"),
            Self::PathTooLong => f.write_str("ShmLocator: segment_path > 256 bytes"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LocatorError {}

/// Maximale Pfad-Laenge (DoS-Cap).
const MAX_PATH_LEN: usize = 256;

impl ShmLocator {
    /// Encoded little-endian. Layout siehe Modul-Doku.
    ///
    /// # Errors
    /// `PathTooLong` wenn `segment_path.len() > 256`.
    pub fn to_bytes_le(&self) -> Result<Vec<u8>, LocatorError> {
        let path_bytes = self.segment_path.as_bytes();
        if path_bytes.len() > MAX_PATH_LEN {
            return Err(LocatorError::PathTooLong);
        }
        // CDR-String: u32 length (incl. null-terminator) + UTF-8 + null + padding to 4.
        let str_len = u32::try_from(path_bytes.len() + 1).unwrap_or(u32::MAX);
        let mut out = Vec::with_capacity(16 + 4 + path_bytes.len() + 4);
        out.extend_from_slice(&self.hostname_hash.to_le_bytes());
        out.extend_from_slice(&self.uid.to_le_bytes());
        out.extend_from_slice(&self.slot_count.to_le_bytes());
        out.extend_from_slice(&self.slot_size.to_le_bytes());
        out.extend_from_slice(&str_len.to_le_bytes());
        out.extend_from_slice(path_bytes);
        out.push(0);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        Ok(out)
    }

    /// Decoded aus little-endian-Bytes.
    ///
    /// # Errors
    /// `TruncatedHeader`, `TruncatedString`, `InvalidUtf8`, `PathTooLong`.
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self, LocatorError> {
        if bytes.len() < 20 {
            return Err(LocatorError::TruncatedHeader);
        }
        let hostname_hash = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let uid = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let slot_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let slot_size = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let str_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        if str_len > MAX_PATH_LEN + 1 {
            return Err(LocatorError::PathTooLong);
        }
        if 20 + str_len > bytes.len() {
            return Err(LocatorError::TruncatedString);
        }
        // String inkl. null-terminator.
        let raw = &bytes[20..20 + str_len];
        let str_no_null = if raw.last() == Some(&0) {
            &raw[..raw.len() - 1]
        } else {
            raw
        };
        let segment_path =
            core::str::from_utf8(str_no_null).map_err(|_| LocatorError::InvalidUtf8)?;
        Ok(Self {
            hostname_hash,
            uid,
            slot_count,
            slot_size,
            segment_path: segment_path.into(),
        })
    }
}

/// FNV-1a Hash des Hostnamens (32-bit). Wird vom Writer beim Discovery
/// gesetzt; Reader prueft gegen den eigenen.
#[must_use]
pub fn fnv1a_32(bytes: &[u8]) -> u32 {
    const OFFSET: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let mut h = OFFSET;
    for b in bytes {
        h ^= u32::from(*b);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Same-Host-Match-Helper. Caller passes (lokaler hostname, lokaler uid)
/// und einen empfangenen `ShmLocator`; liefert `true` wenn beides
/// uebereinstimmt — d.h. wir koennen mmap auf das Segment.
#[must_use]
pub fn is_same_host(local_hostname: &str, local_uid: u32, locator: &ShmLocator) -> bool {
    let local_hash = fnv1a_32(local_hostname.as_bytes());
    local_hash == locator.hostname_hash && local_uid == locator.uid
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample() -> ShmLocator {
        ShmLocator {
            hostname_hash: fnv1a_32(b"node1.local"),
            uid: 1000,
            slot_count: 16,
            slot_size: 128,
            segment_path: "/zddspub_AB12CD".into(),
        }
    }

    #[test]
    fn roundtrip_le() {
        let l = sample();
        let bytes = l.to_bytes_le().expect("encode");
        let l2 = ShmLocator::from_bytes_le(&bytes).expect("decode");
        assert_eq!(l, l2);
    }

    #[test]
    fn truncated_header_errors() {
        assert_eq!(
            ShmLocator::from_bytes_le(&[0u8; 19]),
            Err(LocatorError::TruncatedHeader)
        );
    }

    #[test]
    fn path_too_long_errors() {
        let mut l = sample();
        l.segment_path = "x".repeat(MAX_PATH_LEN + 1);
        assert_eq!(l.to_bytes_le(), Err(LocatorError::PathTooLong));
    }

    #[test]
    fn fnv1a_known_value() {
        // Offizieller FNV-1a-Test-Vektor: hash("hello") = 0x4f9f2cab.
        assert_eq!(fnv1a_32(b"hello"), 0x4f9f_2cab);
    }

    #[test]
    fn same_host_match_positive() {
        let l = sample();
        assert!(is_same_host("node1.local", 1000, &l));
    }

    #[test]
    fn same_host_mismatch_uid() {
        let l = sample();
        assert!(!is_same_host("node1.local", 999, &l));
    }

    #[test]
    fn same_host_mismatch_hostname() {
        let l = sample();
        assert!(!is_same_host("node2.local", 1000, &l));
    }
}
