// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced Service Mapping of DDS-RPC 1.0 §7.5.1.2 as Java.
//!
//! §7.11.2 reads the Java column through the C++ one, and
//! `emit::interface_cpp::enhanced` documents the shape: `${I}_Request` and
//! `${I}_Reply` are `@Choice @Autoid` structures whose members are all
//! optional and of which exactly one is present (§7.5.1.2.1.1). A Java
//! reference is nullable, so a member is its holder class and an absent
//! member is `null`. The annotations are in the documentation.
//!
//! Their octets and the function-call classes over them are written by
//! `enhanced_calls`; see `emit::interface_cpp::enhanced`.

use super::*;
use crate::adapter::interface::{EnhancedMemberType, EnhancedOperation, EnhancedStruct};

/// A holder declared in `module`, spelled from anywhere: through the
/// outer class.
pub(super) fn holder_type(
    ast: &Ast,
    here: zerodds_ast_core::Id<zerodds_ast::ModuleNode>,
    module: zerodds_ast_core::Id<zerodds_ast::ModuleNode>,
    name: &str,
) -> Result<java::Type, BindingError> {
    if here == module {
        return Ok(local(name));
    }
    let mut segments: Vec<String> = vec![OUTER.to_string()];
    segments.extend(
        interface::module_chain(ast, module)?
            .into_iter()
            .map(|(_, module)| java::escape_keyword(&module)),
    );
    segments.push(name.to_string());
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(java::Type::named(Path::new(&borrowed)))
}

fn annotations_doc(structure: &EnhancedStruct, what: &str) -> Doc {
    let mut marks = Vec::new();
    if structure.choice {
        marks.push("@Choice");
    }
    if structure.autoid {
        marks.push("@Autoid");
    }
    if structure.empty {
        marks.push("@Empty");
    }
    if structure.mutable {
        marks.push("@Extensibility(MUTABLE_EXTENSIBILITY)");
    }
    Doc::lines(&[what, "", &marks.join(" ")])
}

/// One member of a choice structure.
fn choice_field(
    ast: &Ast,
    here: zerodds_ast_core::Id<zerodds_ast::ModuleNode>,
    owner: Option<&EnhancedOperation>,
    member: &crate::adapter::interface::EnhancedMember,
) -> Result<java::Member, BindingError> {
    let type_ = match &member.type_ {
        EnhancedMemberType::Struct(name) => holder_type(
            ast,
            here,
            owner.map_or(here, |operation| operation.module),
            name,
        )?,
        EnhancedMemberType::Exception(raise) => exception_type(ast, raise)?,
        EnhancedMemberType::RemoteException => qualified(RPC, "RemoteExceptionCode"),
        EnhancedMemberType::Dummy | EnhancedMemberType::Idl(_) => {
            return Err(BindingError::Codec(format!(
                "member `{}` is not a member of a choice structure",
                member.name
            )));
        }
    };
    Ok(field(type_, &java::escape_keyword(&member.name)))
}

/// The members one interface contributes under the Enhanced Service
/// Mapping.
pub(super) fn members(ast: &Ast, service: &Service) -> Result<Vec<java::Member>, BindingError> {
    let module = service.module;
    let name = java::escape_keyword(&service.name);
    let model = interface::enhanced_service(ast, service, service.mapping.empty_style())?;
    let mut out = Vec::new();

    for operation in model
        .operations
        .iter()
        .filter(|operation| operation.module == module && operation.interface == service.name)
    {
        let op = &operation.operation;
        out.push(holder(
            &operation.in_struct.name,
            // An `@Empty` structure has no member at all (§7.5.1.2.1.3).
            if operation.in_struct.members.is_empty() {
                Vec::new()
            } else {
                request_fields(ast, service, op)?
            },
            annotations_doc(
                &operation.in_struct,
                "The `in` and `inout` parameters of one call. DDS-RPC 1.0 §7.5.1.2.2.",
            ),
        ));
        out.push(holder(
            &operation.out_struct.name,
            if operation.out_struct.members.is_empty() {
                Vec::new()
            } else {
                reply_fields(ast, service, op)?
            },
            annotations_doc(
                &operation.out_struct,
                "The return value and the `out` and `inout` parameters of one call. DDS-RPC \
                 1.0 §7.5.1.2.3 rule 1.",
            ),
        ));
        let mut fields = Vec::new();
        for member in &operation.result_struct.members {
            fields.push(choice_field(ast, module, None, member)?);
        }
        out.push(holder(
            &operation.result_struct.name,
            fields,
            annotations_doc(
                &operation.result_struct,
                "The outcome of one operation: the result or an exception it raises. DDS-RPC \
                 1.0 §7.5.1.2.3.",
            ),
        ));
    }
    for (structure, doc) in [
        (
            &model.request,
            "The request topic type: one member per operation, the inherited ones included. \
             DDS-RPC 1.0 §7.5.1.2.4.",
        ),
        (
            &model.reply,
            "The reply topic type: one member per operation, and `remoteEx`. DDS-RPC 1.0 \
             §7.5.1.2.5.",
        ),
    ] {
        let mut fields = Vec::new();
        for member in &structure.members {
            let owner = model
                .operations
                .iter()
                .find(|operation| operation.operation.name == member.name);
            fields.push(choice_field(ast, module, owner, member)?);
        }
        out.push(holder(
            &structure.name,
            fields,
            annotations_doc(structure, doc),
        ));
    }
    let scope: Vec<String> = interface::module_chain(ast, module)?
        .into_iter()
        .map(|(_, module)| module)
        .collect();
    // The XTypes descriptions of the two topic types; a type with no
    // TypeObject leaves the topics matched by type name.
    let descriptions =
        crate::emit::interface_rust::type_information::service_type_objects(ast, service, &model)
            .ok()
            .and_then(|objects| {
                crate::emit::interface_rust::type_information::java_descriptions(&objects)
            });
    out.push(support_class_with(
        service,
        &name,
        &scope,
        true,
        descriptions.as_ref(),
    ));
    out.push(sync_interface(ast, service, &name)?);
    out.push(async_interface(ast, service, &name)?);
    if service.is_remote() {
        out.push(endpoint(
            &format!("{name}Requester"),
            "Requester",
            &name,
            "The service's `Requester`, over its two topic types. DDS-RPC 1.0 §7.11.2.2.1.",
        ));
        out.push(endpoint(
            &format!("{name}Replier"),
            "Replier",
            &name,
            "The service's `Replier`, over its two topic types. DDS-RPC 1.0 §7.11.2.2.1.",
        ));
        out.extend(super::enhanced_calls::members(ast, service, &name, &model)?);
        out.push(client_interface(&name));
        out.push(service_interface(&name));
    }
    Ok(out)
}
