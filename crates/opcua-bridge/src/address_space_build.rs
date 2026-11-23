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
    /// The aggregate Variables by the same paths: a structure, a union, a
    /// `map`, a collection of structures or unions and each of its
    /// elements. Their Value is the whole value
    /// ([`crate::ua_structure::aggregate_values`]).
    pub aggregates: HashMap<String, NodeId>,
    /// The paths of the `@optional` members and of the union cases: a
    /// sample may not have them, and their nodes then read `Bad_NoData`.
    optional: Vec<String>,
    /// The `map` members of the topic's type, each with the node its
    /// entries hang under: a key a sample brings gets its nodes when it
    /// first appears ([`apply_sample`]).
    maps: Vec<MapSlot>,
    /// The entries created so far, by their path.
    map_entries: std::collections::HashSet<String>,
    ns: u16,
    topic_name: String,
    writable: bool,
    historical_depth: Option<usize>,
}

/// A `map` member of a topic's type.
#[derive(Debug, Clone)]
struct MapSlot {
    /// The member's path, as [`crate::dynamic_value::flatten`] names it.
    path: String,
    /// The node of the member; an entry is a component of it.
    node: NodeId,
    /// The type of the map's values, or of the collection's elements.
    value: DdsType,
    /// For an array or a sequence of structures or unions (Gateway 1.0
    /// §9.2.5.1.2.3, §9.2.5.2.2.3): the name of the element type and the
    /// dimensions of an array (none for a sequence). An entry is then an
    /// element, `<TypeName>_<index>`, an ordered component of the node.
    elements: Option<(String, Vec<u32>)>,
}

/// The most map entries one topic gets nodes for while the daemon runs.
/// The keys come from the samples of remote writers, and each is a node
/// that stays; a key past the bound is reported as an unmatched path.
pub const MAX_MAP_ENTRY_NODES: usize = 4096;

/// The longest key a map entry gets a node for, in octets: it becomes a
/// part of the node's string NodeId, which is bounded at 4096.
pub const MAX_MAP_KEY_LEN: usize = 256;

/// Collects the `map` members of `ty` at `path` into `out`. A map at the
/// root hangs under `root`.
///
/// zerodds-lint: recursion-depth 64 (the nesting of the type, bounded as
/// `build_sample_instance` bounds it)
fn collect_maps(
    ty: &DdsType,
    path: &str,
    ns: u16,
    topic_name: &str,
    root: &NodeId,
    out: &mut Vec<MapSlot>,
) {
    match ty {
        DdsType::Alias { target, .. } => collect_maps(target, path, ns, topic_name, root, out),
        DdsType::Struct(def) => {
            for member in &def.members {
                collect_maps(
                    &member.member_type,
                    &join(path, &member.name),
                    ns,
                    topic_name,
                    root,
                    out,
                );
            }
        }
        DdsType::Map { value, .. } => out.push(MapSlot {
            path: path.to_string(),
            node: if path.is_empty() {
                root.clone()
            } else {
                sample_node_id(ns, topic_name, path)
            },
            value: (**value).clone(),
            elements: None,
        }),
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => {
            // Gateway 1.0 §9.2.5.1.2.3: an element that is a structure or
            // a union is named after its type; §9.2.5.2.2.5: one that is a
            // collection after the collection.
            let type_name = match element.resolve_alias() {
                DdsType::Struct(def) => def.name.clone(),
                DdsType::Union(def) => def.name.clone(),
                DdsType::Array { .. } | DdsType::Sequence { .. } | DdsType::Map { .. } => {
                    path.rsplit('.').next().unwrap_or(path).to_string()
                }
                _ => return,
            };
            let dims = match ty {
                DdsType::Array { dimensions, .. } => dimensions.clone(),
                _ => Vec::new(),
            };
            out.push(MapSlot {
                path: path.to_string(),
                node: if path.is_empty() {
                    root.clone()
                } else {
                    sample_node_id(ns, topic_name, path)
                },
                value: element.resolve_alias().clone(),
                elements: Some((type_name, dims)),
            });
        }
        _ => {}
    }
}

impl TopicAddressSpace {
    /// The Variables a client's Write reaches DDS through: the leaves and
    /// the aggregate Variables.
    pub fn writable_nodes(&self) -> impl Iterator<Item = &NodeId> {
        self.leaves.values().chain(self.aggregates.values())
    }

    /// Creates the nodes of the map entry `path` lies in, when `path` is
    /// under a `map` member and the entry has none yet. `true` when nodes
    /// were created.
    fn materialize_map_entry(&mut self, space: &mut AddressSpace, path: &str) -> bool {
        let Some(slot) = self
            .maps
            .iter()
            .filter(|slot| {
                slot.path.is_empty()
                    || path
                        .strip_prefix(slot.path.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            })
            .max_by_key(|slot| slot.path.len())
            .cloned()
        else {
            return false;
        };
        let rest = if slot.path.is_empty() {
            path
        } else {
            &path[slot.path.len() + 1..]
        };
        // A scalar value is the entry itself, so the key is all that is
        // left of the path; an aggregate value has its members after it.
        let scalar = build_sample_instance("value", self.ns, &slot.value)
            .components
            .is_empty();
        let key = if scalar {
            rest
        } else {
            rest.split('.').next().unwrap_or(rest)
        };
        self.create_entry(space, &slot, key)
    }

    /// The elements a sequence of structures had in an earlier sample and
    /// does not have in this one keep their nodes; their leaves get the
    /// status `Bad_NoData` and no value, so that a client does not read the
    /// value of an element that is gone.
    fn mark_absent_elements(
        &self,
        space: &mut AddressSpace,
        fields: &[FlatField],
        source_timestamp: i64,
    ) {
        let sequences: Vec<&str> = self
            .maps
            .iter()
            .filter(|slot| matches!(&slot.elements, Some((_, dims)) if dims.is_empty()))
            .map(|slot| slot.path.as_str())
            .collect();
        if sequences.is_empty() {
            return;
        }
        let present: std::collections::HashSet<&str> =
            fields.iter().map(|field| field.path.as_str()).collect();
        for (path, node_id) in self.leaves.iter().chain(self.aggregates.iter()) {
            let of_an_element = sequences.iter().any(|sequence| {
                path.as_str() != *sequence
                    && (sequence.is_empty()
                        || path
                            .strip_prefix(sequence)
                            .is_some_and(|rest| rest.starts_with('.')))
            });
            if !of_an_element
                || present.contains(path.as_str())
                || space.value(node_id).is_none_or(|dv| dv.value.is_none())
            {
                continue;
            }
            space.set_value(
                node_id.clone(),
                DataValue {
                    value: None,
                    status: Some(zerodds_opcua_server::status::BAD_NO_DATA),
                    source_timestamp: Some(source_timestamp),
                    server_timestamp: None,
                    source_pico_sec: None,
                    server_pico_sec: None,
                },
            );
        }
    }

    /// The `@optional` members and the union cases a sample does not have
    /// keep their nodes; what is under them gets the status `Bad_NoData`
    /// and no value.
    fn mark_absent_optional(
        &self,
        space: &mut AddressSpace,
        fields: &[FlatField],
        source_timestamp: i64,
    ) {
        let under = |path: &str, prefix: &str| {
            path == prefix
                || path
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('.'))
        };
        for optional in &self.optional {
            if fields.iter().any(|field| under(&field.path, optional)) {
                continue;
            }
            for (path, node_id) in self.leaves.iter().chain(self.aggregates.iter()) {
                if !under(path, optional)
                    || space.value(node_id).is_none_or(|dv| dv.value.is_none())
                {
                    continue;
                }
                space.set_value(
                    node_id.clone(),
                    DataValue {
                        value: None,
                        status: Some(zerodds_opcua_server::status::BAD_NO_DATA),
                        source_timestamp: Some(source_timestamp),
                        server_timestamp: None,
                        source_pico_sec: None,
                        server_pico_sec: None,
                    },
                );
            }
        }
    }

    /// Creates the nodes of the entry `key` of `slot`: its Variable, a
    /// component of the slot's node (an ordered one for an element of a
    /// collection), and what the value type has under it. `false` when the
    /// entry is there already or is past a bound.
    fn create_entry(&mut self, space: &mut AddressSpace, slot: &MapSlot, key: &str) -> bool {
        let entry_path = join(&slot.path, key);
        if key.is_empty()
            || key.len() > MAX_MAP_KEY_LEN
            || self.map_entries.len() >= MAX_MAP_ENTRY_NODES
            || self.map_entries.contains(&entry_path)
            // The string NodeId of the entry and of what is under it.
            || self.topic_name.len() + entry_path.len() + 512 > 4096
        {
            return false;
        }
        let entry = build_sample_instance(key, self.ns, &slot.value);
        let entry_id = sample_node_id(self.ns, &self.topic_name, &entry_path);
        place(
            space,
            &Placement {
                ns: self.ns,
                topic_name: &self.topic_name,
                writable: self.writable,
                historical_depth: self.historical_depth,
            },
            &slot.node,
            &entry,
            &slot.value,
            &entry_path,
            &entry_id,
            slot.elements.is_some(),
            &mut Placed {
                leaves: &mut self.leaves,
                aggregates: &mut self.aggregates,
                optional: &mut self.optional,
            },
        );
        // A map or a collection of structures in the value is one of this
        // entry.
        let root = self.sample_root.clone();
        collect_maps(
            &slot.value,
            &entry_path,
            self.ns,
            &self.topic_name,
            &root,
            &mut self.maps,
        );
        self.map_entries.insert(entry_path);
        true
    }
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
///
/// # Errors
/// [`OverrideError::Type`] when a DataType of the type's name is there
/// with another definition.
pub fn materialize_instance(
    space: &mut AddressSpace,
    ns: u16,
    domain_id: u32,
    topic_name: &str,
    ty: &DdsType,
    writable: bool,
) -> Result<TopicAddressSpace, OverrideError> {
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
    /// A DataType of one of the type's names is there with another
    /// definition (Gateway 1.0 §9.2: one DataType per type name).
    Type(crate::ua_types::TypeConflict),
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
            Self::Type(conflict) => write!(f, "{conflict}"),
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
    )?;
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
) -> Result<TopicAddressSpace, OverrideError> {
    crate::ua_types::ensure_types(space, ns, ty).map_err(OverrideError::Type)?;
    let topic_node = ensure_topic(space, ns, domain_id, topic_name);
    let root = build_sample_instance("Value", ns, ty);
    let mut leaves = HashMap::new();
    let mut aggregates = HashMap::new();
    let mut optional = Vec::new();
    place(
        space,
        &Placement {
            ns,
            topic_name,
            writable,
            historical_depth,
        },
        &topic_node,
        &root,
        ty,
        "",
        &root_id,
        false,
        &mut Placed {
            leaves: &mut leaves,
            aggregates: &mut aggregates,
            optional: &mut optional,
        },
    );
    let mut maps = Vec::new();
    collect_maps(ty, "", ns, topic_name, &root_id, &mut maps);
    let mut topic = TopicAddressSpace {
        topic_node,
        sample_root: root_id,
        leaves,
        aggregates,
        optional,
        maps,
        map_entries: std::collections::HashSet::new(),
        ns,
        topic_name: topic_name.to_string(),
        writable,
        historical_depth,
    };
    // An array has its elements from the start: their Variables are there
    // before the first sample, and a client can write them.
    let mut next = 0;
    while next < topic.maps.len() {
        let slot = topic.maps[next].clone();
        next += 1;
        let Some((type_name, dims)) = &slot.elements else {
            continue;
        };
        if dims.is_empty() {
            continue;
        }
        let elements = dims
            .iter()
            .fold(1usize, |count, dim| count.saturating_mul(*dim as usize));
        for index in 0..elements {
            let name = crate::dynamic_value::element_name(type_name, index, dims);
            if !topic.create_entry(space, &slot, &name) {
                break;
            }
        }
    }
    Ok(topic)
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

fn variable_attributes(
    shape: crate::ua_types::Shape,
    writable: bool,
    historizing: bool,
) -> ClassAttributes {
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
        data_type: shape.data_type,
        value_rank: shape.value_rank,
        array_dimensions: shape.array_dimensions,
        access_level: bits,
        user_access_level: bits,
        access_level_ex: u32::from(bits),
        historizing,
        ..VariableAttributes::default()
    })
}

/// What every node of one topic shares.
struct Placement<'a> {
    ns: u16,
    topic_name: &'a str,
    writable: bool,
    historical_depth: Option<usize>,
}

/// Where [`place`] records the nodes it adds, by their paths.
struct Placed<'a> {
    leaves: &'a mut HashMap<String, NodeId>,
    aggregates: &'a mut HashMap<String, NodeId>,
    optional: &'a mut Vec<String>,
}

/// Whether a Variable of `ty` is an aggregate one: its Value is the whole
/// structure, union, `map` or collection of structures under it.
fn is_aggregate(ty: &DdsType) -> bool {
    match ty.resolve_alias() {
        DdsType::Struct(_) | DdsType::Union(_) | DdsType::Map { .. } => true,
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => matches!(
            element.resolve_alias(),
            DdsType::Struct(_)
                | DdsType::Union(_)
                | DdsType::Array { .. }
                | DdsType::Sequence { .. }
                | DdsType::Map { .. }
        ),
        _ => false,
    }
}

/// The type of the component `name` of a Variable of `ty`, and whether a
/// sample may not have it (an `@optional` member, a union case).
fn component_type<'a>(ty: &'a DdsType, name: &str) -> Option<(&'a DdsType, bool)> {
    match ty.resolve_alias() {
        DdsType::Struct(def) => def
            .members
            .iter()
            .find(|member| member.name == name)
            .map(|member| (&member.member_type, member.is_optional)),
        DdsType::Union(def) if name == "SwitchField" => Some((&*def.discriminator, false)),
        DdsType::Union(def) => def
            .cases
            .iter()
            .find(|case| case.name == name)
            .map(|case| (&case.member_type, true)),
        _ => None,
    }
}

/// Adds the Variable `node` of the type `ty` at `path`, a component of
/// `parent`, and what is under it: its DataType, ValueRank and
/// ArrayDimensions and its TypeDefinition from the type layer
/// ([`crate::ua_types`]).
///
/// zerodds-lint: recursion-depth 64
///
/// Indirectly recursive over `InstanceNode::components` — bounded the same
/// way `build_sample_instance` itself is (see its own recursion note).
#[allow(clippy::too_many_arguments)]
fn place(
    space: &mut AddressSpace,
    at: &Placement<'_>,
    parent: &NodeId,
    node: &InstanceNode,
    ty: &DdsType,
    path: &str,
    node_id: &NodeId,
    ordered: bool,
    placed: &mut Placed<'_>,
) {
    let aggregate = is_aggregate(ty);
    let is_leaf = !aggregate && node.components.is_empty();
    // Only a leaf is historizing; an aggregate's history is its leaves'.
    let historizing = is_leaf && at.historical_depth.is_some();
    let type_definition = crate::ua_types::type_definition_of(at.ns, ty);
    let meta = NodeMeta::new(
        node_id.clone(),
        NodeClass::Variable,
        node.browse_name.clone(),
    )
    .with_display_name(LocalizedText {
        locale: None,
        text: Some(node.browse_name.name.clone()),
    })
    .with_type_definition(type_definition.clone())
    .with_class_attributes(variable_attributes(
        crate::ua_types::shape_of(at.ns, ty),
        at.writable,
        historizing,
    ));
    space.add_node(meta);
    space.add_reference(
        parent.clone(),
        if ordered {
            reference_types::HAS_ORDERED_COMPONENT
        } else {
            reference_types::HAS_COMPONENT
        },
        node_id.clone(),
    );
    space.add_reference(
        node_id.clone(),
        reference_types::HAS_TYPE_DEFINITION,
        type_definition,
    );
    if let (true, Some(depth)) = (historizing, at.historical_depth) {
        space.set_historizing(node_id.clone(), depth);
    }

    if aggregate {
        placed.aggregates.insert(path.to_string(), node_id.clone());
    } else if is_leaf {
        placed.leaves.insert(path.to_string(), node_id.clone());
        return;
    }
    for child in &node.components {
        let child_path = join(path, &child.browse_name.name);
        let child_id = sample_node_id(at.ns, at.topic_name, &child_path);
        let Some((child_type, may_be_absent)) = component_type(ty, &child.browse_name.name) else {
            continue;
        };
        if may_be_absent {
            placed.optional.push(child_path.clone());
        }
        place(
            space,
            at,
            node_id,
            child,
            child_type,
            &child_path,
            &child_id,
            false,
            placed,
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
/// instance tree was built from) is reported, not silently ignored. A path
/// under a `map` member whose key is new gets its nodes first (spec §5.3):
/// the entry's Variable, a component of the map's node, and what its value
/// type has under it.
///
/// # Errors
/// The list of paths that named no known leaf.
pub fn apply_sample(
    space: &mut AddressSpace,
    topic: &mut TopicAddressSpace,
    fields: &[FlatField],
    source_timestamp: i64,
) -> Vec<String> {
    let mut unmatched = Vec::new();
    for field in fields {
        if !topic.leaves.contains_key(&field.path) && !topic.aggregates.contains_key(&field.path) {
            topic.materialize_map_entry(space, &field.path);
        }
        match topic
            .leaves
            .get(&field.path)
            .or_else(|| topic.aggregates.get(&field.path))
        {
            Some(node_id) => {
                space.set_value(
                    node_id.clone(),
                    DataValue::new_value(field.value.clone(), 0, source_timestamp),
                );
            }
            None => unmatched.push(field.path.clone()),
        }
    }
    topic.mark_absent_elements(space, fields, source_timestamp);
    topic.mark_absent_optional(space, fields, source_timestamp);
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

    fn readings() -> DdsType {
        let member = |name: &str, member_type: DdsType| MemberDef {
            name: name.into(),
            member_type,
            is_optional: false,
            is_key: false,
        };
        DdsType::Struct(StructDef {
            name: "Readings".into(),
            members: vec![
                member("id", DdsType::Int32),
                member(
                    "values",
                    DdsType::Map {
                        key: Box::new(DdsType::String8),
                        value: Box::new(DdsType::Float64),
                        bound: None,
                    },
                ),
                member(
                    "shapes",
                    DdsType::Map {
                        key: Box::new(DdsType::String8),
                        value: Box::new(shape()),
                        bound: None,
                    },
                ),
            ],
        })
    }

    /// Gateway 1.0 §9.2.5.2.2.5: an inner sequence is a Variable
    /// `<SequenceName>_<index>`, an ordered component of the outer one,
    /// which has the DataType BaseDataType; Gateway §9.2: every Variable
    /// carries its DataType and TypeDefinition.
    #[test]
    fn opcuabridge_5_3_an_inner_sequence_is_a_variable_with_its_datatype() {
        use zerodds_opcua_server::attributes::attribute_ids;
        let rows = DdsType::Struct(StructDef {
            name: "Rows".into(),
            members: vec![MemberDef {
                name: "rows".into(),
                member_type: DdsType::Sequence {
                    element: Box::new(DdsType::Sequence {
                        element: Box::new(DdsType::Int32),
                        bound: None,
                    }),
                    bound: None,
                },
                is_optional: false,
                is_key: false,
            }],
        });
        let mut space = AddressSpace::new();
        let mut topic =
            materialize_instance(&mut space, 2, 0, "Rows", &rows, false).expect("materialize");
        let fields = vec![FlatField {
            path: "rows.rows_0".into(),
            value: Variant {
                array_dimensions: vec![2],
                value: vec![VariantValue::Int32(1), VariantValue::Int32(2)],
            },
        }];
        assert!(apply_sample(&mut space, &mut topic, &fields, 1).is_empty());
        let read = |path: &str, attribute: u32| {
            space
                .read_attribute(&sample_node_id(2, "Rows", path), attribute)
                .expect("read")
                .value
                .expect("value")
                .value[0]
                .clone()
        };
        assert_eq!(
            read("rows.rows_0", attribute_ids::DATA_TYPE),
            VariantValue::NodeId(NodeId::numeric(0, 6))
        );
        assert_eq!(
            read("rows.rows_0", attribute_ids::VALUE_RANK),
            VariantValue::Int32(1)
        );
        assert_eq!(
            read("rows", attribute_ids::DATA_TYPE),
            VariantValue::NodeId(NodeId::numeric(0, 24))
        );
        assert_eq!(
            read("", attribute_ids::DATA_TYPE),
            VariantValue::NodeId(crate::ua_types::data_type_id(2, "Rows"))
        );
        assert_eq!(
            space
                .node_meta(&sample_node_id(2, "Rows", ""))
                .expect("root")
                .type_definition,
            crate::ua_types::variable_type_id(2, "Rows")
        );
        assert!(
            space
                .browse(&sample_node_id(2, "Rows", "rows"), 0, None, false, 0)
                .iter()
                .any(
                    |m| m.reference_type == reference_types::HAS_ORDERED_COMPONENT
                        && m.target.node_id == sample_node_id(2, "Rows", "rows.rows_0")
                )
        );
    }

    /// Spec §5.3: a map key a sample brings gets its nodes then: the
    /// entry's Variable under the map's node and, for a struct value, the
    /// members under it. The key again creates nothing; a key too long
    /// for a NodeId and a path under no map are reported.
    #[test]
    fn opcuabridge_5_3_a_new_map_key_gets_its_nodes_when_a_sample_brings_it() {
        let mut space = AddressSpace::new();
        let mut topic = materialize_instance(&mut space, 2, 0, "Readings", &readings(), false)
            .expect("materialize");
        let int = |path: &str, value: i32| FlatField {
            path: path.into(),
            value: Variant::scalar(VariantValue::Int32(value)),
        };
        let double = |path: &str, value: f64| FlatField {
            path: path.into(),
            value: Variant::scalar(VariantValue::Double(value)),
        };
        let node = |path: &str| {
            NodeId::string(2, std::format!("Topic:Readings/Value/{path}")).expect("node id")
        };
        assert!(space.node_meta(&node("values.a")).is_none());

        let fields = vec![
            int("id", 1),
            double("values.a", 1.5),
            int("shapes.s1.x", 3),
            int("shapes.s1.y", 4),
        ];
        assert!(apply_sample(&mut space, &mut topic, &fields, 10).is_empty());
        let value_of = |space: &AddressSpace, path: &str| {
            space.value(&node(path)).and_then(|dv| dv.value.clone())
        };
        assert_eq!(
            value_of(&space, "values.a"),
            Some(Variant::scalar(VariantValue::Double(1.5)))
        );
        assert_eq!(
            value_of(&space, "shapes.s1.y"),
            Some(Variant::scalar(VariantValue::Int32(4)))
        );
        // The entry is a Variable, and the struct entry has its own node.
        assert!(space.node_meta(&node("values.a")).is_some());
        assert!(space.node_meta(&node("shapes.s1")).is_some());
        let nodes = topic.leaves.len();

        // The same keys again: new values, no new nodes.
        let again = vec![double("values.a", 2.5), int("shapes.s1.x", 30)];
        assert!(apply_sample(&mut space, &mut topic, &again, 11).is_empty());
        assert_eq!(topic.leaves.len(), nodes);
        assert_eq!(
            value_of(&space, "values.a"),
            Some(Variant::scalar(VariantValue::Double(2.5)))
        );

        let long_key = std::format!("values.{}", "k".repeat(MAX_MAP_KEY_LEN + 1));
        let refused = vec![double(&long_key, 1.0), int("nosuch.member", 1)];
        assert_eq!(
            apply_sample(&mut space, &mut topic, &refused, 12),
            vec![long_key, "nosuch.member".to_string()]
        );
    }

    fn fleet() -> DdsType {
        let member = |name: &str, member_type: DdsType| MemberDef {
            name: name.into(),
            member_type,
            is_optional: false,
            is_key: false,
        };
        DdsType::Struct(StructDef {
            name: "Fleet".into(),
            members: vec![
                member(
                    "corners",
                    DdsType::Array {
                        element: Box::new(shape()),
                        dimensions: vec![2],
                    },
                ),
                member(
                    "trail",
                    DdsType::Sequence {
                        element: Box::new(shape()),
                        bound: None,
                    },
                ),
            ],
        })
    }

    /// Gateway 1.0 §9.2.5.1.2.3 and §9.2.5.2.2.3: each element of an array
    /// or a sequence of structures is a Variable `<TypeName>_<index>`, an
    /// ordered component of the collection's Variable, with the members
    /// under it. An array has its elements from the start; a sequence gets
    /// one when a sample brings it, and an element a later sample no
    /// longer has reads `Bad_NoData`.
    #[test]
    fn opcuabridge_5_3_the_elements_of_a_collection_of_structs_are_variables() {
        let mut space = AddressSpace::new();
        let mut topic =
            materialize_instance(&mut space, 2, 0, "Fleet", &fleet(), true).expect("materialize");
        let node = |path: &str| {
            NodeId::string(2, std::format!("Topic:Fleet/Value/{path}")).expect("node id")
        };
        let int = |path: &str, value: i32| FlatField {
            path: path.into(),
            value: Variant::scalar(VariantValue::Int32(value)),
        };
        // The array's two elements are there before any sample.
        assert!(space.node_meta(&node("corners.ShapeType_0")).is_some());
        assert!(topic.leaves.contains_key("corners.ShapeType_1.y"));
        assert!(space.node_meta(&node("corners.ShapeType_2")).is_none());
        assert!(space.node_meta(&node("trail.ShapeType_0")).is_none());
        let components = |reference: &NodeId, subtypes: bool| -> Vec<NodeId> {
            space
                .browse(&node("corners"), 0, Some(reference), subtypes, 0)
                .iter()
                .map(|found| found.target.node_id.clone())
                .collect()
        };
        let elements = [node("corners.ShapeType_0"), node("corners.ShapeType_1")];
        assert_eq!(
            components(&reference_types::HAS_ORDERED_COMPONENT, false),
            elements
        );
        // HasOrderedComponent is a subtype of HasComponent (Part 3 §7.8).
        assert_eq!(components(&reference_types::HAS_COMPONENT, true), elements);
        assert!(components(&reference_types::HAS_COMPONENT, false).is_empty());

        let two = vec![
            int("corners.ShapeType_0.x", 1),
            int("corners.ShapeType_0.y", 2),
            int("corners.ShapeType_1.x", 3),
            int("corners.ShapeType_1.y", 4),
            int("trail.ShapeType_0.x", 10),
            int("trail.ShapeType_0.y", 11),
            int("trail.ShapeType_1.x", 12),
            int("trail.ShapeType_1.y", 13),
        ];
        assert!(apply_sample(&mut space, &mut topic, &two, 100).is_empty());
        let value = |space: &AddressSpace, path: &str| space.value(&node(path)).cloned();
        assert_eq!(
            value(&space, "trail.ShapeType_1.y").and_then(|dv| dv.value),
            Some(Variant::scalar(VariantValue::Int32(13)))
        );

        // The next sample has one element in the sequence.
        let one: Vec<FlatField> = two[..6].to_vec();
        assert!(apply_sample(&mut space, &mut topic, &one, 200).is_empty());
        let gone = value(&space, "trail.ShapeType_1.y").expect("the node stays");
        assert_eq!(gone.value, None);
        assert_eq!(gone.status, Some(zerodds_opcua_server::status::BAD_NO_DATA));
        assert_eq!(
            value(&space, "trail.ShapeType_0.x").and_then(|dv| dv.value),
            Some(Variant::scalar(VariantValue::Int32(10)))
        );
        // And comes back with the element.
        assert!(apply_sample(&mut space, &mut topic, &two, 300).is_empty());
        assert_eq!(
            value(&space, "trail.ShapeType_1.y").and_then(|dv| dv.value),
            Some(Variant::scalar(VariantValue::Int32(13)))
        );
    }

    #[test]
    fn opcuabridge_5_2_scalar_topic_gets_one_leaf_at_the_empty_path() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Temperature", &DdsType::Float64, false)
            .expect("materialize");
        assert_eq!(topic.leaves.len(), 1);
        assert!(topic.leaves.contains_key(""));
        assert_eq!(topic.leaves[""], topic.sample_root);
        assert!(space.node_meta(&topic.topic_node).is_some());
    }

    #[test]
    fn opcuabridge_5_2_struct_topic_gets_one_leaf_per_member() {
        let mut space = AddressSpace::new();
        let topic =
            materialize_instance(&mut space, 2, 0, "Shape", &shape(), false).expect("materialize");
        assert_eq!(topic.leaves.len(), 2);
        assert!(topic.leaves.contains_key("x"));
        assert!(topic.leaves.contains_key("y"));
    }

    #[test]
    fn opcuabridge_6_1_apply_sample_sets_the_right_node_values() {
        let mut space = AddressSpace::new();
        let mut topic =
            materialize_instance(&mut space, 2, 0, "Shape", &shape(), false).expect("materialize");
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
        let unmatched = apply_sample(&mut space, &mut topic, &fields, 1234);
        assert!(unmatched.is_empty());
        let x_node = &topic.leaves["x"];
        assert_eq!(
            space.value(x_node).unwrap().value,
            Some(Variant::scalar(VariantValue::Int32(3)))
        );
    }

    #[test]
    fn opcuabridge_5_3_unmatched_path_is_reported_not_dropped() {
        let mut space = AddressSpace::new();
        let mut topic =
            materialize_instance(&mut space, 2, 0, "Shape", &shape(), false).expect("materialize");
        let fields = vec![FlatField {
            path: "z_does_not_exist".into(),
            value: Variant::scalar(VariantValue::Int32(1)),
        }];
        let unmatched = apply_sample(&mut space, &mut topic, &fields, 0);
        assert_eq!(unmatched, vec!["z_does_not_exist".to_string()]);
    }

    #[test]
    fn opcuabridge_5_2_objects_folder_is_browsable_after_materializing_a_topic() {
        // Without a NodeMeta entry for OBJECTS_FOLDER itself, a Browse
        // starting from it (any client's default entry point) refuses
        // with Bad_NodeIdUnknown even though the Organizes reference to
        // the domain is real — regression guard for that gap.
        let mut space = AddressSpace::new();
        materialize_instance(&mut space, 2, 0, "A", &DdsType::Int32, false).expect("materialize");
        assert!(space.node_meta(&OBJECTS_FOLDER).is_some());
        let refs = space.browse(&OBJECTS_FOLDER, 0, None, false, 0);
        assert!(refs.iter().any(|r| r.target.browse_name.name == "Domain0"));
    }

    #[test]
    fn opcuabridge_5_2_two_topics_do_not_collide() {
        let mut space = AddressSpace::new();
        let a = materialize_instance(&mut space, 2, 0, "A", &DdsType::Int32, false)
            .expect("materialize");
        let b = materialize_instance(&mut space, 2, 0, "B", &DdsType::Int32, true)
            .expect("materialize");
        assert_ne!(a.sample_root, b.sample_root);
        assert_ne!(a.topic_node, b.topic_node);
    }

    #[test]
    fn opcuabridge_5_2_node_id_override_replaces_the_default_root_identity() {
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
    fn opcuabridge_5_2_node_id_override_colliding_with_an_existing_node_is_refused() {
        let mut space = AddressSpace::new();
        materialize_instance(&mut space, 2, 0, "A", &DdsType::Int32, false).expect("materialize");
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
    fn opcuabridge_5_2_empty_browse_path_segment_is_refused() {
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
    fn opcuabridge_5_2_browse_path_organizes_the_root_from_the_objects_folder() {
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
    fn opcuabridge_5_4_historical_depth_marks_every_leaf_historizing() {
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
    fn opcuabridge_5_4_no_historical_depth_leaves_a_leaf_non_historizing() {
        let mut space = AddressSpace::new();
        let topic = materialize_instance(&mut space, 2, 0, "Plain", &DdsType::Int32, false)
            .expect("materialize");
        assert!(!space.is_historizing(&topic.sample_root));
    }

    #[test]
    fn opcuabridge_5_2_dds_to_ua_leaf_is_read_only_ua_to_dds_leaf_is_writable() {
        let mut space = AddressSpace::new();
        let read_only = materialize_instance(&mut space, 2, 0, "FromDds", &DdsType::Int32, false)
            .expect("materialize");
        let writable = materialize_instance(&mut space, 2, 0, "ToDds", &DdsType::Int32, true)
            .expect("materialize");

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
