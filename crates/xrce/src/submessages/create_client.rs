// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CREATE_CLIENT` Submessage (id=0, Spec §8.3.5.1).
//!
//! Direction: Client → Agent. The payload is an XCDR2-encoded
//! `CLIENT_Representation` (xrce_cookie + version + vendor + timestamp
//! + client_key + session_id + optional PropertySeq).
//!
//! In C6.2.A this is treated as opaque bytes; the inner structure
//! follows in C6.2.B.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `CREATE_CLIENT`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreateClientPayload {
    /// XCDR2 bytes of the `CLIENT_Representation`.
    pub representation: Vec<u8>,
}

impl CreateClientPayload {
    /// Packs the payload into a `Submessage` with the LE body flag.
    ///
    /// # Errors
    /// `PayloadTooLarge` if the body is > `u16::MAX`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::CreateClient,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extracts the body from a submessage. Only validates that
    /// the ID matches — the inner XCDR2 structure is out of scope.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the submessage ID is not `CreateClient`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::CreateClient {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not CREATE_CLIENT",
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
    fn create_client_roundtrip_via_submessage() {
        let p = CreateClientPayload {
            representation: alloc::vec![b'X', b'R', b'C', b'E', 1, 0],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::CreateClient);
        let p2 = CreateClientPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn create_client_rejects_wrong_submessage_id() {
        let sm = Submessage::new(SubmessageId::Status, 0, alloc::vec![]).unwrap();
        let res = CreateClientPayload::try_from_submessage(&sm);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }
}
