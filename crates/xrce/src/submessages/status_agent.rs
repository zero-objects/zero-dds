// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `STATUS_AGENT` Submessage (id=4, Spec §8.3.5.5).
//!
//! Direction: Agent → Client. Reply to `CREATE_CLIENT`. Payload =
//! `STATUS_AGENT_Payload { AGENT_Representation }` with
//! `xrce_cookie = 'X','R','C','E'`.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `STATUS_AGENT`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusAgentPayload {
    /// XCDR2 `AGENT_Representation`.
    pub representation: Vec<u8>,
}

impl StatusAgentPayload {
    /// Packs into a `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(
            SubmessageId::StatusAgent,
            FLAG_E_LITTLE_ENDIAN,
            self.representation,
        )
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::StatusAgent {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not STATUS_AGENT",
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
    fn status_agent_roundtrip() {
        let p = StatusAgentPayload {
            representation: alloc::vec![b'X', b'R', b'C', b'E'],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::StatusAgent);
        let p2 = StatusAgentPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
