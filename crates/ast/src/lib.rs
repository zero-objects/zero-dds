// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-ast`. Safety classification: **SAFE**.
//!
//! The common AST: one strictly typed model that every ZeroDDS type
//! frontend lowers into and every backend reads from. A source-neutral
//! core node space carries per-source typed facets, so a construct from
//! IDL, XTypes, CORBA, ROS 2, DDS-XML or the FlatData model is
//! representable without loss and without a stringly-typed escape hatch.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-ast-1.0.md` §3 (core node space), §4 (facets),
//!   §8 (validation).
//! - Feature sources: OMG IDL 4.2, DDS-XTypes 1.3, DDS-DCPS 1.4,
//!   DDS-XML 1.0, DDS-RPC 1.0, CORBA 3.3, CCM 4.0, ROS 2 interface
//!   definitions.
//!
//! ## Layer position
//!
//! Layer 1 — over `zerodds-ast-core`. Direct dependents:
//! `zerodds-ast-layout`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

// The unit-test harness itself needs std; the crate does not.
#[cfg(test)]
extern crate std;

mod decimal;
pub mod edit;
pub mod error;
pub mod facet;
pub mod field;
pub mod invariant;
pub mod kind;
pub mod model;
pub mod node;
pub mod profile;
pub mod query;
pub mod refs;
mod resolve;
pub mod snapshot;

pub use edit::AstTransaction;
pub use error::AstError;
pub use facet::{
    AppliedAnnotation, AppliedArgument, AutoId, ByteSpan, ComponentRole, CorbaFacet, CoreDxFacet,
    DataRepresentationMask, Extensibility, Facets, FastDdsFacet, FlatDataFacet, HomeManages,
    MacroExpansion, MemberIdAssignment, MemberIdOrigin, OpenDdsFacet, RangeConstraint,
    RepositoryIdOrigin, Ros2Facet, RosBoundedOrigin, RosInterfaceRole, RtiFacet, SourceFacet,
    SourceOrigin, StateVisibility, TemplateArg, TemplateParam, TemplateParamKind, TopicAnnotation,
    TryConstruct, VendorFacets, VendorKind, VerbatimDirective, VerbatimPlacement, XTypesFacet,
    XmlFacet,
};
/// The extensibility a type has when its source said nothing.
///
/// Appendable, not final: DDS-XTypes 1.3 §7.2.4.4. A consumer assuming
/// final would emit types that cannot evolve.
pub const DEFAULT_EXTENSIBILITY: facet::Extensibility = facet::Extensibility::Appendable;

/// The bit bound an enum or bitmask has when its source said nothing.
/// DDS-XTypes 1.3 §7.3.1.2.1.9.
pub const DEFAULT_BIT_BOUND: u16 = 32;

pub use invariant::InvariantViolation;
pub use kind::NodeKind;
pub use model::{Ast, MemberSpec, primitive};
pub use node::{
    AnnotationDefNode, AnnotationMemberNode, AnyId, AttributeNode, BinaryOp, BitValueNode,
    BitfieldNode, Bound, CaseLabel, CaseNode, CharWidth, Completeness, ConstExpr, ConstExprNode,
    ConstNode, ConstValue, DomainNode, EndpointNode, EndpointRole, EnumeratorNode, EvalError,
    ExceptionNode, InterfaceKind, InterfaceNode, LibraryNode, Literal, MemberNode, ModuleNode,
    OperationNode, ParamDirection, ParamNode, ParticipantNode, PrimitiveKind, QosPolicy,
    QosPolicyKind, QosProfileNode, TopicNode, TypeKind, TypeNode, UnaryOp, ValuetypeKind,
};
pub use profile::{
    ConstructKind, FacetProperty, Profile, ProfileViolation, Verdict, construct_verdict,
    facet_verdict,
};
pub use refs::{PendingSlot, RefKind};
pub use resolve::ResolveReport;
