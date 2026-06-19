// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DataSet ↔ DDS-topic bridge — maps OPC-UA PubSub DataSets to and from DDS
//! topic samples so a value published on one global data space appears on the
//! other.
//!
//! The two models meet at [`DdsSample`], an ordered list of named values. A
//! [`DataSetTopicMapping`] translates DataSet field names to DDS member names
//! (identity by default) in both directions:
//!
//! - **OPC-UA → DDS**: a [`ReceivedDataSet`] decoded by a
//!   [`crate::reader::DataSetReader`] becomes a [`DdsSample`] and is handed to
//!   a [`DdsPublisher`].
//! - **DDS → OPC-UA**: a [`DdsSample`] taken from a [`DdsSubscriber`] updates a
//!   [`PublishedDataSet`] that a [`crate::writer::DataSetWriter`] then publishes
//!   as UADP.
//!
//! The DDS side is injected through the [`DdsPublisher`] / [`DdsSubscriber`]
//! traits, so any DDS implementation (e.g. `zerodds-dcps`) can back the bridge
//! without this crate depending on it.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::Variant;

use crate::reader::ReceivedDataSet;
use crate::writer::PublishedDataSet;

/// A neutral DDS topic sample: ordered named values, the shape both sides map
/// to and from.
#[derive(Debug, Clone, PartialEq)]
pub struct DdsSample {
    /// Topic the sample belongs to.
    pub topic: String,
    /// Ordered `(member_name, value)` pairs.
    pub fields: Vec<(String, Variant)>,
}

/// An error from the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// The DDS backend reported a failure.
    Dds(String),
}

impl core::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Dds(m) => write!(f, "DDS backend error: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BridgeError {}

/// Publishes mapped samples onto a DDS topic (implemented by a DDS DataWriter).
pub trait DdsPublisher {
    /// Publishes `sample` to its topic.
    ///
    /// # Errors
    /// [`BridgeError`] if the DDS write fails.
    fn publish(&self, sample: &DdsSample) -> Result<(), BridgeError>;
}

/// Takes samples from a DDS topic (implemented by a DDS DataReader).
pub trait DdsSubscriber {
    /// Takes all currently available samples for `topic`.
    ///
    /// # Errors
    /// [`BridgeError`] if the DDS take fails.
    fn take(&self, topic: &str) -> Result<Vec<DdsSample>, BridgeError>;
}

/// Maps an OPC-UA DataSet onto a DDS topic and its members (Part 14 DataSet ↔
/// DDS topic). Field order follows the DataSet field order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataSetTopicMapping {
    /// DDS topic name.
    pub topic: String,
    /// `(dataset_field_name, dds_member_name)` pairs.
    pub field_map: Vec<(String, String)>,
}

impl DataSetTopicMapping {
    /// Creates an identity mapping (DDS member name == DataSet field name) for
    /// `topic` over the given ordered field names.
    #[must_use]
    pub fn identity(topic: impl Into<String>, field_names: &[&str]) -> Self {
        Self {
            topic: topic.into(),
            field_map: field_names
                .iter()
                .map(|n| (String::from(*n), String::from(*n)))
                .collect(),
        }
    }

    fn dds_member(&self, dataset_field: &str) -> Option<&str> {
        self.field_map
            .iter()
            .find(|(ds, _)| ds == dataset_field)
            .map(|(_, dds)| dds.as_str())
    }

    fn dataset_field(&self, dds_member: &str) -> Option<&str> {
        self.field_map
            .iter()
            .find(|(_, dds)| dds == dds_member)
            .map(|(ds, _)| ds.as_str())
    }

    /// Converts a received OPC-UA DataSet into a DDS sample, renaming fields
    /// per the mapping and dropping fields that have no mapping.
    #[must_use]
    pub fn dataset_to_sample(&self, received: &ReceivedDataSet) -> DdsSample {
        let mut fields = Vec::with_capacity(received.fields.len());
        for f in &received.fields {
            if let Some(member) = self.dds_member(&f.name) {
                if let Some(value) = &f.value.value {
                    fields.push((String::from(member), value.clone()));
                }
            }
        }
        DdsSample {
            topic: self.topic.clone(),
            fields,
        }
    }

    /// Applies a DDS sample to a PublishedDataSet, updating the mapped fields
    /// in place. Returns the number of fields updated.
    pub fn apply_sample(&self, sample: &DdsSample, dataset: &mut PublishedDataSet) -> usize {
        let mut updated = 0;
        for (member, value) in &sample.fields {
            if let Some(field) = self.dataset_field(member) {
                if dataset.set_variant(field, value.clone()) {
                    updated += 1;
                }
            }
        }
        updated
    }
}

/// Forwards received OPC-UA DataSets to a DDS topic via a [`DdsPublisher`].
#[derive(Debug, Clone)]
pub struct OpcUaToDdsBridge<P: DdsPublisher> {
    mapping: DataSetTopicMapping,
    publisher: P,
}

impl<P: DdsPublisher> OpcUaToDdsBridge<P> {
    /// Creates the bridge.
    pub fn new(mapping: DataSetTopicMapping, publisher: P) -> Self {
        Self { mapping, publisher }
    }

    /// Maps `received` to a DDS sample and publishes it.
    ///
    /// # Errors
    /// [`BridgeError`] if the DDS publish fails.
    pub fn forward(&self, received: &ReceivedDataSet) -> Result<(), BridgeError> {
        let sample = self.mapping.dataset_to_sample(received);
        self.publisher.publish(&sample)
    }
}

/// Pulls DDS samples and applies them to a PublishedDataSet for OPC-UA
/// publication via a [`DdsSubscriber`].
#[derive(Debug, Clone)]
pub struct DdsToOpcUaBridge<S: DdsSubscriber> {
    mapping: DataSetTopicMapping,
    subscriber: S,
}

impl<S: DdsSubscriber> DdsToOpcUaBridge<S> {
    /// Creates the bridge.
    pub fn new(mapping: DataSetTopicMapping, subscriber: S) -> Self {
        Self {
            mapping,
            subscriber,
        }
    }

    /// Takes all pending DDS samples and applies the last one to `dataset`
    /// (the freshest value wins). Returns the number of samples consumed.
    ///
    /// # Errors
    /// [`BridgeError`] if the DDS take fails.
    pub fn pump(&self, dataset: &mut PublishedDataSet) -> Result<usize, BridgeError> {
        let samples = self.subscriber.take(&self.mapping.topic)?;
        let count = samples.len();
        for sample in &samples {
            self.mapping.apply_sample(sample, dataset);
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::ReceivedField;
    use core::cell::RefCell;
    use zerodds_opcua_gateway::data_value::{DataValue, VariantValue};

    fn variant_field(name: &str, index: u16, v: i32) -> ReceivedField {
        ReceivedField {
            name: String::from(name),
            index,
            value: DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(v))),
                status: None,
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        }
    }

    #[test]
    fn dataset_maps_to_dds_sample_with_renames() {
        let mut mapping = DataSetTopicMapping::identity("Telemetry", &["a", "b"]);
        // Rename "b" -> "speed" on the DDS side.
        mapping.field_map[1].1 = String::from("speed");
        let received = ReceivedDataSet {
            writer_id: 5,
            kind: crate::uadp::dataset_message::DataSetMessageKind::KeyFrame,
            fields: alloc::vec![variant_field("a", 0, 1), variant_field("b", 1, 2)],
        };
        let sample = mapping.dataset_to_sample(&received);
        assert_eq!(sample.topic, "Telemetry");
        assert_eq!(sample.fields[0].0, "a");
        assert_eq!(sample.fields[1].0, "speed");
        assert_eq!(sample.fields[1].1, Variant::scalar(VariantValue::Int32(2)));
    }

    #[test]
    fn dds_sample_applies_to_published_dataset() {
        let mapping = DataSetTopicMapping::identity("T", &["a", "b"]);
        let mut pds = PublishedDataSet::new("ds");
        pds.add_field(
            "a",
            DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(0))),
                status: None,
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        )
        .add_field(
            "b",
            DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(0))),
                status: None,
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        );
        let sample = DdsSample {
            topic: String::from("T"),
            fields: alloc::vec![
                (String::from("a"), Variant::scalar(VariantValue::Int32(11))),
                (String::from("b"), Variant::scalar(VariantValue::Int32(22))),
            ],
        };
        assert_eq!(mapping.apply_sample(&sample, &mut pds), 2);
        assert_eq!(
            pds.values()[0].value,
            Some(Variant::scalar(VariantValue::Int32(11)))
        );
        assert_eq!(
            pds.values()[1].value,
            Some(Variant::scalar(VariantValue::Int32(22)))
        );
    }

    struct MockDds {
        published: RefCell<Vec<DdsSample>>,
    }

    impl DdsPublisher for MockDds {
        fn publish(&self, sample: &DdsSample) -> Result<(), BridgeError> {
            self.published.borrow_mut().push(sample.clone());
            Ok(())
        }
    }

    impl DdsSubscriber for MockDds {
        fn take(&self, _topic: &str) -> Result<Vec<DdsSample>, BridgeError> {
            Ok(self.published.borrow_mut().drain(..).collect())
        }
    }

    #[test]
    fn opcua_to_dds_and_back() {
        let mapping = DataSetTopicMapping::identity("T", &["a"]);
        let dds = MockDds {
            published: RefCell::new(Vec::new()),
        };
        // OPC-UA -> DDS.
        let received = ReceivedDataSet {
            writer_id: 1,
            kind: crate::uadp::dataset_message::DataSetMessageKind::KeyFrame,
            fields: alloc::vec![variant_field("a", 0, 99)],
        };
        OpcUaToDdsBridge::new(mapping.clone(), &dds)
            .forward(&received)
            .expect("forward");

        // DDS -> OPC-UA.
        let mut pds = PublishedDataSet::new("ds");
        pds.add_field(
            "a",
            DataValue {
                value: Some(Variant::scalar(VariantValue::Int32(0))),
                status: None,
                source_timestamp: None,
                server_timestamp: None,
                source_pico_sec: None,
                server_pico_sec: None,
            },
        );
        let consumed = DdsToOpcUaBridge::new(mapping, &dds)
            .pump(&mut pds)
            .expect("pump");
        assert_eq!(consumed, 1);
        assert_eq!(
            pds.values()[0].value,
            Some(Variant::scalar(VariantValue::Int32(99)))
        );
    }

    // Allow &MockDds to satisfy the trait bounds in the round-trip test.
    impl DdsPublisher for &MockDds {
        fn publish(&self, sample: &DdsSample) -> Result<(), BridgeError> {
            (**self).publish(sample)
        }
    }
    impl DdsSubscriber for &MockDds {
        fn take(&self, topic: &str) -> Result<Vec<DdsSample>, BridgeError> {
            (**self).take(topic)
        }
    }
}
