// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PyO3-Listener-Callbacks (§6.5 Vendor-Spec `zerodds-py-1.0`).
//!
//! Verbindet Python-Callbacks mit den Rust-Listener-Traits aus
//! `crates/dcps/src/listener.rs`. Jeder DataReader/DataWriter kann
//! einen Listener mit beliebigen Callback-Subsets registrieren; die
//! Bridge ruft die Python-Funktionen unter GIL-Acquire.

#![allow(clippy::missing_errors_doc)]
#![allow(unsafe_code)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::new_without_default)]
// PyO3 macro-expansion + Python-Callback-Bridge nutzen unwrap/expect intern.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Mutex;

use pyo3::prelude::*;
use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::{DataReaderListener, DataWriterListener};
use zerodds_dcps::status::{
    LivelinessChangedStatus, LivelinessLostStatus, OfferedDeadlineMissedStatus,
    PublicationMatchedStatus, RequestedDeadlineMissedStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};

// ---------------------------------------------------------------------------
// Internal: a struct holding optional Python callables per callback slot.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PyListenerSlots {
    // Writer callbacks
    on_offered_deadline_missed: Option<Py<PyAny>>,
    on_liveliness_lost: Option<Py<PyAny>>,
    on_publication_matched: Option<Py<PyAny>>,
    // Reader callbacks
    on_data_available: Option<Py<PyAny>>,
    on_sample_lost: Option<Py<PyAny>>,
    on_sample_rejected: Option<Py<PyAny>>,
    on_requested_deadline_missed: Option<Py<PyAny>>,
    on_liveliness_changed: Option<Py<PyAny>>,
    on_subscription_matched: Option<Py<PyAny>>,
}

// ---------------------------------------------------------------------------
// PyDataWriterListener — gehoert via `Arc` an einen `DataWriter`.
// ---------------------------------------------------------------------------

/// Listener fuer einen `DataWriter`. Vier Callback-Slots, jeder optional
/// (siehe DDS 1.4 §2.2.4.2.5). Slots werden ueber `on_*`-Setter im Python-
/// Layer befuellt.
#[pyclass(name = "DataWriterListener", module = "zerodds_py")]
pub struct PyDataWriterListener {
    slots: Mutex<PyListenerSlots>,
}

#[pymethods]
impl PyDataWriterListener {
    #[new]
    fn new() -> Self {
        Self {
            slots: Mutex::new(PyListenerSlots::default()),
        }
    }

    fn on_offered_deadline_missed(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_offered_deadline_missed = Some(callback);
    }

    fn on_liveliness_lost(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_liveliness_lost = Some(callback);
    }

    fn on_publication_matched(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_publication_matched = Some(callback);
    }
}

/// Bridge-Type: implementiert `DataWriterListener` indem es die
/// Python-Callbacks ruft. Wird vom Python-Layer per
/// `Arc::new(PyDataWriterListenerBridge { slots })` instanziiert
/// und an `DataWriter::set_listener` weitergegeben.
pub struct PyDataWriterListenerBridge {
    slots: std::sync::Arc<Mutex<PyListenerSlots>>,
}

impl PyDataWriterListenerBridge {
    pub fn from_pyclass(listener: &PyDataWriterListener) -> std::sync::Arc<Self> {
        // Snapshot der aktuellen Callbacks; Setter nach `set_listener`
        // werden nicht propagiert (Caller setzt erst alle Callbacks,
        // dann `set_listener`). `Py::clone_ref` braucht den GIL —
        // an dieser Stelle besitzt das Caller-Frame ihn schon
        // (PyO3-Methode), aber wir holen ihn explizit, um die
        // Abhaengigkeit lokal zu zeigen.
        let snapshot = Python::with_gil(|py| {
            let g = listener.slots.lock().unwrap();
            PyListenerSlots {
                on_offered_deadline_missed: g
                    .on_offered_deadline_missed
                    .as_ref()
                    .map(|c| c.clone_ref(py)),
                on_liveliness_lost: g.on_liveliness_lost.as_ref().map(|c| c.clone_ref(py)),
                on_publication_matched: g.on_publication_matched.as_ref().map(|c| c.clone_ref(py)),
                on_data_available: None,
                on_sample_lost: None,
                on_sample_rejected: None,
                on_requested_deadline_missed: None,
                on_liveliness_changed: None,
                on_subscription_matched: None,
            }
        });
        std::sync::Arc::new(Self {
            slots: std::sync::Arc::new(Mutex::new(snapshot)),
        })
    }
}

fn call_with_handle_and_status<F>(callback: &Py<PyAny>, build_args: F)
where
    F: for<'py> FnOnce(Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyTuple>>,
{
    // Python-Callback unter GIL-Acquire aufrufen. Exceptions werden in
    // stderr geschrieben (PyErr::print) — Rust-Side darf keine
    // Listener-Exception propagieren.
    Python::with_gil(|py| match build_args(py) {
        Ok(args) => {
            if let Err(e) = callback.call1(py, args) {
                e.print(py);
            }
        }
        Err(e) => e.print(py),
    });
}

impl DataWriterListener for PyDataWriterListenerBridge {
    fn on_offered_deadline_missed(
        &self,
        writer: InstanceHandle,
        status: OfferedDeadlineMissedStatus,
    ) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_offered_deadline_missed {
            let handle: u64 = writer.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (status.total_count, status.total_count_change);
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }

    fn on_liveliness_lost(&self, writer: InstanceHandle, status: LivelinessLostStatus) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_liveliness_lost {
            let handle: u64 = writer.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (status.total_count, status.total_count_change);
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }

    fn on_publication_matched(&self, writer: InstanceHandle, status: PublicationMatchedStatus) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_publication_matched {
            let handle: u64 = writer.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (
                    status.total_count,
                    status.total_count_change,
                    status.current_count,
                    status.current_count_change,
                );
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }
}

// ---------------------------------------------------------------------------
// PyDataReaderListener — 7 callbacks.
// ---------------------------------------------------------------------------

#[pyclass(name = "DataReaderListener", module = "zerodds_py")]
pub struct PyDataReaderListener {
    slots: Mutex<PyListenerSlots>,
}

#[pymethods]
impl PyDataReaderListener {
    #[new]
    fn new() -> Self {
        Self {
            slots: Mutex::new(PyListenerSlots::default()),
        }
    }

    fn on_data_available(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_data_available = Some(callback);
    }

    fn on_sample_lost(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_sample_lost = Some(callback);
    }

    fn on_sample_rejected(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_sample_rejected = Some(callback);
    }

    fn on_requested_deadline_missed(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_requested_deadline_missed = Some(callback);
    }

    fn on_liveliness_changed(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_liveliness_changed = Some(callback);
    }

    fn on_subscription_matched(&self, callback: Py<PyAny>) {
        self.slots.lock().unwrap().on_subscription_matched = Some(callback);
    }
}

pub struct PyDataReaderListenerBridge {
    slots: std::sync::Arc<Mutex<PyListenerSlots>>,
}

impl PyDataReaderListenerBridge {
    pub fn from_pyclass(listener: &PyDataReaderListener) -> std::sync::Arc<Self> {
        let snapshot = Python::with_gil(|py| {
            let g = listener.slots.lock().unwrap();
            PyListenerSlots {
                on_offered_deadline_missed: None,
                on_liveliness_lost: None,
                on_publication_matched: None,
                on_data_available: g.on_data_available.as_ref().map(|c| c.clone_ref(py)),
                on_sample_lost: g.on_sample_lost.as_ref().map(|c| c.clone_ref(py)),
                on_sample_rejected: g.on_sample_rejected.as_ref().map(|c| c.clone_ref(py)),
                on_requested_deadline_missed: g
                    .on_requested_deadline_missed
                    .as_ref()
                    .map(|c| c.clone_ref(py)),
                on_liveliness_changed: g.on_liveliness_changed.as_ref().map(|c| c.clone_ref(py)),
                on_subscription_matched: g
                    .on_subscription_matched
                    .as_ref()
                    .map(|c| c.clone_ref(py)),
            }
        });
        std::sync::Arc::new(Self {
            slots: std::sync::Arc::new(Mutex::new(snapshot)),
        })
    }
}

impl DataReaderListener for PyDataReaderListenerBridge {
    fn on_data_available(&self, reader: InstanceHandle) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_data_available {
            let handle: u64 = reader.as_raw();
            call_with_handle_and_status(cb, move |py| {
                Ok(pyo3::types::PyTuple::new_bound(py, [handle.into_py(py)]))
            });
        }
    }

    fn on_sample_lost(&self, reader: InstanceHandle, status: SampleLostStatus) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_sample_lost {
            let handle: u64 = reader.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (status.total_count, status.total_count_change);
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }

    fn on_sample_rejected(&self, reader: InstanceHandle, status: SampleRejectedStatus) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_sample_rejected {
            let handle: u64 = reader.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (status.total_count, status.total_count_change);
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }

    fn on_requested_deadline_missed(
        &self,
        reader: InstanceHandle,
        status: RequestedDeadlineMissedStatus,
    ) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_requested_deadline_missed {
            let handle: u64 = reader.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (status.total_count, status.total_count_change);
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }

    fn on_liveliness_changed(&self, reader: InstanceHandle, status: LivelinessChangedStatus) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_liveliness_changed {
            let handle: u64 = reader.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (
                    status.alive_count,
                    status.not_alive_count,
                    status.alive_count_change,
                    status.not_alive_count_change,
                );
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }

    fn on_subscription_matched(&self, reader: InstanceHandle, status: SubscriptionMatchedStatus) {
        let g = self.slots.lock().unwrap();
        if let Some(cb) = &g.on_subscription_matched {
            let handle: u64 = reader.as_raw();
            call_with_handle_and_status(cb, move |py| {
                let s = (
                    status.total_count,
                    status.total_count_change,
                    status.current_count,
                    status.current_count_change,
                );
                Ok(pyo3::types::PyTuple::new_bound(
                    py,
                    [handle.into_py(py), s.into_py(py)],
                ))
            });
        }
    }
}
