// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ACKNACK` Submessage (id=10, Spec §8.3.5.11).
//!
//! Direction: bidirektional. Header.streamId = `STREAMID_NONE`; das
//! eigentliche Stream-Tag ist im Body.
//!
//! Body-Layout:
//! ```text
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  |    first_unacked_seq_num      |   short (i16)
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  |   nack_bitmap (octet[2])      |
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  |   stream_id   |
//!  +-+-+-+-+-+-+-+-+
//! ```
//! Endianness der `i16`-Felder folgt dem E-Flag.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_i16, write_i16};
use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Body-Wire-Size: 5 Bytes.
pub const ACKNACK_BODY_SIZE: usize = 5;

/// `ACKNACK_Payload` strukturiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AckNackPayload {
    /// Erste Sequenznummer, die noch nicht acked ist.
    pub first_unacked_seq_num: i16,
    /// Bitmap der Lock-Outs (2 Bytes = 16 Bit Fenster).
    pub nack_bitmap: [u8; 2],
    /// Stream, fuer den das ACKNACK gilt.
    pub stream_id: u8,
}

impl AckNackPayload {
    /// Encodiert den Body in einen `Vec<u8>` mit gegebener Endianness.
    ///
    /// # Errors
    /// keine erwartet (Buffer wird selbst alloziert).
    pub fn encode_body(self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = alloc::vec![0u8; ACKNACK_BODY_SIZE];
        write_i16(&mut out[0..2], self.first_unacked_seq_num, e)?;
        out[2] = self.nack_bitmap[0];
        out[3] = self.nack_bitmap[1];
        out[4] = self.stream_id;
        Ok(out)
    }

    /// Decodiert aus Body-Bytes.
    ///
    /// # Errors
    /// `UnexpectedEof`, wenn weniger als 5 Bytes.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        if bytes.len() < ACKNACK_BODY_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: ACKNACK_BODY_SIZE,
                offset: bytes.len(),
            });
        }
        let first = read_i16(&bytes[0..2], e)?;
        Ok(Self {
            first_unacked_seq_num: first,
            nack_bitmap: [bytes[2], bytes[3]],
            stream_id: bytes[4],
        })
    }

    /// Verpackt in `Submessage` mit LE-Body.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::AckNack, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extrahiert aus `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`, `UnexpectedEof`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::AckNack {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not ACKNACK",
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
    fn acknack_le_roundtrip() {
        let p = AckNackPayload {
            first_unacked_seq_num: 0x1234,
            nack_bitmap: [0xAA, 0x55],
            stream_id: 0x80,
        };
        let body = p.encode_body(Endianness::Little).unwrap();
        assert_eq!(body[0], 0x34); // LE
        assert_eq!(body[1], 0x12);
        assert_eq!(body[2], 0xAA);
        assert_eq!(body[3], 0x55);
        assert_eq!(body[4], 0x80);
        let p2 = AckNackPayload::decode_body(&body, Endianness::Little).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn acknack_be_roundtrip() {
        let p = AckNackPayload {
            first_unacked_seq_num: 0x0102,
            nack_bitmap: [0xFF, 0x00],
            stream_id: 1,
        };
        let body = p.encode_body(Endianness::Big).unwrap();
        assert_eq!(body[0], 0x01);
        assert_eq!(body[1], 0x02);
        let p2 = AckNackPayload::decode_body(&body, Endianness::Big).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn acknack_roundtrip_via_submessage() {
        let p = AckNackPayload {
            first_unacked_seq_num: -1,
            nack_bitmap: [0, 0],
            stream_id: 0x80,
        };
        let sm = p.into_submessage().unwrap();
        let p2 = AckNackPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn acknack_decode_short_body_returns_eof() {
        let res = AckNackPayload::decode_body(&[0; 4], Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 5, .. })
        ));
    }
}
