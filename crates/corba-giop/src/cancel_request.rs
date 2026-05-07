// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CancelRequest-Message — Spec §15.4.4.
//!
//! ```text
//! struct CancelRequestHeader {
//!     unsigned long request_id;
//! };
//! ```
//!
//! Sehr kleine Message: nur die `request_id` der zu cancellenden
//! Original-Request. Spec §15.4.4: Server MUST cease processing,
//! darf aber bereits gesendete Reply nicht zurueckziehen.

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::GiopResult;

/// CancelRequest-Body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelRequest {
    /// `request_id` der zu cancelnden Request.
    pub request_id: u32,
}

impl CancelRequest {
    /// CDR-Encode.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, w: &mut BufferWriter) -> GiopResult<()> {
        w.write_u32(self.request_id)?;
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
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
