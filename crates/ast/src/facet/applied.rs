// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotations applied to a node that have no typed field. Spec §4.1.
//!
//! The annotations the specifications define reach typed fields — an
//! `@key` sets [`XTypesFacet::is_key`], not an entry here. This carries
//! what a user declared with `@annotation`: a name, its arguments, and a
//! reference to the declaration they belong to.
//!
//! Without it a user-defined annotation would have nowhere to land, and
//! a backend regenerating IDL would silently emit a declaration the
//! source never had.
//!
//! [`XTypesFacet::is_key`]: crate::facet::xtypes::XTypesFacet::is_key

use crate::node::annotation::AnnotationDefNode;
use crate::node::constant::ConstExprNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, Ref};

/// One argument of an applied annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedArgument {
    /// The parameter name, when the source wrote `k = v`.
    ///
    /// Absent for the positional form `@ann(v)`, which names the
    /// declaration's `value` member.
    pub name: Option<String>,
    /// The argument expression.
    pub value: Id<ConstExprNode>,
}

/// One annotation applied to a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAnnotation {
    /// The name as written, without the `@`.
    ///
    /// Kept alongside the reference: the declaration may arrive after
    /// the use, and a diagnostic has to name the annotation either way.
    pub name: String,
    /// The declaration this applies, once it is known.
    pub definition: Ref<AnnotationDefNode>,
    /// Arguments in the order they were written.
    pub arguments: Vec<AppliedArgument>,
}
