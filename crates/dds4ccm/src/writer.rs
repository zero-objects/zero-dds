// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data access, publishing side (DDS4CCM 1.1 §7.2.2.1.1): the connector
//! fragments behind interface `Writer` and interface `Updater`, each over
//! the DataWriter of its port.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use zerodds_dcps::error::return_code;
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::{
    DataReader, DataWriter, DdsError, DdsType, InstanceHandle, InstanceState, ReadSelection, Time,
    get_current_time,
};

use crate::ccm_dds::{AlreadyCreated, DataNumber, InternalError, NonExistent, UpdaterError};
use crate::ccm_dds_typed::{InstanceHandleManager, Updater, Writer};
use crate::reader_core::lock;

/// The index of a datum in a submitted sequence as an exception carries
/// it. A sequence longer than `DataNumber_t` counts cannot be submitted
/// through the IDL; the last index stands for everything behind it.
fn index_of(position: usize) -> DataNumber {
    DataNumber::try_from(position).unwrap_or(DataNumber::MAX)
}

/// Runs `each` for the positions `0..count`, stopping at the first error
/// (§7.2.2.1.1: "Resulting DDS orders are stopped at the first error (and
/// the index of the erroneous instance value is reported in the raised
/// InternalError exception)"). With `coherent` the orders that succeeded
/// are between `begin_coherent_changes` and `end_coherent_changes` of the
/// publisher of `writer`.
fn batch<T: DdsType>(
    writer: &DataWriter<T>,
    coherent: bool,
    count: usize,
    mut each: impl FnMut(usize) -> Result<(), DdsError>,
) -> Result<(), InternalError> {
    let publisher = writer.get_publisher();
    if coherent {
        publisher.begin_coherent_changes()?;
    }
    let mut failed = None;
    for position in 0..count {
        if let Err(error) = each(position) {
            failed = Some(InternalError::from_dds(&error, index_of(position)));
            break;
        }
    }
    if coherent {
        // The set is closed also after a failed order: what was written is
        // one set, and the publisher is left without an open one.
        let ended = publisher.end_coherent_changes();
        if failed.is_none() {
            ended?;
        }
    }
    failed.map_or(Ok(()), Err)
}

/// The fragment behind `uses Writer data` of port `DDS_Write`.
pub struct WriterFragment<T: DdsType> {
    writer: DataWriter<T>,
    coherent: AtomicBool,
}

impl<T: DdsType> WriterFragment<T> {
    /// A Writer over `writer`, the DataWriter dedicated to its port.
    #[must_use]
    pub fn new(writer: DataWriter<T>) -> Self {
        Self {
            writer,
            coherent: AtomicBool::new(false),
        }
    }

    /// The DataWriter of the port (`dds_entity`).
    #[must_use]
    pub fn dds_entity(&self) -> &DataWriter<T> {
        &self.writer
    }
}

impl<T: DdsType + Send + Sync + 'static> InstanceHandleManager<T> for WriterFragment<T> {
    fn register_instance(&self, datum: &T) -> Result<InstanceHandle, InternalError> {
        Ok(self.writer.register_instance(datum)?)
    }

    fn unregister_instance(
        &self,
        datum: &T,
        instance_handle: InstanceHandle,
    ) -> Result<(), InternalError> {
        Ok(self.writer.unregister_instance(datum, instance_handle)?)
    }
}

impl<T: DdsType + Send + Sync + 'static> Writer<T> for WriterFragment<T> {
    fn write_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), InternalError> {
        Ok(self.writer.write_with_handle(datum, instance_handle)?)
    }

    fn write_many(&self, data: &[T]) -> Result<(), InternalError> {
        batch(&self.writer, self.is_coherent_write(), data.len(), |at| {
            self.writer.write(&data[at])
        })
    }

    fn is_coherent_write(&self) -> bool {
        self.coherent.load(Ordering::Acquire)
    }

    fn set_is_coherent_write(&self, value: bool) {
        self.coherent.store(value, Ordering::Release);
    }
}

/// What an Updater knows of an instance from its own operations.
#[derive(Debug, Clone, Copy)]
struct Own {
    /// Created or updated last (`true`), or deleted last (`false`).
    exists: bool,
    /// When that operation returned, on the clock of the writer.
    at: Time,
}

/// The fragment behind `uses Updater data` of port `DDS_Update`.
///
/// The lifecycle checks are local or global (`is_global_scope`):
///
/// - local: an instance exists if this Updater created it and has not
///   deleted it since.
/// - global: an instance exists if it is `ALIVE` in the data space. The
///   fragment looks it up on a DataReader of the topic (the observer;
///   "Global checks may require an attempt to get the instance under the
///   scene"). Where the observer and this Updater's own operations
///   disagree, the later of the two decides: an operation of this Updater
///   the observer has not received yet counts, and so does a write or a
///   disposal of another participant that came after it.
pub struct UpdaterFragment<T: DdsType> {
    writer: DataWriter<T>,
    coherent: AtomicBool,
    observer: Option<DataReader<T>>,
    /// By the instance handle of the writer.
    own: Mutex<BTreeMap<InstanceHandle, Own>>,
}

/// How a lifecycle operation designates its instance.
enum Lifecycle {
    Create,
    Update,
    Delete,
}

impl<T: DdsType + Send + 'static> UpdaterFragment<T> {
    /// An Updater over `writer` whose lifecycle checks are local to the
    /// component (`is_global_scope` FALSE).
    #[must_use]
    pub fn new(writer: DataWriter<T>) -> Self {
        Self {
            writer,
            coherent: AtomicBool::new(false),
            observer: None,
            own: Mutex::new(BTreeMap::new()),
        }
    }

    /// An Updater over `writer` whose lifecycle checks are global to the
    /// data space (`is_global_scope` TRUE). `observer` is a DataReader of
    /// the topic of `writer` that the fragment keeps for itself.
    #[must_use]
    pub fn with_global_scope(writer: DataWriter<T>, observer: DataReader<T>) -> Self {
        Self {
            observer: Some(observer),
            ..Self::new(writer)
        }
    }

    /// The DataWriter of the port (`dds_entity`).
    #[must_use]
    pub fn dds_entity(&self) -> &DataWriter<T> {
        &self.writer
    }

    /// The DataReader the global checks look the instances up on.
    #[must_use]
    pub fn observer(&self) -> Option<&DataReader<T>> {
        self.observer.as_ref()
    }

    /// The clock the writer stamps its samples with.
    fn now(&self) -> Time {
        self.writer
            .runtime_handle()
            .map_or_else(get_current_time, |(runtime, _)| {
                runtime.config.time_source.now()
            })
    }

    /// §7.2.2.1.1: "If a valid handle is passed, it must be in accordance
    /// with the key value of the passed instance data; otherwise, an
    /// InternalError exception is raised with the returned DDS error code".
    /// The codes are those of the write of DDS 1.4 §2.2.2.4.2.11:
    /// `PRECONDITION_NOT_MET` for the handle of another instance,
    /// `BAD_PARAMETER` for a handle of no instance.
    fn check_handle(&self, datum: &T, handle: InstanceHandle) -> Result<(), InternalError> {
        if handle.is_nil() || self.writer.lookup_instance(datum) == handle {
            return Ok(());
        }
        let known = self
            .writer
            .instance_tracker()
            .get_by_handle(handle)
            .is_some();
        Err(InternalError::code(if known {
            return_code::PRECONDITION_NOT_MET
        } else {
            return_code::BAD_PARAMETER
        }))
    }

    /// Takes what the observer received into its instance states, without
    /// reading a sample: a read of at most 0 samples.
    fn refresh_observer(&self) -> Result<(), InternalError> {
        let Some(observer) = &self.observer else {
            return Ok(());
        };
        match observer.read_selected(&ReadSelection::limited(0)) {
            Ok(_) | Err(DdsError::NoData) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Whether the instance of `datum` exists, in the scope of this
    /// Updater.
    fn exists(&self, datum: &T, own: &BTreeMap<InstanceHandle, Own>) -> bool {
        let handle = self.writer.lookup_instance(datum);
        let own = own.get(&handle).copied().filter(|_| !handle.is_nil());
        let seen = self.observer.as_ref().and_then(|observer| {
            let handle = observer.lookup_instance(datum);
            observer.instance_tracker().get_by_handle(handle)
        });
        match (own, seen) {
            (None, None) => false,
            (Some(own), None) => own.exists,
            (None, Some(seen)) => seen.kind == InstanceStateKind::Alive,
            (Some(own), Some(seen)) => {
                if latest_change(&seen).is_some_and(|at| at > own.at) {
                    seen.kind == InstanceStateKind::Alive
                } else {
                    own.exists
                }
            }
        }
    }

    /// The lifecycle check of §7.2.2.1.1, "performed before any attempt
    /// ordering DDS to write and … applied to all the submitted
    /// instances": the positions of `data` whose instance is not in the
    /// state `operation` needs. A second datum of one instance in a create
    /// or a delete is erroneous as well: the first one leaves the instance
    /// in the state the second one must not find.
    fn erroneous(
        &self,
        operation: &Lifecycle,
        data: &[T],
        own: &BTreeMap<InstanceHandle, Own>,
    ) -> Vec<DataNumber> {
        let must_exist = !matches!(operation, Lifecycle::Create);
        let once = !matches!(operation, Lifecycle::Update);
        let mut seen_keys: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut indexes = Vec::new();
        for (position, datum) in data.iter().enumerate() {
            let repeated = once && !seen_keys.insert(key_of(datum));
            if repeated || self.exists(datum, own) != must_exist {
                indexes.push(index_of(position));
            }
        }
        indexes
    }

    /// One lifecycle operation over `data`: the check, then the orders.
    /// `many`: the operation is a `*_many` one, whose orders
    /// `is_coherent_write` puts into one coherent set (§7.2.2.1.1, Annex A:
    /// "DDS orders resulting from a *_many operation are placed between
    /// begin/end coherent updates"). A `*_one` operation is one order and
    /// opens no set.
    fn run(&self, operation: &Lifecycle, data: &[T], many: bool) -> Result<(), UpdaterError> {
        self.refresh_observer()?;
        let mut own = lock(&self.own);
        let indexes = self.erroneous(operation, data, &own);
        if !indexes.is_empty() {
            return Err(match operation {
                Lifecycle::Create => AlreadyCreated { indexes }.into(),
                Lifecycle::Update | Lifecycle::Delete => NonExistent { indexes }.into(),
            });
        }
        let coherent = many && self.coherent.load(Ordering::Acquire);
        let result = batch(&self.writer, coherent, data.len(), |at| {
            let datum = &data[at];
            let handle = match operation {
                Lifecycle::Create => {
                    let handle = self.writer.register_instance(datum)?;
                    self.writer.write_with_handle(datum, handle)?;
                    handle
                }
                Lifecycle::Update => {
                    self.writer.write(datum)?;
                    self.writer.lookup_instance(datum)
                }
                Lifecycle::Delete => {
                    // An instance another participant created (global
                    // scope) is not registered with this writer yet.
                    let handle = self.writer.register_instance(datum)?;
                    self.writer.dispose(datum, handle)?;
                    handle
                }
            };
            own.insert(
                handle,
                Own {
                    exists: !matches!(operation, Lifecycle::Delete),
                    at: self.now(),
                },
            );
            Ok(())
        });
        Ok(result?)
    }
}

/// The key of `datum` as the octets that identify its instance.
fn key_of<T: DdsType>(datum: &T) -> Vec<u8> {
    if !T::has_key() {
        return Vec::new();
    }
    let mut holder = zerodds_dcps::dds_type::PlainCdr2BeKeyHolder::new();
    datum.encode_key_holder_be(&mut holder);
    holder.as_bytes().to_vec()
}

/// When the observer last saw the instance change.
fn latest_change(state: &InstanceState) -> Option<Time> {
    [
        state.last_sample_timestamp,
        state.disposed_at,
        state.no_writers_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

impl<T: DdsType + Send + Sync + 'static> InstanceHandleManager<T> for UpdaterFragment<T> {
    fn register_instance(&self, datum: &T) -> Result<InstanceHandle, InternalError> {
        Ok(self.writer.register_instance(datum)?)
    }

    fn unregister_instance(
        &self,
        datum: &T,
        instance_handle: InstanceHandle,
    ) -> Result<(), InternalError> {
        let handle = if instance_handle.is_nil() {
            self.writer.lookup_instance(datum)
        } else {
            instance_handle
        };
        self.writer.unregister_instance(datum, instance_handle)?;
        // WRITER_DATA_LIFECYCLE (DDS 1.4 §2.2.3.21): with
        // `autodispose_unregistered_instances` the unregister disposes the
        // instance, which deletes it.
        if self
            .writer
            .qos()
            .writer_data_lifecycle
            .autodispose_unregistered_instances
        {
            let mut own = lock(&self.own);
            if let Some(known) = own.get_mut(&handle) {
                *known = Own {
                    exists: false,
                    at: self.now(),
                };
            }
        }
        Ok(())
    }
}

impl<T: DdsType + Send + Sync + 'static> Updater<T> for UpdaterFragment<T> {
    fn create_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), UpdaterError> {
        self.check_handle(datum, instance_handle)?;
        self.run(&Lifecycle::Create, std::slice::from_ref(datum), false)
    }

    fn update_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), UpdaterError> {
        self.check_handle(datum, instance_handle)?;
        self.run(&Lifecycle::Update, std::slice::from_ref(datum), false)
    }

    fn delete_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), UpdaterError> {
        self.check_handle(datum, instance_handle)?;
        self.run(&Lifecycle::Delete, std::slice::from_ref(datum), false)
    }

    fn create_many(&self, data: &[T]) -> Result<(), UpdaterError> {
        self.run(&Lifecycle::Create, data, true)
    }

    fn update_many(&self, data: &[T]) -> Result<(), UpdaterError> {
        self.run(&Lifecycle::Update, data, true)
    }

    fn delete_many(&self, data: &[T]) -> Result<(), UpdaterError> {
        self.run(&Lifecycle::Delete, data, true)
    }

    fn is_global_scope(&self) -> bool {
        self.observer.is_some()
    }

    fn is_coherent_write(&self) -> bool {
        self.coherent.load(Ordering::Acquire)
    }

    fn set_is_coherent_write(&self, value: bool) {
        self.coherent.store(value, Ordering::Release);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn dds4ccm_7_2_2_1_1_the_index_of_an_exception_is_the_position_in_the_sequence() {
        assert_eq!(index_of(0), 0);
        assert_eq!(index_of(41), 41);
    }

    #[test]
    fn dds4ccm_7_2_2_1_1_the_later_of_the_observer_s_changes_is_its_latest() {
        let tracker = zerodds_dcps::InstanceTracker::new();
        let handle = tracker.register([1; 16], Vec::new(), None);
        let state = |sample: Option<i32>, disposed: Option<i32>| InstanceState {
            last_sample_timestamp: sample.map(|sec| Time::new(sec, 0)),
            disposed_at: disposed.map(|sec| Time::new(sec, 0)),
            ..tracker.get_by_handle(handle).unwrap()
        };
        assert_eq!(latest_change(&state(None, None)), None);
        assert_eq!(latest_change(&state(Some(5), None)), Some(Time::new(5, 0)));
        assert_eq!(
            latest_change(&state(Some(5), Some(9))),
            Some(Time::new(9, 0))
        );
    }
}
