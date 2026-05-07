// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Historical-Sample-Adapter — Spec §9.3.4.4 + §8.3.4.2 history_read.
//!
//! Liefert die Schnittstelle, mit der der OPC-UA-Server-Stack im
//! Gateway den `HistoryRead`-Request auf die DataReader-Sample-
//! Historie umleitet. Der Caller (Daemon-Crate) implementiert
//! [`HistoricalSampleAdapter`] gegen den konkreten History-Cache;
//! dieses Modul liefert ein **InMemory-Default-Backend**, das
//! ausreichend ist fuer Test + kleine Einsaetze.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::data_value::DataValue;

/// Ein historisches Sample mit Zeitstempel + Wert.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalSample {
    /// `SourceTimestamp` als Windows-FILETIME-Ticks (i64).
    pub source_timestamp: i64,
    /// `DataValue` (incl. Variant + StatusCode) — Spec Tab 8.6
    /// HistoryReadResult.history_data hat sequence<DataValue>.
    pub value: DataValue,
}

/// Read-History-Query-Parameter — entspricht der Subset von
/// `ReadRawModifiedDetails` + `ReadAtTimeDetails`, die das Gateway
/// aus dem OPC-UA-Request extrahiert.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadHistoryQuery {
    /// `READ_RAW_MODIFIED_HISTORY_READ_DETAILS_KIND` (§8.3.4.1
    /// `ReadRawModifiedDetails`): Time-Range-Read (raw == false ist
    /// der einzig sinnvolle Modus fuer DDS — modified wird nicht
    /// unterstuetzt, weil DDS keine modified-Versionen liefert).
    Raw {
        /// Range-Start in i64-Ticks.
        start_time: i64,
        /// Range-Ende.
        end_time: i64,
        /// Maximalzahl Werte (0 = unlimited).
        num_values_per_node: u32,
        /// `return_bounds` — wenn true werden grenz-naehe Bounds-Samples
        /// mitgeliefert (Spec OPCUA-11 §6.4.3).
        return_bounds: bool,
    },
    /// `READ_AT_TIME_HISTORY_READ_DETAILS_KIND` (§8.3.4.1
    /// `ReadAtTimeDetails`): Read-At-Time (multi-time-point).
    AtTime {
        /// Liste angeforderter Zeitpunkte.
        req_times: Vec<i64>,
        /// `use_simple_bounds` — Interpolation/Bounds-Verhalten.
        use_simple_bounds: bool,
    },
}

/// Read-History-Result.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadHistoryResult {
    /// Gefundene Samples in zeitlicher Reihenfolge (aufsteigend).
    pub samples: Vec<HistoricalSample>,
    /// Hat das Backend mehr Samples als zurueckgegeben (Pagination
    /// noetig)? Spec §8.3.4.1: liefert dann ContinuationPoint.
    pub more_available: bool,
}

/// Adapter-Contract — Caller implementiert das gegen den Reader-
/// History-Cache aus `crates/dcps/`.
pub trait HistoricalSampleAdapter {
    /// Fuehrt eine HistoryRead-Query aus.
    ///
    /// # Errors
    /// Trait-Implementer waehlen den konkreten Error-Type.
    fn read_history(
        &self,
        instance_handle: &str,
        query: &ReadHistoryQuery,
    ) -> Result<ReadHistoryResult, HistoryReadError>;
}

/// Fehler im History-Read-Pfad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryReadError {
    /// `instance_handle` ist keinem registrierten Sample-Stream
    /// zugeordnet.
    UnknownInstance,
    /// Zeit-Range ist ungueltig (`start > end` oder leere AtTime-Liste).
    InvalidTimeRange,
}

// -------------------------------------------------------------------
// In-Memory-Default-Implementation.
// -------------------------------------------------------------------

/// Einfacher In-Memory-History-Cache. Pro `instance_handle` wird ein
/// `BTreeMap<i64, HistoricalSample>` (sortiert nach SourceTimestamp)
/// gehalten. Fuer Test + kleine Deployments ausreichend; Production-
/// Caller koennen einen Persistent-Backend implementieren.
#[derive(Debug, Default)]
pub struct InMemoryHistoryCache {
    streams: BTreeMap<alloc::string::String, BTreeMap<i64, HistoricalSample>>,
}

impl InMemoryHistoryCache {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuegt ein Sample zum Stream `instance_handle` hinzu.
    pub fn push(&mut self, instance_handle: alloc::string::String, sample: HistoricalSample) {
        self.streams
            .entry(instance_handle)
            .or_default()
            .insert(sample.source_timestamp, sample);
    }

    /// Anzahl Samples pro Instance — fuer Tests.
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
                    // Spec OPCUA-11 §6.4.5: bei AtTime ohne exakten
                    // Match wird interpoliert oder Bound zurueckgegeben.
                    // Wir liefern den nearest-prior-Sample (simple-bounds).
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
