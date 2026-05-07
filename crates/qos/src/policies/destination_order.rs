// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DestinationOrderQosPolicy (DDS 1.4 §2.2.3.18).
//!
//! Wire-Format: u32 kind = 4 byte.
//!
//! Compatibility per §2.2.3 Table: `offered.kind >= requested.kind`.
//! Ordering: `BY_RECEPTION_TIMESTAMP < BY_SOURCE_TIMESTAMP`.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// DestinationOrder-Kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u32)]
pub enum DestinationOrderKind {
    /// Order by Reception-Timestamp (default).
    #[default]
    ByReceptionTimestamp = 0,
    /// Order by Source-Timestamp.
    BySourceTimestamp = 1,
}

impl DestinationOrderKind {
    /// Strikter Mapper.
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::ByReceptionTimestamp),
            1 => Some(Self::BySourceTimestamp),
            _ => None,
        }
    }

    /// Forward-kompatibler Mapper.
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::BySourceTimestamp,
            _ => Self::ByReceptionTimestamp,
        }
    }
}

/// DestinationOrderQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DestinationOrderQosPolicy {
    /// Kind.
    pub kind: DestinationOrderKind,
}

impl DestinationOrderQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.kind as u32)
    }

    /// Wire-Decoding (strict).
    ///
    /// # Errors
    /// Buffer-Underflow oder unbekannter Kind-Wert.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let v = r.read_u32()?;
        let kind = DestinationOrderKind::try_from_u32(v).ok_or(DecodeError::InvalidEnum {
            kind: "DestinationOrderKind",
            value: v,
        })?;
        Ok(Self { kind })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_by_reception() {
        assert_eq!(
            DestinationOrderQosPolicy::default().kind,
            DestinationOrderKind::ByReceptionTimestamp
        );
    }

    #[test]
    fn kind_ordering() {
        assert!(
            DestinationOrderKind::ByReceptionTimestamp < DestinationOrderKind::BySourceTimestamp
        );
    }

    #[test]
    fn roundtrip() {
        for kind in [
            DestinationOrderKind::ByReceptionTimestamp,
            DestinationOrderKind::BySourceTimestamp,
        ] {
            let p = DestinationOrderQosPolicy { kind };
            let mut w = BufferWriter::new(Endianness::Little);
            p.encode_into(&mut w).unwrap();
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            assert_eq!(DestinationOrderQosPolicy::decode_from(&mut r).unwrap(), p);
        }
    }

    /// `try_from_u32` muss für unbekannte Werte `None` liefern (strict).
    #[test]
    fn try_from_u32_unknown_is_none() {
        assert_eq!(DestinationOrderKind::try_from_u32(2), None);
        assert_eq!(DestinationOrderKind::try_from_u32(u32::MAX), None);
    }

    /// Forward-kompatibler Mapper: unbekannt -> `ByReceptionTimestamp`
    /// (konservativster Default; matcht Cyclone-Verhalten).
    #[test]
    fn from_u32_forward_compatible() {
        assert_eq!(
            DestinationOrderKind::from_u32(0),
            DestinationOrderKind::ByReceptionTimestamp
        );
        assert_eq!(
            DestinationOrderKind::from_u32(1),
            DestinationOrderKind::BySourceTimestamp
        );
        assert_eq!(
            DestinationOrderKind::from_u32(42),
            DestinationOrderKind::ByReceptionTimestamp
        );
    }

    /// Decode mit unbekanntem kind-Discriminator → `InvalidEnum` Fehler
    /// (Round-2-Review-Finding: Error-Path bisher ungetestet).
    #[test]
    fn decode_unknown_kind_errors() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(77).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = DestinationOrderQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(matches!(
            err,
            zerodds_cdr::DecodeError::InvalidEnum {
                kind: "DestinationOrderKind",
                value: 77
            }
        ));
    }

    /// Buffer ohne Bytes -> short-read error auf `read_u32`.
    #[test]
    fn decode_empty_buffer_errors() {
        let bytes: [u8; 0] = [];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(DestinationOrderQosPolicy::decode_from(&mut r).is_err());
    }
}
