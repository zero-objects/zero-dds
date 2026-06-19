// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `TIMESTAMP` Submessage (id=14, Spec §8.3.5.15) and shared
//! `Time_t` helper (Annex A).
//!
//! `Time_t = { long seconds; unsigned long nanoseconds; }` — 8-byte
//! body in the endianness of the E-flag.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Wire size of a `Time_t` field.
pub const TIME_T_WIRE_SIZE: usize = 8;

/// `Time_t` representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TimePoint {
    /// Seconds (signed).
    pub seconds: i32,
    /// Nanoseconds (unsigned).
    pub nanoseconds: u32,
}

impl TimePoint {
    /// Encodes into an 8-byte slice.
    ///
    /// # Errors
    /// `WriteOverflow`.
    pub fn encode(self, out: &mut [u8], e: Endianness) -> Result<(), XrceError> {
        if out.len() < TIME_T_WIRE_SIZE {
            return Err(XrceError::WriteOverflow {
                needed: TIME_T_WIRE_SIZE,
                available: out.len(),
            });
        }
        write_u32(&mut out[0..4], self.seconds as u32, e)?;
        write_u32(&mut out[4..8], self.nanoseconds, e)?;
        Ok(())
    }

    /// Decodes from an 8-byte slice.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        if bytes.len() < TIME_T_WIRE_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: TIME_T_WIRE_SIZE,
                offset: bytes.len(),
            });
        }
        let seconds = read_u32(&bytes[0..4], e)? as i32;
        let nanoseconds = read_u32(&bytes[4..8], e)?;
        Ok(Self {
            seconds,
            nanoseconds,
        })
    }
}

/// `TIMESTAMP_Payload`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TimestampPayload {
    /// Transmit timestamp.
    pub transmit_timestamp: TimePoint,
}

impl TimestampPayload {
    /// Encodes the body.
    ///
    /// # Errors
    /// None expected.
    pub fn encode_body(self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = alloc::vec![0u8; TIME_T_WIRE_SIZE];
        self.transmit_timestamp.encode(&mut out, e)?;
        Ok(out)
    }

    /// Decodes the body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        Ok(Self {
            transmit_timestamp: TimePoint::decode(bytes, e)?,
        })
    }

    /// Packs into a `Submessage` (LE).
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::Timestamp, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`, `UnexpectedEof`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Timestamp {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not TIMESTAMP",
            });
        }
        Self::decode_body(&sm.body, sm.header.body_endianness())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn time_t_roundtrip_le() {
        let t = TimePoint {
            seconds: 1_700_000_000,
            nanoseconds: 999_999_999,
        };
        let mut buf = [0u8; 8];
        t.encode(&mut buf, Endianness::Little).unwrap();
        let t2 = TimePoint::decode(&buf, Endianness::Little).unwrap();
        assert_eq!(t2, t);
    }

    #[test]
    fn time_t_roundtrip_be() {
        let t = TimePoint {
            seconds: -1,
            nanoseconds: 0,
        };
        let mut buf = [0u8; 8];
        t.encode(&mut buf, Endianness::Big).unwrap();
        // -1 as u32 = 0xFFFF_FFFF → BE: all 0xFF
        assert_eq!(buf[0..4], [0xFF, 0xFF, 0xFF, 0xFF]);
        let t2 = TimePoint::decode(&buf, Endianness::Big).unwrap();
        assert_eq!(t2, t);
    }

    #[test]
    fn timestamp_roundtrip_via_submessage() {
        let p = TimestampPayload {
            transmit_timestamp: TimePoint {
                seconds: 42,
                nanoseconds: 12345,
            },
        };
        let sm = p.into_submessage().unwrap();
        let p2 = TimestampPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn timestamp_decode_short_body_returns_eof() {
        let res = TimestampPayload::decode_body(&[0; 7], Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 8, .. })
        ));
    }
}
