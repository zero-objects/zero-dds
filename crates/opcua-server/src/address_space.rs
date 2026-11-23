// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A small in-memory OPC-UA AddressSpace the [`crate::server::Server`] serves:
//! the Part-3 §5 Node attributes (for the Read and Write services), a browsable
//! node/reference graph (for Browse/BrowseNext) and method handlers (for the
//! Call service).
//!
//! Method handlers are where an OPC-UA client's `Call` reaches application
//! logic — e.g. the PubSub `AddConnection` / `GetSecurityKeys` binding points
//! from `zerodds-opcua-pubsub` can be registered here.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant, VariantValue};
use zerodds_opcua_gateway::historical::adapter::{
    HistoricalSample, HistoricalSampleAdapter, HistoryReadError, InMemoryHistoryCache,
    ReadHistoryQuery, ReadHistoryResult,
};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};

use crate::attributes::{
    ClassAttributes, VariableAttributes, access_level, attribute_ids, write_mask, write_mask_bit,
};
use crate::status;

/// Well-known ReferenceType NodeIds (ns0, Part 3 §7) used for browsing.
pub mod reference_types {
    use zerodds_opcua_gateway::node_id::NodeId;
    /// `References` — the root of the reference type hierarchy.
    pub const REFERENCES: NodeId = NodeId::numeric(0, 31);
    /// `HierarchicalReferences`.
    pub const HIERARCHICAL_REFERENCES: NodeId = NodeId::numeric(0, 33);
    /// `HasChild`.
    pub const HAS_CHILD: NodeId = NodeId::numeric(0, 34);
    /// `Organizes`.
    pub const ORGANIZES: NodeId = NodeId::numeric(0, 35);
    /// `HasSubtype`.
    pub const HAS_SUBTYPE: NodeId = NodeId::numeric(0, 45);
    /// `HasProperty`.
    pub const HAS_PROPERTY: NodeId = NodeId::numeric(0, 46);
    /// `HasComponent`.
    pub const HAS_COMPONENT: NodeId = NodeId::numeric(0, 47);
    /// `HasOrderedComponent` (Part 3 §7.8), a subtype of `HasComponent`:
    /// the components have an order, the one they are browsed in.
    pub const HAS_ORDERED_COMPONENT: NodeId = NodeId::numeric(0, 49);
    /// `HasTypeDefinition` (non-hierarchical).
    pub const HAS_TYPE_DEFINITION: NodeId = NodeId::numeric(0, 40);
}

/// OPC-UA `NodeClass` (Part 3 §4 / the Browse `nodeClassMask` bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeClass {
    /// Unspecified (`0`).
    Unspecified,
    /// `Object` (`1`).
    Object,
    /// `Variable` (`2`).
    Variable,
    /// `Method` (`4`).
    Method,
    /// `ObjectType` (`8`).
    ObjectType,
    /// `VariableType` (`16`).
    VariableType,
    /// `ReferenceType` (`32`).
    ReferenceType,
    /// `DataType` (`64`).
    DataType,
    /// `View` (`128`).
    View,
}

impl NodeClass {
    /// The bit value used on the wire and in the `nodeClassMask`.
    #[must_use]
    pub const fn bit(self) -> u32 {
        match self {
            Self::Unspecified => 0,
            Self::Object => 1,
            Self::Variable => 2,
            Self::Method => 4,
            Self::ObjectType => 8,
            Self::VariableType => 16,
            Self::ReferenceType => 32,
            Self::DataType => 64,
            Self::View => 128,
        }
    }

    /// The `NodeClass` enum value as encoded in a `ReferenceDescription`.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.bit() as i32
    }
}

/// A node and its Part-3 §5 attributes: the base attributes every NodeClass
/// carries plus the [`ClassAttributes`] of its own class, and the
/// TypeDefinition the Browse service reports.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeMeta {
    /// The node's NodeId.
    pub node_id: NodeId,
    /// Its NodeClass.
    pub node_class: NodeClass,
    /// BrowseName.
    pub browse_name: QualifiedName,
    /// DisplayName.
    pub display_name: LocalizedText,
    /// Description (Part 3 §5.2.5, optional — empty when not set).
    pub description: LocalizedText,
    /// WriteMask (Part 3 §5.2.7) — which attributes a Write may change.
    pub write_mask: u32,
    /// UserWriteMask (Part 3 §5.2.8) — never wider than `write_mask`.
    pub user_write_mask: u32,
    /// The NodeClass-specific attributes.
    pub class_attributes: ClassAttributes,
    /// TypeDefinition (null NodeId when not applicable).
    pub type_definition: NodeId,
}

impl NodeMeta {
    /// A node with the base attributes set and the default attributes of its
    /// class; no TypeDefinition, no Description, nothing writeable.
    #[must_use]
    pub fn new(node_id: NodeId, node_class: NodeClass, browse_name: QualifiedName) -> Self {
        let display_name = LocalizedText {
            locale: None,
            text: Some(browse_name.name.clone()),
        };
        Self {
            node_id,
            node_class,
            browse_name,
            display_name,
            description: LocalizedText {
                locale: None,
                text: None,
            },
            write_mask: 0,
            user_write_mask: 0,
            class_attributes: default_class_attributes(node_class),
            type_definition: NodeId::numeric(0, 0),
        }
    }

    /// Sets the DisplayName.
    #[must_use]
    pub fn with_display_name(mut self, display_name: LocalizedText) -> Self {
        self.display_name = display_name;
        self
    }

    /// Sets the Description.
    #[must_use]
    pub fn with_description(mut self, description: LocalizedText) -> Self {
        self.description = description;
        self
    }

    /// Sets the TypeDefinition reported by Browse.
    #[must_use]
    pub fn with_type_definition(mut self, type_definition: NodeId) -> Self {
        self.type_definition = type_definition;
        self
    }

    /// Sets WriteMask and UserWriteMask to the same value (Part 3 §5.2.8: the
    /// UserWriteMask can only further restrict the WriteMask).
    #[must_use]
    pub fn with_write_mask(mut self, mask: u32) -> Self {
        self.write_mask = mask;
        self.user_write_mask = mask;
        self
    }

    /// Replaces the NodeClass-specific attributes.
    #[must_use]
    pub fn with_class_attributes(mut self, class_attributes: ClassAttributes) -> Self {
        self.class_attributes = class_attributes;
        self
    }
}

/// The default [`ClassAttributes`] for a NodeClass — `Variable` for the
/// `Unspecified` placeholder, because a node whose class was never declared is
/// only ever reached through its Value.
fn default_class_attributes(node_class: NodeClass) -> ClassAttributes {
    match node_class {
        NodeClass::Object | NodeClass::Unspecified => ClassAttributes::Object { event_notifier: 0 },
        NodeClass::Variable => ClassAttributes::Variable(VariableAttributes::default()),
        NodeClass::Method => {
            ClassAttributes::Method(crate::attributes::MethodAttributes::default())
        }
        NodeClass::ObjectType => ClassAttributes::ObjectType { is_abstract: false },
        NodeClass::VariableType => {
            ClassAttributes::VariableType(crate::attributes::VariableTypeAttributes::default())
        }
        NodeClass::ReferenceType => {
            ClassAttributes::ReferenceType(crate::attributes::ReferenceTypeAttributes::default())
        }
        NodeClass::DataType => {
            ClassAttributes::DataType(crate::attributes::DataTypeAttributes::default())
        }
        NodeClass::View => ClassAttributes::View(crate::attributes::ViewAttributes::default()),
    }
}

/// A reference between two nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceRecord {
    /// Source node.
    pub source: NodeId,
    /// ReferenceType.
    pub reference_type: NodeId,
    /// `true` for a forward reference.
    pub is_forward: bool,
    /// Target node.
    pub target: NodeId,
}

/// One browse hit: a reference and the resolved target metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseMatch {
    /// The reference type traversed.
    pub reference_type: NodeId,
    /// Direction as seen from the browsed node.
    pub is_forward: bool,
    /// The target node's metadata (synthesised if the target is unknown).
    pub target: NodeMeta,
}

/// The result of a method handler (Part 4 §5.12 `CallMethodResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct MethodOutcome {
    /// Overall method status code (`0` = Good).
    pub status_code: u32,
    /// Output argument values.
    pub output_arguments: Vec<Variant>,
}

impl MethodOutcome {
    /// A `Good` outcome with the given outputs.
    #[must_use]
    pub fn good(output_arguments: Vec<Variant>) -> Self {
        Self {
            status_code: 0,
            output_arguments,
        }
    }

    /// A failed outcome with the given StatusCode and no outputs.
    #[must_use]
    pub fn fault(status_code: u32) -> Self {
        Self {
            status_code,
            output_arguments: Vec::new(),
        }
    }
}

type MethodHandler = Box<dyn Fn(&MethodContext, &[Variant]) -> MethodOutcome + Send + Sync>;

/// A Method handler that changes the AddressSpace (a Method that adds or
/// removes nodes, OPC 10000-14 §8.5.2).
type MutMethodHandler =
    Box<dyn Fn(&mut AddressSpace, &MethodContext, &[Variant]) -> MethodOutcome + Send + Sync>;

/// One input or output argument of a Method (Part 3 §8.6 `Argument`), as
/// the `InputArguments` and `OutputArguments` Properties of the Method
/// name it (Part 3 §5.7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// `Name`.
    pub name: String,
    /// `DataType`: the NodeId of the argument's DataType.
    pub data_type: NodeId,
    /// `ValueRank`: `-1` a scalar, `1` a one-dimensional array.
    pub value_rank: i32,
}

impl Argument {
    /// A scalar argument.
    #[must_use]
    pub fn scalar(name: &str, data_type: NodeId) -> Self {
        Self {
            name: name.into(),
            data_type,
            value_rank: -1,
        }
    }

    /// A one-dimensional array argument.
    #[must_use]
    pub fn array(name: &str, data_type: NodeId) -> Self {
        Self {
            name: name.into(),
            data_type,
            value_rank: 1,
        }
    }

    /// The `Argument` structure as an ExtensionObject in its default
    /// binary encoding (`Argument_Encoding_DefaultBinary`, `i=298`):
    /// `Name`, `DataType`, `ValueRank`, `ArrayDimensions` (none),
    /// `Description` (none).
    ///
    /// # Errors
    /// A name longer than an `Int32` length.
    pub fn to_extension_object(
        &self,
    ) -> Result<zerodds_opcua_gateway::data_value::ExtensionObject, zerodds_opcua_pubsub::EncodeError>
    {
        use zerodds_opcua_pubsub::UaEncode;
        let mut w = zerodds_opcua_pubsub::UaWriter::with_capacity(32);
        crate::wire::write_string(&mut w, &self.name)?;
        self.data_type.encode(&mut w)?;
        w.write_i32(self.value_rank);
        // ArrayDimensions: the null array.
        w.write_i32(-1);
        // Description: a LocalizedText with neither locale nor text.
        w.write_u8(0);
        Ok(zerodds_opcua_gateway::data_value::ExtensionObject {
            type_id: NodeId::numeric(0, 298),
            body: zerodds_opcua_gateway::data_value::ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }
}

/// What a method handler knows of the call besides its arguments (Part 4
/// §5.12.2): the identity of the calling session and the security of its
/// SecureChannel. Methods whose access depends on them (Part 14 §8.3.2
/// `GetSecurityKeys`: "Encryption is required for this Method") read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodContext {
    /// The user of the calling session.
    pub user: crate::identity::ClientUserId,
    /// The SecureChannel signs its messages.
    pub signed: bool,
    /// The SecureChannel encrypts its messages.
    pub encrypted: bool,
}

impl MethodContext {
    /// An anonymous call over a channel without security.
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            user: crate::identity::ClientUserId::Anonymous,
            signed: false,
            encrypted: false,
        }
    }
}

/// An in-memory AddressSpace: node values + method handlers + a browsable
/// node/reference graph.
#[derive(Default)]
pub struct AddressSpace {
    values: Vec<(NodeId, DataValue)>,
    methods: Vec<(NodeId, MethodHandler)>,
    methods_mut: Vec<(NodeId, MutMethodHandler)>,
    nodes: Vec<NodeMeta>,
    references: Vec<ReferenceRecord>,
    /// `(node, max_depth)` — Spec §9.3.4.4's historizing nodes and the
    /// retention bound each keeps (`HistoryQosValidator`'s counterpart on
    /// the server side).
    historizing: Vec<(NodeId, usize)>,
    /// One shared cache, streams keyed by [`NodeId::to_canonical_string`] —
    /// simpler than one `InMemoryHistoryCache` per node, and the cache
    /// itself is already designed to hold many named streams.
    history: InMemoryHistoryCache,
}

impl core::fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AddressSpace")
            .field("values", &self.values.len())
            .field("methods", &(self.methods.len() + self.methods_mut.len()))
            .field("nodes", &self.nodes.len())
            .field("references", &self.references.len())
            .field("historizing", &self.historizing.len())
            .finish()
    }
}

/// `true` if `reference` is `base` or (transitively, for the well-known set) a
/// subtype of `base` — used for Browse `includeSubtypes`.
fn is_reference_subtype_of(reference: &NodeId, base: &NodeId) -> bool {
    use reference_types::{
        HAS_CHILD, HAS_COMPONENT, HAS_ORDERED_COMPONENT, HAS_PROPERTY, HAS_SUBTYPE,
        HIERARCHICAL_REFERENCES, ORGANIZES, REFERENCES,
    };
    if reference == base || *base == REFERENCES {
        return true;
    }
    if *base == HIERARCHICAL_REFERENCES {
        return [
            ORGANIZES,
            HAS_CHILD,
            HAS_COMPONENT,
            HAS_ORDERED_COMPONENT,
            HAS_PROPERTY,
            HAS_SUBTYPE,
        ]
        .iter()
        .any(|n| n == reference)
            || *reference == HIERARCHICAL_REFERENCES;
    }
    if *base == HAS_CHILD {
        return [
            HAS_COMPONENT,
            HAS_ORDERED_COMPONENT,
            HAS_PROPERTY,
            HAS_SUBTYPE,
        ]
        .iter()
        .any(|n| n == reference);
    }
    if *base == HAS_COMPONENT {
        return *reference == HAS_ORDERED_COMPONENT;
    }
    false
}

impl AddressSpace {
    /// Creates an empty AddressSpace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets (or replaces) a node's `Value` attribute, declaring the node as a
    /// `Variable` with the default Part-3 §5.6 attributes if it is not yet
    /// known — a node that has a Value is a Variable, and the other Attribute
    /// Service operations need its metadata to answer at all.
    ///
    /// Spec §9.3.4.4: if `node_id` was marked historizing
    /// ([`Self::set_historizing`]), this value is also appended to its
    /// bounded history, so a later HistoryRead sees it — the write path is
    /// the only place a historizing node's Value changes, so this is the
    /// one place that has to remember to retain it.
    pub fn set_value(&mut self, node_id: NodeId, value: DataValue) -> &mut Self {
        if self.node_meta(&node_id).is_none() {
            let name = QualifiedName {
                namespace_index: node_id.namespace_index,
                name: String::new(),
            };
            self.add_node(NodeMeta::new(node_id.clone(), NodeClass::Variable, name));
        }
        if let Some(&(_, max_depth)) = self.historizing.iter().find(|(n, _)| *n == node_id) {
            self.history.push_bounded(
                node_id.to_canonical_string(),
                HistoricalSample {
                    source_timestamp: value.source_timestamp.unwrap_or(0),
                    value: value.clone(),
                },
                max_depth,
            );
        }
        if let Some(slot) = self.values.iter_mut().find(|(n, _)| *n == node_id) {
            slot.1 = value;
        } else {
            self.values.push((node_id, value));
        }
        self
    }

    /// Marks `node_id` historizing (Spec §9.3.4.4): `Historizing = true`,
    /// `AccessLevel` must separately carry the `HistoryRead` bit (the
    /// caller's job when it builds the node's [`ClassAttributes`] — see
    /// `zerodds_opcua_gateway::historical::config::HistoricalNodeConfig`),
    /// and every subsequent [`Self::set_value`] on it retains up to
    /// `max_depth` samples for [`Self::read_history`]. Idempotent: calling
    /// it again on the same node replaces `max_depth`, never doubles the
    /// bookkeeping entry.
    pub fn set_historizing(&mut self, node_id: NodeId, max_depth: usize) -> &mut Self {
        // Touches the cache's own stream so a HistoryRead against a
        // historizing-but-still-empty node answers Good/empty, not
        // UnknownInstance — that status is reserved for a node that was
        // never declared historizing at all (checked one layer up, in
        // `read_history`).
        self.history.ensure_stream(node_id.to_canonical_string());
        if let Some(slot) = self.historizing.iter_mut().find(|(n, _)| *n == node_id) {
            slot.1 = max_depth;
        } else {
            self.historizing.push((node_id, max_depth));
        }
        self
    }

    /// Whether `node_id` is historizing.
    #[must_use]
    pub fn is_historizing(&self, node_id: &NodeId) -> bool {
        self.historizing.iter().any(|(n, _)| n == node_id)
    }

    /// Executes a HistoryRead query (Part 4 §5.11.3) against `node_id`'s
    /// retained history.
    ///
    /// # Errors
    /// [`HistoryReadError::UnknownInstance`] if `node_id` is not historizing
    /// (never answered as "no samples" — a client asking a non-historizing
    /// node for history gets told so, not an empty result indistinguishable
    /// from "historizing but currently empty"); [`HistoryReadError::InvalidTimeRange`]
    /// per [`ReadHistoryQuery`]'s own rules.
    pub fn read_history(
        &self,
        node_id: &NodeId,
        query: &ReadHistoryQuery,
    ) -> Result<ReadHistoryResult, HistoryReadError> {
        if !self.is_historizing(node_id) {
            return Err(HistoryReadError::UnknownInstance);
        }
        self.history
            .read_history(&node_id.to_canonical_string(), query)
    }

    /// Returns a node's `Value` attribute, if present.
    #[must_use]
    pub fn value(&self, node_id: &NodeId) -> Option<&DataValue> {
        self.values
            .iter()
            .find(|(n, _)| n == node_id)
            .map(|(_, v)| v)
    }

    /// Registers (or replaces) a method handler for `method_id`.
    pub fn register_method<F>(&mut self, method_id: NodeId, handler: F) -> &mut Self
    where
        F: Fn(&[Variant]) -> MethodOutcome + Send + Sync + 'static,
    {
        self.register_method_with_context(method_id, move |_, args| handler(args))
    }

    /// Registers (or replaces) a method handler for `method_id` that is
    /// given the [`MethodContext`] of each call.
    pub fn register_method_with_context<F>(&mut self, method_id: NodeId, handler: F) -> &mut Self
    where
        F: Fn(&MethodContext, &[Variant]) -> MethodOutcome + Send + Sync + 'static,
    {
        let boxed: MethodHandler = Box::new(handler);
        if let Some(slot) = self.methods.iter_mut().find(|(n, _)| *n == method_id) {
            slot.1 = boxed;
        } else {
            self.methods.push((method_id, boxed));
        }
        self
    }

    /// Adds the Method `method_id`, a component of `object`, with its
    /// `InputArguments` and `OutputArguments` Properties (Part 3 §5.7.1;
    /// a list without arguments has no Property), and registers `handler`
    /// for it. The Properties are Variables of `Argument[]`, NodeIds in the
    /// namespace of the Method with the suffixes `/InputArguments` and
    /// `/OutputArguments` (a numeric Method id gets string ids
    /// `i=<id>/InputArguments`).
    pub fn add_method<F>(
        &mut self,
        object: &NodeId,
        method_id: NodeId,
        browse_name: QualifiedName,
        inputs: &[Argument],
        outputs: &[Argument],
        handler: F,
    ) -> &mut Self
    where
        F: Fn(&MethodContext, &[Variant]) -> MethodOutcome + Send + Sync + 'static,
    {
        self.add_method_node(object, &method_id, browse_name, inputs, outputs);
        self.register_method_with_context(method_id, handler)
    }

    /// As [`Self::add_method`], for a handler that changes the AddressSpace
    /// it is called in ([`Self::call_mut`]).
    pub fn add_method_mut<F>(
        &mut self,
        object: &NodeId,
        method_id: NodeId,
        browse_name: QualifiedName,
        inputs: &[Argument],
        outputs: &[Argument],
        handler: F,
    ) -> &mut Self
    where
        F: Fn(&mut AddressSpace, &MethodContext, &[Variant]) -> MethodOutcome
            + Send
            + Sync
            + 'static,
    {
        self.add_method_node(object, &method_id, browse_name, inputs, outputs);
        self.methods.retain(|(n, _)| *n != method_id);
        let boxed: MutMethodHandler = Box::new(handler);
        if let Some(slot) = self.methods_mut.iter_mut().find(|(n, _)| *n == method_id) {
            slot.1 = boxed;
        } else {
            self.methods_mut.push((method_id, boxed));
        }
        self
    }

    /// The Method node `method_id`, a component of `object`, with its
    /// `InputArguments` and `OutputArguments` Properties.
    fn add_method_node(
        &mut self,
        object: &NodeId,
        method_id: &NodeId,
        browse_name: QualifiedName,
        inputs: &[Argument],
        outputs: &[Argument],
    ) {
        let display = LocalizedText {
            locale: None,
            text: Some(browse_name.name.clone()),
        };
        self.add_node(
            NodeMeta::new(method_id.clone(), NodeClass::Method, browse_name)
                .with_display_name(display)
                .with_class_attributes(ClassAttributes::Method(
                    crate::attributes::MethodAttributes {
                        executable: true,
                        user_executable: true,
                    },
                )),
        );
        self.add_reference(
            object.clone(),
            reference_types::HAS_COMPONENT,
            method_id.clone(),
        );
        for (name, arguments) in [("InputArguments", inputs), ("OutputArguments", outputs)] {
            if arguments.is_empty() {
                continue;
            }
            let values: Vec<VariantValue> = arguments
                .iter()
                .filter_map(|a| a.to_extension_object().ok())
                .map(VariantValue::ExtensionObject)
                .collect();
            let property = NodeId::string(
                method_id.namespace_index,
                alloc::format!("{}/{name}", method_id.to_canonical_string()),
            )
            .unwrap_or_else(|_| NodeId::numeric(method_id.namespace_index, 0));
            self.add_node(
                NodeMeta::new(
                    property.clone(),
                    NodeClass::Variable,
                    QualifiedName {
                        namespace_index: 0,
                        name: name.into(),
                    },
                )
                .with_display_name(LocalizedText {
                    locale: None,
                    text: Some(name.into()),
                })
                .with_type_definition(NodeId::numeric(0, 68)),
            );
            self.set_value(
                property.clone(),
                DataValue::new_value(
                    Variant {
                        array_dimensions: alloc::vec![
                            u32::try_from(values.len()).unwrap_or(u32::MAX)
                        ],
                        value: values,
                    },
                    0,
                    0,
                ),
            );
            self.add_reference(method_id.clone(), reference_types::HAS_PROPERTY, property);
        }
    }

    /// Invokes a registered method, or `None` if no handler exists for it.
    #[must_use]
    pub fn call(&self, method_id: &NodeId, input_arguments: &[Variant]) -> Option<MethodOutcome> {
        self.call_with_context(&MethodContext::anonymous(), method_id, input_arguments)
    }

    /// Invokes a registered method for the call described by `context`.
    #[must_use]
    pub fn call_with_context(
        &self,
        context: &MethodContext,
        method_id: &NodeId,
        input_arguments: &[Variant],
    ) -> Option<MethodOutcome> {
        self.methods
            .iter()
            .find(|(n, _)| n == method_id)
            .map(|(_, h)| h(context, input_arguments))
    }

    /// Calls a Method (Part 4 §5.12.2) as [`Self::call_with_context`] does,
    /// and also a Method of [`Self::add_method_mut`], whose handler may
    /// change this AddressSpace. A handler that removed its own Method's
    /// node is not kept.
    pub fn call_mut(
        &mut self,
        context: &MethodContext,
        method_id: &NodeId,
        input_arguments: &[Variant],
    ) -> Option<MethodOutcome> {
        if let Some(outcome) = self.call_with_context(context, method_id, input_arguments) {
            return Some(outcome);
        }
        let index = self.methods_mut.iter().position(|(n, _)| n == method_id)?;
        let (id, handler) = self.methods_mut.swap_remove(index);
        let outcome = handler(self, context, input_arguments);
        if self.node_meta(&id).is_some() && !self.methods_mut.iter().any(|(n, _)| *n == id) {
            self.methods_mut.push((id, handler));
        }
        Some(outcome)
    }

    /// Removes `node_id` and, recursively, every node it has as a
    /// component, an ordered component or a Property or organizes, with
    /// their values, their Methods and every reference from or to them
    /// (OPC 10000-14 §8.5.5: "removes recursively all contained ...
    /// Objects"). Returns the nodes removed.
    pub fn remove_subtree(&mut self, node_id: &NodeId) -> Vec<NodeId> {
        use reference_types::{HAS_COMPONENT, HAS_ORDERED_COMPONENT, HAS_PROPERTY, ORGANIZES};
        let mut doomed = alloc::vec![node_id.clone()];
        let mut next = 0;
        while next < doomed.len() {
            let current = doomed[next].clone();
            next += 1;
            for r in &self.references {
                if r.source == current
                    && [
                        HAS_COMPONENT,
                        HAS_ORDERED_COMPONENT,
                        HAS_PROPERTY,
                        ORGANIZES,
                    ]
                    .contains(&r.reference_type)
                    && !doomed.contains(&r.target)
                {
                    doomed.push(r.target.clone());
                }
            }
        }
        self.nodes.retain(|n| !doomed.contains(&n.node_id));
        self.values.retain(|(n, _)| !doomed.contains(n));
        self.methods.retain(|(n, _)| !doomed.contains(n));
        self.methods_mut.retain(|(n, _)| !doomed.contains(n));
        self.historizing.retain(|(n, _)| !doomed.contains(n));
        self.references
            .retain(|r| !doomed.contains(&r.source) && !doomed.contains(&r.target));
        doomed
    }

    /// Adds the `Server` Object (ns0;i=2253, Part 5 §8.3.2), organized by
    /// the `Objects` folder, with its `ServerArray` (i=2254) and
    /// `NamespaceArray` (i=2255) Properties (Part 5 §6.3.1): the server
    /// table is this server's ApplicationUri, the namespace table the OPC UA
    /// namespace, the ApplicationUri, then `namespaces` from index 2 on.
    /// The other components of `ServerType` are not added here.
    pub fn ensure_server_object(&mut self, namespaces: &[&str]) -> &mut Self {
        let objects = NodeId::numeric(0, 85);
        let server = NodeId::numeric(0, 2253);
        let qn = |name: &str| QualifiedName {
            namespace_index: 0,
            name: name.into(),
        };
        if self.node_meta(&objects).is_none() {
            self.add_node(NodeMeta::new(
                objects.clone(),
                NodeClass::Object,
                qn("Objects"),
            ));
        }
        if self.node_meta(&server).is_none() {
            let server_type = NodeId::numeric(0, 2004);
            self.add_node(
                NodeMeta::new(server.clone(), NodeClass::Object, qn("Server"))
                    .with_type_definition(server_type.clone()),
            );
            self.add_reference(objects, reference_types::ORGANIZES, server.clone());
            self.add_reference(
                server.clone(),
                reference_types::HAS_TYPE_DEFINITION,
                server_type,
            );
        }
        let mut table: Vec<String> = alloc::vec![
            String::from("http://opcfoundation.org/UA/"),
            String::from(crate::server::APPLICATION_URI),
        ];
        table.extend(namespaces.iter().map(|uri| String::from(*uri)));
        for (id, name, values) in [
            (
                2254,
                "ServerArray",
                alloc::vec![String::from(crate::server::APPLICATION_URI)],
            ),
            (2255, "NamespaceArray", table),
        ] {
            let node = NodeId::numeric(0, id);
            let property_type = NodeId::numeric(0, 68);
            if self.node_meta(&node).is_none() {
                self.add_node(
                    NodeMeta::new(node.clone(), NodeClass::Variable, qn(name))
                        .with_type_definition(property_type.clone())
                        .with_class_attributes(ClassAttributes::Variable(VariableAttributes {
                            data_type: NodeId::numeric(0, 12),
                            value_rank: 1,
                            array_dimensions: alloc::vec![0],
                            access_level: access_level::CURRENT_READ,
                            user_access_level: access_level::CURRENT_READ,
                            access_level_ex: u32::from(access_level::CURRENT_READ),
                            ..VariableAttributes::default()
                        })),
                );
                self.add_reference(server.clone(), reference_types::HAS_PROPERTY, node.clone());
                self.add_reference(
                    node.clone(),
                    reference_types::HAS_TYPE_DEFINITION,
                    property_type,
                );
            }
            let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
            self.set_value(
                node,
                DataValue::new_value(
                    Variant {
                        array_dimensions: alloc::vec![count],
                        value: values.into_iter().map(VariantValue::String).collect(),
                    },
                    0,
                    0,
                ),
            );
        }
        self
    }

    /// Adds (or replaces) a node's browse metadata.
    pub fn add_node(&mut self, meta: NodeMeta) -> &mut Self {
        if let Some(slot) = self.nodes.iter_mut().find(|n| n.node_id == meta.node_id) {
            *slot = meta;
        } else {
            self.nodes.push(meta);
        }
        self
    }

    /// Adds a forward reference `source --reference_type--> target` (the inverse
    /// is implied and returned by an inverse browse).
    pub fn add_reference(
        &mut self,
        source: NodeId,
        reference_type: NodeId,
        target: NodeId,
    ) -> &mut Self {
        self.references.push(ReferenceRecord {
            source,
            reference_type,
            is_forward: true,
            target,
        });
        self
    }

    /// Removes the reference `source --reference_type--> target`.
    pub fn remove_reference(
        &mut self,
        source: &NodeId,
        reference_type: &NodeId,
        target: &NodeId,
    ) -> &mut Self {
        self.references.retain(|r| {
            !(r.source == *source && r.reference_type == *reference_type && r.target == *target)
        });
        self
    }

    /// Returns a node's browse metadata, if present.
    #[must_use]
    pub fn node_meta(&self, node_id: &NodeId) -> Option<&NodeMeta> {
        self.nodes.iter().find(|n| n.node_id == *node_id)
    }

    /// Browses `node_id` (Part 4 §5.9.2): returns the matching references and
    /// their resolved target metadata.
    ///
    /// `direction` is `0` Forward / `1` Inverse / `2` Both. With a
    /// `reference_type` filter and `include_subtypes`, subtypes of the
    /// well-known hierarchical reference types are matched too.
    #[must_use]
    pub fn browse(
        &self,
        node_id: &NodeId,
        direction: i32,
        reference_type: Option<&NodeId>,
        include_subtypes: bool,
        node_class_mask: u32,
    ) -> Vec<BrowseMatch> {
        let want_forward = direction == 0 || direction == 2;
        let want_inverse = direction == 1 || direction == 2;
        let mut out = Vec::new();
        for r in &self.references {
            // A stored reference is forward (source→target). It surfaces on a
            // forward browse of its source and an inverse browse of its target.
            let (matches_node, is_forward) = if r.source == *node_id {
                (want_forward, true)
            } else if r.target == *node_id {
                (want_inverse, false)
            } else {
                (false, true)
            };
            if !matches_node {
                continue;
            }
            if let Some(ft) = reference_type {
                let ok = if include_subtypes {
                    is_reference_subtype_of(&r.reference_type, ft)
                } else {
                    r.reference_type == *ft
                };
                if !ok {
                    continue;
                }
            }
            let other = if is_forward { &r.target } else { &r.source };
            let target_meta = self.node_meta(other).cloned().unwrap_or_else(|| {
                NodeMeta::new(
                    other.clone(),
                    NodeClass::Unspecified,
                    QualifiedName {
                        namespace_index: 0,
                        name: String::new(),
                    },
                )
                .with_display_name(LocalizedText {
                    locale: None,
                    text: None,
                })
            });
            if node_class_mask != 0 && (target_meta.node_class.bit() & node_class_mask) == 0 {
                continue;
            }
            out.push(BrowseMatch {
                reference_type: r.reference_type.clone(),
                is_forward,
                target: target_meta,
            });
        }
        out
    }

    // -----------------------------------------------------------------------
    // Attribute Service Set (Part 4 §5.11) — the full Part-3 §5 attribute set,
    // not only `Value`.
    // -----------------------------------------------------------------------

    /// Reads one attribute of one node (Part 4 §5.11.2).
    ///
    /// # Errors
    /// `Bad_NodeIdUnknown` for a node this AddressSpace does not hold,
    /// `Bad_NodeIdInvalid` for an attribute id outside Part 6 Table A.1, and
    /// `Bad_AttributeIdInvalid` for an attribute the node's NodeClass does not
    /// carry (Part 4 §5.11.2.4).
    pub fn read_attribute(&self, node_id: &NodeId, attribute_id: u32) -> Result<DataValue, u32> {
        use attribute_ids as a;
        let meta = self.node_meta(node_id).ok_or(status::BAD_NODE_ID_UNKNOWN)?;
        if attribute_id == 0 || attribute_id > a::MAX {
            return Err(status::BAD_ATTRIBUTE_ID_INVALID);
        }
        // The Value attribute is the only one whose StatusCode can be Bad
        // while the read itself succeeds, so it is answered first and whole.
        if attribute_id == a::VALUE {
            return self.read_value_attribute(meta, node_id);
        }
        let value = match attribute_id {
            a::NODE_ID => VariantValue::NodeId(meta.node_id.clone()),
            a::NODE_CLASS => VariantValue::Int32(meta.node_class.as_i32()),
            a::BROWSE_NAME => VariantValue::QualifiedName(meta.browse_name.clone()),
            a::DISPLAY_NAME => VariantValue::LocalizedText(meta.display_name.clone()),
            a::DESCRIPTION => VariantValue::LocalizedText(meta.description.clone()),
            a::WRITE_MASK => VariantValue::UInt32(meta.write_mask),
            a::USER_WRITE_MASK => VariantValue::UInt32(meta.user_write_mask),
            other => return read_class_attribute(&meta.class_attributes, other),
        };
        Ok(good_value(Variant::scalar(value)))
    }

    fn read_value_attribute(&self, meta: &NodeMeta, node_id: &NodeId) -> Result<DataValue, u32> {
        match &meta.class_attributes {
            ClassAttributes::Variable(v) => {
                if v.access_level & access_level::CURRENT_READ == 0 {
                    return Ok(status_only(status::BAD_NOT_READABLE));
                }
            }
            // Part 3 §5.7: a VariableType's Value is optional.
            ClassAttributes::VariableType(_) => {}
            _ => return Err(status::BAD_ATTRIBUTE_ID_INVALID),
        }
        match self.value(node_id) {
            Some(dv) => Ok(dv.clone()),
            // Part 4 Table 179 Bad_WaitingForInitialData: "This status is also
            // used if a Variable is created and no default value or no initial
            // value is available."
            None => Ok(status_only(status::BAD_WAITING_FOR_INITIAL_DATA)),
        }
    }

    /// Writes one attribute of one node (Part 4 §5.11.4), honouring the
    /// `WriteMask` (Part 3 §8.60) and, for a Variable's `Value`, the
    /// `AccessLevel` bit `CurrentWrite` (Part 3 §8.57). Returns the operation
    /// StatusCode.
    #[allow(clippy::too_many_lines)]
    pub fn write_attribute(
        &mut self,
        node_id: &NodeId,
        attribute_id: u32,
        value: DataValue,
    ) -> u32 {
        use attribute_ids as a;
        let Some(meta) = self.node_meta(node_id) else {
            return status::BAD_NODE_ID_UNKNOWN;
        };
        if attribute_id == 0 || attribute_id > a::MAX {
            return status::BAD_ATTRIBUTE_ID_INVALID;
        }
        let node_class = meta.node_class;
        let write_mask = meta.write_mask & meta.user_write_mask;

        if attribute_id == a::VALUE {
            return self.write_value_attribute(node_id, write_mask, value);
        }
        // Every other attribute is gated by its WriteMask bit.
        let Some(bit) = write_mask_bit(attribute_id) else {
            return status::BAD_ATTRIBUTE_ID_INVALID;
        };
        if write_mask & bit == 0 {
            return status::BAD_NOT_WRITABLE;
        }
        let Some(variant) = value.value else {
            return status::BAD_TYPE_MISMATCH;
        };
        let Some(scalar) = variant.value.first().cloned() else {
            return status::BAD_TYPE_MISMATCH;
        };
        let Some(meta) = self.nodes.iter_mut().find(|n| n.node_id == *node_id) else {
            return status::BAD_NODE_ID_UNKNOWN;
        };
        match (attribute_id, scalar) {
            (a::BROWSE_NAME, VariantValue::QualifiedName(q)) => meta.browse_name = q,
            (a::DISPLAY_NAME, VariantValue::LocalizedText(t)) => meta.display_name = t,
            (a::DESCRIPTION, VariantValue::LocalizedText(t)) => meta.description = t,
            (a::WRITE_MASK, VariantValue::UInt32(m)) => meta.write_mask = m,
            (a::USER_WRITE_MASK, VariantValue::UInt32(m)) => meta.user_write_mask = m,
            (a::NODE_ID | a::NODE_CLASS, _) => {
                // Both bits exist in Table 44, but re-identifying a live node
                // would silently invalidate every reference pointing at it.
                return status::BAD_WRITE_NOT_SUPPORTED;
            }
            // A base attribute written with the wrong builtin type is
            // Bad_TypeMismatch (Part 4 §5.11.4.4), not a missing attribute.
            (
                a::BROWSE_NAME
                | a::DISPLAY_NAME
                | a::DESCRIPTION
                | a::WRITE_MASK
                | a::USER_WRITE_MASK,
                _,
            ) => return status::BAD_TYPE_MISMATCH,
            (other, scalar) => {
                return write_class_attribute(
                    &mut meta.class_attributes,
                    node_class,
                    other,
                    scalar,
                );
            }
        }
        status::GOOD
    }

    fn write_value_attribute(
        &mut self,
        node_id: &NodeId,
        write_mask: u32,
        value: DataValue,
    ) -> u32 {
        let Some(meta) = self.node_meta(node_id) else {
            return status::BAD_NODE_ID_UNKNOWN;
        };
        match &meta.class_attributes {
            ClassAttributes::Variable(v) => {
                if v.access_level & access_level::CURRENT_WRITE == 0
                    || v.user_access_level & access_level::CURRENT_WRITE == 0
                {
                    return status::BAD_NOT_WRITABLE;
                }
                // Part 4 §5.11.4.2: "If the SourceTimestamp or the
                // ServerTimestamp is specified, the Server shall use these
                // values. The Server returns a Bad_WriteNotSupported error if
                // it does not support writing of timestamps." A zero-tick
                // timestamp is the unset DateTime, not a specified one.
                let writes_timestamp = value.source_timestamp.is_some_and(|t| t != 0)
                    || value.server_timestamp.is_some_and(|t| t != 0);
                if writes_timestamp && v.access_level & access_level::TIMESTAMP_WRITE == 0 {
                    return status::BAD_WRITE_NOT_SUPPORTED;
                }
                let writes_status = value.status.is_some_and(|s| s != status::GOOD);
                if writes_status && v.access_level & access_level::STATUS_WRITE == 0 {
                    return status::BAD_WRITE_NOT_SUPPORTED;
                }
            }
            ClassAttributes::VariableType(_) => {
                if write_mask & write_mask::VALUE_FOR_VARIABLE_TYPE == 0 {
                    return status::BAD_NOT_WRITABLE;
                }
            }
            _ => return status::BAD_ATTRIBUTE_ID_INVALID,
        }
        self.set_value(node_id.clone(), value);
        status::GOOD
    }
}

/// A `DataValue` carrying `value` with StatusCode `Good`.
fn good_value(value: Variant) -> DataValue {
    DataValue {
        value: Some(value),
        status: Some(status::GOOD),
        source_timestamp: None,
        server_timestamp: None,
        source_pico_sec: None,
        server_pico_sec: None,
    }
}

/// A `DataValue` carrying only a StatusCode (Part 4 §7.11.5).
fn status_only(status_code: u32) -> DataValue {
    DataValue {
        value: None,
        status: Some(status_code),
        source_timestamp: None,
        server_timestamp: None,
        source_pico_sec: None,
        server_pico_sec: None,
    }
}

/// Reads a NodeClass-specific attribute (Part 3 §5.3 – §5.8).
fn read_class_attribute(class: &ClassAttributes, attribute_id: u32) -> Result<DataValue, u32> {
    use attribute_ids as a;
    let value = match (class, attribute_id) {
        (ClassAttributes::Object { event_notifier }, a::EVENT_NOTIFIER) => {
            VariantValue::Byte(*event_notifier)
        }
        (ClassAttributes::View(v), a::EVENT_NOTIFIER) => VariantValue::Byte(v.event_notifier),
        (ClassAttributes::View(v), a::CONTAINS_NO_LOOPS) => {
            VariantValue::Boolean(v.contains_no_loops)
        }
        (ClassAttributes::Variable(v), a::DATA_TYPE) => VariantValue::NodeId(v.data_type.clone()),
        (ClassAttributes::Variable(v), a::VALUE_RANK) => VariantValue::Int32(v.value_rank),
        (ClassAttributes::Variable(v), a::ARRAY_DIMENSIONS) => {
            return Ok(good_value(u32_array(&v.array_dimensions)));
        }
        (ClassAttributes::Variable(v), a::ACCESS_LEVEL) => VariantValue::Byte(v.access_level),
        (ClassAttributes::Variable(v), a::USER_ACCESS_LEVEL) => {
            VariantValue::Byte(v.user_access_level)
        }
        (ClassAttributes::Variable(v), a::MINIMUM_SAMPLING_INTERVAL) => {
            VariantValue::Double(v.minimum_sampling_interval)
        }
        (ClassAttributes::Variable(v), a::HISTORIZING) => VariantValue::Boolean(v.historizing),
        (ClassAttributes::Variable(v), a::ACCESS_LEVEL_EX) => {
            VariantValue::UInt32(v.access_level_ex)
        }
        (ClassAttributes::VariableType(v), a::DATA_TYPE) => {
            VariantValue::NodeId(v.data_type.clone())
        }
        (ClassAttributes::VariableType(v), a::VALUE_RANK) => VariantValue::Int32(v.value_rank),
        (ClassAttributes::VariableType(v), a::ARRAY_DIMENSIONS) => {
            return Ok(good_value(u32_array(&v.array_dimensions)));
        }
        (ClassAttributes::VariableType(v), a::IS_ABSTRACT) => VariantValue::Boolean(v.is_abstract),
        (ClassAttributes::ObjectType { is_abstract }, a::IS_ABSTRACT) => {
            VariantValue::Boolean(*is_abstract)
        }
        (ClassAttributes::Method(m), a::EXECUTABLE) => VariantValue::Boolean(m.executable),
        (ClassAttributes::Method(m), a::USER_EXECUTABLE) => {
            VariantValue::Boolean(m.user_executable)
        }
        (ClassAttributes::ReferenceType(r), a::IS_ABSTRACT) => VariantValue::Boolean(r.is_abstract),
        (ClassAttributes::ReferenceType(r), a::SYMMETRIC) => VariantValue::Boolean(r.symmetric),
        (ClassAttributes::ReferenceType(r), a::INVERSE_NAME) => {
            // Part 3 §5.3.4: the InverseName is absent for symmetric and for
            // abstract reference types.
            let Some(name) = r.inverse_name.clone() else {
                return Err(status::BAD_ATTRIBUTE_ID_INVALID);
            };
            VariantValue::LocalizedText(name)
        }
        (ClassAttributes::DataType(d), a::IS_ABSTRACT) => VariantValue::Boolean(d.is_abstract),
        (ClassAttributes::DataType(d), a::DATA_TYPE_DEFINITION) => {
            let Some(def) = d.data_type_definition.clone() else {
                return Err(status::BAD_ATTRIBUTE_ID_INVALID);
            };
            VariantValue::ExtensionObject(def)
        }
        _ => return Err(status::BAD_ATTRIBUTE_ID_INVALID),
    };
    Ok(good_value(Variant::scalar(value)))
}

/// Writes a NodeClass-specific attribute; the caller has already checked the
/// WriteMask bit.
fn write_class_attribute(
    class: &mut ClassAttributes,
    node_class: NodeClass,
    attribute_id: u32,
    scalar: VariantValue,
) -> u32 {
    use attribute_ids as a;
    let _ = node_class;
    match (class, attribute_id, scalar) {
        (ClassAttributes::Object { event_notifier }, a::EVENT_NOTIFIER, VariantValue::Byte(b)) => {
            *event_notifier = b;
        }
        (ClassAttributes::View(v), a::EVENT_NOTIFIER, VariantValue::Byte(b)) => {
            v.event_notifier = b;
        }
        (ClassAttributes::View(v), a::CONTAINS_NO_LOOPS, VariantValue::Boolean(b)) => {
            v.contains_no_loops = b;
        }
        (ClassAttributes::Variable(v), a::DATA_TYPE, VariantValue::NodeId(n)) => v.data_type = n,
        (ClassAttributes::Variable(v), a::VALUE_RANK, VariantValue::Int32(r)) => v.value_rank = r,
        (ClassAttributes::Variable(v), a::ACCESS_LEVEL, VariantValue::Byte(b)) => {
            v.access_level = b;
        }
        (ClassAttributes::Variable(v), a::USER_ACCESS_LEVEL, VariantValue::Byte(b)) => {
            v.user_access_level = b;
        }
        (ClassAttributes::Variable(v), a::MINIMUM_SAMPLING_INTERVAL, VariantValue::Double(d)) => {
            v.minimum_sampling_interval = d;
        }
        (ClassAttributes::Variable(v), a::HISTORIZING, VariantValue::Boolean(b)) => {
            v.historizing = b;
        }
        (ClassAttributes::Variable(v), a::ACCESS_LEVEL_EX, VariantValue::UInt32(x)) => {
            v.access_level_ex = x;
        }
        (ClassAttributes::VariableType(v), a::DATA_TYPE, VariantValue::NodeId(n)) => {
            v.data_type = n;
        }
        (ClassAttributes::VariableType(v), a::VALUE_RANK, VariantValue::Int32(r)) => {
            v.value_rank = r;
        }
        (ClassAttributes::VariableType(v), a::IS_ABSTRACT, VariantValue::Boolean(b)) => {
            v.is_abstract = b;
        }
        (ClassAttributes::ObjectType { is_abstract }, a::IS_ABSTRACT, VariantValue::Boolean(b)) => {
            *is_abstract = b;
        }
        (ClassAttributes::Method(m), a::EXECUTABLE, VariantValue::Boolean(b)) => m.executable = b,
        (ClassAttributes::Method(m), a::USER_EXECUTABLE, VariantValue::Boolean(b)) => {
            m.user_executable = b;
        }
        (ClassAttributes::ReferenceType(r), a::IS_ABSTRACT, VariantValue::Boolean(b)) => {
            r.is_abstract = b;
        }
        (ClassAttributes::ReferenceType(r), a::SYMMETRIC, VariantValue::Boolean(b)) => {
            r.symmetric = b;
        }
        (ClassAttributes::ReferenceType(r), a::INVERSE_NAME, VariantValue::LocalizedText(t)) => {
            r.inverse_name = Some(t);
        }
        (ClassAttributes::DataType(d), a::IS_ABSTRACT, VariantValue::Boolean(b)) => {
            d.is_abstract = b;
        }
        (
            ClassAttributes::DataType(d),
            a::DATA_TYPE_DEFINITION,
            VariantValue::ExtensionObject(eo),
        ) => d.data_type_definition = Some(eo),
        // The attribute exists on this class but the written value has the
        // wrong builtin type (Part 4 §5.11.4.4 Bad_TypeMismatch); anything
        // else is an attribute this class does not carry.
        (class, attribute_id, _) => {
            return if read_class_attribute(class, attribute_id).is_ok() {
                status::BAD_TYPE_MISMATCH
            } else {
                status::BAD_ATTRIBUTE_ID_INVALID
            };
        }
    }
    status::GOOD
}

/// The `ArrayDimensions` attribute value: a `UInt32[]` Variant.
fn u32_array(dimensions: &[u32]) -> Variant {
    Variant {
        array_dimensions: alloc::vec![u32::try_from(dimensions.len()).unwrap_or(u32::MAX)],
        value: dimensions
            .iter()
            .copied()
            .map(VariantValue::UInt32)
            .collect(),
    }
}

#[cfg(test)]
mod tests {

    /// A Method whose handler changes the AddressSpace (OPC 10000-14
    /// §8.5.2): it adds a node in the call; a removal takes the nodes under
    /// the removed one and their references.
    #[test]
    fn uacs_5_12_2_a_method_adds_and_removes_nodes() {
        let mut space = AddressSpace::new();
        let object = NodeId::numeric(1, 1);
        space.add_node(NodeMeta::new(
            object.clone(),
            NodeClass::Object,
            QualifiedName {
                namespace_index: 1,
                name: "Folder".into(),
            },
        ));
        let method = NodeId::numeric(1, 2);
        space.add_method_mut(
            &object,
            method.clone(),
            QualifiedName {
                namespace_index: 1,
                name: "Add".into(),
            },
            &[],
            &[],
            |space, _, _| {
                let child = NodeId::numeric(1, 3);
                space.add_node(NodeMeta::new(
                    child.clone(),
                    NodeClass::Object,
                    QualifiedName {
                        namespace_index: 1,
                        name: "Child".into(),
                    },
                ));
                space.add_reference(NodeId::numeric(1, 1), reference_types::HAS_COMPONENT, child);
                MethodOutcome::good(Vec::new())
            },
        );
        let outcome = space
            .call_mut(&MethodContext::anonymous(), &method, &[])
            .expect("method");
        assert_eq!(outcome.status_code, 0);
        assert!(space.node_meta(&NodeId::numeric(1, 3)).is_some());
        // The handler stays for the next call.
        assert!(
            space
                .call_mut(&MethodContext::anonymous(), &method, &[])
                .is_some()
        );
        let removed = space.remove_subtree(&object);
        assert!(removed.contains(&NodeId::numeric(1, 3)));
        assert!(removed.contains(&method));
        assert!(space.node_meta(&NodeId::numeric(1, 3)).is_none());
        assert!(
            space
                .call_mut(&MethodContext::anonymous(), &method, &[])
                .is_none()
        );
        assert!(space.browse(&object, 2, None, false, 0).is_empty());
    }

    /// OPC 10000-5 §8.3.2 and §6.3.1: the Server Object under Objects, its
    /// NamespaceArray with the OPC UA namespace, the ApplicationUri and the
    /// namespaces of the application, its ServerArray the ApplicationUri.
    #[test]
    fn uacs_5_8_3_2_the_server_object_names_the_namespaces() {
        let mut space = AddressSpace::new();
        space.ensure_server_object(&["urn:example:app"]);
        let table = space
            .read_attribute(&NodeId::numeric(0, 2255), attribute_ids::VALUE)
            .expect("read")
            .value
            .expect("value");
        assert_eq!(
            table.value,
            alloc::vec![
                VariantValue::String("http://opcfoundation.org/UA/".into()),
                VariantValue::String(crate::server::APPLICATION_URI.into()),
                VariantValue::String("urn:example:app".into()),
            ]
        );
        let servers = space
            .read_attribute(&NodeId::numeric(0, 2254), attribute_ids::VALUE)
            .expect("read")
            .value
            .expect("value");
        assert_eq!(
            servers.value,
            alloc::vec![VariantValue::String(crate::server::APPLICATION_URI.into())]
        );
        assert!(
            space
                .browse(&NodeId::numeric(0, 85), 0, None, false, 0)
                .iter()
                .any(|m| m.target.node_id == NodeId::numeric(0, 2253))
        );
    }

    use super::*;
    use zerodds_opcua_gateway::data_value::VariantValue;

    #[test]
    fn values_set_get_replace() {
        let mut a = AddressSpace::new();
        let n = NodeId::numeric(1, 1);
        a.set_value(
            n.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(1)), 0, 0),
        );
        assert_eq!(
            a.value(&n).unwrap().value,
            Some(Variant::scalar(VariantValue::Int32(1)))
        );
        a.set_value(
            n.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(2)), 0, 0),
        );
        assert_eq!(
            a.value(&n).unwrap().value,
            Some(Variant::scalar(VariantValue::Int32(2)))
        );
        assert!(a.value(&NodeId::numeric(1, 99)).is_none());
    }

    #[test]
    fn uacs_5_12_2_method_call_dispatch() {
        let mut a = AddressSpace::new();
        let m = NodeId::numeric(1, 100);
        a.register_method(m.clone(), |args| {
            // Echo: double the first Int32 input.
            if let Some(Variant { value, .. }) = args.first()
                && let Some(VariantValue::Int32(x)) = value.first()
            {
                return MethodOutcome::good(alloc::vec![Variant::scalar(VariantValue::Int32(
                    x * 2
                ))]);
            }
            MethodOutcome::fault(0x8000_0000)
        });
        let out = a
            .call(&m, &[Variant::scalar(VariantValue::Int32(21))])
            .expect("method");
        assert_eq!(out.status_code, 0);
        assert_eq!(
            out.output_arguments[0],
            Variant::scalar(VariantValue::Int32(42))
        );
        assert!(a.call(&NodeId::numeric(1, 7), &[]).is_none());
    }

    fn qn(name: &str) -> QualifiedName {
        QualifiedName {
            namespace_index: 1,
            name: alloc::string::String::from(name),
        }
    }

    fn graph() -> AddressSpace {
        use reference_types::{HAS_COMPONENT, HAS_TYPE_DEFINITION, ORGANIZES};
        let mut a = AddressSpace::new();
        let objects = NodeId::numeric(0, 85);
        let boiler = NodeId::numeric(1, 1);
        let temp = NodeId::numeric(1, 2);
        for (id, class, name) in [
            (objects.clone(), NodeClass::Object, "Objects"),
            (boiler.clone(), NodeClass::Object, "Boiler"),
            (temp.clone(), NodeClass::Variable, "Temperature"),
        ] {
            a.add_node(
                NodeMeta::new(id, class, qn(name)).with_type_definition(NodeId::numeric(0, 58)),
            );
        }
        a.add_reference(objects, ORGANIZES, boiler.clone());
        a.add_reference(boiler.clone(), HAS_COMPONENT, temp);
        a.add_reference(boiler, HAS_TYPE_DEFINITION, NodeId::numeric(0, 58));
        a
    }

    #[test]
    fn uacs_5_9_2_browse_forward_and_inverse() {
        let a = graph();
        let boiler = NodeId::numeric(1, 1);
        // Forward, all references → HasComponent(Temp) + HasTypeDefinition(58).
        let fwd = a.browse(&boiler, 0, None, false, 0);
        assert_eq!(fwd.len(), 2);
        // Inverse → Organizes from Objects(85).
        let inv = a.browse(&boiler, 1, None, false, 0);
        assert_eq!(inv.len(), 1);
        assert_eq!(inv[0].target.node_id, NodeId::numeric(0, 85));
        assert!(!inv[0].is_forward);
    }

    #[test]
    fn uacs_5_9_2_browse_subtype_filter_excludes_non_hierarchical() {
        let a = graph();
        let boiler = NodeId::numeric(1, 1);
        // HierarchicalReferences + subtypes → HasComponent only (not the
        // non-hierarchical HasTypeDefinition).
        let hits = a.browse(
            &boiler,
            0,
            Some(&reference_types::HIERARCHICAL_REFERENCES),
            true,
            0,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].reference_type, reference_types::HAS_COMPONENT);
        assert_eq!(hits[0].target.node_id, NodeId::numeric(1, 2));
    }

    #[test]
    fn uacs_5_9_2_browse_node_class_mask_filters_targets() {
        let a = graph();
        let boiler = NodeId::numeric(1, 1);
        // Only Variables (mask 2) → just Temperature.
        let vars = a.browse(&boiler, 0, None, false, NodeClass::Variable.bit());
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].target.node_class, NodeClass::Variable);
    }
}
