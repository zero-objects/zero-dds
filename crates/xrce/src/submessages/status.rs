// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `STATUS` Submessage (id=5, Spec §8.3.5.6).
//!
//! Direction: Agent → Client. Reply to CREATE/UPDATE/DELETE and
//! to `READ_DATA` on error. Payload = `STATUS_Payload` (extends
//! `BaseObjectReply`).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `STATUS`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusPayload {
    /// XCDR2 `BaseObjectReply`.
    pub representation: Vec<u8>,
}

impl StatusPayload {
    /// Packs into a `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::Status,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Status {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not STATUS",
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
    fn status_roundtrip() {
        let p = StatusPayload {
            representation: alloc::vec![0u8; 12],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::Status);
        let p2 = StatusPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
