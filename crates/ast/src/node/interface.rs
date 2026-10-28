// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Interfaces, operations, parameters and attributes. Spec §3.4.

use crate::facet::Facets;
use crate::node::AnyId;
use crate::node::exception::ExceptionNode;
use crate::node::module::ModuleNode;
use crate::node::type_::{Completeness, TypeNode};
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId, Ref};

/// Which interface form this is. Spec §3.4.
///
/// `Component`, `Home`, `Connector` and `PortType` are CCM forms; their
/// role detail lives in `CorbaFacet` rather than in more core kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterfaceKind {
    /// `interface`
    Interface,
    /// `local interface`
    Local,
    /// `abstract interface`
    Abstract,
    /// `component`
    Component,
    /// `home`
    Home,
    /// `connector`
    Connector,
    /// `porttype`
    PortType,
}

impl InterfaceKind {
    /// Every form of spec §3.4.
    pub const ALL: [Self; 7] = [
        Self::Interface,
        Self::Local,
        Self::Abstract,
        Self::Component,
        Self::Home,
        Self::Connector,
        Self::PortType,
    ];

    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interface => "interface",
            Self::Local => "local interface",
            Self::Abstract => "abstract interface",
            Self::Component => "component",
            Self::Home => "home",
            Self::Connector => "connector",
            Self::PortType => "porttype",
        }
    }

    /// Whether this form belongs to the CORBA component model rather
    /// than to plain IDL interfaces.
    #[must_use]
    pub const fn is_component_model(self) -> bool {
        matches!(
            self,
            Self::Component | Self::Home | Self::Connector | Self::PortType
        )
    }
}

/// An interface or component form. Spec §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Enclosing module.
    pub module: Id<ModuleNode>,
    /// Which form.
    pub kind: InterfaceKind,
    /// Base interfaces, in declaration order.
    ///
    /// The order matters: CORBA multiple inheritance resolves operation
    /// lookup along it.
    pub bases: Vec<Ref<InterfaceNode>>,
    /// Operations in declaration order.
    pub operations: Vec<Id<OperationNode>>,
    /// Attributes in declaration order.
    pub attributes: Vec<Id<AttributeNode>>,
    /// Whether the body has arrived.
    pub completeness: Completeness,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// Parameter passing direction. Spec §3.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamDirection {
    /// `in`
    In,
    /// `out`
    Out,
    /// `inout`
    InOut,
}

impl ParamDirection {
    /// Every direction IDL defines.
    pub const ALL: [Self; 3] = [Self::In, Self::Out, Self::InOut];

    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::InOut => "inout",
        }
    }
}

/// An operation. Spec §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Return type, absent for a `void` operation.
    ///
    /// `void` is the absence of a return, not a data type: giving it a
    /// type node would put a value with no representation into the
    /// layout and profile tables. IDL 4.2 §7.4.3.5.
    pub return_type: Option<Ref<TypeNode>>,
    /// Parameters in declaration order.
    pub params: Vec<Id<ParamNode>>,
    /// Exceptions this operation may raise.
    pub raises: Vec<Ref<ExceptionNode>>,
    /// Whether the operation is `oneway`.
    pub oneway: bool,
    /// `context` identifiers, in declaration order.
    pub contexts: Vec<String>,
    /// The interface or valuetype this operation belongs to.
    pub parent: AnyId,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// An operation parameter. Spec §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Passing direction.
    pub direction: ParamDirection,
    /// Declared type.
    pub type_: Ref<TypeNode>,
    /// The operation this parameter belongs to.
    pub parent: Id<OperationNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// An interface attribute. Spec §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Declared type.
    pub type_: Ref<TypeNode>,
    /// Whether the attribute is `readonly`.
    pub readonly: bool,
    /// Exceptions the getter may raise.
    pub get_raises: Vec<Ref<ExceptionNode>>,
    /// Exceptions the setter may raise. A `readonly` attribute with a
    /// non-empty setter list is representable here and rejected by
    /// profile validation, not by the model. Spec §8.2.
    pub set_raises: Vec<Ref<ExceptionNode>>,
    /// The interface this attribute belongs to.
    pub parent: Id<InterfaceNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}
