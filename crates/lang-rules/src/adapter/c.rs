// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::cst`], which does the same for
//! Rust. Reading the two side by side is the point: where they differ,
//! the difference is C's, and it is written down here rather than
//! spread through a renderer.
//!
//! Values are resolved from the model, never from the overlay — the
//! same rule the Rust materialiser follows and for the same reason
//! (spec §2.3).
//!
//! ## The three things C forces that Rust does not
//!
//! 1. **A name for every type.** Rust writes `Vec<i32>` where a member
//!    needs it; C has no generics, so `sequence<long>` has to become a
//!    declared type with a name of its own. Those names are structural —
//!    `zd_seq_int32_t` — so two identical anonymous collections collapse
//!    onto one declaration instead of two that differ only in a counter.
//! 2. **Declaration before use.** The Rust materialiser walks the model
//!    in declaration order and stops. Here the walk is a post-order over
//!    the type graph, because a C translation unit that names a struct
//!    before declaring it does not compile.
//! 3. **No enumeration wide enough.** A C enumeration constant is an
//!    `int` (C11 §6.7.2.2), and `@bit_bound(32)` admits enumerator
//!    values up to `2^32 - 1`, which an `int` cannot hold. So an IDL
//!    enumeration becomes a fixed-width integer typedef plus one
//!    constant per enumerator, and the C `enum` is used only where every
//!    value fits. Emitting the `enum` unconditionally would make the
//!    holder width implementation-defined for exactly the cases the
//!    interop corpus tests.
//!
//! ## The memory shapes
//!
//! | IDL | C |
//! |---|---|
//! | `string` | `const char *` |
//! | `wstring` | `const char *`, holding UTF-8 |
//! | `fixed<P,S>` | `const char *`, holding the decimal |
//! | `sequence<T>` | `struct { uint32_t _maximum; uint32_t _length; T *_buffer; bool _release; }` |
//! | array | a typedef of the C array |
//! | `map<K,V>` | `struct { uint32_t _maximum; uint32_t _length; Entry *_buffer; bool _release; }` of `struct { K key; V value; }`, named `[key][element]Map`, with the operations of Table 47 (see [`c_map`](super::c_map)) |
//! | `union` | `struct { D _d; union { ... } _u; }` |
//! | `@optional T m` | `T *m;`, NULL for an absent member (a `string` stays `const char *`) |
//! | `@external T m` | `T *m;` (a `string` stays `const char *`), with the functions of [`c_lifecycle`](crate::adapter::c_lifecycle) |
//!
//! A `wstring` holds UTF-8 rather than `uint16_t` units on purpose: it
//! puts the §7.4.1.1.2 conversion — the surrogate rule the byte count
//! depends on — inside the C runtime, where it is a second
//! implementation of the clause and not a copy of the Rust one's answer.
//!
//! ## Where the IDL spelling of a member is kept
//!
//! C has no keyword the generated identifier could keep, so
//! [`c::escape_keyword`] moves a colliding name — `long static;` becomes
//! `int32_t static_;`. That rename is not only a readability question:
//! XTypes 1.3 §7.3.1.2.1.1 computes a member's ID under `@autoid(HASH)`
//! and under `@hashid` from the first four octets of the MD5 of its
//! **IDL** name, so a header that only knows `static_` cannot compute
//! the ID for `static` and the member it writes lands under an ID the
//! peer does not have.
//!
//! Every declaration that has named parts therefore carries them in one
//! macro of its own, beside the declaration:
//!
//! ```c
//! #define ZD_IDL_MEMBERS_package_Sample { { "impl", "impl" }, \
//!     { "static_", "static" } }
//! ```
//!
//! and [`MEMBER_NAME`] is the record the entries fit, so a caller writes
//!
//! ```c
//! static const zd_member_name names[] = ZD_IDL_MEMBERS_package_Sample;
//! ```
//!
//! and has the mapping, in IDL declaration order — which is the order
//! the member IDs are assigned in — with the C spelling on the left and
//! the IDL spelling on the right.
//!
//! Three forms were measured against the flag set the probe compiles
//! with (`-std=c99 -Wall -Wextra -Wpedantic -Wconversion
//! -Wsign-conversion -Werror`); this one is what is left:
//!
//! - **A table as an object** — `static const zd_member_name
//!   zd_names_T[] = { … };` — is what C# and Python effectively have,
//!   and is the form this tree cannot write at all: [`c::Variable`]
//!   carries no array declarator, so adding one would be a change to
//!   `zerodds-lang-cst` made for this. The reason not to make it is the
//!   one the constants above are `#define`s for: the object is a
//!   separate object in every translation unit that includes the header.
//!   Whether an unused one is also diagnosed depends on the compiler —
//!   GCC documents `-Wunused-const-variable` as enabled by
//!   `-Wunused-variable` for C, and Apple clang 21 was silent about a
//!   header-provided table under the flag set above — so the diagnostic
//!   is a hazard here and not the measurement.
//! - **An X-macro block** — `#define ZD_MEMBERS_T(X) X(0, static_,
//!   "static") …` — enumerates and costs nothing, and its replacement
//!   list is a run of juxtaposed invocations, which is not a C
//!   expression at all. It could only reach the output as raw text
//!   smuggled through an [`Ident`], in a tree whose whole point is that
//!   it is not text.
//! - **One `#define` per member** — `#define ZD_IDL_NAME_T_static_
//!   "static"` — is expressible and costs nothing, and its name is
//!   ambiguous to anything reading the header: `ZD_IDL_NAME_a_b_c` does
//!   not say where the type name ends and the field name begins, and IDL
//!   scopes flatten with the same underscore.
//!
//! The braced initialiser is a C expression this tree already carries
//! ([`c::Expr::Initializer`]), it puts both spellings of a member in one
//! entry rather than in two lists that can disagree, it is enumerable —
//! by the preprocessor for a C caller, by a line reader for a tool — and
//! a macro that nothing expands occupies no storage and is not
//! diagnosable. A second declaration reaching the same macro name is a
//! redefinition and fails loudly, which is the property a silent
//! collision would not have.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident, Literal};

/// The support typedefs every generated translation unit needs.
///
/// Emitted once, ahead of everything the model contributes: a `string`
/// has the same C shape wherever it appears, and repeating the typedef
/// per use would not compile.
#[must_use]
pub fn prelude() -> Vec<c::Decl> {
    let mut out = vec![
        c::Decl::Typedef(c::Typedef {
            doc: Doc::lines(&[
                "A `string`. The bytes are the caller's and are not owned here:",
                "nothing in the write path allocates.",
            ]),
            type_: c::Type::Pointer(Box::new(c::Type::Const(Box::new(c::Type::Primitive(
                c::Primitive::Char,
            ))))),
            name: Ident::new(STRING),
            dims: Vec::new(),
        }),
        c::Decl::Typedef(c::Typedef {
            doc: Doc::lines(&[
                "A `wstring`, held as UTF-8. The runtime converts to UTF-16 on",
                "the way out, so the surrogate rule of XTypes 1.3 7.4.1.1.2 is",
                "implemented in C and not taken from the generator.",
            ]),
            type_: c::Type::Pointer(Box::new(c::Type::Const(Box::new(c::Type::Primitive(
                c::Primitive::Char,
            ))))),
            name: Ident::new(WSTRING),
            dims: Vec::new(),
        }),
    ];
    // The record the `ZD_IDL_MEMBERS_…` macros are written to fit. A
    // type and not an object: a `typedef` at file scope that nothing
    // uses is not diagnosable, so every generated translation unit can
    // carry the shape of the mapping whether or not it reads it.
    out.extend(struct_typedef(
        MEMBER_NAME,
        vec![
            c::Member {
                doc: Doc::line("The identifier as this header spells it."),
                type_: c::Type::Named(Ident::new(STRING)),
                name: Ident::new("field"),
                dims: Vec::new(),
                bits: None,
            },
            c::Member {
                doc: Doc::line("The identifier as the IDL source spelled it."),
                type_: c::Type::Named(Ident::new(STRING)),
                name: Ident::new("idl"),
                dims: Vec::new(),
                bits: None,
            },
        ],
        Doc::lines(&[
            "One member's two spellings.",
            "",
            "Load-bearing, not documentation. A member's ID under @autoid(HASH)",
            "and under @hashid is the first four octets of the MD5 of its IDL",
            "name (XTypes 1.3 7.3.1.2.1.1), so a caller holding only the C",
            "spelling — which a keyword collision moved — cannot compute the ID",
            "of the member it is writing.",
            "",
            "The entries come out of the `ZD_IDL_MEMBERS_<type>` macro beside",
            "each declaration, in IDL declaration order:",
            "",
            "    static const zd_member_name names[] = ZD_IDL_MEMBERS_a_Sample;",
        ]),
    ));
    out
}

/// The C name of the `string` support type.
pub const STRING: &str = "zd_string";
/// The C name of the `wstring` support type.
pub const WSTRING: &str = "zd_wstring";
/// The C name of the record one entry of an IDL-name table fits.
pub const MEMBER_NAME: &str = "zd_member_name";
/// The prefix of the macro carrying a declaration's own IDL name.
///
/// The same escape that moves a member moves a type — `struct class`
/// becomes `class_` — and a type's name reaches the wire twice over: as
/// `TypeObject`'s `type_name` and as the topic type name a peer matches
/// on. The C spelling cannot be inverted back to it, because
/// [`c::escape_keyword`] leaves a name that was already spelled `class_`
/// alone and both arrive at the same identifier.
pub const IDL_NAME_PREFIX: &str = "ZD_IDL_NAME_";
/// The prefix of the macro carrying a declaration's IDL member names.
///
/// The rest of the name is the declaration's own C name, which is
/// already the flattened scoped name — so the macro is unique wherever
/// the type is, and two that were not would be a redefinition the
/// preprocessor reports rather than a mapping one of them lost.
pub const IDL_MEMBERS_PREFIX: &str = "ZD_IDL_MEMBERS_";
/// The C name of the `long double` support type.
///
/// The runtime's `zdw_f128` and never the compiler's `long double`: that
/// type is x87 80-bit extended on x86-64, binary128 on aarch64 and a
/// plain `double` on macOS arm64, so a generated structure declaring it
/// would have a different wire format on each. `runtime/c/include/
/// zerodds_wire.h` carries the measurements.
pub const FLOAT128: &str = "zdw_f128";

/// Build the C declarations a rule application produced.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, or a type has no C shape — never a silently dropped
/// declaration.
pub fn c_items(ast: &Ast, applied: &Applied) -> Result<Vec<c::Decl>, MaterialiseError> {
    let mut out = prelude();
    // `@verbatim(placement = BEGIN_FILE)`, wherever it was written,
    // opens the file — OMG IDL 4.2 §8.3.5.1. After the prelude, which
    // is the earliest position this declaration list controls; the
    // caller's own includes stand above the list either way.
    let (begin_file, end_file) = verbatim::file_slots(ast, verbatim::aliases::C);
    out.extend(begin_file.into_iter().map(c::Decl::Verbatim));
    let order = ordered_types(ast);
    no_bits_name_collision(ast, &order)?;
    // Every named aggregate as an incomplete type first. The post-order
    // walk puts a dependency before its user, except where the graph
    // has a cycle — `struct A { sequence<B> bs; }; struct B {
    // sequence<A> as; }` came out with `m_A *_buffer` above the
    // `typedef` that names `m_A`, and the header did not compile. A
    // pointer to an incomplete type is what C offers for exactly this,
    // and a legal cycle always crosses one (a by-value cycle has no
    // layout and is refused before it gets here). The duplicate
    // typedefs the walk then produces are removed below, because
    // `-std=c99` calls a typedef redefinition an error.
    for id in &order {
        let Ok(node) = ast.type_(*id) else { continue };
        if matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. }
        ) {
            let name = c_type_name(ast, *id);
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::none(),
                type_: c::Type::StructTag(Ident::new(&name)),
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
        }
    }
    let mut emitted: Vec<String> = Vec::new();
    // A map's entry struct embeds its key and value by value, and a
    // value on a cycle (`struct Node { map<long, Node> children; }`)
    // is incomplete at the map's position in the walk. Nothing holds
    // an entry except the map's `_buffer` pointer, so every entry
    // definition can wait until all named types are complete.
    let mut deferred: Vec<c::Decl> = Vec::new();
    let lifecycle_order = order.clone();
    for id in order {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut deferred, &mut emitted)?;
        weave_verbatim(ast, id, start, &mut out);
    }
    out.append(&mut deferred);
    // The walk's own `typedef struct N N;` lines repeat the forwards;
    // keep the first of each and drop the rest.
    let mut seen_typedefs: Vec<String> = Vec::new();
    out.retain(|decl| {
        if let c::Decl::Typedef(typedef) = decl {
            if let c::Type::StructTag(tag) = &typedef.type_ {
                if tag.as_str() == typedef.name.as_str() {
                    let name = typedef.name.as_str().to_string();
                    if seen_typedefs.contains(&name) {
                        return false;
                    }
                    seen_typedefs.push(name);
                }
            }
        }
        true
    });

    // Constants, exceptions and natives are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and the Rust materialiser does the same.
    for (id, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "CConst").is_none() {
            continue;
        }
        let _ = id;
        // `eval_const` and not `eval_integral`. Every constant IDL 4.2
        // admits was asked the integral question here, so
        // `const string MODULE_ID = "…";` — a constant that evaluates
        // perfectly well and is not a number — came back as
        // `Unevaluated` and was not emitted at all. The integral
        // question is simply the wrong one for four of the eight forms.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        // A `#define` and not a `static const`: a `static const` in a
        // header is a separate object in every translation unit that
        // includes it, and every one that does not use it warns.
        let type_ = node
            .type_
            .resolved()
            .map_or(c::Type::Primitive(c::Primitive::Int32), |target| {
                c_value_type(ast, target)
            });
        let literal = c_constant_literal(&value, &type_, &node.name)?;
        out.push(c::Decl::Define(c::Define {
            doc: Doc::none(),
            name: Ident::new(&c::escape_keyword(&node.name)),
            value: Some(c::Expr::Paren(Box::new(c::Expr::Cast {
                type_: Box::new(type_),
                value: Box::new(c::Expr::Literal(literal)),
            }))),
        }));
    }

    for (id, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "CError").is_none() {
            continue;
        }
        let _ = id;
        let mut members = Vec::new();
        let mut names = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            let field = c::escape_keyword(&member_node.name);
            names.push((field.clone(), member_node.name.clone()));
            members.push(c::Member {
                doc: Doc::none(),
                type_: c_value_type(ast, target),
                name: Ident::new(&field),
                dims: Vec::new(),
                bits: None,
            });
        }
        let exception_name = c::escape_keyword(&node.name);
        out.extend(struct_typedef(
            &exception_name,
            members,
            Doc::line("An exception, as the structure its members make."),
        ));
        out.extend(idl_members(&exception_name, &names)?);
        out.push(idl_name(&exception_name, &node.name));
    }

    // The initialiser, destructor and copy functions of the aggregates
    // that hold an external member (XTypes 1.3 §7.5.1.2.3.1), after every
    // type they name and before the text that closes the file.
    out.extend(super::c_lifecycle::lifecycle(ast, &lifecycle_order)?);

    // The operations of Table 47 on every map, and the maps of every
    // type (XTypes 1.3 §7.5.1.3.2), after every type they name.
    out.extend(super::c_map::maps(ast, &lifecycle_order, &emitted)?);

    // `@verbatim(placement = END_FILE)` closes the file, after even the
    // constants and exceptions. OMG IDL 4.2 §8.3.5.1.
    out.extend(end_file.into_iter().map(c::Decl::Verbatim));

    Ok(out)
}

/// Weave one type's `@verbatim` text around what `declare` just pushed.
/// OMG IDL 4.2 §8.3.5.1; the slot semantics are [`crate::verbatim`]'s.
///
/// `start` is where the type's declarations begin in `out`.
/// `BEGIN_DECLARATION` and `END_DECLARATION` open and close the member
/// list of the type's `struct`; an enumeration and a typedef have no body
/// to hold raw text, so for them the text lands adjacent to the
/// declaration — the nearest position the tree can express, never a
/// dropped directive.
fn weave_verbatim(ast: &Ast, id: Id<TypeNode>, start: usize, out: &mut Vec<c::Decl>) {
    verbatim::weave(
        verbatim::declaration_slots(ast, id, verbatim::aliases::C),
        &c_type_name(ast, id),
        start,
        out,
    );
}

impl verbatim::Carrier for c::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Struct(node) = self else {
            return false;
        };
        if node.name.as_ref().map(Ident::as_str) != Some(name) {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// Every type in the model, dependencies first.
///
/// Post-order over the type graph. A type already visited is not
/// revisited, which also means a cycle stops rather than recurses — a
/// recursive IDL type has no C declaration order at all, and
/// [`c_codec`](crate::encode_c::c_codec) is where that is reported.
#[must_use]
pub fn ordered_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    let all: Vec<Id<TypeNode>> = ast.types().map(|(id, _)| id).collect();
    let order = ordered_types_from(ast, &all);
    // The node every `any` points at stays in the model when the types
    // that held one have been left out (`Ast::drop_outside_profile`).
    // Nothing reaches it then, and a codec for it is a refusal of a
    // construct no declaration uses. One that something still reaches is
    // kept, so that the refusal by name stays where it is needed.
    order
        .iter()
        .copied()
        .filter(|id| {
            let no_data_form = ast
                .type_(*id)
                .is_ok_and(|node| matches!(node.kind, TypeKind::Any | TypeKind::ValueBase));
            !no_data_form
                || order
                    .iter()
                    .any(|other| other != id && dependencies(ast, *other).contains(id))
        })
        .collect()
}

/// The same walk, from named roots instead of from the whole model.
///
/// What a translation unit for one topic type needs, and no more. The
/// distinction matters in C and not in Rust: a `static` function nothing
/// calls is a warning, so emitting a codec for a type the root cannot
/// reach would make every generated file noisy — and a generator whose
/// output warns trains its users to stop reading warnings.
#[must_use]
pub fn ordered_types_from(ast: &Ast, roots: &[Id<TypeNode>]) -> Vec<Id<TypeNode>> {
    ordered_types_from_pruning(ast, roots, &[])
}

/// [`ordered_types_from`] that neither lists a type in `pruned` nor follows
/// its dependencies: what a type reaches only through a pruned type is not
/// needed, as the pruned type's code is written elsewhere.
#[must_use]
pub fn ordered_types_from_pruning(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    pruned: &[Id<TypeNode>],
) -> Vec<Id<TypeNode>> {
    let mut order = Vec::new();
    let mut seen: Vec<Id<TypeNode>> = pruned.to_vec();
    for root in roots {
        visit(ast, *root, &mut order, &mut seen, 0);
    }
    order
}

/// zerodds-lint: recursion-depth 64 (dependency edges of the type
/// graph; bounded by `MAX_DEPTH` and by `seen`, which admits each
/// type once, so a cycle stops on the second visit)
fn visit(
    ast: &Ast,
    id: Id<TypeNode>,
    order: &mut Vec<Id<TypeNode>>,
    seen: &mut Vec<Id<TypeNode>>,
    depth: usize,
) {
    if depth > MAX_DEPTH || seen.contains(&id) {
        return;
    }
    seen.push(id);
    for dependency in dependencies(ast, id) {
        visit(ast, dependency, order, seen, depth + 1);
    }
    order.push(id);
}

/// How deep the walk follows the type graph, matching the layout
/// planner's own bound.
pub(crate) const MAX_DEPTH: usize = 64;

/// The types a type's C declaration has to be preceded by.
///
/// Also the types whose codec a type's codec calls, which is what
/// [`crate::emit::encode::called_only_in_bulk`] reads it for.
pub(crate) fn dependencies(ast: &Ast, id: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut out = Vec::new();
    let Ok(node) = ast.type_(id) else {
        return out;
    };
    match &node.kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            out.extend(element.resolved());
        }
        TypeKind::Map { key, value, .. } => {
            out.extend(key.resolved());
            out.extend(value.resolved());
        }
        TypeKind::Alias { aliased } => out.extend(aliased.resolved()),
        TypeKind::ValueBox { boxed } => out.extend(boxed.resolved()),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            // A base is a type the derived one is declared in terms of
            // (XTypes 1.3 §7.5.1.4.1: a base class, or the member
            // `parent`), so it comes first.
            if let TypeKind::Structure {
                base: Some(base), ..
            } = &node.kind
            {
                out.extend(base.resolved());
            }
            for member in ast.all_members_of(id) {
                if let Ok(member_node) = ast.member(member) {
                    out.extend(member_node.type_.resolved());
                }
            }
        }
        TypeKind::Union { discriminator, .. } => {
            out.extend(discriminator.resolved());
            for case in ast.cases_of(id) {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        out.extend(member.type_.resolved());
                    }
                }
            }
        }
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Native
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {}
    }
    out
}

/// The C spelling of a type in a position that has to hold a value,
/// refusing the constructs C cannot hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided
/// at run time — there is no C type for it and no wire form to generate
/// a codec against. Emitting `any a;` into a header and reporting
/// success produced a file no compiler reads, from a generator that said
/// nothing was wrong.
fn c_member_type(ast: &Ast, target: Id<TypeNode>, at: &str) -> Result<c::Type, MaterialiseError> {
    let construct = match ast.type_(target).map(|node| &node.kind) {
        Ok(TypeKind::Any) => Some("any"),
        Ok(TypeKind::ValueBase) => Some("ValueBase"),
        // An object reference is a handle into the RPC arc and
        // `coverage.rs` says so; there is no C data type for it, and
        // writing the IDL spelling into the header produced a file no
        // compiler reads.
        Ok(TypeKind::ObjectRef { .. }) => Some("Object"),
        _ => None,
    };
    if let Some(construct) = construct {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: "C",
            at: at.to_string(),
        });
    }
    Ok(c_value_type(ast, target))
}

/// The C storage of one member of `enclosing`, which is
/// [`c_member_type`] except where the value has no room there.
///
/// A C structure holds its members as storage, so a member whose type
/// reaches the structure that declares it back — over members, union
/// branches, array elements and aliases, none of which is a pointer in
/// C — is a structure that contains itself, and the compiler answers
/// `field 'next' has incomplete type`. The pointer is what gives such a
/// type a size, and it is what DDS-XTypes 1.3 Table 21 already calls the
/// case: a member "stored externally to the containing type".
///
/// [`zerodds_ast_layout::member_held_inline_closes_a_cycle`] is the
/// question, asked of the *pair* — a member's type alone does not answer
/// it, because the same type is inline in one aggregate and an
/// indirection in another. The cycle is one reason: `@optional Node next`
/// without `@external` closes it. `@external` (§7.5.1.2.3.1) and
/// `@optional` (§7.5.1.2.4.1) are the other, and each makes the member a
/// pointer whether or not it closes a cycle: `@optional long a` is
/// `int32_t *a`.
///
/// The codec answers the same question at the same seam —
/// [`crate::emit::CodecTarget::behind_indirection`] — so what the header
/// declares and what the encoder reaches through cannot part.
fn c_held_member_type(
    ast: &Ast,
    enclosing: Id<TypeNode>,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
    at: &str,
) -> Result<c::Type, MaterialiseError> {
    let held = c_member_type(ast, target, at)?;
    if member_held_behind_pointer(ast, enclosing, member, target) {
        return Ok(c::Type::Pointer(Box::new(held)));
    }
    Ok(held)
}

/// Whether the C value of a type is a pointer already: a `string`, a
/// `wstring` and a `fixed`, which are `const char *`, and an alias of
/// one. XTypes 1.3 §7.5.1.2.3.1: "String and wide string members are
/// already represented using pointers, so the mappings for these members
/// do not change. The same applies to aliases to string and wide string
/// types."
#[must_use]
pub fn value_is_pointer(ast: &Ast, id: Id<TypeNode>) -> bool {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| &node.kind) {
            Ok(TypeKind::String { .. } | TypeKind::Fixed { .. }) => return true,
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// Whether C holds a member behind a pointer, given whether the member
/// is annotated `@external` or `@optional` (`annotated`, see
/// [`pointer_annotated`]).
///
/// Two reasons, and either is enough. A member that closes a cycle has no
/// other way to have a size (`c_held_member_type`). An `@external`
/// member "shall be represented using pointers […] a member of type X
/// shall instead be mapped as type pointer-to-X" (XTypes 1.3
/// §7.5.1.2.3.1), unless the value is a pointer already
/// ([`value_is_pointer`]); an `@optional` member is held the same way
/// (§7.5.1.2.4.1: "Optional members shall be represented using
/// pointers", "A NULL pointer shall indicate an omitted value"). One
/// pointer for all of them: `@external Node next` inside `Node` is
/// `Node *next`.
#[must_use]
pub fn pointer_member(
    ast: &Ast,
    enclosing: Id<TypeNode>,
    target: Id<TypeNode>,
    annotated: bool,
) -> bool {
    zerodds_ast_layout::member_held_inline_closes_a_cycle(ast, enclosing, target)
        || (annotated && !value_is_pointer(ast, target))
}

/// Whether the annotations of `member` alone make C hold it behind a
/// pointer: `@external` (XTypes 1.3 §7.5.1.2.3.1) or `@optional`
/// (§7.5.1.2.4.1). The `external` argument of [`pointer_member`].
#[must_use]
pub fn pointer_annotated(member: &zerodds_ast::MemberNode) -> bool {
    crate::adapter::annotated_external(member) || member.facets.is_optional()
}

/// [`pointer_member`] for the member of `enclosing` that is called
/// `member_name`: a structure's member or a union's branch. For a caller
/// that holds the name and the type and not the node, as the wire probe's
/// generator does.
///
/// # Errors
/// A message naming the aggregate and the name, when `enclosing` declares
/// no such member: the answer would otherwise be the cycle rule's, for a
/// member that may well be `@external`.
pub fn named_member_held_behind_pointer(
    ast: &Ast,
    enclosing: Id<TypeNode>,
    member_name: &str,
    target: Id<TypeNode>,
) -> Result<bool, String> {
    let structure_member = ast
        .all_members_of(enclosing)
        .into_iter()
        .filter_map(|member| ast.member(member).ok())
        .find(|member| member.name == member_name);
    let branch_member = ast
        .cases_of(enclosing)
        .iter()
        .filter_map(|case| ast.case(*case).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .find(|member| member.name == member_name);
    let member = structure_member.or(branch_member).ok_or_else(|| {
        format!(
            "`{}` declares no member or branch `{member_name}`",
            c_type_name(ast, enclosing)
        )
    })?;
    Ok(pointer_member(
        ast,
        enclosing,
        target,
        pointer_annotated(member),
    ))
}

/// [`pointer_member`] for a member as declared.
#[must_use]
pub fn member_held_behind_pointer(
    ast: &Ast,
    enclosing: Id<TypeNode>,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> bool {
    pointer_member(ast, enclosing, target, pointer_annotated(member))
}

/// The member that holds the base of a derived structure.
/// XTypes 1.3 §7.5.1.4.1.3.
pub const PARENT: &str = "parent";

/// The structure `id` derives from, if it does.
#[must_use]
pub fn struct_base(ast: &Ast, id: Id<TypeNode>) -> Option<Id<TypeNode>> {
    match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Structure {
            base: Some(base), ..
        }) => base.resolved(),
        _ => None,
    }
}

/// One member of a structure as C reaches it.
#[derive(Debug, Clone)]
pub struct CMember {
    /// The member.
    pub handle: Id<zerodds_ast::MemberNode>,
    /// The path to it from the structure: empty for its own, `parent.` for the base's,
    /// `parent.parent.` for the base's base.
    pub path: String,
    /// The structure that declares it.
    pub owner: Id<TypeNode>,
}

/// Every member `id` has, the base's first. Same order as [`Ast::all_members_of`].
#[must_use]
pub fn c_members(ast: &Ast, id: Id<TypeNode>) -> Vec<CMember> {
    let mut chain = vec![id];
    while chain.len() < 64 {
        match struct_base(ast, *chain.last().unwrap_or(&id)) {
            Some(base) if !chain.contains(&base) => chain.push(base),
            _ => break,
        }
    }
    let mut out = Vec::new();
    for (depth, owner) in chain.iter().enumerate().rev() {
        let path = format!("{PARENT}.").repeat(depth);
        for member in ast.members_of(*owner) {
            out.push(CMember {
                handle: *member,
                path: path.clone(),
                owner: *owner,
            });
        }
    }
    out
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// One codec body is emitted per routine name, and the name is built
/// from the type's C name. With the bound left out, `sequence<long>`
/// and `sequence<long, 4>` were one name and whichever was declared
/// first decided: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated C held no `zd_over` check at all, so `a` accepted a stream
/// of any length; with the members the other way round both got the
/// bounded body, so `b` refused — and truncated — a length the IDL puts
/// no limit on. `string<4>` beside `string` and `wstring<4>` beside
/// `wstring` collapsed the same way, onto `zdw_utf8_octets` and
/// `zdw_utf16_units`. Either way the reader answered with a value the
/// IDL does not describe and said nothing.
///
/// An `array` needs nothing here: its `dims` are already in the name.
pub(crate) fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `zd_seq_long_4` is what the `Array`
        // arm answers for an array of four `long`, and the two are
        // different wire forms.
        Some(expr) => match ast.eval_integral(expr) {
            Ok(limit) => format!("_b{limit}"),
            // As the array's `_x`: a bound that does not evaluate is
            // still not the unbounded form, and `c_items` reports the
            // expression that would not evaluate.
            Err(_) => "_bx".to_string(),
        },
    }
}

/// The C spelling of a type where a declarator needs one.
///
/// A primitive is its own spelling; everything else is a declared name,
/// because C has no way to write an anonymous aggregate twice and have
/// the two be the same type.
///
/// A `sequence` and a `map` carry their bound here and not only in
/// [`c_codec_tag`], because in this column the name is a *declaration*
/// as well as a routine name: `zd_seq_long` is a struct `declare`
/// emits, and two codecs over one struct would be two functions whose
/// parameter type says nothing about which of the two IDL types they
/// read. `zd_seq_long` and `zd_seq_b4_long` are two structs of the same
/// shape, one per IDL type, and the bounded member is declared at the
/// bounded one — which is what makes the split reach the header and not
/// just the codec.
///
/// A `string` is the exception and takes its bound in [`type_tag`]
/// alone: both shapes are the prelude's [`STRING`] and [`WSTRING`],
/// there is one C type per width whatever the bound, and `declare`
/// emits nothing for either. A declarator with a bound in it would name
/// a type no header defines.
#[must_use]
pub fn c_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    type_name_at(ast, id, 0)
}

/// [`c_type_name`], carrying how many connector steps the walk has
/// already taken.
///
/// zerodds-lint: recursion-depth 64 (unnamed-typedef chains and the
/// element, key and value tags; bounded by `MAX_DEPTH`, which names the
/// type by its arena slot rather than descending). The slot and not a
/// fixed word: two types that both nest past the bound would otherwise
/// take one C name, and the second declaration would silently stand for
/// both.
pub(crate) fn type_name_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("zd_deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return "void".to_string();
    };
    match &node.kind {
        // `long double` is the one primitive with no C built-in that
        // carries it: the spelling is `zdw_f128`, which is binary128
        // whatever the compiler's `long double` happens to be on this
        // target. Matched ahead of the arm below, which would reach
        // `c_primitive`'s `double`. See `runtime/c/include/zerodds_wire.h`
        // and `docs/float128-column.md` §2 for the three formats that
        // name covers.
        TypeKind::Primitive(PrimitiveKind::Float128) => FLOAT128.to_string(),
        TypeKind::Primitive(kind) => c_primitive(*kind).as_str().to_string(),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => STRING.to_string(),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => WSTRING.to_string(),
        TypeKind::Fixed { digits, scale } => format!("zd_fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "zd_seq{}_{}",
                bound_tag(ast, bound),
                type_tag_at(ast, element, depth + 1)
            ),
            None => "void".to_string(),
        },
        TypeKind::Array { element, dims } => match element.resolved() {
            Some(element) => {
                let mut name = format!("zd_arr_{}", type_tag_at(ast, element, depth + 1));
                for dim in dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => name.push_str(&format!("_{length}")),
                        // A dimension that does not evaluate gets no
                        // number rather than a zero: the name has to
                        // differ from a `[0]` array's, and
                        // `c_items` reports the failure by name.
                        Err(_) => name.push_str("_x"),
                    }
                }
                name
            }
            None => "void".to_string(),
        },
        // XTypes 1.3 §7.5.1.3.2.1: `[key_type][fully_qualified_element_type]Map`
        // where the key is a primitive or a string; any other key keeps the
        // structural name, because the clause gives none.
        TypeKind::Map { key, value, bound } => match (key.resolved(), value.resolved()) {
            (Some(key), Some(value)) => super::c_map::map_type_name(ast, key, value, bound, depth)
                .unwrap_or_else(|| {
                    format!(
                        "zd_map{}_{}_{}",
                        bound_tag(ast, bound),
                        type_tag_at(ast, key, depth + 1),
                        type_tag_at(ast, value, depth + 1)
                    )
                }),
            _ => "void".to_string(),
        },
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the C name is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => type_name_at(ast, target, depth + 1),
            None => "void".to_string(),
        },
        _ => flattened(ast, id),
    }
}

/// The name component a collection takes from its element.
///
/// The IDL kind and not the C spelling, and this is not cosmetic:
/// `double` and `long double` are both a C `double` here — Rust has no
/// 128-bit float either — and they are eight and sixteen bytes on the
/// wire. A structural name built from the C type would give
/// `sequence<double>` and `sequence<long double>` one declaration and
/// one encoder, and the second would go out at the first's width with
/// nothing to say it had. The same collapse would hit `octet` against
/// `uint8` and `unsigned short` against `wchar`, where it happens to be
/// harmless.
///
/// A `string`'s bound rides here and not in [`c_type_name`], for the
/// reason [`c_codec_tag`] states: both bounds are one declarator type
/// and two wire forms, so the name has to keep them apart where the
/// declarator cannot.
///
#[must_use]
pub fn type_tag(ast: &Ast, id: Id<TypeNode>) -> String {
    type_tag_at(ast, id, 0)
}

/// [`type_tag`], carrying how many connector steps the walk has already
/// taken.
///
/// The depth is threaded through [`type_name_at`] and back rather than
/// started over at either door: the two reach each other, so a counter
/// reset at one of them would leave the pair with no bound at all. The
/// marker this function used to carry — `recursion-depth 64 (a chain of
/// unnamed typedefs)` — named a bound nothing enforced.
///
/// zerodds-lint: recursion-depth 64 (unnamed-typedef chains, via
/// `type_name_at`; bounded by `MAX_DEPTH`, which names the type by its
/// arena slot rather than descending)
pub(crate) fn type_tag_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("zd_deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return type_name_at(ast, id, depth + 1);
    };
    match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => format!("{STRING}{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => format!("{WSTRING}{}", bound_tag(ast, bound)),
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the tag is the target's — which
        // is what carries a bounded string's bound through one.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => type_tag_at(ast, target, depth + 1),
            None => type_name_at(ast, id, depth + 1),
        },
        _ => type_name_at(ast, id, depth + 1),
    }
}

/// The name a generated codec function is called after.
///
/// [`type_tag`], which is the *name component* function and not the
/// declarator one: the two part on a string, and only on a string. A
/// `string<4>` and a `string` are both the prelude's [`STRING`] — one C
/// type per width whatever the bound, and `declare` emits nothing for
/// either — so `c_type_name` has to keep answering `zd_string` for both
/// or a member would be declared at a type no header defines. The tag
/// carries the bound instead, and `zd_dec_zd_string` beside
/// `zd_dec_zd_string_b4` is the same arrangement the nineteen other
/// columns have for a string: two routines over one type.
///
/// Everything else is its declaration's own name, which already carries
/// the bound — see [`c_type_name`] — so the routine and the struct it
/// reads split together.
#[must_use]
pub fn c_codec_tag(ast: &Ast, id: Id<TypeNode>) -> String {
    type_tag(ast, id)
}

/// The map entry struct a `map<K,V>` needs a name for.
#[must_use]
pub fn c_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
    super::c_map::entry_name(&c_type_name(ast, id))
}

/// A named type's C identifier: the scoped name with the module
/// separators flattened onto it, which is what both C generators in the
/// field do and what a `#include` of two modules requires.
#[must_use]
pub fn flattened(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "zd_unnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => c::escape_keyword(&scoped.segments().join("_")),
        None => "zd_unnamed".to_string(),
    }
}

/// The flattened module path a file-scope constant carries.
#[must_use]
pub fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return String::new();
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = segments[..segments.len() - 1].join("_");
    prefix.push('_');
    prefix
}

/// The C primitive an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a C spelling is
/// a compile error and not a member that silently takes another's.
///
/// `char` is one byte and `wchar` two, which is what the wire form is;
/// XTypes 1.3 §7.5.1.1.1 leaves the C spelling of `Char16` to the
/// binding, and a fixed-width integer is the only choice that does not
/// depend on the platform's `wchar_t`.
///
/// `long double` is **not** routed here: it takes [`FLOAT128`], which is
/// a real binary128. The `Float128` arm below exists so the match is
/// total and answers for a caller that wants the nearest built-in; both
/// callers in this module — [`c_type_name`] and [`c_value_type`] — match
/// `Float128` before they reach it. A caller that does not is the defect
/// the three-column gate found: `double` is eight bytes where the member
/// is sixteen.
#[must_use]
pub const fn c_primitive(kind: PrimitiveKind) -> c::Primitive {
    match kind {
        PrimitiveKind::Boolean => c::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => c::Primitive::UInt8,
        PrimitiveKind::Int8 => c::Primitive::Int8,
        PrimitiveKind::Char => c::Primitive::Char,
        PrimitiveKind::WChar => c::Primitive::UInt16,
        PrimitiveKind::Int16 => c::Primitive::Int16,
        PrimitiveKind::UInt16 => c::Primitive::UInt16,
        PrimitiveKind::Int32 => c::Primitive::Int32,
        PrimitiveKind::UInt32 => c::Primitive::UInt32,
        PrimitiveKind::Int64 => c::Primitive::Int64,
        PrimitiveKind::UInt64 => c::Primitive::UInt64,
        PrimitiveKind::Float32 => c::Primitive::Float,
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => c::Primitive::Double,
    }
}

/// The unsigned integer a bit width calls for.
///
/// Public because the decoder emitter has to cast into exactly this
/// type: a bitfield is unpacked out of a 64-bit anchor and assigned to
/// a member whose width is decided here, and a second copy of the rule
/// would narrow into a type this one had moved on from.
#[must_use]
pub const fn c_holder(bits: u16) -> c::Primitive {
    match bits {
        0..=8 => c::Primitive::UInt8,
        9..=16 => c::Primitive::UInt16,
        17..=32 => c::Primitive::UInt32,
        _ => c::Primitive::UInt64,
    }
}

fn type_by_name(name: &str) -> c::Type {
    c::Type::Named(Ident::new(name))
}

/// The C declarator type a value of `id` is held in.
///
/// The one answer, for every emitter. A member of a generated structure,
/// a codec's `v` parameter, a defaulter's destination, the element type a
/// sequence's buffer is allocated at — all four have to name the same C
/// type or the translation unit does not compile, and where it happens to
/// compile anyway the two names have to be the same *width* or the
/// allocation is short.
///
/// It was three private copies in `encode_c`, `decode_c` and `default_c`
/// beside this one, and that is what let `long double` diverge: this
/// function answers [`FLOAT128`] for it and the copies asked
/// [`c_primitive`], which answers `double` — sixteen bytes against eight,
/// with the codec's `const double *` pointing at a `zdw_f128` member.
#[must_use]
pub fn c_value_type(ast: &Ast, id: Id<TypeNode>) -> c::Type {
    let Ok(node) = ast.type_(id) else {
        return c::Type::Void;
    };
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float128) => type_by_name(FLOAT128),
        TypeKind::Primitive(kind) => c::Type::Primitive(c_primitive(*kind)),
        _ => type_by_name(&c_type_name(ast, id)),
    }
}

/// A type the model declares under the name `<TypeName>Bits` of a
/// bitmask's flag type.
///
/// DDS-XTypes 1.3 §7.5.1.2.2 names that type as part of the binding, so
/// a user type of the same C name cannot be given another name here
/// without the header no longer being the one the clause describes. The
/// two would be one identifier in the header, the second declaration
/// would be skipped as a repeat (`declare`) or rejected by the C
/// compiler, and either way the source would not mean what it says; the
/// refusal names both declarations instead.
///
/// # Errors
/// [`MaterialiseError::SpellingCollision`] when two declarations reach
/// one C name, the second being a bitmask's `<TypeName>Bits`.
fn no_bits_name_collision(ast: &Ast, order: &[Id<TypeNode>]) -> Result<(), MaterialiseError> {
    let mut spelled: Vec<(String, String)> = Vec::new();
    let mut primitives: Vec<String> = Vec::new();
    for id in order {
        let Ok(node) = ast.type_(*id) else { continue };
        // Anonymous collections share a C name by shape, on purpose.
        if node.name.is_empty() {
            continue;
        }
        let name = c_type_name(ast, *id);
        // Two IDL primitives may have one C type (`octet` and `uint8` are
        // both `uint8_t`); that is not a collision of declarations. A
        // primitive stands in the list once per spelling, so that a
        // declared name equal to one is still found.
        if matches!(node.kind, TypeKind::Primitive(_)) {
            if !primitives.contains(&name) {
                primitives.push(name.clone());
                spelled.push((name, node.name.clone()));
            }
            continue;
        }
        if matches!(node.kind, TypeKind::Bitmask { .. }) {
            spelled.push((name.clone(), node.name.clone()));
            spelled.push((
                format!("{name}Bits"),
                format!("{}Bits (the flag type of bitmask {})", node.name, node.name),
            ));
        } else {
            spelled.push((name, node.name.clone()));
        }
    }
    no_spelling_collision(
        spelled
            .iter()
            .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
        "C",
        "the types of one header",
    )
}

#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<c::Decl>,
    deferred: &mut Vec<c::Decl>,
    emitted: &mut Vec<String>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    let name = c_type_name(ast, id);
    // Two anonymous collections of the same shape are one C type. The
    // check is by name because the name is structural.
    if emitted.contains(&name) {
        return Ok(());
    }

    match &node.kind {
        // A primitive needs no declaration; its C spelling is built in.
        TypeKind::Primitive(_) => return Ok(()),
        // The two string shapes are in the prelude.
        TypeKind::String { .. } => return Ok(()),

        TypeKind::Fixed { digits, scale } => {
            emitted.push(name.clone());
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line(&format!(
                    "`fixed<{digits},{scale}>`, held as the decimal it was written as. \
                     A double cannot carry one, which is the point of the type."
                )),
                type_: c::Type::Pointer(Box::new(c::Type::Const(Box::new(c::Type::Primitive(
                    c::Primitive::Char,
                ))))),
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
        }

        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: name,
                });
            };
            emitted.push(name.clone());
            out.extend(sequence_decl(&name, c_member_type(ast, element, &name)?));
        }

        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: name,
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: name.clone(),
                        })?;
                lengths.push(c::Expr::Literal(Literal::int(length)));
            }
            emitted.push(name.clone());
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line("An array. The element count is in the type and not on the wire."),
                type_: c_member_type(ast, element, &name)?,
                name: Ident::new(&name),
                dims: lengths,
            }));
        }

        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: name,
                });
            };
            let entry = c_map_entry_name(ast, id);
            emitted.push(name.clone());
            // The typedef here, the definition at the end of the file:
            // the map's `_buffer` is a pointer, so an incomplete entry
            // suffices at this position — and has to, when the value
            // type is an enclosing one and not yet complete.
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::none(),
                type_: c::Type::StructTag(Ident::new(&entry)),
                name: Ident::new(&entry),
                dims: Vec::new(),
            }));
            deferred.push(c::Decl::Struct(c::Struct {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::line(
                    "One entry of a map. XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them.",
                ),
                name: Some(Ident::new(&entry)),
                members: vec![
                    c::Member {
                        doc: Doc::none(),
                        type_: c_member_type(ast, key, &name)?,
                        name: Ident::new("key"),
                        dims: Vec::new(),
                        bits: None,
                    },
                    c::Member {
                        doc: Doc::none(),
                        type_: c_member_type(ast, value, &name)?,
                        name: Ident::new("value"),
                        dims: Vec::new(),
                        bits: None,
                    },
                ],
            }));
            out.extend(super::c_map::map_decl(&name, &entry));
        }

        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "CAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: name,
                });
            };
            emitted.push(name.clone());
            // `typedef Row Grid;` where `Row` is itself an array typedef
            // is legal C and keeps the dimensions where they were
            // declared, so the alias never restates them.
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::none(),
                type_: c_value_type(ast, target),
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "CEnum").is_none() {
                return Ok(());
            }
            emitted.push(name.clone());
            let prefix = module_prefix(ast, id);
            let mut values = Vec::new();
            let mut names = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "CEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                // The constant's whole spelling on the left, prefix and
                // all: that is the name a caller writes, and half of it
                // would be a mapping onto something the header does not
                // declare.
                let constant = format!("{prefix}{}", c::escape_keyword(&enum_node.name));
                names.push((constant.clone(), enum_node.name.clone()));
                values.push((constant, value));
            }
            // The storage is a fixed-width integer even when a C `enum`
            // would do: an enumeration constant is an `int`, and the
            // holder the wire wants is the `@bit_bound`'s, which is not
            // the same question.
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line(
                    "An enumeration. The holder is signed because XTypes 1.3 \
                     7.4.3.5.3 makes it so; the width on the wire is the \
                     @bit_bound's and is applied by the runtime.",
                ),
                type_: c::Type::Primitive(c::Primitive::Int32),
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
            for (constant, value) in values {
                // Parenthesised: a macro replacement list that is not
                // self-contained changes meaning at its use site.
                out.push(c::Decl::Define(c::Define {
                    doc: Doc::none(),
                    name: Ident::new(&constant),
                    value: Some(c::Expr::Paren(Box::new(c::Expr::Cast {
                        type_: Box::new(type_by_name(&name)),
                        value: Box::new(c::Expr::Literal(Literal::int(value))),
                    }))),
                }));
            }
            out.extend(idl_members(&name, &names)?);
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "CBitmask").is_none() {
                return Ok(());
            }
            emitted.push(name.clone());
            let bound = ast.effective_bit_bound(id);
            let prefix = module_prefix(ast, id);
            let mut names = Vec::new();
            // XTypes 1.3 §7.5.1.2.2: "the language binding shall be as if
            // there was an enumeration definition like the following:
            // @bit_bound(n) enum <TypeName>Bits { @value(1 << p) FLAG, … }",
            // and a typedef of the unsigned integer Table 44 gives for the
            // bound for "collections of flags from the previously defined
            // enumeration". The enumeration is written as C writes one
            // here (see the enumeration arm): an integer typedef with a
            // constant per flag. Its holder is the unsigned integer of the
            // bound, not a signed 32-bit one: a flag of a 64-bit bitmask
            // has a value an `int` cannot hold.
            let bits_name = format!("{name}Bits");
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line(
                    "The enumeration of the flags of a bitmask (XTypes 1.3 \
                     7.5.1.2.2): one constant per flag, valued 1 << position.",
                ),
                type_: c::Type::Primitive(c_holder(bound)),
                name: Ident::new(&bits_name),
                dims: Vec::new(),
            }));
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line(
                    "A bitmask: a collection of flags, as the unsigned integer \
                     Table 44 of XTypes 1.3 7.5.1.2.2 gives for its bound.",
                ),
                type_: c::Type::Primitive(c_holder(bound)),
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "CBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                // `#define MOVING (FlagsBits)1 << 4` expands inside
                // `x & MOVING` as `(x & (FlagsBits)1) << 4`. The brackets
                // are the difference between a bitmask constant and a
                // number nobody wrote.
                let constant = format!("{prefix}{}", c::escape_keyword(&bit_node.name));
                names.push((constant.clone(), bit_node.name.clone()));
                out.push(c::Decl::Define(c::Define {
                    doc: Doc::none(),
                    name: Ident::new(&constant),
                    value: Some(c::Expr::Paren(Box::new(c::Expr::Binary {
                        op: "<<",
                        lhs: Box::new(c::Expr::Cast {
                            type_: Box::new(type_by_name(&bits_name)),
                            value: Box::new(c::Expr::Literal(Literal::int(1))),
                        }),
                        rhs: Box::new(c::Expr::Literal(Literal::int(i128::from(position)))),
                    }))),
                }));
            }
            out.extend(idl_members(&name, &names)?);
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "CBitset").is_none() {
                return Ok(());
            }
            emitted.push(name.clone());
            let mut members = Vec::new();
            let mut names = Vec::new();
            // The base's bitfields are members of the derived struct
            // too: an inherited bitset is one flat run.
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                let escaped = c::escape_keyword(field_name);
                names.push((escaped.clone(), field_name.clone()));
                members.push(c::Member {
                    doc: Doc::none(),
                    type_: c::Type::Primitive(c_holder(field_node.bits)),
                    name: Ident::new(&escaped),
                    dims: Vec::new(),
                    // Not a C bitfield: C leaves the allocation order of
                    // bitfields implementation-defined, and the order on
                    // the wire is the declaration's. Packing is the
                    // codec's, where it can be seen.
                    bits: None,
                });
            }
            out.extend(struct_typedef(
                &name,
                members,
                Doc::line(
                    "A bitset. The fields are separate integers here; the packing into \
                 the anchor is the codec's, because C leaves bitfield allocation \
                 order to the implementation.",
                ),
            ));
            out.extend(idl_members(&name, &names)?);
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "CValue"
            } else {
                "CStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            emitted.push(name.clone());
            // XTypes 1.3 §7.5.1.4.1.3: "The language binding shall be
            // generated as if an instance of the base type were the first
            // member of the subtype with the name 'parent'". The struct
            // declares that member, then its own; the plan still puts the
            // base's members on the wire first (§7.4.1.1.5.1), and the codec
            // reaches them as `parent.m`. The table of IDL names lists every
            // member the struct has, the base's first, under that path.
            let mut members = Vec::new();
            let mut names = Vec::new();
            if let Some(base) = struct_base(ast, id) {
                members.push(c::Member {
                    doc: Doc::line(
                        "The base: an instance of the base type is the first member \
                         (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    type_: type_by_name(&c_type_name(ast, base)),
                    name: Ident::new(PARENT),
                    dims: Vec::new(),
                    bits: None,
                });
            }
            let own = ast.members_of(id).to_vec();
            for CMember {
                handle: member,
                path,
                ..
            } in c_members(ast, id)
            {
                let Ok(member_node) = ast.member(member) else {
                    continue;
                };
                let field = c::escape_keyword(&member_node.name);
                // XTypes 1.3 §7.5.1.2.4.1: an optional member is a
                // pointer and "A NULL pointer shall indicate an omitted
                // value", so there is no flag beside it. The path is the one
                // a member of the base takes (`parent.m`).
                names.push((format!("{path}{field}"), member_node.name.clone()));
                if !own.contains(&member) {
                    continue;
                }
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                members.push(c::Member {
                    doc: if member_node.facets.is_optional() {
                        Doc::line(
                            "An @optional member: NULL when it is absent. In a `@mutable` \
                             type an absent member is not written at all; outside one its \
                             absence is written — a zero `<is_present>` byte under XCDR2 \
                             (rule 20) or a parameter header declaring a length of zero \
                             under XCDR1 (§7.4.1.2).",
                        )
                    } else {
                        Doc::none()
                    },
                    type_: c_held_member_type(ast, id, member_node, target, &member_node.name)?,
                    name: Ident::new(&field),
                    dims: Vec::new(),
                    bits: None,
                });
            }
            // C has no empty structure. C23 §6.7.2.1p8: "If the
            // struct-declaration-list does not contain any named
            // members … the behavior is undefined" — gcc and clang
            // accept `struct S { };` only as an extension, and the
            // probe builds with `-std=c11 -pedantic-errors`, where it
            // is an error.
            //
            // Refused by name rather than emitted: the generated header
            // did compile nowhere and the failure arrived as
            // "does not compile" against a build directory, 840 slots
            // at a time, with nothing naming the construct. C++ has no
            // such rule and its column carries the case.
            if members.is_empty() {
                return Err(MaterialiseError::NoLanguageForm {
                    construct: format!(
                        "`{name}`, an aggregate with no members — C23 §6.7.2.1p8 \
                         leaves a structure with no named member undefined"
                    ),
                    language: "C",
                    at: name,
                });
            }
            out.extend(struct_typedef(&name, members, Doc::none()));
            out.extend(idl_members(&name, &names)?);
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "CUnion").is_none() {
                return Ok(());
            }
            emitted.push(name.clone());
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let mut arms = Vec::new();
            let mut names = Vec::new();
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "CCase").is_none() {
                    continue;
                }
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                let field = c::escape_keyword(&member.name);
                // The branches and not `_d`: the discriminator is this
                // binding's accessor name, and IDL 4.2 §7.4.1.4.4.1
                // gives it no member name of its own to record.
                names.push((field.clone(), member.name.clone()));
                arms.push(c::Member {
                    doc: Doc::none(),
                    type_: c_held_member_type(ast, id, member, target, &member.name)?,
                    name: Ident::new(&field),
                    dims: Vec::new(),
                    bits: None,
                });
            }
            let inner = format!("{name}_u");
            out.push(c::Decl::Union(c::Union {
                doc: Doc::line("The branches. Which one is live is what `_d` says."),
                name: Some(Ident::new(&inner)),
                members: arms,
            }));
            out.extend(struct_typedef(
                &name,
                vec![
                    c::Member {
                        doc: Doc::none(),
                        type_: c_member_type(ast, disc, &name)?,
                        name: Ident::new("_d"),
                        dims: Vec::new(),
                        bits: None,
                    },
                    c::Member {
                        doc: Doc::none(),
                        type_: c::Type::UnionTag(Ident::new(&inner)),
                        name: Ident::new("_u"),
                        dims: Vec::new(),
                        bits: None,
                    },
                ],
                Doc::none(),
            ));
            out.extend(idl_members(&name, &names)?);
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "CNative").is_none() {
                return Ok(());
            }
            emitted.push(name.clone());
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and a `void *` when there is none.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line("A native type. Opaque unless a facet named its mapping."),
                type_: match mapping {
                    Some(spelling) => c::Type::Named(Ident::new(&spelling)),
                    None => c::Type::Pointer(Box::new(c::Type::Void)),
                },
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
        }
        // An object reference belongs to the RPC arc and an annotation
        // type has no runtime form at all — `zerodds_ast_layout` refuses
        // to plan either — so a C declaration would be a shape nothing
        // could serialise. Named here rather than passed over, so the
        // coverage table and this file say the same thing.
        TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Ok(());
        }
        // `any` for the same reason and a different one: it is in the
        // model because Building Block Any is one of the fifteen, and
        // its content is decided at run time, so there is no C type to
        // declare for it. The codec refuses it by name — `body_of` in
        // `encode_c`, `decode_c` and `default_c` — rather than emitting
        // nothing and reporting success.
        TypeKind::Any | TypeKind::ValueBase => {
            return Ok(());
        }
        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            emitted.push(name.clone());
            out.push(c::Decl::Typedef(c::Typedef {
                doc: Doc::line("A boxed value: a pointer, absent when null."),
                type_: c::Type::Pointer(Box::new(c_member_type(ast, target, &name)?)),
                name: Ident::new(&name),
                dims: Vec::new(),
            }));
        }
    }
    // The declaration's own IDL name, in one place rather than in every
    // arm above: an arm that emitted nothing has already returned, so
    // reaching here is what "this declaration was written" means.
    if has_idl_name(&node.kind) && !node.name.is_empty() {
        out.push(idl_name(&name, &node.name));
    }
    Ok(())
}

/// The `ZD_IDL_MEMBERS_<name>` macro for a declaration's named parts.
///
/// `pairs` is `(the C spelling, the IDL spelling)` in IDL declaration
/// order — the order the member IDs are assigned in
/// (XTypes 1.3 §7.3.1.2.1.1) — and the entries are written even where
/// the two spellings are the same, because a caller computing a member
/// ID needs every member's IDL name and not only the ones a keyword
/// moved.
///
/// [`None`] for a declaration with no named parts: an empty braced
/// initialiser is not C, and the only declarations that reach it are an
/// enumeration with no enumerators and a bitset that is all padding.
/// The file's module documentation carries why this is a macro and not
/// an object.
/// The table only exists where the escape is invertible.
///
/// Two members whose IDL names differ and whose spellings do not are
/// two entries with the same left half — the table would record the
/// collision rather than refuse it, and a reader recovering the IDL
/// name would get whichever entry it hit first. The check is here
/// because this is the function that holds both spellings for a whole
/// scope; nothing upstream sees more than one member at a time.
///
/// # Errors
/// [`MaterialiseError::SpellingCollision`] when two members reach one
/// spelling.
fn idl_members(
    name: &str,
    pairs: &[(String, String)],
) -> Result<Option<c::Decl>, MaterialiseError> {
    no_spelling_collision(
        pairs
            .iter()
            .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
        "C",
        name,
    )?;
    if pairs.is_empty() {
        return Ok(None);
    }
    let entries = pairs
        .iter()
        .map(|(field, idl)| {
            c::Expr::Initializer(vec![
                c::Expr::Literal(Literal::Str(field.clone())),
                c::Expr::Literal(Literal::Str(idl.clone())),
            ])
        })
        .collect();
    Ok(Some(c::Decl::Define(c::Define {
        doc: Doc::lines(&[
            "The IDL spelling of each member, beside the spelling this header",
            "uses. `zd_member_name` is the record an entry fits.",
        ]),
        name: Ident::new(&format!("{IDL_MEMBERS_PREFIX}{name}")),
        value: Some(c::Expr::Initializer(entries)),
    })))
}

/// The `ZD_IDL_NAME_<name>` macro for a declaration's own IDL name.
fn idl_name(name: &str, idl: &str) -> c::Decl {
    c::Decl::Define(c::Define {
        doc: Doc::line("The IDL spelling of the declaration above."),
        name: Ident::new(&format!("{IDL_NAME_PREFIX}{name}")),
        value: Some(c::Expr::Literal(Literal::Str(idl.to_string()))),
    })
}

/// Whether a type's C name was made from an IDL identifier.
///
/// The collections are the reason this is asked: `zd_seq_int32_t` is a
/// name this file invented for a type IDL spells `sequence<long>` and
/// never named, so there is no IDL spelling to record for it. The named
/// kinds below all take their C name from
/// [`c::escape_keyword`] applied to an identifier the source wrote.
const fn has_idl_name(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Structure { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::Union { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Alias { .. }
            | TypeKind::Native
            | TypeKind::ValueBox { .. }
    )
}

/// `typedef struct <name> <name>;` followed by the definition.
///
/// The typedef comes first and names an incomplete type, which is what
/// lets a member refer to the aggregate before its body is closed —
/// C's answer to a self-referential declaration.
fn struct_typedef(name: &str, members: Vec<c::Member>, doc: Doc) -> Vec<c::Decl> {
    vec![
        c::Decl::Typedef(c::Typedef {
            doc: Doc::none(),
            type_: c::Type::StructTag(Ident::new(name)),
            name: Ident::new(name),
            dims: Vec::new(),
        }),
        c::Decl::Struct(c::Struct {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc,
            name: Some(Ident::new(name)),
            members,
        }),
    ]
}

/// `struct { uint32_t _maximum; uint32_t _length; T *_buffer; bool _release; }`, with its typedef
/// (the sequence of [C-MAP], CORBA C mapping §1.14, as XTypes 1.3 §7.5.1 reuses it).
///
/// A length beside a pointer and not a NUL-terminated run: a sequence of
/// `octet` may contain a zero, and a terminator would cut the sample
/// short without saying so. `_maximum` is the capacity of the buffer and `_release` says
/// whether the sequence owns it, as the map's fields do.
fn sequence_decl(name: &str, element: c::Type) -> Vec<c::Decl> {
    struct_typedef(
        name,
        vec![
            c::Member {
                doc: Doc::none(),
                type_: c::Type::Primitive(c::Primitive::UInt32),
                name: Ident::new("_maximum"),
                dims: Vec::new(),
                bits: None,
            },
            c::Member {
                doc: Doc::none(),
                type_: c::Type::Primitive(c::Primitive::UInt32),
                name: Ident::new("_length"),
                dims: Vec::new(),
                bits: None,
            },
            c::Member {
                doc: Doc::none(),
                type_: c::Type::Pointer(Box::new(element)),
                name: Ident::new("_buffer"),
                dims: Vec::new(),
                bits: None,
            },
            c::Member {
                doc: Doc::none(),
                type_: c::Type::Primitive(c::Primitive::Bool),
                name: Ident::new("_release"),
                dims: Vec::new(),
                bits: None,
            },
        ],
        Doc::line(
            "A sequence: a count and the elements. The buffer is the caller's — \
             nothing in the write path allocates.",
        ),
    )
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk the Rust materialiser does: a correspondence is
/// an ordinary node with a link to the anchor and a link to what was
/// created.
fn ghost_created_for(applied: &Applied, node: NodeId, type_name: &str) -> Option<GhostId> {
    let anchor = applied
        .origin
        .iter()
        .find(|(_, model)| **model == node)
        .map(|(ghost, _)| *ghost)?;
    let typ = applied.graph.types.lookup(type_name)?;
    applied
        .graph
        .nodes_of_type(typ)
        .map(|created| created.id)
        .find(|created| {
            applied.graph.connected(&anchor, created)
                || applied
                    .graph
                    .parts(&anchor)
                    .any(|part| applied.graph.connected(&part.other, created))
        })
}

/// The C literal a constant's value takes, behind the cast its declared
/// type puts in front of it.
///
/// The four string-shaped forms are all `const char *` in this binding —
/// `zd_string`, `zd_wstring` and `zd_fixed_P_S` are the same typedef —
/// so each becomes a string literal and the cast names which of the
/// three it is. That is not a widening: a `fixed` is a decimal and a
/// `wstring` is UTF-8 because that is how `runtime/c` takes them, and a
/// constant that spelled either differently from a member of the same
/// type would be a second answer to what the value is.
///
/// `long double` is refused by name. CORBA gives a `long double` literal
/// no spelling C can carry: the compiler's own `long double` is x87
/// 80-bit extended on x86-64, binary128 on aarch64 and a plain `double`
/// on macOS arm64, which is why [`FLOAT128`] is a struct and not that
/// type — and a struct has no literal. Emitting `((zdw_f128)1.5)` would
/// not compile; emitting a `double` would put a different number in the
/// header than the IDL wrote.
fn c_constant_literal(
    value: &zerodds_ast::ConstValue,
    type_: &c::Type,
    at: &str,
) -> Result<Literal, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: "C",
        at: at.to_string(),
    };
    Ok(match value {
        ConstValue::Boolean(flag) => Literal::Bool(*flag),
        // Through `integer_text` and not `Literal::int`: −2^63 has no
        // decimal in C, and a `const long long` carrying it is the one
        // value where the digits are read unsigned before the minus
        // applies. Every other value is its own decimal, which is what
        // `Literal::int` wrote.
        ConstValue::Integer(number) => Literal::Int(zerodds_lang_render::c::integer_text(*number)),
        ConstValue::Char(character) => Literal::Char(*character),
        // The code point as a number, because `wchar` is a `uint16_t`
        // here: a `'ä'` character literal in C is one byte of the
        // execution character set and not a code unit.
        ConstValue::WChar(point) => Literal::int(i128::from(*point)),
        ConstValue::Float { mantissa, exponent } => {
            if matches!(type_, c::Type::Named(name) if name.as_str() == FLOAT128) {
                return Err(refuse("a `long double` constant, which has no C literal"));
            }
            let mut text = constant::float_text(mantissa, exponent.as_deref());
            // `-Wconversion` is on for every generated file, and an
            // initialiser whose value is a `double` narrowing into a
            // `float` is exactly what it reports.
            if matches!(type_, c::Type::Primitive(c::Primitive::Float)) {
                text.push('f');
            }
            Literal::Float(text)
        }
        ConstValue::Fixed { digits, scale } => {
            Literal::Str(constant::fixed_decimal(digits, *scale))
        }
        ConstValue::String(text) => Literal::Str(text.clone()),
        ConstValue::WString(points) => Literal::Str(
            constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        ),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a header that disagrees with the IDL and compiles.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
