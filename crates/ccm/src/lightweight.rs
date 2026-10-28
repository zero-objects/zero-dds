// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lightweight CCM Profile — Spec §13.
//!
//! Spec §13.1 (p. 273): "The Lightweight CCM (LwCCM) profile is intended
//! to be useful in environments where: a small footprint [...] CCM with
//! reduced support for: Persistence, Introspection, Navigation,
//! Type-specific Generic Operations, Segmentation, Transactions,
//! Security, Configurators, Proxy Homes, Home Finders."
//!
//! Applied to the [`ComponentEquivalent`] the transformation produced:
//! the operations §13.2 to §13.10 exclude are removed from the model.

use alloc::vec::Vec;
use core::fmt;

use zerodds_ast::Ast;

use crate::transform::ComponentEquivalent;

/// Why the filter refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightweightFilterError {
    /// The component would have no operations left — typically one whose
    /// only features were persistence, configurator or proxy-home ones.
    EmptyAfterFilter,
    /// The model rejected the removal.
    Model(zerodds_ast::AstError),
}

impl fmt::Display for LightweightFilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAfterFilter => f.write_str(
                "component would have no operations after applying lightweight CCM filter",
            ),
            Self::Model(error) => write!(f, "the model rejected the filter: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LightweightFilterError {}

/// The generic operations §13.3 and §13.7 exclude.
///
/// Type-specific operations — `provide_<n>`, `connect_<n>` and the rest
/// the transformation emits — stay: §13 removes the generic
/// `Components::*` API, not the component's own body.
const FILTERED: [&str; 3] = ["configure", "set_configuration", "configuration_complete"];

/// Applies the Lightweight CCM Profile §13 to a component equivalent.
///
/// * §13.3 (p. 276) — no generic `provide_facet` / `get_all_facets` /
///   `get_named_facets` navigation operations.
/// * §13.3 (p. 276) — no generic `connect` / `disconnect` /
///   `get_connection(s)` on `Receptacles`, and none of the generic
///   `Events` operations.
/// * §13.7 (p. 279) — no configurator operations.
///
/// # Errors
/// [`LightweightFilterError`].
pub fn filter_to_lightweight(
    ast: &mut Ast,
    equivalent: &ComponentEquivalent,
) -> Result<(), LightweightFilterError> {
    let interface = equivalent.equivalent_interface;
    let operations = ast
        .interface(interface)
        .map_err(LightweightFilterError::Model)?
        .operations
        .clone();

    let mut doomed = Vec::new();
    let mut kept = 0usize;
    for id in operations {
        let Ok(node) = ast.operation(id) else {
            continue;
        };
        if FILTERED.contains(&node.name.as_str()) {
            doomed.push(zerodds_ast::AnyId::Operation(id, node.node_id));
        } else {
            kept += 1;
        }
    }
    if kept == 0 && !equivalent.event_consumer_interfaces.is_empty() {
        // The specification allows an empty result; the caller wanted a
        // conformant component and has none, so it is told rather than
        // handed one.
        return Err(LightweightFilterError::EmptyAfterFilter);
    }
    for any in doomed {
        ast.remove(any).map_err(LightweightFilterError::Model)?;
    }
    Ok(())
}
