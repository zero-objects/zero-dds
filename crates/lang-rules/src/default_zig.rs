// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Zig member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Zig — deliberately a further
//! derivation and not a call into any sibling's, because the columns
//! are compared against each other and a shared helper would make them
//! agree about a mistake.
//!
//! ## The shape is the D column's, with two Zig differences
//!
//! A defaulter **returns** Table 9's value and the caller assigns it;
//! `emit/zig.rs`'s decoder opens with `var v: T = try zd_def_T(r.heap);`.
//!
//! **An aggregate's body is one struct literal**, not a declaration and
//! assignments: Zig has no default-initialised `T v;` — a local is
//! either fully written or `undefined` — so the literal is where every
//! member's value is stated. An `@optional` member is left out of it:
//! the field carries `= null` in the type declaration, and absence is
//! §7.2.2.4.4.4.8's default state. A union's unselected branches take
//! `undefined`: no read reaches one — the encoder writes the selected
//! branch, the dumper prints it — and a defaulter call there would
//! recurse forever on a self-referential `@external` branch.
//!
//! **A defaulter takes an allocator.** A non-optional `@external`
//! member is Table 9's value behind the column's indirection —
//! `try zdwire.zdPtr(zd_al, try zd_def_T(zd_al))` — and Zig has no
//! garbage collector to hide the allocation behind. The decoder hands
//! its reader's arena; a defaulter for a type with no such member
//! simply discards the parameter.

use crate::adapter::zig::{field_name, zig_type_name, zig_value_type};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::zig::type_tag;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::zig;

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The name of the allocator parameter every defaulter takes.
const ALLOCATOR: &str = "zd_al";

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
/// member's value goes through `zdwire.zdPtr` with `alloc`; an
/// `@optional` one takes the bare value, which Zig coerces into the
/// `?T` on its own.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell. Refused rather than silently replaced by
/// Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: zig::Expr,
    alloc: zig::Expr,
) -> Result<Vec<zig::Stmt>, EncodeError> {
    let value = member_value(ast, node, type_, &alloc)?;
    let external = crate::adapter::held_externally(ast, node);
    let value = if external { boxed(alloc, value) } else { value };
    Ok(vec![zig::Stmt::Assign { place, value }])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model.
///
/// # Errors
/// [`EncodeError`] for a construct with no Zig default.
pub fn zig_defaulters_called_by(
    ast: &Ast,
    decls: &[zig::Decl],
) -> Result<Vec<zig::Decl>, EncodeError> {
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
            // an undefined-identifier error from the Zig compiler
            // naming the function.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = zig_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    zig_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Zig default.
pub fn zig_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<zig::Decl>, EncodeError> {
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
        let returns = match zig_value_type(ast, resolve_alias(ast, id), &name) {
            Ok(returns) => returns,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no Zig form to return",
                });
            }
        };
        emitted.push(name.clone());
        let mut body = body;
        // The allocator is only reached where an `@external` member or
        // a nested defaulter needs one; Zig refuses an unused
        // parameter, and the discard is its way of saying the non-use
        // is meant.
        if !body_mentions(&body, ALLOCATOR) {
            body.insert(0, zig::Stmt::Discard(Ident::new(ALLOCATOR)));
        }
        out.push(zig::Decl::Func(zig::FuncDecl {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            is_pub: false,
            name: Ident::new(&name),
            params: vec![zig::FnParam {
                name: Ident::new(ALLOCATOR),
                type_: zig::Type::named("std.mem.Allocator"),
            }],
            returns: zig::Type::error_union(zig::Type::named("zdwire.WireError"), returns),
            body,
        }));
    }
    Ok(out)
}

/// Whether any statement of `body` names `name` in an expression.
fn body_mentions(body: &[zig::Stmt], name: &str) -> bool {
    /// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
    /// The tree walked here is the one this generator emitted and owns, so
    /// it holds no cycle; the depth is that expression's height, which
    /// follows the member nesting the type walk already bounded.
    fn in_expr(expr: &zig::Expr, name: &str) -> bool {
        match expr {
            zig::Expr::Name(ident) => ident.as_str() == name,
            zig::Expr::Binary { lhs, rhs, .. } => in_expr(lhs, name) || in_expr(rhs, name),
            zig::Expr::Paren(value)
            | zig::Expr::Unary { value, .. }
            | zig::Expr::Try(value)
            | zig::Expr::Dot { value, .. } => in_expr(value, name),
            zig::Expr::Index { value, index } => in_expr(value, name) || in_expr(index, name),
            zig::Expr::Call { func, args } => {
                in_expr(func, name) || args.iter().any(|arg| in_expr(arg, name))
            }
            zig::Expr::StructLit(fields) => fields.iter().any(|(_, value)| in_expr(value, name)),
            zig::Expr::ListLit(elements) => elements.iter().any(|element| in_expr(element, name)),
            zig::Expr::Literal(_) | zig::Expr::Type(_) => false,
        }
    }
    body.iter().any(|statement| match statement {
        zig::Stmt::Return(Some(value)) | zig::Stmt::Expr(value) | zig::Stmt::DiscardExpr(value) => {
            in_expr(value, name)
        }
        zig::Stmt::Assign { place, value } => in_expr(place, name) || in_expr(value, name),
        zig::Stmt::Var { value, .. } => value.as_ref().is_some_and(|value| in_expr(value, name)),
        _ => false,
    })
}

/// The body one defaulter answers with, or `None` where the type has no
/// Zig data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<zig::Stmt>>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name.
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // Refused and not `Ok(None)`. `None` is this function's word for
    // "the type has no Zig data form", and the caller reads it as a
    // reason to emit no defaulter and walk on. A handle the model will
    // not answer for is not that: the type does have a form, and the
    // defaulter the closure was asked for goes missing from the batch
    // while the generated decoder still calls it by name.
    // Refused and not `Ok(None)`. `None` is this function's word for
    // "the type has no Zig data form", and the caller reads it as a
    // reason to emit no defaulter and walk on. A handle the model will
    // not answer for is not that: the type does have a form, and the
    // defaulter the closure was asked for goes missing from the batch
    // while the generated decoder still calls it by name.
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let give = |value: zig::Expr| Some(vec![zig::Stmt::Return(Some(value))]);
    let alloc = || zig::Expr::Name(Ident::new(ALLOCATOR));
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
        TypeKind::Bitmask { .. } => give(zig::Expr::int(0)),

        // A bitset is a struct of separate holders, every one zero.
        TypeKind::Bitset { .. } => {
            let mut fields = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                fields.push((Ident::new(&field_name(field_ident)), zig::Expr::int(0)));
            }
            give(zig::Expr::StructLit(fields))
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries: the empty slice, typed by the
        // return.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => give(zig::Expr::Unary {
            op: "&",
            value: Box::new(zig::Expr::ListLit(Vec::new())),
        }),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." One call per element, nested list literals, outermost
        // dimension outermost; the return type gives every level its
        // shape.
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
            let mut out = zig::Expr::try_(zig::Expr::call(
                &defaulter_name(ast, element),
                vec![alloc()],
            ));
            for length in lengths.into_iter().rev() {
                out = zig::Expr::ListLit(
                    core::iter::repeat_with(|| out.clone())
                        .take(length)
                        .collect(),
                );
            }
            give(out)
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." The literal
        // states the values; an `@optional` member is left out and its
        // declared `= null` default is the absence.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut fields = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                fields.push((
                    Ident::new(&field_name("parent")),
                    zig::Expr::try_(zig::Expr::call(&defaulter_name(ast, base), vec![alloc()])),
                ));
            }
            for member_id in ast.members_of(id) {
                let member_node = match ast.member(*member_id) {
                    Ok(member_node) => member_node,
                    Err(_) => continue,
                };
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
                let mut value = member_value(ast, member_node, member_type, &alloc())?;
                let external = crate::adapter::held_externally(ast, member_node);
                if external {
                    value = boxed(alloc(), value);
                }
                fields.push((Ident::new(&field_name(&member_node.name)), value));
            }
            give(zig::Expr::StructLit(fields))
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." The unselected branches take `undefined`: no
        // read reaches one, and the module documentation carries why a
        // defaulter call there would be worse.
        TypeKind::Union { discriminator, .. } => {
            let discriminator = match discriminator.resolved() {
                Some(discriminator) => discriminator,
                None => return Err(refuse("union with an unresolved discriminator")),
            };
            let selector = discriminator_default_value(ast, discriminator);
            let selected = selected_by_default(ast, id, selector)?;
            let mut fields = vec![(
                Ident::new("_d"),
                discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            )];
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(refuse("a case the model does not hold"));
                };
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(refuse("a member the model does not hold"));
                };
                let field = field_name(&member.name);
                let value = match &selected {
                    Some((member_type, selected_field)) if *selected_field == field => {
                        zig::Expr::try_(zig::Expr::call(
                            &defaulter_name(ast, *member_type),
                            vec![alloc()],
                        ))
                    }
                    _ => zig::Expr::Name(Ident::new("undefined")),
                };
                fields.push((Ident::new(&field), value));
            }
            give(zig::Expr::StructLit(fields))
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

/// `try zdwire.zdPtr(alloc, value)` — Table 9's value behind the
/// column's `@external` indirection.
fn boxed(alloc: zig::Expr, value: zig::Expr) -> zig::Expr {
    zig::Expr::try_(zig::Expr::Call {
        func: Box::new(zig::Expr::dot(
            zig::Expr::Name(Ident::new("zdwire")),
            "zdPtr",
        )),
        args: vec![alloc, value],
    })
}

/// One member's default as a value: its `@default`, or a call to its
/// type's defaulter.
fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    alloc: &zig::Expr,
) -> Result<zig::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(zig::Expr::try_(zig::Expr::call(
            &defaulter_name(ast, type_),
            vec![alloc.clone()],
        ))),
    }
}

/// Table 9's primitive rows, as the Zig holder each takes. The integers
/// and both character widths are integer zeros — the member holds the
/// code unit — and the three float widths are `0.0`, the binary128
/// included: this column's `f128` is real.
fn primitive_default(kind: PrimitiveKind) -> zig::Expr {
    match kind {
        PrimitiveKind::Boolean => zig::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            zig::Expr::Literal(Literal::Float("0.0".to_string()))
        }
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
        | PrimitiveKind::UInt64 => zig::Expr::int(0),
    }
}

/// An enumerator, in the enum's own scope: `Mode.IDLE`.
fn enumerator_expr(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> zig::Expr {
    zig::Expr::dot(
        zig::Expr::Name(Ident::new(&zig_type_name(ast, id))),
        &field_name(enumerator),
    )
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

/// A discriminator value in the Zig form its type gives it: the
/// enumerator for an enumeration, `true`/`false` for a `boolean`, and
/// the number for everything else — a `char` and a `wchar`
/// discriminator hold their code unit as an integer here.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> zig::Expr {
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
        Some(PrimitiveKind::Boolean) => zig::Expr::Literal(Literal::Bool(value != 0)),
        _ => zig::Expr::int(value),
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

/// An explicit `@default` as a Zig expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<zig::Expr, EncodeError> {
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
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(zig::Expr::Literal(Literal::Bool(*flag)))
        }
        // The code unit as the number the member holds: `u8` for a
        // `char`, `u16` for a `wchar`.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(zig::Expr::int(i128::from(u32::from(*ch)))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(zig::Expr::int(i128::from(*code))),
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
        // The decimal, as a literal: Zig's `comptime_float` is
        // evaluated at `f128` width, so the number in the file is the
        // number the IDL wrote — at every one of the three widths.
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = match exponent.as_ref() {
                Some(exponent) => format!("e{exponent}"),
                None => String::new(),
            };
            Ok(zig::Expr::Literal(Literal::Float(format!(
                "{mantissa}{exponent}"
            ))))
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
                    zig::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    zig::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    zig::Expr::int(value)
                }
                _ => zig::Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &zig::Decl, out: &mut Vec<String>) {
    match decl {
        zig::Decl::Func(func) => calls_in_body(&func.body, out),
        zig::Decl::Const(constant) => calls_in_expr(&constant.value, out),
        zig::Decl::Comment(_) | zig::Decl::Verbatim(_) | zig::Decl::AnalyseAll(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[zig::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stmt(statement: &zig::Stmt, out: &mut Vec<String>) {
    match statement {
        zig::Stmt::Var { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        zig::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        zig::Stmt::Expr(value) | zig::Stmt::DiscardExpr(value) => calls_in_expr(value, out),
        zig::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        zig::Stmt::If {
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
        zig::Stmt::CountUp { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        zig::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        zig::Stmt::Discard(_) | zig::Stmt::Break | zig::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &zig::Expr, out: &mut Vec<String>) {
    match expr {
        zig::Expr::Call { func, args } => {
            // Unqualified only: a defaulter is a file-level function of
            // the generated program, and this generator built every
            // call to one with `defaulter_name`; `zdwire.zdPtr(..)` is
            // the runtime's and never a defaulter.
            if let zig::Expr::Name(name) = func.as_ref() {
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
        zig::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        zig::Expr::Paren(value)
        | zig::Expr::Unary { value, .. }
        | zig::Expr::Try(value)
        | zig::Expr::Dot { value, .. } => calls_in_expr(value, out),
        zig::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        zig::Expr::StructLit(fields) => {
            for (_, value) in fields {
                calls_in_expr(value, out);
            }
        }
        zig::Expr::ListLit(elements) => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        zig::Expr::Literal(_) | zig::Expr::Name(_) | zig::Expr::Type(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> zig::Expr {
    zig::Expr::Literal(Literal::Str(value.to_string()))
}
