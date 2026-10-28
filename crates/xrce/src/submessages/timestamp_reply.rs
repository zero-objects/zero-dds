// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `TIMESTAMP_REPLY` Submessage (id=15, Spec §8.3.5.16).
//!
//! Body = `transmit_timestamp + receive_timestamp + originate_timestamp`
//! = 3 * `Time_t` = 24 bytes.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::Endianness;
use crate::error::XrceError;
use crate::submessages::timestamp::{TIME_T_WIRE_SIZE, TimePoint};
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Body wire size: 24 bytes (3 * Time_t).
pub const TIMESTAMP_REPLY_BODY_SIZE: usize = 3 * TIME_T_WIRE_SIZE;

/// `TIMESTAMP_REPLY_Payload`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TimestampReplyPayload {
    /// Transmit timestamp of the responder.
    pub transmit_timestamp: TimePoint,
    /// Receive timestamp of the original TIMESTAMP message.
    pub receive_timestamp: TimePoint,
    /// Original transmit timestamp of the requester.
    pub originate_timestamp: TimePoint,
}

impl TimestampReplyPayload {
    /// Encodes the body.
    ///
    /// # Errors
    /// None expected.
    pub fn encode_body(self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::new();
        self.transmit_timestamp.encode_into(&mut out, e);
        self.receive_timestamp.encode_into(&mut out, e);
        self.originate_timestamp.encode_into(&mut out, e);
        Ok(out)
    }

    /// Decodes the body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        let (transmit_timestamp, at) = TimePoint::decode_at(bytes, 0, e)?;
        let (receive_timestamp, at) = TimePoint::decode_at(bytes, at, e)?;
        let (originate_timestamp, _) = TimePoint::decode_at(bytes, at, e)?;
        Ok(Self {
            transmit_timestamp,
            receive_timestamp,
            originate_timestamp,
        })
    }

    /// Packs into a `Submessage` (LE).
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::TimestampReply, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`, `UnexpectedEof`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::TimestampReply {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not TIMESTAMP_REPLY",
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
    fn timestamp_reply_roundtrip_le() {
        let p = TimestampReplyPayload {
            transmit_timestamp: TimePoint {
                seconds: 1,
                nanoseconds: 2,
            },
            receive_timestamp: TimePoint {
                seconds: 3,
                nanoseconds: 4,
            },
            originate_timestamp: TimePoint {
                seconds: 5,
                nanoseconds: 6,
            },
        };
        // Three Time_t, each APPENDABLE (Annex A gives it no @extensibility):
        // a DHEADER of 8 and the eight octets, 12 each.
        let body = p.encode_body(Endianness::Little).unwrap();
        assert_eq!(body.len(), 36);
        assert_eq!(&body[..12], &[8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
        let p2 = TimestampReplyPayload::decode_body(&body, Endianness::Little).unwrap();
        assert_eq!(p2, p);
        // The reference implementation's form: 3 * 8 octets.
        crate::encoding::with_unannotated_types(crate::encoding::UnannotatedTypes::Final, || {
            let body = p.encode_body(Endianness::Little).unwrap();
            assert_eq!(body.len(), TIMESTAMP_REPLY_BODY_SIZE);
            assert_eq!(
                TimestampReplyPayload::decode_body(&body, Endianness::Little).unwrap(),
                p
            );
        });
    }

    #[test]
    fn timestamp_reply_roundtrip_via_submessage() {
        let p = TimestampReplyPayload::default();
        let sm = p.into_submessage().unwrap();
        let p2 = TimestampReplyPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn timestamp_reply_decode_short_returns_eof() {
        let res = TimestampReplyPayload::decode_body(&[0; 23], Endianness::Little);
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }
}
