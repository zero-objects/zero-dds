// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Query Service Set — Spec §8.3.3.
//!
//! Modul: `OMG::DDSOPCUA::OPCUA2DDS::QUERY`. Interface: `Query`.
//!
//! Methods: `query_first`, `query_next`.

use alloc::vec::Vec;

use crate::types::StatusCode;

use super::{
    DiagnosticInfo, IDL_MODULE_QUERY, ParamDir, RelativePath, ServiceMethod, ServiceParam,
    ServiceSet,
};

/// Spec Tab 8.5 — `ParsingResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsingResult {
    /// `StatusCode status_code`.
    pub status_code: StatusCode,
    /// `sequence<StatusCode> data_status_codes`.
    pub data_status_codes: Vec<StatusCode>,
    /// `sequence<DiagnosticInfo> data_diagnostic_infos`.
    pub data_diagnostic_infos: Vec<DiagnosticInfo>,
}

/// Spec Tab 8.5 — `QueryDataDescription` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryDataDescription {
    /// `RelativePath relative_path`.
    pub relative_path: RelativePath,
    /// `IntegerId attribute_id`.
    pub attribute_id: super::IntegerId,
    /// `NumericRange index_range`.
    pub index_range: super::NumericRange,
}

/// Spec Tab 8.5 — `NodeTypeDescription` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeTypeDescription {
    /// `ExpandedNodeId type_definition_node`.
    pub type_definition_node: crate::node_id::ExpandedNodeId,
    /// `boolean include_subtypes`.
    pub include_subtypes: bool,
    /// `sequence<QueryDataDescription> data_to_return`.
    pub data_to_return: Vec<QueryDataDescription>,
}

/// Method-name constants (Spec §8.3.3.2 IDL).
pub struct QueryServiceMethods;

impl QueryServiceMethods {
    /// Method `query_first`.
    pub const QUERY_FIRST: &'static str = "query_first";
    /// Method `query_next`.
    pub const QUERY_NEXT: &'static str = "query_next";
}

const QUERY_FIRST_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "query_data_sets",
        type_spec: "sequence<QueryDataSet>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "continuation_point",
        type_spec: "ContinuationPoint",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "parsing_results",
        type_spec: "sequence<ParsingResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "filter_result",
        type_spec: "ContentFilterResult",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "view",
        type_spec: "ViewDescription",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "node_types",
        type_spec: "sequence<NodeTypeDescription>",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "filter",
        type_spec: "ContentFilter",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "max_datasets_to_return",
        type_spec: "Counter",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "max_references_to_return",
        type_spec: "Counter",
        direction: ParamDir::In,
    },
];

const QUERY_NEXT_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "query_data_sets",
        type_spec: "sequence<QueryDataSet>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "revised_continuation_point",
        type_spec: "ContinuationPoint",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "release_continuation_point",
        type_spec: "boolean",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "continuation_point",
        type_spec: "ContinuationPoint",
        direction: ParamDir::In,
    },
];

const METHODS: &[ServiceMethod] = &[
    ServiceMethod {
        name: QueryServiceMethods::QUERY_FIRST,
        return_type: "ResponseHeader",
        params: QUERY_FIRST_PARAMS,
    },
    ServiceMethod {
        name: QueryServiceMethods::QUERY_NEXT,
        return_type: "ResponseHeader",
        params: QUERY_NEXT_PARAMS,
    },
];

/// Query service set description (Spec §8.3.3.2).
pub const SERVICE_SET: ServiceSet = ServiceSet {
    module_path: IDL_MODULE_QUERY,
    interface_name: "Query",
    methods: METHODS,
};

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_set_has_two_methods_per_spec() {
        assert_eq!(SERVICE_SET.methods.len(), 2);
        let names: alloc::vec::Vec<&str> = SERVICE_SET.methods.iter().map(|m| m.name).collect();
        assert_eq!(names, alloc::vec!["query_first", "query_next"]);
    }

    #[test]
    fn query_first_carries_content_filter_inputs() {
        // Spec §8.3.3.2 IDL — query_first hat `in ContentFilter filter`
        // + `in sequence<NodeTypeDescription> node_types`.
        let qf = SERVICE_SET
            .methods
            .iter()
            .find(|m| m.name == "query_first")
            .expect("query_first");
        assert!(qf.params.iter().any(|p| p.type_spec == "ContentFilter"));
        assert!(
            qf.params
                .iter()
                .any(|p| p.type_spec == "sequence<NodeTypeDescription>")
        );
    }

    #[test]
    fn query_next_uses_release_continuation_flag() {
        // Spec §8.3.3.2 IDL — query_next hat `in boolean release_continuation_point`.
        let qn = SERVICE_SET
            .methods
            .iter()
            .find(|m| m.name == "query_next")
            .expect("query_next");
        let flag = qn
            .params
            .iter()
            .find(|p| p.name == "release_continuation_point")
            .expect("flag");
        assert_eq!(flag.type_spec, "boolean");
        assert_eq!(flag.direction, ParamDir::In);
    }
}
