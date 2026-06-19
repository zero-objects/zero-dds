// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endianness helpers for XRCE submessage bodies.
//!
//! XRCE submessage bodies are, per Spec §8.3.4, either little-endian
//! or big-endian (E-flag). Within a stream the endianness is
//! constant (§8.2.3). Submessage-specific scalar fields (e.g. the
//! `short` fields in the ACKNACK body) follow this endianness.
//!
//! This file encapsulates the few primitives we need:
//! `u8`, `u16`, `u32`, as well as the canonical read/write helpers.
//!
//! Full XCDR2 structures are encoded directly via `zerodds-cdr` in
//! C6.2.B; in C6.2.A we treat the submessage payloads as an
//! opaque byte sequence with explicitly named scalar prefix fields.

use crate::error::XrceError;

/// Wire endianness for XRCE submessage bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Endianness {
    /// Big-endian (E-flag = 0).
    Big,
    /// Little-endian (E-flag = 1) — default in almost all XRCE stacks.
    #[default]
    Little,
}

impl Endianness {
    /// `true` if LE.
    #[must_use]
    pub fn is_little(self) -> bool {
        matches!(self, Self::Little)
    }
}

/// Writes `u16` into `out[..2]` with the desired endianness.
///
/// # Errors
/// `WriteOverflow` if `out.len() < 2`.
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

/// Reads `u16` from `bytes[..2]` with the desired endianness.
///
/// # Errors
/// `UnexpectedEof` if `bytes.len() < 2`.
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

/// Writes `i16` into `out[..2]`.
///
/// # Errors
/// `WriteOverflow` if `out.len() < 2`.
pub fn write_i16(out: &mut [u8], value: i16, e: Endianness) -> Result<(), XrceError> {
    write_u16(out, value as u16, e)
}

/// Reads `i16` from `bytes[..2]`.
///
/// # Errors
/// `UnexpectedEof` if `bytes.len() < 2`.
pub fn read_i16(bytes: &[u8], e: Endianness) -> Result<i16, XrceError> {
    Ok(read_u16(bytes, e)? as i16)
}

/// Writes `u32` into `out[..4]`.
///
/// # Errors
/// `WriteOverflow` if `out.len() < 4`.
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

/// Reads `u32` from `bytes[..4]`.
///
/// # Errors
/// `UnexpectedEof` if `bytes.len() < 4`.
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
