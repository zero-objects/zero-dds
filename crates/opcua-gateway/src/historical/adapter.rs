// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Historical-Sample-Adapter — Spec §9.3.4.4 + §8.3.4.2 history_read.
//!
//! Provides the interface with which the OPC-UA server stack in the
//! gateway redirects the `HistoryRead` request onto the DataReader sample
//! history. The caller (daemon crate) implements
//! [`HistoricalSampleAdapter`] against the concrete history cache;
//! this module provides an **in-memory default backend** that
//! is sufficient for tests + small deployments.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::data_value::DataValue;

/// A historical sample with a timestamp + value.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalSample {
    /// `SourceTimestamp` as Windows FILETIME ticks (i64).
    pub source_timestamp: i64,
    /// `DataValue` (incl. Variant + StatusCode) — Spec Tab 8.6
    /// HistoryReadResult.history_data has sequence<DataValue>.
    pub value: DataValue,
}

/// Read-history query parameters — corresponds to the subset of
/// `ReadRawModifiedDetails` + `ReadAtTimeDetails` that the gateway
/// extracts from the OPC-UA request.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadHistoryQuery {
    /// `READ_RAW_MODIFIED_HISTORY_READ_DETAILS_KIND` (§8.3.4.1
    /// `ReadRawModifiedDetails`): time-range read (raw == false is
    /// the only sensible mode for DDS — modified is not
    /// supported, because DDS provides no modified versions).
    Raw {
        /// Range start in i64 ticks.
        start_time: i64,
        /// Range end.
        end_time: i64,
        /// Maximum number of values (0 = unlimited).
        num_values_per_node: u32,
        /// `return_bounds` — if true, boundary-near bounds samples
        /// are included (Spec OPCUA-11 §6.4.3).
        return_bounds: bool,
    },
    /// `READ_AT_TIME_HISTORY_READ_DETAILS_KIND` (§8.3.4.1
    /// `ReadAtTimeDetails`): read-at-time (multi-time-point).
    AtTime {
        /// List of requested points in time.
        req_times: Vec<i64>,
        /// `use_simple_bounds` — interpolation/bounds behavior.
        use_simple_bounds: bool,
    },
}

/// Read-history result.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadHistoryResult {
    /// Found samples in temporal order (ascending).
    pub samples: Vec<HistoricalSample>,
    /// Does the backend have more samples than returned (pagination
    /// needed)? Spec §8.3.4.1: then returns a ContinuationPoint.
    pub more_available: bool,
}

/// Adapter contract — the caller implements this against the reader
/// history cache from `crates/dcps/`.
pub trait HistoricalSampleAdapter {
    /// Executes a HistoryRead query.
    ///
    /// # Errors
    /// Trait implementers choose the concrete error type.
    fn read_history(
        &self,
        instance_handle: &str,
        query: &ReadHistoryQuery,
    ) -> Result<ReadHistoryResult, HistoryReadError>;
}

/// Error in the history-read path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryReadError {
    /// `instance_handle` is not associated with any registered sample
    /// stream.
    UnknownInstance,
    /// The time range is invalid (`start > end` or an empty AtTime list).
    InvalidTimeRange,
}

// -------------------------------------------------------------------
// In-memory default implementation.
// -------------------------------------------------------------------

/// Simple in-memory history cache. A
/// `BTreeMap<i64, HistoricalSample>` (sorted by SourceTimestamp) is held
/// per `instance_handle`. Sufficient for tests + small deployments;
/// production callers can implement a persistent backend.
#[derive(Debug, Default)]
pub struct InMemoryHistoryCache {
    streams: BTreeMap<alloc::string::String, BTreeMap<i64, HistoricalSample>>,
}

impl InMemoryHistoryCache {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a sample to the stream `instance_handle`.
    pub fn push(&mut self, instance_handle: alloc::string::String, sample: HistoricalSample) {
        self.streams
            .entry(instance_handle)
            .or_default()
            .insert(sample.source_timestamp, sample);
    }

    /// Number of samples per instance — for tests.
    #[must_use]
    pub fn len(&self, instance_handle: &str) -> usize {
        self.streams.get(instance_handle).map_or(0, BTreeMap::len)
    }
}

impl HistoricalSampleAdapter for InMemoryHistoryCache {
    fn read_history(
        &self,
        instance_handle: &str,
        query: &ReadHistoryQuery,
    ) -> Result<ReadHistoryResult, HistoryReadError> {
        let stream = self
            .streams
            .get(instance_handle)
            .ok_or(HistoryReadError::UnknownInstance)?;

        match query {
            ReadHistoryQuery::Raw {
                start_time,
                end_time,
                num_values_per_node,
                return_bounds: _,
            } => {
                if start_time > end_time {
                    return Err(HistoryReadError::InvalidTimeRange);
                }
                let limit = if *num_values_per_node == 0 {
                    usize::MAX
                } else {
                    *num_values_per_node as usize
                };
                let mut samples: Vec<HistoricalSample> = stream
                    .range(*start_time..=*end_time)
                    .map(|(_, v)| v.clone())
                    .collect();
                let total = samples.len();
                if total > limit {
                    samples.truncate(limit);
                }
                Ok(ReadHistoryResult {
                    samples,
                    more_available: total > limit,
                })
            }
            ReadHistoryQuery::AtTime {
                req_times,
                use_simple_bounds: _,
            } => {
                if req_times.is_empty() {
                    return Err(HistoryReadError::InvalidTimeRange);
                }
                let mut samples = Vec::with_capacity(req_times.len());
                for t in req_times {
                    // Spec OPCUA-11 §6.4.5: on AtTime without an exact
                    // match, it is interpolated or a bound is returned.
                    // We return the nearest-prior sample (simple bounds).
                    if let Some((_, s)) = stream.range(..=*t).next_back() {
                        samples.push(s.clone());
                    }
                }
                Ok(ReadHistoryResult {
                    samples,
                    more_available: false,
                })
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::data_value::{DataValue, Variant, VariantValue};

    fn make_sample(ts: i64, val: i32) -> HistoricalSample {
        HistoricalSample {
            source_timestamp: ts,
            value: DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(val))),
                status: None,
                source_timestamp: Some(ts),
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        }
    }

    #[test]
    fn raw_read_returns_samples_in_range() {
        let mut cache = InMemoryHistoryCache::new();
        for i in 0..10i64 {
            cache.push("inst".into(), make_sample(i * 100, i as i32));
        }

        let res = cache
            .read_history(
                "inst",
                &ReadHistoryQuery::Raw {
                    start_time: 200,
                    end_time: 500,
                    num_values_per_node: 0,
                    return_bounds: false,
                },
            )
            .unwrap();
        assert_eq!(res.samples.len(), 4); // ts: 200, 300, 400, 500
        assert_eq!(res.samples[0].source_timestamp, 200);
        assert_eq!(res.samples[3].source_timestamp, 500);
    }

    #[test]
    fn raw_read_truncates_at_num_values_per_node() {
        let mut cache = InMemoryHistoryCache::new();
        for i in 0..10i64 {
            cache.push("inst".into(), make_sample(i, i as i32));
        }
        let res = cache
            .read_history(
                "inst",
                &ReadHistoryQuery::Raw {
                    start_time: 0,
                    end_time: 100,
                    num_values_per_node: 3,
                    return_bounds: false,
                },
            )
            .unwrap();
        assert_eq!(res.samples.len(), 3);
        assert!(res.more_available);
    }

    #[test]
    fn raw_read_invalid_range_errors() {
        let mut cache = InMemoryHistoryCache::new();
        cache.push("inst".into(), make_sample(0, 0));
        let err = cache
            .read_history(
                "inst",
                &ReadHistoryQuery::Raw {
                    start_time: 100,
                    end_time: 50,
                    num_values_per_node: 0,
                    return_bounds: false,
                },
            )
            .unwrap_err();
        assert_eq!(err, HistoryReadError::InvalidTimeRange);
    }

    #[test]
    fn at_time_returns_nearest_prior_sample() {
        let mut cache = InMemoryHistoryCache::new();
        cache.push("inst".into(), make_sample(100, 1));
        cache.push("inst".into(), make_sample(200, 2));
        cache.push("inst".into(), make_sample(300, 3));

        let res = cache
            .read_history(
                "inst",
                &ReadHistoryQuery::AtTime {
                    req_times: alloc::vec![150, 250, 400],
                    use_simple_bounds: true,
                },
            )
            .unwrap();
        assert_eq!(res.samples.len(), 3);
        assert_eq!(res.samples[0].source_timestamp, 100);
        assert_eq!(res.samples[1].source_timestamp, 200);
        assert_eq!(res.samples[2].source_timestamp, 300);
    }

    #[test]
    fn at_time_empty_req_times_errors() {
        let mut cache = InMemoryHistoryCache::new();
        cache.push("inst".into(), make_sample(0, 0));
        let err = cache
            .read_history(
                "inst",
                &ReadHistoryQuery::AtTime {
                    req_times: alloc::vec::Vec::new(),
                    use_simple_bounds: true,
                },
            )
            .unwrap_err();
        assert_eq!(err, HistoryReadError::InvalidTimeRange);
    }

    #[test]
    fn unknown_instance_errors() {
        let cache = InMemoryHistoryCache::new();
        let err = cache
            .read_history(
                "missing",
                &ReadHistoryQuery::Raw {
                    start_time: 0,
                    end_time: 100,
                    num_values_per_node: 0,
                    return_bounds: false,
                },
            )
            .unwrap_err();
        assert_eq!(err, HistoryReadError::UnknownInstance);
    }
}
