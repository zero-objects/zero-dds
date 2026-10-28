// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@external` in the C++ and C columns, and the one case that made it
//! matter.
//!
//! DDS-XTypes 1.3 Table 21 makes `@external` a member "stored externally
//! to the containing type". The annotation changes no octet: it says
//! where the binding keeps the value. Fifteen columns can keep it inline
//! and do; two cannot, because a language whose member is a value has no
//! size for a member that reaches its own enclosing type back.
//!
//! `tests/interop-corpus/recursive-optional-external` is that member —
//! `@optional @external Node next` inside `Node` — and the C++ column
//! answered it with six errors of the form `incomplete type 'm::Node'
//! used in type trait expression`, raised out of
//! `<__type_traits/is_trivially_destructible.h>` and five neighbours,
//! because `std::optional<T>` requires `T` complete.
//!
//! `std::shared_ptr<T>` is complete whatever it points at, so it is the
//! holder, for every `@external` member and not only the recursive one:
//! Table 21 says the same thing about all of them, and `dds-psm-cxx`
//! maps the annotation to `dds::core::external<T>`, which is a
//! shared-pointer holder under another name. Where the member is also
//! `@optional` the pointer is the absence too — a `std::optional` around
//! it would carry a present-but-null state the wire cannot encode.
//!
//! ## The C column
//!
//! C reached the same wall from the other side: `m_Node next;` inside
//! `struct m_Node` is `field 'next' has incomplete type`. Its answer is
//! a pointer, and the pointer is also what DDS-XTypes 1.3 §7.5.1.2.3.1
//! asks of every `@external` member: "a member of type X shall instead be
//! mapped as type pointer-to-X". So `@external long a` is `int32_t *a`
//! and not `int32_t a`, whether or not the member closes a cycle;
//! [`zerodds_ast_layout::member_held_inline_closes_a_cycle`] is the other
//! reason for the same pointer. `@optional Node next` without
//! `@external` closes the same cycle and gets the same pointer. A
//! `string`, a `wstring` and a `fixed` are pointers already, and stay as
//! they are.
//!
//! The octets do not move: the annotation moves none, and the codec
//! reaches the value through the pointer.
//!
//! What this file does not do is compile anything. The compile is
//! `tools/wireprobe/wireprobe-zerodds-cpp` over the eleven `@external`
//! corpus cases, and the octets it produced are recorded in the commit
//! that introduced this.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, member_held_inline_closes_a_cycle};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate::source;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("the model resolves");
    ast
}

fn c(idl: &str, encoding: Encoding) -> String {
    source(
        &model(idl),
        Language::C,
        LayoutKey::data(encoding, Endianness::Little),
    )
    .unwrap_or_else(|why| panic!("the C column: {why:?}"))
}

/// Whether the member `member` of `type_` is one no column can hold
/// inline — asked of the model, which is where the answer lives.
fn closes_a_cycle(idl: &str, type_: &str, member: &str) -> bool {
    let ast = model(idl);
    let id = ast
        .types()
        .find(|(_, node)| node.name == type_)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("`{type_}` is in the model"));
    let member_type = ast
        .all_members_of(id)
        .into_iter()
        .filter_map(|m| ast.member(m).ok())
        .find(|m| m.name == member)
        .unwrap_or_else(|| panic!("`{type_}` declares `{member}`"))
        .type_
        .resolved()
        .expect("the member type resolves");
    member_held_inline_closes_a_cycle(&ast, id, member_type)
}

fn cpp(idl: &str, encoding: Encoding) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    source(
        &model(idl),
        Language::Cpp,
        LayoutKey::data(encoding, Endianness::Little),
    )
    .unwrap_or_else(|why| panic!("the C++ column: {why:?}"))
}

/// The corpus case, verbatim.
const RECURSIVE: &str = "module m { struct Node; @mutable struct Node { long value; \
                         @optional @external Node next; }; };";

#[test]
fn a_recursive_external_member_is_a_shared_pointer_and_not_a_value() {
    let generated = cpp(RECURSIVE, Encoding::PlCdr2);
    assert!(
        generated.contains("std::shared_ptr<::m::Node> next;"),
        "the C++ column declares a member of a type that is not complete yet:\n{generated}"
    );
    assert!(
        !generated.contains("std::optional<::m::Node>"),
        "the C++ column still wraps the member in std::optional, which requires \
         a complete type:\n{generated}"
    );
}

#[test]
fn the_reader_fills_the_pointer_before_it_writes_through_it() {
    let generated = cpp(RECURSIVE, Encoding::PlCdr2);
    assert!(
        generated.contains("zerodds::wire::external_slot(v.next)"),
        "the C++ decoder writes into a pointer nothing made point anywhere:\n{generated}"
    );
    // And the absence is the pointer's, not a `std::optional`'s.
    assert!(
        generated.contains("v.next = nullptr"),
        "the C++ decoder marks the member absent with something other than a null \
         pointer:\n{generated}"
    );
    assert!(
        generated.contains("v.next != nullptr"),
        "the C++ encoder asks a std::optional whether an @external member is \
         present:\n{generated}"
    );
}

#[test]
fn a_non_optional_external_member_gets_table_9_behind_the_indirection() {
    // The Table 9 defaulter has to fill the pointer as well, or the
    // value it writes goes into nothing. Measured through
    // `annotation-external-primitive-unannotated`, whose appendable
    // framing is what makes an absent member reach the defaulter at all:
    // without this the C++ probe answered
    // `no matching function for call to 'zd_def_p_long'`.
    let generated = cpp(
        "module m { struct Sample { @external long a; long b; }; };",
        Encoding::DelimitedCdr,
    );
    assert!(
        generated.contains("std::shared_ptr<std::int32_t> a;"),
        "a non-recursive @external member is inline:\n{generated}"
    );
    assert!(
        generated.contains("zd_def_p_long(zerodds::wire::external_slot(v.a))"),
        "the C++ defaulter writes Table 9's value into an unfilled pointer:\n{generated}"
    );
}

#[test]
fn an_external_union_branch_takes_the_same_indirection() {
    let generated = cpp(
        "module m { struct Inner { long a; }; union Choice switch (long) { \
         case 1: @external Inner nested; case 2: long plain; }; };",
        Encoding::DelimitedCdr,
    );
    assert!(
        generated.contains("std::shared_ptr<::m::Inner> zd_nested;"),
        "an @external union branch is held inline:\n{generated}"
    );
    assert!(
        generated.contains("zerodds::wire::external_slot(v.zd_nested)"),
        "the C++ decoder writes into an unfilled union branch:\n{generated}"
    );
}

// -- the C column ---------------------------------------------------------

#[test]
fn the_model_says_which_member_has_no_room_inside_its_own_aggregate() {
    // The corpus case: the member's type is the aggregate.
    assert!(closes_a_cycle(RECURSIVE, "Node", "next"));
    // The same cycle without `@external`. The annotation is not what
    // decides — the cycle is — and a column that keyed on the annotation
    // would hold this one inline and not compile.
    assert!(closes_a_cycle(
        "module m { struct Node; @mutable struct Node { long value; @optional Node next; }; };",
        "Node",
        "next",
    ));
    // A cycle that crosses a sequence: the elements live behind the
    // buffer the sequence points at, so the aggregate holds a handle and
    // has a size. `tests/interop-corpus/recursive-self-sequence`.
    assert!(!closes_a_cycle(
        "module m { @appendable struct Node { long value; sequence<Node> children; }; };",
        "Node",
        "children",
    ));
    // And a map, for the same reason.
    // `tests/interop-corpus/recursive-through-map`.
    assert!(!closes_a_cycle(
        "module m { @appendable struct Node { long value; map<long, Node> children; }; };",
        "Node",
        "children",
    ));
    // The four `annotation-external-*` members close nothing, which is
    // why they stay where the recorded octets found them.
    assert!(!closes_a_cycle(
        "module m { @final struct Sample { @external long a; long b; }; };",
        "Sample",
        "a",
    ));
    assert!(!closes_a_cycle(
        "module m { struct Inner { long a; }; @final struct Sample { @external Inner nested; }; };",
        "Sample",
        "nested",
    ));
}

#[test]
fn the_c_column_declares_a_recursive_member_as_a_pointer() {
    let generated = c(RECURSIVE, Encoding::PlCdr2);
    assert!(
        generated.contains("m_Node *next;"),
        "the C header declares a structure that contains itself:\n{generated}"
    );
    // XTypes 1.3 §7.5.1.2.4.1: NULL is the absent member, so there is
    // no flag beside the pointer.
    assert!(
        !generated.contains("_has_next"),
        "the C header kept a presence flag beside the pointer:\n{generated}"
    );
}

#[test]
fn the_c_encoder_hands_the_pointer_and_not_its_address() {
    let generated = c(RECURSIVE, Encoding::PlCdr2);
    assert!(
        generated.contains("zd_enc_m_Node(w, v->next);"),
        "the C encoder hands a codec the address of a pointer:\n{generated}"
    );
}

#[test]
fn the_c_reader_makes_the_pointer_point_somewhere_before_it_writes_through_it() {
    let generated = c(RECURSIVE, Encoding::PlCdr2);
    assert!(
        generated.contains("v->next = (m_Node *)zdw_reader_alloc(r, sizeof(m_Node));"),
        "the C reader writes through a pointer nothing made point anywhere:\n{generated}"
    );
    // Out of the caller's arena, which can run out — the same guard
    // `make_room` writes for a sequence's buffer.
    assert!(
        generated.contains("if (v->next == NULL)"),
        "the C reader does not check the allocation:\n{generated}"
    );
    assert!(
        generated.contains("zd_dec_m_Node(r, v->next, "),
        "the C reader hands the decoder the address of a pointer:\n{generated}"
    );
}

#[test]
fn a_non_recursive_external_member_is_a_pointer_in_c() {
    // XTypes 1.3 §7.5.1.2.3.1: "Other external members are mapped like
    // non-external members except that a member of type X shall instead
    // be mapped as type pointer-to-X. For example, short shall be
    // replaced by short*."
    let generated = c(
        "module m { @final struct Sample { @external long a; long b; }; };",
        Encoding::DelimitedCdr,
    );
    assert!(
        generated.contains("int32_t *a;"),
        "an @external member is not a pointer:\n{generated}"
    );
    assert!(
        generated.contains("int32_t b;"),
        "a plain member gained an indirection:\n{generated}"
    );
}

#[test]
fn an_external_string_stays_the_pointer_it_is() {
    // "String and wide string members are already represented using
    // pointers, so the mappings for these members do not change."
    let generated = c(
        "module m { @final struct Sample { @external string s; @external wstring w; }; };",
        Encoding::DelimitedCdr,
    );
    assert!(generated.contains("zd_string s;"), "{generated}");
    assert!(generated.contains("zd_wstring w;"), "{generated}");
    assert!(!generated.contains("zd_string *s;"), "{generated}");
}

#[test]
fn the_c_encoder_writes_through_an_external_pointer() {
    let generated = c(
        "module m { @final struct Sample { @external long a; long b; }; };",
        Encoding::DelimitedCdr,
    );
    // The codecs take the address of a value; an external member already
    // is one, so the pointer goes through as it is and its address is not
    // taken (`&*v->a` and `v->a` are the same pointer, C11 6.5.3.2p3).
    assert!(
        generated.contains("(w, v->a)"),
        "the C encoder does not hand over the external pointer:\n{generated}"
    );
    assert!(
        generated.contains("(w, &v->b)"),
        "a member held inline lost its address:\n{generated}"
    );
    assert!(
        !generated.contains("&v->a"),
        "the C encoder hands the address of a pointer:\n{generated}"
    );
}

#[test]
fn the_c_reader_allocates_an_external_member_before_it_writes() {
    let generated = c(
        "module m { @final struct Sample { @external long a; long b; }; };",
        Encoding::DelimitedCdr,
    );
    assert!(
        generated.contains("v->a = (int32_t *)zdw_reader_alloc(r, sizeof(int32_t));"),
        "the C reader writes through a pointer nothing made point anywhere:\n{generated}"
    );
}

// -- a member on a cycle, without the annotation ---------------------------
//
// `@optional Node next` inside `Node` is legal IDL: the absent member is
// what ends a value of it (XTypes 1.3 §7.3.1.2.1.3), and the planner
// admits it. Before `held_externally`, the columns that hold `@external`
// behind an indirection held this member inline because it carries no
// annotation, and a type that contains itself has no size: C++ answered
// with `std::optional<T>`'s incomplete-type errors, Rust with E0072, and
// D refused it by name with a clause the type does not break.

/// `@optional Node next` with no `@external`.
const RECURSIVE_UNANNOTATED: &str =
    "module m { struct Node; @mutable struct Node { long value; @optional Node next; }; };";

/// A cycle with a member on it that is not `@optional`: `B.a` holds an
/// `A`, which holds a `B` only optionally. Both members close the cycle
/// in a column whose member is the value, and `Outer` is there because
/// an appendable member the stream does not carry is what calls `B`'s
/// Table 9 defaulter.
// The members are not named after their types: `B b` in one scope is
// `b` and `B` differing only in case, which IDL 4.2 §7.2.3.1 refuses.
const MIXED: &str = "module m { struct B; \
                     @mutable struct A { long x; @optional B link; }; \
                     @mutable struct B { long y; A back; }; \
                     @appendable struct Outer { long z; B inner; }; };";

fn generated(language: Language, idl: &str) -> String {
    source(
        &model(idl),
        language,
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little),
    )
    .unwrap_or_else(|why| panic!("{language:?}: {why:?}"))
}

#[test]
fn an_optional_member_on_a_cycle_is_held_like_an_external_one() {
    let cpp = generated(Language::Cpp, RECURSIVE_UNANNOTATED);
    // XTypes 1.3 §7.5.1.2.4.2: an optional member is a plain pointer,
    // which has a size whatever it points at.
    assert!(
        cpp.contains("::m::Node* next"),
        "C++ holds a member on a cycle inline:\n{cpp}"
    );
    assert!(!cpp.contains("std::optional<::m::Node>"), "{cpp}");
    assert!(!cpp.contains("shared_ptr"), "{cpp}");

    let rust = generated(Language::Rust, RECURSIVE_UNANNOTATED);
    assert!(
        rust.contains("Option<Box<Node>>"),
        "Rust holds a member on a cycle inline:\n{rust}"
    );

    let d = generated(Language::D, RECURSIVE_UNANNOTATED);
    assert!(d.contains("m_Node* next;"), "D:\n{d}");
    assert!(!d.contains("Nullable!(m_Node)"), "{d}");

    let nim = generated(Language::Nim, RECURSIVE_UNANNOTATED);
    assert!(nim.contains("next*: Option[ref m_Node]"), "Nim:\n{nim}");

    let zig = generated(Language::Zig, RECURSIVE_UNANNOTATED);
    assert!(zig.contains("next: ?*m_Node = null,"), "Zig:\n{zig}");

    let swift = generated(
        Language::Swift,
        "struct Node; @mutable struct Node { long value; @optional Node next; };",
    );
    assert!(
        swift.contains("public var next: ZdExternal<Node>?"),
        "Swift:\n{swift}"
    );

    // C asked the cycle all along, and keeps answering it the same way.
    let c = generated(Language::C, RECURSIVE_UNANNOTATED);
    assert!(c.contains("m_Node *next;"), "C:\n{c}");
}

#[test]
fn a_member_off_any_cycle_keeps_its_holder() {
    // The rule is the cycle; an `@optional` member that closes none is
    // what it was.
    let idl = "module m { struct Inner { long a; }; \
               @mutable struct S { @optional Inner i; @optional long n; }; };";
    let cpp = generated(Language::Cpp, idl);
    assert!(cpp.contains("::m::Inner* i"), "{cpp}");
    assert!(cpp.contains("std::int32_t* n"), "{cpp}");
    assert!(!cpp.contains("shared_ptr"), "{cpp}");
    assert!(!cpp.contains("std::optional"), "{cpp}");
    let rust = generated(Language::Rust, idl);
    assert!(!rust.contains("Box<"), "{rust}");
}

#[test]
fn a_member_on_a_cycle_that_is_not_optional_is_held_too() {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let cpp = generated(Language::Cpp, MIXED);
    assert!(cpp.contains("std::shared_ptr<::m::A> back;"), "{cpp}");
    // `link` is `@optional`: a plain pointer (XTypes 1.3 §7.5.1.2.4.2).
    assert!(cpp.contains("::m::B* link"), "{cpp}");
    let rust = generated(Language::Rust, MIXED);
    assert!(rust.contains("Box<A>"), "{rust}");
    assert!(rust.contains("Option<Box<B>>"), "{rust}");
}

#[test]
fn a_c_default_that_needs_storage_takes_it_from_the_reader() {
    // `B`'s Table 9 value holds an `A`, which C keeps behind `m_A *back`.
    // The defaulter has to point it somewhere before it writes the
    // value; the storage is the read's arena, so this defaulter — and
    // only this one — takes the reader.
    let c = generated(Language::C, MIXED);
    assert!(c.contains("m_A *back;"), "{c}");
    assert!(
        c.contains("static void zd_def_m_B(zdw_reader *r, m_B *v)"),
        "the defaulter has nothing to allocate from:\n{c}"
    );
    assert!(
        c.contains("v->back = (m_A *)zdw_reader_alloc(r, sizeof(m_A));"),
        "the defaulter writes through a pointer nothing made point anywhere:\n{c}"
    );
    assert!(c.contains("if (v->back == NULL)"), "{c}");
    assert!(
        c.contains("zd_def_m_A(v->back);"),
        "the defaulter hands the address of a pointer:\n{c}"
    );
    assert!(
        c.contains("zd_def_m_B(r, &v->inner);"),
        "the caller does not hand the reader on:\n{c}"
    );
    // `A`'s own default needs nothing: its `B` is optional.
    assert!(c.contains("static void zd_def_m_A(m_A *v)"), "{c}");
}
