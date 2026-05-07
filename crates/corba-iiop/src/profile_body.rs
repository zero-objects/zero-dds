// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP ProfileBody — Spec §15.7.2 (alle 4 Versionen 1.0/1.1/1.2/1.3).
//!
//! ```text
//! struct Version {
//!     octet major;
//!     octet minor;
//! };
//!
//! struct ProfileBody_1_0 {
//!     Version            iiop_version;     // 1.0
//!     string             host;
//!     unsigned short     port;
//!     sequence<octet>    object_key;
//! };
//!
//! struct ProfileBody_1_1 {  // also 1.2, 1.3
//!     Version                   iiop_version;
//!     string                    host;
//!     unsigned short            port;
//!     sequence<octet>           object_key;
//!     sequence<TaggedComponent> components;
//! };
//! ```
//!
//! `TaggedComponent` ist ein einfacher `(tag, encapsulation_octets)`-
//! Container; der Inhalt ist abhaengig vom Tag (Spec §13.6.6).
//! Wir halten den Inhalt opaque — `crates/corba-ior/` decodiert die
//! Standard-32 Tags.
//!
//! Die ProfileBody wird in einem `IOR.profiles[i].profile_data`-
//! `sequence<octet>` als CDR-Encapsulation gespeichert (Spec §13.6.3
//! Tab 13-3): erst ein `octet` mit Endianness, dann der Body in
//! dieser Endianness.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

/// IIOP-Version (`major.minor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IiopVersion {
    /// Major-Version.
    pub major: u8,
    /// Minor-Version.
    pub minor: u8,
}

impl IiopVersion {
    /// IIOP 1.0.
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// IIOP 1.1.
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// IIOP 1.2.
    pub const V1_2: Self = Self { major: 1, minor: 2 };
    /// IIOP 1.3.
    pub const V1_3: Self = Self { major: 1, minor: 3 };

    /// Konstruktor.
    #[must_use]
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// `true` wenn die Version `components`-Sequenz im Body hat
    /// (Spec §15.7.2: ab IIOP 1.1).
    #[must_use]
    pub const fn has_components(self) -> bool {
        if self.major > 1 {
            true
        } else {
            self.minor >= 1
        }
    }
}

/// Tagged-Component — Spec §13.6.6.
///
/// `tag` ist ein `unsigned long` (z.B. `TAG_ORB_TYPE = 0`,
/// `TAG_CODE_SETS = 1`, `TAG_POLICIES = 2`, …).
/// `component_data` ist ein CDR-encapsulated Octet-Stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedComponent {
    /// Tag-ID.
    pub tag: u32,
    /// Encapsulation-Bytes (Length+Data oder Endianness+Body).
    pub component_data: Vec<u8>,
}

/// IIOP ProfileBody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IiopProfileBody {
    /// IIOP-Version.
    pub iiop_version: IiopVersion,
    /// Host-Name oder IP-Adresse als String (Spec §15.7.2:
    /// "string identifying the host").
    pub host: String,
    /// TCP-Port.
    pub port: u16,
    /// Object-Key (POA-encoded).
    pub object_key: Vec<u8>,
    /// `components` — leer in IIOP 1.0; ab 1.1 enthaelt es alle
    /// optionalen TaggedComponents.
    pub components: Vec<TaggedComponent>,
}

impl IiopProfileBody {
    /// Konstruktor mit minimalem Pflicht-Feld-Set.
    #[must_use]
    pub fn new(version: IiopVersion, host: String, port: u16, object_key: Vec<u8>) -> Self {
        Self {
            iiop_version: version,
            host,
            port,
            object_key,
            components: Vec::new(),
        }
    }

    /// CDR-Encode den ProfileBody (ohne Encapsulation-Wrapper —
    /// Caller wraps in `crates/corba-ior/` bei Bedarf).
    ///
    /// # Errors
    /// Buffer-Schreibfehler oder Length-Overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), CdrError> {
        w.write_u8(self.iiop_version.major)?;
        w.write_u8(self.iiop_version.minor)?;
        w.write_string(&self.host)?;
        w.write_u16(self.port)?;
        // sequence<octet> object_key.
        let n = u32::try_from(self.object_key.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n)?;
        w.write_bytes(&self.object_key)?;
        // components (ab IIOP 1.1).
        if self.iiop_version.has_components() {
            let cn = u32::try_from(self.components.len()).map_err(|_| CdrError::Overflow)?;
            w.write_u32(cn)?;
            for c in &self.components {
                w.write_u32(c.tag)?;
                let cd = u32::try_from(c.component_data.len()).map_err(|_| CdrError::Overflow)?;
                w.write_u32(cd)?;
                w.write_bytes(&c.component_data)?;
            }
        }
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, CdrError> {
        let major = r.read_u8()?;
        let minor = r.read_u8()?;
        let iiop_version = IiopVersion::new(major, minor);
        let host = r.read_string()?;
        let port = r.read_u16()?;
        let key_len = r.read_u32()? as usize;
        let object_key = r.read_bytes(key_len)?.to_vec();
        let components = if iiop_version.has_components() {
            let cn = r.read_u32()? as usize;
            let mut out = Vec::with_capacity(cn.min(64));
            for _ in 0..cn {
                let tag = r.read_u32()?;
                let cd_len = r.read_u32()? as usize;
                let component_data = r.read_bytes(cd_len)?.to_vec();
                out.push(TaggedComponent {
                    tag,
                    component_data,
                });
            }
            out
        } else {
            Vec::new()
        };
        Ok(Self {
            iiop_version,
            host,
            port,
            object_key,
            components,
        })
    }

    /// Encodiert den ProfileBody als CDR-Encapsulation (Spec §15.3.3):
    /// `octet endianness + body in dieser Endianness`. Das ist die
    /// Form, in der der ProfileBody in `IOR.profiles[i].profile_data`
    /// gespeichert wird.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        let mut out = Vec::with_capacity(64);
        // Endianness-Byte (Bit 0 = 0 BE / 1 LE).
        out.push(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        });
        let mut w = BufferWriter::new(endianness);
        self.encode(&mut w)?;
        out.extend_from_slice(w.as_bytes());
        Ok(out)
    }

    /// Decodiert eine CDR-Encapsulation (siehe `encode_encapsulation`).
    ///
    /// # Errors
    /// Buffer-Lesefehler oder Endianness-Byte ungueltig.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, CdrError> {
        if bytes.is_empty() {
            return Err(CdrError::Truncated);
        }
        let endianness = match bytes[0] {
            0 => Endianness::Big,
            1 => Endianness::Little,
            _ => return Err(CdrError::InvalidEndianness),
        };
        let mut r = BufferReader::new(&bytes[1..], endianness);
        Self::decode(&mut r)
    }
}

/// Lokaler CDR-Fehlertyp (kapselt die `zerodds-cdr`-Errors zusaetzlich um
/// Length-Overflow + Truncation-Faelle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdrError {
    /// `zerodds-cdr`-Encode-Fehler.
    Encode(zerodds_cdr::EncodeError),
    /// `zerodds-cdr`-Decode-Fehler.
    Decode(zerodds_cdr::DecodeError),
    /// Sequenz-Laenge ueberschreitet `u32::MAX`.
    Overflow,
    /// Truncated-Encapsulation (zu wenige Bytes).
    Truncated,
    /// Endianness-Octet ist weder 0 noch 1.
    InvalidEndianness,
}

impl From<zerodds_cdr::EncodeError> for CdrError {
    fn from(e: zerodds_cdr::EncodeError) -> Self {
        Self::Encode(e)
    }
}

impl From<zerodds_cdr::DecodeError> for CdrError {
    fn from(e: zerodds_cdr::DecodeError) -> Self {
        Self::Decode(e)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn iiop_version_constants_match_spec() {
        assert_eq!(IiopVersion::V1_0, IiopVersion::new(1, 0));
        assert_eq!(IiopVersion::V1_3, IiopVersion::new(1, 3));
    }

    #[test]
    fn components_only_from_iiop_1_1() {
        // Spec §15.7.2: components-Sequenz erst ab IIOP 1.1.
        assert!(!IiopVersion::V1_0.has_components());
        assert!(IiopVersion::V1_1.has_components());
        assert!(IiopVersion::V1_2.has_components());
        assert!(IiopVersion::V1_3.has_components());
    }

    fn sample_v1_0() -> IiopProfileBody {
        IiopProfileBody::new(
            IiopVersion::V1_0,
            "example.com".into(),
            7777,
            alloc::vec![0xab, 0xcd],
        )
    }

    fn sample_v1_2_with_components() -> IiopProfileBody {
        IiopProfileBody {
            iiop_version: IiopVersion::V1_2,
            host: "10.0.0.1".into(),
            port: 9999,
            object_key: alloc::vec![1, 2, 3, 4],
            components: alloc::vec![
                TaggedComponent {
                    tag: 0, // TAG_ORB_TYPE
                    component_data: alloc::vec![0xde, 0xad],
                },
                TaggedComponent {
                    tag: 1, // TAG_CODE_SETS
                    component_data: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
                },
            ],
        }
    }

    #[test]
    fn round_trip_v1_0_be() {
        let p = sample_v1_0();
        let bytes = p.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, p);
        // Encapsulation startet mit Endianness-Byte 0 (BE).
        assert_eq!(bytes[0], 0);
    }

    #[test]
    fn round_trip_v1_0_le() {
        let p = sample_v1_0();
        let bytes = p.encode_encapsulation(Endianness::Little).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, p);
        assert_eq!(bytes[0], 1);
    }

    #[test]
    fn round_trip_v1_2_with_components() {
        let p = sample_v1_2_with_components();
        let bytes = p.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, p);
        assert_eq!(decoded.components.len(), 2);
    }

    #[test]
    fn v1_0_does_not_emit_components_field() {
        // Spec §15.7.2: IIOP 1.0 hat keine components-Sequenz im Wire.
        // Ein voller Round-Trip muss auch ohne components funktionieren.
        let mut p = sample_v1_0();
        // components werden bei Encode/Decode in 1.0 ignoriert.
        p.components = alloc::vec![TaggedComponent {
            tag: 99,
            component_data: alloc::vec![1, 2],
        }];
        let bytes = p.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = IiopProfileBody::decode_encapsulation(&bytes).unwrap();
        assert!(decoded.components.is_empty());
    }

    #[test]
    fn invalid_endianness_byte_is_diagnostic() {
        let bytes = alloc::vec![0xff, 0, 0];
        let err = IiopProfileBody::decode_encapsulation(&bytes).unwrap_err();
        assert!(matches!(err, CdrError::InvalidEndianness));
    }

    #[test]
    fn empty_encapsulation_is_truncated() {
        let err = IiopProfileBody::decode_encapsulation(&[]).unwrap_err();
        assert!(matches!(err, CdrError::Truncated));
    }
}
