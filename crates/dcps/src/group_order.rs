// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The order a subscriber with `PRESENTATION.access_scope = GROUP` hands
//! samples of a GROUP-scope publisher to its DataReaders in (DDSI-RTPS 2.5
//! §8.7.5, §8.7.6).
//!
//! §8.7.5: a DataReader of such a subscriber "first orders the samples from
//! a remote Writer as it would in the cases where access scope GROUP is not
//! set. Once a sample is ready to be committed to the DDS DataReader, it will
//! not commit it. Instead, it will hand it off to a HistoryCache of the
//! Subscriber where ordering across remote DataWriters belonging to the same
//! Publisher occurs." [`GroupOrder`] is that history cache. A sample with
//! group sequence number GSN is committed once GSN − 1 is, or once it is
//! determined that no matched writer of a reliable DataReader has GSN − 1:
//!
//! - a HEARTBEAT of the group announced `currentGSN >= GSN`, and
//! - every such writer "has either advanced past the GSN-1 (by committing a
//!   Data sample with Data.inlineQos.groupSequenceNumber >= GSN) … or a Gap
//!   message with Gap.gapEndGSN.value >= GSN-1", or "announced it does not
//!   have the GSN-1 by sending a Heartbeat with Heartbeat.currentGSN.value >=
//!   GSN and GSN-1 ∉ [_Heartbeat.firstGSN.value_, _Heartbeat.lastGSN.value_]".
//!
//! "The above rules should only take into consideration DataWriters that
//! have not lost their liveliness"; "Implementations could use additional
//! timeout-based rules to limit delays." Here the caller names the writers
//! that count, and a head-of-line sample that waited longer than the
//! timeout is committed past the numbers it waited for, which is counted.
//!
//! A writer counts once per DataReader of the subscriber it is matched
//! with: each of them gets its own copy of the writer's sample with the
//! same group sequence number, over its own reliable path. A number is
//! committed once every such pair is past it, so a copy still on its way
//! to one reader is not passed on another's.
//!
//! §8.7.6: in a group coherent set each writer's part is complete when every
//! one of its changes between its first sample of the set and its
//! end-of-coherent-set DATA arrived; "The group coherent set becomes ready to
//! be committed to the DDS DataReader once an ECS sample is committed to the
//! Subscriber"; "Incomplete coherent sets shall not be added to the history
//! of the RTPS DataReaders and the corresponding CacheChanges shall be
//! discarded by the Subscriber."

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::group_digest::GroupDigest;
use zerodds_rtps::submessages::HeartbeatGroupInfo;
use zerodds_rtps::writer_group::ChangeGroupInfo;

/// A remote writer, as its GUID.
pub(crate) type WriterGuid = [u8; 16];

/// What one end-of-coherent-set DATA says about its writer's part of the
/// set: its first sample's writer sequence number, if it wrote one, and
/// its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SetEnd {
    /// The writer's first sample of the set (`PID_COHERENT_SET`).
    pub first_sn: Option<i64>,
    /// The writer sequence number of the end-of-coherent-set DATA.
    pub ecs_sn: Option<i64>,
}

/// One entry of the subscriber's group history.
#[derive(Debug, Clone)]
struct Entry<R, I> {
    reader: R,
    writer: WriterGuid,
    info: ChangeGroupInfo,
    /// `Some` for an end-of-coherent-set DATA.
    end: Option<SetEnd>,
    item: Option<I>,
    offered_at: Duration,
}

/// What one remote writer reported.
#[derive(Debug, Clone)]
struct WriterProgress<R> {
    /// The highest group sequence number of a sample it delivered, per
    /// DataReader.
    seen: BTreeMap<R, i64>,
    /// Its last HEARTBEAT's group sequence numbers.
    heartbeat: Option<HeartbeatGroupInfo>,
    /// Its highest `gapEndGSN`, per DataReader.
    gap_end: BTreeMap<R, i64>,
    /// The writer set its last sample or HEARTBEAT carried.
    writer_set: Option<GroupDigest>,
    /// The `secureWriterSet` of its last HEARTBEAT.
    secure_writer_set: Option<GroupDigest>,
}

impl<R> Default for WriterProgress<R> {
    fn default() -> Self {
        Self {
            seen: BTreeMap::new(),
            heartbeat: None,
            gap_end: BTreeMap::new(),
            writer_set: None,
            secure_writer_set: None,
        }
    }
}

impl<R: Ord> WriterProgress<R> {
    /// `reader` gets nothing with number `gsn` from this writer (any more):
    /// it got a later sample, a GAP ruled the number out, or the writer's
    /// HEARTBEAT announced the group past `gsn` without it in its range.
    fn is_past(&self, reader: &R, gsn: i64) -> bool {
        self.seen.get(reader).is_some_and(|seen| *seen > gsn)
            || self.gap_end.get(reader).is_some_and(|end| *end >= gsn)
            || self.heartbeat.is_some_and(|hb| {
                hb.current_gsn.0 > gsn && !(hb.first_gsn.0 <= gsn && gsn <= hb.last_gsn.0)
            })
    }
}

/// An open group coherent set, collected as its members are committed.
#[derive(Debug)]
struct OpenSet<R, I> {
    members: Vec<Entry<R, I>>,
    /// The end-of-coherent-set DATA each pair of writer and DataReader got.
    ends: BTreeMap<(WriterGuid, R), SetEnd>,
    /// The set's ECS group sequence number has been committed.
    end_committed: bool,
}

impl<R, I> Default for OpenSet<R, I> {
    fn default() -> Self {
        Self {
            members: Vec::new(),
            ends: BTreeMap::new(),
            end_committed: false,
        }
    }
}

/// One remote publisher's group history.
#[derive(Debug)]
struct Group<R, I> {
    /// The highest group sequence number committed or passed.
    committed: i64,
    pending: BTreeMap<i64, Vec<Entry<R, I>>>,
    writers: BTreeMap<WriterGuid, WriterProgress<R>>,
    /// The highest `currentGSN` a HEARTBEAT of the group announced, with
    /// the writer set of the writer that sent it.
    current_gsn: i64,
    sets: BTreeMap<i64, OpenSet<R, I>>,
}

impl<R, I> Default for Group<R, I> {
    fn default() -> Self {
        Self {
            committed: 0,
            pending: BTreeMap::new(),
            writers: BTreeMap::new(),
            current_gsn: 0,
            sets: BTreeMap::new(),
        }
    }
}

/// What a release found besides the samples.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReleaseCounts {
    /// Group sequence numbers committed past after the timeout.
    pub skipped_after_timeout: u64,
    /// Group coherent sets discarded as incomplete.
    pub incomplete_sets: u64,
    /// Samples of those sets.
    pub discarded_samples: u64,
    /// Samples that arrived after their number had been passed.
    pub late_samples: u64,
}

/// The writers that count for one remote group, each with a DataReader of
/// the subscriber: a reliable one it is matched with, and alive; and
/// whether the writer set a sample carried names exactly the group's
/// discovered writers.
pub(crate) trait GroupView<R> {
    /// The pairs of the group's writer and DataReader that count for the
    /// commit rule.
    fn counted(&self, group: &WriterGuid) -> BTreeSet<(WriterGuid, R)>;
    /// `writer_set` is the digest of the group's discovered writers.
    fn writer_set_is_discovered(&self, group: &WriterGuid, writer_set: GroupDigest) -> bool;

    /// Whether the `secureWriterSet` of a HEARTBEAT names the group's secure
    /// writers this subscriber discovered (DDSI-RTPS 2.5 §8.7.5). The set is
    /// reserved for DDS-Security; a subscriber that keeps no secure set
    /// matches the empty group only.
    fn secure_writer_set_is_discovered(
        &self,
        _group: &WriterGuid,
        secure_writer_set: GroupDigest,
    ) -> bool {
        secure_writer_set.is_empty()
    }
}

/// The subscriber's group history across remote publishers, keyed by the
/// remote group's GUID.
#[derive(Debug)]
pub(crate) struct GroupOrder<R, I> {
    groups: BTreeMap<WriterGuid, Group<R, I>>,
    timeout: Duration,
    counts: ReleaseCounts,
}

impl<R: Clone + Ord, I> GroupOrder<R, I> {
    /// An empty history; a head-of-line sample that waits longer than
    /// `timeout` is committed past the numbers it waits for.
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            groups: BTreeMap::new(),
            timeout,
            counts: ReleaseCounts::default(),
        }
    }

    /// The counts since the history was made.
    pub(crate) fn counts(&self) -> ReleaseCounts {
        self.counts
    }

    /// Every writer the order heard of, with its group.
    pub(crate) fn writers(&self) -> Vec<(WriterGuid, WriterGuid)> {
        self.groups
            .iter()
            .flat_map(|(group, g)| g.writers.keys().map(move |w| (*group, *w)))
            .collect()
    }

    /// How many samples for `reader` wait: not committed yet, or committed
    /// into a group coherent set that is not released yet.
    pub(crate) fn held_for(&self, reader: &R) -> usize
    where
        R: PartialEq,
    {
        self.groups
            .values()
            .map(|g| {
                let pending = g
                    .pending
                    .values()
                    .flatten()
                    .filter(|e| e.item.is_some() && e.reader == *reader)
                    .count();
                let in_sets = g
                    .sets
                    .values()
                    .flat_map(|s| s.members.iter())
                    .filter(|e| e.reader == *reader)
                    .count();
                pending + in_sets
            })
            .sum()
    }

    /// No sample waits.
    pub(crate) fn is_empty(&self) -> bool {
        self.groups
            .values()
            .all(|g| g.pending.is_empty() && g.sets.values().all(|s| s.members.is_empty()))
    }

    /// A sample of `writer` of the remote `group`, in its writer's order,
    /// for `reader`. `end` for an end-of-coherent-set DATA, whose `item` is
    /// `None`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn offer(
        &mut self,
        group: WriterGuid,
        writer: WriterGuid,
        reader: R,
        info: ChangeGroupInfo,
        end: Option<SetEnd>,
        item: Option<I>,
        now: Duration,
    ) {
        let g = self.groups.entry(group).or_default();
        let progress = g.writers.entry(writer).or_default();
        let seen = progress.seen.entry(reader.clone()).or_insert(0);
        *seen = (*seen).max(info.gsn);
        progress.writer_set = Some(info.writer_set);
        if info.gsn <= g.committed {
            // Every writer's end of a group coherent set carries the same
            // number (§8.7.6): one after the first belongs to the set.
            if let (Some(end), Some(first)) = (end, info.coherent_set_gsn) {
                if let Some(set) = g.sets.get_mut(&first) {
                    set.ends.insert((writer, reader), end);
                    return;
                }
            }
            // A number already passed, after a timeout: the order cannot
            // place it any more.
            self.counts.late_samples += 1;
            return;
        }
        g.pending.entry(info.gsn).or_default().push(Entry {
            reader,
            writer,
            info,
            end,
            item,
            offered_at: now,
        });
    }

    /// A HEARTBEAT of `writer` of the remote `group` with group information.
    pub(crate) fn heartbeat(
        &mut self,
        group: WriterGuid,
        writer: WriterGuid,
        hb: HeartbeatGroupInfo,
    ) {
        let g = self.groups.entry(group).or_default();
        g.current_gsn = g.current_gsn.max(hb.current_gsn.0);
        let progress = g.writers.entry(writer).or_default();
        progress.heartbeat = Some(hb);
        // §8.7.5: the HEARTBEAT's writerSet is compared with the discovered
        // writers, as the one of a sample is.
        if hb.writer_set.is_some() {
            progress.writer_set = hb.writer_set;
        }
        progress.secure_writer_set = hb.secure_writer_set;
    }

    /// A GAP of `writer` of the remote `group` with group information, to
    /// `reader`.
    pub(crate) fn gap(
        &mut self,
        group: WriterGuid,
        writer: WriterGuid,
        reader: R,
        gap_end_gsn: i64,
    ) {
        let g = self.groups.entry(group).or_default();
        let end = g
            .writers
            .entry(writer)
            .or_default()
            .gap_end
            .entry(reader)
            .or_insert(0);
        *end = (*end).max(gap_end_gsn);
    }

    /// Commits what the rules allow and returns the samples to hand to the
    /// DataReaders, in commit order.
    pub(crate) fn release(&mut self, view: &dyn GroupView<R>, now: Duration) -> Vec<(R, I)> {
        let mut out = Vec::new();
        let timeout = self.timeout;
        let mut counts = self.counts;
        for (key, g) in &mut self.groups {
            release_group(key, g, view, now, timeout, &mut out, &mut counts);
        }
        self.counts = counts;
        out
    }
}

fn release_group<R: Clone + Ord, I>(
    key: &WriterGuid,
    g: &mut Group<R, I>,
    view: &dyn GroupView<R>,
    now: Duration,
    timeout: Duration,
    out: &mut Vec<(R, I)>,
    counts: &mut ReleaseCounts,
) {
    loop {
        // An open set whose end was committed is waiting for the ends of
        // the other counted writers; nothing after it goes first.
        if let Some(first) = g
            .sets
            .iter()
            .find(|(_, s)| s.end_committed)
            .map(|(f, _)| *f)
        {
            let counted = view.counted(key);
            let set = &g.sets[&first];
            let all_ends = counted.iter().all(|pair| set.ends.contains_key(pair));
            let waited = set
                .members
                .first()
                .map_or(Duration::ZERO, |m| now.saturating_sub(m.offered_at));
            if !all_ends && waited < timeout {
                return;
            }
            let set = g.sets.remove(&first).unwrap_or_default();
            finish_set(set, &counted, all_ends, out, counts);
            continue;
        }
        let Some((&next, _)) = g.pending.iter().next() else {
            return;
        };
        let missing = g.committed + 1;
        let ready = (next == missing || passable(key, g, view, missing, next))
            && copies_arrived(key, g, view, next);
        if !ready {
            let waited = g
                .pending
                .get(&next)
                .and_then(|v| v.first())
                .map_or(Duration::ZERO, |e| now.saturating_sub(e.offered_at));
            if waited < timeout {
                return;
            }
            counts.skipped_after_timeout += (next - missing) as u64;
        }
        g.committed = next;
        let entries = g.pending.remove(&next).unwrap_or_default();
        for e in entries {
            // An open set that is not this entry's and whose end was not
            // committed lost its end: the end's number, one after the
            // set's last, lies behind this one.
            let stale: Vec<i64> = g
                .sets
                .iter()
                .filter(|(first, set)| {
                    !set.end_committed && Some(**first) != e.info.coherent_set_gsn
                })
                .map(|(first, _)| *first)
                .collect();
            for first in stale {
                if let Some(set) = g.sets.remove(&first) {
                    counts.incomplete_sets += 1;
                    counts.discarded_samples += set.members.len() as u64;
                }
            }
            commit(g, e, out);
        }
    }
}

/// Every number from `missing` to `next − 1` is determined absent (§8.7.5).
fn passable<R: Ord, I>(
    key: &WriterGuid,
    g: &Group<R, I>,
    view: &dyn GroupView<R>,
    missing: i64,
    next: i64,
) -> bool {
    // A HEARTBEAT announced the group past the missing numbers, and the
    // writer set the samples carried names the writers this subscriber
    // discovered — a writer it does not know cannot be ruled out.
    let announced = g.current_gsn >= next;
    let set_known = g
        .writers
        .values()
        .filter_map(|w| w.writer_set)
        .all(|set| view.writer_set_is_discovered(key, set))
        && g.writers
            .values()
            .filter_map(|w| w.secure_writer_set)
            .all(|set| view.secure_writer_set_is_discovered(key, set));
    if !(announced && set_known) {
        return false;
    }
    let counted = view.counted(key);
    (missing..next).all(|gsn| {
        counted.iter().all(|(w, r)| {
            // A counted writer that sent nothing yet may have it.
            g.writers.get(w).is_some_and(|p| p.is_past(r, gsn))
        })
    })
}

/// Every DataReader matched with the writer of `gsn`'s samples has its
/// copy, or will get none: a copy still on its way to one reader is not
/// passed because another reader's arrived.
fn copies_arrived<R: Ord, I>(
    key: &WriterGuid,
    g: &Group<R, I>,
    view: &dyn GroupView<R>,
    gsn: i64,
) -> bool {
    let Some(entries) = g.pending.get(&gsn) else {
        return true;
    };
    let counted = view.counted(key);
    entries.iter().all(|e| {
        counted
            .iter()
            .filter(|(w, _)| *w == e.writer)
            .all(|(w, r)| {
                entries.iter().any(|other| other.reader == *r)
                    || g.writers.get(w).is_some_and(|p| p.is_past(r, gsn))
            })
    })
}

fn commit<R: Clone + Ord, I>(g: &mut Group<R, I>, e: Entry<R, I>, out: &mut Vec<(R, I)>) {
    if let Some(first) = e.info.coherent_set_gsn {
        let set = g.sets.entry(first).or_default();
        match e.end {
            Some(end) => {
                set.ends.insert((e.writer, e.reader.clone()), end);
                set.end_committed = true;
            }
            None => set.members.push(e),
        }
        return;
    }
    if let Some(item) = e.item {
        out.push((e.reader, item));
    }
}

/// Releases a group coherent set whose end was committed: whole when every
/// counted writer's part arrived at each of its DataReaders, or discarded
/// (§8.7.6).
fn finish_set<R: Ord, I>(
    set: OpenSet<R, I>,
    counted: &BTreeSet<(WriterGuid, R)>,
    all_ends: bool,
    out: &mut Vec<(R, I)>,
    counts: &mut ReleaseCounts,
) {
    let complete = all_ends
        && counted.iter().all(|pair| {
            let end = set.ends[pair];
            let got = set
                .members
                .iter()
                .filter(|m| m.writer == pair.0 && m.reader == pair.1)
                .count() as i64;
            match (end.first_sn, end.ecs_sn) {
                // Every change from its first sample up to its end.
                (Some(first), Some(ecs)) => got == ecs - first,
                // It wrote nothing into the set.
                (None, _) => got == 0,
                // An end without its own sequence number cannot be
                // checked; what arrived is what there is.
                (Some(_), None) => true,
            }
        });
    if complete {
        out.extend(
            set.members
                .into_iter()
                .filter_map(|m| m.item.map(|item| (m.reader, item))),
        );
    } else {
        counts.incomplete_sets += 1;
        counts.discarded_samples += set.members.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use zerodds_rtps::wire_types::SequenceNumber;

    const G: WriterGuid = [9; 16];
    const W1: WriterGuid = [1; 16];
    const W2: WriterGuid = [2; 16];
    const LONG: Duration = Duration::from_secs(3600);

    struct View {
        counted: BTreeSet<(WriterGuid, u8)>,
        known: bool,
    }

    impl GroupView<u8> for View {
        fn counted(&self, _: &WriterGuid) -> BTreeSet<(WriterGuid, u8)> {
            self.counted.clone()
        }
        fn writer_set_is_discovered(&self, _: &WriterGuid, _: GroupDigest) -> bool {
            self.known
        }
    }

    /// Each writer with the reader of its own number: W1 with reader 1, W2
    /// with reader 2.
    fn view(writers: &[WriterGuid]) -> View {
        View {
            counted: writers.iter().map(|w| (*w, w[0])).collect(),
            known: true,
        }
    }

    fn info(gsn: i64, set: Option<i64>) -> ChangeGroupInfo {
        ChangeGroupInfo {
            gsn,
            writer_set: GroupDigest([1, 2, 3, 4]),
            coherent_set_gsn: set,
        }
    }

    fn hb(current: i64, first: i64, last: i64) -> HeartbeatGroupInfo {
        HeartbeatGroupInfo {
            current_gsn: SequenceNumber(current),
            first_gsn: SequenceNumber(first),
            last_gsn: SequenceNumber(last),
            writer_set: None,
            secure_writer_set: None,
        }
    }

    fn sample(o: &mut GroupOrder<u8, i64>, w: WriterGuid, sn: i64, gsn: i64, reader: u8) {
        let _ = sn;
        o.offer(
            G,
            w,
            reader,
            info(gsn, None),
            None,
            Some(gsn),
            Duration::ZERO,
        );
    }

    fn released(o: &mut GroupOrder<u8, i64>, v: &View) -> Vec<i64> {
        o.release(v, Duration::ZERO)
            .into_iter()
            .map(|(_, i)| i)
            .collect()
    }

    #[test]
    fn samples_of_two_writers_go_out_in_group_sequence_order() {
        let mut o = GroupOrder::new(LONG);
        let v = view(&[W1, W2]);
        sample(&mut o, W2, 1, 2, 2);
        assert_eq!(released(&mut o, &v), [] as [i64; 0], "GSN 1 is missing");
        sample(&mut o, W1, 1, 1, 1);
        assert_eq!(released(&mut o, &v), [1, 2]);
    }

    /// GSN 2 belongs to a writer of the group this subscriber does not
    /// read. W1's HEARTBEAT announces the group at 3 and its own range
    /// [1, 1] ∪ [3, 3] not holding 2 once its DATA at 3 arrived.
    #[test]
    fn a_number_no_counted_writer_has_is_passed_once_a_heartbeat_announced_it() {
        let mut o = GroupOrder::new(LONG);
        let v = view(&[W1]);
        sample(&mut o, W1, 1, 1, 1);
        sample(&mut o, W1, 2, 3, 1);
        // W1 delivered GSN 3: it has advanced past 2. No HEARTBEAT yet.
        assert_eq!(released(&mut o, &v), [1]);
        o.heartbeat(G, W1, hb(3, 1, 3));
        assert_eq!(released(&mut o, &v), [3]);
    }

    #[test]
    fn a_counted_writer_whose_heartbeat_range_holds_the_number_is_waited_for() {
        let mut o = GroupOrder::new(LONG);
        let v = view(&[W1, W2]);
        sample(&mut o, W1, 1, 1, 1);
        sample(&mut o, W1, 2, 3, 1);
        // W2's range [2, 2] holds GSN 2, which has not arrived.
        o.heartbeat(G, W2, hb(3, 2, 2));
        o.heartbeat(G, W1, hb(3, 1, 3));
        assert_eq!(released(&mut o, &v), [1]);
        sample(&mut o, W2, 1, 2, 2);
        assert_eq!(released(&mut o, &v), [2, 3]);
    }

    #[test]
    fn a_gap_rules_the_number_out_for_its_writer() {
        let mut o = GroupOrder::new(LONG);
        let v = view(&[W1, W2]);
        sample(&mut o, W1, 1, 1, 1);
        sample(&mut o, W1, 2, 3, 1);
        o.heartbeat(G, W1, hb(3, 1, 3));
        // W2 has said nothing: it may have 2.
        assert_eq!(released(&mut o, &v), [1]);
        o.gap(G, W2, 2, 2);
        assert_eq!(released(&mut o, &v), [3]);
    }

    /// DDSI-RTPS 2.5 §8.7.5: the number is passed when "the
    /// Heartbeat.writerSet (and Heartbeat.secureWriterSet) matches the set of
    /// discovered DataWriters". A HEARTBEAT whose writerSet this subscriber
    /// did not discover, or whose secureWriterSet is not the empty group it
    /// keeps, does not let the number pass.
    #[test]
    fn a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open() {
        struct Picky;
        impl GroupView<u8> for Picky {
            fn counted(&self, _: &WriterGuid) -> BTreeSet<(WriterGuid, u8)> {
                BTreeSet::from([(W1, 1u8)])
            }
            fn writer_set_is_discovered(&self, _: &WriterGuid, set: GroupDigest) -> bool {
                set != GroupDigest([9; 4])
            }
        }
        let released_with = |writer_set: GroupDigest, secure: GroupDigest| {
            let mut o = GroupOrder::new(LONG);
            sample(&mut o, W1, 1, 1, 1);
            sample(&mut o, W1, 2, 3, 1);
            let mut announce = hb(3, 1, 3);
            announce.writer_set = Some(writer_set);
            announce.secure_writer_set = Some(secure);
            o.heartbeat(G, W1, announce);
            o.release(&Picky, Duration::ZERO)
                .into_iter()
                .map(|(_, i)| i)
                .collect::<Vec<i64>>()
        };
        // Discovered sets: GSN 2 is ruled out, 1 and 3 go.
        assert_eq!(
            released_with(GroupDigest([1, 2, 3, 4]), GroupDigest::EMPTY),
            [1, 3]
        );
        // A writer set no discovered writer has: only GSN 1.
        assert_eq!(released_with(GroupDigest([9; 4]), GroupDigest::EMPTY), [1]);
        // A secure set this subscriber keeps none of.
        assert_eq!(
            released_with(GroupDigest([1, 2, 3, 4]), GroupDigest([5; 4])),
            [1]
        );
    }

    #[test]
    fn an_undiscovered_writer_of_the_group_keeps_the_number_open() {
        let mut o = GroupOrder::new(LONG);
        let mut v = view(&[W1]);
        v.known = false;
        sample(&mut o, W1, 1, 1, 1);
        sample(&mut o, W1, 2, 3, 1);
        o.heartbeat(G, W1, hb(3, 1, 3));
        assert_eq!(released(&mut o, &v), [1]);
        v.known = true;
        assert_eq!(released(&mut o, &v), [3]);
    }

    #[test]
    fn a_sample_that_waited_past_the_timeout_goes_and_the_skip_is_counted() {
        let mut o = GroupOrder::new(Duration::from_millis(500));
        let v = view(&[W1, W2]);
        sample(&mut o, W2, 1, 3, 2);
        assert!(o.release(&v, Duration::from_millis(100)).is_empty());
        let out: Vec<i64> = o
            .release(&v, Duration::from_millis(600))
            .into_iter()
            .map(|(_, i)| i)
            .collect();
        assert_eq!(out, [3]);
        assert_eq!(o.counts().skipped_after_timeout, 2);
        // GSN 1 arriving now cannot be placed any more, and is counted.
        sample(&mut o, W1, 1, 1, 1);
        assert!(o.release(&v, Duration::from_millis(700)).is_empty());
        assert_eq!(o.counts().late_samples, 1);
    }

    /// DDSI-RTPS 2.5 Table 9.23: Writer 1 at SN 4 and 5 (GSN 11, 13),
    /// Writer 2 at SN 8 (GSN 12), both ECS at GSN 14, Writer 1's at SN 6,
    /// Writer 2's at SN 9.
    fn table_9_23(o: &mut GroupOrder<u8, i64>, skip: Option<i64>) {
        let set = Some(11);
        let member = |o: &mut GroupOrder<u8, i64>, w, sn, gsn, reader| {
            if Some(gsn) != skip {
                let _ = sn;
                o.offer(
                    G,
                    w,
                    reader,
                    info(gsn, set),
                    None,
                    Some(gsn),
                    Duration::ZERO,
                );
            }
        };
        member(o, W1, 4, 11, 1);
        member(o, W2, 8, 12, 2);
        member(o, W1, 5, 13, 1);
        let end = |first, ecs| SetEnd {
            first_sn: Some(first),
            ecs_sn: Some(ecs),
        };
        o.offer(
            G,
            W1,
            1,
            info(14, set),
            Some(end(4, 6)),
            None,
            Duration::ZERO,
        );
        o.offer(
            G,
            W2,
            2,
            info(14, set),
            Some(end(8, 9)),
            None,
            Duration::ZERO,
        );
    }

    #[test]
    fn the_group_coherent_set_of_table_9_23_goes_out_whole_after_its_end() {
        let mut o = GroupOrder::new(LONG);
        let v = view(&[W1, W2]);
        // The group was at 10 before the set.
        for gsn in 1..=10 {
            sample(&mut o, W1, gsn, gsn, 1);
        }
        let _ = released(&mut o, &v);
        table_9_23(&mut o, None);
        assert_eq!(released(&mut o, &v), [11, 12, 13]);
        assert_eq!(o.counts().incomplete_sets, 0);
    }

    #[test]
    fn a_set_short_of_one_member_is_discarded_and_counted() {
        let mut o = GroupOrder::new(Duration::from_millis(1));
        let v = view(&[W1, W2]);
        for gsn in 1..=10 {
            sample(&mut o, W1, gsn, gsn, 1);
        }
        let _ = released(&mut o, &v);
        // GSN 12, Writer 2's only member, never arrives; the order passes
        // it after the timeout, and the set is incomplete.
        table_9_23(&mut o, Some(12));
        let out = o.release(&v, Duration::from_secs(1));
        assert!(out.is_empty(), "nothing of an incomplete set goes out");
        assert_eq!(o.counts().incomplete_sets, 1);
        assert_eq!(o.counts().discarded_samples, 2);
    }

    /// W1 is matched with readers 1 and 3 of the subscriber; each gets its
    /// own copy of W1's samples. GSN 2 reached reader 1 only: it waits for
    /// reader 3's copy rather than leave it to arrive late.
    #[test]
    fn a_number_waits_for_the_copy_of_every_reader_of_its_writer() {
        let mut o = GroupOrder::new(LONG);
        let mut v = view(&[W1]);
        v.counted.insert((W1, 3));
        sample(&mut o, W1, 1, 1, 1);
        sample(&mut o, W1, 1, 1, 3);
        sample(&mut o, W1, 2, 2, 1);
        assert_eq!(released(&mut o, &v), [1, 1]);
        assert_eq!(o.held_for(&1), 1);
        sample(&mut o, W1, 2, 2, 3);
        assert_eq!(released(&mut o, &v), [2, 2]);
        assert_eq!(o.counts().late_samples, 0);
    }

    /// A number one reader skipped over (a later sample of the writer came
    /// first, so this one was not for it) does not hold the others' copies;
    /// the later number waits for the copy of the reader that has not got
    /// past it.
    #[test]
    fn a_reader_past_the_number_does_not_hold_it() {
        let mut o = GroupOrder::new(LONG);
        let mut v = view(&[W1]);
        v.counted.insert((W1, 3));
        sample(&mut o, W1, 1, 1, 1);
        sample(&mut o, W1, 2, 2, 3);
        assert_eq!(released(&mut o, &v), [1]);
        sample(&mut o, W1, 2, 2, 1);
        assert_eq!(released(&mut o, &v), [2, 2]);
    }

    /// A group coherent set with two readers of W1: each pair's part is
    /// complete, so the set goes out with both copies.
    #[test]
    fn a_set_is_complete_per_reader_of_each_writer() {
        let mut o = GroupOrder::new(LONG);
        let mut v = view(&[W1]);
        v.counted.insert((W1, 3));
        let set = Some(1);
        let end = SetEnd {
            first_sn: Some(1),
            ecs_sn: Some(3),
        };
        for reader in [1, 3] {
            o.offer(G, W1, reader, info(1, set), None, Some(1), Duration::ZERO);
            o.offer(G, W1, reader, info(2, set), None, Some(2), Duration::ZERO);
        }
        o.offer(G, W1, 1, info(3, set), Some(end), None, Duration::ZERO);
        assert_eq!(
            released(&mut o, &v),
            [] as [i64; 0],
            "reader 3's end is missing"
        );
        o.offer(G, W1, 3, info(3, set), Some(end), None, Duration::ZERO);
        let mut out = released(&mut o, &v);
        out.sort_unstable();
        assert_eq!(out, [1, 1, 2, 2]);
        assert_eq!(o.counts().incomplete_sets, 0);
    }

    #[test]
    fn a_set_waits_for_the_end_of_every_counted_writer() {
        let mut o = GroupOrder::new(LONG);
        let v = view(&[W1, W2]);
        let set = Some(1);
        o.offer(G, W1, 1, info(1, set), None, Some(1), Duration::ZERO);
        let end = SetEnd {
            first_sn: Some(1),
            ecs_sn: Some(2),
        };
        o.offer(G, W1, 1, info(2, set), Some(end), None, Duration::ZERO);
        assert_eq!(
            released(&mut o, &v),
            [] as [i64; 0],
            "Writer 2's end is missing"
        );
        let none = SetEnd {
            first_sn: None,
            ecs_sn: Some(1),
        };
        o.offer(G, W2, 2, info(2, set), Some(none), None, Duration::ZERO);
        assert_eq!(released(&mut o, &v), [1]);
    }
}

#[cfg(test)]
mod lost_end {
    #![allow(clippy::unwrap_used)]
    use super::*;

    struct All;
    impl GroupView<u8> for All {
        fn counted(&self, _: &WriterGuid) -> BTreeSet<(WriterGuid, u8)> {
            [([1u8; 16], 1)].into_iter().collect()
        }
        fn writer_set_is_discovered(&self, _: &WriterGuid, _: GroupDigest) -> bool {
            true
        }
    }

    /// The set's end (GSN 3) is lost; the order passes it after the
    /// timeout and commits GSN 4, outside the set: the set is discarded
    /// and counted, not held for ever.
    #[test]
    fn a_set_whose_end_was_passed_is_discarded_when_a_later_sample_commits() {
        let mut o: GroupOrder<u8, i64> = GroupOrder::new(Duration::from_millis(1));
        let w = [1u8; 16];
        let g = [9u8; 16];
        let info = |gsn, set| ChangeGroupInfo {
            gsn,
            writer_set: GroupDigest([1, 2, 3, 4]),
            coherent_set_gsn: set,
        };
        o.offer(g, w, 1, info(1, Some(1)), None, Some(1), Duration::ZERO);
        o.offer(g, w, 1, info(2, Some(1)), None, Some(2), Duration::ZERO);
        o.offer(g, w, 1, info(4, None), None, Some(4), Duration::ZERO);
        let out: Vec<i64> = o
            .release(&All, Duration::from_secs(1))
            .into_iter()
            .map(|(_, i)| i)
            .collect();
        assert_eq!(out, [4]);
        assert_eq!(o.counts().incomplete_sets, 1);
        assert_eq!(o.counts().discarded_samples, 2);
        assert_eq!(o.counts().skipped_after_timeout, 1);
    }
}
