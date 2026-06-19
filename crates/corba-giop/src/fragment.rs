// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Fragment message — spec §15.4.9.
//!
//! GIOP 1.1: the fragment body follows directly after the GIOP header.
//! Only allowed for Request/Reply.
//!
//! GIOP 1.2: the fragment body begins with a `request_id` header
//! and is allowed for all message types:
//! ```text
//! struct FragmentHeader_1_2 {
//!     unsigned long request_id;
//! };
//! ```
//!
//! Spec §15.4.9, normative:
//! * The sender sets the `fragment_bit` in the `flags` octet of all but
//!   the last fragment(s) of the message.
//! * The last fragment has the bit at 0.
//! * `request_id` must be consistent.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::GiopResult;
use crate::version::Version;

/// Fragment header (GIOP 1.2 only). GIOP 1.1 fragment messages
/// have no `request_id` prefix; the caller must do the association
/// via the stream context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentHeader {
    /// `request_id` of the fragment (GIOP 1.2+).
    pub request_id: u32,
}

impl FragmentHeader {
    /// CDR encode.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode(&self, w: &mut BufferWriter) -> GiopResult<()> {
        w.write_u32(self.request_id)?;
        Ok(())
    }

    /// CDR decode.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut BufferReader<'_>) -> GiopResult<Self> {
        Ok(Self {
            request_id: r.read_u32()?,
        })
    }
}

/// Fragment message body — version-uniform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// Present in GIOP 1.2, `None` in 1.1 (no header form).
    pub header: Option<FragmentHeader>,
    /// Body bytes.
    pub body: Vec<u8>,
}

impl Fragment {
    /// CDR encode into a `BufferWriter`. In GIOP 1.2 the
    /// `request_id` is encoded first; in GIOP 1.1 only the
    /// body bytes.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode(&self, version: Version, w: &mut BufferWriter) -> GiopResult<()> {
        if version.uses_v1_2_request_layout() {
            if let Some(h) = &self.header {
                h.encode(w)?;
            }
        }
        w.write_bytes(&self.body)?;
        Ok(())
    }

    /// CDR decode.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(version: Version, r: &mut BufferReader<'_>) -> GiopResult<Self> {
        let header = if version.uses_v1_2_request_layout() {
            Some(FragmentHeader::decode(r)?)
        } else {
            None
        };
        let body = r.read_bytes(r.remaining())?.to_vec();
        Ok(Self { header, body })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn round_trip_giop_1_2_with_header() {
        let f = Fragment {
            header: Some(FragmentHeader { request_id: 42 }),
            body: alloc::vec![1, 2, 3],
        };
        let mut w = BufferWriter::new(Endianness::Big);
        f.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = Fragment::decode(Version::V1_2, &mut r).unwrap();
        assert_eq!(decoded, f);
    }

    #[test]
    fn round_trip_giop_1_1_without_header() {
        let f = Fragment {
            header: None,
            body: alloc::vec![0xa, 0xb, 0xc, 0xd],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        f.encode(Version::V1_1, &mut w).unwrap();
        let bytes = w.into_bytes();
        // GIOP 1.1 — no header, only body bytes.
        assert_eq!(bytes, alloc::vec![0xa, 0xb, 0xc, 0xd]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = Fragment::decode(Version::V1_1, &mut r).unwrap();
        assert_eq!(decoded, f);
    }
}
