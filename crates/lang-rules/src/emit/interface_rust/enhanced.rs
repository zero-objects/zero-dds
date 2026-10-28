// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced Service Mapping of DDS-RPC 1.0 §7.5.1.2 as Rust.
//!
//! `${I}_Request` and `${I}_Reply` are `@Choice @Autoid` structures
//! (§7.5.1.2.4, §7.5.1.2.5): "the structure shall be interpreted as if
//! the structure had the `@Extensibility(MUTABLE_EXTENSIBILITY)`
//! annotation and all the members of the structure had the `@Optional`
//! annotation. Furthermore, exactly one member shall be present at any
//! given time" (§7.5.1.2.1.1). A Rust `enum` is that: one variant per
//! member, and its codec is a PL_CDR2 object (XTypes 1.3 §7.4.3.4) with
//! the one member present: a `DHEADER`, an `EMHEADER` whose member id is
//! the HASH of the member name (§7.5.1.2.1.2) and the member.
//!
//! A member the binding does not know is a variant of its own,
//! `UnknownOp(member id)`, so that a service can answer
//! `REMOTE_EX_UNKNOWN_OPERATION` (§7.7.2) instead of failing to decode.
//!
//! The `In`, `Out` and `Result` structures reuse the Basic mapping's
//! members and codec where they are `@final`; an operation with
//! `@Mutable` gets the PL_CDR2 form of `In` and `Out` (§7.5.1.2.2,
//! §7.5.1.2.3 rule 1) and of `Result` (rule 7), which is an
//! `EMHEADER` per member.
//!
//! The generated client and dispatcher are the Basic mapping's, over
//! `${I}_Call` and `${I}_Return` declared as aliases of the two choice
//! types, and `zerodds_rpc::endpoint_options::RpcMapping::Enhanced`.
//! Their variants are the same, so the same match arms read both.

use super::*;
use crate::adapter::interface::{
    EnhancedMemberType, EnhancedOperation, EnhancedService, EnhancedStruct,
};

/// How a case of a choice structure is written inside its `EMHEADER`.
enum CaseBody {
    /// A structure with its own `encode`/`decode`, at this path.
    Payload(Path),
    /// `dds::rpc::RemoteExceptionCode_t`, an enumeration of four octets.
    RemoteException,
}

/// What a choice structure's decoder does with a member id it does not
/// know: it keeps the id in a variant of its own.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Unknown {
    /// `UnknownOp(member id)`: a request for an operation the interface
    /// does not have (DDS-RPC 1.0 §7.7.2).
    Operation,
    /// `UnknownEx(member id)`: an exception member of a `Result` that
    /// this version of the interface does not declare (§7.7.2.5.6).
    Exception,
}

impl Unknown {
    /// The variant that holds the unknown member id.
    const fn variant(self) -> &'static str {
        match self {
            Self::Operation => UNKNOWN_OP_VARIANT,
            Self::Exception => UNKNOWN_EX_VARIANT,
        }
    }
}

/// One member of a choice structure.
struct ChoiceCase {
    variant: String,
    body: CaseBody,
    member_id: u32,
    doc: String,
}

fn path(parts: &[&str]) -> rust::Expr {
    rust::Expr::Path(Path::new(parts))
}

fn local(name: &str) -> rust::Expr {
    rust::Expr::Path(Path::single(name))
}

fn int(value: i128) -> rust::Expr {
    rust::Expr::Literal(Literal::int(value))
}

fn call(callee: rust::Expr, args: Vec<rust::Expr>) -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(callee),
        args,
    }
}

fn method(receiver: rust::Expr, name: &str, args: Vec<rust::Expr>) -> rust::Expr {
    rust::Expr::MethodCall {
        receiver: Box::new(receiver),
        method: Ident::new(name),
        args,
    }
}

fn try_(expr: rust::Expr) -> rust::Expr {
    rust::Expr::Try(Box::new(expr))
}

fn let_(name: &str, value: rust::Expr, mutable: bool) -> rust::Statement {
    rust::Statement::Let {
        name: Ident::new(name),
        type_: None,
        value: Some(value),
        mutable,
    }
}

fn field(receiver: rust::Expr, name: &str) -> rust::Expr {
    rust::Expr::Field {
        receiver: Box::new(receiver),
        name: Ident::new(name),
    }
}

/// `Err(WireError::refused(<position>, "<why>"))`.
fn refused(position_of: &str, why: &str) -> rust::Expr {
    call(
        local("Err"),
        vec![call(
            path(&["WireError", "refused"]),
            vec![
                method(local(position_of), "position", Vec::new()),
                rust::Expr::Literal(Literal::string(why)),
            ],
        )],
    )
}

fn ok(value: rust::Expr) -> rust::Expr {
    call(local("Ok"), vec![value])
}

/// The length code of an `EMHEADER` for a member of this kind
/// (XTypes 1.3 §7.4.3.4.2): 0 to 3 for a member of 1, 2, 4 or 8 octets,
/// 4 — a `NEXTINT` with the length — for any other.
pub(super) fn length_code(kind: &MemberWrite) -> u8 {
    match kind {
        MemberWrite::Primitive(primitive) => match primitive {
            PrimitiveKind::Boolean
            | PrimitiveKind::Octet
            | PrimitiveKind::Char
            | PrimitiveKind::Int8
            | PrimitiveKind::UInt8 => 0,
            PrimitiveKind::Int16 | PrimitiveKind::UInt16 => 1,
            PrimitiveKind::Int32 | PrimitiveKind::UInt32 | PrimitiveKind::Float32 => 2,
            PrimitiveKind::Int64 | PrimitiveKind::UInt64 | PrimitiveKind::Float64 => 3,
            _ => 4,
        },
        _ => 4,
    }
}

/// The path to a structure declared in module `module`, from module
/// `from`.
fn struct_path(
    ast: &Ast,
    from: Id<ModuleNode>,
    module: Id<ModuleNode>,
    name: &str,
) -> Result<Path, RustInterfaceError> {
    if from == module {
        Ok(Path::single(name))
    } else {
        relative_module_path(ast, from, module, name)
    }
}

/// The enum a choice structure is, and its `UnknownOp` variant when it
/// has one.
fn choice_enum(name: &str, doc: Doc, cases: &[ChoiceCase], unknown: Unknown) -> rust::Item {
    let mut variants = Vec::new();
    for case in cases {
        let payload = match &case.body {
            CaseBody::Payload(path) => rust::Type::named(path.clone()),
            CaseBody::RemoteException => rust::Type::named(Path::new(&[
                "zerodds_rpc",
                "common_types",
                "RemoteExceptionCode",
            ])),
        };
        variants.push(rust::Variant {
            doc: Doc::line(&case.doc),
            attrs: Vec::new(),
            name: Ident::new(&case.variant),
            body: rust::VariantBody::Tuple(vec![payload]),
            discriminant: None,
        });
    }
    {
        let variant = unknown.variant();
        let variant_doc = if unknown == Unknown::Exception {
            "An exception member this binding does not know, by its member id: the \
             client reports `REMOTE_EX_UNKNOWN_EXCEPTION` (DDS-RPC 1.0 §7.7.2.5.6)."
        } else {
            "A member this binding does not know, by its member id: a request for an \
             operation the interface does not have is answered with \
             `REMOTE_EX_UNKNOWN_OPERATION` (DDS-RPC 1.0 §7.7.2)."
        };
        variants.push(rust::Variant {
            doc: Doc::line(variant_doc),
            attrs: Vec::new(),
            name: Ident::new(variant),
            body: rust::VariantBody::Tuple(vec![rust::Type::Primitive(rust::Primitive::U32)]),
            discriminant: None,
        });
    }
    rust::Item::Enum(rust::Enum {
        doc,
        attrs: vec![allow_non_camel_case()],
        derives: data_derives(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        generics: Vec::new(),
        variants,
    })
}

/// The `encode`/`decode` of a choice structure: PL_CDR2 with the one
/// member present.
fn choice_codec(name: &str, cases: &[ChoiceCase], unknown: Unknown) -> rust::Item {
    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();
    for case in cases {
        encode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::TupleStruct {
                path: Path::new(&["Self", &case.variant]),
                elements: vec![rust::Pattern::Binding(Ident::new("value"))],
            },
            guard: None,
            body: rust::Expr::Block(rust::Block {
                statements: case_encode(case),
            }),
        });
        decode_arms.push(rust::MatchArm {
            pattern: rust::Pattern::Literal(Literal::int(i128::from(case.member_id))),
            guard: None,
            body: case_decode(case),
        });
    }
    let variant = unknown.variant();
    encode_arms.push(rust::MatchArm {
        pattern: rust::Pattern::TupleStruct {
            path: Path::new(&["Self", variant]),
            elements: vec![rust::Pattern::Wildcard],
        },
        guard: None,
        body: rust::Expr::Return(Some(Box::new(refused(
            "w",
            "an unknown member has no body to write",
        )))),
    });
    decode_arms.push(rust::MatchArm {
        pattern: rust::Pattern::Wildcard,
        guard: None,
        body: call(
            path(&["Self", variant]),
            vec![field(local("header"), "member_id")],
        ),
    });

    let encode_statements = vec![
        let_(
            "dheader",
            method(local("w"), "write_dheader", Vec::new()),
            false,
        ),
        rust::Statement::Semi(rust::Expr::Match {
            scrutinee: Box::new(local("self")),
            arms: encode_arms,
        }),
        rust::Statement::Semi(method(local("w"), "finish_dheader", vec![local("dheader")])),
        rust::Statement::Tail(ok_unit()),
    ];
    // The size of an `EMHEADER`: what is left after the one member, when
    // it is at least this much, is a second member.
    let emheader_size = || rust::Expr::Literal(Literal::int(4));
    let decode_statements = vec![
        let_(
            "length",
            try_(method(local("r"), "read_dheader", Vec::new())),
            false,
        ),
        // §7.5.1.2.1.1: "exactly one member shall be present at any given
        // time" — none is as wrong as two.
        rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::Lt,
                lhs: Box::new(local("length")),
                rhs: Box::new(emheader_size()),
            }),
            then: rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Return(Some(Box::new(
                    refused("r", "no member of the @Choice structure is present"),
                ))))],
            },
            otherwise: None,
        }),
        let_(
            "end",
            rust::Expr::Binary {
                op: rust::BinaryOp::Add,
                lhs: Box::new(method(local("r"), "position", Vec::new())),
                rhs: Box::new(local("length")),
            },
            false,
        ),
        let_(
            "header",
            try_(method(local("r"), "read_emheader", Vec::new())),
            false,
        ),
        let_(
            "after",
            rust::Expr::Binary {
                op: rust::BinaryOp::Add,
                lhs: Box::new(method(local("r"), "position", Vec::new())),
                rhs: Box::new(field(local("header"), "extent")),
            },
            false,
        ),
        let_(
            "value",
            rust::Expr::Match {
                scrutinee: Box::new(field(local("header"), "member_id")),
                arms: decode_arms,
            },
            false,
        ),
        // Whatever the member did not read is skipped. A further member
        // behind it is two options active in a `@Choice` structure
        // (§7.5.1.2.1.1); the service answers `REMOTE_EX_INVALID_ARGUMENT`
        // for the request that cannot be decoded (§7.5.2).
        rust::Statement::Semi(try_(method(local("r"), "skip_to", vec![local("after")]))),
        rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::Ge,
                lhs: Box::new(local("end")),
                rhs: Box::new(rust::Expr::Binary {
                    op: rust::BinaryOp::Add,
                    lhs: Box::new(method(local("r"), "position", Vec::new())),
                    rhs: Box::new(emheader_size()),
                }),
            }),
            then: rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Return(Some(Box::new(
                    refused("r", "two members of the @Choice structure are present"),
                ))))],
            },
            otherwise: None,
        }),
        rust::Statement::Semi(try_(method(local("r"), "skip_to", vec![local("end")]))),
        rust::Statement::Tail(ok(local("value"))),
    ];
    rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust::Type::named(Path::single(name)),
        items: vec![
            encode_fn_item_named("encode", name, encode_statements),
            decode_fn_item_named("decode", name, decode_statements),
        ],
    })
}

/// The statements of one arm of a choice's `encode`.
fn case_encode(case: &ChoiceCase) -> Vec<rust::Statement> {
    let (code, write) = match &case.body {
        CaseBody::Payload(_) => (4, try_(method(local("value"), "encode", vec![local("w")]))),
        CaseBody::RemoteException => (
            2,
            method(
                local("w"),
                "write_u32",
                vec![method(local("value"), "as_u32", Vec::new())],
            ),
        ),
    };
    vec![
        let_(
            "member",
            try_(method(
                local("w"),
                "write_emheader",
                vec![
                    int(i128::from(case.member_id)),
                    int(code),
                    rust::Expr::Literal(Literal::Bool(false)),
                ],
            )),
            false,
        ),
        rust::Statement::Semi(write),
        rust::Statement::Semi(try_(method(
            local("w"),
            "finish_emheader",
            vec![local("member")],
        ))),
    ]
}

/// The value one arm of a choice's `decode` reads.
fn case_decode(case: &ChoiceCase) -> rust::Expr {
    match &case.body {
        CaseBody::Payload(payload) => call(
            path(&["Self", &case.variant]),
            vec![try_(call(
                rust::Expr::Path(payload.join("decode")),
                vec![local("r")],
            ))],
        ),
        CaseBody::RemoteException => call(
            path(&["Self", &case.variant]),
            vec![rust::Expr::Match {
                scrutinee: Box::new(call(
                    path(&[
                        "zerodds_rpc",
                        "common_types",
                        "RemoteExceptionCode",
                        "from_u32",
                    ]),
                    vec![try_(method(local("r"), "read_u32", Vec::new()))],
                )),
                arms: vec![
                    rust::MatchArm {
                        pattern: rust::Pattern::TupleStruct {
                            path: Path::single("Ok"),
                            elements: vec![rust::Pattern::Binding(Ident::new("code"))],
                        },
                        guard: None,
                        body: local("code"),
                    },
                    rust::MatchArm {
                        pattern: rust::Pattern::Wildcard,
                        guard: None,
                        body: rust::Expr::Return(Some(Box::new(refused(
                            "r",
                            "a remote exception code outside DDS-RPC 1.0 §7.5.2",
                        )))),
                    },
                ],
            }],
        ),
    }
}

/// The PL_CDR2 form of a structure with `@Mutable`: a `DHEADER`, then an
/// `EMHEADER` and the value of each member, in any order on decode.
fn mutable_struct_codec(
    ast: &Ast,
    from: Id<ModuleNode>,
    type_name: &str,
    fields: &[FieldSpec],
    ids: &[u32],
) -> Result<rust::Item, RustInterfaceError> {
    let mut encode_statements = vec![let_(
        "dheader",
        method(local("w"), "write_dheader", Vec::new()),
        false,
    )];
    let mut decode_statements = vec![
        let_(
            "length",
            try_(method(local("r"), "read_dheader", Vec::new())),
            false,
        ),
        let_(
            "end",
            rust::Expr::Binary {
                op: rust::BinaryOp::Add,
                lhs: Box::new(method(local("r"), "position", Vec::new())),
                rhs: Box::new(local("length")),
            },
            false,
        ),
    ];
    let mut slots = Vec::new();
    let mut arms = Vec::new();
    let mut struct_fields = Vec::new();
    for (spec, id) in fields.iter().zip(ids) {
        let (name, kind) = match spec {
            FieldSpec::Real(name, target) => (
                name.clone(),
                member_write_with(ast, from, *target, CodecVariant::Xcdr2),
            ),
            FieldSpec::Octet(name) => (name.clone(), MemberWrite::Primitive(PrimitiveKind::Octet)),
        };
        let code = length_code(&kind);
        encode_statements.push(let_(
            "member",
            try_(method(
                local("w"),
                "write_emheader",
                vec![
                    int(i128::from(*id)),
                    int(i128::from(code)),
                    rust::Expr::Literal(Literal::Bool(false)),
                ],
            )),
            false,
        ));
        encode_statements.push(write_value_to("w", &name, &kind, self_field(&name), false)?);
        encode_statements.push(rust::Statement::Semi(try_(method(
            local("w"),
            "finish_emheader",
            vec![local("member")],
        ))));

        let slot = format!("slot_{name}");
        slots.push(let_(&slot, local("None"), true));
        // One `if` per member and not a `match`: a structure of one member
        // would be a `match` of one arm.
        arms.push(rust::Statement::Semi(rust::Expr::If {
            condition: Box::new(rust::Expr::Binary {
                op: rust::BinaryOp::Eq,
                lhs: Box::new(field(local("header"), "member_id")),
                rhs: Box::new(int(i128::from(*id))),
            }),
            then: rust::Block {
                statements: vec![rust::Statement::Semi(rust::Expr::Assign {
                    target: Box::new(local(&slot)),
                    value: Box::new(call(local("Some"), vec![try_(read_for(&name, &kind)?)])),
                })],
            },
            otherwise: None,
        }));
        let absent_default = match spec {
            FieldSpec::Real(_, target) => {
                crate::default_rust::default_of(ast, from, *target, &name)?
            }
            FieldSpec::Octet(_) => int(0),
        };
        struct_fields.push((
            Ident::new(&name),
            // XTypes 1.3 §7.2.4.4.8 (assignability of aggregated types):
            // "Each non-optional member in a T1 object that is not present
            // in the T2 object takes its default value." A peer that does
            // not send a member of a mutable type, one that predates it, is
            // read with the default.
            method(local(&slot), "unwrap_or", vec![absent_default]),
        ));
    }
    encode_statements.push(rust::Statement::Semi(method(
        local("w"),
        "finish_dheader",
        vec![local("dheader")],
    )));
    encode_statements.push(rust::Statement::Tail(ok_unit()));

    decode_statements.extend(slots);
    decode_statements.push(rust::Statement::Semi(rust::Expr::While {
        condition: Box::new(rust::Expr::Binary {
            op: rust::BinaryOp::Lt,
            lhs: Box::new(method(local("r"), "position", Vec::new())),
            rhs: Box::new(local("end")),
        }),
        body: rust::Block {
            statements: vec![
                let_(
                    "header",
                    try_(method(local("r"), "read_emheader", Vec::new())),
                    false,
                ),
                let_(
                    "after",
                    rust::Expr::Binary {
                        op: rust::BinaryOp::Add,
                        lhs: Box::new(method(local("r"), "position", Vec::new())),
                        rhs: Box::new(field(local("header"), "extent")),
                    },
                    false,
                ),
            ]
            .into_iter()
            .chain(arms)
            .chain([rust::Statement::Semi(try_(method(
                local("r"),
                "skip_to",
                vec![local("after")],
            )))])
            .collect(),
        },
    }));
    decode_statements.push(rust::Statement::Tail(ok(rust::Expr::StructLit {
        path: Path::single("Self"),
        fields: struct_fields,
        base: None,
    })));

    Ok(rust::Item::Impl(rust::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust::Type::named(Path::single(type_name)),
        items: vec![
            encode_fn_item_named("encode", type_name, encode_statements),
            decode_fn_item_named("decode", type_name, decode_statements),
        ],
    }))
}

/// The member ids of a structure's members, in order.
fn ids_of(structure: &EnhancedStruct) -> Vec<u32> {
    structure.members.iter().map(|m| m.member_id).collect()
}

/// The cases of a `Result` structure (§7.5.1.2.3).
fn result_cases(
    ast: &Ast,
    module: Id<ModuleNode>,
    operation: &EnhancedOperation,
) -> Result<Vec<ChoiceCase>, RustInterfaceError> {
    let mut cases = Vec::new();
    for member in &operation.result_struct.members {
        match &member.type_ {
            EnhancedMemberType::Struct(name) => cases.push(ChoiceCase {
                variant: "Ok".to_string(),
                body: CaseBody::Payload(Path::single(name)),
                member_id: member.member_id,
                doc: "`result`. DDS-RPC 1.0 §7.5.1.2.3 rule 5.".to_string(),
            }),
            EnhancedMemberType::Exception(raise) => cases.push(ChoiceCase {
                variant: rust::escape_keyword(&raise.name),
                body: CaseBody::Payload(exception_path(ast, module, raise.id)?),
                member_id: member.member_id,
                doc: format!("`{}`. DDS-RPC 1.0 §7.5.1.2.3 rule 6.", member.name),
            }),
            _ => {}
        }
    }
    Ok(cases)
}

/// The cases of `${I}_Request` or `${I}_Reply`.
fn service_cases(
    ast: &Ast,
    module: Id<ModuleNode>,
    service: &EnhancedService,
    reply: bool,
) -> Result<Vec<ChoiceCase>, RustInterfaceError> {
    let structure = if reply {
        &service.reply
    } else {
        &service.request
    };
    let mut cases = Vec::new();
    for member in &structure.members {
        match &member.type_ {
            EnhancedMemberType::Struct(name) => {
                let owner = service
                    .operations
                    .iter()
                    .find(|operation| operation.operation.name == member.name)
                    .map_or(module, |operation| operation.module);
                cases.push(ChoiceCase {
                    variant: variant_name(&member.name),
                    body: CaseBody::Payload(struct_path(ast, module, owner, name)?),
                    member_id: member.member_id,
                    doc: format!(
                        "`{}`. DDS-RPC 1.0 §7.5.1.2.{}.",
                        member.name,
                        if reply { 5 } else { 4 }
                    ),
                });
            }
            EnhancedMemberType::RemoteException => cases.push(ChoiceCase {
                variant: "RemoteEx".to_string(),
                body: CaseBody::RemoteException,
                member_id: member.member_id,
                doc: format!(
                    "`{}`: the remote exception code. DDS-RPC 1.0 §7.5.1.2.5 rule 5.",
                    member.name
                ),
            }),
            _ => {}
        }
    }
    Ok(cases)
}

/// What `declare` writes for one interface under the Enhanced Service
/// Mapping.
pub(super) fn declare(
    ast: &Ast,
    service: &Service,
    out: &mut Vec<(Id<ModuleNode>, rust::Item)>,
) -> Result<(), RustInterfaceError> {
    // The client and the dispatcher are shared with the Basic mapping and
    // read which of the two they bind off the service.
    let service = &Service {
        mapping: if service.mapping.is_enhanced() {
            service.mapping
        } else {
            interface::ServiceMapping::Enhanced
        },
        ..service.clone()
    };
    let module = service.module;
    let ops = service.all_operations();
    let model = interface::enhanced_service(ast, service, service.mapping.empty_style())?;

    // The exceptions the interface's own operations raise, one codec each
    // in the exception's module.
    for raise in exceptions_of(&ops) {
        out.push((
            exception_module(ast, raise.id)?,
            exception_codec_item(ast, raise.id)?,
        ));
    }

    for operation in model
        .operations
        .iter()
        .filter(|operation| operation.module == module && operation.interface == service.name)
    {
        let op = &operation.operation;
        out.push((module, rust::Item::Struct(in_struct(ast, service, op)?)));
        let in_name = &operation.in_struct.name;
        if operation.in_struct.mutable {
            out.push((
                module,
                mutable_struct_codec(
                    ast,
                    module,
                    in_name,
                    &in_fields_styled(op, service.mapping.empty_style()),
                    &ids_of(&operation.in_struct),
                )?,
            ));
        } else {
            out.push((
                module,
                struct_codec_item(
                    ast,
                    module,
                    in_name,
                    &in_fields_styled(op, service.mapping.empty_style()),
                )?,
            ));
        }
        out.push((module, rust::Item::Struct(out_struct(ast, service, op)?)));
        let out_name = &operation.out_struct.name;
        if operation.out_struct.mutable {
            out.push((
                module,
                mutable_struct_codec(
                    ast,
                    module,
                    out_name,
                    &out_fields_styled(op, service.mapping.empty_style()),
                    &ids_of(&operation.out_struct),
                )?,
            ));
        } else {
            out.push((
                module,
                struct_codec_item(
                    ast,
                    module,
                    out_name,
                    &out_fields_styled(op, service.mapping.empty_style()),
                )?,
            ));
        }
        let cases = result_cases(ast, module, operation)?;
        out.push((
            module,
            choice_enum(
                &operation.result_struct.name,
                Doc::line(&format!(
                    "The outcome of `{}`: a `@Choice` over the result and the exceptions. \
                     DDS-RPC 1.0 §7.5.1.2.3.",
                    op.name
                )),
                &cases,
                Unknown::Exception,
            ),
        ));
        out.push((
            module,
            choice_codec(&operation.result_struct.name, &cases, Unknown::Exception),
        ));
    }

    let request = service_cases(ast, module, &model, false)?;
    let request_name = interface::request_type_name(&service.name);
    out.push((
        module,
        choice_enum(
            &request_name,
            Doc::line(
                "The request topic type: a `@Choice @Autoid` over every operation, the \
                 inherited ones included. DDS-RPC 1.0 §7.5.1.2.4.",
            ),
            &request,
            Unknown::Operation,
        ),
    ));
    out.push((
        module,
        choice_codec(&request_name, &request, Unknown::Operation),
    ));
    let reply = service_cases(ast, module, &model, true)?;
    let reply_name = interface::reply_type_name(&service.name);
    out.push((
        module,
        choice_enum(
            &reply_name,
            Doc::line(
                "The reply topic type: a `@Choice @Autoid` over every operation's result and \
                 `remoteEx`. DDS-RPC 1.0 §7.5.1.2.5.",
            ),
            &reply,
            Unknown::Operation,
        ),
    ));
    out.push((
        module,
        choice_codec(&reply_name, &reply, Unknown::Operation),
    ));

    // The Basic mapping's client and dispatcher name `${I}_Call` and
    // `${I}_Return`; here they are the two choice types.
    for (alias, target) in [
        (interface::call_union_name(&service.name), &request_name),
        (interface::return_union_name(&service.name), &reply_name),
    ] {
        out.push((
            module,
            rust::Item::TypeAlias(rust::TypeAlias {
                doc: Doc::line(
                    "The name the client and the dispatcher of this binding use for the topic type.",
                ),
                attrs: vec![allow_non_camel_case()],
                visibility: Visibility::Public,
                name: Ident::new(&alias),
                generics: Vec::new(),
                type_: rust::Type::named(Path::single(target)),
            }),
        ));
    }

    let exceptions = exceptions_of(&ops);
    out.push((
        module,
        rust::Item::Enum(error_enum(ast, module, service, &exceptions)?),
    ));
    out.push((module, dds_type_impl(&request_name, &request_name, false)));
    out.push((module, dds_type_impl(&reply_name, &reply_name, false)));

    if service.is_remote() {
        out.push((module, rust::Item::Trait(service_trait(ast, service)?)));
        out.push((module, rust::Item::Struct(client_struct(ast, service)?)));
        out.push((module, client_impl(ast, service)?));
        out.push((module, rust::Item::Struct(dispatcher_struct(service))));
        // §7.5.1.2.6: the request choice holds the inherited operations too,
        // and the derived service implements the base trait, so the derived
        // dispatcher serves them with the arms the base's dispatcher has.
        let mut inherited = Vec::new();
        for operation in model
            .operations
            .iter()
            .filter(|operation| operation.module != module || operation.interface != service.name)
        {
            let declaring = Service {
                name: operation.interface.clone(),
                module: operation.module,
                ..service.clone()
            };
            let owner = operation.module;
            inherited.push(dispatcher_call_arm_via(
                service,
                &declaring,
                &operation.operation,
                &|name| struct_path(ast, module, owner, name),
            )?);
        }
        for item in dispatcher_impl(ast, service)? {
            out.push((
                module,
                answer_unknown_operations(item, service, inherited.clone()),
            ));
        }
    }
    Ok(())
}

/// The dispatcher's `handle`, as the Enhanced mapping answers it.
///
/// A member the binding does not know is a request for an operation the
/// interface does not have: `REMOTE_EX_UNKNOWN_OPERATION` (DDS-RPC 1.0
/// §7.7.2; the Basic mapping's `REMOTE_EX_UNSUPPORTED` is §7.7.1.1). The
/// request choice of an interface with a base has a variant for each
/// inherited operation too; `inherited` holds an arm for each, which routes
/// it to the base trait the derived service implements.
fn answer_unknown_operations(
    item: rust::Item,
    service: &Service,
    inherited: Vec<rust::MatchArm>,
) -> rust::Item {
    let rust::Item::Impl(mut implementation) = item else {
        return item;
    };
    let unknown = Path::new(&[
        &interface::call_union_name(&service.name),
        UNKNOWN_OP_VARIANT,
    ]);
    let answer = || {
        call(
            local("Err"),
            vec![rust::Expr::Path(remote_exception_code("UnknownOperation"))],
        )
    };
    for member in &mut implementation.items {
        let rust::Item::Function(function) = member else {
            continue;
        };
        if function.name.as_str() != "handle" {
            continue;
        }
        let Some(body) = function.body.as_mut() else {
            continue;
        };
        for statement in &mut body.statements {
            if let rust::Statement::Tail(rust::Expr::Match { arms, .. }) = statement {
                for arm in arms.iter_mut() {
                    if matches!(
                        &arm.pattern,
                        rust::Pattern::TupleStruct { path, .. } if *path == unknown
                    ) {
                        arm.body = answer();
                    }
                }
                // The inherited operations are served, one arm each.
                arms.extend(inherited.iter().cloned());
            }
        }
    }
    rust::Item::Impl(implementation)
}
