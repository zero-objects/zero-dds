// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a D member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as D — deliberately a ninth derivation
//! and not a call into any sibling's, because the columns are compared
//! against each other and a shared helper would make them agree about a
//! mistake.
//!
//! ## The shape is the Go column's, with one D difference
//!
//! A defaulter **returns** Table 9's value and the caller assigns it;
//! `emit/d.rs`'s decoder opens with `auto v = zd_def_T();`. An
//! **aggregate's** body is a declaration and one assignment per member
//! rather than a struct literal: a D union's branches live in an
//! anonymous `union` whose fields a named struct literal does not
//! reliably reach, and `T v;` default-initialises every field — which
//! for a `Nullable` member *is* §7.2.2.4.4.4.8's absence and for an
//! `@external` pointer is `null` — so the assignments state exactly
//! what Table 9 adds.
//!
//! A **non-optional `@external`** member is Table 9's value behind the
//! column's own indirection: `v.m = zdwire.zdPtr(zd_def_T());`. An
//! `@optional` one — external or not — is left untouched: absence is
//! the default state the declaration built.

use crate::adapter::d::{d_type_name, d_value_type, field_name};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::d::type_tag;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::d;

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type, keyed the way the decoder
/// is: by the IDL kind for a primitive and by the codec tag for
/// everything else.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", type_tag(ast, id, 0)),
    }
}

/// The defaulter that goes with a decoder, by name.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// One member's default — its `@default`, or its type's — assigned into
/// `place`. §7.2.2.4.4.4.9 puts the explicit one first. An `@external`
/// member's value goes through `zdwire.zdPtr`, an `@optional` one
/// through `zdwire.zdNullable` with the member's own type written on
/// the value, and `@optional @external` is the pointer alone.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell. Refused rather than silently replaced by
/// Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: d::Expr,
) -> Result<Vec<d::Stmt>, EncodeError> {
    let mut value = member_value(ast, node, type_)?;
    let external = crate::adapter::held_externally(ast, node);
    if external {
        value = call_on(runtime_name("zdPtr"), vec![value]);
    } else if node.facets.is_optional() {
        let member_type = match d_value_type(ast, type_) {
            Ok(member_type) => member_type,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: node.name.clone(),
                    kind: "optional member whose type has no D form",
                });
            }
        };
        value = call_on(
            runtime_name("zdNullable"),
            vec![d::Expr::Cast {
                type_: Box::new(member_type),
                value: Box::new(value),
            }],
        );
    }
    Ok(vec![d::Stmt::Assign { place, value }])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model.
///
/// # Errors
/// [`EncodeError`] for a construct with no D default.
pub fn d_defaulters_called_by(ast: &Ast, decls: &[d::Decl]) -> Result<Vec<d::Decl>, EncodeError> {
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
            // A name with no type behind it cannot happen for a call
            // this generator emitted; if it ever does, the omission is
            // an undefined-identifier error from the D compiler naming
            // the function.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = d_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    d_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no D default.
pub fn d_defaulters_over(ast: &Ast, types: &[Id<TypeNode>]) -> Result<Vec<d::Decl>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let body = match body_of(ast, id, &name)? {
            Some(body) => body,
            None => continue,
        };
        let returns = match d_value_type(ast, resolve_alias(ast, id)) {
            Ok(returns) => returns,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no D form to return",
                });
            }
        };
        emitted.push(name.clone());
        out.push(d::Decl::Func(d::FuncDecl {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            name: Ident::new(&name),
            params: Vec::new(),
            returns: Some(returns),
            body,
        }));
    }
    Ok(out)
}

/// The body one defaulter answers with, or `None` where the type has no
/// D data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<d::Stmt>>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name.
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // Refused and not `Ok(None)`. `None` is this function's word for
    // "the type has no D data form", and the caller reads it as a
    // reason to emit no defaulter and walk on. A handle the model will
    // not answer for is not that: the type does have a form, and the
    // defaulter the closure was asked for goes missing from the batch
    // while the generated decoder still calls it by name.
    // Refused and not `Ok(None)`. `None` is this function's word for
    // "the type has no D data form", and the caller reads it as a
    // reason to emit no defaulter and walk on. A handle the model will
    // not answer for is not that: the type does have a form, and the
    // defaulter the closure was asked for goes missing from the batch
    // while the generated decoder still calls it by name.
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let give = |value: d::Expr| Some(vec![d::Stmt::Return(Some(value))]);
    let body = match &node.kind {
        TypeKind::Primitive(kind) => give(primitive_default(*kind)),

        // `""`, for both widths: the member holds UTF-8 either way.
        TypeKind::String { .. } => give(text("")),

        // The one decimal spelling of zero for the scale.
        TypeKind::Fixed { scale, .. } => give(text(&zero_decimal(*scale))),

        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10 — by name, in the
        // enum's own scope: `Mode.IDLE`.
        TypeKind::Enumeration { .. } => {
            let enumerator = match crate::emit::default_enumerator_name(ast, id) {
                Some(enumerator) => enumerator,
                None => {
                    return Err(refuse(
                        "enumeration with no enumerator to take as its default",
                    ));
                }
            };
            give(enumerator_expr(ast, id, &enumerator))
        }

        // No bit set; the alias's underlying unsigned takes a plain
        // zero.
        TypeKind::Bitmask { .. } => give(d::Expr::int(0)),

        // A bitset is a struct of separate holders; `T v;` zeroes them
        // and the declaration is the whole body's work.
        TypeKind::Bitset { .. } => {
            let statements = vec![
                d::Stmt::Var {
                    name: Ident::new("v"),
                    type_: Some(d::Type::named(&d_type_name(ast, id))),
                    value: None,
                },
                d::Stmt::Return(Some(d::Expr::Name(Ident::new("v")))),
            ];
            Some(statements)
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries: the empty literal, typed by the
        // return.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => give(d::Expr::ArrayLit(Vec::new())),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." One call per element, nested literals, outermost
        // dimension outermost — the renderer writes the suffixes right
        // to left.
        TypeKind::Array { element, dims } => {
            let element = match element.resolved() {
                Some(element) => element,
                None => return Err(refuse("array with an unresolved element")),
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let length = match ast.eval_integral(*dim) {
                    Ok(length) => length,
                    Err(_) => return Err(refuse("array whose length does not evaluate")),
                };
                let length = match usize::try_from(length) {
                    Ok(length) => length,
                    Err(_) => return Err(refuse("array of a length this column cannot spell")),
                };
                lengths.push(length);
            }
            let mut out = call(&defaulter_name(ast, element), Vec::new());
            for length in lengths.into_iter().rev() {
                out = d::Expr::ArrayLit(
                    core::iter::repeat_with(|| out.clone())
                        .take(length)
                        .collect(),
                );
            }
            give(out)
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." `T v;` builds the
        // absences; one assignment per remaining member states the
        // values.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = vec![d::Stmt::Var {
                name: Ident::new("v"),
                type_: Some(d::Type::named(&d_type_name(ast, id))),
                value: None,
            }];
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                let place = d::Expr::Dot {
                    value: Box::new(d::Expr::Name(Ident::new("v"))),
                    name: Ident::new(&field_name("parent")),
                };
                statements.push(d::Stmt::Assign {
                    place,
                    value: call(&defaulter_name(ast, base), Vec::new()),
                });
            }
            for member_id in ast.members_of(id) {
                let member_node = match ast.member(*member_id) {
                    Ok(member_node) => member_node,
                    Err(_) => continue,
                };
                // Left untouched: the field's default-initialised state
                // is already the absence.
                if member_node.facets.is_optional() {
                    continue;
                }
                let member_type = match member_node.type_.resolved() {
                    Some(member_type) => member_type,
                    None => {
                        return Err(EncodeError::NoWriter {
                            member: member_node.name.clone(),
                            kind: "member with an unresolved type",
                        });
                    }
                };
                let place = d::Expr::Dot {
                    value: Box::new(d::Expr::Name(Ident::new("v"))),
                    name: Ident::new(&field_name(&member_node.name)),
                };
                statements.extend(member_default(ast, member_node, member_type, place)?);
            }
            statements.push(d::Stmt::Return(Some(d::Expr::Name(Ident::new("v")))));
            Some(statements)
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." Assignments and not a literal, because the
        // branches live in an anonymous union.
        TypeKind::Union { discriminator, .. } => {
            let discriminator = match discriminator.resolved() {
                Some(discriminator) => discriminator,
                None => return Err(refuse("union with an unresolved discriminator")),
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut statements = vec![
                d::Stmt::Var {
                    name: Ident::new("v"),
                    type_: Some(d::Type::named(&d_type_name(ast, id))),
                    value: None,
                },
                d::Stmt::Assign {
                    place: d::Expr::Dot {
                        value: Box::new(d::Expr::Name(Ident::new("v"))),
                        name: Ident::new("_d"),
                    },
                    value: discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
                },
            ];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                statements.push(d::Stmt::Assign {
                    place: d::Expr::Dot {
                        value: Box::new(d::Expr::Name(Ident::new("v"))),
                        name: Ident::new(&field),
                    },
                    value: call(&defaulter_name(ast, member_type), Vec::new()),
                });
            }
            statements.push(d::Stmt::Return(Some(d::Expr::Name(Ident::new("v")))));
            Some(statements)
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

/// One member's default as a value: its `@default`, or a call to its
/// type's defaulter.
fn member_value(ast: &Ast, node: &MemberNode, type_: Id<TypeNode>) -> Result<d::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, as the D holder each takes. The integers
/// and both character widths are integer zeros — the member holds the
/// code unit — and the floats are `0.0`. A binary128 zero is the
/// decimal `"0.0"`, which is what `ReadF128` answers for the zero
/// pattern.
fn primitive_default(kind: PrimitiveKind) -> d::Expr {
    match kind {
        PrimitiveKind::Boolean => d::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            d::Expr::Literal(Literal::Float("0.0".to_string()))
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
        | PrimitiveKind::UInt64 => d::Expr::int(0),
    }
}

/// An enumerator, in the enum's own scope: `Mode.IDLE`.
fn enumerator_expr(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> d::Expr {
    d::Expr::Name(Ident::new(&format!(
        "{}.{}",
        d_type_name(ast, id),
        field_name(enumerator)
    )))
}

/// The case a defaulted discriminator selects, if any.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: Option<i128>,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let selector = match selector {
        Some(selector) => selector,
        None => return Ok(None),
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, field_name(&member.name))))
}

/// A discriminator value in the D form its type gives it: the
/// enumerator for an enumeration, `true`/`false` for a `boolean`, and
/// the number for everything else — a `char` and a `wchar`
/// discriminator hold their code unit as an integer here.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> d::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return enumerator_expr(ast, resolved, &enumerator);
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => d::Expr::Literal(Literal::Bool(value != 0)),
        _ => d::Expr::int(value),
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

/// An explicit `@default` as a D expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<d::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = match ast.expr(expr) {
        Ok(node) => node,
        Err(_) => return Err(refuse("a `@default` this frontend cannot follow")),
    };
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    let is_float128 = matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128)));
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => Ok(d::Expr::Literal(Literal::Bool(*flag))),
        // The code unit as the number the member holds: `ubyte` for a
        // `char`, `ushort` for a `wchar`.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(d::Expr::int(i128::from(u32::from(*ch)))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(d::Expr::int(i128::from(*code))),
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(text(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let ch = match char::from_u32(*code) {
                    Some(ch) => ch,
                    None => {
                        return Err(refuse("a `@default` wide string that is not code points"));
                    }
                };
                out.push(ch);
            }
            Ok(text(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = match exponent.as_ref() {
                Some(exponent) => format!("e{exponent}"),
                None => String::new(),
            };
            let written = format!("{mantissa}{exponent}");
            if is_float128 {
                // The member holds decimal text and the runtime parses
                // it exactly.
                return Ok(text(&written));
            }
            Ok(d::Expr::Literal(Literal::Float(written)))
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(text(&fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let target = match reference.resolved() {
                Some(target) => target,
                None => return Err(refuse("a `@default` enumerator that is unresolved")),
            };
            let enumerator = match ast.enumerator(target) {
                Ok(enumerator) => enumerator,
                Err(_) => {
                    return Err(refuse(
                        "a `@default` enumerator this frontend cannot follow",
                    ));
                }
            };
            Ok(enumerator_expr(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = match ast.eval_integral(expr) {
                Ok(value) => value,
                Err(_) => return Err(refuse("a `@default` this frontend cannot evaluate")),
            };
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    d::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    d::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    d::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&value.to_string()),
                _ => d::Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &d::Decl, out: &mut Vec<String>) {
    match decl {
        d::Decl::Func(func) => calls_in_body(&func.body, out),
        d::Decl::Const(constant) => calls_in_expr(&constant.value, out),
        // Types, enums, aliases, interfaces and comments hold no calls.
        d::Decl::Struct(_)
        | d::Decl::Enum(_)
        | d::Decl::Alias(_)
        | d::Decl::Interface(_)
        | d::Decl::Comment(_)
        | d::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[d::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stmt(statement: &d::Stmt, out: &mut Vec<String>) {
    match statement {
        d::Stmt::Var { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        d::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        d::Stmt::Expr(value) => calls_in_expr(value, out),
        d::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        d::Stmt::If {
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
        d::Stmt::For { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        d::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        d::Stmt::Break | d::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &d::Expr, out: &mut Vec<String>) {
    match expr {
        d::Expr::Call { func, args } => {
            // Unqualified only: a defaulter is a module-level function
            // of the generated file, and this generator built every
            // call to one with `defaulter_name`; `zdwire.zdPtr(..)` is
            // the runtime's and never a defaulter.
            if let d::Expr::Name(name) = func.as_ref() {
                let name = name.as_str();
                if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == name) {
                    out.push(name.to_string());
                }
            }
            calls_in_expr(func, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        d::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        d::Expr::Cast { value, .. } | d::Expr::Paren(value) | d::Expr::Unary { value, .. } => {
            calls_in_expr(value, out);
        }
        d::Expr::Dot { value, .. } => calls_in_expr(value, out),
        d::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        d::Expr::SliceTo { value, high } => {
            calls_in_expr(value, out);
            calls_in_expr(high, out);
        }
        d::Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                calls_in_expr(value, out);
            }
        }
        d::Expr::ArrayLit(elements) => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        d::Expr::NewSlice { length, .. } => calls_in_expr(length, out),
        d::Expr::Literal(_) | d::Expr::Name(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> d::Expr {
    d::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<d::Expr>) -> d::Expr {
    call_on(d::Expr::Name(Ident::new(callee)), args)
}

fn call_on(func: d::Expr, args: Vec<d::Expr>) -> d::Expr {
    d::Expr::Call {
        func: Box::new(func),
        args,
    }
}

/// `zdwire.<name>`, qualified so a corpus type of the same name cannot
/// shadow it.
fn runtime_name(name: &str) -> d::Expr {
    d::Expr::Dot {
        value: Box::new(d::Expr::Name(Ident::new("zdwire"))),
        name: Ident::new(name),
    }
}
