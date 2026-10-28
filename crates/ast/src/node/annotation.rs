// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotation declarations. Spec §3.6.

use crate::facet::Facets;
use crate::node::constant::ConstExprNode;
use crate::node::module::ModuleNode;
use crate::node::type_::TypeNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId, Ref};

/// An `@annotation` declaration. Spec §3.6.
///
/// Built-in annotations are ordinary declarations in the builtin module,
/// so user-defined and built-in annotations traverse identically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationDefNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name, without the `@`.
    pub name: String,
    /// Enclosing module.
    pub module: Id<ModuleNode>,
    /// Members in declaration order.
    pub members: Vec<Id<AnnotationMemberNode>>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A member of an annotation declaration. Spec §3.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationMemberNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Declared type.
    pub type_: Ref<TypeNode>,
    /// Default value, if declared.
    pub default: Option<Id<ConstExprNode>>,
    /// Whether this is the implicit single member named `value`, which
    /// enables the `@ann(x)` short form.
    pub is_value_member: bool,
    /// The declaration this member belongs to.
    pub parent: Id<AnnotationDefNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// The built-in annotations the model always carries.
///
/// Sources: DDS-XTypes 1.3 §7.3.1.2 and Annex B, plus the IDL 4.2
/// building-block annotations. The order is fixed, because builtin
/// identities are allocated in it and must be identical in every model
/// so that snapshots from different processes compare. Spec §3.6, §9.
pub const BUILTIN_ANNOTATIONS: [&str; 40] = [
    "id",
    "autoid",
    "optional",
    "position",
    "value",
    "extensibility",
    "final",
    "appendable",
    "mutable",
    "key",
    "must_understand",
    "default_literal",
    "default",
    "range",
    "min",
    "max",
    "unit",
    "bit_bound",
    "external",
    "nested",
    "verbatim",
    "service",
    "oneway",
    "ami",
    "non_serialized",
    "try_construct",
    "topic",
    "hashid",
    "data_representation",
    "ignore_literal_names",
    // Appended, never inserted: the order allocates builtin identities
    // and a snapshot written by another build has to compare.
    "default_nested",
    // DDS-RPC 1.0's own annotations, none of which the frontend can
    // expect a file to declare: §7.4.2.2 `@DDSRequestTopic`/
    // `@DDSReplyTopic`, §7.3.1.4 `@RPCRequestType`/`@RPCReplyType`,
    // §7.10.1 `@Qos`, and §7.5.1.2.1 `@Choice`/`@Autoid`/`@Empty`/
    // `@Mutable` (capitalised, and so distinct from the DDS-XTypes
    // `@autoid`/`@mutable` above). Appended for the same reason.
    "DDSRequestTopic",
    "DDSReplyTopic",
    "RPCRequestType",
    "RPCReplyType",
    "Qos",
    "Choice",
    "Autoid",
    "Empty",
    "Mutable",
];

/// Name of the module holding the built-in declarations.
///
/// Not a legal IDL identifier, so a frontend cannot declare a module
/// that collides with it.
pub const BUILTIN_MODULE_NAME: &str = "@builtin";
