// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the basic ports of one subscribing port share: the DataReader, the
//! query of the Reader (§7.2.2.1.2: "Through the query (if any) of the
//! Reader associated to the port") and the component's view of the
//! instances ([`crate::instance_status`]).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use zerodds_dcps::condition::{Condition, QueryCondition, ReadCondition};
use zerodds_dcps::dds_type::DdsTypeRow;
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{DataReader, DdsError, DdsType, InstanceHandle, ReadSelection, Sample};

use crate::ccm_dds::{DataNumber, QueryFilter, ReadInfo, UNLIMITED};
use crate::instance_status::{
    InstanceView, deliver, order_by_instance, order_delivered_by_instance,
};

/// A lock whose holder panicked still guards consistent data here: every
/// critical section of this crate replaces a value as its last step.
pub(crate) fn lock<V>(mutex: &Mutex<V>) -> MutexGuard<'_, V> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The access parameters of the Reader (§7.2.2.1.2): `READ` or `NOT_READ`,
/// `NEW` or `NOT_NEW`, `ALIVE`.
const READER_SELECTION: ReadSelection = ReadSelection {
    max_samples: -1,
    sample_states: sample_state_mask::ANY,
    view_states: view_state_mask::ANY,
    instance_states: instance_state_mask::ALIVE,
};

/// The access parameters of fresh data (§7.2.2.1.2, Getter): `NOT_READ`,
/// `NEW` or `NOT_NEW`, `ALIVE` or `NOT_ALIVE`.
const FRESH_SELECTION: ReadSelection = ReadSelection {
    max_samples: -1,
    sample_states: sample_state_mask::NOT_READ,
    view_states: view_state_mask::ANY,
    instance_states: instance_state_mask::ANY,
};

struct QueryState {
    filter: QueryFilter,
    /// The query with the access parameters of the Reader; `None` without
    /// a query.
    alive: Option<Arc<QueryCondition>>,
}

/// See the module documentation.
pub(crate) struct ReaderCore<T: DdsType> {
    reader: DataReader<T>,
    query: Mutex<QueryState>,
    /// Triggers while the reader holds a sample that was not read.
    fresh: Arc<ReadCondition>,
    /// Held over a read and the delivery of what it returned, so the
    /// component's view moves in the order the samples leave the reader,
    /// whichever basic port reads.
    view: Mutex<InstanceView>,
}

impl<T: DdsType + Send + 'static> ReaderCore<T> {
    /// The shared state over `reader`, without a query.
    pub(crate) fn new(reader: DataReader<T>) -> Result<Arc<Self>, DdsError> {
        let fresh = reader.create_readcondition(
            FRESH_SELECTION.sample_states,
            FRESH_SELECTION.view_states,
            FRESH_SELECTION.instance_states,
        )?;
        Ok(Arc::new(Self {
            reader,
            query: Mutex::new(QueryState {
                filter: QueryFilter::default(),
                alive: None,
            }),
            fresh,
            view: Mutex::new(InstanceView::default()),
        }))
    }

    /// The DataReader of the port.
    pub(crate) fn reader(&self) -> &DataReader<T> {
        &self.reader
    }

    /// Attribute `query` of the Reader.
    pub(crate) fn query(&self) -> QueryFilter {
        lock(&self.query).filter.clone()
    }

    /// Sets attribute `query`. The condition of the new query exists before
    /// the one of the old query goes, so a value DDS refuses leaves the
    /// attribute as it was.
    pub(crate) fn set_query(&self, filter: QueryFilter) -> Result<(), DdsError> {
        let alive = if filter.is_empty() {
            None
        } else {
            Some(self.reader.create_querycondition(
                READER_SELECTION.sample_states,
                READER_SELECTION.view_states,
                READER_SELECTION.instance_states,
                &filter.expression,
                filter.parameters.clone(),
            )?)
        };
        let old = std::mem::replace(&mut *lock(&self.query), QueryState { filter, alive });
        match old.alive {
            Some(alive) => self.reader.delete_readcondition(alive.base()),
            None => Ok(()),
        }
    }

    /// Deletes the conditions of the port; the reader can be deleted then.
    /// A condition that could not be deleted stays with the core, so a
    /// second call tries it again; one that was deleted is not deleted
    /// twice. The caller calls it again only after an error.
    pub(crate) fn close(&self) -> Result<(), DdsError> {
        let mut query = lock(&self.query);
        if let Some(alive) = &query.alive {
            self.reader.delete_readcondition(alive.base())?;
            query.alive = None;
        }
        drop(query);
        self.reader.delete_readcondition(&self.fresh)
    }

    /// The condition that triggers while the reader holds fresh data, for a
    /// WaitSet.
    pub(crate) fn fresh_condition(&self) -> Arc<dyn Condition> {
        Arc::clone(&self.fresh) as Arc<dyn Condition>
    }

    /// A read with the access parameters of the Reader: every sample of
    /// `instance`, or of every instance, through the query; with
    /// `last_only` the last sample of each instance. The result replaces
    /// the content of `data` and `infos`.
    pub(crate) fn read_alive(
        &self,
        instance: Option<InstanceHandle>,
        last_only: bool,
        data: &mut Vec<T>,
        infos: &mut Vec<ReadInfo>,
    ) -> Result<(), DdsError> {
        data.clear();
        infos.clear();
        let mut view = lock(&self.view);
        let alive = lock(&self.query).alive.clone();
        let read = match (alive, instance) {
            (Some(query), Some(handle)) => self.reader.read_instance_w_condition(handle, &query),
            (Some(query), None) => self.reader.read_w_condition_selected(&query, -1),
            (None, Some(handle)) => self
                .reader
                .read_instance_selected(handle, &READER_SELECTION),
            (None, None) => self.reader.read_selected(&READER_SELECTION),
        };
        let mut samples = no_data_is_empty(read)?;
        // A sample without data of an instance that is alive again reports
        // a state the instance has left; the fragment keeps it.
        samples.retain(|sample| sample.info.valid_data);
        order_by_instance(&mut samples);
        if last_only {
            samples = last_of_each_instance(samples);
        }
        // The samples came through the query in DDS.
        deliver(&mut view, samples, |_| true, data, infos);
        self.forget_purged(&mut view);
        Ok(())
    }

    /// A read of fresh data: the samples that were not read before, through
    /// the query, at most `max` of them ([`UNLIMITED`] for all); the others
    /// stay for the next call. The result replaces the content of `data`
    /// and `infos`.
    ///
    /// The fragment reads every fresh sample and evaluates the query
    /// itself, for two reasons. A sample without data holds the key only,
    /// and the deletion it reports is for the component whatever the query
    /// says of the other members. And a sample with data that the query
    /// rejects is what tells that its instance left the query
    /// (`INSTANCE_FILTERED_OUT`, [`crate::instance_status`]), which a DDS
    /// read through a QueryCondition does not report.
    ///
    /// Every fresh sample is therefore judged once, by the query the Reader
    /// has at that time, and is `READ` afterwards: one the query rejected is
    /// not gotten later under another query. A read of the Reader still
    /// returns it, as `ALREADY_SEEN`, when its query lets it through.
    pub(crate) fn read_fresh(
        &self,
        max: DataNumber,
        data: &mut Vec<T>,
        infos: &mut Vec<ReadInfo>,
    ) -> Result<(), DdsError> {
        data.clear();
        infos.clear();
        let limit = if max == UNLIMITED {
            usize::MAX
        } else {
            usize::try_from(max).unwrap_or(usize::MAX)
        };
        let mut view = lock(&self.view);
        let query = lock(&self.query).alive.clone();
        // As a read with the condition does: a sample the expression cannot
        // be evaluated against is rejected.
        let passes = |datum: &T| {
            query
                .as_ref()
                .is_none_or(|query| query.evaluate(&DdsTypeRow::new(datum)).unwrap_or(false))
        };
        while data.len() < limit {
            let selection = ReadSelection {
                // Beyond `i32::MAX`: all, `LENGTH_UNLIMITED`.
                max_samples: i32::try_from(limit - data.len()).unwrap_or(-1),
                ..FRESH_SELECTION
            };
            let samples = no_data_is_empty(self.reader.read_selected(&selection))?;
            if samples.is_empty() {
                break;
            }
            deliver(&mut view, samples, passes, data, infos);
        }
        order_delivered_by_instance(data, infos);
        self.forget_purged(&mut view);
        Ok(())
    }

    /// The number of instances that exist for the component.
    pub(crate) fn existing(&self) -> usize {
        lock(&self.view).len()
    }

    /// Whether `instance` exists for the component.
    pub(crate) fn exists(&self, instance: InstanceHandle) -> bool {
        lock(&self.view).contains(instance)
    }

    /// Drops the instances the reader purged from the view, once the view
    /// holds more instances than the reader.
    fn forget_purged(&self, view: &mut InstanceView) {
        let tracker = self.reader.instance_tracker();
        if view.len() > tracker.len() {
            view.retain(|handle| tracker.get_by_handle(handle).is_some());
        }
    }
}

/// DDS returns `NO_DATA` where the ports return an empty sequence ("In case
/// of no data, the resulting data will be a void sequence").
fn no_data_is_empty<T>(read: Result<Vec<Sample<T>>, DdsError>) -> Result<Vec<Sample<T>>, DdsError> {
    match read {
        Err(DdsError::NoData) => Ok(Vec::new()),
        other => other,
    }
}

/// Of samples ordered by instance, the last one of each instance.
fn last_of_each_instance<T>(samples: Vec<Sample<T>>) -> Vec<Sample<T>> {
    let mut last: Vec<Sample<T>> = Vec::new();
    for sample in samples {
        match last.last_mut() {
            Some(previous) if previous.info.instance_handle == sample.info.instance_handle => {
                *previous = sample;
            }
            _ => last.push(sample),
        }
    }
    last
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_dcps::sample_info::SampleInfo;

    #[test]
    fn dds4ccm_7_2_2_1_2_read_last_keeps_the_last_sample_of_each_instance() {
        let sample = |handle: u64, value: u32| {
            Sample::new(
                value,
                SampleInfo {
                    instance_handle: InstanceHandle::from_raw(handle),
                    ..SampleInfo::default()
                },
            )
        };
        let last = last_of_each_instance(vec![
            sample(1, 10),
            sample(1, 11),
            sample(2, 20),
            sample(3, 30),
            sample(3, 31),
            sample(3, 32),
        ]);
        let values: Vec<u32> = last.iter().map(|s| s.data).collect();
        assert_eq!(values, [11, 20, 32]);
    }

    #[test]
    fn dds4ccm_7_2_2_1_2_no_data_is_an_empty_sequence_and_an_error_stays_one() {
        assert_eq!(
            no_data_is_empty::<u32>(Err(DdsError::NoData))
                .unwrap()
                .len(),
            0
        );
        assert!(matches!(
            no_data_is_empty::<u32>(Err(DdsError::AlreadyDeleted)),
            Err(DdsError::AlreadyDeleted)
        ));
    }
}
