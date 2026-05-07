// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! View Service Set — Spec §8.3.2.
//!
//! Modul: `OMG::DDSOPCUA::OPCUA2DDS::VIEW`. Interface: `View`.
//!
//! Methoden: `browse`, `browse_next`, `translate_browse_paths_to_node_ids`,
//! `register_nodes`, `unregister_nodes`.

use alloc::vec::Vec;

use crate::node_id::{ExpandedNodeId, NodeId};

use super::{IDL_MODULE_VIEW, ParamDir, ServiceMethod, ServiceParam, ServiceSet};

/// Spec Tab 8.4 — `BrowsePath` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowsePath {
    /// `NodeId starting_node`.
    pub starting_node: NodeId,
    /// `RelativePath relative_path`.
    pub relative_path: super::RelativePath,
}

/// Spec Tab 8.4 — `BrowsePathTarget` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowsePathTarget {
    /// `ExpandedNodeId target_id`.
    pub target_id: ExpandedNodeId,
    /// `Index remaining_path_index`.
    pub remaining_path_index: super::Index,
}

/// Spec Tab 8.4 — `BrowsePathResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowsePathResult {
    /// `StatusCode status_code`.
    pub status_code: crate::types::StatusCode,
    /// `sequence<BrowsePathTarget> targets`.
    pub targets: Vec<BrowsePathTarget>,
}

/// Spec Tab 8.4 — `BrowseDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BrowseDirection {
    /// `@value(0) FORWARD_BROWSE_DIRECTION`.
    Forward = 0,
    /// `@value(1) REVERSE_BROWSE_DIRECTION`.
    Reverse = 1,
    /// `@value(3) BOTH_BROWSE_DIRECTION`. Wert `3` aus Spec (kein
    /// `@value(2)` — Spec laesst die Luecke explizit).
    Both = 3,
}

/// Spec Tab 8.4 — `BrowseDescription` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseDescription {
    /// `NodeId node_id`.
    pub node_id: NodeId,
    /// `BrowseDirection browse_direction`.
    pub browse_direction: BrowseDirection,
    /// `NodeId reference_type_id`.
    pub reference_type_id: NodeId,
    /// `boolean include_subtypes`.
    pub include_subtypes: bool,
    /// `uint32 node_class_mask` — Bitfield aus `NodeClass.value()`.
    pub node_class_mask: u32,
    /// `uint32 result_mask`.
    pub result_mask: u32,
}

/// Methoden-Namen-Konstanten (Spec §8.3.2.2 IDL).
pub struct ViewServiceMethods;

impl ViewServiceMethods {
    /// Methode `browse`.
    pub const BROWSE: &'static str = "browse";
    /// Methode `browse_next`.
    pub const BROWSE_NEXT: &'static str = "browse_next";
    /// Methode `translate_browse_paths_to_node_ids`.
    pub const TRANSLATE_BROWSE_PATHS_TO_NODE_IDS: &'static str =
        "translate_browse_paths_to_node_ids";
    /// Methode `register_nodes`.
    pub const REGISTER_NODES: &'static str = "register_nodes";
    /// Methode `unregister_nodes`.
    pub const UNREGISTER_NODES: &'static str = "unregister_nodes";
}

// Method-Signaturen — Spec §8.3.2.2 IDL @DDSService interface View.
const BROWSE_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<BrowseResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "view_description",
        type_spec: "ViewDescription",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "requested_max_references_per_node",
        type_spec: "Counter",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "nodes_to_browse",
        type_spec: "sequence<BrowseDescription>",
        direction: ParamDir::In,
    },
];

const BROWSE_NEXT_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<BrowseResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "release_continuation_points",
        type_spec: "boolean",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "continuation_points",
        type_spec: "sequence<ContinuationPoint>",
        direction: ParamDir::In,
    },
];

const TRANSLATE_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<BrowsePathResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "browse_paths",
        type_spec: "sequence<BrowsePath>",
        direction: ParamDir::In,
    },
];

const REGISTER_NODES_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "registered_node_ids",
        type_spec: "sequence<NodeId>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "nodes_to_register",
        type_spec: "sequence<NodeId>",
        direction: ParamDir::In,
    },
];

const UNREGISTER_NODES_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "nodes_to_unregister",
        type_spec: "sequence<NodeId>",
        direction: ParamDir::In,
    },
];

const METHODS: &[ServiceMethod] = &[
    ServiceMethod {
        name: ViewServiceMethods::BROWSE,
        return_type: "ResponseHeader",
        params: BROWSE_PARAMS,
    },
    ServiceMethod {
        name: ViewServiceMethods::BROWSE_NEXT,
        return_type: "ResponseHeader",
        params: BROWSE_NEXT_PARAMS,
    },
    ServiceMethod {
        name: ViewServiceMethods::TRANSLATE_BROWSE_PATHS_TO_NODE_IDS,
        return_type: "ResponseHeader",
        params: TRANSLATE_PARAMS,
    },
    ServiceMethod {
        name: ViewServiceMethods::REGISTER_NODES,
        return_type: "ResponseHeader",
        params: REGISTER_NODES_PARAMS,
    },
    ServiceMethod {
        name: ViewServiceMethods::UNREGISTER_NODES,
        return_type: "ResponseHeader",
        params: UNREGISTER_NODES_PARAMS,
    },
];

/// View-Service-Set-Beschreibung (Spec §8.3.2.2).
pub const SERVICE_SET: ServiceSet = ServiceSet {
    module_path: IDL_MODULE_VIEW,
    interface_name: "View",
    methods: METHODS,
};

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_set_has_five_methods_per_spec() {
        // Spec §8.3.2.2 listet exakt 5 Methoden.
        assert_eq!(SERVICE_SET.methods.len(), 5);
        let names: alloc::vec::Vec<&str> = SERVICE_SET.methods.iter().map(|m| m.name).collect();
        assert_eq!(
            names,
            alloc::vec![
                "browse",
                "browse_next",
                "translate_browse_paths_to_node_ids",
                "register_nodes",
                "unregister_nodes",
            ]
        );
    }

    #[test]
    fn browse_method_first_param_is_server_id() {
        // Spec §8.3 einleitend: "The first parameter of each method is
        // always the input parameter server_id".
        for m in SERVICE_SET.methods {
            assert_eq!(
                m.params[0].name, "server_id",
                "method {} first param",
                m.name
            );
            assert_eq!(m.params[0].type_spec, "string");
            assert_eq!(m.params[0].direction, ParamDir::In);
        }
    }

    #[test]
    fn browse_method_carries_view_description() {
        // Spec §8.3.2.2 IDL — browse hat `in ViewDescription view_description`.
        let browse = SERVICE_SET
            .methods
            .iter()
            .find(|m| m.name == "browse")
            .expect("browse method");
        assert!(
            browse
                .params
                .iter()
                .any(|p| p.type_spec == "ViewDescription")
        );
    }

    #[test]
    fn browse_direction_uses_spec_values_with_gap() {
        // Spec laesst den Wert 2 explizit aus.
        assert_eq!(BrowseDirection::Forward as u8, 0);
        assert_eq!(BrowseDirection::Reverse as u8, 1);
        assert_eq!(BrowseDirection::Both as u8, 3);
    }

    #[test]
    fn all_methods_return_response_header() {
        for m in SERVICE_SET.methods {
            assert_eq!(m.return_type, "ResponseHeader", "method {}", m.name);
        }
    }
}
