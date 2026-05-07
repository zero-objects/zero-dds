// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! User/Group/Topic DataQosPolicy (DDS 1.4 §2.2.3.1–3).
//!
//! Alle drei teilen dieselbe Wire-Form: opaque `sequence<octet>` =
//! u32 length + N × octet (plus CDR-Alignment-Padding auf 4 byte).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// DoS-Cap pro opaque-Data-Block. 64 KiB deckt realistische
/// UserData/TopicData/GroupData-Payloads ab (ueblich <1 KiB); groesseres
/// wird als `LengthExceeded` abgelehnt. Spec legt keine harte Grenze
/// fest, aber Cyclone DDS nutzt ~1 MiB als "vernuenftig".
pub const MAX_OPAQUE_LEN: usize = 64 * 1024;

/// Gemeinsame Wire-Form: opaque sequence<octet>.
fn encode_opaque(w: &mut BufferWriter, bytes: &[u8]) -> Result<(), EncodeError> {
    let len = u32::try_from(bytes.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "opaque data length exceeds u32::MAX",
    })?;
    w.write_u32(len)?;
    w.write_bytes(bytes)
}

fn decode_opaque(r: &mut BufferReader<'_>) -> Result<Vec<u8>, DecodeError> {
    let len = r.read_u32()? as usize;
    // Harter DoS-Cap: MAX_OPAQUE_LEN.
    if len > MAX_OPAQUE_LEN {
        return Err(DecodeError::LengthExceeded {
            announced: len,
            remaining: MAX_OPAQUE_LEN,
            offset: 0,
        });
    }
    // read_bytes liefert einen Slice — direkt kopieren, keine Vec-
    // with_capacity-Optimierung (die extend_from_slice re-allokiert
    // sowieso wenn noetig).
    Ok(r.read_bytes(len)?.to_vec())
}

/// UserDataQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserDataQosPolicy {
    /// Opaque Bytes.
    pub value: Vec<u8>,
}

impl UserDataQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// `ValueOutOfRange` bei u32-Ueberlauf.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_opaque(w, &self.value)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            value: decode_opaque(r)?,
        })
    }
}

/// TopicDataQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicDataQosPolicy {
    /// Opaque Bytes.
    pub value: Vec<u8>,
}

impl TopicDataQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// `ValueOutOfRange` bei u32-Ueberlauf.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_opaque(w, &self.value)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            value: decode_opaque(r)?,
        })
    }
}

/// GroupDataQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GroupDataQosPolicy {
    /// Opaque Bytes.
    pub value: Vec<u8>,
}

impl GroupDataQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// `ValueOutOfRange` bei u32-Ueberlauf.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_opaque(w, &self.value)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            value: decode_opaque(r)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn user_data_roundtrip() {
        let p = UserDataQosPolicy {
            value: alloc::vec![1, 2, 3, 4],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(UserDataQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn empty_data_roundtrip() {
        let p = TopicDataQosPolicy::default();
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(TopicDataQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn decoder_rejects_oversized_opaque() {
        let mut bytes = alloc::vec::Vec::new();
        bytes.extend_from_slice(&(MAX_OPAQUE_LEN as u32 + 1).to_le_bytes());
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = UserDataQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(matches!(err, DecodeError::LengthExceeded { .. }));
    }

    #[test]
    fn group_data_large_payload() {
        let p = GroupDataQosPolicy {
            value: (0..200u8).collect(),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(GroupDataQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
