// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `otid_t` — the cross-ORB transaction identifier (OMG OTS App. A).
//!
//! ```text
//! struct otid_t {
//!     long             formatID;        // X/Open-XID-Format (-1 = NULL)
//!     long             bequeath_length; // portion of `tid` inherited by sub-transactions
//!     sequence<octet>  tid;             // global transaction id + branch qualifier
//! };
//! ```
//!
//! The CDR wire form is the canonical, ORB-neutral identity of a transaction —
//! it travels in the [`PropagationContext`](crate::propagation::PropagationContext)
//! with every transactional request.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// `otid_t` (OMG OTS App. A) — globally unique transaction identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Otid {
    /// X/Open-XID `formatID`. `-1` marks a NULL transaction (no context).
    pub format_id: i32,
    /// Length of the `tid` prefix inherited by sub-transactions.
    pub bequeath_length: i32,
    /// Global transaction ID + branch qualifier (X/Open-XID `data`).
    pub tid: Vec<u8>,
}

impl Otid {
    /// Constructs an `otid_t` from `format_id` and transaction-ID bytes
    /// (`bequeath_length` = full `tid` length — typical root transaction).
    #[must_use]
    pub fn new(format_id: i32, tid: Vec<u8>) -> Self {
        let bequeath_length = i32::try_from(tid.len()).unwrap_or(0);
        Self {
            format_id,
            bequeath_length,
            tid,
        }
    }

    /// The NULL `otid_t` (`formatID = -1`) — "no transaction".
    #[must_use]
    pub fn null() -> Self {
        Self {
            format_id: -1,
            bequeath_length: 0,
            tid: Vec::new(),
        }
    }

    /// Whether this is the NULL transaction (`formatID == -1`).
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.format_id == -1
    }

    /// CDR encode (in `w`'s byte order).
    ///
    /// # Errors
    /// Buffer write error / length overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.format_id as u32)?;
        w.write_u32(self.bequeath_length as u32)?;
        let n = u32::try_from(self.tid.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "otid tid length overflow",
        })?;
        w.write_u32(n)?;
        w.write_bytes(&self.tid)
    }

    /// CDR decode.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let format_id = r.read_u32()? as i32;
        let bequeath_length = r.read_u32()? as i32;
        let n = r.read_u32()? as usize;
        let tid = r.read_bytes(n)?.to_vec();
        Ok(Self {
            format_id,
            bequeath_length,
            tid,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn roundtrip_be_le() {
        let o = Otid::new(7, alloc::vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01]);
        for e in [Endianness::Big, Endianness::Little] {
            let mut w = BufferWriter::new(e);
            o.encode(&mut w).unwrap();
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, e);
            assert_eq!(Otid::decode(&mut r).unwrap(), o);
        }
    }

    #[test]
    fn null_otid() {
        let o = Otid::null();
        assert!(o.is_null());
        let mut w = BufferWriter::new(Endianness::Big);
        o.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let d = Otid::decode(&mut r).unwrap();
        assert!(d.is_null());
        assert_eq!(d, o);
    }

    #[test]
    fn byte_exact_golden_be() {
        // Cross-ORB conformance: byte-identical to JacORB 3.9
        // org.omg.CosTransactions.otid_tHelper.write (capture from the Linux test host).
        // formatID=7, bequeath_length=3, tid=[0xAA,0xBB,0xCC] — big-endian.
        let o = Otid {
            format_id: 7,
            bequeath_length: 3,
            tid: alloc::vec![0xAA, 0xBB, 0xCC],
        };
        let mut w = BufferWriter::new(Endianness::Big);
        o.encode(&mut w).unwrap();
        let hex: alloc::string::String = w
            .into_bytes()
            .iter()
            .map(|b| alloc::format!("{b:02x}"))
            .collect();
        // 00000007 | 00000003 | 00000003 | aabbcc
        assert_eq!(hex, "000000070000000300000003aabbcc");
    }
}
