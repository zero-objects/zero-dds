// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RESET` Submessage (id=12, Spec §8.3.5.13).
//!
//! Empty payload. Resets the session_id state.

extern crate alloc;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// `RESET_Payload` — empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ResetPayload;

impl ResetPayload {
    /// Packs into a `Submessage` with an empty body.
    ///
    /// # Errors
    /// None expected.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        Submessage::new(SubmessageId::Reset, FLAG_E_LITTLE_ENDIAN, alloc::vec![])
    }

    /// Extracts from a `Submessage`. The body must be empty.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the body is not empty.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Reset {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not RESET",
            });
        }
        if !sm.body.is_empty() {
            return Err(XrceError::ValueOutOfRange {
                message: "RESET body must be empty",
            });
        }
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn reset_roundtrip_empty_body() {
        let sm = ResetPayload.into_submessage().unwrap();
        assert_eq!(sm.body.len(), 0);
        assert_eq!(sm.header.submessage_length, 0);
        let p2 = ResetPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, ResetPayload);
    }

    #[test]
    fn reset_rejects_nonempty_body() {
        let sm = Submessage::new(SubmessageId::Reset, 0, alloc::vec![1, 2]).unwrap();
        let res = ResetPayload::try_from_submessage(&sm);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }
}
