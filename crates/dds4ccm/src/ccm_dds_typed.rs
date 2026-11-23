// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The template module `CCM_DDS::Typed<typename T, sequence<T> TSeq>`
//! (DDS4CCM 1.1 §7.2.2.1, Annex A): the basic port interfaces that depend on
//! the data type. `TSeq` is `[T]` as a parameter and `Vec<T>` as a result.
//!
//! `in` parameters are references, `inout` and `out` parameters `&mut`
//! (§7.2.1.3: "Sequences to be returned … are designed as 'inout'
//! parameters … This disposal allows for implementation of smarter memory
//! management": an operation replaces the content of the sequences it is
//! given and keeps their allocation).

use zerodds_dcps::{Duration, InstanceHandle};

use crate::ccm_dds::{
    DataNumber, InternalError, QueryFilter, ReadInfo, ReadInfoSeq, ReaderError, UpdaterError,
};

/// Interface `InstanceHandleManager`: the two operations on DDS instance
/// handles, the base of [`Writer`] and [`Updater`].
pub trait InstanceHandleManager<T>: Send + Sync {
    /// Asks DDS to register the instance of the key of `datum` and returns
    /// its handle.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned.
    fn register_instance(&self, datum: &T) -> Result<InstanceHandle, InternalError>;

    /// Asks DDS to unregister the instance `instance_handle` names, or the
    /// one of the key of `datum` for `HANDLE_NIL`.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned.
    fn unregister_instance(
        &self,
        datum: &T,
        instance_handle: InstanceHandle,
    ) -> Result<(), InternalError>;
}

/// Interface `Writer`: publication without regard to the instance
/// lifecycle (§7.2.2.1.1).
pub trait Writer<T>: InstanceHandleManager<T> {
    /// Publishes one value. The instance is the one of `instance_handle`
    /// if it is not `HANDLE_NIL`, else the one of the key of `datum`.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned, also for a handle that
    /// is not in accordance with the key of `datum`.
    fn write_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), InternalError>;

    /// Publishes the values of `data` in one operation. The DDS orders stop
    /// at the first error. With `is_coherent_write` they are between
    /// `begin_coherent_changes` and `end_coherent_changes`, also when an
    /// error occurs.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned and the index of the
    /// erroneous value.
    fn write_many(&self, data: &[T]) -> Result<(), InternalError>;

    /// Attribute `is_coherent_write`; `false` by default.
    fn is_coherent_write(&self) -> bool;
    /// Sets `is_coherent_write`.
    fn set_is_coherent_write(&self, value: bool);
}

/// Interface `Updater`: publication with the instance lifecycle
/// (§7.2.2.1.1). Every operation checks the lifecycle of all submitted
/// instances before the first DDS order: locally to the component, or
/// globally to the data space with `is_global_scope`.
///
/// `create_one` has the `instance_handle` of Annex A; §7.2.2.1.1 lists the
/// operation without it ("this instance is designated by the key value in
/// datum"), which is the call with `HANDLE_NIL`.
pub trait Updater<T>: InstanceHandleManager<T> {
    /// Creates one instance.
    ///
    /// # Errors
    /// `AlreadyCreated` with index 0 if the instance exists; `InternalError`
    /// with the code DDS returned.
    fn create_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), UpdaterError>;

    /// Updates one instance.
    ///
    /// # Errors
    /// `NonExistent` with index 0 if the instance does not exist;
    /// `InternalError` with the code DDS returned.
    fn update_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), UpdaterError>;

    /// Deletes one instance: DDS disposes it.
    ///
    /// # Errors
    /// As [`Self::update_one`].
    fn delete_one(&self, datum: &T, instance_handle: InstanceHandle) -> Result<(), UpdaterError>;

    /// Creates the instances of `data` in one call.
    ///
    /// # Errors
    /// `AlreadyCreated` with the indexes of every instance that exists, and
    /// nothing is written; `InternalError` with the code DDS returned and
    /// the index of the erroneous value, where the orders stopped.
    fn create_many(&self, data: &[T]) -> Result<(), UpdaterError>;

    /// Updates the instances of `data` in one call.
    ///
    /// # Errors
    /// `NonExistent` with the indexes of every instance that does not
    /// exist, and nothing is written; `InternalError` as
    /// [`Self::create_many`].
    fn update_many(&self, data: &[T]) -> Result<(), UpdaterError>;

    /// Deletes the instances of `data` in one call.
    ///
    /// # Errors
    /// As [`Self::update_many`].
    fn delete_many(&self, data: &[T]) -> Result<(), UpdaterError>;

    /// Attribute `is_global_scope` (readonly); `false` by default.
    fn is_global_scope(&self) -> bool;
    /// Attribute `is_coherent_write`; `false` by default. With TRUE the
    /// write and dispose orders of a `*_many` operation are between
    /// `begin_coherent_changes` and `end_coherent_changes`, also when an
    /// error occurs; a `*_one` operation is one order and opens no set.
    fn is_coherent_write(&self) -> bool;
    /// Sets `is_coherent_write`.
    fn set_is_coherent_write(&self, value: bool);
}

/// Interface `Reader`: reads the values DDS holds, without waiting
/// (§7.2.2.1.2). The DDS reads select `READ` or `NOT_READ`, `NEW` or
/// `NOT_NEW`, `ALIVE`, through the query.
pub trait Reader<T>: Send + Sync {
    /// The last sample of every instance. Without data the sequences are
    /// empty.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned.
    fn read_last(&self, data: &mut Vec<T>, infos: &mut ReadInfoSeq) -> Result<(), InternalError>;

    /// All samples of all instances, ordered by instance. Without data the
    /// sequences are empty.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned.
    fn read_all(&self, data: &mut Vec<T>, infos: &mut ReadInfoSeq) -> Result<(), InternalError>;

    /// The last sample of one instance: the one of `instance_handle` if it
    /// is not `HANDLE_NIL`, else the one of the key of `datum`. Of a topic
    /// without a key it is the last value of the topic.
    ///
    /// # Errors
    /// `NonExistent` with index 0 if DDS holds no data of the instance;
    /// `InternalError` with the code DDS returned, also for a handle that
    /// is not in accordance with the key of `datum`.
    fn read_one_last(
        &self,
        datum: &mut T,
        info: &mut ReadInfo,
        instance_handle: InstanceHandle,
    ) -> Result<(), ReaderError>;

    /// All samples of one instance, designated as for
    /// [`Self::read_one_last`].
    ///
    /// # Errors
    /// As [`Self::read_one_last`].
    fn read_one_all(
        &self,
        datum: &T,
        data: &mut Vec<T>,
        infos: &mut ReadInfoSeq,
        instance_handle: InstanceHandle,
    ) -> Result<(), ReaderError>;

    /// Attribute `query`: the filter of every read through the port.
    fn query(&self) -> QueryFilter;

    /// Sets `query`. An empty expression means no query.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned for an expression or
    /// parameters DDS does not accept; the attribute keeps its value.
    fn set_query(&self, query: QueryFilter) -> Result<(), InternalError>;
}

/// Interface `Getter`: gets fresh values in pull mode, waiting for them
/// (§7.2.2.1.2). The DDS reads select `NOT_READ`, `NEW` or `NOT_NEW`,
/// `ALIVE` or `NOT_ALIVE`, through the query of the Reader of the port,
/// within `time_out`.
pub trait Getter<T>: Send + Sync {
    /// The next sample. Returns `false` when `time_out` passed without one;
    /// `datum` and `info` are untouched then.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned.
    fn get_one(&self, datum: &mut T, info: &mut ReadInfo) -> Result<bool, InternalError>;

    /// The available samples, at most `max_delivered_data`; the others stay
    /// for the next call. Returns `false` when `time_out` passed without
    /// one.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned.
    fn get_many(&self, data: &mut Vec<T>, infos: &mut ReadInfoSeq) -> Result<bool, InternalError>;

    /// Attribute `time_out`. The IDL names no default; it is the zero
    /// duration, with which a get does not wait.
    fn time_out(&self) -> Duration;
    /// Sets `time_out`; `DURATION_INFINITE` waits without limit.
    fn set_time_out(&self, value: Duration);
    /// Attribute `max_delivered_data`; `UNLIMITED` by default.
    fn max_delivered_data(&self) -> DataNumber;
    /// Sets `max_delivered_data`.
    fn set_max_delivered_data(&self, value: DataNumber);
}

/// Interface `Listener`: gets fresh values in push mode, whatever the
/// instance status (§7.2.2.1.2). The component provides it; the fragment
/// calls it by the mode of the `DataListenerControl` of the port.
pub trait Listener<T>: Send + Sync {
    /// One sample (mode `ONE_BY_ONE`).
    fn on_one_data(&self, datum: &T, info: &ReadInfo);
    /// A group of samples, at most `max_delivered_data` (mode
    /// `MANY_BY_MANY`).
    fn on_many_data(&self, data: &[T], infos: &[ReadInfo]);
}

/// Interface `StateListener`: gets fresh values in push mode, by the
/// instance status (§7.2.2.1.2).
pub trait StateListener<T>: Send + Sync {
    /// The instance is new for the component.
    fn on_creation(&self, datum: &T, info: &ReadInfo);
    /// The instance changed (mode `ONE_BY_ONE`).
    fn on_one_update(&self, datum: &T, info: &ReadInfo);
    /// Instances changed (mode `MANY_BY_MANY`), at most
    /// `max_delivered_data` samples.
    fn on_many_updates(&self, data: &[T], infos: &[ReadInfo]);
    /// The instance stopped existing. Only the key of `datum` is valid.
    fn on_deletion(&self, datum: &T, info: &ReadInfo);
}
