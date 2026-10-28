// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Java member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Java. [`crate::default_c`] is the same
//! table as C and `crates/wire-probe/src/default.rs` is the reflective
//! column's; that module carries the clause quotations and the reasoning
//! for the three kinds Table 9 does not list. Deliberately a third
//! derivation and not a call into either: the columns are compared
//! against each other, and a shared helper would make them agree about a
//! mistake.
//!
//! ## Why it is a generated method and not a literal
//!
//! A literal cannot carry an aggregate. "A structure without any of the
//! optional members and with other members set to their default values"
//! is a `new S()` and one assignment per member, and writing that at
//! every position a member can go missing would put the same answer in
//! the generated file as many times as the type is reached — where two
//! copies can differ. One method per type is one answer per type, which
//! is what the C column does and for the same reason.
//!
//! ## Why it answers with the value instead of filling one
//!
//! C's defaulter is `void zd_def_T(T *v)`: it writes through a pointer.
//! Java has no pointer and no out-parameter, and half of Table 9's rows
//! are values Java cannot mutate at all — a primitive, a `String`, an
//! array whose length has to change. So the method **returns** Table 9's
//! value and the caller assigns it, which is the same decision one step
//! later. It is also what makes the reader's own shape work: every
//! generated decoder opens with `T v = zd_def_T();`, so a position the
//! stream does not carry is already what Table 9 says before a single
//! octet is read.
//!
//! ## What Java forces that C does not
//!
//! `octet` is an `int` and `unsigned long` is a `long`, so the zero rows
//! of Table 9 are spelled by the *holder* and not by the IDL width —
//! `adapter/java.rs` has the table. A `sequence<T>` is a `T[]` and its
//! "zero-length sequence" is `new T[0]` and not a null: null is what an
//! `@optional` member's absence means here, and one value cannot mean
//! both. §7.2.2.4.4.4.7 asks for a zero-length sequence, and
//! §7.2.2.4.4.4.8's "Implementations shall not provide a default value"
//! for an `@optional` member is the null.

use crate::adapter::cst::MaterialiseError;
use crate::adapter::java::{declared_path, java_member_type, type_tag, unsigned_holder};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::java;

/// The defaulter method name for a type.
///
/// Keyed the same way [`crate::emit::decode::decoder_name`] is, so a
/// primitive's defaulter is named by its IDL kind: `double` and
/// `long double` share no Java type at all, and `octet` and
/// `unsigned short` share one.
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
/// by text lets `crate::emit::java::Java::routine` name the defaulter
/// without carrying the model into a method that has only a signature;
/// a name that does not resolve is a `javac` error naming the method.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_<T>();`
///
/// # Errors
/// Never; the signature matches the emitters around it.
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: java::Expr) -> java::Statement {
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
/// column cannot spell as a Java expression. Refused rather than
/// silently replaced by Table 9's zero: a member the designer gave a
/// default and a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: java::Expr,
) -> Result<Vec<java::Statement>, EncodeError> {
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
/// the two readings drift. Here the seed set is larger than C's —
/// every generated reader opens with its own type's defaulter — and the
/// closure is taken the same way, by emitting and re-reading until the
/// batch calls nothing it does not also define.
///
/// # Errors
/// [`EncodeError`] for a construct with no Java default.
pub fn java_defaulters_called_by(
    ast: &Ast,
    members: &[java::Member],
) -> Result<Vec<java::Member>, EncodeError> {
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
            // A name with no type behind it cannot happen for a call
            // this generator emitted — every one came from
            // `defaulter_name` over a type in this model — and if it
            // ever does, the omission is a `javac` error naming the
            // method, which is louder than anything this could return.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = java_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for member in &produced {
            calls_in_member(member, &mut called);
        }
    }
    java_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Java default.
pub fn java_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<java::Member>, EncodeError> {
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
        out.push(java::Member::Method(java::Method {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            annotations: Vec::new(),
            // Package-private: the function-call classes of an interface
            // (`ZdServices`) take the default of a `@Mutable` member a peer
            // did not send from here (XTypes 1.3 §7.2.4.4.8).
            modifiers: vec![java::Modifier::Static],
            generics: Vec::new(),
            returns: value_type(ast, id),
            name: Ident::new(&name),
            params: Vec::new(),
            throws: Vec::new(),
            body: Some(java::Block { statements: body }),
        }));
    }
    Ok(out)
}

/// The Java type a defaulter answers with.
fn value_type(ast: &Ast, id: Id<TypeNode>) -> java::Type {
    java_member_type(ast, id, "default").unwrap_or_else(|_| {
        java::Type::named(Path::new(
            &[format!("UnresolvedType{}", id.slot()).as_str()],
        ))
    })
}

/// The statements of one defaulter, or `None` where the type has no
/// Java data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<java::Statement>>, EncodeError> {
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

        // `""`, for both widths: a Java `String` is UTF-16 already and
        // the empty string is the same string in either.
        TypeKind::String { .. } => vec![give_back(text(""))],

        // A `fixed<P,S>` is the decimal it was written as, and
        // `canonical::decimal` accepts exactly one spelling of zero for
        // a given scale.
        TypeKind::Fixed { scale, .. } => {
            vec![give_back(big_decimal(&zero_decimal(*scale)))]
        }

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

        // No bit set: the holder's Table 9 default is zero and zero
        // sets none. The holder decides the suffix — a `@bit_bound(40)`
        // mask is a `long`.
        TypeKind::Bitmask { .. } => {
            let holder = unsigned_holder(ast.effective_bit_bound(id));
            vec![give_back(zero_of(&holder))]
        }

        TypeKind::Bitset { .. } => {
            let mut statements = vec![new_local(value_type(ast, id))];
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding and holds no value.
                if let Some(field_name) = field_node.name.as_ref() {
                    statements.push(assign(
                        member("v", &java::escape_keyword(field_name)),
                        zero_of(&unsigned_holder(field_node.bits)),
                    ));
                }
            }
            statements.push(give_back(name("v")));
            statements
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries. An empty array and not a null: a
        // null is what an `@optional` member's absence means here, and
        // one value cannot mean both.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => match value_type(ast, id) {
            // A map with a class of its own (XTypes 1.3 §7.5.1.3.5): an empty
            // one, which is not a null for the reason above.
            java::Type::Named { path, args }
                if args.is_empty()
                    && path.segments().last().is_some_and(|last| {
                        last.as_str().starts_with(crate::adapter::java::MAP_PREFIX)
                    }) =>
            {
                vec![give_back(java::Expr::New {
                    type_: Box::new(java::Type::named(path)),
                    args: Vec::new(),
                })]
            }
            java::Type::Array(element) => vec![give_back(new_array(
                element,
                vec![java::Expr::Literal(Literal::int(0))],
            ))],
            // DDS-Java-PSM 1.0 §9.1.2: an unbounded sequence is a
            // `Collection<E>`; the empty one is an empty list.
            java::Type::Named { args, .. }
                if crate::adapter::java::mapping() == crate::adapter::java::JavaMapping::Psm
                    && !args.is_empty() =>
            {
                vec![give_back(java::Expr::New {
                    type_: Box::new(java::Type::generic(
                        Path::new(crate::adapter::java::ARRAY_LIST),
                        args,
                    )),
                    args: Vec::new(),
                })]
            }
            _ => {
                return Err(refuse(
                    "collection whose Java form is neither an array nor a Collection",
                ));
            }
        },

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." Java's array is allocated here and not declared with a
        // length, so every dimension has to evaluate.
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
            let mut statements = vec![java::Statement::Declare {
                final_: false,
                type_: value_type(ast, id),
                name: Ident::new("v"),
                value: Some(new_array(
                    Box::new(value_type(ast, element)),
                    lengths
                        .iter()
                        .map(|length| java::Expr::Literal(Literal::int(*length)))
                        .collect(),
                )),
            }];
            let mut access = name("v");
            for level in 0..lengths.len() {
                access = java::Expr::Index {
                    value: Box::new(access),
                    index: Box::new(name(&format!("zd_i{level}"))),
                };
            }
            let mut inner = java::Block {
                statements: vec![default_into(ast, element, access)],
            };
            for (level, length) in lengths.iter().enumerate().rev() {
                inner = java::Block {
                    statements: vec![counted_loop(
                        &format!("zd_i{level}"),
                        java::Expr::Literal(Literal::int(*length)),
                        inner,
                    )],
                };
            }
            statements.extend(inner.statements);
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
                let field = java::escape_keyword(&member_node.name);
                if member_node.facets.is_optional() {
                    // "Implementations shall not provide a default
                    // value in such a case" — which here is the null
                    // the boxed field already holds. Written out
                    // anyway, so a defaulter run over a sample that
                    // has been read once before clears it.
                    statements.push(assign(
                        member("v", &field),
                        java::Expr::Literal(Literal::Null),
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
                default_into(ast, discriminator, member("v", "_d")),
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
        // Refused by name rather than passed over. `any` is in the
        // model because IDL 4.2 has it; DDS-XTypes has no `any` and no
        // encoding for one, and only CORBA CDR writes it — as a
        // TypeCode ahead of the value. Returning `Ok(None)` here would
        // generate a reader that silently omitted the member.
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

/// Table 9's primitive rows, as the Java holder each takes.
///
/// The holder and not the IDL width: `octet` is an `int` here and
/// `unsigned long` is a `long`, so the zero that goes in one is not the
/// zero that goes in the other. `adapter/java.rs` has the table.
fn primitive_default(kind: PrimitiveKind) -> java::Expr {
    match kind {
        PrimitiveKind::Boolean => java::Expr::Literal(Literal::Bool(false)),
        // `'\0'`, written as the character it is: the field is a Java
        // `char` and a bare `0` would be an `int` needing a cast.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            java::Expr::Literal(Literal::Int("'\\0'".to_string()))
        }
        PrimitiveKind::Float32 => java::Expr::Literal(Literal::Float("0.0f".to_string())),
        PrimitiveKind::Float64 => java::Expr::Literal(Literal::Float("0.0".to_string())),
        // A binary128 zero is its zero bit pattern, and `Float128`
        // names it so the spelling does not depend on the class layout.
        PrimitiveKind::Float128 => java::Expr::Path(Path::new(&["Float128", "ZERO"])),
        PrimitiveKind::Int8 | PrimitiveKind::Octet => java::Expr::Cast {
            type_: Box::new(java::Type::Primitive(java::Primitive::Byte)),
            value: Box::new(java::Expr::Literal(Literal::int(0))),
        },
        PrimitiveKind::Int16 => java::Expr::Cast {
            type_: Box::new(java::Type::Primitive(java::Primitive::Short)),
            value: Box::new(java::Expr::Literal(Literal::int(0))),
        },
        PrimitiveKind::Int64 | PrimitiveKind::UInt32 | PrimitiveKind::UInt64 => {
            java::Expr::Literal(Literal::Int("0L".to_string()))
        }
        PrimitiveKind::UInt8 | PrimitiveKind::UInt16 | PrimitiveKind::Int32 => {
            java::Expr::Literal(Literal::int(0))
        }
    }
}

/// The zero of a holder the materialiser chose.
fn zero_of(holder: &java::Type) -> java::Expr {
    if *holder == java::Type::Primitive(java::Primitive::Long) {
        return java::Expr::Literal(Literal::Int("0L".to_string()));
    }
    java::Expr::Literal(Literal::int(0))
}

/// `ZdTypes.<modules>.<Enum>.<LITERAL>` — the instance of the enumeration
/// class the Java materialiser declares (CORBA 3.3 part 3 §1.5.9), written
/// from the outer class down.
pub(crate) fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> java::Expr {
    let owner = declared_path(ast, id).unwrap_or_else(|_| {
        // A path the module chain will not answer for is a name
        // `javac` reports; a bare constant would compile against
        // whatever else happened to be in scope.
        format!("UnresolvedEnum{}", id.slot())
    });
    java::Expr::Path(Path::new(
        &format!("{owner}.{}", java::escape_keyword(enumerator))
            .split('.')
            .collect::<Vec<_>>(),
    ))
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
        .map(|(member, member_type)| (member_type, java::escape_keyword(&member.name))))
}

/// An explicit `@default` as a Java expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<java::Expr, EncodeError> {
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
            Ok(java::Expr::Literal(Literal::Bool(*flag)))
        }
        // A code point and not a Java character escape: the member is a
        // `char`, and `(char) 228` is the same code unit with no
        // question about how the source file was encoded.
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
            let suffix = match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Float32)) => "f",
                _ => "",
            };
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            Ok(java::Expr::Literal(Literal::Float(format!(
                "{mantissa}{exponent}{suffix}"
            ))))
        }
        // A `fixed<P,S>` is the decimal text here, as the member's type
        // is a `String`.
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(big_decimal(&fixed_text(digits, *scale)))
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
                    char_literal(u32::try_from(value).unwrap_or(0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Int8)) => java::Expr::Cast {
                    type_: Box::new(java::Type::Primitive(java::Primitive::Byte)),
                    value: Box::new(java::Expr::Literal(Literal::int(value))),
                },
                Ok(TypeKind::Primitive(PrimitiveKind::Int16)) => java::Expr::Cast {
                    type_: Box::new(java::Type::Primitive(java::Primitive::Short)),
                    value: Box::new(java::Expr::Literal(Literal::int(value))),
                },
                Ok(TypeKind::Primitive(
                    PrimitiveKind::Int64 | PrimitiveKind::UInt32 | PrimitiveKind::UInt64,
                )) => wide(value),
                _ => java::Expr::Literal(Literal::int(value)),
            })
        }
    }
}

/// A 64-bit constant as Java writes one: the pattern, with an `L`.
fn wide(value: i128) -> java::Expr {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let pattern = if value > i128::from(i64::MAX) {
        value as u64 as i64
    } else {
        value as i64
    };
    java::Expr::Literal(Literal::Int(format!("{pattern}L")))
}

/// `(char) <code point>`.
fn char_literal(code: u32) -> java::Expr {
    java::Expr::Cast {
        type_: Box::new(java::Type::Primitive(java::Primitive::Char)),
        value: Box::new(java::Expr::Literal(Literal::int(i128::from(code)))),
    }
}

/// `new java.math.BigDecimal("<text>")`: a `fixed<P,S>` is a `BigDecimal` (CORBA 3.3 part 3
/// §1.5.6).
fn big_decimal(decimal: &str) -> java::Expr {
    java::Expr::New {
        type_: Box::new(java::Type::named(Path::new(&[
            "java",
            "math",
            "BigDecimal",
        ]))),
        args: vec![text(decimal)],
    }
}

// -- reading the calls back out ----------------------------------------

fn calls_in_member(member: &java::Member, out: &mut Vec<String>) {
    match member {
        java::Member::Method(method) => {
            if let Some(body) = &method.body {
                calls_in_block(body, out);
            }
        }
        java::Member::Field(field) => {
            if let Some(value) = &field.value {
                calls_in_expr(value, out);
            }
        }
        java::Member::Constructor(constructor) => calls_in_block(&constructor.body, out),
        java::Member::Initialiser { body, .. } => calls_in_block(body, out),
        java::Member::Nested(_) | java::Member::Comment(_) | java::Member::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_block(block: &java::Block, out: &mut Vec<String>) {
    for statement in &block.statements {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
#[allow(clippy::too_many_lines)]
fn calls_in_statement(statement: &java::Statement, out: &mut Vec<String>) {
    match statement {
        java::Statement::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        java::Statement::Expr(expr) | java::Statement::Throw(expr) => calls_in_expr(expr, out),
        java::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        java::Statement::If {
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
        java::Statement::ForEach { range, body, .. } => {
            calls_in_expr(range, out);
            calls_in_block(body, out);
        }
        java::Statement::For {
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
        java::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_block(body, out);
        }
        java::Statement::Switch { value, cases } => {
            calls_in_expr(value, out);
            for case in cases {
                for label in &case.labels {
                    calls_in_expr(label, out);
                }
                calls_in_block(&case.body, out);
            }
        }
        java::Statement::Break | java::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
#[allow(clippy::too_many_lines)]
fn calls_in_expr(expr: &java::Expr, out: &mut Vec<String>) {
    match expr {
        java::Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            // Unqualified only: a defaulter is a static method of the
            // generated class and this generator built every call to
            // one with `defaulter_name`.
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
        java::Expr::Field { value, .. }
        | java::Expr::Cast { value, .. }
        | java::Expr::InstanceOf { value, .. }
        | java::Expr::Paren(value) => calls_in_expr(value, out),
        java::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        java::Expr::New { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        java::Expr::NewArray {
            lengths, elements, ..
        } => {
            for length in lengths {
                calls_in_expr(length, out);
            }
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        java::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        java::Expr::Ternary {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_expr(then, out);
            calls_in_expr(otherwise, out);
        }
        java::Expr::Lambda { body, .. } => calls_in_expr(body, out),
        java::Expr::ArrayInit(elements) => {
            for element in elements {
                calls_in_expr(element, out);
            }
        }
        java::Expr::Literal(_)
        | java::Expr::Path(_)
        | java::Expr::This
        | java::Expr::Annotation(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn name(text: &str) -> java::Expr {
    java::Expr::Path(Path::new(&text.split('.').collect::<Vec<_>>()))
}

fn text(value: &str) -> java::Expr {
    java::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<java::Expr>) -> java::Expr {
    java::Expr::MethodCall {
        receiver: None,
        name: Ident::new(callee),
        args,
    }
}

/// The member `field` of `base`: the field itself, or under
/// [`crate::adapter::java::JavaMapping::Psm`] the accessor that reads it
/// (and, in [`assign`], writes it).
fn member(base: &str, field: &str) -> java::Expr {
    if crate::adapter::java::mapping() == crate::adapter::java::JavaMapping::Psm {
        return crate::adapter::java::accessor(name(base), field);
    }
    java::Expr::Field {
        value: Box::new(name(base)),
        name: Ident::new(field),
    }
}

fn assign(place: java::Expr, value: java::Expr) -> java::Statement {
    crate::adapter::java::assign(place, value)
}

/// `new T[n]…`, or for a `T` that is a parameterized type
/// `Sequences.newArray(T.class, n…)`.
///
/// Java refuses `new java.util.Collection<java.lang.Integer>[3]` (JLS
/// 15.10.1, generic array creation), and under
/// [`crate::adapter::java::JavaMapping::Psm`] an array of unbounded
/// sequences has exactly that element type. The runtime creates the array
/// from the erasure and the assignment gives it the parameterized type.
fn new_array(element: Box<java::Type>, lengths: Vec<java::Expr>) -> java::Expr {
    let mut innermost = &*element;
    while let java::Type::Array(inner) = innermost {
        innermost = inner;
    }
    if let java::Type::Named { path, args } = innermost {
        if !args.is_empty() {
            let mut segments: Vec<&str> = path
                .segments()
                .iter()
                .map(|segment| segment.as_str())
                .collect();
            segments.push("class");
            let mut arguments = vec![java::Expr::Path(Path::new(&segments))];
            arguments.extend(lengths);
            return java::Expr::MethodCall {
                receiver: Some(Box::new(java::Expr::Path(Path::new(&[
                    crate::adapter::java::SEQUENCES,
                ])))),
                name: Ident::new("newArray"),
                args: arguments,
            };
        }
    }
    java::Expr::NewArray {
        element,
        lengths,
        elements: Vec::new(),
    }
}

fn give_back(value: java::Expr) -> java::Statement {
    java::Statement::Return(Some(value))
}

/// `<T> v = new <T>();`
fn new_local(type_: java::Type) -> java::Statement {
    java::Statement::Declare {
        final_: false,
        type_: type_.clone(),
        name: Ident::new("v"),
        value: Some(java::Expr::New {
            type_: Box::new(type_),
            args: Vec::new(),
        }),
    }
}

fn counted_loop(index: &str, count: java::Expr, body: java::Block) -> java::Statement {
    java::Statement::For {
        init: Box::new(java::Statement::Declare {
            final_: false,
            type_: java::Type::Primitive(java::Primitive::Int),
            name: Ident::new(index),
            value: Some(java::Expr::Literal(Literal::int(0))),
        }),
        condition: java::Expr::Binary {
            op: "<",
            lhs: Box::new(name(index)),
            rhs: Box::new(count),
        },
        step: java::Expr::Binary {
            op: "=",
            lhs: Box::new(name(index)),
            rhs: Box::new(java::Expr::Binary {
                op: "+",
                lhs: Box::new(name(index)),
                rhs: Box::new(java::Expr::Literal(Literal::int(1))),
            }),
        },
        body,
    }
}

/// Kept so the module's one dependency on the materialiser's error type
/// is stated rather than inferred.
const _: Option<MaterialiseError> = None;
