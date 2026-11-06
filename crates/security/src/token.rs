// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 token structures (DataHolder + property records).
//!
//! Spec: §7.3.2 (Property_t), §7.3.3 (BinaryProperty_t), §7.3.4 (DataHolder),
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
//! IMPORTANT: the wire `Property_t` (spec §7.3.2 Table 1) has **no**
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

/// Spec §7.3.2 Table 1 — wire `Property_t` (`name` + `value`, both
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

/// Spec §7.3.3 Table 2 — wire `BinaryProperty_t` (`name` + `value` as
/// `sequence<octet>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BinaryProperty {
    /// Property name (reverse-DNS).
    pub name: String,
    /// Raw byte value.
    pub value: Vec<u8>,
    /// `propagate` (DDS-Security 1.2 §7.3.3, Table 2): whether discovery
    /// carries the property. `@non-serialized` (§7.3.3.1): it is not on the
    /// wire of the token, and a property read from it has `false`.
    pub propagate: bool,
}

impl BinaryProperty {
    /// Constructor: for local use only (`propagate` is `false`).
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            propagate: false,
        }
    }

    /// Constructor: propagated by discovery.
    #[must_use]
    pub fn propagated(name: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            propagate: true,
            ..Self::new(name, value)
        }
    }
}

/// Spec §7.3.4 Table 3 — generic `DataHolder_t` wire struct. All
/// security tokens are type aliases of `DataHolder` with a fixed
/// `class_id` value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataHolder {
    /// Plugin class id, e.g. `"DDS:Auth:PKI-DH:1.2"`.
    pub class_id: String,
    /// String properties (spec §7.3.2 Table 1).
    pub properties: Vec<WireProperty>,
    /// Binary properties (spec §7.3.3 Table 2).
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
    /// overwritten — spec §7.3.2: per token each property
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
            self.binary_properties.push(BinaryProperty::new(n, value));
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

    /// XCDR1 encoder in the byte order `little_endian` names.
    #[must_use]
    pub fn to_cdr(&self, little_endian: bool) -> Vec<u8> {
        encode(self, little_endian)
    }

    /// XCDR1 decoder in the byte order `little_endian` names, with the
    /// number of bytes consumed — the inline `sequence<DataHolder>` of a
    /// `ParticipantGenericMessage` in either encapsulation.
    ///
    /// # Errors
    /// `BadArgument` on non-spec-conformant bytes.
    pub fn from_cdr_consumed(bytes: &[u8], little_endian: bool) -> SecurityResult<(Self, usize)> {
        decode_consumed(bytes, little_endian)
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

/// CryptoToken (Spec §10.5.2.1 Table 72).
pub type CryptoToken = DataHolder;

// ---------------------------------------------------------------------
// Convenience constructors for the typical token forms.
// ---------------------------------------------------------------------

/// Plugin class-id constants — spec-1.2-versioned (see C3.3).
pub mod class_id {
    /// Builtin PKI authentication (§10.3.2.1 Tab.51).
    pub const AUTH_PKI_DH_V12: &str = "DDS:Auth:PKI-DH:1.2";
    /// Builtin permissions access control (§10.4.2.2 Table 65).
    pub const ACCESS_PERMISSIONS_V12: &str = "DDS:Access:Permissions:1.2";
    /// Builtin AES-GCM-GMAC crypto (§10.5): the plugin class; the `class_id`
    /// of its CryptoToken is `DDS:Crypto:AES_GCM_GMAC` (Table 72).
    pub const CRYPTO_AES_GCM_GMAC_V12: &str = "DDS:Crypto:AES-GCM-GMAC:1.2";
    /// Permissions credential (§10.4.2 Tab.64).
    pub const ACCESS_PERMISSIONS_CREDENTIAL: &str = "DDS:Access:PermissionsCredential";

    /// Whether the class ids `a` and `b`, each
    /// `<PluginClassName>:<MajorVersion>.<MinorVersion>` (DDS-Security 1.2
    /// §10.3.2.1), have the same PluginClassName and MajorVersion: the
    /// comparison of an IdentityToken in `validate_remote_identity`
    /// (Table 59). A class id without the versions does not have that format
    /// and matches none; for a PermissionsToken, §10.4.2.2 reads it as 1.0
    /// ([`same_permissions_class_and_major_version`]).
    #[must_use]
    pub fn same_class_and_major_version(a: &str, b: &str) -> bool {
        fn split(id: &str) -> Option<(&str, &str)> {
            let (name, version) = id.rsplit_once(':')?;
            let major = version.split_once('.').map_or(version, |(major, _)| major);
            (!name.is_empty() && !major.is_empty()).then_some((name, major))
        }
        matches!((split(a), split(b)), (Some(x), Some(y)) if x == y)
    }

    /// [`same_class_and_major_version`] for the class ids of two
    /// PermissionsTokens: the comparison Table 70 puts first in
    /// `check_remote_participant`, `check_remote_datawriter`,
    /// `check_remote_datareader` and `check_remote_topic`. A class id
    /// without the versions is read as MajorVersion 1 and MinorVersion 0
    /// (§10.4.2.2: "If the MajorVersion and MinorVersion are missing from
    /// the class_id, it shall be interpreted as being MajorVersion 1 and
    /// MinorVersion 0"): `DDS:Access:Permissions` is
    /// `DDS:Access:Permissions:1.0`. The versions are missing when what
    /// follows the last `:` is no number. §10.3.2.1 (IdentityToken) and
    /// §10.7.2.1 have no such rule.
    #[must_use]
    pub fn same_permissions_class_and_major_version(a: &str, b: &str) -> bool {
        fn with_versions(id: &str) -> alloc::borrow::Cow<'_, str> {
            let versioned = id.rsplit_once(':').is_some_and(|(_, version)| {
                let major = version.split_once('.').map_or(version, |(major, _)| major);
                !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit())
            });
            if versioned {
                alloc::borrow::Cow::Borrowed(id)
            } else {
                alloc::borrow::Cow::Owned(alloc::format!("{id}:1.0"))
            }
        }
        same_class_and_major_version(&with_versions(a), &with_versions(b))
    }
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

/// The most octets one `DataHolder` occupies on the wire (DoS cap), the
/// bound of the `ParticipantGenericMessage` that carries it
/// (`generic_message.rs`). DDS-Security 1.2 bounds neither the `string`
/// values nor the `OctetSeq` values of a DataHolder (§7.3.2.1, §7.3.3.1,
/// §7.3.4.1): a property value is as long as the token leaves room for. A
/// separate 8 KiB cap per value refused a signed permissions document above
/// it (`c.perm` of the HandshakeRequestMessageToken, §10.3.2.5.1) and with
/// it the handshake.
const MAX_TOKEN_BYTES: usize = 256 * 1024;
/// Most properties and binary properties per DataHolder (DoS cap).
const MAX_PROPS: u32 = 256;
const MAX_BIN_PROPS: u32 = 256;

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

fn exceeds_cap() -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::BadArgument,
        "token: payload exceeds DoS cap",
    )
}

fn decode(bytes: &[u8], le: bool) -> SecurityResult<DataHolder> {
    if bytes.len() > MAX_TOKEN_BYTES {
        return Err(exceeds_cap());
    }
    decode_consumed(bytes, le).map(|(dh, _)| dh)
}

/// Like [`decode`], additionally returns the number of bytes consumed —
/// needed for the inline decoding of `sequence<DataHolder>` (spec
/// `GenericMessageData`), where the DataHolders lie one after another
/// without a length prefix and the caller must advance the cursor.
///
/// `bytes` runs to the end of the message, so the cap bounds the octets the
/// DataHolder occupies, not the octets offered: the DataHolder is read from
/// the first [`MAX_TOKEN_BYTES`] of them, and one that does not end there
/// exceeds the cap. A check of `bytes.len()` refused every DataHolder
/// followed by more than the cap of message.
fn decode_consumed(bytes: &[u8], le: bool) -> SecurityResult<(DataHolder, usize)> {
    let window = &bytes[..bytes.len().min(MAX_TOKEN_BYTES)];
    decode_window(window, le).map_err(|e| {
        if bytes.len() > MAX_TOKEN_BYTES {
            exceeds_cap()
        } else {
            e
        }
    })
}

fn decode_window(bytes: &[u8], le: bool) -> SecurityResult<(DataHolder, usize)> {
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
        binary_properties.push(BinaryProperty::new(name, value));
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
        if len == 0 {
            return Ok(String::new());
        }
        if len as usize > self.buf.len().saturating_sub(self.pos) {
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
        if len as usize > self.buf.len().saturating_sub(self.pos) {
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
    fn security_7_3_5_3_empty_data_holder_roundtrip_le() {
        let dh = DataHolder::new("DDS:Auth:PKI-DH:1.2");
        let bytes = dh.to_cdr_le();
        let back = DataHolder::from_cdr_le(&bytes).unwrap();
        assert_eq!(dh, back);
    }

    #[test]
    fn security_7_3_4_1_empty_data_holder_roundtrip_be() {
        let dh = DataHolder::new("DDS:Auth:PKI-DH:1.2");
        let bytes = dh.to_cdr_be();
        let back = DataHolder::from_cdr_be(&bytes).unwrap();
        assert_eq!(dh, back);
    }

    #[test]
    fn security_7_3_4_pki_dh_identity_token_roundtrip() {
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
    fn security_7_3_5_permissions_token_roundtrip() {
        let tok = IdentityToken::permissions_v12("DE:AD:BE:EF", "ECDSA-SHA256");
        let le = tok.to_cdr_le();
        let be = tok.to_cdr_be();
        assert_eq!(tok, DataHolder::from_cdr_le(&le).unwrap());
        assert_eq!(tok, DataHolder::from_cdr_be(&be).unwrap());
        assert_ne!(le, be, "BE/LE streams differ");
    }

    #[test]
    fn security_7_3_3_1_token_with_binary_property_roundtrip() {
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
    fn security_7_3_1_2_cdr_le_layout_class_id_only() {
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
    fn security_7_3_4_1_truncated_buffer_is_error() {
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
    fn security_7_3_4_1_missing_trailing_nul_rejected() {
        // Forge: class_id len=1 (no NUL).
        let bytes = vec![0x01, 0x00, 0x00, 0x00, b'A'];
        let err = DataHolder::from_cdr_le(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// A DataHolder written field by field after the IDL of §7.3.4.1
    /// (XCDR1 little-endian, `@final`): `string class_id`, an empty
    /// `PropertySeq`, a `BinaryPropertySeq` of one `c.perm` of `len` octets.
    fn hand_written_holder(len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        let class_id = b"DDS:Auth:PKI-DH:1.2+AuthReq\0";
        out.extend_from_slice(&(class_id.len() as u32).to_le_bytes());
        out.extend_from_slice(class_id);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // properties
        out.extend_from_slice(&1u32.to_le_bytes()); // binary_properties
        out.extend_from_slice(&7u32.to_le_bytes());
        out.extend_from_slice(b"c.perm\0");
        out.push(0); // to the 4-octet boundary of the length
        out.extend_from_slice(&(len as u32).to_le_bytes());
        out.extend((0..len).map(|i| (i % 251) as u8));
        out
    }

    /// §7.3.3.1 types a binary value `OctetSeq`, unbounded: a signed
    /// permissions document of 20 000 octets in `c.perm` is read.
    #[test]
    fn security_7_3_3_1_a_binary_value_above_8_kib_is_read() {
        let bytes = hand_written_holder(20_000);
        let holder = DataHolder::from_cdr_le(&bytes).expect("decode");
        let perm = holder.binary_property("c.perm").expect("c.perm");
        assert_eq!(perm.len(), 20_000);
        assert!(perm.iter().enumerate().all(|(i, b)| *b == (i % 251) as u8));
    }

    /// Inside a `ParticipantGenericMessage` the DataHolder is read from the
    /// rest of the message: what follows it does not count against its cap.
    #[test]
    fn security_7_3_4_1_a_data_holder_followed_by_more_message_than_its_cap_is_read() {
        let mut bytes = hand_written_holder(100);
        let own = bytes.len();
        bytes.resize(own + MAX_TOKEN_BYTES, 0xEE);
        let (holder, consumed) = DataHolder::from_cdr_le_consumed(&bytes).expect("decode");
        assert_eq!(consumed, own);
        assert_eq!(holder.binary_property("c.perm").map(<[u8]>::len), Some(100));
    }

    /// One that does not end within the cap is refused as above it.
    #[test]
    fn a_data_holder_beyond_its_cap_is_refused() {
        let bytes = hand_written_holder(MAX_TOKEN_BYTES);
        let err = DataHolder::from_cdr_le_consumed(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn dos_cap_overall_payload() {
        let big = vec![0u8; MAX_TOKEN_BYTES + 1];
        let err = DataHolder::from_cdr_le(&big).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// §10.4.2.2: "If the MajorVersion and MinorVersion are missing from
    /// the class_id, it shall be interpreted as being MajorVersion 1 and
    /// MinorVersion 0."
    /// The rule is that of the PermissionsToken; an IdentityToken without
    /// the versions (§10.3.2.1 has no such rule) still matches none.
    #[test]
    fn security_10_4_2_2_a_class_id_without_versions_is_version_1_0() {
        use class_id::{
            ACCESS_PERMISSIONS_V12, AUTH_PKI_DH_V12, same_class_and_major_version,
            same_permissions_class_and_major_version,
        };
        assert!(same_permissions_class_and_major_version(
            "DDS:Access:Permissions",
            ACCESS_PERMISSIONS_V12
        ));
        assert!(same_permissions_class_and_major_version(
            ACCESS_PERMISSIONS_V12,
            "DDS:Access:Permissions"
        ));
        assert!(!same_permissions_class_and_major_version(
            "DDS:Access:Permissions",
            "DDS:Access:Permissions:2.0"
        ));
        assert!(!same_permissions_class_and_major_version(
            "DDS:Access",
            ACCESS_PERMISSIONS_V12
        ));
        assert!(!same_class_and_major_version(
            "DDS:Auth:PKI-DH",
            AUTH_PKI_DH_V12
        ));
    }

    /// Table 70: PluginClassName and MajorVersion decide, the MinorVersion
    /// does not. Cyclone DDS and Fast DDS announce "...:1.0".
    #[test]
    fn security_10_4_3_class_ids_compare_by_plugin_class_name_and_major_version() {
        use class_id::{ACCESS_PERMISSIONS_V12, same_class_and_major_version};
        assert!(same_class_and_major_version(
            "DDS:Access:Permissions:1.0",
            ACCESS_PERMISSIONS_V12
        ));
        assert!(!same_class_and_major_version(
            "DDS:Access:Permissions:2.0",
            ACCESS_PERMISSIONS_V12
        ));
        assert!(!same_class_and_major_version(
            "DDS:Access:PSK:1.2",
            ACCESS_PERMISSIONS_V12
        ));
        assert!(!same_class_and_major_version(
            "DDS:Access:Permissions:",
            ACCESS_PERMISSIONS_V12
        ));
        assert!(!same_class_and_major_version("", ""));
    }
}
