// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a subscriber's PRESENTATION does to the samples a DataReader takes
//! off the runtime (DDS 1.4 §2.2.3.6; DDSI-RTPS 2.5 §8.7.5, §8.7.6): the
//! coherent-set hold of one reader, [`CoherentHold`], and the order of a
//! GROUP-scope subscriber across its readers, [`GroupOrdering`]. The typed
//! `DataReader` and the C API take them both.

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use zerodds_rtps::wire_types::EntityId;

use crate::group_order::{GroupOrder, GroupView, ReleaseCounts, SetEnd};
use crate::runtime::{DcpsRuntime, UserSample};

/// The coherent-set gate of one DataReader (DDS 1.4 §2.2.2.4.1.8-9 +
/// §2.2.3.6, wire mechanism DDSI-RTPS 2.5 §8.7.6): the members of a set
/// held back until the set ends.
#[derive(Debug, Default)]
pub struct CoherentHold {
    hold: Mutex<Vec<UserSample>>,
}

impl CoherentHold {
    /// An empty hold.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The gate for one item off the reader's channel.
    ///
    /// * `coherent_access` off — the item passes through unchanged, except
    ///   the ECS marker, which carries no user data and is dropped.
    /// * `coherent_access` on — a change tagged with `PID_COHERENT_SET`, a
    ///   sample or a dispose/unregister, is held back and yields nothing.
    ///   "The end of a Writer's coherent set is defined by the arrival of
    ///   one of the following: A Data Submessage from this Writer that
    ///   belongs to a new coherent set. A Data Submessage from this Writer
    ///   that does not contain a coherent set in-line QoS parameter or
    ///   alternatively, contains a coherent set in-line QoS parameter with
    ///   value SEQUENCENUMBER_UNKNOWN." Each of them releases the writer's
    ///   held set, in write order, ahead of itself; the ECS marker is such a
    ///   DATA without a sample.
    ///
    /// An incomplete set is discarded, and `on_incomplete` hears how many
    /// members it had. Returns the items the reader may act on now — zero,
    /// one, or the whole released set; never an ECS marker.
    pub fn gate(
        &self,
        item: UserSample,
        coherent_access: bool,
        on_incomplete: impl FnOnce(usize),
    ) -> Vec<UserSample> {
        if !coherent_access {
            return match item {
                UserSample::CoherentSetEnd { .. } => Vec::new(),
                other => alloc::vec![other],
            };
        }
        if let Some((writer_guid, sn)) = coherent_member(&item) {
            // A member of a new set ends the writer's earlier one.
            let released = self.release(writer_guid, Some(sn));
            if let Ok(mut hold) = self.hold.lock() {
                hold.push(item);
            }
            return released;
        }
        match item {
            UserSample::CoherentSetEnd {
                writer_guid,
                ecs_sn,
                complete,
                ..
            } => {
                let set = self.release(writer_guid, None);
                if set_is_complete(&set, ecs_sn, complete) {
                    set
                } else {
                    on_incomplete(set.len());
                    Vec::new()
                }
            }
            UserSample::Alive { writer_guid, .. } | UserSample::Lifecycle { writer_guid, .. } => {
                let mut out = self.release(writer_guid, None);
                out.push(item);
                out
            }
        }
    }

    /// How many members of open sets the hold has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hold.lock().map(|h| h.len()).unwrap_or(0)
    }

    /// The hold is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Releases the held coherent-set members of `writer_guid` in write
    /// order: all of them, or with `new_set = Some(x)` those of every set
    /// but `x` — a member of set `x` arrived, so the earlier set ended.
    fn release(&self, writer_guid: [u8; 16], new_set: Option<i64>) -> Vec<UserSample> {
        let Ok(mut hold) = self.hold.lock() else {
            return Vec::new();
        };
        let (released, kept): (Vec<_>, Vec<_>) = hold.drain(..).partition(|held| {
            coherent_member(held).is_some_and(|(g, sn)| g == writer_guid && new_set != Some(sn))
        });
        *hold = kept;
        released
    }
}

/// Reports the discard of an incomplete coherent set of `members` changes
/// on `topic`: a loud event, never a silent loss.
pub fn report_incomplete_set(rt: &DcpsRuntime, topic: &str, members: usize) {
    use zerodds_foundation::observability::{Component, Event, Level};
    rt.config.observability.record(
        &Event::new(
            Level::Warn,
            Component::Dcps,
            "reader.coherent_set.incomplete_discarded",
        )
        .with_attr("topic", topic)
        .with_attr("members", alloc::format!("{members}").as_str()),
    );
}

/// Whether `set`, the held members of one writer's set that ended at the
/// end-of-coherent-set DATA `ecs_sn`, is complete (DDSI-RTPS 2.5 §8.7.6):
/// as the RTPS reader followed it, or where none did, by its members — one
/// for each of the writer's changes from the set's first up to its end. A
/// reader made inside a set has its tail only.
fn set_is_complete(set: &[UserSample], ecs_sn: Option<i64>, complete: Option<bool>) -> bool {
    if let Some(complete) = complete {
        return complete;
    }
    match (set.first().and_then(coherent_member), ecs_sn) {
        (Some((_, first)), Some(end)) => set.len() as i64 == end - first,
        _ => true,
    }
}

/// The writer and the coherent set of a change that is a member of one: a
/// sample or a dispose/unregister that carried `PID_COHERENT_SET`
/// (DDSI-RTPS 2.5 §8.7.6).
fn coherent_member(item: &UserSample) -> Option<([u8; 16], i64)> {
    match item {
        UserSample::Alive {
            writer_guid,
            coherent_set_sn: Some(sn),
            ..
        }
        | UserSample::Lifecycle {
            writer_guid,
            coherent_set_sn: Some(sn),
            ..
        } => Some((*writer_guid, *sn)),
        _ => None,
    }
}

/// How long a sample of a GROUP-scope publisher waits for the group
/// sequence numbers before it (DDSI-RTPS 2.5 §8.7.5: "Implementations could
/// use additional timeout-based rules to limit delays"), and a group
/// coherent set for the ends of its writers.
const GROUP_ORDER_TIMEOUT: core::time::Duration = core::time::Duration::from_secs(1);

/// With `PRESENTATION.access_scope = GROUP` and ordered or coherent access,
/// the history of a subscriber where samples of a GROUP-scope publisher's
/// writers are ordered across its readers (DDSI-RTPS 2.5 §8.7.5, §8.7.6).
/// `R` names a reader of the subscriber.
#[derive(Debug)]
pub struct GroupOrdering<R> {
    order: Mutex<GroupOrder<R, UserSample>>,
    clock: std::time::Instant,
}

impl<R: Clone + Ord> Default for GroupOrdering<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Clone + Ord> GroupOrdering<R> {
    /// An empty order.
    #[must_use]
    pub fn new() -> Self {
        Self {
            order: Mutex::new(GroupOrder::new(GROUP_ORDER_TIMEOUT)),
            clock: std::time::Instant::now(),
        }
    }

    /// Takes `item`, off the channel of `reader`, into the order when it
    /// is a sample of a GROUP-scope publisher's writer; gives it back
    /// otherwise. For a subscriber that orders by group.
    pub fn offer(&self, rt: &DcpsRuntime, reader: R, item: UserSample) -> Option<UserSample> {
        let group = match &item {
            UserSample::Alive { group: Some(g), .. }
            | UserSample::Lifecycle { group: Some(g), .. }
            | UserSample::CoherentSetEnd { group: Some(g), .. } => *g,
            _ => return Some(item),
        };
        let key = rt
            .writer_group_guid(group.writer)
            .unwrap_or_else(|| participant_group_key(group.writer));
        let end = match &item {
            UserSample::CoherentSetEnd {
                set_first_sn,
                ecs_sn,
                ..
            } => Some(SetEnd {
                first_sn: *set_first_sn,
                ecs_sn: *ecs_sn,
            }),
            _ => None,
        };
        let now = self.clock.elapsed();
        let Ok(mut order) = self.order.lock() else {
            return Some(item);
        };
        let item = end.is_none().then_some(item);
        order.offer(key, group.writer, reader, group.info, end, item, now);
        None
    }

    /// How many samples for `reader` the order holds back.
    pub fn held_for(&self, reader: &R) -> usize {
        self.order.lock().map(|o| o.held_for(reader)).unwrap_or(0)
    }

    /// Commits what the order allows and hands each sample to its reader
    /// through `deliver`, under the order's lock; `deliver` says whether
    /// the reader took it. `readers` are the subscriber's, each with its
    /// runtime entity id. What the order passed or discarded is reported,
    /// and the status hooks hear of a sample handed on.
    ///
    /// Two threads release: the runtime's group hook (a HEARTBEAT, a GAP,
    /// the tick) and a reader's own ingest. Every release goes to the
    /// reader's inbox, in the order the group order released it, and a
    /// reader drains its inbox after it released. Lock order: the group
    /// order before an inbox.
    pub fn release(
        &self,
        rt: &Arc<DcpsRuntime>,
        readers: &[(EntityId, R)],
        mut deliver: impl FnMut(&R, UserSample) -> bool,
    ) {
        let (handed_on, before, after) = {
            let Ok(mut order) = self.order.lock() else {
                return;
            };
            if order.is_empty() {
                return;
            }
            for (group, writer) in order.writers() {
                for (eid, reader) in readers {
                    let progress = rt.writer_group_progress(writer, *eid);
                    if let Some(hb) = progress.heartbeat {
                        order.heartbeat(group, writer, hb);
                    }
                    if progress.gap_end_gsn > 0 {
                        order.gap(group, writer, reader.clone(), progress.gap_end_gsn);
                    }
                }
            }
            let view = SubscriberGroupView::new(rt, readers);
            let before = order.counts();
            let released = order.release(&view, self.clock.elapsed());
            let mut handed_on = false;
            for (reader, item) in released {
                handed_on |= deliver(&reader, item);
            }
            (handed_on, before, order.counts())
        };
        report_group_counts(rt, before, after);
        if handed_on {
            rt.notify_status_changed();
        }
    }
}

/// The group key of a writer whose announcement named no group: its
/// participant.
fn participant_group_key(writer: [u8; 16]) -> [u8; 16] {
    let mut key = [0u8; 16];
    key[..12].copy_from_slice(&writer[..12]);
    key
}

/// What the commit rule of DDSI-RTPS 2.5 §8.7.5 asks the subscriber: the
/// writers of each remote group matched with a reliable DataReader of it
/// and alive, and whether a writer set names the group's discovered
/// writers.
struct SubscriberGroupView<R> {
    rt: Arc<DcpsRuntime>,
    counted: BTreeMap<[u8; 16], BTreeSet<([u8; 16], R)>>,
}

impl<R: Clone + Ord> SubscriberGroupView<R> {
    /// The counted writers of each of `readers`, by the group they belong
    /// to, each with the reader.
    fn new(rt: &Arc<DcpsRuntime>, readers: &[(EntityId, R)]) -> Self {
        let mut counted: BTreeMap<[u8; 16], BTreeSet<([u8; 16], R)>> = BTreeMap::new();
        for (eid, reader) in readers {
            for writer in rt.user_reader_counted_writers(*eid) {
                let key = rt
                    .writer_group_guid(writer)
                    .unwrap_or_else(|| participant_group_key(writer));
                counted
                    .entry(key)
                    .or_default()
                    .insert((writer, reader.clone()));
            }
        }
        Self {
            rt: Arc::clone(rt),
            counted,
        }
    }
}

impl<R: Clone + Ord> GroupView<R> for SubscriberGroupView<R> {
    fn counted(&self, group: &[u8; 16]) -> BTreeSet<([u8; 16], R)> {
        self.counted.get(group).cloned().unwrap_or_default()
    }

    fn writer_set_is_discovered(
        &self,
        group: &[u8; 16],
        writer_set: zerodds_rtps::group_digest::GroupDigest,
    ) -> bool {
        // A participant key stands for a group no announcement named; its
        // writers cannot be listed, and the set is taken as it comes.
        if group[12..] == [0, 0, 0, 0] {
            return true;
        }
        zerodds_rtps::group_digest::GroupDigest::of_entities(&self.rt.group_writer_entities(*group))
            == writer_set
    }
}

/// Reports what a release of the group order passed or discarded: a loud
/// event per kind, never a silent loss.
fn report_group_counts(rt: &DcpsRuntime, before: ReleaseCounts, after: ReleaseCounts) {
    use zerodds_foundation::observability::{Component, Event, Level};
    let report = |name: &'static str, n: u64| {
        if n > 0 {
            rt.config.observability.record(
                &Event::new(Level::Warn, Component::Dcps, name)
                    .with_attr("count", alloc::format!("{n}").as_str()),
            );
        }
    };
    report(
        "subscriber.group_order.passed_after_timeout",
        after.skipped_after_timeout - before.skipped_after_timeout,
    );
    report(
        "subscriber.group_order.incomplete_set_discarded",
        after.incomplete_sets - before.incomplete_sets,
    );
    report(
        "subscriber.group_order.late_sample_discarded",
        after.late_samples - before.late_samples,
    );
}

/// What a RELIABLE DataReader's TIME_BASED_FILTER held back (DDS 1.4
/// §2.2.3.12): "In the case where the reliability QoS kind is RELIABLE
/// then in steady-state, defined as the situation where the DataWriter
/// does not write new samples for a period 'long' compared to the
/// minimum_separation, the system should guarantee delivery the last
/// sample to the DataReader." Per instance the last filtered sample, due
/// `minimum_separation` after it arrived; a sample of the instance that
/// passes the filter supersedes it, a later filtered one replaces it.
/// `S` is the sample, `K` the instance.
#[derive(Debug)]
pub struct TbfHold<K, S> {
    held: BTreeMap<K, (S, crate::time::Time)>,
}

impl<K: Ord, S> Default for TbfHold<K, S> {
    fn default() -> Self {
        Self {
            held: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone, S> TbfHold<K, S> {
    /// Holds `sample` of `instance`, filtered on its arrival at `arrival`,
    /// until `arrival + minimum_separation`.
    pub fn hold(
        &mut self,
        instance: K,
        sample: S,
        arrival: crate::time::Time,
        minimum_separation_nanos: u128,
    ) {
        let due = time_after(arrival, minimum_separation_nanos);
        self.held.insert(instance, (sample, due));
    }

    /// A sample of `instance` passed the filter: the held one is older.
    pub fn supersede(&mut self, instance: &K) {
        self.held.remove(instance);
    }

    /// The held samples due at `now`, in instance order.
    pub fn take_due(&mut self, now: crate::time::Time) -> Vec<(K, S)> {
        let due: Vec<K> = self
            .held
            .iter()
            .filter(|(_, (_, at))| *at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        due.into_iter()
            .filter_map(|k| self.held.remove(&k).map(|(s, _)| (k, s)))
            .collect()
    }

    /// Nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Forgets everything held: the filter is off.
    pub fn clear(&mut self) {
        self.held.clear();
    }
}

/// `at + nanos`.
fn time_after(at: crate::time::Time, nanos: u128) -> crate::time::Time {
    let total = u128::from(u64::try_from(at.sec).unwrap_or(0)) * 1_000_000_000
        + u128::from(at.nanosec)
        + nanos;
    crate::time::Time::new(
        i32::try_from(total / 1_000_000_000).unwrap_or(i32::MAX),
        (total % 1_000_000_000) as u32,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::time::Time;

    const SECOND: u128 = 1_000_000_000;

    #[test]
    fn the_last_filtered_sample_of_an_instance_is_due_a_separation_after_it_arrived() {
        let mut hold: TbfHold<u8, &str> = TbfHold::default();
        hold.hold(1, "first", Time::new(10, 0), SECOND);
        // A later filtered sample of the instance replaces it.
        hold.hold(1, "second", Time::new(10, 500_000_000), SECOND);
        hold.hold(2, "other", Time::new(10, 0), SECOND);
        assert!(hold.take_due(Time::new(10, 999_999_999)).is_empty());
        assert_eq!(hold.take_due(Time::new(11, 0)), alloc::vec![(2, "other")]);
        assert_eq!(
            hold.take_due(Time::new(11, 500_000_000)),
            alloc::vec![(1, "second")]
        );
        assert!(hold.is_empty());
    }

    #[test]
    fn a_sample_that_passes_the_filter_supersedes_the_held_one() {
        let mut hold: TbfHold<u8, &str> = TbfHold::default();
        hold.hold(1, "held", Time::new(10, 0), SECOND);
        hold.supersede(&1);
        assert!(hold.is_empty());
        assert!(hold.take_due(Time::new(20, 0)).is_empty());
    }
}
