// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GSSUP — username/password token (Spec §24.7).
//!
//! GSSUP is a GSS-API mechanism with OID `2.23.130.1.1.1`. The
//! initial-context-token form (RFC 2743 §3.1) wraps the GSSUP token
//! in a DER-tagged container.
//!
//! ```text
//! struct InitialContextToken {
//!     CSI::UTF8String  username;
//!     CSI::UTF8String  password;
//!     CSI::GSS_NT_ExportedName target_name;
//! };
//! ```
//!
//! We provide encode/decode with CDR encapsulation; the RFC 2743
//! ASN.1 wrapper is the caller's responsibility (typically
//! `der-parser` or a manual tag-`0x60` wrap).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

/// `INITIAL_CONTEXT_TOKEN` tag (Spec §24.7.1: GSS-API OID tag).
pub const INITIAL_CONTEXT_TOKEN_TAG: u8 = 0x60;

/// `GSSUP` mechanism OID bytes (DER-encoded). OID = `2.23.130.1.1.1`.
pub const GSSUP_OID_DER: &[u8] = &[0x06, 0x06, 0x67, 0x81, 0x02, 0x01, 0x01, 0x01];

/// GSSUP credential token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GssupCredentialToken {
    /// `username` (UTF-8).
    pub username: String,
    /// `password` (UTF-8).
    pub password: String,
    /// `target_name` (GSS exported name in ASN.1 DER form, or simply
    /// realm bytes).
    pub target_name: Vec<u8>,
}

/// Encode error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GssupError {
    /// CDR buffer error.
    Cdr(String),
    /// Length overflow.
    Overflow,
}

impl GssupCredentialToken {
    /// Constructor.
    #[must_use]
    pub fn new(username: String, password: String, target_name: Vec<u8>) -> Self {
        Self {
            username,
            password,
            target_name,
        }
    }

    /// Encodes as a **wire-correct** CDR encapsulation: byte-order octet,
    /// then the fields with alignment relative to the encapsulation start
    /// (ONE `BufferWriter`, auto-align — the `string` length u32 thus lands
    /// at offset 4, as omniORB/TAO expect; NOT the separate-writer style,
    /// which misaligns it to offset 1).
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, GssupError> {
        let mut w = BufferWriter::new(endianness);
        w.write_u8(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        })
        .map_err(cdr)?; // byte-order octet, offset 0
        // GSSUP fields are `sequence<octet>` (CSIv2 §16.2.3 InitialContextToken):
        // length + bytes, NO string null terminator (byte-identical to JacORB).
        write_octets(&mut w, self.username.as_bytes())?;
        write_octets(&mut w, self.password.as_bytes())?;
        write_octets(&mut w, &self.target_name)?;
        Ok(w.into_bytes())
    }

    /// Decodes a CDR encapsulation (counterpart to
    /// [`Self::encode_encapsulation`]). Reads over the ENTIRE buffer
    /// including the byte-order octet at offset 0, so the alignment works
    /// out correctly relative to the encapsulation origin.
    ///
    /// # Errors
    /// CDR buffer error or invalid endianness octet.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, GssupError> {
        let endianness = match bytes.first() {
            Some(0) => Endianness::Big,
            Some(1) => Endianness::Little,
            _ => return Err(GssupError::Cdr("invalid/empty encapsulation".into())),
        };
        let mut r = BufferReader::new(bytes, endianness);
        r.read_u8().map_err(cdr)?; // byte-order octet
        // sequence<octet> (no string null terminator).
        let username =
            String::from_utf8(read_octets(&mut r)?).map_err(|_| cdr_msg("username not UTF-8"))?;
        let password =
            String::from_utf8(read_octets(&mut r)?).map_err(|_| cdr_msg("password not UTF-8"))?;
        let target_name = read_octets(&mut r)?;
        Ok(Self {
            username,
            password,
            target_name,
        })
    }

    /// Wraps the GSSUP token in a **GSS-API InitialContextToken**
    /// (RFC 2743 §3.1 / CSIv2 §24.7.1): `0x60 <DER length> <GSSUP mech-OID DER>
    /// <GSSUP CDR encapsulation>`. This is the form that
    /// `EstablishContext.client_authentication_token` carries cross-ORB
    /// (omniORB/TAO/JacORB expect it).
    ///
    /// # Errors
    /// Encapsulation encode error.
    pub fn to_gss_token(&self, endianness: Endianness) -> Result<Vec<u8>, GssupError> {
        let encap = self.encode_encapsulation(endianness)?;
        let mut inner = Vec::with_capacity(GSSUP_OID_DER.len() + encap.len());
        inner.extend_from_slice(GSSUP_OID_DER);
        inner.extend_from_slice(&encap);
        let mut out = Vec::with_capacity(inner.len() + 4);
        out.push(INITIAL_CONTEXT_TOKEN_TAG); // 0x60 [APPLICATION 0]
        der_write_length(&mut out, inner.len());
        out.extend_from_slice(&inner);
        Ok(out)
    }

    /// Parses a GSS InitialContextToken (counterpart to [`Self::to_gss_token`]):
    /// expects tag `0x60`, the DER length, the GSSUP mech-OID, and the GSSUP
    /// encapsulation.
    ///
    /// # Errors
    /// Wrong tag/OID or decode error.
    pub fn from_gss_token(bytes: &[u8]) -> Result<Self, GssupError> {
        if bytes.first() != Some(&INITIAL_CONTEXT_TOKEN_TAG) {
            return Err(GssupError::Cdr(
                "not a GSS InitialContextToken (0x60)".into(),
            ));
        }
        let (len, body) = der_read_length(&bytes[1..])?;
        if body.len() < len {
            return Err(GssupError::Cdr("truncated GSS token".into()));
        }
        let inner = &body[..len];
        if !inner.starts_with(GSSUP_OID_DER) {
            return Err(GssupError::Cdr("GSS token mech-OID is not GSSUP".into()));
        }
        Self::decode_encapsulation(&inner[GSSUP_OID_DER.len()..])
    }
}

fn cdr<E: core::fmt::Debug>(e: E) -> GssupError {
    GssupError::Cdr(alloc::format!("{e:?}"))
}

fn cdr_msg(msg: &str) -> GssupError {
    GssupError::Cdr(alloc::string::ToString::to_string(msg))
}

/// Writes a `sequence<octet>` (CDR: u32 length + bytes, NO null terminator).
fn write_octets(w: &mut BufferWriter, data: &[u8]) -> Result<(), GssupError> {
    let n = u32::try_from(data.len()).map_err(|_| GssupError::Overflow)?;
    w.write_u32(n).map_err(cdr)?;
    w.write_bytes(data).map_err(cdr)?;
    Ok(())
}

/// Reads a `sequence<octet>`.
fn read_octets(r: &mut BufferReader<'_>) -> Result<Vec<u8>, GssupError> {
    let n = r.read_u32().map_err(cdr)? as usize;
    Ok(r.read_bytes(n).map_err(cdr)?.to_vec())
}

/// Append a DER definite length (X.690 §8.1.3) to `out`.
fn der_write_length(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
    } else {
        // Long form: 0x80|n, then n big-endian length bytes (leading zeros dropped).
        let bytes = (len as u64).to_be_bytes();
        let first = bytes.iter().position(|&b| b != 0).unwrap_or(7);
        let sig = &bytes[first..];
        out.push(0x80 | (sig.len() as u8));
        out.extend_from_slice(sig);
    }
}

/// Read a DER definite length; returns `(len, rest_after_the_length_field)`.
fn der_read_length(bytes: &[u8]) -> Result<(usize, &[u8]), GssupError> {
    let &first = bytes
        .first()
        .ok_or_else(|| GssupError::Cdr("missing DER length".into()))?;
    if first < 0x80 {
        return Ok((first as usize, &bytes[1..]));
    }
    let n = (first & 0x7f) as usize;
    if n == 0 || n > 8 || bytes.len() < 1 + n {
        return Err(GssupError::Cdr("bad DER long-form length".into()));
    }
    let mut len: usize = 0;
    for &b in &bytes[1..1 + n] {
        len = (len << 8) | b as usize;
    }
    Ok((len, &bytes[1 + n..]))
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
    fn encapsulation_is_wire_correct_octet_seq_len_at_offset_4() {
        // Wire-correct: octet@0, 3 pad, then the sequence<octet> length u32 at offset 4.
        let t = GssupCredentialToken::new("ab".into(), String::new(), Vec::new());
        let bytes = t.encode_encapsulation(Endianness::Big).unwrap();
        assert_eq!(bytes[0], 0); // BE octet
        assert_eq!(&bytes[1..4], &[0, 0, 0]); // padding up to offset 4
        // sequence<octet> "ab" → length 2 (NO NUL terminator), BE u32 at offset 4.
        assert_eq!(&bytes[4..8], &[0, 0, 0, 2]);
        assert_eq!(&bytes[8..10], b"ab");
    }

    #[test]
    fn gssup_byte_identical_to_jacorb() {
        // Cross-ORB conformance: JacORB 3.9 InitialContextTokenHelper.write
        // (alice/secret/target) — capture on the Linux test host, byte-identical.
        let t = GssupCredentialToken::new("alice".into(), "secret".into(), b"target".to_vec());
        let bytes = t.encode_encapsulation(Endianness::Big).unwrap();
        // encapsulation = BO octet(0) + 3 pad + GSSUP struct; JacORB writes the
        // struct without a wrapper from offset 0 → our struct from offset 4 must match.
        let hex: alloc::string::String = bytes[4..]
            .iter()
            .map(|b| alloc::format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "00000005616c69636500000000000006736563726574000000000006746172676574"
        );
    }

    #[test]
    fn gss_token_structure_and_round_trip() {
        let t = GssupCredentialToken::new("alice".into(), "secret".into(), Vec::new());
        let token = t.to_gss_token(Endianness::Big).unwrap();
        // 0x60 <len> <OID-DER> <encapsulation>.
        assert_eq!(token[0], 0x60);
        let (len, body) = der_read_length(&token[1..]).unwrap();
        assert_eq!(len, body.len());
        assert!(body.starts_with(GSSUP_OID_DER), "mech-OID = GSSUP");
        // Roundtrip through the wrapper.
        let back = GssupCredentialToken::from_gss_token(&token).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn der_length_short_and_long_form() {
        // Short form (<128) = one byte.
        let mut out = alloc::vec![];
        der_write_length(&mut out, 5);
        assert_eq!(out, alloc::vec![5]);
        assert_eq!(der_read_length(&out).unwrap().0, 5);
        // Long form (>=128): 0x81 <len>.
        let mut out = alloc::vec![];
        der_write_length(&mut out, 200);
        assert_eq!(out[0], 0x81);
        assert_eq!(der_read_length(&out).unwrap().0, 200);
        // Long form 2-byte: 0x82 <hi> <lo>.
        let mut out = alloc::vec![];
        der_write_length(&mut out, 300);
        assert_eq!(out[0], 0x82);
        assert_eq!(der_read_length(&out).unwrap().0, 300);
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
