// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PropertyList wire format for `PID_PROPERTY_LIST` (0x0059).
//!
//! Spec OMG DDS-Security 1.1 §7.2.1 + DDSI-RTPS 2.5 §9.6.4:
//! ```text
//! PropertyQosPolicy {
//!     sequence<Property_t>       value;          // name/value
//!     sequence<BinaryProperty_t> binary_value;   // name/bytes
//! };
//! Property_t { string name; string value; };
//! ```
//!
//! CDR-Encoding (XCDR1 Classic-CDR, DDSI-RTPS 2.5 §10.2):
//! ```text
//!   u32  n_props
//!   [ Property ] * n_props
//!   u32  n_binary         // always 0 for our purposes
//!
//! Property:
//!   align 4
//!   u32  name_len         // incl. null terminator
//!   u8   name[name_len]
//!   align 4
//!   u32  value_len
//!   u8   value[value_len]
//! ```
//!
//! This module is **wire-only** — it knows no security semantics
//! (propagate flag, plugin classes, etc.). The policy layer
//! ([`zerodds-security-runtime`]) builds on this wire type.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::WireError;

/// DoS cap for the number of properties in a PropertyList (otherwise
/// amplified via SPDP broadcast). 1024 fits any realistic
/// security-plugin configuration.
pub const MAX_PROPERTIES: usize = 1_024;

/// DoS cap for the length of name+value in bytes (prevents a
/// peer from sending a single 4-GiB property).
pub const MAX_PROPERTY_STRING_LEN: usize = 64 * 1024;

/// A single property entry on the wire. Both fields are
/// UTF-8 strings without an inner null byte (spec constraint of the
/// CDR string type).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireProperty {
    /// Name (reverse-DNS convention, e.g. `dds.sec.auth.plugin_class`).
    pub name: String,
    /// Value (opaque UTF-8 string).
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

/// PropertyList snapshot for the wire level. The order is
/// preserved on encode/decode — callers that want to avoid duplicate
/// names must enforce that themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WirePropertyList {
    /// List of properties in wire order.
    pub entries: Vec<WireProperty>,
}

impl WirePropertyList {
    /// Empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if no property is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of properties.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Appends a property. Does **not** overwrite on duplicate —
    /// wire semantics: the last entry wins on lookup.
    pub fn push(&mut self, prop: WireProperty) {
        self.entries.push(prop);
    }

    /// Builder variant for [`Self::push`].
    #[must_use]
    pub fn with(mut self, prop: WireProperty) -> Self {
        self.push(prop);
        self
    }

    /// Returns the value of the last entry with this name (spec
    /// semantics: "last value wins").
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|p| p.name == name)
            .map(|p| p.value.as_str())
    }

    /// Encodes to the byte sequence used directly as the value of a
    /// `PID_PROPERTY_LIST` parameter. `BinaryPropertySeq`
    /// is always appended as `count=0`.
    ///
    /// # Errors
    /// * `ValueOutOfRange` if the number of properties
    ///   exceeds [`MAX_PROPERTIES`].
    /// * `ValueOutOfRange` if name/value exceed
    ///   [`MAX_PROPERTY_STRING_LEN`].
    pub fn encode(&self, little_endian: bool) -> Result<Vec<u8>, WireError> {
        if self.entries.len() > MAX_PROPERTIES {
            return Err(WireError::ValueOutOfRange {
                message: "PropertyList: exceeds MAX_PROPERTIES",
            });
        }
        let mut out = Vec::new();
        let n = u32::try_from(self.entries.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "PropertyList: entry count exceeds u32",
        })?;
        write_u32(&mut out, n, little_endian);
        for p in &self.entries {
            check_len(&p.name)?;
            check_len(&p.value)?;
            write_cdr_string(&mut out, &p.name, little_endian)?;
            align4(&mut out);
            write_cdr_string(&mut out, &p.value, little_endian)?;
            align4(&mut out);
        }
        // BinaryPropertySeq: count = 0, no entries.
        write_u32(&mut out, 0, little_endian);
        Ok(out)
    }

    /// Decodes from the value byte slice of a `PID_PROPERTY_LIST`
    /// parameter.
    ///
    /// # Errors
    /// * `UnexpectedEof` on truncated input.
    /// * `ValueOutOfRange` on violated DoS caps.
    pub fn decode(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        let mut pos = 0usize;
        let n_props = read_u32(bytes, &mut pos, little_endian)? as usize;
        if n_props > MAX_PROPERTIES {
            return Err(WireError::ValueOutOfRange {
                message: "PropertyList: count exceeds MAX_PROPERTIES",
            });
        }
        let mut entries = Vec::with_capacity(n_props);
        for _ in 0..n_props {
            let name = read_cdr_string(bytes, &mut pos, little_endian)?;
            align4_read(&mut pos);
            let value = read_cdr_string(bytes, &mut pos, little_endian)?;
            align4_read(&mut pos);
            entries.push(WireProperty { name, value });
        }
        // Read and discard the BinaryPropertySeq (we currently do not
        // persist the binary entries — no security use case in
        // stage 2).
        let n_binary = read_u32(bytes, &mut pos, little_endian)? as usize;
        if n_binary > MAX_PROPERTIES {
            return Err(WireError::ValueOutOfRange {
                message: "PropertyList: binary count exceeds MAX_PROPERTIES",
            });
        }
        for _ in 0..n_binary {
            // Binary: string name + sequence<u8> value.
            let _ = read_cdr_string(bytes, &mut pos, little_endian)?;
            align4_read(&mut pos);
            let vlen = read_u32(bytes, &mut pos, little_endian)? as usize;
            if vlen > MAX_PROPERTY_STRING_LEN {
                return Err(WireError::ValueOutOfRange {
                    message: "PropertyList: binary value exceeds cap",
                });
            }
            if bytes.len() < pos.saturating_add(vlen) {
                return Err(WireError::UnexpectedEof {
                    needed: vlen,
                    offset: pos,
                });
            }
            pos += vlen;
            align4_read(&mut pos);
        }
        Ok(Self { entries })
    }
}

// ============================================================================
// internals
// ============================================================================

fn check_len(s: &str) -> Result<(), WireError> {
    if s.len() > MAX_PROPERTY_STRING_LEN {
        return Err(WireError::ValueOutOfRange {
            message: "PropertyList: string exceeds MAX_PROPERTY_STRING_LEN",
        });
    }
    if s.as_bytes().contains(&0) {
        return Err(WireError::ValueOutOfRange {
            message: "PropertyList: string contains inner null byte",
        });
    }
    Ok(())
}

fn align4(out: &mut Vec<u8>) {
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

fn align4_read(pos: &mut usize) {
    *pos = pos.wrapping_add((4 - (*pos % 4)) % 4);
}

fn write_u32(out: &mut Vec<u8>, v: u32, le: bool) {
    let bytes = if le { v.to_le_bytes() } else { v.to_be_bytes() };
    out.extend_from_slice(&bytes);
}

fn read_u32(bytes: &[u8], pos: &mut usize, le: bool) -> Result<u32, WireError> {
    if bytes.len() < pos.saturating_add(4) {
        return Err(WireError::UnexpectedEof {
            needed: 4,
            offset: *pos,
        });
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&bytes[*pos..*pos + 4]);
    *pos += 4;
    Ok(if le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    })
}

fn write_cdr_string(out: &mut Vec<u8>, s: &str, le: bool) -> Result<(), WireError> {
    let bytes = s.as_bytes();
    let len =
        u32::try_from(bytes.len().saturating_add(1)).map_err(|_| WireError::ValueOutOfRange {
            message: "CDR string length exceeds u32::MAX",
        })?;
    write_u32(out, len, le);
    out.extend_from_slice(bytes);
    out.push(0);
    Ok(())
}

fn read_cdr_string(bytes: &[u8], pos: &mut usize, le: bool) -> Result<String, WireError> {
    let len = read_u32(bytes, pos, le)? as usize;
    if len == 0 {
        return Err(WireError::ValueOutOfRange {
            message: "CDR string length 0 (missing null terminator)",
        });
    }
    if len > MAX_PROPERTY_STRING_LEN {
        return Err(WireError::ValueOutOfRange {
            message: "CDR string exceeds MAX_PROPERTY_STRING_LEN",
        });
    }
    if bytes.len() < pos.saturating_add(len) {
        return Err(WireError::UnexpectedEof {
            needed: len,
            offset: *pos,
        });
    }
    // The null terminator belongs to the length — strip it.
    let text_bytes = &bytes[*pos..*pos + len - 1];
    if bytes[*pos + len - 1] != 0 {
        return Err(WireError::ValueOutOfRange {
            message: "CDR string missing null terminator",
        });
    }
    let s = core::str::from_utf8(text_bytes)
        .map_err(|_| WireError::ValueOutOfRange {
            message: "CDR string is not valid UTF-8",
        })?
        .to_string();
    *pos += len;
    Ok(s)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn empty_list_roundtrip_le() {
        let list = WirePropertyList::new();
        let bytes = list.encode(true).unwrap();
        // 2 * u32(0) = 8 bytes (n_props=0, n_binary=0)
        assert_eq!(bytes, [0u8; 8]);
        let decoded = WirePropertyList::decode(&bytes, true).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn single_property_roundtrip_le() {
        let list = WirePropertyList::new().with(WireProperty::new(
            "dds.sec.auth.plugin_class",
            "DDS:Auth:PKI-DH:1.2",
        ));
        let bytes = list.encode(true).unwrap();
        let decoded = WirePropertyList::decode(&bytes, true).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn multi_property_roundtrip_le() {
        let list = WirePropertyList::new()
            .with(WireProperty::new(
                "dds.sec.auth.plugin_class",
                "DDS:Auth:PKI-DH:1.2",
            ))
            .with(WireProperty::new(
                "dds.sec.crypto.plugin_class",
                "DDS:Crypto:AES-GCM-GMAC:1.2",
            ))
            .with(WireProperty::new(
                "zerodds.sec.supported_suites",
                "AES_128_GCM,AES_256_GCM,HMAC_SHA256",
            ))
            .with(WireProperty::new(
                "zerodds.sec.offered_protection",
                "ENCRYPT",
            ));
        let bytes = list.encode(true).unwrap();
        let decoded = WirePropertyList::decode(&bytes, true).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn multi_property_roundtrip_be() {
        let list = WirePropertyList::new()
            .with(WireProperty::new("a", "b"))
            .with(WireProperty::new("cc", "dd"));
        let bytes = list.encode(false).unwrap();
        let decoded = WirePropertyList::decode(&bytes, false).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn encode_pads_strings_to_4_between_name_and_value() {
        let list = WirePropertyList::new().with(WireProperty::new("ab", "cd"));
        let bytes = list.encode(true).unwrap();
        // Layout LE:
        //   04 00 00 00           n_props = 1
        //   03 00 00 00           name_len = 3 ("ab"+null)
        //   'a' 'b' 0 <pad-1>     → pad to 4
        //   03 00 00 00           value_len = 3
        //   'c' 'd' 0 <pad-1>
        //   00 00 00 00           n_binary = 0
        assert_eq!(bytes.len(), 4 + 4 + 4 + 4 + 4 + 4);
    }

    #[test]
    fn get_returns_last_value_for_duplicate_names() {
        let list = WirePropertyList::new()
            .with(WireProperty::new("dup", "first"))
            .with(WireProperty::new("dup", "second"));
        assert_eq!(list.get("dup"), Some("second"));
    }

    #[test]
    fn get_none_for_unknown_name() {
        let list = WirePropertyList::new().with(WireProperty::new("a", "b"));
        assert!(list.get("x").is_none());
    }

    #[test]
    fn decode_rejects_truncated_count() {
        let err = WirePropertyList::decode(&[0, 0], true).unwrap_err();
        assert!(matches!(err, WireError::UnexpectedEof { .. }));
    }

    #[test]
    fn decode_rejects_count_above_cap() {
        // n_props = u32::MAX — should be rejected before allocation.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::ValueOutOfRange { .. }));
    }

    #[test]
    fn decode_rejects_truncated_value_string() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_props = 1
        bytes.extend_from_slice(&3u32.to_le_bytes()); // name_len = 3
        bytes.extend_from_slice(b"ab\0\0"); // name + pad
        bytes.extend_from_slice(&10u32.to_le_bytes()); // value_len = 10, but no bytes left
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::UnexpectedEof { .. }));
    }

    #[test]
    fn decode_accepts_nonzero_binary_sequence() {
        // We do not persist binary, but must consume it.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes()); // n_props = 0
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_binary = 1
        bytes.extend_from_slice(&4u32.to_le_bytes()); // binary name_len = 4 ("ab"+null padded)
        bytes.extend_from_slice(b"bin\0"); // name "bin" + null
        bytes.extend_from_slice(&3u32.to_le_bytes()); // value_len = 3
        bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0x00]); // bytes + pad
        let decoded = WirePropertyList::decode(&bytes, true).unwrap();
        assert!(decoded.is_empty(), "binary seq is silently consumed");
    }

    #[test]
    fn encode_rejects_inner_null_byte_in_name() {
        let list = WirePropertyList::new().with(WireProperty::new("a\0b", "v"));
        assert!(list.encode(true).is_err());
    }

    #[test]
    fn encode_rejects_inner_null_byte_in_value() {
        let list = WirePropertyList::new().with(WireProperty::new("a", "v\0v"));
        assert!(list.encode(true).is_err());
    }

    #[test]
    fn encode_enforces_max_properties() {
        let mut list = WirePropertyList::new();
        for _ in 0..(MAX_PROPERTIES + 1) {
            list.push(WireProperty::new("k", "v"));
        }
        assert!(list.encode(true).is_err());
    }

    #[test]
    fn decode_rejects_missing_null_terminator() {
        // name_len says 4, but the last byte is not null
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_props = 1
        bytes.extend_from_slice(&4u32.to_le_bytes()); // name_len = 4
        bytes.extend_from_slice(b"abcd"); // no null at the end
        bytes.extend_from_slice(&2u32.to_le_bytes()); // value_len = 2
        bytes.extend_from_slice(b"v\0\0\0"); // would be complete
        bytes.extend_from_slice(&0u32.to_le_bytes()); // n_binary = 0
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::ValueOutOfRange { .. }));
    }

    #[test]
    fn decode_rejects_non_utf8_name() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_props = 1
        bytes.extend_from_slice(&3u32.to_le_bytes()); // name_len = 3
        bytes.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x00]); // invalid UTF-8 + null + pad
        bytes.extend_from_slice(&2u32.to_le_bytes()); // value_len = 2
        bytes.extend_from_slice(b"v\0\0\0");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::ValueOutOfRange { .. }));
    }

    #[test]
    fn len_and_is_empty_consistency() {
        let mut list = WirePropertyList::new();
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
        list.push(WireProperty::new("a", "b"));
        assert_eq!(list.len(), 1);
        assert!(!list.is_empty());
    }

    #[test]
    fn encode_rejects_name_above_max_string_len() {
        let big = "x".repeat(MAX_PROPERTY_STRING_LEN + 1);
        let list = WirePropertyList::new().with(WireProperty::new(big, "v"));
        assert!(list.encode(true).is_err());
    }

    #[test]
    fn decode_rejects_binary_count_above_cap() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes()); // n_props = 0
        bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // n_binary = u32::MAX
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::ValueOutOfRange { .. }));
    }

    #[test]
    fn decode_rejects_binary_value_above_cap() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes()); // n_props = 0
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_binary = 1
        bytes.extend_from_slice(&4u32.to_le_bytes()); // binary name_len = 4
        bytes.extend_from_slice(b"bin\0"); // name + null
        bytes.extend_from_slice(&(MAX_PROPERTY_STRING_LEN as u32 + 1).to_le_bytes());
        // No real value bytes, but we expect the cap check before EOF
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::ValueOutOfRange { .. }));
    }

    #[test]
    fn decode_rejects_truncated_binary_value() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes()); // n_props = 0
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_binary = 1
        bytes.extend_from_slice(&4u32.to_le_bytes()); // binary name_len = 4
        bytes.extend_from_slice(b"bin\0");
        bytes.extend_from_slice(&8u32.to_le_bytes()); // value_len = 8
        bytes.extend_from_slice(&[0xAA, 0xBB]); // only 2 bytes present
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::UnexpectedEof { .. }));
    }

    #[test]
    fn csv_suites_value_roundtrips() {
        let list = WirePropertyList::new().with(WireProperty::new(
            "zerodds.sec.supported_suites",
            "AES_128_GCM,AES_256_GCM,HMAC_SHA256",
        ));
        let bytes = list.encode(true).unwrap();
        let decoded = WirePropertyList::decode(&bytes, true).unwrap();
        assert_eq!(
            decoded.get("zerodds.sec.supported_suites"),
            Some("AES_128_GCM,AES_256_GCM,HMAC_SHA256")
        );
    }
}
