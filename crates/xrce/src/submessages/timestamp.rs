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

impl TimePoint {
    /// Appends the `Time_t` to `out`, which starts the payload. `Time_t`
    /// carries no `@extensibility` (Annex A): `APPENDABLE` with a DHEADER
    /// unless [`crate::encoding::UnannotatedTypes::Final`] is set.
    pub fn encode_into(self, out: &mut Vec<u8>, e: Endianness) {
        let members = crate::encoding::begin_unannotated(out);
        let mut buf = [0u8; TIME_T_WIRE_SIZE];
        // A buffer of eight octets always holds a Time_t.
        let _ = self.encode(&mut buf, e);
        out.extend_from_slice(&buf);
        crate::encoding::end_unannotated(out, members, e);
    }

    /// Decodes the `Time_t` that starts at `pos` of `bytes`, which start the
    /// payload; returns it and the position after it (either form, the one
    /// of the setting first).
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        crate::encoding::decode_unannotated(bytes, pos, e, TIME_T_WIRE_SIZE, |m| Self::decode(m, e))
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
        let mut out = Vec::new();
        self.transmit_timestamp.encode_into(&mut out, e);
        Ok(out)
    }

    /// Decodes the body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        Ok(Self {
            transmit_timestamp: TimePoint::decode_at(bytes, 0, e)?.0,
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

    /// `Time_t` carries no @extensibility (Annex A): APPENDABLE, a DHEADER of
    /// 8 in front of the eight octets; the reference implementation's form
    /// has none.
    #[test]
    fn a_timestamp_body_has_the_dheader_of_an_appendable_time_t() {
        let p = TimestampPayload {
            transmit_timestamp: TimePoint {
                seconds: 1,
                nanoseconds: 2,
            },
        };
        let body = p.encode_body(Endianness::Little).unwrap();
        assert_eq!(body, [8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(
            TimestampPayload::decode_body(&body, Endianness::Little).unwrap(),
            p
        );
        crate::encoding::with_unannotated_types(crate::encoding::UnannotatedTypes::Final, || {
            let body = p.encode_body(Endianness::Little).unwrap();
            assert_eq!(body, [1, 0, 0, 0, 2, 0, 0, 0]);
            assert_eq!(
                TimestampPayload::decode_body(&body, Endianness::Little).unwrap(),
                p
            );
        });
        assert_eq!(
            TimestampPayload::decode_body(&[1, 0, 0, 0, 2, 0, 0, 0], Endianness::Little).unwrap(),
            p
        );
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
