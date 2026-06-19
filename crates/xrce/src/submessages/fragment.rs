// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `FRAGMENT` Submessage (id=13, Spec §8.3.5.14).
//!
//! Direction: bidirectional, only in reliable streams. The body is an
//! opaque continuation of the split original submessage. Bit 1 of the flags
//! marks the last fragment.
//!
//! Reassembly is state-machine logic (C6.2.B); here only the wire format.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// FRAGMENT flag: last fragment (bit 1).
pub const FRAGMENT_FLAG_LAST: u8 = 0x02;

/// `FRAGMENT_Payload` with a last marker.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FragmentPayload {
    /// Opaque bytes of the fragment.
    pub data: Vec<u8>,
    /// `true` if this is the last fragment.
    pub last_fragment: bool,
}

impl FragmentPayload {
    /// Computes the flag byte.
    #[must_use]
    pub fn flags(&self) -> u8 {
        let mut f = FLAG_E_LITTLE_ENDIAN;
        if self.last_fragment {
            f |= FRAGMENT_FLAG_LAST;
        }
        f
    }

    /// Packs into a `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let flags = self.flags();
        Submessage::new(SubmessageId::Fragment, flags, self.data)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Fragment {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not FRAGMENT",
            });
        }
        Ok(Self {
            data: sm.body.clone(),
            last_fragment: (sm.header.flags & FRAGMENT_FLAG_LAST) != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn fragment_roundtrip_carries_last_flag() {
        let p = FragmentPayload {
            data: alloc::vec![1, 2, 3, 4],
            last_fragment: true,
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_ne!(sm.header.flags & FRAGMENT_FLAG_LAST, 0);
        let p2 = FragmentPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn fragment_intermediate_has_no_last_flag() {
        let p = FragmentPayload {
            data: alloc::vec![0xFF; 100],
            last_fragment: false,
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.flags & FRAGMENT_FLAG_LAST, 0);
        let p2 = FragmentPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
