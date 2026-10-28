// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Kotlin member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Kotlin — the counterpart of
//! [`crate::default_java`], which this column's derivation stays close
//! to: Kotlin holds the same JVM values Java does, and every zero and
//! every emptiness in Table 9 is the same zero and the same emptiness
//! (`adapter/kotlin.rs` has the memory-shape table). What is genuinely
//! Kotlin's own:
//!
//! - **A top-level `fun`, not a `static` method.** A `.kt` file needs no
//!   wrapper class to hold one; `crates/lang-cst/src/kotlin/mod.rs`'s
//!   documentation gives the reason.
//! - **`Char`'s Table 9 zero is `Char(0)`, not shared with `wchar`.**
//!   `adapter/kotlin.rs::kotlin_primitive_type` gives an IDL `char` a
//!   Kotlin `Char` and an IDL `wchar` an `Int` — the two share Java's
//!   `char` and do not share Kotlin's, so their zeros are typed apart
//!   here too.
//! - **An enumeration's constant needs no outer path.** Java's
//!   materialiser nests every declaration inside one wrapper class and
//!   `crate::default_java::enumerator_constant` writes the path down
//!   to it; this column's `object` is already a top-level declaration,
//!   so `Mode.IDLE` is the whole of it.
//! - **A `Byte`/`Short`'s zero is the bare literal `0`.** Kotlin infers
//!   an integer literal's type from the position it is written in when
//!   the literal fits (Kotlin language specification, "Integer literal
//!   types"), so `val v: Byte = 0` needs no cast where Java's
//!   `(byte) 0` does.
//!
//! Deliberately a third derivation from the clause and not a call into
//! either sibling: the columns are compared against each other, and a
//! shared helper would make every one of them agree about a mistake in
//! the same place.

use crate::adapter::cst::MaterialiseError;
use crate::adapter::kotlin::{
    kotlin_type_name, kotlin_value_type, property_name, type_tag, unsigned_holder,
};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::kotlin;

/// The defaulter function name for a type. Keyed the way
/// [`crate::emit::decode::decoder_name`] is, so the two schemes differ
/// in one word and a name that does not resolve is a `kotlinc` error
/// naming the function.
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

/// `place = zd_def_<T>()`.
///
/// # Errors
/// Never; the signature matches the emitters around it.
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: kotlin::Expr) -> kotlin::Stmt {
    kotlin::Stmt::Expr(kotlin::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(call(&defaulter_name(ast, id), Vec::new())),
    })
}

/// One member's default: its `@default`, or its type's. §7.2.2.4.4.4.9.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a Kotlin expression.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: kotlin::Expr,
) -> Result<Vec<kotlin::Stmt>, EncodeError> {
    let Some(expr) = node.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(vec![default_into(ast, type_, place)]);
    };
    let value = explicit(ast, expr, type_, &node.name)?;
    Ok(vec![assign(place, value)])
}

/// The defaulters `members` call, and exactly those — the same
/// fixed-point closure [`crate::default_java::java_defaulters_called_by`]
/// takes, reading the set off the emitted calls rather than deriving it
/// a second time from the model.
///
/// # Errors
/// [`EncodeError`] for a construct with no Kotlin default.
pub fn kotlin_defaulters_called_by(
    ast: &Ast,
    decls: &[kotlin::Decl],
) -> Result<Vec<kotlin::Decl>, EncodeError> {
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
        let produced = kotlin_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    kotlin_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Kotlin default.
pub fn kotlin_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<kotlin::Decl>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let Some(body) = body_of(ast, id, &name)? else {
            continue;
        };
        emitted.push(name.clone());
        out.push(kotlin::Decl::Func(kotlin::FuncDecl {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            name: Ident::new(&name),
            params: Vec::new(),
            returns: value_type(ast, id),
            body,
        }));
    }
    Ok(out)
}

/// The Kotlin type a defaulter answers with.
fn value_type(ast: &Ast, id: Id<TypeNode>) -> kotlin::Type {
    kotlin_value_type(ast, id, "default")
        .unwrap_or_else(|_| kotlin::Type::named(&format!("UnresolvedType{}", id.slot())))
}

/// The statements of one defaulter, or `None` where the type has no
/// Kotlin data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<kotlin::Stmt>>, EncodeError> {
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
    let statements = match &node.kind {
        TypeKind::Primitive(kind) => vec![give_back(primitive_default(*kind))],

        TypeKind::String { .. } => vec![give_back(text(""))],

        TypeKind::Fixed { scale, .. } => vec![give_back(text(&zero_decimal(*scale)))],

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(refuse("typedef with an unresolved target"));
            };
            vec![give_back(call(&defaulter_name(ast, target), Vec::new()))]
        }

        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            vec![give_back(enumerator_constant(ast, id, &enumerator))]
        }

        // No bit set: the holder's Table 9 default is zero and zero
        // sets none.
        TypeKind::Bitmask { .. } => {
            let holder = kotlin::Type::Primitive(unsigned_holder(ast.effective_bit_bound(id)));
            vec![give_back(zero_of(&holder))]
        }

        TypeKind::Bitset { .. } => {
            let mut statements = vec![new_local(value_type(ast, id))];
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    return Err(refuse("a bitfield the model does not hold"));
                };
                if let Some(field_name) = field_node.name.as_ref() {
                    statements.push(assign(
                        member("v", &property_name(field_name)),
                        zero_of(&kotlin::Type::Primitive(unsigned_holder(field_node.bits))),
                    ));
                }
            }
            statements.push(give_back(name("v")));
            statements
        }

        // "A zero-length sequence", and the same for a map's entries —
        // an empty array and not `null`, which is what an `@optional`
        // member's absence means here.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => {
            let kotlin::Type::Array(element) = value_type(ast, id) else {
                return Err(refuse("collection whose Kotlin form is not an array"));
            };
            vec![give_back(kotlin::Expr::NewArray {
                element,
                length: Box::new(kotlin::Expr::int(0)),
            })]
        }

        // "An array of the same dimensions... elements take the default
        // value for their corresponding type." Built as a nested literal
        // of repeated element defaults, because Kotlin's `Array(n) { .. }`
        // constructor takes a trailing lambda this tree has no node for,
        // and a literal says the same thing for a length known at
        // generation time.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(
                    ast.eval_integral(*dim)
                        .map_err(|_| refuse("array whose length does not evaluate"))?,
                );
            }
            let element_default = element_default_of(ast, element, own_name)?;
            let mut expr = element_default;
            let mut type_ = value_type(ast, element);
            for length in lengths.iter().rev() {
                let count = usize::try_from(*length)
                    .map_err(|_| refuse("array whose length does not fit"))?;
                expr = kotlin::Expr::ArrayOf {
                    element: Box::new(type_.clone()),
                    elements: vec![expr; count],
                };
                type_ = kotlin::Type::array(type_);
            }
            vec![give_back(expr)]
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = vec![new_local(value_type(ast, id))];
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                statements.push(default_into(
                    ast,
                    base,
                    member("v", &property_name("parent")),
                ));
            }
            for member_id in ast.members_of(id) {
                // Refused, not skipped. A skipped member leaves the
                // property at whatever the primary constructor's resting
                // value put there, which compiles and is not Table 9's
                // value for it.
                let Ok(member_node) = ast.member(*member_id) else {
                    return Err(EncodeError::NoWriter {
                        member: crate::adapter::cst::handle_of(*member_id),
                        kind: "a member the model does not hold",
                    });
                };
                let field = property_name(&member_node.name);
                if member_node.facets.is_optional() {
                    statements.push(assign(
                        member("v", &field),
                        kotlin::Expr::Literal(Literal::Null),
                    ));
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                statements.extend(member_default(
                    ast,
                    member_node,
                    member_type,
                    member("v", &field),
                )?);
            }
            statements.push(give_back(name("v")));
            statements
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value."
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let mut statements = vec![
                new_local(value_type(ast, id)),
                default_into(ast, discriminator, member("v", "_d")),
            ];
            if let Some((member_type, field)) = selected_by_default(ast, id, discriminator)? {
                statements.push(default_into(ast, member_type, member("v", &field)));
            }
            statements.push(give_back(name("v")));
            statements
        }

        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Ok(None);
        }
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
    Ok(Some(statements))
}

/// The single-element default an array's own element type takes,
/// through a defaulter call where the type is an aggregate this
/// generator declares and a literal where it is a plain value — a
/// defaulter call for every element would work too, but the literal
/// keeps a primitive array's default as one expression rather than as
/// `n` calls to the same zero.
fn element_default_of(
    ast: &Ast,
    element: Id<TypeNode>,
    own_name: &str,
) -> Result<kotlin::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    let Ok(node) = ast.type_(element) else {
        return Err(refuse("array element with an unresolved type"));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),
        TypeKind::String { .. } => text(""),
        TypeKind::Fixed { scale, .. } => text(&zero_decimal(*scale)),
        _ => call(&defaulter_name(ast, element), Vec::new()),
    })
}

/// Table 9's primitive rows, as the Kotlin holder each takes.
///
/// `char` and `wchar` are typed apart here, unlike Java's shared `char`:
/// `adapter/kotlin.rs` gives an IDL `char` a Kotlin `Char` and an IDL
/// `wchar` an `Int`, so their zeros are `Char(0)` and `0` and not one
/// spelling for both.
fn primitive_default(kind: PrimitiveKind) -> kotlin::Expr {
    match kind {
        PrimitiveKind::Boolean => kotlin::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Char => kotlin::Expr::Literal(Literal::Char('\u{0}')),
        PrimitiveKind::Float32 => kotlin::Expr::Literal(Literal::Float("0.0f".to_string())),
        PrimitiveKind::Float64 => kotlin::Expr::Literal(Literal::Float("0.0".to_string())),
        // A binary128 zero is its zero bit pattern; `Float128.zero()`
        // names it so the spelling does not depend on the class layout.
        PrimitiveKind::Float128 => call_on(name("Float128"), "zero", Vec::new()),
        PrimitiveKind::Int64 | PrimitiveKind::UInt32 | PrimitiveKind::UInt64 => {
            kotlin::Expr::Literal(Literal::Int("0L".to_string()))
        }
        // `Byte`, `Short`, `Int` and `wchar`'s `Int`: the bare literal.
        // Kotlin infers the literal's type from the property it
        // initialises when it fits — no cast is needed the way Java's
        // `(byte) 0` needs one.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::UInt16
        | PrimitiveKind::WChar
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::Int32 => kotlin::Expr::int(0),
    }
}

/// The zero of a holder the materialiser chose.
fn zero_of(holder: &kotlin::Type) -> kotlin::Expr {
    if *holder == kotlin::Type::Primitive(kotlin::Primitive::Long) {
        return kotlin::Expr::Literal(Literal::Int("0L".to_string()));
    }
    kotlin::Expr::int(0)
}

/// `Enum.LITERAL` — no outer path: the materialiser declares every
/// `object` at the top level.
pub(crate) fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> kotlin::Expr {
    kotlin::Expr::field(name(&kotlin_type_name(ast, id)), &property_name(enumerator))
}

/// The zero a typedef of a plain value takes, for the function of the
/// typedef's own name `adapter/kotlin.rs` declares beside its
/// `typealias`: Table 9's primitive row, an enumeration's default
/// literal, a bitmask's empty holder. `None` for every other target —
/// a generated class already has a constructor the alias calls, and a
/// collection has no one zero worth naming.
pub(crate) fn alias_zero(ast: &Ast, target: Id<TypeNode>) -> Option<kotlin::Expr> {
    let resolved = resolve_alias(ast, target);
    match ast.type_(resolved).map(|node| &node.kind) {
        Ok(TypeKind::Primitive(kind)) => Some(primitive_default(*kind)),
        Ok(TypeKind::Enumeration { .. }) => crate::emit::default_enumerator_name(ast, resolved)
            .map(|enumerator| enumerator_constant(ast, resolved, &enumerator)),
        Ok(TypeKind::Bitmask { .. }) => Some(zero_of(&kotlin::Type::Primitive(unsigned_holder(
            ast.effective_bit_bound(resolved),
        )))),
        _ => None,
    }
}

/// The case a defaulted discriminator selects, if any.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    discriminator: Id<TypeNode>,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let Some(selector) = discriminator_default_value(ast, discriminator) else {
        return Ok(None);
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, property_name(&member.name))))
}

/// An explicit `@default` as a Kotlin expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<kotlin::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = ast
        .expr(expr)
        .map_err(|_| refuse("a `@default` this frontend cannot follow"))?;
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(kotlin::Expr::Literal(Literal::Bool(*flag)))
        }
        // The member's type decides whether this is a `Char` or the
        // `Int` a `wchar` is here.
        ConstExpr::Literal(AstLiteral::Char(ch)) => {
            point_literal(u32::from(*ch), &kind, member_name)
        }
        ConstExpr::Literal(AstLiteral::WChar(code)) => point_literal(*code, &kind, member_name),
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(text(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                out.push(ch);
            }
            Ok(text(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let suffix = match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Float32)) => "f",
                _ => "",
            };
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            Ok(kotlin::Expr::Literal(Literal::Float(format!(
                "{mantissa}{exponent}{suffix}"
            ))))
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(text(&fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let target = reference
                .resolved()
                .ok_or_else(|| refuse("a `@default` enumerator that is unresolved"))?;
            let enumerator = ast
                .enumerator(target)
                .map_err(|_| refuse("a `@default` enumerator this frontend cannot follow"))?;
            Ok(enumerator_constant(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    // Refused rather than folded to zero: a `@default`
                    // outside a code point's range is a value this column
                    // cannot spell, and NUL is a different sample.
                    let code = u32::try_from(value)
                        .map_err(|_| refuse("a `@default` outside the range of a code point"))?;
                    point_literal(code, &kind, member_name)
                }
                Ok(TypeKind::Primitive(
                    PrimitiveKind::Int64 | PrimitiveKind::UInt32 | PrimitiveKind::UInt64,
                )) => Ok(wide(value)),
                _ => Ok(kotlin::Expr::int(value)),
            }
        }
    }
}

/// A 64-bit constant as Kotlin writes one: the pattern, with an `L`.
///
/// Through [`zerodds_lang_render::kotlin::long_text`] and not a bare
/// `{pattern}L`, for the one pattern that has no such spelling: Kotlin
/// has no negative integer literal, so `-9223372036854775808L` asks for
/// a literal one past `Long.MAX_VALUE` and kotlinc 2.4.0 answers
/// "value out of range" — an error, not a warning. `long_text` writes
/// that pattern as `Long.MIN_VALUE`. The same seam
/// [`crate::emit::kotlin`]'s `wide_int` already goes through.
fn wide(value: i128) -> kotlin::Expr {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let pattern = if value > i128::from(i64::MAX) {
        value as u64 as i64
    } else {
        value as i64
    };
    kotlin::Expr::Literal(Literal::Int(zerodds_lang_render::kotlin::long_text(
        pattern,
    )))
}

/// A code point as the member's own Kotlin holder: `Char(code)` for a
/// `char` member, the bare `Int` for a `wchar` one.
///
/// A `char` member's point that is not a Unicode scalar value is refused
/// by name. `Literal::Char` holds a Rust `char`, which a lone surrogate
/// is not; substituting NUL would write a `@default` into the generated
/// decoder that nothing downstream could tell from a `'\0'` the source
/// declared.
///
/// # Errors
/// [`EncodeError::NoWriter`] for a point outside the Unicode scalar
/// values where the member's holder is a Kotlin `Char`.
fn point_literal<E>(
    code: u32,
    kind: &Result<TypeKind, E>,
    member_name: &str,
) -> Result<kotlin::Expr, EncodeError> {
    if matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Char))) {
        let ch = char::from_u32(code).ok_or_else(|| EncodeError::NoWriter {
            member: member_name.to_string(),
            kind: "a `@default` character that is not a Unicode scalar value",
        })?;
        return Ok(kotlin::Expr::Literal(Literal::Char(ch)));
    }
    Ok(kotlin::Expr::int(i128::from(code)))
}

// -- reading the calls back out ------------------------------------------

fn calls_in_decl(decl: &kotlin::Decl, out: &mut Vec<String>) {
    match decl {
        kotlin::Decl::Func(func) => calls_in_block(&func.body, out),
        kotlin::Decl::Const(const_) => calls_in_expr(&const_.value, out),
        kotlin::Decl::Class(_)
        | kotlin::Decl::Object(_)
        | kotlin::Decl::Alias(_)
        | kotlin::Decl::Interface(_)
        | kotlin::Decl::Comment(_)
        | kotlin::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_block(body: &[kotlin::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &kotlin::Stmt, out: &mut Vec<String>) {
    match statement {
        kotlin::Stmt::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        kotlin::Stmt::Expr(expr) | kotlin::Stmt::Throw(expr) => calls_in_expr(expr, out),
        kotlin::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        kotlin::Stmt::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_block(&then.statements, out);
            if let Some(otherwise) = otherwise {
                calls_in_block(&otherwise.statements, out);
            }
        }
        kotlin::Stmt::CountUp { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_block(&body.statements, out);
        }
        kotlin::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_block(&body.statements, out);
        }
        kotlin::Stmt::Break | kotlin::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
#[allow(clippy::too_many_lines)]
fn calls_in_expr(expr: &kotlin::Expr, out: &mut Vec<String>) {
    match expr {
        kotlin::Expr::Call {
            receiver,
            name,
            args,
        } => {
            if receiver.is_none() {
                let name = name.as_str();
                if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == name) {
                    out.push(name.to_string());
                }
            }
            if let Some(receiver) = receiver {
                calls_in_expr(receiver, out);
            }
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        kotlin::Expr::Field { value, .. } | kotlin::Expr::Paren(value) => calls_in_expr(value, out),
        kotlin::Expr::As { value, .. } => calls_in_expr(value, out),
        kotlin::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        kotlin::Expr::New { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        kotlin::Expr::NewArray { length, .. } => calls_in_expr(length, out),
        kotlin::Expr::ArrayOf { elements, .. } => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        kotlin::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        kotlin::Expr::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_expr(then, out);
            calls_in_expr(otherwise, out);
        }
        kotlin::Expr::Not(value) | kotlin::Expr::NotNull(value) => calls_in_expr(value, out),
        kotlin::Expr::Literal(_) | kotlin::Expr::Path(_) => {}
    }
}

// -- the small helpers ----------------------------------------------------

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `kotlin::Expr::name`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn name(text: &str) -> kotlin::Expr {
    kotlin::Expr::name(text)
}

fn text(value: &str) -> kotlin::Expr {
    kotlin::Expr::Literal(Literal::Str(value.to_string()))
}

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `kotlin::Expr::call`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
fn call(callee: &str, args: Vec<kotlin::Expr>) -> kotlin::Expr {
    kotlin::Expr::call(callee, args)
}

fn call_on(receiver: kotlin::Expr, name: &str, args: Vec<kotlin::Expr>) -> kotlin::Expr {
    kotlin::Expr::method_call(receiver, name, args)
}

fn member(base: &str, field: &str) -> kotlin::Expr {
    kotlin::Expr::field(name(base), field)
}

fn assign(place: kotlin::Expr, value: kotlin::Expr) -> kotlin::Stmt {
    kotlin::Stmt::Expr(kotlin::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(value),
    })
}

fn give_back(value: kotlin::Expr) -> kotlin::Stmt {
    kotlin::Stmt::Return(Some(value))
}

/// `val v = T()`.
fn new_local(type_: kotlin::Type) -> kotlin::Stmt {
    kotlin::Stmt::Declare {
        mutable: false,
        type_: type_.clone(),
        name: Ident::new("v"),
        value: Some(kotlin::Expr::New {
            type_: Box::new(type_),
            args: Vec::new(),
        }),
    }
}

/// Kept so the module's one dependency on the materialiser's error type
/// is stated rather than inferred.
const _: Option<MaterialiseError> = None;
