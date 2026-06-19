// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 token structures (DataHolder + property records).
//!
//! Spec: §7.2.6 (Property_t / BinaryProperty_t), §7.2.7 (DataHolder),
//! §7.4.1.4 (IdentityToken in the SPDP announce).
//!
//! A **DataHolder** is the generic wire representation of all
//! security tokens (IdentityToken, PermissionsToken, IdentityStatus-
//! Token, AuthRequestMessageToken, HandshakeMessageToken, CryptoToken,
//! ParticipantCryptoToken, etc.). Fields:
//!
//! ```text
//! struct DataHolder_t {
//!     string                       class_id;
//!     sequence<Property_t>         properties;
//!     sequence<BinaryProperty_t>   binary_properties;
//! };
//! struct Property_t        { string name; string value; };
//! struct BinaryProperty_t  { string name; sequence<octet> value; };
//! ```
//!
//! IMPORTANT: the wire `Property_t` (spec §7.2.6 Tab.5) has **no**
//! `propagate` field — that is local filter state in the plug-configured
//! `zerodds_security::PropertyList`.
//!
//! The encoding is OMG-CDR (XCDR1) — the DataHolder is serialized as a
//! big-/little-endian stream, depending on which encapsulation
//! context it sits in (typically PL_CDR_LE of the ParameterList, i.e. LE).
//!
//! # Usage
//!
//! ```
//! use zerodds_security::token::{DataHolder, IdentityToken};
//! let tok = IdentityToken::pki_dh_v12("01:23:45", "rsa-2048", "FA:CE", "rsa-2048");
//! let bytes = tok.to_cdr_le();
//! let back = DataHolder::from_cdr_le(&bytes).unwrap();
//! assert_eq!(tok, back);
//! ```

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{SecurityError, SecurityErrorKind, SecurityResult};

/// Spec §7.2.6 Tab.5 — wire `Property_t` (`name` + `value`, both
/// CDR strings). Unlike [`crate::Property`], no `propagate`
/// field; token properties are always propagated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireProperty {
    /// Property name (reverse-DNS, e.g. `"dds.cert.sn"`).
    pub name: String,
    /// Property value (UTF-8).
    pub value: String,
}

impl WireProperty {
    /// Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Spec §7.2.6 Tab.6 — wire `BinaryProperty_t` (`name` + `value` as
/// `sequence<octet>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BinaryProperty {
    /// Property name (reverse-DNS).
    pub name: String,
    /// Raw byte value.
    pub value: Vec<u8>,
}

impl BinaryProperty {
    /// Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Spec §7.2.7 Tab.7 — generic `DataHolder_t` wire struct. All
/// security tokens are type aliases of `DataHolder` with a fixed
/// `class_id` value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataHolder {
    /// Plugin class id, e.g. `"DDS:Auth:PKI-DH:1.2"`.
    pub class_id: String,
    /// String properties (spec §7.2.6 Tab.5).
    pub properties: Vec<WireProperty>,
    /// Binary properties (spec §7.2.6 Tab.6).
    pub binary_properties: Vec<BinaryProperty>,
}

impl DataHolder {
    /// Constructor with only `class_id`.
    #[must_use]
    pub fn new(class_id: impl Into<String>) -> Self {
        Self {
            class_id: class_id.into(),
            properties: Vec::new(),
            binary_properties: Vec::new(),
        }
    }

    /// Builder: adds a string property.
    #[must_use]
    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push(WireProperty::new(name, value));
        self
    }

    /// Builder: adds a binary property.
    #[must_use]
    pub fn with_binary_property(
        mut self,
        name: impl Into<String>,
        value: impl Into<Vec<u8>>,
    ) -> Self {
        self.binary_properties
            .push(BinaryProperty::new(name, value));
        self
    }

    /// Mut variant: sets a string property with replace-on-dup
    /// semantics (if the name already exists, the old value is
    /// overwritten — spec §7.2.6: per token each property
    /// name may appear only once).
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let n = name.into();
        if let Some(existing) = self.properties.iter_mut().find(|p| p.name == n) {
            existing.value = value.into();
        } else {
            self.properties.push(WireProperty {
                name: n,
                value: value.into(),
            });
        }
    }

    /// Mut variant: sets a binary property with replace-on-dup.
    pub fn set_binary_property(&mut self, name: impl Into<String>, value: impl Into<Vec<u8>>) {
        let n = name.into();
        if let Some(existing) = self.binary_properties.iter_mut().find(|p| p.name == n) {
            existing.value = value.into();
        } else {
            self.binary_properties.push(BinaryProperty {
                name: n,
                value: value.into(),
            });
        }
    }

    /// Looks up a string property by name.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_str())
    }

    /// Looks up a binary property by name.
    #[must_use]
    pub fn binary_property(&self, name: &str) -> Option<&[u8]> {
        self.binary_properties
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_slice())
    }

    /// XCDR1 little-endian encoder. Returns the bytes without the
    /// encapsulation header — that is supplied by the caller (e.g. the
    /// ParameterList value).
    #[must_use]
    pub fn to_cdr_le(&self) -> Vec<u8> {
        encode(self, true)
    }

    /// XCDR1-Big-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_be(&self) -> Vec<u8> {
        encode(self, false)
    }

    /// XCDR1-Decoder, Little-Endian.
    ///
    /// # Errors
    /// `BadArgument` if the bytes are not spec-conformant (e.g.
    /// premature end, overly long length prefixes).
    pub fn from_cdr_le(bytes: &[u8]) -> SecurityResult<Self> {
        decode(bytes, true)
    }

    /// Like [`from_cdr_le`](Self::from_cdr_le), additionally returns the number of
    /// bytes consumed. For inline `sequence<DataHolder>` decoding (spec
    /// `GenericMessageData`), where multiple DataHolders lie one after another
    /// without a length prefix.
    ///
    /// # Errors
    /// `BadArgument` on non-spec-conformant bytes.
    pub fn from_cdr_le_consumed(bytes: &[u8]) -> SecurityResult<(Self, usize)> {
        decode_consumed(bytes, true)
    }

    /// XCDR1-Decoder, Big-Endian.
    ///
    /// # Errors
    /// `BadArgument` if the bytes are not spec-conformant.
    pub fn from_cdr_be(bytes: &[u8]) -> SecurityResult<Self> {
        decode(bytes, false)
    }
}

// ---------------------------------------------------------------------
// Type aliases for the token families (spec name / class_id)
// ---------------------------------------------------------------------

/// IdentityToken (spec §7.4.1.4 Tab.16; PKI-DH §10.3.2 Tab.51).
/// Sent in the SPDP announce as `PID_IDENTITY_TOKEN` (0x1001).
pub type IdentityToken = DataHolder;

/// PermissionsToken (spec §7.4.1.5 Tab.17; PKI-Permissions §10.4.2
/// Tab.65). `PID_PERMISSIONS_TOKEN` (0x1002).
pub type PermissionsToken = DataHolder;

/// IdentityStatusToken (spec §7.4.1.6, §10.3.2 Tab.53). `PID_IDENTITY_
/// STATUS_TOKEN` (0x1006). Carrier for OCSP live status.
pub type IdentityStatusToken = DataHolder;

/// PermissionsCredentialToken (spec §10.4.2 Tab.64). Transported in the
/// stateless-topic handshake, not in SPDP.
pub type PermissionsCredentialToken = DataHolder;

/// AuthRequestMessageToken (Spec §10.3.2 Tab.55).
pub type AuthRequestMessageToken = DataHolder;

/// HandshakeMessageToken — Request/Reply/Final (Spec §10.3.2 Tab.56-58).
pub type HandshakeMessageToken = DataHolder;

/// CryptoToken (Spec §10.5.2 Tab.72).
pub type CryptoToken = DataHolder;

// ---------------------------------------------------------------------
// Convenience constructors for the typical token forms.
// ---------------------------------------------------------------------

/// Plugin class-id constants — spec-1.2-versioned (see C3.3).
pub mod class_id {
    /// Builtin PKI authentication (§10.3.2.1 Tab.51).
    pub const AUTH_PKI_DH_V12: &str = "DDS:Auth:PKI-DH:1.2";
    /// Builtin permissions access control (§10.4 Tab.69).
    pub const ACCESS_PERMISSIONS_V12: &str = "DDS:Access:Permissions:1.2";
    /// Builtin AES-GCM-GMAC crypto (§10.5 Tab.72).
    pub const CRYPTO_AES_GCM_GMAC_V12: &str = "DDS:Crypto:AES-GCM-GMAC:1.2";
    /// Permissions credential (§10.4.2 Tab.64).
    pub const ACCESS_PERMISSIONS_CREDENTIAL: &str = "DDS:Access:PermissionsCredential";
}

/// Property names — spec 1.2 (selection).
pub mod prop {
    /// IdentityToken: cert subject name serial. Spec §10.3.2.1 Tab.51.
    pub const CERT_SN: &str = "dds.cert.sn";
    /// IdentityToken: cert signature algorithm.
    pub const CERT_ALGO: &str = "dds.cert.algo";
    /// IdentityToken: CA subject name serial.
    pub const CA_SN: &str = "dds.ca.sn";
    /// IdentityToken: CA signature algorithm.
    pub const CA_ALGO: &str = "dds.ca.algo";
    /// PermissionsToken: permissions-CA subject name serial.
    /// Spec §10.4.2 Tab.65.
    pub const PERM_CA_SN: &str = "dds.perm_ca.sn";
    /// PermissionsToken: permissions-CA signature algorithm.
    pub const PERM_CA_ALGO: &str = "dds.perm_ca.algo";
}

impl IdentityToken {
    /// Builtin helper for the PKI-DH IdentityToken (§10.3.2.1 Tab.51).
    /// Properties `dds.cert.sn`, `dds.cert.algo`, `dds.ca.sn`,
    /// `dds.ca.algo`. No binary_properties.
    #[must_use]
    pub fn pki_dh_v12(
        cert_sn: impl Into<String>,
        cert_algo: impl Into<String>,
        ca_sn: impl Into<String>,
        ca_algo: impl Into<String>,
    ) -> Self {
        DataHolder::new(class_id::AUTH_PKI_DH_V12)
            .with_property(prop::CERT_SN, cert_sn)
            .with_property(prop::CERT_ALGO, cert_algo)
            .with_property(prop::CA_SN, ca_sn)
            .with_property(prop::CA_ALGO, ca_algo)
    }

    /// Builtin helper for the permissions token (§10.4.2 Tab.65).
    /// `class_id="DDS:Access:Permissions:1.2"`, properties
    /// `dds.perm_ca.sn` + `dds.perm_ca.algo`.
    #[must_use]
    pub fn permissions_v12(perm_ca_sn: impl Into<String>, perm_ca_algo: impl Into<String>) -> Self {
        DataHolder::new(class_id::ACCESS_PERMISSIONS_V12)
            .with_property(prop::PERM_CA_SN, perm_ca_sn)
            .with_property(prop::PERM_CA_ALGO, perm_ca_algo)
    }
}

// ---------------------------------------------------------------------
// XCDR1-Codec
// ---------------------------------------------------------------------
//
// Layout (LE or BE, depending on `little_endian`):
//   string class_id              -- aligned 4
//   sequence<Property> props     -- aligned 4
//     uint32 count               -- 4 byte
//     N x { string name; string value; }  -- aligned 4 each
//   sequence<BinaryProperty> bp  -- aligned 4
//     uint32 count
//     N x { string name; sequence<octet> value; }  -- aligned 4 each
//
// CDR string: uint32 length (incl. trailing \0) + UTF-8 bytes + \0 +
// padding to 4 bytes. Length=0 is allowed (= empty string).

/// Maximum counts/lengths we accept from the wire —
/// DoS cap. 64 KiB per token, 256 properties, 256 binary properties,
/// 8 KiB per property value.
const MAX_TOKEN_BYTES: usize = 64 * 1024;
const MAX_PROPS: u32 = 256;
const MAX_BIN_PROPS: u32 = 256;
const MAX_STRING_LEN: u32 = 8 * 1024;
const MAX_BINARY_LEN: u32 = 8 * 1024;

fn encode(d: &DataHolder, le: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    encode_string(&mut out, &d.class_id, le);
    encode_u32(&mut out, d.properties.len() as u32, le);
    for p in &d.properties {
        encode_string(&mut out, &p.name, le);
        encode_string(&mut out, &p.value, le);
    }
    encode_u32(&mut out, d.binary_properties.len() as u32, le);
    for p in &d.binary_properties {
        encode_string(&mut out, &p.name, le);
        encode_octet_seq(&mut out, &p.value, le);
    }
    out
}

fn decode(bytes: &[u8], le: bool) -> SecurityResult<DataHolder> {
    decode_consumed(bytes, le).map(|(dh, _)| dh)
}

/// Like [`decode`], additionally returns the number of bytes consumed —
/// needed for the inline decoding of `sequence<DataHolder>` (spec
/// `GenericMessageData`), where the DataHolders lie one after another
/// without a length prefix and the caller must advance the cursor.
fn decode_consumed(bytes: &[u8], le: bool) -> SecurityResult<(DataHolder, usize)> {
    if bytes.len() > MAX_TOKEN_BYTES {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "token: payload exceeds DoS cap",
        ));
    }
    let mut cur = Cursor::new(bytes);
    let class_id = cur.read_string(le)?;
    let prop_count = cur.read_u32(le)?;
    if prop_count > MAX_PROPS {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "token: property count exceeds cap",
        ));
    }
    let mut properties = Vec::with_capacity(prop_count as usize);
    for _ in 0..prop_count {
        let name = cur.read_string(le)?;
        let value = cur.read_string(le)?;
        properties.push(WireProperty { name, value });
    }
    let bin_count = cur.read_u32(le)?;
    if bin_count > MAX_BIN_PROPS {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "token: binary_property count exceeds cap",
        ));
    }
    let mut binary_properties = Vec::with_capacity(bin_count as usize);
    for _ in 0..bin_count {
        let name = cur.read_string(le)?;
        let value = cur.read_octet_seq(le)?;
        binary_properties.push(BinaryProperty { name, value });
    }
    Ok((
        DataHolder {
            class_id,
            properties,
            binary_properties,
        },
        cur.pos,
    ))
}

fn align_to(out: &mut Vec<u8>, n: usize) {
    let pad = (n - out.len() % n) % n;
    for _ in 0..pad {
        out.push(0);
    }
}

fn encode_u32(out: &mut Vec<u8>, v: u32, le: bool) {
    align_to(out, 4);
    if le {
        out.extend_from_slice(&v.to_le_bytes());
    } else {
        out.extend_from_slice(&v.to_be_bytes());
    }
}

fn encode_string(out: &mut Vec<u8>, s: &str, le: bool) {
    let bytes = s.as_bytes();
    let len = (bytes.len() + 1) as u32;
    encode_u32(out, len, le);
    out.extend_from_slice(bytes);
    out.push(0);
}

fn encode_octet_seq(out: &mut Vec<u8>, v: &[u8], le: bool) {
    encode_u32(out, v.len() as u32, le);
    out.extend_from_slice(v);
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn align_to(&mut self, n: usize) {
        let pad = (n - self.pos % n) % n;
        self.pos = self.pos.saturating_add(pad);
    }

    fn read_u32(&mut self, le: bool) -> SecurityResult<u32> {
        self.align_to(4);
        if self.pos + 4 > self.buf.len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "token: truncated u32",
            ));
        }
        let raw = [
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ];
        self.pos += 4;
        Ok(if le {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        })
    }

    fn read_string(&mut self, le: bool) -> SecurityResult<String> {
        let len = self.read_u32(le)?;
        if len > MAX_STRING_LEN {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "token: string exceeds cap",
            ));
        }
        if len == 0 {
            return Ok(String::new());
        }
        if self.pos + len as usize > self.buf.len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "token: truncated string",
            ));
        }
        let body = &self.buf[self.pos..self.pos + len as usize];
        self.pos += len as usize;
        // Spec: a trailing NUL is mandatory; UTF-8 must be valid.
        if let Some((_, rest)) = body.split_last() {
            if body.last() != Some(&0) {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "token: string missing trailing NUL",
                ));
            }
            let s = core::str::from_utf8(rest).map_err(|_| {
                SecurityError::new(SecurityErrorKind::BadArgument, "token: string not UTF-8")
            })?;
            Ok(s.to_string())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "token: zero-length string body",
            ))
        }
    }

    fn read_octet_seq(&mut self, le: bool) -> SecurityResult<Vec<u8>> {
        let len = self.read_u32(le)?;
        if len > MAX_BINARY_LEN {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "token: binary value exceeds cap",
            ));
        }
        if self.pos + len as usize > self.buf.len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "token: truncated binary",
            ));
        }
        let v = self.buf[self.pos..self.pos + len as usize].to_vec();
        self.pos += len as usize;
        Ok(v)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_holder_roundtrip_le() {
        let dh = DataHolder::new("DDS:Auth:PKI-DH:1.2");
        let bytes = dh.to_cdr_le();
        let back = DataHolder::from_cdr_le(&bytes).unwrap();
        assert_eq!(dh, back);
    }

    #[test]
    fn empty_data_holder_roundtrip_be() {
        let dh = DataHolder::new("DDS:Auth:PKI-DH:1.2");
        let bytes = dh.to_cdr_be();
        let back = DataHolder::from_cdr_be(&bytes).unwrap();
        assert_eq!(dh, back);
    }

    #[test]
    fn pki_dh_identity_token_roundtrip() {
        let tok =
            IdentityToken::pki_dh_v12("01:23:45:67", "ECDSA-SHA256", "FA:CE:0B:01", "RSA-SHA256");
        let bytes = tok.to_cdr_le();
        let back = DataHolder::from_cdr_le(&bytes).unwrap();
        assert_eq!(tok, back);
        assert_eq!(back.class_id, "DDS:Auth:PKI-DH:1.2");
        assert_eq!(back.property("dds.cert.sn"), Some("01:23:45:67"));
        assert_eq!(back.property("dds.cert.algo"), Some("ECDSA-SHA256"));
        assert_eq!(back.property("dds.ca.sn"), Some("FA:CE:0B:01"));
        assert_eq!(back.property("dds.ca.algo"), Some("RSA-SHA256"));
        assert!(back.binary_properties.is_empty());
    }

    #[test]
    fn permissions_token_roundtrip() {
        let tok = IdentityToken::permissions_v12("DE:AD:BE:EF", "ECDSA-SHA256");
        let le = tok.to_cdr_le();
        let be = tok.to_cdr_be();
        assert_eq!(tok, DataHolder::from_cdr_le(&le).unwrap());
        assert_eq!(tok, DataHolder::from_cdr_be(&be).unwrap());
        assert_ne!(le, be, "BE/LE streams differ");
    }

    #[test]
    fn token_with_binary_property_roundtrip() {
        let tok = DataHolder::new("DDS:Auth:PKI-DH:1.2")
            .with_property("dds.cert.sn", "01:23")
            .with_binary_property("dds.cert.bytes", vec![0xCA, 0xFE, 0xBA, 0xBE, 0xDE]);
        let bytes = tok.to_cdr_le();
        let back = DataHolder::from_cdr_le(&bytes).unwrap();
        assert_eq!(tok, back);
        assert_eq!(
            back.binary_property("dds.cert.bytes"),
            Some(&[0xCA, 0xFE, 0xBA, 0xBE, 0xDE][..])
        );
    }

    #[test]
    fn cdr_le_layout_class_id_only() {
        // class_id="A" (1 char + NUL = len=2). Expected stream:
        //   uint32 len=2 LE: 02 00 00 00
        //   bytes        :  41 00
        //   pad-to-4     :  00 00
        //   prop_count=0 :  00 00 00 00
        //   bin_count=0  :  00 00 00 00
        let dh = DataHolder::new("A");
        let bytes = dh.to_cdr_le();
        assert_eq!(
            bytes,
            vec![
                0x02, 0x00, 0x00, 0x00, b'A', 0x00, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
    }

    #[test]
    fn truncated_buffer_is_error() {
        let err = DataHolder::from_cdr_le(&[0x10, 0x00, 0x00]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn property_count_cap_rejects_huge() {
        // Forge a stream with prop_count = 1_000_000.
        let mut bytes = Vec::new();
        encode_string(&mut bytes, "X", true);
        encode_u32(&mut bytes, 1_000_000, true); // prop count > cap
        let err = DataHolder::from_cdr_le(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn missing_trailing_nul_rejected() {
        // Forge: class_id len=1 (no NUL).
        let bytes = vec![0x01, 0x00, 0x00, 0x00, b'A'];
        let err = DataHolder::from_cdr_le(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn dos_cap_overall_payload() {
        let big = vec![0u8; MAX_TOKEN_BYTES + 1];
        let err = DataHolder::from_cdr_le(&big).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
