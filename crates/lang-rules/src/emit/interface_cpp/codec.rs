// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The octets of the topic types, for the C++ binding of DDS-RPC 1.0.
//!
//! `dds::rpc::Requester` and `dds::rpc::Replier` write the header of
//! §7.5.1.1.1 themselves and ask `dds_type_traits<${interface}_Request>`
//! and `dds_type_traits<${interface}_Reply>` for the rest: the `Call` and
//! `Return` union (§7.5.1.1.6, §7.5.1.1.7). This writes those two
//! specialisations and the functions under them: one encoder and one
//! decoder for each synthesized structure and union (§7.5.1.1.4-.7), which
//! write the discriminator and call the codec of each member's type.
//!
//! The codecs of the member types — a parameter's, a return value's, an
//! exception's member's — are the ones the shared codec walk generates for
//! a declared type (`crate::emit::encode`, `crate::decode_cpp`); they are
//! generated here for the types the interface reaches and no others, each
//! once however many operations name it.
//!
//! The walk's key is the one the Rust column uses (`PlainCdr2`,
//! little-endian): for the `@final` structures of the Basic mapping the
//! forms differ in the alignment cap only, and `zerodds::wire::Writer`
//! takes that as a run-time argument — eight under XCDR1, four under XCDR2
//! (`dds::rpc::WireEncoding`).

use super::{Operation, Service, cxx_escape, qualified, return_member_name};
use crate::adapter::cpp::cxx_name;
use crate::adapter::interface::{self, BindingError};
use crate::emit::cpp::Cpp;
use zerodds_ast::{Ast, ModuleNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp;

fn key() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// The key of classic CDR (XCDR1), which the Basic mapping uses when the
/// params say [`WireEncoding::XCDR1`](https://www.omg.org/spec/DDS-RPC/1.0):
/// no DHEADER for an `@appendable` member, a parameter list for a
/// `@mutable` one (XTypes 1.3 §7.4.1.1, §7.4.1.2).
fn key_xcdr1() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
}

/// The namespace the XCDR1 copy of the codecs of a Basic-mapping file is
/// in; its functions have the names of the XCDR2 ones.
pub(super) const XCDR1_NAMESPACE: &str = "zd_xcdr1";

/// `decls` in [`XCDR1_NAMESPACE`], at the global scope.
pub(super) fn in_xcdr1_namespace(decls: Vec<cpp::Decl>) -> cpp::Decl {
    cpp::Decl::Namespace(cpp::Namespace {
        doc: Doc::line(
            "The same codecs under classic CDR (XCDR1), which the Basic mapping writes when \
             the params say so. DDS-RPC 1.0 §7.5.1.1, XTypes 1.3 §7.4.1.",
        ),
        name: Ident::new(XCDR1_NAMESPACE),
        decls,
    })
}

/// `zd_xcdr1::<function>`.
fn xcdr1_function(function: &str) -> cpp::Expr {
    path(&[XCDR1_NAMESPACE, function])
}

pub(super) fn name(text: &str) -> cpp::Expr {
    cpp::Expr::Path(Path::single(text))
}

/// The function a synthesized codec calls beside itself, by its name in
/// parentheses.
///
/// The codecs stand once at file scope and once in [`XCDR1_NAMESPACE`] with
/// the same names, over types declared at file scope: an unqualified call
/// is also looked up by the namespaces of its arguments (ADL), finds the
/// file-scope function as well as the one beside it, and is ambiguous. A
/// parenthesised name is looked up as an ordinary name only, which finds
/// the nearest.
pub(super) fn sibling(function: &str) -> cpp::Expr {
    cpp::Expr::Paren(Box::new(name(function)))
}

pub(super) fn path(segments: &[&str]) -> cpp::Expr {
    cpp::Expr::Path(Path::new(segments))
}

pub(super) fn dot(base: cpp::Expr, member: &str) -> cpp::Expr {
    cpp::Expr::Member {
        value: Box::new(base),
        name: Ident::new(member),
    }
}

pub(super) fn method(receiver: cpp::Expr, method_name: &str, args: Vec<cpp::Expr>) -> cpp::Expr {
    cpp::Expr::MethodCall {
        receiver: Box::new(receiver),
        name: Ident::new(method_name),
        args,
    }
}

pub(super) fn call(callee: cpp::Expr, args: Vec<cpp::Expr>) -> cpp::Expr {
    cpp::Expr::Call {
        callee: Box::new(callee),
        args,
    }
}

pub(super) fn binary(op: &'static str, lhs: cpp::Expr, rhs: cpp::Expr) -> cpp::Expr {
    cpp::Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

pub(super) fn block(statements: Vec<cpp::Statement>) -> cpp::Block {
    cpp::Block { statements }
}

pub(super) fn expr_statement(expr: cpp::Expr) -> cpp::Statement {
    cpp::Statement::Expr(expr)
}

pub(super) fn assign(lhs: cpp::Expr, rhs: cpp::Expr) -> cpp::Statement {
    cpp::Statement::Expr(binary("=", lhs, rhs))
}

/// `static_cast<void>(variable);`
pub(super) fn unused(variable: &str) -> cpp::Statement {
    expr_statement(cpp::Expr::Cast {
        kind: cpp::CastKind::Static,
        type_: Box::new(cpp::Type::Void),
        value: Box::new(name(variable)),
    })
}

/// `body`, ahead of which neither parameter is reported unused.
pub(super) fn with_unused(statements: Vec<cpp::Statement>, stream: &str) -> Vec<cpp::Statement> {
    let mut out = vec![unused(stream), unused("v")];
    out.extend(statements);
    out
}

pub(super) fn writer_type() -> cpp::Type {
    cpp::Type::Ref(Box::new(cpp::Type::named(Path::new(&[
        "zerodds", "wire", "Writer",
    ]))))
}

pub(super) fn reader_type() -> cpp::Type {
    cpp::Type::Ref(Box::new(cpp::Type::named(Path::new(&[
        "zerodds", "wire", "Reader",
    ]))))
}

pub(super) fn const_ref(type_: cpp::Type) -> cpp::Type {
    cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(type_))))
}

pub(super) fn mut_ref(type_: cpp::Type) -> cpp::Type {
    cpp::Type::Ref(Box::new(type_))
}

pub(super) fn param(type_: cpp::Type, param_name: &str) -> cpp::Param {
    cpp::Param {
        type_,
        name: Some(Ident::new(param_name)),
        default: None,
    }
}

pub(super) fn function(
    fn_name: &str,
    params: Vec<cpp::Param>,
    statements: Vec<cpp::Statement>,
    doc: &str,
) -> cpp::Decl {
    cpp::Decl::Function(cpp::Function {
        doc: Doc::line(doc),
        returns: cpp::Type::Void,
        name: Ident::new(fn_name),
        params,
        is_inline: false,
        is_static: true,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: true,
        body: cpp::FunctionBody::Block(block(statements)),
    })
}

/// `::a::b::Name` — the C++ spelling the adapter gives a declaration in
/// `module`.
pub(super) fn scoped(
    ast: &Ast,
    module: Id<ModuleNode>,
    simple: &str,
) -> Result<String, BindingError> {
    let mut chain = Vec::new();
    let mut current = Some(module);
    let mut guard = 0;
    while let Some(id) = current {
        guard += 1;
        if guard > 64 {
            return Err(BindingError::Codec(
                "the module chain runs past 64 levels".to_string(),
            ));
        }
        let node = ast
            .module(id)
            .map_err(|error| BindingError::Codec(format!("a module is gone: {error:?}")))?;
        if !node.name.is_empty() {
            chain.push(crate::adapter::cpp::cxx_name(&node.name));
        }
        current = node.parent;
    }
    chain.reverse();
    let mut out = String::new();
    // Under `--cpp-mapping psm` the topic structures are the native types,
    // which stand in `zd_native` (DDS-PSM-Cxx 1.0 §8.1).
    if crate::adapter::cpp::cpp_mapping() == crate::adapter::cpp::CppMapping::Psm {
        out.push_str("::");
        out.push_str(crate::adapter::cpp::NATIVE_NAMESPACE);
    }
    for segment in chain {
        out.push_str("::");
        out.push_str(&segment);
    }
    out.push_str("::");
    out.push_str(simple);
    Ok(out)
}

/// `a::b::Name`, the IDL spelling of the scope, which is the DDS type
/// name (DDS-RPC 1.0 §7.5.1.1.6 rule 1, "in the same module as the
/// interface").
pub(super) fn dds_type_name(
    ast: &Ast,
    module: Id<ModuleNode>,
    simple: &str,
) -> Result<String, BindingError> {
    let mut chain = Vec::new();
    let mut current = Some(module);
    let mut guard = 0;
    while let Some(id) = current {
        guard += 1;
        if guard > 64 {
            return Err(BindingError::Codec(
                "the module chain runs past 64 levels".to_string(),
            ));
        }
        let node = ast
            .module(id)
            .map_err(|error| BindingError::Codec(format!("a module is gone: {error:?}")))?;
        if !node.name.is_empty() {
            chain.push(node.name.clone());
        }
        current = node.parent;
    }
    chain.reverse();
    chain.push(simple.to_string());
    Ok(chain.join("::"))
}

fn encoder_of(ast: &Ast, id: Id<TypeNode>) -> String {
    crate::emit::encode::codec_name::<Cpp>(ast, id, key().purpose)
}

fn decoder_of(ast: &Ast, id: Id<TypeNode>) -> String {
    crate::decode_cpp::decoder_name(ast, id)
}

pub(super) fn discard() -> cpp::Expr {
    path(&["zerodds", "wire", "TryConstruct", "Discard"])
}

/// `zd_enc_<type>(w, value)`.
pub(super) fn encode_member(ast: &Ast, id: Id<TypeNode>, value: cpp::Expr) -> cpp::Statement {
    expr_statement(call(name(&encoder_of(ast, id)), vec![name("w"), value]))
}

/// `zd_dec_<type>(r, target, Discard)`.
pub(super) fn decode_member(ast: &Ast, id: Id<TypeNode>, target: cpp::Expr) -> cpp::Statement {
    expr_statement(call(
        name(&decoder_of(ast, id)),
        vec![name("r"), target, discard()],
    ))
}

/// The member types of the interface's operations and the exceptions they
/// raise, each once.
pub(super) fn member_types(
    ast: &Ast,
    services: &[Service],
) -> Result<Vec<Id<TypeNode>>, BindingError> {
    let mut out: Vec<Id<TypeNode>> = Vec::new();
    let mut add = |id: Id<TypeNode>| {
        if !out.contains(&id) {
            out.push(id);
        }
    };
    for service in services {
        for operation in service.all_operations() {
            for parameter in &operation.params {
                add(parameter.type_);
            }
            if let Some(returns) = operation.returns {
                add(returns);
            }
            for raised in &operation.raises {
                for member in exception_members(ast, raised)? {
                    add(member.1);
                }
            }
        }
    }
    Ok(out)
}

/// An exception's members: the name the adapter gives the field, and its
/// type.
pub(super) fn exception_members(
    ast: &Ast,
    raised: &interface::Raise,
) -> Result<Vec<(String, Id<TypeNode>)>, BindingError> {
    let node = ast
        .exception(raised.id)
        .map_err(|error| BindingError::Codec(format!("an exception is gone: {error:?}")))?;
    let mut out = Vec::new();
    for member in &node.members {
        let member = ast
            .member(*member)
            .map_err(|error| BindingError::Codec(format!("a member is gone: {error:?}")))?;
        let type_ = member.type_.resolved().ok_or_else(|| {
            BindingError::Codec(format!(
                "member `{}` of exception `{}` names no type",
                member.name, raised.name
            ))
        })?;
        out.push((cxx_name(&member.name), type_));
    }
    Ok(out)
}

/// The codec functions of the types the interfaces' operations carry:
/// what the shared walk generates for a declared type, once for the file.
pub(super) fn member_codecs(
    ast: &Ast,
    services: &[Service],
) -> Result<Vec<cpp::Decl>, BindingError> {
    member_codecs_in(ast, services, key())
}

/// [`member_codecs`] under classic CDR, for [`XCDR1_NAMESPACE`].
pub(super) fn member_codecs_xcdr1(
    ast: &Ast,
    services: &[Service],
) -> Result<Vec<cpp::Decl>, BindingError> {
    member_codecs_in(ast, services, key_xcdr1())
}

fn member_codecs_in(
    ast: &Ast,
    services: &[Service],
    key: LayoutKey,
) -> Result<Vec<cpp::Decl>, BindingError> {
    let roots = member_types(ast, services)?;
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let options = crate::emit::encode::CodecOptions::default();
    let mut out = crate::emit::encode::codec_from_many::<Cpp>(ast, &roots, key, options)
        .map_err(|error| BindingError::Codec(error.to_string()))?;
    out.extend(
        crate::decode_cpp::cpp_decoder_from_many(ast, &roots, key, options)
            .map_err(|error| BindingError::Codec(error.to_string()))?,
    );
    Ok(out)
}

fn codec_suffix(service: &Service, local: &str) -> String {
    format!("rpc_{}_{local}", service.qualified_name)
}

pub(super) fn enc_name(service: &Service, local: &str) -> String {
    format!("zd_enc_{}", codec_suffix(service, local))
}

pub(super) fn dec_name(service: &Service, local: &str) -> String {
    format!("zd_dec_{}", codec_suffix(service, local))
}

fn return_member(operation: &Operation) -> String {
    let taken: Vec<String> = operation
        .outputs()
        .map(|parameter| cxx_escape(&parameter.name))
        .collect();
    return_member_name(&taken)
}

/// The members of an `In` structure: the name and the type, or the
/// `dummy` octet of §7.5.1.1.4 rule 3c.
enum Field {
    Typed(String, Id<TypeNode>),
    Dummy,
}

fn in_fields(operation: &Operation) -> Vec<Field> {
    let mut out: Vec<Field> = operation
        .inputs()
        .map(|parameter| Field::Typed(cxx_escape(&parameter.name), parameter.type_))
        .collect();
    if out.is_empty() {
        out.push(Field::Dummy);
    }
    out
}

fn out_fields(operation: &Operation) -> Vec<Field> {
    let mut out: Vec<Field> = operation
        .outputs()
        .map(|parameter| Field::Typed(cxx_escape(&parameter.name), parameter.type_))
        .collect();
    if let Some(returns) = operation.returns {
        out.push(Field::Typed(return_member(operation), returns));
    }
    if out.is_empty() {
        out.push(Field::Dummy);
    }
    out
}

fn struct_codecs(
    ast: &Ast,
    service: &Service,
    local: &str,
    type_name: &str,
    fields: &[Field],
) -> Vec<cpp::Decl> {
    let mut encode = Vec::new();
    let mut decode = Vec::new();
    for field in fields {
        match field {
            Field::Typed(member, id) => {
                encode.push(encode_member(ast, *id, dot(name("v"), member)));
                decode.push(decode_member(ast, *id, dot(name("v"), member)));
            }
            Field::Dummy => {
                encode.push(expr_statement(method(
                    name("w"),
                    "write_u8",
                    vec![dot(name("v"), "dummy")],
                )));
                decode.push(assign(
                    dot(name("v"), "dummy"),
                    method(name("r"), "read_u8", Vec::new()),
                ));
            }
        }
    }
    vec![
        function(
            &enc_name(service, local),
            vec![
                param(writer_type(), "w"),
                param(const_ref(cpp::Type::named(Path::single(type_name))), "v"),
            ],
            with_unused(encode, "w"),
            "The octets of one topic structure. DDS-RPC 1.0 §7.5.1.1.",
        ),
        function(
            &dec_name(service, local),
            vec![
                param(reader_type(), "r"),
                param(mut_ref(cpp::Type::named(Path::single(type_name))), "v"),
            ],
            with_unused(decode, "r"),
            "One topic structure from its octets. DDS-RPC 1.0 §7.5.1.1.",
        ),
    ]
}

/// `if (a) {..} else if (b) {..} else {..}`.
pub(super) fn if_chain(mut arms: Vec<(cpp::Expr, cpp::Block)>, last: cpp::Block) -> cpp::Statement {
    let Some((condition, then)) = arms.first().cloned() else {
        return cpp::Statement::If {
            condition: cpp::Expr::Literal(Literal::Bool(true)),
            then: last,
            otherwise: None,
        };
    };
    arms.remove(0);
    let otherwise = if arms.is_empty() {
        last
    } else {
        block(vec![if_chain(arms, last)])
    };
    cpp::Statement::If {
        condition,
        then,
        otherwise: Some(otherwise),
    }
}

pub(super) fn status_refuse() -> cpp::Statement {
    expr_statement(method(
        name("r"),
        "reader_refuse",
        vec![path(&["zerodds", "wire", "Status", "OutOfRange"])],
    ))
}

/// Everything the specialisations stand on: the structures' codecs, the
/// exceptions' and the unions'.
pub(super) fn synthesized_codecs(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
    operations: &[Operation],
) -> Result<Vec<cpp::Decl>, BindingError> {
    let scope = |simple: &str| scoped(ast, service.module, simple);
    let mut out = Vec::new();

    // The exceptions, once each however many operations raise them.
    let mut seen_exceptions: Vec<String> = Vec::new();
    for operation in operations {
        for raised in &operation.raises {
            if seen_exceptions.contains(&raised.name) {
                continue;
            }
            seen_exceptions.push(raised.name.clone());
            let exception = cxx_escape(&raised.name);
            // Declared in the module of the exception, which need not be
            // the interface's.
            let exception_module = ast
                .exception(raised.id)
                .map_err(|error| BindingError::Codec(format!("an exception is gone: {error:?}")))?
                .module;
            let type_name = scoped(ast, exception_module, &exception)?;
            let members = exception_members(ast, raised)?;
            let fields: Vec<Field> = members
                .into_iter()
                .map(|(member, id)| Field::Typed(member, id))
                .collect();
            // An exception without members has nothing on the wire.
            let local = format!("ex_{exception}");
            let mut encode = Vec::new();
            let mut decode = Vec::new();
            for field in &fields {
                if let Field::Typed(member, id) = field {
                    encode.push(encode_member(ast, *id, dot(name("v"), member)));
                    decode.push(decode_member(ast, *id, dot(name("v"), member)));
                }
            }
            out.push(function(
                &enc_name(service, &local),
                vec![
                    param(writer_type(), "w"),
                    param(const_ref(cpp::Type::named(Path::single(&type_name))), "v"),
                ],
                with_unused(encode, "w"),
                "The octets of one exception. DDS-RPC 1.0 §7.5.1.1.5.",
            ));
            out.push(function(
                &dec_name(service, &local),
                vec![
                    param(reader_type(), "r"),
                    param(mut_ref(cpp::Type::named(Path::single(&type_name))), "v"),
                ],
                with_unused(decode, "r"),
                "One exception from its octets. DDS-RPC 1.0 §7.5.1.1.5.",
            ));
        }
    }

    // The `In`, `Out` and `Result` of each operation.
    for operation in operations {
        let operation_name = cxx_escape(&operation.name);
        let in_local = format!("{operation_name}_In");
        let in_type = scope(&interface::in_struct_name(interface_name, &operation_name))?;
        out.extend(struct_codecs(
            ast,
            service,
            &in_local,
            &in_type,
            &in_fields(operation),
        ));
        let out_local = format!("{operation_name}_Out");
        let out_type = scope(&interface::out_struct_name(interface_name, &operation_name))?;
        out.extend(struct_codecs(
            ast,
            service,
            &out_local,
            &out_type,
            &out_fields(operation),
        ));

        // The `Result` union: the discriminator, then the case it names.
        let result_local = format!("{operation_name}_Result");
        let result_type = scope(&interface::result_union_name(
            interface_name,
            &operation_name,
        ))?;
        let mut encode_arms = vec![(
            binary(
                "==",
                dot(name("v"), "_d"),
                path(&["dds", "rpc", "RETCODE_OK"]),
            ),
            block(vec![expr_statement(call(
                sibling(&enc_name(service, &out_local)),
                vec![name("w"), dot(name("v"), interface::RESULT_MEMBER)],
            ))]),
        )];
        let mut decode_arms = vec![(
            binary(
                "==",
                dot(name("v"), "_d"),
                path(&["dds", "rpc", "RETCODE_OK"]),
            ),
            block(vec![expr_statement(call(
                sibling(&dec_name(service, &out_local)),
                vec![name("r"), dot(name("v"), interface::RESULT_MEMBER)],
            ))]),
        )];
        for raised in &operation.raises {
            let exception = cxx_escape(&raised.name);
            let hash = scope(&interface::exception_hash_constant(&exception))?;
            let member = interface::exception_member_name(&raised.name);
            let ex_local = format!("ex_{exception}");
            encode_arms.push((
                binary("==", dot(name("v"), "_d"), name(&hash)),
                block(vec![expr_statement(call(
                    sibling(&enc_name(service, &ex_local)),
                    vec![name("w"), dot(name("v"), &member)],
                ))]),
            ));
            decode_arms.push((
                binary("==", dot(name("v"), "_d"), name(&hash)),
                block(vec![expr_statement(call(
                    sibling(&dec_name(service, &ex_local)),
                    vec![name("r"), dot(name("v"), &member)],
                ))]),
            ));
        }
        out.push(function(
            &enc_name(service, &result_local),
            vec![
                param(writer_type(), "w"),
                param(const_ref(cpp::Type::named(Path::single(&result_type))), "v"),
            ],
            with_unused(
                vec![
                    expr_statement(method(name("w"), "write_i32", vec![dot(name("v"), "_d")])),
                    if_chain(encode_arms, block(Vec::new())),
                ],
                "w",
            ),
            "The octets of one result. DDS-RPC 1.0 §7.5.1.1.5.",
        ));
        out.push(function(
            &dec_name(service, &result_local),
            vec![
                param(reader_type(), "r"),
                param(mut_ref(cpp::Type::named(Path::single(&result_type))), "v"),
            ],
            with_unused(
                vec![
                    assign(
                        dot(name("v"), "_d"),
                        method(name("r"), "read_i32", Vec::new()),
                    ),
                    if_chain(decode_arms, block(vec![status_refuse()])),
                ],
                "r",
            ),
            "One result from its octets. DDS-RPC 1.0 §7.5.1.1.5.",
        ));
    }

    // The `Call` and `Return` unions.
    for reply in [false, true] {
        let union_local = if reply { "Return" } else { "Call" };
        let union_name = if reply {
            interface::return_union_name(interface_name)
        } else {
            interface::call_union_name(interface_name)
        };
        let union_type = scope(&union_name)?;
        let mut encode_arms = Vec::new();
        let mut decode_arms = Vec::new();
        for operation in operations {
            let operation_name = cxx_escape(&operation.name);
            let hash = scope(&interface::operation_hash_constant(
                interface_name,
                &operation_name,
            ))?;
            let case_local = if reply {
                format!("{operation_name}_Result")
            } else {
                format!("{operation_name}_In")
            };
            encode_arms.push((
                binary("==", dot(name("v"), "_d"), name(&hash)),
                block(vec![expr_statement(call(
                    sibling(&enc_name(service, &case_local)),
                    vec![name("w"), dot(name("v"), &operation_name)],
                ))]),
            ));
            decode_arms.push((
                binary("==", dot(name("v"), "_d"), name(&hash)),
                block(vec![expr_statement(call(
                    sibling(&dec_name(service, &case_local)),
                    vec![name("r"), dot(name("v"), &operation_name)],
                ))]),
            ));
        }
        // §7.5.1.1.6 rule 3: the `default` case holds `unknownOp`.
        out.push(function(
            &enc_name(service, union_local),
            vec![
                param(writer_type(), "w"),
                param(const_ref(cpp::Type::named(Path::single(&union_type))), "v"),
            ],
            with_unused(
                vec![
                    expr_statement(method(name("w"), "write_i32", vec![dot(name("v"), "_d")])),
                    if_chain(
                        encode_arms,
                        block(vec![expr_statement(method(
                            name("w"),
                            "write_u8",
                            vec![dot(name("v"), interface::UNKNOWN_OP_MEMBER)],
                        ))]),
                    ),
                ],
                "w",
            ),
            "The octets of one union. DDS-RPC 1.0 §7.5.1.1.6, §7.5.1.1.7.",
        ));
        out.push(function(
            &dec_name(service, union_local),
            vec![
                param(reader_type(), "r"),
                param(mut_ref(cpp::Type::named(Path::single(&union_type))), "v"),
            ],
            with_unused(
                vec![
                    assign(
                        dot(name("v"), "_d"),
                        method(name("r"), "read_i32", Vec::new()),
                    ),
                    if_chain(
                        decode_arms,
                        block(vec![assign(
                            dot(name("v"), interface::UNKNOWN_OP_MEMBER),
                            method(name("r"), "read_u8", Vec::new()),
                        )]),
                    ),
                ],
                "r",
            ),
            "One union from its octets. DDS-RPC 1.0 §7.5.1.1.6, §7.5.1.1.7.",
        ));
    }
    Ok(out)
}

/// The call of `function` under the encoding the `serialize_at` or
/// `deserialize` in scope was given: classic CDR in [`XCDR1_NAMESPACE`],
/// XCDR2 outside it.
pub(super) fn by_encoding(function: &str, args: Vec<cpp::Expr>) -> cpp::Statement {
    if_chain(
        vec![(
            binary(
                "==",
                name("encoding"),
                path(&["dds", "rpc", "WireEncoding", "XCDR1"]),
            ),
            block(vec![expr_statement(call(
                xcdr1_function(function),
                args.clone(),
            ))]),
        )],
        block(vec![expr_statement(call(name(function), args))]),
    )
}

/// `namespace dds::rpc { template <> struct dds_type_traits<T> {..}; }`
/// for the request and the reply type of one interface.
pub(super) fn traits(
    ast: &Ast,
    service: &Service,
    interface_name: &str,
) -> Result<cpp::Decl, BindingError> {
    let mut decls = Vec::new();
    for reply in [false, true] {
        let simple = if reply {
            interface::reply_type_name(interface_name)
        } else {
            interface::request_type_name(interface_name)
        };
        let union_local = if reply { "Return" } else { "Call" };
        let type_name = scoped(ast, service.module, &simple)?;
        let dds_name = dds_type_name(ast, service.module, &simple)?;
        let topic_type = cpp::Type::named(Path::single(&type_name));
        let octets = qualified("dds::rpc", "Octets");
        let encoding = qualified("dds::rpc", "WireEncoding");
        let wire = |last: &str| Path::new(&["zerodds", "wire", last]);
        let little = path(&["zerodds", "wire", "Endianness", "Little"]);
        let alignment = call(
            path(&["dds", "rpc", "max_alignment"]),
            vec![name("encoding")],
        );

        let serialize_at = cpp::Method {
            doc: Doc::line(
                "The octets of the union, counted from the start of the sample (the header \
                 before it is `offset` octets long). DDS-RPC 1.0 §7.5.1.1.",
            ),
            returns: octets.clone(),
            name: Ident::new("serialize_at"),
            params: vec![
                param(const_ref(topic_type.clone()), "value"),
                param(qualified("std", "size_t"), "offset"),
                param(encoding.clone(), "encoding"),
            ],
            is_const: false,
            is_static: true,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: false,
            body: Some(block(vec![
                cpp::Statement::Declare {
                    type_: cpp::Type::named(wire("Writer")),
                    name: Ident::new("w"),
                    value: Some(cpp::Expr::Construct {
                        type_: Box::new(cpp::Type::named(wire("Writer"))),
                        args: vec![little.clone(), alignment.clone()],
                        braced: false,
                    }),
                },
                // The alignment is counted from the start of the sample:
                // the writer starts `offset` octets in.
                cpp::Statement::For {
                    init: Box::new(cpp::Statement::Declare {
                        type_: qualified("std", "size_t"),
                        name: Ident::new("zd_i"),
                        value: Some(cpp::Expr::Literal(Literal::int(0))),
                    }),
                    condition: binary("<", name("zd_i"), name("offset")),
                    step: binary(
                        "=",
                        name("zd_i"),
                        binary("+", name("zd_i"), cpp::Expr::Literal(Literal::int(1))),
                    ),
                    body: block(vec![expr_statement(method(
                        name("w"),
                        "write_u8",
                        vec![cpp::Expr::Literal(Literal::int(0))],
                    ))]),
                },
                by_encoding(
                    &enc_name(service, union_local),
                    vec![name("w"), dot(name("value"), interface::DATA_MEMBER)],
                ),
                cpp::Statement::Declare {
                    type_: const_ref(octets.clone()),
                    name: Ident::new("zd_all"),
                    value: Some(method(name("w"), "bytes", Vec::new())),
                },
                cpp::Statement::Return(Some(cpp::Expr::Construct {
                    type_: Box::new(octets.clone()),
                    args: vec![
                        binary(
                            "+",
                            method(name("zd_all"), "begin", Vec::new()),
                            name("offset"),
                        ),
                        method(name("zd_all"), "end", Vec::new()),
                    ],
                    braced: false,
                })),
            ])),
        };
        let serialize = cpp::Method {
            doc: Doc::line("The octets of the union on its own."),
            returns: octets.clone(),
            name: Ident::new("serialize"),
            params: vec![param(const_ref(topic_type.clone()), "value")],
            is_const: false,
            is_static: true,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: false,
            body: Some(block(vec![cpp::Statement::Return(Some(call(
                name("serialize_at"),
                vec![
                    name("value"),
                    cpp::Expr::Literal(Literal::int(0)),
                    path(&["dds", "rpc", "WireEncoding", "XCDR1"]),
                ],
            )))])),
        };
        let deserialize = |with_encoding: bool| {
            let mut params = vec![
                param(const_ref(octets.clone()), "in"),
                param(mut_ref(qualified("std", "size_t")), "at"),
                param(mut_ref(topic_type.clone()), "out"),
            ];
            let mut statements = Vec::new();
            if with_encoding {
                params.push(param(encoding.clone(), "encoding"));
                statements.push(cpp::Statement::Declare {
                    type_: cpp::Type::named(wire("Reader")),
                    name: Ident::new("r"),
                    value: Some(cpp::Expr::Construct {
                        type_: Box::new(cpp::Type::named(wire("Reader"))),
                        args: vec![name("in"), little.clone(), alignment.clone()],
                        braced: false,
                    }),
                });
                statements.push(expr_statement(method(name("r"), "seek", vec![name("at")])));
                statements.push(by_encoding(
                    &dec_name(service, union_local),
                    vec![name("r"), dot(name("out"), interface::DATA_MEMBER)],
                ));
                statements.push(assign(
                    name("at"),
                    method(name("r"), "position", Vec::new()),
                ));
                statements.push(cpp::Statement::Return(Some(binary(
                    "==",
                    method(name("r"), "reader_status", Vec::new()),
                    path(&["zerodds", "wire", "Status", "Ok"]),
                ))));
            } else {
                statements.push(cpp::Statement::Return(Some(call(
                    name("deserialize"),
                    vec![
                        name("in"),
                        name("at"),
                        name("out"),
                        path(&["dds", "rpc", "WireEncoding", "XCDR1"]),
                    ],
                ))));
            }
            cpp::Method {
                doc: Doc::line(if with_encoding {
                    "The union from the octets at `at`, in the form `encoding` names."
                } else {
                    "The union from the octets at `at`, classic CDR."
                }),
                returns: cpp::Type::Primitive(cpp::Primitive::Bool),
                name: Ident::new("deserialize"),
                params,
                is_const: false,
                is_static: true,
                is_virtual: false,
                is_pure: false,
                is_constexpr: false,
                is_noexcept: false,
                is_override: false,
                body: Some(block(statements)),
            }
        };
        let type_name_method = cpp::Method {
            doc: Doc::line("The DDS type name, §7.5.1.1.6 and §7.5.1.1.7 rule 1."),
            returns: cpp::Type::Pointer(Box::new(cpp::Type::Const(Box::new(
                cpp::Type::Primitive(cpp::Primitive::Char),
            )))),
            name: Ident::new("type_name"),
            params: Vec::new(),
            is_const: false,
            is_static: true,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: false,
            body: Some(block(vec![cpp::Statement::Return(Some(
                cpp::Expr::Literal(Literal::Str(dds_name)),
            ))])),
        };
        let members = [
            type_name_method,
            serialize_at,
            serialize,
            deserialize(true),
            deserialize(false),
        ]
        .into_iter()
        .map(cpp::ClassMember::Method)
        .collect();
        decls.push(cpp::Decl::Template(cpp::Template {
            doc: Doc::none(),
            params: Vec::new(),
            decl: Box::new(cpp::Decl::Class(cpp::Class {
                doc: Doc::line("The codec of one topic type. DDS-RPC 1.0 §7.11.1.4.22."),
                is_struct: true,
                name: Ident::new("dds_type_traits"),
                // The dependent types of §7.11.1.4.22, which a
                // specialisation does not inherit from the primary template.
                bases: vec![cpp::Base {
                    access: cpp::Access::Public,
                    virtual_: false,
                    path: Path::single(&format!("dds_type_traits_base<{type_name}>")),
                }],
                sections: vec![cpp::Section {
                    access: cpp::Access::Public,
                    members,
                }],
            })),
            specialisation: vec![topic_type],
        }));
    }
    Ok(cpp::Decl::Namespace(cpp::Namespace {
        doc: Doc::none(),
        name: Ident::new("dds"),
        decls: vec![cpp::Decl::Namespace(cpp::Namespace {
            doc: Doc::none(),
            name: Ident::new("rpc"),
            decls,
        })],
    }))
}
