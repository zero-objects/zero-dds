// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Lua member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Lua. [`crate::default_python`] is the
//! same table as Python and carries the clause quotations; this is a
//! further derivation and deliberately not a call into any sibling —
//! the columns are compared against each other, and a shared helper
//! would make them agree about a mistake.
//!
//! ## The Lua spellings, where they differ from every sibling
//!
//! **An aggregate is a table constructor, not a call.** A Lua table
//! needs no class and no constructor: `{ x = 0, y = 0.0 }` *is* the
//! value, member defaults nested inside. A nested aggregate member is
//! still a call — `ZD.zd_def_Inner()` — so one type has one answer and
//! two positions cannot drift.
//!
//! **An `@optional` member is a key left out.** §7.2.2.4.4.4.8's
//! "Implementations shall not provide a default value" is the absence,
//! and in Lua the absence of a value is the absence of the key; a
//! constructor entry holding `nil` would not even create one.
//!
//! **A `char` and a `wchar` are the code point, as an integer.** The
//! member holds an integer — `adapter/lua.rs`'s mapping — so Table 9's
//! zero is `0`, not a one-character string.
//!
//! **A `long double` is the decimal text `"0.0"`.** A Lua float is a
//! binary64; the member holds text and `runtime/lua` converts exactly.
//! `"0.0"` and not `"0"`, because it is also what
//! `float128.bits_to_decimal` answers for the zero pattern, and one
//! value should have one spelling on both directions.
//!
//! **An enumeration's default is read out of the descriptor.**
//! `M.a.b.Colour.values.RED` — the materialiser's own table, through
//! [`CodecTarget::enumerator_constant`], so the file that declared the
//! value is the file this reads it from.
//!
//! **A fixed array is that many entries, spelled out.** A Lua table
//! constructor evaluates each entry, so `{ ZD.zd_def_T(), ZD.zd_def_T() }`
//! is two distinct tables — there is no `[x] * n` aliasing trap to
//! avoid, and the spelled-out form is simply the only form.
//!
//! [`CodecTarget::enumerator_constant`]: crate::emit::CodecTarget::enumerator_constant

use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::CodecTarget;
use crate::emit::lua::{CODECS, Lua};
use crate::encode::EncodeError;
use crate::fixed_literal::zero_decimal;
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Literal};
use zerodds_lang_cst::lua::{self, Entry, Expr, Place, Stat};

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type — keyed like
/// [`crate::emit::decode::decoder_name`], by the IDL kind for a
/// primitive: ten of the fifteen are one Lua `integer` and a name built
/// from the holder would give them one defaulter.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", Lua::type_name(ast, id)),
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

/// `place = ZD.zd_def_<T>()`.
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: Expr) -> Stat {
    Stat::Assign {
        doc: Doc::none(),
        target: place_of(place),
        value: codec_call(&defaulter_name(ast, id), Vec::new()),
    }
}

/// One member's default: its `@default`, or its type's. §7.2.2.4.4.4.9
/// puts the explicit one first.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a Lua expression — refused rather than
/// replaced by Table 9's zero, which is a different sample.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: Expr,
) -> Result<Vec<Stat>, EncodeError> {
    Ok(vec![Stat::Assign {
        doc: Doc::none(),
        target: place_of(place),
        value: member_value(ast, node, type_)?,
    }])
}

/// The defaulters `decls` call, and exactly those, read off the emitted
/// calls rather than derived a second time from the model — the same
/// closure discipline `default_python` and `default_c` state at length.
///
/// # Errors
/// [`EncodeError`] for a construct with no Lua default.
pub fn lua_defaulters_called_by(ast: &Ast, decls: &[Stat]) -> Result<Vec<Stat>, EncodeError> {
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
        let produced = lua_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_stat(decl, &mut called);
        }
    }
    lua_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Lua default.
pub fn lua_defaulters_over(ast: &Ast, types: &[Id<TypeNode>]) -> Result<Vec<Stat>, EncodeError> {
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
        out.push(Stat::Function(lua::Function {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not carry it.",
            ),
            target: Place::name(CODECS).field(&name),
            params: Vec::new(),
            body: vec![Stat::Return(Some(value))],
        }));
    }
    Ok(out)
}

/// The value one defaulter answers with, or `None` where the type has
/// no Lua data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<Expr>, EncodeError> {
    // Resolved before the table: a typedef and its target share a
    // defaulter name, and an arm answering with a call would name this
    // very function.
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

        // The empty string, for both widths.
        TypeKind::String { .. } => Expr::string(""),

        // The one spelling of zero the canonical form has for a scale.
        TypeKind::Fixed { scale, .. } => Expr::string(&zero_decimal(*scale)),

        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or `@default_literal`'s
        // (§7.3.1.2.1.10), read out of the descriptor by name so a moved
        // `@value` moves this with it.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            Lua::enumerator_constant(ast, id, &enumerator)
        }

        // No bit set.
        TypeKind::Bitmask { .. } => Expr::int(0),

        // One field per named bitfield, each at zero; the packing into
        // the anchor is the codec's.
        TypeKind::Bitset { .. } => {
            let mut entries = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding and holds no value.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                entries.push(Entry {
                    doc: Doc::none(),
                    key: lua::table_key(field_name),
                    value: Expr::int(0),
                });
            }
            Expr::Table(lua::Table::of(entries))
        }

        // "A zero-length sequence", and the same for a map's entries:
        // an empty table. Never `nil` — that is what an `@optional`
        // member's absence means, and a table entry holding `nil` does
        // not exist at all.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => Expr::Table(lua::Table::default()),

        // "An array of the same dimensions … whose elements take the
        // default value": one call per element, one table per
        // dimension. A Lua constructor evaluates each entry, so every
        // element is a distinct value.
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
            let mut out = codec_call(&defaulter_name(ast, element), Vec::new());
            for length in lengths.into_iter().rev() {
                out = Expr::Table(lua::Table::of(
                    vec![out; length]
                        .into_iter()
                        .map(Entry::positional)
                        .collect(),
                ));
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut entries = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                entries.push(Entry {
                    doc: Doc::none(),
                    key: lua::table_key("parent"),
                    value: codec_call(&defaulter_name(ast, base), Vec::new()),
                });
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
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
                entries.push(Entry {
                    doc: Doc::none(),
                    key: lua::table_key(&member_node.name),
                    value: member_value(ast, member_node, member_type)?,
                });
            }
            Expr::Table(lua::Table::of(entries))
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set" — and where it selects none,
        // `_d` alone is the whole value, which a Lua table holds as it
        // stands.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut entries = vec![Entry::named(
                "_d",
                discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                entries.push(Entry {
                    doc: Doc::none(),
                    key: lua::table_key(&field),
                    value: codec_call(&defaulter_name(ast, member_type), Vec::new()),
                });
            }
            Expr::Table(lua::Table::of(entries))
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
        None => Ok(codec_call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, as the Lua holder each takes.
fn primitive_default(kind: PrimitiveKind) -> Expr {
    match kind {
        PrimitiveKind::Boolean => Expr::bool(false),
        // The code point, as the integer the member holds.
        PrimitiveKind::Char | PrimitiveKind::WChar => Expr::int(0),
        // `0.0` and not `0`: `math.type` answers `float` for the one
        // and `integer` for the other, and the member is a float.
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            Expr::Literal(Literal::Float(lua::float_text("0")))
        }
        // The decimal text, spelled as `bits_to_decimal` spells the
        // zero pattern, so both directions answer one string.
        PrimitiveKind::Float128 => Expr::string("0.0"),
        PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => Expr::int(0),
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
        .map(|(member, member_type)| (member_type, member.name.clone())))
}

/// A discriminator value in the Lua form its type gives it: an
/// enumeration takes the constant read out of the descriptor, a
/// boolean the literal, and everything else — a `char` included, whose
/// holder is the code point — the number.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return Lua::enumerator_constant(ast, resolved, &enumerator);
        }
    }
    if matches!(
        resolved_primitive(ast, resolved),
        Some(PrimitiveKind::Boolean)
    ) {
        return Expr::bool(value != 0);
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

/// An explicit `@default` as a Lua expression for `type_`.
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
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => Ok(Expr::bool(*flag)),
        // The code point as the integer the member holds — a lone
        // surrogate included, because §7.4.1.1.1 writes a `wchar` as
        // one UTF-16 code unit and an integer holds any of them.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(Expr::int(i128::from(u32::from(*ch)))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(Expr::int(i128::from(*code))),
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
                // The member holds the decimal text and `runtime/lua`
                // parses it exactly; a Lua float literal would round to
                // binary64 before the value reached the member.
                return Ok(Expr::string(&written));
            }
            Ok(Expr::Literal(Literal::Float(lua::float_text(&written))))
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
            Ok(Lua::enumerator_constant(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => Expr::bool(value != 0),
                // The code point as the integer the member holds; one
                // outside a UTF-16 code unit is refused rather than
                // folded, because the wire has no form for it.
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    let point = u32::try_from(value)
                        .ok()
                        .filter(|code| *code <= u32::from(u16::MAX))
                        .ok_or_else(|| {
                            refuse("a `@default` character outside the range of a UTF-16 code unit")
                        })?;
                    Expr::int(i128::from(point))
                }
                // The decimal text of the integer, exactly.
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => {
                    Expr::string(&format!("{value}"))
                }
                // Every integer width, through the wide spelling: a Lua
                // decimal above 2^63 − 1 would denote a float.
                _ => Lua::wide_int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `ZD.zd_def_*` functions one statement calls, appended to `out`.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stat(statement: &Stat, out: &mut Vec<String>) {
    match statement {
        Stat::Local { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        Stat::Assign { target, value, .. } => {
            calls_in_place(target, out);
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
        Stat::Perform(value) => calls_in_expr(value, out),
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
        Stat::NumericFor { from, to, body, .. } => {
            calls_in_expr(from, out);
            calls_in_expr(to, out);
            for inner in body {
                calls_in_stat(inner, out);
            }
        }
        Stat::Break | Stat::Comment(_) | Stat::Annotation(_) | Stat::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Call { func, args } => {
            // A defaulter call is `ZD.zd_def_*()` and nothing else this
            // generator emits looks like one.
            if let Place::Field { base, name } = func.as_ref() {
                if matches!(base.as_ref(), Place::Name(root) if root.as_str() == CODECS)
                    && name.as_str().starts_with("zd_def_")
                    && !out.iter().any(|seen| seen == name.as_str())
                {
                    out.push(name.as_str().to_string());
                }
            }
            calls_in_place(func, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        Expr::Table(table) => {
            for entry in &table.entries {
                if let lua::Key::Expr(key) = &entry.key {
                    calls_in_expr(key, out);
                }
                calls_in_expr(&entry.value, out);
            }
        }
        Expr::Place(place) => calls_in_place(place, out),
        Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        Expr::Unary { value, .. } | Expr::Paren(value) => calls_in_expr(value, out),
        Expr::Nil | Expr::Literal(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (place-expression nesting the walk
/// builds). The tree walked here is the one this generator emitted and
/// owns, so it holds no cycle; the depth is that expression's height.
fn calls_in_place(place: &Place, out: &mut Vec<String>) {
    match place {
        Place::Name(_) => {}
        Place::Field { base, .. } => calls_in_place(base, out),
        Place::Index { base, key } => {
            calls_in_place(base, out);
            calls_in_expr(key, out);
        }
        Place::Paren(inner) => calls_in_expr(inner, out),
    }
}

// -- the small helpers --------------------------------------------------

fn codec_call(callee: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        func: Box::new(Place::name(CODECS).field(callee)),
        args,
    }
}

fn place_of(value: Expr) -> Place {
    match value {
        Expr::Place(place) => place,
        Expr::Paren(inner) => match *inner {
            Expr::Place(place) => place,
            other => Place::Paren(Box::new(other)),
        },
        other => Place::Paren(Box::new(other)),
    }
}
