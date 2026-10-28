// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bodies of the Java function-call binding under the Enhanced Service
//! Mapping of DDS-RPC 1.0 §7.5.1.2.
//!
//! `enhanced` declares the topic types — `@Choice @Autoid` holders whose
//! members are all `null` but one. This writes what makes them run, as
//! `calls` does for the Basic mapping:
//!
//! - `<I>Codec`: the PL_CDR2 octets of `${I}_Request` and `${I}_Reply` as
//!   `org.omg.dds.rpc.TypeSupport`s — the reply's an
//!   `EnhancedReplySupport`, which the runtime asks for the `remoteEx` code
//!   (§7.5.1.2.5 rule 5) — and of the `In`, `Out` and `Result` holders of
//!   the interface's own operations. The forms are those of
//!   `emit::interface_rust::enhanced`, octet for octet:
//!   - a choice holder (`Request`, `Reply`, `Result`): a `DHEADER`, the
//!     `EMHEADER` of the one member present (member id = HASH &
//!     0x0FFFFFFF, length code 4; 2 for `remoteEx`), the member;
//!   - a `@Mutable` `In` or `Out`: a `DHEADER`, an `EMHEADER` and the value
//!     of each member, the length code of the member's type
//!     ([`crate::emit::interface_rust::emheader_length_code`]); an absent
//!     member takes its default (XTypes 1.3 §7.2.4.4.8) and an unknown one
//!     is skipped by its length;
//!   - any other `In` or `Out` and the exceptions: the members one after
//!     the other.
//! - `<I>ClientImpl`: each operation sets its member of the request and
//!   unpacks the `Result` of that member from the reply, throwing what it
//!   carries. A derived interface answers the operations of its bases itself
//!   (§7.5.1.2.6) and holds no client of a base.
//! - `<I>ServiceImpl`: the member of the request that is present selects the
//!   operation. A request with none this interface has is answered
//!   `REMOTE_EX_UNKNOWN_OPERATION` (§7.7.2).
//!
//! The octets of a member's own type are written by the `ZdCodec` method the
//! layout plan names for it, as for the Basic mapping.

use super::calls::{
    CODEC_CLASS, Member, ResultShape, assign, binary, block, call, chain_of, constructor, declare,
    dot, field_decl, function_of, int, name, nested_class, new_of, null, param, path_expr,
    public_method, qpath, return_field, rpc, signature, static_method, stmt, string, wire,
};
use super::enhanced::holder_type;
use super::{OUTER, local, path};
use crate::adapter::interface::{
    self, BindingError, EnhancedMemberType, EnhancedOperation, EnhancedService, EnhancedStruct,
    Service,
};
use zerodds_ast::{Ast, ModuleNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::java;

/// The remote exception codes of §7.5.2 are `0..=REMOTE_EX_MAX`.
const REMOTE_EX_MAX: i64 = 5;

fn public_static_final() -> Vec<java::Modifier> {
    vec![
        java::Modifier::Public,
        java::Modifier::Static,
        java::Modifier::Final,
    ]
}

/// `org.zerodds.wire.<outer>.<inner>`.
fn wire_nested(outer: &str, inner: &str) -> java::Type {
    java::Type::named(Path::new(&["org", "zerodds", "wire", outer, inner]))
}

fn module_names(ast: &Ast, module: Id<ModuleNode>) -> Result<Vec<String>, BindingError> {
    Ok(interface::module_chain(ast, module)?
        .into_iter()
        .map(|(_, name_)| java::escape_keyword(&name_))
        .collect())
}

/// The class of the codec of the interface `interface` declared in
/// `module`, spelled from anywhere.
fn codec_class_of(
    ast: &Ast,
    module: Id<ModuleNode>,
    interface_name: &str,
) -> Result<java::Expr, BindingError> {
    let mut segments: Vec<String> = vec![OUTER.to_string()];
    segments.extend(module_names(ast, module)?);
    segments.push(format!("{}Codec", java::escape_keyword(interface_name)));
    let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(java::Expr::Path(Path::new(&refs)))
}

/// What a structure's member is.
struct Slot {
    /// The Java field.
    field: String,
    /// Its member id.
    member_id: u32,
    /// The type its codec is named after, `None` for the `dummy` octet.
    type_: Option<Id<TypeNode>>,
}

fn slots_of(structure: &EnhancedStruct) -> Result<Vec<Slot>, BindingError> {
    let mut out = Vec::new();
    for member in &structure.members {
        let type_ = match &member.type_ {
            EnhancedMemberType::Idl(id) => Some(*id),
            EnhancedMemberType::Dummy => None,
            _ => {
                return Err(BindingError::Codec(format!(
                    "member `{}` of `{}` is not a member of an In or Out structure",
                    member.name, structure.name
                )));
            }
        };
        out.push(Slot {
            field: java::escape_keyword(&member.name),
            member_id: member.member_id,
            type_,
        });
    }
    Ok(out)
}

fn as_member(slot: &Slot) -> Member {
    (slot.field.clone(), slot.type_)
}

/// `int zd_end = r.position() + length;` and the `DHEADER` read before it.
fn frame_open() -> Vec<java::Statement> {
    vec![
        declare(
            java::Type::Primitive(java::Primitive::Int),
            "length",
            call(Some(name("r")), "readDheader", Vec::new()),
        ),
        declare(
            java::Type::Primitive(java::Primitive::Int),
            "end",
            binary(
                "+",
                call(Some(name("r")), "position", Vec::new()),
                name("length"),
            ),
        ),
    ]
}

/// `r.readerStatus() == Reader.Status.OK`.
fn reading() -> java::Expr {
    binary(
        "==",
        call(Some(name("r")), "readerStatus", Vec::new()),
        qpath(&["org", "zerodds", "wire", "Reader", "Status", "OK"]),
    )
}

/// The `EMHEADER` of the next member and where that member ends.
fn member_header() -> Vec<java::Statement> {
    vec![
        declare(
            wire_nested("Reader", "EmHeaderRead"),
            "header",
            call(Some(name("r")), "readEmheader", Vec::new()),
        ),
        declare(
            java::Type::Primitive(java::Primitive::Int),
            "after",
            binary(
                "+",
                call(Some(name("r")), "position", Vec::new()),
                java::Expr::Cast {
                    type_: Box::new(java::Type::Primitive(java::Primitive::Int)),
                    value: Box::new(call(Some(name("header")), "extent", Vec::new())),
                },
            ),
        ),
    ]
}

fn skip_to(target: &str) -> java::Statement {
    stmt(call(Some(name("r")), "readerSkipTo", vec![name(target)]))
}

fn member_id_is(id: u32) -> java::Expr {
    binary(
        "==",
        call(Some(name("header")), "memberId", Vec::new()),
        int(i64::from(id)),
    )
}

/// `if (a) {..}` for each arm, one after the other: the conditions exclude
/// one another, and a chain of `else` would nest as deep as there are
/// operations.
fn one_of(arms: Vec<(java::Expr, java::Block)>) -> Vec<java::Statement> {
    arms.into_iter()
        .map(|(condition, then)| java::Statement::If {
            condition,
            then,
            otherwise: None,
        })
        .collect()
}

/// The encoder and the decoder of an `In` or an `Out` structure.
fn fields_codec(
    ast: &Ast,
    module: Id<ModuleNode>,
    structure: &EnhancedStruct,
    type_: java::Type,
) -> Result<Vec<java::Member>, BindingError> {
    let slots = slots_of(structure)?;
    let what = &structure.name;
    if !structure.mutable {
        let members: Vec<Member> = slots.iter().map(as_member).collect();
        return Ok(super::calls::struct_codec(ast, what, type_, &members));
    }

    // PL_CDR2: a DHEADER, then an EMHEADER and the value of each member.
    let mut encode = vec![declare(
        wire_nested("Writer", "DHeader"),
        "dheader",
        call(Some(name("w")), "writeDheader", Vec::new()),
    )];
    for (index, slot) in slots.iter().enumerate() {
        let code = match slot.type_ {
            Some(target) => crate::emit::interface_rust::emheader_length_code(ast, module, target),
            None => 0,
        };
        let header = format!("member{index}");
        encode.push(declare(
            wire_nested("Writer", "EmHeader"),
            &header,
            call(
                Some(name("w")),
                "writeEmheader",
                vec![
                    int(i64::from(slot.member_id)),
                    int(i64::from(code)),
                    java::Expr::Literal(Literal::Bool(false)),
                ],
            ),
        ));
        encode.push(super::calls::encode_member(ast, "value", &as_member(slot)));
        encode.push(stmt(call(
            Some(name("w")),
            "finishEmheader",
            vec![name(&header)],
        )));
    }
    encode.push(stmt(call(
        Some(name("w")),
        "finishDheader",
        vec![name("dheader")],
    )));

    // A member the peer did not send keeps the default of its type
    // (XTypes 1.3 §7.2.4.4.8); one this does not know is skipped by its
    // length.
    let mut decode = vec![declare(
        type_.clone(),
        "value",
        new_of(type_.clone(), Vec::new()),
    )];
    for slot in &slots {
        decode.push(assign(
            dot(name("value"), &slot.field),
            default_of(ast, slot.type_),
        ));
    }
    decode.extend(frame_open());
    let mut arms = Vec::new();
    for slot in &slots {
        arms.push((
            member_id_is(slot.member_id),
            block(vec![super::calls::decode_member(
                ast,
                "value",
                &as_member(slot),
            )]),
        ));
    }
    let mut body = member_header();
    body.extend(one_of(arms));
    body.push(skip_to("after"));
    decode.push(java::Statement::While {
        condition: binary(
            "&&",
            binary(
                "<",
                call(Some(name("r")), "position", Vec::new()),
                name("end"),
            ),
            reading(),
        ),
        body: block(body),
    });
    decode.push(skip_to("end"));
    decode.push(java::Statement::Return(Some(name("value"))));
    Ok(vec![
        static_method(
            java::Type::Void,
            &format!("encode_{what}"),
            vec![param(wire("Writer"), "w"), param(type_.clone(), "value")],
            encode,
        ),
        static_method(
            type_,
            &format!("decode_{what}"),
            vec![param(wire("Reader"), "r")],
            decode,
        ),
    ])
}

/// The default of a member's type: what `ZdCodec` gives a member a peer
/// did not send; the `dummy` octet is zero.
fn default_of(ast: &Ast, type_: Option<Id<TypeNode>>) -> java::Expr {
    match type_ {
        Some(id) => call(
            Some(name(CODEC_CLASS)),
            &crate::default_java::defaulter_name(ast, id),
            Vec::new(),
        ),
        None => new_of(rpc("UnusedMember"), Vec::new()),
    }
}

/// One member of a choice structure.
struct Case {
    /// The Java field, `null` when absent.
    field: String,
    member_id: u32,
    /// How the member's payload is written and read: the class the codec
    /// methods are in (`None` for this one) and the structure they are
    /// named for; `None` for `remoteEx`.
    payload: Option<(Option<java::Expr>, String)>,
}

/// The encoder and the decoder of a choice structure.
fn choice_codec(what: &str, type_: java::Type, cases: &[Case]) -> Vec<java::Member> {
    let mut encode = vec![declare(
        java::Type::Primitive(java::Primitive::Int),
        "present",
        int(0),
    )];
    for case in cases {
        encode.push(java::Statement::If {
            condition: binary("!=", dot(name("value"), &case.field), null()),
            then: block(vec![stmt(binary("+=", name("present"), int(1)))]),
            otherwise: None,
        });
    }
    // "exactly one member shall be present at any given time" (§7.5.1.2.1.1).
    encode.push(java::Statement::If {
        condition: binary("!=", name("present"), int(1)),
        then: block(vec![java::Statement::Throw(new_of(
            java::Type::named(Path::single("IllegalStateException")),
            vec![string(&format!(
                "{what} holds exactly one member (DDS-RPC 1.0 §7.5.1.2.1.1)"
            ))],
        ))]),
        otherwise: None,
    });
    encode.push(declare(
        wire_nested("Writer", "DHeader"),
        "dheader",
        call(Some(name("w")), "writeDheader", Vec::new()),
    ));
    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();
    for case in cases {
        let member = dot(name("value"), &case.field);
        let (code, write) = match &case.payload {
            Some((receiver, structure)) => (
                4,
                stmt(call(
                    receiver.clone(),
                    &format!("encode_{structure}"),
                    vec![name("w"), member.clone()],
                )),
            ),
            None => (
                2,
                stmt(call(
                    Some(name("w")),
                    "writeU32",
                    vec![call(Some(member.clone()), "wireValue", Vec::new())],
                )),
            ),
        };
        encode_arms.push((
            binary("!=", member.clone(), null()),
            block(vec![
                declare(
                    wire_nested("Writer", "EmHeader"),
                    "member",
                    call(
                        Some(name("w")),
                        "writeEmheader",
                        vec![
                            int(i64::from(case.member_id)),
                            int(code),
                            java::Expr::Literal(Literal::Bool(false)),
                        ],
                    ),
                ),
                write,
                stmt(call(
                    Some(name("w")),
                    "finishEmheader",
                    vec![name("member")],
                )),
            ]),
        ));
        let read = match &case.payload {
            Some((receiver, structure)) => vec![assign(
                member.clone(),
                call(
                    receiver.clone(),
                    &format!("decode_{structure}"),
                    vec![name("r")],
                ),
            )],
            None => vec![
                declare(
                    java::Type::Primitive(java::Primitive::Long),
                    "code",
                    call(Some(name("r")), "readU32", Vec::new()),
                ),
                // §7.5.2: a code outside the six is not one this binding
                // understands.
                java::Statement::If {
                    condition: binary(">", name("code"), int(REMOTE_EX_MAX)),
                    then: block(vec![stmt(call(
                        Some(name("r")),
                        "readerRefuse",
                        vec![qpath(&[
                            "org",
                            "zerodds",
                            "wire",
                            "Reader",
                            "Status",
                            "OUT_OF_RANGE",
                        ])],
                    ))]),
                    otherwise: Some(block(vec![assign(
                        member.clone(),
                        call(
                            Some(path_expr(super::RPC, "RemoteExceptionCode")),
                            "fromWire",
                            vec![java::Expr::Cast {
                                type_: Box::new(java::Type::Primitive(java::Primitive::Int)),
                                value: Box::new(name("code")),
                            }],
                        ),
                    )])),
                },
            ],
        };
        decode_arms.push((member_id_is(case.member_id), block(read)));
    }
    encode.extend(one_of(encode_arms));
    encode.push(stmt(call(
        Some(name("w")),
        "finishDheader",
        vec![name("dheader")],
    )));

    // Exactly one member is present; a member this does not know is
    // skipped by its length and leaves every field `null`.
    let mut decode = vec![declare(
        type_.clone(),
        "value",
        new_of(type_.clone(), Vec::new()),
    )];
    decode.extend(frame_open());
    let mut guarded = member_header();
    guarded.extend(one_of(decode_arms));
    guarded.push(skip_to("after"));
    // "exactly one member shall be present at any given time"
    // (§7.5.1.2.1.1): a second member is a malformed sample, not one to
    // skip over.
    guarded.push(java::Statement::If {
        condition: binary(
            "&&",
            binary(
                "<",
                call(Some(name("r")), "position", Vec::new()),
                name("end"),
            ),
            reading(),
        ),
        then: block(vec![stmt(call(
            Some(name("r")),
            "readerRefuse",
            vec![qpath(&[
                "org",
                "zerodds",
                "wire",
                "Reader",
                "Status",
                "OUT_OF_RANGE",
            ])],
        ))]),
        otherwise: None,
    });
    decode.push(java::Statement::If {
        condition: binary(
            "&&",
            binary(
                "<",
                call(Some(name("r")), "position", Vec::new()),
                name("end"),
            ),
            reading(),
        ),
        then: block(guarded),
        otherwise: None,
    });
    decode.push(skip_to("end"));
    decode.push(java::Statement::Return(Some(name("value"))));
    vec![
        static_method(
            java::Type::Void,
            &format!("encode_{what}"),
            vec![param(wire("Writer"), "w"), param(type_.clone(), "value")],
            encode,
        ),
        static_method(
            type_,
            &format!("decode_{what}"),
            vec![param(wire("Reader"), "r")],
            decode,
        ),
    ]
}

/// The codec methods of one exception: its members one after the other.
fn exception_codec(
    ast: &Ast,
    raised: &interface::Raise,
) -> Result<Vec<java::Member>, BindingError> {
    let (module, fields) = super::calls::exception_members(ast, raised)?;
    Ok(super::calls::data_struct_codec(
        ast,
        &format!("ex_{}", raised.name),
        super::calls::data_type(ast, module, &raised.name)?,
        &fields,
    ))
}

/// The `TypeSupport` of `${I}_Request` or `${I}_Reply`.
fn support_class(
    class_name: &str,
    sample: java::Type,
    what: &str,
    reply: Option<&str>,
) -> java::Member {
    let bytes = || java::Type::Array(Box::new(java::Type::Primitive(java::Primitive::Byte)));
    let int_type = || java::Type::Primitive(java::Primitive::Int);
    let encoding = || rpc("WireEncoding");
    // The alignment of the encoding is four: the Enhanced mapping is XCDR2
    // whatever the params say.
    let max_align = || int(4);
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
    let lead_loop = java::Statement::For {
        init: Box::new(declare(int_type(), "pad", int(0))),
        condition: binary("<", name("pad"), name("lead")),
        step: binary("=", name("pad"), binary("+", name("pad"), int(1))),
        body: block(vec![stmt(call(Some(name("w")), "writeU8", vec![int(0)]))]),
    };
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
                qpath(&["org", "omg", "dds", "rpc", "WireEncoding", "XCDR2"]),
            ],
        )))],
    );
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
            declare(int_type(), "lead", binary("%", name("offset"), int(4))),
            declare(
                wire("Writer"),
                "w",
                new_of(
                    wire("Writer"),
                    vec![
                        qpath(&["org", "zerodds", "wire", "Endianness", "LITTLE"]),
                        max_align(),
                        null(),
                        java::Expr::Literal(Literal::Bool(false)),
                    ],
                ),
            ),
            lead_loop,
            stmt(call(
                None,
                &format!("encode_{what}"),
                vec![name("w"), name("value")],
            )),
            declare(bytes(), "all", call(Some(name("w")), "finish", Vec::new())),
            java::Statement::Return(Some(call(
                Some(path_expr("java.util", "Arrays")),
                "copyOfRange",
                vec![name("all"), name("lead"), dot(name("all"), "length")],
            ))),
        ],
    );
    let deserialize_default = public_method(
        Doc::none(),
        true,
        sample.clone(),
        "deserialize",
        vec![param(bytes(), "octets"), param(cursor(), "cursor")],
        vec![java::Statement::Return(Some(call(
            None,
            "deserialize",
            vec![
                name("octets"),
                name("cursor"),
                qpath(&["org", "omg", "dds", "rpc", "WireEncoding", "XCDR2"]),
            ],
        )))],
    );
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
                binary("%", dot(name("cursor"), "at"), int(4)),
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
                        max_align(),
                        null(),
                    ],
                ),
            ),
            stmt(call(Some(name("r")), "skip", vec![name("lead")])),
            declare(
                sample.clone(),
                "value",
                call(None, &format!("decode_{what}"), vec![name("r")]),
            ),
            // A refusal of the reader (a second member of a choice, an
            // `remoteEx` outside the six codes, a length that overruns its
            // frame) is not a sample: raising it keeps a malformed sample
            // from being read as an empty one.
            java::Statement::If {
                condition: binary(
                    "!=",
                    call(Some(name("r")), "readerStatus", Vec::new()),
                    qpath(&["org", "zerodds", "wire", "Reader", "Status", "OK"]),
                ),
                then: block(vec![java::Statement::Throw(new_of(
                    rpc("RemoteInvalidArgumentError"),
                    vec![binary(
                        "+",
                        string(&format!("{what} is malformed (DDS-RPC 1.0 §7.5.1.2.1.1): ")),
                        call(Some(name("r")), "readerStatus", Vec::new()),
                    )],
                ))]),
                otherwise: None,
            },
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
    let mut members = vec![
        serialize_default,
        serialize,
        deserialize_default,
        deserialize,
    ];
    let mut implements = vec![java::Type::generic(
        super::path(super::RPC, "TypeSupport"),
        vec![sample.clone()],
    )];
    if let Some(remote_ex) = reply {
        // §7.5.1.2.5 rule 5: the code is the `remoteEx` member of the reply.
        implements = vec![java::Type::generic(
            super::path(super::RPC, "EnhancedReplySupport"),
            vec![sample.clone()],
        )];
        members.push(public_method(
            Doc::none(),
            true,
            rpc("RemoteExceptionCode"),
            "remoteExceptionOf",
            vec![param(sample.clone(), "reply")],
            vec![java::Statement::Return(Some(java::Expr::Ternary {
                condition: Box::new(binary("==", dot(name("reply"), remote_ex), null())),
                then: Box::new(qpath(&[
                    "org",
                    "omg",
                    "dds",
                    "rpc",
                    "RemoteExceptionCode",
                    "REMOTE_EX_OK",
                ])),
                otherwise: Box::new(dot(name("reply"), remote_ex)),
            }))],
        ));
        members.push(public_method(
            Doc::none(),
            true,
            sample.clone(),
            "remoteExceptionSample",
            vec![param(rpc("RemoteExceptionCode"), "code")],
            vec![
                declare(sample.clone(), "reply", new_of(sample.clone(), Vec::new())),
                assign(dot(name("reply"), remote_ex), name("code")),
                java::Statement::Return(Some(name("reply"))),
            ],
        ));
    }
    nested_class(
        class_name,
        vec![
            java::Modifier::Private,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        None,
        implements,
        members,
        Doc::none(),
    )
}

/// The operations of the interface itself.
fn own<'a>(model: &'a EnhancedService, service: &Service) -> Vec<&'a EnhancedOperation> {
    model
        .operations
        .iter()
        .filter(|operation| {
            operation.module == service.module && operation.interface == service.name
        })
        .collect()
}

/// `<I>Codec` — the Enhanced mapping's octets for one interface.
fn codec_class(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    model: &EnhancedService,
) -> Result<java::Member, BindingError> {
    let module = service.module;
    let request = local(&interface::request_type_name(name_escaped));
    let reply = local(&interface::reply_type_name(name_escaped));
    let remote_ex = model
        .reply
        .members
        .iter()
        .find(|member| member.type_ == EnhancedMemberType::RemoteException)
        .map(|member| java::escape_keyword(&member.name))
        .ok_or_else(|| {
            BindingError::Codec("the reply structure has no remoteEx member".to_string())
        })?;

    let mut members = vec![
        field_decl(
            public_static_final(),
            java::Type::generic(path(super::RPC, "TypeSupport"), vec![request.clone()]),
            "REQUEST",
            Some(new_of(local("RequestSupport"), Vec::new())),
        ),
        field_decl(
            public_static_final(),
            java::Type::generic(path(super::RPC, "TypeSupport"), vec![reply.clone()]),
            "REPLY",
            Some(new_of(local("ReplySupport"), Vec::new())),
        ),
        support_class("RequestSupport", request.clone(), &model.request.name, None),
        support_class(
            "ReplySupport",
            reply.clone(),
            &model.reply.name,
            Some(&remote_ex),
        ),
    ];

    // The exceptions the interface's own operations raise, once each.
    let mut declared: Vec<String> = Vec::new();
    for operation in own(model, service) {
        for raised in &operation.operation.raises {
            if declared.contains(&raised.name) {
                continue;
            }
            declared.push(raised.name.clone());
            members.extend(exception_codec(ast, raised)?);
        }
    }
    for operation in own(model, service) {
        for structure in [&operation.in_struct, &operation.out_struct] {
            members.extend(fields_codec(
                ast,
                module,
                structure,
                local(&structure.name),
            )?);
        }
        let mut cases = Vec::new();
        for member in &operation.result_struct.members {
            let structure = match &member.type_ {
                EnhancedMemberType::Struct(structure) => structure.clone(),
                EnhancedMemberType::Exception(raised) => format!("ex_{}", raised.name),
                _ => continue,
            };
            cases.push(Case {
                field: java::escape_keyword(&member.name),
                member_id: member.member_id,
                payload: Some((None, structure)),
            });
        }
        members.extend(choice_codec(
            &operation.result_struct.name,
            local(&operation.result_struct.name),
            &cases,
        ));
    }
    for (structure, type_) in [(&model.request, request), (&model.reply, reply)] {
        let mut cases = Vec::new();
        for member in &structure.members {
            match &member.type_ {
                EnhancedMemberType::Struct(name_) => {
                    let owner = model
                        .operations
                        .iter()
                        .find(|operation| operation.operation.name == member.name)
                        .map_or((module, service.name.as_str()), |operation| {
                            (operation.module, operation.interface.as_str())
                        });
                    // The structures of an inherited operation are written
                    // by the codec of the interface that declares it.
                    let receiver = if owner.0 == module && owner.1 == service.name {
                        None
                    } else {
                        Some(codec_class_of(ast, owner.0, owner.1)?)
                    };
                    cases.push(Case {
                        field: java::escape_keyword(&member.name),
                        member_id: member.member_id,
                        payload: Some((receiver, name_.clone())),
                    });
                }
                EnhancedMemberType::RemoteException => cases.push(Case {
                    field: java::escape_keyword(&member.name),
                    member_id: member.member_id,
                    payload: None,
                }),
                _ => {}
            }
        }
        members.extend(choice_codec(&structure.name, type_, &cases));
    }
    Ok(nested_class(
        &format!("{name_escaped}Codec"),
        public_static_final(),
        None,
        Vec::new(),
        members,
        Doc::lines(&[
            "The octets of the Enhanced Service Mapping for this interface.",
            "",
            "DDS-RPC 1.0 §7.5.1.2: the bare `${I}_Request` and `${I}_Reply`, PL_CDR2,",
            "little-endian. Members are written by `ZdCodec`.",
        ]),
    ))
}

/// The interface that declares `operation`, among `service` and its
/// ancestors.
fn owner_of(
    ast: &Ast,
    service: &Service,
    operation: &EnhancedOperation,
) -> Result<Service, BindingError> {
    let mut candidates = vec![service.clone()];
    candidates.extend(super::calls::ancestors(ast, service)?);
    candidates
        .into_iter()
        .find(|candidate| {
            candidate.module == operation.module && candidate.name == operation.interface
        })
        .ok_or_else(|| {
            BindingError::Codec(format!(
                "the interface `{}` that declares `{}` is not a base of `{}`",
                operation.interface, operation.operation.name, service.name
            ))
        })
}

/// A structure of the mapping, spelled from the module of `here`.
fn structure_type(
    ast: &Ast,
    here: Id<ModuleNode>,
    operation: &EnhancedOperation,
    structure: &EnhancedStruct,
) -> Result<java::Type, BindingError> {
    holder_type(ast, here, operation.module, &structure.name)
}

/// `<op>_request`, `<op>_Unpack`, the synchronous and the asynchronous
/// method of one operation.
fn client_operation(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    operation: &EnhancedOperation,
) -> Result<Vec<java::Member>, BindingError> {
    let op = &operation.operation;
    let owner = owner_of(ast, service, operation)?;
    let owner_chain = chain_of(ast, &owner)?;
    let spelled = java::escape_keyword(&op.name);
    let sig = signature(ast, service, &owner, &owner_chain, op)?;
    let request_type = local(&interface::request_type_name(name_escaped));
    let reply_type = local(&interface::reply_type_name(name_escaped));
    let builder = format!("{}_request", op.name);
    let unpack = format!("{}_Unpack", op.name);
    let in_type = structure_type(ast, service.module, operation, &operation.in_struct)?;
    let case = || dot(name("request"), &spelled);

    // The request: the holder with the operation's member filled.
    let mut build = vec![
        declare(
            request_type.clone(),
            "request",
            new_of(request_type.clone(), Vec::new()),
        ),
        assign(case(), new_of(in_type, Vec::new())),
    ];
    for slot in slots_of(&operation.in_struct)? {
        let value = match slot.type_ {
            Some(_) => name(&slot.field),
            None => new_of(rpc("UnusedMember"), Vec::new()),
        };
        build.push(assign(dot(case(), &slot.field), value));
    }
    build.push(java::Statement::Return(Some(name("request"))));
    let mut members = vec![super::calls::private_static_method(
        request_type,
        &builder,
        sig.params.clone(),
        build,
    )];

    let request_call = call(None, &builder, super::calls::arguments(op));

    // §7.5.1.2.3: the `Result` holds the result or one exception.
    let result_type = structure_type(ast, service.module, operation, &operation.result_struct)?;
    let mut raise = Vec::new();
    for member in &operation.result_struct.members {
        if let EnhancedMemberType::Exception(_) = &member.type_ {
            let field = java::escape_keyword(&member.name);
            raise.push((
                binary("!=", dot(name("result"), &field), null()),
                block(vec![java::Statement::Throw(new_of(
                    rpc("UserExceptionRaised"),
                    vec![dot(name("result"), &field)],
                ))]),
            ));
        }
    }
    let mut failure: Vec<java::Statement> = raise
        .into_iter()
        .map(|(condition, then)| java::Statement::If {
            condition,
            then,
            otherwise: None,
        })
        .collect();
    failure.push(java::Statement::Throw(new_of(
        rpc("RemoteUnknownExceptionError"),
        vec![string(&format!(
            "{}.{}: the service raised an exception this client does not know",
            service.name, op.name
        ))],
    )));
    let returned = match sig.result {
        ResultShape::Outputs => dot(name("result"), interface::RESULT_MEMBER),
        ResultShape::Value => dot(
            dot(name("result"), interface::RESULT_MEMBER),
            &return_field(op),
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
            declare(result_type, "result", dot(name("reply"), &spelled)),
            java::Statement::If {
                condition: binary("==", name("result"), null()),
                then: block(vec![java::Statement::Throw(new_of(
                    rpc("RemoteUnknownOperationError"),
                    vec![string(&format!(
                        "{}.{}: the reply does not answer the operation of the request",
                        service.name, op.name
                    ))],
                ))]),
                otherwise: None,
            },
            java::Statement::If {
                condition: binary("==", dot(name("result"), interface::RESULT_MEMBER), null()),
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
        java::Type::generic(path(super::RPC, "Future"), vec![sig.value_type]),
        &format!("{}_async", sig.method),
        sig.params,
        vec![java::Statement::Return(Some(call(
            Some(path_expr(super::RPC, "Futures")),
            "map",
            vec![
                call(None, "callAsync", vec![request_call]),
                new_of(local(&unpack), Vec::new()),
            ],
        )))],
    ));
    Ok(members)
}

/// `<I>ClientImpl`.
fn client_class(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    model: &EnhancedService,
) -> Result<java::Member, BindingError> {
    let request_type = local(&interface::request_type_name(name_escaped));
    let reply_type = local(&interface::reply_type_name(name_escaped));
    let class_name = format!("{name_escaped}ClientImpl");
    // DDS-RPC 1.0 §7.11.2: the params of the machine-readable Java files take no type
    // arguments.
    let params_type = java::Type::named(path(super::RPC, "RequesterParams"));
    let participant_type = rpc("DomainParticipant");
    let support = format!("{name_escaped}Support");

    let mut members = vec![
        constructor(
            &class_name,
            vec![param(params_type, "params")],
            vec![stmt(call(None, "super", vec![name("params")]))],
            Doc::line("A client over the requester the params describe."),
        ),
        constructor(
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
        ),
        constructor(
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
        ),
    ];
    // §7.5.1.2.6: the operations of the bases are in this request and this
    // reply, so this client answers them itself.
    for operation in &model.operations {
        members.extend(client_operation(ast, service, name_escaped, operation)?);
    }
    Ok(nested_class(
        &class_name,
        public_static_final(),
        Some(java::Type::generic(
            path(super::RPC, "FunctionCallClient"),
            vec![request_type, reply_type],
        )),
        vec![local(&format!("{name_escaped}Client"))],
        members,
        Doc::lines(&[
            "The function-call client over a `Requester`.",
            "",
            "DDS-RPC 1.0 §7.11.1.5.4, §7.11.2.3, §7.5.1.2: it instantiates the DataWriter and the",
            "DataReader and calls the service through them, correlating by sample identity.",
        ]),
    ))
}

/// `<op>_invoke` of the dispatcher.
fn invoke_method(
    ast: &Ast,
    service: &Service,
    operation: &EnhancedOperation,
) -> Result<java::Member, BindingError> {
    let op = &operation.operation;
    let owner = owner_of(ast, service, operation)?;
    let owner_chain = chain_of(ast, &owner)?;
    let sig = signature(ast, service, &owner, &owner_chain, op)?;
    let in_type = structure_type(ast, service.module, operation, &operation.in_struct)?;
    let implementation_call = || {
        call(
            Some(java::Expr::Path(Path::single("implementation"))),
            &sig.method,
            op.inputs()
                .map(|input| dot(name("in"), &java::escape_keyword(&input.name)))
                .collect(),
        )
    };
    let lambda = || java::Expr::Lambda {
        params: Vec::new(),
        body: Box::new(implementation_call()),
    };
    let outcome_type =
        |value: java::Type| java::Type::generic(path(super::RPC, "Outcome"), vec![value]);
    let invoke_name = format!("{}_invoke", op.name);
    let result_type = structure_type(ast, service.module, operation, &operation.result_struct)?;
    let out_type = structure_type(ast, service.module, operation, &operation.out_struct)?;
    let mut body = vec![declare(
        result_type.clone(),
        "result",
        new_of(result_type.clone(), Vec::new()),
    )];
    let call_expr = if sig.result == ResultShape::Nothing {
        call(
            Some(path_expr(super::RPC, "Outcome")),
            "ofVoid",
            vec![lambda()],
        )
    } else {
        call(Some(path_expr(super::RPC, "Outcome")), "of", vec![lambda()])
    };
    body.push(declare(
        outcome_type(sig.value_type.clone()),
        "outcome",
        call_expr,
    ));
    // A declared exception fills its member of the `Result` structure.
    let mut raised_arms = Vec::new();
    for member in &operation.result_struct.members {
        let EnhancedMemberType::Exception(raised) = &member.type_ else {
            continue;
        };
        let (module, _) = super::calls::exception_members(ast, raised)?;
        let holder = super::calls::data_type(ast, module, &raised.name)?;
        raised_arms.push((
            java::Expr::InstanceOf {
                value: Box::new(name("raised")),
                type_: Box::new(holder.clone()),
            },
            block(vec![
                assign(
                    dot(name("result"), &java::escape_keyword(&member.name)),
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
    when_raised.extend(one_of(raised_arms));
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
    let has_dummy = operation
        .out_struct
        .members
        .iter()
        .any(|member| member.type_ == EnhancedMemberType::Dummy);
    match sig.result {
        ResultShape::Outputs => body.push(assign(
            dot(name("result"), interface::RESULT_MEMBER),
            call(Some(name("outcome")), "value", Vec::new()),
        )),
        ResultShape::Value => {
            body.push(assign(
                dot(name("result"), interface::RESULT_MEMBER),
                new_of(out_type, Vec::new()),
            ));
            body.push(assign(
                dot(
                    dot(name("result"), interface::RESULT_MEMBER),
                    &return_field(op),
                ),
                call(Some(name("outcome")), "value", Vec::new()),
            ));
        }
        ResultShape::Nothing => {
            body.push(assign(
                dot(name("result"), interface::RESULT_MEMBER),
                new_of(out_type, Vec::new()),
            ));
            if has_dummy {
                body.push(assign(
                    dot(
                        dot(name("result"), interface::RESULT_MEMBER),
                        interface::DUMMY_MEMBER,
                    ),
                    new_of(rpc("UnusedMember"), Vec::new()),
                ));
            }
        }
    }
    body.push(java::Statement::Return(Some(name("result"))));
    Ok(java::Member::Method(java::Method {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Private],
        generics: Vec::new(),
        returns: result_type,
        name: Ident::new(&invoke_name),
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
    model: &EnhancedService,
) -> Result<java::Member, BindingError> {
    let request_type = local(&interface::request_type_name(name_escaped));
    let reply_type = local(&interface::reply_type_name(name_escaped));
    let class_name = format!("{name_escaped}ServiceImpl");
    let interface_type = local(name_escaped);
    // DDS-RPC 1.0 §7.11.2: the params of the machine-readable Java files take no type
    // arguments.
    let params_type = java::Type::named(path(super::RPC, "ReplierParams"));
    let string_type = java::Type::named(Path::single("String"));
    let handler_type = java::Type::generic(
        path(super::RPC, "RequestHandler"),
        vec![request_type.clone(), reply_type.clone()],
    );

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

    // handle(): the member of the request that is present selects the
    // operation (§7.5.1.2.4).
    let mut arms = Vec::new();
    for operation in &model.operations {
        let op = &operation.operation;
        let spelled = java::escape_keyword(&op.name);
        let invoke = format!("{}_invoke", op.name);
        let arguments = vec![dot(name("request"), &spelled)];
        let body = vec![
            assign(dot(name("reply"), &spelled), call(None, &invoke, arguments)),
            java::Statement::Return(Some(call(
                Some(path_expr(super::RPC, "Response")),
                "ok",
                vec![name("reply")],
            ))),
        ];
        arms.push((
            binary("!=", dot(name("request"), &spelled), null()),
            block(body),
        ));
    }
    let mut handle = vec![declare(
        reply_type.clone(),
        "reply",
        new_of(reply_type.clone(), Vec::new()),
    )];
    handle.extend(one_of(arms));
    // §7.7.2: an operation this service does not know is
    // REMOTE_EX_UNKNOWN_OPERATION.
    handle.push(java::Statement::Return(Some(call(
        Some(path_expr(super::RPC, "Response")),
        "failed",
        vec![
            name("reply"),
            qpath(&[
                "org",
                "omg",
                "dds",
                "rpc",
                "RemoteExceptionCode",
                "REMOTE_EX_UNKNOWN_OPERATION",
            ]),
        ],
    ))));
    dispatcher_members.push(public_method(
        Doc::none(),
        true,
        java::Type::generic(path(super::RPC, "Response"), vec![reply_type.clone()]),
        "handle",
        vec![param(request_type.clone(), "request")],
        handle,
    ));
    // The reply for a request the implementation failed: the runtime puts
    // the code in `remoteEx` (§7.5.1.2.5 rule 5), so it carries nothing.
    dispatcher_members.push(public_method(
        Doc::none(),
        true,
        reply_type.clone(),
        "failureReply",
        vec![param(request_type.clone(), "request")],
        vec![java::Statement::Return(Some(new_of(
            reply_type.clone(),
            Vec::new(),
        )))],
    ));
    for operation in &model.operations {
        dispatcher_members.push(invoke_method(ast, service, operation)?);
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

    let support = format!("{name_escaped}Support");
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
            vec![
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
            ],
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
        public_static_final(),
        Some(java::Type::generic(
            path(super::RPC, "FunctionCallService"),
            vec![request_type, reply_type],
        )),
        vec![local(&format!("{name_escaped}Service"))],
        members,
        Doc::lines(&[
            "The function-call service over a `Replier`.",
            "",
            "DDS-RPC 1.0 §7.9.1, §7.11.1.5.1, §7.11.2.3, §7.5.1.2: creating it activates the",
            "service, the requests it receives call the implementation, `close()` deactivates it",
            "(§7.9.3).",
        ]),
    ))
}

/// The bodies of one function-call service under the Enhanced mapping: its
/// codec, its client and its service.
pub(super) fn members(
    ast: &Ast,
    service: &Service,
    name_escaped: &str,
    model: &EnhancedService,
) -> Result<Vec<java::Member>, BindingError> {
    Ok(vec![
        codec_class(ast, service, name_escaped, model)?,
        client_class(ast, service, name_escaped, model)?,
        service_class(ast, service, name_escaped, model)?,
    ])
}
