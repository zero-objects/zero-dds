// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CancelRequest message — spec §15.4.4.
//!
//! ```text
//! struct CancelRequestHeader {
//!     unsigned long request_id;
//! };
//! ```
//!
//! Very small message: only the `request_id` of the original request
//! to be cancelled. Spec §15.4.4: the server MUST cease processing,
//! but must not retract a reply that has already been sent.

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::GiopResult;

/// CancelRequest body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelRequest {
    /// `request_id` of the request to be cancelled.
    pub request_id: u32,
}

impl CancelRequest {
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
        let request_id = r.read_u32()?;
        Ok(Self { request_id })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn round_trip() {
        let c = CancelRequest { request_id: 42 };
        let mut w = BufferWriter::new(Endianness::Big);
        c.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes, &[0, 0, 0, 42]);
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(CancelRequest::decode(&mut r).unwrap(), c);
    }
}
