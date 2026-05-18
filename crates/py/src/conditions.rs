// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PyO3-Bindings fuer ReadCondition + QueryCondition (§6.6 Vendor-Spec
//! `zerodds-py-1.0`).
//!
//! `GuardCondition` und `WaitSet` leben bereits in `ffi.rs`. Hier kommen
//! die zwei reader-state-getriebenen Conditions hinzu, plus die
//! SampleState/ViewState/InstanceState-Bitmask-Konstanten.

#![allow(clippy::missing_errors_doc)]
#![allow(unsafe_code)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::new_without_default)]
// PyO3 macro-expansion uses unwrap/expect internally.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use zerodds_dcps::condition::{Condition, QueryCondition, ReadCondition};

// ---------------------------------------------------------------------------
// PyReadCondition
// ---------------------------------------------------------------------------

/// `ReadCondition` (Spec §2.2.2.5.8). Triggert wenn der Reader-State
/// die uebergebenen Bitmasks erfuellt. Die Standard-Closure-Logik
/// triggert sobald irgendeines der drei Masken im "ANY"-Set steht —
/// fuer den Python-Use-Case ist das die nuetzlichste Default-Semantik.
#[pyclass(name = "ReadCondition", module = "zerodds_py")]
pub struct PyReadCondition {
    pub inner: Arc<ReadCondition>,
}

#[pymethods]
impl PyReadCondition {
    /// Konstruiert eine ReadCondition mit Sample-/View-/Instance-State-
    /// Masken (DDS 1.4 §2.2.2.5.8). Use-Convenience: alle drei
    /// Argumente sind u32-Bitmasks.
    ///
    /// `state_check_mode` waehlt die Trigger-Closure:
    /// * `"any"` — triggert, wenn der Reader-Status irgendeine
    ///   passende Sample/View/Instance-Kombination hat (Default).
    /// * `"never"` — triggert nie (fuer Test/Demo).
    /// * `"always"` — triggert immer.
    #[new]
    #[pyo3(signature = (sample_state_mask, view_state_mask, instance_state_mask, state_check_mode="any"))]
    fn new(
        sample_state_mask: u32,
        view_state_mask: u32,
        instance_state_mask: u32,
        state_check_mode: &str,
    ) -> PyResult<Self> {
        let trigger: Box<dyn Fn(u32, u32, u32) -> bool + Send + Sync + 'static> =
            match state_check_mode {
                "any" => Box::new(|ss, vs, is_| ss != 0 && vs != 0 && is_ != 0),
                "never" => Box::new(|_, _, _| false),
                "always" => Box::new(|_, _, _| true),
                other => {
                    return Err(PyRuntimeError::new_err(format!(
                        "unknown state_check_mode {other:?}; expected 'any'|'never'|'always'"
                    )));
                }
            };
        let cond = ReadCondition::new(
            sample_state_mask,
            view_state_mask,
            instance_state_mask,
            trigger,
        );
        Ok(Self { inner: cond })
    }

    fn get_sample_state_mask(&self) -> u32 {
        self.inner.get_sample_state_mask()
    }

    fn get_view_state_mask(&self) -> u32 {
        self.inner.get_view_state_mask()
    }

    fn get_instance_state_mask(&self) -> u32 {
        self.inner.get_instance_state_mask()
    }

    fn get_trigger_value(&self) -> bool {
        self.inner.get_trigger_value()
    }
}

// ---------------------------------------------------------------------------
// PyQueryCondition
// ---------------------------------------------------------------------------

/// `QueryCondition` (Spec §2.2.2.5.9). ReadCondition + SQL-Filter-
/// Ausdruck. Der Filter wird beim Konstruieren validiert; eine
/// ungueltige Expression liefert `RuntimeError`.
#[pyclass(name = "QueryCondition", module = "zerodds_py")]
pub struct PyQueryCondition {
    pub inner: Arc<QueryCondition>,
}

#[pymethods]
impl PyQueryCondition {
    #[new]
    #[pyo3(signature = (
        sample_state_mask,
        view_state_mask,
        instance_state_mask,
        query_expression,
        query_parameters=Vec::new()
    ))]
    fn new(
        sample_state_mask: u32,
        view_state_mask: u32,
        instance_state_mask: u32,
        query_expression: String,
        query_parameters: Vec<String>,
    ) -> PyResult<Self> {
        let base = ReadCondition::new(
            sample_state_mask,
            view_state_mask,
            instance_state_mask,
            |ss, vs, is_| ss != 0 && vs != 0 && is_ != 0,
        );
        let qc = QueryCondition::new(base, query_expression, query_parameters)
            .map_err(|e| PyRuntimeError::new_err(format!("QueryCondition::new failed: {e:?}")))?;
        Ok(Self { inner: qc })
    }

    fn get_trigger_value(&self) -> bool {
        self.inner.get_trigger_value()
    }
}
