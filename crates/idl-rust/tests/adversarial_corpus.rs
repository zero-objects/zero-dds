// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Adversarial corpus — generated Rust is compiled against the real
//! zerodds-cdr/dcps/types deps (`cargo check --offline`), the target toolchain.
//!
//! Three corpora, each a single compile to stay light on shared CI boxes:
//!   * `reserved_keyword_corpus` — every Rust reserved word usable as an IDL
//!     identifier, at each declaration position (member, struct, enum,
//!     enumerator, module, const, union branch);
//!   * `construct_corpus` — every IDL construct minimally (fixed, enum `@value`,
//!     const of each scalar type, struct inheritance, unions with every
//!     discriminator kind, bitset, bitmask, `@optional` + each extensibility,
//!     sequence, multidimensional array, map incl. `map<struct,_>`, nested +
//!     REOPENED modules);
//!   * `compose_two_files` — two IDLs generated SEPARATELY, then idiomatically
//!     composed into one parent module via `include!`.
//!
//! All are `#[ignore]` (need a toolchain + offline path deps); run with
//! `--include-ignored`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Rust reserved words that are legal IDL identifiers. Excludes the words that
/// are ALSO IDL keywords (`const`/`enum`/`struct`/`union`/`in`/`abstract`),
/// which the IDL front end rejects as identifiers.
const RESERVED: &[&str] = &[
    "as", "break", "continue", "crate", "else", "extern", "fn", "for", "if", "impl", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "super",
    "trait", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "gen", "become",
    "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield",
    "try",
];

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .map(std::path::Path::to_path_buf)
        .unwrap_or(manifest)
}

fn cargo_toml(name: &str) -> String {
    let r = workspace_root();
    let r = r.display();
    format!(
        r#"[package]
name = "adv_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-cdr = {{ path = "{r}/crates/cdr" }}
zerodds-dcps = {{ path = "{r}/crates/dcps" }}
zerodds-types = {{ path = "{r}/crates/types" }}
"#
    )
}

/// Writes a scratch crate whose `src/lib.rs` is `lib_body`, runs
/// `cargo check --offline`, and panics with the source on failure.
fn compile(name: &str, lib_body: &str) {
    let tmp = std::env::temp_dir().join(format!("idlrust_adv_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src/generated")).expect("mkdir");
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("cargo.toml")
        .write_all(cargo_toml(name).as_bytes())
        .expect("write cargo.toml");
    std::fs::File::create(tmp.join("src/lib.rs"))
        .expect("lib.rs")
        .write_all(lib_body.as_bytes())
        .expect("write lib.rs");

    let status = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .status()
        .expect("cargo invocation");
    assert!(
        status.success(),
        "generated code did not compile (exit {:?}). lib.rs:\n{lib_body}",
        status.code()
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Generates one file and returns its Rust source (panicking on failure).
fn gen_one(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default())
        .unwrap_or_else(|e| panic!("parse `{idl}`: {e:?}"));
    generate_rust_module(&ast, &RustGenOptions::default())
        .unwrap_or_else(|e| panic!("gen `{idl}`: {e:?}"))
}

/// Every reserved word at every declaration position, generated + compiled.
#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn reserved_keyword_corpus() {
    // Each position lives in its OWN module to keep the per-word names in
    // distinct IDL scopes (a struct `match` and a const `match` in the same
    // scope would be an IDL redefinition, not a Rust concern).
    let mut idl = String::new();

    // member position: one struct, one field per reserved word.
    idl.push_str("module members { struct AllFields {\n");
    for kw in RESERVED {
        idl.push_str(&format!("  long {kw};\n"));
    }
    idl.push_str("}; };\n");

    // struct-name position.
    idl.push_str("module structs {\n");
    for kw in RESERVED {
        idl.push_str(&format!("  struct {kw} {{ long v; }};\n"));
    }
    idl.push_str("};\n");

    // enum-name + enumerator position.
    idl.push_str("module enums {\n");
    for kw in RESERVED {
        idl.push_str(&format!("  enum E_{kw} {{ {kw} }};\n"));
    }
    idl.push_str("};\n");

    // module-name position.
    idl.push_str("module mods {\n");
    for kw in RESERVED {
        idl.push_str(&format!("  module {kw} {{ struct S {{ long a; }}; }};\n"));
    }
    idl.push_str("};\n");

    // const-name position.
    idl.push_str("module consts {\n");
    for (i, kw) in RESERVED.iter().enumerate() {
        idl.push_str(&format!("  const long {kw} = {i};\n"));
    }
    idl.push_str("};\n");

    // union-branch position.
    idl.push_str("module unions {\n");
    for kw in RESERVED {
        idl.push_str(&format!(
            "  union U_{kw} switch(long) {{ case 0: long {kw}; default: long other; }};\n"
        ));
    }
    idl.push_str("};\n");

    let src = gen_one(&idl);
    compile("reserved", &src);
}

/// Every IDL construct, minimally, in one compile.
#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn construct_corpus() {
    let idl = r#"
        // fixed
        struct FixedHolder { fixed<10,2> price; };

        // enum with explicit @value gaps
        enum Coded { @value(1) A, B, @value(9) C };

        // const of every scalar type
        const short          C_SHORT  = -3;
        const unsigned short C_USHORT = 3;
        const long           C_LONG   = -100;
        const unsigned long  C_ULONG  = 100;
        const long long      C_LLONG  = -1000;
        const double         C_DOUBLE = 3.14;
        const double         C_DINT   = 7;
        const float          C_FLOAT  = 1.5;
        const boolean        C_BOOL   = TRUE;
        const char           C_CHAR   = 'A';
        const octet          C_OCTET  = 255;
        const string         C_STR    = "hello";

        // struct inheritance (base members precede derived members)
        @final struct Base2 { long a; long b; };
        @final struct Derived2 : Base2 { long c; };

        // unions with every discriminator kind
        union UInt  switch(long)    { case 0: long a; case 1: short b; default: octet o; };
        union UChar switch(char)    { case 'A': long a; case 'B': short b; default: octet o; };
        union UBool switch(boolean) { case TRUE: long a; default: octet o; };
        enum Disc { D_A, D_B, D_C };
        union UEnum switch(Disc)    { case D_A: long a; case D_B: short b; default: octet o; };

        // bitset + bitmask
        bitset Flags { bitfield<1> ready; bitfield<1> error; bitfield<3> level; };
        bitmask Perms { READ, WRITE, EXECUTE };

        // @optional + each extensibility
        @final     struct FinalS  { long a; @optional long b; };
        @appendable struct AppS   { long a; @optional string s; };
        @mutable   struct MutS    { @id(1) long a; @optional @id(2) long b; @default(5) @id(3) long c; };

        // sequence, multidimensional array, map (incl. map<struct,_>)
        @final struct KeyPart { long id; };
        @final struct Collections {
            sequence<long>        nums;
            sequence<sequence<long>> nested;
            long                  grid[3][4];
            map<long, string>     m1;
            map<KeyPart, long>    m2;
        };

        // nested + REOPENED modules
        module outer {
            module inner { struct First { long a; }; };
        };
        module outer {
            module inner { struct Second { long b; }; };
            struct Third { long c; };
        };
    "#;
    let src = gen_one(idl);
    compile("constructs", &src);
}

/// Two IDL files generated independently, then composed into one parent module
/// via `include!` — the idiomatic multi-file build pattern. Proves the output
/// is `include!`-composable (no file-level inner attributes, no top-level
/// `use`, no duplicate preamble symbol across files).
#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn compose_two_files() {
    let file_a = gen_one(
        "module common { @appendable struct KeyLabel { @key uint32 id; string label; }; }; \
         enum Severity { LOW, HIGH };",
    );
    let file_b = gen_one(
        "module metrics { @appendable struct CpuInfo { @key string host; float usage; }; }; \
         const long MAX_METRICS = 64;",
    );

    let tmp = std::env::temp_dir().join("idlrust_adv_compose");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src/generated")).expect("mkdir");
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("cargo.toml")
        .write_all(cargo_toml("compose").as_bytes())
        .expect("write cargo.toml");
    std::fs::File::create(tmp.join("src/generated/file_a.rs"))
        .expect("file_a")
        .write_all(file_a.as_bytes())
        .expect("write a");
    std::fs::File::create(tmp.join("src/generated/file_b.rs"))
        .expect("file_b")
        .write_all(file_b.as_bytes())
        .expect("write b");
    // Idiomatic composition: both generated files pulled into ONE parent
    // module. `pub` so the generated `pub const`/types are reachable from the
    // crate root (else `-D warnings` flags them dead, a harness artifact, not a
    // codegen defect).
    let lib = "pub mod generated {\n    include!(\"generated/file_a.rs\");\n    \
               include!(\"generated/file_b.rs\");\n}\n";
    std::fs::File::create(tmp.join("src/lib.rs"))
        .expect("lib.rs")
        .write_all(lib.as_bytes())
        .expect("write lib");

    let status = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .env("RUSTFLAGS", "-D warnings")
        .status()
        .expect("cargo invocation");
    assert!(
        status.success(),
        "multi-file include! composition did not compile (exit {:?})\n--- file_a ---\n{file_a}\n--- file_b ---\n{file_b}",
        status.code()
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
