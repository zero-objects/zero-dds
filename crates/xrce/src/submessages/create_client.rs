// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CREATE_CLIENT` Submessage (id=0, Spec §8.3.5.1).
//!
//! Direction: Client → Agent. Payload ist ein XCDR2-encodetes
//! `CLIENT_Representation` (xrce_cookie + version + vendor + timestamp
//! + client_key + session_id + optional PropertySeq).
//!
//! In C6.2.A wird das als opake Bytes behandelt; die innere Struktur
//! folgt in C6.2.B.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaker Body fuer `CREATE_CLIENT`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreateClientPayload {
    /// XCDR2-Bytes des `CLIENT_Representation`.
    pub representation: Vec<u8>,
}

impl CreateClientPayload {
    /// Verpackt den Payload in eine `Submessage` mit LE-Body-Flag.
    ///
    /// # Errors
    /// `PayloadTooLarge`, wenn der Body > `u16::MAX` ist.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::CreateClient,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extrahiert den Body aus einer Submessage. Validiert nur, dass
    /// die ID stimmt — die innere XCDR2-Struktur ist out-of-scope.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn die Submessage-ID nicht `CreateClient` ist.
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
