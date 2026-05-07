// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GSSUP — Username/Password Token (Spec §24.7).
//!
//! GSSUP ist eine GSS-API-Mechanism mit OID `2.23.130.1.1.1`. Die
//! Initial-Context-Token-Form (RFC 2743 §3.1) wraps das GSSUP-Token
//! in einen DER-getaggten Container.
//!
//! ```text
//! struct InitialContextToken {
//!     CSI::UTF8String  username;
//!     CSI::UTF8String  password;
//!     CSI::GSS_NT_ExportedName target_name;
//! };
//! ```
//!
//! Wir liefern Encode/Decode mit CDR-Encapsulation; der RFC-2743-
//! ASN.1-Wrapper ist Caller-Sache (typisch `der-parser` oder
//! manueller Tag-`0x60`-Wrap).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

/// `INITIAL_CONTEXT_TOKEN`-Tag (Spec §24.7.1: GSS-API-OID-Tag).
pub const INITIAL_CONTEXT_TOKEN_TAG: u8 = 0x60;

/// `GSSUP`-Mechanism-OID-Bytes (DER-encoded). OID = `2.23.130.1.1.1`.
pub const GSSUP_OID_DER: &[u8] = &[0x06, 0x06, 0x67, 0x81, 0x02, 0x01, 0x01, 0x01];

/// GSSUP-Credential-Token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GssupCredentialToken {
    /// `username` (UTF-8).
    pub username: String,
    /// `password` (UTF-8).
    pub password: String,
    /// `target_name` (GSS-Exported-Name in ASN.1-DER-Form, oder
    /// einfach Realm-Bytes).
    pub target_name: Vec<u8>,
}

/// Encode-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GssupError {
    /// CDR-Buffer-Fehler.
    Cdr(String),
    /// Length-Overflow.
    Overflow,
}

impl GssupCredentialToken {
    /// Konstruktor.
    #[must_use]
    pub fn new(username: String, password: String, target_name: Vec<u8>) -> Self {
        Self {
            username,
            password,
            target_name,
        }
    }

    /// Encodiert als CDR-Encapsulation (`endianness-byte + body`).
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, GssupError> {
        let mut out = Vec::with_capacity(64);
        out.push(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        });
        let mut w = BufferWriter::new(endianness);
        w.write_string(&self.username)
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        w.write_string(&self.password)
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        let n = u32::try_from(self.target_name.len()).map_err(|_| GssupError::Overflow)?;
        w.write_u32(n)
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        w.write_bytes(&self.target_name)
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        out.extend_from_slice(w.as_bytes());
        Ok(out)
    }

    /// Decodiert eine CDR-Encapsulation.
    ///
    /// # Errors
    /// CDR-Buffer-Fehler oder Endianness-Octet ungueltig.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, GssupError> {
        if bytes.is_empty() {
            return Err(GssupError::Cdr("empty encapsulation".into()));
        }
        let endianness = match bytes[0] {
            0 => Endianness::Big,
            1 => Endianness::Little,
            _ => return Err(GssupError::Cdr("invalid endianness byte".into())),
        };
        let mut r = BufferReader::new(&bytes[1..], endianness);
        let username = r
            .read_string()
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        let password = r
            .read_string()
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        let n = r
            .read_u32()
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))? as usize;
        let target_bytes = r
            .read_bytes(n)
            .map_err(|e| GssupError::Cdr(alloc::format!("{e:?}")))?;
        Ok(Self {
            username,
            password,
            target_name: target_bytes.to_vec(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn initial_context_token_tag_is_0x60() {
        // Spec §24.7.1 / RFC 2743 §3.1.
        assert_eq!(INITIAL_CONTEXT_TOKEN_TAG, 0x60);
    }

    #[test]
    fn gssup_oid_starts_with_der_tag() {
        // 0x06 = OBJECT IDENTIFIER, 0x06 = length, 6 OID bytes follow.
        assert_eq!(GSSUP_OID_DER[0], 0x06);
        assert_eq!(GSSUP_OID_DER[1], 0x06);
    }

    #[test]
    fn token_round_trip_be() {
        let t =
            GssupCredentialToken::new("alice".into(), "swordfish".into(), b"REALM.LAB".to_vec());
        let bytes = t.encode_encapsulation(Endianness::Big).unwrap();
        assert_eq!(bytes[0], 0); // BE marker.
        let d = GssupCredentialToken::decode_encapsulation(&bytes).unwrap();
        assert_eq!(d, t);
    }

    #[test]
    fn token_round_trip_le() {
        let t = GssupCredentialToken::new("bob".into(), "x".into(), b"R".to_vec());
        let bytes = t.encode_encapsulation(Endianness::Little).unwrap();
        assert_eq!(bytes[0], 1);
        let d = GssupCredentialToken::decode_encapsulation(&bytes).unwrap();
        assert_eq!(d, t);
    }

    #[test]
    fn invalid_endianness_byte_is_diagnostic() {
        let err =
            GssupCredentialToken::decode_encapsulation(&[0xff, 0, 0, 0, 5, b'a']).unwrap_err();
        assert!(matches!(err, GssupError::Cdr(_)));
    }
}
