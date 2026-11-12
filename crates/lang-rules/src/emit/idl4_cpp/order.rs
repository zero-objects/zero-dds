// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The order the header declares in.
//!
//! C++ has to see a type complete before a member holds it by value and a
//! typedef before a use of its name. OMG IDL 4.2 has the same rule for its
//! own declarations, so the declaration order of the IDL is the start;
//! three things move away from it: a structure that was declared forward
//! and completed after a typedef of a sequence of it (§7.2.4.3.4), a
//! module that was reopened (one module of the model, its declarations
//! from two places), and the equivalent IDL of components and homes,
//! which the model appends. Each declaration is therefore written after
//! the ones it needs, with the IDL order deciding where nothing does.
//! What a reference or a sequence needs is a class declared, which every
//! class is ahead of all definitions.

use super::{Gen, Idl4CppError, Item, resolved_all, unresolved};
use std::collections::BTreeSet;
use zerodds_ast::{TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId, Ref};

/// What a use of a type needs of it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Need {
    /// Its definition: a member holds it by value.
    Complete,
    /// Its name: a reference, a sequence element, a parameter.
    Declared,
}

impl Gen<'_> {
    /// `items` in an order C++ accepts.
    pub(super) fn ordered(&self, items: &[Item]) -> Result<Vec<Item>, Idl4CppError> {
        let mut by_position: Vec<(NodeId, Item)> = Vec::new();
        for item in items {
            by_position.push((self.node_id(*item)?, *item));
        }
        by_position.sort();
        let known: BTreeSet<Item> = items.iter().copied().collect();
        let mut state = Walk {
            done: BTreeSet::new(),
            active: BTreeSet::new(),
            out: Vec::new(),
        };
        for (_, item) in &by_position {
            self.visit(*item, &known, &mut state, 0)?;
        }
        Ok(state.out)
    }

    fn node_id(&self, item: Item) -> Result<NodeId, Idl4CppError> {
        Ok(match item {
            Item::Type(id) => {
                self.ast
                    .type_(id)
                    .map_err(|_| unresolved("a type"))?
                    .node_id
            }
            Item::Const(id) => {
                self.ast
                    .constant(id)
                    .map_err(|_| unresolved("a constant"))?
                    .node_id
            }
            Item::Exception(id) => {
                self.ast
                    .exception(id)
                    .map_err(|_| unresolved("an exception"))?
                    .node_id
            }
            Item::Interface(id) => {
                self.ast
                    .interface(id)
                    .map_err(|_| unresolved("an interface"))?
                    .node_id
            }
        })
    }

    fn visit(
        &self,
        item: Item,
        known: &BTreeSet<Item>,
        state: &mut Walk,
        depth: usize,
    ) -> Result<(), Idl4CppError> {
        if state.done.contains(&item) || state.active.contains(&item) {
            return Ok(());
        }
        if depth > super::MAX_DEPTH * 16 {
            return Err(Idl4CppError::Unevaluated {
                what: "the order of declarations: a chain of dependencies deeper than the \
                       mapping follows"
                    .to_string(),
                at: String::new(),
            });
        }
        state.active.insert(item);
        let mut needs = Vec::new();
        self.needs_of_item(item, &mut needs)?;
        let mut positioned = Vec::new();
        for need in needs {
            if need != item && known.contains(&need) {
                positioned.push((self.node_id(need)?, need));
            }
        }
        positioned.sort();
        positioned.dedup();
        for (_, need) in positioned {
            self.visit(need, known, state, depth + 1)?;
        }
        state.active.remove(&item);
        state.done.insert(item);
        state.out.push(item);
        Ok(())
    }

    /// The items an item needs written ahead of it.
    fn needs_of_item(&self, item: Item, out: &mut Vec<Item>) -> Result<(), Idl4CppError> {
        match item {
            Item::Type(id) => self.needs_of_type_declaration(id, out),
            Item::Const(id) => {
                let node = self
                    .ast
                    .constant(id)
                    .map_err(|_| unresolved("a constant"))?;
                self.needs_of(&node.type_, Need::Declared, out, 0)
            }
            Item::Exception(id) => {
                let node = self
                    .ast
                    .exception(id)
                    .map_err(|_| unresolved("an exception"))?;
                for member in &node.members {
                    let m = self
                        .ast
                        .member(*member)
                        .map_err(|_| unresolved("a member"))?;
                    self.needs_of(&m.type_, Need::Complete, out, 0)?;
                }
                Ok(())
            }
            Item::Interface(id) => {
                let node = self
                    .ast
                    .interface(id)
                    .map_err(|_| unresolved("an interface"))?;
                for base in resolved_all(&node.bases, "a base interface")? {
                    out.push(Item::Interface(base));
                }
                self.needs_of_signatures(id, out)?;
                self.needs_of_embedded(node.module, &node.name, out)
            }
        }
    }

    fn needs_of_type_declaration(
        &self,
        id: Id<TypeNode>,
        out: &mut Vec<Item>,
    ) -> Result<(), Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        match &node.kind {
            TypeKind::Alias { aliased } => self.needs_of(aliased, Need::Declared, out, 0)?,
            TypeKind::Structure { base, members } => {
                if let Some(base) = base {
                    self.needs_of(base, Need::Complete, out, 0)?;
                }
                for member in members {
                    let m = self
                        .ast
                        .member(*member)
                        .map_err(|_| unresolved("a member"))?;
                    let external = m.facets.xtypes.as_ref().is_some_and(|x| x.is_external);
                    let need = if external {
                        Need::Declared
                    } else {
                        Need::Complete
                    };
                    self.needs_of(&m.type_, need, out, 0)?;
                }
            }
            TypeKind::Union {
                discriminator,
                cases,
                default_case,
            } => {
                self.needs_of(discriminator, Need::Complete, out, 0)?;
                for case in cases.iter().chain(default_case.iter()) {
                    let c = self.ast.case(*case).map_err(|_| unresolved("a case"))?;
                    let m = self
                        .ast
                        .member(c.member)
                        .map_err(|_| unresolved("a member"))?;
                    let external = m.facets.xtypes.as_ref().is_some_and(|x| x.is_external);
                    let need = if external {
                        Need::Declared
                    } else {
                        Need::Complete
                    };
                    self.needs_of(&m.type_, need, out, 0)?;
                }
            }
            TypeKind::Bitset {
                base: Some(base), ..
            } => {
                self.needs_of(base, Need::Complete, out, 0)?;
            }
            TypeKind::Valuetype {
                bases,
                state,
                supported,
                ..
            } => {
                for base in bases {
                    self.needs_of(base, Need::Complete, out, 0)?;
                }
                for member in state {
                    let m = self
                        .ast
                        .member(*member)
                        .map_err(|_| unresolved("a member"))?;
                    self.needs_of(&m.type_, Need::Declared, out, 0)?;
                }
                for interface in resolved_all(supported, "a supported interface")? {
                    out.push(Item::Interface(interface));
                }
                for attribute in self.ast.attributes_of_value(id) {
                    let a = self
                        .ast
                        .attribute(attribute)
                        .map_err(|_| unresolved("an attribute"))?;
                    self.needs_of(&a.type_, Need::Declared, out, 0)?;
                }
                for operation in self.ast.operations_of_value(id) {
                    self.needs_of_operation(operation, out)?;
                }
                self.needs_of_embedded(node.module, &node.name, out)?;
            }
            TypeKind::ValueBox { boxed } => self.needs_of(boxed, Need::Complete, out, 0)?,
            _ => {}
        }
        Ok(())
    }

    fn needs_of_signatures(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
        out: &mut Vec<Item>,
    ) -> Result<(), Idl4CppError> {
        for attribute in self.ast.attributes_of(id).iter().copied() {
            let a = self
                .ast
                .attribute(attribute)
                .map_err(|_| unresolved("an attribute"))?;
            self.needs_of(&a.type_, Need::Declared, out, 0)?;
        }
        for operation in self.ast.operations_of(id).iter().copied() {
            self.needs_of_operation(operation, out)?;
        }
        Ok(())
    }

    fn needs_of_operation(
        &self,
        id: Id<zerodds_ast::OperationNode>,
        out: &mut Vec<Item>,
    ) -> Result<(), Idl4CppError> {
        let node = self
            .ast
            .operation(id)
            .map_err(|_| unresolved("an operation"))?;
        if let Some(returns) = &node.return_type {
            self.needs_of(returns, Need::Declared, out, 0)?;
        }
        for param in &node.params {
            let p = self
                .ast
                .param(*param)
                .map_err(|_| unresolved("a parameter"))?;
            self.needs_of(&p.type_, Need::Declared, out, 0)?;
        }
        Ok(())
    }

    fn needs_of_embedded(
        &self,
        module: Id<zerodds_ast::ModuleNode>,
        scope: &str,
        out: &mut Vec<Item>,
    ) -> Result<(), Idl4CppError> {
        let embedded = self.embedded(module, scope);
        for id in embedded.types {
            self.needs_of_type_declaration(id, out)?;
        }
        for id in embedded.constants {
            let node = self
                .ast
                .constant(id)
                .map_err(|_| unresolved("a constant"))?;
            self.needs_of(&node.type_, Need::Declared, out, 0)?;
        }
        for id in embedded.exceptions {
            let node = self
                .ast
                .exception(id)
                .map_err(|_| unresolved("an exception"))?;
            for member in &node.members {
                let m = self
                    .ast
                    .member(*member)
                    .map_err(|_| unresolved("a member"))?;
                self.needs_of(&m.type_, Need::Complete, out, 0)?;
            }
        }
        Ok(())
    }

    /// What a use of the type `reference` names needs, as `need` says.
    fn needs_of(
        &self,
        reference: &Ref<TypeNode>,
        need: Need,
        out: &mut Vec<Item>,
        depth: usize,
    ) -> Result<(), Idl4CppError> {
        if depth > super::MAX_DEPTH {
            return Err(Idl4CppError::Unevaluated {
                what: "a type nested deeper than the mapping follows".to_string(),
                at: String::new(),
            });
        }
        let id = reference.resolved().ok_or_else(|| unresolved("a type"))?;
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        let owner = || -> Item {
            match node.scope.as_deref() {
                Some(scope) => match self.scope_owner(node.module, scope) {
                    Some(zerodds_ast::AnyId::Interface(owner, _)) => Item::Interface(owner),
                    Some(zerodds_ast::AnyId::Type(owner, _)) => Item::Type(owner),
                    _ => Item::Type(id),
                },
                None => Item::Type(id),
            }
        };
        match &node.kind {
            TypeKind::Alias { aliased } => {
                out.push(owner());
                if need == Need::Complete {
                    self.needs_of(aliased, Need::Complete, out, depth + 1)?;
                }
            }
            TypeKind::Enumeration { .. } => out.push(owner()),
            TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Bitmask { .. } => {
                if need == Need::Complete || node.scope.is_some() {
                    out.push(owner());
                }
            }
            TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => {
                if node.scope.is_some() {
                    out.push(owner());
                }
            }
            TypeKind::Sequence { element, .. } => {
                self.needs_of(element, Need::Declared, out, depth + 1)?;
            }
            TypeKind::Map { key, value, .. } => {
                self.needs_of(key, Need::Declared, out, depth + 1)?;
                self.needs_of(value, Need::Declared, out, depth + 1)?;
            }
            TypeKind::Array { element, .. } => self.needs_of(element, need, out, depth + 1)?,
            _ => {}
        }
        Ok(())
    }
}

struct Walk {
    done: BTreeSet<Item>,
    active: BTreeSet<Item>,
    out: Vec<Item>,
}
