// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value an Ada member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Ada — deliberately a further
//! derivation and not a call into any sibling's, because the columns
//! are compared against each other and a shared helper would make them
//! agree about a mistake.
//!
//! ## The shape is the D column's, with two Ada differences
//!
//! A defaulter **returns** Table 9's value and the caller assigns it;
//! `emit/ada.rs`'s decoder opens with `v : T := zd_def_T;`. An
//! aggregate's body is **one record aggregate** rather than a
//! declaration and assignments: an Ada union's branch components live
//! behind the discriminant, which only a whole-record value can set
//! (RM 3.7.1), and the aggregate states every component — an
//! `@optional` member as `(Present => False)`, which *is*
//! §7.2.2.4.4.4.8's absence.
//!
//! A **sequence** defaults to `new …_Array (0 .. -1)` — an allocated
//! empty array, never `null`: the encoder measures `v.all'Length`, and
//! a null access would turn Table 9's empty sequence into a
//! `Constraint_Error`.

use crate::adapter::ada::{DISCRIMINANT, PRESENT, ada_type_name, component_name, type_tag};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::ada;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

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
        _ => format!("zd_def_{}", type_tag(ast, id)),
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
/// `place`. §7.2.2.4.4.4.9 puts the explicit one first. An `@optional`
/// member's wrapper record takes the engaged aggregate, because its two
/// halves change together or not at all (RM 3.7.1).
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell. Refused rather than silently replaced by
/// Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: ada::Expr,
) -> Result<Vec<ada::Stmt>, EncodeError> {
    let mut value = member_value(ast, node, type_)?;
    if node.facets.is_optional() {
        value = engaged(value);
    }
    Ok(vec![ada::Stmt::Assign { place, value }])
}

/// `(Present => True, Value => value)`.
fn engaged(value: ada::Expr) -> ada::Expr {
    ada::Expr::Aggregate(vec![
        ada::Association {
            choice: Ident::new(PRESENT),
            value: ada::Expr::Literal(Literal::Bool(true)),
        },
        ada::Association {
            choice: Ident::new(crate::adapter::ada::VALUE),
            value,
        },
    ])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model. The
/// answer is every profile first and every body after, because Ada
/// resolves nothing forward and the defaulters call each other.
///
/// # Errors
/// [`EncodeError`] for a construct with no Ada default.
pub fn ada_defaulters_called_by(
    ast: &Ast,
    decls: &[ada::Decl],
) -> Result<Vec<ada::Decl>, EncodeError> {
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
            // an undefined-identifier error from GNAT naming the
            // function.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = ada_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    ada_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name: the
/// profiles, then the bodies.
///
/// # Errors
/// [`EncodeError`] for a construct with no Ada default.
pub fn ada_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<ada::Decl>, EncodeError> {
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
        let returns = match crate::adapter::ada::ada_value_path(ast, resolve_alias(ast, id)) {
            Ok(returns) => returns,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no Ada form to return",
                });
            }
        };
        emitted.push(name.clone());
        let spec = ada::Subprogram {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            kind: ada::SubprogramKind::Function,
            name: Ident::new(&name),
            params: Vec::new(),
            result: Some(returns),
            is_abstract: false,
        };
        specs.push(ada::Decl::Subprogram(spec.clone()));
        bodies.push(ada::Decl::SubprogramBody(ada::SubprogramBody {
            spec: ada::Subprogram {
                doc: Doc::none(),
                ..spec
            },
            body,
        }));
    }
    specs.append(&mut bodies);
    Ok(specs)
}

/// The body one defaulter answers with, or `None` where the type has no
/// Ada data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<ada::Stmt>>, EncodeError> {
    // Before the table and not as one of its rows: a typedef's body is
    // its target's, under the typedef's own name.
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
    let give = |value: ada::Expr| Some(vec![ada::Stmt::Return(Some(value))]);
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
        // `@default_literal` names — §7.3.1.2.1.10 — by name; the
        // literal resolves by the function's result type.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            give(ada::Expr::name(&component_name(&enumerator)))
        }

        // No bit set; the derived modular type takes the literal.
        TypeKind::Bitmask { .. } => give(ada::Expr::int(0)),

        // A bitset is a record of separate holders, each zero.
        TypeKind::Bitset { .. } => {
            let mut parts = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                parts.push(ada::Association {
                    choice: Ident::new(&component_name(field_name)),
                    value: ada::Expr::int(0),
                });
            }
            give(ada::Expr::Aggregate(parts))
        }

        // "A zero-length sequence of the same element type", allocated:
        // the encoder reads `v.all'Length` and a `null` access is a
        // different value from the empty sequence.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => give(ada::Expr::New {
            mark: Path::single(&format!("{}_Array", ada_type_name(ast, id))),
            init: ada::AllocInit::Range {
                low: Box::new(ada::Expr::int(0)),
                high: Box::new(ada::Expr::int(-1)),
            },
        }),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." `(others => …)`, one level per dimension.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut out = call(&defaulter_name(ast, element));
            for _ in dims {
                out = ada::Expr::Aggregate(vec![ada::Association {
                    choice: Ident::new("others"),
                    value: out,
                }]);
            }
            give(out)
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." One aggregate:
        // Ada requires every component stated, and `(Present => False)`
        // is the absence.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut parts = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                parts.push(ada::Association {
                    choice: Ident::new(&component_name("parent")),
                    value: call(&defaulter_name(ast, base)),
                });
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                let choice = Ident::new(&component_name(&member_node.name));
                // A member the adapter holds by reference is absent as
                // `null`; `(Present => False)` is the record form the
                // access type is not
                // (`crate::adapter::ada::held_by_reference`).
                let external = member_node
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|xtypes| xtypes.is_external);
                if member_node.type_.resolved().is_some_and(|member_type| {
                    crate::adapter::ada::held_by_reference(
                        ast,
                        id,
                        member_type,
                        member_node.facets.is_optional(),
                        external,
                    )
                }) {
                    parts.push(ada::Association {
                        choice,
                        value: ada::Expr::Literal(Literal::Null),
                    });
                    continue;
                }
                if member_node.facets.is_optional() {
                    parts.push(ada::Association {
                        choice,
                        value: ada::Expr::Aggregate(vec![ada::Association {
                            choice: Ident::new(PRESENT),
                            value: ada::Expr::Literal(Literal::Bool(false)),
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
                parts.push(ada::Association {
                    choice,
                    value: member_value(ast, member_node, member_type)?,
                });
            }
            give(ada::Expr::Aggregate(parts))
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." One aggregate, because only a whole value sets
        // a discriminant.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut parts = vec![ada::Association {
                choice: Ident::new(DISCRIMINANT),
                value: discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            }];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                parts.push(ada::Association {
                    choice: Ident::new(&field),
                    value: call(&defaulter_name(ast, member_type)),
                });
            }
            give(ada::Expr::Aggregate(parts))
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
fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<ada::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(call(&defaulter_name(ast, type_))),
    }
}

/// Table 9's primitive rows, as the Ada holder each takes. The integers
/// and both character widths are integer zeros — the member holds the
/// code unit — and the floats are `0.0`. A binary128 zero is the
/// decimal `"0.0"`, which is what `Read_F128` would answer for the zero
/// pattern.
fn primitive_default(kind: PrimitiveKind) -> ada::Expr {
    match kind {
        PrimitiveKind::Boolean => ada::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            ada::Expr::Literal(Literal::Float("0.0".to_string()))
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
        | PrimitiveKind::UInt64 => ada::Expr::int(0),
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

/// A discriminator value in the Ada form its type gives it: the
/// enumeration literal, `True`/`False` for a `boolean`, and the number
/// for everything else — a variant aggregate's discriminant has to be a
/// value of its subtype, and the aggregate here is static in every
/// arm.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> ada::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return ada::Expr::name(&component_name(&enumerator));
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => ada::Expr::Literal(Literal::Bool(value != 0)),
        _ => ada::Expr::int(value),
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

/// An explicit `@default` as an Ada expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<ada::Expr, EncodeError> {
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
            Ok(ada::Expr::Literal(Literal::Bool(*flag)))
        }
        // The code unit as the number the member holds.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(ada::Expr::int(i128::from(u32::from(*ch)))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(ada::Expr::int(i128::from(*code))),
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
                // The member holds decimal text and the runtime parses
                // it exactly.
                return Ok(text(&format!(
                    "{mantissa}{}",
                    exponent
                        .as_ref()
                        .map(|e| format!("e{e}"))
                        .unwrap_or_default()
                )));
            }
            Ok(ada::Expr::Literal(Literal::Float(written)))
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
            Ok(ada::Expr::name(&component_name(&enumerator.name)))
        }
        _ => {
            let Ok(value) = ast.eval_integral(expr) else {
                return Err(refuse("a `@default` this frontend cannot evaluate"));
            };
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    ada::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    ada::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    ada::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&value.to_string()),
                _ => ada::Expr::int(value),
            })
        }
    }
}

/// A floating-point literal Ada reads as a real number: RM 2.4.1 makes
/// a numeric literal with no point an integer literal, exponent or not,
/// so the point is put back in front of the exponent.
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

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &ada::Decl, out: &mut Vec<String>) {
    match decl {
        ada::Decl::SubprogramBody(body) => calls_in_body(&body.body, out),
        ada::Decl::Object(constant) => calls_in_expr(&constant.value, out),
        // Types, subtypes, representation clauses, profiles and
        // comments hold no calls a defaulter answers.
        ada::Decl::Type(_)
        | ada::Decl::Subtype(_)
        | ada::Decl::EnumRep(_)
        | ada::Decl::Subprogram(_)
        | ada::Decl::Comment(_)
        | ada::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[ada::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stmt(statement: &ada::Stmt, out: &mut Vec<String>) {
    match statement {
        ada::Stmt::Null | ada::Stmt::Exit | ada::Stmt::Comment(_) | ada::Stmt::Subtype { .. } => {}
        ada::Stmt::Var(var) => {
            if let Some(constraint) = &var.constraint {
                calls_in_expr(constraint, out);
            }
            if let Some(value) = &var.value {
                calls_in_expr(value, out);
            }
        }
        ada::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        ada::Stmt::Call(value) => calls_in_expr(value, out),
        ada::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        ada::Stmt::If {
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
        ada::Stmt::For {
            low, high, body, ..
        } => {
            calls_in_expr(low, out);
            calls_in_expr(high, out);
            calls_in_body(body, out);
        }
        ada::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        ada::Stmt::Declare { vars, body } => {
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
fn calls_in_expr(expr: &ada::Expr, out: &mut Vec<String>) {
    match expr {
        ada::Expr::Call { name, args } => {
            // Single-segment only: a defaulter is a declaration of the
            // generated program itself, and this generator built every
            // call to one with `defaulter_name`; `Zd_Wire.…` is the
            // runtime's and never a defaulter.
            if name.segments().len() == 1 {
                if let Some(first) = name.segments().first() {
                    let text = first.as_str();
                    if text.starts_with("zd_def_") && !out.iter().any(|seen| seen == text) {
                        out.push(text.to_string());
                    }
                }
            }
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        ada::Expr::Literal(_) | ada::Expr::Name(_) | ada::Expr::BoxDefault => {}
        ada::Expr::Negate(value) | ada::Expr::Unary { value, .. } | ada::Expr::Deref(value) => {
            calls_in_expr(value, out);
        }
        ada::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        ada::Expr::Selected { value, .. } => calls_in_expr(value, out),
        ada::Expr::Indexed { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        ada::Expr::Slice { value, low, high } => {
            calls_in_expr(value, out);
            calls_in_expr(low, out);
            calls_in_expr(high, out);
        }
        ada::Expr::Attribute { value, args, .. } => {
            calls_in_expr(value, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        ada::Expr::Convert { value, .. } => calls_in_expr(value, out),
        ada::Expr::Aggregate(parts) => {
            for part in parts {
                calls_in_expr(&part.value, out);
            }
        }
        ada::Expr::New { init, .. } => match init {
            ada::AllocInit::Range { low, high } => {
                calls_in_expr(low, out);
                calls_in_expr(high, out);
            }
            ada::AllocInit::Qualified(value) => calls_in_expr(value, out),
        },
    }
}

// -- the small helpers --------------------------------------------------

/// [`text`] where the value comes from a source: refused outside
/// graphic ASCII, because GNAT reads an unmarked source file as Latin-1
/// and a UTF-8 byte in the 0x80–0x9F range is not a graphic character
/// an Ada string literal can carry (RM 2.6).
fn text_checked(value: &str, member_name: &str) -> Result<ada::Expr, EncodeError> {
    if value.chars().all(|ch| ('\u{20}'..='\u{7e}').contains(&ch)) {
        return Ok(text(value));
    }
    Err(EncodeError::NoWriter {
        member: member_name.to_string(),
        kind: "a `@default` string outside graphic ASCII, which an Ada string literal \
               cannot carry",
    })
}

/// `Zd_Wire.To_Text ("…")` — an `Unbounded_String` from a literal;
/// the value is graphic ASCII or the caller refused it upstream.
fn text(value: &str) -> ada::Expr {
    ada::Expr::Call {
        name: Path::new(&["Zd_Wire", "To_Text"]),
        args: vec![ada::Expr::Literal(Literal::Str(value.to_string()))],
    }
}

fn call(callee: &str) -> ada::Expr {
    ada::Expr::Call {
        name: Path::single(callee),
        args: Vec::new(),
    }
}
