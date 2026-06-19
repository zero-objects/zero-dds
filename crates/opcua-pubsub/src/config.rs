// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PubSub configuration model (OPC Foundation Part 14 §6.2 / §9.1) — the
//! declarative objects that describe *what* is published and *how* it is
//! framed, consumed by the [`crate::writer`] and [`crate::reader`] runtime.
//!
//! The model mirrors the OPC-UA PubSub information model: a
//! [`PubSubConnectionConfig`] owns [`WriterGroupConfig`]s and
//! [`ReaderGroupConfig`]s; a [`WriterGroupConfig`] owns
//! [`DataSetWriterConfig`]s that each reference a PublishedDataSet by name;
//! a [`DataSetReaderConfig`] carries the [`DataSetMetaData`] needed to
//! interpret RawData-encoded messages.
//!
//! These are pure in-memory descriptors. Their on-wire serialisation as
//! `DataSetMetaData` for discovery is stage 4; their transport binding is
//! stage 6.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::Variant;
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{BuiltinTypeKind, Guid, LocalizedText, QualifiedName};

use crate::uadp::dataset_message::FieldEncoding;
use crate::uadp::network_message::PublisherId;

/// `DataSetFieldContentMask` (Part 14 §6.2.4.2) — selects how each DataSet
/// field value is encoded inside a DataSetMessage.
///
/// An empty mask means each field is a bare `Variant`. The `RAW_DATA` bit
/// switches to the type-tag-free RawData encoding (which needs
/// [`DataSetMetaData`] to interpret). Any other bit selects the `DataValue`
/// encoding and additionally controls which DataValue members are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataSetFieldContentMask(u32);

impl DataSetFieldContentMask {
    /// Include the field `StatusCode`.
    pub const STATUS_CODE: u32 = 0x01;
    /// Include the field `SourceTimestamp`.
    pub const SOURCE_TIMESTAMP: u32 = 0x02;
    /// Include the field `ServerTimestamp`.
    pub const SERVER_TIMESTAMP: u32 = 0x04;
    /// Include the field source `PicoSeconds`.
    pub const SOURCE_PICOSECONDS: u32 = 0x08;
    /// Include the field server `PicoSeconds`.
    pub const SERVER_PICOSECONDS: u32 = 0x10;
    /// Encode fields as RawData (no per-field type tags).
    pub const RAW_DATA: u32 = 0x20;

    /// Builds a mask from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// The raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// `true` if `flag` is set.
    #[must_use]
    pub const fn contains(self, flag: u32) -> bool {
        self.0 & flag != 0
    }

    /// The default Variant encoding (empty mask).
    #[must_use]
    pub const fn variant() -> Self {
        Self(0)
    }

    /// The RawData encoding.
    #[must_use]
    pub const fn raw_data() -> Self {
        Self(Self::RAW_DATA)
    }

    /// The [`FieldEncoding`] implied by this mask (Part 14 §6.2.4.2): the
    /// `RAW_DATA` bit takes precedence, an empty mask is `Variant`, and any
    /// other bit selects `DataValue`.
    #[must_use]
    pub const fn field_encoding(self) -> FieldEncoding {
        if self.0 & Self::RAW_DATA != 0 {
            FieldEncoding::RawData
        } else if self.0 == 0 {
            FieldEncoding::Variant
        } else {
            FieldEncoding::DataValue
        }
    }
}

/// `UadpDataSetMessageContentMask` (Part 14 §7.2.3.3.2) — which optional
/// DataSetMessage header fields a DataSetWriter emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataSetMessageContentMask(u32);

impl DataSetMessageContentMask {
    /// Emit the DataSetMessage `Timestamp`.
    pub const TIMESTAMP: u32 = 0x01;
    /// Emit the DataSetMessage `PicoSeconds`.
    pub const PICOSECONDS: u32 = 0x02;
    /// Emit the DataSetMessage `Status`.
    pub const STATUS: u32 = 0x04;
    /// Emit the configuration `MajorVersion`.
    pub const MAJOR_VERSION: u32 = 0x08;
    /// Emit the configuration `MinorVersion`.
    pub const MINOR_VERSION: u32 = 0x10;
    /// Emit a per-message `SequenceNumber`.
    pub const SEQUENCE_NUMBER: u32 = 0x20;

    /// Builds a mask from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// The raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// `true` if `flag` is set.
    #[must_use]
    pub const fn contains(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// `UadpNetworkMessageContentMask` (Part 14 §7.2.3.2.2) — which optional
/// NetworkMessage header fields a WriterGroup emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetworkMessageContentMask(u32);

impl NetworkMessageContentMask {
    /// Emit the `PublisherId`.
    pub const PUBLISHER_ID: u32 = 0x01;
    /// Emit a `GroupHeader` block.
    pub const GROUP_HEADER: u32 = 0x02;
    /// Emit the `WriterGroupId` inside the GroupHeader.
    pub const WRITER_GROUP_ID: u32 = 0x04;
    /// Emit the `GroupVersion` inside the GroupHeader.
    pub const GROUP_VERSION: u32 = 0x08;
    /// Emit the `NetworkMessageNumber` inside the GroupHeader.
    pub const NETWORK_MESSAGE_NUMBER: u32 = 0x10;
    /// Emit the group `SequenceNumber` inside the GroupHeader.
    pub const SEQUENCE_NUMBER: u32 = 0x20;
    /// Emit a `PayloadHeader` (DataSetWriter-id list).
    pub const PAYLOAD_HEADER: u32 = 0x40;
    /// Emit the NetworkMessage `Timestamp`.
    pub const TIMESTAMP: u32 = 0x80;
    /// Emit the NetworkMessage `PicoSeconds`.
    pub const PICOSECONDS: u32 = 0x100;
    /// Emit the `DataSetClassId`.
    pub const DATASET_CLASS_ID: u32 = 0x200;
    /// Emit `PromotedFields`.
    pub const PROMOTED_FIELDS: u32 = 0x400;

    /// Builds a mask from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// The raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// `true` if `flag` is set.
    #[must_use]
    pub const fn contains(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// `ConfigurationVersion` (Part 14 §6.2.3.2.5) — a pair of `VersionTime`
/// counters identifying a DataSet layout. A subscriber rejects a message
/// whose major version differs from the one it was configured for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConfigurationVersion {
    /// Incremented on a structural (incompatible) change.
    pub major_version: u32,
    /// Incremented on a compatible change.
    pub minor_version: u32,
}

/// A property attached to a [`FieldMetaData`] (Part 14 §6.2.3.2.3,
/// `KeyValuePair`).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyValuePair {
    /// Browse-name key.
    pub key: QualifiedName,
    /// Property value.
    pub value: Variant,
}

/// `FieldMetaData` (Part 14 §6.2.3.2.3) — the description of one DataSet
/// field, sufficient to decode a RawData-encoded value and to map it to a
/// target variable.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMetaData {
    /// Field name (matches the PublishedDataSet field name).
    pub name: String,
    /// Human-readable description.
    pub description: LocalizedText,
    /// `DataSetFieldFlags` bitfield (bit 0 = PromotedField).
    pub field_flags: u16,
    /// Built-in type of the value (drives RawData decoding).
    pub builtin_type: BuiltinTypeKind,
    /// Concrete DataType NodeId (equals the built-in type for builtins).
    pub data_type: NodeId,
    /// Value rank: `-1` scalar, `0` one-or-more, `>=1` fixed array rank.
    pub value_rank: i32,
    /// Array dimensions when `value_rank >= 1`.
    pub array_dimensions: Vec<u32>,
    /// Maximum string/byte-string length, `0` = unbounded.
    pub max_string_length: u32,
    /// Stable field identifier.
    pub data_set_field_id: Guid,
    /// Additional field properties.
    pub properties: Vec<KeyValuePair>,
}

impl FieldMetaData {
    /// Builds a scalar field's metadata with default flags and identity.
    #[must_use]
    pub fn scalar(name: impl Into<String>, builtin_type: BuiltinTypeKind) -> Self {
        Self {
            name: name.into(),
            description: LocalizedText {
                locale: None,
                text: None,
            },
            field_flags: 0,
            builtin_type,
            data_type: NodeId::numeric(0, u32::from(builtin_type.value())),
            value_rank: -1,
            array_dimensions: Vec::new(),
            max_string_length: 0,
            data_set_field_id: Guid::from_bytes([0; 16]),
            properties: Vec::new(),
        }
    }

    /// `true` if this field is a promoted field (`DataSetFieldFlags` bit 0).
    #[must_use]
    pub const fn is_promoted(&self) -> bool {
        self.field_flags & 0x0001 != 0
    }
}

/// Kind of a `StructureDefinition` (OPC-UA Part 3 §8.49, `StructureType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StructureType {
    /// A plain structure.
    #[default]
    Structure,
    /// A structure with optional fields.
    StructureWithOptionalFields,
    /// A union.
    Union,
    /// A structure whose fields may carry subtyped values.
    StructureWithSubtypedValues,
    /// A union whose fields may carry subtyped values.
    UnionWithSubtypedValues,
}

impl StructureType {
    /// Wire value (Part 3 enum).
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        match self {
            Self::Structure => 0,
            Self::StructureWithOptionalFields => 1,
            Self::Union => 2,
            Self::StructureWithSubtypedValues => 3,
            Self::UnionWithSubtypedValues => 4,
        }
    }

    /// Builds from a wire value.
    #[must_use]
    pub const fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Structure,
            1 => Self::StructureWithOptionalFields,
            2 => Self::Union,
            3 => Self::StructureWithSubtypedValues,
            4 => Self::UnionWithSubtypedValues,
            _ => return None,
        })
    }
}

/// One field of a `StructureDefinition` (Part 3 §8.51, `StructureField`).
#[derive(Debug, Clone, PartialEq)]
pub struct StructureField {
    /// Field name.
    pub name: String,
    /// Description.
    pub description: LocalizedText,
    /// Field DataType.
    pub data_type: NodeId,
    /// Value rank (`-1` scalar).
    pub value_rank: i32,
    /// Array dimensions.
    pub array_dimensions: Vec<u32>,
    /// Maximum string length (`0` = unbounded).
    pub max_string_length: u32,
    /// Whether the field is optional.
    pub is_optional: bool,
}

/// A structured DataType layout (Part 3 §8.48, `StructureDefinition`).
#[derive(Debug, Clone, PartialEq)]
pub struct StructureDefinition {
    /// NodeId of the default binary encoding.
    pub default_encoding_id: NodeId,
    /// Base DataType.
    pub base_data_type: NodeId,
    /// Structure kind.
    pub structure_type: StructureType,
    /// Ordered fields.
    pub fields: Vec<StructureField>,
}

/// Describes a custom structured DataType (Part 6 §, `StructureDescription`).
#[derive(Debug, Clone, PartialEq)]
pub struct StructureDescription {
    /// DataType NodeId.
    pub data_type_id: NodeId,
    /// Browse name.
    pub name: QualifiedName,
    /// The layout.
    pub structure_definition: StructureDefinition,
}

/// One member of an `EnumDefinition` (Part 3 §8.46, `EnumField`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumField {
    /// Enumeration value.
    pub value: i64,
    /// Display name.
    pub display_name: LocalizedText,
    /// Description.
    pub description: LocalizedText,
    /// Field name.
    pub name: String,
}

/// A custom enumeration layout (Part 3 §8.45, `EnumDefinition`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnumDefinition {
    /// Ordered members.
    pub fields: Vec<EnumField>,
}

/// Describes a custom enumeration DataType (`EnumDescription`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDescription {
    /// DataType NodeId.
    pub data_type_id: NodeId,
    /// Browse name.
    pub name: QualifiedName,
    /// The enumeration layout.
    pub enum_definition: EnumDefinition,
    /// Built-in type the enumeration is based on (BuiltInType byte).
    pub builtin_type: u8,
}

/// Describes a custom simple (sub-typed scalar) DataType
/// (`SimpleTypeDescription`).
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleTypeDescription {
    /// DataType NodeId.
    pub data_type_id: NodeId,
    /// Browse name.
    pub name: QualifiedName,
    /// Base DataType this type refines.
    pub base_data_type: NodeId,
    /// Underlying built-in type (BuiltInType byte).
    pub builtin_type: u8,
}

/// `DataSetMetaData` (Part 14 §6.2.3.2.2) — the field descriptions of a
/// DataSet plus the custom DataType descriptions and the configuration
/// version (the full `DataSetMetaDataType` / `DataTypeSchemaHeader`).
///
/// The [`structure_data_types`](Self::structure_data_types),
/// [`enum_data_types`](Self::enum_data_types) and
/// [`simple_data_types`](Self::simple_data_types) arrays describe non-built-in
/// DataTypes referenced by the fields and round-trip on the wire. Decoding a
/// RawData *value* of such a custom type additionally requires dynamic
/// structured-type support in `zerodds-opcua-gateway` (its `Variant` models
/// only the 25 built-in types); built-in-typed fields are fully covered by
/// [`FieldMetaData::builtin_type`].
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetMetaData {
    /// DataSet name.
    pub name: String,
    /// Human-readable description.
    pub description: LocalizedText,
    /// Namespace URIs referenced by the field NodeIds.
    pub namespaces: Vec<String>,
    /// Custom structured DataType descriptions.
    pub structure_data_types: Vec<StructureDescription>,
    /// Custom enumeration DataType descriptions.
    pub enum_data_types: Vec<EnumDescription>,
    /// Custom simple (sub-typed scalar) DataType descriptions.
    pub simple_data_types: Vec<SimpleTypeDescription>,
    /// Ordered field descriptions.
    pub fields: Vec<FieldMetaData>,
    /// DataSetClass this layout belongs to (null Guid = unclassified).
    pub data_set_class_id: Guid,
    /// Layout version.
    pub configuration_version: ConfigurationVersion,
}

impl Default for DataSetMetaData {
    fn default() -> Self {
        // The gateway `LocalizedText`/`Guid` have no `Default`, so this
        // cannot be derived.
        Self {
            name: String::new(),
            description: LocalizedText {
                locale: None,
                text: None,
            },
            namespaces: Vec::new(),
            structure_data_types: Vec::new(),
            enum_data_types: Vec::new(),
            simple_data_types: Vec::new(),
            fields: Vec::new(),
            data_set_class_id: Guid::from_bytes([0; 16]),
            configuration_version: ConfigurationVersion::default(),
        }
    }
}

impl DataSetMetaData {
    /// Builds metadata from a name and an ordered list of fields, leaving the
    /// configuration version at `0.0`.
    #[must_use]
    pub fn new(name: impl Into<String>, fields: Vec<FieldMetaData>) -> Self {
        Self {
            name: name.into(),
            fields,
            ..Self::default()
        }
    }
}

/// `DataSetWriterConfig` (Part 14 §6.2.4) — binds a PublishedDataSet to a
/// numeric writer id and the encoding/key-frame policy used to turn its
/// samples into DataSetMessages.
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetWriterConfig {
    /// Human-readable writer name.
    pub name: String,
    /// Numeric DataSetWriter id (referenced from the PayloadHeader).
    pub data_set_writer_id: u16,
    /// Name of the PublishedDataSet this writer publishes.
    pub data_set_name: String,
    /// Field value encoding policy.
    pub field_content_mask: DataSetFieldContentMask,
    /// Emit a full KeyFrame every `key_frame_count` publishes; intermediate
    /// publishes are DeltaFrames. `1` means every message is a KeyFrame;
    /// `0` is treated as `1`.
    pub key_frame_count: u32,
    /// Which DataSetMessage header fields to emit.
    pub message_content_mask: DataSetMessageContentMask,
}

impl DataSetWriterConfig {
    /// A KeyFrame-only, Variant-encoded writer with no optional header
    /// fields — the simplest valid configuration.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        data_set_writer_id: u16,
        data_set_name: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            data_set_writer_id,
            data_set_name: data_set_name.into(),
            field_content_mask: DataSetFieldContentMask::variant(),
            key_frame_count: 1,
            message_content_mask: DataSetMessageContentMask::default(),
        }
    }
}

/// `WriterGroupConfig` (Part 14 §6.2.5) — groups DataSetWriters that publish
/// together into one NetworkMessage stream.
#[derive(Debug, Clone, PartialEq)]
pub struct WriterGroupConfig {
    /// Human-readable group name.
    pub name: String,
    /// Numeric WriterGroup id.
    pub writer_group_id: u16,
    /// Publishing cycle in milliseconds (informational at this layer;
    /// the transport carrier drives the cycle in stage 6).
    pub publishing_interval_ms: f64,
    /// Group configuration version.
    pub group_version: u32,
    /// Which NetworkMessage header fields to emit.
    pub network_message_content_mask: NetworkMessageContentMask,
}

impl WriterGroupConfig {
    /// A group emitting a PayloadHeader and a GroupHeader carrying the
    /// WriterGroupId and a rolling SequenceNumber — the common default.
    #[must_use]
    pub fn new(name: impl Into<String>, writer_group_id: u16) -> Self {
        Self {
            name: name.into(),
            writer_group_id,
            publishing_interval_ms: 1000.0,
            group_version: 0,
            network_message_content_mask: NetworkMessageContentMask::from_bits(
                NetworkMessageContentMask::PAYLOAD_HEADER
                    | NetworkMessageContentMask::GROUP_HEADER
                    | NetworkMessageContentMask::WRITER_GROUP_ID
                    | NetworkMessageContentMask::SEQUENCE_NUMBER,
            ),
        }
    }
}

/// `DataSetReaderConfig` (Part 14 §6.2.8) — the filter and decoding policy a
/// subscriber applies to incoming NetworkMessages.
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetReaderConfig {
    /// Human-readable reader name.
    pub name: String,
    /// Accept only messages from this Publisher; `None` = any publisher.
    pub publisher_id: Option<PublisherId>,
    /// Accept only this WriterGroup; `0` = any group.
    pub writer_group_id: u16,
    /// Accept only this DataSetWriter; `0` = any writer.
    pub data_set_writer_id: u16,
    /// Expected field encoding policy (must match the writer for RawData).
    pub field_content_mask: DataSetFieldContentMask,
    /// Expected DataSetMessage header fields.
    pub message_content_mask: DataSetMessageContentMask,
    /// Field descriptions used to decode RawData and to name decoded fields.
    pub meta_data: DataSetMetaData,
}

impl DataSetReaderConfig {
    /// A reader accepting any publisher/group/writer with the given
    /// metadata and a Variant field encoding.
    #[must_use]
    pub fn new(name: impl Into<String>, meta_data: DataSetMetaData) -> Self {
        Self {
            name: name.into(),
            publisher_id: None,
            writer_group_id: 0,
            data_set_writer_id: 0,
            field_content_mask: DataSetFieldContentMask::variant(),
            message_content_mask: DataSetMessageContentMask::default(),
            meta_data,
        }
    }
}

/// `ReaderGroupConfig` (Part 14 §6.2.7) — groups DataSetReaders sharing a
/// connection.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReaderGroupConfig {
    /// Human-readable group name.
    pub name: String,
}

/// `PubSubConnectionConfig` (Part 14 §6.2.6) — a Publisher identity plus the
/// transport binding shared by its writer and reader groups.
#[derive(Debug, Clone, PartialEq)]
pub struct PubSubConnectionConfig {
    /// Human-readable connection name.
    pub name: String,
    /// Identity advertised in every NetworkMessage from this connection.
    pub publisher_id: PublisherId,
    /// Transport profile URI (e.g. the UADP-over-UDP profile).
    pub transport_profile_uri: String,
    /// Transport address (e.g. `opc.udp://239.0.0.1:4840`).
    pub address_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_mask_selects_field_encoding() {
        assert_eq!(
            DataSetFieldContentMask::variant().field_encoding(),
            FieldEncoding::Variant
        );
        assert_eq!(
            DataSetFieldContentMask::raw_data().field_encoding(),
            FieldEncoding::RawData
        );
        assert_eq!(
            DataSetFieldContentMask::from_bits(DataSetFieldContentMask::SOURCE_TIMESTAMP)
                .field_encoding(),
            FieldEncoding::DataValue
        );
        // RawData wins over any DataValue selector bit.
        assert_eq!(
            DataSetFieldContentMask::from_bits(
                DataSetFieldContentMask::RAW_DATA | DataSetFieldContentMask::STATUS_CODE
            )
            .field_encoding(),
            FieldEncoding::RawData
        );
    }

    #[test]
    fn field_metadata_scalar_defaults() {
        let f = FieldMetaData::scalar("temperature", BuiltinTypeKind::Double);
        assert_eq!(f.value_rank, -1);
        assert_eq!(f.builtin_type, BuiltinTypeKind::Double);
        assert!(!f.is_promoted());
    }

    #[test]
    fn writer_group_default_mask_has_payload_and_group_header() {
        let g = WriterGroupConfig::new("g1", 7);
        let m = g.network_message_content_mask;
        assert!(m.contains(NetworkMessageContentMask::PAYLOAD_HEADER));
        assert!(m.contains(NetworkMessageContentMask::GROUP_HEADER));
        assert!(m.contains(NetworkMessageContentMask::WRITER_GROUP_ID));
        assert!(m.contains(NetworkMessageContentMask::SEQUENCE_NUMBER));
    }
}
