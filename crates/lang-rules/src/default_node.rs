// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Node (plain JavaScript) member takes when the stream does
//! not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as JavaScript. [`crate::default_lua`] is
//! the nearest sibling — a dynamically-typed target needs no class and
//! no constructor for an aggregate default, a plain literal *is* the
//! value — and this is a further derivation and deliberately not a call
//! into it or into [`crate::default_typescript`]: the columns are
//! compared against each other, and a shared helper would make them
//! agree about a mistake.
//!
//! ## The Node spellings, where they differ from Lua's
//!
//! **An `@optional` member is a key left out**, exactly as in Lua, and
//! for the same clause — §7.2.2.4.4.4.8's "Implementations shall not
//! provide a default value" is the absence, and in a JavaScript object
//! literal the absence of a value is the absence of the key.
//!
//! **A `char` and a `wchar` are the code point, as a one-character
//! `string`.** [`crate::emit::node`]'s module documentation carries why
//! this column keeps `crate::typescript`'s value model rather than
//! Lua's or Zig's integer one: Table 9's zero is therefore `"\0"`
//! (`Literal::Char('\0')`), not `0`.
//!
//! **The three sixty-four-bit primitives are `bigint` literals.**
//! `0n`, spelled through [`zerodds_lang_cst::node::Expr::Literal`] with
//! the trailing `n` [`crate::emit::node::Node::wide_signed`] and
//! [`crate::emit::typescript`]'s own overrides already carry.
//!
//! **A `long double` is the decimal text `"0"`.** A JavaScript `number`
//! is a binary64; the member holds the decimal exactly as
//! `runtime/node/zdwire.js`'s `float128FromDecimalString` takes it.
//!
//! **An enumeration's default is read out of the flattened declaration**
//! through [`CodecTarget::enumerator_constant`], so the file that
//! declared the value is the file this reads it from, exactly as every
//! sibling column reads it.
//!
//! [`CodecTarget::enumerator_constant`]: crate::emit::CodecTarget::enumerator_constant

use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::CodecTarget;
use crate::emit::node::Node;
use crate::encode::EncodeError;
use crate::fixed_literal::zero_decimal;
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Literal};
use zerodds_lang_cst::node::{Expr, Function, Stat};

/// How deep a chain of typedefs is followed before the model is taken to
/// be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type — keyed like
/// [`crate::emit::decode::decoder_name`], by the IDL kind for a
/// primitive: the walk needs one function per wire shape and a name
/// built from the JavaScript holder would give a `long` and an `octet`
/// one defaulter, because both are `number`.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", Node::type_name(ast, id)),
    }
}

/// The defaulter that goes with a decoder, by name: the two schemes
/// differ in one word because both are built from the same key.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_<T>();`.
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: Expr) -> Stat {
    Stat::Assign {
        doc: Doc::none(),
        place,
        value: Expr::call(&defaulter_name(ast, id), Vec::new()),
    }
}

/// One member's default: its `@default`, or its type's. §7.2.2.4.4.4.9
/// puts the explicit one first.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a JavaScript expression — refused rather than
/// replaced by Table 9's zero, which is a different sample.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: Expr,
) -> Result<Vec<Stat>, EncodeError> {
    Ok(vec![Stat::Assign {
        doc: Doc::none(),
        place,
        value: member_value(ast, node, type_)?,
    }])
}

/// The defaulters `decls` call, and exactly those, read off the emitted
/// calls rather than derived a second time from the model — the same
/// closure discipline [`crate::default_python`] and [`crate::default_c`]
/// state at length.
///
/// # Errors
/// [`EncodeError`] for a construct with no Node default.
pub fn node_defaulters_called_by(ast: &Ast, decls: &[Stat]) -> Result<Vec<Stat>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_stat(decl, &mut called);
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
        let produced = node_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_stat(decl, &mut called);
        }
    }
    node_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Node default.
pub fn node_defaulters_over(ast: &Ast, types: &[Id<TypeNode>]) -> Result<Vec<Stat>, EncodeError> {
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
        out.push(Stat::Function(Function {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not carry it.",
            ),
            name: zerodds_lang_cst::common::Ident::new(&name),
            params: Vec::new(),
            body: vec![Stat::Return(Some(value))],
        }));
    }
    Ok(out)
}

/// The value one defaulter answers with, or `None` where the type has no
/// Node data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<Expr>, EncodeError> {
    // Resolved before the match: a typedef and its target share a
    // defaulter name, and an arm answering with a call would name this
    // very function.
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // A type the model does not hold. `Ok(None)` here reads to the
    // caller as "nothing to emit for this one" and skips the defaulter,
    // so a stale handle quietly costs a type its Table 9 value and the
    // first sign of it is a member the generated file leaves out. The
    // Kotlin and Swift columns already refuse it by name; this one does
    // too.
    let Ok(type_node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let value = match &type_node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),

        // The empty string, for both widths.
        TypeKind::String { .. } => Expr::string(""),

        // The one spelling of zero the canonical form has for a scale.
        TypeKind::Fixed { scale, .. } => Expr::string(&zero_decimal(*scale)),

        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or `@default_literal`'s
        // (§7.3.1.2.1.10), read out of the flattened declaration by name
        // so a moved `@value` moves this with it.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            Node::enumerator_constant(ast, id, &enumerator)
        }

        // No bit set. `0n` where the holder is a `bigint`, otherwise a
        // plain `0` — the same boundary `crate::emit::node::Node::scalar`
        // draws for every other integer.
        TypeKind::Bitmask { .. } => {
            if ast.effective_bit_bound(id) > 32 {
                Node::wide_unsigned(0)
            } else {
                Expr::int(0)
            }
        }

        // One field per named bitfield, each at zero; the packing into
        // the anchor is the codec's.
        TypeKind::Bitset { .. } => {
            let mut entries = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                // Refused, not skipped: the key would simply be absent
                // from the object literal, and `undefined` is what an
                // `@optional` member's absence means here — a different
                // value from the zero Table 9 gives a bitfield.
                let Ok(field_node) = ast.bitfield(*field) else {
                    return Err(refuse("a bitfield the model does not hold"));
                };
                // An anonymous bitfield is padding and holds no value.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                entries.push((
                    zerodds_lang_cst::common::Ident::new(field_name),
                    Expr::int(0),
                ));
            }
            Expr::Object(entries)
        }

        // "A zero-length sequence", and the same for a map's entries: an
        // empty array. Never `undefined` — that is what an `@optional`
        // member's absence means, and an array holding no elements is a
        // real, present value.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => Expr::Array(Vec::new()),

        // "An array of the same dimensions … whose elements take the
        // default value": one call per element, one array literal per
        // dimension. Each element is its own call, so two elements of a
        // structure type are two distinct objects and not one aliased
        // twice.
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
            let mut out = Expr::call(&defaulter_name(ast, element), Vec::new());
            for length in lengths.into_iter().rev() {
                out = Expr::Array(vec![out; length]);
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut entries = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                entries.push((
                    zerodds_lang_cst::common::Ident::new("parent"),
                    Expr::call(&defaulter_name(ast, base), Vec::new()),
                ));
            }
            for member_id in ast.members_of(id) {
                // Refused, not skipped. A skipped member leaves its key
                // out of the object literal, which is exactly the shape
                // an `@optional` member's absence takes two lines down —
                // the two states would be indistinguishable in the
                // generated file.
                let Ok(member_node) = ast.member(*member_id) else {
                    return Err(refuse("a member the model does not hold"));
                };
                // Left out: the absence of the key is the absence.
                if member_node.facets.is_optional() {
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                entries.push((
                    zerodds_lang_cst::common::Ident::new(&member_node.name),
                    member_value(ast, member_node, member_type)?,
                ));
            }
            Expr::Object(entries)
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set" — and where it selects none, `_d`
        // alone is the whole value, which a plain object holds as it
        // stands.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            // Refused, not defaulted to zero. A discriminator whose type
            // has no Table 9 value of its own is one IDL 4.2 7.4.1.4.4.2
            // does not admit; writing `_d: 0` there would state a
            // discriminant the source never named and select whichever
            // branch happens to carry that label.
            let selector = discriminator_default_value(ast, discriminator)
                .ok_or_else(|| refuse("union whose discriminator type has no default value"))?;
            let mut entries = vec![(
                zerodds_lang_cst::common::Ident::new("_d"),
                discriminant_literal(ast, discriminator, selector),
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                entries.push((
                    zerodds_lang_cst::common::Ident::new(&field),
                    Expr::call(&defaulter_name(ast, member_type), Vec::new()),
                ));
            }
            Expr::Object(entries)
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
fn member_value(ast: &Ast, node: &MemberNode, type_: Id<TypeNode>) -> Result<Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(Expr::call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, as the JavaScript holder each takes.
fn primitive_default(kind: PrimitiveKind) -> Expr {
    match kind {
        PrimitiveKind::Boolean => Expr::bool(false),
        // The code point, as the one-character string the member holds
        // — the value model `crate::emit::node`'s module documentation
        // keeps from `crate::typescript`.
        PrimitiveKind::Char | PrimitiveKind::WChar => Expr::Literal(Literal::Char('\0')),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            Expr::Literal(Literal::Float("0".to_string()))
        }
        // The decimal text; `runtime/node/zdwire.js` parses it exactly.
        PrimitiveKind::Float128 => Expr::string("0"),
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => Node::wide_signed(0),
        PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => Expr::int(0),
    }
}

/// The case a defaulted discriminator selects, if any.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: i128,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, member.name.clone())))
}

/// A discriminator value in the Node form its type gives it: an
/// enumeration takes the constant read out of its flattened declaration,
/// a boolean the literal, and everything else — a `char` included, whose
/// holder is a one-character string — the number.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return Node::enumerator_constant(ast, resolved, &enumerator);
        }
    }
    if matches!(
        resolved_primitive(ast, resolved),
        Some(PrimitiveKind::Boolean)
    ) {
        return Expr::bool(value != 0);
    }
    if matches!(
        resolved_primitive(ast, resolved),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar)
    ) {
        let point = u32::try_from(value).ok().and_then(char::from_u32);
        if let Some(ch) = point {
            return Expr::Literal(Literal::Char(ch));
        }
    }
    if matches!(
        resolved_primitive(ast, resolved),
        Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64)
    ) {
        return Node::wide_signed(value);
    }
    Expr::int(value)
}

/// The primitive `id` is once typedefs are seen through.
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

/// An explicit `@default` as a Node expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<Expr, EncodeError> {
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
    let is_wide = matches!(
        kind,
        Ok(TypeKind::Primitive(
            PrimitiveKind::Int64 | PrimitiveKind::UInt64
        ))
    );
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => Ok(Expr::bool(*flag)),
        // The code point as the one-character string the member holds.
        // §7.4.1.1.1 writes a `wchar` as one UTF-16 code unit and
        // `String.fromCharCode` would hold a lone surrogate, but
        // `Literal::Char` holds a Rust `char` and a surrogate is not
        // one — so the point that this tree cannot carry is refused by
        // name rather than written out as NUL, which is the same
        // `@default` a source spelling `'\0'` would produce.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(Expr::Literal(Literal::Char(*ch))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => {
            let point = char::from_u32(*code)
                .ok_or_else(|| refuse("a `@default` `wchar` that is not a Unicode scalar value"))?;
            Ok(Expr::Literal(Literal::Char(point)))
        }
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(Expr::string(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                out.push(ch);
            }
            Ok(Expr::string(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            let written = format!("{mantissa}{exponent}");
            if is_float128 {
                // The member holds the decimal text and
                // `runtime/node/zdwire.js` parses it exactly; a
                // JavaScript `number` literal would round to binary64
                // before the value reached the member.
                return Ok(Expr::string(&written));
            }
            Ok(Expr::Literal(Literal::Float(written)))
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => Ok(Expr::string(
            &crate::adapter::constant::fixed_decimal(digits, *scale),
        )),
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let target = reference
                .resolved()
                .ok_or_else(|| refuse("a `@default` enumerator that is unresolved"))?;
            let enumerator = ast
                .enumerator(target)
                .map_err(|_| refuse("a `@default` enumerator this frontend cannot follow"))?;
            Ok(Node::enumerator_constant(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => Expr::bool(value != 0),
                // The code point as the one-character string the member
                // holds; one outside a UTF-16 code unit is refused
                // rather than folded, because the wire has no form for
                // it.
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    let point = u32::try_from(value)
                        .ok()
                        .filter(|code| *code <= u32::from(u16::MAX))
                        .and_then(char::from_u32)
                        .ok_or_else(|| {
                            refuse("a `@default` character outside the range of a UTF-16 code unit")
                        })?;
                    Expr::Literal(Literal::Char(point))
                }
                // The decimal text of the integer, exactly.
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => {
                    Expr::string(&format!("{value}"))
                }
                _ if is_wide => Node::wide_signed(value),
                _ => Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ------------------------------------------

/// The `zd_def_*` functions one statement calls, appended to `out`.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stat(statement: &Stat, out: &mut Vec<String>) {
    match statement {
        Stat::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        Stat::Assign { place, value, .. } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        Stat::Function(function) => {
            for inner in &function.body {
                calls_in_stat(inner, out);
            }
        }
        Stat::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        Stat::Expr(value) | Stat::Throw(value) => calls_in_expr(value, out),
        Stat::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            for inner in then {
                calls_in_stat(inner, out);
            }
            if let Some(rest) = otherwise {
                for inner in rest {
                    calls_in_stat(inner, out);
                }
            }
        }
        Stat::While { condition, body } => {
            calls_in_expr(condition, out);
            for inner in body {
                calls_in_stat(inner, out);
            }
        }
        Stat::For { bound, body, .. } => {
            calls_in_expr(bound, out);
            for inner in body {
                calls_in_stat(inner, out);
            }
        }
        Stat::Break | Stat::Comment(_) | Stat::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args } => {
            // A defaulter call is `zd_def_*(...)` and nothing else this
            // generator emits looks like one.
            if let Expr::Name(name) = callee.as_ref() {
                if name.as_str().starts_with("zd_def_")
                    && !out.iter().any(|seen| seen == name.as_str())
                {
                    out.push(name.as_str().to_string());
                }
            }
            calls_in_expr(callee, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        Expr::Array(elements) => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        Expr::Object(fields) => {
            for (_, value) in fields {
                calls_in_expr(value, out);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        Expr::Member { object, .. } => calls_in_expr(object, out),
        Expr::Index { object, index } => {
            calls_in_expr(object, out);
            calls_in_expr(index, out);
        }
        Expr::Paren(inner) | Expr::Unary { value: inner, .. } => calls_in_expr(inner, out),
        Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        Expr::Ternary {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_expr(then, out);
            calls_in_expr(otherwise, out);
        }
        Expr::Literal(_) | Expr::Undefined | Expr::Name(_) => {}
    }
}
