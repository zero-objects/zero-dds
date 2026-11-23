// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Propagation of Messaging QoS — CORBA 3.3 Part 1 §17.4.
//!
//! ```text
//! module Messaging {
//!     typedef CORBA::OctetSeq PolicyData;
//!     struct PolicyValue {
//!         CORBA::PolicyType ptype;
//!         PolicyData        pvalue;
//!     };
//!     typedef sequence<PolicyValue> PolicyValueSeq;
//!     const IOP::ComponentId TAG_POLICIES        = 2;
//!     const IOP::ServiceId   INVOCATION_POLICIES = 7;
//! };
//! ```
//!
//! The body of the `TAG_POLICIES` component of an object reference
//! (§17.4.2) and of the `INVOCATION_POLICIES` service context of a request
//! or reply (§17.4.3) is a CDR encapsulation of a `PolicyValueSeq`. The
//! `pvalue` of each entry is the format the specification of that policy
//! gives; for the ZIOP policies a CDR encapsulation of the policy value
//! ([`crate::ziop`]).

use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

use crate::error::{GiopError, GiopResult};
use crate::service_context::{ServiceContext, ServiceContextList, ServiceContextTag};

/// `Messaging::TAG_POLICIES` — the `IOP::ComponentId` of the Messaging
/// QoS profile component (Part 1 §17.4.2).
pub const TAG_POLICIES: u32 = 2;

/// `Messaging::INVOCATION_POLICIES` — the `IOP::ServiceId` of the
/// Messaging QoS service context (Part 1 §17.4.3).
pub const INVOCATION_POLICIES: u32 = ServiceContextTag::InvocationPolicies.as_u32();

/// `Messaging::PolicyValue` (Part 1 §17.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyValue {
    /// `CORBA::PolicyType` of the policy.
    pub ptype: u32,
    /// The value in the format of that policy's specification.
    pub pvalue: Vec<u8>,
}

/// Writes the byte-order octet that starts a CDR encapsulation.
pub(crate) fn begin_encapsulation(endianness: Endianness) -> Writer {
    let mut w = Writer::new(endianness);
    w.write_u8(match endianness {
        Endianness::Big => 0,
        Endianness::Little => 1,
    });
    w
}

/// A reader over a CDR encapsulation, past its byte-order octet; the
/// alignment counts from that octet.
///
/// # Errors
/// `Malformed` for an empty encapsulation or a byte-order octet that is
/// neither 0 nor 1.
pub(crate) fn open_encapsulation(encap: &[u8]) -> GiopResult<Reader<'_>> {
    let endianness = match encap.first() {
        Some(0) => Endianness::Big,
        Some(1) => Endianness::Little,
        Some(other) => {
            return Err(GiopError::Malformed(alloc::format!(
                "encapsulation byte-order octet {other}"
            )));
        }
        None => return Err(GiopError::Malformed("empty encapsulation".into())),
    };
    let mut r = Reader::new(encap, endianness);
    r.read_u8()?;
    Ok(r)
}

/// Encodes a `Messaging::PolicyValueSeq` as a CDR encapsulation, the body
/// of `TAG_POLICIES` and of `INVOCATION_POLICIES`.
///
/// # Errors
/// `Malformed` for more entries or a longer `pvalue` than a CDR sequence
/// length holds.
pub fn encode_policy_values(values: &[PolicyValue], endianness: Endianness) -> GiopResult<Vec<u8>> {
    let mut w = begin_encapsulation(endianness);
    w.write_u32(seq_len(values.len())?);
    for v in values {
        w.write_u32(v.ptype);
        w.write_u32(seq_len(v.pvalue.len())?);
        w.write_bytes(&v.pvalue);
    }
    Ok(w.finish())
}

/// Decodes the `Messaging::PolicyValueSeq` encapsulation of a
/// `TAG_POLICIES` component or `INVOCATION_POLICIES` service context.
///
/// # Errors
/// `Malformed`/`Wire` for a truncated or inconsistent encapsulation (a
/// count larger than the octets that follow can hold).
pub fn decode_policy_values(encap: &[u8]) -> GiopResult<Vec<PolicyValue>> {
    let mut r = open_encapsulation(encap)?;
    let count = r.read_u32()? as usize;
    // Each entry takes at least 8 octets (ptype and the pvalue length).
    if count > r.remaining() / 8 {
        return Err(GiopError::Malformed(alloc::format!(
            "PolicyValueSeq announces {count} entries in {} octets",
            r.remaining()
        )));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let ptype = r.read_u32()?;
        let len = r.read_u32()? as usize;
        if len > r.remaining() {
            return Err(GiopError::Malformed(alloc::format!(
                "PolicyValue pvalue of {len} octets in {} octets",
                r.remaining()
            )));
        }
        let pvalue = r.read_bytes(len)?.to_vec();
        out.push(PolicyValue { ptype, pvalue });
    }
    Ok(out)
}

/// The `INVOCATION_POLICIES` service context carrying `values`
/// (Part 1 §17.4.3).
///
/// # Errors
/// As [`encode_policy_values`].
pub fn invocation_policies_context(
    values: &[PolicyValue],
    endianness: Endianness,
) -> GiopResult<ServiceContext> {
    Ok(ServiceContext::new(
        INVOCATION_POLICIES,
        encode_policy_values(values, endianness)?,
    ))
}

/// The policy values of the `INVOCATION_POLICIES` entry of `list`;
/// `Ok(None)` without such an entry.
///
/// # Errors
/// As [`decode_policy_values`] for an entry that does not decode.
pub fn invocation_policies_of(list: &ServiceContextList) -> GiopResult<Option<Vec<PolicyValue>>> {
    match list.0.iter().find(|c| c.context_id == INVOCATION_POLICIES) {
        Some(c) => decode_policy_values(&c.context_data).map(Some),
        None => Ok(None),
    }
}

fn seq_len(n: usize) -> GiopResult<u32> {
    u32::try_from(n).map_err(|_| GiopError::Malformed("sequence longer than u32".into()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn corba_p1_17_4_policy_value_seq_layout() {
        let values = [
            PolicyValue {
                ptype: 64,
                pvalue: alloc::vec![0, 1],
            },
            PolicyValue {
                ptype: 66,
                pvalue: alloc::vec![0, 0, 0, 0, 0, 0, 1, 0],
            },
        ];
        let bytes = encode_policy_values(&values, Endianness::Big).unwrap();
        let expected: &[u8] = &[
            0, 0, 0, 0, // byte order, padding to 4
            0, 0, 0, 2, // two entries
            0, 0, 0, 64, 0, 0, 0, 2, 0, 1, // ptype 64, pvalue of 2 octets
            0, 0, // padding to 4
            0, 0, 0, 66, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 1, 0,
        ];
        assert_eq!(bytes, expected);
        assert_eq!(decode_policy_values(&bytes).unwrap(), values);
    }

    #[test]
    fn corba_p1_17_4_little_endian_policy_values_round_trip() {
        let values = [PolicyValue {
            ptype: 65,
            pvalue: alloc::vec![1, 2, 3],
        }];
        let bytes = encode_policy_values(&values, Endianness::Little).unwrap();
        assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
        assert_eq!(decode_policy_values(&bytes).unwrap(), values);
    }

    #[test]
    fn corba_p1_17_4_a_count_beyond_the_octets_is_refused() {
        let bytes = [0u8, 0, 0, 0, 0x10, 0, 0, 0, 0, 0, 0, 64];
        assert!(matches!(
            decode_policy_values(&bytes),
            Err(GiopError::Malformed(_))
        ));
        let truncated = [0u8, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 9, 1];
        assert!(decode_policy_values(&truncated).is_err());
        assert!(decode_policy_values(&[7]).is_err());
        assert!(decode_policy_values(&[]).is_err());
    }

    #[test]
    fn corba_p1_17_4_invocation_policies_is_service_context_7() {
        let values = [PolicyValue {
            ptype: 64,
            pvalue: alloc::vec![0, 1],
        }];
        let ctx = invocation_policies_context(&values, Endianness::Big).unwrap();
        assert_eq!(ctx.context_id, 7);
        let list = ServiceContextList(alloc::vec![
            ServiceContext::new(1, alloc::vec![0, 0, 0, 0]),
            ctx
        ]);
        assert_eq!(
            invocation_policies_of(&list).unwrap(),
            Some(values.to_vec())
        );
        assert_eq!(
            invocation_policies_of(&ServiceContextList::default()).unwrap(),
            None
        );
    }
}
