// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC common types — Spec §7.5.1.1.1.
//!
//! This file provides the wire structures that accompany every RPC
//! request or reply:
//!
//! ```text
//! struct SampleIdentity {
//!     octet[16] writer_guid;
//!     unsigned long long sequence_number;
//! };
//! struct RequestHeader {
//!     SampleIdentity request_id;
//!     string instance_name;
//! };
//! enum RemoteExceptionCode_t {
//!     REMOTE_EX_OK,
//!     REMOTE_EX_UNSUPPORTED,
//!     REMOTE_EX_INVALID_ARGUMENT,
//!     REMOTE_EX_OUT_OF_RESOURCES,
//!     REMOTE_EX_UNKNOWN_OPERATION,
//!     REMOTE_EX_UNKNOWN_EXCEPTION,
//!     REMOTE_EX_UNKNOWN_INTERFACE,
//! };
//! struct ReplyHeader {
//!     SampleIdentity related_request_id;
//!     RemoteExceptionCode_t remote_ex;
//! };
//! ```
//!
//! Encoding: **XCDR2** with `Final` extensibility. Spec §7.5.1.1.1 only
//! prescribes the wire layout (no DHEADER, no EMHEADER); we choose
//! Final because of its low overhead cost and because RPC headers are
//! not intended for forward-compat extensions.
//!
//! Wire layout (Final XCDR2, little-endian example):
//!
//! ```text
//! SampleIdentity:
//!   octet[16] writer_guid    -- 16 byte, no padding
//!   align(8)
//!   uint64    sequence_number
//!
//! RequestHeader:
//!   SampleIdentity request_id   -- 24 byte
//!   align(4)
//!   uint32    instance_name_len -- inkl. NUL-Terminator
//!   bytes     instance_name + NUL
//!
//! ReplyHeader:
//!   SampleIdentity related_request_id
//!   align(4)
//!   uint32    remote_ex_kind
//! ```

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};

/// Maximale akzeptierte Wire-Payload pro Header (DoS-Cap).
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Maximale Stringlaenge in Headern.
pub const MAX_STRING_LEN: u32 = 8 * 1024;

// ---------------------------------------------------------------------
// SampleIdentity
// ---------------------------------------------------------------------

/// Spec §7.5.1.1.1 — Identifies a sample by writer GUID + sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SampleIdentity {
    /// 16-byte GUID des absendenden Writers.
    pub writer_guid: [u8; 16],
    /// Sequence-Number des Samples (`uint64`, RTPS-konvention 1-basiert).
    pub sequence_number: u64,
}

impl SampleIdentity {
    /// Constructor.
    #[must_use]
    pub const fn new(writer_guid: [u8; 16], sequence_number: u64) -> Self {
        Self {
            writer_guid,
            sequence_number,
        }
    }

    /// Reserved "unknown" value (Spec §7.5.1.1.1 — all 0).
    pub const UNKNOWN: Self = Self {
        writer_guid: [0u8; 16],
        sequence_number: 0,
    };

    /// XCDR2-Little-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_le(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        encode_sample_identity(&mut out, self, true);
        out
    }

    /// XCDR2-Big-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_be(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        encode_sample_identity(&mut out, self, false);
        out
    }

    /// XCDR2-Decoder, Little-Endian.
    ///
    /// # Errors
    /// `RpcError::Codec` on a too-short buffer.
    pub fn from_cdr_le(bytes: &[u8]) -> RpcResult<Self> {
        check_cap(bytes)?;
        let mut cur = Cursor::new(bytes);
        cur.read_sample_identity(true)
    }

    /// XCDR2-Decoder, Big-Endian.
    ///
    /// # Errors
    /// `RpcError::Codec` on a too-short buffer.
    pub fn from_cdr_be(bytes: &[u8]) -> RpcResult<Self> {
        check_cap(bytes)?;
        let mut cur = Cursor::new(bytes);
        cur.read_sample_identity(false)
    }
}

// ---------------------------------------------------------------------
// RemoteExceptionCode_t
// ---------------------------------------------------------------------

/// Spec §7.5.1.1.1 Tab.7.55 — Server-side error code in `ReplyHeader`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum RemoteExceptionCode {
    /// The operation completed successfully.
    #[default]
    Ok = 0,
    /// The service does not support this operation.
    Unsupported = 1,
    /// The argument was not spec-conformant.
    InvalidArgument = 2,
    /// The server does not have enough resources.
    OutOfResources = 3,
    /// The operation does not exist in the service.
    UnknownOperation = 4,
    /// The operation threw a user exception that the stub does not
    /// know.
    UnknownException = 5,
    /// The service interface is unknown to the server.
    UnknownInterface = 6,
}

impl RemoteExceptionCode {
    /// Converts the wire discriminator `u32` into the enum.
    ///
    /// # Errors
    /// `RpcError::UnknownExceptionCode` on an unknown discriminator.
    pub fn from_u32(v: u32) -> RpcResult<Self> {
        match v {
            0 => Ok(Self::Ok),
            1 => Ok(Self::Unsupported),
            2 => Ok(Self::InvalidArgument),
            3 => Ok(Self::OutOfResources),
            4 => Ok(Self::UnknownOperation),
            5 => Ok(Self::UnknownException),
            6 => Ok(Self::UnknownInterface),
            other => Err(RpcError::UnknownExceptionCode(other)),
        }
    }

    /// Returns the wire discriminator.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

// ---------------------------------------------------------------------
// RequestHeader
// ---------------------------------------------------------------------

/// Spec §7.5.1.1.1 — Pro Sample im Request-Topic prepended.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestHeader {
    /// Unique ID of this request.
    pub request_id: SampleIdentity,
    /// Optional service instance name (empty if not set).
    pub instance_name: String,
}

impl RequestHeader {
    /// Constructor.
    #[must_use]
    pub fn new(request_id: SampleIdentity, instance_name: impl Into<String>) -> Self {
        Self {
            request_id,
            instance_name: instance_name.into(),
        }
    }

    /// XCDR2-Little-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_le(&self) -> Vec<u8> {
        encode_request_header(self, true)
    }

    /// XCDR2-Big-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_be(&self) -> Vec<u8> {
        encode_request_header(self, false)
    }

    /// XCDR2-Decoder, Little-Endian.
    ///
    /// # Errors
    /// `RpcError::Codec` on a too-short or invalid buffer.
    pub fn from_cdr_le(bytes: &[u8]) -> RpcResult<Self> {
        check_cap(bytes)?;
        let mut cur = Cursor::new(bytes);
        let request_id = cur.read_sample_identity(true)?;
        let instance_name = cur.read_string(true)?;
        Ok(Self {
            request_id,
            instance_name,
        })
    }

    /// XCDR2-Decoder, Big-Endian.
    ///
    /// # Errors
    /// `RpcError::Codec` on a too-short or invalid buffer.
    pub fn from_cdr_be(bytes: &[u8]) -> RpcResult<Self> {
        check_cap(bytes)?;
        let mut cur = Cursor::new(bytes);
        let request_id = cur.read_sample_identity(false)?;
        let instance_name = cur.read_string(false)?;
        Ok(Self {
            request_id,
            instance_name,
        })
    }
}

// ---------------------------------------------------------------------
// ReplyHeader
// ---------------------------------------------------------------------

/// Spec §7.5.1.1.1 — Pro Sample im Reply-Topic prepended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplyHeader {
    /// References the `request_id` of the corresponding request.
    pub related_request_id: SampleIdentity,
    /// Server-Side Result-Code.
    pub remote_ex: RemoteExceptionCode,
}

impl ReplyHeader {
    /// Constructor.
    #[must_use]
    pub const fn new(related_request_id: SampleIdentity, remote_ex: RemoteExceptionCode) -> Self {
        Self {
            related_request_id,
            remote_ex,
        }
    }

    /// XCDR2-Little-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_le(&self) -> Vec<u8> {
        encode_reply_header(self, true)
    }

    /// XCDR2-Big-Endian-Encoder.
    #[must_use]
    pub fn to_cdr_be(&self) -> Vec<u8> {
        encode_reply_header(self, false)
    }

    /// XCDR2-Decoder, Little-Endian.
    ///
    /// # Errors
    /// `RpcError::Codec` on a too-short or invalid buffer.
    /// `RpcError::UnknownExceptionCode` on an unknown discriminator.
    pub fn from_cdr_le(bytes: &[u8]) -> RpcResult<Self> {
        check_cap(bytes)?;
        let mut cur = Cursor::new(bytes);
        let related_request_id = cur.read_sample_identity(true)?;
        let raw = cur.read_u32(true)?;
        let remote_ex = RemoteExceptionCode::from_u32(raw)?;
        Ok(Self {
            related_request_id,
            remote_ex,
        })
    }

    /// XCDR2-Decoder, Big-Endian.
    ///
    /// # Errors
    /// `RpcError::Codec` on a too-short or invalid buffer.
    /// `RpcError::UnknownExceptionCode` on an unknown discriminator.
    pub fn from_cdr_be(bytes: &[u8]) -> RpcResult<Self> {
        check_cap(bytes)?;
        let mut cur = Cursor::new(bytes);
        let related_request_id = cur.read_sample_identity(false)?;
        let raw = cur.read_u32(false)?;
        let remote_ex = RemoteExceptionCode::from_u32(raw)?;
        Ok(Self {
            related_request_id,
            remote_ex,
        })
    }
}

// ---------------------------------------------------------------------
// XCDR2-Codec (Final-Extensibility, primitive-only)
// ---------------------------------------------------------------------
//
// XCDR2 specifics vs XCDR1:
//   * Alignment cap = 4 bytes (instead of 8). uint64 is therefore reduced
//     from 8-byte alignment -> 4 in XCDR2.
//   * String: uint32 length incl. trailing NUL + UTF-8 bytes + NUL.
//   * Final extensibility: no DHEADER before the struct.
//
// We implement this hand-rolled (analogous to crates/security/src/token.rs)
// instead of consuming zerodds-cdr — zerodds-cdr is no_std and the foundation
// stub here needs nothing beyond that.

fn check_cap(bytes: &[u8]) -> RpcResult<()> {
    if bytes.len() > MAX_HEADER_BYTES {
        return Err(RpcError::PayloadTooLarge {
            got: bytes.len(),
            max: MAX_HEADER_BYTES,
        });
    }
    Ok(())
}

fn align_to(out: &mut Vec<u8>, n: usize) {
    let pad = (n - out.len() % n) % n;
    for _ in 0..pad {
        out.push(0);
    }
}

fn encode_u32(out: &mut Vec<u8>, v: u32, le: bool) {
    align_to(out, 4);
    if le {
        out.extend_from_slice(&v.to_le_bytes());
    } else {
        out.extend_from_slice(&v.to_be_bytes());
    }
}

fn encode_u64_xcdr2(out: &mut Vec<u8>, v: u64, le: bool) {
    // XCDR2: uint64 alignment-Cap = 4 byte.
    align_to(out, 4);
    if le {
        out.extend_from_slice(&v.to_le_bytes());
    } else {
        out.extend_from_slice(&v.to_be_bytes());
    }
}

fn encode_string(out: &mut Vec<u8>, s: &str, le: bool) {
    let bytes = s.as_bytes();
    let len = (bytes.len() + 1) as u32;
    encode_u32(out, len, le);
    out.extend_from_slice(bytes);
    out.push(0);
}

fn encode_sample_identity(out: &mut Vec<u8>, id: &SampleIdentity, le: bool) {
    // octet[16] hat alignment 1 — direkt einfuegen.
    out.extend_from_slice(&id.writer_guid);
    encode_u64_xcdr2(out, id.sequence_number, le);
}

fn encode_request_header(h: &RequestHeader, le: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    encode_sample_identity(&mut out, &h.request_id, le);
    encode_string(&mut out, &h.instance_name, le);
    out
}

fn encode_reply_header(h: &ReplyHeader, le: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    encode_sample_identity(&mut out, &h.related_request_id, le);
    encode_u32(&mut out, h.remote_ex.as_u32(), le);
    out
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn align_to(&mut self, n: usize) {
        let pad = (n - self.pos % n) % n;
        self.pos = self.pos.saturating_add(pad);
    }

    fn ensure(&self, need: usize) -> RpcResult<()> {
        if self.pos.saturating_add(need) > self.buf.len() {
            return Err(RpcError::codec("truncated buffer"));
        }
        Ok(())
    }

    fn read_u32(&mut self, le: bool) -> RpcResult<u32> {
        self.align_to(4);
        self.ensure(4)?;
        let raw = [
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ];
        self.pos += 4;
        Ok(if le {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        })
    }

    fn read_u64_xcdr2(&mut self, le: bool) -> RpcResult<u64> {
        // XCDR2 Alignment-Cap = 4 byte.
        self.align_to(4);
        self.ensure(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&self.buf[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(if le {
            u64::from_le_bytes(raw)
        } else {
            u64::from_be_bytes(raw)
        })
    }

    fn read_string(&mut self, le: bool) -> RpcResult<String> {
        let len = self.read_u32(le)?;
        if len > MAX_STRING_LEN {
            return Err(RpcError::codec("string exceeds cap"));
        }
        if len == 0 {
            return Err(RpcError::codec("zero-length string body"));
        }
        self.ensure(len as usize)?;
        let body = &self.buf[self.pos..self.pos + len as usize];
        self.pos += len as usize;
        if body.last() != Some(&0) {
            return Err(RpcError::codec("string missing trailing NUL"));
        }
        let rest = &body[..body.len() - 1];
        let s = core::str::from_utf8(rest)
            .map_err(|_| RpcError::codec("string not UTF-8"))?
            .to_string();
        Ok(s)
    }

    fn read_sample_identity(&mut self, le: bool) -> RpcResult<SampleIdentity> {
        self.ensure(16)?;
        let mut writer_guid = [0u8; 16];
        writer_guid.copy_from_slice(&self.buf[self.pos..self.pos + 16]);
        self.pos += 16;
        let sequence_number = self.read_u64_xcdr2(le)?;
        Ok(SampleIdentity {
            writer_guid,
            sequence_number,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample_id() -> SampleIdentity {
        SampleIdentity::new(
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
                0x0F, 0x10,
            ],
            0xDEAD_BEEF_CAFE_BABE,
        )
    }

    #[test]
    fn sample_identity_roundtrip_le() {
        let id = sample_id();
        let bytes = id.to_cdr_le();
        assert_eq!(bytes.len(), 24);
        let back = SampleIdentity::from_cdr_le(&bytes).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn sample_identity_roundtrip_be() {
        let id = sample_id();
        let bytes = id.to_cdr_be();
        let back = SampleIdentity::from_cdr_be(&bytes).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn sample_identity_le_be_streams_differ() {
        let id = sample_id();
        let le = id.to_cdr_le();
        let be = id.to_cdr_be();
        assert_ne!(le, be);
    }

    #[test]
    fn sample_identity_unknown_constant_is_zero() {
        let id = SampleIdentity::UNKNOWN;
        assert_eq!(id.writer_guid, [0u8; 16]);
        assert_eq!(id.sequence_number, 0);
    }

    #[test]
    fn sample_identity_truncated_buffer_is_error() {
        let bytes = vec![0u8; 23];
        let err = SampleIdentity::from_cdr_le(&bytes).unwrap_err();
        assert!(matches!(err, RpcError::Codec(_)));
    }

    #[test]
    fn request_header_roundtrip_le() {
        let h = RequestHeader::new(sample_id(), "calc-instance-1");
        let bytes = h.to_cdr_le();
        let back = RequestHeader::from_cdr_le(&bytes).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn request_header_roundtrip_be() {
        let h = RequestHeader::new(sample_id(), "calc-instance-1");
        let bytes = h.to_cdr_be();
        let back = RequestHeader::from_cdr_be(&bytes).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn request_header_empty_instance_name_roundtrip() {
        let h = RequestHeader::new(sample_id(), "");
        let bytes = h.to_cdr_le();
        let back = RequestHeader::from_cdr_le(&bytes).unwrap();
        assert_eq!(h, back);
        assert!(back.instance_name.is_empty());
    }

    #[test]
    fn request_header_string_missing_nul_rejected() {
        // Forge: id (24 byte) + len=1 + body byte != 0
        let mut bytes = sample_id().to_cdr_le();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.push(b'A');
        let err = RequestHeader::from_cdr_le(&bytes).unwrap_err();
        assert!(matches!(err, RpcError::Codec(_)));
    }

    #[test]
    fn request_header_zero_length_string_rejected() {
        let mut bytes = sample_id().to_cdr_le();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let err = RequestHeader::from_cdr_le(&bytes).unwrap_err();
        assert!(matches!(err, RpcError::Codec(_)));
    }

    #[test]
    fn request_header_invalid_utf8_rejected() {
        let mut bytes = sample_id().to_cdr_le();
        // len=3 (incl NUL): bytes 0xFF 0xFE 0x00
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&[0xFF, 0xFE, 0x00]);
        let err = RequestHeader::from_cdr_le(&bytes).unwrap_err();
        assert!(matches!(err, RpcError::Codec(_)));
    }

    #[test]
    fn reply_header_roundtrip_all_codes() {
        for code in [
            RemoteExceptionCode::Ok,
            RemoteExceptionCode::Unsupported,
            RemoteExceptionCode::InvalidArgument,
            RemoteExceptionCode::OutOfResources,
            RemoteExceptionCode::UnknownOperation,
            RemoteExceptionCode::UnknownException,
            RemoteExceptionCode::UnknownInterface,
        ] {
            let h = ReplyHeader::new(sample_id(), code);
            let le = h.to_cdr_le();
            let be = h.to_cdr_be();
            assert_eq!(h, ReplyHeader::from_cdr_le(&le).unwrap());
            assert_eq!(h, ReplyHeader::from_cdr_be(&be).unwrap());
        }
    }

    #[test]
    fn reply_header_unknown_discriminator_is_error() {
        let mut bytes = sample_id().to_cdr_le();
        bytes.extend_from_slice(&999u32.to_le_bytes());
        let err = ReplyHeader::from_cdr_le(&bytes).unwrap_err();
        assert_eq!(err, RpcError::UnknownExceptionCode(999));
    }

    #[test]
    fn remote_exception_code_as_u32_round_trips() {
        for v in 0u32..=6 {
            let code = RemoteExceptionCode::from_u32(v).unwrap();
            assert_eq!(code.as_u32(), v);
        }
    }

    #[test]
    fn remote_exception_code_default_is_ok() {
        assert_eq!(RemoteExceptionCode::default(), RemoteExceptionCode::Ok);
    }

    #[test]
    fn dos_cap_rejects_oversized_buffer() {
        let big = vec![0u8; MAX_HEADER_BYTES + 1];
        let err = RequestHeader::from_cdr_le(&big).unwrap_err();
        assert!(matches!(
            err,
            RpcError::PayloadTooLarge {
                got: _,
                max: MAX_HEADER_BYTES
            }
        ));
    }

    #[test]
    fn xcdr2_layout_sample_identity_le_is_24_bytes_no_padding() {
        // GUID 16 bytes + uint64 8 bytes (XCDR2 cap=4, so no
        // additional 4 bytes of padding).
        let id = SampleIdentity::new([0xAB; 16], 0x0102_0304_0506_0708);
        let bytes = id.to_cdr_le();
        assert_eq!(bytes.len(), 24);
        // Letzten 8 byte = LE-Encoding des u64.
        assert_eq!(&bytes[16..24], &0x0102_0304_0506_0708u64.to_le_bytes());
    }

    #[test]
    fn xcdr2_layout_string_includes_nul() {
        // class_id="A" (1 char + NUL = len=2) via RequestHeader.
        let h = RequestHeader::new(SampleIdentity::UNKNOWN, "A");
        let bytes = h.to_cdr_le();
        // Layout: 16 byte GUID + 8 byte uint64 + uint32 len=2 + 'A' + NUL
        // = 24 + 4 + 2 = 30 byte.
        assert_eq!(bytes.len(), 30);
        assert_eq!(&bytes[24..28], &2u32.to_le_bytes());
        assert_eq!(bytes[28], b'A');
        assert_eq!(bytes[29], 0);
    }
}
