// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Members and aggregate parts. Spec §3.3.

use crate::facet::Facets;
use crate::node::AnyId;
use crate::node::constant::ConstExprNode;
use crate::node::type_::TypeNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId, Ref};

/// A field of an aggregate.
///
/// Covers struct members, valuetype state members, union case members
/// and exception members. Spec §3.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Declared type.
    pub type_: Ref<TypeNode>,
    /// The aggregate this member belongs to.
    pub parent: AnyId,
    /// Per-source information, including the XTypes member id. Spec §4.
    pub facets: Facets,
}

/// A union case label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseLabel {
    /// `case <expr>:`
    Value(Id<ConstExprNode>),
    /// `default:`
    Default,
}

impl CaseLabel {
    /// Whether this is the `default:` label.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Default)
    }

    /// The label expression, if this is not `default:`.
    #[must_use]
    pub const fn expr(self) -> Option<Id<ConstExprNode>> {
        match self {
            Self::Value(id) => Some(id),
            Self::Default => None,
        }
    }
}

/// One `case` of a union, with its labels. Spec §3.3.
///
/// A case may carry several labels — `case 1: case 2: long x;` — so the
/// labels are a list, not a single value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Labels selecting this case, in declaration order.
    pub labels: Vec<CaseLabel>,
    /// The member this case declares.
    pub member: Id<MemberNode>,
    /// The union this case belongs to.
    pub parent: Id<TypeNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// One enumerator. Spec §3.3.
///
/// `value` is `None` for a positional enumerator and `Some` for an
/// explicit `@value(n)`. The two encode differently, so collapsing the
/// distinction is a wire bug — one that the emitters this rebuild
/// replaces actually shipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumeratorNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Explicit `@value(n)`, if declared.
    pub value: Option<Id<ConstExprNode>>,
    /// The enumeration this enumerator belongs to.
    pub parent: Id<TypeNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// One bitmask bit. Spec §3.3.
///
/// `position` is `None` for a positional bit and `Some` for an explicit
/// `@position(k)`, for the same reason as [`EnumeratorNode::value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitValueNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Explicit `@position(k)`, if declared.
    pub position: Option<u16>,
    /// The bitmask this bit belongs to.
    pub parent: Id<TypeNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// One bitset field. Spec §3.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitfieldNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name. Anonymous padding fields have none.
    pub name: Option<String>,
    /// Width in bits.
    pub bits: u16,
    /// Explicit holder type, if declared.
    pub holder_type: Option<Ref<TypeNode>>,
    /// The bitset this field belongs to.
    pub parent: Id<TypeNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

impl BitfieldNode {
    /// Whether this is an anonymous padding field.
    #[must_use]
    pub const fn is_padding(&self) -> bool {
        self.name.is_none()
    }
}
