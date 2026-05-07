// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AddressSpace-Mapping DDS ↔ OPC UA — Spec §9.3.

use alloc::string::String;

use crate::node_id::NodeId;
use crate::types::{NodeClass, QualifiedName};

/// Spec §9.3.2 — DDS `Domain` → OPC-UA `ObjectNode` mit BrowseName
/// `Domain<id>` und Numeric-NodeId im DDS-Namespace (Index 1 nach
/// Convention).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainNode {
    /// `node_id` der Domain-Object-Node.
    pub node_id: NodeId,
    /// `browse_name` (Spec: "Domain<id>").
    pub browse_name: QualifiedName,
    /// DDS-Domain-ID.
    pub domain_id: u32,
}

impl DomainNode {
    /// Spec §9.3.2 — konstruiert eine Domain-Object-Node mit BrowseName
    /// `"Domain<id>"`.
    #[must_use]
    pub fn for_domain(namespace_index: u16, domain_id: u32) -> Self {
        Self {
            // Spec — ObjectNode-NodeId-Generation ist Vendor-defined.
            // Wir nutzen NUMERIC mit Domain-ID als Identifier.
            node_id: NodeId::numeric(namespace_index, domain_id),
            browse_name: QualifiedName {
                namespace_index,
                name: alloc::format!("Domain{domain_id}"),
            },
            domain_id,
        }
    }
}

/// Spec §9.3.3 — DDS `Topic` → OPC-UA `ObjectNode` unter dem Domain-
/// Parent. BrowseName ist der Topic-Name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicNode {
    /// NodeId der Topic-Object-Node.
    pub node_id: NodeId,
    /// BrowseName = Topic-Name (Spec).
    pub browse_name: QualifiedName,
    /// Topic-Type-Name (DDS-IDL-Type).
    pub topic_type: String,
    /// Parent-Domain-Node (Cross-Reference).
    pub parent_domain_id: u32,
}

/// Spec §9.3.3 — Sanitization: DDS-Topic-Names koennen Charactere
/// enthalten, die in OPC-UA-BrowseNames nicht erlaubt sind. Wir
/// passen das Mapping nicht an (Spec sagt: BrowseName = Topic-Name);
/// Caller muss Topic-Names mit Spec-konformen Zeichen vergeben.
///
/// Diese Funktion liefert den Topic-Namen 1:1 als BrowseName-Komponente
/// und ist nur als ergonomischer Wrapper gedacht.
#[must_use]
pub fn mangle_topic_node_browse_name(topic_name: &str, namespace_index: u16) -> QualifiedName {
    QualifiedName {
        namespace_index,
        name: String::from(topic_name),
    }
}

/// Spec §9.3.4 — DDS-Sample → OPC-UA-Variable mit DataType-NodeId
/// + DataValue-Snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleVariable {
    /// NodeId der Variable.
    pub node_id: NodeId,
    /// BrowseName.
    pub browse_name: QualifiedName,
    /// `node_class` ist immer `Variable` (Spec §9.3.4).
    pub node_class: NodeClass,
    /// `data_type` NodeId (verweist auf den DDS-Topic-Type).
    pub data_type: NodeId,
    /// `value_rank` — Spec OPCUA-03: -2 = Any, -1 = Scalar, 0 =
    /// OneOrMoreDimensions, n>0 = explicit-N-D.
    pub value_rank: i32,
}

impl SampleVariable {
    /// Spec §9.3.4 — typischer DDS-Sample-Mapping: Scalar (`value_rank
    /// = -1`).
    #[must_use]
    pub fn scalar(node_id: NodeId, browse_name: QualifiedName, data_type: NodeId) -> Self {
        Self {
            node_id,
            browse_name,
            node_class: NodeClass::Variable,
            data_type,
            value_rank: -1,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn domain_node_for_zero_yields_browse_name_domain0() {
        // Spec §9.3.2.
        let d = DomainNode::for_domain(2, 0);
        assert_eq!(d.domain_id, 0);
        assert_eq!(d.browse_name.name, "Domain0");
        assert_eq!(d.browse_name.namespace_index, 2);
        assert!(matches!(
            d.node_id.identifier_type,
            crate::node_id::NodeIdentifier::Numeric(0)
        ));
    }

    #[test]
    fn domain_node_for_42_yields_browse_name_domain42() {
        let d = DomainNode::for_domain(2, 42);
        assert_eq!(d.browse_name.name, "Domain42");
    }

    #[test]
    fn mangle_topic_node_browse_name_is_pass_through() {
        // Spec §9.3.3 — BrowseName = Topic-Name (1:1).
        let q = mangle_topic_node_browse_name("Temperature", 2);
        assert_eq!(q.name, "Temperature");
        assert_eq!(q.namespace_index, 2);
    }

    #[test]
    fn topic_node_carries_parent_domain_reference() {
        let t = TopicNode {
            node_id: NodeId::string(2, "TempTopic").expect("ok"),
            browse_name: mangle_topic_node_browse_name("Temperature", 2),
            topic_type: String::from("TemperatureSample"),
            parent_domain_id: 0,
        };
        assert_eq!(t.parent_domain_id, 0);
        assert_eq!(t.topic_type, "TemperatureSample");
    }

    #[test]
    fn sample_variable_scalar_has_value_rank_minus_1() {
        // Spec OPCUA-03 — value_rank=-1 = scalar.
        let nid = NodeId::numeric(2, 100);
        let dt = NodeId::numeric(2, 200);
        let v = SampleVariable::scalar(
            nid.clone(),
            QualifiedName {
                namespace_index: 2,
                name: String::from("Sample"),
            },
            dt.clone(),
        );
        assert_eq!(v.node_class, NodeClass::Variable);
        assert_eq!(v.value_rank, -1);
        assert_eq!(v.node_id, nid);
        assert_eq!(v.data_type, dt);
    }
}
