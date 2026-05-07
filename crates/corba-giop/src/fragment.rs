// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Fragment-Message — Spec §15.4.9.
//!
//! GIOP 1.1: Fragment-Body folgt direkt nach dem GIOP-Header. Nur
//! fuer Request/Reply zulaessig.
//!
//! GIOP 1.2: Fragment-Body beginnt mit einem `request_id`-Header
//! und ist fuer alle Message-Types zulaessig:
//! ```text
//! struct FragmentHeader_1_2 {
//!     unsigned long request_id;
//! };
//! ```
//!
//! Spec §15.4.9 normativ:
//! * Sender setzt das `fragment_bit` im `flags`-Octet aller bis-
//!   auf-die-letzte Fragment(s) der Message.
//! * Letztes Fragment hat das Bit auf 0.
//! * `request_id` muss konsistent sein.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::GiopResult;
use crate::version::Version;

/// Fragment-Header (GIOP 1.2 only). GIOP 1.1 Fragment-Messages
/// haben keinen `request_id`-Prefix; der Caller muss die Zuordnung
/// ueber den Stream-Kontext machen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentHeader {
    /// `request_id` des Fragments (GIOP 1.2+).
    pub request_id: u32,
}

impl FragmentHeader {
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
        Ok(Self {
            request_id: r.read_u32()?,
        })
    }
}

/// Fragment-Message-Body — version-uniform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// In GIOP 1.2 vorhanden, in 1.1 `None` (keine Header-Form).
    pub header: Option<FragmentHeader>,
    /// Body-Bytes.
    pub body: Vec<u8>,
}

impl Fragment {
    /// CDR-Encode in einen `BufferWriter`. In GIOP 1.2 wird
    /// `request_id` zuerst encodiert; in GIOP 1.1 nur die
    /// Body-Bytes.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, version: Version, w: &mut BufferWriter) -> GiopResult<()> {
        if version.uses_v1_2_request_layout() {
            if let Some(h) = &self.header {
                h.encode(w)?;
            }
        }
        w.write_bytes(&self.body)?;
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
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
        // GIOP 1.1 — kein header, nur body bytes.
        assert_eq!(bytes, alloc::vec![0xa, 0xb, 0xc, 0xd]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = Fragment::decode(Version::V1_1, &mut r).unwrap();
        assert_eq!(decoded, f);
    }
}
