// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endianness-Helpers fuer XRCE-Submessage-Bodies.
//!
//! XRCE-Submessage-Bodies sind nach Spec §8.3.4 entweder Little-Endian
//! oder Big-Endian (E-Flag). Innerhalb eines Streams ist die Endianness
//! konstant (§8.2.3). Submessage-spezifische Skalare-Felder (z.B. die
//! `short`-Felder im ACKNACK-Body) folgen dieser Endianness.
//!
//! Diese Datei kapselt die wenigen Primitiven, die wir brauchen:
//! `u8`, `u16`, `u32`, sowie die kanonischen Read/Write-Helfer.
//!
//! Volle XCDR2-Strukturen werden in C6.2.B direkt ueber `zerodds-cdr`
//! kodiert; in C6.2.A behandeln wir die Submessage-Payloads als
//! opake Byte-Sequenz mit explizit benannten skalaren Praefix-Feldern.

use crate::error::XrceError;

/// Wire-Endianness fuer XRCE-Submessage-Bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Endianness {
    /// Big-Endian (E-Flag = 0).
    Big,
    /// Little-Endian (E-Flag = 1) — Default in fast allen XRCE-Stacks.
    #[default]
    Little,
}

impl Endianness {
    /// `true`, wenn LE.
    #[must_use]
    pub fn is_little(self) -> bool {
        matches!(self, Self::Little)
    }
}

/// Schreibt `u16` in `out[..2]` mit gewuenschter Endianness.
///
/// # Errors
/// `WriteOverflow`, wenn `out.len() < 2`.
pub fn write_u16(out: &mut [u8], value: u16, e: Endianness) -> Result<(), XrceError> {
    if out.len() < 2 {
        return Err(XrceError::WriteOverflow {
            needed: 2,
            available: out.len(),
        });
    }
    let bytes = if e.is_little() {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    out[..2].copy_from_slice(&bytes);
    Ok(())
}

/// Liest `u16` aus `bytes[..2]` mit gewuenschter Endianness.
///
/// # Errors
/// `UnexpectedEof`, wenn `bytes.len() < 2`.
pub fn read_u16(bytes: &[u8], e: Endianness) -> Result<u16, XrceError> {
    if bytes.len() < 2 {
        return Err(XrceError::UnexpectedEof {
            needed: 2,
            offset: bytes.len(),
        });
    }
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&bytes[..2]);
    if e.is_little() {
        Ok(u16::from_le_bytes(buf))
    } else {
        Ok(u16::from_be_bytes(buf))
    }
}

/// Schreibt `i16` in `out[..2]`.
///
/// # Errors
/// `WriteOverflow`, wenn `out.len() < 2`.
pub fn write_i16(out: &mut [u8], value: i16, e: Endianness) -> Result<(), XrceError> {
    write_u16(out, value as u16, e)
}

/// Liest `i16` aus `bytes[..2]`.
///
/// # Errors
/// `UnexpectedEof`, wenn `bytes.len() < 2`.
pub fn read_i16(bytes: &[u8], e: Endianness) -> Result<i16, XrceError> {
    Ok(read_u16(bytes, e)? as i16)
}

/// Schreibt `u32` in `out[..4]`.
///
/// # Errors
/// `WriteOverflow`, wenn `out.len() < 4`.
pub fn write_u32(out: &mut [u8], value: u32, e: Endianness) -> Result<(), XrceError> {
    if out.len() < 4 {
        return Err(XrceError::WriteOverflow {
            needed: 4,
            available: out.len(),
        });
    }
    let bytes = if e.is_little() {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    out[..4].copy_from_slice(&bytes);
    Ok(())
}

/// Liest `u32` aus `bytes[..4]`.
///
/// # Errors
/// `UnexpectedEof`, wenn `bytes.len() < 4`.
pub fn read_u32(bytes: &[u8], e: Endianness) -> Result<u32, XrceError> {
    if bytes.len() < 4 {
        return Err(XrceError::UnexpectedEof {
            needed: 4,
            offset: bytes.len(),
        });
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&bytes[..4]);
    if e.is_little() {
        Ok(u32::from_le_bytes(buf))
    } else {
        Ok(u32::from_be_bytes(buf))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn u16_le_roundtrip() {
        let mut buf = [0u8; 2];
        write_u16(&mut buf, 0x1234, Endianness::Little).unwrap();
        assert_eq!(buf, [0x34, 0x12]);
        assert_eq!(read_u16(&buf, Endianness::Little).unwrap(), 0x1234);
    }

    #[test]
    fn u16_be_roundtrip() {
        let mut buf = [0u8; 2];
        write_u16(&mut buf, 0x1234, Endianness::Big).unwrap();
        assert_eq!(buf, [0x12, 0x34]);
        assert_eq!(read_u16(&buf, Endianness::Big).unwrap(), 0x1234);
    }

    #[test]
    fn u16_overflow_on_short_buffer() {
        let mut buf = [0u8; 1];
        let res = write_u16(&mut buf, 1, Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::WriteOverflow { needed: 2, .. })
        ));
    }

    #[test]
    fn i16_negative_roundtrip() {
        let mut buf = [0u8; 2];
        write_i16(&mut buf, -1, Endianness::Little).unwrap();
        assert_eq!(buf, [0xFF, 0xFF]);
        assert_eq!(read_i16(&buf, Endianness::Little).unwrap(), -1);
    }

    #[test]
    fn u32_roundtrip_le() {
        let mut buf = [0u8; 4];
        write_u32(&mut buf, 0xDEADBEEF, Endianness::Little).unwrap();
        assert_eq!(buf, [0xEF, 0xBE, 0xAD, 0xDE]);
        assert_eq!(read_u32(&buf, Endianness::Little).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn read_u16_short_buffer_returns_eof() {
        let buf = [0u8; 1];
        let res = read_u16(&buf, Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 2, .. })
        ));
    }
}
