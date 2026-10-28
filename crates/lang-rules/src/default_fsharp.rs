// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value an F# member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as F# — deliberately a further derivation
//! and not a call into any sibling's, for the reason `default_zig.rs`'s
//! own documentation gives: the columns are compared against each other
//! and a shared helper would make them agree about a mistake.
//!
//! ## Two uses, and only one of them needs a generated function
//!
//! [`default_expr`] answers Table 9's value as one **expression**, and
//! `crates/lang-rules/src/adapter/fsharp.rs` calls it directly to give
//! every [`fsharp::ClassField::default`] its value at the moment a class
//! is declared — no function needed there, because the value is written
//! once, at the declaration, and never repeated.
//!
//! `crates/lang-rules/src/emit/fsharp.rs`'s [`crate::emit::Routine`]
//! carries no `Ast` and no `Id<TypeNode>` — only the already-materialised
//! [`fsharp::Type`] — so a decoder's own starting value cannot call
//! [`default_expr`] directly the way the adapter does; it needs a
//! **name** it can build a call to, exactly the shape
//! `default_zig.rs`'s `defaulter_from_decoder` answers for Zig and for
//! the same reason. [`defaulter_name`], [`defaulter_from_decoder`],
//! [`fsharp_defaulters_over`] and [`fsharp_defaulters_called_by`] are
//! that machinery, generating `let zd_def_X () : T = <Table 9's value>`
//! — a `unit -> T` function and not a plain value, because a plain
//! `let zd_def_X : T = Pose()` would construct **one** `Pose` at module
//! load and hand every caller the same mutable object; every one of
//! `crates/lang-rules/src/emit/fsharp.rs`'s three call sites —
//! [`crate::emit::CodecTarget::routine`]'s decode start,
//! [`crate::emit::CodecTarget::default_into`] and
//! [`crate::emit::CodecTarget::member_default`]'s implicit branch — call
//! `zd_def_X ()` and expect a fresh one each time.
//!
//! What makes this machinery shorter than `default_zig.rs`'s own copy of
//! it is the body each generated function answers with: for an
//! aggregate it is `T()`, one call, because
//! `crates/lang-cst/src/fsharp/mod.rs`'s module documentation is where
//! the design that makes `T()` already Table 9's value is argued — Zig
//! has to build a whole struct literal there and this column does not.

use crate::adapter::fsharp::{field_name, fsharp_type_name};
use crate::default_common::{discriminator_default_value, resolve_alias};
use crate::emit::default_enumerator_name;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::fsharp;

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// DDS-XTypes 1.3 Table 9's value for `id`, as an F# expression.
///
/// # Errors
/// [`EncodeError::NoWriter`] where the type has no F# default this
/// column can spell — `any`, `ValueBase`, a boxed value, or one whose
/// element or member type does not resolve.
///
/// zerodds-lint: recursion-depth 64 (aggregate member and alias chains; bounded by `MAX_ALIAS_DEPTH`)
pub fn default_expr(ast: &Ast, id: Id<TypeNode>) -> Result<fsharp::Expr, EncodeError> {
    default_expr_at(ast, id, 0)
}

/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_ALIAS_DEPTH`, which returns `EncodeError::NoWriter`
/// naming the type that nested)
fn default_expr_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<fsharp::Expr, EncodeError> {
    if depth > MAX_ALIAS_DEPTH {
        return Err(EncodeError::NoWriter {
            member: fsharp_type_name(ast, id),
            kind: "type nested past the recursion bound",
        });
    }
    let id = resolve_alias(ast, id);
    let name = fsharp_type_name(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: name.clone(),
        kind,
    };
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("type that does not resolve"));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),

        // `""`, for both widths: the member holds text either way, and
        // .NET's `string` needs no separate encoding to state one.
        TypeKind::String { .. } => text(""),

        // The one decimal spelling of zero for the scale.
        TypeKind::Fixed { scale, .. } => text(&zero_decimal(*scale)),

        TypeKind::Alias { .. } => return Err(refuse("typedef with an unresolved target")),

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10.
        TypeKind::Enumeration { .. } => {
            let enumerator = default_enumerator_name(ast, id)
                .ok_or_else(|| refuse("enumeration with no enumerator to take as its default"))?;
            enumerator_expr(ast, id, &enumerator)
        }

        // No bit set. `typed_int` wraps the zero in the bitmask's own
        // holder type — `crates/lang-rules/src/adapter/fsharp.rs`'s
        // documentation of `typed_int` carries why a bare `0` does not
        // compile against anything narrower or wider than `int32`.
        TypeKind::Bitmask { .. } => {
            let bound = ast.effective_bit_bound(id);
            crate::adapter::fsharp::typed_int(
                crate::adapter::fsharp::fsharp_holder(bound),
                fsharp::Expr::int(0),
            )
        }

        // A bitset is a class of separate holders; every field already
        // defaults to zero at its own declaration, so `T()` is exactly
        // Table 9's answer.
        TypeKind::Bitset { .. } => fsharp::Expr::new(&name, Vec::new()),

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => fsharp::Expr::ArrayLit(Vec::new()),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." Nested array literals, outermost dimension outermost.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| refuse("array with an unresolved element"))?;
            let mut lengths = Vec::new();
            for dim in dims {
                let length = ast
                    .eval_integral(*dim)
                    .map_err(|_| refuse("array whose length does not evaluate"))?;
                let length =
                    usize::try_from(length).map_err(|_| refuse("array of a negative length"))?;
                lengths.push(length);
            }
            let inner = default_expr_at(ast, element, depth + 1)?;
            let mut out = inner;
            for length in lengths.into_iter().rev() {
                out = fsharp::Expr::ArrayLit(core::iter::repeat_n(out.clone(), length).collect());
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." Every field of
        // the class already carries exactly this at its own
        // declaration (`crates/lang-rules/src/adapter/fsharp.rs` built
        // it the same way), so the type's own constructor answers.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            fsharp::Expr::new(&name, Vec::new())
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." Every branch field already carries its own
        // type's Table 9 value at the class's own declaration — the
        // module documentation of `crates/lang-cst/src/fsharp/mod.rs`
        // says why an unselected one is not left undefined — so the
        // constructor is Table 9's whole answer here too.
        TypeKind::Union { .. } => fsharp::Expr::new(&name, Vec::new()),

        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Err(refuse("construct with no F# default value"));
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
    })
}

/// A union discriminator's own Table 9 default, in the discriminator's
/// F# form: the enumerator for an enumeration, the boolean literal for
/// a `boolean`, and the number for everything else — a `char` and a
/// `wchar` discriminator hold their code unit as an integer here, as
/// `default_zig.rs`'s `discriminant_literal` does for Zig.
///
/// # Errors
/// [`EncodeError`] where the discriminator's own type has no default.
pub fn discriminator_default(
    ast: &Ast,
    discriminator: Id<TypeNode>,
) -> Result<fsharp::Expr, EncodeError> {
    let resolved = resolve_alias(ast, discriminator);
    let is_enum = matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    );
    if is_enum {
        if let Some(enumerator) = default_enumerator_name(ast, resolved) {
            return Ok(enumerator_expr(ast, resolved, &enumerator));
        }
    }
    match discriminator_default_value(ast, resolved) {
        Some(value) if resolved_primitive(ast, resolved) == Some(PrimitiveKind::Boolean) => {
            Ok(fsharp::Expr::Literal(Literal::Bool(value != 0)))
        }
        Some(value) => Ok(match resolved_primitive(ast, resolved) {
            Some(kind) => match crate::adapter::fsharp::fsharp_primitive_type(kind) {
                fsharp::Type::Primitive(primitive) => {
                    crate::adapter::fsharp::typed_int(primitive, fsharp::Expr::int(value))
                }
                _ => fsharp::Expr::int(value),
            },
            None => fsharp::Expr::int(value),
        }),
        None => default_expr(ast, resolved),
    }
}

/// One member's default: its `@default`, or its type's Table 9 value.
/// §7.2.2.4.4.4.9 puts the explicit one first.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell — refused rather than silently replaced by
/// Table 9's zero, which is a different sample from the one the
/// designer declared.
pub fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<fsharp::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        // `zd_def_T ()`, not the value inlined: the module documentation
        // says why the read direction needs the named form here, and
        // `crates/lang-rules/src/adapter/fsharp.rs` is the caller that
        // wants the plain value instead, so it calls `default_expr`
        // directly and never reaches this arm.
        None => Ok(fsharp::Expr::call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// The defaulter function name for a type, keyed the way the decoder is:
/// by the IDL kind for a primitive and by the codec tag for everything
/// else — `default_zig.rs`'s `defaulter_name` read onto this column's
/// tag function.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", crate::emit::fsharp::type_tag(ast, id, 0)),
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

/// Build the defaulter for each of `types`, once per name: `let
/// zd_def_X () : T = <Table 9's value>`, a `unit -> T` function so every
/// caller gets a fresh, unaliased value — the module documentation says
/// why a plain value would be wrong for a mutable class.
///
/// # Errors
/// [`EncodeError`] for a construct with no F# default.
pub fn fsharp_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<fsharp::Decl>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let returns = fsharp_value_type_or(ast, id, &name)?;
        let value = default_expr(ast, id)?;
        emitted.push(name.clone());
        out.push(fsharp::Decl::Let(fsharp::LetDecl {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it. `unit -> T` so every call site gets a value of its own.",
            ),
            name: Ident::new(&name),
            params: vec![fsharp::FnParam {
                name: Ident::new("_zd_unit"),
                type_: fsharp::Type::Primitive(fsharp::Primitive::Unit),
            }],
            type_: Some(returns),
            body: vec![fsharp::Stmt::Return(value)],
        }));
    }
    Ok(out)
}

/// The F# type [`fsharp_defaulters_over`] declares a defaulter's result
/// as: `crate::adapter::fsharp::fsharp_value_type`, folded into an
/// [`EncodeError`] where it refuses.
fn fsharp_value_type_or(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
) -> Result<fsharp::Type, EncodeError> {
    crate::adapter::fsharp::fsharp_value_type(ast, resolve_alias(ast, id), at).map_err(|_| {
        EncodeError::NoWriter {
            member: at.to_string(),
            kind: "type with no F# form to return",
        }
    })
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model, the
/// same reachability walk `default_zig.rs`'s
/// `zig_defaulters_called_by` runs.
///
/// # Errors
/// [`EncodeError`] for a construct with no F# default.
pub fn fsharp_defaulters_called_by(
    ast: &Ast,
    decls: &[fsharp::Decl],
) -> Result<Vec<fsharp::Decl>, EncodeError> {
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
        let produced = fsharp_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    fsharp_defaulters_over(ast, &wanted)
}

fn calls_in_decl(decl: &fsharp::Decl, out: &mut Vec<String>) {
    if let fsharp::Decl::Let(let_decl) = decl {
        calls_in_body(&let_decl.body, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[fsharp::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stmt(statement: &fsharp::Stmt, out: &mut Vec<String>) {
    match statement {
        fsharp::Stmt::Let { value, .. } => calls_in_expr(value, out),
        fsharp::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        fsharp::Stmt::Expr(value) | fsharp::Stmt::Return(value) => calls_in_expr(value, out),
        fsharp::Stmt::If {
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
        fsharp::Stmt::CountUp { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        fsharp::Stmt::Until { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        fsharp::Stmt::Guarded { body } => calls_in_body(body, out),
        fsharp::Stmt::Leave | fsharp::Stmt::GiveBack | fsharp::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &fsharp::Expr, out: &mut Vec<String>) {
    match expr {
        fsharp::Expr::Call { func, args } | fsharp::Expr::CurriedCall { func, args } => {
            if let fsharp::Expr::Name(name) = func.as_ref() {
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
        fsharp::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        fsharp::Expr::Paren(value) | fsharp::Expr::Unary { value, .. } => {
            calls_in_expr(value, out);
        }
        fsharp::Expr::Dot { value, .. } => calls_in_expr(value, out),
        fsharp::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        fsharp::Expr::Some_(value) => calls_in_expr(value, out),
        fsharp::Expr::New { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        fsharp::Expr::ObjectInit { fields, .. } => {
            for (_, value) in fields {
                calls_in_expr(value, out);
            }
        }
        fsharp::Expr::ArrayLit(elements) => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        fsharp::Expr::ArrayZeroCreate { count, .. } => calls_in_expr(count, out),
        fsharp::Expr::Literal(_) | fsharp::Expr::Name(_) | fsharp::Expr::None_ => {}
    }
}

/// Table 9's primitive rows, as the F# holder each takes. The integers
/// and both character widths are integer zeros — the member holds the
/// code unit — and the two float widths are `0.0`. `long double` is
/// `"0"`, this column's decimal text.
fn primitive_default(kind: PrimitiveKind) -> fsharp::Expr {
    match kind {
        PrimitiveKind::Boolean => fsharp::Expr::Literal(Literal::Bool(false)),
        // `float64`'s bare `0.0` is already `float` — F#'s default
        // literal width for a decimal — but `float32` needs the same
        // conversion-function wrapping every narrower integer needs:
        // measured (`fsi`, .NET SDK 9.0.106), a bare `0.0` ascribed to
        // `float32` is `error FS0001`, "This expression was expected to
        // have type 'float32' … 'float'".
        PrimitiveKind::Float64 => fsharp::Expr::Literal(Literal::Float("0.0".to_string())),
        PrimitiveKind::Float32 => fsharp::Expr::call(
            "float32",
            vec![fsharp::Expr::Literal(Literal::Float("0.0".to_string()))],
        ),
        PrimitiveKind::Float128 => text("0"),
        // Every integer width: `typed_int` wraps in the conversion
        // function the target width needs — `crates/lang-rules/src/
        // adapter/fsharp.rs`'s `typed_int` documentation carries the
        // measurement that a bare `0` under a `byte` (or narrower or
        // wider) ascription does not compile.
        other => match crate::adapter::fsharp::fsharp_primitive_type(other) {
            fsharp::Type::Primitive(primitive) => {
                crate::adapter::fsharp::typed_int(primitive, fsharp::Expr::int(0))
            }
            _ => fsharp::Expr::int(0),
        },
    }
}

/// An enumerator, in the enum's own scope: `Mode.IDLE`.
fn enumerator_expr(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> fsharp::Expr {
    fsharp::Expr::dot(
        fsharp::Expr::Name(Ident::new(&fsharp_type_name(ast, id))),
        &field_name(enumerator),
    )
}

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

/// An explicit `@default` as an F# expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<fsharp::Expr, EncodeError> {
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
            Ok(fsharp::Expr::Literal(Literal::Bool(*flag)))
        }
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(crate::adapter::fsharp::typed_int(
            fsharp::Primitive::Byte,
            fsharp::Expr::int(i128::from(u32::from(*ch))),
        )),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(crate::adapter::fsharp::typed_int(
            fsharp::Primitive::UInt16,
            fsharp::Expr::int(i128::from(*code)),
        )),
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(text(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let ch = char::from_u32(*code)
                    .ok_or_else(|| refuse("a `@default` wide string that is not code points"))?;
                out.push(ch);
            }
            Ok(text(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = exponent
                .as_ref()
                .map_or_else(String::new, |exponent| format!("e{exponent}"));
            let literal = fsharp::Expr::Literal(Literal::Float(format!("{mantissa}{exponent}")));
            // `float32`'s bare decimal is `float` (F#'s default), same
            // as every narrower integer's bare decimal is `int32` —
            // `primitive_default`'s own documentation carries the
            // measurement.
            if matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float32))) {
                Ok(fsharp::Expr::call("float32", vec![literal]))
            } else {
                Ok(literal)
            }
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
            Ok(enumerator_expr(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    fsharp::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    crate::adapter::fsharp::typed_int(
                        fsharp::Primitive::Byte,
                        fsharp::Expr::int(value),
                    )
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    crate::adapter::fsharp::typed_int(
                        fsharp::Primitive::UInt16,
                        fsharp::Expr::int(value),
                    )
                }
                // Every other integer width: `typed_int` reads the
                // primitive off the resolved type and wraps
                // accordingly, or answers the value unwrapped where
                // `kind` is not a bare primitive at all — `int32` and a
                // named alias both take the bare literal, the first
                // because F# gives it one and the second because a
                // `let`-bound bitmask constant is ascribed to the
                // abbreviation's own name and not to this call's
                // result directly.
                Ok(TypeKind::Primitive(primitive)) => {
                    match crate::adapter::fsharp::fsharp_primitive_type(primitive) {
                        fsharp::Type::Primitive(target) => {
                            crate::adapter::fsharp::typed_int(target, fsharp::Expr::int(value))
                        }
                        _ => fsharp::Expr::int(value),
                    }
                }
                _ => fsharp::Expr::int(value),
            })
        }
    }
}

fn text(value: &str) -> fsharp::Expr {
    fsharp::Expr::Literal(Literal::Str(value.to_string()))
}
