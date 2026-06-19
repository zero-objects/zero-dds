// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stringified-IOR — Spec §13.6.10.
//!
//! `IOR:<hex-encoded-cdr-encapsulation>`
//!
//! Example:
//! ```text
//! IOR:000000000000002849444c3a6f6d672e6f72672f436f73...
//! ```
//!
//! * The `IOR:` prefix is case-sensitive (normative per spec).
//! * Hex bytes are lowercase per spec, but callers typically also
//!   accept uppercase. We read both and write lowercase.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::Endianness;

use crate::error::{IorError, IorResult};
use crate::ior::Ior;

/// `IOR:` prefix (case-sensitive).
pub const STRINGIFIED_IOR_PREFIX: &str = "IOR:";

/// Encodes an IOR into its stringified form.
///
/// `endianness` sets the wire endianness in the CDR encapsulation;
/// most ORBs (TAO/omniORB) emit big-endian.
///
/// # Errors
/// CDR error during encapsulation.
pub fn to_stringified(ior: &Ior, endianness: Endianness) -> IorResult<String> {
    let bytes = ior.encode_encapsulation(endianness)?;
    let mut out = String::with_capacity(STRINGIFIED_IOR_PREFIX.len() + bytes.len() * 2);
    out.push_str(STRINGIFIED_IOR_PREFIX);
    for b in bytes {
        out.push(hex_nibble(b >> 4));
        out.push(hex_nibble(b & 0x0f));
    }
    Ok(out)
}

/// Decodes a stringified IOR.
///
/// # Errors
/// `MissingIorPrefix`, `OddHexLength`, `InvalidHexChar`, or
/// CDR error.
pub fn from_stringified(s: &str) -> IorResult<Ior> {
    let s = s.trim();
    let payload = s
        .strip_prefix(STRINGIFIED_IOR_PREFIX)
        .ok_or(IorError::MissingIorPrefix)?;
    if payload.len() % 2 != 0 {
        return Err(IorError::OddHexLength);
    }
    let mut bytes = Vec::with_capacity(payload.len() / 2);
    let mut chars = payload.chars();
    while let (Some(hi), Some(lo)) = (chars.next(), chars.next()) {
        let h = hex_value(hi)?;
        let l = hex_value(lo)?;
        bytes.push((h << 4) | l);
    }
    Ok(Ior::decode_encapsulation(&bytes)?)
}

const fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + n - 10) as char,
        _ => '0', // unreachable for n & 0x0f, kept const-fn-safe.
    }
}

fn hex_value(c: char) -> IorResult<u8> {
    match c {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        'A'..='F' => Ok(c as u8 - b'A' + 10),
        other => Err(IorError::InvalidHexChar(other)),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::tagged_profile::TaggedProfile;
    use zerodds_corba_iiop::{IiopProfileBody, IiopVersion};

    fn sample_ior() -> Ior {
        let body = IiopProfileBody::new(
            IiopVersion::V1_2,
            "host.example".into(),
            7777,
            alloc::vec![0xde, 0xad, 0xbe, 0xef],
        );
        let profile = TaggedProfile::iiop(&body, Endianness::Big).unwrap();
        Ior::new("IDL:demo/Echo:1.0".into(), alloc::vec![profile])
    }

    #[test]
    fn round_trip_be() {
        let ior = sample_ior();
        let s = to_stringified(&ior, Endianness::Big).unwrap();
        assert!(s.starts_with("IOR:"));
        let decoded = from_stringified(&s).unwrap();
        assert_eq!(decoded, ior);
    }

    #[test]
    fn round_trip_le() {
        let ior = sample_ior();
        let s = to_stringified(&ior, Endianness::Little).unwrap();
        let decoded = from_stringified(&s).unwrap();
        assert_eq!(decoded, ior);
    }

    #[test]
    fn upper_and_lower_hex_both_decoded() {
        let ior = sample_ior();
        let s = to_stringified(&ior, Endianness::Big).unwrap();
        // Convert the hex part to upper-case; the prefix stays
        // case-sensitive `IOR:`.
        let mut canonical_prefix_then_upper = String::from("IOR:");
        canonical_prefix_then_upper.push_str(&s["IOR:".len()..].to_uppercase());
        let decoded = from_stringified(&canonical_prefix_then_upper).unwrap();
        assert_eq!(decoded, ior);
    }

    #[test]
    fn missing_prefix_is_diagnostic() {
        let err = from_stringified("00000000abcdef").unwrap_err();
        assert!(matches!(err, IorError::MissingIorPrefix));
    }

    #[test]
    fn odd_hex_length_is_diagnostic() {
        let err = from_stringified("IOR:abc").unwrap_err();
        assert!(matches!(err, IorError::OddHexLength));
    }

    #[test]
    fn non_hex_char_is_diagnostic() {
        let err = from_stringified("IOR:zz").unwrap_err();
        assert!(matches!(err, IorError::InvalidHexChar('z')));
    }

    #[test]
    fn nil_ior_round_trip() {
        let nil = Ior::default();
        let s = to_stringified(&nil, Endianness::Big).unwrap();
        let decoded = from_stringified(&s).unwrap();
        assert_eq!(decoded, nil);
        assert!(decoded.is_nil());
    }
}
