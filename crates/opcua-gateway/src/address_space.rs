// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AddressSpace-Mapping DDS ↔ OPC UA — Spec §9.3.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::dds_to_ua::walker::DdsType;
use crate::node_id::NodeId;
use crate::types::{NodeClass, QualifiedName};

/// Spec §9.3.2 — DDS `Domain` → OPC-UA `ObjectNode` with BrowseName
/// `Domain<id>` and a numeric NodeId in the DDS namespace (index 1 by
/// convention).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainNode {
    /// `node_id` of the domain object node.
    pub node_id: NodeId,
    /// `browse_name` (Spec: "Domain<id>").
    pub browse_name: QualifiedName,
    /// DDS domain id.
    pub domain_id: u32,
}

impl DomainNode {
    /// Spec §9.3.2 — constructs a domain object node with BrowseName
    /// `"Domain<id>"`.
    #[must_use]
    pub fn for_domain(namespace_index: u16, domain_id: u32) -> Self {
        Self {
            // Spec — ObjectNode NodeId generation is vendor-defined.
            // We use NUMERIC with the domain id as the identifier.
            node_id: NodeId::numeric(namespace_index, domain_id),
            browse_name: QualifiedName {
                namespace_index,
                name: alloc::format!("Domain{domain_id}"),
            },
            domain_id,
        }
    }
}

/// Spec §9.3.3 — DDS `Topic` → OPC-UA `ObjectNode` under the domain
/// parent. The BrowseName is the topic name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicNode {
    /// NodeId of the topic object node.
    pub node_id: NodeId,
    /// BrowseName = topic name (Spec).
    pub browse_name: QualifiedName,
    /// Topic type name (DDS IDL type).
    pub topic_type: String,
    /// Parent domain node (cross-reference).
    pub parent_domain_id: u32,
}

/// Spec §9.3.3 — sanitization: DDS topic names can contain characters
/// that are not allowed in OPC-UA BrowseNames. We do
/// not adapt the mapping (the spec says: BrowseName = topic name);
/// the caller must assign topic names with spec-conformant characters.
///
/// This function returns the topic name 1:1 as a BrowseName component
/// and is only intended as an ergonomic wrapper.
#[must_use]
pub fn mangle_topic_node_browse_name(topic_name: &str, namespace_index: u16) -> QualifiedName {
    QualifiedName {
        namespace_index,
        name: String::from(topic_name),
    }
}

/// Spec §9.3.4 — DDS sample → OPC-UA variable with DataType NodeId
/// + DataValue snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleVariable {
    /// NodeId of the variable.
    pub node_id: NodeId,
    /// BrowseName.
    pub browse_name: QualifiedName,
    /// `node_class` is always `Variable` (Spec §9.3.4).
    pub node_class: NodeClass,
    /// `data_type` NodeId (refers to the DDS topic type).
    pub data_type: NodeId,
    /// `value_rank` — Spec OPCUA-03: -2 = Any, -1 = Scalar, 0 =
    /// OneOrMoreDimensions, n>0 = explicit-N-D.
    pub value_rank: i32,
}

impl SampleVariable {
    /// Spec §9.3.4 — typical DDS sample mapping: scalar (`value_rank
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

/// Spec §9.3.4 — recursive instance decomposition of a (structured)
/// DDS sample into an OPC-UA variable hierarchy.
///
/// An **aggregated** sample (struct/union) is not mapped as an opaque
/// scalar variable, but decomposed into browsable **component variables**
/// per member (`HasComponent`), recursively into nested struct/union
/// members. **Collections** (array/sequence/map) become array/object variables
/// (the element structure lives in the §9.2 type layer; the element count is
/// runtime, so no static per-element nodes). This mirrors the
/// §9.2 *type* recursion of the walker on the §9.3 *instance* side.
///
/// Symbolic (BrowseName-based, the caller allocates the NodeIds) — exactly like
/// the walker node specs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceNode {
    /// BrowseName of this variable (member name; at the root node, the sample name).
    pub browse_name: QualifiedName,
    /// Always `Variable`, except the map wrapper (`Object`, Spec Tab 9.40).
    pub node_class: NodeClass,
    /// OPC-UA DataType name (Tab 9.2 builtin or generated `<Name>DataType`).
    pub data_type: String,
    /// `-1` scalar, `1` 1-dim collection, `n` n-dim array (Spec OPCUA-03).
    pub value_rank: i32,
    /// `ArrayDimensions` (for `value_rank >= 1`).
    pub array_dimensions: Vec<u32>,
    /// Spec §9.2.8 — a `@key` member carries `IsKey` on the instance variable.
    pub is_key: bool,
    /// Spec Tab 9.16 — an `@optional` member → ModelingRule "Optional".
    pub is_optional: bool,
    /// `HasComponent` children — recursively decomposed members. Empty for leaves.
    pub components: Vec<InstanceNode>,
}

/// Builds the §9.3.4 instance variable tree for a DDS topic sample of type
/// `ty`, rooted under `browse_name` (typically the topic name).
///
/// A scalar topic type returns a leaf without components (compatible with
/// [`SampleVariable::scalar`]); a structured type returns the full
/// recursive component tree.
#[must_use]
pub fn build_sample_instance(
    browse_name: &str,
    namespace_index: u16,
    ty: &DdsType,
) -> InstanceNode {
    build_member_instance(browse_name, namespace_index, ty, false, false)
}

/// zerodds-lint: recursion-depth 64
///
/// Indirectly recursive (a struct/union member type can itself be a struct/union).
/// Same depth bound as the walker (DDS-XTypes trees are flat in practice).
fn build_member_instance(
    name: &str,
    ns: u16,
    ty: &DdsType,
    is_key: bool,
    is_optional: bool,
) -> InstanceNode {
    let resolved = ty.resolve_alias();
    let qn = QualifiedName {
        namespace_index: ns,
        name: name.to_string(),
    };
    let leaf =
        |data_type: String, value_rank: i32, dims: Vec<u32>, node_class: NodeClass| InstanceNode {
            browse_name: qn.clone(),
            node_class,
            data_type,
            value_rank,
            array_dimensions: dims,
            is_key,
            is_optional,
            components: Vec::new(),
        };

    match resolved {
        DdsType::Struct(s) => {
            let components = s
                .members
                .iter()
                .map(|m| {
                    build_member_instance(&m.name, ns, &m.member_type, m.is_key, m.is_optional)
                })
                .collect();
            InstanceNode {
                components,
                ..leaf(
                    resolved.opc_ua_data_type(),
                    -1,
                    Vec::new(),
                    NodeClass::Variable,
                )
            }
        }
        DdsType::Union(u) => {
            // Spec §9.2.4.2 / Tab 9.16 — SwitchField + one component slot per
            // case. The active arm is runtime; statically all branches are
            // exposed (cases are "Optional", only the active one is present).
            let mut components = alloc::vec![InstanceNode {
                browse_name: QualifiedName {
                    namespace_index: ns,
                    name: "SwitchField".to_string(),
                },
                node_class: NodeClass::Variable,
                data_type: u.discriminator.opc_ua_data_type(),
                value_rank: -1,
                array_dimensions: Vec::new(),
                is_key: false,
                is_optional: false,
                components: Vec::new(),
            }];
            components.extend(
                u.cases
                    .iter()
                    .map(|c| build_member_instance(&c.name, ns, &c.member_type, false, true)),
            );
            InstanceNode {
                components,
                ..leaf(
                    resolved.opc_ua_data_type(),
                    -1,
                    Vec::new(),
                    NodeClass::Variable,
                )
            }
        }
        DdsType::Array {
            element,
            dimensions,
        } => {
            // N-dim array variable (Spec §9.2.5.1). The element count is static,
            // but the elements are the array value, not their own instance nodes.
            let rank = i32::try_from(dimensions.len()).unwrap_or(1).max(1);
            leaf(
                element.opc_ua_data_type(),
                rank,
                dimensions.clone(),
                NodeClass::Variable,
            )
        }
        DdsType::Sequence { element, bound } => {
            // 1-dim array variable; ArrayDimensions = [bound] or [0] (unbounded).
            leaf(
                element.opc_ua_data_type(),
                1,
                alloc::vec![bound.unwrap_or(0)],
                NodeClass::Variable,
            )
        }
        DdsType::Map { value, bound, .. } => {
            // Spec Tab 9.40 — Map → object wrapper over MapEntry; the sample value
            // is the (possibly bounded) entry collection.
            leaf(
                value.opc_ua_data_type(),
                1,
                alloc::vec![bound.unwrap_or(0)],
                NodeClass::Object,
            )
        }
        // Primitive / String / Enum / Bitmask — atomic leaf.
        scalar => leaf(
            scalar.opc_ua_data_type(),
            -1,
            Vec::new(),
            NodeClass::Variable,
        ),
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
        // Spec §9.3.3 — BrowseName = topic name (1:1).
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

    // ---- §9.3.4 recursive instance decomposition ----

    use crate::dds_to_ua::walker::{DdsType, MemberDef, StructDef, UnionCase, UnionDef};

    fn member(name: &str, ty: DdsType) -> MemberDef {
        MemberDef {
            name: name.into(),
            member_type: ty,
            is_optional: false,
            is_key: false,
        }
    }

    fn shape() -> DdsType {
        DdsType::Struct(StructDef {
            name: "ShapeType".into(),
            members: alloc::vec![
                member("color", DdsType::String8),
                member("x", DdsType::Int32),
                member("y", DdsType::Int32),
            ],
        })
    }

    #[test]
    fn scalar_topic_sample_is_a_leaf() {
        let n = build_sample_instance("Temperature", 2, &DdsType::Float64);
        assert_eq!(n.browse_name.name, "Temperature");
        assert_eq!(n.node_class, NodeClass::Variable);
        assert_eq!(n.data_type, "Double");
        assert_eq!(n.value_rank, -1);
        assert!(n.components.is_empty());
    }

    #[test]
    fn struct_sample_decomposes_into_component_variables() {
        let n = build_sample_instance("Shape", 2, &shape());
        assert_eq!(n.data_type, "ShapeTypeDataType");
        // One HasComponent child per member, browse-names preserved + ordered.
        let names: Vec<&str> = n
            .components
            .iter()
            .map(|c| c.browse_name.name.as_str())
            .collect();
        assert_eq!(names, alloc::vec!["color", "x", "y"]);
        assert_eq!(n.components[0].data_type, "String");
        assert_eq!(n.components[1].data_type, "Int32");
    }

    #[test]
    fn nested_struct_sample_recurses_into_components() {
        let inner = DdsType::Struct(StructDef {
            name: "Inner".into(),
            members: alloc::vec![member("v", DdsType::Int32)],
        });
        let outer = DdsType::Struct(StructDef {
            name: "Outer".into(),
            members: alloc::vec![member("inner", inner)],
        });
        let n = build_sample_instance("Sample", 2, &outer);
        assert_eq!(n.components.len(), 1);
        let inner_node = &n.components[0];
        assert_eq!(inner_node.browse_name.name, "inner");
        assert_eq!(inner_node.data_type, "InnerDataType");
        // Recursion: the inner struct exposes its own leaf component.
        assert_eq!(inner_node.components.len(), 1);
        assert_eq!(inner_node.components[0].browse_name.name, "v");
        assert_eq!(inner_node.components[0].data_type, "Int32");
        assert!(inner_node.components[0].components.is_empty());
    }

    #[test]
    fn array_member_is_an_array_variable() {
        let s = DdsType::Struct(StructDef {
            name: "WithArray".into(),
            members: alloc::vec![member(
                "grid",
                DdsType::Array {
                    element: alloc::boxed::Box::new(DdsType::Int32),
                    dimensions: alloc::vec![3, 4],
                },
            )],
        });
        let grid = &build_sample_instance("S", 2, &s).components[0];
        assert_eq!(grid.value_rank, 2);
        assert_eq!(grid.array_dimensions, alloc::vec![3, 4]);
        assert_eq!(grid.data_type, "Int32");
        assert!(grid.components.is_empty());
    }

    #[test]
    fn sequence_member_is_a_one_dim_array_variable() {
        let s = DdsType::Struct(StructDef {
            name: "WithSeq".into(),
            members: alloc::vec![member(
                "tags",
                DdsType::Sequence {
                    element: alloc::boxed::Box::new(DdsType::String8),
                    bound: Some(10),
                },
            )],
        });
        let tags = &build_sample_instance("S", 2, &s).components[0];
        assert_eq!(tags.value_rank, 1);
        assert_eq!(tags.array_dimensions, alloc::vec![10]);
        assert_eq!(tags.data_type, "String");
    }

    #[test]
    fn sequence_of_struct_is_an_array_not_decomposed() {
        // Design boundary: a collection of structs is a single array Variable
        // (the element structure lives in the §9.2 type layer; element count is
        // runtime). The array points at the element DataType but has NO static
        // per-element child nodes.
        let s = DdsType::Sequence {
            element: alloc::boxed::Box::new(shape()),
            bound: None,
        };
        let n = build_sample_instance("Shapes", 2, &s);
        assert_eq!(n.value_rank, 1);
        assert_eq!(n.data_type, "ShapeTypeDataType");
        assert!(n.components.is_empty());
    }

    #[test]
    fn union_sample_has_switchfield_and_case_components() {
        let u = DdsType::Union(UnionDef {
            name: "ElementValue".into(),
            discriminator: alloc::boxed::Box::new(DdsType::Int32),
            cases: alloc::vec![
                UnionCase {
                    name: "i16".into(),
                    member_type: DdsType::Int16,
                    is_default: false,
                },
                UnionCase {
                    name: "shape".into(),
                    member_type: shape(),
                    is_default: true,
                },
            ],
        });
        let n = build_sample_instance("U", 2, &u);
        let names: Vec<&str> = n
            .components
            .iter()
            .map(|c| c.browse_name.name.as_str())
            .collect();
        assert_eq!(names, alloc::vec!["SwitchField", "i16", "shape"]);
        assert_eq!(n.components[0].data_type, "Int32"); // SwitchField = discriminator
        // The struct case recurses into its own components.
        assert_eq!(n.components[2].components.len(), 3);
        // Union arms are optional (only the active one is present at runtime).
        assert!(n.components[1].is_optional);
    }

    #[test]
    fn key_and_optional_members_surface_on_instance() {
        let s = DdsType::Struct(StructDef {
            name: "Keyed".into(),
            members: alloc::vec![
                MemberDef {
                    name: "id".into(),
                    member_type: DdsType::Int64,
                    is_optional: false,
                    is_key: true,
                },
                MemberDef {
                    name: "note".into(),
                    member_type: DdsType::String8,
                    is_optional: true,
                    is_key: false,
                },
            ],
        });
        let n = build_sample_instance("K", 2, &s);
        assert!(n.components[0].is_key && !n.components[0].is_optional);
        assert!(n.components[1].is_optional && !n.components[1].is_key);
    }

    #[test]
    fn alias_member_is_transparent_on_instance() {
        let s = DdsType::Struct(StructDef {
            name: "Aliased".into(),
            members: alloc::vec![member(
                "v",
                DdsType::Alias {
                    name: "MyInt".into(),
                    target: alloc::boxed::Box::new(DdsType::Int32),
                },
            )],
        });
        let v = &build_sample_instance("A", 2, &s).components[0];
        assert_eq!(v.data_type, "Int32");
        assert_eq!(v.value_rank, -1);
    }
}
