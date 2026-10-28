// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A member whose type lives in another module.
//!
//! This is the shape reported as issues #41 and #42 — a deserialisation
//! error at a Cyclone peer, and a topic mismatch in the other direction:
//!
//! ```text
//! module common {
//!   module util { struct Location { double x; double y; double z; }; };
//!   struct Robot { ::common::util::Location location; };
//! };
//! ```
//!
//! Ordinary IDL, and the reference did not resolve. Lowering leaves a
//! name pending — a forward declaration needs that — and something has
//! to run `Ast::resolve_all_by_name` afterwards. Nothing in a production
//! path did: the call appeared only in tests and in one build script
//! that had found this out for itself. The member reached the generator
//! unresolved, the field was left out of the generated type, and the
//! sample went onto the wire missing it.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::fs;

use zerodds_build::Config;

const CROSS_MODULE: &str = "\
module common {
    module util {
        struct Location { double x; double y; double z; };
        struct Orientation { double x; double y; double z; };
    };
    struct Robot {
        ::common::util::Location location;
        ::common::util::Orientation orientation;
    };
};
";

#[test]
fn a_member_typed_from_another_module_reaches_the_generated_struct() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Robot.idl");
    fs::write(&idl, CROSS_MODULE).expect("write");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out.join("Robot.rs")).expect("read");
    assert!(generated.contains("struct Robot"), "{generated}");
    assert!(
        generated.contains("location"),
        "the member is in the struct: {generated}"
    );
    assert!(
        generated.contains("orientation"),
        "and so is the second one: {generated}"
    );
    // The member's *type*, not just its name. An unresolved reference
    // produced a field the generator could not type, which is what put a
    // short sample on the wire.
    assert!(
        generated.contains("Location"),
        "the type it names resolves: {generated}"
    );
    assert!(generated.contains("Orientation"), "{generated}");
}

/// Two of the three spellings resolve; the third is a known gap.
///
/// `::common::util::Location` and `common::util::Location` both reach
/// the type. `util::Location` — the relative form, valid under §7.5 from
/// inside `module common` — does not: the lowering marks **every** name
/// it reads absolute, whether or not the source wrote a leading `::`,
/// and a pending reference carries no scope to resolve against later.
///
/// The same root was fixed today for constants and enumerators, where
/// `name_expr` walks the scope stack on the spot. A type reference
/// cannot do that — it may be forward-declared — so it needs the scope
/// carried into the pending reference, which is a change to
/// `PendingRef`. Written down rather than left to be met again.
#[test]
fn a_qualified_name_resolves_and_a_relative_one_is_the_known_gap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Spellings.idl");
    fs::write(
        &idl,
        "module common {
            module util { struct Location { double x; }; };
            struct A { ::common::util::Location loc; };
            struct B { common::util::Location loc; };
        };",
    )
    .expect("write");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out.join("Spellings.rs")).expect("read");
    assert!(
        generated.matches("Location").count() >= 2,
        "both qualified spellings resolve: {generated}"
    );

    // The relative form, which was a gap when this test was written and
    // is not one now: §7.5 admits `util::Location` from inside `common`
    // by walking the enclosing scopes, and the resolver does that.
    //
    // Worth holding open rather than deleting. It is the spelling in
    // issues #41 and #42, and it is the one Fast DDS's generator does
    // *not* take — see docs/edge-case-map.md — so it has to keep working
    // here for a file written against a mixed fleet to be translatable
    // at all.
    let relative = dir.path().join("Relative.idl");
    fs::write(
        &relative,
        "module common {
            module util { struct Location { double x; }; };
            struct C { util::Location loc; };
        };",
    )
    .expect("write");
    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&relative])
        .expect("§7.5 admits `util::Location` from inside `common`");

    let relative_out = fs::read_to_string(out.join("Relative.rs")).expect("read");
    assert!(
        relative_out.contains("Location"),
        "the member survives with its type, rather than the struct \
         emitting without it: {relative_out}"
    );
}

/// A name that genuinely does not resolve is an **error** now. It used
/// to be nothing at all: the member was dropped and the build succeeded.
#[test]
fn a_name_that_resolves_to_nothing_fails_the_build() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Missing.idl");
    fs::write(&idl, "module m { struct S { ::nowhere::Absent gap; }; };").expect("write");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    let result = Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&idl]);
    assert!(
        result.is_err(),
        "a member typed by a name that names nothing must not compile"
    );
}
