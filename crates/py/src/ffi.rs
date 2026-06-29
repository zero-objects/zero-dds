// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PyO3 FFI module. Only active with `--features extension-module`.
//!
//! Current scope: raw-bytes topic. This is deliberately a thin
//! wrapper layer — the heavy lifting (QoS, discovery, wire) lives in
//! `zerodds-dcps`. No Python-specific logic here apart from
//! Rust↔Python conversion.
//!
//! # SAFETY
//!
//! The `unsafe` expansions of `#[pymodule]` / `#[pyclass]` are
//! PyO3-macro-internal; we write no explicit `unsafe` here.
//! Thread safety: DataReader/Writer hold `Arc` and `Mutex` internally,
//! so that the GIL release during `write`/`take` is safe.

#![allow(clippy::missing_errors_doc)]
#![allow(unsafe_code)] // PyO3 macros expand to unsafe
#![allow(unsafe_op_in_unsafe_fn)] // Rust 2024: PyO3 macros cannot wrap
#![allow(clippy::useless_conversion)] // PyO3-macro-generated `.into()` calls
#![allow(clippy::needless_lifetimes)] // PyO3 signatures with Bound<'py, T>
#![allow(clippy::new_without_default)] // #[new] constructors are Python constructors, not Rust Default

use std::sync::Arc;
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyTimeoutError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

use zerodds_dcps::condition::{GuardCondition, WaitSet};
use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::sample::Sample;
use zerodds_dcps::sample_info::{InstanceStateKind, SampleStateKind, ViewStateKind};
use zerodds_dcps::{
    DataReader, DataReaderQos, DataWriter, DataWriterQos, DdsError, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, Publisher, PublisherQos, RawBytes, Subscriber,
    SubscriberQos, Topic, TopicQos,
};

fn dds_err_to_py(e: DdsError) -> PyErr {
    match e {
        DdsError::Timeout => PyTimeoutError::new_err("dds timeout"),
        other => PyRuntimeError::new_err(format!("{other:?}")),
    }
}

/// Converts a `Vec<Sample<RawBytes>>` into a Python `list[(bytes, SampleInfo)]`.
fn bytes_samples_with_info<'py>(
    py: Python<'py>,
    samples: Vec<Sample<RawBytes>>,
) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for s in samples {
        let info = Py::new(py, PySampleInfo::from_info(&s.info))?;
        let data = PyBytes::new(py, &s.data.data);
        list.append((data, info))?;
    }
    Ok(list)
}

/// Converts a `Vec<Sample<ShapeType>>` into a Python `list[(Shape, SampleInfo)]`.
fn shape_samples_with_info<'py>(
    py: Python<'py>,
    samples: Vec<Sample<ShapeType>>,
) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for s in samples {
        let info = Py::new(py, PySampleInfo::from_info(&s.info))?;
        let shape = Py::new(py, PyShape::from(s.data))?;
        list.append((shape, info))?;
    }
    Ok(list)
}

// =============================================================================
// SampleInfo (Spec §2.2.2.5.1) — read-back metadata incl. instance_state
// =============================================================================

/// Python view of a DDS `SampleInfo` (DDS 1.4 §2.2.2.5.1). Carries the
/// per-sample lifecycle metadata that `take()`/`read()` cannot express
/// through `bytes` alone — most importantly `instance_state`
/// (ALIVE / NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS) and `valid_data`
/// for keyed lifecycle observability.
#[pyclass(name = "SampleInfo", module = "zerodds_py", skip_from_py_object)]
#[derive(Clone)]
struct PySampleInfo {
    /// Instance lifecycle state as a string:
    /// `"Alive"` | `"NotAliveDisposed"` | `"NotAliveNoWriters"`.
    #[pyo3(get)]
    instance_state: &'static str,
    /// Sample state: `"Read"` | `"NotRead"`.
    #[pyo3(get)]
    sample_state: &'static str,
    /// View state: `"New"` | `"NotNew"`.
    #[pyo3(get)]
    view_state: &'static str,
    /// `false` when the sample carries only a dispose/unregister marker
    /// (no payload). Spec §2.2.2.5.1.13.
    #[pyo3(get)]
    valid_data: bool,
    /// Per-instance handle (derived from the key hash). `0` for an
    /// unkeyed type.
    #[pyo3(get)]
    instance_handle: u64,
    /// Handle of the publication (writer) that produced the sample.
    #[pyo3(get)]
    publication_handle: u64,
    /// Source timestamp in seconds (DDS `Time` → f64).
    #[pyo3(get)]
    source_timestamp_secs: f64,
    /// Spec §2.2.2.5.1 generation counters.
    #[pyo3(get)]
    disposed_generation_count: i32,
    /// Spec §2.2.2.5.1 generation counters.
    #[pyo3(get)]
    no_writers_generation_count: i32,
}

impl PySampleInfo {
    fn from_info(info: &zerodds_dcps::sample_info::SampleInfo) -> Self {
        let instance_state = match info.instance_state {
            InstanceStateKind::Alive => "Alive",
            InstanceStateKind::NotAliveDisposed => "NotAliveDisposed",
            InstanceStateKind::NotAliveNoWriters => "NotAliveNoWriters",
        };
        let sample_state = match info.sample_state {
            SampleStateKind::Read => "Read",
            SampleStateKind::NotRead => "NotRead",
        };
        let view_state = match info.view_state {
            ViewStateKind::New => "New",
            ViewStateKind::NotNew => "NotNew",
        };
        Self {
            instance_state,
            sample_state,
            view_state,
            valid_data: info.valid_data,
            instance_handle: info.instance_handle.as_raw(),
            publication_handle: info.publication_handle.as_raw(),
            source_timestamp_secs: f64::from(info.source_timestamp.seconds())
                + f64::from(info.source_timestamp.nanoseconds()) / 1e9,
            disposed_generation_count: info.disposed_generation_count,
            no_writers_generation_count: info.no_writers_generation_count,
        }
    }
}

#[pymethods]
impl PySampleInfo {
    fn __repr__(&self) -> String {
        format!(
            "SampleInfo(instance_state={:?}, valid_data={}, instance_handle={}, sample_state={:?}, view_state={:?})",
            self.instance_state,
            self.valid_data,
            self.instance_handle,
            self.sample_state,
            self.view_state
        )
    }
}

// =============================================================================
// Factory
// =============================================================================

/// Python wrapper around `DomainParticipantFactory`.
#[pyclass(name = "DomainParticipantFactory", module = "zerodds_py")]
struct PyFactory;

#[pymethods]
impl PyFactory {
    /// Singleton.
    #[staticmethod]
    fn instance() -> Self {
        Self
    }

    /// Offline participant (no UDP bind) — ideal for unit tests.
    fn create_participant_offline(&self, domain_id: i32) -> PyParticipant {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(domain_id, DomainParticipantQos::default());
        PyParticipant { inner: p }
    }

    /// Live participant with UDP + SPDP/SEDP.
    fn create_participant(&self, domain_id: i32) -> PyResult<PyParticipant> {
        let p = DomainParticipantFactory::instance()
            .create_participant(domain_id, DomainParticipantQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyParticipant { inner: p })
    }

    /// Live participant with tuned timings (fast discovery).
    fn create_participant_fast(&self, domain_id: i32) -> PyResult<PyParticipant> {
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(domain_id, DomainParticipantQos::default(), cfg)
            .map_err(dds_err_to_py)?;
        Ok(PyParticipant { inner: p })
    }
}

// =============================================================================
// DomainParticipant
// =============================================================================

#[pyclass(name = "DomainParticipant", module = "zerodds_py")]
struct PyParticipant {
    inner: DomainParticipant,
}

#[pymethods]
impl PyParticipant {
    #[getter]
    fn domain_id(&self) -> i32 {
        self.inner.domain_id()
    }

    /// Topic registry size.
    fn topics_len(&self) -> usize {
        self.inner.topics_len()
    }

    /// Number of remote participants discovered via SPDP.
    fn discovered_participants_count(&self) -> usize {
        self.inner.discovered_participants_count()
    }

    /// Bytes topic (type name `zerodds::RawBytes`). For untyped
    /// payload transfer.
    fn create_bytes_topic(&self, name: &str) -> PyResult<PyBytesTopic> {
        let topic = self
            .inner
            .create_topic::<RawBytes>(name, TopicQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyBytesTopic { inner: topic })
    }

    /// ShapesDemo topic (type name `ShapeType`). For interop tests
    /// against Cyclone/Fast-DDS ShapesDemo.
    fn create_shape_topic(&self, name: &str) -> PyResult<PyShapeTopic> {
        let topic = self
            .inner
            .create_topic::<ShapeType>(name, TopicQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyShapeTopic { inner: topic })
    }

    /// Keyed conformance topic (`conformance::Reading`, keyed by `id`).
    /// For behavioral keyed-lifecycle demonstration (dispose/unregister +
    /// `take_with_info` instance_state).
    fn create_keyed_topic(&self, name: &str) -> PyResult<PyKeyedTopic> {
        let topic = self
            .inner
            .create_topic::<KeyedReading>(name, TopicQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyKeyedTopic { inner: topic })
    }

    fn create_publisher(&self) -> PyPublisher {
        PyPublisher {
            inner: Arc::new(self.inner.create_publisher(PublisherQos::default())),
        }
    }

    fn create_subscriber(&self) -> PySubscriber {
        PySubscriber {
            inner: Arc::new(self.inner.create_subscriber(SubscriberQos::default())),
        }
    }

    /// Asserts liveliness for all manual_by_participant writers.
    fn assert_liveliness(&self) {
        if let Some(rt) = self.inner.runtime() {
            rt.assert_liveliness();
        }
    }

    /// Ignore a remote participant by InstanceHandle (Spec §2.2.2.2.1.14).
    fn ignore_participant(&self, handle: u64) -> PyResult<()> {
        self.inner
            .ignore_participant(zerodds_dcps::instance_handle::InstanceHandle::from_raw(
                handle,
            ))
            .map_err(dds_err_to_py)
    }
    /// Ignore topic.
    fn ignore_topic(&self, handle: u64) -> PyResult<()> {
        self.inner
            .ignore_topic(zerodds_dcps::instance_handle::InstanceHandle::from_raw(
                handle,
            ))
            .map_err(dds_err_to_py)
    }
    /// Ignore publication.
    fn ignore_publication(&self, handle: u64) -> PyResult<()> {
        self.inner
            .ignore_publication(zerodds_dcps::instance_handle::InstanceHandle::from_raw(
                handle,
            ))
            .map_err(dds_err_to_py)
    }
    /// Ignore subscription.
    fn ignore_subscription(&self, handle: u64) -> PyResult<()> {
        self.inner
            .ignore_subscription(zerodds_dcps::instance_handle::InstanceHandle::from_raw(
                handle,
            ))
            .map_err(dds_err_to_py)
    }
    /// `contains_entity(handle)` — Spec §2.2.2.2.1.10.
    fn contains_entity(&self, handle: u64) -> bool {
        self.inner
            .contains_entity(zerodds_dcps::instance_handle::InstanceHandle::from_raw(
                handle,
            ))
    }
    /// Returns the InstanceHandles of all discovered topics.
    fn get_discovered_topics(&self) -> Vec<u64> {
        self.inner
            .get_discovered_topics()
            .into_iter()
            .map(|h| h.as_raw())
            .collect()
    }
    /// Returns the InstanceHandles of all discovered participants.
    fn get_discovered_participants(&self) -> Vec<u64> {
        self.inner
            .get_discovered_participants()
            .into_iter()
            .map(|h| h.as_raw())
            .collect()
    }

    /// Spec §2.2.2.2.1.13 `create_contentfilteredtopic`. Creates a
    /// content-filtered view of a `BytesTopic`.
    ///
    /// The Python binding accepts a *callable predicate* `bytes -> bool`
    /// (more idiomatic than a SQL expression over an unkeyed/opaque
    /// `RawBytes` payload, which carries no named fields). The predicate
    /// is evaluated on every sample in the reader's `take()`/`read()`
    /// path; returning `False` discards the sample (Spec §2.2.2.5.4).
    fn create_bytes_contentfilteredtopic(
        &self,
        name: &str,
        related_topic: &PyBytesTopic,
        predicate: Py<PyAny>,
    ) -> PyResult<PyBytesContentFilteredTopic> {
        if name.is_empty() {
            return Err(PyRuntimeError::new_err("content-filtered-topic name empty"));
        }
        Ok(PyBytesContentFilteredTopic {
            name: name.to_string(),
            related: related_topic.inner.clone(),
            predicate,
        })
    }
}

// =============================================================================
// ContentFilteredTopic (Spec §2.2.2.3.3 / §2.2.2.5.4)
// =============================================================================

/// Python content-filtered topic over a `BytesTopic`. Holds the related
/// topic and a Python predicate `bytes -> bool` that the reader applies
/// to every sample (DDS 1.4 §2.2.2.5.4).
#[pyclass(name = "BytesContentFilteredTopic", module = "zerodds_py")]
struct PyBytesContentFilteredTopic {
    name: String,
    related: Topic<RawBytes>,
    predicate: Py<PyAny>,
}

#[pymethods]
impl PyBytesContentFilteredTopic {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
    #[getter]
    fn related_topic_name(&self) -> String {
        self.related.name().to_string()
    }
    #[getter]
    fn type_name(&self) -> &'static str {
        <RawBytes as zerodds_dcps::DdsType>::TYPE_NAME
    }
}

// =============================================================================
// Topics
// =============================================================================

#[pyclass(name = "BytesTopic", module = "zerodds_py")]
struct PyBytesTopic {
    inner: Topic<RawBytes>,
}

#[pymethods]
impl PyBytesTopic {
    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }
    #[getter]
    fn type_name(&self) -> &'static str {
        <RawBytes as zerodds_dcps::DdsType>::TYPE_NAME
    }
}

#[pyclass(name = "ShapeTopic", module = "zerodds_py")]
struct PyShapeTopic {
    inner: Topic<ShapeType>,
}

#[pymethods]
impl PyShapeTopic {
    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }
    #[getter]
    fn type_name(&self) -> &'static str {
        <ShapeType as zerodds_dcps::DdsType>::TYPE_NAME
    }
}

// =============================================================================
// Publisher + Subscriber
// =============================================================================

#[pyclass(name = "Publisher", module = "zerodds_py")]
struct PyPublisher {
    inner: Arc<Publisher>,
}

#[pymethods]
impl PyPublisher {
    fn create_bytes_writer(&self, topic: &PyBytesTopic) -> PyResult<PyBytesWriter> {
        let w = self
            .inner
            .create_datawriter::<RawBytes>(&topic.inner, DataWriterQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyBytesWriter { inner: Arc::new(w) })
    }

    fn create_shape_writer(&self, topic: &PyShapeTopic) -> PyResult<PyShapeWriter> {
        let w = self
            .inner
            .create_datawriter::<ShapeType>(&topic.inner, DataWriterQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyShapeWriter { inner: Arc::new(w) })
    }

    /// §6.2 — explicit QoS selection. Spec: DDS 1.4 §2.2.2.4.1.5.
    fn create_bytes_writer_with_qos(
        &self,
        topic: &PyBytesTopic,
        qos: &crate::qos::PyDataWriterQos,
    ) -> PyResult<PyBytesWriter> {
        let w = self
            .inner
            .create_datawriter::<RawBytes>(&topic.inner, qos.cloned_inner())
            .map_err(dds_err_to_py)?;
        Ok(PyBytesWriter { inner: Arc::new(w) })
    }

    /// §6.2 — explicit QoS selection for ShapeType topics.
    fn create_shape_writer_with_qos(
        &self,
        topic: &PyShapeTopic,
        qos: &crate::qos::PyDataWriterQos,
    ) -> PyResult<PyShapeWriter> {
        let w = self
            .inner
            .create_datawriter::<ShapeType>(&topic.inner, qos.cloned_inner())
            .map_err(dds_err_to_py)?;
        Ok(PyShapeWriter { inner: Arc::new(w) })
    }

    /// Keyed conformance writer (`conformance::Reading`).
    fn create_keyed_writer(&self, topic: &PyKeyedTopic) -> PyResult<PyKeyedWriter> {
        let w = self
            .inner
            .create_datawriter::<KeyedReading>(&topic.inner, DataWriterQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyKeyedWriter { inner: Arc::new(w) })
    }

    /// Keyed conformance writer with explicit QoS.
    fn create_keyed_writer_with_qos(
        &self,
        topic: &PyKeyedTopic,
        qos: &crate::qos::PyDataWriterQos,
    ) -> PyResult<PyKeyedWriter> {
        let w = self
            .inner
            .create_datawriter::<KeyedReading>(&topic.inner, qos.cloned_inner())
            .map_err(dds_err_to_py)?;
        Ok(PyKeyedWriter { inner: Arc::new(w) })
    }
}

#[pyclass(name = "Subscriber", module = "zerodds_py")]
struct PySubscriber {
    inner: Arc<Subscriber>,
}

#[pymethods]
impl PySubscriber {
    fn create_bytes_reader(&self, topic: &PyBytesTopic) -> PyResult<PyBytesReader> {
        let r = self
            .inner
            .create_datareader::<RawBytes>(&topic.inner, DataReaderQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyBytesReader { inner: Arc::new(r) })
    }

    fn create_shape_reader(&self, topic: &PyShapeTopic) -> PyResult<PyShapeReader> {
        let r = self
            .inner
            .create_datareader::<ShapeType>(&topic.inner, DataReaderQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyShapeReader { inner: Arc::new(r) })
    }

    /// §6.2 — explicit QoS selection. Spec: DDS 1.4 §2.2.2.5.1.5.
    fn create_bytes_reader_with_qos(
        &self,
        topic: &PyBytesTopic,
        qos: &crate::qos::PyDataReaderQos,
    ) -> PyResult<PyBytesReader> {
        let r = self
            .inner
            .create_datareader::<RawBytes>(&topic.inner, qos.cloned_inner())
            .map_err(dds_err_to_py)?;
        Ok(PyBytesReader { inner: Arc::new(r) })
    }

    /// §6.2 — explicit QoS selection for ShapeType topics.
    fn create_shape_reader_with_qos(
        &self,
        topic: &PyShapeTopic,
        qos: &crate::qos::PyDataReaderQos,
    ) -> PyResult<PyShapeReader> {
        let r = self
            .inner
            .create_datareader::<ShapeType>(&topic.inner, qos.cloned_inner())
            .map_err(dds_err_to_py)?;
        Ok(PyShapeReader { inner: Arc::new(r) })
    }

    /// Spec §2.2.2.5.4 — creates a DataReader on a
    /// `BytesContentFilteredTopic`. The reader applies the topic's Python
    /// predicate to every sample in `take()`/`read()`; samples for which
    /// the predicate returns `False` are discarded.
    fn create_bytes_reader_cft(
        &self,
        cft: &PyBytesContentFilteredTopic,
    ) -> PyResult<PyBytesReader> {
        self.create_bytes_reader_cft_with_qos_impl(cft, DataReaderQos::default())
    }

    /// As `create_bytes_reader_cft`, with explicit DataReaderQos.
    fn create_bytes_reader_cft_with_qos(
        &self,
        cft: &PyBytesContentFilteredTopic,
        qos: &crate::qos::PyDataReaderQos,
    ) -> PyResult<PyBytesReader> {
        self.create_bytes_reader_cft_with_qos_impl(cft, qos.cloned_inner())
    }

    /// Keyed conformance reader (`conformance::Reading`).
    fn create_keyed_reader(&self, topic: &PyKeyedTopic) -> PyResult<PyKeyedReader> {
        let r = self
            .inner
            .create_datareader::<KeyedReading>(&topic.inner, DataReaderQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyKeyedReader { inner: Arc::new(r) })
    }

    /// Keyed conformance reader with explicit QoS.
    fn create_keyed_reader_with_qos(
        &self,
        topic: &PyKeyedTopic,
        qos: &crate::qos::PyDataReaderQos,
    ) -> PyResult<PyKeyedReader> {
        let r = self
            .inner
            .create_datareader::<KeyedReading>(&topic.inner, qos.cloned_inner())
            .map_err(dds_err_to_py)?;
        Ok(PyKeyedReader { inner: Arc::new(r) })
    }
}

impl PySubscriber {
    fn create_bytes_reader_cft_with_qos_impl(
        &self,
        cft: &PyBytesContentFilteredTopic,
        qos: DataReaderQos,
    ) -> PyResult<PyBytesReader> {
        // Clone the Python predicate into the filter closure. The closure
        // re-acquires the GIL on each sample (it runs on the take path,
        // where the GIL was released via `py.detach`).
        let predicate: Py<PyAny> = Python::attach(|py| cft.predicate.clone_ref(py));
        let r = self
            .inner
            .create_datareader::<RawBytes>(&cft.related, qos)
            .map_err(dds_err_to_py)?
            .with_filter(move |sample: &RawBytes| {
                Python::attach(|py| {
                    let arg = PyBytes::new(py, &sample.data);
                    match predicate.call1(py, (arg,)) {
                        Ok(res) => res.is_truthy(py).unwrap_or(true),
                        // A predicate that raises is treated as "pass"
                        // (fail-open) to avoid silently dropping data.
                        Err(_) => true,
                    }
                })
            });
        Ok(PyBytesReader { inner: Arc::new(r) })
    }
}

// =============================================================================
// Bytes-Writer / -Reader
// =============================================================================

#[pyclass(name = "BytesWriter", module = "zerodds_py")]
struct PyBytesWriter {
    inner: Arc<DataWriter<RawBytes>>,
}

#[pymethods]
impl PyBytesWriter {
    /// Writes bytes as a sample. The GIL is released during the send.
    fn write(&self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        let sample = RawBytes::new(data.to_vec());
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.write(&sample)).map_err(dds_err_to_py)
    }

    fn wait_for_matched_subscription(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            writer.wait_for_matched_subscription(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }

    fn matched_subscription_count(&self) -> usize {
        self.inner.matched_subscription_count()
    }

    /// Spec §2.2.2.4.2.5 `register_instance`. `RawBytes` is an unkeyed
    /// type (`HAS_KEY=false`), so there is a single notional instance and
    /// this returns `HANDLE_NIL` (0). The method exists for spec-shaped
    /// API parity; keyed lifecycle is exercised via `ShapeWriter`.
    fn register_instance(&self, py: Python<'_>, data: &[u8]) -> PyResult<u64> {
        let sample = RawBytes::new(data.to_vec());
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.register_instance(&sample))
            .map(|h| h.as_raw())
            .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.14 `lookup_instance`. Returns `HANDLE_NIL` (0) for
    /// the unkeyed `RawBytes` type.
    fn lookup_instance(&self, py: Python<'_>, data: &[u8]) -> u64 {
        let sample = RawBytes::new(data.to_vec());
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.lookup_instance(&sample)).as_raw()
    }

    /// Spec §2.2.2.4.2.10 `dispose`. Sends a wire-lifecycle marker; for a
    /// keyed type readers then observe `NOT_ALIVE_DISPOSED`. `RawBytes` is
    /// unkeyed, so this is a no-op marker (no per-instance state). Use
    /// `ShapeWriter.dispose` for behaviorally observable keyed disposal.
    fn dispose(&self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        let sample = RawBytes::new(data.to_vec());
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            let handle = writer.lookup_instance(&sample);
            writer.dispose(&sample, handle)
        })
        .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`. No-op for unkeyed
    /// `RawBytes`; see `ShapeWriter.unregister_instance` for keyed
    /// behavior (NOT_ALIVE_NO_WRITERS).
    fn unregister_instance(&self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        let sample = RawBytes::new(data.to_vec());
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            let handle = writer.lookup_instance(&sample);
            writer.unregister_instance(&sample, handle)
        })
        .map_err(dds_err_to_py)
    }
    // NOTE: `RawBytes` is unkeyed → `lookup_instance` is `HANDLE_NIL` and
    // dispose/unregister are no-op markers (the unkeyed DCPS path accepts a
    // nil handle without error). Keyed lifecycle is exercised via
    // `ShapeWriter` (keyed by color).

    /// `get_publication_matched_status` — (current_count, current_count, last_handle=0).
    /// RC1: ZeroDDS-DCPS exposes only `matched_subscription_count`; total/change maintenance follows.
    fn publication_matched_status(&self) -> (i32, i32, i32, i32, u64) {
        let n = self.inner.matched_subscription_count() as i32;
        (n, 0, n, 0, 0)
    }
    /// `get_liveliness_lost_status` — (count, 0).
    fn liveliness_lost_status(&self) -> (i32, i32) {
        (self.inner.liveliness_lost_count() as i32, 0)
    }
    /// `get_offered_deadline_missed_status` — (count, 0).
    fn offered_deadline_missed_status(&self) -> (i32, i32) {
        (self.inner.offered_deadline_missed_count() as i32, 0)
    }

    /// §6.5 — attach a listener with all status kinds.
    /// `mask` = StatusMask (u32, DDS 1.4 §2.2.4.1). 0 = all bits active.
    fn set_listener(&self, listener: &crate::listener::PyDataWriterListener, mask: u32) {
        let bridge = crate::listener::PyDataWriterListenerBridge::from_pyclass(listener);
        self.inner.set_listener(Some(bridge), mask);
    }

    /// §6.5 — clear the listener slot (Spec §2.2.2.4.2.x).
    fn clear_listener(&self) {
        self.inner.set_listener(None, 0);
    }
}

#[pyclass(name = "BytesReader", module = "zerodds_py")]
struct PyBytesReader {
    inner: Arc<DataReader<RawBytes>>,
}

#[pymethods]
impl PyBytesReader {
    /// Takes all available samples as `list[bytes]`.
    fn take<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py.detach(|| reader.take()).map_err(dds_err_to_py)?;
        let list = PyList::empty(py);
        for s in samples {
            list.append(PyBytes::new(py, &s.data))?;
        }
        Ok(list)
    }

    /// `take_with_info` — Spec §2.2.2.5.3.5. Returns
    /// `list[(bytes, SampleInfo)]` so that per-sample lifecycle metadata
    /// (most importantly `instance_state` and `valid_data`) is observable
    /// from Python — required to read back keyed dispose/unregister
    /// markers (NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS).
    fn take_with_info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py
            .detach(|| reader.take_with_info())
            .map_err(dds_err_to_py)?;
        bytes_samples_with_info(py, samples)
    }

    /// `read_with_info` — Spec §2.2.2.5.3.4. Non-consuming variant of
    /// `take_with_info`.
    fn read_with_info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py
            .detach(|| reader.read_with_info())
            .map_err(dds_err_to_py)?;
        bytes_samples_with_info(py, samples)
    }

    fn wait_for_data(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.detach(|| reader.wait_for_data(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
    }

    fn wait_for_matched_publication(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.detach(|| {
            reader.wait_for_matched_publication(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }

    fn matched_publication_count(&self) -> usize {
        self.inner.matched_publication_count()
    }

    /// `get_subscription_matched_status` — (current_count, 0, current, 0, last=0).
    fn subscription_matched_status(&self) -> (i32, i32, i32, i32, u64) {
        let n = self.inner.matched_publication_count() as i32;
        (n, 0, n, 0, 0)
    }
    /// `get_sample_lost_status` — (count, 0).
    fn sample_lost_status(&self) -> (i32, i32) {
        (self.inner.sample_lost_count() as i32, 0)
    }
    /// `get_requested_deadline_missed_status` — (count, 0).
    fn requested_deadline_missed_status(&self) -> (i32, i32) {
        (self.inner.requested_deadline_missed_count() as i32, 0)
    }
    /// `get_liveliness_changed_status` — Spec §2.2.4.2.14
    /// `LIVELINESS_CHANGED_STATUS`. Returns
    /// `(alive_now: bool, alive_count: int, not_alive_count: int)`.
    fn liveliness_changed_status(&self) -> (bool, u64, u64) {
        self.inner.liveliness_changed_status()
    }

    /// §6.5 — attach a listener with all status kinds.
    fn set_listener(&self, listener: &crate::listener::PyDataReaderListener, mask: u32) {
        let bridge = crate::listener::PyDataReaderListenerBridge::from_pyclass(listener);
        self.inner.set_listener(Some(bridge), mask);
    }

    /// §6.5 — clear the listener slot.
    fn clear_listener(&self) {
        self.inner.set_listener(None, 0);
    }
}

// =============================================================================
// Shape writer / reader (for ShapesDemo interop)
// =============================================================================

/// Python repr of a ShapeType instance. A plain dict would also be
/// possible, but a dedicated class makes `isinstance` checks and
/// getter/setter cleaner.
#[pyclass(name = "Shape", module = "zerodds_py", from_py_object)]
#[derive(Clone)]
struct PyShape {
    /// Color / instance key.
    #[pyo3(get, set)]
    color: String,
    /// X coordinate.
    #[pyo3(get, set)]
    x: i32,
    /// Y coordinate.
    #[pyo3(get, set)]
    y: i32,
    /// Size in pixels.
    #[pyo3(get, set)]
    shapesize: i32,
}

#[pymethods]
impl PyShape {
    #[new]
    #[pyo3(signature = (color, x, y, shapesize=30))]
    fn new(color: String, x: i32, y: i32, shapesize: i32) -> Self {
        Self {
            color,
            x,
            y,
            shapesize,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Shape(color={:?}, x={}, y={}, shapesize={})",
            self.color, self.x, self.y, self.shapesize
        )
    }
}

impl From<&PyShape> for ShapeType {
    fn from(s: &PyShape) -> Self {
        ShapeType::new(&*s.color, s.x, s.y, s.shapesize)
    }
}

impl From<ShapeType> for PyShape {
    fn from(s: ShapeType) -> Self {
        Self {
            color: s.color,
            x: s.x,
            y: s.y,
            shapesize: s.shapesize,
        }
    }
}

#[pyclass(name = "ShapeWriter", module = "zerodds_py")]
struct PyShapeWriter {
    inner: Arc<DataWriter<ShapeType>>,
}

#[pymethods]
impl PyShapeWriter {
    fn write(&self, py: Python<'_>, shape: &PyShape) -> PyResult<()> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.write(&sample)).map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.5 `register_instance`. Returns an
    /// instance handle that can later be used for dispose/unregister.
    fn register_instance(&self, py: Python<'_>, shape: &PyShape) -> PyResult<u64> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.register_instance(&sample))
            .map(|h| h.as_raw())
            .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.10 `dispose`. Sends a wire-lifecycle
    /// marker — readers see the instance as `NotAliveDisposed`.
    ///
    /// Spec §2.2.2.4.2.4: `write` implicitly registers the instance, but
    /// the DCPS live `write` path is fire-and-forget and skips the
    /// instance tracker. We therefore register the instance here if it is
    /// not yet known, so `dispose` always resolves a valid handle.
    fn dispose(&self, py: Python<'_>, shape: &PyShape) -> PyResult<()> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            let mut handle = writer.lookup_instance(&sample);
            if handle.is_nil() {
                handle = writer.register_instance(&sample)?;
            }
            writer.dispose(&sample, handle)
        })
        .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`. With
    /// WriterDataLifecycle.autodispose=true (default), implicitly also
    /// disposes.
    fn unregister_instance(&self, py: Python<'_>, shape: &PyShape) -> PyResult<()> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            let mut handle = writer.lookup_instance(&sample);
            if handle.is_nil() {
                handle = writer.register_instance(&sample)?;
            }
            writer.unregister_instance(&sample, handle)
        })
        .map_err(dds_err_to_py)
    }

    fn wait_for_matched_subscription(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            writer.wait_for_matched_subscription(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }

    /// §6.5 — attach a listener with all status kinds.
    fn set_listener(&self, listener: &crate::listener::PyDataWriterListener, mask: u32) {
        let bridge = crate::listener::PyDataWriterListenerBridge::from_pyclass(listener);
        self.inner.set_listener(Some(bridge), mask);
    }

    fn clear_listener(&self) {
        self.inner.set_listener(None, 0);
    }
}

#[pyclass(name = "ShapeReader", module = "zerodds_py")]
struct PyShapeReader {
    inner: Arc<DataReader<ShapeType>>,
}

#[pymethods]
impl PyShapeReader {
    fn take<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py.detach(|| reader.take()).map_err(dds_err_to_py)?;
        let list = PyList::empty(py);
        for s in samples {
            list.append(Py::new(py, PyShape::from(s))?)?;
        }
        Ok(list)
    }

    /// `take_with_info` — Spec §2.2.2.5.3.5. Returns
    /// `list[(Shape, SampleInfo)]`. For the keyed `ShapeType`,
    /// `info.instance_state` reflects the per-color instance lifecycle —
    /// e.g. `NotAliveDisposed` after `ShapeWriter.dispose`, with
    /// `info.valid_data == false` for the pure marker.
    fn take_with_info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py
            .detach(|| reader.take_with_info())
            .map_err(dds_err_to_py)?;
        shape_samples_with_info(py, samples)
    }

    /// `read_with_info` — Spec §2.2.2.5.3.4. Non-consuming variant.
    fn read_with_info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py
            .detach(|| reader.read_with_info())
            .map_err(dds_err_to_py)?;
        shape_samples_with_info(py, samples)
    }

    fn wait_for_data(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.detach(|| reader.wait_for_data(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
    }

    fn wait_for_matched_publication(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.detach(|| {
            reader.wait_for_matched_publication(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }

    /// `get_liveliness_changed_status` — Spec §2.2.4.2.14.
    /// `(alive_now: bool, alive_count: int, not_alive_count: int)`.
    fn liveliness_changed_status(&self) -> (bool, u64, u64) {
        self.inner.liveliness_changed_status()
    }

    /// §6.5 — attach a listener with all status kinds.
    fn set_listener(&self, listener: &crate::listener::PyDataReaderListener, mask: u32) {
        let bridge = crate::listener::PyDataReaderListenerBridge::from_pyclass(listener);
        self.inner.set_listener(Some(bridge), mask);
    }

    fn clear_listener(&self) {
        self.inner.set_listener(None, 0);
    }
}

// =============================================================================
// KeyedReading — a binding-local keyed DdsType for behavioral keyed-lifecycle
// =============================================================================
//
// Mirrors the conformance IDL `Reading { @key long id; long seq; double value; }`.
// Provided so the Python binding can behaviorally demonstrate keyed lifecycle
// (dispose → NOT_ALIVE_DISPOSED, unregister → NOT_ALIVE_NO_WRITERS) with a wire
// codec whose `decode` is *key-only tolerant* — i.e. it reconstructs the value
// from the BE key-holder bytes the runtime stores for a lifecycle marker
// (Spec §2.2.2.5.1.13). The `ShapeType` codec is not key-tolerant, so markers
// for that type cannot be materialized on the reader.

/// Keyed conformance type: `Reading { @key long id; long seq; double value }`.
/// Wire form (self-consistent, big-endian): `u32 id | u32 seq | f64 value`.
#[pyclass(name = "KeyedReading", module = "zerodds_py", from_py_object)]
#[derive(Clone)]
struct PyKeyedReading {
    /// `@key long id` — the instance key.
    #[pyo3(get, set)]
    id: i32,
    /// `long seq`.
    #[pyo3(get, set)]
    seq: i32,
    /// `double value`.
    #[pyo3(get, set)]
    value: f64,
}

#[pymethods]
impl PyKeyedReading {
    #[new]
    #[pyo3(signature = (id, seq=0, value=0.0))]
    fn new(id: i32, seq: i32, value: f64) -> Self {
        Self { id, seq, value }
    }
    fn __repr__(&self) -> String {
        format!(
            "KeyedReading(id={}, seq={}, value={})",
            self.id, self.seq, self.value
        )
    }
}

/// Rust DdsType backing `PyKeyedReading`.
#[derive(Clone)]
struct KeyedReading {
    id: i32,
    seq: i32,
    value: f64,
}

impl zerodds_dcps::DdsType for KeyedReading {
    const TYPE_NAME: &'static str = "conformance::Reading";
    const HAS_KEY: bool = true;
    // `@key long id` → 4-byte BE key holder → zero-pad KeyHash path.
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(
        &self,
        out: &mut Vec<u8>,
    ) -> core::result::Result<(), zerodds_dcps::dds_type::EncodeError> {
        out.extend_from_slice(&(self.id as u32).to_be_bytes());
        out.extend_from_slice(&(self.seq as u32).to_be_bytes());
        out.extend_from_slice(&self.value.to_bits().to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, zerodds_dcps::dds_type::DecodeError> {
        // Key-only tolerant (Spec §2.2.2.5.1.13): a lifecycle marker holder
        // carries just the 4-byte BE `id`; full samples carry all 16 bytes.
        if bytes.len() < 4 {
            return Err(zerodds_dcps::dds_type::DecodeError::Invalid {
                what: "KeyedReading: truncated key",
            });
        }
        let id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i32;
        let seq = if bytes.len() >= 8 {
            u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as i32
        } else {
            0
        };
        let value = if bytes.len() >= 16 {
            f64::from_bits(u64::from_be_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ]))
        } else {
            0.0
        };
        Ok(Self { id, seq, value })
    }

    fn encode_key_holder_be(&self, holder: &mut zerodds_dcps::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id as u32);
    }
}

impl From<&PyKeyedReading> for KeyedReading {
    fn from(r: &PyKeyedReading) -> Self {
        Self {
            id: r.id,
            seq: r.seq,
            value: r.value,
        }
    }
}

impl From<KeyedReading> for PyKeyedReading {
    fn from(r: KeyedReading) -> Self {
        Self {
            id: r.id,
            seq: r.seq,
            value: r.value,
        }
    }
}

#[pyclass(name = "KeyedTopic", module = "zerodds_py")]
struct PyKeyedTopic {
    inner: Topic<KeyedReading>,
}

#[pymethods]
impl PyKeyedTopic {
    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }
    #[getter]
    fn type_name(&self) -> &'static str {
        <KeyedReading as zerodds_dcps::DdsType>::TYPE_NAME
    }
}

#[pyclass(name = "KeyedWriter", module = "zerodds_py")]
struct PyKeyedWriter {
    inner: Arc<DataWriter<KeyedReading>>,
}

#[pymethods]
impl PyKeyedWriter {
    fn write(&self, py: Python<'_>, sample: &PyKeyedReading) -> PyResult<()> {
        let s: KeyedReading = sample.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.write(&s)).map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.5 `register_instance` — returns the per-key handle.
    fn register_instance(&self, py: Python<'_>, sample: &PyKeyedReading) -> PyResult<u64> {
        let s: KeyedReading = sample.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.register_instance(&s))
            .map(|h| h.as_raw())
            .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.14 `lookup_instance`.
    fn lookup_instance(&self, py: Python<'_>, sample: &PyKeyedReading) -> u64 {
        let s: KeyedReading = sample.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| writer.lookup_instance(&s)).as_raw()
    }

    /// Spec §2.2.2.4.2.10 `dispose` → readers observe `NotAliveDisposed`.
    fn dispose(&self, py: Python<'_>, sample: &PyKeyedReading) -> PyResult<()> {
        let s: KeyedReading = sample.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            let mut handle = writer.lookup_instance(&s);
            if handle.is_nil() {
                handle = writer.register_instance(&s)?;
            }
            writer.dispose(&s, handle)
        })
        .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`. With the default
    /// WriterDataLifecycle.autodispose=true, also disposes.
    fn unregister_instance(&self, py: Python<'_>, sample: &PyKeyedReading) -> PyResult<()> {
        let s: KeyedReading = sample.into();
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            let mut handle = writer.lookup_instance(&s);
            if handle.is_nil() {
                handle = writer.register_instance(&s)?;
            }
            writer.unregister_instance(&s, handle)
        })
        .map_err(dds_err_to_py)
    }

    fn wait_for_matched_subscription(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let writer = Arc::clone(&self.inner);
        py.detach(|| {
            writer.wait_for_matched_subscription(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }
}

#[pyclass(name = "KeyedReader", module = "zerodds_py")]
struct PyKeyedReader {
    inner: Arc<DataReader<KeyedReading>>,
}

#[pymethods]
impl PyKeyedReader {
    /// `take_with_info` — returns `list[(KeyedReading, SampleInfo)]`.
    /// `info.instance_state` reflects the per-key lifecycle:
    /// `Alive` / `NotAliveDisposed` / `NotAliveNoWriters`.
    fn take_with_info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py
            .detach(|| reader.take_with_info())
            .map_err(dds_err_to_py)?;
        let list = PyList::empty(py);
        for s in samples {
            let info = Py::new(py, PySampleInfo::from_info(&s.info))?;
            let data = Py::new(py, PyKeyedReading::from(s.data))?;
            list.append((data, info))?;
        }
        Ok(list)
    }

    fn wait_for_data(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.detach(|| reader.wait_for_data(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
    }

    fn wait_for_matched_publication(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.detach(|| {
            reader.wait_for_matched_publication(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }

    /// `get_liveliness_changed_status` — Spec §2.2.4.2.14.
    fn liveliness_changed_status(&self) -> (bool, u64, u64) {
        self.inner.liveliness_changed_status()
    }
}

// =============================================================================
// Module registration
// =============================================================================

// =============================================================================
// Conditions + WaitSet (Spec §2.2.2.1.7)
// =============================================================================

/// Python wrapper around `GuardCondition`.
#[pyclass(name = "GuardCondition", module = "zerodds_py")]
struct PyGuardCondition {
    inner: Arc<GuardCondition>,
}

#[pymethods]
impl PyGuardCondition {
    /// Creates a GuardCondition with an initial trigger=false.
    #[new]
    fn new() -> Self {
        Self {
            inner: GuardCondition::new(),
        }
    }
    /// Sets the trigger value.
    fn set_trigger_value(&self, value: bool) {
        self.inner.set_trigger_value(value);
    }
    /// Reads the current trigger value.
    fn get_trigger_value(&self) -> bool {
        use zerodds_dcps::condition::Condition;
        self.inner.get_trigger_value()
    }
}

/// Python wrapper around `WaitSet`.
#[pyclass(name = "WaitSet", module = "zerodds_py")]
struct PyWaitSet {
    inner: WaitSet,
    attached: std::sync::Mutex<Vec<Arc<GuardCondition>>>,
}

#[pymethods]
impl PyWaitSet {
    #[new]
    fn new() -> Self {
        Self {
            inner: WaitSet::new(),
            attached: std::sync::Mutex::new(Vec::new()),
        }
    }
    /// Attach a GuardCondition.
    fn attach_guard_condition(&self, gc: &PyGuardCondition) -> PyResult<()> {
        let cond: Arc<dyn zerodds_dcps::condition::Condition> =
            Arc::clone(&gc.inner) as Arc<dyn zerodds_dcps::condition::Condition>;
        self.inner.attach_condition(cond).map_err(dds_err_to_py)?;
        if let Ok(mut g) = self.attached.lock() {
            g.push(Arc::clone(&gc.inner));
        }
        Ok(())
    }

    /// §6.6 — attach a ReadCondition.
    fn attach_read_condition(&self, rc: &crate::conditions::PyReadCondition) -> PyResult<()> {
        let cond: Arc<dyn zerodds_dcps::condition::Condition> =
            Arc::clone(&rc.inner) as Arc<dyn zerodds_dcps::condition::Condition>;
        self.inner.attach_condition(cond).map_err(dds_err_to_py)
    }

    /// §6.6 — attach a QueryCondition.
    fn attach_query_condition(&self, qc: &crate::conditions::PyQueryCondition) -> PyResult<()> {
        let cond: Arc<dyn zerodds_dcps::condition::Condition> =
            Arc::clone(&qc.inner) as Arc<dyn zerodds_dcps::condition::Condition>;
        self.inner.attach_condition(cond).map_err(dds_err_to_py)
    }

    /// Wait — returns the number of triggered conditions.
    fn wait(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<usize> {
        // WaitSet is not Clone — use inner directly.
        py.detach(|| self.inner.wait(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
            .map(|v| v.len())
    }
}

/// PyO3 module `zerodds._core`. The outer Python package `zerodds`
/// re-exports from it (see `python/zerodds/__init__.py`).
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactory>()?;
    m.add_class::<PyParticipant>()?;
    m.add_class::<PyBytesTopic>()?;
    m.add_class::<PyShapeTopic>()?;
    m.add_class::<PyPublisher>()?;
    m.add_class::<PySubscriber>()?;
    m.add_class::<PyBytesWriter>()?;
    m.add_class::<PyBytesReader>()?;
    m.add_class::<PyShape>()?;
    m.add_class::<PyShapeWriter>()?;
    m.add_class::<PyShapeReader>()?;
    m.add_class::<PySampleInfo>()?;
    m.add_class::<PyBytesContentFilteredTopic>()?;
    m.add_class::<PyKeyedReading>()?;
    m.add_class::<PyKeyedTopic>()?;
    m.add_class::<PyKeyedWriter>()?;
    m.add_class::<PyKeyedReader>()?;
    m.add_class::<PyGuardCondition>()?;
    m.add_class::<PyWaitSet>()?;
    m.add_class::<crate::qos::PyDataWriterQos>()?;
    m.add_class::<crate::qos::PyDataReaderQos>()?;
    m.add_class::<crate::listener::PyDataWriterListener>()?;
    m.add_class::<crate::listener::PyDataReaderListener>()?;
    m.add_class::<crate::conditions::PyReadCondition>()?;
    m.add_class::<crate::conditions::PyQueryCondition>()?;
    // §6.6 — SampleState/ViewState/InstanceState bitmask constants
    // (DDS 1.4 §2.2.2.5.1.4). Exposed as module attributes so that
    // users can use `zerodds._core.SAMPLE_STATE_ANY` etc.
    m.add(
        "SAMPLE_STATE_NOT_READ",
        zerodds_dcps::sample_info::sample_state_mask::NOT_READ,
    )?;
    m.add(
        "SAMPLE_STATE_READ",
        zerodds_dcps::sample_info::sample_state_mask::READ,
    )?;
    m.add(
        "SAMPLE_STATE_ANY",
        zerodds_dcps::sample_info::sample_state_mask::ANY,
    )?;
    m.add(
        "VIEW_STATE_NEW",
        zerodds_dcps::sample_info::view_state_mask::NEW,
    )?;
    m.add(
        "VIEW_STATE_NOT_NEW",
        zerodds_dcps::sample_info::view_state_mask::NOT_NEW,
    )?;
    m.add(
        "VIEW_STATE_ANY",
        zerodds_dcps::sample_info::view_state_mask::ANY,
    )?;
    m.add(
        "INSTANCE_STATE_ALIVE",
        zerodds_dcps::sample_info::instance_state_mask::ALIVE,
    )?;
    m.add(
        "INSTANCE_STATE_NOT_ALIVE_DISPOSED",
        zerodds_dcps::sample_info::instance_state_mask::NOT_ALIVE_DISPOSED,
    )?;
    m.add(
        "INSTANCE_STATE_NOT_ALIVE_NO_WRITERS",
        zerodds_dcps::sample_info::instance_state_mask::NOT_ALIVE_NO_WRITERS,
    )?;
    m.add(
        "INSTANCE_STATE_NOT_ALIVE",
        zerodds_dcps::sample_info::instance_state_mask::NOT_ALIVE,
    )?;
    m.add(
        "INSTANCE_STATE_ANY",
        zerodds_dcps::sample_info::instance_state_mask::ANY,
    )?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
