// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CONV_FRAME::CodeSetContext` — Codeset-Negotiation-ServiceContext (§13.10.2.5).
//!
//! ```text
//! module CONV_FRAME {
//!     typedef unsigned long CodeSetId;
//!     struct CodeSetContext {
//!         CodeSetId char_data;
//!         CodeSetId wchar_data;
//!     };
//! };
//! ```
//!
//! On the first request the client attaches a `ServiceContext` with
//! `context_id = IOP::CodeSets (1)`; `context_data` is a CDR
//! encapsulation (byte-order octet + `char_data` + `wchar_data`). The
//! `CodeSetId` values are determined from the target IOR via
//! [`CodeSetComponentInfo`] negotiation (see `zerodds_corba_ior`). The defaults
//! are UTF-8 for `char` and UTF-16 for `wchar` — the latter keeps the WString BOM
//! (§15.3.1.6) interoperable with omniORB/TAO/JacORB.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

use crate::error::{GiopError, GiopResult};
use crate::service_context::{ServiceContext, ServiceContextList, ServiceContextTag};

/// Well-known `CodeSetId` values (OSF registry, spec §13.10.5.1).
pub mod well_known {
    /// `ISO 8859-1:1987` (Latin-1) — classic native `char` codeset.
    pub const ISO_8859_1: u32 = 0x0001_0001;
    /// `X/Open UTF-8` — `char` transmission codeset (ZeroDDS default, since
    /// IDL `string` is UTF-8 internally).
    pub const UTF_8: u32 = 0x0501_0001;
    /// `Unicode UTF-16` — default `wchar` transmission codeset.
    pub const UTF_16: u32 = 0x0001_0109;
    /// `Unicode UCS-2 Level 1` — `wchar` alternative.
    pub const UCS_2: u32 = 0x0001_0100;
}

/// `CONV_FRAME::CodeSetContext` — selected transmission codesets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSetContext {
    /// `char_data` — transmission codeset for `char`/`string` (TCSC).
    pub char_data: u32,
    /// `wchar_data` — transmission codeset for `wchar`/`wstring` (TCSW).
    pub wchar_data: u32,
}

impl Default for CodeSetContext {
    fn default() -> Self {
        Self::default_pair()
    }
}

impl CodeSetContext {
    /// Constructor.
    #[must_use]
    pub const fn new(char_data: u32, wchar_data: u32) -> Self {
        Self {
            char_data,
            wchar_data,
        }
    }

    /// ZeroDDS default pair: UTF-8 (`char`) + UTF-16 (`wchar`).
    #[must_use]
    pub const fn default_pair() -> Self {
        Self {
            char_data: well_known::UTF_8,
            wchar_data: well_known::UTF_16,
        }
    }

    /// Encodes the body as a CDR encapsulation (byte-order octet + two
    /// `unsigned long`). With standard CDR alignment relative to the start of
    /// the encapsulation, `char_data` sits at offset 4 and `wchar_data` at offset 8.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> GiopResult<Vec<u8>> {
        // Byte-order octet + body in ONE buffer → natural CDR alignment
        // from offset 0 (the uint32 lands automatically at offset 4).
        let mut w = BufferWriter::new(endianness);
        w.write_u8(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        })?;
        w.write_u32(self.char_data)?;
        w.write_u32(self.wchar_data)?;
        Ok(w.into_bytes())
    }

    /// Decodes a `CodeSetContext` encapsulation (byte-order octet + body).
    ///
    /// # Errors
    /// Truncated/invalid endianness or buffer read error.
    pub fn decode_encapsulation(encap: &[u8]) -> GiopResult<Self> {
        if encap.is_empty() {
            return Err(GiopError::Malformed(
                "empty CodeSetContext encapsulation".into(),
            ));
        }
        let endianness = match encap[0] {
            0 => Endianness::Big,
            1 => Endianness::Little,
            other => {
                return Err(GiopError::Malformed(alloc::format!(
                    "invalid CodeSetContext byte-order octet: {other}"
                )));
            }
        };
        // Reader over the WHOLE encapsulation (origin = byte-order octet), so
        // the uint32 alignment at offset 4 is correct; the byte-order octet
        // itself is skipped.
        let mut r = BufferReader::new(encap, endianness);
        let _bo = r.read_u8()?;
        let char_data = r.read_u32()?;
        let wchar_data = r.read_u32()?;
        Ok(Self {
            char_data,
            wchar_data,
        })
    }

    /// Builds the GIOP `ServiceContext` (context_id = `IOP::CodeSets` = 1).
    ///
    /// # Errors
    /// Encapsulation write error.
    pub fn to_service_context(&self, endianness: Endianness) -> GiopResult<ServiceContext> {
        let data = self.encode_encapsulation(endianness)?;
        Ok(ServiceContext::new(
            ServiceContextTag::CodeSets.as_u32(),
            data,
        ))
    }

    /// Looks up the `CodeSets` entry in a [`ServiceContextList`] and decodes
    /// it. Returns `Ok(None)` if no codeset context is present.
    ///
    /// # Errors
    /// Decode error if the entry exists but is corrupt.
    pub fn from_service_context_list(list: &ServiceContextList) -> GiopResult<Option<Self>> {
        let tag = ServiceContextTag::CodeSets.as_u32();
        match list.0.iter().find(|c| c.context_id == tag) {
            Some(ctx) => Ok(Some(Self::decode_encapsulation(&ctx.context_data)?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn well_known_ids_match_osf_registry() {
        // Spec §13.10.5.1 — OSF codeset registry values.
        assert_eq!(well_known::ISO_8859_1, 0x0001_0001);
        assert_eq!(well_known::UTF_8, 0x0501_0001);
        assert_eq!(well_known::UTF_16, 0x0001_0109);
        assert_eq!(well_known::UCS_2, 0x0001_0100);
    }

    #[test]
    fn encapsulation_wire_layout_be() {
        let ctx = CodeSetContext::new(well_known::UTF_8, well_known::UTF_16);
        let encap = ctx.encode_encapsulation(Endianness::Big).unwrap();
        // [bo=0][pad pad pad][char_data BE][wchar_data BE] = 12 bytes.
        assert_eq!(encap.len(), 12);
        assert_eq!(encap[0], 0); // big-endian octet
        assert_eq!(&encap[4..8], &0x0501_0001u32.to_be_bytes());
        assert_eq!(&encap[8..12], &0x0001_0109u32.to_be_bytes());
    }

    #[test]
    fn encapsulation_roundtrip_both_orders() {
        for e in [Endianness::Big, Endianness::Little] {
            let ctx = CodeSetContext::new(well_known::ISO_8859_1, well_known::UCS_2);
            let encap = ctx.encode_encapsulation(e).unwrap();
            assert_eq!(CodeSetContext::decode_encapsulation(&encap).unwrap(), ctx);
        }
    }

    #[test]
    fn service_context_roundtrip_via_list() {
        let ctx = CodeSetContext::default_pair();
        let sc = ctx.to_service_context(Endianness::Little).unwrap();
        assert_eq!(sc.context_id, 1);
        let list = ServiceContextList(alloc::vec![sc]);
        let found = CodeSetContext::from_service_context_list(&list)
            .unwrap()
            .expect("CodeSets context present");
        assert_eq!(found, ctx);
    }

    #[test]
    fn absent_context_is_none() {
        let list = ServiceContextList(alloc::vec![ServiceContext::new(42, alloc::vec![1, 2, 3])]);
        assert_eq!(
            CodeSetContext::from_service_context_list(&list).unwrap(),
            None
        );
    }

    #[test]
    fn invalid_byte_order_octet_rejected() {
        let bad = alloc::vec![0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(CodeSetContext::decode_encapsulation(&bad).is_err());
    }
}
