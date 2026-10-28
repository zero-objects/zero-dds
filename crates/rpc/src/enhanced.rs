// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The wire of the Enhanced Service Mapping (Spec §7.5.1.2, §7.8.2).
//!
//! An Enhanced sample is the encoding of the `@Choice @Autoid` structure
//! `${I}_Request` or `${I}_Reply` (see [`crate::codegen::EnhancedService`])
//! and nothing else: there is no header. The request is identified by the
//! sample identity of its DATA, the reply by the
//! `PID_RELATED_SAMPLE_IDENTITY` of its DATA (§7.8.2).
//!
//! A `@Choice` structure is read as `@mutable` with every member
//! `@optional` (§7.5.1.2.1.1), so under XCDR2 it is a PL_CDR2 object: a
//! `DHEADER`, then an `EMHEADER` and the value of each member that is
//! present. A reply that reports a remote exception carries one member,
//! `remoteEx` (§7.5.1.2.5 rule 5), and the module encodes and recognises
//! exactly that one; the structures that carry results are the generated
//! type's own codec.

extern crate alloc;

use alloc::vec::Vec;

use crate::common_types::RemoteExceptionCode;

/// The member id `@Autoid` gives a member of the given name: the HASH of
/// the name (§7.5.1.2.1.2), 28 bits of it.
#[must_use]
pub fn member_id(name: &str) -> u32 {
    crate::rpc_hash::rpc_member_hash(name) & 0x0FFF_FFFF
}

/// `EMHEADER` length code for a member of 4 octets (XTypes 1.3 §7.4.3.4.3).
const LENGTH_CODE_4: u32 = 2;

/// The encoding of a `Reply` that carries only `remoteEx` — the reply to
/// a request the service could not or would not answer. `member` is the
/// name of the member (`remoteEx`, or `remoteEx_N` when an operation has
/// that name, §7.5.1.2.5 rule 5).
#[must_use]
pub fn encode_remote_ex_reply(code: RemoteExceptionCode, member: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&((LENGTH_CODE_4 << 28) | member_id(member)).to_le_bytes());
    out.extend_from_slice(&code.as_u32().to_le_bytes());
    out
}

/// The remote exception code of a `Reply` whose present member is
/// `member`, or `None` when the reply carries another member (a result)
/// or is not a `Reply` this module can tell.
#[must_use]
pub fn decode_remote_ex_reply(bytes: &[u8], member: &str) -> Option<RemoteExceptionCode> {
    let dheader = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?);
    let body = bytes.get(4..)?;
    if usize::try_from(dheader).ok()? != body.len() {
        return None;
    }
    let emheader = u32::from_le_bytes(body.get(0..4)?.try_into().ok()?);
    if emheader & 0x0FFF_FFFF != member_id(member) || (emheader >> 28) & 0x7 != LENGTH_CODE_4 {
        return None;
    }
    let value = u32::from_le_bytes(body.get(4..8)?.try_into().ok()?);
    RemoteExceptionCode::from_u32(value).ok()
}

pub use crate::codegen::remote_ex_member_name;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// §7.5.1.2.5: the reply that reports a remote exception is a
    /// `@Choice` with the one member `remoteEx`; XCDR2 writes a mutable
    /// structure as `DHEADER`, `EMHEADER`, value.
    #[test]
    fn a_remote_ex_reply_is_a_mutable_structure_with_one_member() {
        let bytes = encode_remote_ex_reply(RemoteExceptionCode::Unsupported, "remoteEx");
        assert_eq!(bytes.len(), 12);
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 8);
        let emheader = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(emheader >> 28, 2, "LC 2: a member of 4 octets");
        assert_eq!(emheader & 0x0FFF_FFFF, member_id("remoteEx"));
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
    }

    #[test]
    fn a_remote_ex_reply_round_trips_every_code() {
        for code in [
            RemoteExceptionCode::Ok,
            RemoteExceptionCode::Unsupported,
            RemoteExceptionCode::InvalidArgument,
            RemoteExceptionCode::OutOfResources,
            RemoteExceptionCode::UnknownOperation,
            RemoteExceptionCode::UnknownException,
        ] {
            let bytes = encode_remote_ex_reply(code, "remoteEx");
            assert_eq!(decode_remote_ex_reply(&bytes, "remoteEx"), Some(code));
        }
    }

    /// A reply whose present member is a result, not `remoteEx`, is not
    /// a remote exception.
    #[test]
    fn a_result_member_is_not_a_remote_exception() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&((LENGTH_CODE_4 << 28) | member_id("add")).to_le_bytes());
        bytes.extend_from_slice(&5u32.to_le_bytes());
        assert_eq!(decode_remote_ex_reply(&bytes, "remoteEx"), None);
    }

    #[test]
    fn a_remote_ex_member_renamed_for_a_colliding_operation_is_found_by_its_name() {
        let bytes = encode_remote_ex_reply(RemoteExceptionCode::UnknownOperation, "remoteEx_1");
        assert_eq!(decode_remote_ex_reply(&bytes, "remoteEx"), None);
        assert_eq!(
            decode_remote_ex_reply(&bytes, "remoteEx_1"),
            Some(RemoteExceptionCode::UnknownOperation)
        );
    }

    #[test]
    fn a_truncated_or_inconsistent_reply_is_not_a_remote_exception() {
        let bytes = encode_remote_ex_reply(RemoteExceptionCode::Unsupported, "remoteEx");
        assert_eq!(decode_remote_ex_reply(&bytes[..11], "remoteEx"), None);
        let mut wrong_dheader = bytes.clone();
        wrong_dheader[0] = 9;
        assert_eq!(decode_remote_ex_reply(&wrong_dheader, "remoteEx"), None);
        assert_eq!(decode_remote_ex_reply(&[], "remoteEx"), None);
    }
}
