// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a derived interface's operations become, column by column.
//!
//! OMG IDL 4.2 §7.4.3: a derived interface "inherits all the operations
//! and attributes of its base interfaces", and a client holding a
//! `Derived` may call every one of them.
//!
//! `zerodds_lang_rules::generate` answers this in exactly two ways now,
//! and there is no third:
//!
//! 1. **The language has a complete DDS-RPC 1.0 binding** — C++, Java,
//!    C#, TypeScript through `generate::declarations`
//!    (`COMPLETE_DECLARATIONS_LANGUAGES`), Rust through `generate::
//!    source` besides (`COMPLETE_INTERFACE_LANGUAGES` — Rust's binding
//!    needs a codec, which `declarations` never carries for any
//!    language, so `declarations(ast, Language::Rust)` refuses too;
//!    see that constant's own doc). Each reaches the base's operation
//!    its own way — a base clause, a supertrait, `extends` — and this
//!    file measures that the inherited operation is reachable.
//! 2. **It does not** — every other language. `generate::
//!    refuse_interface_unless` refuses the whole model rather than let
//!    a partial answer through, which is what
//!    `docs/spec-coverage/dds-rpc-1.0.md` calls the worst bug class in
//!    this project.
//!
//! Before this file, nine columns without a complete binding still
//! flattened the base's operations into the derived declaration and
//! called that an answer — correct about inheritance and silent about
//! everything else DDS-RPC 1.0 §7.5.1.1 asks of an `interface`
//! (`raises`, an attribute, the request/reply topic types). The task
//! this file now measures is C2(4)'s own: no language emits a part of
//! an interface any more.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::generate::{COMPLETE_DECLARATIONS_LANGUAGES, COMPLETE_INTERFACE_LANGUAGES};

/// `Base` declares `g`, `Derived` declares `f` and inherits `g`.
const DERIVED: &str = "interface Base { void g(); }; interface Derived : Base { void f(); };";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "inherited.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    // A base interface is written as a scoped name and arrives as an
    // unresolved reference; without this every `bases` entry answers
    // `None` and the walk has nothing to walk.
    ast.resolve_all_by_name().expect("the model resolves");
    ast
}

/// IDL in, one column's declarations out — for a language this file
/// expects to succeed.
fn generate(idl: &str, language: Language) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    zerodds_lang_rules::generate::declarations(&model(idl), language)
        .unwrap_or_else(|error| panic!("{language:?}: {error:?}"))
}

/// The text of the derived declaration: everything from `anchor` on.
fn after<'a>(out: &'a str, anchor: &str) -> &'a str {
    out.split_once(anchor)
        .unwrap_or_else(|| panic!("no `{anchor}` in the output:\n{out}"))
        .1
}

// -- the columns complete through `declarations` ----------------------------

/// Java's binding writes `interface Derived extends Base`, so the
/// inherited operation is reached through the supertype and the derived
/// interface declares only `f`.
#[test]
fn the_java_interface_reaches_the_bases_operation_through_extends() {
    let out = generate(DERIVED, Language::Java);
    assert!(
        out.contains("public interface Derived extends Base"),
        "{out}"
    );
    let base = after(&out, "public interface Base {");
    assert!(base.contains("void g();"), "{out}");
    let derived = after(&out, "public interface Derived extends Base {");
    assert!(derived.contains("void f();"), "{out}");
}

/// C++'s binding writes `class Derived : public Base`, for the same
/// reason and with the same consequence.
#[test]
fn the_cpp_class_reaches_the_bases_operation_through_its_base_clause() {
    let out = generate(DERIVED, Language::Cpp);
    assert!(out.contains("class Derived : public Base {"), "{out}");
    let base = after(&out, "class Base {");
    assert!(base.contains("virtual void g() = 0;"), "{out}");
    let derived = after(&out, "class Derived : public Base {");
    assert!(derived.contains("virtual void f() = 0;"), "{out}");
}

/// TypeScript's binding writes `extends` on both halves of the service
/// and names the base descriptor in `inherits`.
#[test]
fn the_typescript_interfaces_reach_the_bases_operation_through_extends() {
    let out = generate(DERIVED, Language::TypeScript);
    assert!(
        out.contains("export interface DerivedClient extends BaseClient {"),
        "{out}"
    );
    assert!(
        out.contains("export interface DerivedHandler extends BaseHandler {"),
        "{out}"
    );
    let descriptor = after(&out, "export const DerivedService");
    assert!(descriptor.contains("inherits: [BaseService]"), "{out}");
}

/// C#'s binding flattens: OMG IDL 4.2 §7.4.3's inherited operation is
/// written directly into the derived declaration, which has no base
/// list (`crates/lang-rules/src/adapter/csharp.rs::interfaces`'s own
/// doc gives the reason — none of C#, Ada, D, Elixir, Go, Julia, Lua,
/// Nim, OCaml, Swift or Zig has a language mechanism this crate
/// composes a base's members through instead, unlike Java's `extends`,
/// C++'s base clause or Rust's supertrait above).
#[test]
fn the_csharp_interface_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::CSharp);
    assert!(out.contains("public interface Derived"), "{out}");
    assert!(
        !out.contains("Derived : Base"),
        "flattened, not extends: {out}"
    );
    let derived = after(&out, "public interface Derived");
    assert!(derived.contains("void G();"), "{out}");
    assert!(derived.contains("void F();"), "{out}");
}

/// Go's binding flattens too, for the same reason C#'s does: no
/// embedded-interface mechanism this crate composes a base's method set
/// through.
#[test]
fn the_go_interface_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::Go);
    assert!(out.contains("type Derived interface"), "{out}");
    let derived = after(&out, "type Derived interface");
    assert!(derived.contains("G() error"), "{out}");
    assert!(derived.contains("F() error"), "{out}");
}

/// Swift's binding flattens too, for the same reason C#'s and Go's do.
#[test]
fn the_swift_protocol_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::Swift);
    assert!(out.contains("public protocol Derived"), "{out}");
    let derived = after(&out, "public protocol Derived");
    assert!(derived.contains("func g()"), "{out}");
    assert!(derived.contains("func f()"), "{out}");
}

/// Nim's binding flattens too: the object has no base object to reach
/// an inherited field through.
#[test]
fn the_nim_object_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::Nim);
    assert!(out.contains("Derived* = object"), "{out}");
    let derived = after(&out, "Derived* = object");
    assert!(derived.contains("g*: proc ()"), "{out}");
    assert!(derived.contains("f*: proc ()"), "{out}");
}

/// Zig's binding flattens too: Zig has no inheritance at all.
#[test]
fn the_zig_struct_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::Zig);
    assert!(out.contains("pub const Derived = struct {"), "{out}");
    let derived = after(&out, "pub const Derived = struct {");
    assert!(derived.contains("g: *const fn () void"), "{out}");
    assert!(derived.contains("f: *const fn () void"), "{out}");
}

/// D's binding flattens too: `d::InterfaceDecl` has no base list at
/// all.
#[test]
fn the_d_interface_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::D);
    assert!(out.contains("interface Derived"), "{out}");
    let derived = after(&out, "interface Derived");
    assert!(derived.contains("void g();"), "{out}");
    assert!(derived.contains("void f();"), "{out}");
}

/// Elixir's binding flattens too: a behaviour does not extend another.
#[test]
fn the_elixir_behaviour_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::Elixir);
    assert!(out.contains("@callback g() :: :ok"), "{out}");
    assert!(out.contains("@callback f() :: :ok"), "{out}");
}

/// Julia has no interface construct at all: an operation becomes a
/// generic function at file scope, bound to no type, so a base's own
/// declaration already covers a derived interface's copy of the same
/// name — the dedup check, not a flattened list, is what carries
/// inheritance here.
#[test]
fn the_julia_column_declares_the_bases_operation_once_and_shares_it() {
    let out = generate(DERIVED, Language::Julia);
    assert!(out.contains("function g end"), "{out}");
    assert!(out.contains("function f end"), "{out}");
    assert_eq!(
        out.matches("function g end").count(),
        1,
        "the base's own function is declared once, not once per interface: {out}"
    );
}

/// Lua's binding flattens too: the descriptor table has no link to a
/// base's table.
#[test]
fn the_lua_descriptor_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::Lua);
    assert!(out.contains("kind = \"interface\""), "{out}");
    assert!(out.contains("name = \"g\""), "{out}");
    assert!(out.contains("name = \"f\""), "{out}");
}

/// OCaml's binding flattens too: `ocaml::ClassTypeDecl` has no
/// `inherit` clause.
#[test]
fn the_ocaml_class_type_carries_the_bases_operation_flattened_in() {
    let out = generate(DERIVED, Language::OCaml);
    assert!(out.contains("class type derived = object"), "{out}");
    let derived = after(&out, "class type derived = object");
    assert!(derived.contains("method g : unit"), "{out}");
    assert!(derived.contains("method f : unit"), "{out}");
}

// -- the column complete only through `source` -------------------------------

/// Rust writes a supertrait — `pub trait Derived: Base` — over
/// `emit::interface_rust::service_trait`, reached through
/// `generate::source` and not through `generate::declarations`, which
/// has no codec to give the binding at all.
/// `crates/rpc-rust-e2e/tests/e2e.rs` is where the inherited operation
/// is actually *called*, through the composed base client field and not
/// through the trait alone.
#[test]
fn the_rust_trait_reaches_the_bases_operation_through_a_supertrait() {
    let ast = model(DERIVED);
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let out = zerodds_lang_rules::generate::source(&ast, Language::Rust, key)
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert!(out.contains("pub trait Derived: Base {"), "{out}");
    let base = after(&out, "pub trait Base {");
    assert!(base.contains("fn g(&self)"), "{out}");
    let derived = after(&out, "pub trait Derived: Base {");
    assert!(derived.contains("fn f(&self)"), "{out}");
    assert!(
        !derived.contains("fn g(&self)"),
        "the base's own operation is reached through the supertrait, not repeated: {out}"
    );
}

/// `declarations(ast, Language::Rust)` refuses the same model:
/// `COMPLETE_DECLARATIONS_LANGUAGES` does not carry Rust, precisely
/// because this codec-less entry point cannot give the answer the test
/// above measures.
#[test]
fn declarations_alone_refuses_rust_for_the_same_model_source_answers() {
    let ast = model(DERIVED);
    let error = zerodds_lang_rules::generate::declarations(&ast, Language::Rust)
        .expect_err("declarations has no codec, so it cannot be Rust's complete answer");
    assert!(
        error
            .to_string()
            .contains("has no complete DDS-RPC 1.0 binding"),
        "{error}"
    );
}

/// Ada writes `type Derived is interface;` with no progenitor list —
/// the same flattening C#'s, Go's, Swift's, Nim's and Zig's bindings
/// take, over `adapter::ada`'s own `Get_`/`Set_` primitive-operation
/// convention — reached through `generate::source` (`ada_source`) and
/// not through `generate::declarations`, for the reason RM 7.2(4)
/// gives on `source`'s own doc: Ada keeps a subprogram's profile and
/// its body in two different compilation units, so the codec's bodies
/// cannot be spliced into `declarations`'s text the way every free-
/// function column's can.
#[test]
fn the_ada_interface_carries_the_bases_operation_flattened_in() {
    let ast = model(DERIVED);
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let out = zerodds_lang_rules::generate::source(&ast, Language::Ada, key)
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert!(out.contains("type Derived is interface;"), "{out}");
    assert!(
        out.contains("procedure G (Self : in Derived) is abstract;"),
        "{out}"
    );
    assert!(
        out.contains("procedure F (Self : in Derived) is abstract;"),
        "{out}"
    );
}

/// `declarations(ast, Language::Ada)` refuses the same model, for the
/// same reason Rust's does above.
#[test]
fn declarations_alone_refuses_ada_for_the_same_model_source_answers() {
    let ast = model(DERIVED);
    let error = zerodds_lang_rules::generate::declarations(&ast, Language::Ada)
        .expect_err("declarations has no codec, so it cannot be Ada's complete answer");
    assert!(
        error
            .to_string()
            .contains("has no complete DDS-RPC 1.0 binding"),
        "{error}"
    );
}

// -- every other column: refuses, not flattens or drops ----------------------

/// Every language outside [`COMPLETE_DECLARATIONS_LANGUAGES`] refuses
/// the whole model through `declarations` — never a derived declaration
/// with the base's operation quietly missing from it, and never one
/// with it flattened in while `raises` and an attribute stay unspoken.
/// The message names the interface, its position, and which languages
/// do have a binding.
#[test]
fn every_incomplete_language_refuses_rather_than_flattens_or_drops() {
    let ast = model(DERIVED);
    let mut refused = Vec::new();
    for language in Language::ALL {
        if COMPLETE_DECLARATIONS_LANGUAGES.contains(&language) {
            continue;
        }
        let error = zerodds_lang_rules::generate::declarations(&ast, language)
            .expect_err("an incomplete language must refuse an interface");
        let message = error.to_string();
        assert!(
            message.contains("Base") && message.contains("Derived"),
            "{language:?}: the refusal does not name both interfaces: {message}"
        );
        assert!(
            message.contains("c++")
                && message.contains("c#")
                && message.contains("java")
                && message.contains("typescript"),
            "{language:?}: the refusal does not name the languages that do have a binding: {message}"
        );
        refused.push(language);
    }
    // `Language::ALL` less the three complete through `declarations`.
    assert_eq!(
        refused.len(),
        Language::ALL.len() - COMPLETE_DECLARATIONS_LANGUAGES.len()
    );
}

/// C, Python and Node used to declare nothing for an interface and
/// answer `Ok` — silently. They now refuse like every other incomplete
/// language; measured on their own, because they are the three this
/// project already knew had no mapping at all, and a regression back to
/// the silent, exit-0 answer must fail loudly here.
#[test]
fn c_python_and_node_now_refuse_instead_of_silently_answering_empty() {
    let ast = model(DERIVED);
    for language in [Language::C, Language::Python, Language::Node] {
        let error = zerodds_lang_rules::generate::declarations(&ast, language)
            .expect_err("C, Python and Node must refuse rather than answer empty");
        assert!(
            error
                .to_string()
                .contains("has no complete DDS-RPC 1.0 binding"),
            "{language:?}: {error}"
        );
    }
}

/// A model with no interface at all is unaffected: every one of the
/// eighteen still generates its ordinary declarations.
#[test]
fn a_model_with_no_interface_is_not_refused() {
    let ast = model("struct S { long a; };");
    for language in Language::ALL {
        if COMPLETE_DECLARATIONS_LANGUAGES.contains(&language) {
            continue;
        }
        zerodds_lang_rules::generate::declarations(&ast, language)
            .unwrap_or_else(|error| panic!("{language:?}: {error:?}"));
    }
}

/// `COMPLETE_INTERFACE_LANGUAGES` (`source`) is a superset of
/// `COMPLETE_DECLARATIONS_LANGUAGES` (`declarations`) by exactly Rust —
/// stated once here so a change to either constant that breaks the
/// relationship fails a test that names it, rather than being noticed
/// only in the two lists drifting apart silently.
#[test]
fn source_completeness_is_declarations_completeness_plus_rust_and_ada() {
    for language in COMPLETE_DECLARATIONS_LANGUAGES {
        assert!(
            COMPLETE_INTERFACE_LANGUAGES.contains(&language),
            "{language:?} is complete for `declarations` but not for `source`"
        );
    }
    // Rust and Ada: both need a codec `source` carries and
    // `declarations` never does (`COMPLETE_DECLARATIONS_LANGUAGES`'s
    // own doc names the two reasons — a `Requester` with no encoder,
    // and RM 7.2(4)). Order-independent: which of the two a future
    // change adds first to `COMPLETE_INTERFACE_LANGUAGES` is not this
    // test's business.
    let mut added: Vec<Language> = COMPLETE_INTERFACE_LANGUAGES
        .into_iter()
        .filter(|language| !COMPLETE_DECLARATIONS_LANGUAGES.contains(language))
        .collect();
    added.sort_by_key(|language| language.as_str());
    assert_eq!(added, [Language::Ada, Language::Rust]);
}

/// A base chain deeper than the flattening walk follows is refused, not
/// cut: stopping at the limit would drop every operation of the bases
/// past it from Go's (and every flattening column's) interface.
#[test]
fn an_inheritance_chain_past_the_limit_is_refused_not_cut() {
    let mut idl = String::from("module deep {\n  interface I0 { long op0(); };\n");
    for level in 1..=70 {
        idl.push_str(&format!(
            "  interface I{level} : I{} {{ long op{level}(); }};\n",
            level - 1
        ));
    }
    idl.push_str("};\n");
    let ast = model(&idl);
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let error = zerodds_lang_rules::generate::source(&ast, Language::Go, key)
        .expect_err("a 70-level chain must not flatten silently");
    let text = format!("{error:?}");
    assert!(
        text.contains("deeper than") || text.contains("TooDeep"),
        "the refusal names the depth: {text}"
    );
}
