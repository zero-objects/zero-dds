// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The publishing side of OPC-UA PubSub (Part 14 §6.2.4 / §6.2.5).
//!
//! A [`PublishedDataSet`] holds the current values of a named set of
//! fields. A [`DataSetWriter`] turns those values into a
//! [`DataSetMessage`], applying its [`DataSetFieldContentMask`] (Variant /
//! DataValue / RawData) and its key-frame policy (a full KeyFrame every
//! `key_frame_count` publishes, DeltaFrames in between). A [`WriterGroup`]
//! frames the DataSetMessages of several writers into one
//! [`NetworkMessage`] per its [`NetworkMessageContentMask`].
//!
//! This layer is transport-agnostic: it produces the in-memory message
//! tree; serialising and sending it is stage 6.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant};

use crate::binary::{UaEncode, UaWriter, len_i32};
use crate::config::{
    ConfigurationVersion, DataSetFieldContentMask, DataSetMessageContentMask, DataSetWriterConfig,
    NetworkMessageContentMask, WriterGroupConfig,
};
use crate::error::EncodeError;
use crate::uadp::dataset_message::{
    DataSetData, DataSetMessage, DataSetMessageKind, FieldEncoding,
};
use crate::uadp::network_message::{GroupHeader, NetworkMessage, PublisherId};

/// The empty `Variant` (the OPC-UA null value), used when a published field
/// carries no value but a slot must still be emitted.
fn null_variant() -> Variant {
    Variant {
        array_dimensions: Vec::new(),
        value: Vec::new(),
    }
}

/// A PublishedDataSet (Part 14 §6.2.3) — the ordered, named fields whose
/// current values a [`DataSetWriter`] publishes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PublishedDataSet {
    name: String,
    field_names: Vec<String>,
    values: Vec<DataValue>,
}

impl PublishedDataSet {
    /// Creates an empty PublishedDataSet.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_names: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Appends a field with its initial value. Field order is the on-wire
    /// order and must match the subscriber's [`DataSetMetaData`].
    ///
    /// [`DataSetMetaData`]: crate::config::DataSetMetaData
    pub fn add_field(&mut self, name: impl Into<String>, initial: DataValue) -> &mut Self {
        self.field_names.push(name.into());
        self.values.push(initial);
        self
    }

    /// Updates the named field's value, returning `false` if no such field
    /// exists.
    pub fn set(&mut self, name: &str, value: DataValue) -> bool {
        match self.field_names.iter().position(|n| n == name) {
            Some(i) => {
                self.values[i] = value;
                true
            }
            None => false,
        }
    }

    /// Updates the named field with a bare value (status/timestamps left
    /// unset), returning `false` if no such field exists.
    pub fn set_variant(&mut self, name: &str, value: Variant) -> bool {
        self.set(
            name,
            DataValue {
                value: Some(value),
                status: None,
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        )
    }

    /// The DataSet name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The ordered field names.
    #[must_use]
    pub fn field_names(&self) -> &[String] {
        &self.field_names
    }

    /// The ordered current field values.
    #[must_use]
    pub fn values(&self) -> &[DataValue] {
        &self.values
    }

    /// The number of fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// `true` if the DataSet has no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// A DataSetWriter (Part 14 §6.2.4) — produces DataSetMessages from a
/// [`PublishedDataSet`] according to its configuration, tracking the
/// key-frame cycle, the delta baseline and the rolling sequence number.
#[derive(Debug, Clone)]
pub struct DataSetWriter {
    config: DataSetWriterConfig,
    config_version: ConfigurationVersion,
    publish_count: u32,
    sequence_number: u16,
    last_values: Vec<DataValue>,
    has_baseline: bool,
}

impl DataSetWriter {
    /// Creates a writer for `config`, advertising `config_version`.
    #[must_use]
    pub fn new(config: DataSetWriterConfig, config_version: ConfigurationVersion) -> Self {
        Self {
            config,
            config_version,
            publish_count: 0,
            sequence_number: 0,
            last_values: Vec::new(),
            has_baseline: false,
        }
    }

    /// The writer configuration.
    #[must_use]
    pub fn config(&self) -> &DataSetWriterConfig {
        &self.config
    }

    /// Produces the next DataSetMessage from `dataset`.
    ///
    /// The first publish, every `key_frame_count`-th publish, any RawData
    /// configuration and any field-count change emit a KeyFrame; the rest
    /// emit a DeltaFrame containing only the fields that changed since the
    /// last publish. `timestamp` (DateTime ticks) supplies the message
    /// Timestamp when the `TIMESTAMP` content-mask bit is set.
    ///
    /// # Errors
    /// Returns [`EncodeError`] if RawData encoding overflows a length field
    /// or encounters an unsupported (multi-dimensional or value-less) field.
    pub fn produce(
        &mut self,
        dataset: &PublishedDataSet,
        timestamp: Option<i64>,
    ) -> Result<DataSetMessage, EncodeError> {
        let encoding = self.config.field_content_mask.field_encoding();
        let key_frame_count = self.config.key_frame_count.max(1);
        let field_count_changed = self.last_values.len() != dataset.values().len();
        let is_key = !self.has_baseline
            || encoding == FieldEncoding::RawData
            || field_count_changed
            || self.publish_count % key_frame_count == 0;

        let data = if is_key {
            self.build_key_frame(dataset, encoding)?
        } else {
            self.build_delta_frame(dataset, encoding)
        };

        let mask = self.config.message_content_mask;
        let sequence_number = mask
            .contains(DataSetMessageContentMask::SEQUENCE_NUMBER)
            .then(|| {
                let sn = self.sequence_number;
                self.sequence_number = self.sequence_number.wrapping_add(1);
                sn
            });

        let message = DataSetMessage {
            writer_id: self.config.data_set_writer_id,
            valid: true,
            kind: if is_key {
                DataSetMessageKind::KeyFrame
            } else {
                DataSetMessageKind::DeltaFrame
            },
            sequence_number,
            timestamp: mask
                .contains(DataSetMessageContentMask::TIMESTAMP)
                .then_some(timestamp)
                .flatten(),
            pico_seconds: mask
                .contains(DataSetMessageContentMask::PICOSECONDS)
                .then_some(0),
            status: mask
                .contains(DataSetMessageContentMask::STATUS)
                .then_some(0),
            config_major_version: mask
                .contains(DataSetMessageContentMask::MAJOR_VERSION)
                .then_some(self.config_version.major_version),
            config_minor_version: mask
                .contains(DataSetMessageContentMask::MINOR_VERSION)
                .then_some(self.config_version.minor_version),
            data,
        };

        self.last_values = dataset.values().to_vec();
        self.has_baseline = true;
        self.publish_count = self.publish_count.wrapping_add(1);
        Ok(message)
    }

    fn build_key_frame(
        &self,
        dataset: &PublishedDataSet,
        encoding: FieldEncoding,
    ) -> Result<DataSetData, EncodeError> {
        Ok(match encoding {
            FieldEncoding::Variant => {
                DataSetData::Variant(dataset.values().iter().map(field_variant).collect())
            }
            FieldEncoding::DataValue => DataSetData::DataValue(
                dataset
                    .values()
                    .iter()
                    .map(|dv| project_data_value(dv, self.config.field_content_mask))
                    .collect(),
            ),
            FieldEncoding::RawData => DataSetData::Raw(encode_raw_fields(dataset.values())?),
        })
    }

    fn build_delta_frame(
        &self,
        dataset: &PublishedDataSet,
        encoding: FieldEncoding,
    ) -> DataSetData {
        let changed = dataset
            .values()
            .iter()
            .enumerate()
            .filter(|(i, dv)| self.last_values.get(*i) != Some(*dv));

        match encoding {
            FieldEncoding::DataValue => DataSetData::DeltaDataValue(
                changed
                    .map(|(i, dv)| {
                        (
                            i as u16,
                            project_data_value(dv, self.config.field_content_mask),
                        )
                    })
                    .collect(),
            ),
            // RawData never reaches the delta path (it always key-frames);
            // Variant is the remaining case.
            _ => DataSetData::DeltaVariant(
                changed
                    .map(|(i, dv)| (i as u16, field_variant(dv)))
                    .collect(),
            ),
        }
    }
}

/// Extracts the bare `Variant` from a field value (the null Variant when the
/// field has no value).
fn field_variant(dv: &DataValue) -> Variant {
    dv.value.clone().unwrap_or_else(null_variant)
}

/// Projects a DataValue down to the members selected by the content mask,
/// always keeping the value itself (Part 14 §6.2.4.2).
fn project_data_value(dv: &DataValue, mask: DataSetFieldContentMask) -> DataValue {
    DataValue {
        value: dv.value.clone(),
        status: mask
            .contains(DataSetFieldContentMask::STATUS_CODE)
            .then_some(dv.status.unwrap_or(0)),
        source_timestamp: mask
            .contains(DataSetFieldContentMask::SOURCE_TIMESTAMP)
            .then_some(dv.source_timestamp.unwrap_or(0)),
        server_timestamp: mask
            .contains(DataSetFieldContentMask::SERVER_TIMESTAMP)
            .then_some(dv.server_timestamp.unwrap_or(0)),
        source_pico_sec: mask
            .contains(DataSetFieldContentMask::SOURCE_PICOSECONDS)
            .then_some(dv.source_pico_sec.unwrap_or(0)),
        server_pico_sec: mask
            .contains(DataSetFieldContentMask::SERVER_PICOSECONDS)
            .then_some(dv.server_pico_sec.unwrap_or(0)),
    }
}

/// Encodes DataSet fields in the RawData encoding (Part 14 §7.2.2.3.4) — the
/// bare values back-to-back with no type tags. Scalars write the value;
/// 1-D arrays write an `Int32` length prefix then the elements.
fn encode_raw_fields(values: &[DataValue]) -> Result<Vec<u8>, EncodeError> {
    let mut w = UaWriter::new();
    for dv in values {
        let v = dv.value.as_ref().ok_or(EncodeError::ValueOutOfRange {
            message: "RawData field has no value",
        })?;
        if v.is_scalar() {
            // A scalar carries exactly one element.
            let elem = v.value.first().ok_or(EncodeError::ValueOutOfRange {
                message: "RawData scalar field is empty",
            })?;
            elem.encode(&mut w)?;
        } else if v.is_1d_array() {
            w.write_i32(len_i32("RawData array", v.value.len())?);
            for elem in &v.value {
                elem.encode(&mut w)?;
            }
        } else {
            return Err(EncodeError::ValueOutOfRange {
                message: "RawData encoding of multi-dimensional arrays is not supported",
            });
        }
    }
    Ok(w.into_vec())
}

/// A WriterGroup (Part 14 §6.2.5) — frames the DataSetMessages of its
/// writers into a [`NetworkMessage`] per its content mask, tracking the
/// rolling NetworkMessageNumber and group SequenceNumber.
#[derive(Debug, Clone)]
pub struct WriterGroup {
    config: WriterGroupConfig,
    publisher_id: PublisherId,
    network_message_number: u16,
    sequence_number: u16,
}

impl WriterGroup {
    /// Creates a WriterGroup publishing under `publisher_id`.
    #[must_use]
    pub fn new(config: WriterGroupConfig, publisher_id: PublisherId) -> Self {
        Self {
            config,
            publisher_id,
            network_message_number: 0,
            sequence_number: 0,
        }
    }

    /// The group configuration.
    #[must_use]
    pub fn config(&self) -> &WriterGroupConfig {
        &self.config
    }

    /// Frames `messages` into a NetworkMessage, applying the
    /// [`NetworkMessageContentMask`] and advancing the rolling counters.
    /// `timestamp` (DateTime ticks) supplies the NetworkMessage Timestamp
    /// when the `TIMESTAMP` bit is set.
    pub fn frame(
        &mut self,
        messages: Vec<DataSetMessage>,
        timestamp: Option<i64>,
    ) -> NetworkMessage {
        let mask = self.config.network_message_content_mask;

        let group_header = mask
            .contains(NetworkMessageContentMask::GROUP_HEADER)
            .then(|| {
                let network_message_number = mask
                    .contains(NetworkMessageContentMask::NETWORK_MESSAGE_NUMBER)
                    .then(|| {
                        let n = self.network_message_number;
                        self.network_message_number = self.network_message_number.wrapping_add(1);
                        n
                    });
                let sequence_number = mask
                    .contains(NetworkMessageContentMask::SEQUENCE_NUMBER)
                    .then(|| {
                        let n = self.sequence_number;
                        self.sequence_number = self.sequence_number.wrapping_add(1);
                        n
                    });
                GroupHeader {
                    writer_group_id: mask
                        .contains(NetworkMessageContentMask::WRITER_GROUP_ID)
                        .then_some(self.config.writer_group_id),
                    group_version: mask
                        .contains(NetworkMessageContentMask::GROUP_VERSION)
                        .then_some(self.config.group_version),
                    network_message_number,
                    sequence_number,
                }
            });

        // A PayloadHeader is mandatory for more than one DataSetMessage.
        let payload_header =
            mask.contains(NetworkMessageContentMask::PAYLOAD_HEADER) || messages.len() > 1;

        NetworkMessage {
            publisher_id: mask
                .contains(NetworkMessageContentMask::PUBLISHER_ID)
                .then(|| self.publisher_id.clone()),
            data_set_class_id: None,
            group_header,
            timestamp: mask
                .contains(NetworkMessageContentMask::TIMESTAMP)
                .then_some(timestamp)
                .flatten(),
            pico_seconds: mask
                .contains(NetworkMessageContentMask::PICOSECONDS)
                .then_some(0),
            promoted_fields: Vec::new(),
            payload_header,
            messages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uadp::dataset_message::DataSetData;
    use zerodds_opcua_gateway::data_value::VariantValue;

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

    fn sample_dataset() -> PublishedDataSet {
        let mut pds = PublishedDataSet::new("ds1");
        pds.add_field("a", dv_int(1)).add_field("b", dv_int(2));
        pds
    }

    #[test]
    fn first_publish_is_key_frame() {
        let mut w = DataSetWriter::new(
            DataSetWriterConfig::new("w1", 5, "ds1"),
            ConfigurationVersion::default(),
        );
        let msg = w.produce(&sample_dataset(), None).expect("produce");
        assert_eq!(msg.kind, DataSetMessageKind::KeyFrame);
        assert_eq!(msg.writer_id, 5);
        assert_eq!(
            msg.data,
            DataSetData::Variant(alloc::vec![
                Variant::scalar(VariantValue::Int32(1)),
                Variant::scalar(VariantValue::Int32(2)),
            ])
        );
    }

    #[test]
    fn second_publish_is_delta_with_only_changed_fields() {
        let mut cfg = DataSetWriterConfig::new("w1", 5, "ds1");
        cfg.key_frame_count = 10; // key only every 10th publish
        let mut w = DataSetWriter::new(cfg, ConfigurationVersion::default());
        let mut pds = sample_dataset();

        let _key = w.produce(&pds, None).expect("key");
        pds.set("b", dv_int(99));
        let delta = w.produce(&pds, None).expect("delta");

        assert_eq!(delta.kind, DataSetMessageKind::DeltaFrame);
        assert_eq!(
            delta.data,
            DataSetData::DeltaVariant(alloc::vec![(1, Variant::scalar(VariantValue::Int32(99)))])
        );
    }

    #[test]
    fn key_frame_count_forces_periodic_key() {
        let mut cfg = DataSetWriterConfig::new("w1", 1, "ds1");
        cfg.key_frame_count = 2; // key on publishes 0, 2, 4, ...
        let mut w = DataSetWriter::new(cfg, ConfigurationVersion::default());
        let pds = sample_dataset();

        assert_eq!(
            w.produce(&pds, None).expect("p0").kind,
            DataSetMessageKind::KeyFrame
        );
        assert_eq!(
            w.produce(&pds, None).expect("p1").kind,
            DataSetMessageKind::DeltaFrame
        );
        assert_eq!(
            w.produce(&pds, None).expect("p2").kind,
            DataSetMessageKind::KeyFrame
        );
    }

    #[test]
    fn data_value_mask_projects_selected_members() {
        let mut cfg = DataSetWriterConfig::new("w1", 1, "ds1");
        cfg.field_content_mask = DataSetFieldContentMask::from_bits(
            DataSetFieldContentMask::STATUS_CODE | DataSetFieldContentMask::SOURCE_TIMESTAMP,
        );
        let mut w = DataSetWriter::new(cfg, ConfigurationVersion::default());
        let mut pds = PublishedDataSet::new("ds1");
        pds.add_field(
            "a",
            DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(7))),
                status: Some(0x8000_0000),
                source_timestamp: Some(123),
                server_timestamp: Some(456),
                source_pico_sec: Some(9),
                server_pico_sec: Some(9),
            },
        );

        let msg = w.produce(&pds, None).expect("produce");
        let DataSetData::DataValue(fields) = msg.data else {
            panic!("expected DataValue encoding");
        };
        let f = &fields[0];
        assert_eq!(f.status, Some(0x8000_0000));
        assert_eq!(f.source_timestamp, Some(123));
        // Not selected by the mask:
        assert_eq!(f.server_timestamp, None);
        assert_eq!(f.source_pico_sec, None);
        assert_eq!(f.server_pico_sec, None);
    }

    #[test]
    fn raw_data_is_always_key_frame() {
        let mut cfg = DataSetWriterConfig::new("w1", 1, "ds1");
        cfg.field_content_mask = DataSetFieldContentMask::raw_data();
        cfg.key_frame_count = 1;
        let mut w = DataSetWriter::new(cfg, ConfigurationVersion::default());
        let pds = sample_dataset();

        let m0 = w.produce(&pds, None).expect("p0");
        let m1 = w.produce(&pds, None).expect("p1");
        assert_eq!(m0.kind, DataSetMessageKind::KeyFrame);
        assert_eq!(m1.kind, DataSetMessageKind::KeyFrame);
        assert!(matches!(m0.data, DataSetData::Raw(_)));
    }

    #[test]
    fn sequence_number_emitted_and_rolls_when_masked() {
        let mut cfg = DataSetWriterConfig::new("w1", 1, "ds1");
        cfg.message_content_mask =
            DataSetMessageContentMask::from_bits(DataSetMessageContentMask::SEQUENCE_NUMBER);
        let mut w = DataSetWriter::new(cfg, ConfigurationVersion::default());
        let pds = sample_dataset();
        assert_eq!(w.produce(&pds, None).expect("p0").sequence_number, Some(0));
        assert_eq!(w.produce(&pds, None).expect("p1").sequence_number, Some(1));
    }

    #[test]
    fn writer_group_frames_with_group_header_and_publisher() {
        let cfg = WriterGroupConfig::new("g1", 42);
        let mut group = WriterGroup::new(cfg, PublisherId::UInt16(7));
        let mut wcfg = WriterGroupConfig::new("g1", 42);
        wcfg.network_message_content_mask = NetworkMessageContentMask::from_bits(
            NetworkMessageContentMask::PUBLISHER_ID
                | NetworkMessageContentMask::GROUP_HEADER
                | NetworkMessageContentMask::WRITER_GROUP_ID
                | NetworkMessageContentMask::SEQUENCE_NUMBER
                | NetworkMessageContentMask::PAYLOAD_HEADER,
        );
        group.config = wcfg;

        let msg = DataSetMessage::key_frame_variant(
            5,
            alloc::vec![Variant::scalar(VariantValue::Int32(1))],
        );
        let nm0 = group.frame(alloc::vec![msg.clone()], None);
        assert_eq!(nm0.publisher_id, Some(PublisherId::UInt16(7)));
        let gh = nm0.group_header.expect("group header");
        assert_eq!(gh.writer_group_id, Some(42));
        assert_eq!(gh.sequence_number, Some(0));

        let nm1 = group.frame(alloc::vec![msg], None);
        assert_eq!(nm1.group_header.expect("gh").sequence_number, Some(1));
    }
}
