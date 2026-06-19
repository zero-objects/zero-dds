// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Attribute Service Set — Spec §8.3.4.
//!
//! Modul: `OMG::DDSOPCUA::OPCUA2DDS::ATTRIBUTE`. Interface: `Attribute`.
//!
//! Methods: `read`, `history_read`, `write`, `history_update`.

use alloc::vec::Vec;

use crate::data_value::DataValue;
use crate::node_id::NodeId;
use crate::types::{ByteString, StatusCode};

use super::{
    ContentFilter, DiagnosticInfo, ExtensibleParameter, IDL_MODULE_ATTRIBUTE, IntegerId,
    NumericRange, ParamDir, ServiceMethod, ServiceParam, ServiceSet, SimpleAttributeOperand,
    UtcTime,
};

// -------------------------------------------------------------------
// Tab 8.6 — Attribute Service Set Types.
// -------------------------------------------------------------------

/// Spec Tab 8.6 — `EventFilter` (`@nested`). Also referenced by Tab 8.3
/// (subscription); central here, because attribute is the first
/// consumer.
#[derive(Debug, Clone, PartialEq)]
pub struct EventFilter {
    /// `sequence<SimpleAttributeOperand> select_clauses`.
    pub select_clauses: Vec<SimpleAttributeOperand>,
    /// `ContentFilter where_clause`.
    pub where_clause: ContentFilter,
}

/// Spec Tab 8.6 — `HistoryEventFieldList` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEventFieldList {
    /// `sequence<BaseDataType> event_fields` (= variant sequence).
    pub event_fields: Vec<crate::data_value::Variant>,
}

/// Spec Tab 8.6 — `HistoryEvent` (top-level struct, no `@nested`
/// per the spec).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEvent {
    /// `sequence<HistoryEventFieldList> events`.
    pub events: Vec<HistoryEventFieldList>,
}

/// Spec Tab 8.6 — `HistoryData` (`@nested`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryData {
    /// `sequence<DataValue> data_values`.
    pub data_values: Vec<DataValue>,
}

/// Spec Tab 8.6 — `ExtensibleParameterHistoryData` (inheritance from
/// `ExtensibleParameter`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensibleParameterHistoryData {
    /// Geerbtes `parameter_type_id`.
    pub base: ExtensibleParameter,
    /// `HistoryData parameter_data`.
    pub parameter_data: HistoryData,
}

/// Spec Tab 8.6 — `HistoryReadResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReadResult {
    /// `StatusCode status_code`.
    pub status_code: StatusCode,
    /// `ContinuationPoint continuation_point`.
    pub continuation_point: ByteString,
    /// `ExtensibleParameterHistoryData history_data`.
    pub history_data: ExtensibleParameterHistoryData,
}

/// Spec Tab 8.6 — `HistoryReadValueId` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReadValueId {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `NumericRange index_range`.
    pub index_range: NumericRange,
    /// `QualifiedName data_encoding`.
    pub data_encoding: crate::types::QualifiedName,
    /// `ContinuationPoint continuation_point`.
    pub continuation_point: ByteString,
}

/// Spec Tab 8.6 — `WriteValue` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct WriteValue {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `IntegerId attribute_id`.
    pub attribute_id: IntegerId,
    /// `NumericRange index_range`.
    pub index_range: NumericRange,
    /// `DataValue value`.
    pub value: DataValue,
}

/// Spec Tab 8.6 — `HistoryUpdateResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryUpdateResult {
    /// `StatusCode status_code`.
    pub status_code: StatusCode,
    /// `sequence<StatusCode> operation_results`.
    pub operation_results: Vec<StatusCode>,
    /// `sequence<DiagnosticInfo> diagnostic_infos`.
    pub diagnostic_infos: Vec<DiagnosticInfo>,
}

/// Spec Tab 8.6 — `HistoryUpdateType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HistoryUpdateType {
    /// `@value(1) INSERT_HISTORY_UPDATE_TYPE`.
    Insert = 1,
    /// `@value(2) REPLACE_HISTORY_UPDATE_TYPE`.
    Replace = 2,
    /// `@value(3) UPDATE_HISTORY_UPDATE_TYPE`.
    Update = 3,
    /// `@value(4) DELETE_HISTORY_UPDATE_TYPE`.
    Delete = 4,
}

/// Spec Tab 8.6 — `ExtensibleParameterHistoryUpdate`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensibleParameterHistoryUpdate {
    /// Geerbtes `parameter_type_id`.
    pub base: ExtensibleParameter,
    /// `HistoryUpdateType parameter_data`.
    pub parameter_data: HistoryUpdateType,
}

/// Spec Tab 8.6 — `HistoryReadDetailsKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryReadDetailsKind {
    /// `READ_EVENT_HISTORY_READ_DETAILS_KIND`.
    Event,
    /// `READ_RAW_MODIFIED_HISTORY_READ_DETAILS_KIND`.
    RawModified,
    /// `READ_PROCESSED_HISTORY_READ_DETAILS_KIND`.
    Processed,
    /// `READ_AT_TIME_HISTORY_READ_DETAILS_KIND`.
    AtTime,
}

/// Spec Tab 8.6 — `ReadEventDetails` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadEventDetails {
    /// `Counter num_values_per_node`.
    pub num_values_per_node: super::Counter,
    /// `UtcTime start_time`.
    pub start_time: UtcTime,
    /// `UtcTime end_time`.
    pub end_time: UtcTime,
    /// `EventFilter filter`.
    pub filter: EventFilter,
}

/// Spec Tab 8.6 — `ReadRawModifiedDetails` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadRawModifiedDetails {
    /// `boolean is_read_modified`.
    pub is_read_modified: bool,
    /// `UtcTime start_time`.
    pub start_time: UtcTime,
    /// `UtcTime end_time`.
    pub end_time: UtcTime,
    /// `Counter num_values_per_node`.
    pub num_values_per_node: super::Counter,
    /// `boolean return_bounds`.
    pub return_bounds: bool,
}

/// Spec Tab 8.6 — `AggregateConfiguration` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateConfiguration {
    /// `boolean user_server_capabilities_defaults`.
    pub user_server_capabilities_defaults: bool,
    /// `boolean treat_uncertain_as_bad`.
    pub treat_uncertain_as_bad: bool,
    /// `octet percent_data_bad`.
    pub percent_data_bad: u8,
    /// `octet percent_data_good`.
    pub percent_data_good: u8,
    /// `boolean use_sloped_extrapolation`.
    pub use_sloped_extrapolation: bool,
}

/// Spec Tab 8.6 — `ReadProcessedDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadProcessedDetails {
    /// `UtcTime start_time`.
    pub start_time: UtcTime,
    /// `UtcTime end_time`.
    pub end_time: UtcTime,
    /// `Duration processing_interval`.
    pub processing_interval: super::Duration,
    /// `sequence<NodeId> aggregate_type`.
    pub aggregate_type: Vec<NodeId>,
    /// `AggregateConfiguration aggregate_configuration`.
    pub aggregate_configuration: AggregateConfiguration,
}

/// Spec Tab 8.6 — `ReadAtTimeDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAtTimeDetails {
    /// `sequence<UtcTime> req_times`.
    pub req_times: Vec<UtcTime>,
    /// `boolean use_simple_bounds`.
    pub use_simple_bounds: bool,
}

/// Spec Tab 8.6 — `HistoryReadDetails` (`@nested union`).
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryReadDetails {
    /// `READ_EVENT_HISTORY_READ_DETAILS_KIND`.
    Event(ReadEventDetails),
    /// `READ_RAW_MODIFIED_HISTORY_READ_DETAILS_KIND`.
    RawModified(ReadRawModifiedDetails),
    /// `READ_PROCESSED_HISTORY_READ_DETAILS_KIND`.
    Processed(ReadProcessedDetails),
    /// `READ_AT_TIME_HISTORY_READ_DETAILS_KIND`.
    AtTime(ReadAtTimeDetails),
}

impl HistoryReadDetails {
    /// Discriminant.
    #[must_use]
    pub fn kind(&self) -> HistoryReadDetailsKind {
        match self {
            Self::Event(_) => HistoryReadDetailsKind::Event,
            Self::RawModified(_) => HistoryReadDetailsKind::RawModified,
            Self::Processed(_) => HistoryReadDetailsKind::Processed,
            Self::AtTime(_) => HistoryReadDetailsKind::AtTime,
        }
    }
}

/// Spec Tab 8.6 — `ExtensibleParameterHistoryReadDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensibleParameterHistoryReadDetails {
    /// Geerbtes `parameter_type_id`.
    pub base: ExtensibleParameter,
    /// `HistoryReadDetails parameter_data`.
    pub parameter_data: HistoryReadDetails,
}

/// Spec Tab 8.6 — `PerformUpdateType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PerformUpdateType {
    /// `@value(1) INSERT_PERFORM_UPDATE_TYPE`.
    Insert = 1,
    /// `@value(2) REPLACE_PERFORM_UPDATE_TYPE`.
    Replace = 2,
    /// `@value(3) UPDATE_PERFORM_UPDATE_TYPE`.
    Update = 3,
    /// `@value(4) REMOVE_PERFORM_UPDATE_TYPE`.
    Remove = 4,
}

/// Spec Tab 8.6 — `UpdateDataDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateDataDetails {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `PerformUpdateType perform_insert_replace`.
    pub perform_insert_replace: PerformUpdateType,
    /// `sequence<DataValue> update_values`.
    pub update_values: Vec<DataValue>,
}

/// Spec Tab 8.6 — `UpdateStructureDataDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStructureDataDetails {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `PerformUpdateType perform_insert_replace`.
    pub perform_insert_replace: PerformUpdateType,
    /// `sequence<DataValue> update_values`.
    pub update_values: Vec<DataValue>,
}

/// Spec Tab 8.6 — `UpdateEventDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateEventDetails {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `PerformUpdateType perform_insert_replace`.
    pub perform_insert_replace: PerformUpdateType,
    /// `EventFilter filter`.
    pub filter: EventFilter,
    /// `sequence<HistoryEventFieldList> event_data`.
    pub event_data: Vec<HistoryEventFieldList>,
}

/// Spec Tab 8.6 — `DeleteRawModifiedDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteRawModifiedDetails {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `boolean is_delete_modified`.
    pub is_delete_modified: bool,
    /// `UtcTime start_time`.
    pub start_time: UtcTime,
    /// `UtcTime end_time`.
    pub end_time: UtcTime,
}

/// Spec Tab 8.6 — `DeleteAtTimeDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteAtTimeDetails {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `sequence<UtcTime> req_times`.
    pub req_times: Vec<UtcTime>,
}

/// Spec Tab 8.6 — `DeleteEventDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteEventDetails {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `sequence<ByteString> event_id`.
    pub event_id: Vec<ByteString>,
}

/// Spec Tab 8.6 — `HistoryUpdateDetailsKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryUpdateDetailsKind {
    /// `UPDATE_DATA_HISTORY_UPDATE_DETAILS_KIND`.
    UpdateData,
    /// `UPDATE_STRUCTURE_HISTORY_UPDATE_DETAILS_KIND`.
    UpdateStructure,
    /// `UPDATE_EVENT_HISTORY_UPDATE_DETAILS_KIND`.
    UpdateEvent,
    /// `DELETE_RAW_MODIFIED_HISTORY_UPDATE_DETAILS_KIND`.
    DeleteRawModified,
    /// `DELETE_AT_TIMES_HISTORY_UPDATE_DETAILS_KIND`.
    DeleteAtTimes,
    /// `DELETE_EVENTS_HISTORY_UPDATE_DETAILS_KIND`.
    DeleteEvents,
}

/// Spec Tab 8.6 — `HistoryUpdateDetails` (`union switch`).
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryUpdateDetails {
    /// `UPDATE_DATA_HISTORY_UPDATE_DETAILS_KIND`.
    UpdateData(UpdateDataDetails),
    /// `UPDATE_STRUCTURE_HISTORY_UPDATE_DETAILS_KIND`.
    UpdateStructure(UpdateStructureDataDetails),
    /// `UPDATE_EVENT_HISTORY_UPDATE_DETAILS_KIND`.
    UpdateEvent(UpdateEventDetails),
    /// `DELETE_RAW_MODIFIED_HISTORY_UPDATE_DETAILS_KIND`.
    DeleteRawModified(DeleteRawModifiedDetails),
    /// `DELETE_AT_TIMES_HISTORY_UPDATE_DETAILS_KIND`.
    DeleteAtTimes(DeleteAtTimeDetails),
    /// `DELETE_EVENTS_HISTORY_UPDATE_DETAILS_KIND`.
    DeleteEvents(DeleteEventDetails),
}

impl HistoryUpdateDetails {
    /// Discriminant.
    #[must_use]
    pub fn kind(&self) -> HistoryUpdateDetailsKind {
        match self {
            Self::UpdateData(_) => HistoryUpdateDetailsKind::UpdateData,
            Self::UpdateStructure(_) => HistoryUpdateDetailsKind::UpdateStructure,
            Self::UpdateEvent(_) => HistoryUpdateDetailsKind::UpdateEvent,
            Self::DeleteRawModified(_) => HistoryUpdateDetailsKind::DeleteRawModified,
            Self::DeleteAtTimes(_) => HistoryUpdateDetailsKind::DeleteAtTimes,
            Self::DeleteEvents(_) => HistoryUpdateDetailsKind::DeleteEvents,
        }
    }
}

/// Spec Tab 8.6 — `ExtensibleParameterHistoryUpdateDetails`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensibleParameterHistoryUpdateDetails {
    /// Geerbtes `parameter_type_id`.
    pub base: ExtensibleParameter,
    /// `HistoryUpdateDetails parameter_data`.
    pub parameter_data: HistoryUpdateDetails,
}

/// Method-name constants (Spec §8.3.4.2 IDL).
pub struct AttributeServiceMethods;

impl AttributeServiceMethods {
    /// Method `read`.
    pub const READ: &'static str = "read";
    /// Method `history_read`.
    pub const HISTORY_READ: &'static str = "history_read";
    /// Method `write`.
    pub const WRITE: &'static str = "write";
    /// Method `history_update`.
    pub const HISTORY_UPDATE: &'static str = "history_update";
}

const READ_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<DataValue>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "max_age",
        type_spec: "Duration",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "timestamps_to_return",
        type_spec: "TimestampsToReturn",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "nodes_to_read",
        type_spec: "sequence<ReadValueId>",
        direction: ParamDir::In,
    },
];

const HISTORY_READ_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<HistoryReadResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "history_read_details",
        type_spec: "ExtensibleParameterHistoryReadDetails",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "timestamps_to_return",
        type_spec: "TimestampsToReturn",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "release_continuation_points",
        type_spec: "boolean",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "nodes_to_read",
        type_spec: "sequence<HistoryReadValueId>",
        direction: ParamDir::In,
    },
];

const WRITE_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<StatusCode>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "nodes_to_write",
        type_spec: "sequence<WriteValue>",
        direction: ParamDir::In,
    },
];

const HISTORY_UPDATE_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<HistoryUpdateResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "details",
        type_spec: "sequence<ExtensibleParameterHistoryUpdateDetails>",
        direction: ParamDir::In,
    },
];

const METHODS: &[ServiceMethod] = &[
    ServiceMethod {
        name: AttributeServiceMethods::READ,
        return_type: "ResponseHeader",
        params: READ_PARAMS,
    },
    ServiceMethod {
        name: AttributeServiceMethods::HISTORY_READ,
        return_type: "ResponseHeader",
        params: HISTORY_READ_PARAMS,
    },
    ServiceMethod {
        name: AttributeServiceMethods::WRITE,
        return_type: "ResponseHeader",
        params: WRITE_PARAMS,
    },
    ServiceMethod {
        name: AttributeServiceMethods::HISTORY_UPDATE,
        return_type: "ResponseHeader",
        params: HISTORY_UPDATE_PARAMS,
    },
];

/// Attribute service set description (Spec §8.3.4.2).
pub const SERVICE_SET: ServiceSet = ServiceSet {
    module_path: IDL_MODULE_ATTRIBUTE,
    interface_name: "Attribute",
    methods: METHODS,
};

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_set_has_four_methods_per_spec() {
        assert_eq!(SERVICE_SET.methods.len(), 4);
        let names: alloc::vec::Vec<&str> = SERVICE_SET.methods.iter().map(|m| m.name).collect();
        assert_eq!(
            names,
            alloc::vec!["read", "history_read", "write", "history_update"]
        );
    }

    #[test]
    fn read_carries_timestamps_to_return() {
        let read = SERVICE_SET
            .methods
            .iter()
            .find(|m| m.name == "read")
            .expect("read");
        let p = read
            .params
            .iter()
            .find(|p| p.name == "timestamps_to_return")
            .expect("ts param");
        assert_eq!(p.type_spec, "TimestampsToReturn");
        assert_eq!(p.direction, ParamDir::In);
    }

    #[test]
    fn history_update_uses_extensible_details_sequence() {
        // Spec §8.3.4.2 IDL — `in sequence<ExtensibleParameterHistoryUpdateDetails> details`.
        let hu = SERVICE_SET
            .methods
            .iter()
            .find(|m| m.name == "history_update")
            .expect("history_update");
        let d = hu.params.iter().find(|p| p.name == "details").unwrap();
        assert_eq!(
            d.type_spec,
            "sequence<ExtensibleParameterHistoryUpdateDetails>"
        );
    }

    #[test]
    fn history_read_details_kind_discriminant() {
        let d = HistoryReadDetails::AtTime(ReadAtTimeDetails {
            req_times: alloc::vec::Vec::new(),
            use_simple_bounds: false,
        });
        assert_eq!(d.kind(), HistoryReadDetailsKind::AtTime);
    }

    #[test]
    fn history_update_details_kind_discriminant() {
        let d = HistoryUpdateDetails::DeleteEvents(DeleteEventDetails {
            node_id: NodeId::numeric(0, 0),
            event_id: alloc::vec::Vec::new(),
        });
        assert_eq!(d.kind(), HistoryUpdateDetailsKind::DeleteEvents);
    }

    #[test]
    fn perform_update_type_values_match_spec() {
        assert_eq!(PerformUpdateType::Insert as u8, 1);
        assert_eq!(PerformUpdateType::Replace as u8, 2);
        assert_eq!(PerformUpdateType::Update as u8, 3);
        assert_eq!(PerformUpdateType::Remove as u8, 4);
    }

    #[test]
    fn history_update_type_values_match_spec() {
        assert_eq!(HistoryUpdateType::Insert as u8, 1);
        assert_eq!(HistoryUpdateType::Replace as u8, 2);
        assert_eq!(HistoryUpdateType::Update as u8, 3);
        assert_eq!(HistoryUpdateType::Delete as u8, 4);
    }
}
