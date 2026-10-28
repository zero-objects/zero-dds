// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a TypeScript member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as TypeScript. [`crate::default_c`] is the
//! same table as C, [`crate::default_java`] as Java, [`crate::default_cpp`]
//! as C++, and `crates/wire-probe/src/default.rs` is the reflective
//! column's; that module carries the clause quotations and the reasoning
//! for the three kinds Table 9 does not list. Deliberately a fifth
//! derivation and not a call into any of them: the columns are compared
//! against each other, and a shared helper would make them agree about a
//! mistake.
//!
//! ## Why it is a generated function and not a literal
//!
//! A literal cannot carry an aggregate. "A structure without any of the
//! optional members and with other members set to their default values"
//! is a `newS()` and one assignment per member, and writing that at
//! every position a member can go missing would put the same answer in
//! the generated file as many times as the type is reached — where two
//! copies can differ. One function per type is one answer per type,
//! which is what the C and Java columns do and for the same reason.
//!
//! ## Why it answers with the value instead of filling one
//!
//! C's defaulter is `void zd_def_T(T *v)`: it writes through a pointer.
//! TypeScript has no pointer and no out-parameter, and half of Table 9's
//! rows are values a callee cannot mutate at all — a `number`, a
//! `string`, an array whose length has to change. So the function
//! **returns** Table 9's value and the caller assigns it, which is the
//! same decision one step later. It is also what makes the reader's own
//! shape work: every generated decoder opens with
//! `let v: T = zd_def_T();`, so a position the stream does not carry is
//! already what Table 9 says before a single octet is read.
//!
//! ## What TypeScript forces that Java does not
//!
//! **Nothing is constructed at all.** The materialiser emits no
//! TypeScript class for an IDL data type — DDS-TS 1.0 §7.4 forbids one —
//! so there is no `new` anywhere in this column. A union's defaulter
//! answers with an object literal, because the materialiser declares a
//! union as a `type` over object types and a `type` is erased; every
//! other aggregate has a factory function beside its interface, and it
//! is that function the defaulter calls:
//! `crate::adapter::typescript::factory_path` names it.
//!
//! **An absent `@optional` member is a missing property and not a
//! null.** §7.2.2.4.4.4.8's "Implementations shall not provide a default
//! value" is `delete v.field` here, and `delete` rather than
//! `v.field = undefined` because the materialiser declares the field
//! `field?: T`: under `exactOptionalPropertyTypes` — which
//! `runtime/typescript/tsconfig.json` sets — assigning `undefined` to
//! such a field is a `tsc` error, while deleting it is the operation the
//! declaration was written for.
//!
//! **A `fixed<P,S>` is a decimal string.** Table 9's zero for it is the
//! one spelling `crates/wire-probe/src/canonical.rs` accepts for that
//! scale — `"0.00"` for a `fixed<5,2>` — and not the empty string, which
//! is not a decimal at all.

use crate::adapter::cst::MaterialiseError;
use crate::adapter::typescript::{declared_path, factory_path, unsigned_holder};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::CodecTarget;
use crate::emit::typescript::TypeScript;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::typescript as ts;

/// How deep a chain of typedefs is followed before the model is taken to
/// be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type.
///
/// Keyed the same way [`crate::emit::decode::decoder_name`] is, so a
/// primitive's defaulter is named by its IDL kind: `double` and
/// `long double` share no TypeScript type at all — one is a `number` and
/// the other a `ZdFloat128` — and `octet` and `unsigned short` share
/// one.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", TypeScript::type_name(ast, id)),
    }
}

/// The defaulter that goes with a decoder, by name.
///
/// The two schemes differ in one word — `zd_dec_p_long` against
/// `zd_def_p_long` — because both are built from the same key. Doing it
/// by text lets a caller name the defaulter without carrying the model
/// into a place that has only a signature; a name that does not resolve
/// is a `tsc` error naming the function.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_<T>();`
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: ts::Expr) -> ts::Statement {
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
/// column cannot spell as a TypeScript expression. Refused rather than
/// silently replaced by Table 9's zero: a member the designer gave a
/// default and a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: ts::Expr,
) -> Result<Vec<ts::Statement>, EncodeError> {
    let Some(expr) = node.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(vec![default_into(ast, type_, place)]);
    };
    let value = explicit(ast, expr, type_, &node.name)?;
    Ok(vec![assign(place, value)])
}

/// The defaulters `decls` call, and exactly those.
///
/// The set is read **off the emitted calls** rather than derived a second
/// time from the model, for the reason
/// [`crate::default_c::c_defaulters_called_by`] gives at length: a
/// derivation beside the decoder's is a second reading of §7.2.2.7, and
/// the two readings drift. The closure is taken by emitting and
/// re-reading until the batch calls nothing it does not also define.
///
/// # Errors
/// [`EncodeError`] for a construct with no TypeScript default.
pub fn typescript_defaulters_called_by(
    ast: &Ast,
    decls: &[ts::Decl],
) -> Result<Vec<ts::Decl>, EncodeError> {
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
    // Bounded by the number of named defaulters, because every round but
    // the last adds at least one. Written down rather than left to the
    // loop, so a name that never resolves cannot spin.
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
            // ever does, the omission is a `tsc` error naming the
            // function, which is louder than anything this could return.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = typescript_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    typescript_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no TypeScript default.
pub fn typescript_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<ts::Decl>, EncodeError> {
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
        out.push(ts::Decl::Function(ts::Function {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            exported: false,
            name: Ident::new(&name),
            generics: Vec::new(),
            params: Vec::new(),
            returns: Some(TypeScript::value_type(ast, id)),
            body: Some(ts::Block { statements: body }),
        }));
    }
    Ok(out)
}

/// The statements of one defaulter, or `None` where the type has no
/// TypeScript data form at all.
#[allow(clippy::too_many_lines)]
fn body_of(
    ast: &Ast,
    id: Id<TypeNode>,
    own_name: &str,
) -> Result<Option<Vec<ts::Statement>>, EncodeError> {
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

        // `""`, for both widths: a TypeScript `string` is UTF-16 already
        // and the empty string is the same string in either.
        TypeKind::String { .. } => vec![give_back(text(""))],

        // A `fixed<P,S>` is the decimal it was written as, and
        // `canonical::decimal` accepts exactly one spelling of zero for
        // a given scale.
        TypeKind::Fixed { scale, .. } => vec![give_back(text(&zero_decimal(*scale)))],

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(refuse("typedef with an unresolved target"));
            };
            vec![give_back(call(&defaulter_name(ast, target), Vec::new()))]
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10. By name and not as
        // the number it happens to be: the materialiser declared the
        // constant, and a generated file that spelled the value instead
        // would keep compiling after `@value` moved it.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            vec![give_back(enumerator_constant(ast, id, &enumerator))]
        }

        // No bit set: the holder's Table 9 default is zero and zero sets
        // none. The holder decides the spelling — a `@bit_bound(40)`
        // mask is a `bigint` and `0` is not one.
        TypeKind::Bitmask { .. } => {
            let holder = unsigned_holder(ast.effective_bit_bound(id));
            vec![give_back(zero_of(&holder))]
        }

        // A bitset's fields are separate integers in the materialiser's
        // shape, so its default is the object with every one of them at
        // zero. The packing into the anchor is the codec's and does not
        // appear here at all.
        TypeKind::Bitset { .. } => {
            let mut statements = vec![new_local(ast, id, own_name)?];
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding and holds no value.
                if let Some(field_name) = field_node.name.as_ref() {
                    statements.push(assign(
                        member("v", &ts::escape_member(field_name)),
                        zero_of(&unsigned_holder(field_node.bits)),
                    ));
                }
            }
            statements.push(give_back(name("v")));
            statements
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries. An empty array and not `undefined`:
        // `undefined` is what an `@optional` member's absence means
        // here, and one value cannot mean both. §7.2.2.4.4.4.7 asks for
        // a zero-length sequence, and §7.2.2.4.4.4.8's "Implementations
        // shall not provide a default value" for an `@optional` member
        // is the absence.
        TypeKind::Sequence { .. } => {
            vec![give_back(ts::Expr::Array(Vec::new()))]
        }

        // The same clause for a map, and DDS-TS 1.0 §7.9.1 makes its
        // TypeScript surface a `ReadonlyMap` — so the zero-length value
        // is an empty `Map` and not an empty array.
        TypeKind::Map { .. } => {
            vec![give_back(ts::Expr::New {
                callee: Path::new(&["Map"]),
                args: Vec::new(),
            })]
        }

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding type."
        //
        // `Array.from({ length: n }, () => …)` and not a literal of n
        // elements: an IDL array's length is a constant expression and
        // nothing bounds it, so `long a[4096]` would otherwise be four
        // thousand tokens. The arrow also gives every element its *own*
        // value, which `new Array(n).fill(zd_def_T())` would not — a
        // filled array holds n references to one object, and writing
        // through element 0 would change all of them.
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
            let mut out = call(&defaulter_name(ast, element), Vec::new());
            for length in lengths.into_iter().rev() {
                out = filled_array(length, out);
            }
            vec![give_back(out)]
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = vec![new_local(ast, id, own_name)?];
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                statements.push(assign(
                    member("v", "parent"),
                    call(&defaulter_name(ast, base), Vec::new()),
                ));
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                let field = ts::escape_member(&member_node.name);
                if member_node.facets.is_optional() {
                    // "Implementations shall not provide a default value
                    // in such a case" — which here is the property not
                    // being there. The factory already leaves it out;
                    // written anyway, so a defaulter run over a value
                    // that has been read into before clears it.
                    statements.push(ts::Statement::Expr(ts::Expr::Unary {
                        op: "delete ",
                        value: Box::new(member("v", &field)),
                    }));
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
        //
        // An object literal and not a constructed value: the
        // materialiser declares a union as a `type` over object types,
        // and a `type` is erased — there is nothing to `new`.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut entries = vec![(
                ts::PropertyName::ident(crate::adapter::typescript::DISCRIMINANT_PROPERTY),
                discriminant_literal(ast, discriminator, selector.unwrap_or(0), refuse)?,
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                entries.push((
                    ts::PropertyName::ident(&field),
                    call(&defaulter_name(ast, member_type), Vec::new()),
                ));
            }
            vec![give_back(ts::Expr::Object(entries))]
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

/// Table 9's primitive rows, as the TypeScript holder each takes.
///
/// The holder and not the IDL width: every integer to 32 bits is a
/// `number` here and the two 64-bit ones are a `bigint`, so the zero
/// that goes in one is not the zero that goes in the other.
/// `adapter/typescript.rs` has the table.
fn primitive_default(kind: PrimitiveKind) -> ts::Expr {
    match kind {
        PrimitiveKind::Boolean => ts::Expr::Literal(Literal::Bool(false)),
        // `"\0"`, which is a one-character `string`: TypeScript has no
        // character type and the member is a `string`, so a `0` would be
        // a different type and not a different spelling of the same
        // value.
        PrimitiveKind::Char | PrimitiveKind::WChar => ts::Expr::Literal(Literal::Char('\0')),
        // A binary128 zero is its zero bit pattern, and sixteen zero
        // octets are it. The object literal and not a named constant:
        // `ZdFloat128` is an interface the prelude declares, and an
        // interface has no value side to hang a `ZERO` on.
        PrimitiveKind::Float128 => ts::Expr::Object(vec![(
            ts::PropertyName::ident("octets"),
            ts::Expr::New {
                callee: Path::new(&["Uint8Array"]),
                args: vec![ts::Expr::Literal(Literal::int(16))],
            },
        )]),
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
            ts::Expr::Literal(Literal::Int("0n".to_string()))
        }
        PrimitiveKind::Float32
        | PrimitiveKind::Float64
        | PrimitiveKind::Int8
        | PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => ts::Expr::Literal(Literal::int(0)),
    }
}

/// The zero of a holder the materialiser chose.
fn zero_of(holder: &ts::Type) -> ts::Expr {
    if *holder == ts::Type::bigint() {
        return ts::Expr::Literal(Literal::Int("0n".to_string()));
    }
    ts::Expr::Literal(Literal::int(0))
}

/// `Array.from({ length: n }, () => <element>)`
fn filled_array(length: i128, element: ts::Expr) -> ts::Expr {
    // A concise arrow body starting with `{` is a *block*, so
    // `() => { _d: 0 }` is a function with a labelled statement in it
    // and not one answering with a value. The renderer writes what the
    // tree holds, so the tree holds the brackets.
    let element = match element {
        ts::Expr::Object(_) => ts::Expr::Parenthesised(Box::new(element)),
        other => other,
    };
    ts::Expr::Call {
        callee: Box::new(ts::Expr::Member {
            object: Box::new(ts::Expr::Path(Path::new(&["Array"]))),
            name: Ident::new("from"),
        }),
        args: vec![
            ts::Expr::Object(vec![(
                ts::PropertyName::ident("length"),
                ts::Expr::Literal(Literal::int(length)),
            )]),
            ts::Expr::Arrow {
                params: Vec::new(),
                body: Box::new(element),
            },
        ],
    }
}

/// `a.b.Enum.LITERAL` — the `as const` object the materialiser declared,
/// written from the file root.
///
/// Not the bare enumerator name, which is what the trait's default
/// answers: the materialiser puts the object inside a namespace per IDL
/// module and a generated codec sits at the file root, so `RED` there
/// names nothing at all — or, worse, names whatever else happened to be
/// in scope.
#[must_use]
pub fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> ts::Expr {
    let mut segments: Vec<String> = match declared_path(ast, id) {
        Ok(path) => path
            .segments()
            .iter()
            .map(|segment| segment.as_str().to_string())
            .collect(),
        // A path the module chain will not answer for is a name `tsc`
        // reports; a bare constant would compile against whatever else
        // happened to be in scope.
        Err(_) => vec![format!("UnresolvedEnum{}", id.slot())],
    };
    segments.push(ts::escape_member(enumerator));
    ts::Expr::Path(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// `const v: T = a.b.newT();`
///
/// The interface for the annotation and the factory for the value: an
/// interface is erased and nothing can be constructed from one, and this
/// PSM emits no class to construct (DDS-TS 1.0 §7.4). The local is typed
/// as the interface, so the generated codec depends on the shape and on
/// nothing else.
fn new_local(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<ts::Statement, EncodeError> {
    let path = factory_path(ast, id).map_err(|_| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind: "aggregate whose declared name the module chain will not answer for",
    })?;
    Ok(ts::Statement::Declare {
        kind: ts::VarKind::Const,
        name: Ident::new("v"),
        type_: Some(TypeScript::value_type(ast, id)),
        value: Some(ts::Expr::Call {
            callee: Box::new(ts::Expr::Path(path)),
            args: Vec::new(),
        }),
    })
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
        .map(|(member, member_type)| (member_type, ts::escape_member(&member.name))))
}

/// A discriminator value in the TypeScript form its type gives it.
///
/// The three shapes are not interchangeable and the compiler says so: a
/// `char` discriminator's `_d` is a `string`, a 64-bit one's is a
/// `bigint`, and everything else's is a `number`. `0` in a `_d` declared
/// `"\0"` is a type error, which is the outcome this column wants.
fn discriminant_literal(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: i128,
    refuse: impl Fn(&'static str) -> EncodeError,
) -> Result<ts::Expr, EncodeError> {
    Ok(match resolved_primitive(ast, discriminator) {
        Some(PrimitiveKind::Boolean) => ts::Expr::Literal(Literal::Bool(value != 0)),
        // Through `code_unit_of` for the same reason `char_literal` no
        // longer takes a Rust `char`: a discriminator whose value is a
        // lone surrogate is a legal UTF-16 code unit and not a Unicode
        // scalar value, and folding it to NUL would put a `_d` in the
        // defaulted union that no peer ever sent.
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            char_literal(code_unit_of(value, refuse)?)
        }
        Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => wide(value),
        _ => {
            // An enumeration discriminator names the enumerator through
            // the `as const` object: DDS-TS 1.0 §7.6 makes the alias a
            // string-literal union, and the ordinal is not a value of
            // it. A value that names no enumerator is refused rather
            // than written as the number, which would be a discriminant
            // the declared type does not hold.
            match resolve_enumeration(ast, discriminator) {
                Some(enumeration) => {
                    let Some(name) = enumerator_named_by(ast, enumeration, value) else {
                        return Err(refuse(
                            "a union discriminator value that names no enumerator",
                        ));
                    };
                    enumerator_constant(ast, enumeration, &name)
                }
                None => ts::Expr::Literal(Literal::int(value)),
            }
        }
    })
}

/// `a.b.Tag.TAG_A` — the constant a union case label of an enumeration
/// discriminator is compared against.
///
/// `None` where the discriminator is not an enumeration, or where the
/// value names no enumerator of it; the caller answers with the numeric
/// form in both cases, which is right for the first and a `tsc` error
/// for the second.
#[must_use]
pub fn case_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Option<ts::Expr> {
    let enumeration = resolve_enumeration(ast, discriminator)?;
    let name = enumerator_named_by(ast, enumeration, value)?;
    Some(enumerator_constant(ast, enumeration, &name))
}

/// The enumeration `id` is once typedefs are seen through, or `None`
/// where it is not one.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_ALIAS_DEPTH`)
fn resolve_enumeration(ast: &Ast, id: Id<TypeNode>) -> Option<Id<TypeNode>> {
    let mut current = id;
    for _ in 0..MAX_ALIAS_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Enumeration { .. }) => return Some(current),
            Ok(TypeKind::Alias { aliased }) => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// The first enumerator of `id` whose effective value is `value`.
///
/// First and not any: two enumerators may carry the same `@value`, and
/// the generated codec's own chains take the first in declaration order
/// in both directions.
fn enumerator_named_by(ast: &Ast, id: Id<TypeNode>, value: i128) -> Option<String> {
    for enumerator in ast.enumerators_of(id) {
        if ast.effective_enumerator_value(*enumerator) == Some(value) {
            return ast
                .enumerator(*enumerator)
                .ok()
                .map(|node| node.name.clone());
        }
    }
    None
}

/// The primitive `id` is once typedefs are seen through, or `None` where
/// it is not one.
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

/// An explicit `@default` as a TypeScript expression for `type_`.
///
/// The materialiser's side-table constant of DDS-TS 1.0 §7.11 and the
/// defaulter this module writes hold the same value, so both take it
/// from here: two readings of one annotation are how a constant and a
/// decoder come to disagree about what a member starts from.
///
/// # Errors
/// [`EncodeError::NoWriter`] where the `@default` names a value this
/// column cannot spell as a TypeScript expression.
pub fn explicit_default(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<ts::Expr, EncodeError> {
    explicit(ast, expr, type_, member_name)
}

/// An explicit `@default` as a TypeScript expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<ts::Expr, EncodeError> {
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
            Ok(ts::Expr::Literal(Literal::Bool(*flag)))
        }
        // A one-character string, which is what a `char` member is here.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(char_literal(u32::from(*ch))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => {
            Ok(char_literal(code_unit_of(i128::from(*code), refuse)?))
        }
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
            // No suffix: TypeScript has one numeric type and a `float`
            // member is the same `number` a `double` member is. The
            // rounding to binary32 is the writer's, at the octet.
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            Ok(ts::Expr::Literal(Literal::Float(format!(
                "{mantissa}{exponent}"
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
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    ts::Expr::Literal(Literal::Bool(value != 0))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    char_literal(code_unit_of(value, refuse)?)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64)) => {
                    wide(value)
                }
                _ => ts::Expr::Literal(Literal::int(value)),
            })
        }
    }
}

/// A 64-bit constant as TypeScript writes one: the value, with an `n`.
///
/// The value and not the two's-complement pattern the Java column
/// writes: a `bigint` is unbounded, so 18446744073709551615 is itself
/// here where `long` has to carry it as `-1L`.
fn wide(value: i128) -> ts::Expr {
    ts::Expr::Literal(Literal::Int(format!("{value}n")))
}

/// A code point as the one-character `string` a `char` member holds.
fn char_literal(code: u32) -> ts::Expr {
    // `String.fromCharCode` and not a character literal, because
    // `Literal::Char` carries a Rust `char` and a Rust `char` is a
    // Unicode scalar value. A TypeScript string is UTF-16 and holds a
    // lone surrogate; the two types disagree exactly on
    // `0xD800..=0xDFFF`, and `char::from_u32(code).unwrap_or('\0')`
    // answered that disagreement by silently substituting NUL. Measured:
    // `@default(0xD800) wchar c` produced `v.C = '\0'` in this column's
    // sibling while the C column wrote `55296` and the Java column
    // `(char) 55296`.
    //
    // Above `0xFFFF` there is no single code unit; `code_unit_of` refuses
    // that before it reaches here.
    ts::Expr::Call {
        callee: Box::new(ts::Expr::Member {
            object: Box::new(ts::Expr::Path(Path::new(&["String"]))),
            name: Ident::new("fromCharCode"),
        }),
        args: vec![ts::Expr::Literal(Literal::int(i128::from(code)))],
    }
}

/// The one UTF-16 code unit `value` is, or a refusal naming why it is not
/// one.
///
/// Both ends of the range were folded before: a negative `@default` went
/// through `u32::try_from(..).unwrap_or(0)` to NUL, and one above the
/// Basic Multilingual Plane has no single code unit at all.
fn code_unit_of(
    value: i128,
    refuse: impl Fn(&'static str) -> EncodeError,
) -> Result<u32, EncodeError> {
    let code = u32::try_from(value)
        .map_err(|_| refuse("a `@default` character outside the range of a code unit"))?;
    if code > u32::from(u16::MAX) {
        return Err(refuse(
            "a `@default` character above the Basic Multilingual Plane, \
             which no single UTF-16 code unit holds",
        ));
    }
    Ok(code)
}

// -- reading the calls back out ----------------------------------------

/// zerodds-lint: recursion-depth 64 (declaration nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that declaration's height.
fn calls_in_decl(decl: &ts::Decl, out: &mut Vec<String>) {
    match decl {
        ts::Decl::Function(function) => {
            if let Some(body) = &function.body {
                calls_in_block(body, out);
            }
        }
        ts::Decl::Variable(variable) => {
            if let Some(value) = &variable.value {
                calls_in_expr(value, out);
            }
        }
        ts::Decl::Class(class) => {
            for member in &class.members {
                match member {
                    ts::ClassMember::Field(field) => {
                        if let Some(value) = &field.value {
                            calls_in_expr(value, out);
                        }
                    }
                    ts::ClassMember::Method(method) => {
                        if let Some(body) = &method.body {
                            calls_in_block(body, out);
                        }
                    }
                    ts::ClassMember::Constructor(constructor) => {
                        calls_in_block(&constructor.body, out);
                    }
                    ts::ClassMember::Comment(_) | ts::ClassMember::Verbatim(_) => {}
                }
            }
        }
        ts::Decl::Namespace(namespace) => {
            for nested in &namespace.decls {
                calls_in_decl(nested, out);
            }
        }
        ts::Decl::Statement(statement) => calls_in_statement(statement, out),
        ts::Decl::Interface(_)
        | ts::Decl::TypeAlias(_)
        | ts::Decl::Enum(_)
        | ts::Decl::Comment(_)
        | ts::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_block(block: &ts::Block, out: &mut Vec<String>) {
    for statement in &block.statements {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &ts::Statement, out: &mut Vec<String>) {
    match statement {
        ts::Statement::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        ts::Statement::Expr(expr) | ts::Statement::Throw(expr) => calls_in_expr(expr, out),
        ts::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        ts::Statement::If {
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
        ts::Statement::For { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_block(body, out);
        }
        ts::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_block(body, out);
        }
        ts::Statement::ForOf { range, body, .. } => {
            calls_in_expr(range, out);
            calls_in_block(body, out);
        }
        ts::Statement::Switch { value, cases } => {
            calls_in_expr(value, out);
            for case in cases {
                for label in &case.labels {
                    calls_in_expr(label, out);
                }
                calls_in_block(&case.body, out);
            }
        }
        ts::Statement::Break | ts::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
#[allow(clippy::too_many_lines)]
fn calls_in_expr(expr: &ts::Expr, out: &mut Vec<String>) {
    match expr {
        ts::Expr::Call { callee, args } => {
            // Unqualified only: a defaulter is a module-level function
            // of the generated file and this generator built every call
            // to one with `defaulter_name`.
            if let ts::Expr::Path(path) = callee.as_ref() {
                if path.segments().len() == 1 {
                    let name = path
                        .segments()
                        .first()
                        .map(|segment| segment.as_str().to_string())
                        .unwrap_or_default();
                    if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == &name) {
                        out.push(name);
                    }
                }
            }
            calls_in_expr(callee, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        ts::Expr::Member { object, .. } => calls_in_expr(object, out),
        ts::Expr::Index { object, index } => {
            calls_in_expr(object, out);
            calls_in_expr(index, out);
        }
        ts::Expr::Array(values) => {
            for value in values {
                calls_in_expr(value, out);
            }
        }
        ts::Expr::Object(entries) => {
            for (_, value) in entries {
                calls_in_expr(value, out);
            }
        }
        ts::Expr::New { args, .. } => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        // `NonNull` is `v[i]!`, which the encode walk writes for an
        // indexed read; a defaulter call can sit under one, so the
        // descent has to reach through it.
        ts::Expr::Paren(value)
        | ts::Expr::Parenthesised(value)
        | ts::Expr::NonNull(value)
        | ts::Expr::AsConst(value)
        | ts::Expr::Unary { value, .. }
        | ts::Expr::As { value, .. } => calls_in_expr(value, out),
        ts::Expr::Arrow { body, .. } => calls_in_expr(body, out),
        ts::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        ts::Expr::Ternary {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_expr(then, out);
            calls_in_expr(otherwise, out);
        }
        ts::Expr::Literal(_) | ts::Expr::Path(_) | ts::Expr::This | ts::Expr::Undefined => {}
    }
}

// -- the small helpers --------------------------------------------------

fn name(text: &str) -> ts::Expr {
    ts::Expr::Path(Path::new(&text.split('.').collect::<Vec<_>>()))
}

fn text(value: &str) -> ts::Expr {
    ts::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<ts::Expr>) -> ts::Expr {
    ts::Expr::Call {
        callee: Box::new(name(callee)),
        args,
    }
}

fn member(base: &str, field: &str) -> ts::Expr {
    ts::Expr::Member {
        object: Box::new(name(base)),
        name: Ident::new(field),
    }
}

fn assign(place: ts::Expr, value: ts::Expr) -> ts::Statement {
    ts::Statement::Expr(ts::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(value),
    })
}

fn give_back(value: ts::Expr) -> ts::Statement {
    ts::Statement::Return(Some(value))
}

/// Kept so the module's one dependency on the materialiser's error type
/// is stated rather than inferred.
const _: Option<MaterialiseError> = None;
