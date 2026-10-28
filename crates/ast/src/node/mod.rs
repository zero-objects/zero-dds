// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The node space. Spec §3.

pub mod annotation;
pub mod constant;
pub mod deployment;
pub mod exception;
pub mod interface;
pub mod member;
pub mod module;
pub mod type_;

pub use annotation::{AnnotationDefNode, AnnotationMemberNode};
pub use constant::{
    BinaryOp, ConstExpr, ConstExprNode, ConstNode, ConstValue, EvalError, Literal, UnaryOp,
};
pub use deployment::{
    DomainNode, EndpointNode, EndpointRole, LibraryNode, ParticipantNode, QosPolicy, QosPolicyKind,
    QosProfileNode, TopicNode,
};
pub use exception::ExceptionNode;
pub use interface::{
    AttributeNode, InterfaceKind, InterfaceNode, OperationNode, ParamDirection, ParamNode,
};
pub use member::{BitValueNode, BitfieldNode, CaseLabel, CaseNode, EnumeratorNode, MemberNode};
pub use module::ModuleNode;
pub use type_::{Bound, CharWidth, Completeness, PrimitiveKind, TypeKind, TypeNode, ValuetypeKind};

use crate::kind::NodeKind;
use zerodds_ast_core::{Id, NodeId};

/// A handle to any node, carrying its kind and its stable identity.
///
/// Used where a container holds mixed kinds — a module's children, a
/// member's parent — without erasing which kind each entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnyId {
    /// A module.
    Module(Id<ModuleNode>, NodeId),
    /// A type.
    Type(Id<TypeNode>, NodeId),
    /// An aggregate member.
    Member(Id<MemberNode>, NodeId),
    /// A union case.
    Case(Id<CaseNode>, NodeId),
    /// An enumerator.
    Enumerator(Id<EnumeratorNode>, NodeId),
    /// A bitmask bit.
    BitValue(Id<BitValueNode>, NodeId),
    /// A bitset field.
    Bitfield(Id<BitfieldNode>, NodeId),
    /// A constant.
    Const(Id<ConstNode>, NodeId),
    /// An expression node.
    ConstExpr(Id<ConstExprNode>, NodeId),
    /// An interface.
    Interface(Id<InterfaceNode>, NodeId),
    /// An operation.
    Operation(Id<OperationNode>, NodeId),
    /// An operation parameter.
    Param(Id<ParamNode>, NodeId),
    /// An interface attribute.
    Attribute(Id<AttributeNode>, NodeId),
    /// An exception.
    Exception(Id<ExceptionNode>, NodeId),
    /// An annotation declaration.
    AnnotationDef(Id<AnnotationDefNode>, NodeId),
    /// An annotation member.
    AnnotationMember(Id<AnnotationMemberNode>, NodeId),
    /// A domain.
    Domain(Id<DomainNode>, NodeId),
    /// A topic.
    Topic(Id<TopicNode>, NodeId),
    /// A QoS profile.
    QosProfile(Id<QosProfileNode>, NodeId),
    /// A participant.
    Participant(Id<ParticipantNode>, NodeId),
    /// An endpoint.
    Endpoint(Id<EndpointNode>, NodeId),
    /// A deployment library.
    Library(Id<LibraryNode>, NodeId),
}

impl AnyId {
    /// The node's stable identity.
    #[must_use]
    pub const fn node_id(self) -> NodeId {
        match self {
            Self::Module(_, id)
            | Self::Type(_, id)
            | Self::Member(_, id)
            | Self::Case(_, id)
            | Self::Enumerator(_, id)
            | Self::BitValue(_, id)
            | Self::Bitfield(_, id)
            | Self::Const(_, id)
            | Self::ConstExpr(_, id)
            | Self::Interface(_, id)
            | Self::Operation(_, id)
            | Self::Param(_, id)
            | Self::Attribute(_, id)
            | Self::Exception(_, id)
            | Self::AnnotationDef(_, id)
            | Self::AnnotationMember(_, id)
            | Self::Domain(_, id)
            | Self::Topic(_, id)
            | Self::QosProfile(_, id)
            | Self::Participant(_, id)
            | Self::Endpoint(_, id)
            | Self::Library(_, id) => id,
        }
    }

    /// The node's kind.
    #[must_use]
    pub const fn kind(self) -> NodeKind {
        match self {
            Self::Module(..) => NodeKind::Module,
            Self::Type(..) => NodeKind::Type,
            Self::Member(..) => NodeKind::Member,
            Self::Case(..) => NodeKind::Case,
            Self::Enumerator(..) => NodeKind::Enumerator,
            Self::BitValue(..) => NodeKind::BitValue,
            Self::Bitfield(..) => NodeKind::Bitfield,
            Self::Const(..) => NodeKind::Const,
            Self::ConstExpr(..) => NodeKind::ConstExpr,
            Self::Interface(..) => NodeKind::Interface,
            Self::Operation(..) => NodeKind::Operation,
            Self::Param(..) => NodeKind::Param,
            Self::Attribute(..) => NodeKind::Attribute,
            Self::Exception(..) => NodeKind::Exception,
            Self::AnnotationDef(..) => NodeKind::AnnotationDef,
            Self::AnnotationMember(..) => NodeKind::AnnotationMember,
            Self::Domain(..) => NodeKind::Domain,
            Self::Topic(..) => NodeKind::Topic,
            Self::QosProfile(..) => NodeKind::QosProfile,
            Self::Participant(..) => NodeKind::Participant,
            Self::Endpoint(..) => NodeKind::Endpoint,
            Self::Library(..) => NodeKind::Library,
        }
    }

    /// The type handle, if this names a type.
    #[must_use]
    pub const fn as_type(self) -> Option<Id<TypeNode>> {
        match self {
            Self::Type(id, _) => Some(id),
            _ => None,
        }
    }

    /// The interface handle, if this names an interface.
    #[must_use]
    pub const fn as_interface(self) -> Option<Id<crate::InterfaceNode>> {
        match self {
            Self::Interface(id, _) => Some(id),
            _ => None,
        }
    }

    /// The module handle, if this names a module.
    #[must_use]
    pub const fn as_module(self) -> Option<Id<ModuleNode>> {
        match self {
            Self::Module(id, _) => Some(id),
            _ => None,
        }
    }

    /// The member handle, if this names a member.
    #[must_use]
    pub const fn as_member(self) -> Option<Id<MemberNode>> {
        match self {
            Self::Member(id, _) => Some(id),
            _ => None,
        }
    }
}
