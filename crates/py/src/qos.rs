// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PyO3-Bindings fuer die DCPS-QoS-Surface (§6.2 Vendor-Spec
//! `zerodds-py-1.0`).
//!
//! Exponiert `DataWriterQos` und `DataReaderQos` als PyClasses mit
//! Setter-Methoden fuer alle 22 DDS-1.4-Policies. Anwender bauen QoS-
//! Objekte und uebergeben sie an `Publisher.create_*_writer_with_qos()`
//! / `Subscriber.create_*_reader_with_qos()`.

#![allow(clippy::missing_errors_doc)]
#![allow(unsafe_code)] // PyO3-Macros expanden zu unsafe
#![allow(unsafe_op_in_unsafe_fn)] // Rust 2024: PyO3-Macros koennen nicht wrappen
#![allow(clippy::useless_conversion)] // PyO3-Macro-generated `.into()`-Calls
#![allow(clippy::needless_lifetimes)] // PyO3-Signatures mit Bound<'py, T>
#![allow(clippy::new_without_default)] // #[new]-Konstruktoren sind Python-Construktoren
#![allow(clippy::unwrap_used)] // PyO3 macros + Test-Helper
#![allow(clippy::expect_used)] // PyO3 macros + Test-Helper

use std::sync::Mutex;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DeadlineQosPolicy, DestinationOrderKind,
    DestinationOrderQosPolicy, DurabilityKind, DurabilityQosPolicy, DurabilityServiceQosPolicy,
    GroupDataQosPolicy, HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy,
    LivelinessKind, LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy,
    OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
    ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
    TimeBasedFilterQosPolicy, TopicDataQosPolicy, TransportPriorityQosPolicy, UserDataQosPolicy,
    WriterDataLifecycleQosPolicy,
};
use zerodds_qos::Duration as DdsDuration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Konvertiert eine `f64`-Sekundenzahl in `zerodds_qos::Duration`.
///
/// - `secs < 0.0`, `NaN` oder `Inf` → `Duration::INFINITE` (§9.3.2).
/// - sonst → Millisekunden, gerundet (Duration speichert i32-Sekunden +
///   u32-Fraction; `from_millis(i32)` ist die Standard-Konvertierung
///   aus dem Crate).
fn duration_from_secs_or_infinite(secs: f64) -> DdsDuration {
    if !secs.is_finite() || secs < 0.0 {
        return DdsDuration::INFINITE;
    }
    let millis = (secs * 1000.0).round();
    let clamped = millis.clamp(0.0, i32::MAX as f64) as i32;
    DdsDuration::from_millis(clamped)
}

fn parse_reliability_kind(s: &str) -> PyResult<ReliabilityKind> {
    match s {
        "Reliable" | "RELIABLE" | "reliable" => Ok(ReliabilityKind::Reliable),
        "BestEffort" | "BEST_EFFORT" | "best_effort" | "besteffort" => {
            Ok(ReliabilityKind::BestEffort)
        }
        other => Err(PyValueError::new_err(format!(
            "unknown ReliabilityKind {other:?}; expected 'Reliable' | 'BestEffort'"
        ))),
    }
}

fn parse_durability_kind(s: &str) -> PyResult<DurabilityKind> {
    match s {
        "Volatile" | "volatile" => Ok(DurabilityKind::Volatile),
        "TransientLocal" | "transient_local" => Ok(DurabilityKind::TransientLocal),
        "Transient" | "transient" => Ok(DurabilityKind::Transient),
        "Persistent" | "persistent" => Ok(DurabilityKind::Persistent),
        other => Err(PyValueError::new_err(format!(
            "unknown DurabilityKind {other:?}; expected 'Volatile'|'TransientLocal'|'Transient'|'Persistent'"
        ))),
    }
}

fn parse_history_kind(s: &str) -> PyResult<HistoryKind> {
    match s {
        "KeepLast" | "keep_last" => Ok(HistoryKind::KeepLast),
        "KeepAll" | "keep_all" => Ok(HistoryKind::KeepAll),
        other => Err(PyValueError::new_err(format!(
            "unknown HistoryKind {other:?}; expected 'KeepLast' | 'KeepAll'"
        ))),
    }
}

fn parse_liveliness_kind(s: &str) -> PyResult<LivelinessKind> {
    match s {
        "Automatic" | "automatic" => Ok(LivelinessKind::Automatic),
        "ManualByParticipant" | "manual_by_participant" => Ok(LivelinessKind::ManualByParticipant),
        "ManualByTopic" | "manual_by_topic" => Ok(LivelinessKind::ManualByTopic),
        other => Err(PyValueError::new_err(format!(
            "unknown LivelinessKind {other:?}; expected 'Automatic'|'ManualByParticipant'|'ManualByTopic'"
        ))),
    }
}

fn parse_ownership_kind(s: &str) -> PyResult<OwnershipKind> {
    match s {
        "Shared" | "shared" => Ok(OwnershipKind::Shared),
        "Exclusive" | "exclusive" => Ok(OwnershipKind::Exclusive),
        other => Err(PyValueError::new_err(format!(
            "unknown OwnershipKind {other:?}; expected 'Shared' | 'Exclusive'"
        ))),
    }
}

fn parse_destination_order_kind(s: &str) -> PyResult<DestinationOrderKind> {
    match s {
        "ByReceptionTimestamp" | "by_reception" => Ok(DestinationOrderKind::ByReceptionTimestamp),
        "BySourceTimestamp" | "by_source" => Ok(DestinationOrderKind::BySourceTimestamp),
        other => Err(PyValueError::new_err(format!(
            "unknown DestinationOrderKind {other:?}; expected 'ByReceptionTimestamp'|'BySourceTimestamp'"
        ))),
    }
}

fn parse_presentation_scope(s: &str) -> PyResult<PresentationAccessScope> {
    match s {
        "Instance" | "instance" => Ok(PresentationAccessScope::Instance),
        "Topic" | "topic" => Ok(PresentationAccessScope::Topic),
        "Group" | "group" => Ok(PresentationAccessScope::Group),
        other => Err(PyValueError::new_err(format!(
            "unknown PresentationAccessScope {other:?}; expected 'Instance'|'Topic'|'Group'"
        ))),
    }
}

// ---------------------------------------------------------------------------
// PyDataWriterQos
// ---------------------------------------------------------------------------

/// QoS-Builder fuer einen `DataWriter`. Spec: DDS 1.4 §2.2.3 (alle 22
/// Policies). Default-Werte folgen DDS 1.4 §2.2.3 Defaults (siehe
/// `crates/dcps/src/qos.rs::DataWriterQos::default`).
#[pyclass(name = "DataWriterQos", module = "zerodds_py")]
pub struct PyDataWriterQos {
    pub inner: Mutex<DataWriterQos>,
}

impl PyDataWriterQos {
    pub fn cloned_inner(&self) -> DataWriterQos {
        self.inner
            .lock()
            .expect("DataWriterQos lock poisoned")
            .clone()
    }
}

#[pymethods]
impl PyDataWriterQos {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mutex::new(DataWriterQos::default()),
        }
    }

    fn set_reliability(&self, kind: &str, max_blocking_time_secs: f64) -> PyResult<()> {
        let k = parse_reliability_kind(kind)?;
        let mut q = self.inner.lock().unwrap();
        q.reliability = ReliabilityQosPolicy {
            kind: k,
            max_blocking_time: duration_from_secs_or_infinite(max_blocking_time_secs),
        };
        Ok(())
    }

    fn set_durability(&self, kind: &str) -> PyResult<()> {
        let k = parse_durability_kind(kind)?;
        self.inner.lock().unwrap().durability = DurabilityQosPolicy { kind: k };
        Ok(())
    }

    fn set_durability_service(
        &self,
        cleanup_delay_secs: f64,
        history_kind: &str,
        history_depth: i32,
        max_samples: i32,
        max_instances: i32,
        max_samples_per_instance: i32,
    ) -> PyResult<()> {
        let hk = parse_history_kind(history_kind)?;
        self.inner.lock().unwrap().durability_service = DurabilityServiceQosPolicy {
            service_cleanup_delay: duration_from_secs_or_infinite(cleanup_delay_secs),
            history_kind: hk,
            history_depth,
            max_samples,
            max_instances,
            max_samples_per_instance,
        };
        Ok(())
    }

    fn set_deadline(&self, period_secs: f64) -> PyResult<()> {
        self.inner.lock().unwrap().deadline = DeadlineQosPolicy {
            period: duration_from_secs_or_infinite(period_secs),
        };
        Ok(())
    }

    fn set_latency_budget(&self, duration_secs: f64) -> PyResult<()> {
        self.inner.lock().unwrap().latency_budget = LatencyBudgetQosPolicy {
            duration: duration_from_secs_or_infinite(duration_secs),
        };
        Ok(())
    }

    fn set_liveliness(&self, kind: &str, lease_duration_secs: f64) -> PyResult<()> {
        let k = parse_liveliness_kind(kind)?;
        self.inner.lock().unwrap().liveliness = LivelinessQosPolicy {
            kind: k,
            lease_duration: duration_from_secs_or_infinite(lease_duration_secs),
        };
        Ok(())
    }

    fn set_destination_order(&self, kind: &str) -> PyResult<()> {
        let k = parse_destination_order_kind(kind)?;
        self.inner.lock().unwrap().destination_order = DestinationOrderQosPolicy { kind: k };
        Ok(())
    }

    fn set_lifespan(&self, duration_secs: f64) -> PyResult<()> {
        self.inner.lock().unwrap().lifespan = LifespanQosPolicy {
            duration: duration_from_secs_or_infinite(duration_secs),
        };
        Ok(())
    }

    fn set_ownership(&self, kind: &str) -> PyResult<()> {
        let k = parse_ownership_kind(kind)?;
        self.inner.lock().unwrap().ownership = OwnershipQosPolicy { kind: k };
        Ok(())
    }

    fn set_ownership_strength(&self, value: i32) -> PyResult<()> {
        self.inner.lock().unwrap().ownership_strength = OwnershipStrengthQosPolicy { value };
        Ok(())
    }

    fn set_partition(&self, names: Vec<String>) -> PyResult<()> {
        self.inner.lock().unwrap().partition = PartitionQosPolicy { names };
        Ok(())
    }

    fn set_presentation(
        &self,
        scope: &str,
        coherent_access: bool,
        ordered_access: bool,
    ) -> PyResult<()> {
        let s = parse_presentation_scope(scope)?;
        self.inner.lock().unwrap().presentation = PresentationQosPolicy {
            access_scope: s,
            coherent_access,
            ordered_access,
        };
        Ok(())
    }

    fn set_history(&self, kind: &str, depth: i32) -> PyResult<()> {
        let k = parse_history_kind(kind)?;
        self.inner.lock().unwrap().history = HistoryQosPolicy { kind: k, depth };
        Ok(())
    }

    fn set_resource_limits(
        &self,
        max_samples: i32,
        max_instances: i32,
        max_samples_per_instance: i32,
    ) -> PyResult<()> {
        self.inner.lock().unwrap().resource_limits = ResourceLimitsQosPolicy {
            max_samples,
            max_instances,
            max_samples_per_instance,
        };
        Ok(())
    }

    fn set_transport_priority(&self, value: i32) -> PyResult<()> {
        self.inner.lock().unwrap().transport_priority = TransportPriorityQosPolicy { value };
        Ok(())
    }

    fn set_writer_data_lifecycle(&self, autodispose_unregistered_instances: bool) -> PyResult<()> {
        self.inner.lock().unwrap().writer_data_lifecycle = WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances,
        };
        Ok(())
    }

    fn set_user_data(&self, data: Vec<u8>) -> PyResult<()> {
        self.inner.lock().unwrap().user_data = UserDataQosPolicy { value: data };
        Ok(())
    }

    fn set_topic_data(&self, data: Vec<u8>) -> PyResult<()> {
        self.inner.lock().unwrap().topic_data = TopicDataQosPolicy { value: data };
        Ok(())
    }

    fn set_group_data(&self, data: Vec<u8>) -> PyResult<()> {
        self.inner.lock().unwrap().group_data = GroupDataQosPolicy { value: data };
        Ok(())
    }

    // --- Read-only Reflection ---

    fn reliability_kind(&self) -> &'static str {
        match self.inner.lock().unwrap().reliability.kind {
            ReliabilityKind::Reliable => "Reliable",
            ReliabilityKind::BestEffort => "BestEffort",
        }
    }

    fn durability_kind(&self) -> &'static str {
        match self.inner.lock().unwrap().durability.kind {
            DurabilityKind::Volatile => "Volatile",
            DurabilityKind::TransientLocal => "TransientLocal",
            DurabilityKind::Transient => "Transient",
            DurabilityKind::Persistent => "Persistent",
        }
    }

    fn history_kind(&self) -> &'static str {
        match self.inner.lock().unwrap().history.kind {
            HistoryKind::KeepLast => "KeepLast",
            HistoryKind::KeepAll => "KeepAll",
        }
    }

    fn history_depth(&self) -> i32 {
        self.inner.lock().unwrap().history.depth
    }
}

// ---------------------------------------------------------------------------
// PyDataReaderQos
// ---------------------------------------------------------------------------

#[pyclass(name = "DataReaderQos", module = "zerodds_py")]
pub struct PyDataReaderQos {
    pub inner: Mutex<DataReaderQos>,
}

impl PyDataReaderQos {
    pub fn cloned_inner(&self) -> DataReaderQos {
        self.inner
            .lock()
            .expect("DataReaderQos lock poisoned")
            .clone()
    }
}

#[pymethods]
impl PyDataReaderQos {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mutex::new(DataReaderQos::default()),
        }
    }

    fn set_reliability(&self, kind: &str, max_blocking_time_secs: f64) -> PyResult<()> {
        let k = parse_reliability_kind(kind)?;
        self.inner.lock().unwrap().reliability = ReliabilityQosPolicy {
            kind: k,
            max_blocking_time: duration_from_secs_or_infinite(max_blocking_time_secs),
        };
        Ok(())
    }

    fn set_durability(&self, kind: &str) -> PyResult<()> {
        let k = parse_durability_kind(kind)?;
        self.inner.lock().unwrap().durability = DurabilityQosPolicy { kind: k };
        Ok(())
    }

    fn set_deadline(&self, period_secs: f64) -> PyResult<()> {
        self.inner.lock().unwrap().deadline = DeadlineQosPolicy {
            period: duration_from_secs_or_infinite(period_secs),
        };
        Ok(())
    }

    fn set_latency_budget(&self, duration_secs: f64) -> PyResult<()> {
        self.inner.lock().unwrap().latency_budget = LatencyBudgetQosPolicy {
            duration: duration_from_secs_or_infinite(duration_secs),
        };
        Ok(())
    }

    fn set_liveliness(&self, kind: &str, lease_duration_secs: f64) -> PyResult<()> {
        let k = parse_liveliness_kind(kind)?;
        self.inner.lock().unwrap().liveliness = LivelinessQosPolicy {
            kind: k,
            lease_duration: duration_from_secs_or_infinite(lease_duration_secs),
        };
        Ok(())
    }

    fn set_destination_order(&self, kind: &str) -> PyResult<()> {
        let k = parse_destination_order_kind(kind)?;
        self.inner.lock().unwrap().destination_order = DestinationOrderQosPolicy { kind: k };
        Ok(())
    }

    fn set_ownership(&self, kind: &str) -> PyResult<()> {
        let k = parse_ownership_kind(kind)?;
        self.inner.lock().unwrap().ownership = OwnershipQosPolicy { kind: k };
        Ok(())
    }

    fn set_partition(&self, names: Vec<String>) -> PyResult<()> {
        self.inner.lock().unwrap().partition = PartitionQosPolicy { names };
        Ok(())
    }

    fn set_presentation(
        &self,
        scope: &str,
        coherent_access: bool,
        ordered_access: bool,
    ) -> PyResult<()> {
        let s = parse_presentation_scope(scope)?;
        self.inner.lock().unwrap().presentation = PresentationQosPolicy {
            access_scope: s,
            coherent_access,
            ordered_access,
        };
        Ok(())
    }

    fn set_history(&self, kind: &str, depth: i32) -> PyResult<()> {
        let k = parse_history_kind(kind)?;
        self.inner.lock().unwrap().history = HistoryQosPolicy { kind: k, depth };
        Ok(())
    }

    fn set_resource_limits(
        &self,
        max_samples: i32,
        max_instances: i32,
        max_samples_per_instance: i32,
    ) -> PyResult<()> {
        self.inner.lock().unwrap().resource_limits = ResourceLimitsQosPolicy {
            max_samples,
            max_instances,
            max_samples_per_instance,
        };
        Ok(())
    }

    fn set_time_based_filter(&self, minimum_separation_secs: f64) -> PyResult<()> {
        self.inner.lock().unwrap().time_based_filter = TimeBasedFilterQosPolicy {
            minimum_separation: duration_from_secs_or_infinite(minimum_separation_secs),
        };
        Ok(())
    }

    fn set_reader_data_lifecycle(
        &self,
        autopurge_nowriter_samples_delay_secs: f64,
        autopurge_disposed_samples_delay_secs: f64,
    ) -> PyResult<()> {
        self.inner.lock().unwrap().reader_data_lifecycle = ReaderDataLifecycleQosPolicy {
            autopurge_nowriter_samples_delay: duration_from_secs_or_infinite(
                autopurge_nowriter_samples_delay_secs,
            ),
            autopurge_disposed_samples_delay: duration_from_secs_or_infinite(
                autopurge_disposed_samples_delay_secs,
            ),
        };
        Ok(())
    }

    fn set_user_data(&self, data: Vec<u8>) -> PyResult<()> {
        self.inner.lock().unwrap().user_data = UserDataQosPolicy { value: data };
        Ok(())
    }

    fn set_topic_data(&self, data: Vec<u8>) -> PyResult<()> {
        self.inner.lock().unwrap().topic_data = TopicDataQosPolicy { value: data };
        Ok(())
    }

    fn set_group_data(&self, data: Vec<u8>) -> PyResult<()> {
        self.inner.lock().unwrap().group_data = GroupDataQosPolicy { value: data };
        Ok(())
    }

    // --- Read-only Reflection ---

    fn reliability_kind(&self) -> &'static str {
        match self.inner.lock().unwrap().reliability.kind {
            ReliabilityKind::Reliable => "Reliable",
            ReliabilityKind::BestEffort => "BestEffort",
        }
    }

    fn durability_kind(&self) -> &'static str {
        match self.inner.lock().unwrap().durability.kind {
            DurabilityKind::Volatile => "Volatile",
            DurabilityKind::TransientLocal => "TransientLocal",
            DurabilityKind::Transient => "Transient",
            DurabilityKind::Persistent => "Persistent",
        }
    }

    fn history_kind(&self) -> &'static str {
        match self.inner.lock().unwrap().history.kind {
            HistoryKind::KeepLast => "KeepLast",
            HistoryKind::KeepAll => "KeepAll",
        }
    }

    fn history_depth(&self) -> i32 {
        self.inner.lock().unwrap().history.depth
    }
}
