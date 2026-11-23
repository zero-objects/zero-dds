// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Stream framing by the HeaderExtension (`zerodds-tcp-transport-1.0`
//! §2.5), the `rtps` dialect.
//!
//! DDSI-RTPS 2.5 §8.3.3: "a stream-oriented transport (like TCP) would
//! need to include the length in the RTPS HeaderExtension in order to
//! identify the boundary of the RTPS message". In this dialect a
//! connection carries RTPS messages back to back, with no handshake (§3)
//! and no length in front of a message (§2.1). Every message begins with
//! these 28 octets:
//!
//! ```text
//!  0  'R' 'T' 'P' 'S'                 RTPS header (§9.4.4), 20 octets
//!  4  major minor                     protocol version, 2.5 or above
//!  6  vendorId (2)
//!  8  guidPrefix (12)
//! 20  0x00                            submessageId RTPS_HE (§9.4.5.1.1)
//! 21  flags                           E = 0x01, L = 0x02 set
//! 22  octetsToNextHeader (u16)        in the byte order of E, 4 or more
//! 24  messageLength (u32)             in the byte order of E (§9.4.5.2)
//! ```
//!
//! The messageLength counts the whole message from the first octet of the
//! RTPS header (§8.3.7.5); the HeaderExtension is the first submessage
//! (§8.3.7.3). The reader takes the 28 octets, then `messageLength - 28`
//! more, and hands the message on as it is, HeaderExtension included. The
//! writer puts nothing in front and changes nothing: a message that does
//! not state its own length is refused.

extern crate alloc;

use std::io::{Read, Write};

use crate::framing::{FramingError, MAX_FRAME_SIZE, read_exact_or_eof, read_head};

/// The octets a message of the `rtps` dialect begins with: RTPS header
/// (20), submessage header of the HeaderExtension (4), messageLength (4).
pub const STREAM_HEAD_LEN: usize = 28;

/// Offset of the HeaderExtension's submessage header in the message.
const HE_AT: usize = 20;
/// SubmessageId of the HeaderExtension (DDSI-RTPS 2.5 §9.4.5.1.1).
const SUBMESSAGE_ID_HEADER_EXTENSION: u8 = 0x00;
/// E flag: the submessage is little-endian.
const FLAG_E: u8 = 0x01;
/// L flag: the HeaderExtension carries the messageLength.
const FLAG_L: u8 = 0x02;

/// The messageLength the head of a message states.
///
/// # Errors
/// `NotRtps` for a head that does not begin with `RTPS`;
/// `NoMessageLength` for a protocol version below 2.5, a first submessage
/// that is no HeaderExtension, one without the L flag or one whose
/// octetsToNextHeader has no room for the field; `LengthTooSmall` for a
/// messageLength below the RTPS header and the HeaderExtension;
/// `FrameTooLarge` for one above [`MAX_FRAME_SIZE`].
pub fn message_length(head: &[u8; STREAM_HEAD_LEN]) -> Result<u32, FramingError> {
    let magic = [head[0], head[1], head[2], head[3]];
    if &magic != b"RTPS" {
        return Err(FramingError::NotRtps { got: magic });
    }
    // The submessage id 0x00 is the HeaderExtension from 2.5 on.
    if (head[4], head[5]) < (2, 5)
        || head[HE_AT] != SUBMESSAGE_ID_HEADER_EXTENSION
        || head[HE_AT + 1] & FLAG_L == 0
    {
        return Err(FramingError::NoMessageLength);
    }
    let little_endian = head[HE_AT + 1] & FLAG_E != 0;
    let octets = [head[HE_AT + 2], head[HE_AT + 3]];
    let octets_to_next_header = if little_endian {
        u16::from_le_bytes(octets)
    } else {
        u16::from_be_bytes(octets)
    };
    if octets_to_next_header < 4 {
        return Err(FramingError::NoMessageLength);
    }
    let length = [head[24], head[25], head[26], head[27]];
    let announced = if little_endian {
        u32::from_le_bytes(length)
    } else {
        u32::from_be_bytes(length)
    };
    // §8.3.7.3: "The messageLength is too small to fit the RTPS Header and
    // the HeaderExtension."
    if announced < 24 + u32::from(octets_to_next_header) {
        return Err(FramingError::LengthTooSmall { announced });
    }
    if announced > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge { announced });
    }
    Ok(announced)
}

/// Reads one message of the `rtps` dialect: the 28 octets of its head and
/// the rest its messageLength states. The message is returned as it was
/// sent, HeaderExtension included.
///
/// # Errors
/// `Closed` if the stream ends before the first octet of a message,
/// `UnexpectedEof` if it ends inside one, the errors of
/// [`message_length`], I/O error. After any error but `Closed` the stream
/// has no known message boundary and is to be closed.
pub fn read_message<R: Read>(r: &mut R) -> Result<alloc::vec::Vec<u8>, FramingError> {
    let mut head = [0u8; STREAM_HEAD_LEN];
    read_head(r, &mut head)?;
    read_body(&head, r)
}

/// Reads the rest of a message whose 28-octet `head` has been read, and
/// returns the whole message, `head` included.
///
/// # Errors
/// The errors of [`message_length`]; `UnexpectedEof` if the stream ends
/// inside the message; I/O error.
pub fn read_body<R: Read>(
    head: &[u8; STREAM_HEAD_LEN],
    r: &mut R,
) -> Result<alloc::vec::Vec<u8>, FramingError> {
    let length = message_length(head)? as usize;
    let mut message = alloc::vec![0u8; length];
    message[..STREAM_HEAD_LEN].copy_from_slice(head);
    read_exact_or_eof(r, &mut message[STREAM_HEAD_LEN..])?;
    Ok(message)
}

/// Whether `message` can go on a stream of the `rtps` dialect: it states
/// its own length in a HeaderExtension directly behind the RTPS header.
///
/// # Errors
/// `NoMessageLength` for a message shorter than its head, the errors of
/// [`message_length`], `LengthMismatch` if the messageLength is not the
/// length of `message`.
pub fn check_message(message: &[u8]) -> Result<(), FramingError> {
    let Some(head) = message.first_chunk::<STREAM_HEAD_LEN>() else {
        return Err(match message.first_chunk::<4>() {
            Some(magic) if magic != b"RTPS" => FramingError::NotRtps { got: *magic },
            _ => FramingError::NoMessageLength,
        });
    };
    let announced = message_length(head)?;
    if announced as usize != message.len() {
        return Err(FramingError::LengthMismatch {
            announced,
            actual: message.len(),
        });
    }
    Ok(())
}

/// Writes one message of the `rtps` dialect, as it is.
///
/// # Errors
/// The errors of [`check_message`], before an octet is written; I/O
/// error.
pub fn write_message<W: Write>(w: &mut W, message: &[u8]) -> Result<(), FramingError> {
    check_message(message)?;
    w.write_all(message)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use std::io::Cursor;

    /// An RTPS 2.5 message by the octets of DDSI-RTPS 2.5 §9.4.4 and
    /// §9.4.5.2: header of vendor `vendor`, a HeaderExtension with only
    /// the messageLength in the byte order asked for, then `rest`.
    pub(crate) fn stream_message(vendor: [u8; 2], little_endian: bool, rest: &[u8]) -> Vec<u8> {
        let total = u32::try_from(STREAM_HEAD_LEN + rest.len()).unwrap();
        let mut m = vec![b'R', b'T', b'P', b'S', 2, 5, vendor[0], vendor[1]];
        m.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        if little_endian {
            m.extend_from_slice(&[0x00, 0x03, 4, 0]);
            m.extend_from_slice(&total.to_le_bytes());
        } else {
            m.extend_from_slice(&[0x00, 0x02, 0, 4]);
            m.extend_from_slice(&total.to_be_bytes());
        }
        m.extend_from_slice(rest);
        m
    }

    /// INFO_TS (0x09), little-endian, 8 octets.
    pub(crate) const INFO_TS: [u8; 12] = [0x09, 0x01, 8, 0, 1, 0, 0, 0, 2, 0, 0, 0];

    #[test]
    fn tcp_2_5_reads_a_message_delimited_by_header_extension_length() {
        let message = stream_message([0x01, 0xF0], true, &INFO_TS);
        assert_eq!(message.len(), 40);
        // The octets of the layout: id 0x00, flags E|L, 4, length 40 LE.
        assert_eq!(message[20..28], [0x00, 0x03, 4, 0, 40, 0, 0, 0]);
        // Octets of a next message behind it stay in the stream.
        let mut wire = message.clone();
        wire.extend_from_slice(b"RT");
        let mut r = Cursor::new(wire);
        assert_eq!(read_message(&mut r).unwrap(), message);
        assert_eq!(r.position(), 40);
        // The message is the one zerodds-rtps decodes, HeaderExtension
        // and length included.
        let he = zerodds_rtps::header_extension::HeaderExtension::decode(&message[20..]).unwrap();
        assert_eq!(he.message_length, Some(40));
        assert!(zerodds_rtps::datagram::decode_datagram(&message).is_ok());
    }

    #[test]
    fn tcp_2_5_reads_a_big_endian_message_length() {
        let message = stream_message([0x01, 0xF0], false, &[0u8; 300]);
        assert_eq!(message[20..28], [0x00, 0x02, 0, 4, 0, 0, 0x01, 0x48]);
        assert_eq!(read_message(&mut Cursor::new(&message)).unwrap(), message);
        // Read as little-endian the same four octets were 0x4801_0000,
        // above the limit.
        let mut le = message.clone();
        le[21] = 0x03;
        le[22..24].copy_from_slice(&[4, 0]);
        assert!(matches!(
            read_message(&mut Cursor::new(&le)),
            Err(FramingError::FrameTooLarge {
                announced: 0x4801_0000
            })
        ));
    }

    /// A HeaderExtension with more fields behind the messageLength: the
    /// length is still the first field of the body (§9.4.5.2).
    #[test]
    fn tcp_2_5_reads_a_header_extension_with_further_fields() {
        // flags E|L|T, octetsToNextHeader 12: length, then the timestamp.
        let mut message = vec![b'R', b'T', b'P', b'S', 2, 5, 0x01, 0xF0];
        message.extend_from_slice(&[0u8; 12]);
        message.extend_from_slice(&[0x00, 0x07, 12, 0, 48, 0, 0, 0]);
        message.extend_from_slice(&[7, 0, 0, 0, 9, 0, 0, 0]);
        message.extend_from_slice(&INFO_TS);
        assert_eq!(message.len(), 48);
        assert_eq!(read_message(&mut Cursor::new(&message)).unwrap(), message);
        assert!(check_message(&message).is_ok());
        // A length that leaves no room for the extension it stands in.
        message[24] = 35;
        assert!(matches!(
            read_message(&mut Cursor::new(&message)),
            Err(FramingError::LengthTooSmall { announced: 35 })
        ));
    }

    #[test]
    fn tcp_2_5_refuses_a_stream_message_without_length() {
        let good = stream_message([0x01, 0xF0], true, &INFO_TS);
        // No HeaderExtension: the first submessage is INFO_TS.
        let mut plain = good[..20].to_vec();
        plain.extend_from_slice(&INFO_TS);
        plain.extend_from_slice(&INFO_TS);
        assert!(matches!(
            read_message(&mut Cursor::new(&plain)),
            Err(FramingError::NoMessageLength)
        ));
        // A HeaderExtension without the L flag.
        let mut no_l = good.clone();
        no_l[21] = 0x01;
        assert!(matches!(
            read_message(&mut Cursor::new(&no_l)),
            Err(FramingError::NoMessageLength)
        ));
        // The L flag, but no room for the field.
        let mut no_room = good.clone();
        no_room[22] = 0;
        assert!(matches!(
            read_message(&mut Cursor::new(&no_room)),
            Err(FramingError::NoMessageLength)
        ));
        // Protocol version 2.4: the id 0x00 is no HeaderExtension there.
        let mut old = good.clone();
        old[5] = 4;
        assert!(matches!(
            read_message(&mut Cursor::new(&old)),
            Err(FramingError::NoMessageLength)
        ));
        // No RTPS message: the request of §3.1.
        let mut handshake = b"ZDDS".to_vec();
        handshake.extend_from_slice(&[0u8; 24]);
        assert!(matches!(
            read_message(&mut Cursor::new(&handshake)),
            Err(FramingError::NotRtps { got }) if &got == b"ZDDS"
        ));
        // The same on the sending side, with nothing written.
        for bad in [&plain, &no_l, &no_room, &old, &handshake] {
            let mut out = Vec::new();
            assert!(write_message(&mut out, bad).is_err());
            assert!(out.is_empty());
        }
        let mut out = Vec::new();
        write_message(&mut out, &good).unwrap();
        assert_eq!(out, good);
    }

    #[test]
    fn tcp_2_5_refuses_a_message_length_below_28() {
        for announced in [0u32, 1, 20, 27] {
            let mut message = stream_message([0x01, 0xF0], true, &INFO_TS);
            message[24..28].copy_from_slice(&announced.to_le_bytes());
            match read_message(&mut Cursor::new(&message)) {
                Err(FramingError::LengthTooSmall { announced: got }) => assert_eq!(got, announced),
                other => panic!("{announced}: {other:?}"),
            }
        }
        // 28 is the message of head alone.
        let empty = stream_message([0x01, 0xF0], true, &[]);
        assert_eq!(empty.len(), 28);
        assert_eq!(read_message(&mut Cursor::new(&empty)).unwrap(), empty);
        // Above the limit: refused on the length, before the body is read
        // or allocated.
        let mut big = empty.clone();
        big[24..28].copy_from_slice(&(MAX_FRAME_SIZE + 1).to_le_bytes());
        assert!(matches!(
            read_message(&mut Cursor::new(&big)),
            Err(FramingError::FrameTooLarge { announced }) if announced == MAX_FRAME_SIZE + 1
        ));
        let mut at_limit = empty;
        at_limit[24..28].copy_from_slice(&MAX_FRAME_SIZE.to_le_bytes());
        assert!(matches!(
            read_message(&mut Cursor::new(&at_limit)),
            Err(FramingError::UnexpectedEof)
        ));
    }

    /// A reader that hands out at most `chunk` octets per call.
    struct Chunked {
        data: Vec<u8>,
        at: usize,
        chunk: usize,
    }

    impl Read for Chunked {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.chunk).min(self.data.len() - self.at);
            buf[..n].copy_from_slice(&self.data[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    #[test]
    fn tcp_2_5_two_messages_back_to_back() {
        let first = stream_message([0x01, 0xF0], true, &INFO_TS);
        let second = stream_message([0x01, 0x10], false, &[0xAB; 100]);
        let mut wire = first.clone();
        wire.extend_from_slice(&second);
        for chunk in [1, 3, 27, 28, 29, 4096] {
            let mut r = Chunked {
                data: wire.clone(),
                at: 0,
                chunk,
            };
            assert_eq!(read_message(&mut r).unwrap(), first, "chunk {chunk}");
            assert_eq!(read_message(&mut r).unwrap(), second, "chunk {chunk}");
            assert!(matches!(read_message(&mut r), Err(FramingError::Closed)));
        }
    }

    /// §2.4 holds in this dialect: the end of the stream on a message
    /// boundary is the regular close, inside a message it is an error.
    #[test]
    fn tcp_2_5_eof_inside_a_message_is_an_error() {
        let message = stream_message([0x01, 0xF0], true, &INFO_TS);
        assert!(matches!(
            read_message(&mut Cursor::new(&[0u8; 0][..])),
            Err(FramingError::Closed)
        ));
        for cut in [1, 19, 27, 28, 39] {
            assert!(
                matches!(
                    read_message(&mut Cursor::new(&message[..cut])),
                    Err(FramingError::UnexpectedEof)
                ),
                "cut at {cut}"
            );
        }
    }

    /// The sender's check: the stated length is the length of the
    /// message.
    #[test]
    fn tcp_2_5_a_message_to_send_states_its_own_length() {
        let message = stream_message([0x01, 0xF0], true, &INFO_TS);
        assert!(check_message(&message).is_ok());
        let mut longer = message.clone();
        longer.push(0);
        assert!(matches!(
            check_message(&longer),
            Err(FramingError::LengthMismatch {
                announced: 40,
                actual: 41
            })
        ));
        assert!(matches!(
            check_message(&message[..39]),
            Err(FramingError::LengthMismatch {
                announced: 40,
                actual: 39
            })
        ));
        assert!(matches!(
            check_message(&message[..27]),
            Err(FramingError::NoMessageLength)
        ));
        assert!(matches!(
            check_message(b"GET /"),
            Err(FramingError::NotRtps { .. })
        ));
        // What zerodds-rtps makes of a message without the extension
        // passes the check.
        let mut plain = message[..20].to_vec();
        plain.extend_from_slice(&INFO_TS);
        assert!(check_message(&plain).is_err());
        let stated =
            zerodds_rtps::header_extension::HeaderExtension::with_message_length(&plain).unwrap();
        assert!(check_message(&stated).is_ok());
        assert_eq!(stated, message);
    }
}
