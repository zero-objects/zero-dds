// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Nim member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Nim — deliberately another derivation
//! and not a call into any sibling's, because the columns are compared
//! against each other and a shared helper would make them agree about a
//! mistake.
//!
//! ## The shape is the D column's, with two Nim differences
//!
//! A defaulter **returns** Table 9's value and the caller assigns it;
//! `emit/nim.rs`'s decoder opens with `var v = zd_def_T()`. An
//! aggregate's body is a declaration and one assignment per member:
//! `var v: T` default-initialises every field — which for an `Option`
//! member *is* §7.2.2.4.4.4.8's absence and for a `ref` is `nil` — and
//! the assignments state exactly what Table 9 adds.
//!
//! The first difference is the union: its branches are `Option[T]`
//! fields, so the branch a defaulted discriminator selects is assigned
//! through `some(..)` — the marker for "this is the selected branch",
//! which the D column's anonymous union does not carry.
//!
//! The second is the array. Nim's default initialisation gives an
//! enumeration element its **ordinal-zero** pattern, and Table 9 gives
//! it the *first enumerator*, which `@value` may have put anywhere — so
//! an array defaulter fills every element from the element's own
//! defaulter rather than trusting `var v: array[..]`.
//!
//! ## Ordering is part of the answer
//!
//! Nim resolves a module-scope name only after its declaration, so
//! [`nim_defaulters_called_by`] answers the forward declarations first
//! and every body after them — a defaulter may call another (a struct
//! member's, an array element's) in either direction.

use crate::adapter::nim::{field_name, nim_enumerator_constant, nim_type_name, nim_value_type};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::nim::type_tag;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::nim;

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
/// member's value goes through `zdwire.zdRef`, an `@optional` one
/// through `some(..)`, and `@optional @external` through both, outside
/// in — the same order the declaration nests the types.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell. Refused rather than silently replaced by
/// Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: nim::Expr,
) -> Result<Vec<nim::Stmt>, EncodeError> {
    let explicit = node.facets.xtypes.as_ref().and_then(|x| x.default_value);
    let mut value = match explicit {
        Some(expr) => {
            let literal = explicit_value(ast, expr, type_, &node.name)?;
            // The conversion writes the member's own type on the
            // literal: a bare `5` under `some(..)` or `zdwire.zdRef(..)`
            // would fix the generic at Nim's `int` and not at the
            // member's `int32`. A defaulter call needs none — its
            // result type is already the member's.
            let member_type = match nim_value_type(ast, type_) {
                Ok(member_type) => member_type,
                Err(_) => {
                    return Err(EncodeError::NoWriter {
                        member: node.name.clone(),
                        kind: "member whose type has no Nim form",
                    });
                }
            };
            nim::Expr::Convert {
                type_: Box::new(member_type),
                value: Box::new(literal),
            }
        }
        None => call(&defaulter_name(ast, type_), Vec::new()),
    };
    let external = crate::adapter::held_externally(ast, node);
    if external {
        value = call_on(runtime_name("zdRef"), vec![value]);
    }
    if node.facets.is_optional() {
        value = call("some", vec![value]);
    }
    Ok(vec![nim::Stmt::Assign { place, value }])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model. The
/// forward declarations come first; the module documentation says why.
///
/// # Errors
/// [`EncodeError`] for a construct with no Nim default.
pub fn nim_defaulters_called_by(
    ast: &Ast,
    decls: &[nim::ProcDecl],
) -> Result<Vec<nim::ProcDecl>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_proc(decl, &mut called);
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
            // an undeclared-identifier error from the Nim compiler
            // naming the function.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = nim_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_proc(decl, &mut called);
        }
    }
    nim_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name: every
/// forward declaration, then every body.
///
/// # Errors
/// [`EncodeError`] for a construct with no Nim default.
pub fn nim_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<nim::ProcDecl>, EncodeError> {
    let mut prototypes = Vec::new();
    let mut bodies = Vec::new();
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
        let returns = match nim_value_type(ast, resolve_alias(ast, id)) {
            Ok(returns) => returns,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no Nim form to return",
                });
            }
        };
        emitted.push(name.clone());
        let with_body = nim::ProcDecl {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not carry it.",
            ),
            name: Ident::new(&name),
            // Exported for the reason the codec routines are: a caller
            // outside this module reads a sample that the stream did
            // not carry every member of, and Table 9's value for the
            // rest is this procedure's answer.
            exported: true,
            params: Vec::new(),
            returns: Some(returns),
            body: Some(body),
        };
        prototypes.push(nim::ProcDecl {
            doc: Doc::none(),
            body: None,
            ..with_body.clone()
        });
        bodies.push(with_body);
    }
    prototypes.extend(bodies);
    Ok(prototypes)
}

/// The body one defaulter answers with, or `None` where the type has no
/// Nim data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<nim::Stmt>>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name.
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // Refused and not `Ok(None)`. `None` is this function's word for
    // "the type has no Nim data form", and the caller reads it as a
    // reason to emit no defaulter and walk on. A handle the model will
    // not answer for is not that: the type does have a form, and the
    // defaulter the closure was asked for goes missing from the batch
    // while the generated decoder still calls it by name.
    // Refused and not `Ok(None)`. `None` is this function's word for
    // "the type has no Nim data form", and the caller reads it as a
    // reason to emit no defaulter and walk on. A handle the model will
    // not answer for is not that: the type does have a form, and the
    // defaulter the closure was asked for goes missing from the batch
    // while the generated decoder still calls it by name.
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let give = |value: nim::Expr| Some(vec![nim::Stmt::Return(Some(value))]);
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
        // `@default_literal` names — §7.3.1.2.1.10 — by the constant
        // the materialiser declared it under.
        TypeKind::Enumeration { .. } => {
            let enumerator = match crate::emit::default_enumerator_name(ast, id) {
                Some(enumerator) => enumerator,
                None => {
                    return Err(refuse(
                        "enumeration with no enumerator to take as its default",
                    ));
                }
            };
            give(nim::Expr::name(&nim_enumerator_constant(
                ast,
                id,
                &enumerator,
            )))
        }

        // No bit set; the alias's underlying unsigned takes a plain
        // zero.
        TypeKind::Bitmask { .. } => give(nim::Expr::int(0)),

        // A bitset is an object of separate holders; `var v: T` zeroes
        // them and the declaration is the whole body's work.
        TypeKind::Bitset { .. } => Some(vec![
            declare_default(ast, id),
            nim::Stmt::Return(Some(nim::Expr::name("v"))),
        ]),

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries: `var v: seq[..]` is the empty one.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => Some(vec![
            nim::Stmt::Var {
                name: Ident::new("v"),
                type_: Some(match nim_value_type(ast, id) {
                    Ok(type_) => type_,
                    Err(_) => return Err(refuse("collection with no Nim form")),
                }),
                value: None,
            },
            nim::Stmt::Return(Some(nim::Expr::name("v"))),
        ]),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." Every element is filled from the element's own
        // defaulter: Nim's zero-initialisation would give an
        // enumeration element its ordinal-zero pattern, which Table 9
        // does not say.
        TypeKind::Array { element, dims } => {
            let element = match element.resolved() {
                Some(element) => element,
                None => return Err(refuse("array with an unresolved element")),
            };
            let mut lengths = Vec::new();
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => lengths.push(length),
                    Err(_) => return Err(refuse("array whose length does not evaluate")),
                }
            }
            let mut access = nim::Expr::name("v");
            for level in 0..lengths.len() {
                access = nim::Expr::Index {
                    value: Box::new(access),
                    index: Box::new(nim::Expr::Convert {
                        type_: Box::new(nim::Type::named("int")),
                        value: Box::new(nim::Expr::name(&format!("zd_i{level}"))),
                    }),
                };
            }
            let mut inner = vec![nim::Stmt::Assign {
                place: access,
                value: call(&defaulter_name(ast, element), Vec::new()),
            }];
            for (level, length) in lengths.iter().enumerate().rev() {
                inner = vec![nim::Stmt::For {
                    counter: Ident::new(&format!("zd_i{level}")),
                    bound: Box::new(nim::Expr::int(*length)),
                    body: inner,
                }];
            }
            let mut statements = vec![declare_typed(ast, id, own_name)?];
            statements.extend(inner);
            statements.push(nim::Stmt::Return(Some(nim::Expr::name("v"))));
            Some(statements)
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." `var v: T` builds
        // the absences; one assignment per remaining member states the
        // values.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = vec![declare_default(ast, id)];
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                statements.push(nim::Stmt::Assign {
                    place: nim::Expr::dot(nim::Expr::name("v"), &field_name("parent")),
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
                let place = nim::Expr::dot(nim::Expr::name("v"), &field_name(&member_node.name));
                statements.extend(member_default(ast, member_node, member_type, place)?);
            }
            statements.push(nim::Stmt::Return(Some(nim::Expr::name("v"))));
            Some(statements)
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." The branch goes in through `some(..)`, which is
        // this column's marker for the selected branch.
        TypeKind::Union { discriminator, .. } => {
            let discriminator = match discriminator.resolved() {
                Some(discriminator) => discriminator,
                None => return Err(refuse("union with an unresolved discriminator")),
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut statements = vec![
                declare_default(ast, id),
                nim::Stmt::Assign {
                    place: nim::Expr::dot(nim::Expr::name("v"), crate::adapter::nim::DISCRIMINATOR),
                    value: discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
                },
            ];
            if let Some((member_type, field, external)) = selected_by_default(ast, id, selector)? {
                let mut value = call(&defaulter_name(ast, member_type), Vec::new());
                if external {
                    value = call_on(runtime_name("zdRef"), vec![value]);
                }
                statements.push(nim::Stmt::Assign {
                    place: nim::Expr::dot(nim::Expr::name("v"), &field),
                    value: call("some", vec![value]),
                });
            }
            statements.push(nim::Stmt::Return(Some(nim::Expr::name("v"))));
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

/// `var v: <declared name>` — the default-initialised aggregate.
fn declare_default(ast: &Ast, id: Id<TypeNode>) -> nim::Stmt {
    nim::Stmt::Var {
        name: Ident::new("v"),
        type_: Some(nim::Type::named(&nim_type_name(ast, id))),
        value: None,
    }
}

/// `var v: <value type>`, for a collection whose spelling is inline.
fn declare_typed(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<nim::Stmt, EncodeError> {
    let type_ = nim_value_type(ast, id).map_err(|_| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind: "type with no Nim form",
    })?;
    Ok(nim::Stmt::Var {
        name: Ident::new("v"),
        type_: Some(type_),
        value: None,
    })
}

/// Table 9's primitive rows, as the Nim holder each takes. The integers
/// and both character widths are integer zeros — the member holds the
/// code unit — and the floats are `0.0`. A binary128 zero is the
/// decimal `"0.0"`, which is what `readF128` answers for the zero
/// pattern.
fn primitive_default(kind: PrimitiveKind) -> nim::Expr {
    match kind {
        PrimitiveKind::Boolean => nim::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            nim::Expr::Literal(Literal::Float("0.0".to_string()))
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
        | PrimitiveKind::UInt64 => nim::Expr::int(0),
    }
}

/// The case a defaulted discriminator selects, if any, with the
/// `@external` flag its assignment routes through.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: Option<i128>,
) -> Result<Option<(Id<TypeNode>, String, bool)>, EncodeError> {
    let selector = match selector {
        Some(selector) => selector,
        None => return Ok(None),
    };
    Ok(
        selected_case(ast, id, selector)?.map(|(member, member_type)| {
            (
                member_type,
                field_name(&member.name),
                crate::adapter::held_externally(ast, member),
            )
        }),
    )
}

/// A discriminator value in the Nim form its type gives it: the
/// enumerator for an enumeration, `true`/`false` for a `boolean`, and
/// the number for everything else — a `char` and a `wchar`
/// discriminator hold their code unit as an integer here.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> nim::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return nim::Expr::name(&nim_enumerator_constant(ast, resolved, &enumerator));
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => nim::Expr::Literal(Literal::Bool(value != 0)),
        _ => nim::Expr::int(value),
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

/// An explicit `@default` as a Nim expression for `type_`, before the
/// member's own conversion is written around it.
#[allow(clippy::too_many_lines)]
fn explicit_value(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<nim::Expr, EncodeError> {
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
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(nim::Expr::Literal(Literal::Bool(*flag)))
        }
        // The code unit as the number the member holds: `uint8` for a
        // `char`, `uint16` for a `wchar`.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(nim::Expr::int(i128::from(u32::from(*ch)))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(nim::Expr::int(i128::from(*code))),
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
            Ok(nim::Expr::Literal(Literal::Float(written)))
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
            Ok(nim::Expr::name(&nim_enumerator_constant(
                ast,
                resolved,
                &enumerator.name,
            )))
        }
        _ => {
            let value = match ast.eval_integral(expr) {
                Ok(value) => value,
                Err(_) => return Err(refuse("a `@default` this frontend cannot evaluate")),
            };
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    nim::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    nim::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    nim::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&value.to_string()),
                _ => nim::Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` procedures one declaration calls, appended to `out`.
fn calls_in_proc(decl: &nim::ProcDecl, out: &mut Vec<String>) {
    if let Some(body) = &decl.body {
        calls_in_body(body, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting; the walk emits shallow bodies)
fn calls_in_body(body: &[nim::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting; the walk emits shallow bodies)
fn calls_in_stmt(statement: &nim::Stmt, out: &mut Vec<String>) {
    match statement {
        nim::Stmt::Var { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        nim::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        nim::Stmt::Expr(value) | nim::Stmt::Discard(value) => calls_in_expr(value, out),
        nim::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        nim::Stmt::If {
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
        nim::Stmt::For { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        nim::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        nim::Stmt::Break | nim::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting; the walk emits shallow trees)
fn calls_in_expr(expr: &nim::Expr, out: &mut Vec<String>) {
    match expr {
        nim::Expr::Call { func, args } => {
            // Unqualified only: a defaulter is a module-level procedure
            // of the generated file, and this generator built every
            // call to one with `defaulter_name`; `zdwire.zdRef(..)` is
            // the runtime's and never a defaulter.
            if let nim::Expr::Name(name) = func.as_ref() {
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
        nim::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        nim::Expr::Convert { value, .. }
        | nim::Expr::Cast { value, .. }
        | nim::Expr::Paren(value)
        | nim::Expr::Unary { value, .. }
        | nim::Expr::Deref(value) => {
            calls_in_expr(value, out);
        }
        nim::Expr::Dot { value, .. } => calls_in_expr(value, out),
        nim::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        nim::Expr::NewSeq { length, .. } => calls_in_expr(length, out),
        nim::Expr::Literal(_) | nim::Expr::Name(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> nim::Expr {
    nim::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<nim::Expr>) -> nim::Expr {
    call_on(nim::Expr::name(callee), args)
}

fn call_on(func: nim::Expr, args: Vec<nim::Expr>) -> nim::Expr {
    nim::Expr::Call {
        func: Box::new(func),
        args,
    }
}

/// `zdwire.<name>`, qualified so a corpus type of the same name cannot
/// shadow it.
fn runtime_name(name: &str) -> nim::Expr {
    nim::Expr::dot(nim::Expr::name("zdwire"), name)
}
