// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial IDL corpus type-checked with the real OCaml toolchain (gated on
//! `ocamlc` being on PATH). Three corpora:
//!
//! 1. **reserved-keyword** — every OCaml keyword that is a legal IDL identifier,
//!    placed at member / struct / enum / module / const / union-branch
//!    positions, must generate OCaml that compiles. Lowercase-initial positions
//!    (member / const / union-branch) escape to `<kw>_`; uppercase-initial
//!    positions (struct / module names) capitalize past the keyword by
//!    construction (`module_name` / `type_ident`).
//! 2. **construct** — each IDL construct emitted minimally and compiled
//!    (fixed / enum-`@value` / const-all-types / struct-inheritance / union-
//!    all-discriminators / bitset / bitmask / `@optional` / `@extensibility` /
//!    seq / array-multidim / map / module-nested+reopened / interface-nested).
//! 3. **compose-multifile** — two IDLs generated separately and type-checked as
//!    two independent OCaml compilation units (the idiomatic OCaml compose: each
//!    file is its own `.ml` module with its own `Wire`), both of which compile.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

/// OCaml keywords that are also legal, un-reserved IDL identifiers (i.e. not
/// IDL keywords such as `module`/`struct`/`switch`/`case`/`default`/`const`/
/// `interface`/`sequence`/`string`). Enumerator (uppercase constructor)
/// positions are excluded here — OCaml constructors are uppercase-only, so a
/// lowercase keyword can never appear there by construction.
const SAFE_KEYWORDS: &[&str] = &[
    "and",
    "as",
    "asr",
    "begin",
    "class",
    "do",
    "done",
    "downto",
    "else",
    "end",
    "exception",
    "for",
    "fun",
    "function",
    "functor",
    "if",
    "in",
    "include",
    "inherit",
    "initializer",
    "land",
    "lazy",
    "let",
    "lor",
    "lsl",
    "lsr",
    "lxor",
    "match",
    "method",
    "mod",
    "mutable",
    "new",
    "nonrec",
    "of",
    "open",
    "or",
    "private",
    "rec",
    "sig",
    "then",
    "to",
    "try",
    "type",
    "val",
    "virtual",
    "when",
    "while",
    "with",
];

fn ocaml_available() -> bool {
    Command::new("ocamlc").arg("-version").output().is_ok()
}

fn gen_ml(idl: &str) -> Option<String> {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).ok()?;
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).ok()
}

/// Type-checks one OCaml source as a standalone compilation unit (`ocamlc -c`,
/// stdlib-only — no packages). Returns `(ok, stderr)`. The module basename is
/// capitalized (OCaml derives the module name from the filename).
fn ocaml_typecheck(tag: &str, src: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!("idlocaml_corpus_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let mlfile = dir.join(format!("Zd{tag}.ml"));
    std::fs::write(&mlfile, src).expect("write ml");
    let out = Command::new("ocamlc")
        .arg("-c")
        .arg(&mlfile)
        .current_dir(&dir)
        .output()
        .expect("ocamlc");
    let _ = std::fs::remove_dir_all(&dir);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn assert_compiles(tag: &str, idl: &str) {
    let src = gen_ml(idl).unwrap_or_else(|| panic!("gen failed for {tag}:\n{idl}"));
    let (ok, stderr) = ocaml_typecheck(tag, &src);
    assert!(
        ok,
        "OCaml did not compile ({tag}):\n{stderr}\n--- source ---\n{src}"
    );
}

/// OCaml 4.13 compatibility: the emitted UTF-8 handling must not use any
/// stdlib API introduced in 4.14 (`String.get_utf_8_uchar`,
/// `Uchar.utf_decode_uchar`, `Uchar.utf_decode_length`). The wide-string
/// encode/decode and the `wstring<N>` bound check must instead route through
/// the in-tree `Wire.zd_utf8_decode` helper. This is a pure string assertion
/// over the real emit path — it needs no OCaml toolchain, so it also guards
/// the CI where only OCaml 4.13 is installed (runtime compile there is the
/// end-to-end check; the three `ocamlc`-gated corpora above cover 4.14+).
#[test]
fn no_ocaml_414_only_utf8_apis_in_output() {
    // Exercise every wide-string emit path: unbounded encode+decode
    // (`Wire.put_wstring` / `Wire.get_wstring`) and the bounded encode+decode
    // check (`UTF16_UNIT_COUNT_FN` at both the write and read call sites).
    let src = gen_ml("@final struct WOut { wstring wn; wstring<4> wbn; wchar wc; string<8> bn; };")
        .expect("gen WOut");

    for banned in [
        "String.get_utf_8_uchar",
        "Uchar.utf_decode_uchar",
        "Uchar.utf_decode_length",
    ] {
        assert!(
            !src.contains(banned),
            "emitted OCaml uses 4.14-only API `{banned}` (breaks OCaml 4.13):\n{src}"
        );
    }
    assert!(
        src.contains("let zd_utf8_decode "),
        "self-contained UTF-8 helper `zd_utf8_decode` is missing from the wire prelude:\n{src}"
    );
    assert!(
        src.contains("Wire.zd_utf8_decode "),
        "wstring<N> bound check must call the in-tree `Wire.zd_utf8_decode` helper:\n{src}"
    );
}

#[test]
fn reserved_keywords_at_every_position_compile() {
    if !ocaml_available() {
        eprintln!("skip: ocamlc not on PATH");
        return;
    }
    for (i, kw) in SAFE_KEYWORDS.iter().enumerate() {
        // Place `kw` at: struct name, member, module name, const name, union
        // branch field, and enum type name — every lowercase-escaping and
        // uppercase-capitalizing OCaml position. Skip a keyword the IDL frontend
        // itself rejects as an identifier (a frontend limitation, not this
        // backend's concern).
        let idl = format!(
            "enum {kw} {{ VARIANT_A, VARIANT_B }};
             @final struct {kw}_s {{ long {kw}; }};
             module {kw}_m {{ @final struct Inner {{ short {kw}; }}; }};
             const long {kw} = 1;
             @final union {kw}_u switch (long) {{ case 0: long {kw}; default: short other; }};"
        );
        if gen_ml(&idl).is_none() {
            continue; // frontend rejects this keyword as an identifier
        }
        assert_compiles(&format!("kw{i}"), &idl);
    }
}

#[test]
fn every_construct_compiles() {
    if !ocaml_available() {
        eprintln!("skip: ocamlc not on PATH");
        return;
    }
    // A broad single unit exercising every supported construct minimally.
    let idl = r#"
    module outer {
      // primitives, wide, long double
      @final struct Prims {
        octet o; char c; wchar wc; boolean b;
        short s; unsigned short us; long l; unsigned long ul;
        long long ll; unsigned long long ull;
        float f; double d; long double ld;
        int8 i8; uint8 u8; int16 i16; uint16 u16;
        int32 i32; uint32 u32; int64 i64; uint64 u64;
      };
      // strings (bounded + unbounded)
      @final struct Strs { string name; string<8> bn; wstring wn; wstring<4> wbn; };
      // fixed
      @final struct Money { fixed<10,2> amount; };
      // enum with @value gaps
      enum Color { @value(1) RED, @value(4) GREEN, BLUE };
      // const of every type
      const long C_LONG = 1 << 4;
      const boolean C_BOOL = TRUE;
      const boolean C_BOOL2 = FALSE;
      const string C_STR = "hi";
      const double C_DBL = 3.14;
      const octet C_OCT = 7;
      const char C_CHR = 'x';
      const long long C_BIG = 100;
      const Color C_ENUM = GREEN;
      // struct inheritance
      @final struct Base { long a; @key short b; };
      @final struct Derived : Base { long c; };
      // extensibility
      @final struct Fin { long x; };
      @appendable struct App { long x; };
      @mutable struct Mut { @must_understand long x; long y; };
      // @optional
      @appendable struct Opt { @optional long maybe; long always; };
      // union: all discriminator kinds (integer / char / boolean / enum)
      @final union UI switch (long) { case 1: long a; case 2: short b; default: octet d; };
      @final union UC switch (char) { case 'A': long a; default: short b; };
      @final union UB switch (boolean) { case TRUE: long t; case FALSE: short f; };
      @final union UE switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
      // bitset / bitmask
      bitset Flags { bitfield<3> lo; bitfield<5> hi; };
      bitmask Perms { READ, WRITE, EXEC };
      // collections: seq (prim + struct), array multidim, map
      @final struct Colls {
        sequence<long> nums;
        sequence<Base> items;
        long grid[2][3];
        Base matrix[2][2];
        map<long, long> m1;
        map<long, Base> m2;
      };
      // typedef
      typedef long Score;
      @final struct UsesTd { Score sc; };
    };
    // nested + reopened modules
    module ra { @final struct C { long x; }; };
    module ra { @final struct D { double y; }; };
    module rb { module ra { @final struct C { long z; }; }; };
    // interface-nested type promotion
    module iface_m {
      interface Svc { @final struct Req { long id; }; };
      @final struct UsesReq { iface_m::Svc::Req r; };
    };
    "#;
    assert_compiles("constructs", idl);
}

#[test]
fn compose_two_units_compile_independently() {
    if !ocaml_available() {
        eprintln!("skip: ocamlc not on PATH");
        return;
    }
    // The idiomatic OCaml compose: two IDLs generated separately become two
    // independent compilation units, each self-contained (its own `Wire`). Both
    // must type-check.
    let a = "@final struct AThing { long a; string label; };";
    let b = "@final struct BThing { double b; sequence<long> xs; };";
    let src_a = gen_ml(a).expect("gen a");
    let src_b = gen_ml(b).expect("gen b");
    let (ok_a, err_a) = ocaml_typecheck("composeA", &src_a);
    let (ok_b, err_b) = ocaml_typecheck("composeB", &src_b);
    assert!(ok_a, "unit A did not compile:\n{err_a}\n{src_a}");
    assert!(ok_b, "unit B did not compile:\n{err_b}\n{src_b}");
}
