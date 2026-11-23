// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data access, subscribing side, in pull mode (DDS4CCM 1.1 §7.2.2.1.2):
//! the connector fragments behind interface `Reader` and interface
//! `Getter`, both over the DataReader of their port.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use zerodds_dcps::error::return_code;
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::{
    DataReader, DdsError, DdsType, Duration, InstanceHandle, ReadSelection, WaitSet,
};

use crate::ccm_dds::{
    DataNumber, InternalError, NonExistent, QueryFilter, ReadInfo, ReadInfoSeq, ReaderError,
    UNLIMITED,
};
use crate::ccm_dds_typed::{Getter, Reader};
use crate::reader_core::{ReaderCore, lock};

/// The fragment behind `uses Reader data` of the subscribing ports.
pub struct ReaderFragment<T: DdsType> {
    core: Arc<ReaderCore<T>>,
}

impl<T: DdsType + Send + 'static> ReaderFragment<T> {
    pub(crate) fn new(core: Arc<ReaderCore<T>>) -> Self {
        Self { core }
    }

    /// The DataReader of the port (`dds_entity`).
    #[must_use]
    pub fn dds_entity(&self) -> &DataReader<T> {
        self.core.reader()
    }

    /// The instance a `read_one` operation designates: the one of
    /// `instance_handle` if it is not `HANDLE_NIL`, else the one of the
    /// key of `datum` (§7.2.2.1.2). "If a valid handle is passed, it must
    /// be in accordance with the key value of the passed data; otherwise,
    /// an InternalError exception is raised with the returned DDS error
    /// code": `PRECONDITION_NOT_MET` for the handle of another instance,
    /// `BAD_PARAMETER` for a handle of no instance, as a DataWriter answers
    /// them (DDS 1.4 §2.2.2.4.2.11).
    fn designate(
        &self,
        datum: &T,
        instance_handle: InstanceHandle,
    ) -> Result<InstanceHandle, ReaderError> {
        let reader = self.core.reader();
        // What arrived becomes an instance of the reader with a read of at
        // most 0 samples; no sample changes its state.
        match reader.read_selected(&ReadSelection::limited(0)) {
            Ok(_) | Err(DdsError::NoData) => {}
            Err(error) => return Err(InternalError::from(error).into()),
        }
        let by_key = reader.lookup_instance(datum);
        if instance_handle.is_nil() {
            if by_key.is_nil() {
                return Err(NonExistent { indexes: vec![0] }.into());
            }
            return Ok(by_key);
        }
        if by_key == instance_handle {
            return Ok(instance_handle);
        }
        let known = reader
            .instance_tracker()
            .get_by_handle(instance_handle)
            .is_some();
        Err(InternalError::code(if known {
            return_code::PRECONDITION_NOT_MET
        } else {
            return_code::BAD_PARAMETER
        })
        .into())
    }

    /// Whether DDS holds the instance as `ALIVE`.
    fn is_alive(&self, handle: InstanceHandle) -> bool {
        self.core
            .reader()
            .instance_tracker()
            .get_by_handle(handle)
            .is_some_and(|state| state.kind == InstanceStateKind::Alive)
    }
}

impl<T: DdsType + Send + Sync + 'static> Reader<T> for ReaderFragment<T> {
    fn read_last(&self, data: &mut Vec<T>, infos: &mut ReadInfoSeq) -> Result<(), InternalError> {
        Ok(self.core.read_alive(None, true, data, infos)?)
    }

    fn read_all(&self, data: &mut Vec<T>, infos: &mut ReadInfoSeq) -> Result<(), InternalError> {
        Ok(self.core.read_alive(None, false, data, infos)?)
    }

    fn read_one_last(
        &self,
        datum: &mut T,
        info: &mut ReadInfo,
        instance_handle: InstanceHandle,
    ) -> Result<(), ReaderError> {
        let handle = self.designate(datum, instance_handle)?;
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        self.core
            .read_alive(Some(handle), true, &mut data, &mut infos)
            .map_err(InternalError::from)?;
        match (data.pop(), infos.pop()) {
            (Some(last), Some(last_info)) => {
                *datum = last;
                *info = last_info;
                Ok(())
            }
            // The instance lives and the query lets no sample of it
            // through: DDS has no data to return, which is its NO_DATA.
            _ if self.is_alive(handle) => Err(InternalError::code(return_code::NO_DATA).into()),
            _ => Err(NonExistent { indexes: vec![0] }.into()),
        }
    }

    fn read_one_all(
        &self,
        datum: &T,
        data: &mut Vec<T>,
        infos: &mut ReadInfoSeq,
        instance_handle: InstanceHandle,
    ) -> Result<(), ReaderError> {
        data.clear();
        infos.clear();
        let handle = self.designate(datum, instance_handle)?;
        self.core
            .read_alive(Some(handle), false, data, infos)
            .map_err(InternalError::from)?;
        if data.is_empty() && !self.is_alive(handle) {
            return Err(NonExistent { indexes: vec![0] }.into());
        }
        Ok(())
    }

    fn query(&self) -> QueryFilter {
        self.core.query()
    }

    fn set_query(&self, query: QueryFilter) -> Result<(), InternalError> {
        Ok(self.core.set_query(query)?)
    }
}

/// How long one wait of a get with `DURATION_INFINITE` lasts before it
/// looks again.
const INFINITE_WAIT_STEP: std::time::Duration = std::time::Duration::from_secs(3600);

/// The fragment behind `uses Getter fresh_data` of port `DDS_Get`.
pub struct GetterFragment<T: DdsType> {
    core: Arc<ReaderCore<T>>,
    time_out: Mutex<Duration>,
    max_delivered_data: AtomicU32,
}

impl<T: DdsType + Send + 'static> GetterFragment<T> {
    pub(crate) fn new(core: Arc<ReaderCore<T>>) -> Self {
        Self {
            core,
            time_out: Mutex::new(Duration::ZERO),
            max_delivered_data: AtomicU32::new(UNLIMITED),
        }
    }

    /// Fresh data, at most `max` samples, waiting for it within
    /// `time_out`. `false` when the time passed without any.
    fn get(
        &self,
        max: DataNumber,
        data: &mut Vec<T>,
        infos: &mut ReadInfoSeq,
    ) -> Result<bool, InternalError> {
        let time_out = *lock(&self.time_out);
        // No deadline for DURATION_INFINITE, or one beyond the clock.
        let deadline = if time_out.is_infinite() {
            None
        } else {
            Instant::now().checked_add(time_out.to_core())
        };
        loop {
            self.core.read_fresh(max, data, infos)?;
            if !data.is_empty() {
                return Ok(true);
            }
            let wait = match deadline {
                None => INFINITE_WAIT_STEP,
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(false);
                    }
                    left
                }
            };
            // A WaitSet of this call: two gets may wait at once.
            let waitset = WaitSet::new();
            waitset.attach_condition(self.core.fresh_condition())?;
            match waitset.wait(wait) {
                // After the time-out one more look, then the deadline
                // answers.
                Ok(_) | Err(DdsError::Timeout) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl<T: DdsType + Send + Sync + 'static> Getter<T> for GetterFragment<T> {
    fn get_one(&self, datum: &mut T, info: &mut ReadInfo) -> Result<bool, InternalError> {
        let (mut data, mut infos) = (Vec::new(), Vec::new());
        if !self.get(1, &mut data, &mut infos)? {
            return Ok(false);
        }
        match (data.pop(), infos.pop()) {
            (Some(next), Some(next_info)) => {
                *datum = next;
                *info = next_info;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn get_many(&self, data: &mut Vec<T>, infos: &mut ReadInfoSeq) -> Result<bool, InternalError> {
        self.get(self.max_delivered_data(), data, infos)
    }

    fn time_out(&self) -> Duration {
        *lock(&self.time_out)
    }

    fn set_time_out(&self, value: Duration) {
        *lock(&self.time_out) = value;
    }

    fn max_delivered_data(&self) -> DataNumber {
        self.max_delivered_data.load(Ordering::Acquire)
    }

    fn set_max_delivered_data(&self, value: DataNumber) {
        self.max_delivered_data.store(value, Ordering::Release);
    }
}
