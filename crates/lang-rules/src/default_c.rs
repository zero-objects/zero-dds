// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a C member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as C. The reflective column's copy of the
//! same table is `crates/wire-probe/src/default.rs`, and that module
//! carries the clause quotations and the reasoning for the three kinds
//! Table 9 does not list. Deliberately a second derivation and not a
//! call into the first: the two columns are compared against each other,
//! and a shared helper would make them agree about a mistake.
//!
//! ## What the C column has to do that the reflective one does not
//!
//! Give the member somewhere to be. A `string` is a `const char *`, so
//! its default is the string literal `""` — no heap, because an empty
//! string needs no storage and taking some would make a default value
//! able to fail. A `sequence<T>` is a count beside a pointer, so its
//! default is `0` and `NULL`; §7.2.2.4.4.4.7 asks for "a zero-length
//! sequence", which is what a length of zero is whatever the pointer
//! says. An `@optional` member is its pointer set to NULL and
//! nothing else — "Implementations shall not provide a default value in
//! such a case."
//!
//! One default does need storage: a member that is not `@optional` and
//! whose type reaches the structure that declares it back. C holds such
//! a member as a pointer (`adapter/c.rs`, `c_held_member_type`), and
//! Table 9's value is a value of the member's type, so the pointer has
//! to point at one. The storage comes from the same arena the reader
//! allocates a sequence's buffer from, and only the defaulters whose
//! default reaches such a member take the reader to get it
//! ([`takes_reader`]); every other defaulter keeps `(T *v)`.

use crate::adapter::c::{c_type_name, c_value_type, module_prefix};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident, Literal};

/// The defaulter function name for a type.
///
/// Keyed the same way [`crate::decode_c::decoder_name`] is, so a
/// primitive's defaulter is named by its IDL kind: `double` and
/// `long double` share a C type and do not share a default spelling.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", c_type_name(ast, id)),
    }
}

/// `zd_def_<T>(<destination>);`
///
/// # Errors
/// Never; the signature matches the emitters around it.
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: c::Expr) -> c::Statement {
    let mut args = Vec::new();
    if takes_reader(ast, id) {
        args.push(name("r"));
    }
    args.push(destination(ast, id, place));
    c::Statement::Expr(c::Expr::Call {
        callee: Box::new(name(&defaulter_name(ast, id))),
        args,
    })
}

/// Whether the defaulter of `id` takes the reader: whether Table 9's
/// value of `id` reaches a member C holds behind a pointer that is not
/// `@optional`, and so needs storage to be written at all.
///
/// The walk follows exactly the members a defaulter body defaults — a
/// structure's members that are not `@optional`, a union's branch the
/// default discriminator selects, an array's element, an alias's target
/// — and asks each the question `c_held_member_type` asks of the
/// declaration, [`crate::adapter::c::pointer_member`]: the member closes a
/// cycle, or it is `@external` and not a pointer already. Every caller, the defaulter's own signature included,
/// goes through here, so a call and the function it names cannot
/// disagree about the parameter.
///
/// Terminates without a depth bound of its own: `seen` holds every type
/// entered, and a type is entered once.
#[must_use]
pub fn takes_reader(ast: &Ast, id: Id<TypeNode>) -> bool {
    let mut seen = Vec::new();
    reaches_held_storage(ast, id, &mut seen)
}

/// zerodds-lint: recursion-depth 4096 (one level per distinct type).
/// `seen` is checked before a type is entered and grows by one per
/// entry, so the depth is bounded by the number of types in the model.
fn reaches_held_storage(ast: &Ast, id: Id<TypeNode>, seen: &mut Vec<Id<TypeNode>>) -> bool {
    if seen.contains(&id) {
        return false;
    }
    seen.push(id);
    let Ok(node) = ast.type_(id) else {
        return false;
    };
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            for crate::adapter::c::CMember {
                handle: member,
                owner,
                ..
            } in &crate::adapter::c::c_members(ast, id)
            {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                if member_node.facets.is_optional() {
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    continue;
                };
                if crate::adapter::c::member_held_behind_pointer(
                    ast,
                    *owner,
                    member_node,
                    member_type,
                ) || reaches_held_storage(ast, member_type, seen)
                {
                    return true;
                }
            }
            false
        }
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return false;
            };
            match selected_by_default(ast, id, discriminator) {
                Ok(Some((member_type, _, external))) => {
                    crate::adapter::c::pointer_member(ast, id, member_type, external)
                        || reaches_held_storage(ast, member_type, seen)
                }
                _ => false,
            }
        }
        TypeKind::Alias { aliased } => aliased
            .resolved()
            .is_some_and(|target| reaches_held_storage(ast, target, seen)),
        TypeKind::Array { element, .. } => element
            .resolved()
            .is_some_and(|element| reaches_held_storage(ast, element, seen)),
        _ => false,
    }
}

/// A member C holds behind a pointer, given Table 9's value: storage
/// from the reader's arena first, then the default written through it.
///
/// ```c
/// v->a = (m_A *)zdw_reader_alloc(r, sizeof(m_A));
/// if (v->a == NULL) { return; }
/// zd_def_m_A(v->a);
/// ```
///
/// The allocation and its guard are the reader's own
/// ([`crate::emit::CodecTarget::make_indirection`]): an exhausted arena
/// sets the reader's status, and the read that called this defaulter
/// reports it.
fn held_default(
    ast: &Ast,
    member_type: Id<TypeNode>,
    place: c::Expr,
    write: impl FnOnce(c::Expr) -> Result<Vec<c::Statement>, EncodeError>,
) -> Result<Vec<c::Statement>, EncodeError> {
    use crate::emit::CodecTarget;
    let (mut statements, through) = crate::emit::c::C::make_indirection(ast, member_type, place);
    statements.extend(write(through)?);
    Ok(statements)
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
/// column cannot spell as a C initialiser. Refused rather than silently
/// replaced by Table 9's zero: a member the designer gave a default and
/// a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: c::Expr,
) -> Result<Vec<c::Statement>, EncodeError> {
    let Some(expr) = node.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(vec![default_into(ast, type_, place)]);
    };
    let value = explicit(ast, expr, type_, &node.name)?;
    Ok(vec![assign(place, value)])
}

/// The defaulters `decls` call, and exactly those.
///
/// `decls` is the decoder this generator has just built. The set is read
/// **off the emitted calls** rather than derived a second time from the
/// model, and that is the whole point of the function: a derivation
/// beside the decoder's is a second reading of §7.2.2.7, and the two
/// readings drift.
///
/// They did. The previous derivation seeded from the members that can go
/// missing — a `@mutable` aggregate's, because a writer may omit any of
/// them (§7.4.1.2.3); an `@appendable` one's, because a writer with fewer
/// members closes its DHEADER first (§7.2.4.4.8) — plus a
/// `@try_construct(USE_DEFAULT)` member and a collection's element slot.
/// Every one of those is a real seed. What it missed is that a **bounded
/// collection or a bounded string defaults itself**: `string_fit` and
/// `sequence_body` emit `default_into(ast, id, …)` for the collection
/// when the stream overruns the bound, the `USE_DEFAULT` arm of `narrow`
/// is emitted whatever any member declares because the behaviour is a
/// runtime parameter, and `@final` permits all of it. So a `@final
/// struct { string<5> s; }` called `zd_def_zd_string` and nothing
/// declared it, and the translation unit did not compile. Six corpus
/// families reached it.
///
/// Reading the calls closes the class rather than that one case: a
/// decoder site added later gets its defaulter without anyone
/// remembering this file. It also keeps the other half of the contract,
/// which the derivation was there for — a `static` function with no
/// caller is a `-Wunused-function` error in the C probe's build, and a
/// set taken from the calls has none by construction.
///
/// # Errors
/// [`EncodeError`] for a construct with no C default.
pub fn c_defaulters_called_by(ast: &Ast, decls: &[c::Decl]) -> Result<Vec<c::Decl>, EncodeError> {
    let index = defaulter_index(ast);
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_decl(decl, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    // A defaulter body calls other defaulters — a structure's calls its
    // members', an array's calls its element's — so one pass over the
    // decoder is not the closure. Emitting and re-reading is: the
    // fixpoint is reached when the batch calls nothing it does not also
    // define, which is the property the C compiler checks anyway.
    //
    // Bounded by the number of named defaulters, because every round but
    // the last adds at least one. The bound is written down rather than
    // left to the loop, so a name that never resolves cannot spin.
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
            // ever does, the omission is a C compile error naming the
            // function, which is louder than anything this could return.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = c_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    c_defaulters_over(ast, &wanted)
}

/// Every type in the model under the defaulter name it would take.
///
/// The name and not the id is the key the emitted C carries, and it is a
/// sound one: two ids that share a defaulter name share a defaulter
/// *body*. `zd_def_zd_string` is `*v = ""` for `string<5>` and for
/// `string<10>`; `zd_def_zd_seq_long` is a length of zero for any bound;
/// an array's name carries its element and its dimensions; every other
/// name is a flattened scoped name and unique. [`c_defaulters_over`]
/// already dedupes on the same key.
fn defaulter_index(ast: &Ast) -> Vec<(String, Id<TypeNode>)> {
    ast.types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect()
}

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &c::Decl, out: &mut Vec<String>) {
    match decl {
        c::Decl::Function(function) => {
            if let Some(body) = &function.body {
                calls_in_block(body, out);
            }
        }
        c::Decl::Variable(variable) => {
            if let Some(value) = &variable.value {
                calls_in_expr(value, out);
            }
        }
        c::Decl::Define(define) => {
            if let Some(value) = &define.value {
                calls_in_expr(value, out);
            }
        }
        c::Decl::Typedef(_)
        | c::Decl::Struct(_)
        | c::Decl::Union(_)
        | c::Decl::Enum(_)
        | c::Decl::Comment(_)
        | c::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_block(block: &c::Block, out: &mut Vec<String>) {
    for statement in &block.statements {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &c::Statement, out: &mut Vec<String>) {
    match statement {
        c::Statement::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        c::Statement::Expr(expr) => calls_in_expr(expr, out),
        c::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        c::Statement::If {
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
        c::Statement::For {
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
        c::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_block(body, out);
        }
        c::Statement::Break | c::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &c::Expr, out: &mut Vec<String>) {
    match expr {
        c::Expr::Call { callee, args } => {
            if let c::Expr::Name(name) = callee.as_ref() {
                let name = name.as_str();
                // The prefix and not the whole name: the callee of a
                // defaulter call is always a bare identifier this
                // generator built with `defaulter_name`.
                if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == name) {
                    out.push(name.to_string());
                }
            }
            calls_in_expr(callee, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        c::Expr::Member { value, .. }
        | c::Expr::Arrow { value, .. }
        | c::Expr::Unary { value, .. }
        | c::Expr::AddressOf(value)
        | c::Expr::Deref(value)
        | c::Expr::Paren(value)
        | c::Expr::Cast { value, .. } => calls_in_expr(value, out),
        c::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        c::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        c::Expr::Initializer(items) => {
            for item in items {
                calls_in_expr(item, out);
            }
        }
        c::Expr::Literal(_) | c::Expr::Name(_) | c::Expr::SizeOf(_) => {}
    }
}

/// Build the defaulter for each of `types`, prototypes first.
///
/// The shape [`crate::decode_c::c_decoder`] uses, and for the same
/// reason: a defaulter may call another, and C wants the declaration
/// before the call.
///
/// # Errors
/// [`EncodeError`] for a construct with no C default.
pub fn c_defaulters_over(ast: &Ast, types: &[Id<TypeNode>]) -> Result<Vec<c::Decl>, EncodeError> {
    let mut prototypes = Vec::new();
    let mut bodies = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let function_name = defaulter_name(ast, id);
        if emitted.contains(&function_name) {
            continue;
        }
        let Some(body) = body_of(ast, id, &function_name)? else {
            continue;
        };
        emitted.push(function_name.clone());
        prototypes.push(function(ast, id, &function_name, None));
        bodies.push(function(ast, id, &function_name, Some(body)));
    }
    prototypes.extend(bodies);
    Ok(prototypes)
}

/// `static void <name>(<T> *v)`, with or without a body.
///
/// No `zdw_reader`, except where [`takes_reader`] says the default needs
/// storage. A default is a property of the type and reads nothing; the
/// one that needs storage takes it from the arena the read it is part of
/// allocates from, and a failure there is the read's, reported by the
/// reader's status like any other exhausted arena.
fn function(ast: &Ast, id: Id<TypeNode>, function_name: &str, body: Option<c::Block>) -> c::Decl {
    let value = if is_array(ast, id, 0) {
        c::Param {
            type_: c_value_type(ast, id),
            name: Some(Ident::new("v")),
        }
    } else {
        c::Param {
            type_: c::Type::Pointer(Box::new(c_value_type(ast, id))),
            name: Some(Ident::new("v")),
        }
    };
    c::Decl::Function(c::Function {
        doc: Doc::none(),
        is_static: true,
        is_inline: false,
        // Always emitted and always called; only the `Char16`
        // codec's caller depends on a setting. See
        // `encode_c::maybe_unused`.
        is_maybe_unused: false,
        returns: c::Type::Void,
        name: Ident::new(function_name),
        params: if takes_reader(ast, id) {
            vec![
                c::Param {
                    type_: c::Type::Pointer(Box::new(c::Type::Named(Ident::new("zdw_reader")))),
                    name: Some(Ident::new("r")),
                },
                value,
            ]
        } else {
            vec![value]
        },
        body,
    })
}

#[allow(clippy::too_many_lines)]
fn body_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<c::Block>, EncodeError> {
    // Refused by name, not answered as `Ok(None)`. The caller reads
    // `None` as "nothing to emit for this one" and skips it, so a
    // type whose handle no longer resolves quietly loses its
    // defaulter and the first sign of it is a member left at
    // whatever the language zero-initialises to.
    let Ok(node) = ast.type_(id) else {
        return Err(EncodeError::NoWriter {
            member: own_name.to_string(),
            kind: "a type the model does not hold",
        });
    };
    let statements = match &node.kind {
        TypeKind::Primitive(kind) => vec![assign(deref("v"), primitive_default(*kind))],

        // `""`, for both widths. A `wstring` is UTF-8 in this column,
        // and the empty string is the same bytes in either.
        TypeKind::String { .. } => vec![assign(deref("v"), literal("\"\""))],

        // `fixed<P,S>` is a decimal string here, and
        // `canonical::decimal` accepts exactly one spelling of zero for
        // a given scale.
        TypeKind::Fixed { scale, .. } => vec![assign(
            deref("v"),
            literal(&format!("\"{}\"", zero_decimal(*scale))),
        )],

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "typedef with an unresolved target",
                });
            };
            vec![c::Statement::Expr(c::Expr::Call {
                callee: Box::new(name(&defaulter_name(ast, target))),
                args: vec![name("v")],
            })]
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10.
        TypeKind::Enumeration { .. } => {
            let Some(constant) = default_enumerator(ast, id) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "enumeration with no enumerator to take as its default",
                });
            };
            vec![assign(deref("v"), name(&constant))]
        }

        // No bit set: the holder's Table 9 default is zero and zero sets
        // none.
        TypeKind::Bitmask { .. } => vec![assign(deref("v"), literal("0"))],

        TypeKind::Bitset { .. } => {
            let mut statements = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding and holds no value.
                if let Some(field_name) = field_node.name.as_ref() {
                    statements.push(assign(
                        arrow("v", &c::escape_keyword(field_name)),
                        literal("0"),
                    ));
                }
            }
            statements
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => vec![
            assign(arrow("v", "_maximum"), literal("0u")),
            assign(arrow("v", "_length"), literal("0u")),
            assign(arrow("v", "_buffer"), name("NULL")),
            assign(arrow("v", "_release"), literal("false")),
        ],

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding type."
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "array with an unresolved element",
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(ast.eval_integral(*dim).map_err(|_| EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "array whose length does not evaluate",
                })?);
            }
            let mut access = name("v");
            for level in 0..lengths.len() {
                access = c::Expr::Index {
                    value: Box::new(access),
                    index: Box::new(name(&format!("zd_i{level}"))),
                };
            }
            let mut inner = c::Block {
                statements: vec![default_into(ast, element, access)],
            };
            for (level, length) in lengths.iter().enumerate().rev() {
                inner = c::Block {
                    statements: vec![counted_loop(
                        &format!("zd_i{level}"),
                        literal(&format!("{length}u")),
                        inner,
                    )],
                };
            }
            inner.statements
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = Vec::new();
            for crate::adapter::c::CMember {
                handle: member,
                path,
                owner,
            } in &crate::adapter::c::c_members(ast, id)
            {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                // Through the base where the structure nests it as `parent`
                // (XTypes 1.3 §7.5.1.4.1.3).
                let field = format!("{path}{}", c::escape_keyword(&member_node.name));
                if member_node.facets.is_optional() {
                    statements.push(assign(arrow("v", &field), name("NULL")));
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                if crate::adapter::c::member_held_behind_pointer(
                    ast,
                    *owner,
                    member_node,
                    member_type,
                ) {
                    statements.extend(held_default(
                        ast,
                        member_type,
                        arrow("v", &field),
                        |through| member_default(ast, member_node, member_type, through),
                    )?);
                    continue;
                }
                statements.extend(member_default(
                    ast,
                    member_node,
                    member_type,
                    arrow("v", &field),
                )?);
            }
            statements
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type."
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "union with an unresolved discriminator",
                });
            };
            let mut statements = vec![default_into(ast, discriminator, arrow("v", "_d"))];
            if let Some((member_type, field, external)) =
                selected_by_default(ast, id, discriminator)?
            {
                let place = c::Expr::Member {
                    value: Box::new(arrow("v", "_u")),
                    name: Ident::new(&field),
                };
                if crate::adapter::c::pointer_member(ast, id, member_type, external) {
                    statements.extend(held_default(ast, member_type, place, |through| {
                        Ok(vec![default_into(ast, member_type, through)])
                    })?);
                } else {
                    statements.push(default_into(ast, member_type, place));
                }
            }
            statements
        }

        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Ok(None);
        }
        TypeKind::ValueBox { .. } => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "boxed value, whose presence flag has no settled wire form",
            });
        }
        // Refused by name rather than passed over. `any` is in the model
        // because IDL 4.2 has it; DDS-XTypes has no `any` and no
        // encoding for one, and only CORBA CDR writes it — as a TypeCode
        // ahead of the value. Returning `Ok(None)` here would generate a
        // codec that silently omitted the member.
        TypeKind::Any => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "`any`, which DDS-XTypes has no encoding for; only CORBA CDR writes one, \
                       as a TypeCode ahead of the value",
            });
        }
        TypeKind::ValueBase => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "`ValueBase`, which DDS-XTypes has no encoding for; only CORBA CDR writes \
                       one, as a chunked value with its repository id",
            });
        }
    };
    Ok(Some(c::Block { statements }))
}

/// Table 9's primitive rows as C initialisers.
fn primitive_default(kind: PrimitiveKind) -> c::Expr {
    match kind {
        PrimitiveKind::Boolean => literal("false"),
        // `'\0'`, written as the number it is: `zdw_read_char` hands
        // back a code point and the field holds one.
        PrimitiveKind::Char | PrimitiveKind::WChar => literal("0"),
        PrimitiveKind::Float32 => literal("0.0f"),
        PrimitiveKind::Float64 => literal("0.0"),
        // The `long double` column carries a `zdw_f128`, whose zero is
        // its zero bit pattern. `zdw_f128_zero` is the runtime's name
        // for it, so the spelling does not depend on the struct layout.
        PrimitiveKind::Float128 => c::Expr::Call {
            callee: Box::new(name("zdw_f128_zero")),
            args: Vec::new(),
        },
        PrimitiveKind::Int64 => literal("INT64_C(0)"),
        PrimitiveKind::UInt64 => literal("UINT64_C(0)"),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => literal("0"),
    }
}

/// The enumerator a default takes, as its C constant.
///
/// The *choice* is §7.3.1.2.1.10's and is made once, in
/// [`crate::emit::default_enumerator_name`]; only the spelling is C's.
/// The two used to be one function here, and the reader's fallback in
/// `emit/decode.rs` called into it — which is how a C constant name
/// reached a walk that is supposed to be language-neutral.
pub(crate) fn default_enumerator(ast: &Ast, id: Id<TypeNode>) -> Option<String> {
    let name = crate::emit::default_enumerator_name(ast, id)?;
    Some(format!(
        "{}{}",
        module_prefix(ast, id),
        c::escape_keyword(&name)
    ))
}

/// The case a defaulted discriminator selects, if any.
///
/// The discriminator's own default is Table 9's for its type — zero for
/// an integer, false for a boolean, the first literal for an enumeration
/// — and the case it lands in is decided here rather than at runtime,
/// because the type already fixes it.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    discriminator: Id<TypeNode>,
) -> Result<Option<(Id<TypeNode>, String, bool)>, EncodeError> {
    let Some(selector) = discriminator_default_value(ast, discriminator) else {
        return Ok(None);
    };
    Ok(
        selected_case(ast, id, selector)?.map(|(member, member_type)| {
            (
                member_type,
                c::escape_keyword(&member.name),
                crate::adapter::annotated_external(member),
            )
        }),
    )
}

/// An explicit `@default` as a C initialiser for `type_`.
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<c::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = ast
        .expr(expr)
        .map_err(|_| refuse("a `@default` this frontend cannot follow"))?;
    let resolved = resolve_alias(ast, type_);
    match (
        &node.expr,
        ast.type_(resolved).map(|node| node.kind.clone()),
    ) {
        (ConstExpr::Literal(AstLiteral::Boolean(flag)), _) => {
            Ok(literal(if *flag { "true" } else { "false" }))
        }
        (ConstExpr::Literal(AstLiteral::Char(ch)), _) => {
            Ok(literal(&format!("{}", u32::from(*ch))))
        }
        (ConstExpr::Literal(AstLiteral::WChar(code)), _) => Ok(literal(&format!("{code}"))),
        (ConstExpr::Literal(AstLiteral::String(text)), _) => {
            Ok(literal(&format!("\"{}\"", c_escape(text))))
        }
        (ConstExpr::Literal(AstLiteral::WString(codes)), _) => {
            let mut text = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                text.push(ch);
            }
            Ok(literal(&format!("\"{}\"", c_escape(&text))))
        }
        (ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }), kind) => {
            let suffix = match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Float32)) => "f",
                _ => "",
            };
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            Ok(literal(&format!("{mantissa}{exponent}{suffix}")))
        }
        (ConstExpr::Literal(AstLiteral::Fixed { digits, scale }), _) => {
            Ok(literal(&format!("\"{}\"", fixed_text(digits, *scale))))
        }
        (ConstExpr::Literal(AstLiteral::Enumerator(reference)), _) => {
            let target = reference
                .resolved()
                .ok_or_else(|| refuse("a `@default` enumerator that is unresolved"))?;
            let enumerator = ast
                .enumerator(target)
                .map_err(|_| refuse("a `@default` enumerator this frontend cannot follow"))?;
            Ok(name(&format!(
                "{}{}",
                module_prefix(ast, resolved),
                c::escape_keyword(&enumerator.name)
            )))
        }
        (_, kind) => {
            // An integer literal, a named constant or arithmetic over
            // them, folded the same way a bound or a case label is.
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                // −2^63 is the one `long long` value `INT64_C(..)`
                // cannot carry: the macro pastes a suffix onto the
                // digits and C has no signed type for them, so the
                // minus applies to an unsigned constant.
                // `zerodds_lang_render::c::integer_text` answers with
                // the subtraction `<stdint.h>` uses.
                Ok(TypeKind::Primitive(PrimitiveKind::Int64)) if value == i128::from(i64::MIN) => {
                    literal(&zerodds_lang_render::c::integer_text(value))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Int64)) => {
                    literal(&format!("INT64_C({value})"))
                }
                Ok(TypeKind::Primitive(PrimitiveKind::UInt64)) => {
                    literal(&format!("UINT64_C({value})"))
                }
                _ => literal(&format!("{value}")),
            })
        }
    }
}

/// A string literal's body, escaped for C.
fn c_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

// -- the same small helpers the two codec emitters each keep ------------

/// zerodds-lint: recursion-depth 64 (typedef chains; bounded by the
/// `depth > 64` check, past which the answer is `false` — the
/// destination form a non-array takes)
fn is_array(ast: &Ast, id: Id<TypeNode>, depth: usize) -> bool {
    if depth > 64 {
        return false;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Array { .. }) => true,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| is_array(ast, target, depth + 1)),
        _ => false,
    }
}

fn destination(ast: &Ast, id: Id<TypeNode>, place: c::Expr) -> c::Expr {
    if is_array(ast, id, 0) {
        return place;
    }
    // `&*p` is `p` (C11 §6.5.3.2p3); the place behind a held member's
    // pointer is a dereference, and the defaulter wants the pointer.
    match place {
        c::Expr::Deref(pointer) => *pointer,
        place => c::Expr::AddressOf(Box::new(place)),
    }
}

fn name(text: &str) -> c::Expr {
    c::Expr::Name(Ident::new(text))
}

fn literal(text: &str) -> c::Expr {
    c::Expr::Literal(Literal::Int(text.to_string()))
}

fn deref(text: &str) -> c::Expr {
    c::Expr::Deref(Box::new(name(text)))
}

fn arrow(value: &str, field: &str) -> c::Expr {
    c::Expr::Arrow {
        value: Box::new(name(value)),
        name: Ident::new(field),
    }
}

fn assign(place: c::Expr, value: c::Expr) -> c::Statement {
    c::Statement::Expr(c::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(value),
    })
}

fn counted_loop(index: &str, count: c::Expr, body: c::Block) -> c::Statement {
    c::Statement::For {
        init: Box::new(c::Statement::Declare {
            type_: c::Type::Primitive(c::Primitive::UInt32),
            name: Ident::new(index),
            value: Some(literal("0u")),
        }),
        condition: c::Expr::Binary {
            op: "<",
            lhs: Box::new(name(index)),
            rhs: Box::new(count),
        },
        step: c::Expr::Binary {
            op: "=",
            lhs: Box::new(name(index)),
            rhs: Box::new(c::Expr::Binary {
                op: "+",
                lhs: Box::new(name(index)),
                rhs: Box::new(literal("1u")),
            }),
        },
        body,
    }
}
