// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus, as Rust, through `rustc`.
//!
//! The counterpart of the `every_corpus_case_that_renders_also_compiles`
//! that `swift_end_to_end.rs`, `go_end_to_end.rs` and eight more carry.
//! The Rust column had none: `end_to_end.rs` walks the chain and asserts
//! the text, and `docs/OPEN-ITEMS.md` records that the six oldest columns
//! lost their compile gate when `zerodds-idl-<lang>` was removed.
//!
//! `--emit=metadata` and not `cargo check`: what is in question is the
//! generated file, and a scratch crate per case would spend a second in
//! Cargo before `rustc` saw a line. `rustc` answers a corpus case in
//! about fifty milliseconds, which makes this the cheapest of the six
//! gates rather than the most expensive.
//!
//! ## What a generated file needs around it
//!
//! Two things, and both are the column's contract with its consumer
//! rather than this harness inventing a runtime:
//!
//! 1. **`use std::collections::BTreeMap;`** — `adapter::cst` renders an
//!    IDL `map` as a bare `BTreeMap<K, V>`, so the file must be placed
//!    where that name is in scope. `crates/wire-conformance/build.rs`
//!    and `crates/corba-rust/tests/compile_check.rs` both write a
//!    preamble for the same reason.
//! 2. **`zerodds_float128`** — an IDL `long double` becomes
//!    `zerodds_float128::Float128`, a real binary128. The crate is in
//!    this tree and is compiled here, by the same `rustc`, because an
//!    rlib built by another toolchain is an `E0514` about the harness
//!    rather than about the generated code.
//!
//! Both `rustc` invocations run with the repository as their working
//! directory, so `rust-toolchain.toml` decides which compiler answers.
//!
//! `print_stderr` is allowed here on purpose: a skipped compiler check
//! and a frontend refusal both have to be *said*, and standard error is
//! the channel a passing test does not otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use zerodds_lang_cst::Language;

#[path = "support/corpus.rs"]
mod corpus_harness;
#[path = "support/generate.rs"]
mod generate_harness;

/// What every generated file is compiled behind.
///
/// `dead_code` and `unused_imports`: a declarations file declares and
/// calls nothing, and a case with no `map` member does not use the
/// import. Neither is a statement about the generated code.
const PREAMBLE: &str = "#![allow(dead_code, unused_imports)]\nuse std::collections::BTreeMap;\n";
// -- the corpus ----------------------------------------------------------
/// How many corpus cases have to reach `rustc` for this gate to mean
/// anything. 418 cases render today.
const MUST_RENDER: usize = 350;

/// The names the Rust column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[];

/// The cases whose generated Rust `rustc` rejects, each with the message
/// it answers.
///
/// Frontend-invalid array cycles are checked by `corpus_frontend`, not
/// accepted as compiler failures. An admitted binding failure, including
/// missing `@external` indirection, must fail this compiler gate.
const DOES_NOT_COMPILE: &[(&str, &str)] = &[];

// -- the compiler --------------------------------------------------------

/// Whether `rustc` is usable at all.
fn rustc_available() -> bool {
    Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Where the scratch artefacts go.
fn work() -> PathBuf {
    std::env::temp_dir().join(format!("zd-rust-corpus-{}", std::process::id()))
}

/// `zerodds-float128`, compiled once for the whole test binary.
///
/// Once, because two tests calling `rustc` into one output directory at
/// the same time is a race the compiler answers with `failed to open
/// object file` — a message about the harness and not about the
/// generated code.
static DEPENDENCY: LazyLock<Result<PathBuf, String>> = LazyLock::new(build_dependency);

/// The rlib every case is compiled against, or the compiler's own
/// message.
fn dependency() -> Result<PathBuf, String> {
    DEPENDENCY.clone()
}

/// `zerodds-float128`, compiled by the same `rustc` that compiles the
/// cases.
///
/// Not the artefact Cargo left in `target/debug/deps`: that one was
/// built by whichever toolchain last ran there, and `rustc` answers
/// `E0514` for an rlib from another — again a message about the harness
/// and not about the generated code.
fn build_dependency() -> Result<PathBuf, String> {
    let directory = work().join("deps");
    let rlib = directory.join("libzerodds_float128.rlib");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let output = Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .args([
            "--edition",
            "2024",
            "--crate-type=lib",
            "--crate-name",
            "zerodds_float128",
        ])
        .arg("--out-dir")
        .arg(&directory)
        .arg(corpus_harness::repository_root().join("crates/float128/src/lib.rs"))
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(rlib)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// `zerodds-wire`, compiled once for the whole test binary, for the reason [`DEPENDENCY`] is.
///
/// A generated map member is a `Vec<zerodds_wire::MapEntry<K, V>>` (XTypes 1.3 §7.5.1.3.6).
static WIRE: LazyLock<Result<PathBuf, String>> = LazyLock::new(|| build_wire(&dependency()?));

/// The `zerodds-wire` rlib, or the compiler's own message.
fn wire() -> Result<PathBuf, String> {
    WIRE.clone()
}

/// One generated file through `rustc`, behind [`PREAMBLE`].
fn rust_compiles(dependency: &Path, source: &str, tag: &str) -> Result<(), String> {
    let wire = wire()?;
    let directory = work().join(tag);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let file = directory.join("zd_types.rs");
    std::fs::write(&file, format!("{PREAMBLE}{source}")).map_err(|error| error.to_string())?;
    let output = Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .args(["--edition", "2024", "--crate-type=lib", "--emit=metadata"])
        .arg("--extern")
        .arg(format!("zerodds_float128={}", dependency.display()))
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("-L")
        .arg(dependency.parent().unwrap_or(Path::new(".")))
        .arg("-L")
        .arg(wire.parent().unwrap_or(Path::new(".")))
        .arg("-o")
        .arg(directory.join("zd_types.rmeta"))
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&directory);
    result
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Without this, a green result below could come from a compiler that
    // accepts anything — or from a harness that never ran one.
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    let dependency = dependency().expect("zerodds-float128 compiles");
    let bad = "pub struct S {\n    pub x: NoSuchTypeExists,\n}\n";
    let why = rust_compiles(&dependency, bad, "deliberate").expect_err("rustc accepted a bad file");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    // And a good one passes, so the check is not simply always red — and
    // it names the two things the preamble and the `--extern` are here
    // for, so a gate that lost either goes red at this test rather than
    // over a third of the corpus.
    rust_compiles(
        &dependency,
        "pub struct S {\n    pub table: BTreeMap<i32, i32>,\n    \
         pub wide: zerodds_float128::Float128,\n}\n",
        "control",
    )
    .expect("rustc rejected a good file");
}

// -- a union's discriminator, compiled and run -------------------------

/// `zerodds-wire`, compiled by the same `rustc` as the program below.
fn build_wire(float128: &Path) -> Result<PathBuf, String> {
    let directory = work().join("wire");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let output = Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .args([
            "--edition",
            "2024",
            "--crate-type=lib",
            "--crate-name",
            "zerodds_wire",
        ])
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("--out-dir")
        .arg(&directory)
        .arg(corpus_harness::repository_root().join("crates/wire/src/lib.rs"))
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(directory.join("libzerodds_wire.rlib"))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// IDL 4.2 §7.4.1.4.4.4: a union's value is its discriminator and the
/// member it selects. The Rust column dropped the discriminator where the
/// case does not fix it — a value read under the second label of a case,
/// or under any value the `default:` case takes, went out under another —
/// had no value for a discriminator selecting no member, and wrote a
/// `boolean` or `char` discriminator as an integer `rustc` refuses. The
/// program round-trips each shape, checks the octets of a discriminator,
/// and checks that a pairing the type does not allow is refused.
const UNION_IDL: &str = "
    union U switch (long) { case 1: case 2: long m; default: short n; };
    union Ua switch (long) { case 1: long m; };
    enum Color { RED, GREEN, BLUE };
    union V switch (Color) { case RED: case GREEN: long m; case BLUE: short n; };
    union X switch (boolean) { case TRUE: long t; case FALSE: short f; };
    union Y switch (char) { case 'a': long m; default: short d; };
";

/// What `idlc generate --lang rust` writes ahead of the body, little
/// endian.
const UNION_PREAMBLE: &str = "#![allow(dead_code, unused_imports)]\n\
     use zerodds_wire::{Endianness, Reader, WireError, Writer};\n\
     pub const ZD_ENDIANNESS: Endianness = Endianness::Little;\n";

const UNION_PROGRAM: &str = r#"
fn bytes<T>(value: &T, encode: fn(&T, &mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::new(ZD_ENDIANNESS);
    encode(value, &mut w).expect("encodes");
    w.finish()
}

fn refused<T>(value: &T, encode: fn(&T, &mut Writer) -> Result<(), WireError>) -> bool {
    let mut w = Writer::new(ZD_ENDIANNESS);
    encode(value, &mut w).is_err()
}

macro_rules! round_trip {
    ($type:ident, $value:expr) => {{
        let value = $value;
        let octets = bytes(&value, $type::encode);
        let mut r = Reader::new(&octets, ZD_ENDIANNESS);
        assert_eq!($type::decode(&mut r).expect("decodes"), value);
        octets
    }};
}

fn main() {
    // The second label of a case comes back as itself, and goes out as
    // itself: the discriminator sits right after the union's DHEADER.
    let octets = round_trip!(U, U::M(2, 5));
    assert_eq!(&octets[4..8], &2i32.to_le_bytes());
    round_trip!(U, U::M(1, 6));
    let octets = round_trip!(U, U::N(7, 3));
    assert_eq!(&octets[4..8], &7i32.to_le_bytes());
    assert!(refused(&U::M(3, 1), U::encode), "3 is no label of `m`");
    assert!(refused(&U::N(1, 0), U::encode), "1 selects `m`, not the default");

    // A discriminator that selects no member.
    let octets = round_trip!(Ua, Ua::_NoMember(9));
    assert_eq!(octets.len(), 8, "the DHEADER and the discriminator, no member");
    round_trip!(Ua, Ua::M(4));
    assert!(refused(&Ua::_NoMember(1), Ua::encode), "1 selects `m`");
    assert_eq!(Ua::default(), Ua::_NoMember(0));

    // An enumeration, a boolean and a character discriminator.
    round_trip!(V, V::M(Color::GREEN, 4));
    round_trip!(V, V::N(8));
    assert!(refused(&V::M(Color::BLUE, 4), V::encode), "BLUE selects `n`");
    round_trip!(X, X::T(1));
    round_trip!(X, X::F(2));
    round_trip!(Y, Y::M(1));
    let octets = round_trip!(Y, Y::D('z', 2));
    assert_eq!(octets[4], b'z');
    assert!(refused(&Y::D('a', 2), Y::encode), "'a' selects `m`");
}
"#;

#[test]
fn a_union_keeps_its_discriminator_through_rustc_and_back() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    let float128 = dependency().expect("zerodds-float128 compiles");
    let wire = wire().expect("zerodds-wire compiles");
    let parsed = zerodds_idl_parse::parse(UNION_IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "unions.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolves");
    let key = zerodds_ast_layout::key::LayoutKey::data(
        zerodds_ast_layout::key::Encoding::DelimitedCdr,
        zerodds_ast_layout::key::Endianness::Little,
    );
    let source =
        zerodds_lang_rules::generate::source(&ast, Language::Rust, key).expect("generates");

    let directory = work().join("unions");
    std::fs::create_dir_all(&directory).expect("scratch directory");
    let file = directory.join("main.rs");
    // `generate::source` is the body; the `use` line and the byte order
    // are the preamble `idlc` puts in front (`tools/idlc/src/generate.rs`).
    std::fs::write(&file, format!("{UNION_PREAMBLE}{source}\n{UNION_PROGRAM}")).expect("writes");
    let binary = directory.join("unions");
    let output = Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .args(["--edition", "2024", "--crate-type=bin"])
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("-L")
        .arg(wire.parent().expect("a directory"))
        .arg("-L")
        .arg(float128.parent().expect("a directory"))
        .arg("-o")
        .arg(&binary)
        .arg(&file)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "the generated unions do not compile:\n{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary).output().expect("the program runs");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        run.status.success(),
        "a union round trip failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// XTypes 1.3 §7.5.1.4.1.3: a language with no mapping of its own for inheritance binds a
/// derived structure as if an instance of the base type were its first member `parent`. The wire
/// still carries the base's members first (§7.4.1.1.5.1), so the derived structure's octets are
/// those of the flat one; a key of the base is a key of the derived structure.
const PARENT_IDL: &str = "
    @final struct Base { @key long id; octet o; };
    @final struct Derived : Base { string s; @optional long maybe; };
    @final struct Further : Derived { short z; };
    @final struct Flat { long id; octet o; string s; @optional long maybe; short z; };
    @mutable struct MBase { long a; };
    @mutable struct MDerived : MBase { long b; };
";

const PARENT_PROGRAM: &str = r#"
fn bytes<T>(value: &T, encode: fn(&T, &mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::new(ZD_ENDIANNESS);
    encode(value, &mut w).expect("encodes");
    w.finish()
}

fn main() {
    // The base is the first member and the others are the structure's own.
    let further = Further {
        parent: Derived {
            parent: Base { id: 7, o: 200 },
            s: "text".to_string(),
            maybe: Some(5),
        },
        z: -3,
    };
    let flat = Flat { id: 7, o: 200, s: "text".to_string(), maybe: Some(5), z: -3 };
    // Final structures of different shapes are not the same octets by accident: the derived one
    // is what the flat one is, member by member.
    let derived_octets = bytes(&further, Further::encode);
    let flat_octets = bytes(&flat, Flat::encode);
    assert_eq!(derived_octets, flat_octets, "the wire form of a derived structure");
    let mut r = Reader::new(&derived_octets, ZD_ENDIANNESS);
    assert_eq!(Further::decode(&mut r).expect("decodes"), further);
    assert_eq!(further.parent.parent.id, 7);
    // The default of a derived structure holds the default of its base.
    assert_eq!(Further::default().parent.parent, Base::default());

    // A mutable structure: the parameter list names every member of the chain.
    let m = MDerived { parent: MBase { a: 1 }, b: 2 };
    let octets = bytes(&m, MDerived::encode);
    let mut r = Reader::new(&octets, ZD_ENDIANNESS);
    assert_eq!(MDerived::decode(&mut r).expect("decodes"), m);

    // The key of the base is the key of the derived structure.
    let a = Derived { parent: Base { id: 9, o: 1 }, s: String::new(), maybe: None };
    let b = Derived { parent: Base { id: 9, o: 2 }, s: "x".to_string(), maybe: Some(1) };
    assert_eq!(a.key_hash().expect("hash"), b.key_hash().expect("hash"));
    let c = Derived { parent: Base { id: 10, o: 1 }, s: String::new(), maybe: None };
    assert_ne!(a.key_hash().expect("hash"), c.key_hash().expect("hash"));
    let holder = a.key_holder_be().expect("holder");
    let mut r = Reader::new(&holder, Endianness::Big);
    let back = Derived::key_from_holder_be(&mut r).expect("key from holder");
    assert_eq!(back.parent.id, 9);
}
"#;

#[test]
fn a_derived_structure_holds_its_base_as_its_first_member_parent() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    let float128 = dependency().expect("zerodds-float128 compiles");
    let wire = wire().expect("zerodds-wire compiles");
    let parsed = zerodds_idl_parse::parse(PARENT_IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "parent.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolves");
    let key = zerodds_ast_layout::key::LayoutKey::data(
        zerodds_ast_layout::key::Encoding::DelimitedCdr,
        zerodds_ast_layout::key::Endianness::Little,
    );
    let source =
        zerodds_lang_rules::generate::source(&ast, Language::Rust, key).expect("generates");
    assert!(source.contains("pub parent: Base,"), "{source}");
    let directory = work().join("parent");
    std::fs::create_dir_all(&directory).expect("scratch directory");
    let file = directory.join("main.rs");
    std::fs::write(&file, format!("{UNION_PREAMBLE}{source}\n{PARENT_PROGRAM}")).expect("writes");
    let binary = directory.join("parent");
    let output = Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .args(["--edition", "2024", "--crate-type=bin"])
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("-L")
        .arg(wire.parent().expect("a directory"))
        .arg("-L")
        .arg(float128.parent().expect("a directory"))
        .arg("-o")
        .arg(&binary)
        .arg(&file)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "the generated structures do not compile:\n{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary).output().expect("the program runs");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        run.status.success(),
        "a derived structure failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// XTypes 1.3 §7.5.1.3.6: a map in a language with no mapping of its own is the sequence of
/// `MapEntry` structures of §7.4.1.1.4. A Rust map member is a `Vec<MapEntry<K, V>>`: the entries
/// keep the order they were put in and go out in it; `BTreeMap` is the named option
/// (`--rust-map btree-map`) and writes ascending by key.
const MAP_IDL: &str = "
    @final struct Table { map<string, long> counts; map<long, string, 3> small; };
";

const MAP_PREAMBLE: &str = "#![allow(dead_code, unused_imports)]\n\
     use zerodds_wire::{Endianness, Narrowed, Reader, TryConstruct, WireError, Writer};\n\
     pub const ZD_ENDIANNESS: Endianness = Endianness::Little;\n";

const MAP_PROGRAM: &str = r#"
use zerodds_wire::MapEntry;

fn bytes<T>(value: &T, encode: fn(&T, &mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::new(ZD_ENDIANNESS);
    encode(value, &mut w).expect("encodes");
    w.finish()
}

fn main() {
    let table = Table {
        counts: vec![MapEntry::new("b".to_string(), 1), MapEntry::new("a".to_string(), 2)],
        small: vec![MapEntry::new(5, "x".to_string())],
    };
    let octets = bytes(&table, Table::encode);
    // The DHEADER of the delimited map, the count of entries, then the entries as they were
    // put in: "b" before "a".
    assert_eq!(&octets[4..8], &2u32.to_le_bytes());
    let b = octets.iter().position(|&o| o == b'b').expect("b");
    let a = octets.iter().position(|&o| o == b'a').expect("a");
    assert!(b < a, "the entries go out in the order they are held");
    let mut r = Reader::new(&octets, ZD_ENDIANNESS);
    assert_eq!(Table::decode(&mut r).expect("decodes"), table);
    // A bound counts entries.
    let over = Table {
        counts: Vec::new(),
        small: (0..4).map(|n| MapEntry::new(n, String::new())).collect(),
    };
    let mut w = Writer::new(ZD_ENDIANNESS);
    assert!(over.encode(&mut w).is_err(), "four entries in a map<long, string, 3>");
}
"#;

#[test]
fn a_map_is_a_sequence_of_entries_that_keeps_its_order() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    let float128 = dependency().expect("zerodds-float128 compiles");
    let wire = wire().expect("zerodds-wire compiles");
    let parsed = zerodds_idl_parse::parse(MAP_IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "map.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolves");
    let key = zerodds_ast_layout::key::LayoutKey::data(
        zerodds_ast_layout::key::Encoding::PlainCdr2,
        zerodds_ast_layout::key::Endianness::Little,
    );
    let source =
        zerodds_lang_rules::generate::source(&ast, Language::Rust, key).expect("generates");
    assert!(
        source.contains("Vec<zerodds_wire::MapEntry<String, i32>>"),
        "{source}"
    );
    // The idiomatic container, by name.
    {
        let _form = zerodds_lang_rules::adapter::cst::RustMapFormScope::enter(
            zerodds_lang_rules::adapter::cst::RustMapForm::BTreeMap,
        );
        let btree =
            zerodds_lang_rules::generate::source(&ast, Language::Rust, key).expect("generates");
        assert!(btree.contains("BTreeMap<String, i32>"), "{btree}");
    }
    let directory = work().join("map");
    std::fs::create_dir_all(&directory).expect("scratch directory");
    let file = directory.join("main.rs");
    std::fs::write(&file, format!("{MAP_PREAMBLE}{source}\n{MAP_PROGRAM}")).expect("writes");
    let binary = directory.join("map");
    let output = Command::new("rustc")
        .current_dir(corpus_harness::repository_root())
        .args(["--edition", "2024", "--crate-type=bin"])
        .arg("--extern")
        .arg(format!("zerodds_wire={}", wire.display()))
        .arg("--extern")
        .arg(format!("zerodds_float128={}", float128.display()))
        .arg("-L")
        .arg(wire.parent().expect("a directory"))
        .arg("-L")
        .arg(float128.parent().expect("a directory"))
        .arg("-o")
        .arg(&binary)
        .arg(&file)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "the generated maps do not compile:\n{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary).output().expect("the program runs");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        run.status.success(),
        "a map round trip failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// The whole corpus through `rustc`, one invocation per case.
///
/// `#[ignore]`, because it is one compiler start per corpus case. It is
/// the cheapest of the six — about half a minute for the corpus as it
/// stands — and it is still a compiler per case rather than a unit test,
/// so it is run with `--include-ignored` and its result is what the
/// `idl-rust-corpus` job records.
#[test]
#[ignore = "one `rustc` start per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    if !rustc_available() {
        eprintln!("SKIPPED: `rustc` is not usable.");
        return;
    }
    let dependency = dependency().expect("zerodds-float128 compiles");
    let mut failures = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut frontend = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus_harness::corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let refused = REFUSED.iter().find(|(case, _)| *case == name);
        let text = match (
            generate_harness::generate_declarations(
                &source,
                &path.display().to_string(),
                Language::Rust,
            ),
            refused,
        ) {
            (Ok(text), None) => text,
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
                continue;
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure it
                // is. `parse` and `lower` are the frontend's and are the
                // same for every column.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
                continue;
            }
            (Err(_), Some(_)) => continue,
        };
        checked += 1;
        let known = DOES_NOT_COMPILE.iter().find(|(case, _)| *case == name);
        match (rust_compiles(&dependency, &text, &name), known) {
            (Ok(()), None) | (Err(_), Some(_)) => {}
            (Ok(()), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: compiles now, but is listed as {why}"));
            }
            (Err(why), None) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!(
        "{checked} corpus case(s) reached rustc; {} failed unexpectedly",
        failures.len()
    );
    // The per-case directories are gone already, each removed by the
    // invocation that made it. What is left under `work()` is the one
    // rlib `DEPENDENCY` built, and it is not removed here: the other test
    // in this binary holds the same `LazyLock` and would be handed the
    // path of a file that no longer exists. Serially — the order
    // `--test-threads=1` gives — that is exactly what happened, and
    // `rustc` answered `extern location for zerodds_float128 does not
    // exist`. The Java column leaves its shared runtime classes in
    // `std::env::temp_dir()` for the same reason.
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Rust never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    assert!(
        checked >= MUST_RENDER,
        "only {checked} corpus cases reached `rustc`; the gate says nothing \
         below {MUST_RENDER}"
    );
}
