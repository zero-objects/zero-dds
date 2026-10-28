// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Gateway 1.0 §9.3 AddressSpace materialization: DDS `Domain` → `Object`
//! node, DDS `Topic` → `Object` node, DDS sample → the `Variable` hierarchy
//! [`zerodds_opcua_gateway::address_space::build_sample_instance`] already
//! computes symbolically, turned into real
//! [`zerodds_opcua_server::address_space::NodeMeta`]s + `HasComponent`
//! references and kept live by [`crate::dynamic_value::flatten`].
//!
//! NodeId generation is spec "vendor-defined"
//! ([`zerodds_opcua_gateway::address_space::DomainNode::for_domain`]'s own
//! doc); this module's choice: `Topic:<name>` for the topic object,
//! `Topic:<name>/Value[/<dotted member path>]` for the sample variable
//! tree, all as `NodeId::string` in one operator-configured namespace
//! index.

use std::collections::HashMap;
use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_opcua_gateway::address_space::{DomainNode, InstanceNode, build_sample_instance};
use zerodds_opcua_gateway::data_value::DataValue;
use zerodds_opcua_gateway::dds_to_ua::walker::DdsType;
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};
use zerodds_opcua_server::address_space::{AddressSpace, NodeClass, NodeMeta, reference_types};
use zerodds_opcua_server::attributes::{ClassAttributes, VariableAttributes, access_level};

use crate::dynamic_value::FlatField;

/// The `Objects` folder (ns0;i=85, Part 5) every domain object is organized
/// under, so a generic OPC-UA client's default Browse from the root finds
/// this gateway's domains without vendor-specific navigation.
pub const OBJECTS_FOLDER: NodeId = NodeId::numeric(0, 85);

/// One topic's materialized sample-variable tree: the topic object node,
/// the root sample variable, and every leaf's NodeId keyed by the same
/// dotted path [`crate::dynamic_value::flatten`] produces (`""` for a
/// scalar topic's single value).
#[derive(Debug, Clone)]
pub struct TopicAddressSpace {
    /// The `Topic:<name>` object node.
    pub topic_node: NodeId,
    /// The `Topic:<name>/Value` root sample variable.
    pub sample_root: NodeId,
    /// `flatten` path → the leaf's NodeId. Only paths `flatten` actually
    /// emits a `Variant` for are present (a skipped array-of-aggregate path
    /// still gets a node here — created directly by
    /// [`materialize_instance`], not through a sample — just no live value).
    pub leaves: HashMap<String, NodeId>,
}

/// Ensures [`OBJECTS_FOLDER`] itself (ns0;i=85, Part 5) has a `NodeMeta`
/// entry — without one, [`AddressSpace::browse`]'s own precondition (the
/// *source* node of a Browse must be known) refuses every Browse starting
/// from it with `Bad_NodeIdUnknown`, even though every reference this
/// module ever adds *from* it (a domain, a `<browse_path>` segment) is
/// perfectly real. Idempotent.
pub fn ensure_objects_folder(space: &mut AddressSpace) {
    if space.node_meta(&OBJECTS_FOLDER).is_none() {
        space.add_node(NodeMeta::new(
            OBJECTS_FOLDER,
            NodeClass::Object,
            QualifiedName {
                namespace_index: 0,
                name: "Objects".to_string(),
            },
        ));
    }
}

/// Ensures a `Domain<id>` object node exists under [`OBJECTS_FOLDER`] and
/// returns its NodeId. Idempotent.
pub fn ensure_domain(space: &mut AddressSpace, ns: u16, domain_id: u32) -> NodeId {
    ensure_objects_folder(space);
    let domain = DomainNode::for_domain(ns, domain_id);
    if space.node_meta(&domain.node_id).is_none() {
        space.add_node(NodeMeta::new(
            domain.node_id.clone(),
            NodeClass::Object,
            domain.browse_name,
        ));
        space.add_reference(
            OBJECTS_FOLDER,
            reference_types::ORGANIZES,
            domain.node_id.clone(),
        );
    }
    domain.node_id
}

/// Ensures a `Topic:<name>` object node exists under the domain and returns
/// its NodeId. Idempotent.
pub fn ensure_topic(space: &mut AddressSpace, ns: u16, domain_id: u32, topic_name: &str) -> NodeId {
    let domain_node = ensure_domain(space, ns, domain_id);
    let topic_node = topic_object_id(ns, topic_name);
    if space.node_meta(&topic_node).is_none() {
        let browse_name = QualifiedName {
            namespace_index: ns,
            name: topic_name.to_string(),
        };
        space.add_node(NodeMeta::new(
            topic_node.clone(),
            NodeClass::Object,
            browse_name,
        ));
        space.add_reference(
            domain_node,
            reference_types::HAS_COMPONENT,
            topic_node.clone(),
        );
    }
    topic_node
}

/// Builds (or replaces) `topic_name`'s sample-variable tree from its DDS
/// type `ty`, wiring every node and `HasComponent` reference into `space`.
/// Call once per distinct `(topic, type)` — typically at config-load time,
/// before the first sample arrives.
///
/// `writable` sets the Value attribute's `AccessLevel` (Part 3 §5.6.2) on
/// every leaf: `true` for a `ua_to_dds_connection` (a third-party OPC-UA
/// client's Write must actually reach DDS, not fail silently against a
/// read-only node); `false` for a `dds_to_ua_connection`, so a Write
/// against it comes back `Bad_NotWritable` instead of succeeding with no
/// effect (§9's direction is DDS → AddressSpace, never the other way).
pub fn materialize_instance(
    space: &mut AddressSpace,
    ns: u16,
    domain_id: u32,
    topic_name: &str,
    ty: &DdsType,
    writable: bool,
) -> TopicAddressSpace {
    let root_id = sample_node_id(ns, topic_name, "");
    materialize_root(
        space, ns, domain_id, topic_name, ty, writable, root_id, None,
    )
}

/// Failure to apply a Spec §10 XML `<node_id>`/`<browse_path>` override
/// (§9.3). Named, never a log line: the caller (`daemon::server`) turns
/// this into a load-time [`crate::daemon::server::ServerError`], not a
/// warning that leaves the connection materialized under the wrong
/// identity or unreachable from the requested path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideError {
    /// The requested `<node_id>` already names a node this AddressSpace
    /// holds — from an earlier connection, or the daemon's own well-known
    /// nodes (e.g. `ns0;i=85`).
    NodeIdCollision(NodeId),
    /// `<browse_path>` contained an empty segment (`"Objects//Camera1"` or
    /// an empty string), which names no valid BrowseName.
    InvalidBrowsePath,
}

impl core::fmt::Display for OverrideError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NodeIdCollision(id) => {
                write!(
                    f,
                    "<node_id> `{}` collides with an existing node",
                    id.to_canonical_string()
                )
            }
            Self::InvalidBrowsePath => write!(f, "<browse_path> has an empty segment"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OverrideError {}

/// As [`materialize_instance`], but applies the connection's Spec §10
/// XML `<node_id>` (replaces the sample root's auto-generated identity)
/// and `<browse_path>` (an additional `Organizes` chain from the
/// `Objects` folder to the same sample root) — Spec §9.3. The connection
/// keeps its default `Topic:<name>` reachability either way; the
/// override adds to it, it does not remove it (Spec §10 names no
/// "replace" semantics for either field, only an alternate/additional
/// identification).
///
/// `historical_depth`, when set, marks every leaf historizing with that
/// retention bound (Spec §9.3.4.4) — see `materialize_root`.
///
/// # Errors
/// [`OverrideError`] if `node_id` collides with an existing node, or
/// `browse_path` has an empty segment. Checked before any node for this
/// connection is added, so a refused override leaves `space` unchanged.
#[allow(clippy::too_many_arguments)]
pub fn materialize_instance_with_override(
    space: &mut AddressSpace,
    ns: u16,
    domain_id: u32,
    topic_name: &str,
    ty: &DdsType,
    writable: bool,
    node_id_override: Option<NodeId>,
    browse_path: &[String],
    historical_depth: Option<usize>,
) -> Result<TopicAddressSpace, OverrideError> {
    if browse_path.iter().any(|seg| seg.is_empty()) {
        return Err(OverrideError::InvalidBrowsePath);
    }
    let root_id = node_id_override.unwrap_or_else(|| sample_node_id(ns, topic_name, ""));
    if space.node_meta(&root_id).is_some() {
        return Err(OverrideError::NodeIdCollision(root_id));
    }
    let topic = materialize_root(
        space,
        ns,
        domain_id,
        topic_name,
        ty,
        writable,
        root_id,
        historical_depth,
    );
    if !browse_path.is_empty() {
        ensure_browse_path(space, ns, browse_path, &topic.sample_root);
    }
    Ok(topic)
}

/// Shared body of [`materialize_instance`] /
/// [`materialize_instance_with_override`]: builds the topic object (if
/// needed) and the sample-variable tree at `root_id`. `historical_depth`
/// (Spec §9.3.4.4), when set, marks every leaf historizing with that
/// retention bound and adds the `HistoryRead` AccessLevel bit.
#[allow(clippy::too_many_arguments)]
fn materialize_root(
    space: &mut AddressSpace,
    ns: u16,
    domain_id: u32,
    topic_name: &str,
    ty: &DdsType,
    writable: bool,
    root_id: NodeId,
    historical_depth: Option<usize>,
) -> TopicAddressSpace {
    let topic_node = ensure_topic(space, ns, domain_id, topic_name);
    let root = build_sample_instance("Value", ns, ty);
    let mut leaves = HashMap::new();
    place(
        space,
        ns,
        topic_name,
        &topic_node,
        &root,
        "",
        &root_id,
        writable,
        historical_depth,
        &mut leaves,
    );
    TopicAddressSpace {
        topic_node,
        sample_root: root_id,
        leaves,
    }
}

/// Ensures an `Organizes` chain from the `Objects` folder through each of
/// `segments` (creating the intermediate Object nodes on first use,
/// shared across connections with a common path prefix), ending with an
/// `Organizes` reference from the last segment straight to `target`.
fn ensure_browse_path(space: &mut AddressSpace, ns: u16, segments: &[String], target: &NodeId) {
    ensure_objects_folder(space);
    let mut parent = OBJECTS_FOLDER;
    let mut cumulative = String::new();
    for seg in segments {
        cumulative = if cumulative.is_empty() {
            seg.clone()
        } else {
            std::format!("{cumulative}/{seg}")
        };
        let node_id = checked_string_node_id(ns, std::format!("Path:{cumulative}"));
        if space.node_meta(&node_id).is_none() {
            space.add_node(NodeMeta::new(
                node_id.clone(),
                NodeClass::Object,
                QualifiedName {
                    namespace_index: ns,
                    name: seg.clone(),
                },
            ));
            space.add_reference(parent.clone(), reference_types::ORGANIZES, node_id.clone());
        }
        parent = node_id;
    }
    space.add_reference(parent, reference_types::ORGANIZES, target.clone());
}

fn variable_attributes(node: &InstanceNode, writable: bool, historizing: bool) -> ClassAttributes {
    let mut bits = if writable {
        access_level::CURRENT_READ | access_level::CURRENT_WRITE
    } else {
        access_level::CURRENT_READ
    };
    if historizing {
        // Spec §9.3.4.4: AccessLevel must carry the HistoryRead bit
        // alongside CurrentRead.
        bits |= access_level::HISTORY_READ;
    }
    ClassAttributes::Variable(VariableAttributes {
        value_rank: node.value_rank,
        array_dimensions: node.array_dimensions.clone(),
        access_level: bits,
        user_access_level: bits,
        historizing,
        ..VariableAttributes::default()
    })
}

/// zerodds-lint: recursion-depth 64
///
/// Indirectly recursive over `InstanceNode::components` — bounded the same
/// way `build_sample_instance` itself is (see its own recursion note).
#[allow(clippy::too_many_arguments)]
fn place(
    space: &mut AddressSpace,
    ns: u16,
    topic_name: &str,
    parent: &NodeId,
    node: &InstanceNode,
    path: &str,
    node_id: &NodeId,
    writable: bool,
    historical_depth: Option<usize>,
    leaves: &mut HashMap<String, NodeId>,
) {
    let is_leaf = node.components.is_empty();
    // Only a leaf carries a live Value ([`apply_sample`] never sets one on
    // an aggregate node), so only a leaf is historizing.
    let historizing = is_leaf && historical_depth.is_some();
    let meta = NodeMeta::new(
        node_id.clone(),
        NodeClass::Variable,
        node.browse_name.clone(),
    )
    .with_display_name(LocalizedText {
        locale: None,
        text: Some(node.browse_name.name.clone()),
    })
    .with_class_attributes(variable_attributes(node, writable, historizing));
    space.add_node(meta);
    space.add_reference(
        parent.clone(),
        reference_types::HAS_COMPONENT,
        node_id.clone(),
    );
    if let (true, Some(depth)) = (historizing, historical_depth) {
        space.set_historizing(node_id.clone(), depth);
    }

    if is_leaf {
        leaves.insert(path.to_string(), node_id.clone());
        return;
    }
    for child in &node.components {
        let child_path = join(path, &child.browse_name.name);
        let child_id = sample_node_id(ns, topic_name, &child_path);
        place(
            space,
            ns,
            topic_name,
            node_id,
            child,
            &child_path,
            &child_id,
            writable,
            historical_depth,
            leaves,
        );
    }
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        std::format!("{path}.{name}")
    }
}

/// Builds a string NodeId, or panics naming the identifier that overflowed
/// [`NodeId::string`]'s 4096-byte limit — never a silent fallback: two
/// distinct topics/paths collapsing onto the same substitute NodeId would
/// make one invisible to Browse and corrupt the other's Value, which is
/// worse than a loud failure at config-load time (this is only ever called
/// while materializing a topic's tree, not per sample).
#[allow(clippy::panic)]
fn checked_string_node_id(ns: u16, identifier: String) -> NodeId {
    NodeId::string(ns, identifier.clone())
        .unwrap_or_else(|e| panic!("NodeId `{identifier}` ({} bytes): {e}", identifier.len()))
}

fn topic_object_id(ns: u16, topic_name: &str) -> NodeId {
    checked_string_node_id(ns, std::format!("Topic:{topic_name}"))
}

fn sample_node_id(ns: u16, topic_name: &str, path: &str) -> NodeId {
    let suffix = if path.is_empty() {
        std::format!("Topic:{topic_name}/Value")
    } else {
        std::format!("Topic:{topic_name}/Value/{path}")
    };
    checked_string_node_id(ns, suffix)
}

/// Applies a flattened sample to `topic`'s leaves — [`AddressSpace::set_value`]
/// per matched path, timestamped `source_timestamp`. A `field.path` this
/// topic's tree has no leaf for (a shape drift against the type this
/// instance tree was built from) is reported, not silently ignored.
///
/// # Errors
/// The list of paths that named no known leaf.
pub fn apply_sample(
    space: &mut AddressSpace,
    topic: &TopicAddressSpace,
    fields: &[FlatField],
    source_timestamp: i64,
) -> Vec<String> {
    let mut unmatched = Vec::new();
    for field in fields {
        match topic.leaves.get(&field.path) {
            Some(node_id) => {
                space.set_value(
                    node_id.clone(),
                    DataValue::new_value(field.value.clone(), 0, source_timestamp),
                );
            }
            None => unmatched.push(field.path.clone()),
        }
    }
    unmatched
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_opcua_gateway::data_value::{Variant, VariantValue};
    use zerodds_opcua_gateway::dds_to_ua::walker::{MemberDef, StructDef};

    fn shape() -> DdsType {
        DdsType::Struct(StructDef {
            name: "ShapeType".into(),
            members: vec![
                MemberDef {
                    name: "x".into(),
                    member_type: DdsType::Int32,
                    is_optional: false,
                    is_key: false,
                },
                MemberDef {
                    name: "y".into(),
                    member_type: DdsType::Int32,
                    is_optional: false,
                    is_key: false,
                },
            ],
        })
    }

    #[test]
    fn scalar_topic_gets_one_leaf_at_the_empty_path() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Temperature", &DdsType::Float64, false);
        assert_eq!(topic.leaves.len(), 1);
        assert!(topic.leaves.contains_key(""));
        assert_eq!(topic.leaves[""], topic.sample_root);
        assert!(space.node_meta(&topic.topic_node).is_some());
    }

    #[test]
    fn struct_topic_gets_one_leaf_per_member() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Shape", &shape(), false);
        assert_eq!(topic.leaves.len(), 2);
        assert!(topic.leaves.contains_key("x"));
        assert!(topic.leaves.contains_key("y"));
    }

    #[test]
    fn apply_sample_sets_the_right_node_values() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Shape", &shape(), false);
        let fields = vec![
            FlatField {
                path: "x".into(),
                value: Variant::scalar(VariantValue::Int32(3)),
            },
            FlatField {
                path: "y".into(),
                value: Variant::scalar(VariantValue::Int32(4)),
            },
        ];
        let unmatched = apply_sample(&mut space, &topic, &fields, 1234);
        assert!(unmatched.is_empty());
        let x_node = &topic.leaves["x"];
        assert_eq!(
            space.value(x_node).unwrap().value,
            Some(Variant::scalar(VariantValue::Int32(3)))
        );
    }

    #[test]
    fn unmatched_path_is_reported_not_dropped() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Shape", &shape(), false);
        let fields = vec![FlatField {
            path: "z_does_not_exist".into(),
            value: Variant::scalar(VariantValue::Int32(1)),
        }];
        let unmatched = apply_sample(&mut space, &topic, &fields, 0);
        assert_eq!(unmatched, vec!["z_does_not_exist".to_string()]);
    }

    #[test]
    fn objects_folder_is_browsable_after_materializing_a_topic() {
        // Without a NodeMeta entry for OBJECTS_FOLDER itself, a Browse
        // starting from it (any client's default entry point) refuses
        // with Bad_NodeIdUnknown even though the Organizes reference to
        // the domain is real — regression guard for that gap.
        let mut space = AddressSpace::new();
        materialize_instance(&mut space, 2, 0, "A", &DdsType::Int32, false);
        assert!(space.node_meta(&OBJECTS_FOLDER).is_some());
        let refs = space.browse(&OBJECTS_FOLDER, 0, None, false, 0);
        assert!(refs.iter().any(|r| r.target.browse_name.name == "Domain0"));
    }

    #[test]
    fn two_topics_do_not_collide() {
        let mut space = AddressSpace::new();
        let a = materialize_instance(&mut space, 2, 0, "A", &DdsType::Int32, false);
        let b = materialize_instance(&mut space, 2, 0, "B", &DdsType::Int32, true);
        assert_ne!(a.sample_root, b.sample_root);
        assert_ne!(a.topic_node, b.topic_node);
    }

    #[test]
    fn node_id_override_replaces_the_default_root_identity() {
        let mut space = AddressSpace::new();
        let custom = NodeId::numeric(2, 12345);
        let topic = materialize_instance_with_override(
            &mut space,
            2,
            0,
            "Camera1",
            &DdsType::Int32,
            false,
            Some(custom.clone()),
            &[],
            None,
        )
        .expect("no override conflict");
        assert_eq!(topic.sample_root, custom);
        assert!(space.node_meta(&custom).is_some());
    }

    #[test]
    fn node_id_override_colliding_with_an_existing_node_is_refused() {
        let mut space = AddressSpace::new();
        materialize_instance(&mut space, 2, 0, "A", &DdsType::Int32, false);
        let existing = sample_node_id(2, "A", "");
        let err = materialize_instance_with_override(
            &mut space,
            2,
            0,
            "B",
            &DdsType::Int32,
            false,
            Some(existing.clone()),
            &[],
            None,
        )
        .expect_err("must refuse a colliding node_id, not overwrite");
        assert_eq!(err, OverrideError::NodeIdCollision(existing));
        // The refused connection must not have touched `space` at all.
        assert!(space.node_meta(&topic_object_id(2, "B")).is_none());
    }

    #[test]
    fn empty_browse_path_segment_is_refused() {
        let mut space = AddressSpace::new();
        let err = materialize_instance_with_override(
            &mut space,
            2,
            0,
            "Camera1",
            &DdsType::Int32,
            false,
            None,
            &["Objects".to_string(), String::new(), "Camera1".to_string()],
            None,
        )
        .expect_err("an empty segment names no BrowseName");
        assert_eq!(err, OverrideError::InvalidBrowsePath);
    }

    #[test]
    fn browse_path_organizes_the_root_from_the_objects_folder() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance_with_override(
            &mut space,
            2,
            0,
            "Camera1",
            &DdsType::Int32,
            false,
            None,
            &["Sensors".to_string(), "Camera1".to_string()],
            None,
        )
        .expect("valid browse path");
        // Objects -> Sensors -> Camera1(path object) -> sample_root, and the
        // default Topic:<name> reachability is kept alongside it.
        let sensors = space
            .browse(&OBJECTS_FOLDER, 0, None, false, 0)
            .into_iter()
            .find(|r| r.target.browse_name.name == "Sensors")
            .expect("Sensors object under Objects");
        let camera1 = space
            .browse(&sensors.target.node_id, 0, None, false, 0)
            .into_iter()
            .find(|r| r.target.browse_name.name == "Camera1")
            .expect("Camera1 object under Sensors");
        let reaches_root = space
            .browse(&camera1.target.node_id, 0, None, false, 0)
            .into_iter()
            .any(|r| r.target.node_id == topic.sample_root);
        assert!(reaches_root, "Camera1 must Organizes the sample root");
        assert!(
            space.node_meta(&topic.topic_node).is_some(),
            "the default Topic:<name> object must still exist"
        );
    }

    #[test]
    fn historical_depth_marks_every_leaf_historizing() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance_with_override(
            &mut space,
            2,
            0,
            "Shape",
            &shape(),
            false,
            None,
            &[],
            Some(50),
        )
        .expect("no override conflict");
        assert!(space.is_historizing(&topic.leaves["x"]));
        assert!(space.is_historizing(&topic.leaves["y"]));
        let ClassAttributes::Variable(attrs) = &space
            .node_meta(&topic.leaves["x"])
            .expect("meta")
            .class_attributes
        else {
            panic!("expected variable attributes");
        };
        assert!(attrs.historizing);
        assert_eq!(
            attrs.access_level & access_level::HISTORY_READ,
            access_level::HISTORY_READ
        );
    }

    #[test]
    fn no_historical_depth_leaves_a_leaf_non_historizing() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Plain", &DdsType::Int32, false);
        assert!(!space.is_historizing(&topic.sample_root));
    }

    #[test]
    fn dds_to_ua_leaf_is_read_only_ua_to_dds_leaf_is_writable() {
        let mut space = AddressSpace::new();
        let read_only = materialize_instance(&mut space, 2, 0, "FromDds", &DdsType::Int32, false);
        let writable = materialize_instance(&mut space, 2, 0, "ToDds", &DdsType::Int32, true);

        let ro_meta = space.node_meta(&read_only.sample_root).expect("meta");
        let ClassAttributes::Variable(ro_attrs) = &ro_meta.class_attributes else {
            panic!("expected variable attributes");
        };
        assert_eq!(
            ro_attrs.access_level,
            zerodds_opcua_server::attributes::access_level::CURRENT_READ
        );

        let rw_meta = space.node_meta(&writable.sample_root).expect("meta");
        let ClassAttributes::Variable(rw_attrs) = &rw_meta.class_attributes else {
            panic!("expected variable attributes");
        };
        assert_eq!(
            rw_attrs.access_level,
            zerodds_opcua_server::attributes::access_level::CURRENT_READ
                | zerodds_opcua_server::attributes::access_level::CURRENT_WRITE
        );
    }
}
