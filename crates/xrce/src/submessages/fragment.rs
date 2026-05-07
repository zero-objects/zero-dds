// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `FRAGMENT` Submessage (id=13, Spec §8.3.5.14).
//!
//! Direction: bidirektional, nur in reliable Streams. Body ist opake
//! Fortsetzung der zerlegten Original-Submessage. Bit 1 der Flags
//! markiert das letzte Fragment.
//!
//! Reassembly ist State-Machine-Logik (C6.2.B); hier nur Wire-Format.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// FRAGMENT-Flag: Last-Fragment (Bit 1).
pub const FRAGMENT_FLAG_LAST: u8 = 0x02;

/// `FRAGMENT_Payload` mit Last-Marker.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FragmentPayload {
    /// Opake Bytes des Fragments.
    pub data: Vec<u8>,
    /// `true`, wenn dies das letzte Fragment ist.
    pub last_fragment: bool,
}

impl FragmentPayload {
    /// Berechnet das Flag-Byte.
    #[must_use]
    pub fn flags(&self) -> u8 {
        let mut f = FLAG_E_LITTLE_ENDIAN;
        if self.last_fragment {
            f |= FRAGMENT_FLAG_LAST;
        }
        f
    }

    /// Verpackt in `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let flags = self.flags();
        Submessage::new(SubmessageId::Fragment, flags, self.data)
    }

    /// Extrahiert aus `Submessage`.
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
