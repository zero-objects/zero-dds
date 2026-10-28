// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a DataReader knows about the liveliness of each writer it is
//! matched with — the state behind `LIVELINESS_CHANGED` (DDS 1.4 §2.2.4.1).
//!
//! The status counts writers, not transitions:
//!
//! - `alive_count` is "the total number of currently active DataWriters
//!   that write the Topic read by the DataReader". It rises when a matched
//!   writer asserts its liveliness for the first time or reasserts it after
//!   being not alive, and falls when an alive writer becomes not alive or
//!   is unmatched.
//! - `not_alive_count` is the number of writers that are no longer
//!   asserting their liveliness. It rises when an alive writer's lease
//!   runs out or its participant's does, and falls when such a writer
//!   reasserts or is unmatched.
//!
//! A reader used to hold one `alive` flag and two counters of the
//! transitions of that flag, fed by its own received samples: two writers
//! counted as one, an AUTOMATIC writer that had matched but not yet written
//! counted as none, and a writer that went silent within the reader's lease
//! was declared not alive although its participant was asserting it.
//!
//! # What asserts a writer's liveliness (§2.2.3.11)
//!
//! | writer's kind           | asserted by                                                  |
//! |-------------------------|--------------------------------------------------------------|
//! | `AUTOMATIC`             | any message of its participant (SPDP, WLP, DATA, HEARTBEAT) |
//! | `MANUAL_BY_PARTICIPANT` | a manual assertion of its participant, or data of any writer of it |
//! | `MANUAL_BY_TOPIC`       | its own data, or its own HEARTBEAT with the liveliness flag |
//!
//! Every writer is also asserted by its own data, whatever its kind.
//!
//! # Matching
//!
//! An `AUTOMATIC` writer is alive from the moment it matches: its
//! participant was heard from, or it would not have been discovered. A
//! manual writer has asserted nothing yet when it matches, and is counted
//! in neither number until it does — the spec's "asserts its liveliness for
//! the first time". Cyclone DDS counts every newly matched writer alive
//! (`ddsi_new_proxy_writer` sets `pwr->alive = 1`); for a manual writer that
//! is a claim no assertion backs, so ZeroDDS does not follow it there.
//!
//! Unmatching removes the writer from whichever number it was in, as
//! Cyclone's `LIVELINESS_CHANGED_REMOVE_ALIVE` / `_REMOVE_NOT_ALIVE` do.
//!
//! # A writer lost with its participant
//!
//! A writer whose participant's lease ran out was not "deleted normally"
//! (§2.2.4.1): it "fails to assert its liveliness and becomes not alive for
//! some reason other than the normal deletion of that DataWriter", so it
//! moves from `alive_count` to `not_alive_count`, and stays there until it
//! "reasserts its liveliness" — it matches again under the same GUID and
//! asserts — "or is deleted normally". A partitioned network is the case:
//! the writer still writes, out of reach. Cyclone DDS 11
//! (`ddsi_delete_proxy_writer` ignores `isimplicit`) and OpenDDS
//! (`DataReaderImpl::writer_removed`) remove it as a deleted writer.

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use core::time::Duration;

use zerodds_qos::LivelinessKind;
use zerodds_rtps::wire_types::Guid;

use crate::instance_handle::InstanceHandle;
use crate::status::LivelinessChangedStatus;

/// Where a matched writer stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Matched, and has not asserted its liveliness yet.
    Unasserted,
    /// Asserted within its lease.
    Alive,
    /// Its lease ran out without an assertion.
    NotAlive,
}

#[derive(Debug, Clone, Copy)]
struct Tracked {
    kind: LivelinessKind,
    /// The writer's offered lease; `None` is INFINITE.
    lease: Option<Duration>,
    last_assert: Duration,
    state: State,
}

/// The liveliness of every writer one reader is matched with.
#[derive(Debug, Default)]
pub struct LivelinessTracker {
    writers: BTreeMap<[u8; 16], Tracked>,
    alive: i32,
    not_alive: i32,
    alive_change: i32,
    not_alive_change: i32,
    last_writer: Option<[u8; 16]>,
    /// Monotonic count of changes to the two numbers. A listener compares it
    /// with the value it last saw; the numbers themselves can return to where
    /// they were between two looks (one writer lost, another gained).
    changes: u64,
    /// Writers whose lease ran out since [`Self::take_lost`] was last
    /// called; the DataReader takes them out of its instances.
    lost: alloc::vec::Vec<[u8; 16]>,
    /// Writers lost with their participant ([`Self::lost_with_participant`])
    /// and not matched since: counted in `not_alive`.
    departed: BTreeSet<[u8; 16]>,
}

impl LivelinessTracker {
    /// An empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The writer `writer` matched, with the LIVELINESS it offers.
    ///
    /// Returns `true` if a count changed. A writer matched again (an SEDP
    /// update of one already matched) keeps its state.
    pub fn matched(
        &mut self,
        writer: [u8; 16],
        kind: LivelinessKind,
        lease: Option<Duration>,
        now: Duration,
    ) -> bool {
        if let Some(t) = self.writers.get_mut(&writer) {
            t.kind = kind;
            t.lease = lease;
            return false;
        }
        // A writer lost with its participant comes back not alive; its
        // first assertion is the reassertion that moves it back.
        let (state, last_assert) = if self.departed.remove(&writer) {
            (State::NotAlive, Duration::ZERO)
        } else {
            (State::Unasserted, now)
        };
        self.writers.insert(
            writer,
            Tracked {
                kind,
                lease,
                last_assert,
                state,
            },
        );
        if kind == LivelinessKind::Automatic {
            self.renew_participant(prefix_of(&writer), false, Some(writer), now)
        } else {
            false
        }
    }

    /// The writer `writer` is no longer matched. Returns `true` if a count
    /// changed.
    pub fn unmatched(&mut self, writer: [u8; 16]) -> bool {
        let Some(t) = self.writers.remove(&writer) else {
            return false;
        };
        match t.state {
            State::Unasserted => false,
            State::Alive => {
                self.alive -= 1;
                self.alive_change -= 1;
                self.changed(writer);
                true
            }
            State::NotAlive => {
                self.not_alive -= 1;
                self.not_alive_change -= 1;
                self.changed(writer);
                true
            }
        }
    }

    /// The writer `writer` is no longer matched because its participant's
    /// lease ran out: not a normal deletion (§2.2.4.1). An alive writer
    /// moves to `not_alive_count`; a not alive one stays there. Either is
    /// counted there until it matches again and reasserts. Returns `true`
    /// if a count changed.
    pub fn lost_with_participant(&mut self, writer: [u8; 16]) -> bool {
        let Some(t) = self.writers.remove(&writer) else {
            return false;
        };
        match t.state {
            State::Unasserted => false,
            State::Alive => {
                self.alive -= 1;
                self.alive_change -= 1;
                self.not_alive += 1;
                self.not_alive_change += 1;
                self.departed.insert(writer);
                self.changed(writer);
                true
            }
            State::NotAlive => {
                self.departed.insert(writer);
                false
            }
        }
    }

    /// The writer `writer` asserted its own liveliness: a DATA of it, a
    /// HEARTBEAT of it with the liveliness flag, or — for a writer in the
    /// reader's own participant — its `write` or `assert_liveliness`.
    ///
    /// That is also an assertion by its participant (§2.2.3.11
    /// MANUAL_BY_PARTICIPANT: "as long as at least one Entity within the
    /// DomainParticipant has asserted its liveliness the other Entities in
    /// that same DomainParticipant are also alive").
    ///
    /// Returns `true` if a count changed.
    pub fn writer_asserted(&mut self, writer: [u8; 16], now: Duration) -> bool {
        self.renew_participant(prefix_of(&writer), true, Some(writer), now)
    }

    /// A message of the participant `prefix` arrived. With `manual`, it is
    /// a manual assertion of the participant (a WLP
    /// `MANUAL_LIVELINESS_UPDATE`); without, it only shows the participant
    /// is running (SPDP, a WLP `AUTOMATIC_LIVELINESS_UPDATE`, a HEARTBEAT).
    ///
    /// Asserts its `AUTOMATIC` writers, and with `manual` its
    /// `MANUAL_BY_PARTICIPANT` writers. Returns `true` if a count changed.
    pub fn participant_asserted(&mut self, prefix: [u8; 12], manual: bool, now: Duration) -> bool {
        self.renew_participant(prefix, manual, None, now)
    }

    /// Renews the writers of `prefix` the assertion covers: the `AUTOMATIC`
    /// ones, with `manual` the `MANUAL_BY_PARTICIPANT` ones, and `writer`
    /// itself whatever its kind. Walks the writers of `prefix` only, in
    /// place: this runs for every DATA a reader receives.
    fn renew_participant(
        &mut self,
        prefix: [u8; 12],
        manual: bool,
        writer: Option<[u8; 16]>,
        now: Duration,
    ) -> bool {
        let mut low = [0u8; 16];
        low[..12].copy_from_slice(&prefix);
        let mut high = [0xffu8; 16];
        high[..12].copy_from_slice(&prefix);
        let (mut alive, mut not_alive, mut transitions) = (0i32, 0i32, 0u64);
        let mut last = None;
        for (guid, t) in self.writers.range_mut(low..=high) {
            let covered = Some(*guid) == writer
                || match t.kind {
                    LivelinessKind::Automatic => true,
                    LivelinessKind::ManualByParticipant => manual,
                    LivelinessKind::ManualByTopic => false,
                };
            // An assertion older than the one on record changes nothing;
            // one as old as the assertion whose lease already ran out is
            // that same assertion, and does not revive the writer.
            if !covered
                || now < t.last_assert
                || (now == t.last_assert && t.state == State::NotAlive)
            {
                continue;
            }
            t.last_assert = now;
            match t.state {
                State::Alive => continue,
                State::Unasserted => alive += 1,
                State::NotAlive => {
                    alive += 1;
                    not_alive -= 1;
                }
            }
            t.state = State::Alive;
            transitions += 1;
            last = Some(*guid);
        }
        if transitions == 0 {
            return false;
        }
        self.alive += alive;
        self.alive_change += alive;
        self.not_alive += not_alive;
        self.not_alive_change += not_alive;
        self.last_writer = last;
        self.changes = self.changes.wrapping_add(transitions);
        true
    }

    /// Declares every alive writer whose lease has run out at `now` not
    /// alive. Returns whether a count changed, and when the next lease runs
    /// out if nothing asserts before then.
    pub fn expire(&mut self, now: Duration) -> (bool, Option<Duration>) {
        let mut next: Option<Duration> = None;
        let mut expired: alloc::vec::Vec<[u8; 16]> = alloc::vec::Vec::new();
        for (guid, t) in &self.writers {
            let (State::Alive, Some(lease)) = (t.state, t.lease) else {
                continue;
            };
            let due = t.last_assert.saturating_add(lease);
            if now >= due {
                expired.push(*guid);
            } else {
                next = Some(next.map_or(due, |n| n.min(due)));
            }
        }
        let changed = !expired.is_empty();
        for guid in expired {
            if let Some(t) = self.writers.get_mut(&guid) {
                t.state = State::NotAlive;
            }
            self.alive -= 1;
            self.alive_change -= 1;
            self.not_alive += 1;
            self.not_alive_change += 1;
            self.changed(guid);
            self.lost.push(guid);
        }
        (changed, next)
    }

    /// The writers that lost their liveliness in [`Self::expire`] since the
    /// last call. DDS 1.4 §2.2.2.5.1.3: an instance none of whose writers is
    /// alive is NOT_ALIVE_NO_WRITERS; §2.2.3.9.2 (c): a change in liveliness
    /// changes the ownership of an EXCLUSIVE instance.
    pub fn take_lost(&mut self) -> alloc::vec::Vec<[u8; 16]> {
        core::mem::take(&mut self.lost)
    }

    /// The status as it stands, change fields included, without resetting
    /// them.
    #[must_use]
    pub fn status(&self) -> LivelinessChangedStatus {
        LivelinessChangedStatus {
            alive_count: self.alive,
            not_alive_count: self.not_alive,
            alive_count_change: self.alive_change,
            not_alive_count_change: self.not_alive_change,
            last_publication_handle: self.last_writer.map_or(InstanceHandle::NIL, |g| {
                InstanceHandle::from_guid(Guid::from_bytes(g))
            }),
        }
    }

    /// The status, resetting its change fields — what reading the status
    /// does (§2.2.4.1: the changes are "since the last time the listener
    /// was called or the status was read").
    pub fn take_status(&mut self) -> LivelinessChangedStatus {
        let status = self.status();
        self.acknowledge(&status);
        status
    }

    /// `reported` was handed to a listener: its changes are no longer
    /// pending. What changed after it was taken stays.
    pub fn acknowledge(&mut self, reported: &LivelinessChangedStatus) {
        self.alive_change -= reported.alive_count_change;
        self.not_alive_change -= reported.not_alive_count_change;
    }

    /// The matched writers of the participant `prefix`.
    #[must_use]
    pub fn matched_writers_of(&self, prefix: [u8; 12]) -> alloc::vec::Vec<[u8; 16]> {
        self.writers
            .keys()
            .filter(|guid| prefix_of(guid) == prefix)
            .copied()
            .collect()
    }

    /// How many times a count has changed since the reader was created.
    #[must_use]
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// `true` if `writer` is matched and its lease ran out without an
    /// assertion.
    #[must_use]
    pub fn has_lost_liveliness(&self, writer: [u8; 16]) -> bool {
        self.writers
            .get(&writer)
            .is_some_and(|t| t.state == State::NotAlive)
    }

    /// `true` if at least one matched writer is alive.
    #[must_use]
    pub fn any_alive(&self) -> bool {
        self.alive > 0
    }

    fn changed(&mut self, writer: [u8; 16]) {
        self.last_writer = Some(writer);
        self.changes = self.changes.wrapping_add(1);
    }
}

fn prefix_of(guid: &[u8; 16]) -> [u8; 12] {
    let mut prefix = [0u8; 12];
    prefix.copy_from_slice(&guid[..12]);
    prefix
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn guid(participant: u8, writer: u8) -> [u8; 16] {
        let mut g = [0u8; 16];
        g[0] = participant;
        g[15] = writer;
        g
    }

    fn counts(t: &LivelinessTracker) -> (i32, i32) {
        let s = t.status();
        (s.alive_count, s.not_alive_count)
    }

    #[test]
    fn two_automatic_writers_are_two_alive_writers() {
        let mut t = LivelinessTracker::new();
        t.matched(guid(1, 1), LivelinessKind::Automatic, None, Duration::ZERO);
        t.matched(guid(2, 1), LivelinessKind::Automatic, None, Duration::ZERO);
        assert_eq!(counts(&t), (2, 0));
        let s = t.take_status();
        assert_eq!((s.alive_count_change, s.not_alive_count_change), (2, 0));
        let s = t.take_status();
        assert_eq!((s.alive_count_change, s.not_alive_count_change), (0, 0));
    }

    #[test]
    fn an_automatic_writer_is_alive_from_its_match_without_writing() {
        let mut t = LivelinessTracker::new();
        assert!(t.matched(
            guid(1, 1),
            LivelinessKind::Automatic,
            Some(200 * MS),
            Duration::ZERO
        ));
        assert_eq!(counts(&t), (1, 0));
    }

    #[test]
    fn a_manual_writer_counts_from_its_first_assertion() {
        let mut t = LivelinessTracker::new();
        let w = guid(1, 1);
        assert!(!t.matched(
            w,
            LivelinessKind::ManualByTopic,
            Some(100 * MS),
            Duration::ZERO
        ));
        assert_eq!(counts(&t), (0, 0));
        // A lease that was never started does not run out.
        assert_eq!(t.expire(Duration::from_secs(10)), (false, None));
        assert!(t.writer_asserted(w, Duration::from_secs(10)));
        assert_eq!(counts(&t), (1, 0));
    }

    #[test]
    fn a_silent_writer_moves_from_alive_to_not_alive_and_back() {
        let mut t = LivelinessTracker::new();
        let w = guid(1, 1);
        t.matched(
            w,
            LivelinessKind::ManualByTopic,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.writer_asserted(w, 10 * MS);
        assert_eq!(t.expire(50 * MS), (false, Some(110 * MS)));
        assert!(t.expire(110 * MS).0);
        assert_eq!(counts(&t), (0, 1));
        let s = t.take_status();
        // Unasserted → alive (+1), alive → not alive (-1, +1).
        assert_eq!((s.alive_count_change, s.not_alive_count_change), (0, 1));
        assert_eq!(
            s.last_publication_handle,
            InstanceHandle::from_guid(Guid::from_bytes(w))
        );
        assert!(t.writer_asserted(w, 200 * MS));
        assert_eq!(counts(&t), (1, 0));
        let s = t.take_status();
        assert_eq!((s.alive_count_change, s.not_alive_count_change), (1, -1));
    }

    #[test]
    fn unmatching_takes_a_writer_out_of_the_number_it_was_in() {
        let mut t = LivelinessTracker::new();
        let (a, b, c) = (guid(1, 1), guid(1, 2), guid(1, 3));
        t.matched(a, LivelinessKind::Automatic, Some(100 * MS), Duration::ZERO);
        t.matched(
            b,
            LivelinessKind::ManualByTopic,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.matched(
            c,
            LivelinessKind::ManualByTopic,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.writer_asserted(b, Duration::ZERO);
        // `a` is kept alive by its participant; `b` falls silent.
        t.participant_asserted(prefix_of(&a), false, 150 * MS);
        t.expire(150 * MS);
        assert_eq!(counts(&t), (1, 1));
        assert!(t.unmatched(b));
        assert_eq!(counts(&t), (1, 0));
        assert!(t.unmatched(a));
        assert_eq!(counts(&t), (0, 0));
        // `c` never asserted and was in neither number.
        assert!(!t.unmatched(c));
    }

    /// §2.2.4.1: a writer lost with its participant's lease "becomes not
    /// alive for some reason other than the normal deletion": it moves
    /// from alive_count to not_alive_count and stays there until it
    /// matches again and reasserts. A writer unmatched by its deletion
    /// leaves both.
    #[test]
    fn a_writer_lost_with_its_participant_is_not_alive_until_it_reasserts() {
        let mut t = LivelinessTracker::new();
        let lost = guid(1, 1);
        let deleted = guid(2, 1);
        t.matched(lost, LivelinessKind::Automatic, None, MS);
        t.matched(deleted, LivelinessKind::Automatic, None, MS);
        let _ = t.take_status();
        assert!(t.lost_with_participant(lost));
        assert!(t.unmatched(deleted));
        let s = t.take_status();
        assert_eq!((s.alive_count, s.not_alive_count), (0, 1));
        assert_eq!((s.alive_count_change, s.not_alive_count_change), (-2, 1));
        // Matched again under the same GUID: its participant was heard
        // from, which asserts an AUTOMATIC writer.
        assert!(t.matched(lost, LivelinessKind::Automatic, None, 10 * MS));
        let s = t.take_status();
        assert_eq!((s.alive_count, s.not_alive_count), (1, 0));
        assert_eq!((s.alive_count_change, s.not_alive_count_change), (1, -1));
    }

    /// A manual writer lost with its participant comes back not alive and
    /// counts as alive from its first assertion; one lost after its lease
    /// ran out was already not alive and changes nothing; one that never
    /// asserted was in neither number.
    #[test]
    fn a_lost_manual_writer_comes_back_not_alive() {
        let mut t = LivelinessTracker::new();
        let w = guid(1, 1);
        t.matched(w, LivelinessKind::ManualByTopic, Some(100 * MS), MS);
        assert!(t.writer_asserted(w, 2 * MS));
        assert!(t.lost_with_participant(w));
        assert!(!t.matched(w, LivelinessKind::ManualByTopic, Some(100 * MS), 3 * MS));
        assert_eq!(counts(&t), (0, 1));
        assert!(t.writer_asserted(w, 4 * MS));
        assert_eq!(counts(&t), (1, 0));

        let expired = guid(2, 1);
        t.matched(expired, LivelinessKind::ManualByTopic, Some(100 * MS), MS);
        t.writer_asserted(expired, 2 * MS);
        t.expire(Duration::from_secs(1));
        assert_eq!(counts(&t), (0, 2));
        assert!(!t.lost_with_participant(expired));
        assert_eq!(counts(&t), (0, 2));

        let silent = guid(3, 1);
        t.matched(silent, LivelinessKind::ManualByTopic, Some(100 * MS), MS);
        assert!(!t.lost_with_participant(silent));
        assert_eq!(counts(&t), (0, 2));
    }

    #[test]
    fn a_participant_message_asserts_only_what_its_kind_covers() {
        let mut t = LivelinessTracker::new();
        let (auto, by_pp, by_topic) = (guid(1, 1), guid(1, 2), guid(1, 3));
        let other = guid(2, 1);
        t.matched(
            auto,
            LivelinessKind::Automatic,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.matched(
            by_pp,
            LivelinessKind::ManualByParticipant,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.matched(
            by_topic,
            LivelinessKind::ManualByTopic,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.matched(
            other,
            LivelinessKind::ManualByParticipant,
            Some(100 * MS),
            Duration::ZERO,
        );
        assert_eq!(counts(&t), (1, 0));
        // SPDP or an AUTOMATIC beat of participant 1.
        t.participant_asserted(prefix_of(&auto), false, 10 * MS);
        assert_eq!(counts(&t), (1, 0));
        // A manual assertion of participant 1.
        t.participant_asserted(prefix_of(&auto), true, 20 * MS);
        assert_eq!(counts(&t), (2, 0));
        // Data of the MANUAL_BY_TOPIC writer asserts it and its participant.
        t.writer_asserted(by_topic, 30 * MS);
        assert_eq!(counts(&t), (3, 0));
    }

    #[test]
    fn a_writer_matched_again_keeps_its_state() {
        let mut t = LivelinessTracker::new();
        let w = guid(1, 1);
        t.matched(w, LivelinessKind::Automatic, Some(100 * MS), Duration::ZERO);
        t.expire(100 * MS);
        assert_eq!(counts(&t), (0, 1));
        assert!(!t.matched(w, LivelinessKind::Automatic, Some(100 * MS), 120 * MS));
        assert_eq!(counts(&t), (0, 1));
    }

    #[test]
    fn an_assertion_older_than_the_last_one_changes_nothing() {
        let mut t = LivelinessTracker::new();
        let w = guid(1, 1);
        t.matched(
            w,
            LivelinessKind::ManualByParticipant,
            Some(100 * MS),
            Duration::ZERO,
        );
        t.participant_asserted(prefix_of(&w), true, 50 * MS);
        t.expire(150 * MS);
        assert_eq!(counts(&t), (0, 1));
        // The same assertion, handed in again, does not revive it.
        assert!(!t.participant_asserted(prefix_of(&w), true, 50 * MS));
        assert!(!t.participant_asserted(prefix_of(&w), true, 20 * MS));
        assert_eq!(counts(&t), (0, 1));
        assert!(t.participant_asserted(prefix_of(&w), true, 160 * MS));
        assert_eq!(counts(&t), (1, 0));
    }

    #[test]
    fn a_listener_acknowledges_what_it_was_handed_and_no_more() {
        let mut t = LivelinessTracker::new();
        t.matched(guid(1, 1), LivelinessKind::Automatic, None, Duration::ZERO);
        let handed = t.status();
        // A second writer matches between the look and the acknowledgement.
        t.matched(guid(2, 1), LivelinessKind::Automatic, None, Duration::ZERO);
        t.acknowledge(&handed);
        let s = t.status();
        assert_eq!((s.alive_count, s.alive_count_change), (2, 1));
    }

    #[test]
    fn an_infinite_lease_never_runs_out() {
        let mut t = LivelinessTracker::new();
        t.matched(guid(1, 1), LivelinessKind::Automatic, None, Duration::ZERO);
        assert_eq!(t.expire(Duration::from_secs(3600)), (false, None));
        assert_eq!(counts(&t), (1, 0));
    }

    #[test]
    fn the_change_counter_moves_when_the_numbers_come_back() {
        let mut t = LivelinessTracker::new();
        let (a, b) = (guid(1, 1), guid(2, 1));
        t.matched(a, LivelinessKind::Automatic, None, Duration::ZERO);
        let seen = t.changes();
        t.unmatched(a);
        t.matched(b, LivelinessKind::Automatic, None, Duration::ZERO);
        assert_eq!(counts(&t), (1, 0));
        assert_eq!(t.changes(), seen + 2);
    }
}
