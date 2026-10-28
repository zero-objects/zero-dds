// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The edit surface. Spec §5.1.
//!
//! Every mutation runs in a transaction. A call made outside an explicit
//! one opens a single-operation transaction and commits it, so the two
//! paths cannot diverge: the direct call is the transactional call with
//! a `begin` and `commit` around it, not a second implementation.
//!
//! A rejected commit walks the undo log backwards and leaves the model
//! exactly as it was before `begin`.

use crate::error::AstError;
use crate::invariant::{self, InvariantViolation};
use crate::model::Ast;
use crate::node::AnyId;
use crate::node::type_::TypeKind;
use alloc::vec::Vec;
use zerodds_ast_core::{
    Change, Changeset, FieldValue, NodeId, Ref, TxnLog, UndoPayload, UndoRecord,
};

/// A node removed during a transaction, kept so rollback can put it back
/// without having serialised it on the success path.
pub(crate) enum Stashed {
    /// A type together with the module it belonged to.
    Type(crate::TypeNode),
    /// A member together with its position in its aggregate.
    Member(crate::MemberNode, usize),
}

/// An open transaction over a model.
///
/// Dropping one without calling [`AstTransaction::commit`] rolls it
/// back: a transaction that goes out of scope unfinished must not leave
/// half its changes behind.
pub struct AstTransaction<'a> {
    ast: &'a mut Ast,
    finished: bool,
}

impl<'a> AstTransaction<'a> {
    pub(crate) fn new(ast: &'a mut Ast) -> Self {
        ast.sequence = ast.sequence.saturating_add(1);
        let sequence = ast.sequence;
        ast.pending = Some(TxnLog::new(sequence));
        Self {
            ast,
            finished: false,
        }
    }

    /// The model under edit.
    ///
    /// Mutating calls made through this reference join the transaction
    /// rather than committing on their own.
    pub fn ast(&mut self) -> &mut Ast {
        self.ast
    }

    /// The model under edit, read-only.
    #[must_use]
    pub fn model(&self) -> &Ast {
        self.ast
    }

    /// Validate and publish the transaction.
    ///
    /// # Errors
    /// [`AstError::Invariant`] when a structural invariant fails, in
    /// which case the model has been rolled back to its state before
    /// `begin` and no observer was notified. Spec §8.1.
    pub fn commit(mut self) -> Result<Changeset, AstError> {
        self.finished = true;
        let Some(log) = self.ast.pending.take() else {
            return Ok(Changeset::new(self.ast.sequence));
        };

        let touched: Vec<NodeId> = log.nodes_touched().collect();
        let violations = invariant::check(self.ast, &touched);
        if !violations.is_empty() {
            self.ast.apply_rollback(&log);
            return Err(AstError::Invariant(violations));
        }

        let changeset = log.into_changeset();
        self.ast.stash.clear();
        self.ast.observers.notify(&changeset);
        Ok(changeset)
    }

    /// Discard every change the transaction made.
    pub fn rollback(mut self) {
        self.finished = true;
        if let Some(log) = self.ast.pending.take() {
            self.ast.apply_rollback(&log);
        }
    }
}

impl Drop for AstTransaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(log) = self.ast.pending.take() {
                self.ast.apply_rollback(&log);
            }
        }
    }
}

impl Ast {
    /// Open a transaction.
    ///
    /// Mutations made through it are published as one changeset when it
    /// commits, and reversed entirely if it does not. Spec §5.1.
    pub fn begin(&mut self) -> AstTransaction<'_> {
        AstTransaction::new(self)
    }

    /// Record a created node, and auto-commit when no transaction is
    /// open.
    pub(crate) fn record_created(&mut self, any: AnyId) -> Result<(), AstError> {
        let change = Change::Created {
            node: any.node_id(),
            kind: any.kind().tag(),
        };
        let undo = UndoRecord {
            node: any.node_id(),
            payload: UndoPayload::Remove,
        };
        self.record(undo, change)
    }

    fn record(&mut self, undo: UndoRecord, change: Change) -> Result<(), AstError> {
        if let Some(log) = self.pending.as_mut() {
            log.record(undo, change);
            return Ok(());
        }
        self.record_standalone(undo, change)
    }

    /// Validate and publish one mutation as its own transaction.
    pub(crate) fn record_standalone(
        &mut self,
        undo: UndoRecord,
        change: Change,
    ) -> Result<(), AstError> {
        // No transaction open: this is a single-operation one.
        self.sequence = self.sequence.saturating_add(1);
        let mut log = TxnLog::new(self.sequence);
        log.record(undo, change);

        let touched: Vec<NodeId> = log.nodes_touched().collect();
        let violations = invariant::check(self, &touched);
        if !violations.is_empty() {
            self.apply_rollback(&log);
            return Err(AstError::Invariant(violations));
        }

        let changeset = log.into_changeset();
        self.stash.clear();
        self.observers.notify(&changeset);
        Ok(())
    }

    /// Reverse every record of a log, newest first.
    pub(crate) fn apply_rollback(&mut self, log: &TxnLog) {
        for record in log.undo_records().rev() {
            self.apply_undo(record);
        }
        self.stash.clear();
    }

    fn apply_undo(&mut self, record: &UndoRecord) {
        match &record.payload {
            UndoPayload::Remove => {
                if let Some(any) = self.resolve_node_id(record.node) {
                    self.detach_and_remove(any);
                }
            }
            UndoPayload::Recreate { stash } => {
                self.restore_from_stash(*stash as usize);
            }
            UndoPayload::RestoreField { field, value } => {
                self.restore_field(record.node, *field, value);
            }
            UndoPayload::RestorePending { key, slot } => {
                self.restore_pending(record.node, key, *slot);
            }
        }
    }

    /// Put one field back to a recorded value.
    ///
    /// The match is over the fields that have a setter; `PARTS` is a
    /// notification rather than stored state, and its reversal is the
    /// part's own removal record.
    fn restore_field(
        &mut self,
        node: NodeId,
        field: zerodds_ast_core::FieldTag,
        value: &FieldValue,
    ) {
        let Some(any) = self.resolve_node_id(node) else {
            return;
        };
        match field {
            crate::field::COMPLETENESS => {
                let FieldValue::Bool(complete) = value else {
                    return;
                };
                if let Some(id) = any.as_type() {
                    if let Ok(t) = self.types.get_mut(id) {
                        t.completeness = if *complete {
                            crate::Completeness::Complete
                        } else {
                            crate::Completeness::Forward
                        };
                    }
                }
            }
            crate::field::NAME => {
                let FieldValue::Text(name) = value else {
                    return;
                };
                self.restore_name(any, name);
            }
            // PARTS and FACET carry no prior value to restore, and
            // REFERENCE is reversed through RestorePending, which
            // carries the key that FieldValue cannot.
            _ => {}
        }
    }

    fn restore_name(&mut self, any: AnyId, name: &str) {
        match any {
            AnyId::Type(id, _) => {
                if let Ok(t) = self.types.get_mut(id) {
                    t.name = name.into();
                }
            }
            AnyId::Member(id, _) => {
                if let Ok(m) = self.members.get_mut(id) {
                    m.name = name.into();
                }
            }
            AnyId::Module(id, _) => {
                if let Ok(m) = self.modules.get_mut(id) {
                    m.name = name.into();
                }
            }
            _ => {}
        }
    }

    /// Put a resolved reference back to pending on the key it waited on.
    fn restore_pending(&mut self, node: NodeId, key: &zerodds_ast_core::PendingRef, slot: usize) {
        let Some(any) = self.resolve_node_id(node) else {
            return;
        };
        let back = Ref::Pending(key.clone());
        match any {
            AnyId::Type(id, _) => {
                if let Ok(t) = self.types.get_mut(id) {
                    if let Some(reference) = t.kind.type_refs_mut().into_iter().nth(slot) {
                        *reference = back;
                    }
                }
            }
            AnyId::Member(id, _) => {
                if let Ok(m) = self.members.get_mut(id) {
                    m.type_ = back;
                }
            }
            _ => return,
        }
        // The reference waits again, so the index has to know again.
        self.index_pending(any);
    }

    fn restore_from_stash(&mut self, index: usize) {
        let Some(entry) = self.stash.get_mut(index).and_then(Option::take) else {
            return;
        };
        match entry {
            Stashed::Type(node) => {
                let module = node.module;
                let node_id = node.node_id;
                if let Ok(id) = self.types.insert(node) {
                    let any = AnyId::Type(id, node_id);
                    let _ = self.by_node_id.insert(node_id, any);
                    if let Ok(m) = self.modules.get_mut(module) {
                        m.children.push(any);
                    }
                    self.index_node(any);
                }
            }
            Stashed::Member(node, position) => {
                let parent = node.parent;
                let node_id = node.node_id;
                if let Ok(id) = self.members.insert(node) {
                    let any = AnyId::Member(id, node_id);
                    let _ = self.by_node_id.insert(node_id, any);
                    self.index_node(any);
                    if let Some(type_id) = parent.as_type() {
                        if let Ok(t) = self.types.get_mut(type_id) {
                            match &mut t.kind {
                                TypeKind::Structure { members, .. }
                                | TypeKind::Valuetype { state: members, .. } => {
                                    let at = position.min(members.len());
                                    members.insert(at, id);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    /// Remove a node, cascading to the parts it owns and breaking every
    /// reference that pointed at it.
    ///
    /// A dangling resolved handle must not survive a delete; incoming
    /// references become `Broken(NotFound)` so the loss is visible
    /// rather than silent. Spec §5.2.
    ///
    /// # Errors
    /// [`AstError::NotFound`] when the identity is not in the model,
    /// [`AstError::Invariant`] when the resulting model breaks a
    /// structural rule, in which case nothing was removed.
    pub fn remove(&mut self, any: AnyId) -> Result<(), AstError> {
        if self.resolve_node_id(any.node_id()).is_none() {
            return Err(AstError::NotFound {
                node: any.node_id(),
            });
        }

        let own_transaction = self.pending.is_none();
        if own_transaction {
            self.sequence = self.sequence.saturating_add(1);
            let sequence = self.sequence;
            self.pending = Some(TxnLog::new(sequence));
        }

        self.remove_cascade(any);

        if !own_transaction {
            return Ok(());
        }

        let Some(log) = self.pending.take() else {
            return Ok(());
        };
        let touched: Vec<NodeId> = log.nodes_touched().collect();
        let violations = invariant::check(self, &touched);
        if !violations.is_empty() {
            self.apply_rollback(&log);
            return Err(AstError::Invariant(violations));
        }
        let changeset = log.into_changeset();
        self.stash.clear();
        self.observers.notify(&changeset);
        Ok(())
    }

    fn remove_cascade(&mut self, any: AnyId) {
        // Owned parts go first, so a child is never orphaned by its
        // parent's removal.
        for child in self.owned_parts_of(any) {
            self.remove_cascade(child);
        }

        let node_id = any.node_id();
        let stash_index = self.stash_node(any);
        self.detach_and_remove(any);
        self.break_references_to(any);

        let undo = UndoRecord {
            node: node_id,
            payload: stash_index
                .map_or(UndoPayload::Remove, |stash| UndoPayload::Recreate { stash }),
        };
        let change = Change::Deleted {
            node: node_id,
            kind: any.kind().tag(),
        };
        if let Some(log) = self.pending.as_mut() {
            log.record(undo, change);
        }
    }

    /// The nodes a node owns outright, which go with it.
    ///
    /// An interface owns its operations and attributes, an operation its
    /// parameters, an annotation declaration its members. Only types had
    /// this, so removing an interface left its operations in the arena
    /// with no owner and its handle in the module's child list.
    fn owned_parts_of(&self, any: AnyId) -> Vec<AnyId> {
        match any {
            AnyId::Type(id, _) => self.owned_parts_of_type(id),
            AnyId::Interface(id, _) => {
                let Ok(node) = self.interfaces.get(id) else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for operation in &node.operations {
                    if let Ok(o) = self.operations.get(*operation) {
                        out.push(AnyId::Operation(*operation, o.node_id));
                    }
                }
                for attribute in &node.attributes {
                    if let Ok(a) = self.attributes.get(*attribute) {
                        out.push(AnyId::Attribute(*attribute, a.node_id));
                    }
                }
                out
            }
            AnyId::Operation(id, _) => {
                let Ok(node) = self.operations.get(id) else {
                    return Vec::new();
                };
                node.params
                    .iter()
                    .filter_map(|param| {
                        self.params
                            .get(*param)
                            .ok()
                            .map(|p| AnyId::Param(*param, p.node_id))
                    })
                    .collect()
            }
            AnyId::AnnotationDef(id, _) => {
                let Ok(node) = self.annotation_defs.get(id) else {
                    return Vec::new();
                };
                node.members
                    .iter()
                    .filter_map(|member| {
                        self.annotation_members
                            .get(*member)
                            .ok()
                            .map(|m| AnyId::AnnotationMember(*member, m.node_id))
                    })
                    .collect()
            }
            AnyId::Exception(id, _) => {
                let Ok(node) = self.exceptions.get(id) else {
                    return Vec::new();
                };
                node.members
                    .iter()
                    .filter_map(|member| {
                        self.members
                            .get(*member)
                            .ok()
                            .map(|m| AnyId::Member(*member, m.node_id))
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn owned_parts_of_type(&self, id: zerodds_ast_core::Id<crate::TypeNode>) -> Vec<AnyId> {
        let Ok(node) = self.types.get(id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        match &node.kind {
            TypeKind::Structure { members, .. } | TypeKind::Valuetype { state: members, .. } => {
                for member in members {
                    if let Ok(m) = self.members.get(*member) {
                        out.push(AnyId::Member(*member, m.node_id));
                    }
                }
            }
            TypeKind::Union { cases, .. } => {
                for case in cases {
                    if let Ok(c) = self.cases.get(*case) {
                        if let Ok(m) = self.members.get(c.member) {
                            out.push(AnyId::Member(c.member, m.node_id));
                        }
                        out.push(AnyId::Case(*case, c.node_id));
                    }
                }
            }
            TypeKind::Enumeration { enumerators } => {
                for enumerator in enumerators {
                    if let Ok(e) = self.enumerators.get(*enumerator) {
                        out.push(AnyId::Enumerator(*enumerator, e.node_id));
                    }
                }
            }
            TypeKind::Bitmask { bits } => {
                for bit in bits {
                    if let Ok(b) = self.bit_values.get(*bit) {
                        out.push(AnyId::BitValue(*bit, b.node_id));
                    }
                }
            }
            TypeKind::Bitset { fields, .. } => {
                for field in fields {
                    if let Ok(b) = self.bitfields.get(*field) {
                        out.push(AnyId::Bitfield(*field, b.node_id));
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// Move a node into the rollback stash, returning its index.
    ///
    /// Only the kinds a rollback has to restore are stashed; the rest
    /// record a plain `Remove`, which is correct for nodes the same
    /// transaction created.
    fn stash_node(&mut self, any: AnyId) -> Option<u32> {
        let entry = match any {
            AnyId::Type(id, _) => Stashed::Type(self.types.get(id).ok()?.clone()),
            AnyId::Member(id, _) => {
                let member = self.members.get(id).ok()?.clone();
                let position = member
                    .parent
                    .as_type()
                    .and_then(|t| self.types.get(t).ok())
                    .and_then(|t| t.kind.members().iter().position(|m| *m == id))
                    .unwrap_or(0);
                Stashed::Member(member, position)
            }
            _ => return None,
        };
        let index = u32::try_from(self.stash.len()).ok()?;
        self.stash.push(Some(entry));
        Some(index)
    }

    fn detach_and_remove(&mut self, any: AnyId) {
        let node_id = any.node_id();
        let _ = self.by_node_id.remove(&node_id);
        // Indices are maintained from the change, never rebuilt: a
        // deleted node leaves every index in the same commit.
        self.unindex_node(node_id);

        match any {
            AnyId::Type(id, _) => {
                if let Ok(node) = self.types.get(id) {
                    let module = node.module;
                    if let Ok(m) = self.modules.get_mut(module) {
                        m.children.retain(|c| c.node_id() != node_id);
                    }
                }
                let _ = self.types.remove(id);
            }
            AnyId::Member(id, _) => {
                if let Ok(node) = self.members.get(id) {
                    let parent = node.parent;
                    if let Some(type_id) = parent.as_type() {
                        if let Ok(t) = self.types.get_mut(type_id) {
                            match &mut t.kind {
                                TypeKind::Structure { members, .. }
                                | TypeKind::Valuetype { state: members, .. } => {
                                    members.retain(|m| *m != id);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                let _ = self.members.remove(id);
            }
            AnyId::Case(id, _) => {
                if let Ok(node) = self.cases.get(id) {
                    let parent = node.parent;
                    if let Ok(t) = self.types.get_mut(parent) {
                        if let TypeKind::Union {
                            cases,
                            default_case,
                            ..
                        } = &mut t.kind
                        {
                            cases.retain(|c| *c != id);
                            if *default_case == Some(id) {
                                *default_case = None;
                            }
                        }
                    }
                }
                let _ = self.cases.remove(id);
            }
            AnyId::Enumerator(id, _) => {
                if let Ok(node) = self.enumerators.get(id) {
                    let parent = node.parent;
                    if let Ok(t) = self.types.get_mut(parent) {
                        if let TypeKind::Enumeration { enumerators } = &mut t.kind {
                            enumerators.retain(|e| *e != id);
                        }
                    }
                }
                let _ = self.enumerators.remove(id);
            }
            AnyId::BitValue(id, _) => {
                if let Ok(node) = self.bit_values.get(id) {
                    let parent = node.parent;
                    if let Ok(t) = self.types.get_mut(parent) {
                        if let TypeKind::Bitmask { bits } = &mut t.kind {
                            bits.retain(|b| *b != id);
                        }
                    }
                }
                let _ = self.bit_values.remove(id);
            }
            AnyId::Bitfield(id, _) => {
                if let Ok(node) = self.bitfields.get(id) {
                    let parent = node.parent;
                    if let Ok(t) = self.types.get_mut(parent) {
                        if let TypeKind::Bitset { fields, .. } = &mut t.kind {
                            fields.retain(|f| *f != id);
                        }
                    }
                }
                let _ = self.bitfields.remove(id);
            }
            AnyId::Module(id, _) => {
                if let Ok(node) = self.modules.get(id) {
                    if let Some(parent) = node.parent {
                        if let Ok(m) = self.modules.get_mut(parent) {
                            m.children.retain(|c| c.node_id() != node_id);
                        }
                    }
                }
                let _ = self.modules.remove(id);
            }
            // Every kind below leaves a handle in whatever list holds
            // it unless it is taken out here. The arena is generational,
            // so a stale handle answers `Err` rather than the wrong
            // node — which is why the omission was silent: the list
            // still carried an entry that resolved to nothing, and
            // every consumer walking `interface.attributes` or
            // `module.children` saw one node too many.
            AnyId::Const(id, _) => {
                if let Ok(node) = self.constants.get(id) {
                    let module = node.module;
                    if let Ok(m) = self.modules.get_mut(module) {
                        m.children.retain(|c| c.node_id() != node_id);
                    }
                }
                let _ = self.constants.remove(id);
            }
            AnyId::ConstExpr(id, _) => {
                let _ = self.exprs.remove(id);
            }
            AnyId::Interface(id, _) => {
                if let Ok(node) = self.interfaces.get(id) {
                    let module = node.module;
                    if let Ok(m) = self.modules.get_mut(module) {
                        m.children.retain(|c| c.node_id() != node_id);
                    }
                }
                let _ = self.interfaces.remove(id);
            }
            AnyId::Operation(id, _) => {
                if let Ok(node) = self.operations.get(id) {
                    if let Some(interface) = node.parent.as_interface() {
                        if let Ok(i) = self.interfaces.get_mut(interface) {
                            i.operations.retain(|o| *o != id);
                        }
                    }
                    if let Some(type_id) = node.parent.as_type() {
                        if let Ok(t) = self.types.get_mut(type_id) {
                            if let TypeKind::Valuetype { initializers, .. } = &mut t.kind {
                                initializers.retain(|o| *o != id);
                            }
                        }
                    }
                }
                let _ = self.operations.remove(id);
            }
            AnyId::Param(id, _) => {
                if let Ok(node) = self.params.get(id) {
                    let parent = node.parent;
                    if let Ok(o) = self.operations.get_mut(parent) {
                        o.params.retain(|p| *p != id);
                    }
                }
                let _ = self.params.remove(id);
            }
            AnyId::Attribute(id, _) => {
                if let Ok(node) = self.attributes.get(id) {
                    let parent = node.parent;
                    if let Ok(i) = self.interfaces.get_mut(parent) {
                        i.attributes.retain(|a| *a != id);
                    }
                }
                let _ = self.attributes.remove(id);
            }
            AnyId::Exception(id, _) => {
                if let Ok(node) = self.exceptions.get(id) {
                    let module = node.module;
                    if let Ok(m) = self.modules.get_mut(module) {
                        m.children.retain(|c| c.node_id() != node_id);
                    }
                }
                let _ = self.exceptions.remove(id);
            }
            AnyId::AnnotationDef(id, _) => {
                if let Ok(node) = self.annotation_defs.get(id) {
                    let module = node.module;
                    if let Ok(m) = self.modules.get_mut(module) {
                        m.children.retain(|c| c.node_id() != node_id);
                    }
                }
                let _ = self.annotation_defs.remove(id);
            }
            AnyId::AnnotationMember(id, _) => {
                if let Ok(node) = self.annotation_members.get(id) {
                    let parent = node.parent;
                    if let Ok(d) = self.annotation_defs.get_mut(parent) {
                        d.members.retain(|m| *m != id);
                    }
                }
                let _ = self.annotation_members.remove(id);
            }
            AnyId::Domain(id, _) => {
                let _ = self.domains.remove(id);
            }
            AnyId::Topic(id, _) => {
                let _ = self.topics.remove(id);
            }
            AnyId::QosProfile(id, _) => {
                let _ = self.qos_profiles.remove(id);
            }
            AnyId::Participant(id, _) => {
                let _ = self.participants.remove(id);
            }
            AnyId::Endpoint(id, _) => {
                let _ = self.endpoints.remove(id);
            }
            AnyId::Library(id, _) => {
                let _ = self.libraries.remove(id);
            }
        }
    }

    /// Rewrite every resolved reference to a removed node into
    /// `Broken(NotFound)`.
    fn break_references_to(&mut self, removed: AnyId) {
        let Some(type_id) = removed.as_type() else {
            return;
        };
        let cause = zerodds_ast_core::BreakCause::NotFound {
            what: zerodds_ast_core::PendingRef::ScopedName(zerodds_ast_core::ScopedName::absolute(
                alloc::vec::Vec::new(),
            )),
        };

        for (_, node) in self.types.iter_mut_all() {
            break_in_kind(&mut node.kind, type_id, &cause);
        }
        for (_, member) in self.members.iter_mut_all() {
            if member.type_.resolved() == Some(type_id) {
                member.type_ = Ref::Broken(cause.clone());
            }
        }
    }
}

fn break_in_kind(
    kind: &mut TypeKind,
    removed: zerodds_ast_core::Id<crate::TypeNode>,
    cause: &zerodds_ast_core::BreakCause,
) {
    let fix = |reference: &mut Ref<crate::TypeNode>| {
        if reference.resolved() == Some(removed) {
            *reference = Ref::Broken(cause.clone());
        }
    };
    match kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => fix(element),
        TypeKind::Map { key, value, .. } => {
            fix(key);
            fix(value);
        }
        TypeKind::Alias { aliased } => fix(aliased),
        TypeKind::ValueBox { boxed } => fix(boxed),
        TypeKind::Union { discriminator, .. } => fix(discriminator),
        TypeKind::Structure { base, .. } | TypeKind::Bitset { base, .. } => {
            if let Some(base) = base.as_mut() {
                fix(base);
            }
        }
        TypeKind::Valuetype { bases, .. } => {
            for base in bases.iter_mut() {
                fix(base);
            }
        }
        _ => {}
    }
}

/// Re-exported so consumers can match on what a commit rejected.
pub use crate::invariant::InvariantViolation as CommitViolation;

const _: () = {
    // Keeps the alias referenced even when no consumer uses it yet.
    let _ = core::mem::size_of::<InvariantViolation>();
};
