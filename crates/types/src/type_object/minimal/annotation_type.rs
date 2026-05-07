// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalAnnotationType (XTypes §7.3.4.4.10).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{MemberId, decode_seq, encode_seq};
use crate::type_object::flags::{AnnotationParameterFlag, AnnotationTypeFlag};

/// AnnotationParameter-Default-Value. Im Minimal wird der Default als
/// CDR-serialisierter Blob mit fuehrendem Type-Discriminator abgelegt
/// (§7.3.4.5 AnnotationParameterValue). Fuer Phase 1 speichern wir ihn
/// als opaque bytes — Validator/Consumer muss den Discriminator-Byte
/// lesen und daraufhin den Wert interpretieren.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationParameterValue {
    /// Raw-Bytes (inkl. Discriminator).
    pub raw: Vec<u8>,
}

impl AnnotationParameterValue {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        // Als sequence<octet>: u32 Laenge + bytes.
        let len = u32::try_from(self.raw.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "annotation parameter value exceeds u32::MAX bytes",
        })?;
        w.write_u32(len)?;
        w.write_bytes(&self.raw)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len = r.read_u32()? as usize;
        let raw = r.read_bytes(len)?.to_vec();
        Ok(Self { raw })
    }
}

/// Common-Annotation-Parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonAnnotationParameter {
    /// Member-ID.
    pub member_id: MemberId,
    /// Flags.
    pub member_flags: AnnotationParameterFlag,
    /// Parameter-Typ.
    pub member_type_id: TypeIdentifier,
}

/// MinimalAnnotationParameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAnnotationParameter {
    /// Common.
    pub common: CommonAnnotationParameter,
    /// Parameter-Name (im Minimal als volle String, nicht Hash — weil
    /// Annotation-Parameter oft by-name angesprochen werden).
    pub name: String,
    /// Default-Wert.
    pub default_value: AnnotationParameterValue,
}

impl MinimalAnnotationParameter {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.common.member_id)?;
        w.write_u16(self.common.member_flags.0)?;
        self.common.member_type_id.encode_into(w)?;
        w.write_string(&self.name)?;
        self.default_value.encode_into(w)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let member_id = r.read_u32()?;
        let member_flags = AnnotationParameterFlag(r.read_u16()?);
        let member_type_id = TypeIdentifier::decode_from(r)?;
        let name = r.read_string()?;
        let default_value = AnnotationParameterValue::decode_from(r)?;
        Ok(Self {
            common: CommonAnnotationParameter {
                member_id,
                member_flags,
                member_type_id,
            },
            name,
            default_value,
        })
    }
}

/// MinimalAnnotationType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAnnotationType {
    /// Flags.
    pub annotation_flag: AnnotationTypeFlag,
    /// Parameter-Liste.
    pub member_seq: Vec<MinimalAnnotationParameter>,
}

impl MinimalAnnotationType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.annotation_flag.0)?;
        encode_seq(w, &self.member_seq, |w, p| p.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let annotation_flag = AnnotationTypeFlag(r.read_u16()?);
        let member_seq = decode_seq(r, MinimalAnnotationParameter::decode_from)?;
        Ok(Self {
            annotation_flag,
            member_seq,
        })
    }
}
