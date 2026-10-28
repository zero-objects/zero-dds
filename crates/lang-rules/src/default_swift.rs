// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Swift member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Swift — deliberately a further
//! derivation and not a call into any sibling's, because the columns
//! are compared against each other and a shared helper would make them
//! agree about a mistake.
//!
//! ## The shape is the D column's, with two Swift differences
//!
//! A defaulter **returns** Table 9's value and the caller assigns it;
//! `emit/swift.rs`'s decoder opens with `var v = zd_def_T()`. An
//! aggregate's body is **one memberwise-initialiser call** rather than
//! a declaration and assignments: Swift has no default-initialised
//! struct value to start from — a generated struct's only initialiser
//! is the memberwise one the adapter writes out — so every property is
//! named, `nil` standing where an `@optional` member's absence is the
//! default state.
//!
//! The second difference is the **map entry defaulter**. A reader
//! sizes a map's storage with real entries — Swift has no
//! uninitialised array slot — so beside every `map<K, V>`'s own
//! defaulter (the empty array) this file emits one for its entry
//! struct, keyed by the struct's name: `zd_def_ZdMapEntry_K_V()`.
//! `emit/swift.rs::make_room` and `storage` call it by that name.

use crate::adapter::swift::{
    property_name, swift_map_entry_name, swift_type_name, swift_value_type,
};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::swift::type_tag;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::swift;

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

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

/// The defaulter a map's *entry struct* answers to, keyed by the
/// struct's own name because that is all `make_room` holds.
#[must_use]
pub fn entry_defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    // The same fallback `emit::swift` spells for the declaration, so
    // the defaulter and the entry struct it answers to keep one name
    // even when the tag walk refused.
    let entry =
        swift_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()));
    format!("zd_def_{entry}")
}

/// The defaulter that goes with a decoder, by name.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// One member's default — its `@default`, or its type's — assigned
/// into `place`. §7.2.2.4.4.4.9 puts the explicit one first. An
/// `@external` member's value goes through the `ZdExternal(..)` box;
/// an `@optional` one is assigned bare, `T` promoting to `T?`.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell. Refused rather than silently replaced by
/// Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: swift::Expr,
) -> Result<Vec<swift::Stmt>, EncodeError> {
    let mut value = member_value(ast, node, type_)?;
    let external = crate::adapter::held_externally(ast, node);
    if external {
        value = call("ZdExternal", vec![value]);
    }
    Ok(vec![swift::Stmt::Assign { place, value }])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model.
///
/// # Errors
/// [`EncodeError`] for a construct with no Swift default.
pub fn swift_defaulters_called_by(
    ast: &Ast,
    decls: &[swift::Decl],
) -> Result<Vec<swift::Decl>, EncodeError> {
    let mut index: Vec<(String, Job)> = Vec::new();
    for (id, node) in ast.types() {
        index.push((defaulter_name(ast, id), Job::Type(id)));
        if matches!(node.kind, TypeKind::Map { .. }) {
            index.push((entry_defaulter_name(ast, id), Job::MapEntry(id)));
        }
    }
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_decl(decl, &mut called);
    }
    let mut wanted: Vec<Job> = Vec::new();
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
            // an undefined-identifier error from the Swift compiler
            // naming the function.
            if let Some(job) = index.iter().find(|(n, _)| n == name).map(|(_, job)| *job) {
                wanted.push(job);
                grew = true;
            }
        }
        let produced = swift_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    swift_defaulters_over(ast, &wanted)
}

/// What one defaulter is for: a type, or a map type's entry struct.
#[derive(Debug, Clone, Copy)]
enum Job {
    Type(Id<TypeNode>),
    MapEntry(Id<TypeNode>),
}

/// Build the defaulter for each job, once per name.
fn swift_defaulters_over(ast: &Ast, jobs: &[Job]) -> Result<Vec<swift::Decl>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for job in jobs.iter().copied() {
        match job {
            Job::Type(id) => {
                let name = defaulter_name(ast, id);
                if emitted.contains(&name) {
                    continue;
                }
                let Some(body) = body_of(ast, id, &name)? else {
                    continue;
                };
                let returns = swift_value_type(ast, resolve_alias(ast, id)).map_err(|_| {
                    EncodeError::NoWriter {
                        member: name.clone(),
                        kind: "type with no Swift form to return",
                    }
                })?;
                emitted.push(name.clone());
                out.push(function(&name, returns, body));
            }
            Job::MapEntry(id) => {
                let name = entry_defaulter_name(ast, id);
                if emitted.contains(&name) {
                    continue;
                }
                // Both refused by name. `continue` here would drop the
                // entry defaulter for this map and say nothing — while
                // the very next check, three lines down, refuses an
                // unresolved key or value by name. The three answer the
                // same question about the same type and had two
                // different answers.
                let Ok(node) = ast.type_(id) else {
                    return Err(EncodeError::NoWriter {
                        member: name,
                        kind: "a map type the model does not hold",
                    });
                };
                let TypeKind::Map { key, value, .. } = &node.kind else {
                    return Err(EncodeError::NoWriter {
                        member: name,
                        kind: "a map entry job on a type that is not a map",
                    });
                };
                let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                    return Err(EncodeError::NoWriter {
                        member: name,
                        kind: "map with an unresolved key or value",
                    });
                };
                let entry = swift_map_entry_name(ast, id).map_err(|_| EncodeError::NoWriter {
                    member: name.clone(),
                    kind: "map whose entry name nests past the recursion bound",
                })?;
                let body = vec![swift::Stmt::Return(Some(swift::Expr::Invoke {
                    receiver: None,
                    name: Ident::new(&entry),
                    args: vec![
                        labelled("key", call(&defaulter_name(ast, key), Vec::new())),
                        labelled("value", call(&defaulter_name(ast, value), Vec::new())),
                    ],
                }))];
                emitted.push(name.clone());
                out.push(function(&name, swift::Type::named(&entry), body));
            }
        }
    }
    Ok(out)
}

fn function(name: &str, returns: swift::Type, body: Vec<swift::Stmt>) -> swift::Decl {
    swift::Decl::Func(swift::FuncDecl {
        doc: Doc::line(
            "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not carry it.",
        ),
        // Public for the reason the codec routines are: a consumer in
        // another module reads a sample whose stream did not carry
        // every member, and Table 9's value for the rest is this
        // function's answer.
        visibility: zerodds_lang_cst::common::Visibility::Public,
        name: Ident::new(name),
        params: Vec::new(),
        // Table 9's value is built from literals and other defaulters,
        // never from the wire, so a defaulter never throws.
        throws: false,
        returns: Some(returns),
        body,
    })
}

/// The body one defaulter answers with, or `None` where the type has
/// no Swift data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<swift::Stmt>>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name.
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
    let give = |value: swift::Expr| Some(vec![swift::Stmt::Return(Some(value))]);
    let body = match &node.kind {
        TypeKind::Primitive(kind) => give(primitive_default(*kind)),

        // `""`, for both widths: the member holds a Swift `String`
        // either way.
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
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            give(enumerator_expr(ast, id, &enumerator))
        }

        // No bit set; the `OptionSet` struct enters by its raw value.
        TypeKind::Bitmask { .. } => give(swift::Expr::Invoke {
            receiver: None,
            name: Ident::new(&swift_type_name(ast, id)),
            args: vec![labelled("rawValue", swift::Expr::int(0))],
        }),

        // A bitset is a struct of separate holders, all zero, through
        // the memberwise initialiser.
        TypeKind::Bitset { .. } => {
            let mut args = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                // Refused, not skipped: the memberwise initialiser below
                // is built from exactly these arguments, and a bitfield
                // left out of it is a struct property that keeps whatever
                // the adapter's own property default put there instead of
                // Table 9's zero.
                let Ok(field_node) = ast.bitfield(*field) else {
                    return Err(refuse("a bitfield the model does not hold"));
                };
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                args.push(labelled(&property_name(field_name), swift::Expr::int(0)));
            }
            give(swift::Expr::Invoke {
                receiver: None,
                name: Ident::new(&swift_type_name(ast, id)),
                args,
            })
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries: the empty literal, typed by the
        // return.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => give(swift::Expr::ArrayLit(Vec::new())),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." One call per element, nested literals, outermost
        // dimension outermost.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let Ok(length) = ast.eval_integral(*dim) else {
                    return Err(refuse("array whose length does not evaluate"));
                };
                let Ok(length) = usize::try_from(length) else {
                    return Err(refuse("array of a length this column cannot spell"));
                };
                lengths.push(length);
            }
            let mut out = call(&defaulter_name(ast, element), Vec::new());
            for length in lengths.into_iter().rev() {
                out = swift::Expr::ArrayLit(
                    core::iter::repeat_with(|| out.clone())
                        .take(length)
                        .collect(),
                );
            }
            give(out)
        }

        // "A structure without any of the optional members and with
        // other members set to their default values." One memberwise
        // call: `nil` is the optional members' absence, and every
        // other member states its value.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut args = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                args.push(labelled(
                    &property_name("parent"),
                    call(&defaulter_name(ast, base), Vec::new()),
                ));
            }
            for member_id in ast.members_of(id) {
                // Refused, not skipped: the memberwise initialiser below
                // is built from exactly these arguments, so a member left
                // out of it is a property the defaulter never states.
                let Ok(member_node) = ast.member(*member_id) else {
                    return Err(refuse("a member the model does not hold"));
                };
                let label = property_name(&member_node.name);
                if member_node.facets.is_optional() {
                    args.push(labelled(&label, swift::Expr::Literal(Literal::Null)));
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                let mut value = member_value(ast, member_node, member_type)?;
                if crate::adapter::held_externally(ast, member_node) {
                    value = call("ZdExternal", vec![value]);
                }
                args.push(labelled(&label, value));
            }
            give(swift::Expr::Invoke {
                receiver: None,
                name: Ident::new(&swift_type_name(ast, id)),
                args,
            })
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." One memberwise call: `_d`, then one argument
        // per branch, `nil` for every unselected one.
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
            let selected = selected_by_default(ast, id, selector)?;
            let mut args = vec![labelled(
                "_d",
                discriminant_literal(ast, discriminator, selector),
            )];
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(refuse("a case the model does not hold"));
                };
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(refuse("a member the model does not hold"));
                };
                let label = property_name(&member.name);
                let is_selected = selected.as_ref().is_some_and(|(_, field)| *field == label);
                if !is_selected {
                    args.push(labelled(&label, swift::Expr::Literal(Literal::Null)));
                    continue;
                }
                let Some(member_type) = member.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member.name.clone(),
                        kind: "union case with an unresolved type",
                    });
                };
                let mut value = call(&defaulter_name(ast, member_type), Vec::new());
                if crate::adapter::held_externally(ast, member) {
                    value = call("ZdExternal", vec![value]);
                }
                args.push(labelled(&label, value));
            }
            give(swift::Expr::Invoke {
                receiver: None,
                name: Ident::new(&swift_type_name(ast, id)),
                args,
            })
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

/// One member's default as a value: its `@default`, or a call to its
/// type's defaulter.
fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<swift::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, as the Swift holder each takes. The
/// integers and both character widths are integer zeros — the member
/// holds the code unit — and the floats are `0.0`. A binary128 zero is
/// the decimal `"0.0"`, which is what `readF128` answers for the zero
/// pattern.
fn primitive_default(kind: PrimitiveKind) -> swift::Expr {
    match kind {
        PrimitiveKind::Boolean => swift::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            swift::Expr::Literal(Literal::Float("0.0".to_string()))
        }
        PrimitiveKind::Float128 => text("0.0"),
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
        | PrimitiveKind::UInt64 => swift::Expr::int(0),
    }
}

/// An enumerator, in the enum's own scope: `Mode.IDLE`.
fn enumerator_expr(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> swift::Expr {
    swift::Expr::Member {
        value: Box::new(swift::Expr::Name(Ident::new(&swift_type_name(ast, id)))),
        name: Ident::new(&property_name(enumerator)),
    }
}

/// The case a defaulted discriminator selects, if any, by its Swift
/// property name.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: i128,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, property_name(&member.name))))
}

/// A discriminator value in the Swift form its type gives it: the
/// enumerator for an enumeration, `true`/`false` for a `boolean`, and
/// the number for everything else — a `char` and a `wchar`
/// discriminator hold their code unit as an integer here.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> swift::Expr {
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
        Some(PrimitiveKind::Boolean) => swift::Expr::Literal(Literal::Bool(value != 0)),
        _ => swift::Expr::int(value),
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

/// An explicit `@default` as a Swift expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<swift::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let Ok(node) = ast.expr(expr) else {
        return Err(refuse("a `@default` this frontend cannot follow"));
    };
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    let is_float128 = matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128)));
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(swift::Expr::Literal(Literal::Bool(*flag)))
        }
        // The code unit as the number the member holds: `UInt8` for a
        // `char`, `UInt16` for a `wchar`.
        ConstExpr::Literal(AstLiteral::Char(ch)) => {
            Ok(swift::Expr::int(i128::from(u32::from(*ch))))
        }
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(swift::Expr::int(i128::from(*code))),
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
            let exponent = match exponent.as_ref() {
                Some(exponent) => format!("e{exponent}"),
                None => String::new(),
            };
            let written = format!("{mantissa}{exponent}");
            if is_float128 {
                // The member holds decimal text and the runtime parses
                // it exactly.
                return Ok(text(&written));
            }
            Ok(swift::Expr::Literal(Literal::Float(written)))
        }
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(text(&fixed_text(digits, *scale)))
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
            Ok(enumerator_expr(ast, resolved, &enumerator.name))
        }
        _ => {
            let Ok(value) = ast.eval_integral(expr) else {
                return Err(refuse("a `@default` this frontend cannot evaluate"));
            };
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    swift::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    swift::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    swift::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&value.to_string()),
                _ => swift::Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &swift::Decl, out: &mut Vec<String>) {
    match decl {
        swift::Decl::Func(func) => calls_in_body(&func.body, out),
        swift::Decl::Constant(constant) => calls_in_expr(&constant.value, out),
        swift::Decl::Aggregate(node) => {
            for static_ in &node.statics {
                calls_in_expr(&static_.value, out);
            }
        }
        swift::Decl::Enum(_)
        | swift::Decl::TypeAlias(_)
        | swift::Decl::Protocol(_)
        | swift::Decl::Comment(_)
        | swift::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds)
fn calls_in_body(body: &[swift::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds)
fn calls_in_stmt(statement: &swift::Stmt, out: &mut Vec<String>) {
    match statement {
        swift::Stmt::AssignProperty { value, .. } => calls_in_expr(value, out),
        swift::Stmt::Var { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        swift::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        swift::Stmt::Expr(value) => calls_in_expr(value, out),
        swift::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        swift::Stmt::If {
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
        swift::Stmt::ForRange { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        swift::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        swift::Stmt::Break | swift::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds)
fn calls_in_expr(expr: &swift::Expr, out: &mut Vec<String>) {
    match expr {
        swift::Expr::Invoke {
            receiver,
            name,
            args,
        } => {
            // Unqualified only: a defaulter is a file-level function of
            // the generated program, and this generator built every
            // call to one with its own naming.
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
                calls_in_expr(&arg.value, out);
            }
        }
        swift::Expr::Call { args, .. } => {
            for arg in args {
                calls_in_expr(&arg.value, out);
            }
        }
        swift::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        swift::Expr::Paren(value)
        | swift::Expr::Unary { value, .. }
        | swift::Expr::ForceUnwrap(value)
        | swift::Expr::Try(value)
        | swift::Expr::Member { value, .. } => calls_in_expr(value, out),
        swift::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        swift::Expr::ArrayLit(elements) => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        swift::Expr::RepeatingArray {
            repeated, count, ..
        } => {
            calls_in_expr(repeated, out);
            calls_in_expr(count, out);
        }
        swift::Expr::Literal(_) | swift::Expr::Name(_) | swift::Expr::TypeSelf(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> swift::Expr {
    swift::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<swift::Expr>) -> swift::Expr {
    swift::Expr::Invoke {
        receiver: None,
        name: Ident::new(callee),
        args: args
            .into_iter()
            .map(|value| swift::Argument { label: None, value })
            .collect(),
    }
}

fn labelled(label: &str, value: swift::Expr) -> swift::Argument {
    swift::Argument {
        label: Some(Ident::new(label)),
        value,
    }
}
