// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TaggedComponent + structured decoders for important tags.
//!
//! Spec §13.6.7.3 + §15.6.6:
//! ```text
//! struct TaggedComponent {
//!     ComponentId      tag;
//!     sequence<octet>  component_data;
//! };
//! ```
//!
//! `component_data` is a CDR encapsulation (endianness octet + body),
//! its content is tag-specific.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_csiv2::CompoundSecMechList;
use zerodds_corba_iiop::profile_body::CdrError;
use zerodds_wire::{Endianness, Reader, Writer};

use crate::component_tags::ComponentId;

/// `TaggedComponent` — tag + encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedComponent {
    /// Component tag.
    pub tag: ComponentId,
    /// Encapsulation bytes (endianness octet + tag-specific body).
    pub component_data: Vec<u8>,
}

impl TaggedComponent {
    /// CDR encode.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode(&self, w: &mut Writer) -> Result<(), CdrError> {
        w.write_u32(self.tag.as_u32());
        let n = u32::try_from(self.component_data.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n);
        w.write_bytes(&self.component_data);
        Ok(())
    }

    /// CDR decode.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, CdrError> {
        let tag = ComponentId::from_u32(r.read_u32()?);
        let n = r.read_u32()? as usize;
        let bytes = r.read_bytes(n)?;
        Ok(Self {
            tag,
            component_data: bytes.to_vec(),
        })
    }

    /// Attempts to decode the component as a known structured type.
    ///
    /// # Errors
    /// CDR error in the encapsulation body.
    pub fn structured(&self) -> Result<StructuredComponent, CdrError> {
        StructuredComponent::decode(self.tag, &self.component_data)
    }
}

// -------------------------------------------------------------------
// Structured component bodies for the most important tags.
// -------------------------------------------------------------------

/// `TAG_ORB_TYPE` (spec §13.6.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbType(pub u32);

/// Code-set component (spec §13.10.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSetComponent {
    /// Native code set (e.g. `0x00010001` = ISO-8859-1).
    pub native_code_set: u32,
    /// Conversion code sets — we hold the list header (count); the full
    /// content is at the caller. CodeSetComponentInfo models that fully.
    pub conversion_code_sets: Vec<u32>,
}

/// `CodeSetComponentInfo` (spec §13.10.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSetComponentInfo {
    /// Char codeset.
    pub for_char_data: CodeSetComponent,
    /// Wide-char codeset.
    pub for_wchar_data: CodeSetComponent,
}

/// `TAG_ALTERNATE_IIOP_ADDRESS` (spec §15.7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateIiopAddress {
    /// Host name.
    pub host: String,
    /// TCP port.
    pub port: u16,
}

/// `TAG_SSL_SEC_TRANS` (OMG `Security/SSLIOP` module spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ssl {
    /// `target_supports` — bitmask of supported AssociationOptions.
    pub target_supports: u16,
    /// `target_requires` — bitmask of enforced AssociationOptions.
    pub target_requires: u16,
    /// SSL port.
    pub port: u16,
}

/// `TAG_TLS_SEC_TRANS` (OMG `Security/TLSIOP` module spec) — wire layout
/// identical to SSL_SEC_TRANS plus a TransportAddressList.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsSecTrans {
    /// `target_supports`.
    pub target_supports: u16,
    /// `target_requires`.
    pub target_requires: u16,
    /// `addresses` — list of alternative TLS endpoints.
    pub addresses: Vec<AlternateIiopAddress>,
}

/// `TAG_RMI_CUSTOM_MAX_STREAM_FORMAT` (spec §13.6.7.3 + JavaToIDL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFormatVersion(pub u8);

/// Structured form for the most important components.
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
    /// `TAG_CSI_SEC_MECH_LIST = 33` (spec CORBA 3.3 Part 2 §10.5).
    CsiSecMechList(CompoundSecMechList),
    /// `TAG_RMI_CUSTOM_MAX_STREAM_FORMAT`.
    StreamFormatVersion(StreamFormatVersion),
    /// `TAG_JAVA_CODEBASE` (spec §13.6.6.7) — list of codebase URLs.
    JavaCodebase(String),
    /// Other tags — opaque encapsulation.
    Opaque {
        /// Tag.
        tag: ComponentId,
        /// Body.
        bytes: Vec<u8>,
    },
}

impl StructuredComponent {
    /// Decodes a component encapsulation into a structured form, as far
    /// as the tag is known.
    ///
    /// # Errors
    /// CDR decode error in the body.
    pub fn decode(tag: ComponentId, encap: &[u8]) -> Result<Self, CdrError> {
        let endianness = read_endianness(encap)?;
        // CORBA 3.3 Part 2 §9.3.3: an encapsulation aligns from its first
        // octet, the byte order — a `u32` after it sits at offset 4, not 1.
        let reader = || -> Result<Reader<'_>, CdrError> {
            let mut r = Reader::new(encap, endianness);
            r.read_u8()?;
            Ok(r)
        };
        match tag {
            ComponentId::OrbType => {
                let mut r = reader()?;
                Ok(Self::OrbType(OrbType(r.read_u32()?)))
            }
            ComponentId::CodeSets => {
                let mut r = reader()?;
                let for_char = decode_code_set_component(&mut r)?;
                let for_wchar = decode_code_set_component(&mut r)?;
                Ok(Self::CodeSets(CodeSetComponentInfo {
                    for_char_data: for_char,
                    for_wchar_data: for_wchar,
                }))
            }
            ComponentId::AlternateIiopAddress => {
                let mut r = reader()?;
                let host = r.read_string()?;
                let port = r.read_u16()?;
                Ok(Self::AlternateIiopAddress(AlternateIiopAddress {
                    host,
                    port,
                }))
            }
            ComponentId::SslSecTrans => {
                let mut r = reader()?;
                Ok(Self::Ssl(Ssl {
                    target_supports: r.read_u16()?,
                    target_requires: r.read_u16()?,
                    port: r.read_u16()?,
                }))
            }
            ComponentId::TlsSecTrans => {
                let mut r = reader()?;
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
                let mut r = reader()?;
                Ok(Self::CsiSecMechList(CompoundSecMechList::decode(&mut r)?))
            }
            ComponentId::RmiCustomMaxStreamFormat => {
                let mut r = reader()?;
                Ok(Self::StreamFormatVersion(StreamFormatVersion(r.read_u8()?)))
            }
            ComponentId::JavaCodebase => {
                let mut r = reader()?;
                Ok(Self::JavaCodebase(r.read_string()?))
            }
            other => Ok(Self::Opaque {
                tag: other,
                bytes: encap.to_vec(),
            }),
        }
    }

    /// Encodes a structured component into an encapsulation
    /// (endianness octet + body).
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        // The byte-order octet goes through the writer: the body aligns from
        // it (CORBA 3.3 Part 2 §9.3.3).
        let mut w = Writer::new(endianness);
        w.write_u8(endianness_to_byte(endianness));
        match self {
            Self::OrbType(OrbType(v)) => w.write_u32(*v),
            Self::CodeSets(info) => {
                encode_code_set_component(&mut w, &info.for_char_data)?;
                encode_code_set_component(&mut w, &info.for_wchar_data)?;
            }
            Self::AlternateIiopAddress(a) => {
                w.write_string(&a.host);
                w.write_u16(a.port);
            }
            Self::Ssl(s) => {
                w.write_u16(s.target_supports);
                w.write_u16(s.target_requires);
                w.write_u16(s.port);
            }
            Self::TlsSecTrans(t) => {
                w.write_u16(t.target_supports);
                w.write_u16(t.target_requires);
                let n = u32::try_from(t.addresses.len()).map_err(|_| CdrError::Overflow)?;
                w.write_u32(n);
                for a in &t.addresses {
                    w.write_string(&a.host);
                    w.write_u16(a.port);
                }
            }
            Self::CsiSecMechList(list) => list.encode(&mut w)?,
            Self::StreamFormatVersion(StreamFormatVersion(v)) => w.write_u8(*v),
            Self::JavaCodebase(s) => w.write_string(s),
            Self::Opaque { bytes, .. } => {
                // The body in `bytes` already includes the endianness
                // octet — we just return it verbatim.
                return Ok(bytes.clone());
            }
        }
        Ok(w.as_bytes().to_vec())
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

fn decode_code_set_component(r: &mut Reader<'_>) -> Result<CodeSetComponent, CdrError> {
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

/// Well-known `CodeSetId` fallbacks for the negotiation (OSF registry).
const CS_UTF_8: u32 = 0x0501_0001;
const CS_UTF_16: u32 = 0x0001_0109;
const CS_ISO_8859_1: u32 = 0x0001_0001;

impl CodeSetComponent {
    /// Selects the transmission code set between client and server for
    /// *one* axis (char or wchar), the algorithm of CORBA 3.3 Part 2
    /// §7.10.2.6:
    ///
    /// 1. `CNCS == SNCS` → the native code set (no conversion).
    /// 2. `SNCS ∈ CCCS` → `SNCS` (the client converts).
    /// 3. `CNCS ∈ SCCS` → `CNCS` (the server converts).
    /// 4. `CCCS ∩ SCCS` not empty → the common code set "most preferable to
    ///    the server": the first of the server's conversion code sets,
    ///    which the component lists "in order of preference" (§7.10.2.4).
    /// 5. `compatible(CNCS, SNCS)` → `fallback` (UTF-8 for char, UTF-16 for
    ///    wchar), whether or not either side lists it.
    /// 6. Otherwise `None`: the caller raises `CODESET_INCOMPATIBLE` with
    ///    minor code 1.
    ///
    /// `self` is the client's component, `server` the one from the IOR;
    /// [`zerodds_corba_giop::code_set_registry::compatible`] is the test of
    /// step 5. A null native code set (0) matches nothing.
    #[must_use]
    pub fn negotiate(&self, server: &CodeSetComponent, fallback: u32) -> Option<u32> {
        let cn = self.native_code_set;
        let sn = server.native_code_set;
        if cn != 0 && cn == sn {
            return Some(cn);
        }
        if sn != 0 && self.conversion_code_sets.contains(&sn) {
            return Some(sn);
        }
        if cn != 0 && server.conversion_code_sets.contains(&cn) {
            return Some(cn);
        }
        if let Some(common) = server
            .conversion_code_sets
            .iter()
            .find(|id| **id != 0 && self.conversion_code_sets.contains(id))
        {
            return Some(*common);
        }
        zerodds_corba_giop::code_set_registry::compatible(cn, sn).then_some(fallback)
    }
}

/// The transmission code sets for a connection (Part 2 §7.10.2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransmissionCodeSets {
    /// TCS-C, the code set of `char` and `string` data.
    pub char_data: u32,
    /// TCS-W, the code set of `wchar` and `wstring` data; `None` when the
    /// server names no wchar code set (§7.10.2.4: "there is no default
    /// wchar code set"), see [`Self::wchar_or_exception_minor`].
    pub wchar_data: Option<u32>,
    /// Whether the IOR carried the code set component.
    pub from_component: bool,
}

impl TransmissionCodeSets {
    /// The TCS-W for wide data, or the minor code of the `INV_OBJREF`
    /// the client raises when it has to marshal wide data (§7.10.2.4,
    /// §7.10.2.6): 2 when the reference has no code set component, 1 when
    /// the component names no wchar code set.
    ///
    /// # Errors
    /// The `INV_OBJREF` minor code.
    pub fn wchar_or_exception_minor(&self) -> Result<u32, u32> {
        match (self.wchar_data, self.from_component) {
            (Some(tcs), _) => Ok(tcs),
            (None, false) => Err(2),
            (None, true) => Err(1),
        }
    }
}

impl CodeSetComponentInfo {
    /// Negotiates both axes (char + wchar) against the server component from
    /// its IOR. `self` = client capabilities. Returns `(TCSC, TCSW)` or
    /// `None` if one of the axes is incompatible (→ `CODESET_INCOMPATIBLE`
    /// minor 1). Fallbacks: UTF-8 for `char`, UTF-16 for `wchar`.
    #[must_use]
    pub fn negotiate(&self, server: &CodeSetComponentInfo) -> Option<(u32, u32)> {
        let tcsc = self
            .for_char_data
            .negotiate(&server.for_char_data, CS_UTF_8)?;
        let tcsw = self
            .for_wchar_data
            .negotiate(&server.for_wchar_data, CS_UTF_16)?;
        Some((tcsc, tcsw))
    }

    /// The transmission code sets for a reference whose IIOP profile
    /// carries `server` as its `TAG_CODE_SETS` component, or none
    /// (Part 2 §7.10.2.4, §7.10.2.6): without the component the server
    /// supports ISO 8859-1 for char data and no wchar data; a server that
    /// names no native wchar code set (null) has no TCS-W, also when it
    /// lists wchar conversion code sets — §7.10.2.6 requires a server
    /// with wide character data to specify its native wchar code set, "if
    /// one is not specified, then the client-side ORB raises exception
    /// INV_OBJREF, with standard minor code set to 1"
    /// ([`TransmissionCodeSets::wchar_or_exception_minor`]). The null
    /// native set "possibly with ... conversion code sets" of §7.10.2.4
    /// is taken for char data, where §7.10.2.6 reads it as ISO 8859-1.
    ///
    /// # Errors
    /// `CODESET_INCOMPATIBLE` minor 1 (returned as `1`) when an axis has
    /// no transmission code set.
    pub fn transmission_code_sets(
        &self,
        server: Option<&CodeSetComponentInfo>,
    ) -> Result<TransmissionCodeSets, u32> {
        let Some(server) = server else {
            let latin1 = CodeSetComponent {
                native_code_set: CS_ISO_8859_1,
                conversion_code_sets: Vec::new(),
            };
            return Ok(TransmissionCodeSets {
                char_data: self
                    .for_char_data
                    .negotiate(&latin1, CS_UTF_8)
                    .ok_or(1u32)?,
                wchar_data: None,
                from_component: false,
            });
        };
        // §7.10.2.6: "If a server's native char code set is not specified
        // in the IOR multi-component profile, then it is considered to be
        // ISO 8859-1".
        let mut server_char = server.for_char_data.clone();
        if server_char.native_code_set == 0 {
            server_char.native_code_set = CS_ISO_8859_1;
        }
        let char_data = self
            .for_char_data
            .negotiate(&server_char, CS_UTF_8)
            .ok_or(1u32)?;
        let w = &server.for_wchar_data;
        let wchar_data = if w.native_code_set == 0 {
            None
        } else {
            Some(self.for_wchar_data.negotiate(w, CS_UTF_16).ok_or(1u32)?)
        };
        Ok(TransmissionCodeSets {
            char_data,
            wchar_data,
            from_component: true,
        })
    }
}

fn encode_code_set_component(w: &mut Writer, c: &CodeSetComponent) -> Result<(), CdrError> {
    w.write_u32(c.native_code_set);
    let n = u32::try_from(c.conversion_code_sets.len()).map_err(|_| CdrError::Overflow)?;
    w.write_u32(n);
    for cs in &c.conversion_code_sets {
        w.write_u32(*cs);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The alignment of an encapsulation counts its byte-order octet: after
    /// it come three padding octets before a `u32` (CORBA 3.3 Part 2
    /// §9.3.3). omniORB 4.3's `catior` called the unpadded form "Broken
    /// component with tag 1" (Linux test host).
    #[test]
    fn a_code_sets_encapsulation_pads_after_the_byte_order_octet() {
        let cs = StructuredComponent::CodeSets(CodeSetComponentInfo {
            for_char_data: CodeSetComponent {
                native_code_set: 0x0501_0001,
                conversion_code_sets: alloc::vec![0x0001_0001],
            },
            for_wchar_data: CodeSetComponent {
                native_code_set: 0x0001_0109,
                conversion_code_sets: alloc::vec![],
            },
        });
        let bytes = cs.encode_encapsulation(Endianness::Big).unwrap();
        assert_eq!(
            bytes,
            [
                0, 0, 0, 0, // byte order, then padding to 4
                0x05, 0x01, 0x00, 0x01, // native char: UTF-8
                0, 0, 0, 1, 0x00, 0x01, 0x00, 0x01, // conversion: ISO 8859-1
                0x00, 0x01, 0x01, 0x09, // native wchar: UTF-16
                0, 0, 0, 0, // no conversion
            ]
        );
        let ssl = StructuredComponent::Ssl(Ssl {
            target_supports: 0x66,
            target_requires: 0x40,
            port: 6834,
        });
        let bytes = ssl.encode_encapsulation(Endianness::Big).unwrap();
        assert_eq!(bytes, [0, 0, 0x00, 0x66, 0x00, 0x40, 0x1a, 0xb2]);
        assert_eq!(
            StructuredComponent::decode(ComponentId::SslSecTrans, &bytes).unwrap(),
            ssl
        );
    }

    #[test]
    fn orb_type_round_trip() {
        let s = StructuredComponent::OrbType(OrbType(0x4F4D_4732)); // "OMG2" vendor style
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

    fn cs(native: u32, conv: &[u32]) -> CodeSetComponent {
        CodeSetComponent {
            native_code_set: native,
            conversion_code_sets: conv.to_vec(),
        }
    }

    #[test]
    fn negotiate_native_match() {
        // Both native ISO-8859-1 → exactly that.
        let client = cs(0x0001_0001, &[]);
        let server = cs(0x0001_0001, &[]);
        assert_eq!(client.negotiate(&server, CS_UTF_8), Some(0x0001_0001));
    }

    #[test]
    fn negotiate_client_converts_to_server_native() {
        // Server-native (UTF-8) is in the client conversion set → server-native.
        let client = cs(0x0001_0001, &[CS_UTF_8]);
        let server = cs(CS_UTF_8, &[]);
        assert_eq!(client.negotiate(&server, CS_UTF_8), Some(CS_UTF_8));
    }

    #[test]
    fn negotiate_server_converts_to_client_native() {
        // Client-native (ISO) is in the server conversion set → client-native.
        let client = cs(0x0001_0001, &[]);
        let server = cs(CS_UTF_8, &[0x0001_0001]);
        assert_eq!(client.negotiate(&server, CS_UTF_8), Some(0x0001_0001));
    }

    #[test]
    fn negotiate_common_conversion_set() {
        // No native bridge, but UTF-8 in both conversion lists.
        let client = cs(0x0001_0002, &[CS_UTF_8]);
        let server = cs(0x0001_0003, &[CS_UTF_8]);
        assert_eq!(client.negotiate(&server, 0xDEAD), Some(CS_UTF_8));
    }

    #[test]
    fn negotiate_universal_fallback() {
        // Disjoint sets, but both carry the fallback (UTF-16) as native/conv.
        let client = cs(CS_UTF_16, &[]);
        let server = cs(0x0001_0100, &[CS_UTF_16]);
        assert_eq!(client.negotiate(&server, CS_UTF_16), Some(CS_UTF_16));
    }

    #[test]
    fn negotiate_incompatible_yields_none() {
        let client = cs(0x0001_0002, &[0x0001_0004]);
        let server = cs(0x0001_0003, &[0x0001_0005]);
        assert_eq!(client.negotiate(&server, 0xBEEF), None);
    }

    #[test]
    fn negotiate_info_both_axes_default_fallbacks() {
        // Disjoint natives, but UTF-8/UTF-16 universal → default pair.
        let client = CodeSetComponentInfo {
            for_char_data: cs(0x0001_0001, &[CS_UTF_8]),
            for_wchar_data: cs(CS_UTF_16, &[]),
        };
        let server = CodeSetComponentInfo {
            for_char_data: cs(CS_UTF_8, &[]),
            for_wchar_data: cs(CS_UTF_16, &[]),
        };
        assert_eq!(client.negotiate(&server), Some((CS_UTF_8, CS_UTF_16)));
    }

    #[test]
    fn negotiate_info_none_if_one_axis_fails() {
        let client = CodeSetComponentInfo {
            for_char_data: cs(CS_UTF_8, &[]),
            for_wchar_data: cs(0x0001_0002, &[]), // incompatible
        };
        let server = CodeSetComponentInfo {
            for_char_data: cs(CS_UTF_8, &[]),
            for_wchar_data: cs(0x0001_0003, &[]),
        };
        assert_eq!(client.negotiate(&server), None);
    }

    /// §7.10.2.6 step 4: of several common conversion code sets the one
    /// "most preferable to the server", its conversion code sets being
    /// "listed in order of preference" (§7.10.2.4).
    #[test]
    fn corba_p2_7_10_2_6_intermediate_set_is_the_servers_most_preferred() {
        let client = cs(0x0003_0001, &[0x0001_0001, CS_UTF_8]);
        let server = cs(0x0003_0010, &[CS_UTF_8, 0x0001_0001]);
        assert_eq!(client.negotiate(&server, CS_UTF_8), Some(CS_UTF_8));
        let server = cs(0x0003_0010, &[0x0001_0001, CS_UTF_8]);
        assert_eq!(client.negotiate(&server, CS_UTF_8), Some(0x0001_0001));
    }

    /// §7.10.2.6 step 5: the fallback is taken when the native code sets
    /// are compatible, whether or not either side lists it; UCS-2 and
    /// UTF-16 encode the same character set.
    #[test]
    fn corba_p2_7_10_2_6_the_fallback_needs_compatible_natives() {
        let client = cs(0x0001_0100, &[]);
        let server = cs(0x0001_0104, &[]);
        assert_eq!(client.negotiate(&server, CS_UTF_16), Some(CS_UTF_16));
    }

    /// §7.10.2.6: native code sets of different character sets and no
    /// conversion in common: `CODESET_INCOMPATIBLE` minor 1 (`None`).
    #[test]
    fn corba_p2_7_10_2_6_incompatible_character_sets_raise_codeset_incompatible_1() {
        let client = cs(0x0001_0001, &[]);
        let server = cs(CS_UTF_8, &[]);
        assert_eq!(client.negotiate(&server, CS_UTF_8), None);
        let info = |c: CodeSetComponent| CodeSetComponentInfo {
            for_char_data: c,
            for_wchar_data: cs(CS_UTF_16, &[]),
        };
        let server = info(cs(CS_UTF_8, &[]));
        assert_eq!(
            info(cs(0x0001_0001, &[])).transmission_code_sets(Some(&server)),
            Err(1)
        );
    }

    /// §7.10.2.4, §7.10.2.6: without the code set component the server
    /// takes ISO 8859-1 for char data and no wchar data (`INV_OBJREF`
    /// minor 2 for wide data); a null native char code set is ISO 8859-1;
    /// a component without wchar code sets gives no TCS-W (`INV_OBJREF`
    /// minor 1 for wide data).
    #[test]
    fn corba_p2_7_10_2_4_defaults_without_the_component_or_a_wchar_set() {
        let client = CodeSetComponentInfo {
            for_char_data: cs(CS_UTF_8, &[0x0001_0001]),
            for_wchar_data: cs(CS_UTF_16, &[]),
        };
        let tcs = client.transmission_code_sets(None).unwrap();
        assert_eq!(tcs.char_data, 0x0001_0001);
        assert_eq!(tcs.wchar_or_exception_minor(), Err(2));
        let server = CodeSetComponentInfo {
            for_char_data: cs(0, &[]),
            for_wchar_data: cs(0, &[]),
        };
        let tcs = client.transmission_code_sets(Some(&server)).unwrap();
        assert_eq!(tcs.char_data, 0x0001_0001);
        assert_eq!(tcs.wchar_or_exception_minor(), Err(1));
        // §7.10.2.6: no native wchar code set is INV_OBJREF minor 1, also
        // with wchar conversion code sets, one of them the client's
        // native set or UTF-16; never CODESET_INCOMPATIBLE.
        for conversions in [&[CS_UTF_16][..], &[0x0001_0100, CS_UTF_16]] {
            let server = CodeSetComponentInfo {
                for_char_data: cs(CS_UTF_8, &[]),
                for_wchar_data: cs(0, conversions),
            };
            let tcs = client.transmission_code_sets(Some(&server)).unwrap();
            assert_eq!(tcs.char_data, CS_UTF_8);
            assert_eq!(tcs.wchar_or_exception_minor(), Err(1), "{conversions:?}");
        }
        let server = CodeSetComponentInfo {
            for_char_data: cs(CS_UTF_8, &[]),
            for_wchar_data: cs(CS_UTF_16, &[]),
        };
        let tcs = client.transmission_code_sets(Some(&server)).unwrap();
        assert_eq!(
            (tcs.char_data, tcs.wchar_or_exception_minor()),
            (CS_UTF_8, Ok(CS_UTF_16))
        );
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
        let mut w = Writer::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let buf = w.finish();
        let mut r = Reader::new(&buf, Endianness::Big);
        let decoded = TaggedComponent::decode(&mut r).unwrap();
        assert_eq!(decoded, tc);
    }
}
