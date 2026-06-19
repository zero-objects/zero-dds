// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TCP framing for RTPS messages.
//!
//! TCP is a stream protocol — every RTPS datagram must mark its own
//! length so the receiver can reconstruct message boundaries. We use a
//! simple 4-byte big-endian length prefix (DDS-TCP-PSM §6.2:
//! `SubmessageFlag E` + the length field in the submessage header are
//! irrelevant for TCP, only the outer frame counts).
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
pub enum FramingError {
    /// I/O error on read/write.
    Io(std::io::Error),
    /// Frame larger than [`MAX_FRAME_SIZE`].
    FrameTooLarge {
        /// Announced length.
        announced: u32,
    },
    /// Stream delivered 0 bytes before the end of the frame (EOF/peer closed).
    UnexpectedEof,
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
/// `UnexpectedEof` on premature EOF.
pub fn read_frame<R: Read>(r: &mut R) -> Result<alloc::vec::Vec<u8>, FramingError> {
    let mut hdr = [0u8; FRAME_HEADER_LEN];
    read_exact_or_eof(r, &mut hdr)?;
    let len = u32::from_be_bytes(hdr);
    if len > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge { announced: len });
    }
    let mut buf = alloc::vec![0u8; len as usize];
    read_exact_or_eof(r, &mut buf)?;
    Ok(buf)
}

extern crate alloc;

fn read_exact_or_eof<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), FramingError> {
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
    fn roundtrip_small_frame() {
        let mut buf = alloc::vec::Vec::new();
        write_frame(&mut buf, b"hello").unwrap();
        assert_eq!(buf.len(), 4 + 5);
        assert_eq!(&buf[..4], &5u32.to_be_bytes());
        let mut cur = Cursor::new(&buf);
        let back = read_frame(&mut cur).unwrap();
        assert_eq!(back, b"hello");
    }

    #[test]
    fn roundtrip_empty_frame() {
        let mut buf = alloc::vec::Vec::new();
        write_frame(&mut buf, &[]).unwrap();
        assert_eq!(buf, 0u32.to_be_bytes());
        let mut cur = Cursor::new(&buf);
        let back = read_frame(&mut cur).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn rejects_oversized_announcement() {
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

    #[test]
    fn unexpected_eof_mid_header() {
        let short = alloc::vec![0u8, 0u8];
        let mut cur = Cursor::new(&short);
        let err = read_frame(&mut cur).unwrap_err();
        assert!(matches!(err, FramingError::UnexpectedEof));
    }

    #[test]
    fn unexpected_eof_mid_payload() {
        let mut bad = alloc::vec::Vec::new();
        bad.extend_from_slice(&10u32.to_be_bytes());
        bad.extend_from_slice(&[1, 2, 3]); // only 3 of 10
        let mut cur = Cursor::new(&bad);
        let err = read_frame(&mut cur).unwrap_err();
        assert!(matches!(err, FramingError::UnexpectedEof));
    }

    #[test]
    fn two_frames_back_to_back() {
        let mut buf = alloc::vec::Vec::new();
        write_frame(&mut buf, b"aaa").unwrap();
        write_frame(&mut buf, b"bbbb").unwrap();
        let mut cur = Cursor::new(&buf);
        assert_eq!(read_frame(&mut cur).unwrap(), b"aaa");
        assert_eq!(read_frame(&mut cur).unwrap(), b"bbbb");
    }
}
