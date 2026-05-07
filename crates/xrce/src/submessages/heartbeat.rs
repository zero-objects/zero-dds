// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `HEARTBEAT` Submessage (id=11, Spec §8.3.5.12).
//!
//! Direction: bidirektional. Header.streamId = `STREAMID_NONE`; das
//! Stream-Tag ist im Body.
//!
//! Body-Layout (5 Bytes):
//! ```text
//!  +---------------+---------------+
//!  | first_unacked_seq_nr (i16)    |
//!  +---------------+---------------+
//!  | last_unacked_seq_nr  (i16)    |
//!  +---------------+---------------+
//!  | stream_id (u8) |
//!  +----------------+
//! ```

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_i16, write_i16};
use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Body-Wire-Size: 5 Bytes.
pub const HEARTBEAT_BODY_SIZE: usize = 5;

/// `HEARTBEAT_Payload` strukturiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HeartbeatPayload {
    /// Erste Sequenznummer im Sender-Window.
    pub first_unacked_seq_nr: i16,
    /// Letzte Sequenznummer im Sender-Window.
    pub last_unacked_seq_nr: i16,
    /// Stream-Tag.
    pub stream_id: u8,
}

impl HeartbeatPayload {
    /// Encodiert den Body.
    ///
    /// # Errors
    /// keine erwartet.
    pub fn encode_body(self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = alloc::vec![0u8; HEARTBEAT_BODY_SIZE];
        write_i16(&mut out[0..2], self.first_unacked_seq_nr, e)?;
        write_i16(&mut out[2..4], self.last_unacked_seq_nr, e)?;
        out[4] = self.stream_id;
        Ok(out)
    }

    /// Decodiert den Body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        if bytes.len() < HEARTBEAT_BODY_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: HEARTBEAT_BODY_SIZE,
                offset: bytes.len(),
            });
        }
        let first = read_i16(&bytes[0..2], e)?;
        let last = read_i16(&bytes[2..4], e)?;
        Ok(Self {
            first_unacked_seq_nr: first,
            last_unacked_seq_nr: last,
            stream_id: bytes[4],
        })
    }

    /// Verpackt in `Submessage` (LE).
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::Heartbeat, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extrahiert aus `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`, `UnexpectedEof`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Heartbeat {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not HEARTBEAT",
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
    fn heartbeat_le_roundtrip() {
        let p = HeartbeatPayload {
            first_unacked_seq_nr: 5,
            last_unacked_seq_nr: 100,
            stream_id: 0x80,
        };
        let body = p.encode_body(Endianness::Little).unwrap();
        let p2 = HeartbeatPayload::decode_body(&body, Endianness::Little).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn heartbeat_be_roundtrip() {
        let p = HeartbeatPayload {
            first_unacked_seq_nr: 0x0102,
            last_unacked_seq_nr: 0x0304,
            stream_id: 0x81,
        };
        let body = p.encode_body(Endianness::Big).unwrap();
        assert_eq!(body[0], 0x01);
        assert_eq!(body[1], 0x02);
        assert_eq!(body[2], 0x03);
        assert_eq!(body[3], 0x04);
        let p2 = HeartbeatPayload::decode_body(&body, Endianness::Big).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn heartbeat_roundtrip_via_submessage() {
        let p = HeartbeatPayload {
            first_unacked_seq_nr: -10,
            last_unacked_seq_nr: 10,
            stream_id: 0x80,
        };
        let sm = p.into_submessage().unwrap();
        let p2 = HeartbeatPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn heartbeat_decode_short_body_returns_eof() {
        let res = HeartbeatPayload::decode_body(&[0; 4], Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 5, .. })
        ));
    }
}
