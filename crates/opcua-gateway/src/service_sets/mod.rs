// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OPC UA Service Sets Mapping — Spec §8.3.
//!
//! Defines the DDS service interfaces that map the OPC-UA service sets
//! from [OPCUA-04] onto DDS-RPC. The spec groups the
//! mapping definitions into four service sets:
//!
//! * **View** (§8.3.2) — Browse, BrowseNext, TranslateBrowsePathsToNodeIds,
//!   RegisterNodes, UnregisterNodes.
//! * **Query** (§8.3.3) — QueryFirst, QueryNext.
//! * **Attribute** (§8.3.4) — Read, HistoryRead, Write, HistoryUpdate.
//! * **Method** (§8.3.5) — Call.
//!
//! Each service set lives in the IDL module `OMG::DDSOPCUA::OPCUA2DDS::<SET>`.
//! In Rust each set is a sub-module (`view`, `query`, `attribute`,
//! `method`) that exports the set-specific types + the service-interface
//! description.
//!
//! The types defined here are **pure data structs** that mirror the IDL
//! from tables 8.3-8.7. Wire encoding (CDR) stays in
//! `crates/cdr/`, service dispatch + topic generation stays in
//! `crates/rpc/`. This module provides the schema information that an
//! IDL codegen consumer needs to emit wire bindings from these types.
//!
//! Cross-ref: `internal/standards/cache/omg/dds-opcua-1.0.pdf` §8.3,
//! tables 8.3 (standard DataTypes/NodeClasses), 8.4 (View), 8.5
//! (Query), 8.6 (Attribute), 8.7 (Method).

use alloc::string::String;
use alloc::vec::Vec;

use crate::data_value::Variant;
use crate::node_id::{ExpandedNodeId, NodeId};
use crate::types::{ByteString, LocalizedText, NodeClass, QualifiedName, StatusCode};

pub mod attribute;
pub mod method;
pub mod query;
pub mod view;

/// IDL module path for the common types from Tab 8.3 — Spec §8.3
/// introductory: `OMG::DDSOPCUA::OPCUA2DDS`.
pub const IDL_MODULE_BASE: &str = "OMG::DDSOPCUA::OPCUA2DDS";

/// IDL module path for the View set (Tab 8.4) — Spec §8.3.2.
pub const IDL_MODULE_VIEW: &str = "OMG::DDSOPCUA::OPCUA2DDS::VIEW";

/// IDL module path for the Query set (Tab 8.5) — Spec §8.3.3.
pub const IDL_MODULE_QUERY: &str = "OMG::DDSOPCUA::OPCUA2DDS::QUERY";

/// IDL module path for the Attribute set (Tab 8.6) — Spec §8.3.4.
pub const IDL_MODULE_ATTRIBUTE: &str = "OMG::DDSOPCUA::OPCUA2DDS::ATTRIBUTE";

/// IDL module path for the Method set (Tab 8.7) — Spec §8.3.5.
pub const IDL_MODULE_METHOD: &str = "OMG::DDSOPCUA::OPCUA2DDS::METHOD";

// -------------------------------------------------------------------
// Tab 8.3 — standard DataTypes / NodeClasses mapping (shared).
// -------------------------------------------------------------------

/// Spec Tab 8.3 — `Duration` as `double` (ms).
pub type Duration = f64;

/// Spec Tab 8.3 — `UtcTime` as `DateTime` (i64 Windows FILETIME ticks).
pub type UtcTime = i64;

/// Spec Tab 8.3 — `ContinuationPoint` as `ByteString`.
pub type ContinuationPoint = ByteString;

/// Spec Tab 8.3 — `Index` as `uint32`.
pub type Index = u32;

/// Spec Tab 8.3 — `IntegerId` as `uint32`.
pub type IntegerId = u32;

/// Spec Tab 8.3 — `Counter` as `uint32`.
pub type Counter = u32;

/// Spec Tab 8.3 — `NumericRange` as `string` (e.g. `"3:5"` for an
/// array slice or `"3:5,1:7"` for a multi-dim slice).
pub type NumericRange = String;

/// Spec Tab 8.3 — `BaseNodeClass` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct BaseNodeClass {
    /// `NodeId node_id` — unique NodeId.
    pub node_id: NodeId,
    /// `NodeClass node_class` — Spec §8.3.1 Tab 8.3.
    pub node_class: NodeClass,
    /// `QualifiedName browse_name`.
    pub browse_name: QualifiedName,
    /// `LocalizedText display_name`.
    pub display_name: LocalizedText,
    /// `@optional LocalizedText description`.
    pub description: Option<LocalizedText>,
    /// `@optional uint32 write_mask`.
    pub write_mask: Option<u32>,
    /// `@optional uint32 user_write_mask`.
    pub user_write_mask: Option<u32>,
}

/// Spec Tab 8.3 — `EnumValueType` (member of a DataType.enum_values).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumValueType {
    /// `int64 value`.
    pub value: i64,
    /// `LocalizedText display_name`.
    pub display_name: LocalizedText,
    /// `LocalizedText description`.
    pub description: LocalizedText,
}

/// Spec Tab 8.3 — `DataType` (`@nested`, inherits `BaseNodeClass`).
#[derive(Debug, Clone, PartialEq)]
pub struct DataType {
    /// Inherited base fields.
    pub base: BaseNodeClass,
    /// `boolean is_abstract`.
    pub is_abstract: bool,
    /// `sequence<NodeId> has_property`.
    pub has_property: Vec<NodeId>,
    /// `sequence<NodeId> has_subtype`.
    pub has_subtype: Vec<NodeId>,
    /// `sequence<NodeId> has_encoding`.
    pub has_encoding: Vec<NodeId>,
    /// `@optional string node_version`.
    pub node_version: Option<String>,
    /// `@optional sequence<LocalizedText> enum_strings`.
    pub enum_strings: Option<Vec<LocalizedText>>,
    /// `@optional sequence<EnumValueType> enum_values`.
    pub enum_values: Option<Vec<EnumValueType>>,
    /// `@optional sequence<LocalizedText> option_set_values`.
    pub option_set_values: Option<Vec<LocalizedText>>,
}

/// Spec Tab 8.3 — `ViewDescription` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewDescription {
    /// `NodeId view_id`.
    pub view_id: NodeId,
    /// `UtcTime timestamp`.
    pub timestamp: UtcTime,
    /// `uint32 view_version`.
    pub view_version: u32,
}

/// Spec Tab 8.3 — `RelativePathElement` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct RelativePathElement {
    /// `NodeId reference_type_id`.
    pub reference_type_id: NodeId,
    /// `boolean is_inverse`.
    pub is_inverse: bool,
    /// `boolean include_subtypes`.
    pub include_subtypes: bool,
    /// `QualifiedName target_name`.
    pub target_name: QualifiedName,
}

/// Spec Tab 8.3 — `RelativePath` (`@nested`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RelativePath {
    /// `sequence<RelativePathElement> elements`.
    pub elements: Vec<RelativePathElement>,
}

/// Spec Tab 8.3 — `ReferenceDescription` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceDescription {
    /// `NodeId reference_type_id`.
    pub reference_type_id: NodeId,
    /// `boolean is_forward`.
    pub is_forward: bool,
    /// `ExpandedNodeId node_id`.
    pub node_id: ExpandedNodeId,
    /// `QualifiedName browse_name`.
    pub browse_name: QualifiedName,
    /// `LocalizedText display_name`.
    pub display_name: LocalizedText,
    /// `NodeClass node_class`.
    pub node_class: NodeClass,
    /// `ExpandedNodeId type_definition`.
    pub type_definition: ExpandedNodeId,
}

/// Spec Tab 8.3 — `BrowseResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseResult {
    /// `StatusCode status_code`.
    pub status_code: StatusCode,
    /// `ContinuationPoint continuation_point`.
    pub continuation_point: ContinuationPoint,
    /// `sequence<ReferenceDescription> references`.
    pub references: Vec<ReferenceDescription>,
}

/// Spec Tab 8.3 — `ResponseHeader` (`@nested @appendable`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseHeader {
    /// `UtcTime timestamp`.
    pub timestamp: UtcTime,
    /// `IntegerId request_handle`.
    pub request_handle: IntegerId,
    /// `StatusCode service_result`.
    pub service_result: StatusCode,
    /// `DiagnosticInfo service_diagnostics`.
    pub service_diagnostics: DiagnosticInfo,
    /// `sequence<string> string_table`.
    pub string_table: Vec<String>,
}

/// Spec Tab 8.2 (Built-in) — `DiagnosticInfo` (`@mutable`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiagnosticInfo {
    /// `@id(1) @optional int32 symbolic_id`.
    pub symbolic_id: Option<i32>,
    /// `@id(2) @optional int32 namespace_uri`.
    pub namespace_uri: Option<i32>,
    /// `@id(4) @optional int32 localized_text`.
    pub localized_text: Option<i32>,
    /// `@id(8) @optional int32 locale`.
    pub locale: Option<i32>,
    /// `@id(10) @optional string additional_info`.
    pub additional_info: Option<String>,
    /// `@id(32) @optional StatusCode inner_status_code`.
    pub inner_status_code: Option<StatusCode>,
    /// `@id(64) @optional DiagnosticInfo inner_diagnostic_info`.
    /// Boxed so that the recursive type has a finite size.
    pub inner_diagnostic_info: Option<alloc::boxed::Box<DiagnosticInfo>>,
}

/// Spec Tab 8.3 — `ExtensibleParameter` (base struct for all
/// extensions with `: ExtensibleParameter` inheritance in the IDL).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensibleParameter {
    /// `NodeId parameter_type_id`.
    pub parameter_type_id: NodeId,
}

// -------------------------------------------------------------------
// Tab 8.3 — ContentFilter (filter subsystem for Query + EventFilter).
// -------------------------------------------------------------------

/// Spec Tab 8.3 — `FilterOperator` (18 values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FilterOperator {
    /// `@value(0)`.
    Equals = 0,
    /// `@value(1)`.
    IsNull = 1,
    /// `@value(2)`.
    GreaterThan = 2,
    /// `@value(3)`.
    LessThan = 3,
    /// `@value(4)`.
    GreaterThanOrEqual = 4,
    /// `@value(5)`.
    LessThanOrEqual = 5,
    /// `@value(6)`.
    Like = 6,
    /// `@value(7)`.
    Not = 7,
    /// `@value(8)`.
    Between = 8,
    /// `@value(9)`.
    InList = 9,
    /// `@value(10)`.
    And = 10,
    /// `@value(11)`.
    Or = 11,
    /// `@value(12)`.
    Cast = 12,
    /// `@value(13)`.
    InView = 13,
    /// `@value(14)`.
    OfType = 14,
    /// `@value(15)`.
    RelatedTo = 15,
    /// `@value(16)`.
    BitwiseAnd = 16,
    /// `@value(17)`.
    BitwiseOr = 17,
}

/// Spec Tab 8.3 — `FilterOperandKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperandKind {
    /// `ELEMENT_FILTER_OPERAND_KIND`.
    Element,
    /// `LITERAL_FILTER_OPERAND_KIND`.
    Literal,
    /// `ATTRIBUTE_FILTER_OPERAND_KIND`.
    Attribute,
    /// `SIMPLE_ATTRIBUTE_FILTER_OPERAND_KIND`.
    SimpleAttribute,
}

/// Spec Tab 8.3 — `ElementOperand` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ElementOperand {
    /// `uint32 index` — index into the filter element array.
    pub index: u32,
}

/// Spec Tab 8.3 — `LiteralOperand` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct LiteralOperand {
    /// `BaseDataType value` (= Variant).
    pub value: Variant,
}

/// Spec Tab 8.3 — `AttributeOperand` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeOperand {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `string operand_alias`.
    pub operand_alias: String,
    /// `RelativePath browse_path`.
    pub browse_path: RelativePath,
    /// `IntegerId attribute_id`.
    pub attribute_id: IntegerId,
    /// `NumericRange index_range`.
    pub index_range: NumericRange,
}

/// Spec Tab 8.3 — `SimpleAttributeOperand` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleAttributeOperand {
    /// `NodeId type_id`.
    pub type_id: NodeId,
    /// `sequence<QualifiedName> browse_path`.
    pub browse_path: Vec<QualifiedName>,
    /// `IntegerId attribute_id`.
    pub attribute_id: IntegerId,
    /// `NumericRange index_range`.
    pub index_range: NumericRange,
}

/// Spec Tab 8.3 — `FilterOperand` (`@nested union switch
/// (FilterOperandKind)`).
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOperand {
    /// `ELEMENT_FILTER_OPERAND_KIND`.
    Element(ElementOperand),
    /// `LITERAL_FILTER_OPERAND_KIND`.
    Literal(LiteralOperand),
    /// `ATTRIBUTE_FILTER_OPERAND_KIND`.
    Attribute(AttributeOperand),
    /// `SIMPLE_ATTRIBUTE_FILTER_OPERAND_KIND`.
    SimpleAttribute(SimpleAttributeOperand),
}

impl FilterOperand {
    /// Discriminant for the IDL switch case.
    #[must_use]
    pub fn kind(&self) -> FilterOperandKind {
        match self {
            Self::Element(_) => FilterOperandKind::Element,
            Self::Literal(_) => FilterOperandKind::Literal,
            Self::Attribute(_) => FilterOperandKind::Attribute,
            Self::SimpleAttribute(_) => FilterOperandKind::SimpleAttribute,
        }
    }
}

/// Spec Tab 8.3 — `ExtensibleParameterFilterOperand`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensibleParameterFilterOperand {
    /// Inherited `parameter_type_id`.
    pub base: ExtensibleParameter,
    /// `FilterOperand parameter_data`.
    pub parameter_data: FilterOperand,
}

/// Spec Tab 8.3 — `ContentFilterElement` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ContentFilterElement {
    /// `FilterOperator filter_operator`.
    pub filter_operator: FilterOperator,
    /// `sequence<ExtensibleParameterFilterOperand> filter_operands`.
    pub filter_operands: Vec<ExtensibleParameterFilterOperand>,
}

/// Spec Tab 8.3 — `ContentFilterElementResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ContentFilterElementResult {
    /// `StatusCode status_code`.
    pub status_code: StatusCode,
    /// `sequence<StatusCode> operand_status_codes`.
    pub operand_status_codes: Vec<StatusCode>,
    /// `sequence<DiagnosticInfo> operand_diagnostic_infos`.
    pub operand_diagnostic_infos: Vec<DiagnosticInfo>,
}

/// Spec Tab 8.3 — `ContentFilter` (`@nested`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContentFilter {
    /// `sequence<ContentFilterElement> content_filter_element`.
    pub content_filter_element: Vec<ContentFilterElement>,
}

/// Spec Tab 8.3 — `ContentFilterResult` (`@nested`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContentFilterResult {
    /// `sequence<ContentFilterElementResult> element_results`.
    pub element_results: Vec<ContentFilterElementResult>,
    /// `sequence<DiagnosticInfo> element_diagnostic_infos`.
    pub element_diagnostic_infos: Vec<DiagnosticInfo>,
}

// -------------------------------------------------------------------
// Tab 8.3 — subscription/monitoring (also referenced in service sets).
// -------------------------------------------------------------------

/// Spec Tab 8.3 — `TimestampsToReturn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TimestampsToReturn {
    /// `@value(0) SOURCE_TIMESTAMPS_TO_RETURN`.
    Source = 0,
    /// `@value(1) SERVER_TIMESTAMPS_TO_RETURN`.
    Server = 1,
    /// `@value(2) BOTH_TIMESTAMPS_TO_RETURN`.
    Both = 2,
    /// `@value(3) NEITHER_TIMESTAMPS_TO_RETURN`.
    Neither = 3,
}

/// Spec Tab 8.3 — `ReadValueId` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadValueId {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `IntegerId attribute_id`.
    pub attribute_id: IntegerId,
    /// `NumericRange index_range`.
    pub index_range: NumericRange,
    /// `QualifiedName data_encoding`.
    pub data_encoding: QualifiedName,
}

/// Spec Tab 8.3 — `QueryDataSet` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryDataSet {
    /// `ExpandedNodeId node_id`.
    pub node_id: ExpandedNodeId,
    /// `ExpandedNodeId type_definition_node`.
    pub type_definition_node: ExpandedNodeId,
    /// `sequence<BaseDataType> values` (= variant sequence).
    pub values: Vec<Variant>,
}

// -------------------------------------------------------------------
// Service-interface description — compact data representation per
// service set, for codegen consumers.
// -------------------------------------------------------------------

/// Parameter direction in a service method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDir {
    /// `in` — request-only.
    In,
    /// `out` — reply-only.
    Out,
}

/// Parameter description in a service-set method signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceParam {
    /// IDL parameter name (e.g. `nodes_to_browse`).
    pub name: &'static str,
    /// IDL type spec as a source snippet (e.g.
    /// `sequence<BrowseDescription>`).
    pub type_spec: &'static str,
    /// Direction (`in`/`out`). Spec §8.3 uses only these two,
    /// `inout` does not occur.
    pub direction: ParamDir,
}

/// Service method description — corresponds to a method in the
/// `@DDSService interface ...` definition from Spec §8.3.x.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceMethod {
    /// Method name (e.g. `browse`).
    pub name: &'static str,
    /// Return type (always `ResponseHeader` in §8.3, but modeled
    /// explicitly for spec-conformance tests).
    pub return_type: &'static str,
    /// Parameter list in source order.
    pub params: &'static [ServiceParam],
}

/// Complete service set — module path + interface name + methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSet {
    /// IDL module path, e.g. `OMG::DDSOPCUA::OPCUA2DDS::VIEW`.
    pub module_path: &'static str,
    /// Interface name (e.g. `View`).
    pub interface_name: &'static str,
    /// Methods per the spec tables 8.3.x.2.
    pub methods: &'static [ServiceMethod],
}

/// Returns all four service-set descriptions from §8.3 (View, Query,
/// Attribute, Method) in spec order.
#[must_use]
pub fn all_service_sets() -> [ServiceSet; 4] {
    [
        view::SERVICE_SET,
        query::SERVICE_SET,
        attribute::SERVICE_SET,
        method::SERVICE_SET,
    ]
}

// Re-exports of the service-set-specific top-level types.
pub use attribute::{
    AttributeServiceMethods, DeleteAtTimeDetails, DeleteEventDetails, DeleteRawModifiedDetails,
    ExtensibleParameterHistoryData, ExtensibleParameterHistoryReadDetails,
    ExtensibleParameterHistoryUpdate, ExtensibleParameterHistoryUpdateDetails, HistoryData,
    HistoryEvent, HistoryEventFieldList, HistoryReadDetails, HistoryReadDetailsKind,
    HistoryReadResult, HistoryReadValueId, HistoryUpdateDetails, HistoryUpdateDetailsKind,
    HistoryUpdateResult, HistoryUpdateType, PerformUpdateType, ReadAtTimeDetails, ReadEventDetails,
    ReadProcessedDetails, ReadRawModifiedDetails, UpdateDataDetails, UpdateEventDetails,
    UpdateStructureDataDetails, WriteValue,
};
pub use method::{CallMethodRequest, CallMethodResult, MethodServiceMethods};
pub use query::{NodeTypeDescription, ParsingResult, QueryDataDescription, QueryServiceMethods};
pub use view::{
    BrowseDescription, BrowseDirection, BrowsePath, BrowsePathResult, BrowsePathTarget,
    ViewServiceMethods,
};

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idl_module_paths_match_spec() {
        assert_eq!(IDL_MODULE_BASE, "OMG::DDSOPCUA::OPCUA2DDS");
        assert_eq!(IDL_MODULE_VIEW, "OMG::DDSOPCUA::OPCUA2DDS::VIEW");
        assert_eq!(IDL_MODULE_QUERY, "OMG::DDSOPCUA::OPCUA2DDS::QUERY");
        assert_eq!(IDL_MODULE_ATTRIBUTE, "OMG::DDSOPCUA::OPCUA2DDS::ATTRIBUTE");
        assert_eq!(IDL_MODULE_METHOD, "OMG::DDSOPCUA::OPCUA2DDS::METHOD");
    }

    #[test]
    fn all_service_sets_covers_four_sets() {
        let sets = all_service_sets();
        let names: alloc::vec::Vec<&str> = sets.iter().map(|s| s.interface_name).collect();
        assert_eq!(names, alloc::vec!["View", "Query", "Attribute", "Method"]);
    }

    #[test]
    fn filter_operator_values_match_spec() {
        // Spec Tab 8.3 enum FilterOperator @value(0)..@value(17).
        assert_eq!(FilterOperator::Equals as u8, 0);
        assert_eq!(FilterOperator::IsNull as u8, 1);
        assert_eq!(FilterOperator::Like as u8, 6);
        assert_eq!(FilterOperator::Between as u8, 8);
        assert_eq!(FilterOperator::And as u8, 10);
        assert_eq!(FilterOperator::Or as u8, 11);
        assert_eq!(FilterOperator::OfType as u8, 14);
        assert_eq!(FilterOperator::BitwiseOr as u8, 17);
    }

    #[test]
    fn timestamps_to_return_values_match_spec() {
        assert_eq!(TimestampsToReturn::Source as u8, 0);
        assert_eq!(TimestampsToReturn::Server as u8, 1);
        assert_eq!(TimestampsToReturn::Both as u8, 2);
        assert_eq!(TimestampsToReturn::Neither as u8, 3);
    }

    #[test]
    fn filter_operand_kind_discriminant_roundtrip() {
        let op = FilterOperand::Element(ElementOperand { index: 7 });
        assert_eq!(op.kind(), FilterOperandKind::Element);
    }
}
