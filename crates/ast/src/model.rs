// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The model façade. Spec §3.

use crate::error::AstError;
use crate::facet::Facets;
use crate::node::AnyId;
use crate::node::annotation::{
    AnnotationDefNode, AnnotationMemberNode, BUILTIN_ANNOTATIONS, BUILTIN_MODULE_NAME,
};
use crate::node::constant::{
    ConstExpr, ConstExprNode, ConstNode, ConstValue, EvalError, Literal, MAX_EXPR_DEPTH, UnaryOp,
};
use crate::node::deployment::{
    DomainNode, EndpointNode, EndpointRole, LibraryNode, ParticipantNode, QosPolicy,
    QosProfileNode, TopicNode,
};
use crate::node::exception::ExceptionNode;
use crate::node::interface::{
    AttributeNode, InterfaceKind, InterfaceNode, OperationNode, ParamDirection, ParamNode,
};
use crate::node::member::{
    BitValueNode, BitfieldNode, CaseLabel, CaseNode, EnumeratorNode, MemberNode,
};
use crate::node::module::ModuleNode;
use crate::node::type_::{Completeness, PrimitiveKind, TypeKind, TypeNode};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_ast_core::{
    Arena, Id, IdAllocator, IdSpace, NodeId, ObserverRegistry, Ref, ScopedName,
};

/// The common AST.
///
/// Nodes live in per-kind generational arenas; handles are the fast
/// access path, `NodeId` the stable one. Spec §1, §3.
pub struct Ast {
    pub(crate) modules: Arena<ModuleNode>,
    pub(crate) types: Arena<TypeNode>,
    pub(crate) members: Arena<MemberNode>,
    pub(crate) cases: Arena<CaseNode>,
    pub(crate) enumerators: Arena<EnumeratorNode>,
    pub(crate) bit_values: Arena<BitValueNode>,
    pub(crate) bitfields: Arena<BitfieldNode>,
    pub(crate) constants: Arena<ConstNode>,
    pub(crate) exprs: Arena<ConstExprNode>,
    pub(crate) interfaces: Arena<InterfaceNode>,
    pub(crate) operations: Arena<OperationNode>,
    pub(crate) params: Arena<ParamNode>,
    pub(crate) attributes: Arena<AttributeNode>,
    pub(crate) exceptions: Arena<ExceptionNode>,
    pub(crate) annotation_defs: Arena<AnnotationDefNode>,
    pub(crate) annotation_members: Arena<AnnotationMemberNode>,
    pub(crate) domains: Arena<DomainNode>,
    pub(crate) topics: Arena<TopicNode>,
    pub(crate) qos_profiles: Arena<QosProfileNode>,
    pub(crate) participants: Arena<ParticipantNode>,
    pub(crate) endpoints: Arena<EndpointNode>,
    pub(crate) libraries: Arena<LibraryNode>,

    pub(crate) model_ids: IdAllocator,
    pub(crate) builtin_ids: IdAllocator,
    pub(crate) synthetic_ids: IdAllocator,

    pub(crate) by_node_id: BTreeMap<NodeId, AnyId>,
    pub(crate) by_scoped_name: zerodds_ast_core::Index<ScopedName>,
    pub(crate) by_type_identifier: zerodds_ast_core::Index<zerodds_ast_core::TypeIdentifierKey>,
    pub(crate) by_ros_name: zerodds_ast_core::Index<zerodds_ast_core::RosTypeName>,
    pub(crate) by_repository_id: zerodds_ast_core::Index<zerodds_ast_core::RepositoryId>,
    /// Nodes waiting on each unresolved lookup key, so a newly arrived
    /// type resolves its waiters in one step. Spec §6.
    pub(crate) by_pending: zerodds_ast_core::Index<zerodds_ast_core::PendingRef>,
    pub(crate) observers: ObserverRegistry,
    pub(crate) sequence: u64,
    /// The transaction currently open, if any. Spec §5.1.
    pub(crate) pending: Option<zerodds_ast_core::TxnLog>,
    /// Nodes removed during the open transaction, kept so a rollback
    /// can restore them without serialising on the success path.
    pub(crate) stash: Vec<Option<crate::edit::Stashed>>,
    /// Failure of the most recent auto-committed mutation.
    pub(crate) last_commit_error: Option<AstError>,

    root: Id<ModuleNode>,
    builtin_module: Id<ModuleNode>,
}

/// Everything needed to declare a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberSpec {
    /// Simple name.
    pub name: String,
    /// Declared type.
    pub type_: Ref<TypeNode>,
    /// Per-source information.
    pub facets: Facets,
}

impl MemberSpec {
    /// A member with default facets.
    #[must_use]
    pub fn new(name: &str, type_: Ref<TypeNode>) -> Self {
        Self {
            name: name.to_string(),
            type_,
            facets: Facets::synthetic("member created through the model API"),
        }
    }
}

impl Default for Ast {
    fn default() -> Self {
        Self::new()
    }
}

impl Ast {
    /// A model containing the root module and the built-in declarations.
    ///
    /// Built-in identities come from [`IdSpace::BUILTIN`] and are
    /// allocated in a fixed order, so they are identical in every model
    /// and snapshots from different processes compare. Spec §3.6, §9.
    #[must_use]
    pub fn new() -> Self {
        let mut ast = Self::empty();
        ast.populate_builtins();
        ast
    }

    pub(crate) fn empty_public() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        let mut modules = Arena::new();
        let mut model_ids = IdAllocator::new(IdSpace::MODEL);
        // The root module is the first model allocation, so its identity
        // is the same in every model.
        let root_id = model_ids
            .allocate()
            .unwrap_or_else(|_| NodeId::from_parts(IdSpace::MODEL, 1));
        let root = modules
            .insert(ModuleNode {
                node_id: root_id,
                name: String::new(),
                parent: None,
                children: Vec::new(),
                facets: Facets::synthetic("root module"),
            })
            .unwrap_or(Id::first_at(0));

        let mut builtin_ids = IdAllocator::new(IdSpace::BUILTIN);
        let builtin_module_id = builtin_ids
            .allocate()
            .unwrap_or_else(|_| NodeId::from_parts(IdSpace::BUILTIN, 1));
        let builtin_module = modules
            .insert(ModuleNode {
                node_id: builtin_module_id,
                name: BUILTIN_MODULE_NAME.to_string(),
                parent: None,
                children: Vec::new(),
                facets: Facets::builtin(),
            })
            .unwrap_or(Id::first_at(1));

        let mut by_node_id = BTreeMap::new();
        let _ = by_node_id.insert(root_id, AnyId::Module(root, root_id));
        let _ = by_node_id.insert(
            builtin_module_id,
            AnyId::Module(builtin_module, builtin_module_id),
        );

        Self {
            modules,
            types: Arena::new(),
            members: Arena::new(),
            cases: Arena::new(),
            enumerators: Arena::new(),
            bit_values: Arena::new(),
            bitfields: Arena::new(),
            constants: Arena::new(),
            exprs: Arena::new(),
            interfaces: Arena::new(),
            operations: Arena::new(),
            params: Arena::new(),
            attributes: Arena::new(),
            exceptions: Arena::new(),
            annotation_defs: Arena::new(),
            annotation_members: Arena::new(),
            domains: Arena::new(),
            topics: Arena::new(),
            qos_profiles: Arena::new(),
            participants: Arena::new(),
            endpoints: Arena::new(),
            libraries: Arena::new(),
            model_ids,
            builtin_ids,
            synthetic_ids: IdAllocator::new(IdSpace::SYNTHETIC),
            by_node_id,
            by_scoped_name: zerodds_ast_core::Index::new(),
            by_type_identifier: zerodds_ast_core::Index::new(),
            by_ros_name: zerodds_ast_core::Index::new(),
            by_repository_id: zerodds_ast_core::Index::new(),
            by_pending: zerodds_ast_core::Index::new(),
            observers: ObserverRegistry::new(),
            sequence: 0,
            pending: None,
            stash: Vec::new(),
            last_commit_error: None,
            root,
            builtin_module,
        }
    }

    fn populate_builtins(&mut self) {
        for name in BUILTIN_ANNOTATIONS {
            let Ok(node_id) = self.builtin_ids.allocate() else {
                return;
            };
            let Ok(id) = self.annotation_defs.insert(AnnotationDefNode {
                node_id,
                name: name.to_string(),
                module: self.builtin_module,
                members: Vec::new(),
                facets: Facets::builtin(),
            }) else {
                return;
            };
            let any = AnyId::AnnotationDef(id, node_id);
            let _ = self.by_node_id.insert(node_id, any);
            if let Ok(module) = self.modules.get_mut(self.builtin_module) {
                module.children.push(any);
            }
            // Every built-in annotation carries the implicit `value`
            // member that enables the `@ann(x)` short form.
            let Ok(member_id) = self.builtin_ids.allocate() else {
                return;
            };
            let Ok(member) = self.annotation_members.insert(AnnotationMemberNode {
                node_id: member_id,
                name: "value".to_string(),
                type_: Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(
                    ScopedName::absolute(alloc::vec![name.to_string()]),
                )),
                default: None,
                is_value_member: true,
                parent: id,
                facets: Facets::builtin(),
            }) else {
                return;
            };
            let _ = self
                .by_node_id
                .insert(member_id, AnyId::AnnotationMember(member, member_id));
            if let Ok(def) = self.annotation_defs.get_mut(id) {
                def.members.push(member);
            }
        }
    }

    /// The anonymous global scope.
    #[must_use]
    pub const fn root_module(&self) -> Id<ModuleNode> {
        self.root
    }

    /// The module holding the built-in declarations.
    #[must_use]
    pub const fn builtin_module(&self) -> Id<ModuleNode> {
        self.builtin_module
    }

    /// A built-in annotation by name, without the `@`.
    #[must_use]
    pub fn builtin_annotation(&self, name: &str) -> Option<Id<AnnotationDefNode>> {
        self.modules
            .get(self.builtin_module)
            .ok()?
            .children
            .iter()
            .filter_map(|child| match child {
                AnyId::AnnotationDef(id, _) => Some(*id),
                _ => None,
            })
            .find(|id| {
                self.annotation_defs
                    .get(*id)
                    .is_ok_and(|def| def.name == name)
            })
    }

    /// Map a stable identity back to its handle and kind.
    #[must_use]
    pub fn resolve_node_id(&self, node: NodeId) -> Option<AnyId> {
        self.by_node_id.get(&node).copied()
    }

    /// The observer registry, for registering change consumers.
    pub fn observers_mut(&mut self) -> &mut ObserverRegistry {
        &mut self.observers
    }

    /// Total number of live nodes across every arena.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.by_node_id.len()
    }

    /// The sequence number of the last committed changeset.
    ///
    /// Zero on a fresh model; every commit increments it, so a consumer
    /// can tell whether it has seen every changeset. Spec §5.2.
    #[must_use]
    pub const fn commit_sequence(&self) -> u64 {
        self.sequence
    }

    /// Allocate an identity in [`IdSpace::SYNTHETIC`].
    ///
    /// For nodes a tool fabricates that correspond to nothing in any
    /// source — an implicit wrapper, a generated helper type. Keeping
    /// them in their own namespace means a diff between a parsed model
    /// and a fabricated one never confuses the two. Spec §1.2.
    ///
    /// # Errors
    /// [`AstError::Core`] when the synthetic counter is exhausted.
    pub fn allocate_synthetic_id(&mut self) -> Result<NodeId, AstError> {
        Ok(self.synthetic_ids.allocate()?)
    }

    pub(crate) fn next_model_id(&mut self) -> Result<NodeId, AstError> {
        Ok(self.model_ids.allocate()?)
    }

    /// Record that an aggregate's part list changed.
    ///
    /// Adding a member changes the member *and* the struct that holds
    /// it; without this the struct's own consumers never hear about it.
    pub(crate) fn note_parts_changed(&mut self, owner: NodeId) {
        let change = zerodds_ast_core::Change::Updated {
            node: owner,
            field: crate::field::PARTS,
            before: zerodds_ast_core::FieldValue::Absent,
            after: zerodds_ast_core::FieldValue::Node(owner),
        };
        let undo = zerodds_ast_core::UndoRecord {
            node: owner,
            payload: zerodds_ast_core::UndoPayload::RestoreField {
                field: crate::field::PARTS,
                value: zerodds_ast_core::FieldValue::Absent,
            },
        };
        if let Some(log) = self.pending.as_mut() {
            log.record(undo, change);
        } else if let Err(error) = self.record_standalone(undo, change) {
            self.last_commit_error = Some(error);
        }
    }

    pub(crate) fn register(&mut self, any: AnyId) {
        let _ = self.by_node_id.insert(any.node_id(), any);
        self.index_node(any);
        self.index_pending(any);
        // Emission is deliberately infallible here: the caller has
        // already mutated, so a rejected invariant must surface at the
        // caller's own Result, which every add_* returns. Auto-commit
        // rejection is reported through `last_commit_error`.
        if let Err(error) = self.record_created(any) {
            self.last_commit_error = Some(error);
        }
    }

    /// Hand back a mutation's result, or the rejection that undid it.
    ///
    /// A mutation outside a transaction validates on the spot and is
    /// rolled back when it fails. Returning `Ok` with a handle to a node
    /// that no longer exists would be the worst of both: the caller
    /// believes it succeeded and holds nothing.
    fn committed<T>(&mut self, value: T) -> Result<T, AstError> {
        match self.last_commit_error.take() {
            Some(error) => Err(error),
            None => Ok(value),
        }
    }

    /// The error of the most recent auto-committed mutation, if it was
    /// rejected.
    ///
    /// A mutation made outside an explicit transaction validates on the
    /// spot; when it fails, the model is rolled back and the failure is
    /// recorded here rather than lost. Taking it clears it.
    pub fn take_commit_error(&mut self) -> Option<AstError> {
        self.last_commit_error.take()
    }

    // -- modules ---------------------------------------------------------

    /// Borrow a module.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn module(&self, id: Id<ModuleNode>) -> Result<&ModuleNode, AstError> {
        Ok(self.modules.get(id)?)
    }

    /// Create the named child module, or return the existing one.
    ///
    /// This is what makes reopening a module across translation units
    /// converge on one node. Spec §3.1.
    ///
    /// # Errors
    /// [`AstError::Core`] if the parent handle is stale or identity
    /// space is exhausted.
    pub fn ensure_module(
        &mut self,
        parent: Id<ModuleNode>,
        name: &str,
    ) -> Result<Id<ModuleNode>, AstError> {
        let existing = self
            .modules
            .get(parent)?
            .children
            .iter()
            .filter_map(|child| child.as_module())
            .find(|id| self.modules.get(*id).is_ok_and(|m| m.name == name));
        if let Some(id) = existing {
            return Ok(id);
        }

        let node_id = self.next_model_id()?;
        let id = self.modules.insert(ModuleNode {
            node_id,
            name: name.to_string(),
            parent: Some(parent),
            children: Vec::new(),
            facets: Facets::synthetic("module created through the model API"),
        })?;
        let any = AnyId::Module(id, node_id);
        self.register(any);
        self.modules.get_mut(parent)?.children.push(any);
        self.committed(id)
    }

    /// Iterate every module in the model, in arena order.
    pub fn modules(&self) -> impl Iterator<Item = (Id<ModuleNode>, &ModuleNode)> {
        self.modules.iter()
    }

    /// Look a module up by its path from the root.
    #[must_use]
    pub fn module_by_path(&self, path: &ScopedName) -> Option<Id<ModuleNode>> {
        let mut current = self.root;
        for segment in path.segments() {
            current = self
                .modules
                .get(current)
                .ok()?
                .children
                .iter()
                .filter_map(|child| child.as_module())
                .find(|id| self.modules.get(*id).is_ok_and(|m| &m.name == segment))?;
        }
        Some(current)
    }

    // -- types -----------------------------------------------------------

    /// Borrow a type.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn type_(&self, id: Id<TypeNode>) -> Result<&TypeNode, AstError> {
        Ok(self.types.get(id)?)
    }

    /// Declare a type in a module.
    ///
    /// # Errors
    /// [`AstError::Core`] if the module handle is stale or identity
    /// space is exhausted.
    pub fn add_type(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        kind: TypeKind,
    ) -> Result<Id<TypeNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.types.insert(TypeNode {
            node_id,
            name: name.to_string(),
            module,
            scope: None,
            kind,
            completeness: Completeness::Complete,
            facets: Facets::synthetic("type created through the model API"),
        })?;
        let any = AnyId::Type(id, node_id);
        self.register(any);
        self.modules.get_mut(module)?.children.push(any);
        self.committed(id)
    }

    /// Declare a type whose body has not arrived yet.
    ///
    /// Not a separate node kind: completing it later is an update, so
    /// handles taken now stay valid. Spec §3.2.
    ///
    /// # Errors
    /// As [`Ast::add_type`].
    pub fn add_forward_type(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        kind: TypeKind,
    ) -> Result<Id<TypeNode>, AstError> {
        let id = self.add_type(module, name, kind)?;
        self.types.get_mut(id)?.completeness = Completeness::Forward;
        self.committed(id)
    }

    /// Give a forward-declared type its body.
    ///
    /// An update of the existing node, not a second one: every reference
    /// taken while only the forward declaration existed stays valid, which
    /// is why forwardness is a state rather than a kind. Spec §3.2.
    ///
    /// # Errors
    /// [`AstError::AlreadyComplete`] if the type already has a body —
    /// two definitions of one name are a duplicate, not a completion.
    /// [`AstError::Core`] on a stale handle.
    pub fn complete_type(&mut self, id: Id<TypeNode>, kind: TypeKind) -> Result<(), AstError> {
        let node = self.types.get(id)?;
        let node_id = node.node_id;
        if node.completeness == Completeness::Complete {
            return Err(AstError::AlreadyComplete { node: node_id });
        }
        // The old kind's pending references are about to disappear; the
        // waiter index has to lose them or a later definition of that
        // name would rewrite a member that no longer exists.
        self.unindex_pending(AnyId::Type(id, node_id));

        let node = self.types.get_mut(id)?;
        node.kind = kind;
        node.completeness = Completeness::Complete;

        self.index_pending(AnyId::Type(id, node_id));
        self.record_completeness_change(node_id);
        Ok(())
    }

    fn record_completeness_change(&mut self, node: NodeId) {
        let change = zerodds_ast_core::Change::Updated {
            node,
            field: crate::field::COMPLETENESS,
            before: zerodds_ast_core::FieldValue::Bool(false),
            after: zerodds_ast_core::FieldValue::Bool(true),
        };
        let undo = zerodds_ast_core::UndoRecord {
            node,
            payload: zerodds_ast_core::UndoPayload::RestoreField {
                field: crate::field::COMPLETENESS,
                value: zerodds_ast_core::FieldValue::Bool(false),
            },
        };
        if let Some(log) = self.pending.as_mut() {
            log.record(undo, change);
        } else if let Err(error) = self.record_standalone(undo, change) {
            self.last_commit_error = Some(error);
        }
    }

    /// Iterate every type in the model, in arena order.
    pub fn types(&self) -> impl Iterator<Item = (Id<TypeNode>, &TypeNode)> {
        self.types.iter()
    }

    // -- members and aggregate parts -------------------------------------

    /// Borrow a member.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn member(&self, id: Id<MemberNode>) -> Result<&MemberNode, AstError> {
        Ok(self.members.get(id)?)
    }

    /// Iterate every member in the model, in arena order.
    pub fn members(&self) -> impl Iterator<Item = (Id<MemberNode>, &MemberNode)> {
        self.members.iter()
    }

    /// Add a member to a struct or valuetype.
    ///
    /// # Errors
    /// [`AstError::WrongKind`] if the target is not an aggregate that
    /// holds members, [`AstError::Core`] on a stale handle.
    pub fn add_member(
        &mut self,
        aggregate: Id<TypeNode>,
        spec: MemberSpec,
    ) -> Result<Id<MemberNode>, AstError> {
        let parent_node_id = self.types.get(aggregate)?.node_id;
        let parent = AnyId::Type(aggregate, parent_node_id);
        let node_id = self.next_model_id()?;
        let id = self.members.insert(MemberNode {
            node_id,
            name: spec.name,
            type_: spec.type_,
            parent,
            facets: spec.facets,
        })?;
        self.register(AnyId::Member(id, node_id));
        match &mut self.types.get_mut(aggregate)?.kind {
            TypeKind::Structure { members, .. } | TypeKind::Valuetype { state: members, .. } => {
                members.push(id);
                self.note_parts_changed(parent_node_id);
                self.committed(id)
            }
            other => {
                let found = other.as_str();
                let _ = self.members.remove(id);
                let _ = self.by_node_id.remove(&node_id);
                Err(AstError::NotAnAggregate {
                    node: parent_node_id,
                    found,
                })
            }
        }
    }

    /// [`Self::add_member`] at a position among the aggregate's members.
    ///
    /// DDS-XTypes 1.3 §7.5.2.7.6: "any value greater than the current
    /// largest index value in the type shall be taken to indicate that the
    /// new member will become the last member"; a member at `index` and
    /// those after it "shall be shifted up by a single index value".
    ///
    /// # Errors
    /// As [`Self::add_member`].
    pub fn add_member_at(
        &mut self,
        aggregate: Id<TypeNode>,
        spec: MemberSpec,
        index: usize,
    ) -> Result<Id<MemberNode>, AstError> {
        let id = self.add_member(aggregate, spec)?;
        let parent_node_id = self.types.get(aggregate)?.node_id;
        if let TypeKind::Structure { members, .. } | TypeKind::Valuetype { state: members, .. } =
            &mut self.types.get_mut(aggregate)?.kind
        {
            // The member just pushed is last; it moves to its position.
            members.pop();
            let at = index.min(members.len());
            members.insert(at, id);
            self.note_parts_changed(parent_node_id);
        }
        Ok(id)
    }

    /// [`Self::add_case`] at a position among the union's cases, with the
    /// rule of [`Self::add_member_at`].
    ///
    /// # Errors
    /// As [`Self::add_case`].
    pub fn add_case_at(
        &mut self,
        union: Id<TypeNode>,
        labels: Vec<CaseLabel>,
        spec: MemberSpec,
        index: usize,
    ) -> Result<Id<CaseNode>, AstError> {
        let id = self.add_case(union, labels, spec)?;
        let union_node_id = self.types.get(union)?.node_id;
        if let TypeKind::Union { cases, .. } = &mut self.types.get_mut(union)?.kind {
            cases.pop();
            let at = index.min(cases.len());
            cases.insert(at, id);
            self.note_parts_changed(union_node_id);
        }
        Ok(id)
    }

    /// Add a member to an exception.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_exception_member(
        &mut self,
        exception: Id<ExceptionNode>,
        spec: MemberSpec,
    ) -> Result<Id<MemberNode>, AstError> {
        let parent_node_id = self.exceptions.get(exception)?.node_id;
        let node_id = self.next_model_id()?;
        let id = self.members.insert(MemberNode {
            node_id,
            name: spec.name,
            type_: spec.type_,
            parent: AnyId::Exception(exception, parent_node_id),
            facets: spec.facets,
        })?;
        self.register(AnyId::Member(id, node_id));
        self.exceptions.get_mut(exception)?.members.push(id);
        self.note_parts_changed(parent_node_id);
        self.committed(id)
    }

    /// Borrow a union case.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn case(&self, id: Id<CaseNode>) -> Result<&CaseNode, AstError> {
        Ok(self.cases.get(id)?)
    }

    /// Iterate every cases node in the model, in arena order.
    pub fn cases(&self) -> impl Iterator<Item = (Id<CaseNode>, &CaseNode)> {
        self.cases.iter()
    }

    /// Add a case to a union.
    ///
    /// A case carrying `CaseLabel::Default` is also recorded as the
    /// union's default case.
    ///
    /// # Errors
    /// [`AstError::NotAnAggregate`] if the target is not a union.
    pub fn add_case(
        &mut self,
        union: Id<TypeNode>,
        labels: Vec<CaseLabel>,
        spec: MemberSpec,
    ) -> Result<Id<CaseNode>, AstError> {
        let union_node_id = self.types.get(union)?.node_id;
        if !matches!(self.types.get(union)?.kind, TypeKind::Union { .. }) {
            return Err(AstError::NotAnAggregate {
                node: union_node_id,
                found: self.types.get(union)?.kind.as_str(),
            });
        }

        let member_node_id = self.next_model_id()?;
        let member = self.members.insert(MemberNode {
            node_id: member_node_id,
            name: spec.name,
            type_: spec.type_,
            parent: AnyId::Type(union, union_node_id),
            facets: spec.facets,
        })?;
        self.register(AnyId::Member(member, member_node_id));

        let node_id = self.next_model_id()?;
        let is_default = labels.iter().any(|l| l.is_default());
        let id = self.cases.insert(CaseNode {
            node_id,
            labels,
            member,
            parent: union,
            facets: Facets::synthetic("case created through the model API"),
        })?;
        self.register(AnyId::Case(id, node_id));

        if let TypeKind::Union {
            cases,
            default_case,
            ..
        } = &mut self.types.get_mut(union)?.kind
        {
            cases.push(id);
            if is_default {
                *default_case = Some(id);
            }
        }
        self.note_parts_changed(union_node_id);
        self.committed(id)
    }

    /// Borrow an enumerator.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn enumerator(&self, id: Id<EnumeratorNode>) -> Result<&EnumeratorNode, AstError> {
        Ok(self.enumerators.get(id)?)
    }

    /// Iterate every enumerator in the model, in arena order.
    pub fn enumerators(&self) -> impl Iterator<Item = (Id<EnumeratorNode>, &EnumeratorNode)> {
        self.enumerators.iter()
    }

    /// Borrow an enumerator mutably.
    ///
    /// For `@value(n)`, which a frontend applies after the enumerator
    /// exists.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn enumerator_mut(
        &mut self,
        id: Id<EnumeratorNode>,
    ) -> Result<&mut EnumeratorNode, AstError> {
        Ok(self.enumerators.get_mut(id)?)
    }

    /// Add an enumerator to an enumeration.
    ///
    /// `value` is `None` for a positional enumerator and `Some` for an
    /// explicit `@value(n)`; the two encode differently. Spec §3.3.
    ///
    /// # Errors
    /// [`AstError::NotAnAggregate`] if the target is not an enumeration.
    pub fn add_enumerator(
        &mut self,
        enumeration: Id<TypeNode>,
        name: &str,
        value: Option<Id<ConstExprNode>>,
    ) -> Result<Id<EnumeratorNode>, AstError> {
        let parent_node_id = self.types.get(enumeration)?.node_id;
        if !matches!(
            self.types.get(enumeration)?.kind,
            TypeKind::Enumeration { .. }
        ) {
            return Err(AstError::NotAnAggregate {
                node: parent_node_id,
                found: self.types.get(enumeration)?.kind.as_str(),
            });
        }
        let node_id = self.next_model_id()?;
        let id = self.enumerators.insert(EnumeratorNode {
            node_id,
            name: name.to_string(),
            value,
            parent: enumeration,
            facets: Facets::synthetic("enumerator created through the model API"),
        })?;
        self.register(AnyId::Enumerator(id, node_id));
        if let TypeKind::Enumeration { enumerators } = &mut self.types.get_mut(enumeration)?.kind {
            enumerators.push(id);
        }
        self.note_parts_changed(parent_node_id);
        self.committed(id)
    }

    /// Borrow a bitmask bit.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn bit_value(&self, id: Id<BitValueNode>) -> Result<&BitValueNode, AstError> {
        Ok(self.bit_values.get(id)?)
    }

    /// Borrow a bitmask bit mutably.
    ///
    /// For `@position(k)`, which a frontend applies after the bit exists.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn bit_value_mut(&mut self, id: Id<BitValueNode>) -> Result<&mut BitValueNode, AstError> {
        Ok(self.bit_values.get_mut(id)?)
    }

    /// Iterate every bit values node in the model, in arena order.
    pub fn bit_values(&self) -> impl Iterator<Item = (Id<BitValueNode>, &BitValueNode)> {
        self.bit_values.iter()
    }

    /// Add a bit to a bitmask.
    ///
    /// `position` is `None` for a positional bit and `Some` for an
    /// explicit `@position(k)`. Spec §3.3.
    ///
    /// # Errors
    /// [`AstError::NotAnAggregate`] if the target is not a bitmask.
    pub fn add_bit_value(
        &mut self,
        bitmask: Id<TypeNode>,
        name: &str,
        position: Option<u16>,
    ) -> Result<Id<BitValueNode>, AstError> {
        let parent_node_id = self.types.get(bitmask)?.node_id;
        if !matches!(self.types.get(bitmask)?.kind, TypeKind::Bitmask { .. }) {
            return Err(AstError::NotAnAggregate {
                node: parent_node_id,
                found: self.types.get(bitmask)?.kind.as_str(),
            });
        }
        let node_id = self.next_model_id()?;
        let id = self.bit_values.insert(BitValueNode {
            node_id,
            name: name.to_string(),
            position,
            parent: bitmask,
            facets: Facets::synthetic("bit value created through the model API"),
        })?;
        self.register(AnyId::BitValue(id, node_id));
        if let TypeKind::Bitmask { bits } = &mut self.types.get_mut(bitmask)?.kind {
            bits.push(id);
        }
        self.note_parts_changed(parent_node_id);
        self.committed(id)
    }

    /// Borrow a bitset field.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn bitfield(&self, id: Id<BitfieldNode>) -> Result<&BitfieldNode, AstError> {
        Ok(self.bitfields.get(id)?)
    }

    /// Add a field to a bitset. `name` is `None` for anonymous padding.
    ///
    /// # Errors
    /// [`AstError::NotAnAggregate`] if the target is not a bitset.
    pub fn add_bitfield(
        &mut self,
        bitset: Id<TypeNode>,
        name: Option<&str>,
        bits: u16,
        holder_type: Option<Ref<TypeNode>>,
    ) -> Result<Id<BitfieldNode>, AstError> {
        let parent_node_id = self.types.get(bitset)?.node_id;
        if !matches!(self.types.get(bitset)?.kind, TypeKind::Bitset { .. }) {
            return Err(AstError::NotAnAggregate {
                node: parent_node_id,
                found: self.types.get(bitset)?.kind.as_str(),
            });
        }
        let node_id = self.next_model_id()?;
        let id = self.bitfields.insert(BitfieldNode {
            node_id,
            name: name.map(ToString::to_string),
            bits,
            holder_type,
            parent: bitset,
            facets: Facets::synthetic("bitfield created through the model API"),
        })?;
        self.register(AnyId::Bitfield(id, node_id));
        if let TypeKind::Bitset { fields, .. } = &mut self.types.get_mut(bitset)?.kind {
            fields.push(id);
        }
        self.note_parts_changed(parent_node_id);
        self.committed(id)
    }

    // -- constants -------------------------------------------------------

    /// Borrow an expression node.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn expr(&self, id: Id<ConstExprNode>) -> Result<&ConstExprNode, AstError> {
        Ok(self.exprs.get(id)?)
    }

    /// Add an expression node.
    ///
    /// # Errors
    /// [`AstError::Core`] when identity or slot space is exhausted.
    pub fn add_expr(&mut self, expr: ConstExpr) -> Result<Id<ConstExprNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.exprs.insert(ConstExprNode {
            node_id,
            expr,
            facets: Facets::synthetic("expression created through the model API"),
        })?;
        self.register(AnyId::ConstExpr(id, node_id));
        self.committed(id)
    }

    /// Add an integer literal expression, the common case.
    ///
    /// # Errors
    /// As [`Ast::add_expr`].
    pub fn add_int_expr(&mut self, value: i128) -> Result<Id<ConstExprNode>, AstError> {
        self.add_expr(ConstExpr::Literal(Literal::Integer(value)))
    }

    /// Borrow a constant.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn constant(&self, id: Id<ConstNode>) -> Result<&ConstNode, AstError> {
        Ok(self.constants.get(id)?)
    }

    /// Iterate every constants node in the model, in arena order.
    pub fn constants(&self) -> impl Iterator<Item = (Id<ConstNode>, &ConstNode)> {
        self.constants.iter()
    }

    /// Declare a constant in a module.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale module handle.
    pub fn add_const(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        type_: Ref<TypeNode>,
        expr: Id<ConstExprNode>,
    ) -> Result<Id<ConstNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.constants.insert(ConstNode {
            node_id,
            name: name.to_string(),
            module,
            scope: None,
            type_,
            expr,
            facets: Facets::synthetic("constant created through the model API"),
        })?;
        let any = AnyId::Const(id, node_id);
        self.register(any);
        self.modules.get_mut(module)?.children.push(any);
        self.committed(id)
    }

    /// Evaluate an expression to an integral value.
    ///
    /// The tree is not mutated: evaluation is a query, so the written
    /// form stays available to the formatter and the linter. Spec §3.5.
    ///
    /// # Errors
    /// [`EvalError::Unresolved`] when the tree reaches a reference that
    /// is not resolved — a partially discovered model fails loudly
    /// rather than substituting a default. Also
    /// [`EvalError::DivisionByZero`], [`EvalError::Overflow`],
    /// [`EvalError::InvalidShift`], [`EvalError::NotIntegral`],
    /// [`EvalError::DanglingExpr`] and [`EvalError::TooDeep`].
    pub fn eval_integral(&self, expr: Id<ConstExprNode>) -> Result<i128, EvalError> {
        self.eval_integral_at(expr, 0)
    }

    /// Evaluate an expression to its value, whatever kind that is.
    ///
    /// [`Self::eval_integral`] is the right question wherever the wire
    /// needs a number, and the wrong one wherever a backend only needs to
    /// know what a constant says. `const string MODULE_ID = "…";` is
    /// evaluated and has no integer, so an integral evaluator reports
    /// `NotIntegral` and a caller that has nothing else to ask cannot emit
    /// the constant at all.
    ///
    /// An expression with an operator in it is integral and comes back as
    /// [`ConstValue::Integer`], unless a floating-point operand stands in
    /// it: then it is evaluated in `double` as IDL 4.2 §7.4.1.4.2 says and
    /// comes back as [`ConstValue::Float`]; a fixed-point operand makes it
    /// exact decimal arithmetic after Table 7-11, back as
    /// [`ConstValue::Fixed`].
    ///
    /// # Errors
    /// As [`Self::eval_integral`]; for floating arithmetic also
    /// [`EvalError::MixedOperands`], [`EvalError::NotFloatingOperator`]
    /// and [`EvalError::LongDouble`], for fixed-point arithmetic
    /// [`EvalError::NotFixedOperator`]. A literal alone always has a value.
    pub fn eval_const(&self, expr: Id<ConstExprNode>) -> Result<ConstValue, EvalError> {
        self.eval_const_at(expr, expr, 0)
    }

    /// `root` is the whole expression `expr` is part of: the constant it
    /// belongs to, if any, decides the precision of floating arithmetic.
    fn eval_const_at(
        &self,
        expr: Id<ConstExprNode>,
        root: Id<ConstExprNode>,
        depth: usize,
    ) -> Result<ConstValue, EvalError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(EvalError::TooDeep);
        }
        let node = self.exprs.get(expr).map_err(|_| EvalError::DanglingExpr)?;
        let at = node.node_id;
        match &node.expr {
            ConstExpr::Literal(Literal::Boolean(value)) => Ok(ConstValue::Boolean(*value)),
            ConstExpr::Literal(Literal::Integer(value)) => Ok(ConstValue::Integer(*value)),
            ConstExpr::Literal(Literal::Char(value)) => Ok(ConstValue::Char(*value)),
            ConstExpr::Literal(Literal::WChar(value)) => Ok(ConstValue::WChar(*value)),
            ConstExpr::Literal(Literal::String(text)) => Ok(ConstValue::String(text.clone())),
            ConstExpr::Literal(Literal::WString(units)) => Ok(ConstValue::WString(units.clone())),
            ConstExpr::Literal(Literal::Float { mantissa, exponent }) => Ok(ConstValue::Float {
                mantissa: mantissa.clone(),
                exponent: exponent.clone(),
            }),
            ConstExpr::Literal(Literal::Fixed { digits, scale }) => Ok(ConstValue::Fixed {
                digits: digits.clone(),
                scale: *scale,
            }),
            ConstExpr::Literal(Literal::Enumerator(_)) => {
                self.eval_integral_at(expr, depth).map(ConstValue::Integer)
            }
            ConstExpr::NameRef(reference) => {
                let target = reference.resolved().ok_or(EvalError::Unresolved { at })?;
                let constant = self
                    .constants
                    .get(target)
                    .map_err(|_| EvalError::DanglingExpr)?;
                self.eval_const_at(constant.expr, constant.expr, depth + 1)
            }
            // A sign in front of a floating literal, before the integral
            // evaluator gets it.
            //
            // Every unary and binary node used to go straight to
            // `eval_integral_at`, whatever its operands were, so
            // `const double SLOPE = -1.5;` asked for the integer value
            // of a `Float` literal and came back "does not evaluate".
            // IDL 4.2 §7.4.1.4.2 has `<const_exp>` over the floating
            // types as much as over the integer ones, and the Ada column
            // hit this the moment it wanted a negative constant in a
            // sample.
            //
            // Only the sign is answered here, and answering it needs no
            // arithmetic at all: `ConstValue::Float` keeps the mantissa
            // as the text the source wrote, so a `-` is one character in
            // front of it. Nothing is rounded and nothing is parsed; the
            // rest of floating-point arithmetic, which does round, is
            // `eval_floating_root` below.
            ConstExpr::Unary {
                op: op @ (UnaryOp::Minus | UnaryOp::Plus),
                operand,
            } => {
                let inner = self.eval_const_at(*operand, root, depth + 1)?;
                match inner {
                    ConstValue::Float { mantissa, exponent } if *op == UnaryOp::Minus => {
                        Ok(ConstValue::Float {
                            mantissa: match mantissa.strip_prefix('-') {
                                // `- -1.5` is `1.5`: two signs cancel
                                // rather than stacking into `--1.5`,
                                // which no reader of the text would take
                                // for a number.
                                Some(rest) => rest.to_string(),
                                None => alloc::format!("-{mantissa}"),
                            },
                            exponent,
                        })
                    }
                    ConstValue::Float { .. } => Ok(inner),
                    // A fixed-point value's sign is text as well: "Prefix
                    // operators do not affect the precision" (§7.4.1.4.2).
                    ConstValue::Fixed { digits, scale } => {
                        let value = crate::decimal::Decimal::parse(&digits, scale)
                            .map_err(|_| EvalError::NotIntegral { at })?;
                        let value = if *op == UnaryOp::Minus {
                            value.negated()
                        } else {
                            value
                        };
                        let (digits, scale) =
                            value.to_parts().map_err(|_| EvalError::Overflow { at })?;
                        Ok(ConstValue::Fixed { digits, scale })
                    }
                    _ => self.eval_integral_at(expr, depth).map(ConstValue::Integer),
                }
            }
            ConstExpr::Unary { .. } | ConstExpr::Binary { .. } => {
                if self.is_floating_at(expr, depth)? {
                    self.eval_floating_root(expr, root, depth)
                } else if self.is_fixed_at(expr, depth)? {
                    self.eval_fixed_root(expr, root, depth)
                } else {
                    self.eval_integral_at(expr, depth).map(ConstValue::Integer)
                }
            }
        }
    }

    /// Whether an expression is floating-point arithmetic: a floating
    /// literal, or a constant that evaluates to one, stands among its
    /// operands. Mixtures are decided later, by
    /// [`Self::eval_floating_at`], which refuses them.
    fn is_floating_at(&self, expr: Id<ConstExprNode>, depth: usize) -> Result<bool, EvalError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(EvalError::TooDeep);
        }
        let node = self.exprs.get(expr).map_err(|_| EvalError::DanglingExpr)?;
        match &node.expr {
            ConstExpr::Literal(Literal::Float { .. }) => Ok(true),
            ConstExpr::Literal(_) => Ok(false),
            ConstExpr::NameRef(reference) => {
                let target = reference
                    .resolved()
                    .ok_or(EvalError::Unresolved { at: node.node_id })?;
                let constant = self
                    .constants
                    .get(target)
                    .map_err(|_| EvalError::DanglingExpr)?;
                self.is_floating_at(constant.expr, depth + 1)
            }
            ConstExpr::Unary { operand, .. } => self.is_floating_at(*operand, depth + 1),
            ConstExpr::Binary { lhs, rhs, .. } => Ok(
                self.is_floating_at(*lhs, depth + 1)? || self.is_floating_at(*rhs, depth + 1)?
            ),
        }
    }

    /// Whether an expression is fixed-point arithmetic: a fixed-point
    /// literal, or a constant that evaluates to one, stands among its
    /// operands. Mixtures are refused by [`Self::eval_fixed_at`].
    fn is_fixed_at(&self, expr: Id<ConstExprNode>, depth: usize) -> Result<bool, EvalError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(EvalError::TooDeep);
        }
        let node = self.exprs.get(expr).map_err(|_| EvalError::DanglingExpr)?;
        match &node.expr {
            ConstExpr::Literal(Literal::Fixed { .. }) => Ok(true),
            ConstExpr::Literal(_) => Ok(false),
            ConstExpr::NameRef(reference) => {
                let target = reference
                    .resolved()
                    .ok_or(EvalError::Unresolved { at: node.node_id })?;
                let constant = self
                    .constants
                    .get(target)
                    .map_err(|_| EvalError::DanglingExpr)?;
                self.is_fixed_at(constant.expr, depth + 1)
            }
            ConstExpr::Unary { operand, .. } => self.is_fixed_at(*operand, depth + 1),
            ConstExpr::Binary { lhs, rhs, .. } => {
                Ok(self.is_fixed_at(*lhs, depth + 1)? || self.is_fixed_at(*rhs, depth + 1)?)
            }
        }
    }

    /// Fixed-point arithmetic, IDL 4.2 §7.4.1.4.2 and Table 7-11: each
    /// pairwise computation exact, a result of more than 31 significant
    /// digits cut to 31 on the right with no rounding, a quotient run to
    /// 31 significant digits. A constant of a `fixed<D,S>` type keeps
    /// what fits: places past `S` are discarded ("Truncation on the right
    /// for fixed point types is OK") and more than `D - S` whole digits
    /// are an [`EvalError::Overflow`] ("too large to fit … shall be
    /// treated as an error").
    fn eval_fixed_root(
        &self,
        expr: Id<ConstExprNode>,
        root: Id<ConstExprNode>,
        depth: usize,
    ) -> Result<ConstValue, EvalError> {
        let at = self
            .exprs
            .get(expr)
            .map_err(|_| EvalError::DanglingExpr)?
            .node_id;
        let mut value = self.eval_fixed_at(expr, depth)?;
        let declared = match self
            .constants
            .iter()
            .find(|(_, constant)| constant.expr == root)
        {
            Some((_, constant)) => self.fixed_type_of(&constant.type_, at)?,
            None => None,
        };
        if let Some((digits, scale)) = declared {
            value = value
                .fitted(digits, scale)
                .ok_or(EvalError::Overflow { at })?;
        }
        let (digits, scale) = value.to_parts().map_err(|_| EvalError::Overflow { at })?;
        Ok(ConstValue::Fixed { digits, scale })
    }

    /// `fixed<D,S>` of a constant's type, through typedefs; `None` for any
    /// other type.
    fn fixed_type_of(&self, ty: &Ref<TypeNode>, at: NodeId) -> Result<Option<(u8, u8)>, EvalError> {
        let mut next = ty.resolved().ok_or(EvalError::Unresolved { at })?;
        for _ in 0..MAX_EXPR_DEPTH {
            let node = self.types.get(next).map_err(|_| EvalError::DanglingExpr)?;
            match &node.kind {
                TypeKind::Fixed { digits, scale } => return Ok(Some((*digits, *scale))),
                TypeKind::Alias { aliased } => {
                    next = aliased.resolved().ok_or(EvalError::Unresolved { at })?;
                }
                _ => return Ok(None),
            }
        }
        Err(EvalError::TooDeep)
    }

    fn eval_fixed_at(
        &self,
        expr: Id<ConstExprNode>,
        depth: usize,
    ) -> Result<crate::decimal::Decimal, EvalError> {
        use crate::decimal::{Decimal, DecimalError};
        if depth > MAX_EXPR_DEPTH {
            return Err(EvalError::TooDeep);
        }
        let node = self.exprs.get(expr).map_err(|_| EvalError::DanglingExpr)?;
        let at = node.node_id;
        let failed = |error: DecimalError| match error {
            DecimalError::DivisionByZero => EvalError::DivisionByZero { at },
            DecimalError::TooLarge => EvalError::Overflow { at },
            DecimalError::NotDecimal => EvalError::NotIntegral { at },
        };
        match &node.expr {
            ConstExpr::Literal(Literal::Fixed { digits, scale }) => {
                Decimal::parse(digits, *scale).map_err(failed)
            }
            ConstExpr::Literal(_) => Err(EvalError::MixedOperands { at }),
            ConstExpr::NameRef(reference) => {
                let target = reference.resolved().ok_or(EvalError::Unresolved { at })?;
                let constant = self
                    .constants
                    .get(target)
                    .map_err(|_| EvalError::DanglingExpr)?;
                self.eval_fixed_at(constant.expr, depth + 1)
            }
            ConstExpr::Unary { op, operand } => {
                let value = self.eval_fixed_at(*operand, depth + 1)?;
                match op {
                    UnaryOp::Plus => Ok(value),
                    UnaryOp::Minus => Ok(value.negated()),
                    UnaryOp::Not => Err(EvalError::NotFixedOperator { at }),
                }
            }
            ConstExpr::Binary { op, lhs, rhs } => {
                use crate::node::constant::BinaryOp as B;
                let left = self.eval_fixed_at(*lhs, depth + 1)?;
                let right = self.eval_fixed_at(*rhs, depth + 1)?;
                match op {
                    B::Add => left.add(&right).map_err(failed),
                    B::Sub => left.subtract(&right).map_err(failed),
                    B::Mul => left.multiply(&right).map_err(failed),
                    B::Div => left.divide(&right).map_err(failed),
                    B::Or | B::Xor | B::And | B::Shl | B::Shr | B::Mod => {
                        Err(EvalError::NotFixedOperator { at })
                    }
                }
            }
        }
    }

    /// Floating-point arithmetic, IDL 4.2 §7.4.1.4.2: "If the type of a
    /// floating-point constant is double, then each sub-expression of the
    /// associated constant expression is treated as a double", and "If
    /// either argument is long double, it shall use long double. Otherwise
    /// it shall use double." A `float` constant is no exception — the
    /// clause has no float arithmetic — and its final value has to fit
    /// the `float` range.
    ///
    /// The result is the shortest decimal that reads back as the same
    /// `double` (`{:?}` on `f64`), split into the two halves
    /// [`ConstValue::Float`] keeps.
    fn eval_floating_root(
        &self,
        expr: Id<ConstExprNode>,
        root: Id<ConstExprNode>,
        depth: usize,
    ) -> Result<ConstValue, EvalError> {
        let at = self
            .exprs
            .get(expr)
            .map_err(|_| EvalError::DanglingExpr)?
            .node_id;
        // The constant this expression is the value of; an expression
        // that belongs to none (an annotation parameter, a case label) is
        // evaluated in `double`, which is what the clause gives a
        // sub-expression without a long double operand.
        let declared = match self
            .constants
            .iter()
            .find(|(_, constant)| constant.expr == root)
        {
            Some((_, constant)) => self.floating_kind_of(&constant.type_, at)?,
            None => None,
        };
        if declared == Some(PrimitiveKind::Float128) {
            return Err(EvalError::LongDouble { at });
        }
        let value = self.eval_floating_at(expr, depth)?;
        if declared == Some(PrimitiveKind::Float32) && value.abs() > f64::from(f32::MAX) {
            return Err(EvalError::Overflow { at });
        }
        let text = alloc::format!("{value:?}");
        Ok(match text.split_once('e') {
            Some((mantissa, exponent)) => ConstValue::Float {
                mantissa: mantissa.to_string(),
                exponent: Some(exponent.to_string()),
            },
            None => ConstValue::Float {
                mantissa: text,
                exponent: None,
            },
        })
    }

    /// The floating-point kind a constant's type is, through typedefs;
    /// `None` for any other type.
    ///
    /// # Errors
    /// [`EvalError::Unresolved`] at `at` when the type or a typedef on the
    /// way is not resolved: whether the arithmetic may run in `double`
    /// depends on it, and a partial model is not taken for a `double`
    /// one. [`EvalError::DanglingExpr`] for a stale handle and
    /// [`EvalError::TooDeep`] for a typedef chain longer than evaluation
    /// follows.
    fn floating_kind_of(
        &self,
        ty: &Ref<TypeNode>,
        at: NodeId,
    ) -> Result<Option<PrimitiveKind>, EvalError> {
        let mut next = ty.resolved().ok_or(EvalError::Unresolved { at })?;
        for _ in 0..MAX_EXPR_DEPTH {
            let node = self.types.get(next).map_err(|_| EvalError::DanglingExpr)?;
            match &node.kind {
                TypeKind::Primitive(
                    kind @ (PrimitiveKind::Float32
                    | PrimitiveKind::Float64
                    | PrimitiveKind::Float128),
                ) => return Ok(Some(*kind)),
                TypeKind::Alias { aliased } => {
                    next = aliased.resolved().ok_or(EvalError::Unresolved { at })?;
                }
                _ => return Ok(None),
            }
        }
        Err(EvalError::TooDeep)
    }

    fn eval_floating_at(&self, expr: Id<ConstExprNode>, depth: usize) -> Result<f64, EvalError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(EvalError::TooDeep);
        }
        let node = self.exprs.get(expr).map_err(|_| EvalError::DanglingExpr)?;
        let at = node.node_id;
        let finite = |value: f64| {
            if value.is_finite() {
                Ok(value)
            } else {
                Err(EvalError::Overflow { at })
            }
        };
        match &node.expr {
            ConstExpr::Literal(Literal::Float { mantissa, exponent }) => {
                let text = match exponent {
                    Some(exponent) => alloc::format!("{mantissa}e{exponent}"),
                    None => mantissa.clone(),
                };
                let value: f64 = text.parse().map_err(|_| EvalError::NotIntegral { at })?;
                // "Constant floating point literals are considered double
                // unless the value is too large, then they are considered
                // long double."
                if value.is_finite() {
                    Ok(value)
                } else {
                    Err(EvalError::LongDouble { at })
                }
            }
            ConstExpr::Literal(_) => Err(EvalError::MixedOperands { at }),
            ConstExpr::NameRef(reference) => {
                let target = reference.resolved().ok_or(EvalError::Unresolved { at })?;
                let constant = self
                    .constants
                    .get(target)
                    .map_err(|_| EvalError::DanglingExpr)?;
                if self.floating_kind_of(&constant.type_, at)? == Some(PrimitiveKind::Float128) {
                    return Err(EvalError::LongDouble { at });
                }
                self.eval_floating_at(constant.expr, depth + 1)
            }
            ConstExpr::Unary { op, operand } => {
                let value = self.eval_floating_at(*operand, depth + 1)?;
                match op {
                    UnaryOp::Plus => Ok(value),
                    UnaryOp::Minus => Ok(-value),
                    UnaryOp::Not => Err(EvalError::NotFloatingOperator { at }),
                }
            }
            ConstExpr::Binary { op, lhs, rhs } => {
                use crate::node::constant::BinaryOp as B;
                let left = self.eval_floating_at(*lhs, depth + 1)?;
                let right = self.eval_floating_at(*rhs, depth + 1)?;
                match op {
                    B::Add => finite(left + right),
                    B::Sub => finite(left - right),
                    B::Mul => finite(left * right),
                    B::Div if right == 0.0 => Err(EvalError::DivisionByZero { at }),
                    B::Div => finite(left / right),
                    B::Or | B::Xor | B::And | B::Shl | B::Shr | B::Mod => {
                        Err(EvalError::NotFloatingOperator { at })
                    }
                }
            }
        }
    }

    fn eval_integral_at(&self, expr: Id<ConstExprNode>, depth: usize) -> Result<i128, EvalError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(EvalError::TooDeep);
        }
        let node = self.exprs.get(expr).map_err(|_| EvalError::DanglingExpr)?;
        let at = node.node_id;
        match &node.expr {
            ConstExpr::Literal(Literal::Integer(value)) => Ok(*value),
            ConstExpr::Literal(Literal::Boolean(value)) => Ok(i128::from(*value)),
            ConstExpr::Literal(Literal::Char(value)) => Ok(i128::from(u32::from(*value))),
            ConstExpr::Literal(Literal::WChar(value)) => Ok(i128::from(*value)),
            // An enumerator's value is not an expression of its own: it
            // is `@value(n)` or its position among its siblings. OMG IDL
            // 4.2 §7.4.1.4.2 admits it wherever a `<const_expr>` is
            // admitted, and §7.4.1.4.4.4.2 rule (54) makes a union's
            // `<case_label>` one of those places.
            ConstExpr::Literal(Literal::Enumerator(reference)) => {
                let target = reference.resolved().ok_or(EvalError::Unresolved { at })?;
                self.effective_enumerator_value(target)
                    .ok_or(EvalError::Unresolved { at })
            }
            ConstExpr::Literal(_) => Err(EvalError::NotIntegral { at }),
            ConstExpr::NameRef(reference) => {
                let target = reference.resolved().ok_or(EvalError::Unresolved { at })?;
                let constant = self
                    .constants
                    .get(target)
                    .map_err(|_| EvalError::DanglingExpr)?;
                self.eval_integral_at(constant.expr, depth + 1)
            }
            ConstExpr::Unary { op, operand } => {
                let value = self.eval_integral_at(*operand, depth + 1)?;
                match op {
                    UnaryOp::Plus => Ok(value),
                    UnaryOp::Minus => value.checked_neg().ok_or(EvalError::Overflow { at }),
                    UnaryOp::Not => Ok(!value),
                }
            }
            ConstExpr::Binary { op, lhs, rhs } => {
                let left = self.eval_integral_at(*lhs, depth + 1)?;
                let right = self.eval_integral_at(*rhs, depth + 1)?;
                Self::apply_binary(*op, left, right, at)
            }
        }
    }

    fn apply_binary(
        op: crate::node::constant::BinaryOp,
        left: i128,
        right: i128,
        at: NodeId,
    ) -> Result<i128, EvalError> {
        use crate::node::constant::BinaryOp as B;
        match op {
            B::Or => Ok(left | right),
            B::Xor => Ok(left ^ right),
            B::And => Ok(left & right),
            B::Shl | B::Shr => {
                let distance = u32::try_from(right).map_err(|_| EvalError::InvalidShift { at })?;
                if distance >= 128 {
                    return Err(EvalError::InvalidShift { at });
                }
                if matches!(op, B::Shl) {
                    left.checked_shl(distance).ok_or(EvalError::Overflow { at })
                } else {
                    left.checked_shr(distance).ok_or(EvalError::Overflow { at })
                }
            }
            B::Add => left.checked_add(right).ok_or(EvalError::Overflow { at }),
            B::Sub => left.checked_sub(right).ok_or(EvalError::Overflow { at }),
            B::Mul => left.checked_mul(right).ok_or(EvalError::Overflow { at }),
            B::Div => {
                if right == 0 {
                    Err(EvalError::DivisionByZero { at })
                } else {
                    left.checked_div(right).ok_or(EvalError::Overflow { at })
                }
            }
            B::Mod => {
                if right == 0 {
                    Err(EvalError::DivisionByZero { at })
                } else {
                    left.checked_rem(right).ok_or(EvalError::Overflow { at })
                }
            }
        }
    }

    // -- interfaces ------------------------------------------------------

    /// Borrow an interface.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn interface(&self, id: Id<InterfaceNode>) -> Result<&InterfaceNode, AstError> {
        Ok(self.interfaces.get(id)?)
    }

    /// Declare an interface in a module.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale module handle.
    pub fn add_interface(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        kind: InterfaceKind,
        bases: Vec<Ref<InterfaceNode>>,
    ) -> Result<Id<InterfaceNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.interfaces.insert(InterfaceNode {
            node_id,
            name: name.to_string(),
            module,
            kind,
            bases,
            operations: Vec::new(),
            attributes: Vec::new(),
            completeness: Completeness::Complete,
            facets: Facets::synthetic("interface created through the model API"),
        })?;
        let any = AnyId::Interface(id, node_id);
        self.register(any);
        self.modules.get_mut(module)?.children.push(any);
        self.committed(id)
    }

    /// Declare an interface whose body has not arrived.
    ///
    /// `interface I;` — IDL 4.2 §7.4.3, rule (75). The same shape
    /// [`Self::add_forward_type`] gives a struct or a union, and for the
    /// same reason: forwardness is a state of one node, so a reference
    /// taken against the declaration stays valid when the definition
    /// completes it.
    ///
    /// # Errors
    /// As [`Self::add_interface`].
    pub fn add_forward_interface(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        kind: InterfaceKind,
    ) -> Result<Id<InterfaceNode>, AstError> {
        let id = self.add_interface(module, name, kind, Vec::new())?;
        self.interfaces.get_mut(id)?.completeness = Completeness::Forward;
        self.committed(id)
    }

    /// Give a forward-declared interface its body.
    ///
    /// An update of the existing node, not a second one — the reason
    /// [`Self::complete_type`] gives.
    ///
    /// # Errors
    /// [`AstError::AlreadyComplete`] if the interface already has a body:
    /// two definitions of one name are a duplicate, not a completion.
    /// [`AstError::Core`] on a stale handle.
    pub fn complete_interface(
        &mut self,
        id: Id<InterfaceNode>,
        kind: InterfaceKind,
        bases: Vec<Ref<InterfaceNode>>,
    ) -> Result<(), AstError> {
        let node = self.interfaces.get(id)?;
        let node_id = node.node_id;
        if node.completeness == Completeness::Complete {
            return Err(AstError::AlreadyComplete { node: node_id });
        }
        // The bases the declaration is about to gain are pending
        // references; the waiter index has to see them, and the empty
        // list it holds now has to go.
        self.unindex_pending(AnyId::Interface(id, node_id));

        let node = self.interfaces.get_mut(id)?;
        node.kind = kind;
        node.bases = bases;
        node.completeness = Completeness::Complete;

        self.index_pending(AnyId::Interface(id, node_id));
        self.record_completeness_change(node_id);
        Ok(())
    }

    /// Iterate every interface in the model, in arena order.
    pub fn interfaces(&self) -> impl Iterator<Item = (Id<InterfaceNode>, &InterfaceNode)> {
        self.interfaces.iter()
    }

    /// Iterate every operation in the model, in arena order.
    ///
    /// An operation's owner is [`OperationNode::parent`], which is an
    /// [`AnyId`] because a valuetype declares operations too. There is
    /// no list on the valuetype for the exported ones — only for its
    /// initialisers, which is what rule (105) distinguishes — so this
    /// is how a consumer finds them.
    pub fn operations(&self) -> impl Iterator<Item = (Id<OperationNode>, &OperationNode)> {
        self.operations.iter()
    }

    /// Rewrite an interface's form and base list.
    ///
    /// Distinct from [`Self::complete_interface`], which gives a forward
    /// declaration the body it was still waiting for. This restates a
    /// declaration that already has one: CCM 4.0 §6.2's equivalent
    /// interface is the component declaration with a different form and
    /// a different base list, and the identity has to survive the change
    /// so that references taken against the component keep pointing at
    /// it.
    ///
    /// The waiter index is rebuilt, because the old bases' pending keys
    /// have to go and the new ones have to be seen.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn set_interface_form(
        &mut self,
        id: Id<InterfaceNode>,
        kind: InterfaceKind,
        bases: Vec<Ref<InterfaceNode>>,
    ) -> Result<(), AstError> {
        let node_id = self.interfaces.get(id)?.node_id;
        self.unindex_pending(AnyId::Interface(id, node_id));
        let node = self.interfaces.get_mut(id)?;
        node.kind = kind;
        node.bases = bases;
        self.index_pending(AnyId::Interface(id, node_id));
        Ok(())
    }

    /// Add a base to a valuetype.
    ///
    /// A method rather than mutable access to the list, for the reason
    /// [`Self::add_raises`] gives: a reference added behind the model's
    /// back is never indexed, and an unindexed pending reference is one
    /// that never resolves.
    ///
    /// # Errors
    /// [`AstError::NotAnAggregate`] when the type is not a valuetype,
    /// [`AstError::Core`] on a stale handle.
    pub fn add_valuetype_base(
        &mut self,
        id: Id<TypeNode>,
        base: Ref<TypeNode>,
    ) -> Result<(), AstError> {
        let node_id = self.types.get(id)?.node_id;
        if !matches!(self.types.get(id)?.kind, TypeKind::Valuetype { .. }) {
            return Err(AstError::NotAnAggregate {
                node: node_id,
                found: self.types.get(id)?.kind.as_str(),
            });
        }
        if let TypeKind::Valuetype { bases, .. } = &mut self.types.get_mut(id)?.kind {
            bases.push(base);
        }
        self.reindex_pending(AnyId::Type(id, node_id));
        Ok(())
    }

    /// The type that is an object reference to `interface`.
    ///
    /// An interface name written where a type is expected is an object
    /// reference — OMG IDL 4.2 §7.4.6 — and `struct S { I x; }` is the
    /// ordinary way to write one. The reference in `S::x` is a
    /// `Ref<TypeNode>` and an interface is not a type, so the model needs
    /// a type node standing for "a reference to `I`"; this returns it,
    /// creating it the first time and returning the same one after.
    ///
    /// It carries the interface's own name so a backend renders `I`, and
    /// it is deliberately left out of the name index: the name in the
    /// scope belongs to the interface, and indexing both would make every
    /// use of it ambiguous.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale interface handle.
    pub fn object_reference_to(
        &mut self,
        interface: Id<InterfaceNode>,
    ) -> Result<Id<TypeNode>, AstError> {
        let existing = self.types.iter().find_map(|(id, node)| {
            matches!(
                &node.kind,
                TypeKind::ObjectRef {
                    interface: Some(Ref::Resolved(target)),
                } if *target == interface
            )
            .then_some(id)
        });
        if let Some(id) = existing {
            return Ok(id);
        }
        let node = self.interfaces.get(interface)?;
        let (module, name) = (node.module, node.name.clone());
        self.add_type(
            module,
            &name,
            TypeKind::ObjectRef {
                interface: Some(Ref::Resolved(interface)),
            },
        )
    }

    /// Borrow an operation.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn operation(&self, id: Id<OperationNode>) -> Result<&OperationNode, AstError> {
        Ok(self.operations.get(id)?)
    }

    /// Add an operation to an interface.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_operation(
        &mut self,
        interface: Id<InterfaceNode>,
        name: &str,
        return_type: Option<Ref<TypeNode>>,
        oneway: bool,
    ) -> Result<Id<OperationNode>, AstError> {
        let parent_node_id = self.interfaces.get(interface)?.node_id;
        let node_id = self.next_model_id()?;
        let id = self.operations.insert(OperationNode {
            node_id,
            name: name.to_string(),
            return_type,
            params: Vec::new(),
            raises: Vec::new(),
            oneway,
            contexts: Vec::new(),
            parent: AnyId::Interface(interface, parent_node_id),
            facets: Facets::synthetic("operation created through the model API"),
        })?;
        self.register(AnyId::Operation(id, node_id));
        self.interfaces.get_mut(interface)?.operations.push(id);
        self.committed(id)
    }

    /// Add an initialiser or an exported operation to a valuetype.
    ///
    /// OMG IDL 4.2 rule (105): a `valuetype` body holds `factory`
    /// initialisers and ordinary operations alongside its state.
    /// [`TypeKind::Valuetype::initializers`] was in the model from the
    /// start and nothing could fill it — [`Self::add_operation`] takes
    /// an [`Id<InterfaceNode>`], so every `factory` in every IDL file
    /// this tree read was parsed, kept in the CST and dropped at the
    /// model boundary with no diagnostic.
    ///
    /// `initializer` decides which list the operation joins: an
    /// initialiser goes into `initializers`, an exported operation is
    /// reachable through its `parent` and is not one.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle, and
    /// [`AstError::Core`] when the type is not a valuetype.
    pub fn add_valuetype_operation(
        &mut self,
        valuetype: Id<TypeNode>,
        name: &str,
        return_type: Option<Ref<TypeNode>>,
        initializer: bool,
    ) -> Result<Id<OperationNode>, AstError> {
        let parent_node_id = self.types.get(valuetype)?.node_id;
        if !matches!(self.types.get(valuetype)?.kind, TypeKind::Valuetype { .. }) {
            return Err(AstError::NotAnAggregate {
                node: parent_node_id,
                found: "not a valuetype",
            });
        }
        let node_id = self.next_model_id()?;
        let id = self.operations.insert(OperationNode {
            node_id,
            name: name.to_string(),
            return_type,
            params: Vec::new(),
            raises: Vec::new(),
            oneway: false,
            contexts: Vec::new(),
            parent: AnyId::Type(valuetype, parent_node_id),
            facets: Facets::synthetic("valuetype operation created through the model API"),
        })?;
        self.register(AnyId::Operation(id, node_id));
        if initializer
            && let TypeKind::Valuetype { initializers, .. } =
                &mut self.types.get_mut(valuetype)?.kind
        {
            initializers.push(id);
        }
        self.committed(id)
    }

    /// Mark an operation `oneway`.
    ///
    /// `@oneway` writes the same flag as the keyword form; both reach
    /// the operation through here.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn set_oneway(&mut self, id: Id<OperationNode>, oneway: bool) -> Result<(), AstError> {
        self.operations.get_mut(id)?.oneway = oneway;
        Ok(())
    }

    /// Add an exception to an operation's `raises` clause.
    ///
    /// A method rather than mutable access to the list: a reference
    /// added behind the model's back is never indexed, and an
    /// unindexed pending reference is one that never resolves.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn add_raises(
        &mut self,
        id: Id<OperationNode>,
        exception: Ref<ExceptionNode>,
    ) -> Result<(), AstError> {
        let node_id = self.operations.get(id)?.node_id;
        self.operations.get_mut(id)?.raises.push(exception);
        self.reindex_pending(AnyId::Operation(id, node_id));
        Ok(())
    }

    /// Add a `context` identifier to an operation.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn add_context(&mut self, id: Id<OperationNode>, context: &str) -> Result<(), AstError> {
        self.operations
            .get_mut(id)?
            .contexts
            .push(context.to_string());
        Ok(())
    }

    /// Borrow a parameter.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn param(&self, id: Id<ParamNode>) -> Result<&ParamNode, AstError> {
        Ok(self.params.get(id)?)
    }

    /// Add a parameter to an operation.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_param(
        &mut self,
        operation: Id<OperationNode>,
        name: &str,
        direction: ParamDirection,
        type_: Ref<TypeNode>,
    ) -> Result<Id<ParamNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.params.insert(ParamNode {
            node_id,
            name: name.to_string(),
            direction,
            type_,
            parent: operation,
            facets: Facets::synthetic("parameter created through the model API"),
        })?;
        self.register(AnyId::Param(id, node_id));
        self.operations.get_mut(operation)?.params.push(id);
        self.committed(id)
    }

    /// Borrow an attribute.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn attribute(&self, id: Id<AttributeNode>) -> Result<&AttributeNode, AstError> {
        Ok(self.attributes.get(id)?)
    }

    /// Add an exception to an attribute's `getraises` or `setraises`
    /// clause.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn add_attribute_raises(
        &mut self,
        id: Id<AttributeNode>,
        exception: Ref<ExceptionNode>,
        on_set: bool,
    ) -> Result<(), AstError> {
        let node_id = self.attributes.get(id)?.node_id;
        let node = self.attributes.get_mut(id)?;
        if on_set {
            node.set_raises.push(exception);
        } else {
            node.get_raises.push(exception);
        }
        self.reindex_pending(AnyId::Attribute(id, node_id));
        Ok(())
    }

    /// Add an attribute to an interface.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_attribute(
        &mut self,
        interface: Id<InterfaceNode>,
        name: &str,
        type_: Ref<TypeNode>,
        readonly: bool,
    ) -> Result<Id<AttributeNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.attributes.insert(AttributeNode {
            node_id,
            name: name.to_string(),
            type_,
            readonly,
            get_raises: Vec::new(),
            set_raises: Vec::new(),
            parent: interface,
            facets: Facets::synthetic("attribute created through the model API"),
        })?;
        self.register(AnyId::Attribute(id, node_id));
        self.interfaces.get_mut(interface)?.attributes.push(id);
        self.committed(id)
    }

    /// Borrow an exception.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn exception(&self, id: Id<ExceptionNode>) -> Result<&ExceptionNode, AstError> {
        Ok(self.exceptions.get(id)?)
    }

    /// Iterate every exceptions node in the model, in arena order.
    pub fn exceptions(&self) -> impl Iterator<Item = (Id<ExceptionNode>, &ExceptionNode)> {
        self.exceptions.iter()
    }

    /// Record that a type, constant or exception is declared inside the
    /// interface or value type `scope` rather than directly in its module.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle; [`AstError::WrongKind`] for a
    /// node of another kind, which no interface body declares.
    pub fn set_declaration_scope(&mut self, any: AnyId, scope: &str) -> Result<(), AstError> {
        let slot = match any {
            AnyId::Type(id, _) => &mut self.types.get_mut(id)?.scope,
            AnyId::Const(id, _) => &mut self.constants.get_mut(id)?.scope,
            AnyId::Exception(id, _) => &mut self.exceptions.get_mut(id)?.scope,
            other => {
                return Err(AstError::WrongKind {
                    node: other.node_id(),
                    expected: crate::NodeKind::Type,
                    found: other.kind(),
                });
            }
        };
        *slot = Some(scope.to_string());
        // The name the node was indexed under when it was declared did
        // not have the scope in it yet.
        self.by_scoped_name.remove_node(any.node_id());
        if let Some(name) = self.scoped_name_of(any) {
            self.by_scoped_name.insert(name, any.node_id());
        }
        Ok(())
    }

    /// Declare an exception in a module.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale module handle.
    pub fn add_exception(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
    ) -> Result<Id<ExceptionNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.exceptions.insert(ExceptionNode {
            node_id,
            name: name.to_string(),
            module,
            scope: None,
            members: Vec::new(),
            facets: Facets::synthetic("exception created through the model API"),
        })?;
        let any = AnyId::Exception(id, node_id);
        self.register(any);
        self.modules.get_mut(module)?.children.push(any);
        self.committed(id)
    }

    // -- annotations -----------------------------------------------------

    /// Every annotation declaration in the model, in arena order.
    ///
    /// The counterpart of [`Ast::types`] for the other declaration kind
    /// that has a `TypeObject`: Annex B gives `MinimalTypeObject` a
    /// `case TK_ANNOTATION: MinimalAnnotationType annotation_type;`, and
    /// an `@annotation` declaration is not a `TypeNode` in this model,
    /// so a caller that wants to build one had no way to reach it.
    pub fn annotation_defs(
        &self,
    ) -> impl Iterator<Item = (Id<AnnotationDefNode>, &AnnotationDefNode)> {
        self.annotation_defs.iter()
    }

    /// Borrow an annotation declaration.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn annotation_def(
        &self,
        id: Id<AnnotationDefNode>,
    ) -> Result<&AnnotationDefNode, AstError> {
        Ok(self.annotation_defs.get(id)?)
    }

    /// Declare an annotation in a module.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale module handle.
    pub fn add_annotation_def(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
    ) -> Result<Id<AnnotationDefNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.annotation_defs.insert(AnnotationDefNode {
            node_id,
            name: name.to_string(),
            module,
            members: Vec::new(),
            facets: Facets::synthetic("annotation created through the model API"),
        })?;
        let any = AnyId::AnnotationDef(id, node_id);
        self.register(any);
        self.modules.get_mut(module)?.children.push(any);
        self.committed(id)
    }

    /// Borrow an annotation member.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn annotation_member(
        &self,
        id: Id<AnnotationMemberNode>,
    ) -> Result<&AnnotationMemberNode, AstError> {
        Ok(self.annotation_members.get(id)?)
    }

    /// Add a member to an annotation declaration.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_annotation_member(
        &mut self,
        annotation: Id<AnnotationDefNode>,
        name: &str,
        type_: Ref<TypeNode>,
        default: Option<Id<ConstExprNode>>,
    ) -> Result<Id<AnnotationMemberNode>, AstError> {
        let node_id = self.next_model_id()?;
        let is_value_member = name == "value";
        let id = self.annotation_members.insert(AnnotationMemberNode {
            node_id,
            name: name.to_string(),
            type_,
            default,
            is_value_member,
            parent: annotation,
            facets: Facets::synthetic("annotation member created through the model API"),
        })?;
        self.register(AnyId::AnnotationMember(id, node_id));
        self.annotation_defs.get_mut(annotation)?.members.push(id);
        self.committed(id)
    }

    // -- deployment ------------------------------------------------------

    /// Borrow a library.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn library(&self, id: Id<LibraryNode>) -> Result<&LibraryNode, AstError> {
        Ok(self.libraries.get(id)?)
    }

    /// Declare a deployment library.
    ///
    /// # Errors
    /// [`AstError::Core`] when identity space is exhausted.
    pub fn add_library(&mut self, name: &str) -> Result<Id<LibraryNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.libraries.insert(LibraryNode {
            node_id,
            name: name.to_string(),
            children: Vec::new(),
            facets: Facets::synthetic("library created through the model API"),
        })?;
        self.register(AnyId::Library(id, node_id));
        self.committed(id)
    }

    /// Borrow a domain.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn domain(&self, id: Id<DomainNode>) -> Result<&DomainNode, AstError> {
        Ok(self.domains.get(id)?)
    }

    /// Declare a domain in a library.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_domain(
        &mut self,
        library: Id<LibraryNode>,
        name: &str,
        domain_id: u32,
    ) -> Result<Id<DomainNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.domains.insert(DomainNode {
            node_id,
            name: name.to_string(),
            domain_id,
            topics: Vec::new(),
            parent: library,
            facets: Facets::synthetic("domain created through the model API"),
        })?;
        let any = AnyId::Domain(id, node_id);
        self.register(any);
        self.libraries.get_mut(library)?.children.push(any);
        self.committed(id)
    }

    /// Borrow a topic.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn topic(&self, id: Id<TopicNode>) -> Result<&TopicNode, AstError> {
        Ok(self.topics.get(id)?)
    }

    /// Declare a topic in a domain.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_topic(
        &mut self,
        domain: Id<DomainNode>,
        name: &str,
        type_: Ref<TypeNode>,
        qos: Option<Ref<QosProfileNode>>,
    ) -> Result<Id<TopicNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.topics.insert(TopicNode {
            node_id,
            name: name.to_string(),
            type_,
            qos,
            parent: domain,
            facets: Facets::synthetic("topic created through the model API"),
        })?;
        self.register(AnyId::Topic(id, node_id));
        self.domains.get_mut(domain)?.topics.push(id);
        self.committed(id)
    }

    /// Borrow a QoS profile.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn qos_profile(&self, id: Id<QosProfileNode>) -> Result<&QosProfileNode, AstError> {
        Ok(self.qos_profiles.get(id)?)
    }

    /// Declare a QoS profile in a library.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_qos_profile(
        &mut self,
        library: Id<LibraryNode>,
        name: &str,
        base: Option<Ref<QosProfileNode>>,
        policies: Vec<QosPolicy>,
    ) -> Result<Id<QosProfileNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.qos_profiles.insert(QosProfileNode {
            node_id,
            name: name.to_string(),
            base,
            policies,
            parent: library,
            facets: Facets::synthetic("QoS profile created through the model API"),
        })?;
        let any = AnyId::QosProfile(id, node_id);
        self.register(any);
        self.libraries.get_mut(library)?.children.push(any);
        self.committed(id)
    }

    /// Borrow a participant.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn participant(&self, id: Id<ParticipantNode>) -> Result<&ParticipantNode, AstError> {
        Ok(self.participants.get(id)?)
    }

    /// Declare a participant in a library.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_participant(
        &mut self,
        library: Id<LibraryNode>,
        name: &str,
        domain: Ref<DomainNode>,
        qos: Option<Ref<QosProfileNode>>,
    ) -> Result<Id<ParticipantNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.participants.insert(ParticipantNode {
            node_id,
            name: name.to_string(),
            domain,
            qos,
            endpoints: Vec::new(),
            parent: library,
            facets: Facets::synthetic("participant created through the model API"),
        })?;
        let any = AnyId::Participant(id, node_id);
        self.register(any);
        self.libraries.get_mut(library)?.children.push(any);
        self.committed(id)
    }

    /// Borrow an endpoint.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale or unknown handle.
    pub fn endpoint(&self, id: Id<EndpointNode>) -> Result<&EndpointNode, AstError> {
        Ok(self.endpoints.get(id)?)
    }

    /// Declare an endpoint on a participant.
    ///
    /// # Errors
    /// [`AstError::Core`] on a stale handle.
    pub fn add_endpoint(
        &mut self,
        participant: Id<ParticipantNode>,
        name: &str,
        role: EndpointRole,
        topic: Ref<TopicNode>,
        qos: Option<Ref<QosProfileNode>>,
    ) -> Result<Id<EndpointNode>, AstError> {
        let node_id = self.next_model_id()?;
        let id = self.endpoints.insert(EndpointNode {
            node_id,
            name: name.to_string(),
            role,
            topic,
            qos,
            parent: participant,
            facets: Facets::synthetic("endpoint created through the model API"),
        })?;
        self.register(AnyId::Endpoint(id, node_id));
        self.participants.get_mut(participant)?.endpoints.push(id);
        self.committed(id)
    }

    // -- facet access ----------------------------------------------------

    /// The facets of any node.
    #[must_use]
    pub fn facets(&self, any: AnyId) -> Option<&Facets> {
        Some(match any {
            AnyId::Module(id, _) => &self.modules.get(id).ok()?.facets,
            AnyId::Type(id, _) => &self.types.get(id).ok()?.facets,
            AnyId::Member(id, _) => &self.members.get(id).ok()?.facets,
            AnyId::Case(id, _) => &self.cases.get(id).ok()?.facets,
            AnyId::Enumerator(id, _) => &self.enumerators.get(id).ok()?.facets,
            AnyId::BitValue(id, _) => &self.bit_values.get(id).ok()?.facets,
            AnyId::Bitfield(id, _) => &self.bitfields.get(id).ok()?.facets,
            AnyId::Const(id, _) => &self.constants.get(id).ok()?.facets,
            AnyId::ConstExpr(id, _) => &self.exprs.get(id).ok()?.facets,
            AnyId::Interface(id, _) => &self.interfaces.get(id).ok()?.facets,
            AnyId::Operation(id, _) => &self.operations.get(id).ok()?.facets,
            AnyId::Param(id, _) => &self.params.get(id).ok()?.facets,
            AnyId::Attribute(id, _) => &self.attributes.get(id).ok()?.facets,
            AnyId::Exception(id, _) => &self.exceptions.get(id).ok()?.facets,
            AnyId::AnnotationDef(id, _) => &self.annotation_defs.get(id).ok()?.facets,
            AnyId::AnnotationMember(id, _) => &self.annotation_members.get(id).ok()?.facets,
            AnyId::Domain(id, _) => &self.domains.get(id).ok()?.facets,
            AnyId::Topic(id, _) => &self.topics.get(id).ok()?.facets,
            AnyId::QosProfile(id, _) => &self.qos_profiles.get(id).ok()?.facets,
            AnyId::Participant(id, _) => &self.participants.get(id).ok()?.facets,
            AnyId::Endpoint(id, _) => &self.endpoints.get(id).ok()?.facets,
            AnyId::Library(id, _) => &self.libraries.get(id).ok()?.facets,
        })
    }

    /// The facets of any node, mutably.
    pub fn facets_mut(&mut self, any: AnyId) -> Option<&mut Facets> {
        Some(match any {
            AnyId::Module(id, _) => &mut self.modules.get_mut(id).ok()?.facets,
            AnyId::Type(id, _) => &mut self.types.get_mut(id).ok()?.facets,
            AnyId::Member(id, _) => &mut self.members.get_mut(id).ok()?.facets,
            AnyId::Case(id, _) => &mut self.cases.get_mut(id).ok()?.facets,
            AnyId::Enumerator(id, _) => &mut self.enumerators.get_mut(id).ok()?.facets,
            AnyId::BitValue(id, _) => &mut self.bit_values.get_mut(id).ok()?.facets,
            AnyId::Bitfield(id, _) => &mut self.bitfields.get_mut(id).ok()?.facets,
            AnyId::Const(id, _) => &mut self.constants.get_mut(id).ok()?.facets,
            AnyId::ConstExpr(id, _) => &mut self.exprs.get_mut(id).ok()?.facets,
            AnyId::Interface(id, _) => &mut self.interfaces.get_mut(id).ok()?.facets,
            AnyId::Operation(id, _) => &mut self.operations.get_mut(id).ok()?.facets,
            AnyId::Param(id, _) => &mut self.params.get_mut(id).ok()?.facets,
            AnyId::Attribute(id, _) => &mut self.attributes.get_mut(id).ok()?.facets,
            AnyId::Exception(id, _) => &mut self.exceptions.get_mut(id).ok()?.facets,
            AnyId::AnnotationDef(id, _) => &mut self.annotation_defs.get_mut(id).ok()?.facets,
            AnyId::AnnotationMember(id, _) => &mut self.annotation_members.get_mut(id).ok()?.facets,
            AnyId::Domain(id, _) => &mut self.domains.get_mut(id).ok()?.facets,
            AnyId::Topic(id, _) => &mut self.topics.get_mut(id).ok()?.facets,
            AnyId::QosProfile(id, _) => &mut self.qos_profiles.get_mut(id).ok()?.facets,
            AnyId::Participant(id, _) => &mut self.participants.get_mut(id).ok()?.facets,
            AnyId::Endpoint(id, _) => &mut self.endpoints.get_mut(id).ok()?.facets,
            AnyId::Library(id, _) => &mut self.libraries.get_mut(id).ok()?.facets,
        })
    }

    /// The `AnyId` for a type handle.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale handle.
    pub fn any_of_type(&self, id: Id<TypeNode>) -> Result<AnyId, AstError> {
        Ok(AnyId::Type(id, self.types.get(id)?.node_id))
    }

    /// The `AnyId` for a member handle.
    ///
    /// # Errors
    /// [`AstError::Core`] with a stale handle.
    pub fn any_of_member(&self, id: Id<MemberNode>) -> Result<AnyId, AstError> {
        Ok(AnyId::Member(id, self.members.get(id)?.node_id))
    }
}

/// Convenience: the primitive kinds a frontend most often needs, as a
/// helper for building small models in tests and tools.
#[must_use]
pub fn primitive(kind: PrimitiveKind) -> TypeKind {
    TypeKind::Primitive(kind)
}
