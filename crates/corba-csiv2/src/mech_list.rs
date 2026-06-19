// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CompoundSecMechList — Spec §24.2.6.5.
//!
//! Transported in the IOR as a `TAG_CSI_SEC_MECH_LIST` TaggedComponent.
//! Describes the available security layers and their mechanisms.
//!
//! Modeled in a simplified form (caller-layer wire codec):
//!
//! ```text
//! struct CompoundSecMechList {
//!     boolean stateful;
//!     sequence<CompoundSecMech> mechanism_list;
//! };
//!
//! struct CompoundSecMech {
//!     AssociationOptions   target_requires;
//!     IOP::TaggedComponent transport_mech;  // typisch TAG_TLS_SEC_TRANS
//!     AsContextSec         as_context_mech;
//!     SasContextSec        sas_context_mech;
//! };
//! ```

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::association_options::AssociationOptions;

/// CSI AS-layer mechanism description (Spec §24.2.6.5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsContextSec {
    /// `target_supports`.
    pub target_supports: AssociationOptions,
    /// `target_requires`.
    pub target_requires: AssociationOptions,
    /// `client_authentication_mech` — DER-encoded GSS mechanism OID
    /// (typically GSSUP).
    pub client_authentication_mech: Vec<u8>,
    /// `target_name` — target name (e.g. realm bytes).
    pub target_name: Vec<u8>,
}

/// CSI SAS-layer mechanism description (Spec §24.2.6.5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SasContextSec {
    /// `target_supports`.
    pub target_supports: AssociationOptions,
    /// `target_requires`.
    pub target_requires: AssociationOptions,
    /// `privilege_authorities` — list of DER-encoded authorities.
    pub privilege_authorities: Vec<Vec<u8>>,
    /// `supported_naming_mechanisms` — DER-encoded GSS naming mechs.
    pub supported_naming_mechanisms: Vec<Vec<u8>>,
    /// `supported_identity_types` — bitfield (Spec §24.2.5).
    pub supported_identity_types: u32,
}

/// CompoundSecMech.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundSecMech {
    /// `target_requires`.
    pub target_requires: AssociationOptions,
    /// `transport_mech` — opaque TaggedComponent bytes (typically
    /// TAG_TLS_SEC_TRANS = 36 or TAG_SECIOP_SEC_TRANS = 35).
    pub transport_mech_tag: u32,
    /// Encapsulation of the transport-mech body (e.g. `TLS_SEC_TRANS`).
    pub transport_mech_data: Vec<u8>,
    /// AS-layer description.
    pub as_context: AsContextSec,
    /// SAS-layer description.
    pub sas_context: SasContextSec,
}

/// CompoundSecMechList.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompoundSecMechList {
    /// `stateful` — `true` means multiple requests reuse the same
    /// security context (performance optimization).
    pub stateful: bool,
    /// List of offered mechanisms (in preference order).
    pub mechanism_list: Vec<CompoundSecMech>,
}

// ============================================================
// CDR-Wire-Encoding (Spec §24.2.6.5 + §10.5 IOR-Component-Body)
// ============================================================

fn write_octet_seq(w: &mut BufferWriter, bytes: &[u8]) -> Result<(), EncodeError> {
    let n = u32::try_from(bytes.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "csiv2 octet/seq length exceeds u32::MAX",
    })?;
    w.write_u32(n)?;
    w.write_bytes(bytes)
}

fn read_octet_seq(r: &mut BufferReader<'_>) -> Result<Vec<u8>, DecodeError> {
    let n = r.read_u32()? as usize;
    let bytes = r.read_bytes(n)?;
    Ok(bytes.to_vec())
}

fn write_octet_seq_seq(w: &mut BufferWriter, items: &[Vec<u8>]) -> Result<(), EncodeError> {
    let n = u32::try_from(items.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "csiv2 octet/seq length exceeds u32::MAX",
    })?;
    w.write_u32(n)?;
    for item in items {
        write_octet_seq(w, item)?;
    }
    Ok(())
}

fn read_octet_seq_seq(r: &mut BufferReader<'_>) -> Result<Vec<Vec<u8>>, DecodeError> {
    let n = r.read_u32()? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(read_octet_seq(r)?);
    }
    Ok(out)
}

impl AsContextSec {
    /// CDR-Encode (Spec §24.2.6.5.4).
    ///
    /// # Errors
    /// `EncodeError::ValueOutOfRange { message: "csiv2 octet/seq length exceeds u32::MAX" }` if an octet-seq would be larger than
    /// `u32::MAX`.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.target_supports.0)?;
        w.write_u16(self.target_requires.0)?;
        write_octet_seq(w, &self.client_authentication_mech)?;
        write_octet_seq(w, &self.target_name)
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// `DecodeError::*` if the reader is truncated.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let target_supports = AssociationOptions(r.read_u16()?);
        let target_requires = AssociationOptions(r.read_u16()?);
        let client_authentication_mech = read_octet_seq(r)?;
        let target_name = read_octet_seq(r)?;
        Ok(Self {
            target_supports,
            target_requires,
            client_authentication_mech,
            target_name,
        })
    }
}

impl SasContextSec {
    /// CDR-Encode (Spec §24.2.6.5.5).
    ///
    /// # Errors
    /// `EncodeError::ValueOutOfRange { message: "csiv2 octet/seq length exceeds u32::MAX" }` if a seq would be larger than
    /// `u32::MAX`.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.target_supports.0)?;
        w.write_u16(self.target_requires.0)?;
        write_octet_seq_seq(w, &self.privilege_authorities)?;
        write_octet_seq_seq(w, &self.supported_naming_mechanisms)?;
        w.write_u32(self.supported_identity_types)
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// `DecodeError::*` if the reader is truncated.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let target_supports = AssociationOptions(r.read_u16()?);
        let target_requires = AssociationOptions(r.read_u16()?);
        let privilege_authorities = read_octet_seq_seq(r)?;
        let supported_naming_mechanisms = read_octet_seq_seq(r)?;
        let supported_identity_types = r.read_u32()?;
        Ok(Self {
            target_supports,
            target_requires,
            privilege_authorities,
            supported_naming_mechanisms,
            supported_identity_types,
        })
    }
}

impl CompoundSecMech {
    /// CDR-Encode (Spec §24.2.6.5).
    ///
    /// # Errors
    /// `EncodeError::ValueOutOfRange { message: "csiv2 octet/seq length exceeds u32::MAX" }`.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.target_requires.0)?;
        // IOP::TaggedComponent for transport_mech: tag (u32) + sequence<octet> data.
        w.write_u32(self.transport_mech_tag)?;
        write_octet_seq(w, &self.transport_mech_data)?;
        self.as_context.encode(w)?;
        self.sas_context.encode(w)
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// `DecodeError::*`.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let target_requires = AssociationOptions(r.read_u16()?);
        let transport_mech_tag = r.read_u32()?;
        let transport_mech_data = read_octet_seq(r)?;
        let as_context = AsContextSec::decode(r)?;
        let sas_context = SasContextSec::decode(r)?;
        Ok(Self {
            target_requires,
            transport_mech_tag,
            transport_mech_data,
            as_context,
            sas_context,
        })
    }
}

impl CompoundSecMechList {
    /// CDR-Encode (Spec §24.2.6.5). Used in the IOR as the
    /// `TAG_CSI_SEC_MECH_LIST=33` component body (CDR encapsulation:
    /// 1-byte endianness + 1-byte padding/reserved + this body).
    ///
    /// # Errors
    /// `EncodeError::ValueOutOfRange { message: "csiv2 octet/seq length exceeds u32::MAX" }` if `mechanism_list.len()` > `u32::MAX`.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u8(u8::from(self.stateful))?;
        let n =
            u32::try_from(self.mechanism_list.len()).map_err(|_| EncodeError::ValueOutOfRange {
                message: "csiv2 octet/seq length exceeds u32::MAX",
            })?;
        w.write_u32(n)?;
        for mech in &self.mechanism_list {
            mech.encode(w)?;
        }
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// `DecodeError::*`.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let stateful = r.read_u8()? != 0;
        let n = r.read_u32()? as usize;
        let mut mechanism_list = Vec::with_capacity(n);
        for _ in 0..n {
            mechanism_list.push(CompoundSecMech::decode(r)?);
        }
        Ok(Self {
            stateful,
            mechanism_list,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn build_mech_list_with_tls_and_gssup() {
        let mech = CompoundSecMech {
            target_requires: AssociationOptions::default()
                .with(AssociationOptions::INTEGRITY)
                .with(AssociationOptions::CONFIDENTIALITY),
            transport_mech_tag: 36, // TAG_TLS_SEC_TRANS
            transport_mech_data: alloc::vec![0xab, 0xcd],
            as_context: AsContextSec {
                target_supports: AssociationOptions::from_bits(
                    AssociationOptions::ESTABLISH_TRUST_IN_CLIENT,
                ),
                target_requires: AssociationOptions::default(),
                client_authentication_mech: super::super::gssup::GSSUP_OID_DER.to_vec(),
                target_name: b"REALM.LAB".to_vec(),
            },
            sas_context: SasContextSec {
                target_supports: AssociationOptions::default()
                    .with(AssociationOptions::IDENTITY_ASSERTION),
                target_requires: AssociationOptions::default(),
                privilege_authorities: alloc::vec![],
                supported_naming_mechanisms: alloc::vec![],
                supported_identity_types: 0,
            },
        };
        let list = CompoundSecMechList {
            stateful: true,
            mechanism_list: alloc::vec![mech.clone()],
        };
        assert_eq!(list.mechanism_list.len(), 1);
        assert_eq!(list.mechanism_list[0].transport_mech_tag, 36);
        assert_eq!(
            list.mechanism_list[0].as_context.client_authentication_mech,
            super::super::gssup::GSSUP_OID_DER
        );
    }

    #[test]
    fn empty_list_has_no_mechanisms() {
        let list = CompoundSecMechList::default();
        assert!(list.mechanism_list.is_empty());
        assert!(!list.stateful);
    }

    /// Spec §24.2.6.5 — CompoundSecMechList CDR roundtrip. Wire-up
    /// for F-CORBA-CSIV2-NOT-WIRED: csiv2 now uses zerodds-cdr in
    /// production (it was a Cargo dep without actual use before the
    /// review).
    #[test]
    fn cdr_roundtrip_compound_sec_mech_list() {
        use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

        let original = CompoundSecMechList {
            stateful: true,
            mechanism_list: alloc::vec![CompoundSecMech {
                target_requires: AssociationOptions::default()
                    .with(AssociationOptions::INTEGRITY)
                    .with(AssociationOptions::CONFIDENTIALITY),
                transport_mech_tag: 36, // TAG_TLS_SEC_TRANS
                transport_mech_data: alloc::vec![0xab, 0xcd, 0xef],
                as_context: AsContextSec {
                    target_supports: AssociationOptions::from_bits(
                        AssociationOptions::ESTABLISH_TRUST_IN_CLIENT,
                    ),
                    target_requires: AssociationOptions::default(),
                    client_authentication_mech: super::super::gssup::GSSUP_OID_DER.to_vec(),
                    target_name: b"REALM.LAB".to_vec(),
                },
                sas_context: SasContextSec {
                    target_supports: AssociationOptions::default()
                        .with(AssociationOptions::IDENTITY_ASSERTION),
                    target_requires: AssociationOptions::default(),
                    privilege_authorities: alloc::vec![b"auth-1".to_vec(), b"auth-2".to_vec()],
                    supported_naming_mechanisms: alloc::vec![b"GSS_KRB5".to_vec()],
                    supported_identity_types: 0x0000_0007,
                },
            }],
        };

        let mut w = BufferWriter::new(Endianness::Little);
        original.encode(&mut w).expect("encode");
        let bytes = w.into_bytes();
        assert!(!bytes.is_empty());

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = CompoundSecMechList::decode(&mut r).expect("decode");
        assert_eq!(original, decoded);
    }

    #[test]
    fn cdr_roundtrip_empty_list() {
        use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
        let original = CompoundSecMechList::default();
        let mut w = BufferWriter::new(Endianness::Little);
        original.encode(&mut w).expect("encode");
        let bytes = w.into_bytes();
        // 1-byte stateful + 3-byte CDR padding to u32 alignment + 4-byte length=0
        assert_eq!(bytes.len(), 8);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = CompoundSecMechList::decode(&mut r).expect("decode");
        assert_eq!(original, decoded);
    }
}
