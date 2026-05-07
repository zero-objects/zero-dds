// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Subscription-Mapping Configuration — Spec Tab 8.8-8.15.
//!
//! Datenmodell fuer das Gateway-Config-File (XML/IDL/JSON), das die
//! OPC-UA-Subscriptions, MonitoredItems und Input/Output-Mappings
//! beschreibt. Pure Rust-Structs ohne IO — Wire-Loader sind separat
//! (`crates/opcua-gateway/src/xml.rs` fuer den vereinfachten
//! Bridge-Loader; ein voller Subscription-Mapping-XML-Loader laesst
//! sich darauf aufbauen).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::data_value::Variant;
use crate::node_id::NodeId;
use crate::service_sets::attribute::EventFilter;

// -------------------------------------------------------------------
// Tab 8.10/8.11 — OPC UA Connection + Subscription Protocol.
// -------------------------------------------------------------------

/// Spec Tab 8.10 — `OpcUaConnectionConfig` (`@nested`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaConnectionConfig {
    /// `uint32 protocol_version`.
    pub protocol_version: u32,
    /// `uint32 send_buffer_size`.
    pub send_buffer_size: u32,
    /// `uint32 recv_buffer_size`.
    pub recv_buffer_size: u32,
    /// `uint32 max_message_size`.
    pub max_message_size: u32,
    /// `uint32 max_chunk_count`.
    pub max_chunk_count: u32,
}

/// Spec Tab 8.10 — `OpcUaConnection` (`@nested`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaConnection {
    /// `string endpoint_url` — z.B. `opc.tcp://10.0.0.1:4840`.
    pub endpoint_url: String,
    /// `uint32 timeout` (ms).
    pub timeout: u32,
    /// `uint32 secure_channel_lifetime` (ms).
    pub secure_channel_lifetime: u32,
    /// `OpcUaConnectionConfig local_connection`.
    pub local_connection: OpcUaConnectionConfig,
}

/// Spec Tab 8.11 — `SubscriptionProtocol` (`@nested`).
///
/// `requested_publishing_interval` ist `f64` (ms). Spec erlaubt 0/Negativ
/// fuer "Server waehlt schnellstmoeglich"; das ist ein Wert-Aspekt, kein
/// Type-Aspekt — der Caller muss die Sentinel-Semantik ehren.
#[derive(Debug, Clone, PartialEq)]
pub struct SubscriptionProtocol {
    /// `double requested_publishing_interval` (ms).
    pub requested_publishing_interval: f64,
    /// `uint32 requested_lifetime_count` — must be at least 3 *
    /// `requested_max_keepalive_count` (Spec §8.4.1.1 normativ).
    pub requested_lifetime_count: u32,
    /// `uint32 requested_max_keepalive_count`.
    pub requested_max_keepalive_count: u32,
    /// `uint32 max_notifications_per_publish` — `0` = unlimited.
    pub max_notifications_per_publish: u32,
    /// `boolean publishing_enabled`.
    pub publishing_enabled: bool,
    /// `octet priority`.
    pub priority: u8,
}

impl SubscriptionProtocol {
    /// Spec §8.4.1.1: `requested_lifetime_count` muss mindestens
    /// `3 * requested_max_keepalive_count` sein. Liefert `false` wenn
    /// die Spec-Constraint verletzt ist.
    #[must_use]
    pub fn lifetime_constraint_ok(&self) -> bool {
        // u64 zur Vermeidung von u32-Overflow bei keepalive*3.
        u64::from(self.requested_lifetime_count)
            >= 3u64 * u64::from(self.requested_max_keepalive_count)
    }
}

// -------------------------------------------------------------------
// Tab 8.12 — MonitoredItem (DataItem/EventItem-Union).
// -------------------------------------------------------------------

/// Spec Tab 8.12 — `DataChangeFilter` Re-Export aus Tab 8.3 — wir
/// re-nutzen die Subscription-`DataChangeFilter`-Definition.
pub use crate::service_sets::attribute::AggregateConfiguration;

/// Spec Tab 8.3 — `DataChangeTrigger`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DataChangeTrigger {
    /// `@value(0) STATUS_DATA_CHANGE_TRIGGER`.
    Status = 0,
    /// `@value(1) STATUS_VALUE_DATA_CHANGE_TRIGGER`.
    StatusValue = 1,
    /// `@value(2) STATUS_VALUE_TIMESTAMP_DATA_CHANGE_TRIGGER`.
    StatusValueTimestamp = 2,
}

/// Spec Tab 8.3 — `DataChangeFilter`.
#[derive(Debug, Clone, PartialEq)]
pub struct DataChangeFilter {
    /// `DataChangeTrigger trigger`.
    pub trigger: DataChangeTrigger,
    /// `uint32 deadband_type` — 0=None, 1=Absolute, 2=Percent
    /// (OPCUA-04 §7.17.2 "DeadbandType").
    pub deadband_type: u32,
    /// `double deadband_value`.
    pub deadband_value: f64,
}

/// Spec Tab 8.3 — `AggregateFilter`.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateFilter {
    /// `UtcTime start_time` (i64 Ticks).
    pub start_time: i64,
    /// `NodeId aggregate_type`.
    pub aggregate_type: NodeId,
    /// `Duration processing_interval` (ms).
    pub processing_interval: f64,
    /// `AggregateConfiguration aggregate_configuration`.
    pub aggregate_configuration: AggregateConfiguration,
}

/// Spec Tab 8.12 — `DataItem` (`@nested`). Genau einer der zwei
/// Filter darf gesetzt sein (Spec normativ "they shall not be combined").
#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `uint32 attribute_id`.
    pub attribute_id: u32,
    /// `double sampling_interval` (ms).
    pub sampling_interval: f64,
    /// `uint32 queue_size`.
    pub queue_size: u32,
    /// `boolean discard_oldest`.
    pub discard_oldest: bool,
    /// `@optional DataChangeFilter data_change_filter`.
    pub data_change_filter: Option<DataChangeFilter>,
    /// `@optional AggregateFilter aggregate_filter`.
    pub aggregate_filter: Option<AggregateFilter>,
}

impl DataItem {
    /// Spec §8.4.2.2.4 normativ: ein DataItem darf hoechstens einen
    /// Filter haben. Liefert `Err(DualFilterError)` wenn beide gesetzt.
    ///
    /// # Errors
    /// `DualFilterError` wenn sowohl `data_change_filter` als auch
    /// `aggregate_filter` `Some` sind.
    pub fn validate_filters(&self) -> Result<(), DualFilterError> {
        if self.data_change_filter.is_some() && self.aggregate_filter.is_some() {
            return Err(DualFilterError);
        }
        Ok(())
    }
}

/// Validierungs-Fehler `DataItem mit zwei Filtern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualFilterError;

/// Spec Tab 8.12 — `EventItem` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct EventItem {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `double sampling_interval` (ms).
    pub sampling_interval: f64,
    /// `uint32 queue_size`.
    pub queue_size: u32,
    /// `boolean discard_oldest`.
    pub discard_oldest: bool,
    /// `@optional EventFilter event_filter`.
    pub event_filter: Option<EventFilter>,
}

/// Spec Tab 8.12 — `MonitoredItemKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoredItemKind {
    /// `DATA_MONITORED_ITEM`.
    Data,
    /// `EVENT_MONITORED_ITEM`.
    Event,
}

/// Spec Tab 8.12 — `MonitoredItem` (`@nested union`).
#[derive(Debug, Clone, PartialEq)]
pub enum MonitoredItem {
    /// `DATA_MONITORED_ITEM`.
    Data(DataItem),
    /// `EVENT_MONITORED_ITEM`.
    Event(EventItem),
}

impl MonitoredItem {
    /// Discriminant.
    #[must_use]
    pub fn kind(&self) -> MonitoredItemKind {
        match self {
            Self::Data(_) => MonitoredItemKind::Data,
            Self::Event(_) => MonitoredItemKind::Event,
        }
    }
}

// -------------------------------------------------------------------
// Tab 8.9 — OPC UA Input.
// -------------------------------------------------------------------

/// Spec Tab 8.9 — `OpcUaInput` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaInput {
    /// `string name` — eindeutiger Bezeichner (Spec §8.4.2.2.1).
    pub name: String,
    /// `OpcUaConnection opcua_connection`.
    pub opcua_connection: OpcUaConnection,
    /// `SubscriptionProtocol subscription_protocol`.
    pub subscription_protocol: SubscriptionProtocol,
    /// `sequence<MonitoredItem> monitored_items`.
    pub monitored_items: Vec<MonitoredItem>,
}

// -------------------------------------------------------------------
// Tab 8.13/8.14 — DDS Output + DomainParticipant.
// -------------------------------------------------------------------

/// Spec Tab 8.14 — `DdsRegisterType` (`@nested`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsRegisterType {
    /// `string type_name` — Topic-side registrierter Typname.
    pub type_name: String,
    /// `string type_ref` — IDL-Type-Reference.
    pub type_ref: String,
}

/// Spec Tab 8.14 — `DdsDomainParticipant` (`@nested`).
///
/// `participant_qos` aus DDS PSM ist hier als `String` (XML-Snippet)
/// modelliert; ein voller `DomainParticipantQos`-Typ aus
/// `crates/dcps/` waere zyklisch und ist hier nicht noetig — dieses
/// Modul ist Schema-Ebene, der Caller materialisiert das QoS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsDomainParticipant {
    /// `int32 domain_id`.
    pub domain_id: i32,
    /// `sequence<DdsRegisterType> register_types`.
    pub register_types: Vec<DdsRegisterType>,
    /// `DDS::DomainParticipantQos participant_qos` — als XML-Snippet
    /// oder andere Caller-Repraesentation; leerer String = Default-QoS.
    pub participant_qos: String,
}

/// Spec Tab 8.13 — `DdsOutput` (`@nested`).
///
/// `domain_participant_ref` ist `@external` in der IDL — also ein
/// Verweis, kein eingebetteter Wert. Hier modelliert als
/// `Box<DdsDomainParticipant>` damit der Verweis Owner-frei resolved
/// wird; Spec laesst die Aufloesung an den Caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsOutput {
    /// `string name`.
    pub name: String,
    /// `@external DdsDomainParticipant domain_participant_ref`.
    pub domain_participant_ref: Box<DdsDomainParticipant>,
    /// `string topic_name`.
    pub topic_name: String,
    /// `string registered_type_name`.
    pub registered_type_name: String,
    /// `@optional DDS::DataWriterQos datawriter_qos` — Caller-
    /// materialisiert; leerer String = Default-QoS.
    pub datawriter_qos: String,
}

// -------------------------------------------------------------------
// Tab 8.15 — InputOutputMapping (Assignment + FieldAssignment).
// -------------------------------------------------------------------

/// Spec Tab 8.15 — `AssignmentKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentKind {
    /// `DATA_ITEM_ASSIGNMENT`.
    DataItem,
    /// `EVENT_FIELD_ASSIGNMENT`.
    EventField,
    /// `CONSTANT_VALUE_ASSIGNMENT`.
    ConstantValue,
}

/// Spec Tab 8.15 — `DataItemRef` — Name eines DataItems aus dem Input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataItemRef {
    /// `string data_item_name` — referenziert `DataItem` ueber den
    /// Namen, den der Konfigurator definiert hat.
    pub data_item_name: String,
}

/// Spec Tab 8.15 — `EventFieldRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventFieldRef {
    /// `string event_name` — referenziert ein EventItem.
    pub event_name: String,
    /// `uint32 event_field_index` — Index in den `event_fields` aus
    /// dem `EventFieldList` (Spec §8.4.3.2.2 normativ).
    pub event_field_index: u32,
}

/// Spec Tab 8.15 — `AssignmentInput` (`@nested union`).
#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentInput {
    /// `DATA_ITEM_ASSIGNMENT`.
    DataItem(DataItemRef),
    /// `EVENT_FIELD_ASSIGNMENT`.
    EventField(EventFieldRef),
    /// `CONSTANT_VALUE_ASSIGNMENT`.
    ConstantValue(Variant),
}

impl AssignmentInput {
    /// Discriminant.
    #[must_use]
    pub fn kind(&self) -> AssignmentKind {
        match self {
            Self::DataItem(_) => AssignmentKind::DataItem,
            Self::EventField(_) => AssignmentKind::EventField,
            Self::ConstantValue(_) => AssignmentKind::ConstantValue,
        }
    }
}

/// Spec Tab 8.15 — `FieldAssignment` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAssignment {
    /// `string dds_output_field_ref` — vollqualifizierter Member-Name
    /// des Topic-Type-Felds (`<member>[.<nested>]*`, Spec §8.4.2.4
    /// normativ).
    pub dds_output_field_ref: String,
    /// `@optional @external OpcUaInput opcua_input_ref` — wenn None,
    /// gilt der Default-Input aus der `Assignment`-Ebene.
    pub opcua_input_ref: Option<String>,
    /// `AssignmentInput assignment_input`.
    pub assignment_input: AssignmentInput,
}

/// Spec Tab 8.15 — `Assignment` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// `@external DdsOutput dds_output_ref` — Verweis (per Name).
    pub dds_output_ref: String,
    /// `@external OpcUaInput opcua_input_ref` — Verweis (per Name).
    pub opcua_input_ref: String,
    /// `sequence<FieldAssignment> field_assignments`.
    pub field_assignments: Vec<FieldAssignment>,
}

/// Spec Tab 8.15 — `InputOutputMapping` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct InputOutputMapping {
    /// `sequence<Assignment> assignments`.
    pub assignments: Vec<Assignment>,
}

// -------------------------------------------------------------------
// Tab 8.8 — SubscriptionMapping (Top-Level-Aggregate).
// -------------------------------------------------------------------

/// Spec Tab 8.8 — `SubscriptionMapping` (Top-Level-Config). Eine
/// vollstaendige Gateway-Sub-Mapping-Section.
#[derive(Debug, Clone, PartialEq)]
pub struct SubscriptionMapping {
    /// `sequence<OpcUaInput> opcua_inputs`.
    pub opcua_inputs: Vec<OpcUaInput>,
    /// `sequence<DdsOutput> dds_outputs`.
    pub dds_outputs: Vec<DdsOutput>,
    /// `InputOutputMapping mapping`.
    pub mapping: InputOutputMapping,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::data_value::VariantValue;

    #[test]
    fn data_item_with_two_filters_is_invalid() {
        let item = DataItem {
            node_id: NodeId::numeric(0, 1),
            attribute_id: 13,
            sampling_interval: 100.0,
            queue_size: 1,
            discard_oldest: true,
            data_change_filter: Some(DataChangeFilter {
                trigger: DataChangeTrigger::StatusValue,
                deadband_type: 0,
                deadband_value: 0.0,
            }),
            aggregate_filter: Some(AggregateFilter {
                start_time: 0,
                aggregate_type: NodeId::numeric(0, 2342),
                processing_interval: 1000.0,
                aggregate_configuration: AggregateConfiguration {
                    user_server_capabilities_defaults: true,
                    treat_uncertain_as_bad: false,
                    percent_data_bad: 0,
                    percent_data_good: 0,
                    use_sloped_extrapolation: false,
                },
            }),
        };
        assert_eq!(item.validate_filters(), Err(DualFilterError));
    }

    #[test]
    fn data_item_with_one_filter_is_valid() {
        let item = DataItem {
            node_id: NodeId::numeric(0, 1),
            attribute_id: 13,
            sampling_interval: 100.0,
            queue_size: 1,
            discard_oldest: true,
            data_change_filter: Some(DataChangeFilter {
                trigger: DataChangeTrigger::StatusValue,
                deadband_type: 0,
                deadband_value: 0.0,
            }),
            aggregate_filter: None,
        };
        assert!(item.validate_filters().is_ok());
    }

    #[test]
    fn subscription_protocol_lifetime_constraint() {
        // Spec §8.4.1.1: lifetime >= 3*keepalive.
        let ok = SubscriptionProtocol {
            requested_publishing_interval: 100.0,
            requested_lifetime_count: 30,
            requested_max_keepalive_count: 10,
            max_notifications_per_publish: 0,
            publishing_enabled: true,
            priority: 0,
        };
        assert!(ok.lifetime_constraint_ok());

        let bad = SubscriptionProtocol {
            requested_lifetime_count: 29,
            ..ok
        };
        assert!(!bad.lifetime_constraint_ok());
    }

    #[test]
    fn monitored_item_kind_discriminants() {
        let d = MonitoredItem::Data(DataItem {
            node_id: NodeId::numeric(0, 1),
            attribute_id: 13,
            sampling_interval: 100.0,
            queue_size: 1,
            discard_oldest: true,
            data_change_filter: None,
            aggregate_filter: None,
        });
        assert_eq!(d.kind(), MonitoredItemKind::Data);
    }

    #[test]
    fn assignment_input_kind_discriminants() {
        let c = AssignmentInput::ConstantValue(Variant::scalar(VariantValue::Int32(42)));
        assert_eq!(c.kind(), AssignmentKind::ConstantValue);
        let d = AssignmentInput::DataItem(DataItemRef {
            data_item_name: "temp".into(),
        });
        assert_eq!(d.kind(), AssignmentKind::DataItem);
        let e = AssignmentInput::EventField(EventFieldRef {
            event_name: "alarm".into(),
            event_field_index: 3,
        });
        assert_eq!(e.kind(), AssignmentKind::EventField);
    }
}
