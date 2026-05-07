// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DELETE` Submessage (id=3, Spec §8.3.5.4).
//!
//! Direction: Client → Agent. Bei `OBJK_CLIENT` → `Root::delete_client`,
//! sonst `ProxyClient::delete`.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaker Body fuer `DELETE`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeletePayload {
    /// XCDR2 `BaseObjectRequest`.
    pub representation: Vec<u8>,
}

impl DeletePayload {
    /// Verpackt in `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::Delete,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extrahiert aus `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Delete {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DELETE",
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
    fn delete_roundtrip() {
        let p = DeletePayload {
            representation: alloc::vec![0xDE, 0xAD, 0xBE, 0xEF],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::Delete);
        let p2 = DeletePayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
