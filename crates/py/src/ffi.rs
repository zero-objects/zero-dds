// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PyO3-FFI-Modul. Nur mit `--features extension-module` aktiv.
//!
//! Aktueller Scope: Raw-Bytes-Topic. Dies ist absichtlich ein dünnes
//! Wrapper-Layer — die schwere Arbeit (QoS, Discovery, Wire) lebt in
//! `zerodds-dcps`. Keine Python-spezifische Logik hier ausser Konversion
//! Rust↔Python.
//!
//! # SAFETY
//!
//! Die `unsafe`-Expansions von `#[pymodule]` / `#[pyclass]` sind
//! PyO3-Makro-internal; wir schreiben kein explizites `unsafe` hier.
//! Thread-Safety: DataReader/Writer halten intern `Arc` und `Mutex`,
//! damit der GIL-Release waehrend `write`/`take` sicher ist.

#![allow(clippy::missing_errors_doc)]
#![allow(unsafe_code)] // PyO3-Macros expanden zu unsafe
#![allow(unsafe_op_in_unsafe_fn)] // Rust 2024: PyO3-Macros koennen nicht wrappen
#![allow(clippy::useless_conversion)] // PyO3-Macro-generated `.into()`-Calls
#![allow(clippy::needless_lifetimes)] // PyO3-Signatures mit Bound<'py, T>
#![allow(clippy::new_without_default)] // #[new]-Konstruktoren sind Python-Construktoren, nicht Rust-Default

use std::sync::Arc;
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyTimeoutError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

use zerodds_dcps::condition::{GuardCondition, WaitSet};
use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::runtime::RuntimeConfig;
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

// =============================================================================
// Factory
// =============================================================================

/// Python-Wrapper um `DomainParticipantFactory`.
#[pyclass(name = "DomainParticipantFactory", module = "zerodds_py")]
struct PyFactory;

#[pymethods]
impl PyFactory {
    /// Singleton.
    #[staticmethod]
    fn instance() -> Self {
        Self
    }

    /// Offline-Participant (kein UDP-Bind) — ideal fuer Unit-Tests.
    fn create_participant_offline(&self, domain_id: i32) -> PyParticipant {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(domain_id, DomainParticipantQos::default());
        PyParticipant { inner: p }
    }

    /// Live-Participant mit UDP + SPDP/SEDP.
    fn create_participant(&self, domain_id: i32) -> PyResult<PyParticipant> {
        let p = DomainParticipantFactory::instance()
            .create_participant(domain_id, DomainParticipantQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyParticipant { inner: p })
    }

    /// Live-Participant mit getunten Timings (schnelles Discovery).
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

    /// Topic-Registry-Groesse.
    fn topics_len(&self) -> usize {
        self.inner.topics_len()
    }

    /// Anzahl ueber SPDP entdeckter Remote-Participants.
    fn discovered_participants_count(&self) -> usize {
        self.inner.discovered_participants_count()
    }

    /// Bytes-Topic (Type-Name `zerodds::RawBytes`). Fuer ungetypten
    /// Payload-Transfer.
    fn create_bytes_topic(&self, name: &str) -> PyResult<PyBytesTopic> {
        let topic = self
            .inner
            .create_topic::<RawBytes>(name, TopicQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyBytesTopic { inner: topic })
    }

    /// ShapesDemo-Topic (Type-Name `ShapeType`). Fuer Interop-Tests
    /// gegen Cyclone/Fast-DDS ShapesDemo.
    fn create_shape_topic(&self, name: &str) -> PyResult<PyShapeTopic> {
        let topic = self
            .inner
            .create_topic::<ShapeType>(name, TopicQos::default())
            .map_err(dds_err_to_py)?;
        Ok(PyShapeTopic { inner: topic })
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
    /// Liefert die InstanceHandles aller entdeckten Topics.
    fn get_discovered_topics(&self) -> Vec<u64> {
        self.inner
            .get_discovered_topics()
            .into_iter()
            .map(|h| h.as_raw())
            .collect()
    }
    /// Liefert die InstanceHandles aller entdeckten Participants.
    fn get_discovered_participants(&self) -> Vec<u64> {
        self.inner
            .get_discovered_participants()
            .into_iter()
            .map(|h| h.as_raw())
            .collect()
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
    /// Schreibt bytes als Sample. GIL wird waehrend des Sends freigegeben.
    fn write(&self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        let sample = RawBytes::new(data.to_vec());
        let writer = Arc::clone(&self.inner);
        py.allow_threads(|| writer.write(&sample))
            .map_err(dds_err_to_py)
    }

    fn wait_for_matched_subscription(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let writer = Arc::clone(&self.inner);
        py.allow_threads(|| {
            writer.wait_for_matched_subscription(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }

    fn matched_subscription_count(&self) -> usize {
        self.inner.matched_subscription_count()
    }

    /// `get_publication_matched_status` — (current_count, current_count, last_handle=0).
    /// RC1: ZeroDDS-DCPS expose nur `matched_subscription_count`; total/change-Pflege folgt.
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
}

#[pyclass(name = "BytesReader", module = "zerodds_py")]
struct PyBytesReader {
    inner: Arc<DataReader<RawBytes>>,
}

#[pymethods]
impl PyBytesReader {
    /// Nimmt alle verfuegbaren Samples als `list[bytes]`.
    fn take<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let reader = Arc::clone(&self.inner);
        let samples = py.allow_threads(|| reader.take()).map_err(dds_err_to_py)?;
        let list = PyList::empty_bound(py);
        for s in samples {
            list.append(PyBytes::new_bound(py, &s.data))?;
        }
        Ok(list)
    }

    fn wait_for_data(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.allow_threads(|| reader.wait_for_data(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
    }

    fn wait_for_matched_publication(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.allow_threads(|| {
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
}

// =============================================================================
// Shape-Writer / -Reader (fuer ShapesDemo-Interop)
// =============================================================================

/// Python-Repr einer ShapeType-Instanz. Ein plain dict wäre auch
/// möglich, aber eine eigene Klasse macht `isinstance`-Checks und
/// getter/setter sauberer.
#[pyclass(name = "Shape", module = "zerodds_py")]
#[derive(Clone)]
struct PyShape {
    /// Farbe / Instance-Key.
    #[pyo3(get, set)]
    color: String,
    /// X-Koordinate.
    #[pyo3(get, set)]
    x: i32,
    /// Y-Koordinate.
    #[pyo3(get, set)]
    y: i32,
    /// Groesse in Pixeln.
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
        py.allow_threads(|| writer.write(&sample))
            .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.5 `register_instance`. Liefert einen
    /// Instance-Handle, der spaeter zum dispose/unregister benutzt
    /// werden kann.
    fn register_instance(&self, py: Python<'_>, shape: &PyShape) -> PyResult<u64> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.allow_threads(|| writer.register_instance(&sample))
            .map(|h| h.as_raw())
            .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.10 `dispose`. Schickt einen Wire-Lifecycle-
    /// Marker — Reader sehen die Instanz als `NotAliveDisposed`.
    fn dispose(&self, py: Python<'_>, shape: &PyShape) -> PyResult<()> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.allow_threads(|| {
            let handle = writer.lookup_instance(&sample);
            writer.dispose(&sample, handle)
        })
        .map_err(dds_err_to_py)
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`. Bei
    /// WriterDataLifecycle.autodispose=true (Default) implizit auch
    /// dispose.
    fn unregister_instance(&self, py: Python<'_>, shape: &PyShape) -> PyResult<()> {
        let sample: ShapeType = shape.into();
        let writer = Arc::clone(&self.inner);
        py.allow_threads(|| {
            let handle = writer.lookup_instance(&sample);
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
        py.allow_threads(|| {
            writer.wait_for_matched_subscription(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
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
        let samples = py.allow_threads(|| reader.take()).map_err(dds_err_to_py)?;
        let list = PyList::empty_bound(py);
        for s in samples {
            list.append(Py::new(py, PyShape::from(s))?)?;
        }
        Ok(list)
    }

    fn wait_for_data(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.allow_threads(|| reader.wait_for_data(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
    }

    fn wait_for_matched_publication(
        &self,
        py: Python<'_>,
        min_count: usize,
        timeout_secs: f64,
    ) -> PyResult<()> {
        let reader = Arc::clone(&self.inner);
        py.allow_threads(|| {
            reader.wait_for_matched_publication(min_count, Duration::from_secs_f64(timeout_secs))
        })
        .map_err(dds_err_to_py)
    }
}

// =============================================================================
// Module registration
// =============================================================================

// =============================================================================
// Conditions + WaitSet (Spec §2.2.2.1.7)
// =============================================================================

/// Python-Wrapper um `GuardCondition`.
#[pyclass(name = "GuardCondition", module = "zerodds_py")]
struct PyGuardCondition {
    inner: Arc<GuardCondition>,
}

#[pymethods]
impl PyGuardCondition {
    /// Erzeugt eine GuardCondition mit initialem trigger=false.
    #[new]
    fn new() -> Self {
        Self {
            inner: GuardCondition::new(),
        }
    }
    /// Setzt den Trigger-Wert.
    fn set_trigger_value(&self, value: bool) {
        self.inner.set_trigger_value(value);
    }
    /// Liest den aktuellen Trigger-Wert.
    fn get_trigger_value(&self) -> bool {
        use zerodds_dcps::condition::Condition;
        self.inner.get_trigger_value()
    }
}

/// Python-Wrapper um `WaitSet`.
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
    /// Wait — liefert die Anzahl getriggerter Conditions.
    fn wait(&self, py: Python<'_>, timeout_secs: f64) -> PyResult<usize> {
        // WaitSet ist nicht Clone — nutze inner direkt.
        py.allow_threads(|| self.inner.wait(Duration::from_secs_f64(timeout_secs)))
            .map_err(dds_err_to_py)
            .map(|v| v.len())
    }
}

/// PyO3-Modul `zerodds._core`. Das aeussere Python-Package `zerodds`
/// re-exportiert daraus (siehe `python/zerodds/__init__.py`).
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
    m.add_class::<PyGuardCondition>()?;
    m.add_class::<PyWaitSet>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
