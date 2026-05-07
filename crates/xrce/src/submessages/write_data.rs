// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `WRITE_DATA` Submessage (id=7, Spec §8.3.5.8).
//!
//! Direction: Client → Agent. Flags Bits 1..3 kodieren das `DataFormat`:
//! - `FORMAT_DATA = 0b000`
//! - `FORMAT_SAMPLE = 0b001`
//! - `FORMAT_DATA_SEQ = 0b100`
//! - `FORMAT_SAMPLE_SEQ = 0b101`
//! - `FORMAT_PACKED_SAMPLES = 0b111`
//!
//! Bit 0 ist E-Flag (LE). Bits 4..7 sind reserved.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// `DataFormat`-Werte (3-Bit-Feld in Bits 1..3 der Submessage-Flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum DataFormat {
    #[default]
    Data = 0b000,
    Sample = 0b001,
    DataSeq = 0b100,
    SampleSeq = 0b101,
    PackedSamples = 0b111,
}

impl DataFormat {
    /// Encodiert in Flag-Byte (Bits 1..3).
    #[must_use]
    pub fn to_flag_bits(self) -> u8 {
        (self as u8) << 1
    }

    /// Extrahiert aus Flag-Byte (Bits 1..3). Reserved-Werte
    /// (z.B. `0b010`) werden auf `Data` gemappt mit Fehler-Result —
    /// hier konservativ lesen wir alle 8 Bit-Kombinationen, melden
    /// Reserved als `ValueOutOfRange`.
    ///
    /// # Errors
    /// `ValueOutOfRange` bei reservierten Bit-Kombinationen.
    pub fn from_flag_bits(flags: u8) -> Result<Self, XrceError> {
        match (flags >> 1) & 0b111 {
            0b000 => Ok(Self::Data),
            0b001 => Ok(Self::Sample),
            0b100 => Ok(Self::DataSeq),
            0b101 => Ok(Self::SampleSeq),
            0b111 => Ok(Self::PackedSamples),
            _ => Err(XrceError::ValueOutOfRange {
                message: "reserved DataFormat bit pattern",
            }),
        }
    }
}

/// Opaker Body + DataFormat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteDataPayload {
    /// XCDR2-Body fuer das gewaehlte Format.
    pub representation: Vec<u8>,
    /// DataFormat-Tag.
    pub data_format: DataFormat,
}

impl WriteDataPayload {
    /// Berechnet das Flag-Byte.
    #[must_use]
    pub fn flags(&self) -> u8 {
        FLAG_E_LITTLE_ENDIAN | self.data_format.to_flag_bits()
    }

    /// Verpackt in `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let flags = self.flags();
        Submessage::new(SubmessageId::WriteData, flags, self.representation)
    }

    /// Extrahiert aus `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn ID falsch oder DataFormat reserved.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        let data_format = DataFormat::from_flag_bits(sm.header.flags)?;
        Ok(Self {
            representation: sm.body.clone(),
            data_format,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn write_data_roundtrip_format_data() {
        let p = WriteDataPayload {
            representation: alloc::vec![1, 2, 3, 4],
            data_format: DataFormat::Data,
        };
        let sm = p.clone().into_submessage().unwrap();
        let p2 = WriteDataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn write_data_roundtrip_all_formats() {
        for fmt in [
            DataFormat::Data,
            DataFormat::Sample,
            DataFormat::DataSeq,
            DataFormat::SampleSeq,
            DataFormat::PackedSamples,
        ] {
            let p = WriteDataPayload {
                representation: alloc::vec![0; 8],
                data_format: fmt,
            };
            let sm = p.clone().into_submessage().unwrap();
            let p2 = WriteDataPayload::try_from_submessage(&sm).unwrap();
            assert_eq!(p2.data_format, fmt);
        }
    }

    #[test]
    fn write_data_reserved_format_rejected() {
        // 0b010 ist reserved
        let bad_flags = FLAG_E_LITTLE_ENDIAN | (0b010 << 1);
        let sm = Submessage::new(SubmessageId::WriteData, bad_flags, alloc::vec![]).unwrap();
        let res = WriteDataPayload::try_from_submessage(&sm);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }
}
