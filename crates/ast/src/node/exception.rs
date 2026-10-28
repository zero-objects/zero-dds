// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Exceptions. Spec §3.4.

use crate::facet::Facets;
use crate::node::member::MemberNode;
use crate::node::module::ModuleNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId};

/// An exception. Spec §3.4.
///
/// Structurally an aggregate, but not a `TypeKind`: an exception cannot
/// appear as a member type, so giving it its own kind keeps the type
/// space honest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Enclosing module.
    pub module: Id<ModuleNode>,
    /// The interface or value type this is declared in, when it is not
    /// declared directly in a module (OMG IDL 4.2 §7.4.3.4: an interface
    /// body holds type, constant and exception declarations). Its name is
    /// a segment of the scoped name and of the repository id:
    /// `CosNaming::NamingContext::NotFound`.
    pub scope: Option<String>,
    /// Members in declaration order.
    pub members: Vec<Id<MemberNode>>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}
