// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Declarations a profile has no data type for.
//!
//! DDS-XTypes 1.3 §7.3.1.1.1 admits IDL constructs outside the building
//! blocks it names, with a rule for the compiler that does not support
//! them: it "shall ignore them or issue a warning rather than halting
//! with an error". `any`, `ValueBase`, `native` and an object reference
//! have no type identifier and no wire form in DDS-XTypes, so a
//! declaration that holds one — directly, through a member, an alias, a
//! collection element or a base — cannot get a codec or a `TypeObject`.
//!
//! [`Ast::drop_outside_profile`] is the ignoring: it removes those
//! declarations from the model and says which, so a code generator that
//! walks the model never meets one and the rest of the file is generated.
//! It does not decide for the caller: a front end that wants the model
//! unchanged (a CORBA flow that does carry `any`) does not call it.

use super::{Profile, Verdict, construct_verdict, verdict::ConstructKind};
use crate::error::AstError;
use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::TypeKind;
use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId};

/// A declaration [`Ast::drop_outside_profile`] removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    /// The scoped name of the declaration, or its kind when it has none.
    pub name: String,
    /// The construct that made it unrepresentable, as the profile's
    /// table spells it (`any`, `ValueBase`, `native`, ...).
    pub construct: &'static str,
    /// The paragraph that rules the construct out.
    pub spec_reference: &'static str,
    /// Why the profile has no type for the construct.
    pub detail: &'static str,
}

/// The kinds that stop a data type from being planned: no wire form and
/// no type identifier, so a declaration reaching one has neither a codec
/// nor a `TypeObject`.
const fn has_no_data_form(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Any | TypeKind::ValueBase | TypeKind::Native | TypeKind::ObjectRef { .. }
    )
}

/// A node that is itself a construct the profile rules out, and what the
/// profile says about it.
struct Seed {
    node: NodeId,
    construct: ConstructKind,
    spec_reference: &'static str,
    detail: &'static str,
}

impl Ast {
    /// Remove the data types that reach a construct `profile` has no type
    /// for, and return what was removed.
    ///
    /// A type is removed when it is, or reaches through members, cases,
    /// aliases, collection elements, map keys and values, bases or a
    /// discriminator, a construct `construct_verdict(profile, …)`
    /// rejects among `any`, `ValueBase`, `native` and an object
    /// reference. The construct's own node stays in the model (it is the
    /// shared node every `any` points at).
    ///
    /// A type that a constant, an operation, a parameter, an attribute,
    /// an annotation member or a topic names is **kept**: removing it
    /// would leave that declaration with a reference to nothing, and the
    /// caller would then be told about a broken reference instead of the
    /// construct. Such a type stays in the model and a code generator
    /// reports the construct by name, as before.
    ///
    /// Nothing is removed under a profile that admits all four.
    ///
    /// # Errors
    /// [`AstError`] when a removal breaks a structural rule of the model;
    /// the types removed before it stay removed, so a caller treats the
    /// model as spoilt.
    pub fn drop_outside_profile(&mut self, profile: Profile) -> Result<Vec<Dropped>, AstError> {
        // The nodes that are the offending construct, with what the
        // profile says about them.
        let mut seeds: Vec<Seed> = Vec::new();
        for (_, node) in self.types() {
            if !has_no_data_form(&node.kind) {
                continue;
            }
            let construct = ConstructKind::of(&node.kind);
            if let Verdict::Rejected {
                spec_reference,
                detail,
                ..
            } = construct_verdict(profile, construct)
            {
                seeds.push(Seed {
                    node: node.node_id,
                    construct,
                    spec_reference,
                    detail,
                });
            }
        }
        if seeds.is_empty() {
            return Ok(Vec::new());
        }

        let named_elsewhere = self.types_named_outside_the_type_graph();
        let mut doomed: Vec<(Id<crate::TypeNode>, NodeId, usize)> = Vec::new();
        for (id, node) in self.types() {
            if seeds.iter().any(|seed| seed.node == node.node_id) {
                continue;
            }
            let reach = self.reachable_types(id);
            if let Some(seed) = seeds.iter().position(|seed| reach.contains(seed.node)) {
                if !named_elsewhere.contains(&node.node_id) {
                    doomed.push((id, node.node_id, seed));
                }
            }
        }

        // The scoped names are read before anything is removed: a name
        // cannot be asked of a node that is gone. An anonymous type — the
        // `sequence<any>` behind a typedef — is removed with the
        // declaration that holds it and is not reported on its own.
        let names: Vec<Option<String>> = doomed
            .iter()
            .map(|(id, node, _)| {
                if self.types.get(*id).map_or(true, |t| t.name.is_empty()) {
                    return None;
                }
                self.scoped_name_of(AnyId::Type(*id, *node))
                    .map(|scoped| scoped.to_string())
            })
            .collect();
        let mut dropped = Vec::new();
        for ((id, node, seed_at), name) in doomed.into_iter().zip(names) {
            let seed = &seeds[seed_at];
            self.remove(AnyId::Type(id, node))?;
            if let Some(name) = name {
                dropped.push(Dropped {
                    name,
                    construct: seed.construct.as_str(),
                    spec_reference: seed.spec_reference,
                    detail: seed.detail,
                });
            }
        }
        Ok(dropped)
    }

    /// The types that a declaration outside the type graph names: a
    /// constant, an operation's result, a parameter, an attribute, an
    /// annotation member or a topic.
    fn types_named_outside_the_type_graph(&self) -> BTreeSet<NodeId> {
        let mut named: BTreeSet<NodeId> = BTreeSet::new();
        let mut note = |reference: &zerodds_ast_core::Ref<crate::TypeNode>| {
            if let Some(id) = reference.resolved() {
                if let Ok(node) = self.types.get(id) {
                    named.insert(node.node_id);
                }
            }
        };
        for (_, constant) in self.constants.iter() {
            note(&constant.type_);
        }
        for (_, operation) in self.operations.iter() {
            if let Some(result) = &operation.return_type {
                note(result);
            }
        }
        for (_, parameter) in self.params.iter() {
            note(&parameter.type_);
        }
        for (_, attribute) in self.attributes.iter() {
            note(&attribute.type_);
        }
        for (_, member) in self.annotation_members.iter() {
            note(&member.type_);
        }
        for (_, topic) in self.topics.iter() {
            note(&topic.type_);
        }
        named
    }
}
