// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Julia member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Julia. [`crate::default_python`] is the
//! same table as Python and [`crate::default_c`] as C; deliberately a
//! further derivation and not a call into any of them, because the
//! columns are compared against each other and a shared helper would
//! make them agree about a mistake.
//!
//! ## Why a defaulter answers a `ZdScratch` and not the declared struct
//!
//! `adapter/julia.rs` declares every aggregate immutable, and the
//! decoder in `emit/julia.rs` opens with `v = zd_def_T()` and then
//! assigns members one at a time — which an immutable value cannot
//! take. So an aggregate's defaulter answers the runtime's mutable
//! `ZdScratch`, pre-filled with Table 9's values under the *escaped
//! field names* `zd_freeze` will read them back by, and the freeze at
//! every return builds the immutable value. A scalar's defaulter
//! answers the scalar itself: there is nothing to assign into one.
//!
//! ## The width is in the value
//!
//! `Int32(0)` and never a bare `0`: a bare integer literal is a
//! platform `Int`, and although `zd_freeze` would convert it at the
//! end, a scratch that already holds the declared width is one whose
//! intermediate reads — a discriminator compared against a label —
//! behave like the member they stand for.
//!
//! ## An `@optional` member is left out of the scratch
//!
//! §7.2.2.4.4.4.8's "Implementations shall not provide a default value"
//! is the absence: `ZdScratch` answers `nothing` for a slot nothing
//! filled, and `nothing` is what the declared `Union{T, Nothing}` field
//! holds for an absent member.

use crate::adapter::julia::julia_type_name;
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::CodecTarget;
use crate::emit::julia::Julia;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::julia::{self as jl, escape_field, escape_name};

/// How deep a chain of typedefs is followed before the model is taken to
/// be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type.
///
/// Keyed the same way `emit::decode::decoder_name` is: a primitive's
/// defaulter is named by its IDL kind, because `octet` and `char` share
/// a Julia type and do not share a wire form.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", Julia::type_name(ast, id)),
    }
}

/// The defaulter that goes with a decoder, by name.
///
/// The two schemes differ in one word — `zd_dec_p_long` against
/// `zd_def_p_long` — because both are built from the same key.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_<T>()`.
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: jl::Expr) -> jl::Statement {
    jl::Statement::Assign {
        place,
        value: jl::Expr::free_call(&defaulter_name(ast, id), Vec::new()),
    }
}

/// One member's default: its `@default`, or its type's.
///
/// §7.2.2.4.4.4.9 puts the explicit one first and falls back to
/// Table 9.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a Julia expression. Refused rather than
/// silently replaced by Table 9's zero: a member the designer gave a
/// default and a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: jl::Expr,
) -> Result<Vec<jl::Statement>, EncodeError> {
    Ok(vec![jl::Statement::Assign {
        place,
        value: member_value(ast, node, type_)?,
    }])
}

/// The defaulters `decls` call, and exactly those.
///
/// The set is read off the emitted calls rather than derived a second
/// time from the model, for the reason
/// [`crate::default_c::c_defaulters_called_by`] gives: a derivation
/// beside the decoder's is a second reading of §7.2.2.7, and the two
/// readings drift. The closure is taken by emitting and re-reading
/// until the batch calls nothing it does not also define.
///
/// # Errors
/// [`EncodeError`] for a construct with no Julia default.
pub fn julia_defaulters_called_by(
    ast: &Ast,
    decls: &[jl::Decl],
) -> Result<Vec<jl::Decl>, EncodeError> {
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
    // Bounded by the number of named defaulters, because every round but
    // the last adds at least one.
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
        let produced = julia_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    julia_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Julia default.
pub fn julia_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<jl::Decl>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let Some(value) = value_of(ast, id, &name)? else {
            continue;
        };
        emitted.push(name.clone());
        out.push(jl::Decl::FunctionDef(jl::FunctionDef {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it. An aggregate answers the mutable scratch zd_freeze builds \
                 the immutable value from.",
            ),
            name: Ident::new(&name),
            params: Vec::new(),
            returns: None,
            body: vec![jl::Statement::Return(Some(value))],
        }));
    }
    Ok(out)
}

/// The value one defaulter answers with, or `None` where the type has no
/// Julia data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<jl::Expr>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name, so an arm answering with a call
    // would name this very function.
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
    let value = match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),

        // `""`, for both widths: a Julia `String` holds code points and
        // the empty string is the same string in either.
        TypeKind::String { .. } => text(""),

        // A `fixed<P,S>` is the decimal it was written as, and the
        // canonical form accepts exactly one spelling of zero for a
        // given scale.
        TypeKind::Fixed { scale, .. } => text(&zero_decimal(*scale)),

        // Only an alias `resolve_alias` could not follow reaches here.
        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10. By name and not as
        // the number: a generated file that spelled the value would
        // keep working after `@value` moved it.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            Julia::enumerator_constant(ast, id, &enumerator)
        }

        // No bit set. Through the declared alias — `a_Flags(0)` — so
        // the scratch holds the bitmask's own width.
        TypeKind::Bitmask { .. } => {
            jl::Expr::convert(jl::Type::named(&julia_type_name(ast, id)), jl::Expr::int(0))
        }

        // A bitset's fields are separate integers in `adapter/julia.rs`
        // — a struct with one field per bitfield — so its default is
        // the scratch with every field at its holder's zero. The
        // packing into the anchor is the codec's.
        TypeKind::Bitset { .. } => {
            let mut pairs = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding: it moves the next
                // field along and holds no value.
                if let Some(field_name) = field_node.name.as_ref() {
                    pairs.push((
                        escape_field(field_name),
                        jl::Expr::convert(
                            Julia::unsigned_holder(field_node.bits),
                            jl::Expr::int(0),
                        ),
                    ));
                }
            }
            scratch_of(pairs)
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries. `Any[]` and not `nothing`: `nothing`
        // is what an `@optional` member's absence means here, and one
        // value cannot mean both.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => jl::Expr::AnyVector(Vec::new()),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." One call per element, one `Any[…]` per dimension:
        // `Any[…]` evaluates each element expression separately, so no
        // two slots share a reference.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let length = ast
                    .eval_integral(*dim)
                    .map_err(|_| refuse("array whose length does not evaluate"))?;
                lengths.push(
                    usize::try_from(length)
                        .map_err(|_| refuse("array of a length this column cannot spell"))?,
                );
            }
            let mut out = jl::Expr::free_call(&defaulter_name(ast, element), Vec::new());
            // Innermost dimension first, so the outer one ends up
            // outermost: `long a[2][3]` is two rows of three.
            for length in lengths.into_iter().rev() {
                out = jl::Expr::AnyVector(vec![out; length]);
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut pairs = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3); its slot holds
            // the base's own scratch, which `zd_freeze` freezes in turn.
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                pairs.push((
                    escape_field("parent"),
                    jl::Expr::free_call(&defaulter_name(ast, base), Vec::new()),
                ));
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                // Left out rather than filled with `nothing`: the
                // scratch answers `nothing` for an untouched slot, and
                // one answer per question.
                if member_node.facets.is_optional() {
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                pairs.push((
                    escape_field(&member_node.name),
                    member_value(ast, member_node, member_type)?,
                ));
            }
            scratch_of(pairs)
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." Where it selects none, `_d` alone is the whole
        // value: every case slot answers `nothing`.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut pairs = vec![(
                "_d".to_string(),
                discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                pairs.push((
                    field,
                    jl::Expr::free_call(&defaulter_name(ast, member_type), Vec::new()),
                ));
            }
            scratch_of(pairs)
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
    Ok(Some(value))
}

/// One member's default as a value: its `@default`, or a call to its
/// type's defaulter.
fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<jl::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(jl::Expr::free_call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, at the Julia holder's own width.
fn primitive_default(kind: PrimitiveKind) -> jl::Expr {
    let converted = |primitive: jl::Primitive| {
        jl::Expr::convert(jl::Type::Primitive(primitive), jl::Expr::int(0))
    };
    match kind {
        PrimitiveKind::Boolean => jl::Expr::Literal(Literal::Bool(false)),
        // The octet zero, in the code-unit holder the materialiser
        // declared: a `char` member is a `UInt8` and a `wchar` a
        // `UInt16`, and Julia has no separate character-of-one-octet
        // type to spell it in.
        PrimitiveKind::Char | PrimitiveKind::Octet | PrimitiveKind::UInt8 => {
            converted(jl::Primitive::UInt8)
        }
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => converted(jl::Primitive::UInt16),
        PrimitiveKind::Int8 => converted(jl::Primitive::Int8),
        PrimitiveKind::Int16 => converted(jl::Primitive::Int16),
        PrimitiveKind::Int32 => converted(jl::Primitive::Int32),
        PrimitiveKind::UInt32 => converted(jl::Primitive::UInt32),
        PrimitiveKind::Int64 => converted(jl::Primitive::Int64),
        PrimitiveKind::UInt64 => converted(jl::Primitive::UInt64),
        PrimitiveKind::Float32 => jl::Expr::convert(
            jl::Type::Primitive(jl::Primitive::Float32),
            jl::Expr::Literal(Literal::Float("0.0".to_string())),
        ),
        PrimitiveKind::Float64 => jl::Expr::Literal(Literal::Float("0.0".to_string())),
        // The decimal, because a `long double` member is a `String`
        // holding one: a binary128 zero written through a `Float64`
        // would be a different type, not a different spelling.
        PrimitiveKind::Float128 => text("0"),
    }
}

/// `zd_scratch_of("x", …, "y", …)` — the runtime's mutable scratch,
/// keyed by the escaped Julia field names `zd_freeze` reads back.
fn scratch_of(pairs: Vec<(String, jl::Expr)>) -> jl::Expr {
    let mut args = Vec::new();
    for (name, value) in pairs {
        args.push(text(&name));
        args.push(value);
    }
    jl::Expr::free_call("zd_scratch_of", args)
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
        .map(|(member, member_type)| (member_type, escape_field(&member.name))))
}

/// A discriminator value in the Julia form its type gives it, mirroring
/// `emit/julia.rs`'s `cast` at the labels: the enumerator by name, the
/// booleans as `true`/`false`, a character as its code-unit holder.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> jl::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return Julia::enumerator_constant(ast, resolved, &enumerator);
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => jl::Expr::Literal(Literal::Bool(value != 0)),
        Some(PrimitiveKind::Char) => jl::Expr::convert(
            jl::Type::Primitive(jl::Primitive::UInt8),
            jl::Expr::int(value),
        ),
        Some(PrimitiveKind::WChar) => jl::Expr::convert(
            jl::Type::Primitive(jl::Primitive::UInt16),
            jl::Expr::int(value),
        ),
        _ => jl::Expr::int(value),
    }
}

/// The primitive `id` is once typedefs are seen through, or `None`.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_ALIAS_DEPTH`)
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

/// An explicit `@default` as a Julia expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<jl::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = ast
        .expr(expr)
        .map_err(|_| refuse("a `@default` this frontend cannot follow"))?;
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    let is_float128 = matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128)));
    let code_unit = |code: u32, wide: bool| {
        jl::Expr::convert(
            jl::Type::Primitive(if wide {
                jl::Primitive::UInt16
            } else {
                jl::Primitive::UInt8
            }),
            jl::Expr::int(i128::from(code)),
        )
    };
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(jl::Expr::Literal(Literal::Bool(*flag)))
        }
        ConstExpr::Literal(AstLiteral::Char(ch)) => {
            let code = u32::from(*ch);
            if code > 0xFF {
                return Err(refuse("a `@default` character above one octet"));
            }
            Ok(code_unit(code, false))
        }
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(code_unit(*code, true)),
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
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            let written = format!("{mantissa}{exponent}");
            if is_float128 {
                // The member holds the decimal text, exactly.
                return Ok(text(&written));
            }
            Ok(jl::Expr::Literal(Literal::Float(written)))
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
            Ok(Julia::enumerator_constant(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    jl::Expr::Literal(Literal::Bool(value != 0))
                }
                // Refused rather than folded where the value has no
                // code unit: a `@default` the designer wrote and a
                // member that got zero instead are different samples.
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    let code = u32::try_from(value)
                        .ok()
                        .filter(|code| *code <= 0xFF)
                        .ok_or_else(|| refuse("a `@default` character above one octet"))?;
                    code_unit(code, false)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    let code = u32::try_from(value)
                        .ok()
                        .filter(|code| *code <= u32::from(u16::MAX))
                        .ok_or_else(|| {
                            refuse("a `@default` character outside the range of a UTF-16 code unit")
                        })?;
                    code_unit(code, true)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&format!("{value}")),
                _ => jl::Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &jl::Decl, out: &mut Vec<String>) {
    match decl {
        jl::Decl::FunctionDef(function) => calls_in_body(&function.body, out),
        jl::Decl::Const(constant) => calls_in_expr(&constant.value, out),
        jl::Decl::Enum(declaration) => {
            for member in &declaration.members {
                calls_in_expr(&member.value, out);
            }
        }
        jl::Decl::Abstract(_)
        | jl::Decl::Struct(_)
        | jl::Decl::Function(_)
        | jl::Decl::Comment(_)
        | jl::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[jl::Statement], out: &mut Vec<String>) {
    for statement in body {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &jl::Statement, out: &mut Vec<String>) {
    match statement {
        jl::Statement::Local { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        jl::Statement::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        jl::Statement::Expr(value) => calls_in_expr(value, out),
        jl::Statement::If {
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
        jl::Statement::For { iter, body, .. } => {
            calls_in_expr(iter, out);
            calls_in_body(body, out);
        }
        jl::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        jl::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        jl::Statement::Break | jl::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &jl::Expr, out: &mut Vec<String>) {
    match expr {
        jl::Expr::FreeCall { func, args } => {
            let name = func.as_str();
            if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == name) {
                out.push(name.to_string());
            }
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        jl::Expr::Call { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        jl::Expr::AnyVector(items) => {
            for item in items {
                calls_in_expr(item, out);
            }
        }
        jl::Expr::Field { value, .. } | jl::Expr::Unary { value, .. } | jl::Expr::Paren(value) => {
            calls_in_expr(value, out);
        }
        jl::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        jl::Expr::Range { start, stop } => {
            calls_in_expr(start, out);
            calls_in_expr(stop, out);
        }
        jl::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        jl::Expr::Literal(_) | jl::Expr::Name(_) | jl::Expr::Type(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> jl::Expr {
    jl::Expr::Literal(Literal::Str(value.to_string()))
}

// The names the emit backend spells and this module has to agree with.
const _: () = {
    // `escape_name` is what the enumerator constants go through, and it
    // is reached via `Julia::enumerator_constant`; stated so the import
    // is not "unused" the day the last direct call moves.
    let _ = escape_name;
};
