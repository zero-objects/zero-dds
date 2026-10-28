// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The IDL-interface -> WSDL bridge plan §3 asks for: turns a
//! config-named IDL `interface` into the `Operation` list
//! `zerodds_soap::wsdl::WsdlGenerator` needs, reusing
//! `zerodds_lang_rules::adapter::interface` (`services`,
//! `all_operations_flattened`, `in_struct_name`,
//! `out_struct_name`) rather than re-deriving DDS-RPC 1.0
//! §7.4.3's operation model a third time.
//!
//! # Scope
//!
//! This builds the **WSDL document** — every operation's name and its
//! request/reply type names, spec-conformant and discoverable at
//! `GET /wsdl/<service>` — for any `interface` in a config-named IDL
//! file, whether or not the file also names a topic type. Real request/
//! reply dispatch of a *business* interface operation (writing an `In`
//! struct DDS-RPC 1.0 §7.5.1.1.4 synthesizes to a request topic, taking
//! the matching `Out`/`Result` from a reply topic) needs the struct
//! synthesis `crates/rpc/src/codegen.rs` owns — a separate, already
//! decided C2 item (`docs/doc-map/befunde.md:31` item 4: "idlc generates
//! a client and a service on zerodds-rpc for an IDL interface"). Wiring
//! that in here would duplicate it. `soap_service::dispatch_interface_operation`
//! therefore returns a documented SOAP Fault for a business-interface
//! operation invocation today; the WSDL it is described by is real and
//! already useful for client codegen. See the crate README.
//!
//! DDS-WEB's own object-model operations (Application/Participant/
//! Topic/.../write/read — the same ones REST exposes) are unaffected:
//! `soap_service` dispatches those directly against `Gateway`, and
//! [`builtin_operations`] below lists their fixed `Operation` set for
//! the "built-in" WSDL binding every gateway always serves.

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::interface::{
    InterfaceError, Service, all_operations_flattened, in_struct_name, out_struct_name, services,
};
use zerodds_soap::wsdl::Operation;

/// Every operation of every `interface` in `ast`, as WSDL `Operation`s.
/// One `Operation` per §7.4.3 declared/implied operation, including a
/// base interface's inherited ones and the getter/setter DDS-RPC 1.0
/// §7.5.1.1.3 implies for each attribute — [`all_operations_flattened`]
/// is what walks that, not `Service::operations` (this interface's own
/// declared operations alone).
///
/// # Errors
/// [`InterfaceError`] — a malformed interface (unresolved base,
/// incomplete forward declaration, or an inheritance cycle) in the
/// model.
pub fn operations_of(ast: &Ast) -> Result<Vec<(Service, Vec<Operation>)>, InterfaceError> {
    let mut out = Vec::new();
    for service in services(ast)? {
        let flattened = all_operations_flattened(ast, service.id)?;
        let mut ops = Vec::with_capacity(flattened.len());
        for op in &flattened {
            ops.push(Operation {
                name: op.name.clone(),
                input_type: in_struct_name(&service.name, &op.name),
                output_type: out_struct_name(&service.name, &op.name),
            });
        }
        out.push((service, ops));
    }
    Ok(out)
}

/// The DDS-WEB object-model operations every gateway serves over SOAP,
/// independent of any config-named `interface` — the same set REST
/// exposes under `/dds/rest1`. Names follow the PIM operation names of
/// Spec §7.4 Tab 5 (`Root::create_application`, `DataWriter::write`,
/// ...), and every one round-trips through `application/dds-web+xml`
/// request and reply bodies, matching the REST representation.
#[must_use]
pub fn builtin_operations() -> Vec<Operation> {
    let pairs: &[&str] = &[
        "CreateApplication",
        "DeleteApplication",
        "GetApplications",
        "CreateParticipant",
        "UpdateParticipant",
        "DeleteParticipant",
        "GetParticipants",
        "CreateTopic",
        "UpdateTopic",
        "DeleteTopic",
        "GetTopics",
        "CreatePublisher",
        "UpdatePublisher",
        "DeletePublisher",
        "GetPublishers",
        "CreateSubscriber",
        "UpdateSubscriber",
        "DeleteSubscriber",
        "GetSubscribers",
        "CreateDataWriter",
        "UpdateDataWriter",
        "DeleteDataWriter",
        "GetDataWriters",
        "CreateDataReader",
        "UpdateDataReader",
        "DeleteDataReader",
        "GetDataReaders",
        "WriteSample",
        "ReadSample",
        "CreateWaitset",
        "UpdateWaitset",
        "GetWaitset",
        "DeleteWaitset",
        "GetWaitsets",
    ];
    pairs
        .iter()
        .map(|name| Operation {
            name: (*name).to_string(),
            input_type: format!("{name}Request"),
            output_type: format!("{name}Response"),
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn interface_operations_become_wsdl_operations() {
        let ast = zerodds_wire_probe::encode::model_from_idl(
            "interface Calculator { long add(in long a, in long b); };",
            "t.idl",
        )
        .expect("parses");
        let all = operations_of(&ast).expect("ok");
        assert_eq!(all.len(), 1);
        let (service, ops) = &all[0];
        assert_eq!(service.name, "Calculator");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "add");
        assert!(ops[0].input_type.contains("Calculator"));
        assert!(ops[0].output_type.contains("Calculator"));
    }

    #[test]
    fn builtin_operations_cover_the_object_model() {
        let ops = builtin_operations();
        assert!(ops.iter().any(|o| o.name == "WriteSample"));
        assert!(ops.iter().any(|o| o.name == "ReadSample"));
        assert!(ops.iter().any(|o| o.name == "CreateApplication"));
        assert!(ops.iter().any(|o| o.name == "UpdateTopic"));
        assert_eq!(ops.len(), 34);
    }

    #[test]
    fn interfaces_with_attributes_get_implied_operations() {
        let ast = zerodds_wire_probe::encode::model_from_idl(
            "interface Counter { attribute long value; };",
            "t.idl",
        )
        .expect("parses");
        let all = operations_of(&ast).expect("ok");
        let (_, ops) = &all[0];
        // A read-write attribute implies a getter + a setter (DDS-RPC
        // 1.0 §7.5.1.1.3).
        assert_eq!(ops.len(), 2);
    }
}
