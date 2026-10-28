// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The octets of the topic types of the Enhanced Service Mapping, for the
//! C++ binding of DDS-RPC 1.0.
//!
//! `dds::rpc::Requester` and `dds::rpc::Replier` under the Enhanced profile
//! (§7.8.2) write no header: the sample is the bare `${I}_Request` or
//! `${I}_Reply` structure, serialized as PL_CDR2 because the clause reads
//! an `@Choice` structure "as if the structure had the
//! `@Extensibility(MUTABLE_EXTENSIBILITY)` annotation and all the members
//! of the structure had the `@Optional` annotation" (§7.5.1.2.1.1). This
//! writes the encoder and the decoder of each structure and the
//! `dds_type_traits` of the two topic types.
//!
//! The forms are those of `emit::interface_rust::enhanced`, octet for octet:
//!
//! - a choice structure (`Request`, `Reply`, `Result`): a `DHEADER`, the
//!   `EMHEADER` of the one member present (member id = HASH & 0x0FFFFFFF,
//!   length code 4 with the `NEXTINT`; 2 for `remoteEx`, a 32-bit value),
//!   the member;
//! - a `@Mutable` `In` or `Out`: a `DHEADER`, an `EMHEADER` and the value of
//!   each member, the length code the member's type calls for
//!   ([`crate::emit::interface_rust::emheader_length_code`]); on decode in
//!   any order, a member that is absent takes its default (XTypes 1.3
//!   §7.2.4.4.8) and one that is not known is skipped by its length;
//! - any other `In` or `Out`, and an exception: the members one after the
//!   other.
//!
//! The octets of a member's own type are those of the shared codec walk
//! (`super::codec::member_codecs`), under the key the Basic mapping uses.

use super::codec::{
    assign, binary, block, call, const_ref, dds_type_name, decode_member, dot, encode_member,
    exception_members, expr_statement, function, method, mut_ref, name, param, path, reader_type,
    scoped, with_unused, writer_type,
};
use super::{cxx_escape, qualified};
use crate::adapter::interface::{
    BindingError, EnhancedMemberType, EnhancedOperation, EnhancedService, EnhancedStruct, Raise,
    Service,
};
use zerodds_ast::{Ast, ExceptionNode, ModuleNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp;

/// The remote exception codes of §7.5.2 are `0..=REMOTE_EX_MAX`.
const REMOTE_EX_MAX: i128 = 5;

/// The size of an `EMHEADER`: what is left of a choice structure after its
/// one member, when it is at least this much, is a second member.
const EMHEADER_SIZE: i128 = 4;

/// `zerodds::wire::Status::Ok`.
fn status_ok() -> cpp::Expr {
    path(&["zerodds", "wire", "Status", "Ok"])
}

fn int(value: i128) -> cpp::Expr {
    cpp::Expr::Literal(Literal::int(value))
}

fn unsigned_type(name_: &str) -> cpp::Type {
    qualified("std", name_)
}

fn zero_of(type_: cpp::Type) -> cpp::Expr {
    cpp::Expr::Construct {
        type_: Box::new(type_),
        args: Vec::new(),
        braced: true,
    }
}

/// The identifier a structure's functions are named by: its scope and its
/// name, so that two structures of two modules do not meet.
fn suffix(ast: &Ast, module: Id<ModuleNode>, simple: &str) -> Result<String, BindingError> {
    Ok(dds_type_name(ast, module, simple)?.replace("::", "_"))
}

/// `zd_enc_enh_<scope>_<Name>`.
pub(super) fn enc_struct(
    ast: &Ast,
    module: Id<ModuleNode>,
    simple: &str,
) -> Result<String, BindingError> {
    Ok(format!("zd_enc_enh_{}", suffix(ast, module, simple)?))
}

/// `zd_dec_enh_<scope>_<Name>`.
pub(super) fn dec_struct(
    ast: &Ast,
    module: Id<ModuleNode>,
    simple: &str,
) -> Result<String, BindingError> {
    Ok(format!("zd_dec_enh_{}", suffix(ast, module, simple)?))
}

fn exception_module(ast: &Ast, raised: &Raise) -> Result<Id<ModuleNode>, BindingError> {
    Ok(ast
        .exception(raised.id)
        .map_err(|error| BindingError::Codec(format!("an exception is gone: {error:?}")))?
        .module)
}

fn enc_exception(ast: &Ast, raised: &Raise) -> Result<String, BindingError> {
    let module = exception_module(ast, raised)?;
    Ok(format!(
        "zd_enc_enh_ex_{}",
        suffix(ast, module, &cxx_escape(&raised.name))?
    ))
}

fn dec_exception(ast: &Ast, raised: &Raise) -> Result<String, BindingError> {
    let module = exception_module(ast, raised)?;
    Ok(format!(
        "zd_dec_enh_ex_{}",
        suffix(ast, module, &cxx_escape(&raised.name))?
    ))
}

/// The pair of a structure's functions: parameters `(Writer&, const T&)`
/// and `(Reader&, T&)`.
fn pair(
    enc: &str,
    dec: &str,
    type_name: &str,
    encode: Vec<cpp::Statement>,
    decode: Vec<cpp::Statement>,
    what: &str,
) -> Vec<cpp::Decl> {
    let type_ = cpp::Type::named(Path::single(type_name));
    vec![
        function(
            enc,
            vec![
                param(writer_type(), "w"),
                param(const_ref(type_.clone()), "v"),
            ],
            with_unused(encode, "w"),
            &format!("The octets of {what}. DDS-RPC 1.0 §7.5.1.2."),
        ),
        function(
            dec,
            vec![param(reader_type(), "r"), param(mut_ref(type_), "v")],
            with_unused(decode, "r"),
            &format!("{what} from its octets. DDS-RPC 1.0 §7.5.1.2."),
        ),
    ]
}

/// One member of an `In` or an `Out`.
enum Slot {
    Typed(zerodds_ast_core::Id<zerodds_ast::TypeNode>),
    Dummy,
}

/// Write one member's value.
fn write_slot(ast: &Ast, slot: &Slot, member: &str) -> cpp::Statement {
    match slot {
        Slot::Typed(id) => encode_member(ast, *id, dot(name("v"), member)),
        Slot::Dummy => expr_statement(method(name("w"), "write_u8", vec![dot(name("v"), member)])),
    }
}

/// Read one member's value.
fn read_slot(ast: &Ast, slot: &Slot, member: &str) -> cpp::Statement {
    match slot {
        Slot::Typed(id) => decode_member(ast, *id, dot(name("v"), member)),
        Slot::Dummy => assign(
            dot(name("v"), member),
            method(name("r"), "read_u8", Vec::new()),
        ),
    }
}

fn slots_of(structure: &EnhancedStruct) -> Result<Vec<(String, u32, Slot)>, BindingError> {
    let mut out = Vec::new();
    for member in &structure.members {
        let slot = match &member.type_ {
            EnhancedMemberType::Idl(id) => Slot::Typed(*id),
            EnhancedMemberType::Dummy => Slot::Dummy,
            _ => {
                return Err(BindingError::Codec(format!(
                    "member `{}` of `{}` is not a member of an In or Out structure",
                    member.name, structure.name
                )));
            }
        };
        out.push((cxx_escape(&member.name), member.member_id, slot));
    }
    Ok(out)
}

/// The encoder and the decoder of an `In` or an `Out` structure.
fn fields_codec(
    ast: &Ast,
    module: Id<ModuleNode>,
    structure: &EnhancedStruct,
) -> Result<Vec<cpp::Decl>, BindingError> {
    let type_name = scoped(ast, module, &structure.name)?;
    let enc = enc_struct(ast, module, &structure.name)?;
    let dec = dec_struct(ast, module, &structure.name)?;
    let slots = slots_of(structure)?;
    let mut encode = Vec::new();
    let mut decode = Vec::new();
    if !structure.mutable {
        for (member, _, slot) in &slots {
            encode.push(write_slot(ast, slot, member));
            decode.push(read_slot(ast, slot, member));
        }
        return Ok(pair(
            &enc,
            &dec,
            &type_name,
            encode,
            decode,
            "one structure of the call",
        ));
    }

    // PL_CDR2: a DHEADER, then an EMHEADER and the value of each member.
    encode.push(cpp::Statement::Declare {
        type_: cpp::Type::Auto,
        name: Ident::new("zd_dheader"),
        value: Some(method(name("w"), "write_dheader", Vec::new())),
    });
    for (index, (member, id, slot)) in slots.iter().enumerate() {
        let code = match slot {
            Slot::Typed(target) => {
                crate::emit::interface_rust::emheader_length_code(ast, module, *target)
            }
            Slot::Dummy => 0,
        };
        let header = format!("zd_member_{index}");
        encode.push(cpp::Statement::Declare {
            type_: cpp::Type::Auto,
            name: Ident::new(&header),
            value: Some(method(
                name("w"),
                "write_emheader",
                vec![
                    int(i128::from(*id)),
                    int(i128::from(code)),
                    cpp::Expr::Literal(Literal::Bool(false)),
                ],
            )),
        });
        encode.push(write_slot(ast, slot, member));
        encode.push(expr_statement(method(
            name("w"),
            "finish_emheader",
            vec![name(&header)],
        )));
    }
    encode.push(expr_statement(method(
        name("w"),
        "finish_dheader",
        vec![name("zd_dheader")],
    )));

    // A member the peer did not send keeps the default of its type
    // (XTypes 1.3 §7.2.4.4.8), one this does not know is skipped by its
    // length.
    decode.push(assign(
        name("v"),
        zero_of(cpp::Type::named(Path::single(&type_name))),
    ));
    decode.extend(frame_open());
    let mut arms = Vec::new();
    for (member, id, slot) in &slots {
        arms.push((
            binary(
                "==",
                dot(name("zd_header"), "member_id"),
                int(i128::from(*id)),
            ),
            block(vec![read_slot(ast, slot, member)]),
        ));
    }
    let mut loop_body = member_header();
    if !arms.is_empty() {
        loop_body.extend(one_of(arms));
    }
    loop_body.push(skip_to("zd_after"));
    decode.push(cpp::Statement::While {
        condition: binary(
            "&&",
            binary(
                "<",
                method(name("r"), "position", Vec::new()),
                name("zd_end"),
            ),
            binary(
                "==",
                method(name("r"), "reader_status", Vec::new()),
                status_ok(),
            ),
        ),
        body: block(loop_body),
    });
    decode.push(skip_to("zd_end"));
    Ok(pair(
        &enc,
        &dec,
        &type_name,
        encode,
        decode,
        "one structure of the call",
    ))
}

/// `if (a) {..}` for each arm, one after the other: the conditions exclude
/// one another, and a chain of `else` would nest as deep as there are
/// operations, which a compiler limits.
fn one_of(arms: Vec<(cpp::Expr, cpp::Block)>) -> Vec<cpp::Statement> {
    arms.into_iter()
        .map(|(condition, then)| cpp::Statement::If {
            condition,
            then,
            otherwise: None,
        })
        .collect()
}

/// `r.reader_skip_to(target);`
fn skip_to(target: &str) -> cpp::Statement {
    expr_statement(method(name("r"), "reader_skip_to", vec![name(target)]))
}

/// `const std::uint32_t zd_length = r.read_dheader();`
/// `const std::size_t zd_end = r.position() + zd_length;`
fn frame_open() -> Vec<cpp::Statement> {
    vec![
        cpp::Statement::Declare {
            type_: cpp::Type::Const(Box::new(unsigned_type("uint32_t"))),
            name: Ident::new("zd_length"),
            value: Some(method(name("r"), "read_dheader", Vec::new())),
        },
        cpp::Statement::Declare {
            type_: cpp::Type::Const(Box::new(unsigned_type("size_t"))),
            name: Ident::new("zd_end"),
            value: Some(binary(
                "+",
                method(name("r"), "position", Vec::new()),
                name("zd_length"),
            )),
        },
    ]
}

/// The `EMHEADER` of the next member, and where that member ends.
fn member_header() -> Vec<cpp::Statement> {
    vec![
        cpp::Statement::Declare {
            type_: cpp::Type::Const(Box::new(cpp::Type::Auto)),
            name: Ident::new("zd_header"),
            value: Some(method(name("r"), "read_emheader", Vec::new())),
        },
        cpp::Statement::Declare {
            type_: cpp::Type::Const(Box::new(unsigned_type("size_t"))),
            name: Ident::new("zd_after"),
            value: Some(binary(
                "+",
                method(name("r"), "position", Vec::new()),
                dot(name("zd_header"), "extent"),
            )),
        },
    ]
}

/// One member of a choice structure.
struct Case {
    /// The C++ member, a `std::optional`.
    member: String,
    member_id: u32,
    /// The name of the member's encoder and decoder; `None` for `remoteEx`.
    functions: Option<(String, String)>,
}

/// The encoder and the decoder of a choice structure.
fn choice_codec(
    ast: &Ast,
    module: Id<ModuleNode>,
    structure: &EnhancedStruct,
    cases: &[Case],
    what: &str,
) -> Result<Vec<cpp::Decl>, BindingError> {
    let type_name = scoped(ast, module, &structure.name)?;
    let enc = enc_struct(ast, module, &structure.name)?;
    let dec = dec_struct(ast, module, &structure.name)?;

    let mut encode = vec![cpp::Statement::Declare {
        type_: cpp::Type::Primitive(cpp::Primitive::Int32),
        name: Ident::new("zd_present"),
        value: Some(int(0)),
    }];
    for case in cases {
        encode.push(cpp::Statement::If {
            condition: dot(name("v"), &case.member),
            then: block(vec![expr_statement(binary(
                "+=",
                name("zd_present"),
                int(1),
            ))]),
            otherwise: None,
        });
    }
    // "exactly one member shall be present at any given time" (§7.5.1.2.1.1).
    encode.push(cpp::Statement::If {
        condition: binary("!=", name("zd_present"), int(1)),
        then: block(vec![cpp::Statement::Throw(call(
            path(&["dds", "rpc", "UnsupportedError"]),
            vec![cpp::Expr::Literal(Literal::Str(format!(
                "{} holds exactly one member (DDS-RPC 1.0 §7.5.1.2.1.1)",
                structure.name
            )))],
        ))]),
        otherwise: None,
    });
    encode.push(cpp::Statement::Declare {
        type_: cpp::Type::Auto,
        name: Ident::new("zd_dheader"),
        value: Some(method(name("w"), "write_dheader", Vec::new())),
    });
    let mut encode_arms = Vec::new();
    for case in cases {
        let value = cpp::Expr::Deref(Box::new(dot(name("v"), &case.member)));
        let (code, write) = match &case.functions {
            Some((enc_member, _)) => (
                4,
                expr_statement(call(name(enc_member), vec![name("w"), value])),
            ),
            None => (
                2,
                expr_statement(method(
                    name("w"),
                    "write_u32",
                    vec![cpp::Expr::Cast {
                        kind: cpp::CastKind::Static,
                        type_: Box::new(unsigned_type("uint32_t")),
                        value: Box::new(value),
                    }],
                )),
            ),
        };
        encode_arms.push((
            dot(name("v"), &case.member),
            block(vec![
                cpp::Statement::Declare {
                    type_: cpp::Type::Auto,
                    name: Ident::new("zd_member"),
                    value: Some(method(
                        name("w"),
                        "write_emheader",
                        vec![
                            int(i128::from(case.member_id)),
                            int(code),
                            cpp::Expr::Literal(Literal::Bool(false)),
                        ],
                    )),
                },
                write,
                expr_statement(method(
                    name("w"),
                    "finish_emheader",
                    vec![name("zd_member")],
                )),
            ]),
        ));
    }
    encode.extend(one_of(encode_arms));
    encode.push(expr_statement(method(
        name("w"),
        "finish_dheader",
        vec![name("zd_dheader")],
    )));

    // Exactly one member is present; a member this does not know is
    // skipped by its length and leaves every case empty.
    let mut decode = vec![assign(
        name("v"),
        zero_of(cpp::Type::named(Path::single(&type_name))),
    )];
    decode.extend(frame_open());
    let mut decode_arms = Vec::new();
    for case in cases {
        let body = match &case.functions {
            Some((_, dec_member)) => vec![
                expr_statement(method(dot(name("v"), &case.member), "emplace", Vec::new())),
                expr_statement(call(
                    name(dec_member),
                    vec![
                        name("r"),
                        cpp::Expr::Deref(Box::new(dot(name("v"), &case.member))),
                    ],
                )),
            ],
            None => vec![
                cpp::Statement::Declare {
                    type_: cpp::Type::Const(Box::new(unsigned_type("uint32_t"))),
                    name: Ident::new("zd_code"),
                    value: Some(method(name("r"), "read_u32", Vec::new())),
                },
                // §7.5.2: a code outside the six is not one this binding
                // understands.
                cpp::Statement::If {
                    condition: binary(">", name("zd_code"), int(REMOTE_EX_MAX)),
                    then: block(vec![super::codec::status_refuse()]),
                    otherwise: Some(block(vec![assign(
                        dot(name("v"), &case.member),
                        cpp::Expr::Cast {
                            kind: cpp::CastKind::Static,
                            type_: Box::new(qualified("dds::rpc", "RemoteExceptionCode_t")),
                            value: Box::new(name("zd_code")),
                        },
                    )])),
                },
            ],
        };
        decode_arms.push((
            binary(
                "==",
                dot(name("zd_header"), "member_id"),
                int(i128::from(case.member_id)),
            ),
            block(body),
        ));
    }
    // §7.5.1.2.1.1: "exactly one member shall be present at any given
    // time". A structure with no member at all is as wrong as one with two;
    // the replier answers the request that cannot be decoded with
    // `REMOTE_EX_INVALID_ARGUMENT` (§7.5.2).
    decode.push(cpp::Statement::If {
        condition: binary("<", name("zd_length"), int(EMHEADER_SIZE)),
        then: block(vec![super::codec::status_refuse()]),
        otherwise: None,
    });
    let mut guarded = member_header();
    guarded.extend(one_of(decode_arms));
    guarded.push(skip_to("zd_after"));
    // A further member behind the first: two options are active.
    guarded.push(cpp::Statement::If {
        condition: binary(
            "<=",
            binary(
                "+",
                method(name("r"), "position", Vec::new()),
                int(EMHEADER_SIZE),
            ),
            name("zd_end"),
        ),
        then: block(vec![super::codec::status_refuse()]),
        otherwise: None,
    });
    decode.push(cpp::Statement::If {
        condition: binary(
            "&&",
            binary(
                "<",
                method(name("r"), "position", Vec::new()),
                name("zd_end"),
            ),
            binary(
                "==",
                method(name("r"), "reader_status", Vec::new()),
                status_ok(),
            ),
        ),
        then: block(guarded),
        otherwise: None,
    });
    decode.push(skip_to("zd_end"));
    Ok(pair(&enc, &dec, &type_name, encode, decode, what))
}

/// The raised exceptions of the operations, each once however many raise it.
fn raised_of(operations: &[&EnhancedOperation]) -> Vec<Raise> {
    let mut out: Vec<Raise> = Vec::new();
    for operation in operations {
        for raised in &operation.operation.raises {
            if !out.iter().any(|seen| seen.id == raised.id) {
                out.push(raised.clone());
            }
        }
    }
    out
}

/// The encoder and the decoder of one exception, in the module of the
/// exception.
fn exception_codec(ast: &Ast, raised: &Raise) -> Result<Vec<cpp::Decl>, BindingError> {
    let module = exception_module(ast, raised)?;
    let type_name = scoped(ast, module, &cxx_escape(&raised.name))?;
    let mut encode = Vec::new();
    let mut decode = Vec::new();
    for (member, id) in exception_members(ast, raised)? {
        encode.push(encode_member(ast, id, dot(name("v"), &member)));
        decode.push(decode_member(ast, id, dot(name("v"), &member)));
    }
    Ok(pair(
        &enc_exception(ast, raised)?,
        &dec_exception(ast, raised)?,
        &type_name,
        encode,
        decode,
        "one exception",
    ))
}

/// Everything the `dds_type_traits` of one interface's topic types stand
/// on: the codecs of the structures it declares itself, those of the
/// exceptions its operations raise (each exception once for the file,
/// whatever `declared` says), and those of its `Request` and `Reply`.
///
/// The structures of an inherited operation (§7.5.1.2.6) are written by the
/// base interface; this refers to them by name.
pub(super) fn codecs(
    ast: &Ast,
    service: &Service,
    model: &EnhancedService,
    declared: &mut Vec<Id<ExceptionNode>>,
) -> Result<Vec<cpp::Decl>, BindingError> {
    let module = service.module;
    let own: Vec<&EnhancedOperation> = model
        .operations
        .iter()
        .filter(|operation| operation.module == module && operation.interface == service.name)
        .collect();
    let mut out = Vec::new();
    for raised in raised_of(&own) {
        if declared.contains(&raised.id) {
            continue;
        }
        declared.push(raised.id);
        out.extend(exception_codec(ast, &raised)?);
    }
    for operation in &own {
        out.extend(fields_codec(ast, module, &operation.in_struct)?);
        out.extend(fields_codec(ast, module, &operation.out_struct)?);
        let mut cases = Vec::new();
        for member in &operation.result_struct.members {
            let functions = match &member.type_ {
                EnhancedMemberType::Struct(structure) => (
                    enc_struct(ast, module, structure)?,
                    dec_struct(ast, module, structure)?,
                ),
                EnhancedMemberType::Exception(raised) => {
                    (enc_exception(ast, raised)?, dec_exception(ast, raised)?)
                }
                _ => continue,
            };
            cases.push(Case {
                member: cxx_escape(&member.name),
                member_id: member.member_id,
                functions: Some(functions),
            });
        }
        out.extend(choice_codec(
            ast,
            module,
            &operation.result_struct,
            &cases,
            "the outcome of one operation",
        )?);
    }
    for (structure, reply) in [(&model.request, false), (&model.reply, true)] {
        let mut cases = Vec::new();
        for member in &structure.members {
            match &member.type_ {
                EnhancedMemberType::Struct(name_) => {
                    let owner = model
                        .operations
                        .iter()
                        .find(|operation| operation.operation.name == member.name)
                        .map_or(module, |operation| operation.module);
                    cases.push(Case {
                        member: cxx_escape(&member.name),
                        member_id: member.member_id,
                        functions: Some((
                            enc_struct(ast, owner, name_)?,
                            dec_struct(ast, owner, name_)?,
                        )),
                    });
                }
                EnhancedMemberType::RemoteException => cases.push(Case {
                    member: cxx_escape(&member.name),
                    member_id: member.member_id,
                    functions: None,
                }),
                _ => {}
            }
        }
        out.extend(choice_codec(
            ast,
            module,
            structure,
            &cases,
            if reply { "one reply" } else { "one request" },
        )?);
    }
    Ok(out)
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

/// `namespace dds::rpc { template <> struct dds_type_traits<T> {..}; }` for
/// the request and the reply type of one interface.
///
/// The sample is the bare structure, PL_CDR2, little-endian: whatever the
/// `WireEncoding` of the params says, the clause fixes this form (§7.5.1.2).
pub(super) fn traits(
    ast: &Ast,
    service: &Service,
    model: &EnhancedService,
) -> Result<cpp::Decl, BindingError> {
    let mut decls = Vec::new();
    let octets = qualified("dds::rpc", "Octets");
    let encoding = qualified("dds::rpc", "WireEncoding");
    let wire = |last: &str| Path::new(&["zerodds", "wire", last]);
    let little = path(&["zerodds", "wire", "Endianness", "Little"]);
    // XCDR2 caps every alignment at four (XTypes 1.3 §7.4.1).
    let alignment = int(4);
    // The XTypes description of the two topic types; a type with no
    // TypeObject (a recursive one) leaves the topics matched by type name.
    let objects =
        crate::emit::interface_rust::type_information::service_type_objects(ast, service, model)
            .ok();
    for (structure, reply) in [(&model.request, false), (&model.reply, true)] {
        let type_name = scoped(ast, service.module, &structure.name)?;
        let dds_name = dds_type_name(ast, service.module, &structure.name)?;
        let topic_type = cpp::Type::named(Path::single(&type_name));
        let enc = enc_struct(ast, service.module, &structure.name)?;
        let dec = dec_struct(ast, service.module, &structure.name)?;
        let unnamed = |type_: cpp::Type| cpp::Param {
            type_,
            name: None,
            default: None,
        };

        let serialize_at = static_method(
            octets.clone(),
            "serialize_at",
            vec![
                param(const_ref(topic_type.clone()), "value"),
                unnamed(qualified("std", "size_t")),
                unnamed(encoding.clone()),
            ],
            vec![
                cpp::Statement::Declare {
                    type_: cpp::Type::named(wire("Writer")),
                    name: Ident::new("w"),
                    value: Some(cpp::Expr::Construct {
                        type_: Box::new(cpp::Type::named(wire("Writer"))),
                        args: vec![little.clone(), alignment.clone()],
                        braced: false,
                    }),
                },
                expr_statement(call(name(&enc), vec![name("w"), name("value")])),
                cpp::Statement::Return(Some(method(name("w"), "bytes", Vec::new()))),
            ],
            "The octets of the structure. The sample has no header (§7.8.2): it starts at \
             offset 0 and is PL_CDR2 whatever the encoding of the params.",
        );
        let serialize = static_method(
            octets.clone(),
            "serialize",
            vec![param(const_ref(topic_type.clone()), "value")],
            vec![cpp::Statement::Return(Some(call(
                name("serialize_at"),
                vec![
                    name("value"),
                    int(0),
                    path(&["dds", "rpc", "WireEncoding", "XCDR2"]),
                ],
            )))],
            "The octets of the structure on their own.",
        );
        let deserialize = |with_encoding: bool| {
            let mut params = vec![
                param(const_ref(octets.clone()), "in"),
                param(mut_ref(qualified("std", "size_t")), "at"),
                param(mut_ref(topic_type.clone()), "out"),
            ];
            let statements = if with_encoding {
                params.push(unnamed(encoding.clone()));
                vec![
                    cpp::Statement::Declare {
                        type_: cpp::Type::named(wire("Reader")),
                        name: Ident::new("r"),
                        value: Some(cpp::Expr::Construct {
                            type_: Box::new(cpp::Type::named(wire("Reader"))),
                            args: vec![name("in"), little.clone(), alignment.clone()],
                            braced: false,
                        }),
                    },
                    expr_statement(method(name("r"), "seek", vec![name("at")])),
                    expr_statement(call(name(&dec), vec![name("r"), name("out")])),
                    assign(name("at"), method(name("r"), "position", Vec::new())),
                    cpp::Statement::Return(Some(binary(
                        "==",
                        method(name("r"), "reader_status", Vec::new()),
                        status_ok(),
                    ))),
                ]
            } else {
                vec![cpp::Statement::Return(Some(call(
                    name("deserialize"),
                    vec![
                        name("in"),
                        name("at"),
                        name("out"),
                        path(&["dds", "rpc", "WireEncoding", "XCDR2"]),
                    ],
                )))]
            };
            static_method(
                cpp::Type::Primitive(cpp::Primitive::Bool),
                "deserialize",
                params,
                statements,
                "The structure from the octets at `at`.",
            )
        };
        let type_name_method = static_method(
            cpp::Type::Pointer(Box::new(cpp::Type::Const(Box::new(cpp::Type::Primitive(
                cpp::Primitive::Char,
            ))))),
            "type_name",
            Vec::new(),
            vec![cpp::Statement::Return(Some(cpp::Expr::Literal(
                Literal::Str(dds_name),
            )))],
            "The DDS type name, §7.5.1.2.4 and §7.5.1.2.5.",
        );
        let mut members = vec![
            type_name_method,
            serialize_at,
            serialize,
            deserialize(true),
            deserialize(false),
        ];
        if let Some(objects) = &objects {
            let topic = if reply {
                &objects.reply
            } else {
                &objects.request
            };
            let bytes = |octets: &Vec<u8>| cpp::Expr::Literal(Literal::Bytes(octets.clone()));
            members.push(static_method(
                cpp::Type::Named {
                    path: Path::new(&["std", "shared_ptr"]),
                    args: vec![cpp::Type::Const(Box::new(cpp::Type::named(Path::new(&[
                        "dds",
                        "rpc",
                        "TypeInformation",
                    ]))))],
                },
                "type_information",
                Vec::new(),
                vec![cpp::Statement::Return(Some(call(
                    path(&["dds", "rpc", "make_type_information"]),
                    vec![
                        bytes(&topic.object),
                        cpp::Expr::InitList(topic.dependencies.iter().map(bytes).collect()),
                    ],
                )))],
                "The XTypes description of the topic type: its CompleteTypeObject and the \
                 objects it names (DDS-RPC 1.0 §7.5.1.2, §7.7.2.5).",
            ));
        }
        if reply {
            // §7.5.1.2.5 rule 5: the code is the `remoteEx` member of the
            // reply, so the runtime asks the type for it.
            let remote_ex = structure
                .members
                .iter()
                .find(|member| member.type_ == EnhancedMemberType::RemoteException)
                .map(|member| cxx_escape(&member.name))
                .ok_or_else(|| {
                    BindingError::Codec("the reply structure has no remoteEx member".to_string())
                })?;
            members.push(static_method(
                qualified("dds::rpc", "RemoteExceptionCode_t"),
                "remote_exception",
                vec![param(const_ref(topic_type.clone()), "value")],
                vec![
                    cpp::Statement::If {
                        condition: dot(name("value"), &remote_ex),
                        then: block(vec![cpp::Statement::Return(Some(cpp::Expr::Deref(
                            Box::new(dot(name("value"), &remote_ex)),
                        )))]),
                        otherwise: None,
                    },
                    cpp::Statement::Return(Some(path(&["dds", "rpc", "REMOTE_EX_OK"]))),
                ],
                "The remote exception code the reply carries, REMOTE_EX_OK if none.",
            ));
            members.push(static_method(
                topic_type.clone(),
                "from_remote_exception",
                vec![param(
                    qualified("dds::rpc", "RemoteExceptionCode_t"),
                    "code",
                )],
                vec![
                    cpp::Statement::Declare {
                        type_: topic_type.clone(),
                        name: Ident::new("zd_reply"),
                        value: Some(zero_of(topic_type.clone())),
                    },
                    assign(dot(name("zd_reply"), &remote_ex), name("code")),
                    cpp::Statement::Return(Some(name("zd_reply"))),
                ],
                "A reply that carries `code` and nothing else.",
            ));
        }
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
                    members: members.into_iter().collect(),
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
