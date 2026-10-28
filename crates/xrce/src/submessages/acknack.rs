// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ACKNACK` Submessage (id=10, Spec §8.3.5.11).
//!
//! Direction: bidirectional. Header.streamId = `STREAMID_NONE`; the
//! actual stream tag is in the body.
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
//! The endianness of the `i16` fields follows the E-flag.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_i16, write_i16};
use crate::error::XrceError;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Size of the members: 5 bytes (a DHEADER of 4 comes in front unless
/// `UnannotatedTypes::Final` is set).
pub const ACKNACK_BODY_SIZE: usize = 5;

/// `ACKNACK_Payload`, structured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AckNackPayload {
    /// First sequence number that is not yet acked.
    pub first_unacked_seq_num: i16,
    /// Bitmap of the lock-outs (2 bytes = 16-bit window).
    pub nack_bitmap: [u8; 2],
    /// Stream the ACKNACK applies to.
    pub stream_id: u8,
}

impl AckNackPayload {
    /// Encodes the body into a `Vec<u8>` with the given endianness.
    ///
    /// # Errors
    /// None expected (the buffer is allocated internally).
    pub fn encode_body(self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        // ACKNACK_Payload carries no @extensibility (Annex A): APPENDABLE,
        // a DHEADER in front, unless the setting is `Final`.
        let mut out = Vec::new();
        let members = crate::encoding::begin_unannotated(&mut out);
        let at = out.len();
        out.resize(at + ACKNACK_BODY_SIZE, 0);
        write_i16(&mut out[at..at + 2], self.first_unacked_seq_num, e)?;
        out[at + 2] = self.nack_bitmap[0];
        out[at + 3] = self.nack_bitmap[1];
        out[at + 4] = self.stream_id;
        crate::encoding::end_unannotated(&mut out, members, e);
        Ok(out)
    }

    /// Decodes from body bytes (either form, the one of the setting first).
    ///
    /// # Errors
    /// `UnexpectedEof` if the bytes hold neither form.
    pub fn decode_body(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        crate::encoding::decode_unannotated(bytes, 0, e, ACKNACK_BODY_SIZE, |m| {
            Ok(Self {
                first_unacked_seq_num: read_i16(&m[0..2], e)?,
                nack_bitmap: [m[2], m[3]],
                stream_id: m[4],
            })
        })
        .map(|(payload, _)| payload)
    }

    /// Packs into a `Submessage` with an LE body.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::AckNack, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extracts from a `Submessage`.
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

    /// Annex A gives ACKNACK_Payload no @extensibility: APPENDABLE, a DHEADER
    /// of 5 (the members) in front, in the byte order of the E flag.
    #[test]
    fn acknack_le_roundtrip() {
        let p = AckNackPayload {
            first_unacked_seq_num: 0x1234,
            nack_bitmap: [0xAA, 0x55],
            stream_id: 0x80,
        };
        let body = p.encode_body(Endianness::Little).unwrap();
        assert_eq!(body, [5, 0, 0, 0, 0x34, 0x12, 0xAA, 0x55, 0x80]);
        let p2 = AckNackPayload::decode_body(&body, Endianness::Little).unwrap();
        assert_eq!(p2, p);
        // The form of the reference implementation: the five octets.
        crate::encoding::with_unannotated_types(crate::encoding::UnannotatedTypes::Final, || {
            let body = p.encode_body(Endianness::Little).unwrap();
            assert_eq!(body, [0x34, 0x12, 0xAA, 0x55, 0x80]);
            assert_eq!(
                AckNackPayload::decode_body(&body, Endianness::Little).unwrap(),
                p
            );
        });
        // Either form is read whatever the setting is.
        assert_eq!(
            AckNackPayload::decode_body(&[0x34, 0x12, 0xAA, 0x55, 0x80], Endianness::Little)
                .unwrap(),
            p
        );
    }

    #[test]
    fn acknack_be_roundtrip() {
        let p = AckNackPayload {
            first_unacked_seq_num: 0x0102,
            nack_bitmap: [0xFF, 0x00],
            stream_id: 1,
        };
        let body = p.encode_body(Endianness::Big).unwrap();
        assert_eq!(body, [0, 0, 0, 5, 0x01, 0x02, 0xFF, 0x00, 1]);
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
