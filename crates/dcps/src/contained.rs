// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DataWriters a Publisher created and the DataReaders a Subscriber
//! created, as their factory holds them (DDS 1.4 §2.2.2.4.1.6, .7 and .14;
//! §2.2.2.5.2.6, .7, .10, .11 and .14).
//!
//! A factory holds its endpoints weakly. The application's handles keep an
//! endpoint alive; while it is, its factory finds it for `lookup_*`,
//! `delete_contained_entities` and the participant's `contains_entity`. A
//! deleted endpoint, or one whose last handle is gone, is not found.

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::error::Result;
use crate::instance_handle::InstanceHandle;

/// An endpoint as its factory sees it.
pub(crate) trait Contained: Send + Sync {
    /// The endpoint's `InstanceHandle` (DDS 1.4 §2.2.2.1.1.9).
    fn handle(&self) -> InstanceHandle;
    /// The name of the topic the endpoint is attached to.
    fn topic_name(&self) -> &str;
    /// `true` once the endpoint is deleted.
    fn is_deleted(&self) -> bool;
    /// Deletes the endpoint. A deleted endpoint stays deleted; deleting it
    /// again does nothing.
    fn delete(&self) -> Result<()>;
    /// The endpoint as `Any`, for the typed handle `lookup_*` returns.
    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync>;
}

/// A sample a reader holds, as `get_datareaders` orders readers by it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HeldSample {
    /// The sample's source timestamp.
    pub time: crate::time::Time,
    /// Its group sequence number and group, when the writer belongs to a
    /// GROUP-scope publisher (DDSI-RTPS 2.5 §8.7.5).
    pub group: Option<zerodds_rtps::writer_group::ChangeGroupInfo>,
}

impl HeldSample {
    /// `true` if `self` was written before `other` (DDS 1.4 §2.2.2.5.2.10,
    /// "the order in which the samples were written"): by group sequence
    /// number where both come from the same group, since a source timestamp
    /// of two writers is not their write order, and otherwise by source
    /// timestamp.
    pub(crate) fn written_before(&self, other: &Self) -> bool {
        match (&self.group, &other.group) {
            (Some(a), Some(b)) if a.writer_set == b.writer_set => a.gsn < b.gsn,
            _ => self.time < other.time,
        }
    }
}

/// A DataReader as its Subscriber sees it.
pub(crate) trait ContainedReader: Contained {
    /// `true` while the reader holds a ReadCondition or QueryCondition it
    /// created; `delete_datareader` refuses such a reader (DDS 1.4
    /// §2.2.2.5.2.6).
    fn has_conditions(&self) -> bool;
    /// Deletes the reader's ReadConditions and QueryConditions
    /// (§2.2.2.5.3, `delete_contained_entities`).
    fn delete_conditions(&self) -> Result<()>;
    /// The samples the reader holds in the three state masks, in the order
    /// the reader presents them; empty when it holds none (§2.2.2.5.2.10
    /// `get_datareaders`).
    fn matching_samples(
        self: Arc<Self>,
        sample_states: u32,
        view_states: u32,
        instance_states: u32,
    ) -> Result<Vec<HeldSample>>;
    /// Calls `on_data_available` on the reader's listener if DATA_AVAILABLE
    /// changed and no `on_data_available` was called for the change
    /// (§2.2.2.5.2.11 `notify_datareaders`).
    fn notify_data_available(&self);
    /// Puts `item`, which the subscriber's group order committed, into the
    /// reader's inbox (DDSI-RTPS 2.5 §8.7.5).
    fn accept_group_released(&self, item: crate::runtime::UserSample);
    /// Hands what waits on the reader's channel to the subscriber's group
    /// order, unless the reader is draining it itself right now; what the
    /// order does not take goes to the inbox through the reader's
    /// coherent-set gate.
    fn pump_group_channel(self: Arc<Self>);
    /// The reader's user-reader `EntityId` in the runtime; `None` offline.
    fn entity_id(&self) -> Option<zerodds_rtps::wire_types::EntityId>;
    /// Whether the reader has something its Subscriber's DATA_ON_READERS
    /// did not cover (DDS 1.4 §2.2.4.2.2).
    fn has_data_on_readers(&self) -> bool;
    /// The Subscriber's DATA_ON_READERS was reset: what the reader has now
    /// is covered.
    fn cover_data_on_readers(&self);
    /// The reader's SAMPLE_LOST status with the change since the last look,
    /// which this look resets (§2.2.2.5.2.12 `get_sample_lost_status` of the
    /// Subscriber sums it over its readers).
    fn take_sample_lost_status(&self) -> Result<crate::status::SampleLostStatus>;
}

/// A DataWriter as its Publisher sees it.
pub(crate) trait ContainedWriter: Contained {
    /// Waits until the reliable readers of the writer have acknowledged
    /// everything it wrote, or `deadline` passes; `true` when they have. A
    /// writer without a runtime, or a BEST_EFFORT one, has nothing to wait
    /// for (§2.2.2.4.1.12 `wait_for_acknowledgments` of the Publisher).
    fn await_acknowledged(&self, deadline: std::time::Instant) -> bool;
}

/// The endpoints of one Publisher or Subscriber, in creation order.
pub(crate) struct ContainedList<E: ?Sized> {
    entries: Mutex<Vec<(InstanceHandle, Weak<E>)>>,
}

impl<E: ?Sized> core::fmt::Debug for ContainedList<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ContainedList")
            .field("entries", &self.lock().len())
            .finish()
    }
}

impl<E: ?Sized> ContainedList<E> {
    pub(crate) fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// The list holds weak pointers only; a panic while it was locked left
    /// no entry half written, so a poisoned lock is taken over.
    fn lock(&self) -> MutexGuard<'_, Vec<(InstanceHandle, Weak<E>)>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn add(&self, handle: InstanceHandle, endpoint: Weak<E>) {
        self.lock().push((handle, endpoint));
    }

    pub(crate) fn remove(&self, handle: InstanceHandle) {
        self.lock().retain(|(h, _)| *h != handle);
    }
}

impl<E: ?Sized + Contained> ContainedList<E> {
    /// The endpoints that are alive and not deleted, in creation order. An
    /// entry whose endpoint is gone is dropped from the list.
    pub(crate) fn live(&self) -> Vec<Arc<E>> {
        let mut entries = self.lock();
        entries.retain(|(_, weak)| weak.strong_count() > 0);
        entries
            .iter()
            .filter_map(|(_, weak)| weak.upgrade())
            .filter(|endpoint| !endpoint.is_deleted())
            .collect()
    }

    /// `true` if `handle` names a live endpoint of the list.
    pub(crate) fn contains(&self, handle: InstanceHandle) -> bool {
        self.live()
            .iter()
            .any(|endpoint| endpoint.handle() == handle)
    }

    /// `true` if the list holds no live endpoint.
    pub(crate) fn is_empty(&self) -> bool {
        self.live().is_empty()
    }

    /// The first live endpoint attached to `topic_name` that is a `T`. DDS
    /// 1.4 §2.2.2.4.1.7 and §2.2.2.5.2.7 leave open which one of several;
    /// this is the oldest.
    pub(crate) fn find<T: Send + Sync + 'static>(&self, topic_name: &str) -> Option<Arc<T>> {
        self.live()
            .into_iter()
            .filter(|endpoint| endpoint.topic_name() == topic_name)
            .find_map(|endpoint| endpoint.into_any().downcast::<T>().ok())
    }

    /// Deletes every live endpoint, in creation order.
    pub(crate) fn delete_all(&self) -> Result<()> {
        for endpoint in self.live() {
            endpoint.delete()?;
        }
        Ok(())
    }
}
