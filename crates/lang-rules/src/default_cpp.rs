// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a C++ member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as C++. [`crate::default_c`] is the same
//! table as C and `crates/wire-probe/src/default.rs` as Rust; that last
//! module carries the clause quotations and the reasoning for the three
//! kinds Table 9 does not list. Deliberately a third derivation and not
//! a call into either of the first two: the columns are compared against
//! each other, and a shared helper would make them agree about a
//! mistake.
//!
//! ## Why a function per type and not `T{}`
//!
//! Value-initialisation is not Table 9. `T{}` zeroes a structure's
//! members, which is right for the integers and wrong for three kinds
//! the corpus reaches:
//!
//! - an **enumeration** whose first literal is not zero — `@value(3)` on
//!   the first enumerator, or a `@default_literal` naming another one —
//!   takes that literal (§7.3.1.2.1.10) and not the pattern zero;
//! - a **union** takes its discriminator's default *and the branch that
//!   selects*, which no aggregate initialiser reaches;
//! - a member with an explicit `@default` takes the literal its
//!   declaration gave it (§7.2.2.4.4.4.9), which is not the type's
//!   value at all.
//!
//! Beyond that, `adapter/cpp.rs` emits no member initialisers, so `T{}`
//! on a class with no user-provided constructor is
//! value-initialisation of members that are themselves classes and
//! zero-initialisation of the scalars — a rule that reads differently
//! for `struct S { int a; }` and `struct S { S2 a; }`, and one the C
//! column has no counterpart for. Two columns answering Table 9 by
//! different mechanisms is a divergence no byte comparison shows,
//! because both produce *a* value.
//!
//! ## What C++ makes easier than C
//!
//! Storage. A `string` is a `std::string` and its Table 9 value is the
//! empty one, with no heap for the caller to have supplied and no
//! failure to report; a `sequence<T>` is a `std::vector<T>` and
//! §7.2.2.4.4.4.7's "zero-length sequence of the same element type" is
//! `clear()`. The C column's `_length`/`_buffer` pair and its `NULL` are
//! not here, and neither is C's array-versus-pointer split in the
//! parameter: a `std::array` is a value, so **every** defaulter takes a
//! plain reference.
//!
//! An `@optional` member is NULL and nothing else —
//! "Implementations shall not provide a default value in such a case."

use crate::adapter::cpp::{cpp_type_tag, cpp_value_type, cxx_name};
use crate::default_common::{resolve_alias, selected_case};
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp;

/// The defaulter function name for a type.
///
/// Keyed the same way [`crate::emit::decode::decoder_name`] is, so a
/// primitive's defaulter is named by its IDL kind: `double` and
/// `long double` are one C++ built-in away from each other in
/// `cpp_primitive` and are eight bytes apart on the wire, and
/// `cpp_type_tag` is what keeps every such pair apart.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", cpp_type_tag(ast, id)),
    }
}

/// `zd_def_<T>(<place>);`
///
/// No address-of and no array exception, unlike the C column's: the
/// parameter is a reference, so the destination is written the same way
/// whatever the type is.
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: cpp::Expr) -> cpp::Statement {
    cpp::Statement::Expr(cpp::Expr::Call {
        callee: Box::new(path(&defaulter_name(ast, id))),
        args: vec![place],
    })
}

/// `zerodds::wire::external_slot(place)` — see
/// `runtime/cpp/include/zerodds/wire.hpp`.
fn external_slot(place: cpp::Expr) -> cpp::Expr {
    cpp::Expr::Call {
        callee: Box::new(cpp::Expr::Path(Path::new(&[
            "zerodds",
            "wire",
            "external_slot",
        ]))),
        args: vec![place],
    }
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
/// column cannot spell as a C++ initialiser. Refused rather than
/// silently replaced by Table 9's zero: a member the designer gave a
/// default and a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: cpp::Expr,
) -> Result<Vec<cpp::Statement>, EncodeError> {
    // A non-optional `@external` member is Table 9's value behind the
    // column's own indirection — `std::shared_ptr<T>`, DDS-XTypes 1.3
    // Table 21 — so the pointer is filled and the value written into
    // what it points at. Here and not at the two call sites: the
    // defaulter routine and `emit/decode.rs`'s `defaults_for_absent`
    // both come through this function, which is where the D and Zig
    // columns answer the same question (`default_d.rs`,
    // `default_zig.rs`).
    let place = if crate::adapter::held_externally(ast, node) {
        external_slot(place)
    } else {
        place
    };
    let Some(expr) = node.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(vec![default_into(ast, type_, place)]);
    };
    let value = explicit(ast, expr, type_, &node.name)?;
    Ok(vec![assign(place, value)])
}

/// The defaulters `decls` call, and exactly those.
///
/// The C column's [`crate::default_c::c_defaulters_called_by`] reads its
/// set off the emitted calls rather than deriving it a second time from
/// the model, and the reason it gives applies here unchanged: a
/// derivation beside the decoder's is a second reading of §7.2.2.7, and
/// the two drift. That file records the drift it had — a bounded
/// collection defaults *itself* when the stream overruns its bound, and
/// the derivation that seeded from the members which can go missing did
/// not know it, so six corpus families called a function nothing
/// declared.
///
/// The other half of the contract is the same too: the C++ probe is
/// compiled with `-Wall -Wextra -Werror`, where a `static` function with
/// no caller is an error, and a set taken from the calls has none by
/// construction.
///
/// # Errors
/// [`EncodeError`] for a construct with no C++ default.
pub fn cpp_defaulters_called_by(
    ast: &Ast,
    decls: &[cpp::Decl],
) -> Result<Vec<cpp::Decl>, EncodeError> {
    cpp_defaulters_called_by_excluding(ast, decls, &[])
}

/// [`cpp_defaulters_called_by`] without the types in `excluded`, whose
/// defaulters are written by hand elsewhere.
///
/// # Errors
/// As [`cpp_defaulters_called_by`].
pub fn cpp_defaulters_called_by_excluding(
    ast: &Ast,
    decls: &[cpp::Decl],
    excluded: &[Id<TypeNode>],
) -> Result<Vec<cpp::Decl>, EncodeError> {
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
    // define, which is the property the C++ compiler checks anyway.
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
            // ever does, the omission is a C++ compile error naming the
            // function, which is louder than anything this could return.
            if let Some(id) = index
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, id)| *id)
                .filter(|id| !excluded.contains(id))
            {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = cpp_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    cpp_defaulters_over(ast, &wanted)
}

/// `decls` without the routines `entry` does not reach.
///
/// For a probe that calls one routine of a codec it generated whole —
/// the KeyHash harness calls the key holder and nothing else. Every
/// generated routine is `static`, and the probe is compiled with
/// `-Wall -Werror`, so a routine nothing calls is an error; pruning it
/// keeps that warning sharp instead of silencing it, which is what the C
/// column's harness does for the same reason. A declaration that is not
/// a function — a type, a constant — is kept.
#[must_use]
pub fn reachable_from(decls: Vec<cpp::Decl>, entry: &str) -> Vec<cpp::Decl> {
    let mut wanted = vec![entry.to_string()];
    let mut index = 0;
    while index < wanted.len() {
        let name = wanted[index].clone();
        index += 1;
        for decl in &decls {
            if let cpp::Decl::Function(function) = decl
                && function.name.as_str() == name
            {
                let mut called = Vec::new();
                calls_in_decl(decl, &mut called);
                for callee in called {
                    if !wanted.contains(&callee) {
                        wanted.push(callee);
                    }
                }
            }
        }
    }
    decls
        .into_iter()
        .filter(|decl| match decl {
            cpp::Decl::Function(function) => {
                wanted.iter().any(|kept| kept == function.name.as_str())
            }
            _ => true,
        })
        .collect()
}

/// Every type in the model under the defaulter name it would take.
///
/// The name and not the id is the key the emitted C++ carries, and it is
/// a sound one for the reason the C column's index is: two ids that
/// share a defaulter name share a defaulter *body*. `zd_def_string` is
/// `v.clear()` for `string<5>` and for `string<10>`; `zd_def_seq_int32`
/// is an empty vector at any bound; an array's tag carries its element
/// and its dimensions; every other tag is a flattened scoped name and is
/// unique. [`cpp_defaulters_over`] dedupes on the same key.
fn defaulter_index(ast: &Ast) -> Vec<(String, Id<TypeNode>)> {
    ast.types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect()
}

/// The `zd_def_*` functions one declaration calls, appended to `out`.
///
/// zerodds-lint: recursion-depth 64 (declaration nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that declaration's height.
fn calls_in_decl(decl: &cpp::Decl, out: &mut Vec<String>) {
    match decl {
        cpp::Decl::Function(function) => {
            if let cpp::FunctionBody::Block(body) = &function.body {
                calls_in_block(body, out);
            }
        }
        cpp::Decl::Variable(variable) => {
            if let Some(value) = &variable.value {
                calls_in_expr(value, out);
            }
        }
        cpp::Decl::Namespace(namespace) => {
            for inner in &namespace.decls {
                calls_in_decl(inner, out);
            }
        }
        cpp::Decl::Template(template) => calls_in_decl(&template.decl, out),
        cpp::Decl::Class(class) => {
            for section in &class.sections {
                for member in &section.members {
                    calls_in_class_member(member, out);
                }
            }
        }
        cpp::Decl::Enum(_)
        | cpp::Decl::Alias(_)
        | cpp::Decl::Comment(_)
        | cpp::Decl::Verbatim(_)
        | cpp::Decl::Forward { .. } => {}
    }
}

/// zerodds-lint: recursion-depth 64 (declaration nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that declaration's height.
fn calls_in_class_member(member: &cpp::ClassMember, out: &mut Vec<String>) {
    match member {
        cpp::ClassMember::Method(method) => {
            if let Some(body) = &method.body {
                calls_in_block(body, out);
            }
        }
        cpp::ClassMember::Constructor(constructor) => {
            for (_, value) in &constructor.initialisers {
                calls_in_expr(value, out);
            }
            if let cpp::FunctionBody::Block(body) = &constructor.body {
                calls_in_block(body, out);
            }
        }
        cpp::ClassMember::Destructor(destructor) => {
            if let cpp::FunctionBody::Block(body) = &destructor.body {
                calls_in_block(body, out);
            }
        }
        cpp::ClassMember::Nested(decl) => calls_in_decl(decl, out),
        cpp::ClassMember::Field(field) => {
            if let Some(value) = &field.default {
                calls_in_expr(value, out);
            }
        }
        cpp::ClassMember::Alias(_)
        | cpp::ClassMember::Comment(_)
        | cpp::ClassMember::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_block(block: &cpp::Block, out: &mut Vec<String>) {
    for statement in &block.statements {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &cpp::Statement, out: &mut Vec<String>) {
    match statement {
        cpp::Statement::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        cpp::Statement::Expr(expr) | cpp::Statement::Throw(expr) => calls_in_expr(expr, out),
        cpp::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        cpp::Statement::If {
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
        cpp::Statement::For {
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
        cpp::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_block(body, out);
        }
        cpp::Statement::Try { body, handlers } => {
            calls_in_block(body, out);
            for handler in handlers {
                calls_in_block(&handler.body, out);
            }
        }
        cpp::Statement::ForEach { range, body, .. } => {
            calls_in_expr(range, out);
            calls_in_block(body, out);
        }
        cpp::Statement::Switch { value, cases } => {
            calls_in_expr(value, out);
            for case in cases {
                for label in &case.labels {
                    calls_in_expr(label, out);
                }
                calls_in_block(&case.body, out);
            }
        }
        cpp::Statement::Break | cpp::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &cpp::Expr, out: &mut Vec<String>) {
    match expr {
        cpp::Expr::Call { callee, args } => {
            if let cpp::Expr::Path(path) = callee.as_ref()
                && let Some(last) = path.last()
            {
                let name = last.as_str();
                // Every generated routine's name, not only the
                // defaulters': the callee of a codec call is always a
                // one-segment path this generator built, and the
                // defaulter set looks each name up in its own index, so
                // an encoder's name in the list adds nothing to it.
                // [`reachable_from`] needs them all.
                if name.starts_with("zd_") && !out.iter().any(|seen| seen == name) {
                    out.push(name.to_string());
                }
            }
            calls_in_expr(callee, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        cpp::Expr::MethodCall { receiver, args, .. } => {
            calls_in_expr(receiver, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        cpp::Expr::Member { value, .. }
        | cpp::Expr::Arrow { value, .. }
        | cpp::Expr::AddressOf(value)
        | cpp::Expr::Deref(value)
        | cpp::Expr::Paren(value)
        | cpp::Expr::Cast { value, .. } => calls_in_expr(value, out),
        cpp::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        cpp::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        cpp::Expr::Unary { value, .. } => calls_in_expr(value, out),
        cpp::Expr::Construct { args, .. } | cpp::Expr::InitList(args) => {
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        cpp::Expr::Literal(_) | cpp::Expr::Path(_) | cpp::Expr::SizeOf(_) => {}
    }
}

/// Build the defaulter for each of `types`, prototypes first.
///
/// The shape `crate::emit::decode::decoder_over` uses, and for the same
/// reason: a defaulter may call another, and C++ resolves a call to a
/// free function against what is declared above it.
///
/// # Errors
/// [`EncodeError`] for a construct with no C++ default.
pub fn cpp_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<cpp::Decl>, EncodeError> {
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
        prototypes.push(function(ast, id, &function_name, None)?);
        bodies.push(function(ast, id, &function_name, Some(body))?);
    }
    prototypes.extend(bodies);
    Ok(prototypes)
}

/// `static void <name>(<T> &v)`, with or without a body.
///
/// A reference for every type, including an array: `std::array` is a
/// value and does not decay, so the C column's split between a pointer
/// parameter and a bare array has no counterpart.
///
/// No `Reader`. A default is a property of the type and reads nothing,
/// so a defaulter that took one could fail, and a default value that can
/// fail is not a default value.
///
/// # Errors
/// [`EncodeError::NoWriter`] when the materialiser has no C++ type for
/// `id` — refused by name rather than spelled approximately, which is
/// the rule `adapter/cpp.rs` states for every type it cannot write.
fn function(
    ast: &Ast,
    id: Id<TypeNode>,
    function_name: &str,
    body: Option<cpp::Block>,
) -> Result<cpp::Decl, EncodeError> {
    let type_ = cpp_value_type(ast, id, function_name).map_err(|error| EncodeError::NoWriter {
        member: cpp_type_tag(ast, id),
        kind: match error {
            crate::adapter::cst::MaterialiseError::NoLanguageForm { .. } => {
                "type with no C++ form, whose Table 9 default therefore has nowhere to go"
            }
            _ => "type whose C++ spelling does not evaluate",
        },
    })?;
    Ok(cpp::Decl::Function(cpp::Function {
        doc: Doc::none(),
        returns: cpp::Type::Void,
        name: Ident::new(function_name),
        params: vec![cpp::Param {
            type_: cpp::Type::Ref(Box::new(type_)),
            name: Some(Ident::new("v")),
            default: None,
        }],
        is_inline: false,
        is_static: true,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: false,
        body: body.map_or(cpp::FunctionBody::None, cpp::FunctionBody::Block),
    }))
}

#[allow(clippy::too_many_lines)]
fn body_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<cpp::Block>, EncodeError> {
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
        TypeKind::Primitive(kind) => vec![assign(name("v"), primitive_default(*kind))],

        // `v.clear()` and not `v = ""`: the same value, and the member
        // function says the type owns storage where the assignment would
        // read as a conversion from a literal.
        //
        // Both widths. A `wstring` is UTF-8 in this column — the reason
        // is in `adapter/cpp.rs` — and the empty string is the same
        // octets either way.
        TypeKind::String { .. } => vec![clear("v")],

        // `fixed<P,S>` is a decimal string here, and
        // `canonical::decimal` accepts exactly one spelling of zero for
        // a given scale. Not `clear()`: an empty string is not a
        // decimal, and the C column writes the same digits.
        TypeKind::Fixed { scale, .. } => {
            vec![assign(name("v"), string_literal(&zero_decimal(*scale)))]
        }

        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "typedef with an unresolved target",
                });
            };
            vec![cpp::Statement::Expr(cpp::Expr::Call {
                callee: Box::new(path(&defaulter_name(ast, target))),
                args: vec![name("v")],
            })]
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10.
        TypeKind::Enumeration { .. } => {
            let Some(literal) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(EncodeError::NoWriter {
                    member: own_name.to_string(),
                    kind: "enumeration with no enumerator to take as its default",
                });
            };
            vec![assign(
                name("v"),
                path(&crate::emit::cpp::enumerator_constant_name(
                    ast, id, &literal,
                )),
            )]
        }

        // No bit set: the holder's Table 9 default is zero and zero sets
        // none. The type is an integer alias, so a plain zero is a value
        // of it.
        TypeKind::Bitmask { .. } => vec![assign(name("v"), cpp::Expr::Literal(Literal::int(0)))],

        TypeKind::Bitset { .. } => {
            let mut statements = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding and holds no value.
                if let Some(field_name) = field_node.name.as_ref() {
                    statements.push(assign(
                        member("v", &cxx_name(field_name)),
                        cpp::Expr::Literal(Literal::int(0)),
                    ));
                }
            }
            statements
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries — which are a `std::vector` of pairs
        // here, so one call answers both.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => vec![clear("v")],

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
                access = cpp::Expr::Index {
                    value: Box::new(access),
                    index: Box::new(name(&format!("zd_i{level}"))),
                };
            }
            let mut inner = cpp::Block {
                statements: vec![default_into(ast, element, access)],
            };
            for (level, length) in lengths.iter().enumerate().rev() {
                inner = cpp::Block {
                    statements: vec![counted_loop(&format!("zd_i{level}"), *length, inner)],
                };
            }
            inner.statements
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut statements = Vec::new();
            for member_id in &ast.all_members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                let field = cxx_name(&member_node.name);
                if member_node.facets.is_optional() {
                    // The member is NULL and nothing else. §7.2.2.4.4.4.7:
                    // "Implementations shall not provide a default value
                    // in such a case." An `@optional` member is a plain
                    // pointer (XTypes 1.3 §7.5.1.2.4.2) and an `@external`
                    // one a `std::shared_ptr`; `destroy` lets go of what
                    // the member held and leaves it empty for both.
                    statements.push(cpp::Statement::Expr(cpp::Expr::Call {
                        callee: Box::new(cpp::Expr::Path(Path::new(&[
                            "zerodds", "wire", "destroy",
                        ]))),
                        args: vec![member("v", &field)],
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
            let mut statements = vec![default_into(
                ast,
                discriminator,
                member("v", crate::adapter::cpp::UNION_DISCRIMINATOR),
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, discriminator)? {
                // One field per case directly on the class, so there is
                // no `_u` to go through. `adapter/cpp.rs` says why: two
                // cases may carry the same type, which a `std::variant`
                // could not tell apart.
                statements.push(default_into(
                    ast,
                    member_type,
                    member("v", &crate::adapter::cpp::union_storage(&field)),
                ));
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
        // Refused by name rather than passed over, exactly as the C
        // column refuses them. `any` is in the model because IDL 4.2 has
        // it; DDS-XTypes has no `any` and no encoding for one, and only
        // CORBA CDR writes it — as a TypeCode ahead of the value.
        // Returning `Ok(None)` here would generate a codec that silently
        // omitted the member.
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
    // An empty aggregate has nothing to default and its `v` would be
    // the one unused parameter in the file — the probes compile with
    // `-Werror=unused-parameter`, and `empty-struct-in-sequence` found
    // the defaulter this way. The codec functions suppress
    // unconditionally; here only the empty body needs it.
    let mut statements = statements;
    if statements.is_empty() {
        statements.push(cpp::Statement::Expr(cpp::Expr::Cast {
            kind: cpp::CastKind::Static,
            type_: Box::new(cpp::Type::Void),
            value: Box::new(name("v")),
        }));
    }
    Ok(Some(cpp::Block { statements }))
}

/// Table 9's primitive rows as C++ initialisers.
fn primitive_default(kind: PrimitiveKind) -> cpp::Expr {
    match kind {
        PrimitiveKind::Boolean => cpp::Expr::Literal(Literal::Bool(false)),
        // `'\0'`, written as the number it is: `read_char` hands back a
        // code point and the field holds one.
        PrimitiveKind::Char | PrimitiveKind::WChar => cpp::Expr::Literal(Literal::int(0)),
        PrimitiveKind::Float32 => cpp::Expr::Literal(Literal::Float("0.0f".to_string())),
        PrimitiveKind::Float64 => cpp::Expr::Literal(Literal::Float("0.0".to_string())),
        // The `long double` column carries a `zerodds::wire::Float128`,
        // whose zero is its zero bit pattern. `Float128::zero()` is the
        // runtime's name for it, so the spelling does not depend on the
        // struct's layout — and `Float128{}` would not do: the type is
        // an aggregate of two `std::uint64_t` and its zero *is* two
        // zeroes today, which is a fact about the representation and not
        // about the value.
        PrimitiveKind::Float128 => cpp::Expr::Call {
            callee: Box::new(cpp::Expr::Path(Path::rooted(&[
                "zerodds", "wire", "Float128", "zero",
            ]))),
            args: Vec::new(),
        },
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => cpp::Expr::Literal(Literal::int(0)),
    }
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
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let Some(selector) = discriminator_default_value(ast, discriminator) else {
        return Ok(None);
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, cxx_name(&member.name))))
}

/// The integer a defaulted discriminator has, for label comparison.
fn discriminator_default_value(ast: &Ast, discriminator: Id<TypeNode>) -> Option<i128> {
    // Resolved by the counted loop and not by an `Alias` arm calling
    // this function again: a `typedef` cycle is a finding the model's
    // own invariant pass reports rather than a shape it rejects, and
    // the recursive form read such a chain with nothing counting it.
    let discriminator = resolve_alias(ast, discriminator);
    match ast.type_(discriminator).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { .. }) => {
            // The first literal's *value*, which `@value` may have moved
            // away from zero.
            let literal = crate::emit::default_enumerator_name(ast, discriminator)?;
            for enumerator in ast.enumerators_of(discriminator) {
                // `None`, not `continue`. This function answers "the
                // discriminator's default value", and the line below already
                // says `None` when an enumerator's value does not evaluate.
                // Skipping the enumerator instead makes the *next* one the
                // default — a union that selects a different arm than the
                // source declared, with nothing to show for it.
                let Ok(node) = ast.enumerator(*enumerator) else {
                    return None;
                };
                if node.name == literal {
                    return ast.effective_enumerator_value(*enumerator);
                }
            }
            None
        }
        // Every other discriminator kind IDL 4.2 §7.4.1.4.4.4.2 allows —
        // integer, `char`, `boolean` — has zero for its Table 9 default,
        // and `'\0'` and `false` are zero.
        Ok(TypeKind::Primitive(_)) => Some(0),
        _ => None,
    }
}

/// An explicit `@default` as a C++ initialiser for `type_`.
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<cpp::Expr, EncodeError> {
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
            Ok(cpp::Expr::Literal(Literal::Bool(*flag)))
        }
        // The code point as a number, not a character literal: the
        // member holds a code point and `cpp_primitive` gives `wchar` a
        // fixed-width integer, because the platform's `wchar_t` is not
        // the wire's code unit.
        (ConstExpr::Literal(AstLiteral::Char(ch)), _) => {
            Ok(cpp::Expr::Literal(Literal::int(i128::from(u32::from(*ch)))))
        }
        (ConstExpr::Literal(AstLiteral::WChar(code)), _) => {
            Ok(cpp::Expr::Literal(Literal::int(i128::from(*code))))
        }
        (ConstExpr::Literal(AstLiteral::String(text)), _) => Ok(string_literal(text)),
        (ConstExpr::Literal(AstLiteral::WString(codes)), _) => {
            let mut text = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                text.push(ch);
            }
            Ok(string_literal(&text))
        }
        (ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }), kind) => {
            // `long double` is `zerodds::wire::Float128`, a class with
            // no literal, and the compiler's own `long double` is three
            // different formats across the platforms this project builds
            // for — `adapter/cpp.rs` refuses a `long double` *constant*
            // for exactly this reason, and a `@default` is the same
            // problem one declaration along.
            if matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128))) {
                return Err(refuse(
                    "a `@default` on a `long double`, which has no C++ literal",
                ));
            }
            // A `double` initialiser narrowing into a `float` is what
            // `-Wconversion` reports, and the probe is compiled with
            // `-Werror`.
            let suffix = match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Float32)) => "f",
                _ => "",
            };
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            Ok(cpp::Expr::Literal(Literal::Float(format!(
                "{mantissa}{exponent}{suffix}"
            ))))
        }
        (ConstExpr::Literal(AstLiteral::Fixed { digits, scale }), _) => {
            Ok(string_literal(&fixed_text(digits, *scale)))
        }
        (ConstExpr::Literal(AstLiteral::Enumerator(reference)), _) => {
            let target = reference
                .resolved()
                .ok_or_else(|| refuse("a `@default` enumerator that is unresolved"))?;
            let enumerator = ast
                .enumerator(target)
                .map_err(|_| refuse("a `@default` enumerator this frontend cannot follow"))?;
            Ok(path(&crate::emit::cpp::enumerator_constant_name(
                ast,
                resolved,
                &enumerator.name,
            )))
        }
        (_, kind) => {
            // An integer literal, a named constant or arithmetic over
            // them, folded the same way a bound or a case label is.
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            // A decimal literal with no suffix takes the first of `int`,
            // `long` and `long long` that holds it, and a value above
            // `i64::MAX` holds in none of them — which clang reports as
            // `-Wimplicitly-unsigned-literal`. `adapter/cpp.rs` writes
            // the same suffix for the same reason.
            //
            // The other end of the same range: −2^63 is a minus over a
            // decimal one past `LLONG_MAX` and so is not a literal
            // either — `zerodds_lang_render::cpp::integer_text` carries
            // the clause and answers with the subtraction.
            Ok(cpp::Expr::Literal(if value > i128::from(i64::MAX) {
                Literal::Int(format!("{value}u"))
            } else {
                let _ = kind;
                Literal::Int(zerodds_lang_render::cpp::integer_text(value))
            }))
        }
    }
}

// -- the small builders this file keeps ---------------------------------

fn name(text: &str) -> cpp::Expr {
    cpp::Expr::Path(Path::single(text))
}

/// A name that may already carry its own scoping, as
/// [`Cpp::enumerator_constant`] and [`defaulter_name`] hand it over.
fn path(text: &str) -> cpp::Expr {
    let rooted = text.starts_with("::");
    let segments: Vec<&str> = text.trim_start_matches("::").split("::").collect();
    cpp::Expr::Path(if rooted {
        Path::rooted(&segments)
    } else {
        Path::new(&segments)
    })
}

fn string_literal(text: &str) -> cpp::Expr {
    cpp::Expr::Literal(Literal::Str(text.to_string()))
}

fn member(base: &str, field: &str) -> cpp::Expr {
    cpp::Expr::Member {
        value: Box::new(name(base)),
        name: Ident::new(field),
    }
}

fn clear(base: &str) -> cpp::Statement {
    cpp::Statement::Expr(cpp::Expr::MethodCall {
        receiver: Box::new(name(base)),
        name: Ident::new("clear"),
        args: Vec::new(),
    })
}

fn assign(place: cpp::Expr, value: cpp::Expr) -> cpp::Statement {
    cpp::Statement::Expr(cpp::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(value),
    })
}

/// `for (std::size_t i = 0; i < count; i = i + 1)`, the same spelling
/// [`Cpp::count_up`] writes, so an array's defaulter and its decoder
/// read alike.
fn counted_loop(index: &str, count: i128, body: cpp::Block) -> cpp::Statement {
    cpp::Statement::For {
        init: Box::new(cpp::Statement::Declare {
            type_: cpp::Type::named(Path::new(&["std", "size_t"])),
            name: Ident::new(index),
            value: Some(cpp::Expr::Literal(Literal::int(0))),
        }),
        condition: cpp::Expr::Binary {
            op: "<",
            lhs: Box::new(name(index)),
            rhs: Box::new(cpp::Expr::Literal(Literal::Int(format!("{count}u")))),
        },
        step: cpp::Expr::Binary {
            op: "=",
            lhs: Box::new(name(index)),
            rhs: Box::new(cpp::Expr::Binary {
                op: "+",
                lhs: Box::new(name(index)),
                rhs: Box::new(cpp::Expr::Literal(Literal::int(1))),
            }),
        },
        body,
    }
}
