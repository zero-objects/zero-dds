// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PyO3 bindings for ReadCondition + QueryCondition (§6.6 vendor spec
//! `zerodds-py-1.0`).
//!
//! `GuardCondition` and `WaitSet` already live in `ffi.rs`. Here the
//! two reader-state-driven conditions are added, plus the
//! SampleState/ViewState/InstanceState bitmask constants.

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

/// `ReadCondition` (Spec §2.2.2.5.8). Triggers when the reader state
/// satisfies the given bitmasks. The default closure logic
/// triggers as soon as any of the three masks is in the "ANY" set —
/// for the Python use case this is the most useful default semantics.
#[pyclass(name = "ReadCondition", module = "zerodds_py")]
pub struct PyReadCondition {
    pub inner: Arc<ReadCondition>,
}

#[pymethods]
impl PyReadCondition {
    /// Constructs a ReadCondition with sample/view/instance-state
    /// masks (DDS 1.4 §2.2.2.5.8). Usage convenience: all three
    /// arguments are u32 bitmasks.
    ///
    /// `state_check_mode` selects the trigger closure:
    /// * `"any"` — triggers when the reader status has any
    ///   matching sample/view/instance combination (default).
    /// * `"never"` — never triggers (for test/demo).
    /// * `"always"` — always triggers.
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

/// `QueryCondition` (Spec §2.2.2.5.9). ReadCondition + SQL filter
/// expression. The filter is validated on construction; an
/// invalid expression returns `RuntimeError`.
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
