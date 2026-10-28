// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reference resolution. Spec §2.
//!
//! Resolution is a state transition on an existing node, not a rebuild.
//! A failed attempt lands in `Broken` with a cause and is not retried by
//! a later pass: a diagnostic already reported must not silently vanish.

use crate::error::AstError;
use crate::model::Ast;
use crate::node::AnyId;
use crate::refs::RefKind;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::ScopedName;
use zerodds_ast_core::{BreakCause, Id, IndexLookup, NodeId, PendingRef, RefState};

/// What one resolution pass achieved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveReport {
    /// How many references moved from pending to resolved.
    pub resolved: usize,
    /// Every reference that moved from pending to broken, with why.
    pub broken: Vec<(NodeId, BreakCause)>,
}

impl ResolveReport {
    /// Whether the pass left nothing unresolved and broke nothing.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.broken.is_empty()
    }
}

impl Ast {
    /// Every node waiting on a given lookup key.
    #[must_use]
    pub fn pending_waiters(&self, key: &PendingRef) -> &[NodeId] {
        self.by_pending.get(key)
    }

    /// How many references are still pending.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.by_pending.iter().map(|(_, nodes)| nodes.len()).sum()
    }

    /// Resolve every waiter on `key` to `target`.
    ///
    /// Returns how many references changed. Each one emits an `Updated`
    /// change carrying both the old and the new reference state.
    ///
    /// # Errors
    /// [`AstError::WrongKind`] when the target cannot satisfy the
    /// waiters — a member's type reference pointing at an interface, for
    /// instance.
    pub fn resolve_pending(
        &mut self,
        key: &PendingRef,
        target: Id<crate::TypeNode>,
    ) -> Result<usize, AstError> {
        let waiters: Vec<NodeId> = self.by_pending.get(key).to_vec();
        if waiters.is_empty() {
            return Ok(0);
        }
        let Ok(node) = self.types.get(target) else {
            return Ok(0);
        };
        let target_any = AnyId::Type(target, node.node_id);
        let mut changed = 0usize;
        for waiter in waiters {
            if self.set_reference_to(waiter, key, target_any) {
                changed += 1;
            }
        }
        self.by_pending.remove_key(key);
        Ok(changed)
    }

    /// Resolve every waiter on `key` to any kind of node.
    ///
    /// The general form of [`Ast::resolve_pending`]: a constant named in
    /// an array dimension, an exception in a `raises` clause and a base
    /// interface all resolve through here.
    ///
    /// # Errors
    /// [`AstError::Core`] on an arena failure while rewriting.
    pub fn resolve_pending_to(
        &mut self,
        key: &PendingRef,
        target: AnyId,
    ) -> Result<usize, AstError> {
        let waiters: Vec<NodeId> = self.by_pending.get(key).to_vec();
        if waiters.is_empty() {
            return Ok(0);
        }
        let mut changed = 0usize;
        for waiter in waiters {
            if self.set_reference_to(waiter, key, target) {
                changed += 1;
            }
        }
        self.by_pending.remove_key(key);
        Ok(changed)
    }

    /// Mark every waiter on `key` broken, with a cause.
    ///
    /// # Errors
    /// Never fails today; the signature matches [`Ast::resolve_pending`]
    /// so callers can treat both the same way.
    pub fn break_pending(
        &mut self,
        key: &PendingRef,
        cause: BreakCause,
    ) -> Result<usize, AstError> {
        let waiters: Vec<NodeId> = self.by_pending.get(key).to_vec();
        let mut changed = 0usize;
        for waiter in waiters {
            if self.break_reference(waiter, key, &cause) {
                changed += 1;
            }
        }
        self.by_pending.remove_key(key);
        Ok(changed)
    }

    /// Resolve every pending scoped-name reference against the name
    /// index, in one pass.
    ///
    /// A name that resolves to the wrong kind of node, that is ambiguous
    /// or that is absent lands in `Broken` with the corresponding cause
    /// rather than staying pending.
    ///
    /// The declaration a pending name reaches from the scopes its
    /// waiters sit in, innermost first.
    ///
    /// The rule is §7.5's: a name is looked for in the scope it is
    /// written in, then in each enclosing one. This runs only after the
    /// root lookup has already failed, so it adds names that were
    /// previously unresolvable and moves none that were not.
    ///
    /// Ambiguity refuses: a name that reaches two different declarations
    /// from two different waiters is not one this can decide, and
    /// picking either would bind half the file to a type nobody named.
    fn find_in_enclosing_scopes(&self, key: &PendingRef) -> Option<AnyId> {
        let PendingRef::ScopedName(name) = key else {
            return None;
        };
        let written = name.segments();
        if written.is_empty() {
            return None;
        }
        let mut found: Option<AnyId> = None;
        for waiter in self.by_pending.get(key) {
            let Some(scope) = self.enclosing_module_path(*waiter) else {
                continue;
            };
            // Innermost first: drop one enclosing segment at a time.
            for depth in (0..=scope.len()).rev() {
                let mut segments: Vec<String> = scope.iter().take(depth).cloned().collect();
                segments.extend(written.iter().cloned());
                let candidate = ScopedName::absolute(segments);
                let Ok(node) = self.by_scoped_name.unique(&candidate) else {
                    continue;
                };
                let Some(any) = self.resolve_node_id(node) else {
                    continue;
                };
                match found {
                    None => found = Some(any),
                    // Two waiters reaching two different declarations
                    // through one written name.
                    Some(first) if first != any => return None,
                    Some(_) => {}
                }
                break;
            }
        }
        found
    }

    /// The module path a node sits in, outermost first.
    fn enclosing_module_path(&self, node: NodeId) -> Option<Vec<String>> {
        let any = self.resolve_node_id(node)?;
        let module = match any {
            AnyId::Type(id, _) => self.types.get(id).ok()?.module,
            AnyId::Member(id, _) => match self.members.get(id).ok()?.parent {
                AnyId::Type(type_, _) => self.types.get(type_).ok()?.module,
                _ => return None,
            },
            AnyId::Case(id, _) => {
                let parent = self.cases.get(id).ok()?.parent;
                self.types.get(parent).ok()?.module
            }
            _ => return None,
        };
        self.scoped_name_of(AnyId::Module(
            module,
            self.modules.get(module).ok()?.node_id,
        ))
        .map(|name| name.segments().to_vec())
        .or_else(|| Some(Vec::new()))
    }

    /// # Errors
    /// [`AstError::Core`] on an arena failure while rewriting.
    pub fn resolve_all_by_name(&mut self) -> Result<ResolveReport, AstError> {
        let keys: Vec<PendingRef> = self.by_pending.keys().cloned().collect();
        let mut report = ResolveReport::default();

        for key in keys {
            let PendingRef::ScopedName(name) = &key else {
                continue;
            };
            match self.by_scoped_name.unique(name) {
                Ok(node) => match self.resolve_node_id(node) {
                    // The waiters decide what the name has to be: a
                    // member's type reference wants a type, a raises
                    // clause an exception, an array dimension a
                    // constant. One name, several kinds of waiter.
                    // An interface named where a type is expected is an
                    // object reference — IDL 4.2 §7.4.6 — so those
                    // waiters get the derived object-reference type and
                    // the rest get the interface. Without this every
                    // `struct S { I x; }`, every interface-typed
                    // parameter and every `provides I p` broke with
                    // `KindMismatch`, because an interface is not a type
                    // and nothing offered the type that stands for one.
                    Some(target @ AnyId::Interface(interface, _))
                        if self.any_waiter_wants(&key, RefKind::Type) =>
                    {
                        let object_ref = self.object_reference_to(interface)?;
                        let node_id = self.types.get(object_ref)?.node_id;
                        report.resolved +=
                            self.point_waiters(&key, AnyId::Type(object_ref, node_id));
                        report.resolved += self.point_waiters(&key, target);
                        self.break_or_clear(&key, &mut report)?;
                    }
                    Some(target) if self.any_waiter_accepts(&key, target) => {
                        report.resolved += self.resolve_pending_to(&key, target)?;
                        // Waiters the target could not satisfy are still
                        // waiting; they break rather than stay pending
                        // on a name that has already been decided.
                        let remaining = self.by_pending.get(&key).to_vec();
                        if !remaining.is_empty() {
                            let cause = BreakCause::KindMismatch {
                                expected: "a different kind of declaration",
                                found: target.kind().as_str(),
                            };
                            for waiter in &remaining {
                                report.broken.push((*waiter, cause.clone()));
                            }
                            let _ = self.break_pending(&key, cause)?;
                        }
                    }
                    found => {
                        let found = found.map_or("nothing", |any| any.kind().as_str());
                        let cause = BreakCause::KindMismatch {
                            expected: "a declaration the reference accepts",
                            found,
                        };
                        for waiter in self.by_pending.get(&key).to_vec() {
                            report.broken.push((waiter, cause.clone()));
                        }
                        let _ = self.break_pending(&key, cause)?;
                    }
                },
                Err(IndexLookup::Missing) => {
                    // §7.5: a name that is not found at the root is
                    // looked for in the enclosing scopes, innermost
                    // first. `Lowerer::scoped_name_of` marks every name
                    // absolute whether or not the source wrote `::`, so
                    // without this a member typed `inner::Point` from
                    // inside `module outer` — ordinary IDL — resolved to
                    // nothing, the field never reached the generated
                    // type, and the sample went onto the wire short.
                    if let Some(target) = self.find_in_enclosing_scopes(&key) {
                        report.resolved += self.point_waiters(&key, target);
                        continue;
                    }
                    let cause = BreakCause::NotFound { what: key.clone() };
                    for waiter in self.by_pending.get(&key).to_vec() {
                        report.broken.push((waiter, cause.clone()));
                    }
                    let _ = self.break_pending(&key, cause)?;
                }
                Err(IndexLookup::Ambiguous(candidates)) => {
                    let cause = BreakCause::Ambiguous {
                        what: key.clone(),
                        candidates,
                    };
                    for waiter in self.by_pending.get(&key).to_vec() {
                        report.broken.push((waiter, cause.clone()));
                    }
                    let _ = self.break_pending(&key, cause)?;
                }
            }
        }

        Ok(report)
    }

    /// Point every waiter on `key` that accepts `target` at it, without
    /// clearing the key.
    ///
    /// [`Ast::resolve_pending_to`] removes the key once it is done, which
    /// is right when one target satisfies every waiter and wrong when a
    /// name is served by two — an interface and the object-reference type
    /// derived from it.
    fn point_waiters(&mut self, key: &PendingRef, target: AnyId) -> usize {
        let waiters: Vec<NodeId> = self.by_pending.get(key).to_vec();
        waiters
            .into_iter()
            .filter(|waiter| self.set_reference_to(*waiter, key, target))
            .count()
    }

    /// Break whatever still waits on `key` and drop it from the index.
    fn break_or_clear(
        &mut self,
        key: &PendingRef,
        report: &mut ResolveReport,
    ) -> Result<(), AstError> {
        let remaining: Vec<NodeId> = self
            .by_pending
            .get(key)
            .iter()
            .copied()
            .filter(|waiter| {
                self.resolve_node_id(*waiter)
                    .is_some_and(|any| self.pending_slots(any).iter().any(|slot| &slot.key == key))
            })
            .collect();
        if remaining.is_empty() {
            self.by_pending.remove_key(key);
            return Ok(());
        }
        let cause = BreakCause::KindMismatch {
            expected: "a different kind of declaration",
            found: "interface",
        };
        for waiter in &remaining {
            report.broken.push((*waiter, cause.clone()));
        }
        let _ = self.break_pending(key, cause)?;
        Ok(())
    }

    /// Whether any node waiting on `key` holds a reference of `kind`.
    fn any_waiter_wants(&self, key: &PendingRef, kind: RefKind) -> bool {
        self.by_pending.get(key).iter().any(|waiter| {
            self.resolve_node_id(*waiter).is_some_and(|any| {
                self.pending_slots(any)
                    .iter()
                    .any(|slot| &slot.key == key && slot.kind == kind)
            })
        })
    }

    /// Whether any node waiting on `key` accepts `target`.
    fn any_waiter_accepts(&self, key: &PendingRef, target: AnyId) -> bool {
        self.by_pending.get(key).iter().any(|waiter| {
            self.resolve_node_id(*waiter).is_some_and(|any| {
                self.pending_slots(any)
                    .iter()
                    .any(|slot| &slot.key == key && slot.kind.accepts(target))
            })
        })
    }

    /// Point every reference of `node` that waits on `key` at `target`.
    ///
    /// Returns whether anything changed. A reference whose kind the
    /// target cannot satisfy is left alone rather than pointed at the
    /// wrong sort of node; `resolve_all_by_name` breaks it with a
    /// `KindMismatch` instead.
    fn set_reference_to(&mut self, node: NodeId, key: &PendingRef, target: AnyId) -> bool {
        let Some(any) = self.resolve_node_id(node) else {
            return false;
        };
        let slots: Vec<usize> = self
            .pending_slots(any)
            .into_iter()
            .filter(|slot| &slot.key == key && slot.kind.accepts(target))
            .map(|slot| slot.slot)
            .collect();

        let mut changed = Vec::new();
        for slot in slots {
            if self.set_slot(any, slot, target) {
                changed.push(slot);
            }
        }
        // One record per reference: a rollback has to put each of them
        // back on its own, and a map whose key and value waited on the
        // same name has two.
        for slot in &changed {
            self.record_reference_change(node, *slot, key, RefState::Resolved(target.node_id()));
        }
        !changed.is_empty()
    }

    /// Break every reference of `node` that waits on `key`.
    fn break_reference(&mut self, node: NodeId, key: &PendingRef, cause: &BreakCause) -> bool {
        let Some(any) = self.resolve_node_id(node) else {
            return false;
        };
        let slots: Vec<usize> = self
            .pending_slots(any)
            .into_iter()
            .filter(|slot| &slot.key == key)
            .map(|slot| slot.slot)
            .collect();

        let mut changed = Vec::new();
        for slot in slots {
            if self.break_slot(any, slot, cause.clone()) {
                changed.push(slot);
            }
        }
        for slot in &changed {
            self.record_reference_change(node, *slot, key, RefState::Broken);
        }
        !changed.is_empty()
    }

    fn record_reference_change(
        &mut self,
        node: NodeId,
        slot: usize,
        key: &PendingRef,
        after: RefState,
    ) {
        let change = zerodds_ast_core::Change::Updated {
            node,
            field: crate::field::REFERENCE,
            before: zerodds_ast_core::FieldValue::Reference(zerodds_ast_core::RefState::Pending),
            after: zerodds_ast_core::FieldValue::Reference(after),
        };
        let undo = zerodds_ast_core::UndoRecord {
            node,
            payload: zerodds_ast_core::UndoPayload::RestorePending {
                key: key.clone(),
                slot,
            },
        };
        if let Some(log) = self.pending.as_mut() {
            log.record(undo, change);
        } else if let Err(error) = self.record_standalone(undo, change) {
            self.last_commit_error = Some(error);
        }
    }

    /// Register a node's pending references in the waiter index.
    pub(crate) fn index_pending(&mut self, any: AnyId) {
        let node = any.node_id();
        for slot in self.pending_slots(any) {
            self.by_pending.insert(slot.key, node);
        }
    }

    /// Rebuild a node's entries in the waiter index.
    ///
    /// For a mutation that replaced or added references: the old keys go
    /// and the current ones are indexed. An unindexed pending reference
    /// never resolves.
    pub fn reindex_pending(&mut self, any: AnyId) {
        self.unindex_pending(any);
        self.index_pending(any);
    }

    /// Drop a node's pending keys from the waiter index.
    ///
    /// Needed wherever a node's references are replaced wholesale — a
    /// forward declaration gaining its body, for one. A stale waiter
    /// entry would send a later resolution at references that are gone.
    pub(crate) fn unindex_pending(&mut self, any: AnyId) {
        self.by_pending.remove_node(any.node_id());
    }
}
