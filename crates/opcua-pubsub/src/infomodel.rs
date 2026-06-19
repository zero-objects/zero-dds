// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OPC-UA PubSub Information Model (Part 14 §9) — the configuration of a
//! Publisher/Subscriber exposed as a browsable OPC-UA AddressSpace under the
//! well-known `PublishSubscribe` object.
//!
//! [`PubSubConfiguration`] is the nested aggregate root: it owns
//! [`ConnectionModel`]s (each with [`WriterGroupModel`]s and
//! [`ReaderGroupModel`]s) plus PublishedDataSets, and offers the management
//! operations the standard methods perform (`AddConnection`,
//! `AddWriterGroup`, `AddDataSetWriter`, `AddReaderGroup`,
//! `AddDataSetReader`, `AddPublishedDataSet`, `RemoveX`). [`nodes`] renders
//! the whole tree into [`PubSubNode`]s — typed AddressSpace nodes with their
//! TypeDefinition, `HasComponent` parent reference and property values — which
//! an OPC-UA server (the gateway) serves so a client can browse and
//! reconfigure PubSub at runtime.
//!
//! [`nodes`]: PubSubConfiguration::nodes

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{Variant, VariantValue};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{NodeClass, QualifiedName};

use crate::config::{
    DataSetMetaData, DataSetReaderConfig, DataSetWriterConfig, PubSubConnectionConfig,
    ReaderGroupConfig, WriterGroupConfig,
};

// Well-known ns0 NodeIds from the OPC-UA Part 14 NodeSet. Verify against the
// canonical `Opc.Ua.NodeSet2.PubSub.xml` when exact wire interop is required.
/// `PublishSubscribe` object (the AddressSpace root for PubSub config).
pub const PUBLISH_SUBSCRIBE: NodeId = NodeId::numeric(0, 14443);
/// `PubSubConnectionType`.
pub const PUBSUB_CONNECTION_TYPE: NodeId = NodeId::numeric(0, 14209);
/// `WriterGroupType`.
pub const WRITER_GROUP_TYPE: NodeId = NodeId::numeric(0, 17725);
/// `DataSetWriterType`.
pub const DATASET_WRITER_TYPE: NodeId = NodeId::numeric(0, 15298);
/// `ReaderGroupType`.
pub const READER_GROUP_TYPE: NodeId = NodeId::numeric(0, 17999);
/// `DataSetReaderType`.
pub const DATASET_READER_TYPE: NodeId = NodeId::numeric(0, 15306);
/// `PublishedDataSetType`.
pub const PUBLISHED_DATASET_TYPE: NodeId = NodeId::numeric(0, 14509);
/// `BaseDataVariableType` (used for property variables).
pub const BASE_DATA_VARIABLE_TYPE: NodeId = NodeId::numeric(0, 63);
/// `HasComponent` reference type.
pub const HAS_COMPONENT: NodeId = NodeId::numeric(0, 47);
/// `HasProperty` reference type.
pub const HAS_PROPERTY: NodeId = NodeId::numeric(0, 46);

/// An error from a PubSub Information Model operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoModelError {
    /// No node with the given id exists (or it is the wrong kind for the op).
    NotFound(NodeId),
}

impl core::fmt::Display for InfoModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound(n) => write!(f, "PubSub node {n:?} not found"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InfoModelError {}

/// A DataSetWriter node (Part 14 §9.1.7, `DataSetWriterType`).
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetWriterModel {
    /// AddressSpace NodeId.
    pub node_id: NodeId,
    /// Writer configuration.
    pub config: DataSetWriterConfig,
}

/// A DataSetReader node (Part 14 §9.1.8, `DataSetReaderType`).
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetReaderModel {
    /// AddressSpace NodeId.
    pub node_id: NodeId,
    /// Reader configuration.
    pub config: DataSetReaderConfig,
}

/// A WriterGroup node (Part 14 §9.1.5, `WriterGroupType`).
#[derive(Debug, Clone, PartialEq)]
pub struct WriterGroupModel {
    /// AddressSpace NodeId.
    pub node_id: NodeId,
    /// Group configuration.
    pub config: WriterGroupConfig,
    /// Contained DataSetWriters.
    pub writers: Vec<DataSetWriterModel>,
}

/// A ReaderGroup node (Part 14 §9.1.6, `ReaderGroupType`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReaderGroupModel {
    /// AddressSpace NodeId.
    pub node_id: NodeId,
    /// Group configuration.
    pub config: ReaderGroupConfig,
    /// Contained DataSetReaders.
    pub readers: Vec<DataSetReaderModel>,
}

/// A PubSubConnection node (Part 14 §9.1.3, `PubSubConnectionType`).
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionModel {
    /// AddressSpace NodeId.
    pub node_id: NodeId,
    /// Connection configuration.
    pub config: PubSubConnectionConfig,
    /// Contained WriterGroups.
    pub writer_groups: Vec<WriterGroupModel>,
    /// Contained ReaderGroups.
    pub reader_groups: Vec<ReaderGroupModel>,
}

/// A PublishedDataSet node (Part 14 §9.1.4, `PublishedDataSetType`).
#[derive(Debug, Clone, PartialEq)]
pub struct PublishedDataSetModel {
    /// AddressSpace NodeId.
    pub node_id: NodeId,
    /// DataSet metadata (the published layout).
    pub meta_data: DataSetMetaData,
}

/// A rendered AddressSpace node of the PubSub Information Model.
#[derive(Debug, Clone, PartialEq)]
pub struct PubSubNode {
    /// NodeId of this node.
    pub node_id: NodeId,
    /// BrowseName.
    pub browse_name: QualifiedName,
    /// NodeClass (Object for the config objects, Variable for properties).
    pub node_class: NodeClass,
    /// TypeDefinition (the `*Type` for objects, `BaseDataVariableType` for
    /// properties).
    pub type_definition: NodeId,
    /// Parent node this node hangs off (`None` for the root).
    pub parent: Option<NodeId>,
    /// Reference type from the parent (`HasComponent` / `HasProperty`).
    pub reference_from_parent: Option<NodeId>,
    /// Property value (only for `Variable` property nodes).
    pub value: Option<Variant>,
}

/// The PubSub Information Model configuration root (Part 14 §9.1.2,
/// `PublishSubscribeType`).
#[derive(Debug, Clone, PartialEq)]
pub struct PubSubConfiguration {
    namespace_index: u16,
    next_id: u32,
    connections: Vec<ConnectionModel>,
    published_data_sets: Vec<PublishedDataSetModel>,
}

impl PubSubConfiguration {
    /// Creates an empty configuration; instance NodeIds are assigned in
    /// `namespace_index` (the server's PubSub configuration namespace).
    #[must_use]
    pub fn new(namespace_index: u16) -> Self {
        Self {
            namespace_index,
            next_id: 1,
            connections: Vec::new(),
            published_data_sets: Vec::new(),
        }
    }

    fn alloc_node(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        NodeId::numeric(self.namespace_index, id)
    }

    /// The connections.
    #[must_use]
    pub fn connections(&self) -> &[ConnectionModel] {
        &self.connections
    }

    /// The published data sets.
    #[must_use]
    pub fn published_data_sets(&self) -> &[PublishedDataSetModel] {
        &self.published_data_sets
    }

    /// `AddConnection` (Part 14 §9.1.2.5.1) — adds a PubSubConnection.
    pub fn add_connection(&mut self, config: PubSubConnectionConfig) -> NodeId {
        let node_id = self.alloc_node();
        self.connections.push(ConnectionModel {
            node_id: node_id.clone(),
            config,
            writer_groups: Vec::new(),
            reader_groups: Vec::new(),
        });
        node_id
    }

    /// `AddPublishedDataSet` (Part 14 §9.1.2.5.x) — registers a
    /// PublishedDataSet by its metadata.
    pub fn add_published_data_set(&mut self, meta_data: DataSetMetaData) -> NodeId {
        let node_id = self.alloc_node();
        self.published_data_sets.push(PublishedDataSetModel {
            node_id: node_id.clone(),
            meta_data,
        });
        node_id
    }

    fn connection_mut(&mut self, node_id: &NodeId) -> Option<&mut ConnectionModel> {
        self.connections.iter_mut().find(|c| &c.node_id == node_id)
    }

    /// `AddWriterGroup` (Part 14 §9.1.3.7.2) on the given connection.
    ///
    /// # Errors
    /// [`InfoModelError::NotFound`] if `connection` is unknown.
    pub fn add_writer_group(
        &mut self,
        connection: &NodeId,
        config: WriterGroupConfig,
    ) -> Result<NodeId, InfoModelError> {
        let node_id = NodeId::numeric(self.namespace_index, self.next_id);
        let conn = self
            .connection_mut(connection)
            .ok_or_else(|| InfoModelError::NotFound(connection.clone()))?;
        conn.writer_groups.push(WriterGroupModel {
            node_id: node_id.clone(),
            config,
            writers: Vec::new(),
        });
        self.next_id += 1;
        Ok(node_id)
    }

    /// `AddReaderGroup` (Part 14 §9.1.3.7.x) on the given connection.
    ///
    /// # Errors
    /// [`InfoModelError::NotFound`] if `connection` is unknown.
    pub fn add_reader_group(
        &mut self,
        connection: &NodeId,
        config: ReaderGroupConfig,
    ) -> Result<NodeId, InfoModelError> {
        let node_id = NodeId::numeric(self.namespace_index, self.next_id);
        let conn = self
            .connection_mut(connection)
            .ok_or_else(|| InfoModelError::NotFound(connection.clone()))?;
        conn.reader_groups.push(ReaderGroupModel {
            node_id: node_id.clone(),
            config,
            readers: Vec::new(),
        });
        self.next_id += 1;
        Ok(node_id)
    }

    /// `AddDataSetWriter` (Part 14 §9.1.5.5.1) on the given WriterGroup.
    ///
    /// # Errors
    /// [`InfoModelError::NotFound`] if `group` is not a known WriterGroup.
    pub fn add_dataset_writer(
        &mut self,
        group: &NodeId,
        config: DataSetWriterConfig,
    ) -> Result<NodeId, InfoModelError> {
        let node_id = NodeId::numeric(self.namespace_index, self.next_id);
        let g = self
            .connections
            .iter_mut()
            .flat_map(|c| c.writer_groups.iter_mut())
            .find(|g| &g.node_id == group)
            .ok_or_else(|| InfoModelError::NotFound(group.clone()))?;
        g.writers.push(DataSetWriterModel {
            node_id: node_id.clone(),
            config,
        });
        self.next_id += 1;
        Ok(node_id)
    }

    /// `AddDataSetReader` (Part 14 §9.1.6.5.1) on the given ReaderGroup.
    ///
    /// # Errors
    /// [`InfoModelError::NotFound`] if `group` is not a known ReaderGroup.
    pub fn add_dataset_reader(
        &mut self,
        group: &NodeId,
        config: DataSetReaderConfig,
    ) -> Result<NodeId, InfoModelError> {
        let node_id = NodeId::numeric(self.namespace_index, self.next_id);
        let g = self
            .connections
            .iter_mut()
            .flat_map(|c| c.reader_groups.iter_mut())
            .find(|g| &g.node_id == group)
            .ok_or_else(|| InfoModelError::NotFound(group.clone()))?;
        g.readers.push(DataSetReaderModel {
            node_id: node_id.clone(),
            config,
        });
        self.next_id += 1;
        Ok(node_id)
    }

    /// Removes any connection / group / writer / reader / PublishedDataSet by
    /// NodeId (`RemoveConnection` / `RemoveGroup` / `RemoveDataSetWriter` /
    /// `RemoveDataSetReader`), returning `true` if something was removed.
    pub fn remove(&mut self, node_id: &NodeId) -> bool {
        let before = self.count();
        self.published_data_sets.retain(|p| &p.node_id != node_id);
        self.connections.retain(|c| &c.node_id != node_id);
        for conn in &mut self.connections {
            conn.writer_groups.retain(|g| &g.node_id != node_id);
            conn.reader_groups.retain(|g| &g.node_id != node_id);
            for g in &mut conn.writer_groups {
                g.writers.retain(|w| &w.node_id != node_id);
            }
            for g in &mut conn.reader_groups {
                g.readers.retain(|r| &r.node_id != node_id);
            }
        }
        self.count() != before
    }

    fn count(&self) -> usize {
        self.published_data_sets.len()
            + self
                .connections
                .iter()
                .map(|c| {
                    1 + c.writer_groups.len()
                        + c.reader_groups.len()
                        + c.writer_groups
                            .iter()
                            .map(|g| g.writers.len())
                            .sum::<usize>()
                        + c.reader_groups
                            .iter()
                            .map(|g| g.readers.len())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }

    /// Renders the whole configuration into AddressSpace nodes rooted at
    /// [`PUBLISH_SUBSCRIBE`].
    #[must_use]
    pub fn nodes(&self) -> Vec<PubSubNode> {
        let mut out = Vec::new();
        let root = PUBLISH_SUBSCRIBE;
        for conn in &self.connections {
            out.push(object_node(
                &conn.node_id,
                &conn.config.name,
                self.namespace_index,
                PUBSUB_CONNECTION_TYPE,
                &root,
            ));
            self.push_props(
                &mut out,
                &conn.node_id,
                &[
                    ("PublisherId", publisher_id_variant(&conn.config)),
                    (
                        "TransportProfileUri",
                        str_variant(&conn.config.transport_profile_uri),
                    ),
                    ("Address", str_variant(&conn.config.address_url)),
                ],
            );
            for g in &conn.writer_groups {
                out.push(object_node(
                    &g.node_id,
                    &g.config.name,
                    self.namespace_index,
                    WRITER_GROUP_TYPE,
                    &conn.node_id,
                ));
                self.push_props(
                    &mut out,
                    &g.node_id,
                    &[
                        ("WriterGroupId", u16_variant(g.config.writer_group_id)),
                        (
                            "PublishingInterval",
                            f64_variant(g.config.publishing_interval_ms),
                        ),
                    ],
                );
                for w in &g.writers {
                    out.push(object_node(
                        &w.node_id,
                        &w.config.name,
                        self.namespace_index,
                        DATASET_WRITER_TYPE,
                        &g.node_id,
                    ));
                    self.push_props(
                        &mut out,
                        &w.node_id,
                        &[
                            ("DataSetWriterId", u16_variant(w.config.data_set_writer_id)),
                            (
                                "KeyFrameCount",
                                Variant::scalar(VariantValue::UInt32(w.config.key_frame_count)),
                            ),
                            ("DataSetName", str_variant(&w.config.data_set_name)),
                        ],
                    );
                }
            }
            for g in &conn.reader_groups {
                out.push(object_node(
                    &g.node_id,
                    &g.config.name,
                    self.namespace_index,
                    READER_GROUP_TYPE,
                    &conn.node_id,
                ));
                for r in &g.readers {
                    out.push(object_node(
                        &r.node_id,
                        &r.config.name,
                        self.namespace_index,
                        DATASET_READER_TYPE,
                        &g.node_id,
                    ));
                    self.push_props(
                        &mut out,
                        &r.node_id,
                        &[("DataSetWriterId", u16_variant(r.config.data_set_writer_id))],
                    );
                }
            }
        }
        for pds in &self.published_data_sets {
            out.push(object_node(
                &pds.node_id,
                &pds.meta_data.name,
                self.namespace_index,
                PUBLISHED_DATASET_TYPE,
                &root,
            ));
        }
        out
    }

    fn push_props(&self, out: &mut Vec<PubSubNode>, parent: &NodeId, props: &[(&str, Variant)]) {
        for (name, value) in props {
            out.push(PubSubNode {
                node_id: NodeId {
                    namespace_index: self.namespace_index,
                    identifier_type: zerodds_opcua_gateway::node_id::NodeIdentifier::String(
                        property_id(parent, name),
                    ),
                },
                browse_name: QualifiedName {
                    namespace_index: self.namespace_index,
                    name: String::from(*name),
                },
                node_class: NodeClass::Variable,
                type_definition: BASE_DATA_VARIABLE_TYPE,
                parent: Some(parent.clone()),
                reference_from_parent: Some(HAS_PROPERTY),
                value: Some(value.clone()),
            });
        }
    }
}

fn object_node(
    node_id: &NodeId,
    name: &str,
    ns: u16,
    type_definition: NodeId,
    parent: &NodeId,
) -> PubSubNode {
    PubSubNode {
        node_id: node_id.clone(),
        browse_name: QualifiedName {
            namespace_index: ns,
            name: String::from(name),
        },
        node_class: NodeClass::Object,
        type_definition,
        parent: Some(parent.clone()),
        reference_from_parent: Some(HAS_COMPONENT),
        value: None,
    }
}

fn property_id(parent: &NodeId, name: &str) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    let _ = write!(&mut s, "{parent:?}.{name}");
    s
}

fn str_variant(s: &str) -> Variant {
    Variant::scalar(VariantValue::String(String::from(s)))
}

fn u16_variant(v: u16) -> Variant {
    Variant::scalar(VariantValue::UInt16(v))
}

fn f64_variant(v: f64) -> Variant {
    Variant::scalar(VariantValue::Double(v))
}

fn publisher_id_variant(c: &PubSubConnectionConfig) -> Variant {
    use crate::uadp::network_message::PublisherId;
    match &c.publisher_id {
        PublisherId::Byte(v) => Variant::scalar(VariantValue::Byte(*v)),
        PublisherId::UInt16(v) => Variant::scalar(VariantValue::UInt16(*v)),
        PublisherId::UInt32(v) => Variant::scalar(VariantValue::UInt32(*v)),
        PublisherId::UInt64(v) => Variant::scalar(VariantValue::UInt64(*v)),
        PublisherId::String(s) => Variant::scalar(VariantValue::String(s.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uadp::network_message::PublisherId;

    fn connection() -> PubSubConnectionConfig {
        PubSubConnectionConfig {
            name: String::from("conn1"),
            publisher_id: PublisherId::UInt16(9),
            transport_profile_uri: String::from("uadp"),
            address_url: String::from("opc.udp://239.0.0.1:4840"),
        }
    }

    #[test]
    fn builds_and_browses_hierarchy() {
        let mut cfg = PubSubConfiguration::new(1);
        let conn = cfg.add_connection(connection());
        let wg = cfg
            .add_writer_group(&conn, WriterGroupConfig::new("wg1", 1))
            .expect("wg");
        let w = cfg
            .add_dataset_writer(&wg, DataSetWriterConfig::new("w1", 5, "ds1"))
            .expect("w");
        let rg = cfg
            .add_reader_group(&conn, ReaderGroupConfig::default())
            .expect("rg");
        cfg.add_dataset_reader(
            &rg,
            DataSetReaderConfig::new("r1", DataSetMetaData::default()),
        )
        .expect("r");
        cfg.add_published_data_set(DataSetMetaData::new("ds1", Vec::new()));

        let nodes = cfg.nodes();
        // Connection object node hangs off the PublishSubscribe root with the
        // PubSubConnectionType definition.
        let conn_node = nodes.iter().find(|n| n.node_id == conn).expect("conn node");
        assert_eq!(conn_node.parent.as_ref(), Some(&PUBLISH_SUBSCRIBE));
        assert_eq!(conn_node.type_definition, PUBSUB_CONNECTION_TYPE);
        assert_eq!(conn_node.node_class, NodeClass::Object);

        // The writer object node hangs off the writer group.
        let w_node = nodes.iter().find(|n| n.node_id == w).expect("writer node");
        assert_eq!(w_node.parent.as_ref(), Some(&wg));
        assert_eq!(w_node.type_definition, DATASET_WRITER_TYPE);

        // A DataSetWriterId property variable exists under the writer.
        assert!(nodes.iter().any(|n| {
            n.parent.as_ref() == Some(&w)
                && n.browse_name.name == "DataSetWriterId"
                && n.value == Some(u16_variant(5))
        }));
    }

    #[test]
    fn add_to_unknown_group_is_rejected() {
        let mut cfg = PubSubConfiguration::new(1);
        let phantom = NodeId::numeric(1, 999);
        assert_eq!(
            cfg.add_dataset_writer(&phantom, DataSetWriterConfig::new("w", 1, "ds")),
            Err(InfoModelError::NotFound(phantom))
        );
    }

    #[test]
    fn remove_prunes_subtree_entries() {
        let mut cfg = PubSubConfiguration::new(1);
        let conn = cfg.add_connection(connection());
        let wg = cfg
            .add_writer_group(&conn, WriterGroupConfig::new("wg1", 1))
            .expect("wg");
        let _w = cfg
            .add_dataset_writer(&wg, DataSetWriterConfig::new("w1", 5, "ds1"))
            .expect("w");

        // Removing the writer group drops it (the writer goes with it on the
        // connection's group vector).
        assert!(cfg.remove(&wg));
        assert!(cfg.connections()[0].writer_groups.is_empty());
        // Removing the connection empties the model.
        assert!(cfg.remove(&conn));
        assert!(cfg.connections().is_empty());
        // Removing an unknown node is a no-op.
        assert!(!cfg.remove(&NodeId::numeric(1, 4242)));
    }
}
