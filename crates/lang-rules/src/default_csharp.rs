// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a C# member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as C#. [`crate::default_c`] is the same
//! table as C, [`crate::default_java`] as Java and
//! `crates/wire-probe/src/default.rs` as the reflective column; that
//! module carries the clause quotations and the reasoning for the three
//! kinds Table 9 does not list. Deliberately a fifth derivation and not
//! a call into any of them: the columns are compared against each
//! other, and a shared helper would make them agree about a mistake.
//!
//! ## Why it is a generated method and not a literal
//!
//! A literal cannot carry an aggregate. "A structure without any of the
//! optional members and with other members set to their default values"
//! is a `new S()` and one assignment per member, and writing that at
//! every position a member can go missing would put the same answer in
//! the generated file as many times as the type is reached — where two
//! copies can differ.
//!
//! ## Why it answers with the value instead of filling one
//!
//! C's defaulter is `void zd_def_T(T *v)`: it writes through a pointer.
//! C# has `ref` and `out` and neither is usable at the call sites the
//! walk produces — a property cannot be passed as either (CS0206), and
//! a `Nullable<T>` member is not an `out T`. So the method **returns**
//! Table 9's value and the caller assigns it, which is the same decision
//! one step later, and it is what makes the reader's shape work: every
//! generated decoder opens with `T v = zd_def_T();`, so a position the
//! stream does not carry is already what Table 9 says before a single
//! octet is read.
//!
//! ## What C# settles that Java could not
//!
//! Every zero row of Table 9 is spelled by the member's own width,
//! because the mapping is one to one — an `octet` is a `byte` and an
//! `unsigned long` is a `uint`, where the Java column has to write both
//! as the next signed type up. What is left is the two rows C# spells
//! differently from every other column: a `float` needs its `f` suffix
//! because a real literal is a `double` and there is no implicit
//! narrowing even for a constant, and an enumeration's default is a real
//! `enum` member rather than an `int`.
//!
//! A `sequence<T>` is a `T[]` and its "zero-length sequence" is
//! `new T[0]` and not a null: null is what an `@optional` member's
//! absence means here, and one value cannot mean both. §7.2.2.4.4.4.7
//! asks for a zero-length sequence, and §7.2.2.4.4.4.8's
//! "Implementations shall not provide a default value" for an
//! `@optional` member is the null.

use crate::adapter::csharp::{csharp_member_type, declared_path, field_name, type_tag};
use crate::adapter::cst::MaterialiseError;
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::csharp::CSharp;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::csharp;

/// The defaulter method name for a type.
///
/// Keyed the same way [`crate::emit::decode::decoder_name`] is, so a
/// primitive's defaulter is named by its IDL kind: `double` and
/// `long double` share no C# type at all, and `char` and `wchar` share
/// one.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!(
            "zd_def_{}",
            type_tag(ast, id, 0).unwrap_or_else(|_| format!("unresolved_type_{}", id.slot()))
        ),
    }
}

/// The defaulter that goes with a decoder, by name.
///
/// The two schemes differ in one word — `zd_dec_p_long` against
/// `zd_def_p_long` — because both are built from the same key. Doing it
/// by text lets `crate::emit::csharp::CSharp::routine` name the
/// defaulter without carrying the model into a method that has only a
/// signature; a name that does not resolve is a `csc` error naming the
/// method.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_<T>();`
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: csharp::Expr) -> csharp::Statement {
    assign(place, call(&defaulter_name(ast, id), Vec::new()))
}

/// One member's default: its `@default`, or its type's.
///
/// §7.2.2.4.4.4.9 puts the explicit one first — "An explicit definition
/// associates a default value with the member at the time where the
/// aggregated type is defined" — and falls back to "the default value of
/// the member type. See Table 9."
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a C# expression. Refused rather than silently
/// replaced by Table 9's zero: a member the designer gave a default and
/// a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: csharp::Expr,
) -> Result<Vec<csharp::Statement>, EncodeError> {
    let Some(expr) = node.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(vec![default_into(ast, type_, place)]);
    };
    let value = explicit(ast, expr, type_, &node.name)?;
    Ok(vec![assign(place, value)])
}

/// The defaulters `members` call, and exactly those.
///
/// The set is read **off the emitted calls** rather than derived a
/// second time from the model, for the reason
/// [`crate::default_c::c_defaulters_called_by`] gives at length: a
/// derivation beside the decoder's is a second reading of §7.2.2.7, and
/// the two readings drift. The closure is taken by emitting and
/// re-reading until the batch calls nothing it does not also define.
///
/// # Errors
/// [`EncodeError`] for a construct with no C# default.
pub fn csharp_defaulters_called_by(
    ast: &Ast,
    members: &[csharp::Member],
) -> Result<Vec<csharp::Member>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for member in members {
        calls_in_member(member, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    // Bounded by the number of named defaulters, because every round
    // but the last adds at least one. Written down rather than left to
    // the loop, so a name that never resolves cannot spin.
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
        let produced = csharp_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for member in &produced {
            calls_in_member(member, &mut called);
        }
    }
    csharp_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no C# default.
pub fn csharp_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<csharp::Member>, EncodeError> {
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
        out.push(csharp::Member::Method(csharp::Method {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            attributes: Vec::new(),
            modifiers: vec![csharp::Modifier::Private, csharp::Modifier::Static],
            returns: value_type(ast, id),
            name: Ident::new(&name),
            generics: Vec::new(),
            params: Vec::new(),
            constraints: Vec::new(),
            body: Some(csharp::Block { statements: body }),
            expression: None,
        }));
    }
    Ok(out)
}

/// The C# type a defaulter answers with.
fn value_type(ast: &Ast, id: Id<TypeNode>) -> csharp::Type {
    csharp_member_type(ast, id, "default").unwrap_or_else(|_| {
        csharp::Type::named(Path::new(
            &[format!("UnresolvedType{}", id.slot()).as_str()],
        ))
    })
}

/// The statements of one defaulter, or `None` where the type has no C#
/// data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<csharp::Statement>>, EncodeError> {
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

        // `""`, for both widths: a .NET `string` is UTF-16 already and
        // the empty string is the same string in either.
        TypeKind::String { .. } => vec![give_back(text(""))],

        // A `fixed<P,S>` is the decimal it was written as, and one
        // scale has exactly one spelling of zero.
        TypeKind::Fixed { scale, .. } => vec![give_back(text(&zero_decimal(*scale)))],

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(refuse("typedef with an unresolved target"));
            };
            vec![give_back(call(&defaulter_name(ast, target), Vec::new()))]
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            vec![give_back(enumerator_constant(ast, id, &enumerator))]
        }

        // No bit set: the holder's Table 9 default is zero and zero sets
        // none. A cast and not a bare `0`: a C# `[Flags]` enum has no
        // implicit conversion from an integer, and `default` would be
        // the same value written in a way that does not say which type
        // it is.
        TypeKind::Bitmask { .. } => vec![give_back(csharp::Expr::Cast {
            type_: Box::new(value_type(ast, id)),
            value: Box::new(csharp::Expr::Literal(Literal::int(0))),
        })],

        TypeKind::Bitset { .. } => {
            let mut statements = vec![new_local(value_type(ast, id))];
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding and holds no value.
                if let Some(name) = field_node.name.as_ref() {
                    statements.push(assign(
                        member("v", &field_name(name)),
                        csharp::Expr::Literal(Literal::int(0)),
                    ));
                }
            }
            statements.push(give_back(name("v")));
            statements
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => {
            let csharp::Type::Array { .. } = value_type(ast, id) else {
                return Err(refuse("collection whose C# form is not an array"));
            };
            let (element, nested) = CSharp::peel(value_type(ast, id));
            vec![give_back(csharp::Expr::NewArray {
                element: Box::new(element),
                length: Some(Box::new(csharp::Expr::Literal(Literal::int(0)))),
                elements: Vec::new(),
                nested: nested.saturating_sub(1),
            })]
        }

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." A C# multi-rank IDL array is jagged — one `[]` per
        // dimension — so every rank but the outermost is allocated
        // inside the loop above it. `new T[a][b]` is not an expression
        // C# has.
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
            // `total` is every `[]` the C# type carries and `ranks` is
            // only the IDL array's own: `long rows[3]` where the element
            // is a `sequence<long>` is an `int[][]`, so the outermost
            // allocation is `new int[3][]` and the element's own rank
            // comes from the element and not from the dimension list.
            // Counting `ranks` alone wrote `new int[3]` into an
            // `int[][]` and `csc` reported it — four corpus cases,
            // `array-1d-sequence` and `array-1d-map` with their
            // unannotated twins.
            let (innermost, total) = CSharp::peel(value_type(ast, id));
            let ranks = lengths.len();
            let mut statements = vec![csharp::Statement::Declare {
                type_: value_type(ast, id),
                name: Ident::new("v"),
                value: Some(csharp::Expr::NewArray {
                    element: Box::new(innermost.clone()),
                    length: Some(Box::new(csharp::Expr::Literal(Literal::int(lengths[0])))),
                    elements: Vec::new(),
                    nested: total - 1,
                }),
            }];

            // The body, built innermost first: at the deepest level the
            // element takes Table 9's value, and at every level above it
            // the next rank is allocated before the loop that fills it.
            let place = |depth: usize| {
                let mut access = name("v");
                for level in 0..=depth {
                    access = csharp::Expr::Index {
                        value: Box::new(access),
                        index: Box::new(name(&format!("zd_i{level}"))),
                    };
                }
                access
            };
            let mut inner = vec![default_into(ast, element, place(ranks - 1))];
            for level in (0..ranks).rev() {
                let mut body = Vec::new();
                if level + 1 < ranks {
                    body.push(assign(
                        place(level),
                        csharp::Expr::NewArray {
                            element: Box::new(innermost.clone()),
                            length: Some(Box::new(csharp::Expr::Literal(Literal::int(
                                lengths[level + 1],
                            )))),
                            elements: Vec::new(),
                            nested: total - level - 2,
                        },
                    ));
                }
                body.extend(inner);
                inner = vec![counted_loop(
                    &format!("zd_i{level}"),
                    csharp::Expr::Literal(Literal::int(lengths[level])),
                    csharp::Block { statements: body },
                )];
            }
            statements.extend(inner);
            statements.push(give_back(name("v")));
            statements
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = vec![new_local(value_type(ast, id))];
            for member_id in &ast.all_members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                let field = field_name(&member_node.name);
                if member_node.facets.is_optional() {
                    // "Implementations shall not provide a default value
                    // in such a case" — which here is the null a fresh
                    // reference or a fresh `Nullable<T>` already holds.
                    // Written out anyway, so a defaulter run over a
                    // sample that has been read once before clears it.
                    statements.push(assign(
                        member("v", &field),
                        csharp::Expr::Literal(Literal::Null),
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
        // selected member is also set to the default value for the
        // member type."
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let mut statements = vec![
                new_local(value_type(ast, id)),
                default_into(ast, discriminator, member("v", &field_name("_d"))),
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
        // Refused by name rather than passed over. `any` is in the model
        // because IDL 4.2 has it; DDS-XTypes has no `any` and no
        // encoding for one, and only CORBA CDR writes it — as a TypeCode
        // ahead of the value. Returning `Ok(None)` here would generate a
        // reader that silently omitted the member.
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

/// Table 9's primitive rows, as the C# holder each takes.
///
/// One to one with the IDL width, which is the difference from the Java
/// column: nothing is widened and nothing is a pattern. The two that are
/// not a bare `0` are the ones C# will not convert for you — a `float`
/// needs its suffix, and a `char` is a character.
fn primitive_default(kind: PrimitiveKind) -> csharp::Expr {
    match kind {
        PrimitiveKind::Boolean => csharp::Expr::Literal(Literal::Bool(false)),
        // `'\0'`, written as the character it is: a bare `0` is an `int`
        // and C# has no implicit conversion from one to a `char`.
        PrimitiveKind::Char | PrimitiveKind::WChar => csharp::Expr::Literal(Literal::Char('\0')),
        // A real literal is a `double`, and there is no implicit
        // narrowing to `float` even for a constant.
        PrimitiveKind::Float32 => csharp::Expr::Literal(Literal::Float("0.0f".to_string())),
        PrimitiveKind::Float64 => csharp::Expr::Literal(Literal::Float("0.0".to_string())),
        // A binary128 zero is its zero bit pattern, and `Float128.Zero`
        // names it so the spelling does not depend on the struct layout.
        PrimitiveKind::Float128 => csharp::Expr::Member {
            value: Box::new(csharp::Expr::Path(Path::rooted(&[
                "ZeroDds", "Idl", "Float128",
            ]))),
            name: Ident::new("Zero"),
        },
        // Every integer width: ECMA-334 §6.4.5.3's implicit constant
        // expression conversion carries an in-range `int` constant into
        // every one of them, so the decimal alone lands in the declared
        // type.
        PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::Int32
        | PrimitiveKind::Int64
        | PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::UInt16
        | PrimitiveKind::UInt32
        | PrimitiveKind::UInt64 => csharp::Expr::Literal(Literal::int(0)),
    }
}

/// `global::<modules>.<Enum>.<LITERAL>` — the enumeration the C#
/// materialiser declares, written from the global namespace down.
pub(crate) fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> csharp::Expr {
    let owner = declared_path(ast, id).unwrap_or_else(|_| {
        // A path the module chain will not answer for is a name `csc`
        // reports; a bare constant would compile against whatever else
        // happened to be in scope.
        format!("UnresolvedEnum{}", id.slot())
    });
    // The enumerator's own name is verbatim but for keyword escaping:
    // `adapter/csharp.rs` folds a *member* and leaves a type's and an
    // enumerator's name alone, because a type's name reaches the wire.
    let name = zerodds_lang_cst::csharp::escape_keyword(enumerator);
    csharp::Expr::Member {
        value: Box::new(path(&owner)),
        name: Ident::new(&name),
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
        .map(|(member, member_type)| (member_type, field_name(&member.name))))
}

/// An explicit `@default` as a C# expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<csharp::Expr, EncodeError> {
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
            Ok(csharp::Expr::Literal(Literal::Bool(*flag)))
        }
        ConstExpr::Literal(AstLiteral::Char(ch)) => char_literal(u32::from(*ch), refuse),
        ConstExpr::Literal(AstLiteral::WChar(code)) => char_literal(*code, refuse),
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
            Ok(csharp::Expr::Literal(Literal::Float(format!(
                "{mantissa}{exponent}{suffix}"
            ))))
        }
        // A `fixed<P,S>` is the decimal text here, as the member's type
        // is a `string`.
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
            // An integer literal, a named constant or arithmetic over
            // them, folded the same way a bound or a case label is.
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    // `try_from` used to fold to zero here, so a negative
                    // `@default` on a character member became NUL with no
                    // diagnostic — the same loss the surrogate case had,
                    // reached from the other side of the range.
                    let code = u32::try_from(value).map_err(|_| {
                        refuse("a `@default` character outside the range of a code unit")
                    })?;
                    char_literal(code, refuse)?
                }
                // An enumerated `@default` given as a number: the cast
                // is what makes it a value of the enumeration, and C#
                // admits every value of the underlying type.
                Ok(TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. }) => csharp::Expr::Cast {
                    type_: Box::new(value_type(ast, resolved)),
                    value: Box::new(csharp::Expr::Literal(Literal::int(value))),
                },
                _ => csharp::Expr::Literal(Literal::int(value)),
            })
        }
    }
}

/// `(char) <code unit>` — a code unit and not a code point.
///
/// A C# `char` is one UTF-16 code unit and holds every value up to
/// `0xFFFF`, a lone surrogate among them; Rust's `char` holds only
/// Unicode scalar values and rejects `0xD800..=0xDFFF`. Routing the code
/// through `char::from_u32` therefore loses exactly the values the two
/// types disagree about, and this function used to answer that loss with
/// `'\0'` on the argument that the frontend had already refused the
/// declaration. It has not: `wireprobe-zerodds-csharp-gen` takes
/// `@default(0xD800) wchar c` with exit 0 and wrote `v.C = '\0';`, where
/// the C column wrote `55296` and the Java column `(char) 55296`.
///
/// The cast is what the Java backend has always emitted and needs no
/// Rust `char` at all. Above `0xFFFF` there is no single C# `char`, and
/// that is refused by name rather than folded.
fn char_literal(
    code: u32,
    refuse: impl Fn(&'static str) -> EncodeError,
) -> Result<csharp::Expr, EncodeError> {
    if code > u32::from(u16::MAX) {
        return Err(refuse(
            "a `@default` character above the Basic Multilingual Plane, \
             which no single C# `char` holds",
        ));
    }
    Ok(csharp::Expr::Cast {
        type_: Box::new(csharp::Type::Predefined(csharp::Predefined::Char)),
        value: Box::new(csharp::Expr::Literal(Literal::int(i128::from(code)))),
    })
}

// -- reading the calls back out ----------------------------------------

fn calls_in_member(member: &csharp::Member, out: &mut Vec<String>) {
    match member {
        csharp::Member::Method(method) => {
            if let Some(body) = &method.body {
                calls_in_block(body, out);
            }
            if let Some(value) = &method.expression {
                calls_in_expr(value, out);
            }
        }
        csharp::Member::Field(field) => {
            if let Some(value) = &field.value {
                calls_in_expr(value, out);
            }
        }
        csharp::Member::Property(property) => {
            if let Some(value) = &property.expression {
                calls_in_expr(value, out);
            }
            if let Some(value) = &property.initialiser {
                calls_in_expr(value, out);
            }
        }
        csharp::Member::Constructor(constructor) => calls_in_block(&constructor.body, out),
        csharp::Member::Nested(_) | csharp::Member::Comment(_) | csharp::Member::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_block(block: &csharp::Block, out: &mut Vec<String>) {
    for statement in &block.statements {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &csharp::Statement, out: &mut Vec<String>) {
    match statement {
        csharp::Statement::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        csharp::Statement::Expr(expr) | csharp::Statement::Throw(expr) => {
            calls_in_expr(expr, out);
        }
        csharp::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        csharp::Statement::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_block(then, out);
            if let Some(otherwise) = otherwise {
                calls_in_block(otherwise, out);
            }
        }
        csharp::Statement::ForEach { range, body, .. } => {
            calls_in_expr(range, out);
            calls_in_block(body, out);
        }
        csharp::Statement::For {
            init,
            condition,
            step,
            body,
        } => {
            calls_in_statement(init, out);
            calls_in_expr(condition, out);
            calls_in_expr(step, out);
            calls_in_block(body, out);
        }
        csharp::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_block(body, out);
        }
        csharp::Statement::Switch { value, sections } => {
            calls_in_expr(value, out);
            for section in sections {
                for label in &section.labels {
                    calls_in_expr(label, out);
                }
                calls_in_block(&section.body, out);
            }
        }
        csharp::Statement::Using { resource, body } => {
            calls_in_statement(resource, out);
            calls_in_block(body, out);
        }
        csharp::Statement::Block(block) => calls_in_block(block, out),
        csharp::Statement::Break | csharp::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
#[allow(clippy::too_many_lines)]
fn calls_in_expr(expr: &csharp::Expr, out: &mut Vec<String>) {
    match expr {
        csharp::Expr::Invoke {
            receiver,
            name,
            args,
        } => {
            // Unqualified only: a defaulter is a static method of the
            // generated class and this generator built every call to one
            // with `defaulter_name`.
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
        csharp::Expr::Member { value, .. }
        | csharp::Expr::Cast { value, .. }
        | csharp::Expr::Is { value, .. }
        | csharp::Expr::Unary { value, .. }
        | csharp::Expr::NameOf(value)
        | csharp::Expr::Paren(value) => calls_in_expr(value, out),
        csharp::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        csharp::Expr::New {
            args, initialiser, ..
        } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
            for (_, value) in initialiser {
                calls_in_expr(value, out);
            }
        }
        csharp::Expr::NewArray {
            length, elements, ..
        } => {
            if let Some(length) = length {
                calls_in_expr(length, out);
            }
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        csharp::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        csharp::Expr::Assign { target, value } => {
            calls_in_expr(target, out);
            calls_in_expr(value, out);
        }
        csharp::Expr::Conditional {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_expr(then, out);
            calls_in_expr(otherwise, out);
        }
        csharp::Expr::Lambda { body, .. } => calls_in_expr(body, out),
        csharp::Expr::Literal(_)
        | csharp::Expr::Path(_)
        | csharp::Expr::This
        | csharp::Expr::Default(_)
        | csharp::Expr::TypeOf(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn path(text: &str) -> csharp::Expr {
    csharp::Expr::Path(match text.strip_prefix("global::") {
        Some(rest) => Path::rooted(&rest.split('.').collect::<Vec<_>>()),
        None => Path::new(&text.split('.').collect::<Vec<_>>()),
    })
}

fn name(text: &str) -> csharp::Expr {
    path(text)
}

fn text(value: &str) -> csharp::Expr {
    csharp::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<csharp::Expr>) -> csharp::Expr {
    csharp::Expr::Invoke {
        receiver: None,
        name: Ident::new(callee),
        args,
    }
}

fn member(base: &str, field: &str) -> csharp::Expr {
    csharp::Expr::Member {
        value: Box::new(name(base)),
        name: Ident::new(field),
    }
}

fn assign(place: csharp::Expr, value: csharp::Expr) -> csharp::Statement {
    csharp::Statement::Expr(csharp::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(value),
    })
}

fn give_back(value: csharp::Expr) -> csharp::Statement {
    csharp::Statement::Return(Some(value))
}

/// `<T> v = new <T>();`
fn new_local(type_: csharp::Type) -> csharp::Statement {
    csharp::Statement::Declare {
        type_: type_.clone(),
        name: Ident::new("v"),
        value: Some(csharp::Expr::New {
            type_: Box::new(type_),
            args: Vec::new(),
            initialiser: Vec::new(),
        }),
    }
}

fn counted_loop(index: &str, count: csharp::Expr, body: csharp::Block) -> csharp::Statement {
    csharp::Statement::For {
        init: Box::new(csharp::Statement::Declare {
            type_: csharp::Type::Predefined(csharp::Predefined::Int),
            name: Ident::new(index),
            value: Some(csharp::Expr::Literal(Literal::int(0))),
        }),
        condition: csharp::Expr::Binary {
            op: "<",
            lhs: Box::new(name(index)),
            rhs: Box::new(count),
        },
        step: csharp::Expr::Binary {
            op: "=",
            lhs: Box::new(name(index)),
            rhs: Box::new(csharp::Expr::Binary {
                op: "+",
                lhs: Box::new(name(index)),
                rhs: Box::new(csharp::Expr::Literal(Literal::int(1))),
            }),
        },
        body,
    }
}

/// Kept so the module's one dependency on the materialiser's error type
/// is stated rather than inferred.
const _: Option<MaterialiseError> = None;
