// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP-1.0 Frame-IO ueber blocking `std::io::Read`/`Write`.
//!
//! Spec `amqp-1.0-transport`:
//! * §2.2 Protocol Header (`AMQP\0\1\0\0` AMQP / `AMQP\3\1\0\0` SASL).
//! * §2.3 Frame Layout (8-Byte Header + Body).

use std::io::{self, Read, Write};

use zerodds_amqp_bridge::frame::{
    FrameError, FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};

/// Spec §2.2 — AMQP Protocol-ID. Erstes Byte des
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

/// AMQP-Protokoll-Variante des Protocol-Headers.
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

/// Eingelesener Protocol-Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolHeader {
    /// AMQP-Frame-Variante.
    pub protocol: AmqpProtocol,
    /// Major-Version (AMQP 1.0: 1).
    pub major: u8,
    /// Minor-Version (AMQP 1.0: 0).
    pub minor: u8,
    /// Revision.
    pub revision: u8,
}

/// Eingelesener Frame: Header + Body-Bytes.
#[derive(Debug, Clone)]
pub struct Frame {
    /// 8-Byte-Header.
    pub header: FrameHeader,
    /// Body-Bytes (kann leer sein bei Empty-Frames per §2.4.5
    /// Heartbeats).
    pub body: Vec<u8>,
}

/// IO-/Frame-Fehler.
#[derive(Debug)]
pub enum FrameIoError {
    /// `std::io::Error` aus dem Stream.
    Io(io::Error),
    /// Frame-Header-Decoding hat fehlgeschlagen.
    Frame(FrameError),
    /// Protocol-Header magic stimmt nicht.
    InvalidProtocolMagic([u8; 4]),
    /// Unbekannte Protocol-Id (nicht 0x00 oder 0x03).
    UnsupportedProtocolId(u8),
    /// Major/Minor stimmt nicht mit AMQP 1.0.0.
    UnsupportedVersion {
        /// Empfangener Major.
        major: u8,
        /// Empfangener Minor.
        minor: u8,
    },
    /// Frame-Body laenger als die Resource-Limits zulassen.
    FrameTooLarge {
        /// Empfangene Frame-Groesse.
        size: u32,
        /// Konfiguriertes max-frame-size.
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
            Self::UnsupportedVersion { major, minor } => {
                write!(f, "unsupported AMQP version {major}.{minor}")
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

/// Spec §2.2 — Protocol-Header lesen (8 Bytes blocking).
///
/// # Errors
/// `Io` bei TCP-Fehler, `InvalidProtocolMagic` bei falschem
/// Magic-Bytes, `UnsupportedProtocolId`/`UnsupportedVersion` bei
/// nicht-AMQP-1.0-Header.
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
    if major != protocol::MAJOR || minor != protocol::MINOR {
        return Err(FrameIoError::UnsupportedVersion { major, minor });
    }
    Ok(ProtocolHeader {
        protocol,
        major,
        minor,
        revision,
    })
}

/// Spec §2.2 — Protocol-Header schreiben.
///
/// # Errors
/// `Io` bei Stream-Fehler.
pub fn write_protocol_header<W: Write>(w: &mut W, p: AmqpProtocol) -> Result<(), FrameIoError> {
    w.write_all(&p.as_bytes())?;
    Ok(())
}

/// Spec §2.3 — kompletten Frame lesen (Header + Body).
///
/// `max_frame_size` ist der konfigurierte DoS-Cap; Frames die das
/// ueberschreiten werden ohne Body-Read verworfen.
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
    // Spec §2.3.1.3: extended-Header-Bytes liegen zwischen 8 und
    // body_offset. Wir lesen sie, ignorieren den Inhalt aber
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

/// Spec §2.3 — Frame schreiben (Header + Body).
///
/// Wir setzen `header.size = 8 + body.len()` automatisch und
/// `doff = 2` (kein extended-header).
///
/// # Errors
/// `Io` bei Stream-Fehler.
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

/// Spec §2.4.5 — Empty-Frame als Heartbeat.
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

    #[test]
    fn protocol_header_round_trip_amqp() {
        let p = AmqpProtocol::Amqp;
        let bytes = p.as_bytes();
        assert_eq!(&bytes[0..4], b"AMQP");
        assert_eq!(bytes[4], 0x00);
        assert_eq!(bytes[5], 1);
        let mut r = Cursor::new(bytes.to_vec());
        let hdr = read_protocol_header(&mut r).unwrap();
        assert_eq!(hdr.protocol, AmqpProtocol::Amqp);
        assert_eq!(hdr.major, 1);
    }

    #[test]
    fn protocol_header_round_trip_sasl() {
        let p = AmqpProtocol::Sasl;
        let bytes = p.as_bytes();
        assert_eq!(bytes[4], 0x03);
        let mut r = Cursor::new(bytes.to_vec());
        let hdr = read_protocol_header(&mut r).unwrap();
        assert_eq!(hdr.protocol, AmqpProtocol::Sasl);
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
            FrameIoError::UnsupportedVersion { major: 2, minor: 5 }
        ));
    }

    #[test]
    fn frame_round_trip_minimal() {
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, FrameType::Amqp, 0, b"hello").unwrap();
        let mut r = Cursor::new(buf);
        let f = read_frame(&mut r, 1024).unwrap();
        assert_eq!(f.header.frame_type, FrameType::Amqp);
        assert_eq!(f.header.channel, 0);
        assert_eq!(f.body, b"hello");
    }

    #[test]
    fn frame_too_large_rejected() {
        // Konstruiere einen Header der size=1024 deklariert.
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
        // Spec §2.4.5: SIZE=8, DOFF=2, body leer.
        assert_eq!(buf.len(), 8);
        let mut r = Cursor::new(buf);
        let f = read_frame(&mut r, 512).unwrap();
        assert!(f.body.is_empty());
    }

    #[test]
    fn sasl_frame_type_round_trip() {
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, FrameType::Sasl, 0, b"x").unwrap();
        let mut r = Cursor::new(buf);
        let f = read_frame(&mut r, 1024).unwrap();
        assert_eq!(f.header.frame_type, FrameType::Sasl);
        assert_eq!(f.body, b"x");
    }

    #[test]
    fn extended_header_skipped() {
        // Spec §2.3.1.3: extended-Header-Bytes (4 byte zwischen
        // header und body) werden uebersprungen.
        let header = FrameHeader::new_amqp(16, 3, 0);
        let mut buf = encode_frame_header(header).to_vec();
        buf.extend([0xDEu8, 0xAD, 0xBE, 0xEF]); // ext header (skip)
        buf.extend(b"body"); // 4 bytes body
        let mut r = Cursor::new(buf);
        let f = read_frame(&mut r, 1024).unwrap();
        assert_eq!(f.body, b"body");
    }
}
