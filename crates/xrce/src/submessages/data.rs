// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DATA` Submessage (id=9, Spec §8.3.5.10).
//!
//! Direction: Agent → Client. Reply to `READ_DATA` on `STATUS_OK`.
//! Flag bits 1..3 = `DataFormat`, analogous to `WRITE_DATA`. The body extends
//! `RelatedObjectRequest` + a data-specific variant.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::write_data::DataFormat;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `DATA`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataPayload {
    /// XCDR2 body.
    pub representation: Vec<u8>,
    /// DataFormat tag.
    pub data_format: DataFormat,
}

impl DataPayload {
    /// Computes the flag byte.
    #[must_use]
    pub fn flags(&self) -> u8 {
        FLAG_E_LITTLE_ENDIAN | self.data_format.to_flag_bits()
    }

    /// Packs into a `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let flags = self.flags();
        Submessage::new(SubmessageId::Data, flags, self.representation)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
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
    fn data_roundtrip_with_packed_samples_format() {
        let p = DataPayload {
            representation: alloc::vec![0; 8],
            data_format: DataFormat::PackedSamples,
        };
        let sm = p.clone().into_submessage().unwrap();
        let p2 = DataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn data_roundtrip_with_sample_seq_format() {
        let p = DataPayload {
            representation: alloc::vec![0xCC; 16],
            data_format: DataFormat::SampleSeq,
        };
        let sm = p.clone().into_submessage().unwrap();
        let p2 = DataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
