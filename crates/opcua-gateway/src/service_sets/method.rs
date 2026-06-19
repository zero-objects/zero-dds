// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Method Service Set — Spec §8.3.5.
//!
//! Modul: `OMG::DDSOPCUA::OPCUA2DDS::METHOD`. Interface: `Method`.
//!
//! Method: `call`.

use alloc::vec::Vec;

use crate::data_value::Variant;
use crate::node_id::NodeId;
use crate::types::StatusCode;

use super::{DiagnosticInfo, IDL_MODULE_METHOD, ParamDir, ServiceMethod, ServiceParam, ServiceSet};

/// Spec Tab 8.7 — `CallMethodRequest` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct CallMethodRequest {
    /// `NodeId object_id` — the object the method is attached to.
    pub object_id: NodeId,
    /// `NodeId method_id` — method on the object.
    pub method_id: NodeId,
    /// `sequence<BaseDataType> input_arguments` (= Variant-Sequenz).
    pub input_arguments: Vec<Variant>,
}

/// Spec Tab 8.7 — `CallMethodResult` (`@nested`).
#[derive(Debug, Clone, PartialEq)]
pub struct CallMethodResult {
    /// `StatusCode status_code`.
    pub status_code: StatusCode,
    /// `sequence<StatusCode> input_arguments_results`.
    pub input_arguments_results: Vec<StatusCode>,
    /// `sequence<DiagnosticInfo> input_arguments_diagnostic_infos`.
    pub input_arguments_diagnostic_infos: Vec<DiagnosticInfo>,
    /// `sequence<BaseDataType> output_arguments`.
    pub output_arguments: Vec<Variant>,
}

/// Method-name constants (Spec §8.3.5.2 IDL).
pub struct MethodServiceMethods;

impl MethodServiceMethods {
    /// Method `call`.
    pub const CALL: &'static str = "call";
}

const CALL_PARAMS: &[ServiceParam] = &[
    ServiceParam {
        name: "server_id",
        type_spec: "string",
        direction: ParamDir::In,
    },
    ServiceParam {
        name: "results",
        type_spec: "sequence<CallMethodResult>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "diagnostic_infos",
        type_spec: "sequence<DiagnosticInfo>",
        direction: ParamDir::Out,
    },
    ServiceParam {
        name: "methods_to_call",
        type_spec: "sequence<CallMethodRequest>",
        direction: ParamDir::In,
    },
];

const METHODS: &[ServiceMethod] = &[ServiceMethod {
    name: MethodServiceMethods::CALL,
    return_type: "ResponseHeader",
    params: CALL_PARAMS,
}];

/// Method service set description (Spec §8.3.5.2).
pub const SERVICE_SET: ServiceSet = ServiceSet {
    module_path: IDL_MODULE_METHOD,
    interface_name: "Method",
    methods: METHODS,
};

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_set_has_one_method_per_spec() {
        // Spec §8.3.5.2 IDL — Method interface hat nur `call`.
        assert_eq!(SERVICE_SET.methods.len(), 1);
        assert_eq!(SERVICE_SET.methods[0].name, "call");
    }

    #[test]
    fn call_carries_methods_to_call_input() {
        let call = &SERVICE_SET.methods[0];
        let p = call
            .params
            .iter()
            .find(|p| p.name == "methods_to_call")
            .expect("methods_to_call");
        assert_eq!(p.type_spec, "sequence<CallMethodRequest>");
        assert_eq!(p.direction, ParamDir::In);
    }

    #[test]
    fn call_method_request_holds_node_ids_and_variants() {
        let req = CallMethodRequest {
            object_id: NodeId::numeric(0, 1),
            method_id: NodeId::numeric(0, 2),
            input_arguments: alloc::vec::Vec::new(),
        };
        assert_eq!(req.input_arguments.len(), 0);
    }
}
