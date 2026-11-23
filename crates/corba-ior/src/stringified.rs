// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stringified IOR — CORBA 3.3 Part 2 §7.6.9.
//!
//! `IOR:<hex-encoded-cdr-encapsulation>`
//!
//! Example:
//! ```text
//! IOR:000000000000002849444c3a6f6d672e6f72672f436f73...
//! ```
//!
//! * "The case for characters in a stringified IOR is not significant"
//!   (§7.6.9, grammar rules (8) to (16)): the prefix `IOR:` and the hex
//!   digits are read in either case.
//! * Written with the prefix `IOR:` and lower-case hex digits.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::Endianness;

use crate::error::{IorError, IorResult};
use crate::ior::Ior;

/// The `IOR:` prefix as it is written; it is read without regard to case.
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

/// Decodes a stringified IOR; the prefix and the hex digits are read
/// without regard to case (§7.6.9).
///
/// # Errors
/// `MissingIorPrefix`, `OddHexLength`, `InvalidHexChar`, or
/// CDR error.
pub fn from_stringified(s: &str) -> IorResult<Ior> {
    let s = s.trim();
    let payload = match s.get(..STRINGIFIED_IOR_PREFIX.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(STRINGIFIED_IOR_PREFIX) => {
            &s[STRINGIFIED_IOR_PREFIX.len()..]
        }
        _ => return Err(IorError::MissingIorPrefix),
    };
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
        // The hex part in upper case behind the prefix as written.
        let mut canonical_prefix_then_upper = String::from("IOR:");
        canonical_prefix_then_upper.push_str(&s["IOR:".len()..].to_uppercase());
        let decoded = from_stringified(&canonical_prefix_then_upper).unwrap();
        assert_eq!(decoded, ior);
    }

    /// Part 2 §7.6.9: `<prefix> ::= <i><o><r>":"` with `<i> ::= "i" |
    /// "I"` and so on; "The case for characters in a stringified IOR is
    /// not significant."
    #[test]
    fn corba_p2_7_6_9_the_prefix_case_is_not_significant() {
        let ior = sample_ior();
        let s = to_stringified(&ior, Endianness::Big).unwrap();
        let hex = &s["IOR:".len()..];
        for prefix in ["ior:", "Ior:", "iOr:", "IoR:", "IOR:"] {
            let text = alloc::format!("{prefix}{hex}");
            assert_eq!(from_stringified(&text).unwrap(), ior, "{prefix}");
            let upper = alloc::format!("{prefix}{}", hex.to_uppercase());
            assert_eq!(from_stringified(&upper).unwrap(), ior, "{prefix} upper");
        }
        // Another scheme, or no colon, is no stringified IOR.
        for bad in ["ioR", "io:00", "IORX:00", "corbaloc::h/k", ""] {
            assert!(
                matches!(from_stringified(bad), Err(IorError::MissingIorPrefix)),
                "{bad}"
            );
        }
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
