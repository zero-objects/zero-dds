// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data access, subscribing side, in push mode (DDS4CCM 1.1 §7.2.2.1.2,
//! §7.2.2.1.3): the fragment that calls the `Listener` or `StateListener`
//! the component provides, and the listener controls behind
//! `uses DataListenerControl data_control` and
//! `uses StateListenerControl data_control`.
//!
//! The delivery is a get in push mode ("similar to the one of
//! Getter::get_one, except that it is in push mode"): it reads the fresh
//! data of the DataReader of the port, through the query of its Reader,
//! when DDS reports data, when the mode is set to an enabled one and when
//! a listener is connected. One thread delivers at a time; a trigger that
//! finds a delivery running leaves its data to it.
//!
//! The delivery runs where the threading policy of the port puts its
//! facets (§7.4.4, [`crate::threading`]): on the thread of the trigger, or
//! on the thread pool of the port, the component or the container.
//!
//! A `Listener` gets every sample with its status. A `StateListener` gets
//! an instance that leaves or reenters the query of the Reader
//! (`INSTANCE_FILTERED_OUT`, `INSTANCE_FILTERED_IN`) as a deletion and a
//! creation or as updates, by `is_filter_interpreted` of its control
//! (`state_event`).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use zerodds_dcps::{DdsError, DdsType, InstanceHandle};

use crate::ccm_dds::{
    DataListenerControl, DataNumber, InstanceStatus, InternalError, ListenerMode, ReadInfo,
    StateListenerControl, UNLIMITED,
};
use crate::ccm_dds_typed::{Listener, StateListener};
use crate::reader_core::{ReaderCore, lock};
use crate::receptacle::Receptacle;
use crate::threading::Facets;

/// Which operation of a `StateListener` a sample triggers (§7.2.2.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateEvent {
    Creation,
    Update,
    Deletion,
}

/// §7.2.2.1.2 and §7.2.2.1.3: `on_creation` "if the instance is considered
/// as new in the component scope", `on_deletion` "if the instance is no
/// more existing", an update "if neither on_creation nor on_deletion
/// apply". With `is_filter_interpreted` an instance that enters the filter
/// is a creation and one that leaves it a deletion; without it "those
/// events should be considered as normal instance updates".
///
/// `withdrawn`: the listener was told the deletion of the instance when it
/// left the query. Then the instance does not exist for the listener: its
/// disposal is no second deletion (`None`), and its reentry is a creation,
/// also when `is_filter_interpreted` was set to FALSE in between.
pub(crate) fn state_event(
    status: InstanceStatus,
    is_filter_interpreted: bool,
    withdrawn: bool,
) -> Option<StateEvent> {
    Some(match status {
        InstanceStatus::InstanceCreated => StateEvent::Creation,
        InstanceStatus::InstanceDeleted if withdrawn => return None,
        InstanceStatus::InstanceDeleted => StateEvent::Deletion,
        InstanceStatus::InstanceFilteredIn if is_filter_interpreted || withdrawn => {
            StateEvent::Creation
        }
        InstanceStatus::InstanceFilteredOut if is_filter_interpreted => StateEvent::Deletion,
        InstanceStatus::InstanceUpdated
        | InstanceStatus::InstanceFilteredIn
        | InstanceStatus::InstanceFilteredOut => StateEvent::Update,
    })
}

/// Calls `listener` for the samples of one read, in their order: a
/// creation or a deletion by itself, the updates between them one by one
/// or as one group, by `mode`. `withdrawn` holds the instances whose exit
/// from the query the listener was told as a deletion ([`state_event`]).
pub(crate) fn call_state_listener<T>(
    listener: &dyn StateListener<T>,
    mode: ListenerMode,
    is_filter_interpreted: bool,
    withdrawn: &mut BTreeSet<InstanceHandle>,
    data: &mut Vec<T>,
    infos: &mut Vec<ReadInfo>,
) {
    let mut updates: Vec<T> = Vec::new();
    let mut update_infos: Vec<ReadInfo> = Vec::new();
    let flush = |updates: &mut Vec<T>, update_infos: &mut Vec<ReadInfo>| {
        if !updates.is_empty() {
            listener.on_many_updates(updates, update_infos);
            updates.clear();
            update_infos.clear();
        }
    };
    for (datum, info) in data.drain(..).zip(infos.drain(..)) {
        // Whatever comes of an instance ends its withdrawal; only an exit
        // told as a deletion begins one.
        let was_withdrawn = withdrawn.remove(&info.instance_handle);
        match state_event(info.instance_status, is_filter_interpreted, was_withdrawn) {
            Some(StateEvent::Creation) => {
                flush(&mut updates, &mut update_infos);
                listener.on_creation(&datum, &info);
            }
            Some(StateEvent::Deletion) => {
                flush(&mut updates, &mut update_infos);
                if info.instance_status == InstanceStatus::InstanceFilteredOut {
                    withdrawn.insert(info.instance_handle);
                }
                listener.on_deletion(&datum, &info);
            }
            Some(StateEvent::Update) if mode == ListenerMode::ManyByMany => {
                updates.push(datum);
                update_infos.push(info);
            }
            Some(StateEvent::Update) => listener.on_one_update(&datum, &info),
            // The disposal of an instance the listener was told deleted.
            None => {}
        }
    }
    flush(&mut updates, &mut update_infos);
}

/// The push side of one port: the control attributes, the listener of the
/// component and the delivery.
pub(crate) struct Push<T: DdsType> {
    /// This push side, for a delivery that runs on a thread pool.
    me: Weak<Self>,
    /// The reader state of the port, once its configuration is complete.
    core: OnceLock<Arc<ReaderCore<T>>>,
    /// Where the facets of the port are called, set with `core`.
    facets: OnceLock<Facets>,
    /// A delivery is queued on the thread pool and has not started.
    queued: AtomicBool,
    mode: Mutex<ListenerMode>,
    max_delivered_data: AtomicU32,
    is_filter_interpreted: AtomicBool,
    /// The instances whose exit from the query the StateListener was told
    /// as a deletion. Only a delivery touches it.
    withdrawn: Mutex<BTreeSet<InstanceHandle>>,
    /// `provides Listener data_listener` of port `DDS_Listen`.
    pub(crate) data_listener: Receptacle<dyn Listener<T>>,
    /// `provides StateListener data_listener` of port `DDS_StateListen`.
    pub(crate) state_listener: Receptacle<dyn StateListener<T>>,
    delivering: AtomicBool,
    again: AtomicBool,
    /// The DDS error of the last delivery, `None` after one that read its
    /// data.
    failed: Mutex<Option<InternalError>>,
}

impl<T: DdsType + Send + 'static> Push<T> {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: Weak::clone(me),
            core: OnceLock::new(),
            facets: OnceLock::new(),
            queued: AtomicBool::new(false),
            mode: Mutex::new(ListenerMode::NotEnabled),
            max_delivered_data: AtomicU32::new(UNLIMITED),
            is_filter_interpreted: AtomicBool::new(false),
            withdrawn: Mutex::new(BTreeSet::new()),
            data_listener: Receptacle::new(),
            state_listener: Receptacle::new(),
            delivering: AtomicBool::new(false),
            again: AtomicBool::new(false),
            failed: Mutex::new(None),
        })
    }

    /// Binds the delivery to the reader of the port, with the place its
    /// facets are called at, and delivers what the reader holds. The
    /// second call of a port is refused by the port.
    pub(crate) fn bind(&self, core: Arc<ReaderCore<T>>, facets: Facets) {
        if self.core.set(core).is_ok() {
            // Not reached twice: `core` is set once.
            let _ = self.facets.set(facets);
            self.deliver();
        }
    }

    /// Delivers the fresh data of the reader to the listener: now, or on
    /// the thread pool of the port. See the module documentation.
    pub(crate) fn deliver(&self) {
        let Some(facets) = self.facets.get().filter(|f| f.pool().is_some()) else {
            self.deliver_now();
            return;
        };
        // One queued delivery reads what every trigger before its start
        // brought.
        if self.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(push) = self.me.upgrade() else {
            return;
        };
        facets.call(move || {
            push.queued.store(false, Ordering::Release);
            push.deliver_now();
        });
    }

    fn deliver_now(&self) {
        self.again.store(true, Ordering::Release);
        loop {
            if self
                .delivering
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            while self.again.swap(false, Ordering::AcqRel) {
                let result = self.deliver_fresh();
                self.note(result);
            }
            self.delivering.store(false, Ordering::Release);
            // A trigger between the last look and the release found the
            // delivery taken.
            if !self.again.load(Ordering::Acquire) {
                return;
            }
        }
    }

    /// Keeps the outcome of a delivery and reports a DDS error to the
    /// observability sink of the participant: a listener operation has no
    /// exception to carry it.
    fn note(&self, result: Result<(), DdsError>) {
        let error = result.err();
        *lock(&self.failed) = error.as_ref().map(|e| InternalError::from_dds(e, 0));
        let (Some(error), Some(core)) = (error, self.core.get()) else {
            return;
        };
        if let Some((runtime, _)) = core.reader().runtime_handle() {
            use zerodds_foundation::observability::{Component, Event, Level};
            runtime.config.observability.record(
                &Event::new(
                    Level::Warn,
                    Component::Dcps,
                    "dds4ccm.listener.delivery_failed",
                )
                .with_attr("topic", core.reader().topic().name())
                .with_attr("error", format!("{error:?}").as_str()),
            );
        }
    }

    fn deliver_fresh(&self) -> Result<(), DdsError> {
        let Some(core) = self.core.get() else {
            return Ok(());
        };
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        loop {
            // Read per call of the listener: a mode the listener sets in a
            // call holds for the next sample.
            let mode = *lock(&self.mode);
            let max = match mode {
                ListenerMode::NotEnabled => return Ok(()),
                ListenerMode::OneByOne => 1,
                ListenerMode::ManyByMany => self.max_delivered_data.load(Ordering::Acquire),
            };
            let data_listener = self.data_listener.get_connection();
            let state_listener = self.state_listener.get_connection();
            if data_listener.is_none() && state_listener.is_none() {
                // Nobody to call: the data stay fresh in the reader.
                return Ok(());
            }
            core.read_fresh(max, &mut data, &mut infos)?;
            if data.is_empty() {
                return Ok(());
            }
            if let Some(listener) = state_listener {
                let mut withdrawn = lock(&self.withdrawn);
                call_state_listener(
                    &*listener,
                    mode,
                    self.is_filter_interpreted.load(Ordering::Acquire),
                    &mut withdrawn,
                    &mut data,
                    &mut infos,
                );
                // An instance the reader purged is deleted by no sample.
                if withdrawn.len() > core.existing() {
                    withdrawn.retain(|instance| core.exists(*instance));
                }
            } else if let Some(listener) = data_listener {
                match mode {
                    ListenerMode::ManyByMany => listener.on_many_data(&data, &infos),
                    ListenerMode::OneByOne | ListenerMode::NotEnabled => {
                        for (datum, info) in data.iter().zip(infos.iter()) {
                            listener.on_one_data(datum, info);
                        }
                    }
                }
            }
        }
    }

    fn set_mode(&self, mode: ListenerMode) {
        *lock(&self.mode) = mode;
        if mode != ListenerMode::NotEnabled {
            self.deliver();
        }
    }
}

/// The fragment behind `uses DataListenerControl data_control` of port
/// `DDS_Listen`.
pub struct DataListenerControlFragment<T: DdsType> {
    push: Arc<Push<T>>,
}

/// The fragment behind `uses StateListenerControl data_control` of port
/// `DDS_StateListen`.
pub struct StateListenerControlFragment<T: DdsType> {
    push: Arc<Push<T>>,
}

impl<T: DdsType + Send + 'static> DataListenerControlFragment<T> {
    pub(crate) fn new(push: Arc<Push<T>>) -> Self {
        Self { push }
    }

    /// The DDS error the last delivery to the listener ran into, `None`
    /// after a delivery that read its data. The operations of a listener
    /// raise no exception; the error is also reported as the event
    /// `dds4ccm.listener.delivery_failed` of the participant's
    /// observability sink.
    #[must_use]
    pub fn delivery_error(&self) -> Option<InternalError> {
        *lock(&self.push.failed)
    }
}

impl<T: DdsType + Send + 'static> StateListenerControlFragment<T> {
    pub(crate) fn new(push: Arc<Push<T>>) -> Self {
        Self { push }
    }

    /// As [`DataListenerControlFragment::delivery_error`].
    #[must_use]
    pub fn delivery_error(&self) -> Option<InternalError> {
        *lock(&self.push.failed)
    }
}

impl<T: DdsType + Send + Sync + 'static> DataListenerControl for DataListenerControlFragment<T> {
    fn mode(&self) -> ListenerMode {
        *lock(&self.push.mode)
    }

    fn set_mode(&self, mode: ListenerMode) {
        self.push.set_mode(mode);
    }

    fn max_delivered_data(&self) -> DataNumber {
        self.push.max_delivered_data.load(Ordering::Acquire)
    }

    fn set_max_delivered_data(&self, value: DataNumber) {
        self.push.max_delivered_data.store(value, Ordering::Release);
    }
}

impl<T: DdsType + Send + Sync + 'static> DataListenerControl for StateListenerControlFragment<T> {
    fn mode(&self) -> ListenerMode {
        *lock(&self.push.mode)
    }

    fn set_mode(&self, mode: ListenerMode) {
        self.push.set_mode(mode);
    }

    fn max_delivered_data(&self) -> DataNumber {
        self.push.max_delivered_data.load(Ordering::Acquire)
    }

    fn set_max_delivered_data(&self, value: DataNumber) {
        self.push.max_delivered_data.store(value, Ordering::Release);
    }
}

impl<T: DdsType + Send + Sync + 'static> StateListenerControl for StateListenerControlFragment<T> {
    fn is_filter_interpreted(&self) -> bool {
        self.push.is_filter_interpreted.load(Ordering::Acquire)
    }

    fn set_is_filter_interpreted(&self, value: bool) {
        self.push
            .is_filter_interpreted
            .store(value, Ordering::Release);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// What a StateListener was called with: the operation and the data.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<(&'static str, Vec<u32>)>>,
    }

    impl StateListener<u32> for Recorder {
        fn on_creation(&self, datum: &u32, _info: &ReadInfo) {
            lock(&self.calls).push(("creation", vec![*datum]));
        }
        fn on_one_update(&self, datum: &u32, _info: &ReadInfo) {
            lock(&self.calls).push(("one_update", vec![*datum]));
        }
        fn on_many_updates(&self, data: &[u32], infos: &[ReadInfo]) {
            assert_eq!(data.len(), infos.len());
            lock(&self.calls).push(("many_updates", data.to_vec()));
        }
        fn on_deletion(&self, datum: &u32, _info: &ReadInfo) {
            lock(&self.calls).push(("deletion", vec![*datum]));
        }
    }

    fn info(status: InstanceStatus) -> ReadInfo {
        ReadInfo {
            instance_handle: InstanceHandle::from_raw(1),
            instance_status: status,
            ..ReadInfo::default()
        }
    }

    /// One read of samples of instance 1, with the instances `withdrawn`
    /// holds from the reads before.
    fn run_with(
        withdrawn: &mut BTreeSet<InstanceHandle>,
        mode: ListenerMode,
        interpreted: bool,
        samples: &[(u32, InstanceStatus)],
    ) -> Vec<(&'static str, Vec<u32>)> {
        let recorder = Recorder::default();
        let mut data: Vec<u32> = samples.iter().map(|(d, _)| *d).collect();
        let mut infos: Vec<ReadInfo> = samples.iter().map(|(_, s)| info(*s)).collect();
        call_state_listener(
            &recorder,
            mode,
            interpreted,
            withdrawn,
            &mut data,
            &mut infos,
        );
        assert!(data.is_empty() && infos.is_empty());
        recorder.calls.into_inner().unwrap()
    }

    fn run(
        mode: ListenerMode,
        interpreted: bool,
        samples: &[(u32, InstanceStatus)],
    ) -> Vec<(&'static str, Vec<u32>)> {
        run_with(&mut BTreeSet::new(), mode, interpreted, samples)
    }

    /// §7.2.2.1.3: with `is_filter_interpreted` an instance entering the
    /// filter is a creation and one leaving it a deletion; without it both
    /// are updates.
    #[test]
    fn dds4ccm_7_2_2_1_3_is_filter_interpreted_decides_what_a_filtered_instance_triggers() {
        use InstanceStatus::{
            InstanceCreated, InstanceDeleted, InstanceFilteredIn, InstanceFilteredOut,
            InstanceUpdated,
        };
        for interpreted in [false, true] {
            assert_eq!(
                state_event(InstanceCreated, interpreted, false),
                Some(StateEvent::Creation)
            );
            assert_eq!(
                state_event(InstanceUpdated, interpreted, false),
                Some(StateEvent::Update)
            );
            assert_eq!(
                state_event(InstanceDeleted, interpreted, false),
                Some(StateEvent::Deletion)
            );
        }
        assert_eq!(
            state_event(InstanceFilteredIn, true, false),
            Some(StateEvent::Creation)
        );
        assert_eq!(
            state_event(InstanceFilteredOut, true, false),
            Some(StateEvent::Deletion)
        );
        assert_eq!(
            state_event(InstanceFilteredIn, false, false),
            Some(StateEvent::Update)
        );
        assert_eq!(
            state_event(InstanceFilteredOut, false, false),
            Some(StateEvent::Update)
        );

        let samples = [(1, InstanceFilteredIn), (2, InstanceFilteredOut)];
        assert_eq!(
            run(ListenerMode::OneByOne, true, &samples),
            [("creation", vec![1]), ("deletion", vec![2])]
        );
        assert_eq!(
            run(ListenerMode::OneByOne, false, &samples),
            [("one_update", vec![1]), ("one_update", vec![2])]
        );
        assert_eq!(
            run(ListenerMode::ManyByMany, false, &samples),
            [("many_updates", vec![1, 2])]
        );
    }

    /// §7.2.2.1.3: an exit from the query that was told as a deletion is
    /// the deletion of the instance for the listener. Its disposal outside
    /// the query is not a second one, and whatever comes of the instance
    /// next is a creation.
    #[test]
    fn dds4ccm_7_2_2_1_3_an_instance_told_deleted_at_its_exit_is_not_deleted_twice() {
        use InstanceStatus::{
            InstanceCreated, InstanceDeleted, InstanceFilteredIn, InstanceFilteredOut,
            InstanceUpdated,
        };
        let one = ListenerMode::OneByOne;
        let handle = InstanceHandle::from_raw(1);

        // Interpreted: the exit is the deletion, the disposal is silent.
        let mut withdrawn = BTreeSet::new();
        assert_eq!(
            run_with(
                &mut withdrawn,
                one,
                true,
                &[(1, InstanceCreated), (2, InstanceFilteredOut)]
            ),
            [("creation", vec![1]), ("deletion", vec![2])]
        );
        assert!(withdrawn.contains(&handle));
        assert!(run_with(&mut withdrawn, one, true, &[(0, InstanceDeleted)]).is_empty());
        assert!(withdrawn.is_empty());
        // The disposal was consumed: the next one is a deletion again.
        assert_eq!(
            run_with(
                &mut withdrawn,
                one,
                true,
                &[(3, InstanceCreated), (0, InstanceDeleted)]
            ),
            [("creation", vec![3]), ("deletion", vec![0])]
        );

        // Not interpreted: the exit is an update, the instance exists for
        // the listener, and its disposal is its deletion.
        let mut withdrawn = BTreeSet::new();
        assert_eq!(
            run_with(
                &mut withdrawn,
                one,
                false,
                &[
                    (1, InstanceCreated),
                    (2, InstanceFilteredOut),
                    (0, InstanceDeleted)
                ]
            ),
            [
                ("creation", vec![1]),
                ("one_update", vec![2]),
                ("deletion", vec![0])
            ]
        );
        assert!(withdrawn.is_empty());

        // Told deleted with the attribute TRUE, reentering with it FALSE:
        // a creation, for the listener holds no such instance. Then the
        // instance is inside and an update is an update.
        let mut withdrawn = BTreeSet::new();
        assert_eq!(
            run_with(&mut withdrawn, one, true, &[(2, InstanceFilteredOut)]),
            [("deletion", vec![2])]
        );
        assert_eq!(
            run_with(
                &mut withdrawn,
                one,
                false,
                &[(3, InstanceFilteredIn), (4, InstanceUpdated)]
            ),
            [("creation", vec![3]), ("one_update", vec![4])]
        );
        assert!(withdrawn.is_empty());
        assert_eq!(
            state_event(InstanceDeleted, false, true),
            None,
            "told deleted before the attribute changed"
        );
        assert_eq!(
            state_event(InstanceFilteredIn, false, true),
            Some(StateEvent::Creation)
        );
    }

    /// §7.2.2.1.2: on_one_update in mode ONE_BY_ONE, on_many_updates in
    /// mode MANY_BY_MANY; a creation or a deletion is its own call and
    /// keeps its place among the updates.
    #[test]
    fn dds4ccm_7_2_2_1_2_state_listener_groups_the_updates_by_the_mode() {
        use InstanceStatus::{InstanceCreated, InstanceDeleted, InstanceUpdated};
        let samples = [
            (1, InstanceCreated),
            (2, InstanceUpdated),
            (3, InstanceUpdated),
            (4, InstanceDeleted),
            (5, InstanceUpdated),
        ];
        assert_eq!(
            run(ListenerMode::OneByOne, false, &samples),
            [
                ("creation", vec![1]),
                ("one_update", vec![2]),
                ("one_update", vec![3]),
                ("deletion", vec![4]),
                ("one_update", vec![5]),
            ]
        );
        assert_eq!(
            run(ListenerMode::ManyByMany, false, &samples),
            [
                ("creation", vec![1]),
                ("many_updates", vec![2, 3]),
                ("deletion", vec![4]),
                ("many_updates", vec![5]),
            ]
        );
    }
}
