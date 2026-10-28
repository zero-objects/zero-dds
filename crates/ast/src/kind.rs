// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Node kind discriminators.
//!
//! The tag numbers are part of the snapshot format (spec §9) — never
//! renumber an existing variant, only append.

use zerodds_ast_core::NodeKindTag;

/// Every kind of node the model holds. Spec §3.
///
/// Exhaustive on purpose. An adapter presenting the model to another
/// system has to name every kind; with a catch-all arm a kind added here
/// would become a construct that system can never see, and nothing would
/// say so. Adding a variant is a compile error at every such consumer,
/// which is the point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKind {
    /// A namespace. Spec §3.1.
    Module,
    /// A named type. Spec §3.2.
    Type,
    /// A field of an aggregate. Spec §3.3.
    Member,
    /// One `case` of a union. Spec §3.3.
    Case,
    /// One enumerator. Spec §3.3.
    Enumerator,
    /// One bitmask bit. Spec §3.3.
    BitValue,
    /// One bitset field. Spec §3.3.
    Bitfield,
    /// A named constant. Spec §3.5.
    Const,
    /// One node of a constant expression tree. Spec §3.5.
    ConstExpr,
    /// An interface or component form. Spec §3.4.
    Interface,
    /// An operation. Spec §3.4.
    Operation,
    /// An operation parameter. Spec §3.4.
    Param,
    /// An interface attribute. Spec §3.4.
    Attribute,
    /// An exception. Spec §3.4.
    Exception,
    /// An `@annotation` declaration. Spec §3.6.
    AnnotationDef,
    /// A member of an annotation declaration. Spec §3.6.
    AnnotationMember,
    /// A DDS domain. Spec §3.7.
    Domain,
    /// A topic. Spec §3.7.
    Topic,
    /// A QoS profile. Spec §3.7.
    QosProfile,
    /// A domain participant. Spec §3.7.
    Participant,
    /// A reader or writer endpoint. Spec §3.7.
    Endpoint,
    /// A deployment library. Spec §3.7.
    Library,
}

impl NodeKind {
    /// Every kind, in tag order. Lets a conformance test assert that the
    /// tag mapping is total and collision-free.
    pub const ALL: [Self; 22] = [
        Self::Module,
        Self::Type,
        Self::Member,
        Self::Case,
        Self::Enumerator,
        Self::BitValue,
        Self::Bitfield,
        Self::Const,
        Self::ConstExpr,
        Self::Interface,
        Self::Operation,
        Self::Param,
        Self::Attribute,
        Self::Exception,
        Self::AnnotationDef,
        Self::AnnotationMember,
        Self::Domain,
        Self::Topic,
        Self::QosProfile,
        Self::Participant,
        Self::Endpoint,
        Self::Library,
    ];

    /// Wire tag for changesets and snapshots.
    #[must_use]
    pub const fn tag(self) -> NodeKindTag {
        NodeKindTag(match self {
            Self::Module => 1,
            Self::Type => 2,
            Self::Member => 3,
            Self::Case => 4,
            Self::Enumerator => 5,
            Self::BitValue => 6,
            Self::Bitfield => 7,
            Self::Const => 8,
            Self::ConstExpr => 9,
            Self::Interface => 10,
            Self::Operation => 11,
            Self::Param => 12,
            Self::Attribute => 13,
            Self::Exception => 14,
            Self::AnnotationDef => 15,
            Self::AnnotationMember => 16,
            Self::Domain => 17,
            Self::Topic => 18,
            Self::QosProfile => 19,
            Self::Participant => 20,
            Self::Endpoint => 21,
            Self::Library => 22,
        })
    }

    /// Inverse of [`NodeKind::tag`].
    #[must_use]
    pub const fn from_tag(tag: NodeKindTag) -> Option<Self> {
        Some(match tag.0 {
            1 => Self::Module,
            2 => Self::Type,
            3 => Self::Member,
            4 => Self::Case,
            5 => Self::Enumerator,
            6 => Self::BitValue,
            7 => Self::Bitfield,
            8 => Self::Const,
            9 => Self::ConstExpr,
            10 => Self::Interface,
            11 => Self::Operation,
            12 => Self::Param,
            13 => Self::Attribute,
            14 => Self::Exception,
            15 => Self::AnnotationDef,
            16 => Self::AnnotationMember,
            17 => Self::Domain,
            18 => Self::Topic,
            19 => Self::QosProfile,
            20 => Self::Participant,
            21 => Self::Endpoint,
            22 => Self::Library,
            _ => return None,
        })
    }

    /// Short name, for diagnostics and `BreakCause::KindMismatch`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Type => "type",
            Self::Member => "member",
            Self::Case => "case",
            Self::Enumerator => "enumerator",
            Self::BitValue => "bit value",
            Self::Bitfield => "bitfield",
            Self::Const => "constant",
            Self::ConstExpr => "constant expression",
            Self::Interface => "interface",
            Self::Operation => "operation",
            Self::Param => "parameter",
            Self::Attribute => "attribute",
            Self::Exception => "exception",
            Self::AnnotationDef => "annotation",
            Self::AnnotationMember => "annotation member",
            Self::Domain => "domain",
            Self::Topic => "topic",
            Self::QosProfile => "QoS profile",
            Self::Participant => "participant",
            Self::Endpoint => "endpoint",
            Self::Library => "library",
        }
    }
}
