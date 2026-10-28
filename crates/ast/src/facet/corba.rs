// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CORBA and component-model information. Spec §4.2.

use crate::facet::applied::AppliedArgument;
use crate::node::constant::ConstExprNode;
use crate::node::interface::InterfaceNode;
use crate::node::type_::TypeNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, Ref, RepositoryId};

/// How a repository identifier was arrived at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepositoryIdOrigin {
    /// Derived from the scoped name by the default rule.
    Derived,
    /// Influenced by `#pragma prefix`.
    PragmaPrefix,
    /// Influenced by `#pragma version`.
    PragmaVersion,
    /// Set outright by `#pragma ID`.
    PragmaId,
}

/// The role a declaration plays in the CORBA component model.
///
/// Carried here rather than as core node kinds, so that CCM support does
/// not widen the type space every consumer has to match on. Spec §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentRole {
    /// `provides` — a facet the component offers.
    Facet {
        /// Port name.
        name: String,
        /// Interface offered.
        interface: Ref<InterfaceNode>,
    },
    /// `uses` — a receptacle the component consumes.
    Receptacle {
        /// Port name.
        name: String,
        /// Interface consumed.
        interface: Ref<InterfaceNode>,
        /// Whether the receptacle accepts multiple connections.
        multiple: bool,
    },
    /// `publishes` or `emits` — an event source.
    EventSource {
        /// Port name.
        name: String,
        /// Event type.
        event: Ref<TypeNode>,
        /// `publishes` when true, `emits` when false.
        publisher: bool,
    },
    /// `consumes` — an event sink.
    EventSink {
        /// Port name.
        name: String,
        /// Event type.
        event: Ref<TypeNode>,
    },
    /// A `home` factory operation.
    HomeFactory,
    /// A `home` finder operation.
    HomeFinder,
    /// A member of a `porttype` declaration.
    PortTypeRole,
    /// A member of a `connector` declaration.
    ConnectorRole,
}

/// Whether a valuetype state member is `public` or `private`.
///
/// IDL 4.2 §7.4.4, rule (106): a state member is written with one of the
/// two keywords and there is no third option, so the model records which
/// one rather than reconstructing it. CCM 4.0 §6.7.2 forbids a private
/// state member in a primary-key valuetype, which is a rule that cannot
/// be checked at all if the keyword is dropped at lowering.
///
/// Here rather than on [`crate::MemberNode`]: DDS-XTypes has no
/// valuetypes and so no visibility, and a struct member has none either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateVisibility {
    /// `public` — part of the marshalled state and readable.
    Public,
    /// `private` — part of the marshalled state, not readable.
    Private,
}

impl StateVisibility {
    /// Both forms rule (106) defines.
    pub const ALL: [Self; 2] = [Self::Public, Self::Private];

    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// Kind of a template module formal parameter. IDL 4.2 §7.4.15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateParamKind {
    /// `typename`
    TypeName,
    /// `const <type>`
    Const,
    /// `sequence`
    Sequence,
    /// `interface`
    Interface,
    /// `valuetype`
    ValueType,
    /// `eventtype`
    EventType,
    /// `struct`
    Struct,
    /// `union`
    Union,
    /// `exception`
    Exception,
    /// `enum`
    Enum,
}

impl TemplateParamKind {
    /// Every kind IDL 4.2 §7.4.15 defines, in specification order.
    ///
    /// The order is the snapshot's tag numbering, so a kind is appended
    /// rather than inserted.
    pub const ALL: [Self; 10] = [
        Self::TypeName,
        Self::Const,
        Self::Sequence,
        Self::Interface,
        Self::ValueType,
        Self::EventType,
        Self::Struct,
        Self::Union,
        Self::Exception,
        Self::Enum,
    ];
}

/// One formal parameter of a template module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParam {
    /// Parameter name.
    pub name: String,
    /// What kind of entity it stands for.
    pub kind: TemplateParamKind,
    /// Constraining type, for `const` and constrained parameters.
    pub constraint: Option<Ref<TypeNode>>,
}

/// One actual argument of a template module instantiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateArg {
    /// A type argument.
    Type(Ref<TypeNode>),
    /// A constant argument.
    Const(Id<ConstExprNode>),
}

/// CORBA information attached to a node. Spec §4.2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorbaFacet {
    /// Repository identifier.
    pub repository_id: Option<RepositoryId>,
    /// How that identifier was arrived at.
    pub repository_id_origin: Option<RepositoryIdOrigin>,
    /// Component-model role, for CCM declarations.
    pub component_role: Option<ComponentRole>,
    /// Interfaces named by a `supports` clause.
    ///
    /// A `component`, `home` or `connector` writes two inheritance
    /// clauses — `: Base` and `supports I1, I2` — and they mean different
    /// things: CCM 4.0 §6.3.2 adds `Components::CCMObject` only when
    /// there is no base, and adds every supported interface on top of
    /// whichever base applies. Merging both into
    /// [`crate::InterfaceNode::bases`] would make the two
    /// indistinguishable and the §6.3.2 mapping unwritable.
    ///
    /// A valuetype's `supports` lives in
    /// [`crate::TypeKind::Valuetype::supported`]; this is the interface
    /// side of the same clause.
    pub supports: Vec<Ref<InterfaceNode>>,
    /// `public` or `private`, on a valuetype state member.
    pub state_visibility: Option<StateVisibility>,
    /// Whether the declaration was written `eventtype` rather than
    /// `valuetype`. IDL 4.2 §7.4.7, rule (166).
    ///
    /// The two lower to the same kind of node — an eventtype is a
    /// valuetype with an extra rule attached — and the keyword is the
    /// only thing that tells them apart. CCM 4.0 §6.6.1.1 maps an
    /// eventtype onto a valuetype deriving from
    /// `Components::EventBase` plus a `<E>Consumer` interface, and maps
    /// a plain valuetype onto neither, so dropping the keyword makes the
    /// clause unapplicable.
    pub is_event_type: bool,
    /// Formal parameters, on a template module.
    pub template_params: Vec<TemplateParam>,
    /// Actual arguments, on a template module instantiation.
    pub template_args: Vec<TemplateArg>,
    /// Mapping hint for a `native` declaration.
    pub native_mapping: Option<String>,
    /// `@service`, marking an interface as a service. IDL 4.2 §8.3.4.
    pub is_service: bool,
    /// The arguments written on that `@service`, in source order.
    ///
    /// Kept whole rather than folded into one named field because the two
    /// specifications that read this annotation disagree about what its
    /// argument means: IDL 4.2 §8.3.4 declares it `string platform
    /// default "*"`, and DDS-RPC 1.0 §7.3 reads a service name from the
    /// same place. Choosing one here would make the other unwritable.
    ///
    /// Empty for the bare `@service`. Before this field existed the
    /// arguments were discarded on the way in, which made `@service` and
    /// `@service(name = "X")` indistinguishable to every consumer.
    pub service_arguments: Vec<AppliedArgument>,
    /// `@ami`, requesting the asynchronous method invocation mapping.
    /// IDL 4.2 §8.3.5.
    pub is_ami: bool,
    /// What a `home` manages, and the key it does so by.
    /// IDL 4.2 §7.4.9.
    pub manages: Option<HomeManages>,
    /// `import` directives, as written. IDL 4.2 §7.4.13.
    pub imports: Vec<String>,
    /// `typeid` and `typeprefix` directives, as written. IDL 4.2 §7.4.14.
    ///
    /// The directives leave no declaration of their own: `typeid` writes a
    /// repository id onto its target and `typeprefix` seeds one, so the
    /// target nodes carry no mark that the file used one. DDS-RPC 1.0
    /// §7.3.1.1 removes `<type_id_dcl>` and `<type_prefix_dcl>` from the
    /// service-definition grammar, so the RPC layer has to be able to see
    /// that a service definition used one. Kept on the root module
    /// because the directives are file-scoped.
    pub repository_directives: Vec<String>,
}

/// A home's `manages X primarykey K` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeManages {
    /// The component the home manages.
    pub component: Ref<InterfaceNode>,
    /// The primary key type, when one was named.
    pub primary_key: Option<Ref<TypeNode>>,
}
