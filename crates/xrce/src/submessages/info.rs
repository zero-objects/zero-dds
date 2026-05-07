// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `INFO` Submessage (id=6, Spec §8.3.5.7).
//!
//! Direction: Agent → Client. Antwort auf `GET_INFO`. Payload =
//! `INFO_Payload : BaseObjectReply { ObjectInfo }`.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaker Body fuer `INFO`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InfoPayload {
    /// XCDR2 `BaseObjectReply { ObjectInfo }`.
    pub representation: Vec<u8>,
}

impl InfoPayload {
    /// Verpackt in `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::Info,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extrahiert aus `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Info {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not INFO",
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
    fn info_roundtrip() {
        let p = InfoPayload {
            representation: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::Info);
        let p2 = InfoPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
