// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bodies of the Java function-call binding: DDS-RPC 1.0 §7.2.2.3,
//! §7.9.1, §7.11.1.5 and §7.11.2.3.
//!
//! `emit::interface_java` declares the two interfaces, the topic types and
//! the endpoints. This writes what makes them run:
//!
//! - `<I>Codec`: the octets of `${I}_Request` and `${I}_Reply` under the
//!   Basic mapping — the union of §7.5.1.1.6 and §7.5.1.1.7, the `In`,
//!   `Out` and `Result` structures, the raised exceptions — as
//!   `org.omg.dds.rpc.TypeSupport`s. Each member is written by the
//!   `ZdCodec` method the layout plan names for its type
//!   ([`crate::emit::encode::codec_name`] and
//!   [`crate::emit::decode::decoder_name`]), so the file is compiled
//!   together with the `ZdCodec` that `generate::source` writes for the
//!   same model. Exceptions are not types of the model, so their members
//!   are written here, one after the other, as the structure they are.
//! - `<I>ClientImpl`: `<I>Client` over a `Requester`. Each operation packs
//!   its request, sends it through `org.omg.dds.rpc.FunctionCallClient`,
//!   unpacks the reply and throws what the reply carries: a remote
//!   exception (§7.11.2.1) or, for a declared exception, an
//!   `org.omg.dds.rpc.UserExceptionRaised` holding the exception's data
//!   holder. A derived interface's client holds the clients of its bases
//!   (§7.5.1.1.8: "two topics for every interface in the hierarchy") and
//!   forwards their operations.
//! - `<I>ServiceImpl`: `<I>Service` over a `Replier`. The request is
//!   unpacked, the operation of the implementation is called and the
//!   result packed; `UserExceptionRaised` becomes the declared exception's
//!   case of the `Result` union, an unknown operation
//!   `REMOTE_EX_UNSUPPORTED` (§7.7.1.1) and any other failure
//!   `REMOTE_EX_UNKNOWN_EXCEPTION` (§7.5.2).

use super::{RPC, boxed, java_param, local, path, qualified, return_member_name};
use crate::adapter::interface::{self, BindingError, Operation, Parameter, Raise, Service};
use crate::adapter::java::java_member_type;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::Purpose;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::java;

/// The class that holds the codec methods `generate::source` writes.
pub(super) const CODEC_CLASS: &str = "ZdCodec";
/// `org.zerodds.wire`.
pub(super) const WIRE: &str = "org.zerodds.wire";

// -- expression and statement helpers --------------------------------------

pub(super) fn name(text: &str) -> java::Expr {
    java::Expr::Path(Path::single(text))
}

pub(super) fn path_expr(package: &str, leaf: &str) -> java::Expr {
    java::Expr::Path(path(package, leaf))
}

pub(super) fn qpath(segments: &[&str]) -> java::Expr {
    java::Expr::Path(Path::new(segments))
}

pub(super) fn int(value: i64) -> java::Expr {
    java::Expr::Literal(Literal::Int(value.to_string()))
}

pub(super) fn null() -> java::Expr {
    java::Expr::Literal(Literal::Null)
}

pub(super) fn string(text: &str) -> java::Expr {
    java::Expr::Literal(Literal::string(text))
}

pub(super) fn dot(base: java::Expr, field: &str) -> java::Expr {
    java::Expr::Field {
        value: Box::new(base),
        name: Ident::new(field),
    }
}

pub(super) fn call(
    receiver: Option<java::Expr>,
    method: &str,
    args: Vec<java::Expr>,
) -> java::Expr {
    java::Expr::MethodCall {
        receiver: receiver.map(Box::new),
        name: Ident::new(method),
        args,
    }
}

pub(super) fn binary(op: &'static str, lhs: java::Expr, rhs: java::Expr) -> java::Expr {
    java::Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

pub(super) fn assign(target: java::Expr, value: java::Expr) -> java::Statement {
    java::Statement::Expr(binary("=", target, value))
}

pub(super) fn new_of(type_: java::Type, args: Vec<java::Expr>) -> java::Expr {
    java::Expr::New {
        type_: Box::new(type_),
        args,
    }
}

pub(super) fn stmt(expr: java::Expr) -> java::Statement {
    java::Statement::Expr(expr)
}

pub(super) fn block(statements: Vec<java::Statement>) -> java::Block {
    java::Block { statements }
}

pub(super) fn declare(type_: java::Type, variable: &str, value: java::Expr) -> java::Statement {
    java::Statement::Declare {
        final_: false,
        type_,
        name: Ident::new(variable),
        value: Some(value),
    }
}

pub(super) fn cast_byte(value: java::Expr) -> java::Expr {
    java::Expr::Cast {
        type_: Box::new(java::Type::Primitive(java::Primitive::Byte)),
        value: Box::new(value),
    }
}

/// `if (a) {..} else if (b) {..} else {..}` from a list of arms.
pub(super) fn chain(
    mut arms: Vec<(java::Expr, java::Block)>,
    otherwise: java::Block,
) -> Vec<java::Statement> {
    let mut tail = otherwise;
    while let Some((condition, then)) = arms.pop() {
        tail = block(vec![java::Statement::If {
            condition,
            then,
            otherwise: Some(tail),
        }]);
    }
    tail.statements
}

pub(super) fn wire(leaf: &str) -> java::Type {
    java::Type::named(path(WIRE, leaf))
}

pub(super) fn rpc(leaf: &str) -> java::Type {
    qualified(RPC, leaf)
}

pub(super) fn param(type_: java::Type, variable: &str) -> java::Param {
    java::Param {
        annotations: Vec::new(),
        final_: false,
        type_,
        name: Ident::new(variable),
        varargs: false,
    }
}

pub(super) fn static_method(
    returns: java::Type,
    method_name: &str,
    params: Vec<java::Param>,
    body: Vec<java::Statement>,
) -> java::Member {
    java::Member::Method(java::Method {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public, java::Modifier::Static],
        generics: Vec::new(),
        returns,
        name: Ident::new(method_name),
        params,
        throws: Vec::new(),
        body: Some(block(body)),
    })
}

pub(super) fn override_annotation() -> java::Annotation {
    java::Annotation {
        path: Path::single("Override"),
        args: Vec::new(),
    }
}

pub(super) fn public_method(
    doc: Doc,
    overrides: bool,
    returns: java::Type,
    method_name: &str,
    params: Vec<java::Param>,
    body: Vec<java::Statement>,
) -> java::Member {
    java::Member::Method(java::Method {
        doc,
        annotations: if overrides {
            vec![override_annotation()]
        } else {
            Vec::new()
        },
        modifiers: vec![java::Modifier::Public],
        generics: Vec::new(),
        returns,
        name: Ident::new(method_name),
        params,
        throws: Vec::new(),
        body: Some(block(body)),
    })
}

pub(super) fn private_static_method(
    returns: java::Type,
    method_name: &str,
    params: Vec<java::Param>,
    body: Vec<java::Statement>,
) -> java::Member {
    java::Member::Method(java::Method {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Private, java::Modifier::Static],
        generics: Vec::new(),
        returns,
        name: Ident::new(method_name),
        params,
        throws: Vec::new(),
        body: Some(block(body)),
    })
}

pub(super) fn nested_class(
    class_name: &str,
    modifiers: Vec<java::Modifier>,
    extends: Option<java::Type>,
    implements: Vec<java::Type>,
    members: Vec<java::Member>,
    doc: Doc,
) -> java::Member {
    java::Member::Nested(Box::new(java::TypeDecl::Class(java::Class {
        doc,
        annotations: Vec::new(),
        modifiers,
        name: Ident::new(class_name),
        generics: Vec::new(),
        extends,
        implements,
        members,
    })))
}

pub(super) fn constructor(
    class_name: &str,
    params: Vec<java::Param>,
    body: Vec<java::Statement>,
    doc: Doc,
) -> java::Member {
    java::Member::Constructor(java::Constructor {
        doc,
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        name: Ident::new(class_name),
        params,
        throws: Vec::new(),
        body: block(body),
    })
}

pub(super) fn field_decl(
    modifiers: Vec<java::Modifier>,
    type_: java::Type,
    field_name: &str,
    value: Option<java::Expr>,
) -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers,
        type_,
        name: Ident::new(field_name),
        value,
    })
}

// -- the shape of a service ------------------------------------------------

/// One member of a structure: the Java field and the type its codec is
/// named after, `None` for the `dummy` member.
pub(super) type Member = (String, Option<Id<TypeNode>>);

/// The module class chain a declaration of this tree nests in, outermost
/// first and keyword-escaped, as `interface_java` nests it.
pub(super) fn chain_of(ast: &Ast, service: &Service) -> Result<Vec<String>, BindingError> {
    Ok(interface::module_chain(ast, service.module)?
        .into_iter()
        .map(|(_, module)| java::escape_keyword(&module))
        .collect())
}

/// `ZdServices.<chain>.<leaf>`: a type this file declares, reachable from
/// any module.
pub(super) fn generated(chain: &[String], leaf: &str) -> java::Type {
    let mut segments: Vec<&str> = vec![super::OUTER];
    segments.extend(chain.iter().map(String::as_str));
    segments.push(leaf);
    java::Type::named(Path::new(&segments))
}

/// `ZdTypes.<chain>.<leaf>`: a data type the type emitter declared.
pub(super) fn data_type(
    ast: &Ast,
    module: Id<zerodds_ast::ModuleNode>,
    leaf: &str,
) -> Result<java::Type, BindingError> {
    let mut segments: Vec<String> = vec![crate::adapter::java::OUTER.to_string()];
    segments.extend(
        interface::module_chain(ast, module)?
            .into_iter()
            .map(|(_, module)| java::escape_keyword(&module)),
    );
    segments.push(java::escape_keyword(leaf));
    let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(java::Type::named(Path::new(&refs)))
}

/// What one operation looks like to the code that calls it: the method's
/// Java name, its parameters and what it returns.
pub(super) struct Signature {
    /// The Java method name. An attribute's accessor is the attribute.
    pub(super) method: String,
    pub(super) params: Vec<java::Param>,
    /// What the synchronous method returns.
    pub(super) sync_returns: java::Type,
    /// The type argument of the asynchronous method's `Future`, boxed.
    pub(super) value_type: java::Type,
    /// The shape of the result.
    pub(super) result: ResultShape,
}

/// What an operation returns, which decides how the reply is unpacked and
/// the implementation's answer packed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ResultShape {
    /// `void`, no `out` or `inout`.
    Nothing,
    /// A return value and no `out` or `inout`.
    Value,
    /// At least one `out` or `inout`: the `Out` structure is the result.
    Outputs,
}

pub(super) fn result_shape(operation: &Operation) -> ResultShape {
    if operation.has_outputs() {
        ResultShape::Outputs
    } else if operation.returns.is_some() {
        ResultShape::Value
    } else {
        ResultShape::Nothing
    }
}

/// The Java name of an operation: §7.11.1.1.2 rule 6.f gives an attribute's
/// accessors the attribute's own name.
pub(super) fn method_name(operation: &Operation) -> String {
    let raw = operation.attribute_name();
    java::escape_keyword(raw)
}

pub(super) fn signature(
    ast: &Ast,
    context: &Service,
    owner: &Service,
    owner_chain: &[String],
    operation: &Operation,
) -> Result<Signature, BindingError> {
    let owner_name = java::escape_keyword(&owner.name);
    let spelled = java::escape_keyword(&operation.name);
    let result = result_shape(operation);
    let mut params = Vec::new();
    for input in operation.inputs() {
        params.push(java_param(ast, context, operation, input)?);
    }
    let out_struct = generated(
        owner_chain,
        &interface::out_struct_name(&owner_name, &spelled),
    );
    let (sync_returns, value_type) = match result {
        ResultShape::Outputs => (out_struct.clone(), out_struct),
        ResultShape::Value => {
            let returns = operation.returns.map_or(Ok(java::Type::Void), |target| {
                java_member_type(ast, target, &super::at(context, operation, "return"))
            })?;
            (returns.clone(), boxed(returns))
        }
        ResultShape::Nothing => (java::Type::Void, java::Type::named(Path::single("Void"))),
    };
    Ok(Signature {
        method: method_name(operation),
        params,
        sync_returns,
        value_type,
        result,
    })
}

/// The members of an `In` structure: the name of the Java field and the
/// type its codec is named after, `None` for the `dummy` member.
pub(super) fn in_members(operation: &Operation) -> Vec<Member> {
    let mut out: Vec<Member> = operation
        .inputs()
        .map(|param: &Parameter| (java::escape_keyword(&param.name), Some(param.type_)))
        .collect();
    if out.is_empty() {
        out.push((interface::DUMMY_MEMBER.to_string(), None));
    }
    out
}

/// The members of an `Out` structure, as `reply_fields` declares them.
pub(super) fn out_members(operation: &Operation) -> Vec<Member> {
    let mut out: Vec<Member> = operation
        .outputs()
        .map(|param| (java::escape_keyword(&param.name), Some(param.type_)))
        .collect();
    if let Some(returns) = operation.returns {
        let taken: Vec<String> = out.iter().map(|(member, _)| member.clone()).collect();
        out.push((return_member_name(&taken), Some(returns)));
    }
    if out.is_empty() {
        out.push((interface::DUMMY_MEMBER.to_string(), None));
    }
    out
}

/// The name of the member that carries an operation's return value: the last
/// of its `Out` structure (§7.5.1.1.5 rule 3b).
pub(super) fn return_field(operation: &Operation) -> String {
    out_members(operation)
        .into_iter()
        .last()
        .map_or_else(|| interface::DUMMY_MEMBER.to_string(), |(field, _)| field)
}

/// The exceptions every operation of `operations` raises, once each.
pub(super) fn exceptions_of(operations: &[Operation]) -> Vec<Raise> {
    let mut out: Vec<Raise> = Vec::new();
    for operation in operations {
        for raised in &operation.raises {
            if !out.iter().any(|seen| seen.name == raised.name) {
                out.push(raised.clone());
            }
        }
    }
    out
}

// -- the codec -------------------------------------------------------------

pub(super) fn encode_member(ast: &Ast, holder: &str, member: &Member) -> java::Statement {
    encode_member_of(ast, holder, member, false)
}

/// The place a member of `holder` is read from. The holders of this
/// binding (`In`, `Out`, `Result`, `Call`, `Return`) have public fields; a
/// data type of the model — an exception — is a class of the mapping in
/// force, and under [`JavaMapping::Psm`] its members are read through
/// their accessors.
fn place(holder: &str, field: &str, data: bool) -> java::Expr {
    if data && crate::adapter::java::mapping() == crate::adapter::java::JavaMapping::Psm {
        crate::adapter::java::accessor(name(holder), field)
    } else {
        dot(name(holder), field)
    }
}

/// [`encode_member`] for a member of a holder (`data == false`) or of a
/// data type of the model (`data == true`).
fn encode_member_of(ast: &Ast, holder: &str, member: &Member, data: bool) -> java::Statement {
    let (field, type_) = member;
    let value = place(holder, field, data);
    match type_ {
        // `UnusedMember`: one octet. §7.5.1.1.1.
        None => stmt(call(
            Some(name("w")),
            "writeU8",
            vec![java::Expr::Ternary {
                condition: Box::new(binary("==", value.clone(), null())),
                then: Box::new(int(0)),
                otherwise: Box::new(binary("&", dot(value, "value"), int(0xFF))),
            }],
        )),
        Some(id) => {
            let function =
                crate::emit::encode::codec_name::<crate::emit::java::Java>(ast, *id, Purpose::Data);
            java::Statement::If {
                condition: classic_cdr("w"),
                then: block(vec![stmt(call(
                    Some(qpath(&[CODEC_CLASS, CLASSIC_CODEC_CLASS])),
                    &function,
                    vec![name("w"), value.clone()],
                ))]),
                otherwise: Some(block(vec![stmt(call(
                    Some(name(CODEC_CLASS)),
                    &function,
                    vec![name("w"), value],
                ))])),
            }
        }
    }
}

/// The nested class of `ZdCodec` that holds the codecs under classic CDR.
pub(super) const CLASSIC_CODEC_CLASS: &str = "Xcdr1";

/// Whether the `Writer` or `Reader` called `holder` was made for
/// [`WireEncoding::XCDR1`](https://www.omg.org/spec/DDS-RPC/1.0): its
/// alignment cap is XCDR1's.
fn classic_cdr(holder: &str) -> java::Expr {
    binary(
        "==",
        call(Some(name(holder)), "maxAlign", Vec::new()),
        call(
            Some(qpath(&[
                "org",
                "omg",
                "dds",
                "rpc",
                "WireEncoding",
                "XCDR1",
            ])),
            "maxAlign",
            Vec::new(),
        ),
    )
}

pub(super) fn decode_member(ast: &Ast, holder: &str, member: &Member) -> java::Statement {
    decode_member_of(ast, holder, member, false)
}

/// [`decode_member`] for a member of a holder (`data == false`) or of a
/// data type of the model (`data == true`); see [`place`].
fn decode_member_of(ast: &Ast, holder: &str, member: &Member, data: bool) -> java::Statement {
    let (field, type_) = member;
    let target = place(holder, field, data);
    let assign = |target: java::Expr, value: java::Expr| {
        if data {
            crate::adapter::java::assign(target, value)
        } else {
            assign(target, value)
        }
    };
    match type_ {
        None => assign(
            target,
            new_of(
                rpc("UnusedMember"),
                vec![cast_byte(call(Some(name("r")), "readU8", Vec::new()))],
            ),
        ),
        Some(id) => {
            let function = crate::emit::decode::decoder_name::<crate::emit::java::Java>(ast, *id);
            let arguments = || {
                vec![
                    name("r"),
                    qpath(&["org", "zerodds", "wire", "TryConstruct", "DISCARD"]),
                ]
            };
            assign(
                target,
                java::Expr::Ternary {
                    condition: Box::new(classic_cdr("r")),
                    then: Box::new(call(
                        Some(qpath(&[CODEC_CLASS, CLASSIC_CODEC_CLASS])),
                        &function,
                        arguments(),
                    )),
                    otherwise: Box::new(call(Some(name(CODEC_CLASS)), &function, arguments())),
                },
            )
        }
    }
}

/// `encode_<what>` and `decode_<what>` for a structure of members.
pub(super) fn struct_codec(
    ast: &Ast,
    what: &str,
    type_: java::Type,
    members: &[Member],
) -> Vec<java::Member> {
    struct_codec_of(ast, what, type_, members, false)
}

/// [`struct_codec`] for a data type of the model (an exception), whose
/// members are reached as the mapping in force spells them.
pub(super) fn data_struct_codec(
    ast: &Ast,
    what: &str,
    type_: java::Type,
    members: &[Member],
) -> Vec<java::Member> {
    struct_codec_of(ast, what, type_, members, true)
}

fn struct_codec_of(
    ast: &Ast,
    what: &str,
    type_: java::Type,
    members: &[Member],
    data: bool,
) -> Vec<java::Member> {
    let encode = static_method(
        java::Type::Void,
        &format!("encode_{what}"),
        vec![param(wire("Writer"), "w"), param(type_.clone(), "value")],
        members
            .iter()
            .map(|member| encode_member_of(ast, "value", member, data))
            .collect(),
    );
    let mut body = vec![declare(
        type_.clone(),
        "value",
        new_of(type_.clone(), Vec::new()),
    )];
    body.extend(
        members
            .iter()
            .map(|member| decode_member_of(ast, "value", member, data)),
    );
    body.push(java::Statement::Return(Some(name("value"))));
    let decode = static_method(
        type_,
        &format!("decode_{what}"),
        vec![param(wire("Reader"), "r")],
        body,
    );
    vec![encode, decode]
}

/// One case of a union: its label, the field it fills, and the codec
/// methods of its payload.
struct Arm {
    label: java::Expr,
    field: String,
    encode: String,
    decode: String,
}

/// `encode<Kind>` and `decode<Kind>` of a union holder: the `long`
/// discriminator, then the case it selects, and an octet for the `default`
/// case (§7.5.1.1.6 rule 3, §7.5.1.1.7 rule 3, §7.5.1.1.5).
fn union_codec(
    kind: &str,
    type_: java::Type,
    arms: Vec<Arm>,
    default_field: &str,
    default_type: java::Type,
) -> Vec<java::Member> {
    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();
    for arm in arms {
        let condition = binary("==", dot(name("value"), "_d"), arm.label.clone());
        encode_arms.push((
            condition.clone(),
            block(vec![stmt(call(
                None,
                &arm.encode,
                vec![name("w"), dot(name("value"), &arm.field)],
            ))]),
        ));
        decode_arms.push((
            condition,
            block(vec![assign(
                dot(name("value"), &arm.field),
                call(None, &arm.decode, vec![name("r")]),
            )]),
        ));
    }
    let default_value = dot(name("value"), default_field);
    let mut encode_body = vec![stmt(call(
        Some(name("w")),
        "writeI32",
        vec![dot(name("value"), "_d")],
    ))];
    encode_body.extend(chain(
        encode_arms,
        block(vec![stmt(call(
            Some(name("w")),
            "writeU8",
            vec![java::Expr::Ternary {
                condition: Box::new(binary("==", default_value.clone(), null())),
                then: Box::new(int(0)),
                otherwise: Box::new(binary("&", dot(default_value.clone(), "value"), int(0xFF))),
            }],
        ))]),
    ));
    let mut decode_body = vec![
        declare(type_.clone(), "value", new_of(type_.clone(), Vec::new())),
        assign(
            dot(name("value"), "_d"),
            call(Some(name("r")), "readI32", Vec::new()),
        ),
    ];
    decode_body.extend(chain(
        decode_arms,
        block(vec![assign(
            default_value,
            new_of(
                default_type,
                vec![cast_byte(call(Some(name("r")), "readU8", Vec::new()))],
            ),
        )]),
    ));
    decode_body.push(java::Statement::Return(Some(name("value"))));
    vec![
        static_method(
            java::Type::Void,
            &format!("encode{kind}"),
            vec![param(wire("Writer"), "w"), param(type_.clone(), "value")],
            encode_body,
        ),
        static_method(
            type_,
            &format!("decode{kind}"),
            vec![param(wire("Reader"), "r")],
            decode_body,
        ),
    ]
}

/// A `TypeSupport` for `${I}_Request` or `${I}_Reply`: the `header` of the
/// structure is the runtime's, the `data` union is this codec's.
///
/// The runtime starts the union on a four-octet boundary of the sample and
/// says where; alignment counts from the start of the sample, with the
/// MAXALIGN of the encoding (XCDR1: 8, XCDR2: 4), so the writer is
/// advanced by the offset's remainder modulo 8 and those octets are cut
/// off again, and the reader starts that much before the union.
fn support_class(class_name: &str, sample: java::Type, kind: &str) -> java::Member {
    let bytes = || java::Type::Array(Box::new(java::Type::Primitive(java::Primitive::Byte)));
    let int_type = || java::Type::Primitive(java::Primitive::Int);
    let encoding = || rpc("WireEncoding");
    let support_for = sample.clone();

    let lead_loop = java::Statement::For {
        init: Box::new(declare(int_type(), "pad", int(0))),
        condition: binary("<", name("pad"), name("lead")),
        step: binary("=", name("pad"), binary("+", name("pad"), int(1))),
        body: block(vec![stmt(call(Some(name("w")), "writeU8", vec![int(0)]))]),
    };
    let serialize = public_method(
        Doc::none(),
        true,
        bytes(),
        "serialize",
        vec![
            param(sample.clone(), "value"),
            param(int_type(), "offset"),
            param(encoding(), "encoding"),
        ],
        vec![
            declare(int_type(), "lead", binary("%", name("offset"), int(8))),
            declare(
                wire("Writer"),
                "w",
                new_of(
                    wire("Writer"),
                    vec![
                        qpath(&["org", "zerodds", "wire", "Endianness", "LITTLE"]),
                        call(Some(name("encoding")), "maxAlign", Vec::new()),
                        null(),
                        java::Expr::Literal(Literal::Bool(false)),
                    ],
                ),
            ),
            lead_loop,
            stmt(call(
                None,
                &format!("encode{kind}"),
                vec![name("w"), dot(name("value"), interface::DATA_MEMBER)],
            )),
            declare(bytes(), "all", call(Some(name("w")), "finish", Vec::new())),
            java::Statement::Return(Some(call(
                Some(path_expr("java.util", "Arrays")),
                "copyOfRange",
                vec![name("all"), name("lead"), dot(name("all"), "length")],
            ))),
        ],
    );
    let serialize_default = public_method(
        Doc::none(),
        true,
        bytes(),
        "serialize",
        vec![param(sample.clone(), "value")],
        vec![java::Statement::Return(Some(call(
            None,
            "serialize",
            vec![
                name("value"),
                int(0),
                qpath(&["org", "omg", "dds", "rpc", "WireEncoding", "XCDR1"]),
            ],
        )))],
    );
    let cursor = || {
        java::Type::named(Path::new(&[
            "org",
            "omg",
            "dds",
            "rpc",
            "HeaderCodec",
            "Cursor",
        ]))
    };
    let deserialize = public_method(
        Doc::none(),
        true,
        sample.clone(),
        "deserialize",
        vec![
            param(bytes(), "octets"),
            param(cursor(), "cursor"),
            param(encoding(), "encoding"),
        ],
        vec![
            declare(
                int_type(),
                "lead",
                binary("%", dot(name("cursor"), "at"), int(8)),
            ),
            declare(
                wire("Reader"),
                "r",
                new_of(
                    wire("Reader"),
                    vec![
                        call(
                            Some(path_expr("java.util", "Arrays")),
                            "copyOfRange",
                            vec![
                                name("octets"),
                                binary("-", dot(name("cursor"), "at"), name("lead")),
                                dot(name("octets"), "length"),
                            ],
                        ),
                        qpath(&["org", "zerodds", "wire", "Endianness", "LITTLE"]),
                        call(Some(name("encoding")), "maxAlign", Vec::new()),
                        null(),
                    ],
                ),
            ),
            stmt(call(Some(name("r")), "skip", vec![name("lead")])),
            declare(sample.clone(), "value", new_of(sample, Vec::new())),
            assign(
                dot(name("value"), interface::DATA_MEMBER),
                call(None, &format!("decode{kind}"), vec![name("r")]),
            ),
            stmt(binary(
                "+=",
                dot(name("cursor"), "at"),
                binary(
                    "-",
                    call(Some(name("r")), "position", Vec::new()),
                    name("lead"),
                ),
            )),
            java::Statement::Return(Some(name("value"))),
        ],
    );
    let deserialize_default = public_method(
        Doc::none(),
        true,
        support_for.clone(),
        "deserialize",
        vec![param(bytes(), "octets"), param(cursor(), "cursor")],
        vec![java::Statement::Return(Some(call(
            None,
            "deserialize",
            vec![
                name("octets"),
                name("cursor"),
                qpath(&["org", "omg", "dds", "rpc", "WireEncoding", "XCDR1"]),
            ],
        )))],
    );
    nested_class(
        class_name,
        vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        None,
        vec![java::Type::generic(
            path(RPC, "TypeSupport"),
            vec![support_for],
        )],
        vec![
            serialize_default,
            serialize,
            deserialize_default,
            deserialize,
        ],
        Doc::none(),
    )
}

// -- the pieces of one service ---------------------------------------------

/// The exception's members as `(field, type)`, from the model: an
/// exception is declared as a structure and has no codec of its own.
pub(super) fn exception_members(
    ast: &Ast,
    raised: &Raise,
) -> Result<(Id<zerodds_ast::ModuleNode>, Vec<Member>), BindingError> {
    let node = ast.exception(raised.id).map_err(|_| {
        BindingError::from(interface::InterfaceError::NoNode {
            kind: "exception",
            at: raised.name.clone(),
        })
    })?;
    let mut members = Vec::new();
    for member in &node.members {
        let member_node = ast.member(*member).map_err(|_| {
            BindingError::from(interface::InterfaceError::NoNode {
                kind: "exception member",
                at: raised.name.clone(),
            })
        })?;
        let target = member_node.type_.resolved().ok_or_else(|| {
            BindingError::from(interface::InterfaceError::NoNode {
                kind: "exception member type",
                at: member_node.name.clone(),
            })
        })?;
        members.push((member_node.name.clone(), Some(target)));
    }
    Ok((node.module, members))
}

/// `<I>Codec` — the Basic mapping's octets for one interface.
fn codec_class(
    ast: &Ast,
    service: &Service,
    name: &str,
    operations: &[Operation],
) -> Result<java::Member, BindingError> {
    let request = local(&interface::request_type_name(name));
    let reply = local(&interface::reply_type_name(name));
    let call_union = local(&interface::call_union_name(name));
    let return_union = local(&interface::return_union_name(name));

    let mut members = vec![
        field_decl(
            vec![
                java::Modifier::Public,
                java::Modifier::Static,
                java::Modifier::Final,
            ],
            java::Type::generic(path(RPC, "TypeSupport"), vec![request.clone()]),
            "REQUEST",
            Some(new_of(local("RequestSupport"), Vec::new())),
        ),
        field_decl(
            vec![
                java::Modifier::Public,
                java::Modifier::Static,
                java::Modifier::Final,
            ],
            java::Type::generic(path(RPC, "TypeSupport"), vec![reply.clone()]),
            "REPLY",
            Some(new_of(local("ReplySupport"), Vec::new())),
        ),
        support_class("RequestSupport", request, "Call"),
        support_class("ReplySupport", reply, "Return"),
    ];

    // §7.5.1.1.6 rule 5: a case per operation; §7.5.1.1.7 rule 5: a case
    // per operation that replies.
    let mut call_arms = Vec::new();
    let mut return_arms = Vec::new();
    for operation in operations {
        let spelled = java::escape_keyword(&operation.name);
        let label = name_of(&interface::operation_hash_constant(name, &spelled));
        call_arms.push(Arm {
            label: label.clone(),
            field: spelled.clone(),
            encode: format!("encode_{}_In", operation.name),
            decode: format!("decode_{}_In", operation.name),
        });
        return_arms.push(Arm {
            label,
            field: spelled,
            encode: format!("encode_{}_Result", operation.name),
            decode: format!("decode_{}_Result", operation.name),
        });
    }
    members.extend(union_codec(
        "Call",
        call_union,
        call_arms,
        interface::UNKNOWN_OP_MEMBER,
        rpc("UnknownOperation"),
    ));
    members.extend(union_codec(
        "Return",
        return_union,
        return_arms,
        interface::UNKNOWN_OP_MEMBER,
        rpc("UnknownOperation"),
    ));

    for operation in operations {
        let spelled = java::escape_keyword(&operation.name);
        members.extend(struct_codec(
            ast,
            &format!("{}_In", operation.name),
            local(&interface::in_struct_name(name, &spelled)),
            &in_members(operation),
        ));
        members.extend(struct_codec(
            ast,
            &format!("{}_Out", operation.name),
            local(&interface::out_struct_name(name, &spelled)),
            &out_members(operation),
        ));
        // §7.5.1.1.5: `RETCODE_OK` carries the `Out` structure, each
        // declared exception its own, and `default` an octet.
        let mut arms = vec![Arm {
            label: qpath(&["org", "omg", "dds", "rpc", "PlaceholderTypes", "RETCODE_OK"]),
            field: interface::RESULT_MEMBER.to_string(),
            encode: format!("encode_{}_Out", operation.name),
            decode: format!("decode_{}_Out", operation.name),
        }];
        for raised in &operation.raises {
            let exception = java::escape_keyword(&raised.name);
            arms.push(Arm {
                label: name_of(&interface::exception_hash_constant(&exception)),
                field: interface::exception_member_name(&exception),
                encode: format!("encode_ex_{}", raised.name),
                decode: format!("decode_ex_{}", raised.name),
            });
        }
        members.extend(union_codec(
            &format!("_{}_Result", operation.name),
            local(&interface::result_union_name(name, &spelled)),
            arms,
            "unknownEx",
            rpc("UnknownException"),
        ));
    }
    for raised in exceptions_of(operations) {
        let (module, fields) = exception_members(ast, &raised)?;
        members.extend(data_struct_codec(
            ast,
            &format!("ex_{}", raised.name),
            data_type(ast, module, &raised.name)?,
            &fields,
        ));
    }
    let _ = service;
    Ok(nested_class(
        &format!("{name}Codec"),
        vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        None,
        Vec::new(),
        members,
        Doc::lines(&[
            "The octets of the Basic Service Mapping for this interface.",
            "",
            "DDS-RPC 1.0 §7.5.1.1.6 and §7.5.1.1.7: the `data` union of the request and of the",
            "reply, XCDR2, little-endian. Members are written by `ZdCodec`.",
        ]),
    ))
}

fn name_of(text: &str) -> java::Expr {
    name(text)
}

/// The ancestors of `service`, nearest first, each once.
pub(super) fn ancestors(ast: &Ast, service: &Service) -> Result<Vec<Service>, BindingError> {
    let mut seen: Vec<Id<zerodds_ast::InterfaceNode>> = vec![service.id];
    let mut out = Vec::new();
    let mut stack: Vec<Service> = vec![service.clone()];
    while let Some(next) = stack.pop() {
        for base in &next.bases {
            if seen.contains(&base.id) {
                continue;
            }
            seen.push(base.id);
            let found = interface::service(ast, base.id)?;
            out.push(found.clone());
            stack.push(found);
        }
    }
    Ok(out)
}

/// The Java method `operation` is called by, with the parameters it is
/// passed as expressions.
pub(super) fn arguments(operation: &Operation) -> Vec<java::Expr> {
    operation
        .inputs()
        .map(|input| name(&java::escape_keyword(&input.name)))
        .collect()
}

pub(super) fn function_of(request_to: java::Type, value: java::Type) -> java::Type {
    java::Type::generic(
        Path::new(&["java", "util", "function", "Function"]),
        vec![request_to, value],
    )
}

/// `<op>_request`, `<op>_Unpack`, the synchronous and the asynchronous
/// method of one operation of the interface itself.
fn client_operation(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    chain: &[String],
    operation: &Operation,
) -> Result<Vec<java::Member>, BindingError> {
    let spelled = java::escape_keyword(&operation.name);
    let sig = signature(ast, service, service, chain, operation)?;
    let request_type = local(&interface::request_type_name(name_escaped));
    let reply_type = local(&interface::reply_type_name(name_escaped));
    let builder = format!("{}_request", operation.name);
    let unpack = format!("{}_Unpack", operation.name);
    let data = || dot(name("request"), interface::DATA_MEMBER);
    let case = || dot(data(), &spelled);

    // The request: the union holder with the operation's case filled.
    let mut build = vec![
        declare(
            request_type.clone(),
            "request",
            new_of(request_type.clone(), Vec::new()),
        ),
        assign(
            data(),
            new_of(local(&interface::call_union_name(name_escaped)), Vec::new()),
        ),
        assign(
            dot(data(), "_d"),
            name(&interface::operation_hash_constant(name_escaped, &spelled)),
        ),
        assign(
            case(),
            new_of(
                local(&interface::in_struct_name(name_escaped, &spelled)),
                Vec::new(),
            ),
        ),
    ];
    for (field, type_) in in_members(operation) {
        match type_ {
            Some(_) => build.push(assign(dot(case(), &field), name(&field))),
            None => build.push(assign(
                dot(case(), &field),
                new_of(rpc("UnusedMember"), Vec::new()),
            )),
        }
    }
    build.push(java::Statement::Return(Some(name("request"))));
    let mut members = vec![private_static_method(
        request_type,
        &builder,
        sig.params.clone(),
        build,
    )];

    let args = arguments(operation);
    let request_call = call(None, &builder, args);

    // What the reply is unpacked to, and what is thrown when it carries
    // an exception.
    let result_type = local(&interface::result_union_name(name_escaped, &spelled));
    let mut raise = Vec::new();
    for raised in &operation.raises {
        let exception = java::escape_keyword(&raised.name);
        raise.push((
            binary(
                "==",
                dot(name("result"), "_d"),
                name(&interface::exception_hash_constant(&exception)),
            ),
            block(vec![java::Statement::Throw(new_of(
                rpc("UserExceptionRaised"),
                vec![dot(
                    name("result"),
                    &interface::exception_member_name(&exception),
                )],
            ))]),
        ));
    }
    let mut failure = chain_arms(raise);
    failure.push(java::Statement::Throw(new_of(
        rpc("RemoteUnknownExceptionError"),
        vec![string(&format!(
            "{}.{}: the service raised an exception this client does not know",
            service.name, operation.name
        ))],
    )));
    let returned = match sig.result {
        ResultShape::Outputs => dot(name("result"), interface::RESULT_MEMBER),
        ResultShape::Value => dot(
            dot(name("result"), interface::RESULT_MEMBER),
            &return_field(operation),
        ),
        ResultShape::Nothing => null(),
    };
    let apply = public_method(
        Doc::none(),
        true,
        sig.value_type.clone(),
        "apply",
        vec![param(reply_type.clone(), "reply")],
        vec![
            declare(
                result_type,
                "result",
                dot(dot(name("reply"), interface::DATA_MEMBER), &spelled),
            ),
            java::Statement::If {
                condition: binary(
                    "!=",
                    dot(name("result"), "_d"),
                    qpath(&["org", "omg", "dds", "rpc", "PlaceholderTypes", "RETCODE_OK"]),
                ),
                then: block(failure),
                otherwise: None,
            },
            java::Statement::Return(Some(returned)),
        ],
    );
    members.push(nested_class(
        &unpack,
        vec![
            java::Modifier::Private,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        None,
        vec![function_of(reply_type, sig.value_type.clone())],
        vec![apply],
        Doc::none(),
    ));

    let unpacked = call(
        Some(new_of(local(&unpack), Vec::new())),
        "apply",
        vec![call(None, "call", vec![request_call.clone()])],
    );
    let sync_body = if sig.result == ResultShape::Nothing {
        vec![stmt(unpacked)]
    } else {
        vec![java::Statement::Return(Some(unpacked))]
    };
    members.push(public_method(
        Doc::none(),
        true,
        sig.sync_returns,
        &sig.method,
        sig.params.clone(),
        sync_body,
    ));
    members.push(public_method(
        Doc::none(),
        true,
        java::Type::generic(path(RPC, "Future"), vec![sig.value_type]),
        &format!("{}_async", sig.method),
        sig.params,
        vec![java::Statement::Return(Some(call(
            Some(path_expr(RPC, "Futures")),
            "map",
            vec![
                call(None, "callAsync", vec![request_call]),
                new_of(local(&unpack), Vec::new()),
            ],
        )))],
    ));
    Ok(members)
}

/// [`chain`] with nothing in the final `else`: the arms, then what follows.
pub(super) fn chain_arms(arms: Vec<(java::Expr, java::Block)>) -> Vec<java::Statement> {
    arms.into_iter()
        .map(|(condition, then)| java::Statement::If {
            condition,
            then,
            otherwise: None,
        })
        .collect()
}

/// The forwarding methods of one base's operations.
fn delegates(
    ast: &Ast,
    service: &Service,
    field: &str,
    owners: &[Service],
    seen: &mut Vec<(String, usize)>,
) -> Result<Vec<java::Member>, BindingError> {
    let mut members = Vec::new();
    for owner in owners {
        let owner_chain = chain_of(ast, owner)?;
        for operation in owner.all_operations() {
            let sig = signature(ast, service, owner, &owner_chain, &operation)?;
            let key = (sig.method.clone(), sig.params.len());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let args = arguments(&operation);
            let target = || Some(dot(java::Expr::This, field));
            let forward = call(target(), &sig.method, args.clone());
            let body = if matches!(sig.sync_returns, java::Type::Void) {
                vec![stmt(forward)]
            } else {
                vec![java::Statement::Return(Some(forward))]
            };
            members.push(public_method(
                Doc::none(),
                true,
                sig.sync_returns,
                &sig.method,
                sig.params.clone(),
                body,
            ));
            members.push(public_method(
                Doc::none(),
                true,
                java::Type::generic(path(RPC, "Future"), vec![sig.value_type]),
                &format!("{}_async", sig.method),
                sig.params,
                vec![java::Statement::Return(Some(call(
                    target(),
                    &format!("{}_async", method_name(&operation)),
                    args,
                )))],
            ));
        }
    }
    Ok(members)
}

/// `ZdServices.<modules>.<I>Support`, as an expression path.
pub(super) fn support_path(ast: &Ast, service: &Service) -> Result<Path, BindingError> {
    let mut segments: Vec<String> = vec![super::OUTER.to_string()];
    segments.extend(chain_of(ast, service)?);
    segments.push(format!("{}Support", java::escape_keyword(&service.name)));
    let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(Path::new(&refs))
}

/// `<I>ClientImpl`.
fn client_class(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    chain: &[String],
    operations: &[Operation],
) -> Result<java::Member, BindingError> {
    let request_type = local(&interface::request_type_name(name_escaped));
    let reply_type = local(&interface::reply_type_name(name_escaped));
    let class_name = format!("{name_escaped}ClientImpl");
    // DDS-RPC 1.0 §7.11.2: the params of the machine-readable Java files take no type
    // arguments.
    let params_type = java::Type::named(path(RPC, "RequesterParams"));
    let participant_type = rpc("DomainParticipant");

    let mut members = Vec::new();
    // One client per direct base: the base's operations travel on the
    // base's own topics (§7.5.1.1.8).
    let mut bases = Vec::new();
    for (at, base) in service.bases.iter().enumerate() {
        let found = interface::service(ast, base.id)?;
        let base_chain = chain_of(ast, &found)?;
        let base_class = generated(
            &base_chain,
            &format!("{}ClientImpl", java::escape_keyword(&found.name)),
        );
        let field = format!("base{at}");
        members.push(field_decl(
            vec![java::Modifier::Private, java::Modifier::Final],
            base_class.clone(),
            &field,
            None,
        ));
        bases.push((field, base_class, found));
    }

    let mut main = vec![stmt(call(None, "super", vec![name("params")]))];
    for (field, base_class, found) in &bases {
        let base_support = support_path(ast, found)?;
        main.push(assign(
            dot(java::Expr::This, field),
            new_of(
                base_class.clone(),
                vec![call(
                    Some(call(
                        Some(call(
                            Some(java::Expr::Path(base_support.clone())),
                            "requesterParams",
                            vec![call(Some(name("params")), "domainParticipant", Vec::new())],
                        )),
                        "instanceName",
                        vec![call(Some(name("params")), "instanceName", Vec::new())],
                    )),
                    "sameSettingsAs",
                    vec![name("params")],
                )],
            ),
        ));
        main.push(stmt(call(
            None,
            "registerBase",
            vec![dot(java::Expr::This, field)],
        )));
    }
    members.push(constructor(
        &class_name,
        vec![param(params_type, "params")],
        main,
        Doc::line("A client over the requester the params describe."),
    ));
    let support = format!("{name_escaped}Support");
    members.push(constructor(
        &class_name,
        vec![param(participant_type.clone(), "participant")],
        vec![stmt(call(
            None,
            "this",
            vec![call(
                Some(name(&support)),
                "requesterParams",
                vec![name("participant")],
            )],
        ))],
        Doc::line("A client of the default instance."),
    ));
    members.push(constructor(
        &class_name,
        vec![
            param(participant_type, "participant"),
            param(java::Type::named(Path::single("String")), "instanceName"),
        ],
        vec![stmt(call(
            None,
            "this",
            vec![call(
                Some(call(
                    Some(name(&support)),
                    "requesterParams",
                    vec![name("participant")],
                )),
                "instanceName",
                vec![name("instanceName")],
            )],
        ))],
        Doc::line("A client bound to one service instance (§7.11.1.4.3)."),
    ));

    let mut seen: Vec<(String, usize)> = Vec::new();
    for operation in operations {
        let sig = signature(ast, service, service, chain, operation)?;
        seen.push((sig.method.clone(), sig.params.len()));
        members.extend(client_operation(
            ast,
            service,
            name_escaped,
            chain,
            operation,
        )?);
    }
    for (field, _, found) in &bases {
        let mut owners = vec![found.clone()];
        owners.extend(ancestors(ast, found)?);
        members.extend(delegates(ast, service, field, &owners, &mut seen)?);
    }

    Ok(nested_class(
        &class_name,
        vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        Some(java::Type::generic(
            path(RPC, "FunctionCallClient"),
            vec![request_type, reply_type],
        )),
        vec![local(&format!("{name_escaped}Client"))],
        members,
        Doc::lines(&[
            "The function-call client over a `Requester`.",
            "",
            "DDS-RPC 1.0 §7.11.1.5.4, §7.11.2.3: it instantiates the DataWriter and the DataReader",
            "and calls the service through them.",
        ]),
    ))
}

/// `<op>_invoke` of the dispatcher.
fn invoke_method(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    chain: &[String],
    operation: &Operation,
) -> Result<java::Member, BindingError> {
    let spelled = java::escape_keyword(&operation.name);
    let sig = signature(ast, service, service, chain, operation)?;
    let in_type = local(&interface::in_struct_name(name_escaped, &spelled));
    let implementation_call = || {
        call(
            Some(java::Expr::Path(Path::single("implementation"))),
            &sig.method,
            operation
                .inputs()
                .map(|input| dot(name("in"), &java::escape_keyword(&input.name)))
                .collect(),
        )
    };
    let lambda = || java::Expr::Lambda {
        params: Vec::new(),
        body: Box::new(implementation_call()),
    };
    let outcome_type = |value: java::Type| java::Type::generic(path(RPC, "Outcome"), vec![value]);
    let result_type = local(&interface::result_union_name(name_escaped, &spelled));
    let out_type = local(&interface::out_struct_name(name_escaped, &spelled));
    let mut body = vec![declare(
        result_type.clone(),
        "result",
        new_of(result_type.clone(), Vec::new()),
    )];
    let call_expr = if sig.result == ResultShape::Nothing {
        call(Some(path_expr(RPC, "Outcome")), "ofVoid", vec![lambda()])
    } else {
        call(Some(path_expr(RPC, "Outcome")), "of", vec![lambda()])
    };
    body.push(declare(
        outcome_type(sig.value_type.clone()),
        "outcome",
        call_expr,
    ));
    // A declared exception fills its case of the `Result` union.
    let mut raised_arms = Vec::new();
    for raised in &operation.raises {
        let exception = java::escape_keyword(&raised.name);
        let (module, _) = exception_members(ast, raised)?;
        let holder = data_type(ast, module, &raised.name)?;
        raised_arms.push((
            java::Expr::InstanceOf {
                value: Box::new(name("raised")),
                type_: Box::new(holder.clone()),
            },
            block(vec![
                assign(
                    dot(name("result"), "_d"),
                    name(&interface::exception_hash_constant(&exception)),
                ),
                assign(
                    dot(
                        name("result"),
                        &interface::exception_member_name(&exception),
                    ),
                    java::Expr::Cast {
                        type_: Box::new(holder),
                        value: Box::new(name("raised")),
                    },
                ),
                java::Statement::Return(Some(name("result"))),
            ]),
        ));
    }
    let mut when_raised = vec![declare(
        java::Type::named(Path::single("Object")),
        "raised",
        call(Some(name("outcome")), "raised", Vec::new()),
    )];
    when_raised.extend(chain_arms(raised_arms));
    // Anything else is the implementation's failure, not a result.
    when_raised.push(java::Statement::Throw(call(
        Some(name("outcome")),
        "failure",
        Vec::new(),
    )));
    body.push(java::Statement::If {
        condition: call(Some(name("outcome")), "isRaised", Vec::new()),
        then: block(when_raised),
        otherwise: None,
    });
    body.push(assign(
        dot(name("result"), "_d"),
        qpath(&["org", "omg", "dds", "rpc", "PlaceholderTypes", "RETCODE_OK"]),
    ));
    match sig.result {
        ResultShape::Outputs => body.push(assign(
            dot(name("result"), interface::RESULT_MEMBER),
            call(Some(name("outcome")), "value", Vec::new()),
        )),
        ResultShape::Value => {
            let ret = return_field(operation);
            body.push(assign(
                dot(name("result"), interface::RESULT_MEMBER),
                new_of(out_type, Vec::new()),
            ));
            body.push(assign(
                dot(dot(name("result"), interface::RESULT_MEMBER), &ret),
                call(Some(name("outcome")), "value", Vec::new()),
            ));
        }
        ResultShape::Nothing => {
            body.push(assign(
                dot(name("result"), interface::RESULT_MEMBER),
                new_of(out_type, Vec::new()),
            ));
            body.push(assign(
                dot(
                    dot(name("result"), interface::RESULT_MEMBER),
                    interface::DUMMY_MEMBER,
                ),
                new_of(rpc("UnusedMember"), Vec::new()),
            ));
        }
    }
    body.push(java::Statement::Return(Some(name("result"))));
    Ok(java::Member::Method(java::Method {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Private],
        generics: Vec::new(),
        returns: result_type,
        name: Ident::new(&format!("{}_invoke", operation.name)),
        params: vec![param(in_type, "in")],
        throws: Vec::new(),
        body: Some(block(body)),
    }))
}

/// `<I>ServiceImpl`.
fn service_class(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    chain: &[String],
    operations: &[Operation],
) -> Result<java::Member, BindingError> {
    let request_type = local(&interface::request_type_name(name_escaped));
    let reply_type = local(&interface::reply_type_name(name_escaped));
    let class_name = format!("{name_escaped}ServiceImpl");
    let interface_type = local(name_escaped);
    // DDS-RPC 1.0 §7.11.2: the params of the machine-readable Java files take no type
    // arguments.
    let params_type = java::Type::named(path(RPC, "ReplierParams"));
    let string_type = java::Type::named(Path::single("String"));

    // The dispatcher: unpack, call, pack.
    let handler_type = java::Type::generic(
        path(RPC, "RequestHandler"),
        vec![request_type.clone(), reply_type.clone()],
    );
    let data = |holder: &str| dot(name(holder), interface::DATA_MEMBER);
    let unknown_reply = |holder: &str| {
        vec![
            assign(
                data(holder),
                new_of(
                    local(&interface::return_union_name(name_escaped)),
                    Vec::new(),
                ),
            ),
            assign(
                dot(data(holder), interface::UNKNOWN_OP_MEMBER),
                new_of(rpc("UnknownOperation"), Vec::new()),
            ),
        ]
    };
    let mut dispatcher_members = vec![
        field_decl(
            vec![java::Modifier::Private, java::Modifier::Final],
            interface_type.clone(),
            "implementation",
            None,
        ),
        java::Member::Constructor(java::Constructor {
            doc: Doc::none(),
            annotations: Vec::new(),
            modifiers: Vec::new(),
            name: Ident::new("Dispatcher"),
            params: vec![param(interface_type.clone(), "implementation")],
            throws: Vec::new(),
            body: block(vec![assign(
                dot(java::Expr::This, "implementation"),
                name("implementation"),
            )]),
        }),
    ];

    // handle()
    let mut arms = Vec::new();
    for operation in operations {
        let spelled = java::escape_keyword(&operation.name);
        let condition = binary(
            "==",
            dot(data("request"), "_d"),
            name(&interface::operation_hash_constant(name_escaped, &spelled)),
        );
        let invoke = format!("{}_invoke", operation.name);
        let arguments = vec![dot(data("request"), &spelled)];
        let body = vec![
            assign(
                data("reply"),
                new_of(
                    local(&interface::return_union_name(name_escaped)),
                    Vec::new(),
                ),
            ),
            assign(
                dot(data("reply"), "_d"),
                name(&interface::operation_hash_constant(name_escaped, &spelled)),
            ),
            assign(dot(data("reply"), &spelled), call(None, &invoke, arguments)),
            java::Statement::Return(Some(call(
                Some(path_expr(RPC, "Response")),
                "ok",
                vec![name("reply")],
            ))),
        ];
        arms.push((condition, block(body)));
    }
    let mut handle = vec![declare(
        reply_type.clone(),
        "reply",
        new_of(reply_type.clone(), Vec::new()),
    )];
    handle.extend(chain_arms(arms));
    // §7.7.1.1: an operation this service does not know is
    // REMOTE_EX_UNSUPPORTED.
    handle.extend(unknown_reply("reply"));
    handle.push(java::Statement::Return(Some(call(
        Some(path_expr(RPC, "Response")),
        "failed",
        vec![
            name("reply"),
            qpath(&[
                "org",
                "omg",
                "dds",
                "rpc",
                "RemoteExceptionCode",
                "REMOTE_EX_UNSUPPORTED",
            ]),
        ],
    ))));
    dispatcher_members.push(public_method(
        Doc::none(),
        true,
        java::Type::generic(path(RPC, "Response"), vec![reply_type.clone()]),
        "handle",
        vec![param(request_type.clone(), "request")],
        handle,
    ));
    let mut failure = vec![declare(
        reply_type.clone(),
        "reply",
        new_of(reply_type.clone(), Vec::new()),
    )];
    failure.extend(unknown_reply("reply"));
    failure.push(java::Statement::Return(Some(name("reply"))));
    dispatcher_members.push(public_method(
        Doc::none(),
        true,
        reply_type.clone(),
        "failureReply",
        vec![param(request_type.clone(), "request")],
        failure,
    ));
    for operation in operations {
        dispatcher_members.push(invoke_method(ast, service, name_escaped, chain, operation)?);
    }
    let dispatcher = nested_class(
        "Dispatcher",
        vec![
            java::Modifier::Private,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        None,
        vec![handler_type],
        dispatcher_members,
        Doc::none(),
    );

    // The service of every direct base, over the same implementation.
    let mut register = Vec::new();
    for base in &service.bases {
        let found = interface::service(ast, base.id)?;
        let base_chain = chain_of(ast, &found)?;
        let base_class = generated(
            &base_chain,
            &format!("{}ServiceImpl", java::escape_keyword(&found.name)),
        );
        register.push(stmt(call(
            None,
            "registerBase",
            vec![new_of(
                base_class,
                vec![
                    call(
                        Some(call(
                            Some(java::Expr::Path(support_path(ast, &found)?)),
                            "replierParams",
                            vec![
                                call(Some(name("params")), "domainParticipant", Vec::new()),
                                call(Some(name("params")), "instanceName", Vec::new()),
                            ],
                        )),
                        "sameSettingsAs",
                        vec![name("params")],
                    ),
                    name("implementation"),
                ],
            )],
        )));
    }

    let support = format!("{name_escaped}Support");
    let mut main = vec![
        stmt(call(
            None,
            "super",
            vec![
                name("params"),
                new_of(local("Dispatcher"), vec![name("implementation")]),
            ],
        )),
        assign(
            dot(java::Expr::This, "implementation"),
            name("implementation"),
        ),
    ];
    main.extend(register);
    let members = vec![
        field_decl(
            vec![java::Modifier::Private, java::Modifier::Final],
            interface_type.clone(),
            "implementation",
            None,
        ),
        constructor(
            &class_name,
            vec![
                param(params_type, "params"),
                param(interface_type.clone(), "implementation"),
            ],
            main,
            Doc::line("A service over the replier the params describe."),
        ),
        constructor(
            &class_name,
            vec![
                param(rpc("DomainParticipant"), "participant"),
                param(interface_type.clone(), "implementation"),
            ],
            vec![stmt(call(
                None,
                "this",
                vec![name("participant"), name("implementation"), string("")],
            ))],
            Doc::line("Activates the service for the default instance."),
        ),
        constructor(
            &class_name,
            vec![
                param(rpc("DomainParticipant"), "participant"),
                param(interface_type.clone(), "implementation"),
                param(string_type, "instanceName"),
            ],
            vec![stmt(call(
                None,
                "this",
                vec![
                    call(
                        Some(name(&support)),
                        "replierParams",
                        vec![name("participant"), name("instanceName")],
                    ),
                    name("implementation"),
                ],
            ))],
            Doc::line("Activates the service as the instance `instanceName`."),
        ),
        public_method(
            Doc::none(),
            true,
            interface_type,
            "implementation",
            Vec::new(),
            vec![java::Statement::Return(Some(dot(
                java::Expr::This,
                "implementation",
            )))],
        ),
        dispatcher,
    ];
    Ok(nested_class(
        &class_name,
        vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        Some(java::Type::generic(
            path(RPC, "FunctionCallService"),
            vec![request_type, reply_type],
        )),
        vec![local(&format!("{name_escaped}Service"))],
        members,
        Doc::lines(&[
            "The function-call service over a `Replier`.",
            "",
            "DDS-RPC 1.0 §7.9.1, §7.11.1.5.1, §7.11.2.3: creating it activates the service, the",
            "requests it receives call the implementation, `close()` deactivates it (§7.9.3).",
        ]),
    ))
}

/// The two factory methods `<I>Support` carries: the params that make a
/// requester or a replier of this interface — its topic names, its type
/// names, its codec and the `@Qos` of §7.10.1.
pub(super) fn support_methods(
    service: &Service,
    name_escaped: &str,
    descriptions: Option<&(
        crate::emit::interface_rust::type_information::JavaDescription,
        crate::emit::interface_rust::type_information::JavaDescription,
    )>,
) -> Vec<java::Member> {
    let codec = format!("{name_escaped}Codec");
    let make = |role: &str, with_instance: bool| {
        let params_type = java::Type::named(path(RPC, &format!("{role}Params")));
        let mut chain_expr = call(
            Some(name("params")),
            "domainParticipant",
            vec![name("participant")],
        );
        for (setter, value) in [
            // §7.4.1 and §7.4.2.3: with the interface name and the annotated
            // service name a run-time service name composes
            // `<interface_name>_<service_name>_Request`, not the
            // request-reply style's `<service_name>_Request`.
            ("interfaceName", name("INTERFACE_NAME")),
            ("annotatedServiceName", name("SERVICE_NAME")),
            ("requestTypeName", name("REQUEST_TYPE_NAME")),
            ("replyTypeName", name("REPLY_TYPE_NAME")),
            ("requestTypeSupport", dot(name(&codec), "REQUEST")),
            ("replyTypeSupport", dot(name(&codec), "REPLY")),
        ] {
            chain_expr = call(Some(chain_expr), setter, vec![value]);
        }
        // §7.4.2.2: a whole topic name is set only where `@DDSRequestTopic` or
        // `@DDSReplyTopic` gave one. Otherwise the run time composes the names from the
        // interface and service name (§7.4.1), and with them the aliases of the bases
        // (§7.4.3), which a whole name suppresses.
        if service.request_topic.is_some() {
            chain_expr = call(
                Some(chain_expr),
                "requestTopicName",
                vec![name("REQUEST_TOPIC_NAME")],
            );
        }
        if service.reply_topic.is_some() {
            chain_expr = call(
                Some(chain_expr),
                "replyTopicName",
                vec![name("REPLY_TOPIC_NAME")],
            );
        }
        // §7.5.1.2, §7.8.2: the Enhanced mapping has no header and its
        // samples are PL_CDR2; the runtime is told so by the params.
        if service.mapping.is_enhanced() {
            chain_expr = call(
                Some(chain_expr),
                "mapping",
                vec![qpath(&[
                    "org",
                    "omg",
                    "dds",
                    "rpc",
                    "ServiceMapping",
                    "ENHANCED",
                ])],
            );
            // §7.4.3, §7.5.1.2.6: a derived interface answers the operations of its
            // bases on its own topics and announces the topics of each base as aliases.
            if !service.bases.is_empty() {
                chain_expr = call(
                    Some(chain_expr),
                    "parents",
                    vec![call(
                        Some(qpath(&["java", "util", "Arrays"])),
                        "asList",
                        service
                            .bases
                            .iter()
                            .map(|base| string(&base.qualified_name))
                            .collect(),
                    )],
                );
            }
            if let Some((request, reply)) = descriptions {
                let description =
                    |d: &crate::emit::interface_rust::type_information::JavaDescription| {
                        call(
                            Some(qpath(&["org", "zerodds", "ddsrpc", "TypeDescription"])),
                            "of",
                            vec![
                                string(&d.information),
                                string(&d.objects),
                                string(&d.references),
                            ],
                        )
                    };
                chain_expr = call(
                    Some(chain_expr),
                    "typeDescriptions",
                    vec![description(request), description(reply)],
                );
            }
            chain_expr = call(
                Some(chain_expr),
                "wireEncoding",
                vec![qpath(&[
                    "org",
                    "omg",
                    "dds",
                    "rpc",
                    "WireEncoding",
                    "XCDR2",
                ])],
            );
        }
        if with_instance {
            chain_expr = call(Some(chain_expr), "instanceName", vec![name("instanceName")]);
        }
        let mut body = vec![
            declare(
                params_type.clone(),
                "params",
                call(
                    Some(path_expr(RPC, &format!("{role}Params"))),
                    "create",
                    Vec::new(),
                ),
            ),
            stmt(chain_expr),
        ];
        // §7.10.1: `@Qos(RequestProfile, ReplyProfile)`.
        if let Some(request_profile) = service.qos_request_profile.as_deref() {
            body.push(stmt(call(
                Some(name("params")),
                "qosProfiles",
                vec![
                    string(request_profile),
                    service
                        .qos_reply_profile
                        .as_deref()
                        .map_or_else(null, string),
                ],
            )));
        }
        body.push(java::Statement::Return(Some(name("params"))));
        let mut method_params = vec![param(rpc("DomainParticipant"), "participant")];
        if with_instance {
            method_params.push(param(
                java::Type::named(Path::single("String")),
                "instanceName",
            ));
        }
        static_method(
            params_type,
            &format!("{}Params", role.to_lowercase()),
            method_params,
            body,
        )
    };
    vec![make("Requester", false), make("Replier", true)]
}

/// The bodies of one function-call service: its codec, its client and its
/// service.
pub(super) fn members(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
) -> Result<Vec<java::Member>, BindingError> {
    let chain = chain_of(ast, service)?;
    let operations = service.all_operations();
    Ok(vec![
        codec_class(ast, service, name_escaped, &operations)?,
        client_class(ast, service, name_escaped, &chain, &operations)?,
        service_class(ast, service, name_escaped, &chain, &operations)?,
    ])
}
