// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Forward application. Spec §2.5.
//!
//! A rule specification lowers to two directed operationalisations; only
//! the forward one runs here. The backward direction is what the engine
//! gives for free and what a later phase will use to read generated code
//! back — exercising it now would let a rule quietly come to depend on
//! it.

use crate::adapter::ast::ModelGraph;
use seesaw_tgg::engine::Engine;
use seesaw_tgg::ident::GhostId;
use seesaw_tgg::rules::format::RuleFile;
use seesaw_tgg::rules::{lower::lower_all, validate::validate};
use std::collections::BTreeMap;
use zerodds_ast_core::NodeId;

/// How many engine steps one application may take.
///
/// A bound rather than trust: a catalogue whose rules trigger each other
/// is a defect, and without this it hangs instead of being reported.
const MAX_STEPS: usize = 100_000;

/// What applying a catalogue produced.
pub struct Applied {
    /// The graph after application, holding both sides.
    pub graph: seesaw_tgg::graph::Graph,
    /// Which model node each created node corresponds to.
    ///
    /// Read off the correspondences the rules established, so a
    /// materialiser can ask the model for the values a node needs —
    /// values never enter the overlay, they are resolved from the
    /// original. Spec §2.3.
    pub origin: BTreeMap<GhostId, NodeId>,
    /// How many steps the engine took.
    pub steps: usize,
}

/// Why an application could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// A rule did not lower, with the engine's reason.
    Lowering {
        /// The rule.
        rule: String,
        /// What the engine said.
        detail: String,
    },
    /// The engine did not settle within the step bound.
    DidNotSettle {
        /// The bound it passed.
        steps: usize,
    },
}

impl core::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lowering { rule, detail } => write!(f, "rule `{rule}` did not lower: {detail}"),
            Self::DidNotSettle { steps } => {
                write!(f, "the catalogue did not settle within {steps} steps")
            }
        }
    }
}

impl core::error::Error for ApplyError {}

/// Apply `rules` to `model`, forward.
///
/// # Errors
/// [`ApplyError::Lowering`] when a rule does not compile, and
/// [`ApplyError::DidNotSettle`] when the catalogue keeps firing.
pub fn apply(model: &ModelGraph, rules: &RuleFile) -> Result<Applied, ApplyError> {
    let mut graph = model.graph.clone();
    let values = model.values.clone();

    // Validation first: the loader resolves node names to positions and
    // reports a rule that names something it never declared, which a
    // hand-built rule value can otherwise carry all the way to a match
    // that silently never fires.
    let resolved = validate(rules).map_err(|error| ApplyError::Lowering {
        rule: rules.name.clone(),
        detail: format!("{error:?}"),
    })?;

    // Both directions come back; only the forward ones run. The backward
    // direction is what the engine gives for free and what a later phase
    // will use to read generated code back — exercising it now would let
    // a rule quietly come to depend on it.
    let all = lower_all(&resolved, &mut graph).map_err(|error| ApplyError::Lowering {
        rule: rules.name.clone(),
        detail: format!("{error:?}"),
    })?;
    let directed: Vec<_> = all
        .into_iter()
        .filter(|rule| !rule.name.ends_with("__backward"))
        .collect();

    let mut engine = Engine::new(&directed);
    engine.seed(&graph, &values);
    let mut steps = 0usize;
    while engine.step(&mut graph, &values).is_some() {
        steps += 1;
        if steps >= MAX_STEPS {
            return Err(ApplyError::DidNotSettle { steps });
        }
    }

    Ok(Applied {
        graph,
        origin: origins(model),
        steps,
    })
}

/// The map from created node to the model node it came from.
///
/// Built from the adapter's own map: every node the model contributed
/// keeps its identity, and a created node's origin is the node its
/// correspondence anchors on.
fn origins(model: &ModelGraph) -> BTreeMap<GhostId, NodeId> {
    let mut out = BTreeMap::new();
    for (ghost, node) in model.pairs() {
        let _ = out.insert(ghost, node);
    }
    out
}
