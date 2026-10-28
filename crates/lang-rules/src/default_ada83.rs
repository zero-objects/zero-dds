// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value an Ada 83 member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Ada 83 — `default_ada` is the closer
//! relative than any other module in this crate, and every shape below
//! is its shape: a defaulter **returns** Table 9's value, an
//! aggregate's body is one record aggregate because a union's
//! discriminant is only ever set through a whole value (RM83 3.7.1),
//! and a sequence defaults to an allocated empty array and never
//! `null`.
//!
//! What differs follows from `adapter::ada83`'s materialisation: a
//! `string`/`fixed`/`long double` default is `Zd_Wire83.Text_Of ("…")`,
//! not `Zd_Wire.To_Text`, and a discriminator on an enumeration
//! defaults to the enumerator's **value**, never a literal name — this
//! column declares no enumeration type for a name to resolve against.

use crate::adapter::ada83::{DISCRIMINANT, PRESENT, ada83_type_name, component_name, type_tag};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::ada83;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The object a defaulter for a componentless record declares and
/// returns.
///
/// Prefixed like every other generated name in this column, so that no
/// IDL identifier reaches it: `component_name` never produces a name
/// beginning `zd83_`.
pub const EMPTY_VALUE: &str = "zd83_empty";

/// The body of a defaulter for a record with no components at all.
///
/// RM83 4.3.1 requires a record aggregate to carry at least one
/// `component_association`, and `(null record)` — the spelling later
/// Ada gives the empty case — is an Ada 2005 addition (AI95-287) this
/// column does not use. None is needed: RM83 3.2.1 lets an object
/// declaration stand without an initial value, and a record with no
/// components has no component left uninitialised by one, so the
/// object *is* the whole value of the type and the function returns
/// it.
///
/// The declaration is a leading [`ada83::Stmt::Var`], which
/// [`ada83::SubprogramBody`] renders into the declarative part —
/// RM83 3.9, and the shape `writing_declares_no_object_below_a_body`
/// checks for the outer one.
fn empty_record_body(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Vec<ada83::Stmt>, EncodeError> {
    let mark =
        crate::adapter::ada83::ada83_value_path(ast, id).map_err(|_| EncodeError::NoWriter {
            member: own_name.to_string(),
            kind: "a record with no components and no Ada 83 subtype mark to declare one by",
        })?;
    Ok(vec![
        ada83::Stmt::Var(ada83::VarDecl {
            name: Ident::new(EMPTY_VALUE),
            mark,
            constraint: None,
            value: None,
        }),
        ada83::Stmt::Return(Some(ada83::Expr::name(EMPTY_VALUE))),
    ])
}

/// The defaulter function name for a type.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd83_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd83_def_{}", type_tag(ast, id)),
    }
}

/// The defaulter that goes with a decoder, by name.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd83_def_{rest}"),
        None => format!("zd83_def_{decoder}"),
    }
}

/// One member's default — its `@default`, or its type's — assigned
/// into `place`.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: ada83::Expr,
) -> Result<Vec<ada83::Stmt>, EncodeError> {
    let mut value = member_value(ast, node, type_)?;
    if node.facets.is_optional() {
        value = engaged(value);
    }
    Ok(vec![ada83::Stmt::Assign { place, value }])
}

/// `(Present => True, Value => value)`.
fn engaged(value: ada83::Expr) -> ada83::Expr {
    ada83::Expr::Aggregate(vec![
        ada83::Association {
            choice: Ident::new(PRESENT),
            value: ada83::Expr::Literal(Literal::Bool(true)),
        },
        ada83::Association {
            choice: Ident::new(crate::adapter::ada83::VALUE),
            value,
        },
    ])
}

/// The defaulters `decls` call, and exactly those.
///
/// # Errors
/// [`EncodeError`] for a construct with no Ada 83 default.
pub fn ada83_defaulters_called_by(
    ast: &Ast,
    decls: &[ada83::Decl],
) -> Result<Vec<ada83::Decl>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_decl(decl, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    for _ in 0..=index.len() {
        let mut grew = false;
        for name in &called {
            if have.contains(name) {
                continue;
            }
            have.push(name.clone());
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = ada83_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    ada83_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Ada 83 default.
pub fn ada83_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<ada83::Decl>, EncodeError> {
    let mut specs = Vec::new();
    let mut bodies = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let Some(body) = body_of(ast, id, &name)? else {
            continue;
        };
        let returns = match crate::adapter::ada83::ada83_value_path(ast, resolve_alias(ast, id)) {
            Ok(returns) => returns,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no Ada 83 form to return",
                });
            }
        };
        emitted.push(name.clone());
        let spec = ada83::Subprogram {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            kind: ada83::SubprogramKind::Function,
            name: Ident::new(&name),
            params: Vec::new(),
            result: Some(returns),
        };
        specs.push(ada83::Decl::Subprogram(spec.clone()));
        bodies.push(ada83::Decl::SubprogramBody(ada83::SubprogramBody {
            spec: ada83::Subprogram {
                doc: Doc::none(),
                ..spec
            },
            body,
        }));
    }
    specs.append(&mut bodies);
    Ok(specs)
}

/// The body one defaulter answers with, or `None` where the type has
/// no Ada 83 data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<ada83::Stmt>>, EncodeError> {
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // A type the model does not hold. It used to answer `Ok(None)`,
    // which the caller reads as "nothing to emit for this one" and
    // skips — so a type whose handle no longer resolves quietly loses
    // its defaulter, and the first sign of it is a member left at
    // whatever the language zero-initialises to. Every other arm of
    // this function either answers a value or refuses by name; this
    // one now does too.
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let give = |value: ada83::Expr| Some(vec![ada83::Stmt::Return(Some(value))]);
    let body = match &node.kind {
        TypeKind::Primitive(kind) => give(primitive_default(*kind)),

        TypeKind::String { .. } => give(text("")),

        TypeKind::Fixed { scale, .. } => give(text(&zero_decimal(*scale))),

        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // The enumerator's Ada 83 spelling — a named `Long_Integer`
        // constant, not a literal — is exactly `component_name` of its
        // IDL name, so this is unchanged from `default_ada`'s reading
        // even though the declaration behind the name differs.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            give(ada83::Expr::name(&component_name(&enumerator)))
        }

        TypeKind::Bitmask { .. } => give(ada83::Expr::int(0)),

        TypeKind::Bitset { .. } => {
            let mut parts = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    return Err(refuse("a bitfield the model does not hold"));
                };
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                parts.push(ada83::Association {
                    choice: Ident::new(&component_name(field_name)),
                    value: ada83::Expr::int(0),
                });
            }
            // A `bitset` with no named field is a record with no
            // components, and RM83 4.3.1 admits no aggregate for one.
            if parts.is_empty() {
                return Ok(Some(empty_record_body(ast, id, own_name)?));
            }
            give(ada83::Expr::Aggregate(parts))
        }

        TypeKind::Sequence { .. } | TypeKind::Map { .. } => give(ada83::Expr::New {
            mark: Path::single(&format!("{}_Array", ada83_type_name(ast, id))),
            init: ada83::AllocInit::Range {
                low: Box::new(ada83::Expr::int(0)),
                high: Box::new(ada83::Expr::int(-1)),
            },
        }),

        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut out = call(&defaulter_name(ast, element));
            for _ in dims {
                out = ada83::Expr::Aggregate(vec![ada83::Association {
                    choice: Ident::new("others"),
                    value: out,
                }]);
            }
            give(out)
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut parts = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                parts.push(ada83::Association {
                    choice: Ident::new(&component_name("parent")),
                    value: call(&defaulter_name(ast, base)),
                });
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    return Err(refuse("a member the model does not hold"));
                };
                let choice = Ident::new(&component_name(&member_node.name));
                // A member the adapter holds by reference is absent as
                // `null`; `(Present => False)` is the record form the
                // access type is not
                // (`crate::adapter::ada83::held_by_reference`).
                let external = member_node
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|xtypes| xtypes.is_external);
                if member_node.type_.resolved().is_some_and(|member_type| {
                    crate::adapter::ada83::held_by_reference(
                        ast,
                        id,
                        member_type,
                        member_node.facets.is_optional(),
                        external,
                    )
                }) {
                    parts.push(ada83::Association {
                        choice,
                        value: ada83::Expr::Literal(Literal::Null),
                    });
                    continue;
                }
                if member_node.facets.is_optional() {
                    parts.push(ada83::Association {
                        choice,
                        value: ada83::Expr::Aggregate(vec![ada83::Association {
                            choice: Ident::new(PRESENT),
                            value: ada83::Expr::Literal(Literal::Bool(false)),
                        }]),
                    });
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                parts.push(ada83::Association {
                    choice,
                    value: member_value(ast, member_node, member_type)?,
                });
            }
            // A record with no components and no discriminant has no
            // aggregate at all here — RM83 4.3.1 requires at least one
            // association, and this column names no `<>` box. Every
            // *other* zero-component shape (a union's `others` arm, a
            // struct whose only members are `@optional`) still carries
            // the discriminant or the `(Present => False)` association
            // and is unaffected; only the fully empty struct is. It
            // needs no aggregate: an object of it is declared and
            // returned instead.
            if parts.is_empty() {
                return Ok(Some(empty_record_body(ast, id, own_name)?));
            }
            give(ada83::Expr::Aggregate(parts))
        }

        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut parts = vec![ada83::Association {
                choice: Ident::new(DISCRIMINANT),
                value: discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            }];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                parts.push(ada83::Association {
                    choice: Ident::new(&field),
                    value: call(&defaulter_name(ast, member_type)),
                });
            }
            give(ada83::Expr::Aggregate(parts))
        }

        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => None,
        TypeKind::ValueBox { .. } => {
            return Err(refuse(
                "boxed value, whose presence flag has no settled wire form",
            ));
        }
        TypeKind::Any => {
            return Err(refuse(
                "`any`, which DDS-XTypes has no encoding for; only CORBA CDR writes one, \
                 as a TypeCode ahead of the value",
            ));
        }
        TypeKind::ValueBase => {
            return Err(refuse(
                "`ValueBase`, which DDS-XTypes has no encoding for; only CORBA CDR writes \
                 one, as a chunked value with its repository id",
            ));
        }
    };
    Ok(body)
}

fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<ada83::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(call(&defaulter_name(ast, type_))),
    }
}

/// Table 9's primitive rows, as the Ada 83 holder each takes.
fn primitive_default(kind: PrimitiveKind) -> ada83::Expr {
    match kind {
        PrimitiveKind::Boolean => ada83::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            ada83::Expr::Literal(Literal::Float("0.0".to_string()))
        }
        PrimitiveKind::Float128 => text("0.0"),
        PrimitiveKind::Char
        | PrimitiveKind::WChar
        | PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => ada83::Expr::int(0),
    }
}

/// The case a defaulted discriminator selects, if any.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: Option<i128>,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, component_name(&member.name))))
}

/// A discriminator value in the Ada 83 form its type gives it:
/// `True`/`False` for a `boolean`, and the bare number for everything
/// else — an enumeration discriminator included, since this column
/// names no literal for it to resolve against.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> ada83::Expr {
    let resolved = resolve_alias(ast, discriminator);
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => ada83::Expr::Literal(Literal::Bool(value != 0)),
        _ => ada83::Expr::int(value),
    }
}

/// The primitive `id` is once typedefs are seen through, or `None`.
fn resolved_primitive(ast: &Ast, id: Id<TypeNode>) -> Option<PrimitiveKind> {
    let mut current = id;
    for _ in 0..MAX_ALIAS_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Primitive(kind)) => return Some(kind),
            Ok(TypeKind::Alias { aliased }) => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// An explicit `@default` as an Ada 83 expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<ada83::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let Ok(node) = ast.expr(expr) else {
        return Err(refuse("a `@default` this frontend cannot follow"));
    };
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    let is_float128 = matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128)));
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(ada83::Expr::Literal(Literal::Bool(*flag)))
        }
        ConstExpr::Literal(AstLiteral::Char(ch)) => {
            Ok(ada83::Expr::int(i128::from(u32::from(*ch))))
        }
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(ada83::Expr::int(i128::from(*code))),
        ConstExpr::Literal(AstLiteral::String(value)) => text_checked(value, member_name),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                out.push(ch);
            }
            text_checked(&out, member_name)
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let written = ada_float_text(mantissa, exponent.as_deref());
            if is_float128 {
                return Ok(text(&format!(
                    "{mantissa}{}",
                    exponent
                        .as_ref()
                        .map(|e| format!("e{e}"))
                        .unwrap_or_default()
                )));
            }
            Ok(ada83::Expr::Literal(Literal::Float(written)))
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(text(&fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let Some(target) = reference.resolved() else {
                return Err(refuse("a `@default` enumerator that is unresolved"));
            };
            let Ok(enumerator) = ast.enumerator(target) else {
                return Err(refuse(
                    "a `@default` enumerator this frontend cannot follow",
                ));
            };
            Ok(ada83::Expr::name(&component_name(&enumerator.name)))
        }
        _ => {
            let Ok(value) = ast.eval_integral(expr) else {
                return Err(refuse("a `@default` this frontend cannot evaluate"));
            };
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    ada83::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    ada83::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    ada83::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&value.to_string()),
                _ => ada83::Expr::int(value),
            })
        }
    }
}

/// A floating-point literal Ada 83 reads as a real number (RM83 2.4.1,
/// unchanged since).
fn ada_float_text(mantissa: &str, exponent: Option<&str>) -> String {
    let text = match exponent {
        Some(exponent) => format!("{mantissa}e{exponent}"),
        None => mantissa.to_string(),
    };
    if text.contains('.') {
        return text;
    }
    match text.split_once(['e', 'E']) {
        Some((head, tail)) => format!("{head}.0e{tail}"),
        None => format!("{text}.0"),
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd83_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &ada83::Decl, out: &mut Vec<String>) {
    match decl {
        ada83::Decl::SubprogramBody(body) => calls_in_body(&body.body, out),
        ada83::Decl::Object(constant) => calls_in_expr(&constant.value, out),
        ada83::Decl::Type(_)
        | ada83::Decl::Subtype(_)
        | ada83::Decl::Subprogram(_)
        | ada83::Decl::Comment(_)
        | ada83::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[ada83::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stmt(statement: &ada83::Stmt, out: &mut Vec<String>) {
    match statement {
        ada83::Stmt::Null
        | ada83::Stmt::Exit
        | ada83::Stmt::Comment(_)
        | ada83::Stmt::Subtype { .. } => {}
        ada83::Stmt::Var(var) => {
            if let Some(constraint) = &var.constraint {
                calls_in_expr(constraint, out);
            }
            if let Some(value) = &var.value {
                calls_in_expr(value, out);
            }
        }
        ada83::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        ada83::Stmt::Call(value) => calls_in_expr(value, out),
        ada83::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        ada83::Stmt::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_body(then, out);
            if let Some(otherwise) = otherwise {
                calls_in_body(otherwise, out);
            }
        }
        ada83::Stmt::For {
            low, high, body, ..
        } => {
            calls_in_expr(low, out);
            calls_in_expr(high, out);
            calls_in_body(body, out);
        }
        ada83::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        ada83::Stmt::Declare { vars, body } => {
            for var in vars {
                if let Some(constraint) = &var.constraint {
                    calls_in_expr(constraint, out);
                }
                if let Some(value) = &var.value {
                    calls_in_expr(value, out);
                }
            }
            calls_in_body(body, out);
        }
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &ada83::Expr, out: &mut Vec<String>) {
    match expr {
        ada83::Expr::Call { name, args } => {
            if name.segments().len() == 1 {
                if let Some(first) = name.segments().first() {
                    let text = first.as_str();
                    if text.starts_with("zd83_def_") && !out.iter().any(|seen| seen == text) {
                        out.push(text.to_string());
                    }
                }
            }
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        ada83::Expr::Literal(_) | ada83::Expr::Name(_) => {}
        ada83::Expr::Negate(value)
        | ada83::Expr::Unary { value, .. }
        | ada83::Expr::Deref(value) => {
            calls_in_expr(value, out);
        }
        ada83::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        ada83::Expr::Selected { value, .. } => calls_in_expr(value, out),
        ada83::Expr::Indexed { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        ada83::Expr::Slice { value, low, high } => {
            calls_in_expr(value, out);
            calls_in_expr(low, out);
            calls_in_expr(high, out);
        }
        ada83::Expr::Attribute { value, args, .. } => {
            calls_in_expr(value, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        ada83::Expr::Convert { value, .. } => calls_in_expr(value, out),
        ada83::Expr::Aggregate(parts) => {
            for part in parts {
                calls_in_expr(&part.value, out);
            }
        }
        ada83::Expr::New { init, .. } => match init {
            ada83::AllocInit::Range { low, high } => {
                calls_in_expr(low, out);
                calls_in_expr(high, out);
            }
            ada83::AllocInit::Qualified(value) => calls_in_expr(value, out),
        },
    }
}

// -- the small helpers --------------------------------------------------

/// [`text`] where the value comes from a source: refused outside
/// graphic ASCII, for the identical reason `default_ada`'s
/// `text_checked` refuses one (RM83 2.6, unchanged from RM 2.6).
fn text_checked(value: &str, member_name: &str) -> Result<ada83::Expr, EncodeError> {
    if value.chars().all(|ch| ('\u{20}'..='\u{7e}').contains(&ch)) {
        return Ok(text(value));
    }
    Err(EncodeError::NoWriter {
        member: member_name.to_string(),
        kind: "a `@default` string outside graphic ASCII, which an Ada string literal \
               cannot carry",
    })
}

/// `Zd_Wire83.Text_Of ("…")`.
fn text(value: &str) -> ada83::Expr {
    ada83::Expr::Call {
        name: Path::new(&["Zd_Wire83", "Text_Of"]),
        args: vec![ada83::Expr::Literal(Literal::Str(value.to_string()))],
    }
}

fn call(callee: &str) -> ada83::Expr {
    ada83::Expr::Call {
        name: Path::single(callee),
        args: Vec::new(),
    }
}
