// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The function-call entities of DDS-RPC 1.0 §7.11.1.5, with bodies.
//!
//! `IClient` (§7.11.1.5.4) derives from `${interface}`, `${interface}Async`
//! and `ClientEndpoint`, and every operation it overrides packs the `Call`
//! union (§7.5.1.1.6), sends it through a `dds::rpc::Requester`, and unpacks
//! the `Return` union (§7.5.1.1.7) and the `Result` union (§7.5.1.1.5) of the
//! reply: the return value and the `out` parameters on success, a thrown
//! exception for every exception the operation raises, and the exception of
//! a remote exception code (§7.11.1.3) for the rest.
//!
//! `IService` (§7.11.1.5.1) derives from `dds::rpc::Service`, so a `Server`
//! takes it (§7.11.1.5.3). Its `dispatch()` takes the requests that have
//! arrived, calls the implementation, and answers: with the `Result` union
//! for a result or a declared exception (§7.5.1.1.5), with a remote
//! exception code for anything else the implementation throws (§7.5.2) and
//! for an operation it does not have (§7.7.1.1).
//!
//! The runtime does what does not depend on the interface:
//! `Requester::call`, `Requester::send_request_mapped`, the correlation and
//! the exception translation. Only the packing and unpacking is generated.

use super::{
    Operation, Origin, Service, alias, async_method, async_payload, at, converted, cxx_escape,
    is_native, is_psm, native_out_type, qualified, return_member_name, sync_method, user_type,
};
use crate::adapter::interface::{self, BindingError, EnhancedMemberType, EnhancedService};
use zerodds_ast::{Ast, ModuleNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp;

const RPC: &str = "dds::rpc";

// -- the two shapes of the topic types ---------------------------------

/// What the Enhanced Service Mapping calls one operation by, spelled for
/// the module the class is written in.
#[derive(Clone)]
pub(super) struct EnhancedOp {
    /// The operation's name.
    name: String,
    /// `${I}_${op}_In`, `${I}_${op}_Out` and `${I}_${op}_Result` of the
    /// interface that declares the operation.
    in_type: cpp::Type,
    out_type: cpp::Type,
    /// The `Out` structure as the application sees it: the topic structure
    /// itself under the native mapping, and under `--cpp-mapping psm` the
    /// application's own, with Table 7.1's spellings, in the IDL's namespace.
    application_out_type: cpp::Type,
    result_type: cpp::Type,
    /// The `Result` member of each raised exception: the exception's
    /// simple name and the member.
    exceptions: Vec<(String, String)>,
}

/// The Enhanced Service Mapping's operations of one interface.
#[derive(Clone)]
pub(super) struct EnhancedShape {
    ops: Vec<EnhancedOp>,
}

impl EnhancedShape {
    /// The operations of `model`, the inherited ones included, spelled from
    /// `here`.
    pub(super) fn new(
        ast: &Ast,
        here: Id<ModuleNode>,
        model: &EnhancedService,
    ) -> Result<Self, BindingError> {
        let mut ops = Vec::new();
        for operation in &model.operations {
            let spelled =
                |structure: &str| super::enhanced::type_in(ast, here, operation.module, structure);
            let mut exceptions = Vec::new();
            for member in &operation.result_struct.members {
                if let EnhancedMemberType::Exception(raised) = &member.type_ {
                    exceptions.push((raised.name.clone(), cxx_escape(&member.name)));
                }
            }
            ops.push(EnhancedOp {
                name: operation.operation.name.clone(),
                in_type: spelled(&operation.in_struct.name)?,
                out_type: spelled(&operation.out_struct.name)?,
                application_out_type: cpp::Type::named(super::enhanced::path_in_idl(
                    ast,
                    here,
                    operation.module,
                    &operation.out_struct.name,
                )?),
                result_type: spelled(&operation.result_struct.name)?,
                exceptions,
            });
        }
        Ok(Self { ops })
    }
}

/// Which of the two topic type shapes the generated bodies pack and unpack.
#[derive(Clone, Copy)]
pub(super) enum Shape<'a> {
    /// The `Call` and `Return` unions behind a header (§7.5.1.1).
    Basic,
    /// The `@Choice` structures (§7.5.1.2).
    Enhanced(&'a EnhancedShape),
}

impl Shape<'_> {
    /// The Enhanced spelling of `operation`; `None` under the Basic
    /// mapping.
    fn op(&self, operation: &Operation) -> Result<Option<&EnhancedOp>, BindingError> {
        match self {
            Self::Basic => Ok(None),
            Self::Enhanced(shape) => shape
                .ops
                .iter()
                .find(|op| op.name == operation.name)
                .map(Some)
                .ok_or_else(|| {
                    BindingError::Codec(format!(
                        "operation `{}` is not an operation of the Enhanced mapping of its interface",
                        operation.name
                    ))
                }),
        }
    }

    fn is_enhanced(&self) -> bool {
        matches!(self, Self::Enhanced(_))
    }
}

// -- small builders ----------------------------------------------------

fn name(text: &str) -> cpp::Expr {
    cpp::Expr::Path(Path::single(text))
}

fn path(segments: &[&str]) -> cpp::Expr {
    cpp::Expr::Path(Path::new(segments))
}

fn text(value: &str) -> cpp::Expr {
    cpp::Expr::Literal(Literal::Str(value.to_string()))
}

fn dot(base: cpp::Expr, member: &str) -> cpp::Expr {
    cpp::Expr::Member {
        value: Box::new(base),
        name: Ident::new(member),
    }
}

fn method_call(receiver: cpp::Expr, method: &str, args: Vec<cpp::Expr>) -> cpp::Expr {
    cpp::Expr::MethodCall {
        receiver: Box::new(receiver),
        name: Ident::new(method),
        args,
    }
}

fn call(callee: cpp::Expr, args: Vec<cpp::Expr>) -> cpp::Expr {
    cpp::Expr::Call {
        callee: Box::new(callee),
        args,
    }
}

fn binary(op: &'static str, lhs: cpp::Expr, rhs: cpp::Expr) -> cpp::Expr {
    cpp::Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn assign(lhs: cpp::Expr, rhs: cpp::Expr) -> cpp::Statement {
    cpp::Statement::Expr(binary("=", lhs, rhs))
}

fn block(statements: Vec<cpp::Statement>) -> cpp::Block {
    cpp::Block { statements }
}

fn local(type_: cpp::Type, local_name: &str, value: Option<cpp::Expr>) -> cpp::Statement {
    cpp::Statement::Declare {
        type_,
        name: Ident::new(local_name),
        value,
    }
}

fn named(type_name: &str) -> cpp::Type {
    cpp::Type::named(Path::single(type_name))
}

fn const_ref(type_: cpp::Type) -> cpp::Type {
    cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_))))
}

fn zero_init(type_name: &str) -> cpp::Expr {
    cpp::Expr::Construct {
        type_: Box::new(named(type_name)),
        args: Vec::new(),
        braced: true,
    }
}

fn throw_error(class: &str, message: &str) -> cpp::Statement {
    cpp::Statement::Throw(call(path(&["dds", "rpc", class]), vec![text(message)]))
}

fn param(type_: cpp::Type, param_name: &str) -> cpp::Param {
    cpp::Param {
        type_,
        name: Some(Ident::new(param_name)),
        default: None,
    }
}

fn static_method(
    returns: cpp::Type,
    method_name: &str,
    params: Vec<cpp::Param>,
    statements: Vec<cpp::Statement>,
    doc: &str,
) -> cpp::ClassMember {
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line(doc),
        returns,
        name: Ident::new(method_name),
        params,
        is_const: false,
        is_static: true,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(block(statements)),
    })
}

fn private_section(members: Vec<cpp::ClassMember>) -> cpp::Section {
    cpp::Section {
        access: cpp::Access::Private,
        members,
    }
}

/// The name a parameter has in a generated body.
fn param_name(parameter: &interface::Parameter) -> String {
    cxx_escape(&parameter.name)
}

/// The name of the return value in the `Out` structure, §7.5.1.1.5
/// rule 3b.
fn return_member(operation: &Operation) -> String {
    let taken: Vec<String> = operation.outputs().map(param_name).collect();
    return_member_name(&taken)
}

// -- the Support class -------------------------------------------------

/// `ISupport::defaults()`: what the interface declares for the runtime.
pub(super) fn defaults_method(service: &Service) -> cpp::ClassMember {
    let literal = |value: &str| text(value);
    let fields = vec![
        literal(&service.qualified_name),
        literal(&service.service_name),
        literal(service.request_topic.as_deref().unwrap_or("")),
        literal(service.reply_topic.as_deref().unwrap_or("")),
        literal(service.qos_request_profile.as_deref().unwrap_or("")),
        literal(service.qos_reply_profile.as_deref().unwrap_or("")),
    ];
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line(
            "What the interface declares for the endpoints: its `<interface_name>` (§7.4.1), \
             `@service`, `@DDSRequestTopic`/`@DDSReplyTopic` (§7.4.2.2) and `@Qos` (§7.10.1).",
        ),
        returns: qualified(RPC, "InterfaceDefaults"),
        name: Ident::new("defaults"),
        params: Vec::new(),
        is_const: false,
        is_static: true,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(block(vec![cpp::Statement::Return(Some(
            cpp::Expr::Construct {
                type_: Box::new(qualified(RPC, "InterfaceDefaults")),
                args: fields,
                braced: true,
            },
        ))])),
    })
}

// -- packing and unpacking ---------------------------------------------

/// `Request zd_request{}; zd_request.data._d = <hash>; zd_request.data.<op>.<p> = <p>;`
fn pack_request(
    ast: &Ast,
    declaring: &Service,
    service: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<Vec<cpp::Statement>, BindingError> {
    let operation_name = cxx_escape(&operation.name);
    let request_type = interface::request_type_name(service);
    if shape.op(operation)?.is_some() {
        // §7.5.1.2.4: the request is the structure whose one member, the
        // operation's, holds its `In`.
        let member = dot(name("zd_request"), &operation_name);
        let mut out = vec![
            local(
                named(&request_type),
                "zd_request",
                Some(zero_init(&request_type)),
            ),
            cpp::Statement::Expr(method_call(member.clone(), "emplace", Vec::new())),
        ];
        for parameter in operation.inputs() {
            let name_ = param_name(parameter);
            out.push(assign(
                cpp::Expr::Arrow {
                    value: Box::new(member.clone()),
                    name: Ident::new(&name_),
                },
                converted(
                    ast,
                    parameter.type_,
                    &at(declaring, operation, &parameter.name),
                    name(&name_),
                    true,
                )?,
            ));
        }
        return Ok(out);
    }
    let case = dot(
        dot(name("zd_request"), interface::DATA_MEMBER),
        &operation_name,
    );
    let mut out = vec![
        local(
            named(&request_type),
            "zd_request",
            Some(zero_init(&request_type)),
        ),
        assign(
            dot(dot(name("zd_request"), interface::DATA_MEMBER), "_d"),
            name(&interface::operation_hash_constant(
                service,
                &operation_name,
            )),
        ),
    ];
    for parameter in operation.inputs() {
        let member = param_name(parameter);
        out.push(assign(
            dot(case.clone(), &member),
            converted(
                ast,
                parameter.type_,
                &at(declaring, operation, &parameter.name),
                name(&member),
                true,
            )?,
        ));
    }
    Ok(out)
}

/// `static const Out& result_<op>(const Reply& zd_reply)`: the checks of
/// §7.5.1.1.7 and §7.5.1.1.5 on a reply, and the `Out` structure of a
/// result; the exception of an exception case; the remote exception of
/// anything else.
fn result_accessor(
    ast: &Ast,
    home: &Service,
    service: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<cpp::ClassMember, BindingError> {
    let operation_name = cxx_escape(&operation.name);
    let reply_type = interface::reply_type_name(service);
    if let Some(enhanced) = shape.op(operation)? {
        // §7.5.1.2.5: the member of the operation holds its `Result`; that
        // holds the `Out` or the exception it raised (§7.5.1.2.3).
        let reply_member = dot(name("zd_reply"), &operation_name);
        let mut statements = vec![
            cpp::Statement::If {
                condition: cpp::Expr::Unary {
                    op: "!",
                    value: Box::new(reply_member.clone()),
                },
                then: block(vec![throw_error(
                    "RemoteUnknownOperationError",
                    "the reply does not answer the operation of the request",
                )]),
                otherwise: None,
            },
            local(
                const_ref(enhanced.result_type.clone()),
                "zd_result",
                Some(cpp::Expr::Deref(Box::new(reply_member))),
            ),
            cpp::Statement::If {
                condition: dot(name("zd_result"), interface::RESULT_MEMBER),
                then: block(vec![cpp::Statement::Return(Some(cpp::Expr::Deref(
                    Box::new(dot(name("zd_result"), interface::RESULT_MEMBER)),
                )))]),
                otherwise: None,
            },
        ];
        for (_, member) in &enhanced.exceptions {
            statements.push(cpp::Statement::If {
                condition: dot(name("zd_result"), member),
                then: block(vec![cpp::Statement::Throw(cpp::Expr::Deref(Box::new(
                    dot(name("zd_result"), member),
                )))]),
                otherwise: None,
            });
        }
        statements.push(throw_error(
            "RemoteUnknownExceptionError",
            "the reply carries neither a result nor an exception that the operation declares",
        ));
        return Ok(static_method(
            const_ref(enhanced.out_type.clone()),
            &format!("result_{operation_name}"),
            vec![param(const_ref(named(&reply_type)), "zd_reply")],
            statements,
            "The outcome of one call, §7.5.1.2.3.",
        ));
    }
    let result_type = interface::result_union_name(service, &operation_name);
    let result_member = dot(
        dot(name("zd_reply"), interface::DATA_MEMBER),
        &operation_name,
    );
    let mut statements = vec![
        cpp::Statement::If {
            condition: binary(
                "!=",
                dot(dot(name("zd_reply"), interface::DATA_MEMBER), "_d"),
                name(&interface::operation_hash_constant(
                    service,
                    &operation_name,
                )),
            ),
            then: block(vec![throw_error(
                "RemoteUnknownOperationError",
                "the reply answers another operation than the request",
            )]),
            otherwise: None,
        },
        local(
            const_ref(named(&result_type)),
            "zd_result",
            Some(result_member),
        ),
        cpp::Statement::If {
            condition: binary(
                "==",
                dot(name("zd_result"), "_d"),
                path(&["dds", "rpc", "RETCODE_OK"]),
            ),
            then: block(vec![cpp::Statement::Return(Some(dot(
                name("zd_result"),
                interface::RESULT_MEMBER,
            )))]),
            otherwise: None,
        },
    ];
    for raised in &operation.raises {
        let exception = cxx_escape(&raised.name);
        statements.push(cpp::Statement::If {
            condition: binary(
                "==",
                dot(name("zd_result"), "_d"),
                name(&interface::exception_hash_constant(&exception)),
            ),
            then: block(vec![cpp::Statement::Throw(dot(
                name("zd_result"),
                &interface::exception_member_name(&raised.name),
            ))]),
            otherwise: None,
        });
    }
    statements.push(throw_error(
        "RemoteUnknownExceptionError",
        "the reply carries an exception that the interface does not declare for this operation",
    ));
    Ok(static_method(
        const_ref(native_out_type(ast, home.module, service, operation)?),
        &format!("result_{operation_name}"),
        vec![param(const_ref(named(&reply_type)), "zd_reply")],
        statements,
        "The outcome of one call, §7.5.1.1.5.",
    ))
}

/// `result_<op>(zd_reply)`.
fn result_call(operation: &Operation) -> cpp::Expr {
    call(
        name(&format!("result_{}", cxx_escape(&operation.name))),
        vec![name("zd_reply")],
    )
}

/// `static R unpack_<op>(const Reply& zd_reply)`: what the `future` of the
/// asynchronous operation (§7.11.1.1.2 rule 7.h and 7.i) holds.
fn unpack_method(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<cpp::ClassMember, BindingError> {
    let operation_name = cxx_escape(&operation.name);
    let reply_type = interface::reply_type_name(interface_name);
    let mut payload = async_payload(ast, service, interface_name, operation)?;
    if let Some(enhanced) = shape.op(operation)?
        && operation.has_outputs()
    {
        payload = enhanced.application_out_type.clone();
    }
    if operation.has_outputs() && is_psm() {
        // The application's `Out` (Table 7.1's spellings) is built from the
        // native one the reply holds.
        let (native_type, application_type) = match shape.op(operation)? {
            Some(enhanced) => (
                enhanced.out_type.clone(),
                enhanced.application_out_type.clone(),
            ),
            None => (
                native_out_type(ast, service.module, interface_name, operation)?,
                named(&interface::out_struct_name(interface_name, &operation_name)),
            ),
        };
        let mut statements = vec![
            local(
                const_ref(native_type),
                "zd_native",
                Some(result_call(operation)),
            ),
            local(
                application_type.clone(),
                "zd_out",
                Some(zero_init_of(&application_type)),
            ),
        ];
        for parameter in operation.outputs() {
            let member = param_name(parameter);
            statements.push(assign(
                dot(name("zd_out"), &member),
                converted(
                    ast,
                    parameter.type_,
                    &at(service, operation, &parameter.name),
                    dot(name("zd_native"), &member),
                    false,
                )?,
            ));
        }
        if let Some(target) = operation.returns {
            let member = return_member(operation);
            statements.push(assign(
                dot(name("zd_out"), &member),
                converted(
                    ast,
                    target,
                    &at(service, operation, "return"),
                    dot(name("zd_native"), &member),
                    false,
                )?,
            ));
        }
        statements.push(cpp::Statement::Return(Some(name("zd_out"))));
        return Ok(static_method(
            payload,
            &format!("unpack_{operation_name}"),
            vec![param(const_ref(named(&reply_type)), "zd_reply")],
            statements,
            "What the future of the asynchronous call holds.",
        ));
    }
    let statement = if operation.has_outputs() {
        cpp::Statement::Return(Some(result_call(operation)))
    } else if let Some(target) = operation.returns {
        cpp::Statement::Return(Some(converted(
            ast,
            target,
            &at(service, operation, "return"),
            dot(result_call(operation), &return_member(operation)),
            false,
        )?))
    } else {
        cpp::Statement::Expr(result_call(operation))
    };
    Ok(static_method(
        payload,
        &format!("unpack_{operation_name}"),
        vec![param(const_ref(named(&reply_type)), "zd_reply")],
        vec![statement],
        "What the future of the asynchronous call holds.",
    ))
}

/// The statements of a synchronous call.
fn sync_body(
    ast: &Ast,
    declaring: &Service,
    interface_name: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<Vec<cpp::Statement>, BindingError> {
    let reply_type = interface::reply_type_name(interface_name);
    let mut statements = pack_request(ast, declaring, interface_name, operation, shape)?;
    statements.push(local(
        cpp::Type::Const(Box::new(named(&reply_type))),
        "zd_reply",
        Some(method_call(
            method_call(name("requester_"), "call", vec![name("zd_request")]),
            "data",
            Vec::new(),
        )),
    ));
    let outputs: Vec<&interface::Parameter> = operation.outputs().collect();
    if outputs.is_empty() && operation.returns.is_none() {
        statements.push(cpp::Statement::Expr(result_call(operation)));
        return Ok(statements);
    }
    let out_type = match shape.op(operation)? {
        Some(enhanced) => enhanced.out_type.clone(),
        None => native_out_type(ast, declaring.module, interface_name, operation)?,
    };
    statements.push(local(
        const_ref(out_type),
        "zd_out",
        Some(result_call(operation)),
    ));
    for parameter in outputs {
        let member = param_name(parameter);
        statements.push(assign(
            name(&member),
            converted(
                ast,
                parameter.type_,
                &at(declaring, operation, &parameter.name),
                dot(name("zd_out"), &member),
                false,
            )?,
        ));
    }
    if let Some(target) = operation.returns {
        let member = return_member(operation);
        statements.push(cpp::Statement::Return(Some(converted(
            ast,
            target,
            &at(declaring, operation, "return"),
            dot(name("zd_out"), &member),
            false,
        )?)));
    }
    Ok(statements)
}

/// A synchronous operation the client overrides.
fn sync_client_method(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<cpp::ClassMember, BindingError> {
    let mut method = sync_method(ast, service, operation)?;
    method.is_virtual = false;
    method.is_pure = false;
    method.is_override = true;
    method.doc = Doc::line("Calls the service. DDS-RPC 1.0 §7.9.1.");
    let returns_constructed = operation
        .returns
        .is_some_and(|target| !is_native(ast, target));
    let mut statements = sync_body(ast, service, interface_name, operation, shape)?;
    if returns_constructed {
        // Rule 6.h: a constructed return value comes back through the
        // leading `cxx_return` parameter and the function returns `void`.
        if let Some(cpp::Statement::Return(Some(value))) = statements.last().cloned() {
            statements.pop();
            statements.push(assign(name("cxx_return"), value));
        }
    }
    method.body = Some(block(statements));
    Ok(cpp::ClassMember::Method(method))
}

/// An asynchronous operation the client overrides.
fn async_client_method(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<cpp::ClassMember, BindingError> {
    let mut method = async_method(ast, service, interface_name, operation)?;
    method.is_virtual = false;
    method.is_pure = false;
    method.is_override = true;
    method.doc = Doc::line("Calls the service and returns the future of its reply. §7.9.1.");
    if let Some(enhanced) = shape.op(operation)?
        && operation.has_outputs()
    {
        // The `Out` structure of an inherited operation is in the module of
        // the interface that declares it.
        method.returns = cpp::Type::template(
            Path::new(&["dds", "rpc", "future"]),
            vec![enhanced.application_out_type.clone()],
        );
    }
    method.body = Some(block(async_body(
        ast,
        service,
        interface_name,
        operation,
        shape,
    )?));
    Ok(cpp::ClassMember::Method(method))
}

fn async_body(
    ast: &Ast,
    declaring: &Service,
    interface_name: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<Vec<cpp::Statement>, BindingError> {
    let operation_name = cxx_escape(&operation.name);
    let mut statements = pack_request(ast, declaring, interface_name, operation, shape)?;
    statements.push(cpp::Statement::Return(Some(method_call(
        name("requester_"),
        "send_request_mapped",
        vec![
            name("zd_request"),
            cpp::Expr::AddressOf(Box::new(name(&format!("unpack_{operation_name}")))),
        ],
    ))));
    Ok(statements)
}

/// The operation an attribute's getter or setter is implied as.
fn implied<'a>(
    operations: &'a [Operation],
    prefix: &str,
    attribute: &str,
) -> Option<&'a Operation> {
    let wanted = format!("{prefix}{attribute}");
    operations.iter().find(|operation| operation.name == wanted)
}

/// The accessors and the cache of one attribute (DDS-Cxx-PSM §7.4.6).
#[allow(clippy::too_many_arguments)]
fn attribute_members(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    client_class: &str,
    operations: &[Operation],
    attribute: &interface::Attribute,
    cache: &mut Vec<cpp::ClassMember>,
    shape: Shape<'_>,
) -> Result<Vec<cpp::ClassMember>, BindingError> {
    let at_text = format!("{}::{}", service.name, attribute.name);
    let type_ = user_type(ast, attribute.type_, &at_text)?;
    let spelled = cxx_escape(&attribute.name);
    let native = is_native(ast, attribute.type_);
    let getter = implied(operations, "get_attribute_", &attribute.name);
    let setter = implied(operations, "set_attribute_", &attribute.name);
    let mut out = Vec::new();
    let method =
        |returns: cpp::Type, method_name: &str, params: Vec<cpp::Param>, is_const, body| {
            cpp::ClassMember::Method(cpp::Method {
                doc: Doc::line("An attribute accessor. DDS-Cxx-PSM 1.0 §7.4.6."),
                returns,
                name: Ident::new(method_name),
                params,
                is_const,
                is_static: false,
                is_virtual: false,
                is_pure: false,
                is_constexpr: false,
                is_noexcept: false,
                is_override: true,
                body: Some(block(body)),
            })
        };
    if let Some(getter) = getter {
        let mut statements = sync_body(ast, service, interface_name, getter, shape)?;
        if native {
            out.push(method(
                type_.clone(),
                &spelled,
                Vec::new(),
                false,
                statements,
            ));
        } else {
            // `CT& attribute()` hands out a reference, so the value of the
            // last call lives in the client.
            let cached = format!("cached_{spelled}_");
            if let Some(cpp::Statement::Return(Some(value))) = statements.last().cloned() {
                statements.pop();
                statements.push(assign(name(&cached), value));
                statements.push(cpp::Statement::Return(Some(name(&cached))));
            }
            cache.push(cpp::ClassMember::Field(cpp::Field {
                doc: Doc::line("The value `attribute()` returned last."),
                type_: type_.clone(),
                name: Ident::new(&cached),
                dims: Vec::new(),
                default: None,
                is_static: false,
                is_constexpr: false,
                bits: None,
            }));
            out.push(method(
                cpp::Type::Ref(Box::new(type_.clone())),
                &spelled,
                Vec::new(),
                false,
                statements,
            ));
            // `const CT& attribute() const` asks the service as well.
            out.push(method(
                const_ref(type_.clone()),
                &spelled,
                Vec::new(),
                true,
                vec![cpp::Statement::Return(Some(call(
                    cpp::Expr::Arrow {
                        value: Box::new(cpp::Expr::Paren(Box::new(cpp::Expr::Cast {
                            kind: cpp::CastKind::Const,
                            type_: Box::new(cpp::Type::Pointer(Box::new(named(client_class)))),
                            value: Box::new(name("this")),
                        }))),
                        name: Ident::new(&spelled),
                    },
                    Vec::new(),
                )))],
            ));
        }
    }
    if let Some(setter) = setter {
        let passed = if native {
            type_.clone()
        } else {
            const_ref(type_.clone())
        };
        out.push(method(
            cpp::Type::Void,
            &spelled,
            vec![param(passed, &spelled)],
            false,
            sync_body(ast, service, interface_name, setter, shape)?,
        ));
    }
    Ok(out)
}

/// The asynchronous accessors of one attribute, rules 7.f and 7.g.
fn attribute_async_members(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operations: &[Operation],
    attribute: &interface::Attribute,
    shape: Shape<'_>,
) -> Result<Vec<cpp::ClassMember>, BindingError> {
    let at_text = format!("{}::{}", service.name, attribute.name);
    // The future holds the value as the topic type does; the argument of the
    // setter is what the application passes.
    let payload = user_type(ast, attribute.type_, &at_text)?;
    let type_ = user_type(ast, attribute.type_, &at_text)?;
    let spelled = cxx_escape(&attribute.name);
    let native = is_native(ast, attribute.type_);
    let method = |returns: cpp::Type, params: Vec<cpp::Param>, body| {
        cpp::ClassMember::Method(cpp::Method {
            doc: Doc::line("An attribute accessor. DDS-RPC 1.0 §7.11.1.1.2 rules 7.f and 7.g."),
            returns,
            name: Ident::new(&format!("{spelled}_async")),
            params,
            is_const: false,
            is_static: false,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: true,
            body: Some(block(body)),
        })
    };
    let future_of = |payload: cpp::Type| {
        cpp::Type::template(Path::new(&["dds", "rpc", "future"]), vec![payload])
    };
    let mut out = Vec::new();
    if let Some(getter) = implied(operations, "get_attribute_", &attribute.name) {
        out.push(method(
            future_of(payload),
            Vec::new(),
            async_body(ast, service, interface_name, getter, shape)?,
        ));
    }
    if let Some(setter) = implied(operations, "set_attribute_", &attribute.name) {
        let passed = if native {
            cpp::Type::Const(Box::new(type_))
        } else {
            const_ref(type_)
        };
        out.push(method(
            future_of(cpp::Type::Void),
            vec![param(passed, &spelled)],
            async_body(ast, service, interface_name, setter, shape)?,
        ));
    }
    Ok(out)
}

// -- the bases ---------------------------------------------------------

/// One direct base of an interface: the member that holds its endpoint,
/// the base itself, and everything a class that derives from it has to
/// implement.
///
/// §7.5.1.1.8: "each interface in the hierarchy shall be mapped to its own
/// pair of request and reply topics", so a client of a derived interface
/// reaches the operations of a base through the base's own client, and a
/// service of a derived interface answers on the base's topics through the
/// base's own service.
#[derive(Clone)]
struct BaseMember {
    /// The data member that holds the base's client or service.
    member: String,
    /// The C++ type of the base's client or service.
    client: String,
    service_type: String,
    /// The operations and attributes visible through the base, each once,
    /// with the interface that declares it.
    operations: Vec<(Service, Operation)>,
    attributes: Vec<(Service, interface::Attribute)>,
}

fn base_members(ast: &Ast, service: &Service) -> Result<Vec<BaseMember>, BindingError> {
    let mut seen: Vec<zerodds_ast_core::NodeId> = Vec::new();
    let mut out = Vec::new();
    for (index, base) in service.bases.iter().enumerate() {
        let base_service = interface::service(ast, base.id)?;
        let mut operations = Vec::new();
        let mut attributes = Vec::new();
        visible(
            ast,
            &base_service,
            &mut seen,
            &mut operations,
            &mut attributes,
            0,
        )?;
        let module = base_service.module;
        let simple = cxx_escape(&base_service.name);
        let scope = |suffix: &str| super::codec::scoped(ast, module, &format!("{simple}{suffix}"));
        out.push(BaseMember {
            member: format!("zd_base_{index}_"),
            client: scope("Client")?,
            service_type: scope("Service")?,
            operations,
            attributes,
        });
    }
    Ok(out)
}

/// Everything `base` declares and its own bases declare, each once.
///
/// zerodds-lint: recursion-depth 64 (one level per base interface; the
/// seam refuses a chain past its inheritance limit)
fn visible(
    ast: &Ast,
    base: &Service,
    seen: &mut Vec<zerodds_ast_core::NodeId>,
    operations: &mut Vec<(Service, Operation)>,
    attributes: &mut Vec<(Service, interface::Attribute)>,
    depth: usize,
) -> Result<(), BindingError> {
    if depth > 64 {
        return Err(BindingError::Codec(
            "the inheritance chain runs past 64 levels".to_string(),
        ));
    }
    for operation in &base.operations {
        if let Some(id) = operation.node_id {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
        }
        operations.push((base.clone(), operation.clone()));
    }
    for attribute in &base.attributes {
        if seen.contains(&attribute.node_id) {
            continue;
        }
        seen.push(attribute.node_id);
        attributes.push((base.clone(), attribute.clone()));
    }
    for ancestor in &base.bases {
        let ancestor = interface::service(ast, ancestor.id)?;
        visible(ast, &ancestor, seen, operations, attributes, depth + 1)?;
    }
    Ok(())
}

/// `<member>.<method>(args)`.
fn forward(member: &str, method_name: &str, args: Vec<cpp::Expr>) -> cpp::Expr {
    method_call(name(member), method_name, args)
}

/// A synchronous operation of a base, forwarded to the base's client.
fn forward_sync(
    ast: &Ast,
    declaring: &Service,
    operation: &Operation,
    member: &str,
) -> Result<cpp::ClassMember, BindingError> {
    let mut method = sync_method(ast, declaring, operation)?;
    method.is_virtual = false;
    method.is_pure = false;
    method.is_override = true;
    method.doc = Doc::line("Forwarded to the client of the base interface.");
    let returns_constructed = operation
        .returns
        .is_some_and(|target| !is_native(ast, target));
    let mut args = Vec::new();
    if returns_constructed {
        args.push(name("cxx_return"));
    }
    for parameter in &operation.params {
        args.push(name(&param_name(parameter)));
    }
    let invoke = forward(member, &cxx_escape(&operation.name), args);
    let statement = if operation.returns.is_some() && !returns_constructed {
        cpp::Statement::Return(Some(invoke))
    } else {
        cpp::Statement::Expr(invoke)
    };
    method.body = Some(block(vec![statement]));
    Ok(cpp::ClassMember::Method(method))
}

/// An asynchronous operation of a base, forwarded.
fn forward_async(
    ast: &Ast,
    declaring: &Service,
    operation: &Operation,
    member: &str,
) -> Result<cpp::ClassMember, BindingError> {
    let declaring_name = cxx_escape(&declaring.name);
    let mut method = async_method(ast, declaring, &declaring_name, operation)?;
    method.is_virtual = false;
    method.is_pure = false;
    method.is_override = true;
    method.doc = Doc::line("Forwarded to the client of the base interface.");
    let args = operation.inputs().map(|p| name(&param_name(p))).collect();
    method.body = Some(block(vec![cpp::Statement::Return(Some(forward(
        member,
        &format!("{}_async", cxx_escape(&operation.name)),
        args,
    )))]));
    Ok(cpp::ClassMember::Method(method))
}

/// The accessors of an attribute of a base, forwarded, both the
/// synchronous and the asynchronous ones.
fn forward_attribute(
    ast: &Ast,
    declaring: &Service,
    attribute: &interface::Attribute,
    member: &str,
) -> Result<Vec<cpp::ClassMember>, BindingError> {
    let at_text = format!("{}::{}", declaring.name, attribute.name);
    let payload = user_type(ast, attribute.type_, &at_text)?;
    let type_ = user_type(ast, attribute.type_, &at_text)?;
    let spelled = cxx_escape(&attribute.name);
    let native = is_native(ast, attribute.type_);
    let accessor = |returns: cpp::Type,
                    accessor_name: &str,
                    params: Vec<cpp::Param>,
                    is_const: bool,
                    statement: cpp::Statement| {
        cpp::ClassMember::Method(cpp::Method {
            doc: Doc::line("Forwarded to the client of the base interface."),
            returns,
            name: Ident::new(accessor_name),
            params,
            is_const,
            is_static: false,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: true,
            body: Some(block(vec![statement])),
        })
    };
    let get = || cpp::Statement::Return(Some(forward(member, &spelled, Vec::new())));
    let set = || cpp::Statement::Expr(forward(member, &spelled, vec![name(&spelled)]));
    let mut out = Vec::new();
    if native {
        out.push(accessor(type_.clone(), &spelled, Vec::new(), false, get()));
    } else {
        out.push(accessor(
            cpp::Type::Ref(Box::new(type_.clone())),
            &spelled,
            Vec::new(),
            false,
            get(),
        ));
        out.push(accessor(
            const_ref(type_.clone()),
            &spelled,
            Vec::new(),
            true,
            get(),
        ));
    }
    let passed = if native {
        type_.clone()
    } else {
        const_ref(type_.clone())
    };
    if !attribute.readonly {
        out.push(accessor(
            cpp::Type::Void,
            &spelled,
            vec![param(passed, &spelled)],
            false,
            set(),
        ));
    }
    let async_name = format!("{spelled}_async");
    let future_of = |payload: cpp::Type| {
        cpp::Type::template(Path::new(&["dds", "rpc", "future"]), vec![payload])
    };
    out.push(accessor(
        future_of(payload),
        &async_name,
        Vec::new(),
        false,
        cpp::Statement::Return(Some(forward(member, &async_name, Vec::new()))),
    ));
    if !attribute.readonly {
        let async_passed = if native {
            cpp::Type::Const(Box::new(type_))
        } else {
            const_ref(type_)
        };
        out.push(accessor(
            future_of(cpp::Type::Void),
            &async_name,
            vec![param(async_passed, &spelled)],
            false,
            cpp::Statement::Return(Some(forward(member, &async_name, vec![name(&spelled)]))),
        ));
    }
    Ok(out)
}

/// The member initialisers of a client's constructor over `params`.
fn client_initialisers(
    support: &str,
    bases: &[BaseMember],
    shape: Shape<'_>,
) -> Vec<(Ident, cpp::Expr)> {
    // §7.8.2: the Enhanced profile correlates by sample identity and the
    // sample is PL_CDR2; the runtime is told so by the params.
    let params_of = if shape.is_enhanced() {
        "enhanced_requester_params_of"
    } else {
        "requester_params_of"
    };
    let mut out = vec![(
        Ident::new("requester_"),
        call(
            path(&["dds", "rpc", params_of]),
            vec![
                name("params"),
                call(path(&[support, "defaults"]), Vec::new()),
            ],
        ),
    )];
    for base in bases {
        out.push((Ident::new(&base.member), name("params")));
    }
    out
}

/// The body of a client's constructor: the endpoint takes the requester
/// and the clients of the bases.
fn client_attachments(bases: &[BaseMember]) -> Vec<cpp::Statement> {
    let mut out = vec![cpp::Statement::Expr(call(
        name("attach"),
        vec![name("requester_")],
    ))];
    for base in bases {
        out.push(cpp::Statement::Expr(call(
            name("attach_base"),
            vec![name(&base.member)],
        )));
    }
    out
}

/// `class IClient` — §7.11.1.5.4.
///
/// "Every `Client` inherits publicly from `${interface}`,
/// `${interface}Async`, and `ClientEndpoint` (public virtual)."
pub(super) fn client_class(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operations: &[Operation],
    shape: Shape<'_>,
) -> Result<cpp::Class, BindingError> {
    let class_name = format!("{interface_name}Client");
    let visible_bases = base_members(ast, service)?;
    // §7.5.1.2.6: under the Enhanced mapping the request and the reply of a
    // derived interface hold the operations of its bases, so the client
    // answers them itself and holds no client of a base.
    let base_list = if shape.is_enhanced() {
        Vec::new()
    } else {
        visible_bases.clone()
    };
    let requester_type = format!("{interface_name}Requester");
    let support = format!("{interface_name}Support");
    let mut public = vec![
        alias("InterfaceType", interface_name),
        alias("AsyncInterfaceType", &format!("{interface_name}Async")),
        alias("RequesterType", &requester_type),
        cpp::ClassMember::Constructor(cpp::Constructor {
            doc: Doc::line(
                "A client on the runtime's participant, bound to the interface's two topics.",
            ),
            name: Ident::new(&class_name),
            params: vec![param(
                cpp::Type::Ref(Box::new(qualified(RPC, "RPCRuntime"))),
                "runtime",
            )],
            initialisers: vec![(
                Ident::new(&class_name),
                method_call(name("runtime"), "client_params", Vec::new()),
            )],
            is_explicit: true,
            is_constexpr: false,
            is_noexcept: false,
            body: cpp::FunctionBody::Block(block(Vec::new())),
        }),
        cpp::ClassMember::Constructor(cpp::Constructor {
            doc: Doc::line(
                "A client over `params`. §7.9.1: creating it creates the underlying DataWriter and DataReader and makes them discoverable; closing it, or destroying the last copy of it, deletes them (§7.9.3).",
            ),
            name: Ident::new(&class_name),
            params: vec![param(const_ref(qualified(RPC, "ClientParams")), "params")],
            initialisers: client_initialisers(&support, &base_list, shape),
            is_explicit: true,
            is_constexpr: false,
            is_noexcept: false,
            body: cpp::FunctionBody::Block(block(client_attachments(&base_list))),
        }),
    ];
    let mut private = Vec::new();
    let mut cache = Vec::new();
    let base_members = &base_list;
    // The operations and the attributes of the bases that this client
    // answers itself, each with the interface that declares it.
    let inherited_operations: Vec<&(Service, Operation)> = visible_bases
        .iter()
        .flat_map(|base| base.operations.iter())
        .filter(|_| shape.is_enhanced())
        .collect();
    let inherited_attributes: Vec<&(Service, interface::Attribute)> = visible_bases
        .iter()
        .flat_map(|base| base.attributes.iter())
        .filter(|_| shape.is_enhanced())
        .collect();
    for operation in &service.operations {
        public.push(sync_client_method(
            ast,
            service,
            interface_name,
            operation,
            shape,
        )?);
    }
    for (declaring, operation) in &inherited_operations {
        public.push(sync_client_method(
            ast,
            declaring,
            interface_name,
            operation,
            shape,
        )?);
    }
    for base in base_members {
        for (declaring, operation) in &base.operations {
            public.push(forward_sync(ast, declaring, operation, &base.member)?);
        }
        for (declaring, attribute) in &base.attributes {
            public.extend(forward_attribute(ast, declaring, attribute, &base.member)?);
        }
    }
    for base in base_members {
        for (declaring, operation) in &base.operations {
            public.push(forward_async(ast, declaring, operation, &base.member)?);
        }
    }
    for attribute in &service.attributes {
        public.extend(attribute_members(
            ast,
            service,
            interface_name,
            &class_name,
            operations,
            attribute,
            &mut cache,
            shape,
        )?);
    }
    for (declaring, attribute) in &inherited_attributes {
        public.extend(attribute_members(
            ast,
            declaring,
            interface_name,
            &class_name,
            &declaring.all_operations(),
            attribute,
            &mut cache,
            shape,
        )?);
    }
    for operation in &service.operations {
        public.push(async_client_method(
            ast,
            service,
            interface_name,
            operation,
            shape,
        )?);
    }
    for (declaring, operation) in &inherited_operations {
        public.push(async_client_method(
            ast,
            declaring,
            interface_name,
            operation,
            shape,
        )?);
    }
    for attribute in &service.attributes {
        public.extend(attribute_async_members(
            ast,
            service,
            interface_name,
            operations,
            attribute,
            shape,
        )?);
    }
    for (declaring, attribute) in &inherited_attributes {
        public.extend(attribute_async_members(
            ast,
            declaring,
            interface_name,
            &declaring.all_operations(),
            attribute,
            shape,
        )?);
    }
    for operation in operations {
        private.push(result_accessor(
            ast,
            service,
            interface_name,
            operation,
            shape,
        )?);
        private.push(unpack_method(
            ast,
            service,
            interface_name,
            operation,
            shape,
        )?);
    }
    private.push(cpp::ClassMember::Field(cpp::Field {
        doc: Doc::line("The requester every call goes through."),
        type_: named(&requester_type),
        name: Ident::new("requester_"),
        dims: Vec::new(),
        default: None,
        is_static: false,
        is_constexpr: false,
        bits: None,
    }));
    for base in &base_list {
        private.push(cpp::ClassMember::Field(cpp::Field {
            doc: Doc::line("The client of a base interface, on the base's own topics. §7.5.1.1.8."),
            type_: named(&base.client),
            name: Ident::new(&base.member),
            dims: Vec::new(),
            default: None,
            is_static: false,
            is_constexpr: false,
            bits: None,
        }));
    }
    private.extend(cache);
    Ok(cpp::Class {
        doc: Doc::line("The function-call client. DDS-RPC 1.0 §7.11.1.5.4."),
        is_struct: false,
        name: Ident::new(&class_name),
        bases: vec![
            cpp::Base {
                access: cpp::Access::Public,
                virtual_: false,
                path: Path::single(interface_name),
            },
            cpp::Base {
                access: cpp::Access::Public,
                virtual_: false,
                path: Path::single(&format!("{interface_name}Async")),
            },
            cpp::Base {
                access: cpp::Access::Public,
                virtual_: true,
                path: Path::new(&["dds", "rpc", "Client"]),
            },
        ],
        sections: vec![
            cpp::Section {
                access: cpp::Access::Public,
                members: public,
            },
            private_section(private),
        ],
    })
}

/// `void serve_<op>(const Request&, Reply&)`: calls the implementation and
/// fills the case of the reply.
fn serve_method(
    ast: &Ast,
    here: Id<ModuleNode>,
    interface_name: &str,
    operation: &Operation,
    shape: Shape<'_>,
) -> Result<cpp::ClassMember, BindingError> {
    let operation_name = cxx_escape(&operation.name);
    let request_type = interface::request_type_name(interface_name);
    let reply_type = interface::reply_type_name(interface_name);
    let at_of = |what: &str| format!("{interface_name}::{}({what})", operation.name);
    let enhanced = shape.op(operation)?;
    let (in_type, out_type) = match enhanced {
        Some(op) => (op.in_type.clone(), op.out_type.clone()),
        None => (
            named(&interface::in_struct_name(interface_name, &operation_name)),
            native_out_type(ast, here, interface_name, operation)?,
        ),
    };
    let reply_case = dot(
        dot(name("zd_reply"), interface::DATA_MEMBER),
        &operation_name,
    );
    // The place in the reply that holds `member` of the operation's case:
    // a member of the `Result` union, or of the `Result` structure the
    // operation's optional member holds.
    let reply_slot = |member: &str| -> cpp::Expr {
        if enhanced.is_some() {
            cpp::Expr::Arrow {
                value: Box::new(dot(name("zd_reply"), &operation_name)),
                name: Ident::new(member),
            }
        } else {
            dot(reply_case.clone(), member)
        }
    };

    let mut outer = Vec::new();
    let has_inputs = operation.inputs().next().is_some();
    if has_inputs {
        let request_in = if enhanced.is_some() {
            cpp::Expr::Deref(Box::new(dot(name("zd_request"), &operation_name)))
        } else {
            dot(
                dot(name("zd_request"), interface::DATA_MEMBER),
                &operation_name,
            )
        };
        outer.push(local(const_ref(in_type), "zd_in", Some(request_in)));
    }
    if enhanced.is_some() {
        // §7.5.1.2.5: the reply holds the operation's `Result`, and that
        // holds the `Out` or one exception.
        outer.push(cpp::Statement::Expr(method_call(
            dot(name("zd_reply"), &operation_name),
            "emplace",
            Vec::new(),
        )));
    } else {
        outer.push(assign(
            dot(dot(name("zd_reply"), interface::DATA_MEMBER), "_d"),
            name(&interface::operation_hash_constant(
                interface_name,
                &operation_name,
            )),
        ));
    }

    // The guarded part: the call and the result.
    let mut guarded = vec![local(
        out_type.clone(),
        "zd_out",
        Some(zero_init_of(&out_type)),
    )];
    // Under the PSM mapping the implementation takes the PSM's types: an
    // argument is converted from the topic structure, and an `out` or `inout`
    // one is a local of the PSM's type that is converted back after the call.
    let psm = is_psm();
    let mut after_call = Vec::new();
    if !psm {
        for parameter in operation
            .params
            .iter()
            .filter(|p| matches!(p.direction, zerodds_ast::ParamDirection::InOut))
        {
            let member = param_name(parameter);
            guarded.push(assign(
                dot(name("zd_out"), &member),
                dot(name("zd_in"), &member),
            ));
        }
    }
    let mut arguments = Vec::new();
    let mut native_return = false;
    if let Some(target) = operation.returns {
        // Rule 6.g and 6.h: a native return value is returned, a
        // constructed one is the leading `cxx_return` argument. An
        // attribute getter has no such argument: DDS-Cxx-PSM §7.4.6 gives
        // it `T attribute()` or `T& attribute()`, and the value is what
        // that returns.
        native_return = is_native(ast, target) || operation.origin == Origin::AttributeGetter;
        if !native_return {
            if psm {
                let returned = user_type(ast, target, &at_of("return"))?;
                guarded.push(local(
                    returned.clone(),
                    "zd_arg_return",
                    Some(zero_init_of(&returned)),
                ));
                arguments.push(name("zd_arg_return"));
                after_call.push(assign(
                    dot(name("zd_out"), &return_member(operation)),
                    converted(ast, target, &at_of("return"), name("zd_arg_return"), true)?,
                ));
            } else {
                arguments.push(dot(name("zd_out"), &return_member(operation)));
            }
        }
    }
    for parameter in &operation.params {
        let member = param_name(parameter);
        if !psm {
            if parameter.is_output() {
                arguments.push(dot(name("zd_out"), &member));
            } else {
                arguments.push(dot(name("zd_in"), &member));
            }
            continue;
        }
        let from_request = converted(
            ast,
            parameter.type_,
            &at_of(&parameter.name),
            dot(name("zd_in"), &member),
            false,
        )?;
        if !parameter.is_output() {
            arguments.push(from_request);
            continue;
        }
        let spelled = user_type(ast, parameter.type_, &at_of(&parameter.name))?;
        let local_name = format!("zd_arg_{member}");
        let initial = if matches!(parameter.direction, zerodds_ast::ParamDirection::InOut) {
            from_request
        } else {
            zero_init_of(&spelled)
        };
        guarded.push(local(spelled, &local_name, Some(initial)));
        arguments.push(name(&local_name));
        after_call.push(assign(
            dot(name("zd_out"), &member),
            converted(
                ast,
                parameter.type_,
                &at_of(&parameter.name),
                name(&local_name),
                true,
            )?,
        ));
    }
    // DDS-Cxx-PSM §7.4.6: the attribute's own name, not the implied
    // operation's (§7.5.1.1.3).
    let implemented = match operation.origin {
        Origin::AttributeGetter => cxx_escape(operation.name.trim_start_matches("get_attribute_")),
        Origin::AttributeSetter => cxx_escape(operation.name.trim_start_matches("set_attribute_")),
        _ => operation_name.clone(),
    };
    let implementation_call = call(
        cpp::Expr::Arrow {
            value: Box::new(name("implementation_")),
            name: Ident::new(&implemented),
        },
        arguments,
    );
    if native_return {
        let target = operation.returns.ok_or_else(|| {
            BindingError::Codec("a native return value of an operation that returns nothing".into())
        })?;
        guarded.push(assign(
            dot(name("zd_out"), &return_member(operation)),
            converted(ast, target, &at_of("return"), implementation_call, true)?,
        ));
    } else {
        guarded.push(cpp::Statement::Expr(implementation_call));
    }
    guarded.extend(after_call);
    guarded.push(assign(reply_slot(interface::RESULT_MEMBER), name("zd_out")));
    if enhanced.is_none() {
        guarded.push(assign(
            dot(reply_case.clone(), "_d"),
            path(&["dds", "rpc", "RETCODE_OK"]),
        ));
    }

    if operation.raises.is_empty() {
        outer.extend(guarded);
    } else {
        // §7.5.1.1.5, §7.5.1.2.3: an exception the operation raises is the
        // case of the `Result` that carries it.
        let mut handlers = Vec::new();
        for raised in &operation.raises {
            let exception = cxx_escape(&raised.name);
            let mut body = Vec::new();
            match enhanced {
                Some(op) => {
                    let member = op
                        .exceptions
                        .iter()
                        .find(|(simple, _)| simple == &raised.name)
                        .map(|(_, member)| member.clone())
                        .ok_or_else(|| {
                            BindingError::Codec(format!(
                                "`{}` raises `{}`, which its Result structure has no member for",
                                operation.name, raised.name
                            ))
                        })?;
                    body.push(assign(reply_slot(&member), name("zd_error")));
                }
                None => {
                    body.push(assign(
                        dot(reply_case.clone(), "_d"),
                        name(&interface::exception_hash_constant(&exception)),
                    ));
                    body.push(assign(
                        dot(
                            reply_case.clone(),
                            &interface::exception_member_name(&raised.name),
                        ),
                        name("zd_error"),
                    ));
                }
            }
            // The exception may be declared in another module.
            let exception_module = ast
                .exception(raised.id)
                .map_err(|error| BindingError::Codec(format!("an exception is gone: {error:?}")))?
                .module;
            handlers.push(cpp::CatchClause {
                type_: Some(super::enhanced::type_in(
                    ast,
                    here,
                    exception_module,
                    &exception,
                )?),
                binding: Some(Ident::new("zd_error")),
                body: block(body),
            });
        }
        outer.push(cpp::Statement::Try {
            body: block(guarded),
            handlers,
        });
    }
    Ok(cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line("Calls the implementation and fills the reply. DDS-RPC 1.0 §7.9.2.1."),
        returns: cpp::Type::Void,
        name: Ident::new(&format!("serve_{operation_name}")),
        params: vec![
            cpp::Param {
                type_: const_ref(named(&request_type)),
                // Unnamed where there is nothing to read from it.
                name: has_inputs.then(|| Ident::new("zd_request")),
                default: None,
            },
            param(cpp::Type::Ref(Box::new(named(&reply_type))), "zd_reply"),
        ],
        is_const: false,
        is_static: false,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(block(outer)),
    }))
}

/// `T{}` for the type a structure is spelled as.
fn zero_init_of(type_: &cpp::Type) -> cpp::Expr {
    cpp::Expr::Construct {
        type_: Box::new(type_.clone()),
        args: Vec::new(),
        braced: true,
    }
}

/// The member initialisers of a service's constructor: the implementation,
/// the replier and the name from `params_expr`, and each base's service
/// from `base_source` (the runtime or the params) and the implementation.
fn service_initialisers(
    support: &str,
    bases: &[BaseMember],
    params_expr: cpp::Expr,
    base_source: cpp::Expr,
    shape: Shape<'_>,
) -> Vec<(Ident, cpp::Expr)> {
    let defaults = || call(path(&[support, "defaults"]), Vec::new());
    // §7.8.2: the Enhanced profile correlates by sample identity and the
    // sample is PL_CDR2; the runtime is told so by the params.
    let params_of = if shape.is_enhanced() {
        "enhanced_replier_params_of"
    } else {
        "replier_params_of"
    };
    let mut out = vec![
        (
            Ident::new("implementation_"),
            cpp::Expr::AddressOf(Box::new(name("implementation"))),
        ),
        (
            Ident::new("replier_"),
            call(
                path(&["dds", "rpc", params_of]),
                vec![params_expr.clone(), defaults()],
            ),
        ),
        (
            Ident::new("service_name_"),
            call(
                path(&["dds", "rpc", "service_name_of"]),
                vec![params_expr, defaults()],
            ),
        ),
    ];
    for base in bases {
        out.push((
            Ident::new(&base.member),
            call(
                name(&base.service_type),
                vec![name("implementation"), base_source.clone()],
            ),
        ));
    }
    out
}

/// The body of a service's constructor: the endpoint takes the replier
/// and the services of the bases.
fn service_attachments(bases: &[BaseMember]) -> Vec<cpp::Statement> {
    let mut out = vec![cpp::Statement::Expr(call(
        name("attach"),
        vec![name("replier_")],
    ))];
    for base in bases {
        out.push(cpp::Statement::Expr(call(
            name("attach"),
            vec![name(&base.member)],
        )));
    }
    out
}

/// `class IService` — §7.11.1.5.1.
///
/// "`Service` is a reference type and accepts a reference to the service
/// implementation. A `Service` instantiates the underlying DDS entities
/// and makes the service discoverable. Every `Service` inherits from
/// `ServiceEndpoint`." — through `dds::rpc::Service`, which a `Server`
/// takes.
pub(super) fn service_class(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operations: &[Operation],
    shape: Shape<'_>,
) -> Result<cpp::Class, BindingError> {
    // §7.5.1.2.6: under the Enhanced mapping the request of a derived
    // interface holds the operations of its bases, so the service answers
    // them itself on its own topics and holds no service of a base.
    let base_list = if shape.is_enhanced() {
        Vec::new()
    } else {
        base_members(ast, service)?
    };
    let here_module = service.module;
    let class_name = format!("{interface_name}Service");
    let replier_type = format!("{interface_name}Replier");
    let support = format!("{interface_name}Support");
    let request_type = interface::request_type_name(interface_name);
    let reply_type = interface::reply_type_name(interface_name);

    let implementation_param = param(
        cpp::Type::Ref(Box::new(named(interface_name))),
        "implementation",
    );
    let public = vec![
        alias("InterfaceType", interface_name),
        alias("ReplierType", &replier_type),
        cpp::ClassMember::Constructor(cpp::Constructor {
            doc: Doc::line("A service over an implementation, on the runtime's participant."),
            name: Ident::new(&class_name),
            params: vec![
                implementation_param.clone(),
                param(
                    cpp::Type::Ref(Box::new(qualified(RPC, "RPCRuntime"))),
                    "runtime",
                ),
            ],
            initialisers: service_initialisers(
                &support,
                &base_list,
                method_call(name("runtime"), "service_params", Vec::new()),
                name("runtime"),
                shape,
            ),
            is_explicit: false,
            is_constexpr: false,
            is_noexcept: false,
            body: cpp::FunctionBody::Block(block(service_attachments(&base_list))),
        }),
        cpp::ClassMember::Constructor(cpp::Constructor {
            doc: Doc::line(
                "A service over an implementation and `params`. §7.9.1: creating it creates the underlying DataReader and DataWriter and activates the service; closing it, or destroying the last copy of it, deletes them (§7.9.3).",
            ),
            name: Ident::new(&class_name),
            params: vec![
                implementation_param,
                param(const_ref(qualified(RPC, "ServiceParams")), "params"),
            ],
            initialisers: service_initialisers(
                &support,
                &base_list,
                name("params"),
                name("params"),
                shape,
            ),
            is_explicit: false,
            is_constexpr: false,
            is_noexcept: false,
            body: cpp::FunctionBody::Block(block(service_attachments(&base_list))),
        }),
        dispatch_method(&base_list, &reply_type),
        cpp::ClassMember::Method(cpp::Method {
            doc: Doc::line("The name the service is discoverable under. §7.4.1."),
            returns: const_ref(qualified("std", "string")),
            name: Ident::new("service_name"),
            params: Vec::new(),
            is_const: true,
            is_static: false,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: true,
            body: Some(block(vec![cpp::Statement::Return(Some(name(
                "service_name_",
            )))])),
        }),
    ];
    let mut private = vec![answer_method(
        interface_name,
        operations,
        &request_type,
        &reply_type,
        shape,
    )];
    for operation in operations {
        private.push(serve_method(
            ast,
            here_module,
            interface_name,
            operation,
            shape,
        )?);
    }
    for (field_name, field_type) in [
        (
            "implementation_",
            cpp::Type::Pointer(Box::new(named(interface_name))),
        ),
        ("replier_", named(&replier_type)),
        ("service_name_", qualified("std", "string")),
    ]
    .into_iter()
    .chain(
        base_list
            .iter()
            .map(|base| (base.member.as_str(), named(&base.service_type))),
    ) {
        private.push(cpp::ClassMember::Field(cpp::Field {
            doc: Doc::none(),
            type_: field_type,
            name: Ident::new(field_name),
            dims: Vec::new(),
            default: None,
            is_static: false,
            is_constexpr: false,
            bits: None,
        }));
    }
    Ok(cpp::Class {
        doc: Doc::line("The function-call service. DDS-RPC 1.0 §7.11.1.5.1."),
        is_struct: false,
        name: Ident::new(&class_name),
        bases: vec![cpp::Base {
            access: cpp::Access::Public,
            virtual_: false,
            path: Path::new(&["dds", "rpc", "Service"]),
        }],
        sections: vec![
            cpp::Section {
                access: cpp::Access::Public,
                members: public,
            },
            private_section(private),
        ],
    })
}

fn size_t_type() -> cpp::Type {
    qualified("std", "size_t")
}

/// `std::size_t dispatch() override`: take the requests, answer them.
fn dispatch_method(bases: &[BaseMember], reply_type: &str) -> cpp::ClassMember {
    let size = qualified("std", "size_t");
    let loop_body = vec![
        local(named(reply_type), "zd_reply", Some(zero_init(reply_type))),
        local(
            qualified(RPC, "RemoteExceptionCode_t"),
            "zd_code",
            Some(path(&["dds", "rpc", "REMOTE_EX_OK"])),
        ),
        local(
            cpp::Type::Primitive(cpp::Primitive::Bool),
            "zd_expected",
            Some(cpp::Expr::Literal(Literal::Bool(true))),
        ),
        local(qualified("std", "exception_ptr"), "zd_error", None),
        cpp::Statement::Try {
            body: block(vec![assign(
                name("zd_reply"),
                call(
                    name("answer"),
                    vec![
                        method_call(name("zd_request"), "data", Vec::new()),
                        name("zd_expected"),
                    ],
                ),
            )]),
            handlers: vec![cpp::CatchClause {
                type_: None,
                binding: None,
                body: block(vec![
                    assign(
                        name("zd_error"),
                        call(path(&["std", "current_exception"]), Vec::new()),
                    ),
                    assign(
                        name("zd_code"),
                        call(
                            path(&["dds", "rpc", "detail", "remote_exception_code_of"]),
                            vec![name("zd_error")],
                        ),
                    ),
                ]),
            }],
        },
        // A `oneway` call has no reply to carry what it threw, so it is
        // thrown at the caller of `dispatch()`.
        cpp::Statement::If {
            condition: name("zd_expected"),
            then: block(vec![cpp::Statement::Expr(method_call(
                name("replier_"),
                "send_reply",
                vec![
                    name("zd_reply"),
                    dot(
                        method_call(name("zd_request"), "info", Vec::new()),
                        "identity",
                    ),
                    name("zd_code"),
                ],
            ))]),
            otherwise: Some(block(vec![cpp::Statement::If {
                condition: name("zd_error"),
                then: block(vec![cpp::Statement::Expr(call(
                    path(&["std", "rethrow_exception"]),
                    vec![name("zd_error")],
                ))]),
                otherwise: None,
            }])),
        },
        cpp::Statement::Expr(binary(
            "+=",
            name("zd_answered"),
            cpp::Expr::Literal(Literal::int(1)),
        )),
    ];
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line(
            "Takes the requests that have arrived and answers them; the number answered. A `Server` calls it. §7.9.2.1.",
        ),
        returns: size.clone(),
        name: Ident::new("dispatch"),
        params: Vec::new(),
        is_const: false,
        is_static: false,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: true,
        body: Some(block(
            vec![
                local(
                    size,
                    "zd_answered",
                    Some(cpp::Expr::Literal(Literal::int(0))),
                ),
                local(
                    cpp::Type::Auto,
                    "zd_requests",
                    Some(method_call(
                        name("replier_"),
                        "receive_requests",
                        Vec::new(),
                    )),
                ),
                // By index: the iterator of a `LoanedSamples` yields a
                // `SampleRef` by value (§7.11.1.4.21), which a `for (auto&
                // ..)` cannot bind.
                cpp::Statement::For {
                    init: Box::new(cpp::Statement::Declare {
                        type_: size_t_type(),
                        name: Ident::new("zd_at"),
                        value: Some(cpp::Expr::Literal(Literal::int(0))),
                    }),
                    condition: binary(
                        "<",
                        name("zd_at"),
                        method_call(
                            method_call(name("zd_requests"), "data_seq", Vec::new()),
                            "size",
                            Vec::new(),
                        ),
                    ),
                    step: binary(
                        "=",
                        name("zd_at"),
                        binary("+", name("zd_at"), cpp::Expr::Literal(Literal::int(1))),
                    ),
                    body: block(
                        std::iter::once(local(
                            cpp::Type::Auto,
                            "zd_request",
                            Some(cpp::Expr::Index {
                                value: Box::new(name("zd_requests")),
                                index: Box::new(name("zd_at")),
                            }),
                        ))
                        .chain(loop_body)
                        .collect(),
                    ),
                },
            ]
            .into_iter()
            // §7.5.1.1.8: the topics of each base are served by the base's
            // own service.
            .chain(bases.iter().map(|base| {
                cpp::Statement::Expr(binary(
                    "+=",
                    name("zd_answered"),
                    method_call(name(&base.member), "dispatch", Vec::new()),
                ))
            }))
            .chain(std::iter::once(cpp::Statement::Return(Some(name(
                "zd_answered",
            )))))
            .collect(),
        )),
    })
}

/// `Reply answer(const Request&)`: the case of the `Call` union selects the
/// operation. A case the interface does not have is an operation the
/// service does not know (§7.7.1.1, §7.5.2).
fn answer_method(
    interface_name: &str,
    operations: &[Operation],
    request_type: &str,
    reply_type: &str,
    shape: Shape<'_>,
) -> cpp::ClassMember {
    if shape.is_enhanced() {
        return answer_member(operations, request_type, reply_type);
    }
    let mut cases = Vec::new();
    for operation in operations {
        let operation_name = cxx_escape(&operation.name);
        let mut body = Vec::new();
        body.push(cpp::Statement::Expr(call(
            name(&format!("serve_{operation_name}")),
            vec![name("zd_request"), name("zd_reply")],
        )));
        cases.push(cpp::SwitchCase {
            labels: vec![name(&interface::operation_hash_constant(
                interface_name,
                &operation_name,
            ))],
            body: block(body),
            breaks: true,
        });
    }
    cases.push(cpp::SwitchCase {
        labels: Vec::new(),
        // §7.7.1.1: the Basic mapping answers an operation it does not
        // have with REMOTE_EX_UNSUPPORTED (the Enhanced mapping with
        // REMOTE_EX_UNKNOWN_OPERATION, §7.7.2).
        body: block(vec![throw_error(
            "RemoteUnsupportedError",
            "the interface has no operation with the case of this request",
        )]),
        breaks: false,
    });
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line("Answers one request. DDS-RPC 1.0 §7.5.1.1.6."),
        returns: named(reply_type),
        name: Ident::new("answer"),
        params: vec![
            param(const_ref(named(request_type)), "zd_request"),
            cpp::Param {
                type_: cpp::Type::Ref(Box::new(cpp::Type::Primitive(cpp::Primitive::Bool))),
                // Every operation has a reply (§7.5.1.1.7 rule 5): nothing to say.
                name: None,
                default: None,
            },
        ],
        is_const: false,
        is_static: false,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(block(vec![
            local(named(reply_type), "zd_reply", Some(zero_init(reply_type))),
            cpp::Statement::Switch {
                value: dot(dot(name("zd_request"), interface::DATA_MEMBER), "_d"),
                cases,
            },
            cpp::Statement::Return(Some(name("zd_reply"))),
        ])),
    })
}

/// `Reply answer(const Request&)` under the Enhanced mapping: the member of
/// the request that is present selects the operation (§7.5.1.2.4). A request
/// with no member this interface has, one for an operation it does not know,
/// is an operation the service does not know (§7.7.2).
fn answer_member(
    operations: &[Operation],
    request_type: &str,
    reply_type: &str,
) -> cpp::ClassMember {
    let mut arms: Vec<(cpp::Expr, cpp::Block)> = Vec::new();
    for operation in operations {
        let operation_name = cxx_escape(&operation.name);
        let mut body = Vec::new();
        body.push(cpp::Statement::Expr(call(
            name(&format!("serve_{operation_name}")),
            vec![name("zd_request"), name("zd_reply")],
        )));
        arms.push((dot(name("zd_request"), &operation_name), block(body)));
    }
    // One `if` per operation, each returning: a chain of `else` would nest
    // as deep as there are operations, which a compiler limits.
    let mut dispatch: Vec<cpp::Statement> = arms
        .into_iter()
        .map(|(condition, mut then)| {
            then.statements
                .push(cpp::Statement::Return(Some(name("zd_reply"))));
            cpp::Statement::If {
                condition,
                then,
                otherwise: None,
            }
        })
        .collect();
    dispatch.push(throw_error(
        "RemoteUnknownOperationError",
        "the request holds no operation of this interface",
    ));
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line("Answers one request. DDS-RPC 1.0 §7.5.1.2.4."),
        returns: named(reply_type),
        name: Ident::new("answer"),
        params: vec![
            param(const_ref(named(request_type)), "zd_request"),
            cpp::Param {
                type_: cpp::Type::Ref(Box::new(cpp::Type::Primitive(cpp::Primitive::Bool))),
                // Every operation has a reply (§7.5.1.1.7 rule 5): nothing to say.
                name: None,
                default: None,
            },
        ],
        is_const: false,
        is_static: false,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(block({
            let mut body = vec![local(
                named(reply_type),
                "zd_reply",
                Some(zero_init(reply_type)),
            )];
            body.extend(dispatch);
            body
        })),
    })
}
