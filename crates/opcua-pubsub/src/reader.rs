// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The subscribing side of OPC-UA PubSub (Part 14 §6.2.8 / §6.2.7).
//!
//! A [`DataSetReader`] filters incoming [`NetworkMessage`]s by Publisher,
//! WriterGroup and DataSetWriter id, then decodes each matching
//! [`DataSetMessage`] into named field values using its
//! [`DataSetMetaData`]. The Variant and DataValue encodings are
//! self-describing; the RawData encoding is decoded against the metadata's
//! built-in types. A [`ReaderGroup`] dispatches one NetworkMessage to all of
//! its readers.
//!
//! This layer consumes the in-memory message tree produced by
//! [`crate::writer`]; receiving the bytes off a transport is stage 6.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant};

use crate::binary::{UaReader, decode_builtin_value};
use crate::config::{DataSetMetaData, DataSetReaderConfig};
use crate::dynamic::{DynamicField, DynamicValue, decode_raw_dataset, variant_to_dynamic};
use crate::error::DecodeError;
use crate::uadp::dataset_message::{DataSetData, DataSetMessage, DataSetMessageKind};
use crate::uadp::network_message::NetworkMessage;

/// One decoded DataSet field paired with its configured name.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedField {
    /// Field name from the [`DataSetMetaData`] (or `field{index}` when the
    /// metadata is shorter than the message).
    pub name: String,
    /// Index of the field within the DataSet.
    pub index: u16,
    /// Decoded value. For the Variant and RawData encodings only
    /// [`DataValue::value`] is populated; the DataValue encoding fills the
    /// status and timestamps the writer included.
    pub value: DataValue,
}

/// The result of decoding one matching [`DataSetMessage`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedDataSet {
    /// DataSetWriter id taken from the NetworkMessage PayloadHeader.
    pub writer_id: u16,
    /// Whether this was a KeyFrame, DeltaFrame, Event or KeepAlive.
    pub kind: DataSetMessageKind,
    /// Decoded fields. A KeyFrame carries every field; a DeltaFrame carries
    /// only the fields that changed, each with its original index.
    pub fields: Vec<ReceivedField>,
}

/// A matched DataSet annotated with the reader that accepted it.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchedDataSet {
    /// Name of the [`DataSetReader`] that accepted the message.
    pub reader_name: String,
    /// The decoded DataSet.
    pub data: ReceivedDataSet,
}

/// A DataSetReader (Part 14 §6.2.8) — accepts and decodes the
/// DataSetMessages it is configured for.
#[derive(Debug, Clone)]
pub struct DataSetReader {
    config: DataSetReaderConfig,
}

impl DataSetReader {
    /// Creates a reader for `config`.
    #[must_use]
    pub fn new(config: DataSetReaderConfig) -> Self {
        Self { config }
    }

    /// The reader configuration.
    #[must_use]
    pub fn config(&self) -> &DataSetReaderConfig {
        &self.config
    }

    /// `true` if `message` (carried by `network`) passes this reader's
    /// Publisher, WriterGroup and DataSetWriter filters. A filter set to its
    /// wildcard (`None` publisher / `0` group / `0` writer) matches anything.
    #[must_use]
    pub fn matches(&self, network: &NetworkMessage, message: &DataSetMessage) -> bool {
        if let Some(expected) = &self.config.publisher_id {
            if network.publisher_id.as_ref() != Some(expected) {
                return false;
            }
        }
        if self.config.writer_group_id != 0 {
            let group_id = network
                .group_header
                .as_ref()
                .and_then(|g| g.writer_group_id);
            if group_id != Some(self.config.writer_group_id) {
                return false;
            }
        }
        if self.config.data_set_writer_id != 0
            && message.writer_id != self.config.data_set_writer_id
        {
            return false;
        }
        true
    }

    /// Decodes one DataSetMessage into named fields using the configured
    /// metadata.
    ///
    /// # Errors
    /// Returns [`DecodeError`] if a RawData payload cannot be decoded against
    /// the metadata (missing field descriptions, truncation, or trailing
    /// bytes).
    pub fn decode(&self, message: &DataSetMessage) -> Result<ReceivedDataSet, DecodeError> {
        let meta = &self.config.meta_data;
        let fields = match &message.data {
            DataSetData::None => Vec::new(),
            DataSetData::Variant(values) => values
                .iter()
                .enumerate()
                .map(|(i, v)| received(meta, i, value_only(v.clone())))
                .collect(),
            DataSetData::DataValue(values) => values
                .iter()
                .enumerate()
                .map(|(i, dv)| received(meta, i, dv.clone()))
                .collect(),
            DataSetData::DeltaVariant(values) => values
                .iter()
                .map(|(idx, v)| received(meta, *idx as usize, value_only(v.clone())))
                .collect(),
            DataSetData::DeltaDataValue(values) => values
                .iter()
                .map(|(idx, dv)| received(meta, *idx as usize, dv.clone()))
                .collect(),
            DataSetData::Raw(bytes) => decode_raw(meta, bytes)?,
        };

        Ok(ReceivedDataSet {
            writer_id: message.writer_id,
            kind: message.kind,
            fields,
        })
    }

    /// Decodes one DataSetMessage into a uniform [`DynamicValue`] tree,
    /// resolving custom (non-built-in) DataTypes in RawData against the
    /// configured [`DataSetMetaData`] descriptions.
    ///
    /// Unlike [`decode`](Self::decode) — which yields built-in [`Variant`]s and
    /// cannot represent custom structs — this fully expands `StructureField`s,
    /// unions, optional fields, enumerations and nested arrays. The
    /// self-describing Variant/DataValue encodings are mapped to the same
    /// uniform view.
    ///
    /// # Errors
    /// Returns [`DecodeError`] if a RawData payload cannot be decoded against
    /// the metadata.
    pub fn decode_dynamic(
        &self,
        message: &DataSetMessage,
    ) -> Result<Vec<DynamicField>, DecodeError> {
        let meta = &self.config.meta_data;
        let dyn_field = |index: usize, value: DynamicValue| DynamicField {
            name: field_name(meta, index),
            value,
        };
        Ok(match &message.data {
            DataSetData::None => Vec::new(),
            DataSetData::Raw(bytes) => decode_raw_dataset(bytes, meta)?,
            DataSetData::Variant(values) => values
                .iter()
                .enumerate()
                .map(|(i, v)| dyn_field(i, variant_to_dynamic(v)))
                .collect(),
            DataSetData::DataValue(values) => values
                .iter()
                .enumerate()
                .map(|(i, dv)| {
                    dyn_field(
                        i,
                        dv.value
                            .as_ref()
                            .map_or(DynamicValue::Null, variant_to_dynamic),
                    )
                })
                .collect(),
            DataSetData::DeltaVariant(values) => values
                .iter()
                .map(|(idx, v)| dyn_field(*idx as usize, variant_to_dynamic(v)))
                .collect(),
            DataSetData::DeltaDataValue(values) => values
                .iter()
                .map(|(idx, dv)| {
                    dyn_field(
                        *idx as usize,
                        dv.value
                            .as_ref()
                            .map_or(DynamicValue::Null, variant_to_dynamic),
                    )
                })
                .collect(),
        })
    }
}

/// Wraps a bare `Variant` in a value-only `DataValue`.
fn value_only(v: Variant) -> DataValue {
    DataValue {
        value: Some(v),
        status: None,
        source_timestamp: None,
        server_timestamp: None,
        source_pico_sec: None,
        server_pico_sec: None,
    }
}

/// The configured name of field `index` (or `field{index}` when the metadata
/// is shorter than the message).
fn field_name(meta: &DataSetMetaData, index: usize) -> String {
    meta.fields
        .get(index)
        .map(|f| f.name.clone())
        .unwrap_or_else(|| format!("field{index}"))
}

/// Builds a [`ReceivedField`], naming it from the metadata when available.
fn received(meta: &DataSetMetaData, index: usize, value: DataValue) -> ReceivedField {
    ReceivedField {
        name: field_name(meta, index),
        index: index as u16,
        value,
    }
}

/// Decodes a RawData payload against the metadata's ordered built-in types
/// (Part 14 §7.2.2.3.4). Scalars are bare values; arrays carry an `Int32`
/// length prefix.
fn decode_raw(meta: &DataSetMetaData, bytes: &[u8]) -> Result<Vec<ReceivedField>, DecodeError> {
    if meta.fields.is_empty() {
        return Err(DecodeError::MalformedMessage {
            message: "RawData DataSetMessage requires DataSetMetaData fields to decode",
        });
    }
    let mut r = UaReader::new(bytes);
    let mut fields = Vec::with_capacity(meta.fields.len());
    for (index, f) in meta.fields.iter().enumerate() {
        let variant = if f.value_rank < 0 {
            Variant::scalar(decode_builtin_value(&mut r, f.builtin_type)?)
        } else {
            let len = r.read_i32()?;
            if len < 0 {
                return Err(DecodeError::NegativeLength {
                    field: "RawData array",
                });
            }
            let mut values = Vec::with_capacity(len as usize);
            for _ in 0..len {
                values.push(decode_builtin_value(&mut r, f.builtin_type)?);
            }
            Variant {
                array_dimensions: alloc::vec![len as u32],
                value: values,
            }
        };
        fields.push(received(meta, index, value_only(variant)));
    }
    if !r.is_empty() {
        return Err(DecodeError::MalformedMessage {
            message: "RawData payload longer than the DataSetMetaData fields describe",
        });
    }
    Ok(fields)
}

/// A ReaderGroup (Part 14 §6.2.7) — dispatches a NetworkMessage to every
/// reader that accepts it.
#[derive(Debug, Clone, Default)]
pub struct ReaderGroup {
    readers: Vec<DataSetReader>,
}

impl ReaderGroup {
    /// Creates an empty ReaderGroup.
    #[must_use]
    pub fn new() -> Self {
        Self {
            readers: Vec::new(),
        }
    }

    /// Adds a reader to the group.
    pub fn add_reader(&mut self, reader: DataSetReader) -> &mut Self {
        self.readers.push(reader);
        self
    }

    /// The readers in the group.
    #[must_use]
    pub fn readers(&self) -> &[DataSetReader] {
        &self.readers
    }

    /// Decodes every DataSetMessage in `network` with each reader that
    /// accepts it, returning all matches in reader-then-message order.
    ///
    /// # Errors
    /// Propagates the first [`DecodeError`] from a matching reader (e.g. an
    /// undecodable RawData payload).
    pub fn accept(&self, network: &NetworkMessage) -> Result<Vec<MatchedDataSet>, DecodeError> {
        let mut out = Vec::new();
        for reader in &self.readers {
            for message in &network.messages {
                if reader.matches(network, message) {
                    out.push(MatchedDataSet {
                        reader_name: reader.config.name.clone(),
                        data: reader.decode(message)?,
                    });
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ConfigurationVersion, DataSetFieldContentMask, DataSetWriterConfig, FieldMetaData,
        NetworkMessageContentMask, WriterGroupConfig,
    };
    use crate::uadp::network_message::PublisherId;
    use crate::writer::{DataSetWriter, PublishedDataSet, WriterGroup};
    use zerodds_opcua_gateway::data_value::{DataValue, VariantValue};
    use zerodds_opcua_gateway::types::BuiltinTypeKind;

    fn meta() -> DataSetMetaData {
        DataSetMetaData::new(
            "ds1",
            alloc::vec![
                FieldMetaData::scalar("a", BuiltinTypeKind::Int32),
                FieldMetaData::scalar("b", BuiltinTypeKind::Int32),
            ],
        )
    }

    fn dv_int(v: i32) -> DataValue {
        DataValue {
            value: Some(Variant::scalar(VariantValue::Int32(v))),
            status: None,
            source_timestamp: None,
            server_timestamp: None,
            source_pico_sec: None,
            server_pico_sec: None,
        }
    }

    fn dataset() -> PublishedDataSet {
        let mut pds = PublishedDataSet::new("ds1");
        pds.add_field("a", dv_int(10)).add_field("b", dv_int(20));
        pds
    }

    /// End-to-end: writer produces a Variant KeyFrame, group frames it, the
    /// reader decodes it back into named fields.
    #[test]
    fn variant_round_trip_through_writer_and_reader() {
        let mut writer = DataSetWriter::new(
            DataSetWriterConfig::new("w1", 5, "ds1"),
            ConfigurationVersion::default(),
        );
        let mut group = WriterGroup::new(WriterGroupConfig::new("g1", 1), PublisherId::UInt16(9));
        let msg = writer.produce(&dataset(), None).expect("produce");
        let nm = group.frame(alloc::vec![msg], None);

        let reader = DataSetReader::new(DataSetReaderConfig::new("r1", meta()));
        let received = reader.decode(&nm.messages[0]).expect("decode");
        assert_eq!(received.writer_id, 5);
        assert_eq!(received.kind, DataSetMessageKind::KeyFrame);
        assert_eq!(received.fields.len(), 2);
        assert_eq!(received.fields[0].name, "a");
        assert_eq!(received.fields[0].value, dv_int(10));
        assert_eq!(received.fields[1].name, "b");
    }

    /// The complete stack: produce → frame → serialise to bytes → decode
    /// from bytes → dispatch to readers, proving the runtime ties into the
    /// Part 6 binary codec end to end.
    #[test]
    fn full_wire_round_trip() {
        use crate::binary::{from_binary, to_binary};

        let mut writer = DataSetWriter::new(
            DataSetWriterConfig::new("w1", 5, "ds1"),
            ConfigurationVersion::default(),
        );
        let mut gcfg = WriterGroupConfig::new("g1", 3);
        gcfg.network_message_content_mask = NetworkMessageContentMask::from_bits(
            NetworkMessageContentMask::PUBLISHER_ID
                | NetworkMessageContentMask::GROUP_HEADER
                | NetworkMessageContentMask::WRITER_GROUP_ID
                | NetworkMessageContentMask::PAYLOAD_HEADER,
        );
        let mut group = WriterGroup::new(gcfg, PublisherId::UInt16(9));

        let msg = writer.produce(&dataset(), None).expect("produce");
        let nm = group.frame(alloc::vec![msg], None);

        let bytes = to_binary(&nm).expect("encode");
        let decoded: NetworkMessage = from_binary(&bytes).expect("decode");

        let mut rg = ReaderGroup::new();
        let mut cfg = DataSetReaderConfig::new("r1", meta());
        cfg.publisher_id = Some(PublisherId::UInt16(9));
        cfg.writer_group_id = 3;
        cfg.data_set_writer_id = 5;
        rg.add_reader(DataSetReader::new(cfg));

        let matched = rg.accept(&decoded).expect("accept");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].data.fields[0].name, "a");
        assert_eq!(matched[0].data.fields[0].value, dv_int(10));
        assert_eq!(matched[0].data.fields[1].value, dv_int(20));
    }

    #[test]
    fn raw_data_round_trip_uses_metadata_types() {
        let mut wcfg = DataSetWriterConfig::new("w1", 5, "ds1");
        wcfg.field_content_mask = DataSetFieldContentMask::raw_data();
        let mut writer = DataSetWriter::new(wcfg, ConfigurationVersion::default());
        let msg = writer.produce(&dataset(), None).expect("produce");
        assert!(matches!(msg.data, DataSetData::Raw(_)));

        let reader = DataSetReader::new(DataSetReaderConfig::new("r1", meta()));
        let received = reader.decode(&msg).expect("decode");
        assert_eq!(received.fields[0].value, dv_int(10));
        assert_eq!(received.fields[1].value, dv_int(20));
    }

    #[test]
    fn data_value_round_trip_preserves_masked_members() {
        let mut wcfg = DataSetWriterConfig::new("w1", 5, "ds1");
        wcfg.field_content_mask =
            DataSetFieldContentMask::from_bits(DataSetFieldContentMask::STATUS_CODE);
        let mut writer = DataSetWriter::new(wcfg, ConfigurationVersion::default());
        let mut pds = PublishedDataSet::new("ds1");
        pds.add_field(
            "a",
            DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(7))),
                status: Some(0x4000_0000),
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        );
        let msg = writer.produce(&pds, None).expect("produce");

        let reader = DataSetReader::new(DataSetReaderConfig::new("r1", meta()));
        let received = reader.decode(&msg).expect("decode");
        assert_eq!(received.fields[0].value.status, Some(0x4000_0000));
    }

    #[test]
    fn publisher_and_writer_filters() {
        let nm = NetworkMessage {
            publisher_id: Some(PublisherId::UInt16(9)),
            data_set_class_id: None,
            group_header: None,
            timestamp: None,
            pico_seconds: None,
            promoted_fields: Vec::new(),
            payload_header: true,
            messages: alloc::vec![DataSetMessage::key_frame_variant(
                5,
                alloc::vec![Variant::scalar(VariantValue::Int32(1))]
            )],
        };

        // Matching publisher + writer id.
        let mut cfg = DataSetReaderConfig::new("r1", meta());
        cfg.publisher_id = Some(PublisherId::UInt16(9));
        cfg.data_set_writer_id = 5;
        assert!(DataSetReader::new(cfg).matches(&nm, &nm.messages[0]));

        // Wrong publisher.
        let mut cfg2 = DataSetReaderConfig::new("r2", meta());
        cfg2.publisher_id = Some(PublisherId::UInt16(1));
        assert!(!DataSetReader::new(cfg2).matches(&nm, &nm.messages[0]));

        // Wrong writer id.
        let mut cfg3 = DataSetReaderConfig::new("r3", meta());
        cfg3.data_set_writer_id = 99;
        assert!(!DataSetReader::new(cfg3).matches(&nm, &nm.messages[0]));
    }

    #[test]
    fn reader_group_dispatches_to_matching_readers() {
        let mut writer = DataSetWriter::new(
            DataSetWriterConfig::new("w1", 5, "ds1"),
            ConfigurationVersion::default(),
        );
        let mut group = WriterGroup::new(WriterGroupConfig::new("g1", 1), PublisherId::UInt16(9));
        let msg = writer.produce(&dataset(), None).expect("produce");
        let nm = group.frame(alloc::vec![msg], None);

        let mut rg = ReaderGroup::new();
        rg.add_reader(DataSetReader::new(DataSetReaderConfig::new("r1", meta())));
        let mut narrow = DataSetReaderConfig::new("r2", meta());
        narrow.data_set_writer_id = 99; // won't match
        rg.add_reader(DataSetReader::new(narrow));

        let matched = rg.accept(&nm).expect("accept");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].reader_name, "r1");
        assert_eq!(matched[0].data.writer_id, 5);
    }

    #[test]
    fn decode_dynamic_maps_variant_fields() {
        use crate::dynamic::DynamicValue;

        let msg = DataSetMessage::key_frame_variant(
            5,
            alloc::vec![
                Variant::scalar(VariantValue::Int32(10)),
                Variant::scalar(VariantValue::Int32(20)),
            ],
        );
        let reader = DataSetReader::new(DataSetReaderConfig::new("r1", meta()));
        let fields = reader.decode_dynamic(&msg).expect("decode_dynamic");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "a");
        assert_eq!(
            fields[0].value,
            DynamicValue::Scalar(VariantValue::Int32(10))
        );
        assert_eq!(fields[1].name, "b");
    }

    #[test]
    fn raw_data_without_metadata_is_rejected() {
        let msg = DataSetMessage {
            writer_id: 1,
            valid: true,
            kind: DataSetMessageKind::KeyFrame,
            sequence_number: None,
            timestamp: None,
            pico_seconds: None,
            status: None,
            config_major_version: None,
            config_minor_version: None,
            data: DataSetData::Raw(alloc::vec![0x01, 0x02]),
        };
        let reader = DataSetReader::new(DataSetReaderConfig::new("r1", DataSetMetaData::default()));
        assert!(matches!(
            reader.decode(&msg),
            Err(DecodeError::MalformedMessage { .. })
        ));
    }
}
