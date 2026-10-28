// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced Service Mapping of DDS-RPC 1.0 §7.5.1.2 as C++.
//!
//! `${I}_Request` and `${I}_Reply` are `@Choice @Autoid` structures
//! (§7.5.1.2.4, §7.5.1.2.5), and "the structure shall be interpreted as
//! if the structure had the `@Extensibility(MUTABLE_EXTENSIBILITY)`
//! annotation and all the members of the structure had the `@Optional`
//! annotation. Furthermore, exactly one member shall be present at any
//! given time" (§7.5.1.2.1.1). DDS-Cxx-PSM maps an `@optional` member to
//! `std::optional<T>`, as `adapter::cpp` does for an ordinary type, so
//! each member of a choice structure is one.
//!
//! The declarations carry the annotations in their documentation: C++
//! has no annotation to attach them to. Their octets are written by
//! `enhanced_codec` — PL_CDR2, the forms of `emit::interface_rust::enhanced`
//! — together with the `dds_type_traits` the Enhanced `Requester` and
//! `Replier` ask for.
//!
//! The function-call classes (`I_Client`, `I_Service`) are the Basic
//! mapping's, written over these structures: the client sets the member of
//! the operation in the `Request` and reads the `Result` of that member from
//! the `Reply`; the service dispatches on the member that is present
//! (`calls`, `Shape::Enhanced`).

use super::*;
use crate::adapter::interface::{EnhancedMemberType, EnhancedOperation, EnhancedStruct};
use zerodds_ast::ExceptionNode;

/// `std::optional<T>`.
fn optional(inner: cpp::Type) -> cpp::Type {
    cpp::Type::template(Path::new(&["std", "optional"]), vec![inner])
}

/// A declaration of `module`, spelled from the namespace of `here`.
pub(super) fn path_in(
    ast: &Ast,
    here: Id<ModuleNode>,
    module: Id<ModuleNode>,
    name: &str,
) -> Result<Path, BindingError> {
    // Under `--cpp-mapping psm` what this names — a topic structure or an
    // exception — is a native type, in `zd_native`, and the classes that name
    // it stand in the namespaces of the IDL.
    if is_psm() {
        let mut spelled = format!("::{}", crate::adapter::cpp::NATIVE_NAMESPACE);
        for (_, segment) in interface::module_chain(ast, module)? {
            spelled.push_str("::");
            spelled.push_str(&cxx_escape(&segment));
        }
        spelled.push_str("::");
        spelled.push_str(name);
        return Ok(Path::single(&spelled));
    }
    path_in_idl(ast, here, module, name)
}

/// As [`path_in`], for a class of the IDL's own namespaces — an interface, which is a
/// class of the application under either mapping and never a native type.
pub(super) fn path_in_idl(
    ast: &Ast,
    here: Id<ModuleNode>,
    module: Id<ModuleNode>,
    name: &str,
) -> Result<Path, BindingError> {
    if here == module {
        return Ok(Path::single(name));
    }
    let mut segments: Vec<String> = interface::module_chain(ast, module)?
        .into_iter()
        .map(|(_, module)| cxx_escape(&module))
        .collect();
    segments.push(name.to_string());
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(Path::new(&borrowed))
}

/// A structure declared in `module`, spelled from anywhere.
pub(super) fn type_in(
    ast: &Ast,
    here: Id<ModuleNode>,
    module: Id<ModuleNode>,
    name: &str,
) -> Result<cpp::Type, BindingError> {
    Ok(cpp::Type::named(path_in(ast, here, module, name)?))
}

/// A documentation block that names the annotations of a structure.
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

/// The member of a choice structure that holds `type_`.
fn choice_member(
    ast: &Ast,
    here: Id<ModuleNode>,
    owner: Option<&EnhancedOperation>,
    member: &crate::adapter::interface::EnhancedMember,
    exception_type: &impl Fn(&interface::Raise) -> Result<cpp::Type, BindingError>,
) -> Result<cpp::ClassMember, BindingError> {
    let inner = match &member.type_ {
        EnhancedMemberType::Struct(name) => type_in(
            ast,
            here,
            owner.map_or(here, |operation| operation.module),
            name,
        )?,
        EnhancedMemberType::Exception(raise) => exception_type(raise)?,
        EnhancedMemberType::RemoteException => qualified(RPC, "RemoteExceptionCode_t"),
        EnhancedMemberType::Dummy | EnhancedMemberType::Idl(_) => {
            return Err(BindingError::Codec(format!(
                "member `{}` is not a member of a choice structure",
                member.name
            )));
        }
    };
    Ok(field(optional(inner), &cxx_escape(&member.name)))
}

/// What `declare` writes for one interface under the Enhanced Service
/// Mapping.
pub(super) fn declare(
    ast: &Ast,
    service: &Service,
    out: &mut super::Sink,
    declared_exceptions: &mut Vec<Id<ExceptionNode>>,
) -> Result<(), BindingError> {
    let module = service.module;
    let name = cxx_escape(&service.name);
    let model = interface::enhanced_service(ast, service, service.mapping.empty_style())?;

    let exception_type = |raise: &interface::Raise| -> Result<cpp::Type, BindingError> {
        let node = ast
            .exception(raise.id)
            .map_err(|_| interface::InterfaceError::NoNode {
                kind: "exception",
                at: raise.name.clone(),
            })?;
        type_in(ast, module, node.module, &cxx_escape(&raise.name))
    };

    for operation in model
        .operations
        .iter()
        .filter(|operation| operation.module == module && operation.interface == service.name)
    {
        let op = &operation.operation;
        out.push_topic((
            module,
            cpp::Decl::Class(struct_of(
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
            )),
        ));
        let out_struct = (
            module,
            cpp::Decl::Class(struct_of(
                &operation.out_struct.name,
                if operation.out_struct.members.is_empty() {
                    Vec::new()
                } else {
                    reply_fields(ast, service, op)?
                },
                annotations_doc(
                    &operation.out_struct,
                    "The return value and the `out` and `inout` parameters of one call. \
                     DDS-RPC 1.0 §7.5.1.2.3 rule 1.",
                ),
            )),
        );
        if is_psm() && op.has_outputs() {
            // The `future` of an asynchronous call holds this structure
            // (§7.11.1.1.2 rule 7.i), and the application sees its members as
            // Table 7.1 spells them: the topic structure is native, in
            // `zd_native`, and the application's own `Out` follows it in the
            // IDL's namespace.
            out.push_topic_unseen(out_struct);
            out.push((
                module,
                cpp::Decl::Class(struct_of(
                    &operation.out_struct.name,
                    reply_fields_for_application(ast, service, op)?,
                    Doc::line("The return value and the `out` and `inout` parameters of one call, as the application sees them. DDS-RPC 1.0 §7.5.1.2.3 rule 1 and §7.11.1.1.2 rule 7.i."),
                )),
            ));
        } else {
            out.push_topic(out_struct);
        }
        let mut members = Vec::new();
        for member in &operation.result_struct.members {
            members.push(choice_member(ast, module, None, member, &exception_type)?);
        }
        out.push_topic((
            module,
            cpp::Decl::Class(struct_of(
                &operation.result_struct.name,
                members,
                annotations_doc(
                    &operation.result_struct,
                    "The outcome of one operation: the result or an exception it raises. \
                     DDS-RPC 1.0 §7.5.1.2.3.",
                ),
            )),
        ));
    }

    for (structure, doc) in [
        (
            &model.request,
            "The request topic type: one member per operation, the inherited ones \
             included. DDS-RPC 1.0 §7.5.1.2.4.",
        ),
        (
            &model.reply,
            "The reply topic type: one member per operation, and `remoteEx`. DDS-RPC 1.0 \
             §7.5.1.2.5.",
        ),
    ] {
        let mut members = Vec::new();
        for member in &structure.members {
            let owner = model
                .operations
                .iter()
                .find(|operation| operation.operation.name == member.name);
            members.push(choice_member(ast, module, owner, member, &exception_type)?);
        }
        out.push_topic((
            module,
            cpp::Decl::Class(struct_of(
                &structure.name,
                members,
                annotations_doc(structure, doc),
            )),
        ));
    }

    // The octets of the topic types: what `dds::rpc::Requester` and
    // `dds::rpc::Replier` ask `dds_type_traits` for.
    if service.is_remote() {
        let root = ast.root_module();
        for decl in enhanced_codec::codecs(ast, service, &model, declared_exceptions)? {
            out.push((root, decl));
        }
        out.push((root, enhanced_codec::traits(ast, service, &model)?));
    }
    out.push((module, cpp::Decl::Class(support_class(service, &name))));
    out.push((
        module,
        cpp::Decl::Forward {
            is_struct: false,
            name: Ident::new(&format!("{name}Async")),
        },
    ));
    let operations = service.all_operations();
    out.push((
        module,
        cpp::Decl::Class(sync_interface(ast, service, &name, &operations)?),
    ));
    out.push((
        module,
        cpp::Decl::Class(async_interface(ast, service, &name, &operations)?),
    ));
    if service.is_remote() {
        let request = interface::request_type_name(&name);
        let reply = interface::reply_type_name(&name);
        out.push((module, requester_alias(&name, &request, &reply)));
        out.push((module, replier_alias(&name, &request, &reply)));
        // §7.5.1.2.6: the request and the reply hold the operations of the
        // bases as well, so the classes answer all of them.
        let flattened: Vec<Operation> = model
            .operations
            .iter()
            .map(|operation| operation.operation.clone())
            .collect();
        let shape = calls::EnhancedShape::new(ast, module, &model)?;
        out.push((
            module,
            cpp::Decl::Class(calls::client_class(
                ast,
                service,
                &name,
                &flattened,
                calls::Shape::Enhanced(&shape),
            )?),
        ));
        out.push((
            module,
            cpp::Decl::Class(calls::service_class(
                ast,
                service,
                &name,
                &flattened,
                calls::Shape::Enhanced(&shape),
            )?),
        ));
    }
    Ok(())
}
