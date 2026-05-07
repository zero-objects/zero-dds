// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PropertyList Wire-Format fuer `PID_PROPERTY_LIST` (0x0059).
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
//!   u32  n_binary         // immer 0 fuer unsere Zwecke
//!
//! Property:
//!   align 4
//!   u32  name_len         // inkl. null-terminator
//!   u8   name[name_len]
//!   align 4
//!   u32  value_len
//!   u8   value[value_len]
//! ```
//!
//! Dieses Modul ist **Wire-only** — es kennt keine Security-Semantik
//! (propagate-Flag, Plugin-Klassen, etc.). Die Policy-Schicht
//! ([`zerodds-security-runtime`]) baut auf diesem Wire-Typ auf.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::WireError;

/// DoS-Cap fuer die Anzahl Properties in einer PropertyList (amplifiziert
/// sonst via SPDP-Broadcast). 1024 passt fuer jede realistische
/// Security-Plugin-Konfiguration.
pub const MAX_PROPERTIES: usize = 1_024;

/// DoS-Cap fuer die Laenge von name+value in Bytes (verhindert einen
/// Peer, der eine einzelne 4-GiB-Property schickt).
pub const MAX_PROPERTY_STRING_LEN: usize = 64 * 1024;

/// Ein einzelner Property-Eintrag auf dem Wire. Beide Felder sind
/// UTF-8-Strings ohne inneren Null-Byte (Spec-Constraint des
/// CDR-String-Typs).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireProperty {
    /// Name (reverse-DNS Convention, z.B. `dds.sec.auth.plugin_class`).
    pub name: String,
    /// Wert (opaker UTF-8-String).
    pub value: String,
}

impl WireProperty {
    /// Konstruktor.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// PropertyList-Snapshot fuer die Wire-Ebene. Die Reihenfolge wird
/// beim Encode/Decode beibehalten — Caller, die Duplikat-Namen
/// vermeiden wollen, muessen das selbst durchsetzen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WirePropertyList {
    /// Liste der Properties in wire-Reihenfolge.
    pub entries: Vec<WireProperty>,
}

impl WirePropertyList {
    /// Leere Liste.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Liefert `true` wenn kein Property vorhanden.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Anzahl Properties.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Fuegt ein Property an. Ueberschreibt **nicht** bei Dublette —
    /// Wire-Semantik: letzter Eintrag gewinnt beim Lookup.
    pub fn push(&mut self, prop: WireProperty) {
        self.entries.push(prop);
    }

    /// Builder-Variante fuer [`Self::push`].
    #[must_use]
    pub fn with(mut self, prop: WireProperty) -> Self {
        self.push(prop);
        self
    }

    /// Liefert den Wert zum letzten Eintrag mit diesem Namen (Spec-
    /// Semantik: "last value wins").
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|p| p.name == name)
            .map(|p| p.value.as_str())
    }

    /// Encode zur Byte-Sequenz, die direkt als Value eines
    /// `PID_PROPERTY_LIST`-Parameters genutzt wird. `BinaryPropertySeq`
    /// wird immer als `count=0` angehaengt.
    ///
    /// # Errors
    /// * `ValueOutOfRange` wenn die Anzahl Properties
    ///   [`MAX_PROPERTIES`] ueberschreitet.
    /// * `ValueOutOfRange` wenn name/value ueber
    ///   [`MAX_PROPERTY_STRING_LEN`] liegen.
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
        // BinaryPropertySeq: count = 0, keine Eintraege.
        write_u32(&mut out, 0, little_endian);
        Ok(out)
    }

    /// Decode aus dem Value-Byte-Slice eines `PID_PROPERTY_LIST`-
    /// Parameters.
    ///
    /// # Errors
    /// * `UnexpectedEof` bei truncated Eingabe.
    /// * `ValueOutOfRange` bei verletzten DoS-Caps.
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
        // BinaryPropertySeq lesen und verwerfen (wir persistieren die
        // binary-Eintraege derzeit nicht — kein Security-Use-Case in
        // Stufe 2).
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
    // Null-terminator gehoert in die Laenge — ihn abschneiden.
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
        // n_props = u32::MAX — sollte vor der Allozierung abgelehnt werden.
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
        bytes.extend_from_slice(&10u32.to_le_bytes()); // value_len = 10, aber keine bytes mehr
        let err = WirePropertyList::decode(&bytes, true).unwrap_err();
        assert!(matches!(err, WireError::UnexpectedEof { .. }));
    }

    #[test]
    fn decode_accepts_nonzero_binary_sequence() {
        // Wir persistieren binary nicht, muessen sie aber konsumieren.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes()); // n_props = 0
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_binary = 1
        bytes.extend_from_slice(&4u32.to_le_bytes()); // binary name_len = 4 ("ab"+null gepadded)
        bytes.extend_from_slice(b"bin\0"); // name "bin" + null
        bytes.extend_from_slice(&3u32.to_le_bytes()); // value_len = 3
        bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0x00]); // bytes + pad
        let decoded = WirePropertyList::decode(&bytes, true).unwrap();
        assert!(decoded.is_empty(), "binary seq wird still konsumiert");
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
        // name_len sagt 4, aber letzes byte ist kein null
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes()); // n_props = 1
        bytes.extend_from_slice(&4u32.to_le_bytes()); // name_len = 4
        bytes.extend_from_slice(b"abcd"); // kein null am Ende
        bytes.extend_from_slice(&2u32.to_le_bytes()); // value_len = 2
        bytes.extend_from_slice(b"v\0\0\0"); // wuerde kompletten sein
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
        // Keine echten Value-Bytes, aber wir erwarten Cap-Check vor EOF
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
        bytes.extend_from_slice(&[0xAA, 0xBB]); // nur 2 byte da
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
