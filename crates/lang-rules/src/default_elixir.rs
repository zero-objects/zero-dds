// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value an Elixir member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Elixir. [`crate::default_python`] is
//! the same table as Python and carries the clause quotations;
//! deliberately a further derivation and not a call into any sibling,
//! because the columns are compared against each other and a shared
//! helper would make them agree about a mistake.
//!
//! ## The shape
//!
//! One zero-arity function per type — `zd_def_T() -> value` — because a
//! literal cannot be trusted to appear identically at every position a
//! member can go missing, and because the decoder's own shape wants it:
//! `emit/elixir.rs`'s reader opens with `v = zd_def_T()`, so every
//! position the stream does not carry is already what Table 9 says
//! before an octet is read.
//!
//! ## What the Elixir mapping decides
//!
//! - **An aggregate is a struct literal**: `%Common.Pose{x: 0, y: 0.0}`,
//!   every non-optional field written. An `@optional` member is left
//!   out — the `defstruct` default is already `nil`, and
//!   §7.2.2.4.4.4.8's "Implementations shall not provide a default
//!   value" is that absence.
//! - **An enumeration's default is its constant function**:
//!   `Common.Mode.idle()`, which answers the declared integer — the
//!   binding holds enumerations as integers, and the function is the
//!   materialiser's own spelling, so a moved `@value` moves both sides.
//! - **A `char` is `"\0"`** — a one-character binary, because the
//!   member holds one and an integer there would be a different type.
//! - **A `long double` is `ZeroddsWire.Float128.zero()`** — sixteen
//!   octets, never a `float()`, which holds 53 of its 113 significand
//!   bits.
//! - **A `fixed<P,S>` is the one decimal spelling of zero for its
//!   scale** — `"0.00"` for `fixed<5,2>`; `""` is not a decimal.
//! - **A collection is `[]`** and an array is its elements spelled out
//!   as a tuple, one defaulter call per element — the `{T, …}` the
//!   materialiser declares, which a list is not `==` to. `List.duplicate(zd_def_T(), n)`
//!   would evaluate the element once, which for an immutable value is
//!   harmless — and is still not written, because the sibling columns
//!   spell the elements out and a reader comparing generated files
//!   should not have to know which columns may share.

use crate::adapter::elixir::type_module_path;
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::CodecTarget;
use crate::emit::elixir::Elixir;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::elixir::{self as ex, atom_name};

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type, keyed the same way
/// [`crate::emit::decode::decoder_name`] is.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", Elixir::type_name(ast, id)),
    }
}

/// The defaulter that goes with a decoder, by name — the two schemes
/// differ in one word because both are built from the same key.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_T()`.
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: ex::Expr) -> ex::Statement {
    assign(place, call(&defaulter_name(ast, id), Vec::new()))
}

/// One member's default: its `@default`, or its type's. §7.2.2.4.4.4.9
/// puts the explicit one first.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell. Refused rather than silently replaced by
/// Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: ex::Expr,
) -> Result<Vec<ex::Statement>, EncodeError> {
    Ok(vec![assign(place, member_value(ast, node, type_)?)])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model, for
/// the reason [`crate::default_python::python_defaulters_called_by`]
/// gives at length.
///
/// # Errors
/// [`EncodeError`] for a construct with no Elixir default.
pub fn elixir_defaulters_called_by(
    ast: &Ast,
    decls: &[ex::CodecFunction],
) -> Result<Vec<ex::CodecFunction>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_function(decl, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    // Bounded by the number of named defaulters: every round but the
    // last adds at least one.
    for _ in 0..=index.len() {
        let mut grew = false;
        for name in &called.clone() {
            if have.contains(name) {
                continue;
            }
            have.push(name.clone());
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = elixir_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_function(decl, &mut called);
        }
    }
    elixir_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Elixir default.
pub fn elixir_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<ex::CodecFunction>, EncodeError> {
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
        out.push(ex::CodecFunction {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not carry it.",
            ),
            name: Ident::new(&name),
            params: Vec::new(),
            body: Vec::new(),
            result: value,
        });
    }
    Ok(out)
}

/// The value one defaulter answers with, or `None` where the type has
/// no Elixir data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<ex::Expr>, EncodeError> {
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

        // `""`, for both widths.
        TypeKind::String { .. } => text(""),

        // The one decimal spelling of zero for the scale.
        TypeKind::Fixed { scale, .. } => text(&zero_decimal(*scale)),

        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // §7.3.1.2.1.10: the `@default_literal`, or the first literal.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            Elixir::enumerator_constant(ast, id, &enumerator)
        }

        // No bit set. The binding holds a bitmask as a plain integer.
        TypeKind::Bitmask { .. } => ex::Expr::int(0),

        // The struct with every named bitfield at zero.
        TypeKind::Bitset { .. } => {
            let mut fields = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                if let Some(field_name) = field_node.name.as_ref() {
                    fields.push((Ident::new(&atom_name(field_name)), ex::Expr::int(0)));
                }
            }
            ex::Expr::StructLiteral {
                module: type_module_path(ast, id),
                fields,
            }
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries. `[]` and not `nil`: `nil` is what
        // an `@optional` member's absence means here.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => ex::Expr::List(Vec::new()),

        // One call per element, one tuple per dimension; see the module
        // documentation for why the elements are spelled out.
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
            let mut out = call(&defaulter_name(ast, element), Vec::new());
            for length in lengths.into_iter().rev() {
                out = ex::Expr::Tuple(vec![out; length]);
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut fields = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                fields.push((
                    Ident::new(&atom_name("parent")),
                    call(&defaulter_name(ast, base), Vec::new()),
                ));
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                if member_node.facets.is_optional() {
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                fields.push((
                    Ident::new(&atom_name(&member_node.name)),
                    member_value(ast, member_node, member_type)?,
                ));
            }
            ex::Expr::StructLiteral {
                module: type_module_path(ast, id),
                fields,
            }
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type", and the member that selects, where
        // one does.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut fields = vec![(
                Ident::new("_d"),
                discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                fields.push((
                    Ident::new(&field),
                    call(&defaulter_name(ast, member_type), Vec::new()),
                ));
            }
            ex::Expr::StructLiteral {
                module: type_module_path(ast, id),
                fields,
            }
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
) -> Result<ex::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, as the Elixir holder each takes.
fn primitive_default(kind: PrimitiveKind) -> ex::Expr {
    match kind {
        PrimitiveKind::Boolean => ex::Expr::Literal(Literal::Bool(false)),
        // A one-character binary; the octet is the C column's `0`.
        PrimitiveKind::Char | PrimitiveKind::WChar => text("\0"),
        // `0.0` and not `0`: the member is a `float()` and the spelling
        // is what says which of the two numeric types the value is.
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            ex::Expr::Literal(Literal::Float("0.0".to_string()))
        }
        // The binary128 zero bit pattern, from the runtime.
        PrimitiveKind::Float128 => ex::Expr::Call {
            func: Path::new(&["ZeroddsWire", "Float128", "zero"]),
            args: Vec::new(),
        },
        PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => ex::Expr::int(0),
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
        .map(|(member, member_type)| (member_type, atom_name(&member.name))))
}

/// A discriminator value in the Elixir form its type gives it — the
/// same rewrites `emit/elixir.rs`'s `cast` makes at every label.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> ex::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return Elixir::enumerator_constant(ast, resolved, &enumerator);
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => ex::Expr::Literal(Literal::Bool(value != 0)),
        // Table 9's zero, which is the only value that reaches here —
        // the enumeration branch has answered every designer-chosen one.
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            char_literal(u32::try_from(value).unwrap_or(0))
        }
        _ => ex::Expr::int(value),
    }
}

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by
/// `MAX_ALIAS_DEPTH`)
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

/// An explicit `@default` as an Elixir expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<ex::Expr, EncodeError> {
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
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(ex::Expr::Literal(Literal::Bool(*flag)))
        }
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(char_literal(u32::from(*ch))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(char_literal(*code)),
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
                // Through the decimal and never through a `float()`:
                // `Float128.from_decimal` is exact.
                return Ok(ex::Expr::Call {
                    func: Path::new(&["ZeroddsWire", "Float128", "from_decimal"]),
                    args: vec![text(&written)],
                });
            }
            // The renderer's `float_text` adds the point and zero
            // Elixir's grammar wants; the value never changes.
            Ok(ex::Expr::Literal(Literal::Float(written)))
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
            Ok(Elixir::enumerator_constant(ast, resolved, &enumerator.name))
        }
        _ => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    ex::Expr::Literal(Literal::Bool(value != 0))
                }
                // Refused where the value is outside a UTF-16 code
                // unit; and a surrogate has no Elixir binary at all, so
                // it too is a refusal here — the codec's own label
                // rewrite compares with an atom instead, but a member
                // *holding* one is a value this binding cannot store.
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    let point = u32::try_from(value)
                        .ok()
                        .filter(|code| *code <= u32::from(u16::MAX))
                        .ok_or_else(|| {
                            refuse("a `@default` character outside the range of a UTF-16 code unit")
                        })?;
                    if char::from_u32(point).is_none() {
                        return Err(refuse(
                            "a `@default` surrogate, which no Elixir binary can hold",
                        ));
                    }
                    char_literal(point)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => ex::Expr::Call {
                    func: Path::new(&["ZeroddsWire", "Float128", "from_int"]),
                    args: vec![ex::Expr::int(value)],
                },
                _ => ex::Expr::int(value),
            })
        }
    }
}

/// A code point as the one-character binary a `char` member holds.
///
/// A surrogate has no UTF-8 spelling and therefore no Elixir binary;
/// the callers bound the value before it gets here, and the one that
/// cannot — an explicit `@default` — refuses by name instead.
fn char_literal(code: u32) -> ex::Expr {
    match char::from_u32(code) {
        Some(point) => ex::Expr::Literal(Literal::Str(point.to_string())),
        None => text("\0"),
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one routine calls, appended to `out`.
fn calls_in_function(function: &ex::CodecFunction, out: &mut Vec<String>) {
    calls_in_statements(&function.body, out);
    calls_in_expr(&function.result, out);
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statements(statements: &[ex::Statement], out: &mut Vec<String>) {
    for statement in statements {
        match statement {
            ex::Statement::Bind { value, .. } | ex::Statement::BindTuple { value, .. } => {
                calls_in_expr(value, out);
            }
            ex::Statement::Update { path, value, .. } => {
                for step in path {
                    if let ex::Access::At(index) = step {
                        calls_in_expr(index, out);
                    }
                }
                calls_in_expr(value, out);
            }
            ex::Statement::If {
                condition,
                then,
                otherwise,
            } => {
                calls_in_expr(condition, out);
                calls_in_statements(then, out);
                calls_in_statements(otherwise, out);
            }
            ex::Statement::For { bound, body, .. } => {
                calls_in_expr(bound, out);
                calls_in_statements(body, out);
            }
            ex::Statement::While { condition, body } => {
                calls_in_expr(condition, out);
                calls_in_statements(body, out);
            }
            ex::Statement::Return(value) => calls_in_expr(value, out),
            ex::Statement::Seq(inner) => calls_in_statements(inner, out),
            ex::Statement::Break | ex::Statement::Comment(_) => {}
        }
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &ex::Expr, out: &mut Vec<String>) {
    match expr {
        ex::Expr::Call { func, args } => {
            if func.segments().len() == 1 {
                let name = func
                    .segments()
                    .first()
                    .map(|segment| segment.as_str().to_string())
                    .unwrap_or_default();
                if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == &name) {
                    out.push(name);
                }
            }
            for argument in args {
                calls_in_expr(argument, out);
            }
        }
        ex::Expr::List(items) | ex::Expr::Tuple(items) => {
            for item in items {
                calls_in_expr(item, out);
            }
        }
        ex::Expr::Keyword(pairs) => {
            for (_, value) in pairs {
                calls_in_expr(value, out);
            }
        }
        ex::Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                calls_in_expr(value, out);
            }
        }
        ex::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        ex::Expr::Unary { value, .. } | ex::Expr::Paren(value) => calls_in_expr(value, out),
        ex::Expr::Field { value, .. } => calls_in_expr(value, out),
        ex::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        ex::Expr::IfElse {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_expr(then, out);
            calls_in_expr(otherwise, out);
        }
        ex::Expr::Literal(_)
        | ex::Expr::Atom(_)
        | ex::Expr::Var(_)
        | ex::Expr::Attr(_)
        | ex::Expr::Module(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> ex::Expr {
    ex::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<ex::Expr>) -> ex::Expr {
    ex::Expr::Call {
        func: Path::single(callee),
        args,
    }
}

fn assign(place: ex::Expr, value: ex::Expr) -> ex::Statement {
    match place_path(&place) {
        Some((base, path)) if path.is_empty() => ex::Statement::Bind { name: base, value },
        Some((base, path)) => ex::Statement::Update { base, path, value },
        // A place this module cannot decompose becomes a call to a name
        // no module defines: `elixirc` stops on it rather than the
        // value being quietly dropped. The no-panic contract's spelling
        // of "unreachable".
        None => ex::Statement::Bind {
            name: Ident::new("_"),
            value: ex::Expr::Call {
                func: Path::single("zd_refused_not_a_place"),
                args: vec![place, value],
            },
        },
    }
}

/// The base variable and access path of a place expression — the same
/// decomposition `emit/elixir.rs` makes, repeated here because the two
/// modules deliberately do not reach into each other's private helpers.
///
/// zerodds-lint: recursion-depth 64 (place-expression nesting the walk
/// builds). The tree walked here is the one this generator emitted and
/// owns, so it holds no cycle; the depth is that expression's height.
fn place_path(place: &ex::Expr) -> Option<(Ident, Vec<ex::Access>)> {
    match place {
        ex::Expr::Var(name) => Some((name.clone(), Vec::new())),
        ex::Expr::Field { value, name } => {
            let (base, mut path) = place_path(value)?;
            path.push(ex::Access::Key(name.clone()));
            Some((base, path))
        }
        ex::Expr::Index { value, index } => {
            let (base, mut path) = place_path(value)?;
            path.push(ex::Access::At((**index).clone()));
            Some((base, path))
        }
        _ => None,
    }
}
