// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `GroupDigest_t` — DDSI-RTPS 2.5 §9.3.2.5 (`typedef octet
//! GroupDigest_t[4]`): a set of entities of one participant in four
//! octets. `WriterGroupInfo_t` (Table 8.88) carries one as `writerSet`,
//! the writers a GROUP-scope publisher had announced when it wrote a
//! sample; `PID_WRITER_GROUP_INFO` (§9.6.4.5) carries that in-line.
//!
//! §9.3.2.5: "The GroupDigest_t is computed from an EntityIdSet_t by
//! first computing a 128 bit MD5 Digest (IETF RFC 1321) applied to the
//! CDR Big-Endian serialization of the structure EntityIdSet_t. The
//! GroupDigest_t is the leading 4 octets of the MD5 Digest." The
//! `entityIds` are sorted "by increasing values of the EntityId_t", each
//! "re-interpreted as if it was the little-endian serialized
//! representation of a 32-bit signed integer". "The empty group is
//! represented by a zero value of the GroupDigest_t. It is not computed
//! as the hash of the serialized empty sequence."

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::wire_types::EntityId;

/// `GroupDigest_t` (§9.3.2.5): four octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GroupDigest(pub [u8; 4]);

impl GroupDigest {
    /// Wire size: four octets.
    pub const WIRE_SIZE: usize = 4;

    /// The empty group: a zero value, not a hash (§9.3.2.5).
    pub const EMPTY: Self = Self([0; 4]);

    /// The digest of a set of entities. The order of `ids` does not
    /// matter, and an entity named twice counts once — it is a set.
    #[must_use]
    pub fn of_entities(ids: &[EntityId]) -> Self {
        let mut sorted: Vec<[u8; 4]> = ids.iter().map(|id| id.to_bytes()).collect();
        sorted.sort_by_key(|b| i32::from_le_bytes(*b));
        sorted.dedup();
        if sorted.is_empty() {
            return Self::EMPTY;
        }
        // CDR big-endian `sequence<EntityId_t>`: the length as an
        // unsigned long, then each EntityId_t's four octets
        // (`entityKey[3]`, `entityKind`), which have no byte order.
        let count = u32::try_from(sorted.len()).unwrap_or(u32::MAX);
        let mut cdr = Vec::with_capacity(4 + sorted.len() * 4);
        cdr.extend_from_slice(&count.to_be_bytes());
        for id in &sorted {
            cdr.extend_from_slice(id);
        }
        let md5 = zerodds_foundation::md5(&cdr);
        Self([md5[0], md5[1], md5[2], md5[3]])
    }

    /// `true` for the empty group.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::EMPTY
    }

    /// The four octets.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        self.0
    }

    /// From four octets.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// The digest at the front of `bytes`.
    ///
    /// # Errors
    /// `UnexpectedEof` for fewer than four octets.
    pub fn read_from(bytes: &[u8]) -> Result<Self, WireError> {
        match bytes.get(..Self::WIRE_SIZE) {
            Some(b) => Ok(Self([b[0], b[1], b[2], b[3]])),
            None => Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    const W1: [u8; 4] = [0x00, 0x00, 0x01, 0x02];
    const W2: [u8; 4] = [0x00, 0x00, 0x02, 0x02];
    const BUILTIN: [u8; 4] = [0x00, 0x00, 0x03, 0xc2];

    fn ids(list: &[[u8; 4]]) -> Vec<EntityId> {
        list.iter().map(|b| EntityId::from_bytes(*b)).collect()
    }

    // The expected octets are the leading four of MD5 over the CDR
    // big-endian EntityIdSet_t, computed outside this crate with Python's
    // hashlib from §9.3.2.5's definition:
    //   {W1, W2}: md5(00000002 00000102 00000202) = 6f621362...
    //   {W1}:     md5(00000001 00000102)          = fe84c02b...
    //   {B, W1, W2}: md5(00000003 000003c2 00000102 00000202) = 4fc50eed...

    #[test]
    fn two_writers_digest_as_section_9_3_2_5_computes_it() {
        assert_eq!(
            GroupDigest::of_entities(&ids(&[W1, W2])).0,
            [0x6f, 0x62, 0x13, 0x62]
        );
    }

    #[test]
    fn the_digest_does_not_depend_on_the_order_given() {
        assert_eq!(
            GroupDigest::of_entities(&ids(&[W2, W1])),
            GroupDigest::of_entities(&ids(&[W1, W2]))
        );
    }

    #[test]
    fn one_writer() {
        assert_eq!(
            GroupDigest::of_entities(&ids(&[W1])).0,
            [0xfe, 0x84, 0xc0, 0x2b]
        );
    }

    #[test]
    fn the_sort_reads_each_entity_id_as_a_little_endian_signed_int32() {
        // 0xc2 in the last octet makes the int32 negative: the builtin
        // entity sorts before the user writers.
        assert_eq!(
            GroupDigest::of_entities(&ids(&[W2, BUILTIN, W1])).0,
            [0x4f, 0xc5, 0x0e, 0xed]
        );
    }

    #[test]
    fn the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence() {
        assert_eq!(GroupDigest::of_entities(&[]), GroupDigest::EMPTY);
        assert!(GroupDigest::EMPTY.is_empty());
        assert_eq!(GroupDigest::EMPTY.0, [0; 4]);
    }

    #[test]
    fn an_entity_named_twice_counts_once() {
        assert_eq!(
            GroupDigest::of_entities(&ids(&[W1, W2, W1])),
            GroupDigest::of_entities(&ids(&[W1, W2]))
        );
    }

    #[test]
    fn read_from_takes_four_octets_and_rejects_fewer() {
        assert_eq!(
            GroupDigest::read_from(&[1, 2, 3, 4, 5]).unwrap(),
            GroupDigest([1, 2, 3, 4])
        );
        assert!(GroupDigest::read_from(&[1, 2, 3]).is_err());
    }
}
