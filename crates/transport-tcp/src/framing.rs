// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TCP framing for RTPS messages.
//!
//! TCP is a stream: the receiver needs the length of every RTPS message
//! to find where it ends. DDSI-RTPS 2.5 §8.3.3 names the HeaderExtension
//! as the place for that length and fixes no TCP mapping (§9.5 maps to
//! UDP/IP only). The ZeroDDS dialect of
//! `docs/spec-coverage/zerodds-tcp-transport-1.0.md` §2 puts a 4-byte
//! big-endian length in front of every message and leaves the message
//! itself as it is on UDP.
//!
//! Wire format:
//!
//! ```text
//! +---------+---------+---------+---------+------- ...
//! |            length (u32, BE)           | RTPS message bytes
//! +---------+---------+---------+---------+------- ...
//! ```
//!
//! The length bounds a single frame to `u32::MAX` bytes; the DoS cap
//! in [`MAX_FRAME_SIZE`] caps actually-accepted frames at a sensible
//! max, to prevent memory exhaustion by malicious peers.

use std::io::{Read, Write};

/// Maximum accepted frame size (16 MiB).
///
/// # Rationale
///
/// As a stream transport, TCP can in theory carry arbitrarily large
/// frames (unlike UDP with its 64 kB datagram cap). We cap here for DoS
/// protection — a malicious peer must not cost us GBs of RAM with a
/// u32-sized announcement.
///
/// **RTPS fragmentation (DDSI-RTPS §8.3.7)** works at the submessage
/// level: large samples are split by the writer into `DATA_FRAG`
/// submessages, **each** of which then fits into a TCP frame. A 16 MiB
/// cap therefore covers samples well beyond typical DDS payloads
/// (ROS pointcloud ~MB, industrial sensors <<1 MB) — reassembly is done
/// by the reader via `FragmentAssembler`
/// (crates/rtps/src/fragment_assembler.rs), not the transport.
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Length of the length prefix in bytes.
pub const FRAME_HEADER_LEN: usize = 4;

/// Framing error.
#[derive(Debug)]
#[non_exhaustive]
pub enum FramingError {
    /// I/O error on read/write.
    Io(std::io::Error),
    /// Frame larger than [`MAX_FRAME_SIZE`].
    FrameTooLarge {
        /// Announced length.
        announced: u32,
    },
    /// The stream ended inside a frame: after one to three octets of the
    /// length, or before the announced body was complete (§2.4).
    UnexpectedEof,
    /// The stream ended on a frame boundary, before the first octet of a
    /// next frame: the regular end of the connection (§2.4).
    Closed,
    /// A message of the `rtps` dialect (§2.5) does not begin with `RTPS`.
    NotRtps {
        /// The four octets found.
        got: [u8; 4],
    },
    /// A message of the `rtps` dialect (§2.5) does not state its length:
    /// its protocol version is below 2.5, its first submessage is no
    /// HeaderExtension, or the HeaderExtension has no messageLength.
    NoMessageLength,
    /// The messageLength of a message of the `rtps` dialect (§2.5) is
    /// smaller than the RTPS header and the HeaderExtension that carries
    /// it.
    LengthTooSmall {
        /// The messageLength stated.
        announced: u32,
    },
    /// A message to send in the `rtps` dialect (§2.5) states a
    /// messageLength that is not its length.
    LengthMismatch {
        /// The messageLength stated.
        announced: u32,
        /// The length of the message.
        actual: usize,
    },
}

impl core::fmt::Display for FramingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "framing i/o error: {e}"),
            Self::FrameTooLarge { announced } => {
                write!(
                    f,
                    "tcp frame announced {announced} bytes, exceeds MAX_FRAME_SIZE"
                )
            }
            Self::UnexpectedEof => f.write_str("tcp stream closed mid-frame"),
            Self::Closed => f.write_str("tcp stream closed between frames"),
            Self::NotRtps { got } => {
                write!(f, "tcp stream message does not begin with RTPS: {got:?}")
            }
            Self::NoMessageLength => {
                f.write_str("tcp stream message without a HeaderExtension messageLength")
            }
            Self::LengthTooSmall { announced } => write!(
                f,
                "tcp stream message states {announced} octets, less than its header and HeaderExtension"
            ),
            Self::LengthMismatch { announced, actual } => write!(
                f,
                "tcp stream message of {actual} octets states a messageLength of {announced}"
            ),
        }
    }
}

impl std::error::Error for FramingError {}

impl From<std::io::Error> for FramingError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Writes a frame (length prefix + payload) into the stream.
///
/// # Errors
/// I/O error; `FrameTooLarge` if `payload.len() > u32::MAX`.
pub fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> Result<(), FramingError> {
    let len = u32::try_from(payload.len()).map_err(|_| FramingError::FrameTooLarge {
        announced: u32::MAX,
    })?;
    if len > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge { announced: len });
    }
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    Ok(())
}

/// Reads a frame (length prefix + payload) from the stream.
///
/// Blocks until a complete frame has been read.
///
/// # Errors
/// I/O error; `FrameTooLarge` if the peer announces > [`MAX_FRAME_SIZE`];
/// `Closed` if the stream ends before the first octet of a frame;
/// `UnexpectedEof` if it ends inside the length or the body.
pub fn read_frame<R: Read>(r: &mut R) -> Result<alloc::vec::Vec<u8>, FramingError> {
    let mut hdr = [0u8; FRAME_HEADER_LEN];
    read_head(r, &mut hdr)?;
    let len = u32::from_be_bytes(hdr);
    if len > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge { announced: len });
    }
    let mut buf = alloc::vec![0u8; len as usize];
    read_exact_or_eof(r, &mut buf)?;
    Ok(buf)
}

extern crate alloc;

/// Fills `head`, the first octets of a frame or message: `Closed` if the
/// stream ends before the first of them, `UnexpectedEof` if it ends
/// inside them (§2.4).
pub(crate) fn read_head<R: Read>(r: &mut R, head: &mut [u8]) -> Result<(), FramingError> {
    let mut got = 0;
    while got < head.len() {
        match r.read(&mut head[got..]) {
            Ok(0) if got == 0 => return Err(FramingError::Closed),
            Ok(0) => return Err(FramingError::UnexpectedEof),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(FramingError::Io(e)),
        }
    }
    Ok(())
}

pub(crate) fn read_exact_or_eof<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), FramingError> {
    match r.read_exact(buf) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Err(FramingError::UnexpectedEof),
        Err(e) => Err(FramingError::Io(e)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn tcp_1_2_roundtrip_small_frame() {
        let mut buf = alloc::vec::Vec::new();
        write_frame(&mut buf, b"hello").unwrap();
        assert_eq!(buf.len(), 4 + 5);
        assert_eq!(&buf[..4], &5u32.to_be_bytes());
        let mut cur = Cursor::new(&buf);
        let back = read_frame(&mut cur).unwrap();
        assert_eq!(back, b"hello");
    }

    #[test]
    fn tcp_2_1_roundtrip_empty_frame() {
        let mut buf = alloc::vec::Vec::new();
        write_frame(&mut buf, &[]).unwrap();
        assert_eq!(buf, 0u32.to_be_bytes());
        let mut cur = Cursor::new(&buf);
        let back = read_frame(&mut cur).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn tcp_2_3_rejects_oversized_announcement() {
        let mut bad = alloc::vec::Vec::new();
        bad.extend_from_slice(&(MAX_FRAME_SIZE + 1).to_be_bytes());
        let mut cur = Cursor::new(&bad);
        let err = read_frame(&mut cur).unwrap_err();
        match err {
            FramingError::FrameTooLarge { announced } => {
                assert_eq!(announced, MAX_FRAME_SIZE + 1);
            }
            other => panic!("expected FrameTooLarge, got {other:?}"),
        }
    }

    /// One, two and three octets of the length, then EOF: inside a frame.
    #[test]
    fn tcp_2_4_unexpected_eof_mid_header() {
        for short in [&[0u8][..], &[0u8, 0], &[0u8, 0, 0]] {
            let mut cur = Cursor::new(short);
            let err = read_frame(&mut cur).unwrap_err();
            assert!(
                matches!(err, FramingError::UnexpectedEof),
                "{} octets: {err:?}",
                short.len()
            );
        }
    }

    /// No octet of a next frame: the end of the connection, told apart
    /// from the end inside a frame. Holds for an empty stream and behind a
    /// complete frame, also behind one with an empty body.
    #[test]
    fn tcp_2_4_eof_on_a_frame_boundary_is_closed() {
        let mut cur = Cursor::new(&[][..]);
        assert!(matches!(read_frame(&mut cur), Err(FramingError::Closed)));

        let mut wire = alloc::vec::Vec::new();
        write_frame(&mut wire, b"abc").unwrap();
        write_frame(&mut wire, &[]).unwrap();
        let mut cur = Cursor::new(&wire);
        assert_eq!(read_frame(&mut cur).unwrap(), b"abc");
        assert!(read_frame(&mut cur).unwrap().is_empty());
        assert!(matches!(read_frame(&mut cur), Err(FramingError::Closed)));
    }

    /// The limit itself passes on the sending side, one octet more does not
    /// and writes nothing.
    #[test]
    fn tcp_2_3_write_frame_takes_the_limit_and_refuses_one_octet_more() {
        let at_limit = alloc::vec![0u8; MAX_FRAME_SIZE as usize];
        let mut wire = alloc::vec::Vec::new();
        write_frame(&mut wire, &at_limit).unwrap();
        assert_eq!(wire.len(), FRAME_HEADER_LEN + MAX_FRAME_SIZE as usize);
        assert_eq!(&wire[..4], &MAX_FRAME_SIZE.to_be_bytes());

        let over = alloc::vec![0u8; MAX_FRAME_SIZE as usize + 1];
        let mut wire = alloc::vec::Vec::new();
        let err = write_frame(&mut wire, &over).unwrap_err();
        assert!(matches!(
            err,
            FramingError::FrameTooLarge { announced } if announced == MAX_FRAME_SIZE + 1
        ));
        assert!(wire.is_empty());
    }

    #[test]
    fn tcp_2_4_unexpected_eof_mid_payload() {
        let mut bad = alloc::vec::Vec::new();
        bad.extend_from_slice(&10u32.to_be_bytes());
        bad.extend_from_slice(&[1, 2, 3]); // only 3 of 10
        let mut cur = Cursor::new(&bad);
        let err = read_frame(&mut cur).unwrap_err();
        assert!(matches!(err, FramingError::UnexpectedEof));
    }

    #[test]
    fn tcp_2_1_two_frames_back_to_back() {
        let mut buf = alloc::vec::Vec::new();
        write_frame(&mut buf, b"aaa").unwrap();
        write_frame(&mut buf, b"bbbb").unwrap();
        let mut cur = Cursor::new(&buf);
        assert_eq!(read_frame(&mut cur).unwrap(), b"aaa");
        assert_eq!(read_frame(&mut cur).unwrap(), b"bbbb");
    }

    /// A `Read` that never hands out more than `chunk` bytes per call —
    /// the in-memory stand-in for TCP segmentation, where one `write`
    /// on the sender can arrive as several `read`s on the receiver.
    struct ChunkedReader<'a> {
        bytes: &'a [u8],
        chunk: usize,
    }

    impl std::io::Read for ChunkedReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.bytes.len().min(buf.len()).min(self.chunk);
            buf[..n].copy_from_slice(&self.bytes[..n]);
            self.bytes = &self.bytes[n..];
            Ok(n)
        }
    }

    /// The length prefix itself arrives split: one byte per `read`.
    /// `read_frame` must still reassemble the announced length and the
    /// complete payload.
    #[test]
    fn tcp_2_2_reassembles_frame_split_across_reads() {
        let payload: alloc::vec::Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let mut wire = alloc::vec::Vec::new();
        write_frame(&mut wire, &payload).unwrap();

        for chunk in [1usize, 3, 4, 5, 4095] {
            let mut r = ChunkedReader {
                bytes: &wire,
                chunk,
            };
            let back = read_frame(&mut r).unwrap();
            assert_eq!(back, payload, "chunk size {chunk}");
        }
    }

    /// A `Read` over a list of writes. Every `read` is served from one
    /// write only and stops at its boundary, which is what a receiver
    /// observes when the sender emits two `write` calls.
    struct WriteBoundaryReader {
        writes: alloc::vec::Vec<alloc::vec::Vec<u8>>,
        at: usize,
    }

    impl std::io::Read for WriteBoundaryReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            while self.at < self.writes.len() && self.writes[self.at].is_empty() {
                self.at += 1;
            }
            if self.at >= self.writes.len() {
                return Ok(0);
            }
            let current = &mut self.writes[self.at];
            let n = current.len().min(buf.len());
            buf[..n].copy_from_slice(&current[..n]);
            current.drain(..n);
            Ok(n)
        }
    }

    /// The exact split the spec item names: length prefix plus the first
    /// half in one write, the second half in a second write. Both halves
    /// are one frame on the wire, so the reader must return exactly one
    /// payload and leave the stream positioned behind it.
    #[test]
    fn tcp_2_2_reassembles_message_split_across_two_writes() {
        let payload = b"the second half arrives in a separate write";
        let split = payload.len() / 2;

        // Write 1: the 4-byte length prefix plus the first half.
        let mut first = alloc::vec::Vec::new();
        first.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        first.extend_from_slice(&payload[..split]);
        // Write 2: the remainder, plus an unrelated follow-up frame so
        // reassembly cannot succeed by simply draining the stream.
        let mut second = alloc::vec::Vec::new();
        second.extend_from_slice(&payload[split..]);
        write_frame(&mut second, b"next").unwrap();

        let mut r = WriteBoundaryReader {
            writes: alloc::vec![first, second],
            at: 0,
        };
        assert_eq!(read_frame(&mut r).unwrap(), payload);
        assert_eq!(read_frame(&mut r).unwrap(), b"next");
    }
}
