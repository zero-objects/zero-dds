// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the writers of one publisher with `PRESENTATION.access_scope =
//! GROUP` share (DDSI-RTPS 2.5 §8.7.5, §8.7.6): the group sequence number,
//! the set of writers, and the open group coherent set.
//!
//! §8.7.5: "The GROUP sequence number is a strictly monotonically
//! increasing sequence number originating from the Publisher. Each time
//! that a DataWriter attached to a Publisher makes a CacheChange (i.e.,
//! increments its own Writer sequence number), the GROUP sequence number
//! is incremented." Each writer of the publisher holds an
//! `Arc<WriterGroup>` and draws its numbers from it.

use portable_atomic::{AtomicI64, AtomicU32, Ordering};

use crate::group_digest::GroupDigest;
use crate::wire_types::EntityId;

/// No group coherent set is open.
const NO_SET: i64 = 0;

/// The state the writers of one GROUP-scope publisher share.
#[derive(Debug, Default)]
pub struct WriterGroup {
    /// The last group sequence number handed out; 0 before the first.
    last_gsn: AtomicI64,
    /// The [`GroupDigest`] of the publisher's writers, its four octets
    /// big-endian.
    writer_set: AtomicU32,
    /// The group sequence number of the first sample of the open group
    /// coherent set, [`NO_SET`] when none is open.
    coherent_first: AtomicI64,
}

/// The group information of one change (DDSI-RTPS 2.5 §8.7.5, §8.7.6):
/// what its DATA carries as `PID_GROUP_SEQ_NUM`, `PID_WRITER_GROUP_INFO`
/// and, in a group coherent set, `PID_GROUP_COHERENT_SET`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeGroupInfo {
    /// The change's group sequence number.
    pub gsn: i64,
    /// The publisher's writers when the change was made.
    pub writer_set: GroupDigest,
    /// The group sequence number of the first sample of the group
    /// coherent set the change belongs to.
    pub coherent_set_gsn: Option<i64>,
}

/// The end of a group coherent set: what each writer's end-of-coherent-set
/// DATA carries (§8.7.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupSetEnd {
    /// `PID_GROUP_COHERENT_SET`: the set's first group sequence number.
    pub first_gsn: i64,
    /// `PID_GROUP_SEQ_NUM` of every writer's ECS: "one greater than the
    /// group sequence number of the final sample in the group coherent
    /// set".
    pub ecs_gsn: i64,
    /// `PID_WRITER_GROUP_INFO`.
    pub writer_set: GroupDigest,
}

impl WriterGroup {
    /// A group with no writers and no number handed out.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the publisher's writers; the digest goes on every change
    /// made from here on.
    pub fn set_writers(&self, writers: &[EntityId]) {
        let digest = GroupDigest::of_entities(writers);
        self.writer_set
            .store(u32::from_be_bytes(digest.to_bytes()), Ordering::SeqCst);
    }

    /// The publisher's writers as last recorded.
    #[must_use]
    pub fn writer_set(&self) -> GroupDigest {
        GroupDigest::from_bytes(self.writer_set.load(Ordering::SeqCst).to_be_bytes())
    }

    /// "the last (highest) group sequence number written by any
    /// DataWriter in the Writer's Group" (§8.3.8.6, `currentGSN`); 0
    /// before the first change.
    #[must_use]
    pub fn current_gsn(&self) -> i64 {
        self.last_gsn.load(Ordering::SeqCst)
    }

    /// The group information of the next change of one of the writers:
    /// a new group sequence number, the writer set, and the open group
    /// coherent set if there is one.
    #[must_use]
    pub fn stamp_change(&self) -> ChangeGroupInfo {
        let gsn = self.last_gsn.fetch_add(1, Ordering::SeqCst) + 1;
        let first = self.coherent_first.load(Ordering::SeqCst);
        ChangeGroupInfo {
            gsn,
            writer_set: self.writer_set(),
            coherent_set_gsn: (first != NO_SET && gsn >= first).then_some(first),
        }
    }

    /// `begin_coherent_changes` on the publisher: the set's first sample
    /// is the next change any of its writers makes. A second call while a
    /// set is open changes nothing.
    pub fn begin_coherent_set(&self) {
        let next = self.last_gsn.load(Ordering::SeqCst) + 1;
        let _ =
            self.coherent_first
                .compare_exchange(NO_SET, next, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// `true` while a group coherent set is open.
    #[must_use]
    pub fn coherent_set_is_open(&self) -> bool {
        self.coherent_first.load(Ordering::SeqCst) != NO_SET
    }

    /// `end_coherent_changes` on the publisher: closes the set and hands
    /// out the group sequence number every writer's ECS carries. `None`
    /// when no set was open or the set holds no change — it has no first
    /// sample to name, and nothing is sent.
    #[must_use]
    pub fn end_coherent_set(&self) -> Option<GroupSetEnd> {
        let first = self.coherent_first.swap(NO_SET, Ordering::SeqCst);
        if first == NO_SET || self.last_gsn.load(Ordering::SeqCst) < first {
            return None;
        }
        let ecs_gsn = self.last_gsn.fetch_add(1, Ordering::SeqCst) + 1;
        Some(GroupSetEnd {
            first_gsn: first,
            ecs_gsn,
            writer_set: self.writer_set(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn w(key: u8) -> EntityId {
        EntityId::user_writer_with_key([0, 0, key])
    }

    /// DDSI-RTPS 2.5 Table 9.23: two writers, a group coherent set of
    /// GSN 11, 12, 13, both writers' ECS at GSN 14, the set named 11.
    #[test]
    fn the_numbers_of_table_9_23() {
        let g = WriterGroup::new();
        g.set_writers(&[w(1), w(2)]);
        for _ in 0..10 {
            let _ = g.stamp_change();
        }
        g.begin_coherent_set();
        let members: [ChangeGroupInfo; 3] = [g.stamp_change(), g.stamp_change(), g.stamp_change()];
        assert_eq!(members.map(|m| m.gsn), [11, 12, 13]);
        assert!(members.iter().all(|m| m.coherent_set_gsn == Some(11)));
        let end = g.end_coherent_set().unwrap();
        assert_eq!((end.first_gsn, end.ecs_gsn), (11, 14));
        assert_eq!(end.writer_set, GroupDigest::of_entities(&[w(1), w(2)]));
        assert_eq!(g.stamp_change().coherent_set_gsn, None);
        assert_eq!(g.current_gsn(), 15);
    }

    #[test]
    fn group_sequence_numbers_rise_by_one_per_change_from_one() {
        let g = WriterGroup::new();
        assert_eq!(g.current_gsn(), 0);
        assert_eq!(g.stamp_change().gsn, 1);
        assert_eq!(g.stamp_change().gsn, 2);
        assert_eq!(g.current_gsn(), 2);
    }

    #[test]
    fn an_empty_set_has_no_end_and_takes_no_number() {
        let g = WriterGroup::new();
        let _ = g.stamp_change();
        g.begin_coherent_set();
        assert!(g.coherent_set_is_open());
        assert_eq!(g.end_coherent_set(), None);
        assert!(!g.coherent_set_is_open());
        assert_eq!(g.current_gsn(), 1);
    }

    #[test]
    fn a_second_begin_keeps_the_open_sets_first_number() {
        let g = WriterGroup::new();
        g.begin_coherent_set();
        let first = g.stamp_change();
        g.begin_coherent_set();
        let second = g.stamp_change();
        assert_eq!(first.coherent_set_gsn, Some(1));
        assert_eq!(second.coherent_set_gsn, Some(1));
    }

    #[test]
    fn the_writer_set_follows_the_writers() {
        let g = WriterGroup::new();
        assert!(g.writer_set().is_empty());
        g.set_writers(&[w(1)]);
        assert_eq!(
            g.stamp_change().writer_set,
            GroupDigest::of_entities(&[w(1)])
        );
        g.set_writers(&[w(1), w(2)]);
        assert_eq!(
            g.stamp_change().writer_set,
            GroupDigest::of_entities(&[w(1), w(2)])
        );
    }
}
