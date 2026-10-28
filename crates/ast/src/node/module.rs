// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Modules. Spec §3.1.

use crate::facet::Facets;
use crate::node::AnyId;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId};

/// A namespace. Spec §3.1.
///
/// Reopening a module in a second translation unit merges into the same
/// node rather than creating a second one; see
/// [`crate::Ast::ensure_module`]. That is the structural resolution of
/// the per-file fragmentation that previously needed an opt-in dedup
/// pass on multi-input builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name. Empty for the root module.
    pub name: String,
    /// Enclosing module; `None` only for the root.
    pub parent: Option<Id<ModuleNode>>,
    /// Everything declared directly inside, in declaration order.
    pub children: Vec<AnyId>,
    /// Per-source information. A module reopened across files records
    /// the first origin; the rest live in the changeset history.
    /// Spec §4.7.
    pub facets: Facets,
}

impl ModuleNode {
    /// Whether this is the anonymous global scope.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}
