// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Stream framing of Cyclone DDS (`ddsi_tcp`), the `cyclone` dialect
//! (`zerodds-tcp-transport-1.0` §4.1).
//!
//! Cyclone DDS has no handshake and no length in front of a message. A
//! connection carries RTPS messages back to back, and the first
//! submessage of each is one of its own, `0x81`
//! (`DDSI_RTPS_SMID_ADLINK_MSG_LEN`), whose 32-bit field counts the whole
//! message, RTPS header and this submessage included (Cyclone DDS,
//! `ddsi_receive.c`, `read_packet_from_stream`; `ddsi_xmsg.c`). Every
//! message on the stream begins with these 28 octets:
//!
//! ```text
//!  0  'R' 'T' 'P' 'S'                 RTPS header, 20 octets
//!  4  major minor                     protocol version
//!  6  vendorId (2)
//!  8  guidPrefix (12)
//! 20  0x81                            submessageId of the length submessage
//! 21  flags                           E = 0x01
//! 22  octetsToNextHeader (u16)        in the byte order of E; 4
//! 24  length (u32)                    in the byte order of E
//! ```
//!
//! The length submessage is framing and no part of the RTPS message: with
//! RTPS protection Cyclone DDS puts it behind the header of the protected
//! message and takes it out before it decodes (`ddsi_security_omg.c`).
//! The writer here puts the eight octets behind the RTPS header of the
//! message it is given, and the reader takes them out again, so that the
//! message above the transport is the one a UDP datagram carries (§1.2).

extern crate alloc;

use std::io::{Read, Write};

use crate::framing::{FramingError, MAX_FRAME_SIZE, read_exact_or_eof, read_head};

/// The octets a message of the `cyclone` dialect begins with: RTPS header
/// (20), the length submessage (8).
pub const STREAM_HEAD_LEN: usize = 28;

/// SubmessageId of the length submessage of Cyclone DDS
/// (`DDSI_RTPS_SMID_ADLINK_MSG_LEN`).
pub const MSG_LEN_SUBMESSAGE_ID: u8 = 0x81;

/// Octets the length submessage adds to a message: its header and its
/// 32-bit field.
pub const MSG_LEN_SIZE: usize = 8;

/// The largest message Cyclone DDS reads from a stream, length submessage
/// included (`ddsi_receive.c`, `do_packet`: the smaller of its receive
/// chunk size and 65536). It drops the connection on a longer one.
pub const MAX_STREAM_MESSAGE_SIZE: usize = 65_536;

/// The largest RTPS message [`write_message`] takes:
/// [`MAX_STREAM_MESSAGE_SIZE`] without the length submessage.
pub const MAX_MESSAGE_SIZE: usize = MAX_STREAM_MESSAGE_SIZE - MSG_LEN_SIZE;

/// Length of the RTPS header.
const RTPS_HEADER_LEN: usize = 20;
/// E flag: the submessage is little-endian.
const FLAG_E: u8 = 0x01;

/// What the head of a stream message states: the length of the whole
/// stream message and the octets of it that are the length submessage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamHead {
    /// Octets of the message on the stream, from the first octet of the
    /// RTPS header.
    pub stream_length: u32,
    /// Octets of the length submessage, its header included.
    pub msg_len_octets: usize,
}

/// Reads the head of a message of the `cyclone` dialect.
///
/// # Errors
/// `NotRtps` for a head that does not begin with `RTPS`;
/// `NoMessageLength` for a first submessage that is not `0x81` or whose
/// octetsToNextHeader has no room for the length; `LengthTooSmall` for a
/// length below the RTPS header and the length submessage; `FrameTooLarge`
/// for one above [`MAX_FRAME_SIZE`].
pub fn stream_head(head: &[u8; STREAM_HEAD_LEN]) -> Result<StreamHead, FramingError> {
    let magic = [head[0], head[1], head[2], head[3]];
    if &magic != b"RTPS" {
        return Err(FramingError::NotRtps { got: magic });
    }
    if head[RTPS_HEADER_LEN] != MSG_LEN_SUBMESSAGE_ID {
        return Err(FramingError::NoMessageLength);
    }
    let little_endian = head[RTPS_HEADER_LEN + 1] & FLAG_E != 0;
    let octets = [head[RTPS_HEADER_LEN + 2], head[RTPS_HEADER_LEN + 3]];
    let octets_to_next_header = usize::from(if little_endian {
        u16::from_le_bytes(octets)
    } else {
        u16::from_be_bytes(octets)
    });
    let length = [head[24], head[25], head[26], head[27]];
    let stream_length = if little_endian {
        u32::from_le_bytes(length)
    } else {
        u32::from_be_bytes(length)
    };
    // Zero is "to the end of the message" (DDSI-RTPS 2.5 §9.4.5.1.3): the
    // length submessage is then the only one.
    let msg_len_octets = match octets_to_next_header {
        0 => (stream_length as usize).saturating_sub(RTPS_HEADER_LEN),
        n if n >= 4 && n % 4 == 0 => 4 + n,
        _ => return Err(FramingError::NoMessageLength),
    };
    if msg_len_octets < MSG_LEN_SIZE || (stream_length as usize) < RTPS_HEADER_LEN + msg_len_octets
    {
        return Err(FramingError::LengthTooSmall {
            announced: stream_length,
        });
    }
    if stream_length > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge {
            announced: stream_length,
        });
    }
    Ok(StreamHead {
        stream_length,
        msg_len_octets,
    })
}

/// Reads the rest of a message whose 28-octet `head` has been read, and
/// returns the RTPS message without the length submessage: the RTPS
/// header, then what followed the length submessage.
///
/// # Errors
/// The errors of [`stream_head`]; `UnexpectedEof` if the stream ends
/// inside the message; I/O error.
pub fn read_body<R: Read>(
    head: &[u8; STREAM_HEAD_LEN],
    r: &mut R,
) -> Result<alloc::vec::Vec<u8>, FramingError> {
    let stated = stream_head(head)?;
    let mut rest = alloc::vec![0u8; stated.stream_length as usize - STREAM_HEAD_LEN];
    read_exact_or_eof(r, &mut rest)?;
    // Of the length submessage, `MSG_LEN_SIZE` octets are in `head`; one
    // longer than that has its remainder at the start of `rest`.
    let skip = stated.msg_len_octets - MSG_LEN_SIZE;
    let mut message = alloc::vec::Vec::with_capacity(RTPS_HEADER_LEN + rest.len() - skip);
    message.extend_from_slice(&head[..RTPS_HEADER_LEN]);
    message.extend_from_slice(&rest[skip..]);
    Ok(message)
}

/// Reads one message of the `cyclone` dialect and returns the RTPS
/// message without the length submessage.
///
/// # Errors
/// `Closed` if the stream ends before the first octet of a message,
/// `UnexpectedEof` if it ends inside one, the errors of [`stream_head`],
/// I/O error. After any error but `Closed` the stream has no known
/// message boundary and is to be closed.
pub fn read_message<R: Read>(r: &mut R) -> Result<alloc::vec::Vec<u8>, FramingError> {
    let mut head = [0u8; STREAM_HEAD_LEN];
    read_head(r, &mut head)?;
    read_body(&head, r)
}

/// Whether `message` can go on a stream of the `cyclone` dialect: it
/// begins with an RTPS header and is, with the length submessage, not
/// longer than Cyclone DDS reads.
///
/// # Errors
/// `NotRtps` for a message that does not begin with `RTPS` or is shorter
/// than an RTPS header; `FrameTooLarge` for one above
/// [`MAX_MESSAGE_SIZE`], with the length it would have on the stream.
pub fn check_message(message: &[u8]) -> Result<(), FramingError> {
    let mut got = [0u8; 4];
    for (to, from) in got.iter_mut().zip(message) {
        *to = *from;
    }
    if &got != b"RTPS" || message.len() < RTPS_HEADER_LEN {
        return Err(FramingError::NotRtps { got });
    }
    if message.len() > MAX_MESSAGE_SIZE {
        return Err(FramingError::FrameTooLarge {
            announced: u32::try_from(message.len() + MSG_LEN_SIZE).unwrap_or(u32::MAX),
        });
    }
    Ok(())
}

/// Writes one RTPS message in the `cyclone` dialect: its RTPS header, the
/// length submessage (little-endian, octetsToNextHeader 4, the length of
/// the message with these eight octets), then the rest of the message.
///
/// # Errors
/// The errors of [`check_message`], before an octet is written; I/O
/// error.
pub fn write_message<W: Write>(w: &mut W, message: &[u8]) -> Result<(), FramingError> {
    check_message(message)?;
    // `check_message` bounds the length by `MAX_MESSAGE_SIZE`.
    let stream_length =
        u32::try_from(message.len() + MSG_LEN_SIZE).map_err(|_| FramingError::FrameTooLarge {
            announced: u32::MAX,
        })?;
    let (header, rest) = message.split_at(RTPS_HEADER_LEN);
    let mut msg_len = [MSG_LEN_SUBMESSAGE_ID, FLAG_E, 4, 0, 0, 0, 0, 0];
    msg_len[4..].copy_from_slice(&stream_length.to_le_bytes());
    w.write_all(header)?;
    w.write_all(&msg_len)?;
    w.write_all(rest)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;

    /// The first message a Cyclone DDS 0.10.5 participant (`ddsperf pub`,
    /// `General/Transport` = `tcp`, no `TCP/Port`) wrote on the TCP
    /// connection to its peer, as captured on the Linux test host: an SPDP
    /// announcement of 328 octets without a unicast locator.
    pub(crate) const CYCLONE_0_10_5_SPDP_NO_PORT: &str =
        include_str!("../tests/data/cyclone_0_10_5_spdp_tcp.hex");

    /// The same with `TCP/Port` = 17500: 384 octets, the default and the
    /// metatraffic unicast locator of kind 4 with port 17500.
    pub(crate) const CYCLONE_0_10_5_SPDP_PORT_17500: &str =
        include_str!("../tests/data/cyclone_0_10_5_spdp_tcp_port_17500.hex");

    pub(crate) fn unhex(text: &str) -> alloc::vec::Vec<u8> {
        let digits: alloc::vec::Vec<u8> = text
            .bytes()
            .filter(|b| b.is_ascii_hexdigit())
            .map(|b| match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                _ => b - b'A' + 10,
            })
            .collect();
        assert_eq!(digits.len() % 2, 0);
        digits
            .chunks(2)
            .map(|pair| pair[0] << 4 | pair[1])
            .collect()
    }

    /// An RTPS message as a UDP datagram carries it: header of vendor
    /// `01 10` and `body` behind it.
    pub(crate) fn plain_message(body: &[u8]) -> alloc::vec::Vec<u8> {
        let mut m = alloc::vec::Vec::new();
        m.extend_from_slice(b"RTPS");
        m.extend_from_slice(&[2, 1, 0x01, 0x10]);
        m.extend_from_slice(&[0xAB; 12]);
        m.extend_from_slice(body);
        m
    }

    /// Reads from `inner` at most `chunk` octets per call.
    struct Chunked<'a> {
        inner: &'a [u8],
        chunk: usize,
    }

    impl Read for Chunked<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.chunk.min(buf.len()).min(self.inner.len());
            buf[..n].copy_from_slice(&self.inner[..n]);
            self.inner = &self.inner[n..];
            Ok(n)
        }
    }

    /// The layout the writer produces, octet by octet, against the layout
    /// of the Cyclone DDS source: header, `81 01 04 00`, the length of the
    /// whole stream message little-endian, the rest.
    #[test]
    fn tcp_4_1_msg_len_submessage_layout() {
        let message = plain_message(&[0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(message.len(), 32);
        let mut wire = alloc::vec::Vec::new();
        write_message(&mut wire, &message).unwrap();
        assert_eq!(wire.len(), 40);
        assert_eq!(&wire[..20], &message[..20]);
        assert_eq!(&wire[20..28], &[0x81, 0x01, 0x04, 0x00, 40, 0, 0, 0]);
        assert_eq!(&wire[28..], &message[20..]);
    }

    /// The oracle outside this crate: what Cyclone DDS 0.10.5 wrote. Its
    /// length submessage states the 328 octets captured, and the message
    /// handed up is the capture without the eight octets at 20.
    #[test]
    fn tcp_4_1_reads_what_cyclone_dds_wrote() {
        for (capture, length) in [
            (CYCLONE_0_10_5_SPDP_NO_PORT, 328usize),
            (CYCLONE_0_10_5_SPDP_PORT_17500, 384),
        ] {
            let wire = unhex(capture);
            assert_eq!(wire.len(), length);
            let head: &[u8; STREAM_HEAD_LEN] = wire.first_chunk().unwrap();
            assert_eq!(
                stream_head(head).unwrap(),
                StreamHead {
                    stream_length: u32::try_from(length).unwrap(),
                    msg_len_octets: 8,
                }
            );
            let mut stream = &wire[..];
            let message = read_message(&mut stream).unwrap();
            assert!(stream.is_empty());
            assert_eq!(message.len(), length - 8);
            assert_eq!(&message[..20], &wire[..20]);
            assert_eq!(&message[20..], &wire[28..]);
            // Vendor id of Cyclone DDS, and INFO_TS as the first submessage
            // of the message handed up.
            assert_eq!(&message[6..8], &[0x01, 0x10]);
            assert_eq!(message[20], 0x09);
            // Written again, the message is the capture.
            let mut again = alloc::vec::Vec::new();
            write_message(&mut again, &message).unwrap();
            assert_eq!(again, wire);
        }
    }

    #[test]
    fn tcp_4_1_the_message_handed_up_has_no_msg_len() {
        let message = plain_message(&[0x15, 0x05, 0x00, 0x00, 9, 9, 9, 9]);
        let mut wire = alloc::vec::Vec::new();
        write_message(&mut wire, &message).unwrap();
        assert!(wire[20..].contains(&MSG_LEN_SUBMESSAGE_ID));
        let read = read_message(&mut &wire[..]).unwrap();
        assert_eq!(read, message);
    }

    /// The length in the byte order of the E flag, as
    /// `read_packet_from_stream` swaps it.
    #[test]
    fn tcp_4_1_reads_a_big_endian_msg_len() {
        let mut wire = plain_message(&[]);
        wire.extend_from_slice(&[0x81, 0x00, 0x00, 0x04, 0, 0, 0, 36]);
        wire.extend_from_slice(&[0x09, 0x00, 0x00, 0x04, 1, 2, 3, 4]);
        let read = read_message(&mut &wire[..]).unwrap();
        assert_eq!(read.len(), 28);
        assert_eq!(&read[20..], &[0x09, 0x00, 0x00, 0x04, 1, 2, 3, 4]);
    }

    #[test]
    fn tcp_4_1_refuses_a_first_submessage_other_than_msg_len() {
        // A message as UDP carries it: INFO_TS first.
        let plain = plain_message(&[0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(matches!(
            read_message(&mut &plain[..]),
            Err(FramingError::NoMessageLength)
        ));
        // The HeaderExtension of the dialect `rtps`.
        let rtps = crate::rtps_stream::tests::stream_message([0x01, 0xF0], true, b"abcd");
        assert!(matches!(
            read_message(&mut &rtps[..]),
            Err(FramingError::NoMessageLength)
        ));
        // The request of §3.1.
        let mut request = alloc::vec::Vec::from(*b"ZDDS");
        request.extend_from_slice(&[0u8; 24]);
        assert!(matches!(
            read_message(&mut &request[..]),
            Err(FramingError::NotRtps { got }) if &got == b"ZDDS"
        ));
        // An octetsToNextHeader without room for the length.
        let mut short = plain_message(&[]);
        short.extend_from_slice(&[0x81, 0x01, 0x02, 0x00, 28, 0, 0, 0]);
        assert!(matches!(
            read_message(&mut &short[..]),
            Err(FramingError::NoMessageLength)
        ));
    }

    /// Lengths below header and length submessage; 28 is the message
    /// without a further submessage; one above 16 MiB is refused before
    /// anything is read.
    #[test]
    fn tcp_4_1_refuses_a_length_below_28_and_above_the_limit() {
        for length in [0u32, 1, 20, 27] {
            let mut wire = plain_message(&[]);
            wire.extend_from_slice(&[0x81, 0x01, 0x04, 0x00]);
            wire.extend_from_slice(&length.to_le_bytes());
            assert!(
                matches!(
                    read_message(&mut &wire[..]),
                    Err(FramingError::LengthTooSmall { announced }) if announced == length
                ),
                "length {length}"
            );
        }
        let mut empty = plain_message(&[]);
        empty.extend_from_slice(&[0x81, 0x01, 0x04, 0x00, 28, 0, 0, 0]);
        assert_eq!(read_message(&mut &empty[..]).unwrap(), plain_message(&[]));
        // octetsToNextHeader 0: the length submessage reaches to the end.
        let mut to_the_end = plain_message(&[]);
        to_the_end.extend_from_slice(&[0x81, 0x01, 0x00, 0x00, 28, 0, 0, 0]);
        assert_eq!(
            read_message(&mut &to_the_end[..]).unwrap(),
            plain_message(&[])
        );
        let mut huge = plain_message(&[]);
        huge.extend_from_slice(&[0x81, 0x01, 0x04, 0x00]);
        huge.extend_from_slice(&(MAX_FRAME_SIZE + 1).to_le_bytes());
        assert!(matches!(
            read_message(&mut &huge[..]),
            Err(FramingError::FrameTooLarge { announced }) if announced == MAX_FRAME_SIZE + 1
        ));
    }

    /// A length submessage longer than eight octets is taken out whole.
    #[test]
    fn tcp_4_1_takes_out_a_longer_length_submessage() {
        let mut wire = plain_message(&[]);
        wire.extend_from_slice(&[0x81, 0x01, 0x08, 0x00, 40, 0, 0, 0, 0xEE, 0xEE, 0xEE, 0xEE]);
        wire.extend_from_slice(&[0x09, 0x01, 0x04, 0x00, 1, 2, 3, 4]);
        assert_eq!(wire.len(), 40);
        let read = read_message(&mut &wire[..]).unwrap();
        assert_eq!(&read[20..], &[0x09, 0x01, 0x04, 0x00, 1, 2, 3, 4]);
    }

    #[test]
    fn tcp_4_1_reassembles_across_reads() {
        let first = plain_message(&[0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
        let second = plain_message(&[0x15, 0x05, 0x00, 0x00]);
        let mut wire = alloc::vec::Vec::new();
        write_message(&mut wire, &first).unwrap();
        write_message(&mut wire, &second).unwrap();
        for chunk in [1usize, 3, 27, 28, 29, 4096] {
            let mut stream = Chunked {
                inner: &wire,
                chunk,
            };
            assert_eq!(read_message(&mut stream).unwrap(), first, "chunk {chunk}");
            assert_eq!(read_message(&mut stream).unwrap(), second, "chunk {chunk}");
            assert!(matches!(
                read_message(&mut stream),
                Err(FramingError::Closed)
            ));
        }
    }

    #[test]
    fn tcp_4_1_eof_inside_a_message_is_an_error() {
        let message = plain_message(&[0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
        let mut wire = alloc::vec::Vec::new();
        write_message(&mut wire, &message).unwrap();
        for cut in [1usize, 19, 27, 28, 39] {
            assert!(
                matches!(
                    read_message(&mut &wire[..cut]),
                    Err(FramingError::UnexpectedEof)
                ),
                "cut {cut}"
            );
        }
    }

    /// What Cyclone DDS would drop the connection on is refused before an
    /// octet is written: a message that is, with its length submessage,
    /// above 65536 octets, and what has no RTPS header.
    #[test]
    fn tcp_4_1_refuses_to_write_what_cyclone_dds_does_not_read() {
        let largest = plain_message(&alloc::vec![0u8; MAX_MESSAGE_SIZE - 20]);
        let mut wire = alloc::vec::Vec::new();
        write_message(&mut wire, &largest).unwrap();
        assert_eq!(wire.len(), MAX_STREAM_MESSAGE_SIZE);

        let too_large = plain_message(&alloc::vec![0u8; MAX_MESSAGE_SIZE - 19]);
        let mut wire = alloc::vec::Vec::new();
        assert!(matches!(
            write_message(&mut wire, &too_large),
            Err(FramingError::FrameTooLarge { announced: 65_537 })
        ));
        assert!(wire.is_empty());

        for no_header in [&b"RTPS"[..], &b"hello, this is no RTPS message"[..], &[]] {
            assert!(matches!(
                write_message(&mut wire, no_header),
                Err(FramingError::NotRtps { .. })
            ));
            assert!(wire.is_empty());
        }
    }
}
