// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The intermediate IDL the IDL4 language mappings map components,
//! homes, ports, connectors and event types through (IDL4 to Java 1.0,
//! IDL4 to C# 1.0 and IDL4 to C++ 1.0, §7.9 to §7.12).

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;

/// The model with its components, homes, connectors, extended ports and
/// event types replaced by their intermediate IDL: §7.9, §7.10 and §7.12
/// of IDL4 to Java, C# and C++ 1.0 ("they shall be mapped to intermediate
/// IDL ... and mapped to `<language>` accordingly"). The intermediate IDL
/// of a component, a home and an event type is CCM 4.0 §6's equivalent IDL; that of an extended port the
/// basic ports of its port type (`zerodds_ccm::expand_extended_ports`).
/// The names CCM 4.0 introduces (`Components::CCMObject` and the rest)
/// resolve against the `Components` module the IDL includes. A copy:
/// the caller's model is not rewritten.
pub fn intermediate_idl(ast: &Ast) -> Result<Ast, String> {
    let bytes = ast
        .to_snapshot()
        .map_err(|error| format!("a copy of the model ({error})"))?;
    let mut copy =
        Ast::from_snapshot(&bytes).map_err(|error| format!("a copy of the model ({error})"))?;
    let events: Vec<Id<TypeNode>> = copy
        .types()
        .filter(|(_, node)| {
            node.facets
                .corba
                .as_ref()
                .is_some_and(|corba| corba.is_event_type)
        })
        .map(|(id, _)| id)
        .collect();
    let transform = |error: zerodds_ccm::TransformError| format!("the equivalent IDL ({error})");
    for event in events {
        zerodds_ccm::transform_event_type(&mut copy, event).map_err(transform)?;
    }
    let interfaces: Vec<(Id<zerodds_ast::InterfaceNode>, zerodds_ast::InterfaceKind)> = copy
        .interfaces()
        .filter(|(_, node)| node.completeness != zerodds_ast::Completeness::Forward)
        .map(|(id, node)| (id, node.kind))
        .collect();
    for (id, kind) in &interfaces {
        match kind {
            zerodds_ast::InterfaceKind::Component => {
                zerodds_ccm::transform_component(&mut copy, *id).map_err(transform)?;
            }
            zerodds_ast::InterfaceKind::Connector => {
                zerodds_ccm::transform_connector(&mut copy, *id).map_err(transform)?;
            }
            _ => {}
        }
    }
    for (id, kind) in &interfaces {
        if *kind == zerodds_ast::InterfaceKind::Home {
            zerodds_ccm::transform_home(&mut copy, *id).map_err(transform)?;
        }
    }
    copy.resolve_all_by_name()
        .map_err(|error| format!("the names of the equivalent IDL ({error})"))?;
    Ok(copy)
}
