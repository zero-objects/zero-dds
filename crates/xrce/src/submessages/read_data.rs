// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `READ_DATA` Submessage (id=8, Spec §8.3.5.9).
//!
//! Direction: Client → Agent. Payload = `READ_DATA_Payload`
//! (extends `BaseObjectRequest` mit `ReadSpecification`).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaker Body fuer `READ_DATA`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadDataPayload {
    /// XCDR2 `BaseObjectRequest + ReadSpecification`.
    pub representation: Vec<u8>,
}

impl ReadDataPayload {
    /// Verpackt in `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::ReadData,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extrahiert aus `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::ReadData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not READ_DATA",
            });
        }
        Ok(Self {
            representation: sm.body.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn read_data_roundtrip() {
        let p = ReadDataPayload {
            representation: alloc::vec![0xAB; 32],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::ReadData);
        let p2 = ReadDataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
