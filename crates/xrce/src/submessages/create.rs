// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CREATE` Submessage (id=1, Spec §8.3.5.2).
//!
//! Direction: Client → Agent. Payload = `CREATE_Payload` (extends
//! `BaseObjectRequest`, contains `ObjectVariant`).
//!
//! Flag bits above the E-flag:
//! - Bit 1 = REUSE
//! - Bit 2 = REPLACE

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// CREATE flag: reuse an existing object (CreationMode.reuse).
pub const CREATE_FLAG_REUSE: u8 = 0x02;
/// CREATE flag: replace an existing object (CreationMode.replace).
pub const CREATE_FLAG_REPLACE: u8 = 0x04;

/// Opaque body for `CREATE`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreatePayload {
    /// XCDR2 body (`BaseObjectRequest` + `ObjectVariant`).
    pub representation: Vec<u8>,
    /// `reuse` flag (bit 1 in the submessage header flag byte).
    pub reuse: bool,
    /// `replace` flag (bit 2 in the submessage header flag byte).
    pub replace: bool,
}

impl CreatePayload {
    /// Computes the flag byte incl. the E-flag (LE).
    #[must_use]
    pub fn flags(&self) -> u8 {
        let mut f = FLAG_E_LITTLE_ENDIAN;
        if self.reuse {
            f |= CREATE_FLAG_REUSE;
        }
        if self.replace {
            f |= CREATE_FLAG_REPLACE;
        }
        f
    }

    /// Packs into a `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let flags = self.flags();
        Submessage::new(SubmessageId::Create, flags, self.representation)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is not `Create`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Create {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not CREATE",
            });
        }
        Ok(Self {
            representation: sm.body.clone(),
            reuse: (sm.header.flags & CREATE_FLAG_REUSE) != 0,
            replace: (sm.header.flags & CREATE_FLAG_REPLACE) != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn create_roundtrip_carries_reuse_replace_flags() {
        let p = CreatePayload {
            representation: alloc::vec![1, 2, 3],
            reuse: true,
            replace: true,
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.flags & CREATE_FLAG_REUSE, CREATE_FLAG_REUSE);
        assert_eq!(sm.header.flags & CREATE_FLAG_REPLACE, CREATE_FLAG_REPLACE);
        let p2 = CreatePayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn create_default_flags_have_no_reuse_no_replace() {
        let p = CreatePayload {
            representation: alloc::vec![],
            reuse: false,
            replace: false,
        };
        assert_eq!(p.flags(), FLAG_E_LITTLE_ENDIAN);
    }

    #[test]
    fn create_rejects_wrong_id() {
        let sm = Submessage::new(SubmessageId::Delete, 0, alloc::vec![]).unwrap();
        assert!(CreatePayload::try_from_submessage(&sm).is_err());
    }
}
