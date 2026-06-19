// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `GET_INFO` Submessage (id=2, Spec §8.3.5.3).
//!
//! Direction: Client → Agent. Payload = `GET_INFO_Payload`
//! (`BaseObjectRequest` + `InfoMask`). The ObjectKind in the lower 4 bits of
//! the `object_id` routes to `Root::get_info` (OBJK_AGENT) or
//! `ProxyClient::get_info`.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `GET_INFO`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GetInfoPayload {
    /// XCDR2 body.
    pub representation: Vec<u8>,
}

impl GetInfoPayload {
    /// Packs into a `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::GetInfo,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::GetInfo {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not GET_INFO",
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
    fn get_info_roundtrip() {
        let p = GetInfoPayload {
            representation: alloc::vec![0xAA; 16],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::GetInfo);
        let p2 = GetInfoPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
