// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotifyFilter §5 — `Filter` + `ConstraintExp` + `FilterFactory`.
//!
//! Full TCL (Trader Constraint Language, §5.4) is a topic of its own; here the
//! event-type match is implemented (the bulk of real-world filters): a filter
//! holds constraints, each with a set of `EventType` patterns (with
//! `"*"` wildcard). An event matches if it matches any constraint pattern.
//! The `constraint_expr` string (TCL) is carried but not yet
//! evaluated (an empty expression = "match all" within the type set).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use crate::event::{EventType, StructuredEvent};

/// `ConstraintExp` (§5.4): event-type set + TCL expression.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConstraintExp {
    /// `event_types` — patterns (with `"*"`).
    pub event_types: Vec<EventType>,
    /// `constraint_expr` — TCL expression (not yet evaluated).
    pub constraint_expr: String,
}

/// A constraint installed in the filter together with its constraint ID.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledConstraint {
    /// `ConstraintID`.
    pub id: i32,
    /// The constraint expression.
    pub exp: ConstraintExp,
}

/// `CosNotifyFilter::Filter` (§5.3) — In-Memory.
#[derive(Debug, Default)]
pub struct Filter {
    constraints: Mutex<Vec<InstalledConstraint>>,
    next_id: AtomicI32,
    grammar: String,
}

impl Filter {
    /// New empty filter with a constraint grammar (default: `"EXTENDED_TCL"`).
    #[must_use]
    pub fn new(grammar: impl Into<String>) -> Self {
        Self {
            constraints: Mutex::new(Vec::new()),
            next_id: AtomicI32::new(1),
            grammar: grammar.into(),
        }
    }

    /// `constraint_grammar`.
    #[must_use]
    pub fn constraint_grammar(&self) -> &str {
        &self.grammar
    }

    /// `add_constraints` (§5.3) — returns the assigned ConstraintIDs.
    pub fn add_constraints(&self, exps: Vec<ConstraintExp>) -> Vec<i32> {
        let Ok(mut g) = self.constraints.lock() else {
            return Vec::new();
        };
        exps.into_iter()
            .map(|exp| {
                let id = self.next_id.fetch_add(1, Ordering::Relaxed);
                g.push(InstalledConstraint { id, exp });
                id
            })
            .collect()
    }

    /// `remove_constraints` (§5.3).
    pub fn remove_constraints(&self, ids: &[i32]) {
        if let Ok(mut g) = self.constraints.lock() {
            g.retain(|c| !ids.contains(&c.id));
        }
    }

    /// `get_all_constraints` (§5.3).
    #[must_use]
    pub fn all_constraints(&self) -> Vec<InstalledConstraint> {
        self.constraints
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// `destroy` — remove all constraints.
    pub fn destroy(&self) {
        if let Ok(mut g) = self.constraints.lock() {
            g.clear();
        }
    }

    /// `match_structured` (§5.3): `true` if the event satisfies a constraint.
    /// An empty filter (no constraints) lets everything through (spec default).
    #[must_use]
    pub fn match_structured(&self, event: &StructuredEvent) -> bool {
        let Ok(g) = self.constraints.lock() else {
            return false;
        };
        if g.is_empty() {
            return true;
        }
        let et = event.event_type();
        g.iter().any(|c| {
            // Empty event_types set = "all types"; otherwise one must match.
            c.exp.event_types.is_empty() || c.exp.event_types.iter().any(|p| et.matches(p))
        })
    }
}

/// `CosNotifyFilter::FilterFactory` (§5.2).
#[derive(Debug, Default)]
pub struct FilterFactory;

impl FilterFactory {
    /// New factory.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// `create_filter` (§5.2) with the given constraint grammar.
    #[must_use]
    pub fn create_filter(&self, grammar: impl Into<String>) -> Arc<Filter> {
        Arc::new(Filter::new(grammar))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ev(domain: &str, type_name: &str) -> StructuredEvent {
        StructuredEvent::new(domain, type_name, "n", zerodds_cdr::CorbaAny::default())
    }

    #[test]
    fn empty_filter_matches_all() {
        let f = Filter::new("EXTENDED_TCL");
        assert!(f.match_structured(&ev("Any", "Thing")));
    }

    #[test]
    fn type_constraint_matches_with_wildcard() {
        let f = Filter::new("EXTENDED_TCL");
        let ids = f.add_constraints(alloc::vec![ConstraintExp {
            event_types: alloc::vec![EventType::new("Telecom", "*")],
            constraint_expr: String::new(),
        }]);
        assert_eq!(ids.len(), 1);
        assert!(f.match_structured(&ev("Telecom", "CallEvent")));
        assert!(!f.match_structured(&ev("Finance", "Trade")));
        f.remove_constraints(&ids);
        assert!(f.match_structured(&ev("Finance", "Trade")));
    }

    #[test]
    fn factory_creates_filter() {
        let fac = FilterFactory::new();
        let f = fac.create_filter("EXTENDED_TCL");
        assert_eq!(f.constraint_grammar(), "EXTENDED_TCL");
    }
}
