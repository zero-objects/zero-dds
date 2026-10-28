// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Payload types for the tests of this crate.

extern crate alloc;

use alloc::vec::Vec;

use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError};

/// A `Call` union stand-in with an 8-octet member behind a 4-octet one:
/// the shape for which classic CDR (XCDR1) and XCDR2 differ.
///
/// It lays itself out the way a generated codec does — alignment counted
/// from the start of the sample it continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wide {
    pub(crate) a: u32,
    pub(crate) b: u64,
}

fn pad_to(out: &mut Vec<u8>, align: usize) {
    while !out.len().is_multiple_of(align) {
        out.push(0);
    }
}

fn read<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], DecodeError> {
    bytes
        .get(at..at + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(DecodeError::Invalid { what: "truncated" })
}

impl Wide {
    fn write(&self, out: &mut Vec<u8>, cap: usize) {
        pad_to(out, 4);
        out.extend_from_slice(&self.a.to_le_bytes());
        pad_to(out, cap);
        out.extend_from_slice(&self.b.to_le_bytes());
    }

    fn read_at(bytes: &[u8], start: usize, cap: usize) -> Result<Self, DecodeError> {
        let a_at = start.next_multiple_of(4);
        let a = u32::from_le_bytes(read(bytes, a_at)?);
        let b_at = (a_at + 4).next_multiple_of(cap);
        let b = u64::from_le_bytes(read(bytes, b_at)?);
        Ok(Self { a, b })
    }
}

impl DdsType for Wide {
    const TYPE_NAME: &'static str = "test::Wide";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.write(out, 4);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::read_at(bytes, 0, 4)
    }

    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.write(out, 8);
        Ok(())
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::read_at(bytes, 0, 8)
    }

    fn encode_continuing(&self, prefix: &mut Vec<u8>, xcdr1: bool) -> Result<(), EncodeError> {
        self.write(prefix, if xcdr1 { 8 } else { 4 });
        Ok(())
    }

    fn decode_continuing(bytes: &[u8], offset: usize, xcdr1: bool) -> Result<Self, DecodeError> {
        Self::read_at(bytes, offset, if xcdr1 { 8 } else { 4 })
    }
}

use crate::common_types::{ReplyHeader, RequestHeader, SampleIdentity};
use crate::error::RpcResult;
use crate::header_pair::{ReplyWithHeader, RequestWithHeader};

/// A request type with its own header (§7.3.1.4): `RequestHeader header;
/// unsigned long a; unsigned long long b;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeaderRequest {
    pub(crate) header: RequestHeader,
    pub(crate) a: u32,
    pub(crate) b: u64,
}

impl HeaderRequest {
    pub(crate) fn new(a: u32, b: u64) -> Self {
        Self {
            header: RequestHeader::new(SampleIdentity::UNKNOWN, ""),
            a,
            b,
        }
    }
}

impl DdsType for HeaderRequest {
    const TYPE_NAME: &'static str = "test::HeaderRequest";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_continuing(out, false)
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_continuing(bytes, 0, false)
    }

    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_continuing(out, true)
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_continuing(bytes, 0, true)
    }

    fn encode_continuing(&self, prefix: &mut Vec<u8>, xcdr1: bool) -> Result<(), EncodeError> {
        let header = self.header.to_cdr_le().map_err(|_| EncodeError::Invalid {
            what: "request header",
        })?;
        prefix.extend_from_slice(&header);
        pad_to(prefix, 4);
        prefix.extend_from_slice(&self.a.to_le_bytes());
        pad_to(prefix, if xcdr1 { 8 } else { 4 });
        prefix.extend_from_slice(&self.b.to_le_bytes());
        Ok(())
    }

    fn decode_continuing(bytes: &[u8], offset: usize, xcdr1: bool) -> Result<Self, DecodeError> {
        let rest = bytes
            .get(offset..)
            .ok_or(DecodeError::Invalid { what: "offset" })?;
        let header = RequestHeader::from_cdr_le(rest)
            .map_err(|_| DecodeError::Invalid { what: "header" })?;
        let header_len = 24 + 4 + header.instance_name.len() + 1;
        let a_at = (offset + header_len).next_multiple_of(4);
        let a = u32::from_le_bytes(read(bytes, a_at)?);
        let b_at = (a_at + 4).next_multiple_of(if xcdr1 { 8 } else { 4 });
        let b = u64::from_le_bytes(read(bytes, b_at)?);
        Ok(Self { header, a, b })
    }
}

impl RequestWithHeader for HeaderRequest {
    fn with_request_header(&self, header: &RequestHeader) -> RpcResult<Self> {
        Ok(Self {
            header: header.clone(),
            ..self.clone()
        })
    }
}

/// A reply type with its own header: `ReplyHeader header; unsigned long
/// long r;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeaderReply {
    pub(crate) header: ReplyHeader,
    pub(crate) r: u64,
}

impl Default for HeaderReply {
    fn default() -> Self {
        Self {
            header: ReplyHeader::new(
                SampleIdentity::UNKNOWN,
                crate::common_types::RemoteExceptionCode::Ok,
            ),
            r: 0,
        }
    }
}

impl DdsType for HeaderReply {
    const TYPE_NAME: &'static str = "test::HeaderReply";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_continuing(out, false)
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_continuing(bytes, 0, false)
    }

    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_continuing(out, true)
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_continuing(bytes, 0, true)
    }

    fn encode_continuing(&self, prefix: &mut Vec<u8>, xcdr1: bool) -> Result<(), EncodeError> {
        prefix.extend_from_slice(&self.header.to_cdr_le());
        pad_to(prefix, if xcdr1 { 8 } else { 4 });
        prefix.extend_from_slice(&self.r.to_le_bytes());
        Ok(())
    }

    fn decode_continuing(bytes: &[u8], offset: usize, xcdr1: bool) -> Result<Self, DecodeError> {
        let rest = bytes
            .get(offset..)
            .ok_or(DecodeError::Invalid { what: "offset" })?;
        let header =
            ReplyHeader::from_cdr_le(rest).map_err(|_| DecodeError::Invalid { what: "header" })?;
        let r_at = (offset + 28).next_multiple_of(if xcdr1 { 8 } else { 4 });
        let r = u64::from_le_bytes(read(bytes, r_at)?);
        Ok(Self { header, r })
    }
}

impl ReplyWithHeader for HeaderReply {
    fn with_reply_header(&self, header: &ReplyHeader) -> RpcResult<Self> {
        Ok(Self {
            header: *header,
            ..self.clone()
        })
    }
}
