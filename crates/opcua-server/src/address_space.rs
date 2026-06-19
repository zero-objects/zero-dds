// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A small in-memory OPC-UA AddressSpace the [`crate::server::Server`] serves:
//! node `Value` attributes (for the Read service) and method handlers (for the
//! Call service).
//!
//! Method handlers are where an OPC-UA client's `Call` reaches application
//! logic — e.g. the PubSub `AddConnection` / `GetSecurityKeys` binding points
//! from `zerodds-opcua-pubsub` can be registered here.

use alloc::boxed::Box;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};

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

/// A node's browse-relevant metadata (Part 3 attributes the Browse service
/// returns).
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
    /// TypeDefinition (null NodeId when not applicable).
    pub type_definition: NodeId,
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

type MethodHandler = Box<dyn Fn(&[Variant]) -> MethodOutcome + Send + Sync>;

/// An in-memory AddressSpace: node values + method handlers + a browsable
/// node/reference graph.
#[derive(Default)]
pub struct AddressSpace {
    values: Vec<(NodeId, DataValue)>,
    methods: Vec<(NodeId, MethodHandler)>,
    nodes: Vec<NodeMeta>,
    references: Vec<ReferenceRecord>,
}

impl core::fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AddressSpace")
            .field("values", &self.values.len())
            .field("methods", &self.methods.len())
            .field("nodes", &self.nodes.len())
            .field("references", &self.references.len())
            .finish()
    }
}

/// `true` if `reference` is `base` or (transitively, for the well-known set) a
/// subtype of `base` — used for Browse `includeSubtypes`.
fn is_reference_subtype_of(reference: &NodeId, base: &NodeId) -> bool {
    use reference_types::{
        HAS_CHILD, HAS_COMPONENT, HAS_PROPERTY, HAS_SUBTYPE, HIERARCHICAL_REFERENCES, ORGANIZES,
        REFERENCES,
    };
    if reference == base || *base == REFERENCES {
        return true;
    }
    if *base == HIERARCHICAL_REFERENCES {
        return [
            ORGANIZES,
            HAS_CHILD,
            HAS_COMPONENT,
            HAS_PROPERTY,
            HAS_SUBTYPE,
        ]
        .iter()
        .any(|n| n == reference)
            || *reference == HIERARCHICAL_REFERENCES;
    }
    if *base == HAS_CHILD {
        return [HAS_COMPONENT, HAS_PROPERTY, HAS_SUBTYPE]
            .iter()
            .any(|n| n == reference);
    }
    false
}

impl AddressSpace {
    /// Creates an empty AddressSpace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets (or replaces) a node's `Value` attribute.
    pub fn set_value(&mut self, node_id: NodeId, value: DataValue) -> &mut Self {
        if let Some(slot) = self.values.iter_mut().find(|(n, _)| *n == node_id) {
            slot.1 = value;
        } else {
            self.values.push((node_id, value));
        }
        self
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
        let boxed: MethodHandler = Box::new(handler);
        if let Some(slot) = self.methods.iter_mut().find(|(n, _)| *n == method_id) {
            slot.1 = boxed;
        } else {
            self.methods.push((method_id, boxed));
        }
        self
    }

    /// Invokes a registered method, or `None` if no handler exists for it.
    #[must_use]
    pub fn call(&self, method_id: &NodeId, input_arguments: &[Variant]) -> Option<MethodOutcome> {
        self.methods
            .iter()
            .find(|(n, _)| n == method_id)
            .map(|(_, h)| h(input_arguments))
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
            let target_meta = self.node_meta(other).cloned().unwrap_or_else(|| NodeMeta {
                node_id: other.clone(),
                node_class: NodeClass::Unspecified,
                browse_name: QualifiedName {
                    namespace_index: 0,
                    name: alloc::string::String::new(),
                },
                display_name: LocalizedText {
                    locale: None,
                    text: None,
                },
                type_definition: NodeId::numeric(0, 0),
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
}

#[cfg(test)]
mod tests {
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
    fn method_call_dispatch() {
        let mut a = AddressSpace::new();
        let m = NodeId::numeric(1, 100);
        a.register_method(m.clone(), |args| {
            // Echo: double the first Int32 input.
            if let Some(Variant { value, .. }) = args.first() {
                if let Some(VariantValue::Int32(x)) = value.first() {
                    return MethodOutcome::good(alloc::vec![Variant::scalar(VariantValue::Int32(
                        x * 2
                    ))]);
                }
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
            a.add_node(NodeMeta {
                node_id: id,
                node_class: class,
                browse_name: qn(name),
                display_name: LocalizedText {
                    locale: None,
                    text: Some(alloc::string::String::from(name)),
                },
                type_definition: NodeId::numeric(0, 58),
            });
        }
        a.add_reference(objects, ORGANIZES, boiler.clone());
        a.add_reference(boiler.clone(), HAS_COMPONENT, temp);
        a.add_reference(boiler, HAS_TYPE_DEFINITION, NodeId::numeric(0, 58));
        a
    }

    #[test]
    fn browse_forward_and_inverse() {
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
    fn browse_subtype_filter_excludes_non_hierarchical() {
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
    fn browse_node_class_mask_filters_targets() {
        let a = graph();
        let boiler = NodeId::numeric(1, 1);
        // Only Variables (mask 2) → just Temperature.
        let vars = a.browse(&boiler, 0, None, false, NodeClass::Variable.bit());
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].target.node_class, NodeClass::Variable);
    }
}
