// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `TIMESTAMP_REPLY` Submessage (id=15, Spec §8.3.5.16).
//!
//! Body = `transmit_timestamp + receive_timestamp + originate_timestamp`
//! = 3 * `Time_t` = 24 Bytes.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::Endianness;
use crate::error::XrceError;
use crate::submessages::timestamp::{TIME_T_WIRE_SIZE, TimePoint};
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Body-Wire-Size: 24 Bytes (3 * Time_t).
pub const TIMESTAMP_REPLY_BODY_SIZE: usize = 3 * TIME_T_WIRE_SIZE;

/// `TIMESTAMP_REPLY_Payload`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TimestampReplyPayload {
    /// Sende-Zeitstempel des Antwortenden.
    pub transmit_timestamp: TimePoint,
    /// Empfangs-Zeitstempel der urspruenglichen TIMESTAMP-Message.
    pub receive_timestamp: TimePoint,
    /// Original-Sende-Zeitstempel des Anfragenden.
    pub originate_timestamp: TimePoint,
}

impl TimestampReplyPayload {
    /// Encodiert den Body.
    ///
    /// # Errors
    /// keine erwartet.
    pub fn encode_body(self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = alloc::vec![0u8; TIMESTAMP_REPLY_BODY_SIZE];
        self.transmit_timestamp.encode(&mut out[0..8], e)?;
        self.receive_timestamp.encode(&mut out[8..16], e)?;
        self.originate_timestamp.encode(&mut out[16..24], e)?;
        Ok(out)
    }

    /// Decodiert den Body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        if bytes.len() < TIMESTAMP_REPLY_BODY_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: TIMESTAMP_REPLY_BODY_SIZE,
                offset: bytes.len(),
            });
        }
        Ok(Self {
            transmit_timestamp: TimePoint::decode(&bytes[0..8], e)?,
            receive_timestamp: TimePoint::decode(&bytes[8..16], e)?,
            originate_timestamp: TimePoint::decode(&bytes[16..24], e)?,
        })
    }

    /// Verpackt in `Submessage` (LE).
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::TimestampReply, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extrahiert aus `Submessage`.
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
        let body = p.encode_body(Endianness::Little).unwrap();
        assert_eq!(body.len(), 24);
        let p2 = TimestampReplyPayload::decode_body(&body, Endianness::Little).unwrap();
        assert_eq!(p2, p);
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
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 24, .. })
        ));
    }
}
