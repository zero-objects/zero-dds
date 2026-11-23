// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 frame IO over blocking `std::io::Read`/`Write`.
//!
//! Spec `amqp-1.0-transport`:
//! * §2.2 Protocol Header (`AMQP\0\1\0\0` AMQP / `AMQP\3\1\0\0` SASL).
//! * §2.3 Frame Layout (8-Byte Header + Body).

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use zerodds_amqp_bridge::frame::{
    FrameError, FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};

/// The byte stream of one connection: `Read + Write`, and what a protocol
/// endpoint needs of the socket below it.
pub trait Transport: Read + Write {
    /// How long a read waits for octets before it returns
    /// `WouldBlock`/`TimedOut`; `None` waits without bound.
    ///
    /// # Errors
    /// The socket's error.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;

    /// Closes the outgoing stream; the incoming one stays readable (Spec
    /// §2.2: "both peers MUST close their outgoing stream and SHOULD read
    /// the incoming stream until it is terminated").
    ///
    /// # Errors
    /// The socket's error.
    fn close_outgoing(&mut self) -> io::Result<()>;
}

impl Transport for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }

    fn close_outgoing(&mut self) -> io::Result<()> {
        self.shutdown(std::net::Shutdown::Write)
    }
}

#[cfg(feature = "tls")]
impl Transport for rustls::StreamOwned<rustls::ServerConnection, TcpStream> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }

    fn close_outgoing(&mut self) -> io::Result<()> {
        self.conn.send_close_notify();
        self.flush()?;
        self.sock.shutdown(std::net::Shutdown::Write)
    }
}

/// Spec §2.2 — AMQP protocol ID. First byte of the
/// Protocol-Headers.
pub mod protocol {
    /// `AMQP` Magic-Bytes.
    pub const MAGIC: [u8; 4] = *b"AMQP";
    /// `0x00` — AMQP-Frame-Protocol-Id.
    pub const PROTO_AMQP: u8 = 0x00;
    /// `0x03` — SASL-Frame-Protocol-Id.
    pub const PROTO_SASL: u8 = 0x03;
    /// AMQP-Major-Version.
    pub const MAJOR: u8 = 0x01;
    /// AMQP-Minor-Version.
    pub const MINOR: u8 = 0x00;
    /// AMQP-Revision.
    pub const REVISION: u8 = 0x00;
}

/// AMQP protocol variant of the protocol header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmqpProtocol {
    /// `AMQP\0\1\0\0` — AMQP Frame-Layer.
    Amqp,
    /// `AMQP\3\1\0\0` — SASL Frame-Layer.
    Sasl,
}

impl AmqpProtocol {
    /// Spec §2.2 — 8-Byte Protocol-Header.
    #[must_use]
    pub fn as_bytes(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..4].copy_from_slice(&protocol::MAGIC);
        out[4] = match self {
            Self::Amqp => protocol::PROTO_AMQP,
            Self::Sasl => protocol::PROTO_SASL,
        };
        out[5] = protocol::MAJOR;
        out[6] = protocol::MINOR;
        out[7] = protocol::REVISION;
        out
    }
}

/// Read protocol header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolHeader {
    /// AMQP frame variant.
    pub protocol: AmqpProtocol,
    /// Major version (AMQP 1.0: 1).
    pub major: u8,
    /// Minor version (AMQP 1.0: 0).
    pub minor: u8,
    /// Revision.
    pub revision: u8,
}

/// Read frame: header + body bytes.
#[derive(Debug, Clone)]
pub struct Frame {
    /// 8-byte header.
    pub header: FrameHeader,
    /// Body bytes (may be empty for empty frames per §2.4.5
    /// Heartbeats).
    pub body: Vec<u8>,
}

/// IO-/Frame error.
#[derive(Debug)]
pub enum FrameIoError {
    /// `std::io::Error` from the stream.
    Io(io::Error),
    /// Frame header decoding failed.
    Frame(FrameError),
    /// Protocol header magic does not match.
    InvalidProtocolMagic([u8; 4]),
    /// Unknown protocol id (not 0x00 or 0x03).
    UnsupportedProtocolId(u8),
    /// The version is not AMQP 1.0.0.
    UnsupportedVersion {
        /// Received major.
        major: u8,
        /// Received minor.
        minor: u8,
        /// Received revision.
        revision: u8,
    },
    /// Frame body longer than the resource limits allow.
    FrameTooLarge {
        /// Received frame size.
        size: u32,
        /// Configured max-frame-size.
        max: u32,
    },
}

impl core::fmt::Display for FrameIoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Frame(e) => write!(f, "frame error: {e}"),
            Self::InvalidProtocolMagic(m) => write!(f, "invalid protocol magic: {m:?}"),
            Self::UnsupportedProtocolId(p) => write!(f, "unsupported protocol-id 0x{p:02x}"),
            Self::UnsupportedVersion {
                major,
                minor,
                revision,
            } => {
                write!(f, "unsupported AMQP version {major}.{minor}.{revision}")
            }
            Self::FrameTooLarge { size, max } => {
                write!(f, "frame size {size} exceeds max-frame-size {max}")
            }
        }
    }
}

impl std::error::Error for FrameIoError {}

impl From<io::Error> for FrameIoError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<FrameError> for FrameIoError {
    fn from(e: FrameError) -> Self {
        Self::Frame(e)
    }
}

/// Spec §2.2 — read protocol header (8 bytes blocking).
///
/// # Errors
/// `Io` on TCP error, `InvalidProtocolMagic` on wrong
/// magic bytes, `UnsupportedProtocolId`/`UnsupportedVersion` on
/// non-AMQP-1.0 header; the version is major, minor and revision (1.0.0).
pub fn read_protocol_header<R: Read>(r: &mut R) -> Result<ProtocolHeader, FrameIoError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    let mut magic = [0u8; 4];
    magic.copy_from_slice(&buf[0..4]);
    if magic != protocol::MAGIC {
        return Err(FrameIoError::InvalidProtocolMagic(magic));
    }
    let protocol = match buf[4] {
        protocol::PROTO_AMQP => AmqpProtocol::Amqp,
        protocol::PROTO_SASL => AmqpProtocol::Sasl,
        other => return Err(FrameIoError::UnsupportedProtocolId(other)),
    };
    let major = buf[5];
    let minor = buf[6];
    let revision = buf[7];
    if major != protocol::MAJOR || minor != protocol::MINOR || revision != protocol::REVISION {
        return Err(FrameIoError::UnsupportedVersion {
            major,
            minor,
            revision,
        });
    }
    Ok(ProtocolHeader {
        protocol,
        major,
        minor,
        revision,
    })
}

/// Spec §2.2 — write protocol header.
///
/// # Errors
/// `Io` on stream error.
pub fn write_protocol_header<W: Write>(w: &mut W, p: AmqpProtocol) -> Result<(), FrameIoError> {
    w.write_all(&p.as_bytes())?;
    Ok(())
}

/// Spec §2.3 — read complete frame (header + body).
///
/// `max_frame_size` is the configured DoS cap; frames that
/// exceed it are discarded without a body read.
///
/// # Errors
/// `Io`/`Frame`/`FrameTooLarge`.
pub fn read_frame<R: Read>(r: &mut R, max_frame_size: u32) -> Result<Frame, FrameIoError> {
    let mut hdr_bytes = [0u8; 8];
    r.read_exact(&mut hdr_bytes)?;
    let header = decode_frame_header(&hdr_bytes)?;
    if header.size > max_frame_size {
        return Err(FrameIoError::FrameTooLarge {
            size: header.size,
            max: max_frame_size,
        });
    }
    let body_offset = header.body_offset();
    // Spec §2.3.1.3: extended header bytes lie between 8 and
    // body_offset. We read it, but ignore the content
    // (forwards-compat).
    let extended_header_len = body_offset.saturating_sub(8);
    if extended_header_len > 0 {
        let mut ext = vec![0u8; extended_header_len];
        r.read_exact(&mut ext)?;
    }
    let body_len = (header.size as usize).saturating_sub(body_offset);
    let mut body = vec![0u8; body_len];
    if body_len > 0 {
        r.read_exact(&mut body)?;
    }
    Ok(Frame { header, body })
}

/// What [`FrameReader::poll`] found.
#[derive(Debug)]
pub enum Polled {
    /// A whole frame.
    Frame(Frame),
    /// The read timed out before a whole frame was there; the octets read
    /// so far are kept.
    Idle,
    /// The peer closed the stream.
    Closed,
}

/// Reads frames from a stream whose reads time out: the octets of a frame
/// cut by a time-out stay in the reader until the rest arrives.
#[derive(Debug, Default)]
pub struct FrameReader {
    buf: Vec<u8>,
    discard: bool,
}

impl FrameReader {
    /// From now on the octets of the stream are read and dropped: after a
    /// framing error there is no frame boundary to find again.
    pub fn discard_rest(&mut self) {
        self.discard = true;
        self.buf.clear();
    }

    /// The next frame, if the stream has one within its read time-out.
    ///
    /// # Errors
    /// `Io` for a failed read, `Frame` for a header that is none (Spec
    /// §2.3.1), `FrameTooLarge` for a frame above `max_frame_size`.
    pub fn poll<R: Read>(
        &mut self,
        r: &mut R,
        max_frame_size: u32,
    ) -> Result<Polled, FrameIoError> {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if !self.discard && self.buf.len() >= 8 {
                let mut hdr = [0u8; 8];
                hdr.copy_from_slice(&self.buf[..8]);
                let header = decode_frame_header(&hdr)?;
                if header.size > max_frame_size {
                    return Err(FrameIoError::FrameTooLarge {
                        size: header.size,
                        max: max_frame_size,
                    });
                }
                let size = header.size as usize;
                if self.buf.len() >= size {
                    // Spec §2.3.1.3: the extended header between octet 8
                    // and the body offset is skipped.
                    let body = self.buf[header.body_offset()..size].to_vec();
                    self.buf.drain(..size);
                    return Ok(Polled::Frame(Frame { header, body }));
                }
            }
            match r.read(&mut chunk) {
                Ok(0) => return Ok(Polled::Closed),
                Ok(_) if self.discard => {}
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(Polled::Idle);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(FrameIoError::Io(e)),
            }
        }
    }
}

/// Spec §2.3 — write frame (header + body).
///
/// We set `header.size = 8 + body.len()` automatically and
/// `doff = 2` (no extended header).
///
/// # Errors
/// `Io` on stream error.
pub fn write_frame<W: Write>(
    w: &mut W,
    frame_type: FrameType,
    channel: u16,
    body: &[u8],
) -> Result<(), FrameIoError> {
    let total = 8u32
        .checked_add(
            u32::try_from(body.len()).map_err(|_| FrameIoError::FrameTooLarge {
                size: u32::MAX,
                max: u32::MAX,
            })?,
        )
        .ok_or(FrameIoError::FrameTooLarge {
            size: u32::MAX,
            max: u32::MAX,
        })?;
    let header = FrameHeader {
        size: total,
        doff: 2,
        frame_type,
        channel,
    };
    let hdr_bytes = encode_frame_header(header);
    w.write_all(&hdr_bytes)?;
    w.write_all(body)?;
    Ok(())
}

/// Spec §2.4.5 — empty frame as heartbeat.
///
/// # Errors
/// `Io`.
pub fn write_empty_frame<W: Write>(
    w: &mut W,
    frame_type: FrameType,
    channel: u16,
) -> Result<(), FrameIoError> {
    write_frame(w, frame_type, channel, &[])
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// OASIS AMQP 1.0 §2.2: the protocol header `"AMQP" %d0 %d1 %d0 %d0`
    /// (protocol id 0, version 1.0.0).
    #[test]
    fn amqp10_2_2_the_amqp_protocol_header_is_amqp_0_1_0_0() {
        assert_eq!(AmqpProtocol::Amqp.as_bytes(), *b"AMQP\x00\x01\x00\x00");
        let mut r = Cursor::new(b"AMQP\x00\x01\x00\x00".to_vec());
        let hdr = read_protocol_header(&mut r).unwrap();
        assert_eq!((hdr.protocol, hdr.major), (AmqpProtocol::Amqp, 1));
    }

    /// §5.3.1 (SASL negotiation): the SASL header has protocol id 3.
    #[test]
    fn amqp10_5_3_1_the_sasl_protocol_header_is_amqp_3_1_0_0() {
        assert_eq!(AmqpProtocol::Sasl.as_bytes(), *b"AMQP\x03\x01\x00\x00");
        let mut r = Cursor::new(b"AMQP\x03\x01\x00\x00".to_vec());
        assert_eq!(
            read_protocol_header(&mut r).unwrap().protocol,
            AmqpProtocol::Sasl
        );
    }

    #[test]
    fn invalid_magic_rejected() {
        let bytes = b"NOTM\x00\x01\x00\x00";
        let mut r = Cursor::new(bytes.to_vec());
        let err = read_protocol_header(&mut r).unwrap_err();
        assert!(matches!(err, FrameIoError::InvalidProtocolMagic(_)));
    }

    #[test]
    fn unsupported_protocol_id_rejected() {
        let bytes = b"AMQP\x42\x01\x00\x00";
        let mut r = Cursor::new(bytes.to_vec());
        let err = read_protocol_header(&mut r).unwrap_err();
        assert!(matches!(err, FrameIoError::UnsupportedProtocolId(0x42)));
    }

    #[test]
    fn unsupported_version_rejected() {
        let bytes = b"AMQP\x00\x02\x05\x00";
        let mut r = Cursor::new(bytes.to_vec());
        let err = read_protocol_header(&mut r).unwrap_err();
        assert!(matches!(
            err,
            FrameIoError::UnsupportedVersion {
                major: 2,
                minor: 5,
                revision: 0
            }
        ));
    }

    /// §2.2: the version is major, minor and revision; 1.0.1 is not 1.0.0.
    #[test]
    fn amqp10_2_2_another_revision_is_another_version() {
        let mut r = Cursor::new(b"AMQP\x00\x01\x00\x01".to_vec());
        assert!(matches!(
            read_protocol_header(&mut r).unwrap_err(),
            FrameIoError::UnsupportedVersion {
                major: 1,
                minor: 0,
                revision: 1
            }
        ));
    }

    /// §2.3.1: SIZE (header and body), DOFF 2, type 0x00, channel, body.
    #[test]
    fn amqp10_2_3_1_a_frame_is_its_header_then_the_body() {
        let octets = b"\x00\x00\x00\x0d\x02\x00\x00\x00hello";
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, FrameType::Amqp, 0, b"hello").unwrap();
        assert_eq!(buf, octets);
        let f = read_frame(&mut Cursor::new(octets.to_vec()), 1024).unwrap();
        assert_eq!(f.header.frame_type, FrameType::Amqp);
        assert_eq!(f.header.channel, 0);
        assert_eq!(f.body, b"hello");
    }

    #[test]
    fn frame_too_large_rejected() {
        // Construct a header that declares size=1024.
        let header = FrameHeader::new_amqp(1024, 2, 0);
        let mut buf = encode_frame_header(header).to_vec();
        buf.extend(std::iter::repeat_n(0u8, 1024 - 8));
        let mut r = Cursor::new(buf);
        let err = read_frame(&mut r, 512).unwrap_err();
        assert!(matches!(
            err,
            FrameIoError::FrameTooLarge {
                size: 1024,
                max: 512
            }
        ));
    }

    #[test]
    fn empty_frame_heartbeat() {
        let mut buf: Vec<u8> = Vec::new();
        write_empty_frame(&mut buf, FrameType::Amqp, 0).unwrap();
        // Spec §2.4.5: SIZE=8, DOFF=2, empty body.
        assert_eq!(buf.len(), 8);
        let mut r = Cursor::new(buf);
        let f = read_frame(&mut r, 512).unwrap();
        assert!(f.body.is_empty());
    }

    /// §5.3.1: a SASL frame has type 0x01.
    #[test]
    fn amqp10_5_3_1_a_sasl_frame_has_type_0x01() {
        let octets = b"\x00\x00\x00\x09\x02\x01\x00\x00x";
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, FrameType::Sasl, 0, b"x").unwrap();
        assert_eq!(buf, octets);
        let f = read_frame(&mut Cursor::new(octets.to_vec()), 1024).unwrap();
        assert_eq!(f.header.frame_type, FrameType::Sasl);
        assert_eq!(f.body, b"x");
    }

    /// A read that returns its octets in pieces and times out in between.
    struct Pieces {
        pieces: std::collections::VecDeque<Option<Vec<u8>>>,
    }
    impl Read for Pieces {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.pieces.pop_front() {
                Some(Some(piece)) => {
                    buf[..piece.len()].copy_from_slice(&piece);
                    Ok(piece.len())
                }
                Some(None) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
                None => Ok(0),
            }
        }
    }

    /// §2.3.1: a frame that arrives in pieces around a read time-out is one
    /// frame; two frames in one read are two; the end of the stream is
    /// told apart from a time-out.
    #[test]
    fn amqp10_2_3_1_a_frame_cut_by_a_read_timeout_is_kept_until_its_rest_arrives() {
        let mut one = Vec::new();
        write_frame(&mut one, FrameType::Amqp, 3, b"hello").unwrap();
        let mut two = Vec::new();
        write_frame(&mut two, FrameType::Amqp, 0, b"").unwrap();
        let mut second_read = one[5..].to_vec();
        second_read.extend_from_slice(&two);
        let mut r = Pieces {
            pieces: [Some(one[..5].to_vec()), None, Some(second_read), None]
                .into_iter()
                .collect(),
        };
        let mut reader = FrameReader::default();
        assert!(matches!(reader.poll(&mut r, 512).unwrap(), Polled::Idle));
        let Polled::Frame(f) = reader.poll(&mut r, 512).unwrap() else {
            panic!("the first frame");
        };
        assert_eq!((f.header.channel, f.body.as_slice()), (3, &b"hello"[..]));
        let Polled::Frame(f) = reader.poll(&mut r, 512).unwrap() else {
            panic!("the second frame");
        };
        assert!(f.body.is_empty());
        assert!(matches!(reader.poll(&mut r, 512).unwrap(), Polled::Idle));
        assert!(matches!(reader.poll(&mut r, 512).unwrap(), Polled::Closed));

        // A frame above the limit, and octets that are no frame header
        // (DOFF 1).
        let mut r = Pieces {
            pieces: [Some(one.clone())].into_iter().collect(),
        };
        assert!(matches!(
            FrameReader::default().poll(&mut r, 12),
            Err(FrameIoError::FrameTooLarge { size: 13, max: 12 })
        ));
        let mut r = Pieces {
            pieces: [Some(b"\x00\x00\x00\x08\x01\x00\x00\x00".to_vec())]
                .into_iter()
                .collect(),
        };
        assert!(matches!(
            FrameReader::default().poll(&mut r, 512),
            Err(FrameIoError::Frame(_))
        ));
        // After discard_rest octets are dropped until the stream ends.
        let mut r = Pieces {
            pieces: [Some(one.clone()), None].into_iter().collect(),
        };
        let mut reader = FrameReader::default();
        reader.discard_rest();
        assert!(matches!(reader.poll(&mut r, 512).unwrap(), Polled::Idle));
        assert!(matches!(reader.poll(&mut r, 512).unwrap(), Polled::Closed));
    }

    #[test]
    fn extended_header_skipped() {
        // Spec §2.3.1.3: extended header bytes (4 bytes between
        // header and body) are skipped.
        let header = FrameHeader::new_amqp(16, 3, 0);
        let mut buf = encode_frame_header(header).to_vec();
        buf.extend([0xDEu8, 0xAD, 0xBE, 0xEF]); // ext header (skip)
        buf.extend(b"body"); // 4 bytes body
        let mut r = Cursor::new(buf);
        let f = read_frame(&mut r, 1024).unwrap();
        assert_eq!(f.body, b"body");
    }
}
