// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a generated Rust member takes when the stream does not
//! carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as a Rust expression. The reflective
//! column's copy of the same table is
//! `zerodds_wire_codec::default` (re-exported by `crates/wire-probe/src/default.rs`), which carries the clause
//! quotations and the reasoning for the kinds Table 9 does not list.
//! Deliberately a third derivation and not a call into the first: the
//! columns are compared against each other, and a shared helper would
//! make them agree about a mistake.
//!
//! ## Why an expression and not `Default::default()`
//!
//! The types this column materialises derive nothing —
//! `crate::adapter::cst` sets `derives: Vec::new()` on every struct and
//! enum it emits, because a derive is a decision about the target
//! language and not about the wire. So the default is written out: a
//! struct literal with a value per member, an enum's first variant, an
//! empty `Vec`. `Default` would also be the *language's* zero and not
//! Table 9's, and the two part company at an enumeration whose first
//! literal has a `@value` other than zero.

use crate::default_common::{discriminator_default_value, resolve_alias};
use crate::encode::EncodeError;
use crate::fixed_literal::fixed_text;
use zerodds_ast::{
    Ast, ConstExpr, ConstValue, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind,
    TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Ident, Literal, Path};
use zerodds_lang_cst::rust;

/// How deep a default may nest before the model is taken to be cyclic.
const MAX_DEPTH: usize = 64;

/// One member's default: its `@default`, or its type's.
///
/// §7.2.2.4.4.4.9 puts the explicit one first — "An explicit definition
/// associates a default value with the member at the time where the
/// aggregated type is defined" — and falls back to "the default value of
/// the member type. See Table 9."
///
/// # Errors
/// [`EncodeError::NoWriter`] for a type or a `@default` this column
/// cannot spell. Refused rather than replaced by Table 9's zero: a
/// member the designer gave a default and a member that got a zero
/// instead are different samples.
pub fn member_default(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<rust::Expr, EncodeError> {
    // §7.4.1.2.3: "if the receiver's member is not optional, it
    // logically takes its default value" — so an `@optional` one does
    // not, and the absence is the value. `None`, not Table 9's zero:
    // the two are different samples, and a reader that substituted the
    // zero would report a member the writer deliberately left out.
    if node.facets.is_optional() {
        return Ok(rust::Expr::Path(Path::single("None")));
    }
    let value = member_value_default(ast, from, node, type_)?;
    Ok(crate::adapter::cst::external_value(ast, node, value))
}

/// Default of the present value, before optional presence or external
/// storage wraps it. Bound handling operates on that value, not its holder.
pub(crate) fn member_value_default(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<rust::Expr, EncodeError> {
    if let Some(mapping) = crate::adapter::cst::member_rust_type_mapping(ast, node) {
        return Ok(mapped_default(
            &mapping,
            idl_default_value(ast, Some(node), resolve_alias(ast, type_))?,
        ));
    }
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, from, expr, type_, &node.name),
        None => default_of(ast, from, type_, &node.name),
    }
}

/// Table 9's default for a type.
///
/// # Errors
/// As [`member_default`].
pub fn default_of(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    type_: Id<TypeNode>,
    at: &str,
) -> Result<rust::Expr, EncodeError> {
    at_depth(ast, from, type_, at, 0)
}

/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_DEPTH`, which returns `EncodeError::NoWriter`
/// naming the member whose default nested)
#[allow(clippy::too_many_lines)]
fn at_depth(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    type_: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<rust::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: at.to_string(),
        kind,
    };
    if depth > MAX_DEPTH {
        return Err(refuse(
            "type whose default nests deeper than an acyclic model can",
        ));
    }
    let node = ast.type_(type_).map_err(|error| EncodeError::NoLayout {
        detail: format!("{error}"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),
        // Table 9's `STRING_TYPE` row, for both widths.
        TypeKind::String { .. } => call_path(&["String", "new"]),
        // `fixed<P,S>` has no Table 9 row and is a decimal string in
        // this column; zero at the declared scale is the one spelling
        // the canonical form accepts.
        TypeKind::Fixed { scale, .. } => {
            let text = if *scale == 0 {
                String::from("0")
            } else {
                format!("0.{}", "0".repeat(usize::from(*scale)))
            };
            rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Literal(Literal::string(&text))),
                method: Ident::new("to_string"),
                args: Vec::new(),
            }
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| refuse("unresolved typedef"))?;
            // A typedef named in `zerodds_build::Config::rust_type`: Table
            // 9's value of the IDL type behind it, converted.
            match crate::adapter::cst::rust_type_mapping(node.node_id) {
                Some(mapping) => mapped_default(
                    &mapping,
                    idl_default_value(ast, None, resolve_alias(ast, target))?,
                ),
                None => at_depth(ast, from, target, at, depth + 1)?,
            }
        }
        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10.
        TypeKind::Enumeration { .. } => {
            let variant = default_enumerator(ast, type_)
                .ok_or_else(|| refuse("enumeration with no enumerator to take as its default"))?;
            // The enumerator is named from the module this expression
            // lands in, not from the enumeration's own.
            let path = crate::adapter::cst::relative_path(ast, from, node)
                .map_err(|_| refuse("an enumeration the model has no module for"))?;
            let mut segments: Vec<String> = path
                .segments()
                .iter()
                .map(|s| s.as_str().to_string())
                .collect();
            segments.push(variant.clone());
            let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
            rust::Expr::Path(Path::new(&borrowed))
        }
        // No bit set: the holder's Table 9 default is zero, and zero
        // sets none. A bitmask is its holder integer in this column.
        TypeKind::Bitmask { .. } => rust::Expr::Literal(Literal::int(0)),
        // A bitset has no Table 9 line of its own; §7.4.1.1.4 makes it
        // the aggregate of its bitfields, and a bitfield holds an
        // unsigned integer whose Table 9 default is zero. Written out
        // field by field rather than answered with a `Default::default`
        // call, because the struct's `impl Default` is built *from* this
        // expression and a call to it here would be a cycle.
        //
        // Padding is left out: an anonymous bitfield has no field in the
        // materialised struct to name.
        TypeKind::Bitset { .. } => {
            let mut fields = Vec::new();
            for field in &ast.all_bitfields_of(type_) {
                // A stale handle is refused: a struct literal missing
                // one field does not compile, which is loud, but the
                // refusal names the construct instead.
                let field_node = ast
                    .bitfield(*field)
                    .map_err(|_| refuse("bitset with a stale bitfield handle"))?;
                // An anonymous bitfield is padding and has no field in
                // the materialised struct to name — the one omission
                // here that is the mapping and not a loss.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                fields.push((
                    Ident::new(&crate::rust_names::of_type_id(ast, type_).field(field_name)),
                    rust::Expr::Literal(Literal::int(0)),
                ));
            }
            rust::Expr::StructLit {
                path: crate::adapter::cst::relative_path(ast, from, node)
                    .map_err(|_| refuse("a bitset the model has no module for"))?,
                fields,
                base: None,
            }
        }
        // "A zero-length sequence of the same element type".
        TypeKind::Sequence { .. } => call_path(&["Vec", "new"]),
        // "An empty map of the same key and value types" — and an empty
        // `BTreeMap`, which is the container `adapter::cst` materialises
        // a `map` into. `Vec::new()` was written here while nothing
        // emitted a map member, so no generated file ever carried the
        // mismatch; the first one that did would have failed at `rustc`
        // with `expected BTreeMap, found Vec`.
        TypeKind::Map { .. } => match crate::adapter::cst::rust_map_form() {
            crate::adapter::cst::RustMapForm::BTreeMap => call_path(&["BTreeMap", "new"]),
            crate::adapter::cst::RustMapForm::Entries => call_path(&["Vec", "new"]),
        },
        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding type."
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| refuse("unresolved array element"))?;
            let mut expression = at_depth(ast, from, element, at, depth + 1)?;
            // `[value; N]` is a repeat expression and Rust requires the
            // element to be `Copy` for it. The materialised types derive
            // `Debug, Clone, PartialEq` and not `Copy`, so an array of
            // structs — or of sequences, where the element is a `Vec` —
            // has to be written out element by element. A primitive is
            // `Copy` and keeps the short form, which is every array in
            // the trees this emitter had been pointed at before an
            // `impl Default` asked for the rest.
            let copyable = matches!(
                ast.type_(element).ok().map(|node| &node.kind),
                Some(TypeKind::Primitive(_))
            );
            for dim in dims.iter().rev() {
                let count = ast
                    .eval_integral(*dim)
                    .map_err(|_| refuse("array whose length does not evaluate"))?;
                expression = if copyable {
                    rust::Expr::Repeat {
                        value: Box::new(expression),
                        count: Box::new(rust::Expr::Literal(Literal::int(count))),
                    }
                } else {
                    let count = usize::try_from(count)
                        .map_err(|_| refuse("array whose length does not fit a usize"))?;
                    rust::Expr::Array(vec![expression.clone(); count])
                };
            }
            expression
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut fields = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, type_) {
                fields.push((
                    Ident::new(crate::adapter::cst::PARENT),
                    at_depth(ast, from, base, crate::adapter::cst::PARENT, depth + 1)?,
                ));
            }
            for member in ast.members_of(type_) {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                let member_type = member_node
                    .type_
                    .resolved()
                    .ok_or_else(|| refuse("member with an unresolved type"))?;
                // §7.4.1.2.3: an `@optional` member's default is the
                // absence and not Table 9's zero — the field is an
                // `Option<T>` and takes `None`. Reading the member's own
                // default here rather than its type's is what carries
                // that: going straight to the type produced `flag: 0`
                // for an `Option<u8>` field, which does not compile.
                let value = if member_node.facets.is_optional() {
                    rust::Expr::Path(Path::single("None"))
                } else if let Some(mapping) =
                    crate::adapter::cst::member_rust_type_mapping(ast, member_node)
                {
                    // Its own `@default` or Table 9's, of the IDL type, converted.
                    crate::adapter::cst::external_value(
                        ast,
                        member_node,
                        mapped_default(
                            &mapping,
                            idl_default_value(
                                ast,
                                Some(member_node),
                                resolve_alias(ast, member_type),
                            )?,
                        ),
                    )
                } else {
                    let value = match member_node
                        .facets
                        .xtypes
                        .as_ref()
                        .and_then(|x| x.default_value)
                    {
                        Some(expr) => explicit(ast, from, expr, member_type, &member_node.name)?,
                        None => at_depth(ast, from, member_type, &member_node.name, depth + 1)?,
                    };
                    crate::adapter::cst::external_value(ast, member_node, value)
                };
                fields.push((
                    Ident::new(&crate::rust_names::of_type(ast, node).field(&member_node.name)),
                    value,
                ));
            }
            rust::Expr::StructLit {
                path: crate::adapter::cst::relative_path(ast, from, node)
                    .map_err(|_| refuse("a struct the model has no module for"))?,
                fields,
                base: None,
            }
        }
        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type."
        //
        // The variant that branch is, with the discriminator ahead of
        // the member where the variant carries it; the variant that
        // holds the discriminator alone where it selects no branch
        // (`crate::rust_union`).
        TypeKind::Union { .. } => {
            let shape = crate::rust_union::shape(ast, type_).map_err(refuse)?;
            let selector = discriminator_default_value(ast, shape.discriminator)
                .ok_or_else(|| refuse("union whose discriminator has no default value"))?;
            let discriminator_value =
                crate::rust_union::label_literal(ast, from, shape.kind, selector)
                    .map_err(refuse)?
                    .expr();
            let path = crate::adapter::cst::relative_path(ast, from, node)
                .map_err(|_| refuse("a union the model has no module for"))?;
            let mut segments: Vec<String> = path
                .segments()
                .iter()
                .map(|s| s.as_str().to_string())
                .collect();
            let selected = shape
                .cases
                .iter()
                .find(|case| case.labels.contains(&selector))
                .or_else(|| shape.cases.iter().find(|case| case.is_default));
            let args = match selected {
                Some(case) => {
                    let member = case.member;
                    let member_type = member
                        .type_
                        .resolved()
                        .ok_or_else(|| refuse("union case with an unresolved type"))?;
                    segments.push(case.variant.clone());
                    let mut args = Vec::new();
                    if case.carries_discriminator() {
                        args.push(discriminator_value);
                    }
                    // A branch named in `zerodds_build::Config::rust_type`
                    // takes Table 9's value of its IDL type, converted.
                    let value = match crate::adapter::cst::rust_type_mapping(member.node_id) {
                        Some(mapping) => mapped_default(
                            &mapping,
                            idl_default_value(ast, None, resolve_alias(ast, member_type))?,
                        ),
                        None => at_depth(ast, from, member_type, &member.name, depth + 1)?,
                    };
                    args.push(crate::adapter::cst::external_value(ast, member, value));
                    args
                }
                None => {
                    segments.push(crate::rust_union::NO_MEMBER.to_string());
                    vec![discriminator_value]
                }
            };
            let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
            rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::new(&borrowed))),
                args,
            }
        }
        other => {
            let _ = other;
            return Err(refuse("type with no default value in this column"));
        }
    })
}

/// Table 9's primitive rows as Rust expressions.
fn primitive_default(kind: PrimitiveKind) -> rust::Expr {
    match kind {
        PrimitiveKind::Boolean => rust::Expr::Literal(Literal::Bool(false)),
        // `'\0'`, which is what a `char` field holds.
        PrimitiveKind::Char | PrimitiveKind::WChar => rust::Expr::Literal(Literal::Char('\0')),
        PrimitiveKind::Float32 => rust::Expr::Literal(Literal::Float(String::from("0.0f32"))),
        PrimitiveKind::Float64 => rust::Expr::Literal(Literal::Float(String::from("0.0f64"))),
        // By its crate, as the field's type is written: the generated file
        // imports no `Float128`.
        PrimitiveKind::Float128 => {
            rust::Expr::Path(Path::new(&["zerodds_float128", "Float128", "ZERO"]))
        }
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => rust::Expr::Literal(Literal::int(0)),
    }
}

/// The enumerator a default takes, as its Rust variant name.
fn default_enumerator(ast: &Ast, type_: Id<TypeNode>) -> Option<String> {
    let enumeration = ast.type_(type_).ok()?;
    let mut first = None;
    for enumerator in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*enumerator) else {
            continue;
        };
        let variant = crate::adapter::cst::enumerator_ident(ast, enumeration, &node.name);
        if node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_default_literal)
        {
            return Some(variant);
        }
        if first.is_none() {
            first = Some(variant);
        }
    }
    first
}

/// An explicit `@default` as a Rust expression of the member's type.
fn explicit(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<rust::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = ast
        .expr(expr)
        .map_err(|_| refuse("a `@default` this frontend cannot follow"))?;
    let resolved = resolve_alias(ast, type_);
    Ok(match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => rust::Expr::Literal(Literal::Bool(*flag)),
        ConstExpr::Literal(AstLiteral::Char(ch)) => rust::Expr::Literal(Literal::Char(*ch)),
        ConstExpr::Literal(AstLiteral::WChar(code)) => {
            let ch = char::from_u32(*code)
                .ok_or_else(|| refuse("a `@default` wide character that is not a code point"))?;
            rust::Expr::Literal(Literal::Char(ch))
        }
        ConstExpr::Literal(AstLiteral::String(text)) => rust::Expr::MethodCall {
            receiver: Box::new(rust::Expr::Literal(Literal::string(text))),
            method: Ident::new("to_string"),
            args: Vec::new(),
        },
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut text = String::new();
            for code in codes {
                let ch = char::from_u32(*code)
                    .ok_or_else(|| refuse("a `@default` wide string that is not code points"))?;
                text.push(ch);
            }
            rust::Expr::MethodCall {
                receiver: Box::new(rust::Expr::Literal(Literal::string(&text))),
                method: Ident::new("to_string"),
                args: Vec::new(),
            }
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let text = match exponent {
                Some(exponent) => format!("{mantissa}e{exponent}"),
                None => mantissa.clone(),
            };
            numeric(ast, resolved, &Number::Float(text), member_name)?
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => numeric(
            ast,
            resolved,
            &Number::Fixed {
                digits: digits.clone(),
                scale: *scale,
            },
            member_name,
        )?,
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let target = reference
                .resolved()
                .ok_or_else(|| refuse("a `@default` enumerator that is unresolved"))?;
            // The enumeration's path from the module the value is written
            // in: the bare type name did not resolve where the enumeration
            // and the member's type sit in a module and the `Default` impl
            // in another.
            rust::Expr::Path(
                crate::rust_union::enumerator_path_of(ast, from, resolved, target)
                    .map_err(|_| refuse("a `@default` enumerator this frontend cannot follow"))?,
            )
        }
        // An integer literal, a named constant or arithmetic over them,
        // folded the way a bound or a case label is: `eval_const`, so that
        // a constant of a floating-point or fixed-point type is a value
        // too (`const float CF = 2.5; @default(CF) float f;` was refused
        // as a `@default` this frontend cannot evaluate).
        _ => {
            let value = ast
                .eval_const(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            match value {
                ConstValue::Integer(value) => {
                    numeric(ast, resolved, &Number::Integer(value), member_name)?
                }
                ConstValue::Float { mantissa, exponent } => {
                    let text = match exponent {
                        Some(exponent) => format!("{mantissa}e{exponent}"),
                        None => mantissa,
                    };
                    numeric(ast, resolved, &Number::Float(text), member_name)?
                }
                ConstValue::Fixed { digits, scale } => {
                    numeric(ast, resolved, &Number::Fixed { digits, scale }, member_name)?
                }
                ConstValue::Boolean(flag) => rust::Expr::Literal(Literal::Bool(flag)),
                ConstValue::Char(ch) => rust::Expr::Literal(Literal::Char(ch)),
                _ => {
                    return Err(refuse(
                        "a `@default` constant of a kind this frontend cannot spell",
                    ));
                }
            }
        }
    })
}

/// A numeric `@default`, as the evaluator gives it.
enum Number {
    /// An integer.
    Integer(i128),
    /// A floating-point literal, as written.
    Float(String),
    /// A fixed-point literal: its digits without the point, and how many
    /// of them follow it.
    Fixed { digits: String, scale: u8 },
}

/// A numeric `@default` written for the member's type.
///
/// The value is one thing and the field's type another: `@default(1)` on a
/// `float` is `1.0f32`, not `1`, which rustc refuses (zero-objects/zero-dds#64);
/// on a `long double` it is the binary128 of the decimal, computed here
/// and written as its bits, because `zerodds_float128::Float128` has no
/// literal; on a `fixed<P,S>` it is the decimal at the member's scale.
fn numeric(
    ast: &Ast,
    resolved: Id<TypeNode>,
    value: &Number,
    member_name: &str,
) -> Result<rust::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let member_kind = ast
        .type_(resolved)
        .map(|node| &node.kind)
        .map_err(|_| refuse("a `@default` whose member type is stale"))?;
    Ok(match (member_kind, value) {
        (TypeKind::Primitive(PrimitiveKind::Float32), Number::Integer(value)) => {
            rust::Expr::Literal(Literal::Float(format!("{value}.0f32")))
        }
        (TypeKind::Primitive(PrimitiveKind::Float64), Number::Integer(value)) => {
            rust::Expr::Literal(Literal::Float(format!("{value}.0f64")))
        }
        (
            TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64),
            Number::Float(text),
        ) => rust::Expr::Literal(Literal::Float(text.clone())),
        (
            TypeKind::Primitive(PrimitiveKind::Float128),
            Number::Integer(_) | Number::Float(_) | Number::Fixed { .. },
        ) => {
            let text = match value {
                Number::Integer(value) => format!("{value}"),
                Number::Float(text) => text.clone(),
                Number::Fixed { digits, scale } => fixed_text(digits, *scale),
            };
            let quad: zerodds_float128::Float128 = text
                .parse()
                .map_err(|_| refuse("a `@default` a long double cannot hold"))?;
            rust::Expr::Call {
                callee: Box::new(rust::Expr::Path(Path::new(&[
                    "zerodds_float128",
                    "Float128",
                    "from_bits",
                ]))),
                args: vec![rust::Expr::Literal(Literal::Int(format!(
                    "0x{:032x}",
                    quad.to_bits()
                )))],
            }
        }
        (
            TypeKind::Fixed {
                scale: member_scale,
                ..
            },
            Number::Integer(value),
        ) => fixed_default(format!("{value}"), 0, *member_scale).map_err(refuse)?,
        (
            TypeKind::Fixed {
                scale: member_scale,
                ..
            },
            Number::Fixed { digits, scale },
        ) => fixed_default(digits.clone(), *scale, *member_scale).map_err(refuse)?,
        (_, Number::Integer(value)) => rust::Expr::Literal(Literal::int(*value)),
        _ => {
            return Err(refuse(
                "a `@default` whose value the member's type does not take",
            ));
        }
    })
}

/// A fixed-point `@default` at the member's scale: `1` on `fixed<5,2>` is
/// `"1.00"`, the one spelling the canonical form accepts. Digits past the
/// member's scale are refused rather than dropped.
fn fixed_default(
    mut digits: String,
    scale: u8,
    member_scale: u8,
) -> Result<rust::Expr, &'static str> {
    if scale < member_scale {
        digits.push_str(&"0".repeat(usize::from(member_scale - scale)));
    } else if scale > member_scale {
        let cut = usize::from(scale - member_scale);
        if digits.len() < cut {
            return Err("a `@default` with more decimals than its fixed type");
        }
        let keep = digits.len() - cut;
        if digits[keep..].bytes().any(|b| b != b'0') {
            return Err("a `@default` with more decimals than its fixed type");
        }
        digits.truncate(keep);
    }
    Ok(rust::Expr::MethodCall {
        receiver: Box::new(rust::Expr::Literal(Literal::string(&fixed_text(
            &digits,
            member_scale,
        )))),
        method: Ident::new("to_string"),
        args: Vec::new(),
    })
}

/// The default of a value the generated Rust holds as another type
/// (`zerodds_build::Config::rust_type`): `idl`, the IDL default, through
/// `zerodds_wire::repr::from_default`, with the `?` that rejects the
/// sample when the Rust type does not accept it.
///
/// Fallible, so it only stands where a `?` can: a decoder, a key-holder
/// reader. A type whose default contains one has no `impl Default`
/// (`encode::default_impl`).
fn mapped_default(mapping: &crate::adapter::cst::RustTypeMapping, idl: rust::Expr) -> rust::Expr {
    let mut args = Vec::new();
    let function = match &mapping.adapter {
        Some(adapter) => {
            args.push(rust::Expr::Ref {
                target: Box::new(rust::Expr::Path(adapter.clone())),
                mutable: false,
            });
            "from_default_with"
        }
        None => "from_default",
    };
    args.push(idl);
    rust::Expr::Try(Box::new(rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::new(&[
            "zerodds_wire",
            "repr",
            function,
        ]))),
        args,
    }))
}

/// The IDL default of a primitive or `string` as the `&Idl` a `WireRepr`
/// reads: a `&str` literal for a `string`, a reference to the literal for
/// a primitive. A member's `@default` (§7.2.2.4.4.4.9) ahead of Table 9's.
///
/// # Errors
/// [`EncodeError::NoWriter`] for a type no `rust_type` mapping stands
/// for, and for a `@default` this column cannot spell.
pub(crate) fn idl_default_value(
    ast: &Ast,
    member: Option<&MemberNode>,
    idl_type: Id<TypeNode>,
) -> Result<rust::Expr, EncodeError> {
    let name = member.map_or("", |member| member.name.as_str());
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: name.to_string(),
        kind,
    };
    let explicit_value =
        member.and_then(|member| member.facets.xtypes.as_ref().and_then(|x| x.default_value));
    let node = ast
        .type_(idl_type)
        .map_err(|_| refuse("a rust_type mapping over a stale type handle"))?;
    match &node.kind {
        TypeKind::String { .. } => {
            let text = match explicit_value {
                Some(expr) => match ast.expr(expr).map(|node| &node.expr) {
                    Ok(ConstExpr::Literal(AstLiteral::String(text))) => text.clone(),
                    _ => return Err(refuse("a `@default` of a mapped string that is no string")),
                },
                None => String::new(),
            };
            Ok(rust::Expr::Literal(Literal::string(&text)))
        }
        TypeKind::Primitive(kind) => {
            let value = match explicit_value {
                // A primitive names no enumerator, so no path is written.
                Some(expr) => explicit(ast, ast.root_module(), expr, idl_type, name)?,
                None => primitive_default(*kind),
            };
            Ok(rust::Expr::Ref {
                target: Box::new(value),
                mutable: false,
            })
        }
        _ => Err(refuse(
            "a rust_type mapping over an IDL type that is not a primitive or a string",
        )),
    }
}

fn call_path(segments: &[&str]) -> rust::Expr {
    rust::Expr::Call {
        callee: Box::new(rust::Expr::Path(Path::new(segments))),
        args: Vec::new(),
    }
}
