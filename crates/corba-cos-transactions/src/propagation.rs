// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `PropagationContext` — transaction context propagation over GIOP.
//!
//! A transactional request carries, in the IOP `ServiceContext` with id
//! [`TRANSACTION_SERVICE_CONTEXT_ID`] (= 0, `TransactionService`), a
//! CDR-encapsulated `PropagationContext`. The server reads it, identifies the
//! transaction by its [`Otid`], and joins it (registering its resources with
//! the propagated coordinator).
//!
//! ```text
//! struct TransIdentity {
//!     Coordinator coord;   // here: opaque IOR bytes
//!     Terminator  term;    // here: opaque IOR bytes
//!     otid_t      otid;
//! };
//! struct PropagationContext {
//!     unsigned long              timeout;
//!     TransIdentity              current;
//!     sequence<TransIdentity>    parents;
//!     any                        implementation_specific_data;  // here: empty any (tk_null)
//! };
//! ```
//!
//! Coordinator/Terminator are object references (IORs): empty → **nil ref**
//! (CDR `IOR` = type_id `""` + 0 profiles, byte-identical to JacORB); non-empty
//! `*_ior` bytes are emitted verbatim as a marshalled ref (live coordinator
//! for distributed 2PC). `implementation_specific_data` = empty `any` (`tk_null`).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError, Endianness};

use crate::otid::Otid;

/// IOP `ServiceContext` id for `TransactionService` (OMG OTS §10.4.6) = 0.
pub const TRANSACTION_SERVICE_CONTEXT_ID: u32 = 0;

/// `tk_null` TypeCode kind (empty `any`).
const TK_NULL: u32 = 0;

/// `TransIdentity` — Coordinator/Terminator (opaque IOR bytes) + transaction `otid`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransIdentity {
    /// Coordinator object reference (opaque IOR bytes; empty = none).
    pub coord_ior: Vec<u8>,
    /// Terminator object reference (opaque IOR bytes; empty = none).
    pub term_ior: Vec<u8>,
    /// Transaction identity.
    pub otid: Otid,
}

impl TransIdentity {
    /// `TransIdentity` from just an `otid` (no object refs) — for
    /// otid-driven propagation.
    #[must_use]
    pub fn from_otid(otid: Otid) -> Self {
        Self {
            coord_ior: Vec::new(),
            term_ior: Vec::new(),
            otid,
        }
    }

    fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        // Coordinator/Terminator are object references (IOR). Empty → nil ref
        // (type_id "" + 0 profiles); otherwise the raw IOR bytes (advanced).
        write_object_ref(w, &self.coord_ior)?;
        write_object_ref(w, &self.term_ior)?;
        self.otid.encode(w)
    }

    fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let coord_ior = read_object_ref(r)?;
        let term_ior = read_object_ref(r)?;
        let otid = Otid::decode(r)?;
        Ok(Self {
            coord_ior,
            term_ior,
            otid,
        })
    }
}

/// Writes an object reference: empty bytes → **nil ref** (CDR `IOR` =
/// type_id `""` + 0 profiles, like JacORB); otherwise the raw IOR bytes verbatim.
fn write_object_ref(w: &mut BufferWriter, ior_bytes: &[u8]) -> Result<(), EncodeError> {
    if ior_bytes.is_empty() {
        w.write_string("")?; // type_id "" (CDR length 1 + NUL)
        w.write_u32(0)?; // 0 TaggedProfiles
    } else {
        w.write_bytes(ior_bytes)?;
    }
    Ok(())
}

/// Reads an object reference (IOR: type_id string + TaggedProfile sequence).
/// A nil ref (`""` + 0 profiles) → empty bytes; otherwise the fields are
/// canonically re-serialized.
fn read_object_ref(r: &mut BufferReader<'_>) -> Result<Vec<u8>, DecodeError> {
    let type_id = r.read_string()?;
    let nprof = r.read_u32()?;
    if type_id.is_empty() && nprof == 0 {
        return Ok(Vec::new()); // nil ref
    }
    let mut tmp = BufferWriter::new(r.endianness());
    tmp.write_string(&type_id)
        .map_err(|_| DecodeError::InvalidEnum {
            kind: "ior re-encode",
            value: 0,
        })?;
    let _ = tmp.write_u32(nprof);
    for _ in 0..nprof {
        let tag = r.read_u32()?;
        let plen = r.read_u32()? as usize;
        let pdata = r.read_bytes(plen)?.to_vec();
        let _ = tmp.write_u32(tag);
        let _ = tmp.write_u32(plen as u32);
        let _ = tmp.write_bytes(&pdata);
    }
    Ok(tmp.into_bytes())
}

/// `PropagationContext` (OMG OTS §10.4.6) — the transaction context propagated over GIOP.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropagationContext {
    /// Remaining transaction timeout in seconds.
    pub timeout: u32,
    /// Identity of the current transaction.
    pub current: TransIdentity,
    /// Parent transactions (nested transactions, outermost last).
    pub parents: Vec<TransIdentity>,
}

impl PropagationContext {
    /// Context of a flat (non-nested) transaction with `otid`.
    #[must_use]
    pub fn flat(timeout: u32, otid: Otid) -> Self {
        Self {
            timeout,
            current: TransIdentity::from_otid(otid),
            parents: Vec::new(),
        }
    }

    /// CDR encode of the body (without the encapsulation octet).
    ///
    /// # Errors
    /// Buffer write error / length overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.timeout)?;
        self.current.encode(w)?;
        let n = u32::try_from(self.parents.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "propagation parents length overflow",
        })?;
        w.write_u32(n)?;
        for p in &self.parents {
            p.encode(w)?;
        }
        // implementation_specific_data: empty `any` = tk_null TypeCode (1 word),
        // byte-identical to JacORB PropagationContextHelper.
        w.write_u32(TK_NULL)
    }

    /// CDR decode of the body.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let timeout = r.read_u32()?;
        let current = TransIdentity::decode(r)?;
        let count = r.read_u32()? as usize;
        let mut parents = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            parents.push(TransIdentity::decode(r)?);
        }
        let _impl_tc = r.read_u32()?; // tk_null TypeCode (empty any)
        Ok(Self {
            timeout,
            current,
            parents,
        })
    }

    /// Builds the `TransactionService` ServiceContext data: a CDR encapsulation
    /// (byte-order octet + body), as carried in `IOP::ServiceContext.context_data`
    /// (id = 0).
    ///
    /// # Errors
    /// Buffer write error.
    pub fn to_service_context_data(&self, endianness: Endianness) -> Result<Vec<u8>, EncodeError> {
        let mut w = BufferWriter::new(endianness);
        w.write_u8(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        })?;
        self.encode(&mut w)?;
        Ok(w.into_bytes())
    }

    /// Parses a `PropagationContext` from `ServiceContext.context_data`
    /// (encapsulation: byte-order octet + body).
    ///
    /// Accepts **both** wire forms seen across ORBs:
    /// 1. the OTS-spec form (§10.4.6) — a bare CDR encapsulation of the
    ///    `PropagationContext` struct; and
    /// 2. JacORB's actual form — its `ClientContextTransferInterceptor` puts
    ///    `Codec.encode(any)` into the context, i.e. a byte-order octet +
    ///    **`TypeCode`** + value. ZeroDDS sends the spec form but is liberal in
    ///    what it accepts so a live JacORB OTS transaction interoperates.
    ///
    /// # Errors
    /// Buffer read error / empty data.
    pub fn from_service_context_data(data: &[u8]) -> Result<Self, DecodeError> {
        let bo = *data.first().ok_or(DecodeError::UnexpectedEof {
            needed: 1,
            offset: 0,
        })?;
        let e = if bo == 0 {
            Endianness::Big
        } else {
            Endianness::Little
        };
        // Discriminate by the first aligned word of the encapsulation body. In
        // the JacORB `any` form it is the `TypeCode` kind `tk_struct` (15); in
        // the spec form it is `timeout`. When it is 15, try the any form first
        // (skip the TypeCode); fall back to the bare struct (covers the rare
        // spec context whose timeout happens to be 15).
        let mut probe = BufferReader::new(data, e);
        probe.read_u8()?;
        let looks_any = probe.read_u32().ok() == Some(zerodds_cdr::type_code::tckind::TK_STRUCT);

        if looks_any {
            let mut r = BufferReader::new(data, e);
            r.read_u8()?;
            if skip_type_code(&mut r).is_ok() {
                if let Ok(ctx) = Self::decode(&mut r) {
                    return Ok(ctx);
                }
            }
        }
        // Spec form (§10.4.6): bare CDR encapsulation of the struct.
        let mut r = BufferReader::new(data, e);
        r.read_u8()?; // byte-order octet
        Self::decode(&mut r)
    }
}

/// Skips a CDR `TypeCode` (§15.3.5.1) without parsing its parameters — enough to
/// advance past the leading `TypeCode` of a JacORB `any`-wrapped service context
/// so the value can be decoded. Complex kinds carry a length-prefixed parameter
/// encapsulation (skipped wholesale, so arbitrary nested complexity is handled);
/// `tk_string`/`tk_wstring` carry a single bound; `tk_fixed` two `ushort`s;
/// indirection a single offset; everything else is parameterless.
fn skip_type_code(r: &mut BufferReader<'_>) -> Result<(), DecodeError> {
    let kind = r.read_u32()?;
    match kind {
        // Complex kinds: tk_objref, tk_struct, tk_union, tk_enum, tk_sequence,
        // tk_array, tk_alias, tk_except, tk_value, tk_value_box, tk_native,
        // tk_abstract_interface, tk_local_interface — a CDR encapsulation param.
        14 | 15 | 16 | 17 | 19 | 20 | 21 | 22 | 29 | 30 | 31 | 32 | 33 => {
            let len = r.read_u32()? as usize;
            let _ = r.read_bytes(len)?;
        }
        18 | 27 => {
            // tk_string / tk_wstring: a single `unsigned long` bound.
            let _ = r.read_u32()?;
        }
        28 => {
            // tk_fixed: digits + scale (two `unsigned short`).
            let _ = r.read_u16()?;
            let _ = r.read_u16()?;
        }
        0xffff_ffff => {
            // Indirection: a `long` offset to the target TypeCode.
            let _ = r.read_u32()?;
        }
        _ => {} // simple kinds carry no parameters
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn service_context_id_is_zero() {
        assert_eq!(TRANSACTION_SERVICE_CONTEXT_ID, 0);
    }

    #[test]
    fn flat_context_service_context_roundtrip() {
        let pc = PropagationContext::flat(30, Otid::new(7, alloc::vec![1, 2, 3, 4]));
        for e in [Endianness::Big, Endianness::Little] {
            let data = pc.to_service_context_data(e).unwrap();
            // The byte-order octet matches the chosen endianness.
            assert_eq!(data[0], u8::from(e == Endianness::Little));
            let decoded = PropagationContext::from_service_context_data(&data).unwrap();
            assert_eq!(decoded, pc);
        }
    }

    #[test]
    fn propagation_context_byte_identical_to_jacorb() {
        // Cross-ORB conformance: byte-identical to JacORB 3.9
        // org.omg.CosTransactions.PropagationContextHelper.write (capture from the Linux test host):
        // timeout=30, current={nil coord, nil term, otid(0,4,[0,0,0,1])}, parents=[],
        // implementation_specific_data = empty any (tk_null) — big-endian.
        let pc = PropagationContext::flat(30, Otid::new(0, alloc::vec![0, 0, 0, 1]));
        let mut w = BufferWriter::new(Endianness::Big);
        pc.encode(&mut w).unwrap();
        let hex: alloc::string::String = w
            .into_bytes()
            .iter()
            .map(|b| alloc::format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "0000001e000000010000000000000000000000010000000000000000000000000000000400000004000000010000000000000000"
        );
        assert_eq!(hex.len(), 104, "JacORB PropagationContext = 52 Byte");
    }

    #[test]
    fn nested_context_with_parents() {
        let pc = PropagationContext {
            timeout: 60,
            current: TransIdentity::from_otid(Otid::new(7, alloc::vec![9, 9])),
            parents: alloc::vec![
                TransIdentity::from_otid(Otid::new(7, alloc::vec![1])),
                TransIdentity::from_otid(Otid::new(7, alloc::vec![2, 2])),
            ],
        };
        let data = pc.to_service_context_data(Endianness::Big).unwrap();
        assert_eq!(
            PropagationContext::from_service_context_data(&data).unwrap(),
            pc
        );
    }

    #[test]
    fn trans_identity_nil_refs_roundtrip() {
        // nil Coordinator/Terminator (object-reference form) + otid.
        let ti = TransIdentity::from_otid(Otid::new(7, alloc::vec![5]));
        for e in [Endianness::Big, Endianness::Little] {
            let mut w = BufferWriter::new(e);
            ti.encode(&mut w).unwrap();
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, e);
            let decoded = TransIdentity::decode(&mut r).unwrap();
            assert!(decoded.coord_ior.is_empty() && decoded.term_ior.is_empty());
            assert_eq!(decoded.otid, ti.otid);
        }
    }
}
