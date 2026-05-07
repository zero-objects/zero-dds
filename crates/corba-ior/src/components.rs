// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TaggedComponent + strukturierte Decoder fuer wichtige Tags.
//!
//! Spec §13.6.7.3 + §15.6.6:
//! ```text
//! struct TaggedComponent {
//!     ComponentId      tag;
//!     sequence<octet>  component_data;
//! };
//! ```
//!
//! `component_data` ist eine CDR-Encapsulation (Endianness-Octet +
//! Body), Inhalt ist Tag-spezifisch.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_csiv2::CompoundSecMechList;
use zerodds_corba_iiop::profile_body::CdrError;

use crate::component_tags::ComponentId;

/// `TaggedComponent` — Tag + Encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedComponent {
    /// Component-Tag.
    pub tag: ComponentId,
    /// Encapsulation-Bytes (Endianness-Octet + tag-spezifischer Body).
    pub component_data: Vec<u8>,
}

impl TaggedComponent {
    /// CDR-Encode.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), CdrError> {
        w.write_u32(self.tag.as_u32())?;
        let n = u32::try_from(self.component_data.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n)?;
        w.write_bytes(&self.component_data)?;
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, CdrError> {
        let tag = ComponentId::from_u32(r.read_u32()?);
        let n = r.read_u32()? as usize;
        let bytes = r.read_bytes(n)?;
        Ok(Self {
            tag,
            component_data: bytes.to_vec(),
        })
    }

    /// Versucht, das Component als bekannten strukturierten Type
    /// zu decoden.
    ///
    /// # Errors
    /// CDR-Fehler im Encapsulation-Body.
    pub fn structured(&self) -> Result<StructuredComponent, CdrError> {
        StructuredComponent::decode(self.tag, &self.component_data)
    }
}

// -------------------------------------------------------------------
// Strukturierte Component-Bodies fuer die wichtigsten Tags.
// -------------------------------------------------------------------

/// `TAG_ORB_TYPE` (Spec §13.6.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbType(pub u32);

/// Code-Set-Component (Spec §13.10.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSetComponent {
    /// Native code set (z.B. `0x00010001` = ISO-8859-1).
    pub native_code_set: u32,
    /// Conversion-Code-Sets — wir halten den Liste-Header
    /// (count); voller Inhalt ist bei Caller. CodeSetComponentInfo
    /// modelliert das voll.
    pub conversion_code_sets: Vec<u32>,
}

/// `CodeSetComponentInfo` (Spec §13.10.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSetComponentInfo {
    /// Char-Codeset.
    pub for_char_data: CodeSetComponent,
    /// Wide-Char-Codeset.
    pub for_wchar_data: CodeSetComponent,
}

/// `TAG_ALTERNATE_IIOP_ADDRESS` (Spec §15.7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateIiopAddress {
    /// Host-Name.
    pub host: String,
    /// TCP-Port.
    pub port: u16,
}

/// `TAG_SSL_SEC_TRANS` (Spec OMG `Security/SSLIOP`-Modul).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ssl {
    /// `target_supports` — Bitmask der unterstuetzten
    /// AssociationOptions.
    pub target_supports: u16,
    /// `target_requires` — Bitmask der erzwungenen
    /// AssociationOptions.
    pub target_requires: u16,
    /// SSL-Port.
    pub port: u16,
}

/// `TAG_TLS_SEC_TRANS` (Spec OMG `Security/TLSIOP`-Modul) — Wire-
/// Layout identisch zu SSL_SEC_TRANS plus TransportAddressList.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsSecTrans {
    /// `target_supports`.
    pub target_supports: u16,
    /// `target_requires`.
    pub target_requires: u16,
    /// `addresses` — Liste alternativer TLS-Endpoints.
    pub addresses: Vec<AlternateIiopAddress>,
}

/// `TAG_RMI_CUSTOM_MAX_STREAM_FORMAT` (Spec §13.6.7.3 + JavaToIDL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFormatVersion(pub u8);

/// Strukturierte Form fuer die wichtigsten Components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredComponent {
    /// `TAG_ORB_TYPE`.
    OrbType(OrbType),
    /// `TAG_CODE_SETS`.
    CodeSets(CodeSetComponentInfo),
    /// `TAG_ALTERNATE_IIOP_ADDRESS`.
    AlternateIiopAddress(AlternateIiopAddress),
    /// `TAG_SSL_SEC_TRANS`.
    Ssl(Ssl),
    /// `TAG_TLS_SEC_TRANS`.
    TlsSecTrans(TlsSecTrans),
    /// `TAG_CSI_SEC_MECH_LIST = 33` (Spec CORBA 3.3 Part 2 §10.5).
    CsiSecMechList(CompoundSecMechList),
    /// `TAG_RMI_CUSTOM_MAX_STREAM_FORMAT`.
    StreamFormatVersion(StreamFormatVersion),
    /// `TAG_JAVA_CODEBASE` (Spec §13.6.6.7) — Liste codebase-URLs.
    JavaCodebase(String),
    /// Andere Tags — opaque encapsulation.
    Opaque {
        /// Tag.
        tag: ComponentId,
        /// Body.
        bytes: Vec<u8>,
    },
}

impl StructuredComponent {
    /// Decodiert eine Component-Encapsulation in eine strukturierte
    /// Form, soweit der Tag bekannt ist.
    ///
    /// # Errors
    /// CDR-Decode-Fehler im Body.
    pub fn decode(tag: ComponentId, encap: &[u8]) -> Result<Self, CdrError> {
        let endianness = read_endianness(encap)?;
        let body = &encap[1..];
        match tag {
            ComponentId::OrbType => {
                let mut r = BufferReader::new(body, endianness);
                Ok(Self::OrbType(OrbType(r.read_u32()?)))
            }
            ComponentId::CodeSets => {
                let mut r = BufferReader::new(body, endianness);
                let for_char = decode_code_set_component(&mut r)?;
                let for_wchar = decode_code_set_component(&mut r)?;
                Ok(Self::CodeSets(CodeSetComponentInfo {
                    for_char_data: for_char,
                    for_wchar_data: for_wchar,
                }))
            }
            ComponentId::AlternateIiopAddress => {
                let mut r = BufferReader::new(body, endianness);
                let host = r.read_string()?;
                let port = r.read_u16()?;
                Ok(Self::AlternateIiopAddress(AlternateIiopAddress {
                    host,
                    port,
                }))
            }
            ComponentId::SslSecTrans => {
                let mut r = BufferReader::new(body, endianness);
                Ok(Self::Ssl(Ssl {
                    target_supports: r.read_u16()?,
                    target_requires: r.read_u16()?,
                    port: r.read_u16()?,
                }))
            }
            ComponentId::TlsSecTrans => {
                let mut r = BufferReader::new(body, endianness);
                let target_supports = r.read_u16()?;
                let target_requires = r.read_u16()?;
                let n = r.read_u32()? as usize;
                let mut addresses = Vec::with_capacity(n.min(32));
                for _ in 0..n {
                    let host = r.read_string()?;
                    let port = r.read_u16()?;
                    addresses.push(AlternateIiopAddress { host, port });
                }
                Ok(Self::TlsSecTrans(TlsSecTrans {
                    target_supports,
                    target_requires,
                    addresses,
                }))
            }
            ComponentId::CsiSecMechList => {
                let mut r = BufferReader::new(body, endianness);
                Ok(Self::CsiSecMechList(CompoundSecMechList::decode(&mut r)?))
            }
            ComponentId::RmiCustomMaxStreamFormat => {
                let mut r = BufferReader::new(body, endianness);
                Ok(Self::StreamFormatVersion(StreamFormatVersion(r.read_u8()?)))
            }
            ComponentId::JavaCodebase => {
                let mut r = BufferReader::new(body, endianness);
                Ok(Self::JavaCodebase(r.read_string()?))
            }
            other => Ok(Self::Opaque {
                tag: other,
                bytes: encap.to_vec(),
            }),
        }
    }

    /// Encodiert eine strukturierte Component in eine Encapsulation
    /// (Endianness-Octet + Body).
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        let mut out = Vec::with_capacity(64);
        out.push(endianness_to_byte(endianness));
        let mut w = BufferWriter::new(endianness);
        match self {
            Self::OrbType(OrbType(v)) => w.write_u32(*v)?,
            Self::CodeSets(info) => {
                encode_code_set_component(&mut w, &info.for_char_data)?;
                encode_code_set_component(&mut w, &info.for_wchar_data)?;
            }
            Self::AlternateIiopAddress(a) => {
                w.write_string(&a.host)?;
                w.write_u16(a.port)?;
            }
            Self::Ssl(s) => {
                w.write_u16(s.target_supports)?;
                w.write_u16(s.target_requires)?;
                w.write_u16(s.port)?;
            }
            Self::TlsSecTrans(t) => {
                w.write_u16(t.target_supports)?;
                w.write_u16(t.target_requires)?;
                let n = u32::try_from(t.addresses.len()).map_err(|_| CdrError::Overflow)?;
                w.write_u32(n)?;
                for a in &t.addresses {
                    w.write_string(&a.host)?;
                    w.write_u16(a.port)?;
                }
            }
            Self::CsiSecMechList(list) => list.encode(&mut w)?,
            Self::StreamFormatVersion(StreamFormatVersion(v)) => w.write_u8(*v)?,
            Self::JavaCodebase(s) => w.write_string(s)?,
            Self::Opaque { bytes, .. } => {
                // Body bereits in `bytes` enthaelt schon die Endianness-
                // Octet — wir geben es einfach 1:1 zurueck.
                return Ok(bytes.clone());
            }
        }
        out.extend_from_slice(w.as_bytes());
        Ok(out)
    }
}

fn read_endianness(encap: &[u8]) -> Result<Endianness, CdrError> {
    if encap.is_empty() {
        return Err(CdrError::Truncated);
    }
    match encap[0] {
        0 => Ok(Endianness::Big),
        1 => Ok(Endianness::Little),
        _ => Err(CdrError::InvalidEndianness),
    }
}

const fn endianness_to_byte(e: Endianness) -> u8 {
    match e {
        Endianness::Big => 0,
        Endianness::Little => 1,
    }
}

fn decode_code_set_component(r: &mut BufferReader<'_>) -> Result<CodeSetComponent, CdrError> {
    let native_code_set = r.read_u32()?;
    let n = r.read_u32()? as usize;
    let mut conversion = Vec::with_capacity(n.min(16));
    for _ in 0..n {
        conversion.push(r.read_u32()?);
    }
    Ok(CodeSetComponent {
        native_code_set,
        conversion_code_sets: conversion,
    })
}

fn encode_code_set_component(w: &mut BufferWriter, c: &CodeSetComponent) -> Result<(), CdrError> {
    w.write_u32(c.native_code_set)?;
    let n = u32::try_from(c.conversion_code_sets.len()).map_err(|_| CdrError::Overflow)?;
    w.write_u32(n)?;
    for cs in &c.conversion_code_sets {
        w.write_u32(*cs)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn orb_type_round_trip() {
        let s = StructuredComponent::OrbType(OrbType(0x4F4D_4732)); // "OMG2" Vendor-Style
        let bytes = s.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = StructuredComponent::decode(ComponentId::OrbType, &bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn code_sets_round_trip_le() {
        let info = CodeSetComponentInfo {
            for_char_data: CodeSetComponent {
                native_code_set: 0x0001_0001,
                conversion_code_sets: alloc::vec![0x0001_0109],
            },
            for_wchar_data: CodeSetComponent {
                native_code_set: 0x0001_0109,
                conversion_code_sets: alloc::vec![],
            },
        };
        let s = StructuredComponent::CodeSets(info.clone());
        let bytes = s.encode_encapsulation(Endianness::Little).unwrap();
        let decoded = StructuredComponent::decode(ComponentId::CodeSets, &bytes).unwrap();
        match decoded {
            StructuredComponent::CodeSets(d) => assert_eq!(d, info),
            other => panic!("expected CodeSets, got {other:?}"),
        }
    }

    #[test]
    fn alternate_iiop_address_round_trip() {
        let s = StructuredComponent::AlternateIiopAddress(AlternateIiopAddress {
            host: "alt.host".into(),
            port: 1234,
        });
        let bytes = s.encode_encapsulation(Endianness::Big).unwrap();
        let decoded =
            StructuredComponent::decode(ComponentId::AlternateIiopAddress, &bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn ssl_sec_trans_round_trip() {
        let s = StructuredComponent::Ssl(Ssl {
            target_supports: 0x0040,
            target_requires: 0x0020,
            port: 4242,
        });
        let bytes = s.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = StructuredComponent::decode(ComponentId::SslSecTrans, &bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn tls_sec_trans_with_addresses_round_trip() {
        let s = StructuredComponent::TlsSecTrans(TlsSecTrans {
            target_supports: 0x0040,
            target_requires: 0x0040,
            addresses: alloc::vec![
                AlternateIiopAddress {
                    host: "tls-a.lab".into(),
                    port: 443,
                },
                AlternateIiopAddress {
                    host: "tls-b.lab".into(),
                    port: 8443,
                },
            ],
        });
        let bytes = s.encode_encapsulation(Endianness::Little).unwrap();
        let decoded = StructuredComponent::decode(ComponentId::TlsSecTrans, &bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn csi_sec_mech_list_round_trip() {
        use zerodds_corba_csiv2::{
            AsContextSec, AssociationOptions, CompoundSecMech, CompoundSecMechList, SasContextSec,
        };
        let list = CompoundSecMechList {
            stateful: true,
            mechanism_list: alloc::vec![CompoundSecMech {
                target_requires: AssociationOptions(
                    AssociationOptions::INTEGRITY | AssociationOptions::CONFIDENTIALITY,
                ),
                transport_mech_tag: 36, // TAG_TLS_SEC_TRANS
                transport_mech_data: alloc::vec![0x01, 0x02, 0x03],
                as_context: AsContextSec {
                    target_supports: AssociationOptions(0x0040),
                    target_requires: AssociationOptions(0x0040),
                    client_authentication_mech: alloc::vec![0xAA, 0xBB],
                    target_name: alloc::vec![0xCC],
                },
                sas_context: SasContextSec {
                    target_supports: AssociationOptions(0x0080),
                    target_requires: AssociationOptions(0x0080),
                    privilege_authorities: alloc::vec![alloc::vec![0xDE, 0xAD]],
                    supported_naming_mechanisms: alloc::vec![alloc::vec![0xBE, 0xEF]],
                    supported_identity_types: 0x0001_0203,
                },
            }],
        };
        let s = StructuredComponent::CsiSecMechList(list.clone());
        let bytes = s.encode_encapsulation(Endianness::Little).unwrap();
        let decoded = StructuredComponent::decode(ComponentId::CsiSecMechList, &bytes).unwrap();
        match decoded {
            StructuredComponent::CsiSecMechList(d) => assert_eq!(d, list),
            other => panic!("expected CsiSecMechList, got {other:?}"),
        }
    }

    #[test]
    fn stream_format_version_round_trip() {
        let s = StructuredComponent::StreamFormatVersion(StreamFormatVersion(2));
        let bytes = s.encode_encapsulation(Endianness::Big).unwrap();
        let decoded =
            StructuredComponent::decode(ComponentId::RmiCustomMaxStreamFormat, &bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn java_codebase_round_trip() {
        let s = StructuredComponent::JavaCodebase("http://server/codebase.jar".into());
        let bytes = s.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = StructuredComponent::decode(ComponentId::JavaCodebase, &bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn opaque_unknown_tag_pass_through() {
        let raw = alloc::vec![1, 0xff, 0xee, 0xdd];
        let s = StructuredComponent::decode(ComponentId::Other(9999), &raw).unwrap();
        match s {
            StructuredComponent::Opaque { tag, bytes } => {
                assert_eq!(tag, ComponentId::Other(9999));
                assert_eq!(bytes, raw);
            }
            other => panic!("expected Opaque, got {other:?}"),
        }
    }

    #[test]
    fn invalid_endianness_byte_is_diagnostic() {
        let bytes = alloc::vec![0xff, 0, 0, 0, 1];
        let err = StructuredComponent::decode(ComponentId::OrbType, &bytes).unwrap_err();
        assert!(matches!(err, CdrError::InvalidEndianness));
    }

    #[test]
    fn tagged_component_round_trip() {
        let s = StructuredComponent::OrbType(OrbType(42));
        let bytes = s.encode_encapsulation(Endianness::Big).unwrap();
        let tc = TaggedComponent {
            tag: ComponentId::OrbType,
            component_data: bytes,
        };
        let mut w = BufferWriter::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let buf = w.into_bytes();
        let mut r = BufferReader::new(&buf, Endianness::Big);
        let decoded = TaggedComponent::decode(&mut r).unwrap();
        assert_eq!(decoded, tc);
    }
}
