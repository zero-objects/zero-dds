// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An IDL `interface`, as the Java language binding DDS-RPC 1.0 §7.11.2
//! specifies.
//!
//! §7.11.2 is short on purpose: "The following sub clauses describe the
//! entities that are either different or not described in the C++
//! language bindings", and §7.11.2.3 then says the function-call style
//! "shall define the same entities as that of C++". So this column is
//! the C++ one of `emit::interface_cpp` read through two substitutions —
//! `org.omg.dds.rpc.Future<T>` for `dds::rpc::future<T>` (§7.11.2.2.4)
//! and the DDS-Java-PSM type mapping for the DDS-Cxx-PSM one — and one
//! difference the substitution cannot absorb.
//!
//! ## The difference: `out` and `inout`
//!
//! C++ passes them by reference; §7.11.1.1.2 rule 6.e sends that to
//! DDS-Cxx-PSM §7.4.5, whose answer is `T&`. Java has no `T&`. The
//! \[IDL2Java\] mapping's answer is a `Holder` class per type, and
//! §7.11.2 does not take it — it sends the type mapping to
//! [DDS-Java-PSM], which has no holders.
//!
//! What it does name is `${interface}_${operation}_Out`: rule 7.i makes
//! that the payload of the `future` an *asynchronous* operation with
//! `out` or `inout` parameters returns. The same type is what the
//! synchronous operation returns here. So the multiple return travels in
//! a type the specification itself names for exactly this call shape,
//! rather than in `n` holder classes it does not name at all.
//!
//! ## Exceptions
//!
//! No `throws` clause, on any generated method. Rule 6.c: "The functions
//! shall not have any exception specification even if the IDL operation
//! has an exception specification. The implementation may still throw
//! the exceptions specified by the IDL operation." §7.11.2 overrides
//! nothing about it, and §7.11.2.1 — Java's own "Mapping of Exceptions"
//! — has no body in the published document, which under §7.11.2's
//! preamble means the C++ clause stands. The C++ hierarchy of Figure 7.6
//! puts every remote exception under `std::runtime_error` or
//! `std::logic_error`, both unchecked; the Java counterpart of an
//! unchecked exception needs no `throws`.
//!
//! The `raises` clause is still visible: it is written into the method's
//! documentation, so a reader of the generated file sees what the IDL
//! declared rather than nothing at all. The same line goes into the C++
//! output for the same reason.
//!
//! ## Where it lands
//!
//! One outer class, as `adapter::java`'s [`crate::adapter::java::OUTER`]
//! is one outer class and for the same reason: Java allows one public
//! top-level type per file. The data types are written from `ZdTypes`
//! down and the services from [`OUTER`] down, which is what
//! `java_member_type` already spells for every type a signature names.

mod calls;
mod enhanced;
mod enhanced_calls;

use crate::adapter::interface::{
    self, Attribute, BindingError, Operation, Parameter, RETURN_MEMBER, Service, ServiceMapping,
};
use crate::adapter::java::java_member_type;
use zerodds_ast::Ast;
use zerodds_lang_cst::common::{Doc, Ident, Path};
use zerodds_lang_cst::java;

/// The class every generated service nests inside.
///
/// `ZdTypes` holds the data types; this holds the services. Two outer
/// classes and not one, because they are two generated artefacts: a
/// model with no `interface` in it produces no service file at all, and
/// a `ZdTypes` that sometimes carried services and sometimes did not
/// would be a class whose shape depended on what else was in the IDL.
pub const OUTER: &str = "ZdServices";

/// The package DDS-RPC 1.0 §7.11.2.2.1 puts the request-reply binding
/// in.
const RPC: &str = "org.omg.dds.rpc";
/// The implementation classes of the RPC runtime (not in a package of the prefix `org.omg.dds`,
/// DDS-Java-PSM 1.0 §7.2.6).
const RPC_IMPL: &str = "org.zerodds.ddsrpc";

/// The Java declarations every interface of `ast` produces.
///
/// # Errors
/// [`BindingError`] — the interface did not reach the seam, or a type in
/// it has no Java spelling.
pub fn java_interface_items(ast: &Ast) -> Result<Vec<java::TypeDecl>, BindingError> {
    java_interface_items_with(ast, ServiceMapping::Basic)
}

/// [`java_interface_items`] for the given service mapping.
///
/// [`ServiceMapping::Enhanced`] writes the topic types of DDS-RPC 1.0
/// §7.5.1.2 — the `@Choice @Autoid` `Request` and `Reply` holders and the
/// `In`, `Out` and `Result` holders — and the interface types that do not
/// depend on the shape of the topic types; the Basic mapping's `Call`
/// union, its codec and its function-call classes are not written.
///
/// # Errors
/// As [`java_interface_items`].
pub fn java_interface_items_with(
    ast: &Ast,
    mapping: ServiceMapping,
) -> Result<Vec<java::TypeDecl>, BindingError> {
    let mut pending: Vec<(Vec<String>, java::Member)> = Vec::new();
    // §7.5.1.1.5 rule 4a names the constant after the exception alone, so
    // two interfaces of one module that raise it share the one constant.
    let mut declared_hashes: Vec<(Vec<String>, String)> = Vec::new();
    for service in interface::services(ast)? {
        let chain: Vec<String> = interface::module_chain(ast, service.module)?
            .into_iter()
            .map(|(_, name)| java::escape_keyword(&name))
            .collect();
        let members = if mapping.is_enhanced() {
            enhanced::members(ast, &Service { mapping, ..service })?
        } else {
            members_of(ast, &service, &chain, &mut declared_hashes)?
        };
        for member in members {
            pending.push((chain.clone(), member));
        }
    }
    // A service defined by a pair of types (DDS-RPC 1.0 §7.3.1.4).
    pending.extend(
        crate::emit::type_pair_java::java_type_pair_members(ast).map_err(BindingError::Codec)?,
    );
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![java::TypeDecl::Class(java::Class {
        doc: Doc::lines(&[
            "The Java shape of every service one IDL file declares.",
            "",
            "DDS-RPC 1.0 §7.11.2. Each IDL module nests as a class, exactly as",
            "the data types nest inside `ZdTypes`.",
        ]),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public, java::Modifier::Final],
        name: Ident::new(OUTER),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members: nest(&pending, 0),
    })])
}

/// One level of the module fold.
///
/// zerodds-lint: recursion-depth 64 (module nesting; every chain is
/// built by `interface::module_chain`, which is bounded by
/// `interface::MAX_MODULE_DEPTH`)
fn nest(items: &[(Vec<String>, java::Member)], depth: usize) -> Vec<java::Member> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < items.len() {
        let Some((chain, member)) = items.get(index) else {
            break;
        };
        let Some(name) = chain.get(depth) else {
            out.push(member.clone());
            index += 1;
            continue;
        };
        let mut end = index;
        while items
            .get(end)
            .is_some_and(|(other, _)| other.get(depth) == Some(name))
        {
            end += 1;
        }
        let Some(run) = items.get(index..end) else {
            break;
        };
        out.push(java::Member::Nested(Box::new(java::TypeDecl::Class(
            java::Class {
                doc: Doc::line("An IDL module, as the class its services nest in."),
                annotations: Vec::new(),
                modifiers: vec![
                    java::Modifier::Public,
                    java::Modifier::Static,
                    java::Modifier::Final,
                ],
                name: Ident::new(name),
                generics: Vec::new(),
                extends: None,
                implements: Vec::new(),
                members: nest(run, depth + 1),
            },
        ))));
        index = end;
    }
    out
}

fn members_of(
    ast: &Ast,
    service: &Service,
    chain: &[String],
    declared_hashes: &mut Vec<(Vec<String>, String)>,
) -> Result<Vec<java::Member>, BindingError> {
    let name = java::escape_keyword(&service.name);
    let operations = service.all_operations();
    let mut out = Vec::new();

    // The topic types of §7.5.1.1.4-7, the same shape and the same names
    // `emit::interface_cpp` writes and `crates/rpc/src/codegen.rs`
    // derives.
    for operation in &operations {
        out.push(holder(
            &interface::in_struct_name(&name, &java::escape_keyword(&operation.name)),
            request_fields(ast, service, operation)?,
            Doc::line("The `in` and `inout` parameters of one call. DDS-RPC 1.0 §7.5.1.1.4."),
        ));
        out.push(holder(
            &interface::out_struct_name(&name, &java::escape_keyword(&operation.name)),
            reply_fields(ast, service, operation)?,
            Doc::line(
                "The return value and the `out` and `inout` parameters of one call. DDS-RPC 1.0 \
                 §7.5.1.1.5.",
            ),
        ));
    }
    // §7.5.1.1.6 rule 4 and §7.5.1.1.5 rule 4a: one label constant per
    // operation and one per exception, beside the unions.
    for operation in &operations {
        let spelled = java::escape_keyword(&operation.name);
        out.push(hash_constant(
            &interface::operation_hash_constant(&name, &spelled),
            interface::operation_hash(&operation.name),
            "The case label of one operation. DDS-RPC 1.0 §7.5.1.1.6 rules 4 and 5a.",
        ));
    }
    for operation in &operations {
        for raised in &operation.raises {
            let key = (chain.to_vec(), raised.scoped_name.clone());
            if declared_hashes.contains(&key) {
                continue;
            }
            declared_hashes.push(key);
            out.push(hash_constant(
                &interface::exception_hash_constant(&java::escape_keyword(&raised.name)),
                interface::exception_hash(&raised.scoped_name),
                "The case label of one raised exception. DDS-RPC 1.0 §7.5.1.1.5 rule 4.",
            ));
        }
    }
    for operation in &operations {
        out.push(result_union_of(ast, &name, operation)?);
    }
    out.push(union_of(
        &interface::call_union_name(&name),
        &operations,
        &name,
        false,
    ));
    out.push(union_of(
        &interface::return_union_name(&name),
        &operations,
        &name,
        true,
    ));
    out.push(holder(
        &interface::request_type_name(&name),
        vec![
            field(qualified(RPC, "RequestHeader"), "header"),
            field(
                local(&interface::call_union_name(&name)),
                interface::DATA_MEMBER,
            ),
        ],
        Doc::line("The request topic type. DDS-RPC 1.0 §7.5.1.1.6."),
    ));
    out.push(holder(
        &interface::reply_type_name(&name),
        vec![
            field(qualified(RPC, "ReplyHeader"), "header"),
            field(
                local(&interface::return_union_name(&name)),
                interface::DATA_MEMBER,
            ),
        ],
        Doc::line("The reply topic type. DDS-RPC 1.0 §7.5.1.1.7."),
    ));
    // §7.5.1.1.6 and §7.5.1.1.7 declare the two topic types in the module
    // of the interface, so the DDS type name is module-qualified, as the
    // C++ column announces it.
    let scope: Vec<String> = interface::module_chain(ast, service.module)?
        .into_iter()
        .map(|(_, module)| module)
        .collect();
    out.push(support_class(service, &name, &scope));

    out.push(sync_interface(ast, service, &name)?);
    out.push(async_interface(ast, service, &name)?);

    if service.is_remote() {
        // §7.9, §7.11.1.5, §7.11.2.3: the codec, the client and the
        // service that run.
        out.extend(calls::members(ast, service, &name)?);
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
        out.push(client_interface(&name));
        out.push(service_interface(&name));
    }
    Ok(out)
}

// -- the topic types ---------------------------------------------------

fn request_fields(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<Vec<java::Member>, BindingError> {
    let mut out = Vec::new();
    for param in operation.inputs() {
        out.push(field(
            java_member_type(ast, param.type_, &at(service, operation, &param.name))?,
            &java::escape_keyword(&param.name),
        ));
    }
    // §7.5.1.1.4 rule 3c: an operation with no `in`/`inout` parameters
    // carries a single `dummy` of type `dds::rpc::UnusedMember`, not an
    // empty structure — XCDR gives an empty structure no octets.
    if out.is_empty() {
        out.push(field(
            qualified(RPC, "UnusedMember"),
            interface::DUMMY_MEMBER,
        ));
    }
    Ok(out)
}

fn reply_fields(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<Vec<java::Member>, BindingError> {
    // §7.5.1.1.5 rule 3a: the `out` and `inout` parameters first, in
    // declaration order and under their formal names; rule 3b: the
    // return value *last*.
    let mut out = Vec::new();
    for param in operation.outputs() {
        out.push(field(
            java_member_type(ast, param.type_, &at(service, operation, &param.name))?,
            &java::escape_keyword(&param.name),
        ));
    }
    if let Some(returns) = operation.returns {
        let taken: Vec<String> = operation
            .outputs()
            .map(|param| java::escape_keyword(&param.name))
            .collect();
        out.push(field(
            java_member_type(ast, returns, &at(service, operation, "return"))?,
            &return_member_name(&taken),
        ));
    }
    // Rule 3c: nothing at all becomes the `dummy` member.
    if out.is_empty() {
        out.push(field(
            qualified(RPC, "UnusedMember"),
            interface::DUMMY_MEMBER,
        ));
    }
    Ok(out)
}

/// The name §7.5.1.1.5 rule 3b gives the return value, with the
/// collision rule: "If an operation has an out argument named `return_`
/// and the operation has a regular return value, the regular return
/// value in the Out structure shall be represented as `return_N`."
fn return_member_name(taken: &[String]) -> String {
    if !taken.iter().any(|name| name == RETURN_MEMBER) {
        return RETURN_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{RETURN_MEMBER}{n}");
        if !taken.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }
    // Unreachable below 2^31 parameters; a name no IDL identifier can
    // take is still better than two members sharing one.
    format!("{RETURN_MEMBER} ")
}

fn union_of(name: &str, operations: &[Operation], service: &str, reply: bool) -> java::Member {
    let mut members = vec![
        discriminator_field(),
        // §7.5.1.1.6 rule 3 / §7.5.1.1.7 rule 3: the `default` case.
        case_field(
            qualified(RPC, "UnknownOperation"),
            interface::UNKNOWN_OP_MEMBER,
            "Live on the `default` case. DDS-RPC 1.0 §7.5.1.1.6 rule 3.",
        ),
    ];
    for operation in operations {
        let spelled = java::escape_keyword(&operation.name);
        let case = if reply {
            interface::result_union_name(service, &spelled)
        } else {
            interface::in_struct_name(service, &spelled)
        };
        members.push(case_field(
            local(&case),
            &spelled,
            &format!(
                "Live when `_d` is `{}`.",
                interface::operation_hash_constant(service, &spelled)
            ),
        ));
    }
    holder(
        name,
        members,
        Doc::line(if reply {
            "The reply union, without the `oneway` operations. DDS-RPC 1.0 §7.5.1.1.7."
        } else {
            "The request union over every operation. DDS-RPC 1.0 §7.5.1.1.6."
        }),
    )
}

/// The `_d` member every synthesized union carries. §7.5.1.1.6 rule 2.
fn discriminator_field() -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::line("Which case this sample carries. DDS-RPC 1.0 §7.5.1.1.6 rule 2."),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        type_: java::Type::Primitive(java::Primitive::Int),
        name: Ident::new("_d"),
        value: None,
    })
}

/// One case member of a synthesized union.
fn case_field(type_: java::Type, name: &str, doc: &str) -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::line(doc),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        type_,
        name: Ident::new(name),
        value: None,
    })
}

/// One `${I}_${op}_Hash` or `${exception}_Ex_Hash` constant.
/// §7.5.1.1.6 rule 4, §7.5.1.1.5 rule 4.
fn hash_constant(name: &str, value: i32, doc: &str) -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::line(doc),
        annotations: Vec::new(),
        modifiers: vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        type_: java::Type::Primitive(java::Primitive::Int),
        name: Ident::new(name),
        value: Some(java::Expr::Literal(zerodds_lang_cst::common::Literal::Int(
            value.to_string(),
        ))),
    })
}

/// `${I}_${op}_Result` — §7.5.1.1.5.
fn result_union_of(
    ast: &Ast,
    name: &str,
    operation: &Operation,
) -> Result<java::Member, BindingError> {
    let spelled = java::escape_keyword(&operation.name);
    let mut members = vec![
        discriminator_field(),
        case_field(
            local(&interface::out_struct_name(name, &spelled)),
            interface::RESULT_MEMBER,
            "Live when `_d` is `dds::RETCODE_OK` (0). DDS-RPC 1.0 §7.5.1.1.5 rule 3a.",
        ),
    ];
    // §7.5.1.1.5: the `default` case carries `dds::rpc::UnknownException`.
    members.push(case_field(
        qualified(RPC, "UnknownException"),
        "unknownEx",
        "Live on the `default` case. DDS-RPC 1.0 §7.5.1.1.5.",
    ));
    for raised in &operation.raises {
        let exception = java::escape_keyword(&raised.name);
        members.push(case_field(
            exception_type(ast, raised)?,
            &interface::exception_member_name(&exception),
            &format!(
                "Live when `_d` is `{}`. DDS-RPC 1.0 §7.5.1.1.5 rule 5.",
                interface::exception_hash_constant(&exception)
            ),
        ));
    }
    Ok(holder(
        &interface::result_union_name(name, &spelled),
        members,
        Doc::line("The outcome of one operation. DDS-RPC 1.0 §7.5.1.1.5."),
    ))
}

/// The data holder an exception is declared as, reachable from any module:
/// `ZdTypes.<modules>.<Name>`.
fn exception_type(ast: &Ast, raised: &interface::Raise) -> Result<java::Type, BindingError> {
    let node = ast
        .exception(raised.id)
        .map_err(|_| interface::InterfaceError::NoNode {
            kind: "exception",
            at: raised.name.clone(),
        })?;
    calls::data_type(ast, node.module, &raised.name)
}

/// `ISupport` — the two topic names of §7.4.1 and the two type names of
/// §7.5.1.1.6-7, which are different strings.
fn support_class(service: &Service, name: &str, scope: &[String]) -> java::Member {
    support_class_with(service, name, scope, true, None)
}

/// [`support_class`], with the factory methods that name the codec
/// (`factories`) or without them.
fn support_class_with(
    service: &Service,
    name: &str,
    scope: &[String],
    factories: bool,
    descriptions: Option<&(
        crate::emit::interface_rust::type_information::JavaDescription,
        crate::emit::interface_rust::type_information::JavaDescription,
    )>,
) -> java::Member {
    let style = interface::BindingStyle::FunctionCall;
    let mut members = vec![
        topic_constant("REQUEST_TOPIC_NAME", &service.request_topic_name(style)),
        topic_constant("REPLY_TOPIC_NAME", &service.reply_topic_name(style)),
        topic_constant("SERVICE_NAME", &service.service_name),
        // §7.4.1: the `<interface_name>` — the module names joined by
        // underscores, then the interface name. A run-time service name
        // (§7.4.2.3) composes the topic names from it.
        topic_constant("INTERFACE_NAME", &service.qualified_name),
        topic_constant(
            "REQUEST_TYPE_NAME",
            &scoped(scope, &interface::request_type_name(name)),
        ),
        topic_constant(
            "REPLY_TYPE_NAME",
            &scoped(scope, &interface::reply_type_name(name)),
        ),
    ];
    // §7.4.3: one alias per parent interface.
    for (at, alias) in service.topic_aliases(style, false).into_iter().enumerate() {
        members.push(topic_constant(&format!("REQUEST_TOPIC_ALIAS_{at}"), &alias));
    }
    for (at, alias) in service.topic_aliases(style, true).into_iter().enumerate() {
        members.push(topic_constant(&format!("REPLY_TOPIC_ALIAS_{at}"), &alias));
    }
    if factories {
        members.extend(calls::support_methods(service, name, descriptions));
    }
    holder(
        &format!("{name}Support"),
        members,
        Doc::line("The service's type support: the names of its two topics and its two types."),
    )
}

/// `module::Name`: the DDS type name of a type declared in `scope`.
fn scoped(scope: &[String], name: &str) -> String {
    let mut out = String::new();
    for module in scope {
        out.push_str(module);
        out.push_str("::");
    }
    out.push_str(name);
    out
}

fn topic_constant(name: &str, topic: &str) -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::line("A topic name. DDS-RPC 1.0 §7.4.1.1."),
        annotations: Vec::new(),
        modifiers: vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        type_: java::Type::named(Path::single("String")),
        name: Ident::new(name),
        value: Some(java::Expr::Literal(zerodds_lang_cst::common::Literal::Str(
            topic.to_string(),
        ))),
    })
}

// -- the two interfaces ------------------------------------------------

fn sync_interface(ast: &Ast, service: &Service, name: &str) -> Result<java::Member, BindingError> {
    let mut members = Vec::new();
    for operation in &service.operations {
        members.push(java::Member::Method(sync_method(
            ast, service, name, operation,
        )?));
    }
    for attribute in &service.attributes {
        members.extend(sync_accessors(ast, service, attribute)?);
    }
    Ok(java::Member::Nested(Box::new(java::TypeDecl::Interface(
        java::Interface {
            doc: Doc::line(
                "The service, as the interface DDS-RPC 1.0 §7.11.1.1.2 rule 1 requires and \
                 §7.11.2.3 carries into Java.",
            ),
            annotations: Vec::new(),
            modifiers: vec![java::Modifier::Public],
            name: Ident::new(name),
            generics: Vec::new(),
            extends: base_types(ast, service, "")?,
            members,
        },
    ))))
}

fn sync_method(
    ast: &Ast,
    service: &Service,
    name: &str,
    operation: &Operation,
) -> Result<java::Method, BindingError> {
    // The multiple return of an `out`/`inout` call travels in the type
    // rule 7.i names for the same call shape.
    let returns = if operation.has_outputs() {
        local(&interface::out_struct_name(
            name,
            &java::escape_keyword(&operation.name),
        ))
    } else if let Some(target) = operation.returns {
        java_member_type(ast, target, &at(service, operation, "return"))?
    } else {
        java::Type::Void
    };
    let mut params = Vec::new();
    for param in operation.inputs() {
        params.push(java_param(ast, service, operation, param)?);
    }
    Ok(method(
        &java::escape_keyword(&operation.name),
        returns,
        params,
        raises_doc(operation),
    ))
}

fn sync_accessors(
    ast: &Ast,
    service: &Service,
    attribute: &Attribute,
) -> Result<Vec<java::Member>, BindingError> {
    let at = format!("{}::{}", service.name, attribute.name);
    let type_ = java_member_type(ast, attribute.type_, &at)?;
    let spelled = java::escape_keyword(&attribute.name);
    let mut out = vec![java::Member::Method(method(
        &spelled,
        type_.clone(),
        Vec::new(),
        Doc::line("An attribute accessor. DDS-RPC 1.0 §7.11.1.1.2 rule 6.f."),
    ))];
    if !attribute.readonly {
        out.push(java::Member::Method(method(
            &spelled,
            java::Type::Void,
            vec![java::Param {
                annotations: Vec::new(),
                final_: false,
                type_,
                name: Ident::new(&spelled),
                varargs: false,
            }],
            Doc::line("An attribute accessor. DDS-RPC 1.0 §7.11.1.1.2 rule 6.f."),
        )));
    }
    Ok(out)
}

fn async_interface(ast: &Ast, service: &Service, name: &str) -> Result<java::Member, BindingError> {
    let mut members = Vec::new();
    for operation in &service.operations {
        members.push(java::Member::Method(async_method(
            ast, service, name, operation,
        )?));
    }
    for attribute in &service.attributes {
        let at = format!("{}::{}", service.name, attribute.name);
        let type_ = java_member_type(ast, attribute.type_, &at)?;
        let spelled = java::escape_keyword(&attribute.name);
        members.push(java::Member::Method(method(
            &format!("{spelled}_async"),
            future_of(boxed(type_.clone())),
            Vec::new(),
            Doc::line("An attribute accessor. DDS-RPC 1.0 §7.11.1.1.2 rule 7.f."),
        )));
        if !attribute.readonly {
            members.push(java::Member::Method(method(
                &format!("{spelled}_async"),
                future_of(java::Type::named(Path::single("Void"))),
                vec![java::Param {
                    annotations: Vec::new(),
                    final_: false,
                    type_,
                    name: Ident::new(&spelled),
                    varargs: false,
                }],
                Doc::line("An attribute accessor. DDS-RPC 1.0 §7.11.1.1.2 rule 7.g."),
            )));
        }
    }
    Ok(java::Member::Nested(Box::new(java::TypeDecl::Interface(
        java::Interface {
            doc: Doc::line("The service's asynchronous half. DDS-RPC 1.0 §7.11.1.1.2 rule 1."),
            annotations: Vec::new(),
            modifiers: vec![java::Modifier::Public],
            name: Ident::new(&format!("{name}Async")),
            generics: Vec::new(),
            extends: base_types(ast, service, "Async")?,
            members,
        },
    ))))
}

fn async_method(
    ast: &Ast,
    service: &Service,
    name: &str,
    operation: &Operation,
) -> Result<java::Method, BindingError> {
    // Rules 7.h and 7.i, with `Void` where C++ writes `void`: a Java
    // type argument cannot be a primitive and cannot be `void`.
    let payload = if operation.has_outputs() {
        local(&interface::out_struct_name(
            name,
            &java::escape_keyword(&operation.name),
        ))
    } else if let Some(target) = operation.returns {
        boxed(java_member_type(
            ast,
            target,
            &at(service, operation, "return"),
        )?)
    } else {
        java::Type::named(Path::single("Void"))
    };
    let mut params = Vec::new();
    for param in operation.inputs() {
        params.push(java_param(ast, service, operation, param)?);
    }
    Ok(method(
        &format!("{}_async", java::escape_keyword(&operation.name)),
        future_of(payload),
        params,
        raises_doc(operation),
    ))
}

// -- the endpoints -----------------------------------------------------

/// `class <I>Requester extends org.zerodds.ddsrpc.RequesterImpl<TReq, TRep>`.
///
/// A class, unlike every other type this file emits except the topic
/// types: `org.omg.dds.rpc.Requester` and `Replier` are interfaces
/// (§7.11.2.2.1), `RequesterImpl` and `ReplierImpl` their implementations,
/// and the typed name is the generic one with its two arguments fixed —
/// what the C++ column says with a `using`. §7.11.1.4.3 calls a `Requester`
/// "a reference type … configured at the time of construction using
/// RequesterParams", which is a constructor and not a method, so the
/// constructor forwards.
fn endpoint(name: &str, template: &str, service: &str, doc: &str) -> java::Member {
    // DDS-RPC 1.0 §7.11.2: the params of the machine-readable Java files take no type
    // arguments.
    let params = java::Type::named(path(RPC, &format!("{template}Params")));
    java::Member::Nested(Box::new(java::TypeDecl::Class(java::Class {
        doc: Doc::line(doc),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public, java::Modifier::Static],
        name: Ident::new(name),
        generics: Vec::new(),
        extends: Some(java::Type::generic(
            path(RPC_IMPL, &format!("{template}Impl")),
            vec![
                local(&interface::request_type_name(service)),
                local(&interface::reply_type_name(service)),
            ],
        )),
        implements: Vec::new(),
        members: vec![java::Member::Constructor(java::Constructor {
            doc: Doc::line("The generic endpoint, over this service's two topic types."),
            annotations: Vec::new(),
            modifiers: vec![java::Modifier::Public],
            name: Ident::new(name),
            params: vec![java::Param {
                annotations: Vec::new(),
                final_: false,
                type_: params,
                name: Ident::new("params"),
                varargs: false,
            }],
            throws: Vec::new(),
            body: java::Block {
                statements: vec![java::Statement::Expr(java::Expr::MethodCall {
                    receiver: None,
                    name: Ident::new("super"),
                    args: vec![java::Expr::Path(Path::single("params"))],
                })],
            },
        })],
    })))
}

/// `IClient` — §7.11.1.5.4 through §7.11.2.3.
///
/// "A `Client` inherits publicly from `${interface}`,
/// `${interface}Async`, and `ClientEndpoint`." Three supertypes, which
/// a Java *class* could not have and a Java interface can.
fn client_interface(name: &str) -> java::Member {
    java::Member::Nested(Box::new(java::TypeDecl::Interface(java::Interface {
        doc: Doc::line("The function-call client. DDS-RPC 1.0 §7.11.1.5.4, §7.11.2.3."),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        name: Ident::new(&format!("{name}Client")),
        generics: Vec::new(),
        extends: vec![
            local(name),
            local(&format!("{name}Async")),
            java::Type::named(path(RPC, "ClientEndpoint")),
        ],
        members: Vec::new(),
    })))
}

/// `IService` — §7.11.1.5.1 through §7.11.2.3.
fn service_interface(name: &str) -> java::Member {
    java::Member::Nested(Box::new(java::TypeDecl::Interface(java::Interface {
        doc: Doc::line("The function-call service. DDS-RPC 1.0 §7.11.1.5.1, §7.11.2.3."),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        name: Ident::new(&format!("{name}Service")),
        generics: Vec::new(),
        extends: vec![java::Type::named(path(RPC, "ServiceEndpoint"))],
        members: vec![java::Member::Method(method(
            "implementation",
            local(name),
            Vec::new(),
            Doc::line("The implementation this service dispatches into."),
        ))],
    })))
}

// -- the shapes ---------------------------------------------------------

fn java_param(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
    param: &Parameter,
) -> Result<java::Param, BindingError> {
    Ok(java::Param {
        annotations: Vec::new(),
        final_: false,
        type_: java_member_type(ast, param.type_, &at(service, operation, &param.name))?,
        name: Ident::new(&java::escape_keyword(&param.name)),
        varargs: false,
    })
}

fn method(name: &str, returns: java::Type, params: Vec<java::Param>, doc: Doc) -> java::Method {
    java::Method {
        doc,
        annotations: Vec::new(),
        // No modifier: an interface method is public and abstract
        // already, and `-Xlint:all -Werror` warns about writing either.
        modifiers: Vec::new(),
        generics: Vec::new(),
        returns,
        name: Ident::new(name),
        params,
        // Rule 6.c: no exception specification.
        throws: Vec::new(),
        body: None,
    }
}

/// `org.omg.dds.rpc.Future<T>` — §7.11.2.2.4.
fn future_of(payload: java::Type) -> java::Type {
    java::Type::generic(path(RPC, "Future"), vec![payload])
}

/// The wrapper class a primitive takes as a type argument.
fn boxed(type_: java::Type) -> java::Type {
    match type_ {
        java::Type::Primitive(primitive) => java::Type::named(Path::single(primitive.boxed())),
        java::Type::Void => java::Type::named(Path::single("Void")),
        other => other,
    }
}

/// The base interfaces of `service`, spelled from its module: a base may be
/// declared in another one.
fn base_types(ast: &Ast, service: &Service, suffix: &str) -> Result<Vec<java::Type>, BindingError> {
    service
        .bases
        .iter()
        .map(|base| {
            let module = interface::service(ast, base.id)?.module;
            enhanced::holder_type(
                ast,
                service.module,
                module,
                &format!("{}{suffix}", java::escape_keyword(&base.name)),
            )
        })
        .collect()
}

fn holder(name: &str, members: Vec<java::Member>, doc: Doc) -> java::Member {
    java::Member::Nested(Box::new(java::TypeDecl::Class(java::Class {
        doc,
        annotations: Vec::new(),
        modifiers: vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        name: Ident::new(name),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members,
    })))
}

fn field(type_: java::Type, name: &str) -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        type_,
        name: Ident::new(name),
        value: None,
    })
}

fn local(name: &str) -> java::Type {
    java::Type::named(Path::single(name))
}

fn path(package: &str, name: &str) -> Path {
    let mut segments: Vec<&str> = package.split('.').collect();
    segments.push(name);
    Path::new(&segments)
}

fn qualified(package: &str, name: &str) -> java::Type {
    java::Type::named(path(package, name))
}

/// The `raises` clause, as documentation. See
/// [`Operation::raises_line`].
fn raises_doc(operation: &Operation) -> Doc {
    operation
        .raises_line()
        .map_or_else(Doc::none, |line| Doc::line(&line))
}

/// Where an operation sits, for a diagnostic.
fn at(service: &Service, operation: &Operation, what: &str) -> String {
    format!("{}::{}({what})", service.name, operation.name)
}
