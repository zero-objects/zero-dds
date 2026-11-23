// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lowering of a CIDL file: the PSDL declarations and the compositions.
//! CORBA 3.3 Part 3 §8; Persistent State Service 2.0 §3.2.
//!
//! A PSDL declaration becomes a local interface of the model, so that it
//! takes part in the scopes like any IDL declaration: an abstract storage
//! type's state members are its attributes, its local operations its
//! operations, its bases its bases. What PSDL adds — the kind, `of`,
//! `implements`, keys, factories, store directives — is kept beside the
//! model in a [`CidlModel`]. A composition is no IDL declaration at all and
//! lives there only.
//!
//! The rules of §8 that read names are [`Composition::validate`]'s; this
//! module supplies it with what the model knows of the home, the
//! component and the abstract storage home, and adds the rules that read
//! types: Table 8.6 (§8.14) and the signature compatibility of §8.15.

use crate::lower::{Lowering, first_ident};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use zerodds_ast::cidl::{
    CidlModel, Composition, CompositionCategory, Delegation, ExecutorDef, FeatureDelegation,
    HomeExecutorDef, HomeFacts, KeyDecl, LocalOperation, ProxyHomeDef, SegmentDef, StateMember,
    StorageDecl, StorageHomeFacts, StorageKind, StoreDirective, Subject,
};
use zerodds_ast::{
    AnyId, ComponentRole, HomeOperation, InterfaceKind, InterfaceNode, OperationNode,
    ParamDirection, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};
use zerodds_idl_cst::{CstKind, Span, SyntaxNode, TokenKind};
use zerodds_idl_parse::DiagnosticCode;

/// The clause the PSDL rules cite.
const PSS: &str = "Persistent State Service 2.0 §3.2.5, §3.2.6";
/// The clause of a composition body.
const COMPOSITION_BODY: &str = "CORBA 3.3 Part 3 §8.5 (6)";

/// What the walk collects of a CIDL file, resolved once the file is done.
#[derive(Default)]
pub(crate) struct CidlState {
    storage: Vec<StorageEntry>,
    compositions: Vec<CompositionEntry>,
}

/// A name written in a composition, looked up after the walk.
struct NameUse {
    /// The scoped names it may denote, innermost scope first (§7.5).
    candidates: Vec<ScopedName>,
    /// As written.
    written: String,
    span: Span,
}

/// A PSDL definition: its interface in the model and what PSDL adds.
struct StorageEntry {
    interface: Id<InterfaceNode>,
    decl: StorageDecl,
    span: Span,
    of: Option<(Ref<InterfaceNode>, Span)>,
    bases: Vec<(Ref<InterfaceNode>, Span)>,
    implements: Vec<(Ref<InterfaceNode>, Span)>,
    key_spans: Vec<Span>,
    factory_spans: Vec<Span>,
    primary_key_span: Option<Span>,
    ref_rep_span: Option<Span>,
    stores: Vec<StoreEntry>,
    /// `ref<X>` state types: the attribute, X and where it is written.
    state_refs: Vec<(Ref<InterfaceNode>, Span)>,
}

impl StorageEntry {
    /// The module path the declaration stands in.
    fn module(&self) -> Vec<String> {
        let mut path: Vec<String> = self.decl.name.split("::").map(String::from).collect();
        path.pop();
        path
    }
}

struct StoreEntry {
    span: Span,
    as_: Option<Ref<InterfaceNode>>,
    scope: Option<Ref<InterfaceNode>>,
}

/// A composition and where each of its parts is written.
struct CompositionEntry {
    model: Composition,
    span: Span,
    name_span: Span,
    implements: Option<NameUse>,
    binds_to: Option<NameUse>,
    stored_on: Option<NameUse>,
    segment_stored_on: Vec<Option<NameUse>>,
    segment_spans: Vec<Span>,
    segment_facet_spans: Vec<Vec<Span>>,
    feature_spans: Vec<Span>,
    storage_delegation_spans: Vec<Span>,
    executor_delegation_spans: Vec<Span>,
    abstract_spans: Vec<Span>,
    proxy_delegation_spans: Vec<Span>,
    proxy_abstract_spans: Vec<Span>,
}

/// A type as the compatibility rules of §8.14 and §8.15 see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    /// No value: `void`.
    Void,
    /// A reference to an interface, a component or a PSDL type.
    Object(Id<InterfaceNode>),
    /// Any other type.
    Type(Id<TypeNode>),
    /// `ref<S>`, a reference to a storage object of the abstract storage
    /// type S (PSS 2.0 §3.2.5.7); no IDL type is one.
    StorageRef(Id<InterfaceNode>),
    /// A type the model could not resolve; compatible with nothing and
    /// reported elsewhere.
    Unknown,
}

/// An operation's signature for §8.15.
struct Signature {
    factory: bool,
    ret: Ty,
    params: Vec<(ParamDirection, Ty)>,
    raises: Vec<String>,
}

impl Lowering<'_> {
    // -- PSDL ------------------------------------------------------------

    /// An abstract storage type, abstract storage home, storage type or
    /// storage home: PSS 2.0 §3.2.5 and §3.2.6.
    pub(crate) fn psdl_declaration(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let kind = match node.kind() {
            CstKind::AbstractStorageTypeDecl => StorageKind::AbstractStorageType,
            CstKind::AbstractStorageHomeDecl => StorageKind::AbstractStorageHome,
            CstKind::StorageTypeDecl => StorageKind::StorageType,
            _ => StorageKind::StorageHome,
        };
        let has_body = node
            .tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::LBrace);
        if !has_body {
            self.psdl_forward(node, &name);
            return;
        }
        let (bases, _) = self.interface_bases(node);
        let Some(id) = self.declare_interface(&name, InterfaceKind::Local, bases.clone(), node)
        else {
            return;
        };
        let Ok(node_id) = self.ast.interface(id).map(|i| i.node_id) else {
            return;
        };
        self.record_origin(AnyId::Interface(id, node_id), node.span());
        self.scopes.bind(&name, AnyId::Interface(id, node_id));
        let qualified = self
            .ast
            .scoped_name_of(AnyId::Interface(id, node_id))
            .map_or_else(|| name.clone(), |n| n.segments().join("::"));

        let mut entry = StorageEntry {
            interface: id,
            decl: StorageDecl {
                name: qualified,
                kind,
                of: None,
                bases: Vec::new(),
                implements: Vec::new(),
                state_members: Vec::new(),
                operations: Vec::new(),
                keys: Vec::new(),
                factories: Vec::new(),
                primary_key: None,
                ref_representation: Vec::new(),
                stores: Vec::new(),
            },
            span: node.span(),
            of: None,
            bases: Vec::new(),
            implements: Vec::new(),
            key_spans: Vec::new(),
            factory_spans: Vec::new(),
            primary_key_span: None,
            ref_rep_span: None,
            stores: Vec::new(),
            state_refs: Vec::new(),
        };
        for clause in node.children() {
            match clause.kind() {
                CstKind::InheritanceClause => {
                    for (scoped, written) in self.scoped_names_with_nodes(&clause) {
                        let reference = self.interface_ref(&scoped, &written);
                        entry.decl.bases.push(written.segments().join("::"));
                        entry.bases.push((reference, scoped.span()));
                    }
                }
                CstKind::OfClause => {
                    if let Some((scoped, written)) =
                        self.scoped_names_with_nodes(&clause).into_iter().next()
                    {
                        let reference = self.interface_ref(&scoped, &written);
                        entry.decl.of = Some(written.segments().join("::"));
                        entry.of = Some((reference, scoped.span()));
                    }
                }
                CstKind::ImplementsClause => {
                    for (scoped, written) in self.scoped_names_with_nodes(&clause) {
                        let reference = self.interface_ref(&scoped, &written);
                        entry.decl.implements.push(written.segments().join("::"));
                        entry.implements.push((reference, scoped.span()));
                    }
                }
                CstKind::RefRepDirective => {
                    entry.decl.ref_representation = declarator_names(&clause);
                    entry.ref_rep_span = Some(clause.span());
                }
                CstKind::PrimaryKeyDecl => {
                    let key = if clause
                        .tokens()
                        .iter()
                        .any(|(_, token)| token.kind() == TokenKind::Ref)
                    {
                        Some("ref".to_string())
                    } else {
                        first_ident(&clause)
                    };
                    entry.decl.primary_key = key;
                    entry.primary_key_span = Some(clause.span());
                }
                // The members, lowered below in the declaration's scope,
                // and what the parser reported in an error node.
                CstKind::StateDecl
                | CstKind::Operation
                | CstKind::KeyDecl
                | CstKind::PsdlFactoryDecl
                | CstKind::StoreDirective
                | CstKind::Error => {}
                _ => self.not_in_place(&clause, "a PSDL declaration", PSS),
            }
        }

        let base_paths = self.base_paths(node, false);
        self.one_namespace_per_scope(node);
        self.scopes.push_container(&name);
        self.scopes.set_bases(base_paths);
        for child in node.children() {
            match child.kind() {
                CstKind::StateDecl => self.psdl_state(&child, id, &mut entry),
                CstKind::Operation => {
                    if let Some(name) = first_ident(&child) {
                        // PSS 2.0 §3.2.5.4: "A const operation does not
                        // update any state member."
                        let is_const = child
                            .tokens()
                            .iter()
                            .any(|(_, token)| token.kind() == TokenKind::Const);
                        entry
                            .decl
                            .operations
                            .push(LocalOperation { name, is_const });
                    }
                    self.operation(&child, id);
                }
                CstKind::KeyDecl | CstKind::PsdlFactoryDecl => {
                    let key_name = first_ident(&child).unwrap_or_default();
                    let mut members = declarator_names(&child);
                    // §3.2.5.7: "`key` <identifier> is just a shortcut
                    // notation for `key` <identifier> `(`<identifier>`)`".
                    if child.kind() == CstKind::KeyDecl && members.is_empty() {
                        members.push(key_name.clone());
                    }
                    let key = KeyDecl {
                        name: key_name,
                        members,
                    };
                    if child.kind() == CstKind::KeyDecl {
                        entry.decl.keys.push(key);
                        entry.key_spans.push(child.span());
                    } else {
                        entry.decl.factories.push(key);
                        entry.factory_spans.push(child.span());
                    }
                }
                CstKind::StoreDirective => self.psdl_store(&child, &mut entry),
                // The clauses, lowered above, and what the parser reported
                // in an error node.
                CstKind::InheritanceClause
                | CstKind::OfClause
                | CstKind::ImplementsClause
                | CstKind::RefRepDirective
                | CstKind::PrimaryKeyDecl
                | CstKind::Error => {}
                _ => self.not_in_place(&child, "a PSDL declaration", PSS),
            }
        }
        self.scopes.pop();
        self.cidl.storage.push(entry);
    }

    /// `abstract storagetype X;`, `abstract storagehome X;`,
    /// `storagetype X;`: the name, completed by the definition that
    /// follows. PSS 2.0 §3.2.5.1, §3.2.5.6.
    fn psdl_forward(&mut self, node: &Arc<SyntaxNode>, name: &str) {
        let Some(module) = self.scopes.current_module() else {
            return;
        };
        if self.scopes.lookup_here(name).is_some() {
            return;
        }
        if node
            .children()
            .iter()
            .any(|child| child.kind() == CstKind::InheritanceClause)
        {
            self.diagnose_at(
                DiagnosticCode::CompositionRule,
                node.span(),
                &format!("the forward declaration of `{name}` names bases; its definition does"),
                "Persistent State Service 2.0 §3.2.3 (7), (17), (28)",
            );
        }
        if let Ok(id) = self
            .ast
            .add_forward_interface(module, name, InterfaceKind::Local)
            && let Ok(node_id) = self.ast.interface(id).map(|i| i.node_id)
        {
            self.record_origin(AnyId::Interface(id, node_id), node.span());
            self.scopes.bind(name, AnyId::Interface(id, node_id));
        }
    }

    /// `[readonly] state <type> a, b;` — PSS 2.0 §3.2.5.2. Each
    /// declarator is an attribute of the storage type's interface.
    fn psdl_state(
        &mut self,
        node: &Arc<SyntaxNode>,
        interface: Id<InterfaceNode>,
        entry: &mut StorageEntry,
    ) {
        let readonly = node
            .tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::Readonly);
        let reference = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::StorageRefType);
        let (type_, written) = match &reference {
            Some(reference) => {
                let target = self.scoped_names_with_nodes(reference).into_iter().next();
                match target {
                    Some((scoped, name)) => {
                        let as_interface = self.interface_ref(&scoped, &name);
                        entry.state_refs.push((as_interface, scoped.span()));
                        (self.named_type_ref(&scoped, &name), syntax_text(reference))
                    }
                    None => (
                        Ref::Broken(crate::lower::broken_cause()),
                        syntax_text(reference),
                    ),
                }
            }
            None => {
                let written = node
                    .children()
                    .into_iter()
                    .find(|child| is_type_node(child.kind()))
                    .map(|child| syntax_text(&child))
                    .unwrap_or_default();
                (
                    self.type_ref(node)
                        .unwrap_or_else(|| Ref::Broken(crate::lower::broken_cause())),
                    written,
                )
            }
        };
        for declarator in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Declarator)
        {
            let Some(name) = first_ident(&declarator) else {
                continue;
            };
            if let Ok(id) = self
                .ast
                .add_attribute(interface, &name, type_.clone(), readonly)
                && let Ok(node_id) = self.ast.attribute(id).map(|a| a.node_id)
            {
                self.record_origin(AnyId::Attribute(id, node_id), declarator.span());
            }
            entry.decl.state_members.push(StateMember {
                name,
                type_: written.clone(),
                readonly,
            });
        }
    }

    /// `stores m as T [scope H];` — PSS 2.0 §3.2.6.2.
    fn psdl_store(&mut self, node: &Arc<SyntaxNode>, entry: &mut StorageEntry) {
        let members = declarator_names(node);
        let reference = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::StorageRefType);
        let names = self.scoped_names_with_nodes(node);
        let mut names = names.into_iter();
        let (as_, as_written) = match &reference {
            Some(reference) => {
                let target = self.scoped_names_with_nodes(reference).into_iter().next();
                (
                    target.map(|(scoped, name)| self.interface_ref(&scoped, &name)),
                    syntax_text(reference),
                )
            }
            None => match names.next() {
                Some((scoped, name)) => (
                    Some(self.interface_ref(&scoped, &name)),
                    name.segments().join("::"),
                ),
                None => (None, String::new()),
            },
        };
        let scope_written = names.next();
        let scope = scope_written
            .as_ref()
            .map(|(scoped, name)| self.interface_ref(scoped, name));
        for member in members {
            entry.decl.stores.push(StoreDirective {
                member,
                as_: as_written.clone(),
                scope: scope_written
                    .as_ref()
                    .map(|(_, name)| name.segments().join("::")),
            });
            entry.stores.push(StoreEntry {
                span: node.span(),
                as_: as_.clone(),
                scope: scope.clone(),
            });
        }
    }

    // -- CIDL ------------------------------------------------------------

    /// `composition <category> X { … };` — CORBA 3.3 Part 3 §8.5. Kept
    /// with the names it uses; resolved and checked in
    /// [`Self::finish_cidl`], once every declaration of the file exists.
    pub(crate) fn composition(&mut self, node: &Arc<SyntaxNode>) {
        let category = node
            .tokens()
            .iter()
            .find_map(|(_, token)| match token.kind() {
                TokenKind::Entity => Some(CompositionCategory::Entity),
                TokenKind::Process => Some(CompositionCategory::Process),
                TokenKind::Service => Some(CompositionCategory::Service),
                TokenKind::Session => Some(CompositionCategory::Session),
                _ => None,
            });
        let (Some(category), Some((name, name_span))) = (category, first_ident_span(node)) else {
            return;
        };
        let qualified = {
            let mut path = self.scopes.path_here();
            path.push(name.clone());
            path.join("::")
        };
        let mut entry = CompositionEntry {
            model: Composition {
                name: qualified,
                category,
                home_executor: HomeExecutorDef::default(),
                proxy_home: None,
            },
            span: node.span(),
            name_span,
            implements: None,
            binds_to: None,
            stored_on: None,
            segment_stored_on: Vec::new(),
            segment_spans: Vec::new(),
            segment_facet_spans: Vec::new(),
            feature_spans: Vec::new(),
            storage_delegation_spans: Vec::new(),
            executor_delegation_spans: Vec::new(),
            abstract_spans: Vec::new(),
            proxy_delegation_spans: Vec::new(),
            proxy_abstract_spans: Vec::new(),
        };
        for child in node.children() {
            match child.kind() {
                CstKind::HomeExecutorDef => self.home_executor(&child, &mut entry),
                CstKind::ProxyHomeDef => self.proxy_home(&child, &mut entry),
                // What the parser reported in an error node.
                CstKind::Error => {}
                _ => self.not_in_place(&child, "a composition body", COMPOSITION_BODY),
            }
        }
        self.cidl.compositions.push(entry);
    }

    /// Production (8): each member in its place, once. The parser keeps a
    /// member out of its place, or a second one, in an error node; one
    /// that reaches the lowering anyway is reported here, never dropped.
    fn home_executor(&mut self, node: &Arc<SyntaxNode>, entry: &mut CompositionEntry) {
        const BODY: &str = "the home executor body";
        const CLAUSE: &str = "CORBA 3.3 Part 3 §8.6 (8)";
        let he = &mut entry.model.home_executor;
        he.name = first_ident(node).unwrap_or_default();
        let mut executor_seen = false;
        for child in node.children() {
            match child.kind() {
                CstKind::HomeImplDecl if entry.implements.is_none() => {
                    entry.implements = self.name_use(&child);
                }
                CstKind::StorageHomeBinding if entry.binds_to.is_none() => {
                    entry.binds_to = self.name_use(&child);
                }
                CstKind::PersistenceDecl if entry.stored_on.is_none() => {
                    entry.stored_on = self.name_use(&child);
                }
                CstKind::ExecutorDef if !executor_seen => {
                    executor_seen = true;
                    self.executor_def(&child, entry);
                }
                CstKind::DelegationSpec => {
                    let has = |kind: TokenKind| {
                        child.tokens().iter().any(|(_, token)| token.kind() == kind)
                    };
                    let (delegations, spans) = delegations(&child);
                    let he = &mut entry.model.home_executor;
                    if has(TokenKind::Storagehome) {
                        he.storage_home_delegations.extend(delegations);
                        entry.storage_delegation_spans.extend(spans);
                    } else if has(TokenKind::Executor) {
                        he.executor_delegations.extend(delegations);
                        entry.executor_delegation_spans.extend(spans);
                    } else {
                        // `delegatesTo home`: production (34), a proxy
                        // home's.
                        self.not_in_place(&child, BODY, CLAUSE);
                    }
                }
                CstKind::AbstractSpec => {
                    let (names, spans) = declarators_with_spans(&child);
                    entry.model.home_executor.abstract_operations.extend(names);
                    entry.abstract_spans.extend(spans);
                }
                // What the parser reported in an error node.
                CstKind::Error => {}
                // A second `implements`, `bindsTo`, `storedOn` or
                // `manages`, a feature delegation (production (15), the
                // executor body's), or anything else.
                _ => self.not_in_place(&child, BODY, CLAUSE),
            }
        }
        let he = &mut entry.model.home_executor;
        if let Some(use_) = &entry.implements {
            he.implements = use_.written.clone();
        }
        he.binds_to = entry.binds_to.as_ref().map(|u| u.written.clone());
        he.stored_on = entry.stored_on.as_ref().map(|u| u.written.clone());
    }

    /// Productions (13) to (15): the segments and feature delegations of
    /// an executor body.
    fn executor_def(&mut self, node: &Arc<SyntaxNode>, entry: &mut CompositionEntry) {
        let mut executor = ExecutorDef {
            name: first_ident(node).unwrap_or_default(),
            ..ExecutorDef::default()
        };
        for child in node.children() {
            match child.kind() {
                CstKind::SegmentDef => {
                    // §8.11: "There is no canonical mechanism for assigning
                    // segment identifier values (other than the component
                    // segment)". Declaration order, from one.
                    let id = i16::try_from(executor.segments.len() + 1).unwrap_or(i16::MAX);
                    let mut segment = SegmentDef {
                        name: first_ident(&child).unwrap_or_default(),
                        id,
                        stored_on: None,
                        facets: Vec::new(),
                    };
                    let mut stored_on = None;
                    let mut facet_spans = Vec::new();
                    for member in child.children() {
                        match member.kind() {
                            CstKind::PersistenceDecl if stored_on.is_none() => {
                                stored_on = self.name_use(&member);
                                segment.stored_on = stored_on.as_ref().map(|u| u.written.clone());
                            }
                            // §8.12: "the abstract storage home upon which
                            // the state of the segment will be stored" —
                            // one.
                            CstKind::PersistenceDecl => self.diagnose_at(
                                DiagnosticCode::CompositionRule,
                                member.span(),
                                &format!(
                                    "the segment `{}` names a second abstract storage home with \
                                     `storedOn`; its state is stored on one",
                                    segment.name
                                ),
                                "CORBA 3.3 Part 3 §8.12",
                            ),
                            CstKind::FacetDecl => {
                                let (names, spans) = declarators_with_spans(&member);
                                segment.facets.extend(names);
                                facet_spans.extend(spans);
                            }
                            // What the parser reported in an error node.
                            CstKind::Error => {}
                            _ => self.not_in_place(
                                &member,
                                "a segment",
                                "CORBA 3.3 Part 3 §8.11 (17)",
                            ),
                        }
                    }
                    executor.segments.push(segment);
                    entry.segment_stored_on.push(stored_on);
                    entry.segment_spans.push(child.span());
                    entry.segment_facet_spans.push(facet_spans);
                }
                CstKind::FeatureDelegationSpec => {
                    for delegation in child
                        .children()
                        .into_iter()
                        .filter(|d| d.kind() == CstKind::FeatureDelegation)
                    {
                        let idents = idents_of(&delegation);
                        if let [feature, member] = idents.as_slice() {
                            executor.feature_delegations.push(FeatureDelegation {
                                feature: feature.clone(),
                                storage_member: member.clone(),
                            });
                            entry.feature_spans.push(delegation.span());
                        }
                    }
                }
                // What the parser reported in an error node.
                CstKind::Error => {}
                // A storage home, executor or home delegation: productions
                // (8) and (33) place them, (15) does not.
                _ => self.not_in_place(&child, "the executor body", "CORBA 3.3 Part 3 §8.10 (15)"),
            }
        }
        entry.model.home_executor.executor = executor;
    }

    /// Productions (32) to (34).
    fn proxy_home(&mut self, node: &Arc<SyntaxNode>, entry: &mut CompositionEntry) {
        let mut proxy = ProxyHomeDef {
            name: first_ident(node).unwrap_or_default(),
            ..ProxyHomeDef::default()
        };
        for child in node.children() {
            match child.kind() {
                CstKind::DelegationSpec
                    if child
                        .tokens()
                        .iter()
                        .any(|(_, token)| token.kind() == TokenKind::Home) =>
                {
                    let (list, spans) = delegations(&child);
                    proxy.home_delegations.extend(list);
                    entry.proxy_delegation_spans.extend(spans);
                }
                CstKind::AbstractSpec => {
                    let (names, spans) = declarators_with_spans(&child);
                    proxy.abstract_operations.extend(names);
                    entry.proxy_abstract_spans.extend(spans);
                }
                // What the parser reported in an error node.
                CstKind::Error => {}
                // A storage home, executor or feature delegation: the home
                // executor's and the executor's, productions (8) and (15).
                _ => self.not_in_place(&child, "a proxy home", "CORBA 3.3 Part 3 §8.18 (33), (34)"),
            }
        }
        entry.model.proxy_home = Some(proxy);
    }

    /// A node where the production of `place` does not put it: reported,
    /// and not read as a member of `place`. The parser keeps such a node
    /// in an error node; this is for one that reaches the lowering
    /// otherwise.
    fn not_in_place(&mut self, node: &Arc<SyntaxNode>, place: &str, clause: &'static str) {
        self.diagnose_at(
            DiagnosticCode::CompositionRule,
            node.span(),
            &format!("`{}` does not belong in {place}", syntax_text(node)),
            clause,
        );
    }

    /// The scoped name under `node`, with the scopes it may be found in.
    fn name_use(&self, node: &Arc<SyntaxNode>) -> Option<NameUse> {
        let scoped = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::ScopedName)?;
        let name = self.scoped_name_of(&scoped)?;
        let candidates = if crate::lower::is_root_anchored(&scoped) {
            alloc::vec![name.clone()]
        } else {
            self.scopes.lookup_candidates(name.segments())
        };
        Some(NameUse {
            candidates,
            written: name.segments().join("::"),
            span: scoped.span(),
        })
    }

    // -- after the walk --------------------------------------------------

    /// Check the PSDL declarations and the compositions against the
    /// finished model, and hand over what the file declared beyond IDL.
    pub(crate) fn finish_cidl(&mut self) -> CidlModel {
        let state = core::mem::take(&mut self.cidl);
        for entry in &state.storage {
            self.check_storage(entry, &state.storage);
        }
        // The model names what `of`, the bases and `implements` resolved
        // to; a name that did not resolve stays as written.
        let storage = state
            .storage
            .iter()
            .map(|entry| {
                let module = entry.module();
                let name = |this: &Self, reference: &Ref<InterfaceNode>, written: &String| {
                    this.psdl_iface(reference, &module)
                        .and_then(|id| {
                            let node_id = this.ast.interface(id).ok()?.node_id;
                            this.qualified(AnyId::Interface(id, node_id))
                        })
                        .unwrap_or_else(|| written.clone())
                };
                let mut decl = entry.decl.clone();
                if let (Some((reference, _)), Some(written)) = (&entry.of, &entry.decl.of) {
                    decl.of = Some(name(self, reference, written));
                }
                decl.bases = entry
                    .bases
                    .iter()
                    .zip(&entry.decl.bases)
                    .map(|((reference, _), written)| name(self, reference, written))
                    .collect();
                decl.implements = entry
                    .implements
                    .iter()
                    .zip(&entry.decl.implements)
                    .map(|((reference, _), written)| name(self, reference, written))
                    .collect();
                decl
            })
            .collect();
        let mut model = CidlModel {
            storage,
            compositions: Vec::new(),
        };
        for (index, entry) in state.compositions.iter().enumerate() {
            let earlier = &state.compositions[..index];
            let mut composition = entry.model.clone();
            self.check_composition(entry, earlier, &state.storage, &mut composition);
            model.compositions.push(composition);
        }
        model
    }

    /// The PSDL rules of PSS 2.0 §3.2.5 and §3.2.6 that name other
    /// declarations or state members.
    fn check_storage(&mut self, entry: &StorageEntry, all: &[StorageEntry]) {
        let kind = entry.decl.kind;
        let module = entry.module();
        // `report` says whether an unresolved name is this rule's to
        // report: a base is a reference of the model, which the resolve
        // pass after lowering reports; `of`, `implements`, `as` and
        // `scope` are kept here only.
        let expect = |this: &mut Self,
                      reference: &Ref<InterfaceNode>,
                      span: Span,
                      want: StorageKind,
                      role: &str,
                      report: bool| {
            let Some(id) = this.psdl_iface(reference, &module) else {
                if report && let Ref::Pending(PendingRef::ScopedName(name)) = reference {
                    this.diagnose_at(
                        DiagnosticCode::UnresolvedName,
                        span,
                        &format!("`{}` names no declaration", name.segments().join("::")),
                        PSS,
                    );
                }
                return;
            };
            let found = all.iter().find(|e| e.interface == id).map(|e| e.decl.kind);
            if found != Some(want) {
                this.diagnose_at(
                    DiagnosticCode::CompositionRule,
                    span,
                    &format!(
                        "{role} is an {}, and this names something else",
                        want.as_str()
                    ),
                    PSS,
                );
            }
        };
        // `of`.
        if let Some((of, span)) = &entry.of {
            let want = if kind == StorageKind::AbstractStorageHome {
                StorageKind::AbstractStorageType
            } else {
                StorageKind::StorageType
            };
            expect(self, of, *span, want, "what a storage home is `of`", true);
        }
        // Bases.
        for (base, span) in &entry.bases {
            expect(self, base, *span, kind, "a base", false);
        }
        if kind == StorageKind::StorageType && entry.bases.len() > 1
            || kind == StorageKind::StorageHome && entry.bases.len() > 1
        {
            self.diagnose_at(
                DiagnosticCode::InvalidInheritance,
                entry.span,
                &format!("a {} has one base", kind.as_str()),
                "Persistent State Service 2.0 §3.2.3 (32), (43)",
            );
        }
        // §3.2.5.9: the type of a base abstract storage home is the
        // home's own type or one of its bases.
        if kind == StorageKind::AbstractStorageHome
            && let Some(own) = entry
                .of
                .as_ref()
                .and_then(|(r, _)| self.psdl_iface(r, &entry.module()))
        {
            for (base, span) in &entry.bases {
                let base_of = self
                    .psdl_iface(base, &entry.module())
                    .and_then(|id| all.iter().find(|e| e.interface == id))
                    .and_then(|e| {
                        e.of.as_ref()
                            .and_then(|(r, _)| self.psdl_iface(r, &e.module()))
                    });
                if let Some(base_of) = base_of
                    && !self.derives_from(own, base_of)
                {
                    self.diagnose_at(
                        DiagnosticCode::InvalidInheritance,
                        *span,
                        "the abstract storage type of a base abstract storage home is the \
                         home's own abstract storage type or one of its bases",
                        "Persistent State Service 2.0 §3.2.5.9",
                    );
                }
            }
        }
        // `implements`.
        let implemented = if kind == StorageKind::StorageType {
            StorageKind::AbstractStorageType
        } else {
            StorageKind::AbstractStorageHome
        };
        for (reference, span) in &entry.implements {
            expect(
                self,
                reference,
                *span,
                implemented,
                "what is implemented",
                true,
            );
        }
        // `ref<X>` state types.
        for (reference, span) in &entry.state_refs {
            expect(
                self,
                reference,
                *span,
                StorageKind::AbstractStorageType,
                "`ref<…>`'s type",
                false,
            );
        }
        // Keys and factories name state members of the home's type.
        let members = if kind.is_home() {
            entry
                .of
                .as_ref()
                .and_then(|(r, _)| self.iface(r))
                .map(|of| self.state_members(of, all))
        } else {
            Some(self.state_members(entry.interface, all))
        };
        if let Some(members) = &members {
            let lists = entry
                .decl
                .keys
                .iter()
                .zip(&entry.key_spans)
                .map(|(k, s)| (k, *s, "key", "§3.2.5.7"))
                .chain(
                    entry
                        .decl
                        .factories
                        .iter()
                        .zip(&entry.factory_spans)
                        .map(|(k, s)| (k, *s, "factory", "§3.2.5.8")),
                );
            let mut found: Vec<(Span, String)> = Vec::new();
            for (key, span, what, _) in lists {
                for (index, member) in key.members.iter().enumerate() {
                    if !members.iter().any(|m| m == member) {
                        found.push((
                            span,
                            format!(
                                "the {what} `{}` names `{member}`, which is no state member of \
                                 the storage type",
                                key.name
                            ),
                        ));
                    }
                    if key.members[..index].contains(member) {
                        found.push((
                            span,
                            format!("the {what} `{}` names `{member}` twice", key.name),
                        ));
                    }
                }
            }
            for (span, message) in found {
                self.diagnose_at(
                    DiagnosticCode::CompositionRule,
                    span,
                    &message,
                    "Persistent State Service 2.0 §3.2.5.7, §3.2.5.8",
                );
            }
            for member in &entry.decl.ref_representation {
                if !members.contains(member)
                    && let Some(span) = entry.ref_rep_span
                {
                    self.diagnose_at(
                        DiagnosticCode::CompositionRule,
                        span,
                        &format!("`ref({member})` names no state member of the storage type"),
                        "Persistent State Service 2.0 §3.2.6.3",
                    );
                }
            }
            for (store, directive) in entry.stores.iter().zip(&entry.decl.stores) {
                if !members.contains(&directive.member) {
                    self.diagnose_at(
                        DiagnosticCode::CompositionRule,
                        store.span,
                        &format!(
                            "`stores {}` names no state member of the storage type",
                            directive.member
                        ),
                        "Persistent State Service 2.0 §3.2.6.2",
                    );
                }
            }
        }
        for store in &entry.stores {
            if let Some(as_) = &store.as_ {
                expect(
                    self,
                    as_,
                    store.span,
                    StorageKind::StorageType,
                    "a stored type",
                    true,
                );
            }
            if let Some(scope) = &store.scope {
                expect(
                    self,
                    scope,
                    store.span,
                    StorageKind::StorageHome,
                    "a `scope`",
                    true,
                );
            }
        }
        // §3.2.6.5: the primary key is one of the home's keys.
        if let (Some(key), Some(span)) = (&entry.decl.primary_key, entry.primary_key_span)
            && key != "ref"
        {
            let mut keys: Vec<&str> = entry.decl.keys.iter().map(|k| k.name.as_str()).collect();
            for (implemented, _) in &entry.implements {
                if let Some(other) = self
                    .psdl_iface(implemented, &entry.module())
                    .and_then(|id| all.iter().find(|e| e.interface == id))
                {
                    keys.extend(other.decl.keys.iter().map(|k| k.name.as_str()));
                }
            }
            if !keys.contains(&key.as_str()) {
                self.diagnose_at(
                    DiagnosticCode::CompositionRule,
                    span,
                    &format!("`primary key {key}` names no key of the storage home"),
                    "Persistent State Service 2.0 §3.2.6.5",
                );
            }
        }
    }

    /// The state members of a storage type and of every type it inherits
    /// from or implements.
    fn state_members(&self, interface: Id<InterfaceNode>, all: &[StorageEntry]) -> Vec<String> {
        let mut out = Vec::new();
        for id in self.storage_closure(interface, all) {
            if let Ok(node) = self.ast.interface(id) {
                for attribute in &node.attributes {
                    if let Ok(attribute) = self.ast.attribute(*attribute)
                        && !out.contains(&attribute.name)
                    {
                        out.push(attribute.name.clone());
                    }
                }
            }
        }
        out
    }

    /// A PSDL declaration and everything it inherits from or
    /// implements, itself first.
    fn storage_closure(
        &self,
        start: Id<InterfaceNode>,
        all: &[StorageEntry],
    ) -> Vec<Id<InterfaceNode>> {
        let mut out = alloc::vec![start];
        let mut index = 0;
        while index < out.len() && out.len() < 256 {
            let id = out[index];
            index += 1;
            let mut next: Vec<Id<InterfaceNode>> = self
                .ast
                .interface(id)
                .map(|node| node.bases.iter().filter_map(|b| self.iface(b)).collect())
                .unwrap_or_default();
            if let Some(entry) = all.iter().find(|e| e.interface == id) {
                next.extend(
                    entry
                        .implements
                        .iter()
                        .filter_map(|(r, _)| self.psdl_iface(r, &entry.module())),
                );
            }
            for id in next {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    /// Whether `sub` is `sup` or derives from it, directly or not.
    fn derives_from(&self, sub: Id<InterfaceNode>, sup: Id<InterfaceNode>) -> bool {
        let mut stack = alloc::vec![sub];
        let mut seen: Vec<Id<InterfaceNode>> = Vec::new();
        while let Some(id) = stack.pop() {
            if id == sup {
                return true;
            }
            if seen.contains(&id) || seen.len() > 256 {
                continue;
            }
            seen.push(id);
            if let Ok(node) = self.ast.interface(id) {
                stack.extend(node.bases.iter().filter_map(|b| self.iface(b)));
            }
        }
        false
    }

    /// Whether the value type `sub` is `sup` or derives from it.
    fn value_derives_from(&self, sub: Id<TypeNode>, sup: Id<TypeNode>) -> bool {
        let mut stack = alloc::vec![sub];
        let mut seen: Vec<Id<TypeNode>> = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(id) = self.unaliased(id) else {
                continue;
            };
            if Some(id) == self.unaliased(sup) {
                return true;
            }
            if seen.contains(&id) || seen.len() > 256 {
                continue;
            }
            seen.push(id);
            if let Ok(node) = self.ast.type_(id)
                && let TypeKind::Valuetype { bases, .. } = &node.kind
            {
                stack.extend(bases.iter().filter_map(|b| match self.ty(Some(b)) {
                    Ty::Type(id) => Some(id),
                    _ => None,
                }));
            }
        }
        false
    }

    /// Resolve a name a composition used.
    fn resolve_use(&self, use_: &NameUse) -> Option<AnyId> {
        use_.candidates
            .iter()
            .find_map(|candidate| self.ast.find_by_scoped_name(candidate).ok())
    }

    /// The composition rules: the name-level ones of
    /// [`Composition::validate`] and the type-level ones of §8.14 and
    /// §8.15.
    fn check_composition(
        &mut self,
        entry: &CompositionEntry,
        earlier: &[CompositionEntry],
        storage: &[StorageEntry],
        composition: &mut Composition,
    ) {
        // §8.5: the composition's name is one of its module's names.
        let path: Vec<String> = composition.name.split("::").map(String::from).collect();
        let clash = self
            .ast
            .find_by_scoped_name(&ScopedName::absolute(path))
            .is_ok()
            || earlier
                .iter()
                .any(|e| self.is_one_name(&e.model.name, &composition.name));
        if clash {
            self.diagnose_at(
                DiagnosticCode::DuplicateName,
                entry.name_span,
                &format!(
                    "`{}` is already declared in this scope; a composition's name is a name \
                     of its enclosing module",
                    composition.name
                ),
                "CORBA 3.3 Part 3 §8.5",
            );
        }

        // §8.7: `implements` names a component home. Without one, the
        // parser reported the body (production (8)), and the rules below,
        // which read the home, have nothing to read.
        let Some(use_) = &entry.implements else {
            return;
        };
        let home = match self.resolve_use(use_) {
            Some(AnyId::Interface(id, _))
                if self
                    .ast
                    .interface(id)
                    .is_ok_and(|node| node.kind == InterfaceKind::Home) =>
            {
                id
            }
            Some(_) => {
                self.diagnose_at(
                    DiagnosticCode::CompositionRule,
                    use_.span,
                    &format!("`{}` is not a component home", use_.written),
                    "CORBA 3.3 Part 3 §8.7",
                );
                return;
            }
            None => {
                self.diagnose_at(
                    DiagnosticCode::UnresolvedName,
                    use_.span,
                    &format!("`{}` names no declaration", use_.written),
                    "CORBA 3.3 Part 3 §8.7",
                );
                return;
            }
        };
        if let Some(name) = self
            .ast
            .interface(home)
            .ok()
            .and_then(|node| self.qualified(AnyId::Interface(home, node.node_id)))
        {
            composition.home_executor.implements = name;
        }

        // §8.8, §8.9, §8.12: the abstract storage homes.
        let abstract_home = |this: &mut Self, use_: &NameUse| -> Option<usize> {
            match this.resolve_use(use_) {
                Some(AnyId::Interface(id, _)) => {
                    let found = storage.iter().position(|e| {
                        e.interface == id && e.decl.kind == StorageKind::AbstractStorageHome
                    });
                    if found.is_none() {
                        this.diagnose_at(
                            DiagnosticCode::CompositionRule,
                            use_.span,
                            &format!("`{}` is not an abstract storage home", use_.written),
                            "CORBA 3.3 Part 3 §8.8, §8.9, §8.12",
                        );
                    }
                    found
                }
                Some(_) => {
                    this.diagnose_at(
                        DiagnosticCode::CompositionRule,
                        use_.span,
                        &format!("`{}` is not an abstract storage home", use_.written),
                        "CORBA 3.3 Part 3 §8.8, §8.9, §8.12",
                    );
                    None
                }
                None => {
                    this.diagnose_at(
                        DiagnosticCode::UnresolvedName,
                        use_.span,
                        &format!("`{}` names no declaration", use_.written),
                        "CORBA 3.3 Part 3 §8.8, §8.9, §8.12",
                    );
                    None
                }
            }
        };
        let bound = entry
            .binds_to
            .as_ref()
            .and_then(|use_| abstract_home(self, use_));
        if let Some(index) = bound {
            composition.home_executor.binds_to = Some(storage[index].decl.name.clone());
        }
        if let Some(use_) = &entry.stored_on
            && let Some(index) = abstract_home(self, use_)
        {
            composition.home_executor.stored_on = Some(storage[index].decl.name.clone());
        }
        for (segment, use_) in entry.segment_stored_on.iter().enumerate() {
            if let Some(use_) = use_
                && let Some(index) = abstract_home(self, use_)
                && let Some(def) = composition.home_executor.executor.segments.get_mut(segment)
            {
                def.stored_on = Some(storage[index].decl.name.clone());
            }
        }

        // The rules that read names.
        let (facts, info) = self.home_facts(home);
        let storage_facts = bound.map(|index| self.storage_home_facts(&storage[index], storage));
        for error in composition.validate(&facts, storage_facts.as_ref()) {
            let span = subject_span(entry, error.subject);
            self.diagnose_at(
                DiagnosticCode::CompositionRule,
                span,
                &error.message,
                error.clause,
            );
        }

        // The rules that read types.
        if let Some(index) = bound {
            self.check_feature_types(entry, composition, &info, &storage[index], storage);
            self.check_storage_delegation_signatures(
                entry,
                composition,
                &info,
                &storage[index],
                storage,
            );
        }
    }

    fn qualified(&self, any: AnyId) -> Option<String> {
        self.ast
            .scoped_name_of(any)
            .map(|name| name.segments().join("::"))
    }

    /// The home's chain, itself first, and its bases.
    fn interface_chain(&self, start: Id<InterfaceNode>) -> Vec<Id<InterfaceNode>> {
        let mut out = alloc::vec![start];
        let mut index = 0;
        while index < out.len() && out.len() < 256 {
            let id = out[index];
            index += 1;
            if let Ok(node) = self.ast.interface(id) {
                for base in node.bases.iter().filter_map(|b| self.iface(b)) {
                    if !out.contains(&base) {
                        out.push(base);
                    }
                }
            }
        }
        out
    }

    /// What [`Composition::validate`] reads of a home, and what the type
    /// rules read of it.
    fn home_facts(&self, home: Id<InterfaceNode>) -> (HomeFacts, HomeInfo) {
        let mut facts = HomeFacts::default();
        let mut info = HomeInfo::default();
        for id in self.interface_chain(home) {
            let Ok(node) = self.ast.interface(id) else {
                continue;
            };
            if let Some(manages) = node.facets.corba.as_ref().and_then(|c| c.manages.as_ref()) {
                if info.component.is_none() {
                    info.component = self.iface(&manages.component);
                }
                if info.key.is_none() {
                    info.key.clone_from(&manages.primary_key);
                }
            }
            for op in &node.operations {
                let Ok(operation) = self.ast.operation(*op) else {
                    continue;
                };
                let role = operation
                    .facets
                    .corba
                    .as_ref()
                    .and_then(|c| c.home_operation);
                match role {
                    Some(HomeOperation::Factory) => facts.factories.push(operation.name.clone()),
                    Some(HomeOperation::Finder) => {
                        facts.explicit_finders.push(operation.name.clone());
                    }
                    None => {}
                }
                facts.operations.push(operation.name.clone());
                info.operations.push(*op);
            }
        }
        facts.primary_key = info.key.is_some();
        // §6.8.1: the orthodox operations of the implicit interface.
        facts.factories.push("create".into());
        facts.operations.push("create".into());
        if facts.primary_key {
            for name in ["find_by_primary_key", "remove", "get_primary_key"] {
                facts.operations.push(name.into());
            }
        }
        if let Some(component) = info.component {
            for id in self.interface_chain(component) {
                let Ok(node) = self.ast.interface(id) else {
                    continue;
                };
                for attribute in &node.attributes {
                    let Ok(attribute_node) = self.ast.attribute(*attribute) else {
                        continue;
                    };
                    let role = attribute_node
                        .facets
                        .corba
                        .as_ref()
                        .and_then(|c| c.component_role.as_ref());
                    match role {
                        Some(ComponentRole::Facet { .. }) => {
                            facts.facets.push(attribute_node.name.clone());
                        }
                        None
                        | Some(
                            ComponentRole::Receptacle { .. } | ComponentRole::EventSource { .. },
                        ) => {
                            facts.stateful_features.push(attribute_node.name.clone());
                            info.features.push(*attribute);
                        }
                        Some(_) => {}
                    }
                }
            }
        }
        (facts, info)
    }

    /// What [`Composition::validate`] reads of an abstract storage home.
    fn storage_home_facts(&self, entry: &StorageEntry, all: &[StorageEntry]) -> StorageHomeFacts {
        let mut facts = StorageHomeFacts::default();
        for id in self.storage_closure(entry.interface, all) {
            if let Ok(node) = self.ast.interface(id) {
                for op in &node.operations {
                    if let Ok(op) = self.ast.operation(*op) {
                        facts.operations.push(op.name.clone());
                    }
                }
            }
            if let Some(home) = all.iter().find(|e| e.interface == id) {
                facts
                    .operations
                    .extend(home.decl.factories.iter().map(|f| f.name.clone()));
                for key in &home.decl.keys {
                    facts.operations.push(format!("find_by_{}", key.name));
                    facts.operations.push(format!("find_ref_by_{}", key.name));
                }
            }
        }
        if let Some(of) = entry
            .of
            .as_ref()
            .and_then(|(r, _)| self.psdl_iface(r, &entry.module()))
        {
            facts.state_members = self.state_members(of, all);
        }
        facts
    }

    /// The interface a reference denotes: resolved, or still pending by
    /// a scoped name the model now holds. The resolve pass of the caller
    /// (`zerodds-idl-compose`) runs after lowering and reports what does
    /// not resolve; here, such a name answers `None` and the rule asking
    /// is not applied to it.
    fn iface(&self, reference: &Ref<InterfaceNode>) -> Option<Id<InterfaceNode>> {
        match reference {
            Ref::Resolved(id) => Some(*id),
            Ref::Pending(PendingRef::ScopedName(name)) => {
                match self.ast.find_by_scoped_name(name) {
                    Ok(AnyId::Interface(id, _)) => Some(id),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The interface a reference of a PSDL declaration denotes. A name
    /// written before its declaration is pending as written; it is looked
    /// up from the declaration's module outwards, as the resolve pass
    /// looks up the model's own references (OMG IDL 4.2 §7.5).
    fn psdl_iface(
        &self,
        reference: &Ref<InterfaceNode>,
        module: &[String],
    ) -> Option<Id<InterfaceNode>> {
        if let Some(id) = self.iface(reference) {
            return Some(id);
        }
        let Ref::Pending(PendingRef::ScopedName(name)) = reference else {
            return None;
        };
        (0..=module.len()).rev().find_map(|depth| {
            let mut segments = module[..depth].to_vec();
            segments.extend_from_slice(name.segments());
            match self
                .ast
                .find_by_scoped_name(&ScopedName::absolute(segments))
            {
                Ok(AnyId::Interface(id, _)) => Some(id),
                _ => None,
            }
        })
    }

    /// A type as the compatibility rules see it, through `typedef`s.
    fn ty(&self, reference: Option<&Ref<TypeNode>>) -> Ty {
        let Some(mut reference) = reference.cloned() else {
            return Ty::Void;
        };
        for _ in 0..64 {
            let id = match &reference {
                Ref::Resolved(id) => *id,
                Ref::Pending(PendingRef::ScopedName(name)) => {
                    match self.ast.find_by_scoped_name(name) {
                        Ok(AnyId::Type(id, _)) => id,
                        Ok(AnyId::Interface(id, _)) => return Ty::Object(id),
                        _ => return Ty::Unknown,
                    }
                }
                _ => return Ty::Unknown,
            };
            match self.ast.type_(id).map(|node| &node.kind) {
                Ok(TypeKind::Alias { aliased }) => reference = aliased.clone(),
                Ok(TypeKind::ObjectRef {
                    interface: Some(interface),
                }) => return self.iface(interface).map_or(Ty::Unknown, Ty::Object),
                Ok(_) => return Ty::Type(id),
                Err(_) => return Ty::Unknown,
            }
        }
        Ty::Unknown
    }

    fn identical(&self, a: Ty, b: Ty) -> bool {
        match (a, b) {
            (Ty::Void, Ty::Void) => true,
            (Ty::Object(a), Ty::Object(b)) | (Ty::StorageRef(a), Ty::StorageRef(b)) => a == b,
            (Ty::Type(a), Ty::Type(b)) => self.same_type(a, b, 0),
            _ => false,
        }
    }

    fn is_reference_or_value(&self, ty: Ty) -> bool {
        match ty {
            Ty::Object(_) => true,
            Ty::Type(id) => matches!(
                self.ast.type_(id).map(|node| &node.kind),
                Ok(TypeKind::Valuetype { .. } | TypeKind::ObjectRef { .. })
            ),
            _ => false,
        }
    }

    /// Whether `sub` is `sup` or more derived: interfaces through their
    /// bases, value types through theirs.
    fn same_or_derived(&self, sub: Ty, sup: Ty) -> bool {
        match (sub, sup) {
            (Ty::Object(sub), Ty::Object(sup)) => self.derives_from(sub, sup),
            (Ty::Type(sub), Ty::Type(sup)) => {
                self.same_type(sub, sup, 0) || self.value_derives_from(sub, sup)
            }
            _ => false,
        }
    }

    /// Table 8.6: the storage member's type against the feature's.
    fn check_feature_types(
        &mut self,
        entry: &CompositionEntry,
        composition: &Composition,
        info: &HomeInfo,
        bound: &StorageEntry,
        all: &[StorageEntry],
    ) {
        const TABLE: &str = "CORBA 3.3 Part 3 §8.14, Table 8.6";
        let Some(of) = bound
            .of
            .as_ref()
            .and_then(|(r, _)| self.psdl_iface(r, &bound.module()))
        else {
            return;
        };
        let mut members: Vec<(String, Ty)> = Vec::new();
        for id in self.storage_closure(of, all) {
            if let Ok(node) = self.ast.interface(id) {
                for attribute in &node.attributes {
                    if let Ok(attribute) = self.ast.attribute(*attribute) {
                        members.push((attribute.name.clone(), self.ty(Some(&attribute.type_))));
                    }
                }
            }
        }
        let delegations = &composition.home_executor.executor.feature_delegations;
        for (index, delegation) in delegations.iter().enumerate() {
            let Some(member) = members
                .iter()
                .find(|(name, _)| *name == delegation.storage_member)
                .map(|(_, ty)| *ty)
            else {
                continue;
            };
            let Some(feature) = info.features.iter().find_map(|id| {
                self.ast
                    .attribute(*id)
                    .ok()
                    .filter(|a| a.name == delegation.feature)
            }) else {
                continue;
            };
            let role = feature
                .facets
                .corba
                .as_ref()
                .and_then(|c| c.component_role.clone());
            let feature_ty = self.ty(Some(&feature.type_));
            // A name the model does not resolve is the resolve pass's to
            // report; no rule is asked of it here.
            let wanted_ty = match &role {
                Some(ComponentRole::Receptacle { interface, .. }) => {
                    self.iface(interface).map_or(Ty::Unknown, Ty::Object)
                }
                _ => feature_ty,
            };
            if member == Ty::Unknown
                || (wanted_ty == Ty::Unknown
                    && !matches!(
                        role,
                        Some(ComponentRole::EventSource {
                            publisher: true,
                            ..
                        })
                    ))
            {
                continue;
            }
            let (compatible, rule) = match role {
                None => {
                    if self.is_reference_or_value(feature_ty) {
                        (
                            self.same_or_derived(feature_ty, member),
                            "of an attribute whose type is an object reference or a value \
                             type is that type or one of its bases",
                        )
                    } else {
                        (
                            self.identical(feature_ty, member),
                            "of an attribute is the attribute's type",
                        )
                    }
                }
                Some(ComponentRole::Receptacle {
                    multiple: false, ..
                }) => (
                    self.same_or_derived(wanted_ty, member),
                    "of a simplex receptacle is its interface or one of its bases",
                ),
                Some(ComponentRole::Receptacle { multiple: true, .. }) => {
                    let element = match member {
                        Ty::Type(id) => match self.ast.type_(id).map(|node| &node.kind) {
                            Ok(TypeKind::Sequence { element, .. }) => Some(self.ty(Some(element))),
                            _ => None,
                        },
                        _ => None,
                    };
                    if element == Some(Ty::Unknown) {
                        continue;
                    }
                    (
                        element.is_some_and(|element| self.same_or_derived(wanted_ty, element)),
                        "of a multiplex receptacle is a sequence of its interface or of one \
                         of its bases",
                    )
                }
                Some(ComponentRole::EventSource {
                    publisher: false, ..
                }) => (
                    self.same_or_derived(feature_ty, member),
                    "of an emitter is its event type or one of its bases",
                ),
                Some(ComponentRole::EventSource {
                    publisher: true, ..
                }) => (
                    matches!(member, Ty::Type(id) if matches!(
                        self.ast.type_(id).map(|node| &node.kind),
                        Ok(TypeKind::Primitive(PrimitiveKind::Int32))
                    )),
                    "of a publisher is `long`, the ChannelId of its notification channel",
                ),
                Some(_) => continue,
            };
            if !compatible {
                let span = entry
                    .feature_spans
                    .get(index)
                    .copied()
                    .unwrap_or(entry.span);
                self.diagnose_at(
                    DiagnosticCode::TypeMismatch,
                    span,
                    &format!(
                        "`{}` cannot hold the state of `{}`: the storage member's type {rule}",
                        delegation.storage_member, delegation.feature
                    ),
                    TABLE,
                );
            }
        }
    }

    /// §8.15: the signature of the storage home operation against the
    /// home operation's.
    fn check_storage_delegation_signatures(
        &mut self,
        entry: &CompositionEntry,
        composition: &Composition,
        info: &HomeInfo,
        bound: &StorageEntry,
        all: &[StorageEntry],
    ) {
        const CLAUSE: &str = "CORBA 3.3 Part 3 §8.15";
        let delegations = &composition.home_executor.storage_home_delegations;
        for (index, delegation) in delegations.iter().enumerate() {
            let span = entry
                .storage_delegation_spans
                .get(index)
                .copied()
                .unwrap_or(entry.span);
            let Some(home_sig) = self.home_signature(info, &delegation.operation) else {
                continue;
            };
            let Some(storage_sig) = self.storage_signature(bound, all, delegation.target_name())
            else {
                continue;
            };
            let mut problems: Vec<String> = Vec::new();
            if home_sig.factory {
                if !storage_sig.factory {
                    problems.push(format!(
                        "`{}` is a factory, and `{}` is not a factory of the storage home",
                        delegation.operation,
                        delegation.target_name()
                    ));
                }
            } else if home_sig.ret == Ty::Unknown || storage_sig.ret == Ty::Unknown {
                // Unresolved: the resolve pass reports it.
            } else if let Ty::StorageRef(_) = storage_sig.ret {
                problems.push(format!(
                    "`{}` returns `ref<…>`, a reference to a storage object, and the return \
                     type of a home operation is an IDL type",
                    delegation.target_name()
                ));
            } else if self.is_reference_or_value(home_sig.ret) {
                if !self.same_or_derived(storage_sig.ret, home_sig.ret) {
                    problems.push(
                        "the storage home operation returns neither the home operation's \
                         type nor a type derived from it"
                            .into(),
                    );
                }
            } else if !self.identical(storage_sig.ret, home_sig.ret) {
                problems.push("the two operations return different types".into());
            }
            for raised in &storage_sig.raises {
                if !home_sig.raises.contains(raised) {
                    problems.push(format!(
                        "the storage home operation raises `{raised}`, which the home \
                         operation does not"
                    ));
                }
            }
            if home_sig.params.len() == storage_sig.params.len() {
                for (position, ((direction, home_ty), (_, storage_ty))) in
                    home_sig.params.iter().zip(&storage_sig.params).enumerate()
                {
                    let compatible = if *home_ty == Ty::Unknown || *storage_ty == Ty::Unknown {
                        true
                    } else if !self.is_reference_or_value(*home_ty) {
                        self.identical(*home_ty, *storage_ty)
                    } else {
                        match direction {
                            ParamDirection::In => self.same_or_derived(*home_ty, *storage_ty),
                            ParamDirection::Out => self.same_or_derived(*storage_ty, *home_ty),
                            ParamDirection::InOut => self.identical(*home_ty, *storage_ty),
                        }
                    };
                    if !compatible {
                        problems.push(format!(
                            "parameter {} of the storage home operation is not compatible with \
                             the home operation's",
                            position + 1
                        ));
                    }
                }
            } else {
                problems.push(format!(
                    "`{}` takes {} parameters and `{}` {}",
                    delegation.operation,
                    home_sig.params.len(),
                    delegation.target_name(),
                    storage_sig.params.len()
                ));
            }
            for problem in problems {
                self.diagnose_at(DiagnosticCode::TypeMismatch, span, &problem, CLAUSE);
            }
        }
    }

    /// The signature of a home operation: an explicit one of the home or
    /// its bases, or an orthodox one of §6.8.1.
    fn home_signature(&self, info: &HomeInfo, name: &str) -> Option<Signature> {
        if let Some(operation) = info
            .operations
            .iter()
            .filter_map(|id| self.ast.operation(*id).ok())
            .find(|op| op.name == name)
        {
            let role = operation
                .facets
                .corba
                .as_ref()
                .and_then(|c| c.home_operation);
            let mut signature = self.signature_of(operation);
            // §6.8.3: the equivalent operation of a factory or a finder
            // returns the component type and raises `CreateFailure`,
            // respectively `FinderFailure`.
            let component = info.component.map_or(Ty::Unknown, Ty::Object);
            match role {
                Some(HomeOperation::Factory) => {
                    signature.factory = true;
                    signature.ret = component;
                    signature.raises.push("Components::CreateFailure".into());
                }
                Some(HomeOperation::Finder) => {
                    signature.ret = component;
                    signature.raises.push("Components::FinderFailure".into());
                }
                None => {}
            }
            return Some(signature);
        }
        let key = info.key.as_ref().map(|key| self.ty(Some(key)));
        let component = info.component.map_or(Ty::Unknown, Ty::Object);
        let names = |list: &[&str]| list.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        match (name, key) {
            ("create", None) => Some(Signature {
                factory: true,
                ret: component,
                params: Vec::new(),
                raises: names(&["Components::CreateFailure"]),
            }),
            ("create", Some(key)) => Some(Signature {
                factory: true,
                ret: component,
                params: alloc::vec![(ParamDirection::In, key)],
                raises: names(&[
                    "Components::CreateFailure",
                    "Components::DuplicateKeyValue",
                    "Components::InvalidKey",
                ]),
            }),
            ("find_by_primary_key", Some(key)) => Some(Signature {
                factory: false,
                ret: component,
                params: alloc::vec![(ParamDirection::In, key)],
                raises: names(&[
                    "Components::FinderFailure",
                    "Components::UnknownKeyValue",
                    "Components::InvalidKey",
                ]),
            }),
            ("remove", Some(key)) => Some(Signature {
                factory: false,
                ret: Ty::Void,
                params: alloc::vec![(ParamDirection::In, key)],
                raises: names(&[
                    "Components::RemoveFailure",
                    "Components::UnknownKeyValue",
                    "Components::InvalidKey",
                ]),
            }),
            ("get_primary_key", Some(key)) => Some(Signature {
                factory: false,
                ret: key,
                params: alloc::vec![(ParamDirection::In, component)],
                raises: Vec::new(),
            }),
            _ => None,
        }
    }

    /// The signature of an abstract storage home operation: a local
    /// operation, a factory (§3.2.5.8) or a key's finder (§3.2.5.7), of
    /// the home or of a base.
    fn storage_signature(
        &self,
        bound: &StorageEntry,
        all: &[StorageEntry],
        name: &str,
    ) -> Option<Signature> {
        for id in self.storage_closure(bound.interface, all) {
            if let Ok(node) = self.ast.interface(id)
                && let Some(operation) = node
                    .operations
                    .iter()
                    .filter_map(|op| self.ast.operation(*op).ok())
                    .find(|op| op.name == name)
            {
                return Some(self.signature_of(operation));
            }
            let Some(home) = all.iter().find(|e| e.interface == id) else {
                continue;
            };
            let of = home
                .of
                .as_ref()
                .and_then(|(r, _)| self.psdl_iface(r, &home.module()));
            let member_types = |members: &[String]| -> Vec<(ParamDirection, Ty)> {
                let types = of
                    .map(|of| self.state_member_types(of, all))
                    .unwrap_or_default();
                members
                    .iter()
                    .map(|member| {
                        let ty = types
                            .iter()
                            .find(|(name, _)| name == member)
                            .map_or(Ty::Unknown, |(_, ty)| *ty);
                        (ParamDirection::In, ty)
                    })
                    .collect()
            };
            let ret = of.map_or(Ty::Unknown, Ty::Object);
            if let Some(factory) = home.decl.factories.iter().find(|f| f.name == name) {
                return Some(Signature {
                    factory: true,
                    ret,
                    params: member_types(&factory.members),
                    raises: Vec::new(),
                });
            }
            // §3.2.5.7: a key `k` declares `S find_by_k(…) raises
            // (CosPersistentState::NotFound)` and `ref<S> find_ref_by_k(…)`,
            // which returns a NULL reference instead.
            for key in &home.decl.keys {
                if name == format!("find_by_{}", key.name) {
                    return Some(Signature {
                        factory: false,
                        ret,
                        params: member_types(&key.members),
                        raises: alloc::vec!["CosPersistentState::NotFound".to_string()],
                    });
                }
                if name == format!("find_ref_by_{}", key.name) {
                    return Some(Signature {
                        factory: false,
                        ret: of.map_or(Ty::Unknown, Ty::StorageRef),
                        params: member_types(&key.members),
                        raises: Vec::new(),
                    });
                }
            }
        }
        None
    }

    fn state_member_types(&self, of: Id<InterfaceNode>, all: &[StorageEntry]) -> Vec<(String, Ty)> {
        let mut out = Vec::new();
        for id in self.storage_closure(of, all) {
            if let Ok(node) = self.ast.interface(id) {
                for attribute in &node.attributes {
                    if let Ok(attribute) = self.ast.attribute(*attribute) {
                        out.push((attribute.name.clone(), self.ty(Some(&attribute.type_))));
                    }
                }
            }
        }
        out
    }

    fn signature_of(&self, operation: &OperationNode) -> Signature {
        let params = operation
            .params
            .iter()
            .filter_map(|id| self.ast.param(*id).ok())
            .map(|param| (param.direction, self.ty(Some(&param.type_))))
            .collect();
        let raises = operation
            .raises
            .iter()
            .filter_map(Ref::resolved)
            .filter_map(|id| {
                let node_id = self.ast.exception(id).ok()?.node_id;
                self.qualified(AnyId::Exception(id, node_id))
            })
            .collect();
        Signature {
            factory: false,
            ret: self.ty(operation.return_type.as_ref()),
            params,
            raises,
        }
    }
}

/// What the type rules read of a home beyond [`HomeFacts`].
#[derive(Default)]
struct HomeInfo {
    component: Option<Id<InterfaceNode>>,
    key: Option<Ref<TypeNode>>,
    operations: Vec<Id<OperationNode>>,
    features: Vec<Id<zerodds_ast::AttributeNode>>,
}

/// Where the part of a composition a rule is about is written.
fn subject_span(entry: &CompositionEntry, subject: Subject) -> Span {
    let pick = |spans: &[Span], index: usize| spans.get(index).copied().unwrap_or(entry.span);
    match subject {
        Subject::Composition => entry.span,
        Subject::BindsTo => entry.binds_to.as_ref().map_or(entry.span, |u| u.span),
        Subject::Segment(index) => pick(&entry.segment_spans, index),
        Subject::SegmentFacet(segment, facet) => entry
            .segment_facet_spans
            .get(segment)
            .and_then(|spans| spans.get(facet))
            .copied()
            .unwrap_or(entry.span),
        Subject::FeatureDelegation(index) => pick(&entry.feature_spans, index),
        Subject::StorageHomeDelegation(index) => pick(&entry.storage_delegation_spans, index),
        Subject::ExecutorDelegation(index) => pick(&entry.executor_delegation_spans, index),
        Subject::AbstractOperation(index) => pick(&entry.abstract_spans, index),
        Subject::ProxyDelegation(index) => pick(&entry.proxy_delegation_spans, index),
        Subject::ProxyAbstractOperation(index) => pick(&entry.proxy_abstract_spans, index),
    }
}

/// The identifiers of a node, in order, the escape removed.
fn idents_of(node: &Arc<SyntaxNode>) -> Vec<String> {
    node.syntax_tokens()
        .iter()
        .filter(|(_, token)| token.kind() == TokenKind::Ident)
        .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
        .collect()
}

/// The first identifier of a node and where it stands.
fn first_ident_span(node: &Arc<SyntaxNode>) -> Option<(String, Span)> {
    node.syntax_tokens()
        .into_iter()
        .find(|(_, token)| token.kind() == TokenKind::Ident)
        .map(|(span, token)| {
            (
                zerodds_idl_cst::identifier_name(token.text()).to_string(),
                span,
            )
        })
}

/// The names of the `Declarator` children of a node.
fn declarator_names(node: &Arc<SyntaxNode>) -> Vec<String> {
    declarators_with_spans(node).0
}

fn declarators_with_spans(node: &Arc<SyntaxNode>) -> (Vec<String>, Vec<Span>) {
    node.children()
        .into_iter()
        .filter(|child| child.kind() == CstKind::Declarator)
        .filter_map(|child| first_ident(&child).map(|name| (name, child.span())))
        .unzip()
}

/// The `Delegation` children of a delegation list.
fn delegations(node: &Arc<SyntaxNode>) -> (Vec<Delegation>, Vec<Span>) {
    node.children()
        .into_iter()
        .filter(|child| child.kind() == CstKind::Delegation)
        .filter_map(|child| {
            let mut idents = idents_of(&child).into_iter();
            let operation = idents.next()?;
            Some((
                Delegation {
                    operation,
                    target: idents.next(),
                },
                child.span(),
            ))
        })
        .unzip()
}

fn is_type_node(kind: CstKind) -> bool {
    matches!(
        kind,
        CstKind::PrimitiveType
            | CstKind::NamedType
            | CstKind::SequenceType
            | CstKind::StringType
            | CstKind::WStringType
            | CstKind::MapType
            | CstKind::FixedType
    )
}

/// A node's text as written, each run of white space one space.
fn syntax_text(node: &Arc<SyntaxNode>) -> String {
    node.text().split_whitespace().collect::<Vec<_>>().join(" ")
}
