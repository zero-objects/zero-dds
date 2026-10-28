// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Presenting the model and the language CSTs as seesaw graphs. Spec §3.

pub mod ada;
pub mod ada83;
pub mod ast;
pub mod c;
pub mod c_lifecycle;
pub mod c_map;
pub mod constant;
pub mod cpp;
pub mod cpp_classic;
pub mod cpp_psm;
pub mod csharp;
pub mod cst;
pub mod d;
pub mod elixir;
pub mod fsharp;
pub mod go;
pub mod interface;
pub mod java;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod nim;
pub mod node;
pub mod ocaml;
pub mod python;
pub mod swift;
pub mod typescript;
pub mod zig;

/// Whether `member` is annotated `@external`.
#[must_use]
pub fn annotated_external(member: &zerodds_ast::MemberNode) -> bool {
    member
        .facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_external)
}

/// Whether a column that keeps an `@external` member behind an
/// indirection keeps `member` there.
///
/// DDS-XTypes 1.3 Table 21 makes `@external` a member "stored
/// externally to the containing type". A member whose type reaches the
/// aggregate that declares it back over inline edges has nowhere else to
/// be, annotated or not: `@optional Node next` inside `Node` is legal —
/// the absent member is what ends a value of it, XTypes 1.3
/// §7.3.1.2.1.3 — and a column whose member is the value declares a
/// type that contains itself. C++ answered it with `std::optional<T>`'s
/// incomplete-type errors, Rust with E0072, D with a refusal that cited
/// a clause the type does not break.
///
/// [`zerodds_ast_layout::member_held_inline_closes_a_cycle`] is the
/// question, asked of the aggregate that *declares* the member and not
/// of the one a walk is in. A base member flattened into a derived type
/// then has one holder wherever it appears, and the answer can only be
/// wider than the derived type's own: a type that reaches the derived
/// one reaches its base, which the derived type holds inline.
///
/// The columns whose member is a reference already — Java, Kotlin, F#,
/// Python — have no use for this, and C asks for the pointer it declares
/// ([`crate::adapter::c::pointer_member`], through
/// [`crate::emit::CodecTarget::behind_indirection`]): the member that closes a
/// cycle, and every `@external` member that is not a pointer already
/// (XTypes 1.3 §7.5.1.2.3.1).
#[must_use]
pub fn held_externally(ast: &zerodds_ast::Ast, member: &zerodds_ast::MemberNode) -> bool {
    if annotated_external(member) {
        return true;
    }
    let (zerodds_ast::AnyId::Type(enclosing, _), Some(target)) =
        (member.parent, member.type_.resolved())
    else {
        return false;
    };
    zerodds_ast_layout::member_held_inline_closes_a_cycle(ast, enclosing, target)
}

/// The corpus cases the IDL front end refuses, so that no adapter ever
/// sees a model for them.
///
/// `tests/interop-corpus/resolve-module-named-as-type` is
/// `module common { struct common { … }; }`. OMG IDL 4.2 §7.5.2 states
/// that "the name of a module, structure, union, map, interface, value
/// type, event type, exception or home may not be redefined within the
/// immediate scope" of that same construct, and prints
/// `module M { typedef short M; }` as its own `// Error` example. The
/// case is in the corpus on purpose — it measures which vendors refuse
/// the file and which resolve it anyway — and
/// `tools/idlc/tests/corpus.rs` carries it as an `Intended` diagnostic
/// for the same reason.
///
/// Lowering now reports it (`DiagnosticCode::DuplicateName`), so every
/// corpus walk in this crate stops at the front end rather than at the
/// column. Named here once, so the reason is stated once and the five
/// walks cannot drift apart.
///
/// A list and never a pattern: a case on it that starts lowering is a
/// finding, and the walks assert that it is still refused.
#[cfg(test)]
pub(crate) const REFUSED_BY_THE_FRONT_END: &[&str] = &["resolve-module-named-as-type"];

/// A corpus walk's failure lines, minus the ones
/// [`REFUSED_BY_THE_FRONT_END`] accounts for.
///
/// Checked in both directions, the way the two
/// `*_corpus_compiles.rs` gates check their own `REFUSED` lists: a line
/// that is not on the list stays a failure, and a listed case that
/// produced no line at all is reported here rather than passing
/// silently. An exemption nothing checks is an exemption that outlives
/// its cause.
///
/// The lines are `"{case}: {why}"`, which is the shape every walk in
/// this crate builds.
#[cfg(test)]
pub(crate) fn without_the_front_end_refusals(failures: Vec<String>) -> Vec<String> {
    let mut rest = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for line in failures {
        match REFUSED_BY_THE_FRONT_END
            .iter()
            .find(|case| line.starts_with(&format!("{case}: ")))
        {
            Some(case) => seen.push(case),
            None => rest.push(line),
        }
    }
    for case in REFUSED_BY_THE_FRONT_END {
        assert!(
            seen.contains(case),
            "`{case}` is listed as refused by the IDL front end and this walk rendered it \
             instead; the list has outlived its cause"
        );
    }
    rest
}
