// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The common AST as a seesaw v2 graph. Spec §3.1.
//!
//! Every node of the model appears as a typed seesaw node; every
//! ownership relation and every reference appears as an anonymous
//! directed link. Facets appear as nodes too, not as attributes, so a
//! rule can match on `@key` being present the same way it matches on a
//! member being present.
//!
//! The adapter is total over `NodeKind`. A node kind with no
//! representation would be a construct no rule could ever match — a
//! silent gap, which is the failure mode this whole layer exists to
//! avoid.

use seesaw_tgg::graph::{Graph, ValueStore};
use seesaw_tgg::ident::{GhostId, Status};
use std::collections::BTreeMap;
use zerodds_ast::{AnyId, Ast, NodeKind, TypeKind};
use zerodds_ast_core::NodeId;

/// A model presented as a seesaw graph, with the map back.
pub struct ModelGraph {
    /// The graph a rule matches over.
    pub graph: Graph,
    /// The values its leaves carry.
    pub values: ValueStore,
    /// Which model node each seesaw node came from.
    by_ghost: BTreeMap<GhostId, NodeId>,
    /// Which seesaw node each model node became.
    by_node: BTreeMap<NodeId, GhostId>,
}

/// One node of the model, as the adapter names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeRef {
    /// The seesaw node.
    pub ghost: GhostId,
    /// The model node it came from.
    pub node: NodeId,
}

impl ModelGraph {
    /// Present `ast` as a graph.
    #[must_use]
    pub fn of(ast: &Ast) -> Self {
        let mut this = Self {
            graph: Graph::new(),
            values: ValueStore::default(),
            by_ghost: BTreeMap::new(),
            by_node: BTreeMap::new(),
        };
        this.add_nodes(ast);
        this.add_links(ast);
        this
    }

    /// The model node a seesaw node came from.
    #[must_use]
    pub fn model_node(&self, ghost: &GhostId) -> Option<NodeId> {
        self.by_ghost.get(ghost).copied()
    }

    /// The seesaw node a model node became.
    #[must_use]
    pub fn ghost(&self, node: NodeId) -> Option<GhostId> {
        self.by_node.get(&node).copied()
    }

    /// Every pairing of seesaw node and model node.
    pub fn pairs(&self) -> impl Iterator<Item = (GhostId, NodeId)> + '_ {
        self.by_ghost.iter().map(|(ghost, node)| (*ghost, *node))
    }

    /// How many nodes the graph holds.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn add_nodes(&mut self, ast: &Ast) {
        // Every node kind, in the order the model iterates them, so two
        // runs over one model produce the same graph.
        for (id, node) in ast.modules() {
            self.add(AnyId::Module(id, node.node_id), type_name_of_module());
        }
        for (id, node) in ast.types() {
            self.add(AnyId::Type(id, node.node_id), type_name_of_type(&node.kind));
            self.add_facets(ast, AnyId::Type(id, node.node_id));
        }
        for (id, node) in ast.members() {
            self.add(AnyId::Member(id, node.node_id), "Member");
            self.add_facets(ast, AnyId::Member(id, node.node_id));
        }
        for (id, node) in ast.enumerators() {
            self.add(AnyId::Enumerator(id, node.node_id), "Enumerator");
            self.add_facets(ast, AnyId::Enumerator(id, node.node_id));
        }
        for (id, node) in ast.interfaces() {
            self.add(AnyId::Interface(id, node.node_id), "Interface");
            self.add_facets(ast, AnyId::Interface(id, node.node_id));
            // An attribute is reachable only from the interface that
            // declares it — the model keeps no arena iterator for one —
            // so it is walked here rather than in a loop of its own.
            for attribute in &node.attributes {
                let Ok(attribute_node) = ast.attribute(*attribute) else {
                    continue;
                };
                let any = AnyId::Attribute(*attribute, attribute_node.node_id);
                let ghost = self.add(any, "Attribute");
                self.add_facets(ast, any);
                // `readonly` is a node and not an attribute value, for
                // the reason `@key` is: a rule matching a read-only
                // attribute is then a two-node pattern rather than a
                // predicate on a field, and "not read-only" is never a
                // pattern.
                if attribute_node.readonly {
                    self.add_flag(ghost, "Readonly");
                }
            }
        }
        // An operation belongs to an interface or to a valuetype, which
        // is why it has an arena of its own and why the walk is over
        // that arena rather than over the interfaces.
        for (id, node) in ast.operations() {
            let any = AnyId::Operation(id, node.node_id);
            let ghost = self.add(any, "Operation");
            self.add_facets(ast, any);
            if node.oneway {
                self.add_flag(ghost, "Oneway");
            }
            for param in &node.params {
                let Ok(param_node) = ast.param(*param) else {
                    continue;
                };
                let any = AnyId::Param(*param, param_node.node_id);
                let param_ghost = self.add(any, "Param");
                self.add_facets(ast, any);
                // Every direction gets a node, unlike the flags above:
                // IDL 4.2 rule (87) makes the direction mandatory, so
                // there is no default whose absence could carry meaning.
                self.add_flag(param_ghost, direction_name(param_node.direction));
            }
        }
        for (id, node) in ast.cases() {
            self.add(AnyId::Case(id, node.node_id), "Case");
        }
        for (id, node) in ast.bit_values() {
            self.add(AnyId::BitValue(id, node.node_id), "BitValue");
        }
        for (id, node) in ast.constants() {
            self.add(AnyId::Const(id, node.node_id), "Const");
        }
        for (id, node) in ast.exceptions() {
            self.add(AnyId::Exception(id, node.node_id), "Exception");
        }
    }

    /// Add one node, keeping both directions of the map.
    fn add(&mut self, any: AnyId, type_name: &str) -> GhostId {
        let node = any.node_id();
        let external = external_name(node);
        let ghost = self.graph.add_baseline(&external, type_name);
        let _ = self.by_ghost.insert(ghost, node);
        let _ = self.by_node.insert(node, ghost);
        ghost
    }

    /// A flag becomes a node hanging off the node it qualifies.
    ///
    /// The same shape [`Self::add_facets`] gives `@key`, for a flag the
    /// model carries as a field rather than in the X-Types facet:
    /// `oneway`, `readonly` and a parameter's direction.
    fn add_flag(&mut self, owner: GhostId, name: &str) {
        let flag = self.graph.add_ghost(&owner, name);
        let _ = self.graph.connect(owner, flag, Status::Solid);
    }

    /// A facet becomes a node linked from the node it describes.
    ///
    /// `@key` on a member is a `Key` node hanging off that member, so a
    /// rule matching a keyed member is a two-node pattern rather than a
    /// value predicate on an attribute.
    fn add_facets(&mut self, ast: &Ast, any: AnyId) {
        let Some(facets) = ast.facets(any) else {
            return;
        };
        let Some(xtypes) = facets.xtypes.as_ref() else {
            return;
        };
        let owner = self.by_node.get(&any.node_id()).copied();
        let Some(owner) = owner else {
            return;
        };

        // One node per flag that a rule can branch on. A flag that is
        // false has no node: the rules are positive, so "not key" is
        // never a pattern, only "key" is.
        for (present, name) in [
            (xtypes.is_key, "Key"),
            (xtypes.is_optional, "Optional"),
            (xtypes.is_external, "External"),
            (xtypes.is_must_understand, "MustUnderstand"),
            (xtypes.is_non_serialized, "NonSerialized"),
            (xtypes.is_nested, "Nested"),
            (xtypes.is_default_literal, "DefaultLiteral"),
        ] {
            if !present {
                continue;
            }
            let facet = self.graph.add_ghost(&owner, name);
            let _ = self.graph.connect(owner, facet, Status::Solid);
        }

        if let Some(extensibility) = xtypes.extensibility {
            let name = match extensibility {
                zerodds_ast::Extensibility::Final => "Final",
                zerodds_ast::Extensibility::Appendable => "Appendable",
                zerodds_ast::Extensibility::Mutable => "Mutable",
            };
            let facet = self.graph.add_ghost(&owner, name);
            let _ = self.graph.connect(owner, facet, Status::Solid);
        }
    }

    /// Ownership and references, as anonymous directed links.
    fn add_links(&mut self, ast: &Ast) {
        for (id, node) in ast.types() {
            let Some(type_ghost) = self.by_node.get(&node.node_id).copied() else {
                continue;
            };
            // A type belongs to its module.
            if let Ok(module) = ast.module(node.module) {
                if let Some(module_ghost) = self.by_node.get(&module.node_id).copied() {
                    let _ = self.graph.connect(module_ghost, type_ghost, Status::Solid);
                }
            }
            // An aggregate owns its members, in declaration order.
            for member in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                if let Some(member_ghost) = self.by_node.get(&member_node.node_id).copied() {
                    let _ = self.graph.connect(type_ghost, member_ghost, Status::Solid);
                }
            }
            // A union owns its cases, a bitmask its bits.
            for case in ast.cases_of(id) {
                let Ok(node) = ast.case(*case) else {
                    continue;
                };
                if let Some(ghost) = self.by_node.get(&node.node_id).copied() {
                    let _ = self.graph.connect(type_ghost, ghost, Status::Solid);
                }
            }
            for bit in ast.bits_of(id) {
                let Ok(node) = ast.bit_value(*bit) else {
                    continue;
                };
                if let Some(ghost) = self.by_node.get(&node.node_id).copied() {
                    let _ = self.graph.connect(type_ghost, ghost, Status::Solid);
                }
            }
            // An enumeration owns its enumerators.
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if let Some(ghost) = self.by_node.get(&node.node_id).copied() {
                    let _ = self.graph.connect(type_ghost, ghost, Status::Solid);
                }
            }
        }

        // An interface belongs to its module, owns its attributes and
        // points at every base it inherits from, in declaration order.
        // Without these links the interface node stands alone in the
        // graph and the `interface-to-…` rules of `rule::{cpp,java,
        // typescript}` match a node with nothing under it.
        for (id, node) in ast.interfaces() {
            let Some(interface_ghost) = self.by_node.get(&node.node_id).copied() else {
                continue;
            };
            if let Ok(module) = ast.module(node.module)
                && let Some(module_ghost) = self.by_node.get(&module.node_id).copied()
            {
                let _ = self
                    .graph
                    .connect(module_ghost, interface_ghost, Status::Solid);
            }
            for base in &node.bases {
                let Some(base_id) = base.resolved() else {
                    continue;
                };
                let Ok(base_node) = ast.interface(base_id) else {
                    continue;
                };
                if let Some(base_ghost) = self.by_node.get(&base_node.node_id).copied() {
                    let _ = self
                        .graph
                        .connect(interface_ghost, base_ghost, Status::Solid);
                }
            }
            for attribute in &node.attributes {
                let Ok(attribute_node) = ast.attribute(*attribute) else {
                    continue;
                };
                if let Some(attribute_ghost) = self.by_node.get(&attribute_node.node_id).copied() {
                    let _ = self
                        .graph
                        .connect(interface_ghost, attribute_ghost, Status::Solid);
                }
                if let Some(target) = attribute_node.type_.resolved()
                    && let Ok(target_node) = ast.type_(target)
                    && let Some(target_ghost) = self.by_node.get(&target_node.node_id).copied()
                    && let Some(attribute_ghost) =
                        self.by_node.get(&attribute_node.node_id).copied()
                {
                    let _ = self
                        .graph
                        .connect(attribute_ghost, target_ghost, Status::Solid);
                }
            }
            let _ = id;
        }

        // An operation belongs to whatever declared it, owns its
        // parameters in declaration order, and points at its return
        // type and at every exception it raises.
        for (_, node) in ast.operations() {
            let Some(operation_ghost) = self.by_node.get(&node.node_id).copied() else {
                continue;
            };
            if let Some(owner_ghost) = self.by_node.get(&node.parent.node_id()).copied() {
                let _ = self
                    .graph
                    .connect(owner_ghost, operation_ghost, Status::Solid);
            }
            if let Some(target) = node.return_type.as_ref().and_then(|r| r.resolved())
                && let Ok(target_node) = ast.type_(target)
                && let Some(target_ghost) = self.by_node.get(&target_node.node_id).copied()
            {
                let _ = self
                    .graph
                    .connect(operation_ghost, target_ghost, Status::Solid);
            }
            for raised in &node.raises {
                let Some(raised_id) = raised.resolved() else {
                    continue;
                };
                let Ok(raised_node) = ast.exception(raised_id) else {
                    continue;
                };
                if let Some(raised_ghost) = self.by_node.get(&raised_node.node_id).copied() {
                    let _ = self
                        .graph
                        .connect(operation_ghost, raised_ghost, Status::Solid);
                }
            }
            for param in &node.params {
                let Ok(param_node) = ast.param(*param) else {
                    continue;
                };
                let Some(param_ghost) = self.by_node.get(&param_node.node_id).copied() else {
                    continue;
                };
                let _ = self
                    .graph
                    .connect(operation_ghost, param_ghost, Status::Solid);
                if let Some(target) = param_node.type_.resolved()
                    && let Ok(target_node) = ast.type_(target)
                    && let Some(target_ghost) = self.by_node.get(&target_node.node_id).copied()
                {
                    let _ = self.graph.connect(param_ghost, target_ghost, Status::Solid);
                }
            }
        }

        // A member points at its type.
        for (_, node) in ast.members() {
            let Some(member_ghost) = self.by_node.get(&node.node_id).copied() else {
                continue;
            };
            let Some(target) = node.type_.resolved() else {
                continue;
            };
            let Ok(target_node) = ast.type_(target) else {
                continue;
            };
            if let Some(target_ghost) = self.by_node.get(&target_node.node_id).copied() {
                let _ = self
                    .graph
                    .connect(member_ghost, target_ghost, Status::Solid);
            }
        }
    }
}

/// The external name a model node takes in the graph.
///
/// The node's own identity, which is stable across snapshots and
/// processes, so a graph built twice from one model is the same graph.
fn external_name(node: NodeId) -> String {
    format!("{}:{}", node.space().as_u32(), node.counter())
}

const fn type_name_of_module() -> &'static str {
    "Module"
}

/// The seesaw node name a parameter's direction takes. IDL 4.2 rule
/// (87).
const fn direction_name(direction: zerodds_ast::ParamDirection) -> &'static str {
    match direction {
        zerodds_ast::ParamDirection::In => "In",
        zerodds_ast::ParamDirection::Out => "Out",
        zerodds_ast::ParamDirection::InOut => "InOut",
    }
}

/// The seesaw type name a model type takes.
///
/// One name per `TypeKind`, with no catch-all: a kind added to the model
/// without a name here is a compile error rather than a type no rule can
/// match.
const fn type_name_of_type(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Primitive(_) => "Primitive",
        TypeKind::Fixed { .. } => "Fixed",
        TypeKind::String { .. } => "StringType",
        TypeKind::Sequence { .. } => "Sequence",
        TypeKind::Array { .. } => "Array",
        TypeKind::Map { .. } => "Map",
        TypeKind::Structure { .. } => "Structure",
        TypeKind::Union { .. } => "Union",
        TypeKind::Enumeration { .. } => "Enumeration",
        TypeKind::Bitmask { .. } => "Bitmask",
        TypeKind::Bitset { .. } => "Bitset",
        TypeKind::Alias { .. } => "Alias",
        TypeKind::Valuetype { .. } => "Valuetype",
        TypeKind::ValueBox { .. } => "ValueBox",
        TypeKind::Native => "Native",
        TypeKind::Any => "Any",
        TypeKind::ValueBase => "ValueBase",
        TypeKind::ObjectRef { .. } => "ObjectRef",
        TypeKind::AnnotationType { .. } => "AnnotationType",
    }
}

/// The seesaw type name a node kind takes.
///
/// Total over `NodeKind` and with no catch-all, which is what §3.1
/// requires: a kind with no name would be a construct no rule can match.
#[must_use]
pub const fn type_name_of_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Module => "Module",
        NodeKind::Type => "Type",
        NodeKind::Member => "Member",
        NodeKind::Case => "Case",
        NodeKind::Enumerator => "Enumerator",
        NodeKind::BitValue => "BitValue",
        NodeKind::Bitfield => "Bitfield",
        NodeKind::Const => "Const",
        NodeKind::ConstExpr => "ConstExpr",
        NodeKind::Interface => "Interface",
        NodeKind::Operation => "Operation",
        NodeKind::Param => "Param",
        NodeKind::Attribute => "Attribute",
        NodeKind::Exception => "Exception",
        NodeKind::AnnotationDef => "AnnotationDef",
        NodeKind::AnnotationMember => "AnnotationMember",
        NodeKind::Domain => "Domain",
        NodeKind::Topic => "Topic",
        NodeKind::QosProfile => "QosProfile",
        NodeKind::Participant => "Participant",
        NodeKind::Endpoint => "Endpoint",
        NodeKind::Library => "Library",
    }
}
