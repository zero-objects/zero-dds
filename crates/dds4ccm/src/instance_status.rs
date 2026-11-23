// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ReadInfo::instance_status` from the `DDS::SampleInfo` of a sample
//! (DDS4CCM 1.1 §7.2.2.1.2, Figure 7.2).
//!
//! The status is "the instance status at the time of the sample", "as
//! perceived by the component" (Annex A). Figure 7.2 is a state chart per
//! instance with the states `INSTANCE_CREATED`, `INSTANCE_UPDATED` and
//! `INSTANCE_DELETED`; a connector fragment keeps that state per instance
//! of the DataReader of its port ([`InstanceView`]) and moves it with each
//! sample it hands to the component:
//!
//! | state before | sample | status |
//! |---|---|---|
//! | none, or deleted | data that pass the query | `INSTANCE_CREATED` |
//! | existing | data that pass, `disposed_generation_count` changed | `INSTANCE_CREATED` |
//! | existing, inside the query | data that pass, `disposed_generation_count` unchanged | `INSTANCE_UPDATED` |
//! | existing, inside the query | data the query rejects | `INSTANCE_FILTERED_OUT` |
//! | existing, outside the query | data that pass, `disposed_generation_count` unchanged | `INSTANCE_FILTERED_IN` |
//! | existing, outside the query | data the query rejects | none: the instance stays outside |
//! | none, or deleted | data the query rejects | none: the instance does not exist for the component |
//! | existing | no data, `NOT_ALIVE_DISPOSED` | `INSTANCE_DELETED` |
//! | any | any other sample without data | none: kept by the fragment |
//!
//! The chart moves on "sample received": a sample the component has not
//! seen (`NOT_READ`) moves the state. A sample it reads again (`READ`,
//! `ALREADY_SEEN`) has the status of its time: `INSTANCE_CREATED` for the
//! sample that created the instance, `INSTANCE_FILTERED_IN` for the one
//! with which it reentered the query last, `INSTANCE_UPDATED` for the
//! others.
//!
//! How the rows follow from the figure and the text beside it:
//!
//! - A sample with `valid_data == false` is passed only as
//!   `INSTANCE_DELETED`: "Except if the instance_status is
//!   INSTANCE_DELETED, the associated data value is valid (other cases
//!   where DDS::SampleInfo::valid_data would be FALSE should be managed by
//!   the connector fragment and shouldn't be passed to the component)".
//!   The DataReader reports every disposal with such a sample, and a
//!   disposal of an instance the component never saw, or whose deletion it
//!   was told already, has no transition in the figure.
//! - A changed `disposed_generation_count` is the figure's second way into
//!   `INSTANCE_DELETED`, and "valid_data == true && disposed_generation_count
//!   changed" is its way from there to `INSTANCE_CREATED`: the instance was
//!   disposed and written again without the component seeing the disposal,
//!   and the one sample that shows it takes both transitions.
//! - A sample with data whose instance is `NOT_ALIVE_DISPOSED` by the time
//!   it is read was written while the instance existed; its status is the
//!   one of that time, and the sample without data that follows it reports
//!   the deletion. The datum of `INSTANCE_DELETED` holds the key only
//!   (`StateListener::on_deletion`), which a sample with data is not.
//! - `NOT_ALIVE_NO_WRITERS` is no deletion: the figure has it on the
//!   transitions into `INSTANCE_UPDATED`.
//! - The guard `view_state == NEW` of the figure is the reader's knowledge
//!   of the instance; the fragment's own state says the same for the
//!   component, and it also holds when several samples of a new instance
//!   are read at once (the DataReader gives all of them the view state of
//!   the instance at the time of the read).
//!
//! # `INSTANCE_FILTERED_IN` and `INSTANCE_FILTERED_OUT`
//!
//! "INSTANCE_FILTERED_OUT if an existing instance just stopped passing the
//! filter", "INSTANCE_FILTERED_IN if an existing instance reenters the
//! filter after having been filtered out". The specification adds the two
//! "for provision" because "in the current DDS, the fact that a data is
//! filtered out is not reported". For the query of the Reader of the port
//! (§7.2.2.1.3: "the filter (if any) of the related Reader") the fragment
//! does not depend on that report: on the path of fresh data it evaluates
//! the query itself and sees every sample the query rejects
//! ([`InstanceView::reject`]).
//!
//! - The first fresh sample with data of an existing instance that the
//!   query rejects is `INSTANCE_FILTERED_OUT`. It is handed to the component
//!   with its data, which are valid as the first note under Figure 7.2 says
//!   of every status but `INSTANCE_DELETED`. The instance still exists for
//!   the component and is outside the query from then on.
//! - Further rejected samples of an instance that is outside, and rejected
//!   samples of an instance that does not exist for the component, are not
//!   handed over.
//! - The next sample of an instance outside the query that passes is
//!   `INSTANCE_FILTERED_IN`, wherever the component gets it (a get, a
//!   listener, or a read of the Reader that finds it `NOT_READ`). With a
//!   changed `disposed_generation_count` it is `INSTANCE_CREATED`: the
//!   instance was deleted and created again while it was outside.
//! - The disposal of an instance outside the query is `INSTANCE_DELETED`:
//!   the instance exists until then.
//!
//! The content filter of a port (attribute `filter`, a
//! ContentFilteredTopic) is a different filter: a sample it rejects does
//! not reach the DataReader, and the two statuses are not computed for it.

use std::collections::BTreeMap;

use zerodds_dcps::sample_info::{InstanceStateKind, SampleInfo, SampleStateKind};
use zerodds_dcps::{InstanceHandle, Sample, Time};

use crate::ccm_dds::{AccessStatus, InstanceStatus, ReadInfo};

/// What the fragment keeps of an instance that exists for the component.
#[derive(Debug, Clone, Copy)]
struct Existing {
    /// The `disposed_generation_count` of the samples the component got.
    generation: i32,
    /// The source timestamp of the sample that was `INSTANCE_CREATED`.
    created: Time,
    /// The last fresh sample of the instance the fragment evaluated the
    /// query of the Reader against failed it.
    outside: bool,
    /// The source timestamp of the sample that was `INSTANCE_FILTERED_IN`
    /// last, of the generation the component knows.
    entered: Option<Time>,
}

/// The instances that exist for the component of one port.
#[derive(Debug, Default)]
pub(crate) struct InstanceView {
    existing: BTreeMap<InstanceHandle, Existing>,
}

impl InstanceView {
    /// The status of a sample the component has not seen (`NOT_READ`) and
    /// that passed the query of the Reader, or has no data; and the state
    /// of its instance after it. `None` for a sample the fragment keeps to
    /// itself. The table of the module documentation.
    ///
    /// A sample with data of a generation before the one the component
    /// knows is older data of an instance that was created again since (a
    /// Reader of the port read the newer sample first); it leaves the
    /// generation as it is.
    pub(crate) fn perceive(&mut self, info: &SampleInfo) -> Option<InstanceStatus> {
        let handle = info.instance_handle;
        if info.valid_data {
            let generation = info.disposed_generation_count;
            return Some(match self.existing.get_mut(&handle) {
                Some(known) if known.generation >= generation => {
                    if known.outside {
                        known.outside = false;
                        known.entered = Some(info.source_timestamp);
                        InstanceStatus::InstanceFilteredIn
                    } else {
                        InstanceStatus::InstanceUpdated
                    }
                }
                _ => {
                    self.existing.insert(
                        handle,
                        Existing {
                            generation,
                            created: info.source_timestamp,
                            outside: false,
                            entered: None,
                        },
                    );
                    InstanceStatus::InstanceCreated
                }
            });
        }
        if info.instance_state == InstanceStateKind::NotAliveDisposed
            && self.existing.remove(&handle).is_some()
        {
            return Some(InstanceStatus::InstanceDeleted);
        }
        None
    }

    /// The status of a fresh sample with data that the query of the Reader
    /// rejects, and the state of its instance after it:
    /// `INSTANCE_FILTERED_OUT` for an instance that exists for the
    /// component and was inside the query, `None` otherwise (the instance
    /// is outside already, or does not exist for the component).
    pub(crate) fn reject(&mut self, info: &SampleInfo) -> Option<InstanceStatus> {
        match self.existing.get_mut(&info.instance_handle) {
            Some(known) if !known.outside => {
                known.outside = true;
                Some(InstanceStatus::InstanceFilteredOut)
            }
            _ => None,
        }
    }

    /// The status of a sample the component saw before (`READ`): the one
    /// it had at its time, `INSTANCE_CREATED` for the sample that created
    /// the instance of the generation the component knows,
    /// `INSTANCE_FILTERED_IN` for the one with which it reentered the query
    /// last, else `INSTANCE_UPDATED`. The state does not move: the figure
    /// moves it on "sample received". `None` for a sample without data.
    pub(crate) fn recall(&self, info: &SampleInfo) -> Option<InstanceStatus> {
        if !info.valid_data {
            return None;
        }
        Some(match self.existing.get(&info.instance_handle) {
            Some(known) if known.generation == info.disposed_generation_count => {
                if known.created == info.source_timestamp {
                    InstanceStatus::InstanceCreated
                } else if known.entered == Some(info.source_timestamp) {
                    InstanceStatus::InstanceFilteredIn
                } else {
                    InstanceStatus::InstanceUpdated
                }
            }
            _ => InstanceStatus::InstanceUpdated,
        })
    }

    /// Forgets the instances `known` does not hold any more: a DataReader
    /// purges an instance without samples and writers, and no sample of it
    /// reaches the port again under that handle.
    pub(crate) fn retain(&mut self, mut known: impl FnMut(InstanceHandle) -> bool) {
        self.existing.retain(|handle, _| known(*handle));
    }

    /// The number of instances that exist for the component.
    pub(crate) fn len(&self) -> usize {
        self.existing.len()
    }

    /// Whether `instance` exists for the component.
    pub(crate) fn contains(&self, instance: InstanceHandle) -> bool {
        self.existing.contains_key(&instance)
    }
}

/// `ReadInfo::access_status` from the sample state.
pub(crate) fn access_status(state: SampleStateKind) -> AccessStatus {
    match state {
        SampleStateKind::NotRead => AccessStatus::FreshInfo,
        SampleStateKind::Read => AccessStatus::AlreadySeen,
    }
}

/// Orders `samples` by instance: the samples of the first instance, then
/// those of the second, each in the order they came ("They will always be
/// ordered by instances", §7.2.2.1.2). A DataReader presents them so
/// already unless its PRESENTATION asks for the order of the topic.
pub(crate) fn order_by_instance<T>(samples: &mut [Sample<T>]) {
    let mut rank: BTreeMap<InstanceHandle, usize> = BTreeMap::new();
    for sample in samples.iter() {
        let next = rank.len();
        rank.entry(sample.info.instance_handle).or_insert(next);
    }
    samples.sort_by_key(|sample| rank.get(&sample.info.instance_handle).copied());
}

/// Orders what [`deliver`] pushed by instance, as [`order_by_instance`]
/// orders samples.
pub(crate) fn order_delivered_by_instance<T>(data: &mut Vec<T>, infos: &mut Vec<ReadInfo>) {
    let mut rank: BTreeMap<InstanceHandle, usize> = BTreeMap::new();
    for info in infos.iter() {
        let next = rank.len();
        rank.entry(info.instance_handle).or_insert(next);
    }
    let mut pairs: Vec<(T, ReadInfo)> = data.drain(..).zip(infos.drain(..)).collect();
    pairs.sort_by_key(|(_, info)| rank.get(&info.instance_handle).copied());
    for (datum, info) in pairs {
        data.push(datum);
        infos.push(info);
    }
}

/// Hands `samples` to the component: each sample the view passes is pushed
/// to `data` with its [`ReadInfo`] in `infos`. A sample that is `NOT_READ`
/// moves the view, one that is `READ` gets the status of its time.
/// `passes` is the query of the Reader for a sample with data: `true` for
/// every sample of a read that went through the query in DDS.
pub(crate) fn deliver<T>(
    view: &mut InstanceView,
    samples: Vec<Sample<T>>,
    passes: impl Fn(&T) -> bool,
    data: &mut Vec<T>,
    infos: &mut Vec<ReadInfo>,
) {
    for sample in samples {
        let status = match sample.info.sample_state {
            SampleStateKind::NotRead if sample.info.valid_data && !passes(&sample.data) => {
                view.reject(&sample.info)
            }
            SampleStateKind::NotRead => view.perceive(&sample.info),
            SampleStateKind::Read => view.recall(&sample.info),
        };
        let Some(instance_status) = status else {
            continue;
        };
        infos.push(ReadInfo {
            instance_handle: sample.info.instance_handle,
            source_timestamp: sample.info.source_timestamp,
            access_status: access_status(sample.info.sample_state),
            instance_status,
        });
        data.push(sample.data);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_dcps::sample_info::ViewStateKind;

    fn info(handle: u64, state: InstanceStateKind, valid: bool, generation: i32) -> SampleInfo {
        SampleInfo {
            instance_handle: InstanceHandle::from_raw(handle),
            instance_state: state,
            valid_data: valid,
            disposed_generation_count: generation,
            ..SampleInfo::default()
        }
    }

    /// Figure 7.2: the initial transition, the two into INSTANCE_UPDATED
    /// and the one into INSTANCE_DELETED.
    #[test]
    fn dds4ccm_7_2_2_1_2_figure_7_2_created_updated_deleted() {
        let mut view = InstanceView::default();
        let alive = InstanceStateKind::Alive;
        assert_eq!(
            view.perceive(&info(1, alive, true, 0)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&info(1, alive, true, 0)),
            Some(InstanceStatus::InstanceUpdated)
        );
        // NOT_ALIVE_NO_WRITERS with data is an update.
        assert_eq!(
            view.perceive(&info(1, InstanceStateKind::NotAliveNoWriters, true, 0)),
            Some(InstanceStatus::InstanceUpdated)
        );
        assert_eq!(
            view.perceive(&info(1, InstanceStateKind::NotAliveDisposed, false, 0)),
            Some(InstanceStatus::InstanceDeleted)
        );
        assert_eq!(view.len(), 0);
        // From INSTANCE_DELETED a sample with data creates again.
        assert_eq!(
            view.perceive(&info(1, alive, true, 1)),
            Some(InstanceStatus::InstanceCreated)
        );
    }

    /// Figure 7.2: "disposed_generation_count changed" leads to
    /// INSTANCE_DELETED and, for a sample with data, on to
    /// INSTANCE_CREATED.
    #[test]
    fn dds4ccm_7_2_2_1_2_figure_7_2_a_changed_disposed_generation_count_creates_again() {
        let mut view = InstanceView::default();
        let alive = InstanceStateKind::Alive;
        assert_eq!(
            view.perceive(&info(7, alive, true, 0)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&info(7, alive, true, 1)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&info(7, alive, true, 1)),
            Some(InstanceStatus::InstanceUpdated)
        );
    }

    /// The note under Figure 7.2: a sample without data is passed only as
    /// INSTANCE_DELETED of an instance that exists for the component.
    #[test]
    fn dds4ccm_7_2_2_1_2_a_sample_without_data_is_passed_only_as_a_deletion() {
        let mut view = InstanceView::default();
        // A disposal of an instance the component never saw.
        assert_eq!(
            view.perceive(&info(2, InstanceStateKind::NotAliveDisposed, false, 0)),
            None
        );
        assert_eq!(
            view.perceive(&info(2, InstanceStateKind::Alive, true, 0)),
            Some(InstanceStatus::InstanceCreated)
        );
        // The last writer left: no deletion, nothing to pass.
        assert_eq!(
            view.perceive(&info(2, InstanceStateKind::NotAliveNoWriters, false, 0)),
            None
        );
        assert_eq!(
            view.perceive(&info(2, InstanceStateKind::NotAliveDisposed, false, 0)),
            Some(InstanceStatus::InstanceDeleted)
        );
        // The deletion is reported once.
        assert_eq!(
            view.perceive(&info(2, InstanceStateKind::NotAliveDisposed, false, 0)),
            None
        );
    }

    /// A sample with data that is read after its instance was disposed has
    /// the status of its own time; the sample without data reports the
    /// deletion.
    #[test]
    fn dds4ccm_7_2_2_1_2_data_read_after_the_disposal_keeps_the_status_of_its_time() {
        let mut view = InstanceView::default();
        let disposed = InstanceStateKind::NotAliveDisposed;
        assert_eq!(
            view.perceive(&info(3, disposed, true, 0)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&info(3, disposed, true, 0)),
            Some(InstanceStatus::InstanceUpdated)
        );
        assert_eq!(
            view.perceive(&info(3, disposed, false, 0)),
            Some(InstanceStatus::InstanceDeleted)
        );
    }

    /// A sample the component reads again has the status of its time and
    /// does not move the state; older data of a generation the component
    /// has left is an update.
    #[test]
    fn dds4ccm_7_2_2_1_2_a_sample_read_again_keeps_its_status_and_does_not_move_the_state() {
        let mut view = InstanceView::default();
        let at = |sec: i32, generation: i32, state: SampleStateKind| SampleInfo {
            source_timestamp: Time::new(sec, 0),
            sample_state: state,
            ..info(5, InstanceStateKind::Alive, true, generation)
        };
        let fresh = SampleStateKind::NotRead;
        let seen = SampleStateKind::Read;
        assert_eq!(
            view.perceive(&at(10, 0, fresh)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&at(11, 0, fresh)),
            Some(InstanceStatus::InstanceUpdated)
        );
        // Read again, any number of times.
        for _ in 0..2 {
            assert_eq!(
                view.recall(&at(10, 0, seen)),
                Some(InstanceStatus::InstanceCreated)
            );
            assert_eq!(
                view.recall(&at(11, 0, seen)),
                Some(InstanceStatus::InstanceUpdated)
            );
        }
        // The instance was disposed and written again: generation 1.
        assert_eq!(
            view.perceive(&at(20, 1, fresh)),
            Some(InstanceStatus::InstanceCreated)
        );
        // Data of generation 0 that comes after it is an update and leaves
        // the state: the next sample of generation 1 is an update too.
        assert_eq!(
            view.perceive(&at(12, 0, fresh)),
            Some(InstanceStatus::InstanceUpdated)
        );
        assert_eq!(
            view.perceive(&at(21, 1, fresh)),
            Some(InstanceStatus::InstanceUpdated)
        );
        assert_eq!(
            view.recall(&at(10, 0, seen)),
            Some(InstanceStatus::InstanceUpdated),
            "the creating sample of a generation the component has left"
        );
        // A sample without data is never recalled.
        assert_eq!(
            view.recall(&SampleInfo {
                sample_state: seen,
                ..info(5, InstanceStateKind::NotAliveDisposed, false, 1)
            }),
            None
        );
    }

    /// §7.2.2.1.2: "INSTANCE_FILTERED_OUT if an existing instance just
    /// stopped passing the filter", "INSTANCE_FILTERED_IN if an existing
    /// instance reenters the filter after having been filtered out".
    #[test]
    fn dds4ccm_7_2_2_1_2_an_existing_instance_leaves_and_reenters_the_query() {
        let mut view = InstanceView::default();
        let alive = InstanceStateKind::Alive;
        let sample = info(4, alive, true, 0);
        // An instance that does not exist for the component cannot leave.
        assert_eq!(view.reject(&sample), None);
        assert!(!view.contains(sample.instance_handle));
        assert_eq!(
            view.perceive(&sample),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&sample),
            Some(InstanceStatus::InstanceUpdated)
        );
        // It just stopped passing: once.
        assert_eq!(
            view.reject(&sample),
            Some(InstanceStatus::InstanceFilteredOut)
        );
        assert_eq!(view.reject(&sample), None);
        assert!(
            view.contains(sample.instance_handle),
            "outside the query the instance still exists"
        );
        // It reenters: once, then it is updated again.
        assert_eq!(
            view.perceive(&sample),
            Some(InstanceStatus::InstanceFilteredIn)
        );
        assert_eq!(
            view.perceive(&sample),
            Some(InstanceStatus::InstanceUpdated)
        );
        assert_eq!(
            view.reject(&sample),
            Some(InstanceStatus::InstanceFilteredOut)
        );
    }

    /// An instance outside the query is deleted by its disposal, and
    /// created when it comes again with a changed
    /// `disposed_generation_count`; neither is a reentry.
    #[test]
    fn dds4ccm_7_2_2_1_2_an_instance_outside_the_query_is_deleted_or_created_again() {
        let mut view = InstanceView::default();
        let alive = InstanceStateKind::Alive;
        let disposed = InstanceStateKind::NotAliveDisposed;
        assert_eq!(
            view.perceive(&info(6, alive, true, 0)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.reject(&info(6, alive, true, 0)),
            Some(InstanceStatus::InstanceFilteredOut)
        );
        assert_eq!(
            view.perceive(&info(6, disposed, false, 0)),
            Some(InstanceStatus::InstanceDeleted)
        );
        // Deleted: a rejected sample is nothing, a passing one creates.
        assert_eq!(view.reject(&info(6, alive, true, 1)), None);
        assert_eq!(
            view.perceive(&info(6, alive, true, 1)),
            Some(InstanceStatus::InstanceCreated)
        );
        // Outside again, then disposed and written again unseen.
        assert_eq!(
            view.reject(&info(6, alive, true, 1)),
            Some(InstanceStatus::InstanceFilteredOut)
        );
        assert_eq!(
            view.perceive(&info(6, alive, true, 2)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&info(6, alive, true, 2)),
            Some(InstanceStatus::InstanceUpdated),
            "the creation left the instance inside the query"
        );
    }

    /// The sample with which an instance reentered the query keeps that
    /// status when it is read again; `deliver` asks the query only for a
    /// sample with data that is `NOT_READ`.
    #[test]
    fn dds4ccm_7_2_2_1_2_deliver_asks_the_query_for_fresh_data_and_recalls_the_reentry() {
        let sample = |sec: i32, value: u32, state: SampleStateKind, valid: bool| {
            Sample::new(
                value,
                SampleInfo {
                    source_timestamp: Time::new(sec, 0),
                    sample_state: state,
                    ..info(
                        8,
                        if valid {
                            InstanceStateKind::Alive
                        } else {
                            InstanceStateKind::NotAliveDisposed
                        },
                        valid,
                        0,
                    )
                },
            )
        };
        let fresh = SampleStateKind::NotRead;
        let seen = SampleStateKind::Read;
        let mut view = InstanceView::default();
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        let even = |value: &u32| value.is_multiple_of(2);
        deliver(
            &mut view,
            vec![
                sample(1, 1, fresh, true),
                sample(2, 2, fresh, true),
                sample(3, 4, fresh, true),
                sample(4, 5, fresh, true),
                sample(5, 7, fresh, true),
                sample(6, 8, fresh, true),
                sample(7, 10, fresh, true),
            ],
            even,
            &mut data,
            &mut infos,
        );
        let statuses: Vec<(u32, InstanceStatus)> = data
            .iter()
            .copied()
            .zip(infos.iter().map(|i| i.instance_status))
            .collect();
        assert_eq!(
            statuses,
            [
                (2, InstanceStatus::InstanceCreated),
                (4, InstanceStatus::InstanceUpdated),
                (5, InstanceStatus::InstanceFilteredOut),
                (8, InstanceStatus::InstanceFilteredIn),
                (10, InstanceStatus::InstanceUpdated),
            ]
        );
        // Read again: the query is not asked (5 and 7 would fail it), and
        // each sample has the status of its time.
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        deliver(
            &mut view,
            vec![
                sample(2, 2, seen, true),
                sample(4, 5, seen, true),
                sample(6, 8, seen, true),
                sample(7, 10, seen, true),
            ],
            |_| false,
            &mut data,
            &mut infos,
        );
        let statuses: Vec<InstanceStatus> = infos.iter().map(|i| i.instance_status).collect();
        assert_eq!(
            statuses,
            [
                InstanceStatus::InstanceCreated,
                InstanceStatus::InstanceUpdated,
                InstanceStatus::InstanceFilteredIn,
                InstanceStatus::InstanceUpdated,
            ]
        );
        // A sample without data is not asked either: the disposal of an
        // instance is its deletion whatever the query says.
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        deliver(
            &mut view,
            vec![sample(8, 0, fresh, false)],
            |_| false,
            &mut data,
            &mut infos,
        );
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].instance_status, InstanceStatus::InstanceDeleted);
    }

    #[test]
    fn dds4ccm_7_2_2_1_2_access_status_follows_the_sample_state() {
        assert_eq!(
            access_status(SampleStateKind::NotRead),
            AccessStatus::FreshInfo
        );
        assert_eq!(
            access_status(SampleStateKind::Read),
            AccessStatus::AlreadySeen
        );
    }

    /// "They will always be ordered by instances": the samples of an
    /// instance are consecutive, in the order of their arrival, and the
    /// instances in the order of their first sample.
    #[test]
    fn dds4ccm_7_2_2_1_2_what_is_returned_is_ordered_by_instance() {
        let sample = |handle: u64, value: u32| {
            Sample::new(
                value,
                SampleInfo {
                    instance_handle: InstanceHandle::from_raw(handle),
                    view_state: ViewStateKind::NotNew,
                    ..SampleInfo::default()
                },
            )
        };
        let mut samples = vec![
            sample(9, 1),
            sample(4, 2),
            sample(9, 3),
            sample(4, 4),
            sample(5, 5),
        ];
        order_by_instance(&mut samples);
        let order: Vec<u32> = samples.iter().map(|s| s.data).collect();
        assert_eq!(order, [1, 3, 2, 4, 5]);

        let mut view = InstanceView::default();
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        deliver(&mut view, samples, |_| true, &mut data, &mut infos);
        assert_eq!(data, [1, 3, 2, 4, 5]);
        let statuses: Vec<InstanceStatus> = infos.iter().map(|i| i.instance_status).collect();
        assert_eq!(
            statuses,
            [
                InstanceStatus::InstanceCreated,
                InstanceStatus::InstanceUpdated,
                InstanceStatus::InstanceCreated,
                InstanceStatus::InstanceUpdated,
                InstanceStatus::InstanceCreated
            ]
        );
    }

    #[test]
    fn dds4ccm_7_2_2_1_2_instances_the_reader_purged_are_forgotten() {
        let mut view = InstanceView::default();
        for handle in 1..=4 {
            view.perceive(&info(handle, InstanceStateKind::Alive, true, 0));
        }
        view.retain(|handle| handle.as_raw() % 2 == 0);
        assert_eq!(view.len(), 2);
        // A forgotten instance is new when it comes again.
        assert_eq!(
            view.perceive(&info(1, InstanceStateKind::Alive, true, 0)),
            Some(InstanceStatus::InstanceCreated)
        );
        assert_eq!(
            view.perceive(&info(2, InstanceStateKind::Alive, true, 0)),
            Some(InstanceStatus::InstanceUpdated)
        );
    }
}
