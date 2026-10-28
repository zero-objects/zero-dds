// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated-code defects users reported, each held open by a test.
//!
//! Every case here is a real report against a real build script. They are
//! in one file because they share a shape: two `.idl` files pulled into
//! one Rust scope, where a name the generator chose for itself collides
//! with the same name from the other file. Rust allows a name once per
//! scope; nothing about emitting two files can fix that, so each case is
//! either a name that has to be distinct or a bundle that has to be one
//! translation unit.
//!
//! One of them was fixed once and came back when this crate was rebuilt.
//! That is the reason for the file: a defect nobody wrote a test for is
//! a defect that returns.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;
use std::path::Path;

use zerodds_build::{Config, RustMapForm};

/// Two units declaring the same IDL module.
fn two_units(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let robot = dir.join("Robot.idl");
    let util = dir.join("Util.idl");
    fs::write(
        &robot,
        "module common { module util { struct Robot { uint32 id; }; }; };",
    )
    .expect("robot");
    fs::write(
        &util,
        "module common { module util { struct Position { uint32 x; }; }; };",
    )
    .expect("util");
    (robot, util)
}

/// **#25** — `the name type_objects is defined multiple times`.
///
/// The module holding the serialised TypeObjects is named after its
/// unit. A fixed name made the second `include!` a redefinition. Fixed
/// once, reintroduced when this crate was rebuilt, and held open here.
#[test]
fn issue_25_the_type_object_module_is_named_after_its_unit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (robot, util) = two_units(dir.path());
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&robot, &util])
        .expect("compile");

    let line_of = |file: &str| {
        fs::read_to_string(out.join(file))
            .expect("read")
            .lines()
            .find(|l| l.contains("pub mod type_objects"))
            .map(str::trim)
            .map(ToString::to_string)
            .expect("a type-object module")
    };
    let a = line_of("Robot.rs");
    let b = line_of("Util.rs");
    assert_ne!(a, b, "two units must not open the same module: {a} / {b}");
    assert!(a.contains("type_objects_robot"), "{a}");
    assert!(b.contains("type_objects_util"), "{b}");
}

/// **#26** — `inner attribute is not permitted in this context`.
///
/// A generated file is pulled in with `include!`, where an inner
/// attribute is a syntax error. Nothing here emits one.
#[test]
fn issue_26_no_inner_attributes_reach_the_generated_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (robot, _) = two_units(dir.path());
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&robot])
        .expect("compile");

    let generated = fs::read_to_string(out.join("Robot.rs")).expect("read");
    assert!(
        !generated.contains("#!["),
        "an inner attribute cannot survive include!: {generated}"
    );
}

/// **#39** — `the name common is defined multiple times`.
///
/// Two units declaring one IDL module each open `pub mod common`, and
/// two `include!` calls in one scope collide. The module tree is
/// per-file, so emitting two files cannot fix it: the units have to be
/// **one** translation unit.
#[test]
fn issue_39_a_bundle_declares_the_shared_module_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (robot, util) = two_units(dir.path());
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile_bundle(&[&robot, &util], "world")
        .expect("bundle");

    let generated = fs::read_to_string(out.join("world.rs")).expect("read");
    assert_eq!(
        generated.matches("pub mod common").count(),
        1,
        "the shared module is declared once: {generated}"
    );
    assert!(generated.contains("struct Robot"), "{generated}");
    assert!(
        generated.contains("struct Position"),
        "and the other unit's type is in the same tree: {generated}"
    );
}

/// The other half of #39: `compile` still gives each unit its own file,
/// and those two files still collide if both are included in one scope.
///
/// This is not a defect and is asserted so nobody 'fixes' it: a caller
/// with two units that share no module wants two files, and the bundle
/// is what a caller with a shared module reaches for.
#[test]
fn issue_39_separate_compilation_is_still_separate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (robot, util) = two_units(dir.path());
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&robot, &util])
        .expect("compile");

    for file in ["Robot.rs", "Util.rs"] {
        assert!(
            fs::read_to_string(out.join(file))
                .expect("read")
                .contains("pub mod common"),
            "{file} opens the module its IDL declared"
        );
    }
}

/// **#28 / #32** — the generated file compiles under `-D warnings`.
///
/// A `build.rs` inherits the lints of the crate it builds for, and a
/// crate with `#![deny(warnings)]` turns `unused_imports` into an error.
/// The file used to import five runtime names unconditionally; a struct
/// of plain scalars reaches for two of them, and the other three were an
/// error in code the consumer did not write. It took down three CI jobs
/// — `interop-cyclone`, `interop-fastdds`, `interop-harness` — and the
/// reproducer of #32 with them, none of which have anything to do with
/// imports.
///
/// Asserted on the text and not by compiling: this crate cannot invoke
/// `rustc` on its own output without a fixture crate, and what the
/// defect was is stated exactly by which names appear.
#[test]
fn issue_28_the_generated_imports_are_the_ones_the_code_uses() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Plain.idl");
    // Scalars only: no `@optional`, no bounded string, nothing that
    // narrows — so `Narrowed`, `TryConstruct` and `WireError` have no
    // reason to appear.
    fs::write(&idl, "module m { struct Plain { uint32 x; double y; }; };").expect("write");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out.join("Plain.rs")).expect("read");
    let import_line = generated
        .lines()
        .find(|line| line.starts_with("use zerodds_wire::"))
        .unwrap_or("");

    // Every imported name appears in the file, and every name the file
    // uses is imported. Which of the five those are is the emitter's
    // business — `WireError` for instance rides in every codec's return
    // type, whether or not the type can fail to narrow — so the test
    // asserts the correspondence rather than a fixed list, and stays
    // true when the emitter changes.
    for name in ["Narrowed", "Reader", "TryConstruct", "WireError", "Writer"] {
        let imported = import_line.contains(name);
        let used = mentions_outside_the_import(&generated, import_line, name);
        assert_eq!(
            imported, used,
            "{name}: imported={imported}, used={used} — an import nothing uses \
             is an error under -D warnings, and a use nothing imports does not \
             resolve\n{import_line}"
        );
    }
}

/// Does `generated` mention `name` anywhere other than on the import
/// line, as a word of its own?
fn mentions_outside_the_import(generated: &str, import_line: &str, name: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    generated
        .lines()
        .filter(|line| *line != import_line)
        .any(|line| {
            let bytes = line.as_bytes();
            let mut from = 0;
            while let Some(found) = line[from..].find(name) {
                let at = from + found;
                let before = at == 0 || !is_ident(char::from(bytes[at - 1]));
                let after_at = at + name.len();
                let after = after_at >= bytes.len() || !is_ident(char::from(bytes[after_at]));
                if before && after {
                    return true;
                }
                from = at + 1;
            }
            false
        })
}

/// **#28 / #32** — a file that needs no runtime name imports none.
///
/// The empty import list is its own case: an emitted `use` with no names
/// is not valid Rust, so the whole statement has to be left out rather
/// than written empty.
#[test]
fn issue_28_a_file_with_no_codec_carries_no_import_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Empty.idl");
    fs::write(&idl, "module m { typedef uint32 Count; };").expect("write");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .typeobject(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out.join("Empty.rs")).expect("read");
    assert!(
        !generated.contains("use zerodds_wire::{}"),
        "an import of nothing is not valid Rust: {generated}"
    );
}

/// A unit that carries a `map` names `zerodds_wire::MapEntry` by its path,
/// and under [`RustMapForm::BTreeMap`] brings `BTreeMap` in with it.
///
/// The default form is the sequence of `MapEntry` that XTypes 1.3
/// §7.5.1.3.6 binds a map to (`Vec<zerodds_wire::MapEntry<K, V>>`), which
/// needs no import. [`RustMapForm::BTreeMap`] renders a bare `BTreeMap<K, V>`,
/// and a `build.rs` consumer has no preamble to write: without the import
/// in the file the generated code names a type it never imported and the
/// consumer sees `cannot find type BTreeMap in this scope` in code they did
/// not write.
///
/// The same shape as `issue_28`: conditional, because an unused import
/// is an error under the `-D warnings` a `build.rs` inherits.
#[test]
fn a_unit_with_a_map_imports_btreemap_and_one_without_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let with_map = dir.path().join("Mapped.idl");
    fs::write(
        &with_map,
        "module m { struct Mapped { map<string, long> counters; }; };",
    )
    .expect("write");
    let without = dir.path().join("Plainer.idl");
    fs::write(&without, "module m { struct Plainer { uint32 x; }; };").expect("write");

    let generate = |form: RustMapForm, out: &Path| {
        fs::create_dir_all(out).expect("out");
        Config::new()
            .out_dir(out)
            .emit_cargo_directives(false)
            .typeobject(false)
            .rust_map(form)
            .compile(&[&with_map, &without])
            .expect("compile");
        (
            fs::read_to_string(out.join("Mapped.rs")).expect("read"),
            fs::read_to_string(out.join("Plainer.rs")).expect("read"),
        )
    };

    let (entries, plainer) = generate(RustMapForm::Entries, &dir.path().join("entries"));
    assert!(
        entries.contains("pub counters: Vec<zerodds_wire::MapEntry<String, i32>>,"),
        "the map member is not the sequence of MapEntry: {entries}"
    );
    assert!(
        !entries.contains("use std::collections::BTreeMap;"),
        "an import nothing uses is an error under -D warnings: {entries}"
    );
    assert!(
        !plainer.contains("use std::collections::BTreeMap;"),
        "an import nothing uses is an error under -D warnings: {plainer}"
    );

    let (mapped, plainer) = generate(RustMapForm::BTreeMap, &dir.path().join("btree"));
    assert!(
        mapped.contains("pub counters: BTreeMap<String, i32>,"),
        "the map member is not a BTreeMap: {mapped}"
    );
    assert!(
        mapped.contains("use std::collections::BTreeMap;"),
        "the file names BTreeMap and does not import it: {mapped}"
    );
    assert!(
        !plainer.contains("use std::collections::BTreeMap;"),
        "an import nothing uses is an error under -D warnings: {plainer}"
    );
}

/// **#14** — a type named `class` is a C++ keyword.
///
/// The generated Rust is unaffected — `class` is not a Rust keyword —
/// and `zerodds-idlc lint` is what reports it before it reaches a C++
/// backend. Asserted here so the Rust side is known not to mangle a name
/// another language will have to escape.
#[test]
fn issue_14_a_name_that_is_a_foreign_keyword_is_untouched_in_rust() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Keyword.idl");
    fs::write(&idl, "module m { struct Class { uint32 x; }; };").expect("write");
    let out = dir.path().join("out");
    fs::create_dir_all(&out).expect("out");

    Config::new()
        .out_dir(&out)
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");

    let generated = fs::read_to_string(out.join("Keyword.rs")).expect("read");
    assert!(
        generated.contains("struct Class"),
        "the name the IDL wrote: {generated}"
    );
}
