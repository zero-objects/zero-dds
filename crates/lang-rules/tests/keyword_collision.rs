// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Two IDL members that escape onto one name, in every column.
//!
//! Every column moves a name that collides with a keyword of its
//! language by appending an underscore (C++ prefixes `cxx_`, DDS-RPC 1.0 §7.11.1.1.2 rule 8): an IDL `int` becomes a C
//! `int_`. A scope that already declares `int_` then holds two members
//! with one spelling, and the escape has made one of them unreachable.
//!
//! # Why this is its own file
//!
//! Because what the columns did about it was not the same, and none of
//! it named the cause. Measured on the tree this test was written
//! against:
//!
//! | column | what it said |
//! |---|---|
//! | C | `gen.h:25: error: duplicate member 'int_'`, from the compiler |
//! | C++ | the same, one layer later |
//! | Java | a duplicate field, from `javac` |
//! | Python | `SyntaxError: keyword argument repeated: class_`, at run |
//! | TypeScript | a duplicate property, which JavaScript permits |
//!
//! All of them are late and point at the generated file rather than at
//! the IDL, and the last is the silent one wherever the file is run as
//! JavaScript rather than checked as TypeScript.
//!
//! Two columns cannot have the collision at all, and that is a property
//! of how they escape rather than of a check: **Rust** writes `r#type`
//! and **C#** writes `@class`, and neither `r#` nor `@` is in the IDL
//! identifier grammar, so the escaped form is not a name a member can
//! already carry. Both are asserted here too — a later change to either
//! escape would move that column into the class of the other five, and
//! the guard would then be the thing standing between the source and a
//! dropped member.
//!
//! The escape is the one layer that holds both spellings at once, which
//! is where the check belongs — [`no_spelling_collision`].
//!
//! # What this test does not assert
//!
//! That the collision is *impossible*. It is a property of the source:
//! `struct S { long int; long int_; };` is legal IDL, and a generator
//! that refused every such source would refuse a program the spec
//! allows. What it asserts is that the refusal happens at the layer
//! that can name both members, not three layers down in a C compiler.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::cst::{MaterialiseError, no_spelling_collision};
use zerodds_lang_rules::{ModelGraph, apply};

/// The AST and the rule application for one source.
fn prepared(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "collision.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    lowered.ast
}

/// Whether a materialise result is the collision, and not some other
/// refusal that happens to be an error.
fn is_collision<T>(result: &Result<T, MaterialiseError>) -> bool {
    matches!(result, Err(MaterialiseError::SpellingCollision { .. }))
}

/// A source whose two members collide in the named language.
///
/// One source per language, because the keyword sets differ: `int` is a
/// C keyword and not a Python one, `class` is a C++ and Python keyword
/// and not a C one. A single source would test one column and pass the
/// rest for the wrong reason.
fn colliding(keyword: &str) -> String {
    colliding_with(keyword, &format!("{keyword}_"))
}

/// As [`colliding`], with the spelling the column's escape gives `keyword`
/// written as the second member: the C++ column prefixes (`cxx_class`,
/// DDS-RPC 1.0 §7.11.1.1.2 rule 8) where the others append an underscore.
fn colliding_with(keyword: &str, escaped: &str) -> String {
    format!("@final struct Collide {{ long {keyword}; long {escaped}; }};")
}

macro_rules! column {
    ($name:ident, $keyword:literal, $catalogue:expr, $materialise:expr) => {
        column!(
            $name,
            $keyword,
            concat!($keyword, "_"),
            $catalogue,
            $materialise
        );
    };
    ($name:ident, $keyword:literal, $escaped:expr, $catalogue:expr, $materialise:expr) => {
        #[test]
        fn $name() {
            let ast = prepared(&colliding_with($keyword, $escaped));
            let model = ModelGraph::of(&ast);
            let applied = apply(&model, &$catalogue).expect("apply");
            let result = $materialise(&ast, &applied);
            assert!(
                is_collision(&result),
                "the escape put two members on one name and the column did not say so: {:?}",
                result.map(|_| "materialised")
            );
            // The message has to carry both IDL names: a reader who
            // sees only the spelling cannot tell which two members of
            // the source produced it.
            let Err(error) = result else { unreachable!() };
            let text = error.to_string();
            assert!(
                text.contains($keyword) && text.contains($escaped),
                "the message names neither member: {text}"
            );
        }
    };
}

column!(
    the_c_column_says_so,
    "int",
    zerodds_lang_rules::rule::c::catalogue(),
    zerodds_lang_rules::adapter::c::c_items
);
column!(
    the_cpp_column_says_so,
    "class",
    "cxx_class",
    zerodds_lang_rules::rule::cpp::catalogue(),
    zerodds_lang_rules::adapter::cpp::cpp_items
);
column!(
    the_java_column_says_so,
    "class",
    zerodds_lang_rules::rule::java::catalogue(),
    zerodds_lang_rules::adapter::java::java_items
);
column!(
    the_python_column_says_so,
    "class",
    zerodds_lang_rules::rule::python::catalogue(),
    zerodds_lang_rules::adapter::python::python_module
);
column!(
    the_typescript_column_says_so,
    "constructor",
    zerodds_lang_rules::rule::typescript::catalogue(),
    zerodds_lang_rules::adapter::typescript::ts_items
);

/// The C# column cannot have this collision either, for two reasons
/// that both have to hold.
///
/// A member's name is not escaped at all: it is PascalCased, and only
/// the first letter moves, so `class` becomes `Class` and `class_`
/// becomes `Class_`. That mapping is injective over IDL identifiers
/// because IDL 4.2 §7.2.3 forbids two identifiers in one scope that
/// differ only in case. Where the keyword escape *is* used — type
/// names — it writes `@class`, a verbatim identifier, and `@` is not in
/// the IDL identifier grammar.
///
/// C# has a second renamer, [`unique`], which appends underscores when
/// a member would take the name of its own class. That one does move a
/// name, and records what it did in an `IdlName` attribute; it is a
/// different mechanism from the escape and not what this file is
/// about.
#[test]
fn the_csharp_column_escapes_out_of_reach() {
    let ast = prepared(&colliding("class"));
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &zerodds_lang_rules::rule::csharp::catalogue()).expect("apply");
    let file = zerodds_lang_rules::adapter::csharp::csharp_file(&ast, &applied)
        .expect("a verbatim identifier collides with nothing");
    let text = zerodds_lang_render::csharp::file(&file);
    assert!(text.contains("Class;"), "the first member is gone:\n{text}");
    assert!(
        text.contains("Class_;"),
        "the second member is gone:\n{text}"
    );
}

/// The Rust column cannot have this collision at all.
///
/// It escapes with a raw identifier — `r#type`, not `type_` — so the
/// escaped spelling is not one an IDL member can carry: `r#` is not in
/// the IDL identifier grammar. This is the one column where the guard
/// has nothing to guard, and asserting that keeps a later change to
/// `rust::escape_keyword` from quietly moving it into the same class as
/// the other six.
#[test]
fn the_rust_column_escapes_out_of_reach() {
    let ast = prepared(&colliding("type"));
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &zerodds_lang_rules::rule::rust::catalogue()).expect("apply");
    let items = zerodds_lang_rules::adapter::cst::rust_items(&ast, &applied)
        .expect("a raw identifier collides with nothing");
    let text = zerodds_lang_render::rust::file(&zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    });
    assert!(text.contains("r#type"), "the raw form is gone:\n{text}");
    assert!(text.contains("type_"), "the second member is gone:\n{text}");
}

/// A source with no collision still materialises.
///
/// The guard rejects a pair, not a keyword: `long int;` on its own is
/// what the escape exists for, and a check that refused it would have
/// broken every case in the corpus that names a member after a keyword.
#[test]
fn one_escaped_member_on_its_own_is_not_a_collision() {
    let ast = prepared("@final struct Fine { long int; long other; };");
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &zerodds_lang_rules::rule::c::catalogue()).expect("apply");
    let items = zerodds_lang_rules::adapter::c::c_items(&ast, &applied);
    assert!(
        items.is_ok(),
        "an escaped name alone was refused: {items:?}"
    );
}

/// The helper reports the earlier declaration first.
///
/// Which of the two is "first" is the model's order, so two runs over
/// one model produce the same message; a set-based check would name
/// whichever the hash happened to hold.
#[test]
fn the_pair_is_reported_in_declaration_order() {
    let error = no_spelling_collision(
        [("int_", "int"), ("other", "other"), ("int_", "int_")],
        "C",
        "Collide",
    )
    .expect_err("two names on one spelling");
    let MaterialiseError::SpellingCollision { first, second, .. } = error else {
        panic!("a different error: {error}");
    };
    assert_eq!(first, "int");
    assert_eq!(second, "int_");
}

/// DDS-XTypes 1.3 §7.5.1.2.2 gives a bitmask the flag type
/// `<TypeName>Bits`. A user type of that name, declared before or after
/// the bitmask, is one C identifier twice: the refusal names both.
#[test]
fn the_c_column_refuses_a_type_named_like_the_flag_type_of_a_bitmask() {
    for source in [
        "bitmask Flags { A, B }; @final struct FlagsBits { long x; };",
        "@final struct FlagsBits { long x; }; bitmask Flags { A, B };",
        "module m { bitmask Flags { A }; @final struct FlagsBits { long x; }; };",
    ] {
        let ast = prepared(source);
        let model = ModelGraph::of(&ast);
        let applied = apply(&model, &zerodds_lang_rules::rule::c::catalogue()).expect("apply");
        let result = zerodds_lang_rules::adapter::c::c_items(&ast, &applied);
        let Err(MaterialiseError::SpellingCollision {
            first,
            second,
            spelled,
            ..
        }) = result
        else {
            panic!("{source}: no collision reported");
        };
        assert!(spelled.ends_with("FlagsBits"), "{source}: {spelled}");
        assert!(
            first.contains("FlagsBits") && second.contains("FlagsBits"),
            "{source}: {first} / {second}"
        );
    }
}

/// A bitmask whose flag type has no namesake is untouched.
#[test]
fn the_c_column_accepts_a_bitmask_and_a_type_with_another_name() {
    let ast = prepared("bitmask Flags { A, B }; @final struct FlagsX { long x; };");
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &zerodds_lang_rules::rule::c::catalogue()).expect("apply");
    assert!(zerodds_lang_rules::adapter::c::c_items(&ast, &applied).is_ok());
}
