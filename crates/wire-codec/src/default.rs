// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a member takes when the stream does not carry it.
//!
//! XTypes 1.3 §7.4.1.2.3: "if the receiver's member is not optional, it
//! logically takes its default value". §7.2.4.4.8, Table 19, says the
//! same thing about assignability in general — "Each non-optional member
//! in a T1 object that is not present in the T2 object takes its default
//! value" — so this is the one rule behind both halves of type
//! evolution: a `@mutable` member the writer omitted and the tail of an
//! `@appendable` type the writer did not have.
//!
//! ## Where the value comes from
//!
//! Two places, in this order, per §7.2.2.4.4.4.9:
//!
//! 1. **The member's explicit default**, if its declaration carries one.
//!    "An explicit definition associates a default value with the member
//!    at the time where the aggregated type is defined." That is
//!    `@default` in IDL. [`member`] is the entry point that consults it.
//! 2. **The type's implicit default**, otherwise: "The implicit default
//!    value for a member is defined to match the default value of the
//!    member type. See Table 9." That is [`of_type`].
//!
//! Substituting Table 9's zero for a member the designer gave an
//! explicit `@default` would be a silently wrong value, which is the
//! failure class this project ranks above every other.
//!
//! ## Table 9, exactly
//!
//! | Type kind | Default |
//! |---|---|
//! | `BYTE` | `0x00` |
//! | `BOOLEAN` | False |
//! | `INT_8`…`UINT_64`, `FLOAT_32`, `FLOAT_64`, `FLOAT_128` | 0 |
//! | `CHAR_8`, `CHAR_16` | `'\0'` |
//! | `STRING_TYPE` | `""` |
//! | `ARRAY_TYPE` | the same dimensions, each element at its default |
//! | `ALIAS_TYPE` | the default of the alias's base type |
//! | `SEQUENCE_TYPE` | a zero-length sequence |
//! | `MAP_TYPE` | an empty map |
//! | `ENUM_TYPE` | the first literal |
//! | `UNION_TYPE` | the discriminator at its default; if that selects a
//!   branch, the selected member at its default too |
//! | `STRUCTURE_TYPE` | no optional members, the others at their
//!   defaults |
//!
//! ## The three kinds Table 9 does not list
//!
//! `BITMASK_TYPE`, `BITSET_TYPE` and `fixed<P,S>` have no row. The
//! standing rule where the specification is silent is the broadest and
//! safest reading, written down:
//!
//! - A **bitmask** is held in a `UInt8`/`16`/`32`/`64` (Table 33) and
//!   §7.4.1.1.3.2 puts the bits at or above the bound outside the
//!   bitmask. The default of the holder is 0 by Table 9's integer row,
//!   and 0 sets no bit, so the default is the empty set. Table 18 pairs
//!   bitmask and enumeration for assignability, which is the same
//!   grouping; no reading of either clause produces a bitmask with a bit
//!   already set.
//! - A **bitset** is likewise packed into an anchor of 1, 2, 4 or 8
//!   bytes, whose Table 9 default is 0; every bitfield reads back 0 out
//!   of it. An anonymous bitfield is padding and carries no value, which
//!   is what the encoder and the decoder already agree on.
//! - `fixed<P,S>` is not in the XTypes type system at all — P4 of
//!   `docs/wire-feature-inventory.md` — so Table 9 could not have a row
//!   for it. IDL 4.2 §7.4.1.4.4.3.4 gives it a decimal value, and the
//!   only value every other row of Table 9 agrees on is zero. Written
//!   with the type's own scale, because that is the one spelling of zero
//!   `canonical::decimal` accepts for the type.
//!
//! Each is the same answer the type's serialised zero would decode to,
//! which is the property that makes the reading safe: a peer that
//! *wrote* the member at its default and a peer that omitted it hand the
//! receiver the same value.

use crate::canonical;
use crate::encode::ProbeError;
use crate::sink::{Aggregate, Sink};
use crate::source::Source;
use std::string::String;
use std::vec::Vec;
use zerodds_ast::{Ast, ConstExpr, Literal, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// How deep a default may nest before the model is taken to be cyclic.
///
/// A structure cannot contain itself by value — the recursion in IDL
/// goes through a sequence, whose default is empty, or through an
/// `@optional` member, which takes no default at all. So a walk this
/// deep means the model holds a cycle the frontend admitted, and
/// refusing is the only answer that is not a stack overflow.
const MAX_DEPTH: usize = 64;

/// The default value of one member: its `@default`, or its type's.
///
/// # Errors
/// [`ProbeError`] when the type has no default this project can name, or
/// when a `@default` and the member's type disagree.
pub fn member<V: Source + Sink>(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    path: &str,
) -> Result<V, ProbeError> {
    member_at_depth(ast, node, type_, path, 0)
}

/// [`member`], carrying the recursion depth of the walk it is part of.
fn member_at_depth<V: Source + Sink>(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    path: &str,
    depth: usize,
) -> Result<V, ProbeError> {
    // §7.2.2.4.4.4.9: "Optional members are not allowed to associate an
    // explicit default value for the member." An optional member never
    // reaches here — it takes no value at all — so a `@default` on one
    // is a declaration the frontend should have refused, and reading it
    // would give an absent member a value.
    if let Some(expr) = node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        if node.facets.is_optional() {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "an `@optional` member with an explicit `@default`, which \
                       §7.2.2.4.4.4.9 does not allow",
            });
        }
        return explicit(ast, expr, type_, path);
    }
    at_depth(ast, type_, path, depth)
}

/// Table 9's default for a type, with no member annotation in play.
///
/// # Errors
/// [`ProbeError`] for a type kind with no default.
pub fn of_type<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
) -> Result<V, ProbeError> {
    at_depth(ast, type_, path, 0)
}

/// zerodds-lint: recursion-depth 64 — `MAX_DEPTH` above, checked at the
/// top of this function. Table 9's default is a walk over the **type** and
/// not over a value, so nothing else bounds it: a model with a cycle would
/// descend forever, and the refusal names that rather than overflowing.
#[allow(clippy::too_many_lines)]
fn at_depth<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
    depth: usize,
) -> Result<V, ProbeError> {
    if depth > MAX_DEPTH {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a type whose default value nests deeper than a model without a cycle can",
        });
    }
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive(*kind, path),
        // `""` for both widths. Table 9 has one `STRING_TYPE` row and
        // §7.2.2.2.1.3 makes the wide string a string type; the empty
        // string is the same value in both.
        TypeKind::String { .. } => Ok(V::text(String::new())),
        TypeKind::Fixed { digits, scale } => {
            Ok(V::fixed(canonical::decimal("0", *digits, *scale, path)?))
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            at_depth(ast, target, path, depth + 1)
        }
        TypeKind::Enumeration { .. } => first_enumerator(ast, type_, path),
        // No bit set. See the module note.
        TypeKind::Bitmask { .. } => Ok(V::bit_names(Vec::new())),
        TypeKind::Bitset { .. } => {
            let mut out = Vec::new();
            for id in &ast.all_bitfields_of(type_) {
                let Ok(field) = ast.bitfield(*id) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    // Anonymous: padding, and no member of the value.
                    continue;
                };
                out.push((name.clone(), 0));
            }
            V::bitfields(out, path)
        }
        // "A zero-length sequence of the same element type" and "an
        // empty map of the same element type". Both are the empty array
        // this project's canonical form gives them.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => Ok(V::collection(Vec::new())),
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts: Vec<usize> = dims
                .iter()
                .map(|dim| {
                    ast.eval_integral(*dim)
                        .ok()
                        .and_then(|value| usize::try_from(value).ok())
                        .unwrap_or(0)
                })
                .collect();
            array(ast, element, path, &counts, depth + 1)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            structure(ast, type_, path, depth + 1)
        }
        TypeKind::Union { .. } => union(ast, type_, path, depth + 1),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// Table 9's primitive rows, each in the shape its own width has.
///
/// The shapes are `canonical::primitive`'s and not new ones — the same
/// [`Sink`] constructors, told the same [`PrimitiveKind`] — because a
/// default that came back as `0` where a read value comes back as `"0"`
/// would compare unequal to the very sample it is supposed to stand in
/// for. That was a real risk while the two spelled their zeros
/// separately.
fn primitive<V: Sink>(kind: PrimitiveKind, path: &str) -> Result<V, ProbeError> {
    match kind {
        PrimitiveKind::Boolean => Ok(V::boolean(false)),
        // `'\0'`, which is a one-character value here as every other
        // character is. §7.2.2.2.1.2.2 makes Char8 eight bits wide, and
        // U+0000 is inside it.
        PrimitiveKind::Char | PrimitiveKind::WChar => Ok(V::character(kind, '\0')),
        PrimitiveKind::Float128 => Ok(V::float128(zerodds_float128::Float128::ZERO)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => V::float(kind, 0.0_f64, path),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => V::integer(kind, 0, path),
    }
}

/// "The first literal in the enumeration", or the one `@default_literal`
/// names.
///
/// §7.3.1.2.1.10 exists for exactly this: "Normally the default value
/// for an object of a type is pre-defined based on the generic rules …
/// for an enumeration it is the first literal. This generic rule is not
/// desirable in some situations. The annotation `@default_literal`
/// allows this behavior to be changed."
///
/// First in **declaration** order and not in value order. Table 9 says
/// "the first literal in the enumeration", and `@value` reorders the
/// numbers without reordering the declaration.
fn first_enumerator<V: Sink>(ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<V, ProbeError> {
    let mut first = None;
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        if node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_default_literal)
        {
            return Ok(V::enumerator(node.name.clone()));
        }
        if first.is_none() {
            first = Some(node.name.clone());
        }
    }
    first
        .map(V::enumerator)
        .ok_or_else(|| ProbeError::Mismatch {
            path: path.to_string(),
            expected: "an enumeration with at least one enumerator, whose first is its default"
                .to_string(),
            found: "an empty enumeration".to_string(),
        })
}

/// "An array of the same dimensions and same element type whose elements
/// take the default value for their corresponding type."
///
/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `at_depth`, which carries `MAX_DEPTH`.
fn array<V: Source + Sink>(
    ast: &Ast,
    element: Id<TypeNode>,
    path: &str,
    counts: &[usize],
    depth: usize,
) -> Result<V, ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return at_depth(ast, element, path, depth);
    };
    // One element built and cloned rather than built `count` times: the
    // default of a type does not depend on the index, and a 4096-element
    // array of a nested structure would otherwise walk the model 4096
    // times to produce 4096 equal values.
    let one: V = array(ast, element, path, rest, depth)?;
    Ok(V::collection(std::iter::repeat_n(one, count).collect()))
}

/// "A structure without any of the optional members and with other
/// members set to their default values based on their corresponding
/// types."
///
/// `all_members_of` and not `members_of`, because §7.2.4.4.8 says
/// members of a base type "shall be considered 'expanded' inside" the
/// derived one: an inherited member is a member of this structure and
/// takes its own default like any other.
fn structure<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
    depth: usize,
) -> Result<V, ProbeError> {
    let mut out = Aggregate::new();
    for id in &ast.all_members_of(type_) {
        let Ok(node) = ast.member(*id) else { continue };
        // "A structure without any of the optional members": an absent
        // optional member is absent, not zero.
        if node.facets.is_optional() {
            continue;
        }
        let Some(member_type) = node.type_.resolved() else {
            return Err(ProbeError::NoLayout {
                detail: format!("{path}.{}: the member type is unresolved", node.name),
            });
        };
        let member_path = format!("{path}.{}", node.name);
        out.insert(
            node.name.clone(),
            member_at_depth(ast, node, member_type, &member_path, depth)?,
        );
    }
    Ok(V::aggregate(out))
}

/// "A union with the discriminator set to the default value for the
/// discriminator type. If this selects a branch, then the selected
/// member is also set to the default value for the member type.
/// Otherwise the value of the union is fully specified by the
/// discriminator value."
///
/// zerodds-lint: recursion-depth 64 — `MAX_DEPTH`, through `at_depth`,
/// which this hands the depth it was given.
fn union<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
    depth: usize,
) -> Result<V, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a union whose kind changed under the probe",
        });
    };
    let discriminator = discriminator
        .resolved()
        .ok_or_else(|| ProbeError::NoLayout {
            detail: format!("{path}: the discriminator is unresolved"),
        })?;
    let discriminant_path = format!("{path}._d");
    let discriminant: V = at_depth(ast, discriminator, &discriminant_path, depth)?;

    let mut out = Aggregate::new();
    let selector =
        crate::decode::selector_of(ast, discriminator, &discriminant, &discriminant_path)?;
    out.insert(String::from("_d"), discriminant);
    // "If this selects a branch" — which includes the `default:` branch,
    // because a discriminator value with no explicit label selects it.
    //
    // `false` for `UnionLabelMatch`, and it is a checked non-existence
    // rather than a plan this walk does not have: the selector here is
    // always Table 9's default for the discriminator's type, which is
    // zero for every integer, `'\0'` for a `char` and `FALSE` for a
    // `boolean`. The setting suppresses **negative** labels only, so no
    // value it can reach is one this walk ever compares against.
    // A label the model cannot evaluate is refused here too. This walk
    // builds the default value of a union, and a skipped label would
    // silently give the type a different branch's default — or none —
    // which is the value every column then agrees on for the wrong
    // reason.
    let selected = crate::decode::select_case(ast, type_, selector, false).map_err(|_| {
        ProbeError::Mismatch {
            path: path.to_string(),
            expected: String::from("a union whose case labels all evaluate"),
            found: String::from("a case label the model cannot evaluate"),
        }
    })?;
    if let Some(case) = selected {
        let member_path = format!("{path}.{}", case.name);
        let value = at_depth(ast, case.type_, &member_path, depth)?;
        out.insert(case.name, value);
    }
    Ok(V::aggregate(out))
}

/// The value an explicit `@default` names, in the member type's own
/// canonical form.
///
/// The literal is read and then put through `canonical::canonicalise`
/// against the member's type, which is the same gate a value file goes
/// through. That is what refuses `@default("abc")` on a `long` instead
/// of quietly writing a zero into it: an annotation whose value the
/// member cannot hold is a declaration error, and a probe that dropped
/// it would produce a member the designer never asked for.
fn explicit<V: Source + Sink>(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    path: &str,
) -> Result<V, ProbeError> {
    let raw: V = literal_value(ast, expr, path)?;
    canonical::canonicalise(ast, type_, &raw, path)
}

/// One `@default` expression in loose form, before the type sees it.
///
/// An integral expression is evaluated — `@default(1 << 3)` is legal
/// IDL — and every other kind has to be a literal, because there is no
/// arithmetic on strings or characters to fold.
///
/// Loose because the literal is written without a type: a `@default(3)`
/// on a `long` and on a `long long` are the same literal, and which one
/// it is a value of is decided one line above, by
/// [`canonical::canonicalise`] against the member's own type. So the
/// text form is [`Sink::text`] for every kind that is not a number,
/// including the enumerator's name — it is not yet known to *be* an
/// enumerator name until the member's type says so.
fn literal_value<V: Sink>(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    path: &str,
) -> Result<V, ProbeError> {
    let node = ast.expr(expr).map_err(|error| ProbeError::NoLayout {
        detail: format!("{path}: {error}"),
    })?;
    match &node.expr {
        ConstExpr::Literal(Literal::Boolean(flag)) => Ok(V::boolean(*flag)),
        ConstExpr::Literal(Literal::Char(ch)) => Ok(V::text(ch.to_string())),
        ConstExpr::Literal(Literal::WChar(code)) => match char::from_u32(*code) {
            Some(ch) => Ok(V::text(ch.to_string())),
            None => Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a `@default` wide character that is a code point".to_string(),
                found: format!("{code:#x}"),
            }),
        },
        ConstExpr::Literal(Literal::String(text)) => Ok(V::text(text.clone())),
        ConstExpr::Literal(Literal::WString(codes)) => {
            let mut text = String::with_capacity(codes.len());
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(ProbeError::Mismatch {
                        path: path.to_string(),
                        expected: "a `@default` wide string of code points".to_string(),
                        found: format!("{code:#x}"),
                    });
                };
                text.push(ch);
            }
            Ok(V::text(text))
        }
        // Written out rather than parsed into an `f64` here: the string
        // is what `canonical::canonicalise` narrows against the member's
        // own width, so a `float` member and a `double` member with the
        // same literal each get the value their width holds.
        ConstExpr::Literal(Literal::Float {
            mantissa,
            exponent: None,
        }) => number_text(mantissa, path),
        ConstExpr::Literal(Literal::Float {
            mantissa,
            exponent: Some(exponent),
        }) => number_text(&format!("{mantissa}e{exponent}"), path),
        ConstExpr::Literal(Literal::Fixed { digits, scale }) => {
            Ok(V::text(fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(Literal::Enumerator(reference)) => {
            let target = reference.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the `@default` enumerator is unresolved"),
            })?;
            let node = ast
                .enumerator(target)
                .map_err(|error| ProbeError::NoLayout {
                    detail: format!("{path}: {error}"),
                })?;
            Ok(V::text(node.name.clone()))
        }
        // An integer literal, a named constant or any arithmetic over
        // them. `eval_integral` is the same folder every bound and every
        // case label goes through.
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|error| ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: "a `@default` this frontend can evaluate".to_string(),
                    found: format!("{error:?}"),
                })?;
            // A string, so that a 64-bit member keeps its low bits;
            // `canonicalise` reads both spellings for the narrow widths.
            Ok(V::text(format!("{value}")))
        }
    }
}

/// A float literal's text as a number the representation can hold,
/// refusing what it cannot write.
fn number_text<V: Sink>(text: &str, path: &str) -> Result<V, ProbeError> {
    let parsed: f64 = text.parse().map_err(|_| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "a `@default` floating-point literal".to_string(),
        found: format!("`{text}`"),
    })?;
    V::number(parsed, path)
}

/// A fixed-point literal's digits and scale back into a decimal.
///
/// `IDL 4.2` records the digits without the point and the scale beside
/// them; `canonical::decimal` wants the point written, and it is the one
/// that decides how many digits the member's own `fixed<P,S>` keeps.
fn fixed_text(digits: &str, scale: u8) -> String {
    let (negative, magnitude) = match digits.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, digits),
    };
    let scale = usize::from(scale);
    let padded = if magnitude.len() <= scale {
        format!("{}{magnitude}", "0".repeat(scale + 1 - magnitude.len()))
    } else {
        magnitude.to_string()
    };
    let split = padded.len() - scale;
    let mut out = String::with_capacity(padded.len() + 2);
    if negative {
        out.push('-');
    }
    out.push_str(&padded[..split]);
    if scale > 0 {
        out.push('.');
        out.push_str(&padded[split..]);
    }
    out
}
