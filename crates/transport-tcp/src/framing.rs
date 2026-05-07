// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TCP-Framing fuer RTPS-Messages.
//!
//! TCP ist ein Stream-Protokoll — jedes RTPS-Datagramm muss seine
//! Laenge selbst kennzeichnen, damit der Receiver Nachrichten-Grenzen
//! rekonstruieren kann. Wir verwenden ein simples 4-byte-Big-Endian-
//! Length-Prefix (DDS-TCP-PSM §6.2: `SubmessageFlag E` + Length-Feld
//! im Submessage-Header sind fuer TCP irrelevant, es zaehlt nur das
//! outer Frame).
//!
//! Wire-Format:
//!
//! ```text
//! +---------+---------+---------+---------+------- ...
//! |            length (u32, BE)           | RTPS-Message-Bytes
//! +---------+---------+---------+---------+------- ...
//! ```
//!
//! Die Laenge begrenzt ein einzelnes Frame auf `u32::MAX` byte; DoS-
//! Cap in [`MAX_FRAME_SIZE`] kappt tatsaechlich akzeptierte Frames auf
//! ein vernuenftiges Max, um Speicher-Exhaustion durch boese Peers
//! zu verhindern.

use std::io::{Read, Write};

/// Maximale akzeptierte Frame-Größe (16 MiB).
///
/// # Rationale
///
/// TCP als Stream-Transport kann theoretisch beliebig grosse Frames
/// tragen (im Gegensatz zu UDP mit 64 kB Datagram-Cap). Wir cappen hier
/// aus DoS-Schutz — ein bösartiger Peer darf uns nicht mit einer
/// u32-grosse-announcement GB an RAM kosten.
///
/// **RTPS-Fragmentation (DDSI-RTPS §8.3.7)** arbeitet auf
/// Submessage-Ebene: grosse Samples werden vom Writer in
/// `DATA_FRAG`-Submessages aufgeteilt, die dann **jede** in ein
/// TCP-Frame passen. Ein 16-MiB-Cap deckt damit Samples bis weit über
/// typische DDS-Payloads (ROS-Pointcloud ~MB, Industriesensoren <<1 MB)
/// ab — Reassembly macht der Reader via `FragmentAssembler`
/// (crates/rtps/src/fragment_assembler.rs), nicht der Transport.
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Laenge des Length-Prefix in Bytes.
pub const FRAME_HEADER_LEN: usize = 4;

/// Fehler beim Framing.
#[derive(Debug)]
pub enum FramingError {
    /// I/O-Fehler beim Read/Write.
    Io(std::io::Error),
    /// Frame groesser als [`MAX_FRAME_SIZE`].
    FrameTooLarge {
        /// Angekuendigte Laenge.
        announced: u32,
    },
    /// Stream lieferte 0 byte vor dem Ende des Frames (EOF/peer closed).
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

/// Schreibt ein Frame (length-prefix + payload) in den Stream.
///
/// # Errors
/// I/O-Fehler; `FrameTooLarge` wenn `payload.len() > u32::MAX`.
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

/// Liest ein Frame (length-prefix + payload) aus dem Stream.
///
/// Blockiert bis ein komplettes Frame gelesen wurde.
///
/// # Errors
/// I/O-Fehler; `FrameTooLarge` wenn Peer > [`MAX_FRAME_SIZE`] ankuendigt;
/// `UnexpectedEof` bei prematurem EOF.
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
        bad.extend_from_slice(&[1, 2, 3]); // nur 3 von 10
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
