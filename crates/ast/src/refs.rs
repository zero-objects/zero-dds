// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every reference a node holds, in one enumeration. Spec §2.
//!
//! Resolution needs three things from a node: which references it has,
//! what each one may point at, and how to write one back. Spelling that
//! out per node kind at every call site is how a reference gets
//! forgotten — a constant named in an array dimension stayed pending
//! forever because only member and type references were ever indexed.
//!
//! The slot is a reference's position in this list. The undo log names
//! references by it, so the order has to be stable for a given node.

use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::TypeKind;
use alloc::vec::Vec;
use zerodds_ast_core::{BreakCause, PendingRef, Ref};

/// What a reference is allowed to point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// A type.
    Type,
    /// An interface.
    Interface,
    /// An exception.
    Exception,
    /// A constant.
    Const,
    /// An enumerator.
    ///
    /// Distinct from [`RefKind::Const`]: an enumerator is admitted
    /// wherever a `<const_expr>` is — OMG IDL 4.2 §7.4.1.4.2 — but it is
    /// not a constant node and does not live in the constants arena.
    Enumerator,
    /// An annotation declaration.
    AnnotationDef,
}

impl RefKind {
    /// Every kind a reference may have.
    pub const ALL: [Self; 6] = [
        Self::Type,
        Self::Interface,
        Self::Exception,
        Self::Const,
        Self::Enumerator,
        Self::AnnotationDef,
    ];

    /// The name a diagnostic uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Interface => "interface",
            Self::Exception => "exception",
            Self::Const => "constant",
            Self::Enumerator => "enumerator",
            Self::AnnotationDef => "annotation",
        }
    }

    /// Whether `any` is a node this kind of reference may point at.
    #[must_use]
    pub const fn accepts(self, any: AnyId) -> bool {
        matches!(
            (self, any),
            (Self::Type, AnyId::Type(..))
                | (Self::Interface, AnyId::Interface(..))
                | (Self::Exception, AnyId::Exception(..))
                | (Self::Const, AnyId::Const(..))
                | (Self::Enumerator, AnyId::Enumerator(..))
                | (Self::AnnotationDef, AnyId::AnnotationDef(..))
        )
    }
}

/// One pending reference of a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSlot {
    /// Position in the node's reference list.
    pub slot: usize,
    /// What it waits on.
    pub key: PendingRef,
    /// What it may point at.
    pub kind: RefKind,
}

impl Ast {
    /// Every pending reference a node holds.
    ///
    /// The one place that knows where references live, so adding a
    /// reference to a node kind is one edit rather than four.
    #[must_use]
    pub fn pending_slots(&self, any: AnyId) -> Vec<PendingSlot> {
        let mut out = Vec::new();
        let mut slot = 0usize;
        let mut push = |reference: Option<&PendingRef>, kind: RefKind, at: usize| {
            if let Some(key) = reference {
                out.push(PendingSlot {
                    slot: at,
                    key: key.clone(),
                    kind,
                });
            }
        };

        match any {
            AnyId::Type(id, _) => {
                let Ok(node) = self.types.get(id) else {
                    return out;
                };
                for reference in node.kind.type_refs() {
                    push(reference.pending_key(), RefKind::Type, slot);
                    slot += 1;
                }
                for reference in interface_refs_of_kind(&node.kind) {
                    push(reference.pending_key(), RefKind::Interface, slot);
                    slot += 1;
                }
            }
            AnyId::Member(id, _) => {
                if let Ok(node) = self.members.get(id) {
                    push(node.type_.pending_key(), RefKind::Type, slot);
                }
            }
            // A union case holds its member by handle; the type
            // reference lives on that member and is indexed there.
            AnyId::Case(..) => {}
            AnyId::Bitfield(id, _) => {
                if let Ok(node) = self.bitfields.get(id) {
                    push(
                        node.holder_type.as_ref().and_then(Ref::pending_key),
                        RefKind::Type,
                        slot,
                    );
                }
            }
            AnyId::Const(id, _) => {
                if let Ok(node) = self.constants.get(id) {
                    push(node.type_.pending_key(), RefKind::Type, slot);
                }
            }
            AnyId::ConstExpr(id, _) => {
                if let Ok(node) = self.exprs.get(id) {
                    match &node.expr {
                        crate::ConstExpr::NameRef(reference) => {
                            push(reference.pending_key(), RefKind::Const, slot);
                        }
                        crate::ConstExpr::Literal(crate::Literal::Enumerator(reference)) => {
                            push(reference.pending_key(), RefKind::Enumerator, slot);
                        }
                        _ => {}
                    }
                }
            }
            AnyId::Interface(id, _) => {
                let Ok(node) = self.interfaces.get(id) else {
                    return out;
                };
                for base in &node.bases {
                    push(base.pending_key(), RefKind::Interface, slot);
                    slot += 1;
                }
            }
            AnyId::Operation(id, _) => {
                let Ok(node) = self.operations.get(id) else {
                    return out;
                };
                push(
                    node.return_type.as_ref().and_then(Ref::pending_key),
                    RefKind::Type,
                    slot,
                );
                slot += 1;
                for raise in &node.raises {
                    push(raise.pending_key(), RefKind::Exception, slot);
                    slot += 1;
                }
            }
            AnyId::Param(id, _) => {
                if let Ok(node) = self.params.get(id) {
                    push(node.type_.pending_key(), RefKind::Type, slot);
                }
            }
            AnyId::Attribute(id, _) => {
                let Ok(node) = self.attributes.get(id) else {
                    return out;
                };
                push(node.type_.pending_key(), RefKind::Type, slot);
                slot += 1;
                for raise in node.get_raises.iter().chain(node.set_raises.iter()) {
                    push(raise.pending_key(), RefKind::Exception, slot);
                    slot += 1;
                }
            }
            AnyId::AnnotationMember(id, _) => {
                if let Ok(node) = self.annotation_members.get(id) {
                    push(node.type_.pending_key(), RefKind::Type, slot);
                }
            }
            AnyId::Module(..)
            | AnyId::Enumerator(..)
            | AnyId::BitValue(..)
            | AnyId::Exception(..)
            | AnyId::AnnotationDef(..)
            | AnyId::Domain(..)
            | AnyId::Topic(..)
            | AnyId::QosProfile(..)
            | AnyId::Participant(..)
            | AnyId::Endpoint(..)
            | AnyId::Library(..) => {}
        }

        // The CORBA facet holds references of its own — what a home
        // manages, the interface or event type behind a port, a template
        // parameter's constraint. They were unreachable from here, so
        // they were never indexed and never resolved: `manages C
        // primarykey K` sat `Pending` for the life of the model however
        // many resolution passes ran over it.
        if let Some(corba) = self.facets(any).and_then(|facets| facets.corba.as_ref()) {
            for (index, reference) in corba_refs(corba).into_iter().enumerate() {
                let (key, kind) = match reference {
                    CorbaRef::Interface(reference) => (reference.pending_key(), RefKind::Interface),
                    CorbaRef::Type(reference) => (reference.pending_key(), RefKind::Type),
                };
                if let Some(key) = key {
                    out.push(PendingSlot {
                        slot: CORBA_SLOT_BASE + index,
                        key: key.clone(),
                        kind,
                    });
                }
            }
        }

        // Applied annotations wait on their declaration, whatever kind of
        // node they sit on.
        let base = slot.max(FACET_SLOT_BASE);
        if let Some(facets) = self.facets(any) {
            for (index, applied) in facets.annotations.iter().enumerate() {
                if let Some(key) = applied.definition.pending_key() {
                    out.push(PendingSlot {
                        slot: base + index,
                        key: key.clone(),
                        kind: RefKind::AnnotationDef,
                    });
                }
            }
        }
        out
    }

    /// Write a reference back, by slot.
    ///
    /// Returns whether the slot existed and changed.
    pub(crate) fn set_slot(&mut self, any: AnyId, slot: usize, target: AnyId) -> bool {
        if slot >= FACET_SLOT_BASE {
            return self.set_applied_annotation(any, slot - FACET_SLOT_BASE, target);
        }
        if slot >= CORBA_SLOT_BASE {
            let index = slot - CORBA_SLOT_BASE;
            let Some(corba) = self.facets_mut(any).and_then(|f| f.corba.as_mut()) else {
                return false;
            };
            return match (corba_refs_mut(corba).into_iter().nth(index), target) {
                (Some(CorbaRefMut::Interface(reference)), AnyId::Interface(id, _)) => {
                    replace(reference, id)
                }
                (Some(CorbaRefMut::Type(reference)), AnyId::Type(id, _)) => replace(reference, id),
                _ => false,
            };
        }
        match (any, target) {
            (AnyId::Type(id, _), AnyId::Type(target, _)) => self
                .types
                .get_mut(id)
                .is_ok_and(|node| set_type_ref_in_kind(&mut node.kind, slot, target)),
            (AnyId::Type(id, _), AnyId::Interface(target, _)) => {
                self.types.get_mut(id).is_ok_and(|node| {
                    let offset = node.kind.type_refs().len();
                    set_interface_ref_in_kind(&mut node.kind, slot.wrapping_sub(offset), target)
                })
            }
            (AnyId::Member(id, _), AnyId::Type(target, _)) => self
                .members
                .get_mut(id)
                .is_ok_and(|node| replace(&mut node.type_, target)),
            (AnyId::Bitfield(id, _), AnyId::Type(target, _)) => {
                self.bitfields.get_mut(id).is_ok_and(|node| {
                    node.holder_type
                        .as_mut()
                        .is_some_and(|reference| replace(reference, target))
                })
            }
            (AnyId::Const(id, _), AnyId::Type(target, _)) => self
                .constants
                .get_mut(id)
                .is_ok_and(|node| replace(&mut node.type_, target)),
            (AnyId::ConstExpr(id, _), AnyId::Const(target, _)) => {
                self.exprs.get_mut(id).is_ok_and(|node| {
                    if let crate::ConstExpr::NameRef(reference) = &mut node.expr {
                        replace(reference, target)
                    } else {
                        false
                    }
                })
            }
            (AnyId::ConstExpr(id, _), AnyId::Enumerator(target, _)) => {
                self.exprs.get_mut(id).is_ok_and(|node| {
                    if let crate::ConstExpr::Literal(crate::Literal::Enumerator(reference)) =
                        &mut node.expr
                    {
                        replace(reference, target)
                    } else {
                        false
                    }
                })
            }
            (AnyId::Interface(id, _), AnyId::Interface(target, _)) => self
                .interfaces
                .get_mut(id)
                .is_ok_and(|node| node.bases.get_mut(slot).is_some_and(|b| replace(b, target))),
            (AnyId::Operation(id, _), AnyId::Type(target, _)) => {
                self.operations.get_mut(id).is_ok_and(|node| {
                    slot == 0
                        && node
                            .return_type
                            .as_mut()
                            .is_some_and(|reference| replace(reference, target))
                })
            }
            (AnyId::Operation(id, _), AnyId::Exception(target, _)) => {
                self.operations.get_mut(id).is_ok_and(|node| {
                    node.raises
                        .get_mut(slot.wrapping_sub(1))
                        .is_some_and(|reference| replace(reference, target))
                })
            }
            (AnyId::Param(id, _), AnyId::Type(target, _)) => self
                .params
                .get_mut(id)
                .is_ok_and(|node| replace(&mut node.type_, target)),
            (AnyId::Attribute(id, _), AnyId::Type(target, _)) => self
                .attributes
                .get_mut(id)
                .is_ok_and(|node| slot == 0 && replace(&mut node.type_, target)),
            (AnyId::Attribute(id, _), AnyId::Exception(target, _)) => {
                self.attributes.get_mut(id).is_ok_and(|node| {
                    let at = slot.wrapping_sub(1);
                    let getters = node.get_raises.len();
                    if at < getters {
                        node.get_raises
                            .get_mut(at)
                            .is_some_and(|reference| replace(reference, target))
                    } else {
                        node.set_raises
                            .get_mut(at - getters)
                            .is_some_and(|reference| replace(reference, target))
                    }
                })
            }
            (AnyId::AnnotationMember(id, _), AnyId::Type(target, _)) => self
                .annotation_members
                .get_mut(id)
                .is_ok_and(|node| replace(&mut node.type_, target)),
            _ => false,
        }
    }

    /// Put a reference into the broken state, by slot.
    pub(crate) fn break_slot(&mut self, any: AnyId, slot: usize, cause: BreakCause) -> bool {
        if slot >= FACET_SLOT_BASE {
            let index = slot - FACET_SLOT_BASE;
            let Some(facets) = self.facets_mut(any) else {
                return false;
            };
            return facets
                .annotations
                .get_mut(index)
                .is_some_and(|applied| break_ref(&mut applied.definition, cause));
        }
        if slot >= CORBA_SLOT_BASE {
            let index = slot - CORBA_SLOT_BASE;
            let Some(corba) = self.facets_mut(any).and_then(|f| f.corba.as_mut()) else {
                return false;
            };
            return match corba_refs_mut(corba).into_iter().nth(index) {
                Some(CorbaRefMut::Interface(reference)) => break_ref(reference, cause),
                Some(CorbaRefMut::Type(reference)) => break_ref(reference, cause),
                None => false,
            };
        }
        match any {
            AnyId::Type(id, _) => self.types.get_mut(id).is_ok_and(|node| {
                let types = node.kind.type_refs_mut().len();
                if slot < types {
                    node.kind
                        .type_refs_mut()
                        .into_iter()
                        .nth(slot)
                        .is_some_and(|reference| break_ref(reference, cause))
                } else {
                    break_interface_ref_in_kind(&mut node.kind, slot - types, cause)
                }
            }),
            AnyId::Member(id, _) => self
                .members
                .get_mut(id)
                .is_ok_and(|node| break_ref(&mut node.type_, cause)),
            AnyId::Bitfield(id, _) => self.bitfields.get_mut(id).is_ok_and(|node| {
                node.holder_type
                    .as_mut()
                    .is_some_and(|reference| break_ref(reference, cause))
            }),
            AnyId::Const(id, _) => self
                .constants
                .get_mut(id)
                .is_ok_and(|node| break_ref(&mut node.type_, cause)),
            AnyId::ConstExpr(id, _) => {
                self.exprs
                    .get_mut(id)
                    .is_ok_and(|node| match &mut node.expr {
                        crate::ConstExpr::NameRef(reference) => break_ref(reference, cause),
                        crate::ConstExpr::Literal(crate::Literal::Enumerator(reference)) => {
                            break_ref(reference, cause)
                        }
                        _ => false,
                    })
            }
            AnyId::Interface(id, _) => self.interfaces.get_mut(id).is_ok_and(|node| {
                node.bases
                    .get_mut(slot)
                    .is_some_and(|reference| break_ref(reference, cause))
            }),
            AnyId::Operation(id, _) => self.operations.get_mut(id).is_ok_and(|node| {
                if slot == 0 {
                    node.return_type
                        .as_mut()
                        .is_some_and(|reference| break_ref(reference, cause))
                } else {
                    node.raises
                        .get_mut(slot - 1)
                        .is_some_and(|reference| break_ref(reference, cause))
                }
            }),
            AnyId::Param(id, _) => self
                .params
                .get_mut(id)
                .is_ok_and(|node| break_ref(&mut node.type_, cause)),
            AnyId::Attribute(id, _) => self.attributes.get_mut(id).is_ok_and(|node| {
                if slot == 0 {
                    return break_ref(&mut node.type_, cause);
                }
                let at = slot - 1;
                let getters = node.get_raises.len();
                if at < getters {
                    node.get_raises
                        .get_mut(at)
                        .is_some_and(|reference| break_ref(reference, cause))
                } else {
                    node.set_raises
                        .get_mut(at - getters)
                        .is_some_and(|reference| break_ref(reference, cause))
                }
            }),
            AnyId::AnnotationMember(id, _) => self
                .annotation_members
                .get_mut(id)
                .is_ok_and(|node| break_ref(&mut node.type_, cause)),
            _ => false,
        }
    }

    fn set_applied_annotation(&mut self, any: AnyId, index: usize, target: AnyId) -> bool {
        let AnyId::AnnotationDef(definition, _) = target else {
            return false;
        };
        let Some(facets) = self.facets_mut(any) else {
            return false;
        };
        facets
            .annotations
            .get_mut(index)
            .is_some_and(|applied| replace(&mut applied.definition, definition))
    }
}

/// Where the applied-annotation slots begin.
///
/// Far above any node's own reference count, so a facet slot can never
/// be mistaken for a structural one. Node reference lists are short —
/// the widest is a valuetype's base list — and this leaves room.
const FACET_SLOT_BASE: usize = 1 << 20;

/// Where the CORBA-facet slots begin.
///
/// Its own region below [`FACET_SLOT_BASE`], for the same reason: a
/// facet reference must never be mistaken for a structural one, and the
/// two facet regions must not be mistaken for each other.
const CORBA_SLOT_BASE: usize = 1 << 19;

/// One reference held by a [`crate::CorbaFacet`], borrowed.
enum CorbaRef<'a> {
    /// Names an interface.
    Interface(&'a Ref<crate::InterfaceNode>),
    /// Names a type.
    Type(&'a Ref<crate::TypeNode>),
}

/// The same, mutably.
enum CorbaRefMut<'a> {
    /// Names an interface.
    Interface(&'a mut Ref<crate::InterfaceNode>),
    /// Names a type.
    Type(&'a mut Ref<crate::TypeNode>),
}

/// Every reference a CORBA facet holds, in slot order.
///
/// The order is the slot numbering, so a reference is appended here
/// rather than inserted; [`corba_refs_mut`] walks it identically and the
/// two have to stay in step.
fn corba_refs(corba: &crate::CorbaFacet) -> Vec<CorbaRef<'_>> {
    let mut out = Vec::new();
    match corba.component_role.as_ref() {
        Some(crate::ComponentRole::Facet { interface, .. }) => {
            out.push(CorbaRef::Interface(interface));
        }
        Some(crate::ComponentRole::Receptacle { interface, .. }) => {
            out.push(CorbaRef::Interface(interface));
        }
        Some(crate::ComponentRole::EventSource { event, .. }) => {
            out.push(CorbaRef::Type(event));
        }
        Some(crate::ComponentRole::EventSink { event, .. }) => {
            out.push(CorbaRef::Type(event));
        }
        Some(
            crate::ComponentRole::HomeFactory
            | crate::ComponentRole::HomeFinder
            | crate::ComponentRole::PortTypeRole
            | crate::ComponentRole::ConnectorRole,
        )
        | None => {}
    }
    if let Some(manages) = corba.manages.as_ref() {
        out.push(CorbaRef::Interface(&manages.component));
        if let Some(key) = manages.primary_key.as_ref() {
            out.push(CorbaRef::Type(key));
        }
    }
    for interface in &corba.supports {
        out.push(CorbaRef::Interface(interface));
    }
    for param in &corba.template_params {
        if let Some(constraint) = param.constraint.as_ref() {
            out.push(CorbaRef::Type(constraint));
        }
    }
    for arg in &corba.template_args {
        if let crate::TemplateArg::Type(reference) = arg {
            out.push(CorbaRef::Type(reference));
        }
    }
    out
}

/// The same references, mutably and in the same order.
fn corba_refs_mut(corba: &mut crate::CorbaFacet) -> Vec<CorbaRefMut<'_>> {
    let mut out = Vec::new();
    match corba.component_role.as_mut() {
        Some(crate::ComponentRole::Facet { interface, .. }) => {
            out.push(CorbaRefMut::Interface(interface));
        }
        Some(crate::ComponentRole::Receptacle { interface, .. }) => {
            out.push(CorbaRefMut::Interface(interface));
        }
        Some(crate::ComponentRole::EventSource { event, .. }) => {
            out.push(CorbaRefMut::Type(event));
        }
        Some(crate::ComponentRole::EventSink { event, .. }) => {
            out.push(CorbaRefMut::Type(event));
        }
        Some(
            crate::ComponentRole::HomeFactory
            | crate::ComponentRole::HomeFinder
            | crate::ComponentRole::PortTypeRole
            | crate::ComponentRole::ConnectorRole,
        )
        | None => {}
    }
    if let Some(manages) = corba.manages.as_mut() {
        out.push(CorbaRefMut::Interface(&mut manages.component));
        if let Some(key) = manages.primary_key.as_mut() {
            out.push(CorbaRefMut::Type(key));
        }
    }
    for interface in &mut corba.supports {
        out.push(CorbaRefMut::Interface(interface));
    }
    for param in &mut corba.template_params {
        if let Some(constraint) = param.constraint.as_mut() {
            out.push(CorbaRefMut::Type(constraint));
        }
    }
    for arg in &mut corba.template_args {
        if let crate::TemplateArg::Type(reference) = arg {
            out.push(CorbaRefMut::Type(reference));
        }
    }
    out
}

fn replace<T>(reference: &mut Ref<T>, target: zerodds_ast_core::Id<T>) -> bool {
    if reference.is_pending() {
        *reference = Ref::Resolved(target);
        return true;
    }
    false
}

fn break_ref<T>(reference: &mut Ref<T>, cause: BreakCause) -> bool {
    if reference.is_pending() {
        *reference = Ref::Broken(cause);
        return true;
    }
    false
}

fn break_interface_ref_in_kind(kind: &mut TypeKind, slot: usize, cause: BreakCause) -> bool {
    match kind {
        TypeKind::Valuetype { supported, .. } => supported
            .get_mut(slot)
            .is_some_and(|reference| break_ref(reference, cause)),
        // The generic `Object` names no interface — rule (117) — so
        // there is no slot zero to break.
        TypeKind::ObjectRef {
            interface: Some(interface),
        } => slot == 0 && break_ref(interface, cause),
        _ => false,
    }
}

/// The interface references a type kind holds, after its type ones.
fn interface_refs_of_kind(kind: &TypeKind) -> Vec<&Ref<crate::InterfaceNode>> {
    match kind {
        TypeKind::Valuetype { supported, .. } => supported.iter().collect(),
        TypeKind::ObjectRef {
            interface: Some(interface),
        } => alloc::vec![interface],
        _ => Vec::new(),
    }
}

fn set_type_ref_in_kind(
    kind: &mut TypeKind,
    slot: usize,
    target: zerodds_ast_core::Id<crate::TypeNode>,
) -> bool {
    kind.type_refs_mut()
        .into_iter()
        .nth(slot)
        .is_some_and(|reference| replace(reference, target))
}

fn set_interface_ref_in_kind(
    kind: &mut TypeKind,
    slot: usize,
    target: zerodds_ast_core::Id<crate::InterfaceNode>,
) -> bool {
    match kind {
        TypeKind::Valuetype { supported, .. } => supported
            .get_mut(slot)
            .is_some_and(|reference| replace(reference, target)),
        TypeKind::ObjectRef {
            interface: Some(interface),
        } => slot == 0 && replace(interface, target),
        _ => false,
    }
}
