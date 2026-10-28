// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value an OCaml member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as OCaml. Deliberately a further
//! derivation and not a call into [`crate::default_go`] or any sibling,
//! because the columns are compared against each other and a shared
//! helper would make them agree about a mistake.
//!
//! ## The shape is Go's, with OCaml's spellings
//!
//! A defaulter **returns** Table 9's value and the caller assigns it —
//! the reader opens with `let v = ref (zd_def_T ()) in`, so a position
//! the stream does not carry is already what the clause says it is
//! before an octet is read. What OCaml decides that Go did not have to:
//!
//! - **A record literal names every field.** OCaml has no zero value,
//!   so an `@optional` member is written `f = None` rather than left
//!   out, and a union lists every branch — the selected one as
//!   `Some (zd_def_T ())`, the rest as `None`.
//! - **An enumeration's default is a constructor.** The variant carries
//!   no number, so "the first literal" (§7.3.1.2.1.10) is spelled as
//!   the constructor the adapter declared.
//! - **An array default is `Array.init`,** one defaulter call per
//!   element at run time, so a `long a[200]` is one line and every
//!   element is its own value.
//! - **The literal zeros carry their suffix.** `0`, `0l`, `0L`, `0.0`,
//!   `""`, and the decimal holders' `"0"` and `"0.0"` — OCaml has no
//!   implicit conversion to paper over a missing one.
//!
//! The defaulters are emitted as one `let rec … and …` chain
//! ([`zerodds_lang_cst::ocaml::ValueFn`]) so a recursive aggregate's
//! defaulter can reach its own parts; the bodies are pure value
//! expressions rendered here, because no typing environment is needed
//! for a value with no statements in it.

use crate::adapter::ocaml::{ocaml_enumerator_name, ocaml_value_type};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::ocaml::{kind_of, type_tag};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::Doc;
use zerodds_lang_cst::ocaml::{self, CodeExpr, CodecFn, Stmt, ValueFn, ValueKind};

/// The defaulter function name for a type, keyed the same way the
/// decoder is: by the IDL kind for a primitive and by the codec tag for
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

/// The defaulter that goes with a decoder, by name: `zd_dec_p_long`
/// against `zd_def_p_long`, both built from the same key.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// One member's default: its `@default`, or its type's, assigned into
/// `place`. §7.2.2.4.4.4.9 puts the explicit one first. An `@optional`
/// member's place holds an `option`, so the value is wrapped in `Some`.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as an OCaml expression.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: CodeExpr,
) -> Result<Vec<Stmt>, EncodeError> {
    let mut text = member_value(ast, node, type_)?;
    if node.facets.is_optional() {
        text = format!("(Some {text})");
    }
    Ok(vec![Stmt::Assign {
        place,
        value: CodeExpr::Raw {
            text,
            kind: ValueKind::Aggregate,
        },
    }])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted trees rather than derived a second time from the model, for
/// the reason [`crate::default_c`] gives at length.
///
/// # Errors
/// [`EncodeError`] for a construct with no OCaml default.
pub fn ocaml_defaulters_called_by(
    ast: &Ast,
    decls: &[CodecFn],
) -> Result<Vec<ValueFn>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_fn(decl, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    // Bounded by the number of named defaulters, because every round
    // but the last adds at least one.
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
        let produced = ocaml_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for function in &produced {
            names_in_text(&function.body, &mut called);
        }
    }
    ocaml_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no OCaml default.
pub fn ocaml_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<ValueFn>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let Some(body) = value_of(ast, id, &name)? else {
            continue;
        };
        let result = match ocaml_value_type(ast, resolve_alias(ast, id)) {
            Ok(type_) => crate::emit::ocaml::ocaml_type_text(&type_),
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no OCaml form to return",
                });
            }
        };
        emitted.push(name.clone());
        out.push(ValueFn {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not carry it.",
            ),
            name,
            result,
            body,
        });
    }
    Ok(out)
}

/// The expression one defaulter answers with, or `None` where the type
/// has no OCaml data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<String>, EncodeError> {
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // A type the model does not hold. It used to answer `Ok(None)`,
    // which the caller reads as "nothing to emit for this one" and
    // skips — so a type whose handle no longer resolves quietly loses
    // its defaulter, and the reader that opens with
    // `let v = ref (zd_def_T ()) in` names a function the chain does
    // not define. Every other arm of this function either answers a
    // value or refuses by name; this one now does too, as
    // `default_lua`, `default_elixir` and `default_julia` already did.
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let value = match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind).to_string(),

        // `""`, for both widths: the member holds UTF-8 either way.
        TypeKind::String { .. } => "\"\"".to_string(),

        // The one decimal spelling of zero for the scale.
        TypeKind::Fixed { scale, .. } => format!("\"{}\"", zero_decimal(*scale)),

        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10 — as the constructor.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            ocaml_enumerator_name(ast, id, &enumerator)
        }

        // No bit set, in the holder's own width.
        TypeKind::Bitmask { .. } => {
            if ast.effective_bit_bound(id) <= 32 {
                "0l".to_string()
            } else {
                "0L".to_string()
            }
        }

        // A bitset's fields are separate integers; the packing into the
        // anchor is the codec's.
        TypeKind::Bitset { .. } => {
            let mut fields = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                let zero = match crate::adapter::ocaml::ocaml_holder(field_node.bits) {
                    ocaml::Primitive::Int32 => "0l",
                    ocaml::Primitive::Int64 => "0L",
                    _ => "0",
                };
                fields.push(format!("{} = {zero}", ocaml::lower_ident(field_name)));
            }
            if fields.is_empty() {
                "()".to_string()
            } else {
                format!("{{ {} }}", fields.join("; "))
            }
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => "[||]".to_string(),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." `Array.init` per dimension, one defaulter call per
        // element at run time.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let Ok(length) = ast.eval_integral(*dim) else {
                    return Err(refuse("array whose length does not evaluate"));
                };
                lengths.push(length);
            }
            let mut out = format!("({} ())", defaulter_name(ast, element));
            for length in lengths.into_iter().rev() {
                out = format!("(Array.init {length} (fun _ -> {out}))");
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." OCaml records
        // name every field, so an absent optional is `None` written
        // out.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut fields = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                fields.push(format!(
                    "{} = ({} ())",
                    ocaml::lower_ident("parent"),
                    defaulter_name(ast, base)
                ));
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                let spelled = ocaml::lower_ident(&member_node.name);
                if member_node.facets.is_optional() {
                    fields.push(format!("{spelled} = None"));
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                fields.push(format!(
                    "{spelled} = {}",
                    member_value(ast, member_node, member_type)?
                ));
            }
            if fields.is_empty() {
                // An aggregate with no members is `unit` in this column.
                "()".to_string()
            } else {
                format!("{{ {} }}", fields.join("; "))
            }
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." Every branch is a field here; the unselected
        // ones are `None`.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut fields = vec![format!(
                "_d = {}",
                discriminant_literal(ast, discriminator, selector.unwrap_or(0))
            )];
            let selected = selected_by_default(ast, id, selector)?;
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(refuse("a case the model does not hold"));
                };
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(refuse("a member the model does not hold"));
                };
                let spelled = ocaml::lower_ident(&member.name);
                match &selected {
                    Some((member_type, field)) if *field == spelled => {
                        fields.push(format!(
                            "{spelled} = Some ({} ())",
                            defaulter_name(ast, *member_type)
                        ));
                    }
                    _ => fields.push(format!("{spelled} = None")),
                }
            }
            format!("{{ {} }}", fields.join("; "))
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

/// One member's default as an expression: its `@default`, or a call to
/// its type's defaulter.
fn member_value(ast: &Ast, node: &MemberNode, type_: Id<TypeNode>) -> Result<String, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(format!("({} ())", defaulter_name(ast, type_))),
    }
}

/// Table 9's primitive rows, in the holder each takes here.
const fn primitive_default(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "false",
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => "0.0",
        // The decimal holders: the exact spelling `read_f128` answers
        // for the zero pattern, and a zero `unsigned long long`.
        PrimitiveKind::Float128 => "\"0.0\"",
        PrimitiveKind::UInt64 => "\"0\"",
        PrimitiveKind::Int32 => "0l",
        PrimitiveKind::UInt32 | PrimitiveKind::Int64 => "0L",
        PrimitiveKind::Char
        | PrimitiveKind::WChar
        | PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16 => "0",
    }
}

/// The case a defaulted discriminator selects, if any, by the field's
/// OCaml spelling.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: Option<i128>,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, ocaml::lower_ident(&member.name))))
}

/// A discriminator value in the form its OCaml holder gives it.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return ocaml_enumerator_name(ast, resolved, &enumerator);
        }
    }
    literal_for_kind(value, &kind_of(ast, resolved, 0))
}

/// An integer literal in the holder `kind` reads.
fn literal_for_kind(value: i128, kind: &ValueKind) -> String {
    match kind {
        ValueKind::Bool => (if value != 0 { "true" } else { "false" }).to_string(),
        ValueKind::Int32 => format!("({value}l)"),
        ValueKind::Int64 => format!("({value}L)"),
        ValueKind::Text => format!("\"{value}\""),
        ValueKind::Float => format!("({value}.0)"),
        _ => {
            if value < 0 {
                format!("({value})")
            } else {
                format!("{value}")
            }
        }
    }
}

/// An OCaml string literal of `text`'s bytes.
fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            byte if !(0x20..0x7F).contains(&byte) => out.push_str(&format!("\\x{byte:02x}")),
            byte => out.push(byte as char),
        }
    }
    out.push('"');
    out
}

/// An explicit `@default` as an OCaml expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<String, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let Ok(node) = ast.expr(expr) else {
        return Err(refuse("a `@default` this frontend cannot follow"));
    };
    let resolved = resolve_alias(ast, type_);
    let holder = kind_of(ast, resolved, 0);
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok((if *flag { "true" } else { "false" }).to_string())
        }
        // The code point as the `int` the member holds; a surrogate is
        // a legal `wchar` value here exactly because it is a number.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(format!("{}", u32::from(*ch))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(format!("{code}")),
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(string_literal(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                out.push(ch);
            }
            Ok(string_literal(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = match exponent.as_ref() {
                Some(exponent) => format!("e{exponent}"),
                None => String::new(),
            };
            let written = format!("{mantissa}{exponent}");
            if holder == ValueKind::Text {
                // A binary128 member holds decimal text.
                return Ok(string_literal(&written));
            }
            // An OCaml float literal needs a point or an exponent.
            if written.contains('.') || written.contains('e') {
                Ok(format!("({written})"))
            } else {
                Ok(format!("({written}.0)"))
            }
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(string_literal(&fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let Some(target) = reference.resolved() else {
                return Err(refuse("a `@default` enumerator that is unresolved"));
            };
            let Ok(enumerator) = ast.enumerator(target) else {
                return Err(refuse(
                    "a `@default` enumerator this frontend cannot follow",
                ));
            };
            Ok(ocaml_enumerator_name(ast, resolved, &enumerator.name))
        }
        _ => {
            let Ok(value) = ast.eval_integral(expr) else {
                return Err(refuse("a `@default` this frontend cannot evaluate"));
            };
            match ast.type_(resolved).map(|node| node.kind.clone()) {
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) if !(0..=0xFF).contains(&value) => {
                    Err(refuse("a `@default` character outside one octet"))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) if !(0..=0xFFFF).contains(&value) => {
                    Err(refuse(
                        "a `@default` character outside the range of a UTF-16 code unit",
                    ))
                }
                _ => Ok(literal_for_kind(value, &holder)),
            }
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// Every `zd_def_*` name a codec function reaches, appended to `out`:
/// the calls its tree makes, the seed its reader opens with, and the
/// placeholder expressions its types carry into `Array.make` and `ref`.
fn calls_in_fn(function: &CodecFn, out: &mut Vec<String>) {
    if let Some(seed) = &function.defaulter {
        push_name(seed, out);
    }
    names_in_text(&function.sample.default, out);
    calls_in_body(&function.body, out);
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[Stmt], out: &mut Vec<String>) {
    for statement in body {
        match statement {
            Stmt::Let { type_, value, .. } => {
                names_in_text(&type_.default, out);
                if let Some(value) = value {
                    calls_in_expr(value, out);
                }
            }
            Stmt::Assign { place, value } => {
                calls_in_expr(place, out);
                calls_in_expr(value, out);
            }
            Stmt::Store {
                place,
                type_,
                value,
            } => {
                calls_in_expr(place, out);
                names_in_text(&type_.default, out);
                calls_in_expr(value, out);
            }
            Stmt::ReadInto {
                place, behaviour, ..
            } => {
                calls_in_expr(place, out);
                calls_in_expr(behaviour, out);
            }
            Stmt::Perform(value) | Stmt::Discard(value) => calls_in_expr(value, out),
            Stmt::When {
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
            Stmt::For { bound, body, .. } => {
                calls_in_expr(bound, out);
                calls_in_body(body, out);
            }
            Stmt::While { condition, body } => {
                calls_in_expr(condition, out);
                calls_in_body(body, out);
            }
            Stmt::Alloc { element, count, .. } => {
                names_in_text(&element.default, out);
                calls_in_expr(count, out);
            }
            Stmt::KeepFirst { count, .. } => calls_in_expr(count, out),
            Stmt::Scratch { .. } | Stmt::Return | Stmt::Leave | Stmt::Note(_) => {}
        }
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &CodeExpr, out: &mut Vec<String>) {
    match expr {
        CodeExpr::Call { callee, args } => {
            push_name(callee, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        CodeExpr::Runtime { args, .. } | CodeExpr::Static { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        CodeExpr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        CodeExpr::Unary { value, .. } | CodeExpr::Paren(value) | CodeExpr::OptionGet(value) => {
            calls_in_expr(value, out)
        }
        CodeExpr::Convert { target, value } => {
            names_in_text(&target.default, out);
            calls_in_expr(value, out);
        }
        CodeExpr::Index { base, index } => {
            calls_in_expr(base, out);
            calls_in_expr(index, out);
        }
        CodeExpr::EntryHalf { entry, .. } => calls_in_expr(entry, out),
        CodeExpr::HeaderField { value, .. } => calls_in_expr(value, out),
        CodeExpr::Raw { text, .. } => names_in_text(text, out),
        _ => {}
    }
}

/// The `zd_def_*` identifiers inside a rendered expression — the
/// placeholder defaults are text by the time they reach a tree, so the
/// scan is textual and the identifier grammar is the tokenizer.
fn names_in_text(text: &str, out: &mut Vec<String>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(found) = text[i..].find("zd_def_") {
        let start = i + found;
        // Not a longer identifier's tail.
        let standalone =
            start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if standalone {
            push_name(&text[start..end], out);
        }
        i = end.max(start + 1);
    }
}

fn push_name(name: &str, out: &mut Vec<String>) {
    if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == name) {
        out.push(name.to_string());
    }
}
