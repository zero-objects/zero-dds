// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Dynamic Properties (CosTradingDynamic `DynamicPropEval`).
//!
//! Some property values are not known statically at export time but are instead
//! **recomputed fresh on every query** — e.g. the current load of a server.
//! A [`DynamicPropEval`] returns the value as of right now; before evaluating
//! constraints, the trader resolves the properties of an offer that are marked
//! dynamic via the registered evaluator.

use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::property::Value;

/// Evaluator for dynamic property values (CosTradingDynamic `DynamicPropEval`).
///
/// Called by the trader once per query and per dynamic property. `statics` are
/// the static properties of the same offer (as context, e.g. to select the data
/// source). `None` means "no value right now" — the property then stays unset
/// and constraints over it do not match.
///
/// Requires [`core::fmt::Debug`] so that `Trader` can still derive `Debug`.
pub trait DynamicPropEval: core::fmt::Debug {
    /// Computes the current value of the dynamic property `property` of an
    /// offer of type `service_type`.
    fn evaluate(
        &self,
        service_type: &str,
        property: &str,
        statics: &BTreeMap<String, Value>,
    ) -> Option<Value>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct LoadEval;
    impl DynamicPropEval for LoadEval {
        fn evaluate(
            &self,
            _service_type: &str,
            property: &str,
            statics: &BTreeMap<String, Value>,
        ) -> Option<Value> {
            if property == "Load" {
                // "Load" = base load from the statics + a fixed offset.
                let base = statics
                    .get("BaseLoad")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                Some(Value::Float(base + 10.0))
            } else {
                None
            }
        }
    }

    #[test]
    fn evaluator_computes_value_from_context() {
        let mut s = BTreeMap::new();
        s.insert(String::from("BaseLoad"), Value::Float(5.0));
        let ev = LoadEval;
        assert_eq!(ev.evaluate("Server", "Load", &s), Some(Value::Float(15.0)));
        assert_eq!(ev.evaluate("Server", "Other", &s), None);
    }
}
