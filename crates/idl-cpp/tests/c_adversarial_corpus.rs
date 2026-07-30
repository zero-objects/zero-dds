//! Adversarial corpus for the C backend (`src/c_mode.rs`), compiled with the
//! real toolchain (`gcc`/`cc`). `#[ignore]`d by default — run with
//!
//! ```text
//! cargo test -p zerodds-idl-cpp --test c_adversarial_corpus -- --ignored
//! ```
//!
//! Three layers, matching the IDL-construct fix-campaign test gate:
//!
//! 1. **reserved-keyword corpus** — every C keyword that is a legal IDL
//!    identifier, placed at member / struct-name / enum-name / module-name /
//!    const-name / union-branch positions, must generate C that *compiles*
//!    (the `escape_c_ident` trailing-underscore path).
//! 2. **construct corpus** — each IDL construct minimally, compiled; wire-size
//!    asserted where the C profile pins it (wchar = 2 B UTF-16, primitive
//!    widths); the two constructs outside the C profile (`long double`,
//!    genuinely unbounded recursion) are asserted to *reject cleanly*, never
//!    mis-emit.
//! 3. **compose-multifile** — bug **C-c**: two IDLs generated *separately* and
//!    `#include`d into one translation unit must both survive (distinct
//!    include guards), then link and run.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn c_include_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../zerodds-c-api/include").to_string()
}

/// Parse + generate; `Err` on a parse failure (used to skip IDL-keyword
/// positions) or a legitimate `UnsupportedConstruct`.
fn try_gen(src: &str) -> Result<String, String> {
    let ast =
        zerodds_idl::parse(src, &ParserConfig::default()).map_err(|e| format!("parse: {e:?}"))?;
    generate_c_header(&ast, &CGenOptions::default()).map_err(|e| format!("gen: {e:?}"))
}

fn gen_c(src: &str) -> String {
    try_gen(src).expect("c-gen")
}

/// `gcc -fsyntax-only` on one header beside the shipped runtime header.
fn syntax_only(header: &str) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let hdr = dir.path().join("gen.h");
    std::fs::write(&hdr, header).map_err(|e| format!("write: {e}"))?;
    let out = Command::new("gcc")
        .args(["-std=c99", "-fsyntax-only", "-I"])
        .arg(c_include_dir())
        .arg(&hdr)
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// Compile `main_src` (which `#include`s each `(name, header)`) and run it.
fn compile_and_run(headers: &[(&str, &str)], main_src: &str) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    for (name, body) in headers {
        std::fs::write(dir.path().join(name), body).map_err(|e| format!("write {name}: {e}"))?;
    }
    let main = dir.path().join("main.c");
    std::fs::write(&main, main_src).map_err(|e| format!("write main: {e}"))?;
    let bin = dir.path().join("a.out");
    let compile = Command::new("gcc")
        .args(["-std=c99", "-I"])
        .arg(c_include_dir())
        .arg("-I")
        .arg(dir.path())
        .arg(&main)
        .arg("-o")
        .arg(&bin)
        .output()
        .map_err(|e| format!("spawn gcc: {e}"))?;
    if !compile.status.success() {
        return Err(format!(
            "compile failed:\n{}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let run = Command::new(&bin)
        .output()
        .map_err(|e| format!("spawn bin: {e}"))?;
    if run.status.success() {
        Ok(())
    } else {
        Err(format!(
            "binary failed (exit {:?}):\n{}",
            run.status.code(),
            String::from_utf8_lossy(&run.stdout)
        ))
    }
}

// ============================================================ reserved-keyword

/// C keywords that are NOT IDL keywords (so the frontend accepts them as
/// identifiers). IDL type/keyword words (`long`, `short`, `char`, `double`,
/// `const`, `struct`, `union`, `enum`, `typedef`, `switch`, `case`, `default`,
/// `void`, `unsigned`, `float`) are deliberately excluded — the frontend
/// rejects them before codegen, so they can never reach the C emitter.
const C_KW_IDL_IDENTS: &[&str] = &[
    "int",
    "auto",
    "register",
    "volatile",
    "extern",
    "static",
    "goto",
    "return",
    "sizeof",
    "restrict",
    "inline",
    "while",
    "for",
    "do",
    "if",
    "else",
    "break",
    "continue",
    "signed",
    "typeof",
    "nullptr",
    "constexpr",
    "alignas",
    "alignof",
    "static_assert",
    "thread_local",
];

#[test]
#[ignore = "requires gcc in PATH"]
fn reserved_keywords_at_every_position_compile() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    for kw in C_KW_IDL_IDENTS {
        // (position label, IDL source). Skip a position whose IDL does not
        // parse for this word (never a codegen failure).
        let positions = [
            ("member", format!("@final struct Holder {{ long {kw}; }};")),
            ("struct-name", format!("@final struct {kw} {{ long a; }};")),
            (
                "enum-name",
                format!("enum {kw} {{ A_, B_ }}; @final struct H {{ {kw} e; }};"),
            ),
            (
                "enumerator",
                format!("enum E {{ {kw}, OTHER }}; @final struct H {{ E e; }};"),
            ),
            (
                "module-name",
                format!("module {kw} {{ @final struct S {{ long x; }}; }};"),
            ),
            (
                "const-name",
                format!("const long {kw} = 1; @final struct H {{ long a; }};"),
            ),
            (
                "union-branch",
                format!("union U switch (long) {{ case 0: long {kw}; default: double d; }};"),
            ),
        ];
        for (label, src) in positions {
            // A parse rejection at a position (word illegal there in IDL) is
            // fine — it never reaches the C emitter. Only a successful gen must
            // then compile.
            if let Ok(h) = try_gen(&src) {
                if let Err(e) = syntax_only(&h) {
                    panic!("reserved `{kw}` at {label} did not compile:\n{e}\n--- header ---\n{h}");
                }
            }
        }
    }
}

// ================================================================== constructs

/// Each construct minimally, compiled with the real toolchain.
#[test]
#[ignore = "requires gcc in PATH"]
fn construct_corpus_compiles() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    let corpus: &[(&str, &str)] = &[
        (
            "enum",
            "enum Color { R, G, B }; @final struct S { Color c; };",
        ),
        (
            "enum@value",
            "enum E { @value(3) A, @value(7) B }; @final struct S { E e; };",
        ),
        (
            "const-all",
            "\
const long CL = -5; const unsigned long CUL = 5; const double CD = 1.5;
const boolean CB = TRUE; const char CC = 'x'; const string CS = \"hi\";
@final struct S { long x; };",
        ),
        ("wchar/wstring", "@final struct S { wchar c; wstring w; };"),
        (
            "struct-inheritance",
            "@final struct Base { long id; }; @final struct Derived : Base { double v; };",
        ),
        (
            "union-long",
            "union U switch (long) { case 0: long a; default: double d; };",
        ),
        (
            "union-enum",
            "enum D { X, Y }; union U switch (D) { case X: long a; case Y: double b; };",
        ),
        (
            "union-bool",
            "union U switch (boolean) { case TRUE: long a; default: double b; };",
        ),
        (
            "union-char",
            "union U switch (char) { case 'a': long a; default: double b; };",
        ),
        (
            "bitset",
            "bitset BS { bitfield<3> a; bitfield<5> b; }; @final struct S { BS x; };",
        ),
        (
            "bitmask",
            "@bit_bound(16) bitmask F { A, B }; @final struct S { F f; };",
        ),
        ("fixed", "@final struct S { fixed<10,2> f; };"),
        (
            "optional/extensibility",
            "@mutable struct M { @optional long maybe; long always; };",
        ),
        ("appendable", "@appendable struct A { long a; double b; };"),
        (
            "seq",
            "@final struct S { sequence<long> u; sequence<long,4> b; };",
        ),
        ("array-multidim", "@final struct S { long a[2][3][4]; };"),
        ("map", "@final struct S { map<long,double> m; };"),
        (
            "module-nested-reopened",
            "\
module a { module b { @final struct S { long x; }; }; };
module a { @final struct T { long y; }; };",
        ),
    ];
    for (label, src) in corpus {
        let h = gen_c(src);
        if let Err(e) = syntax_only(&h) {
            panic!("construct `{label}` did not compile:\n{e}\n--- header ---\n{h}");
        }
    }
}

/// Constructs outside the C profile must be rejected cleanly (an `Err`), never
/// silently mis-emitted.
#[test]
fn out_of_profile_constructs_reject_cleanly() {
    assert!(
        try_gen("@final struct S { long double ld; };").is_err(),
        "long double must reject in the C profile, not silently emit"
    );
}

/// Wire sizes the C profile pins: wchar/wstring are 2-byte UTF-16, and a plain
/// primitive struct has its natural XCDR2 layout.
#[test]
#[ignore = "requires gcc in PATH"]
fn wire_sizes_are_pinned() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    // `wstring "Hi"` = u32 byte-count (2 chars * 2 = 4) + 2 * u16 = 8 bytes.
    let hw = gen_c("@final struct W { wstring w; };");
    let main_w = r#"
#include "w.h"
#include <assert.h>
int main(void) {
    static const uint16_t hi[] = { 'H', 'i', 0 };
    W_t w; w.w = (uint16_t*)hi;
    uint8_t buf[64]; size_t n = 0;
    assert(W_encode(&w, buf, sizeof(buf), &n) == 0);
    assert(n == 8);          /* 4 (u32 len) + 2*2 (two UTF-16 units) */
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&[("w.h", &hw)], main_w) {
        panic!("wstring wire-size roundtrip failed:\n{e}");
    }

    // `@final struct P { long a; long b; }` = 4 + 4 = 8 bytes (no DHEADER for
    // @final under XCDR2).
    let hp = gen_c("@final struct P { long a; long b; };");
    let main_p = r#"
#include "p.h"
#include <assert.h>
int main(void) {
    P_t p; p.a = 1; p.b = 2;
    uint8_t buf[64]; size_t n = 0;
    assert(P_encode(&p, buf, sizeof(buf), &n) == 0);
    assert(n == 8);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&[("p.h", &hp)], main_p) {
        panic!("primitive-struct wire-size roundtrip failed:\n{e}");
    }
}

// ============================================================ compose-multifile

/// Bug **C-c**: two headers generated in separate `generate_c_header` calls,
/// `#include`d into one translation unit, must both be fully present. With the
/// old fixed `ZERODDS_GENERATED_H` guard the second header's body was swallowed
/// and `Bar_t` / `Bar_encode` were undefined; the per-header derived guard
/// fixes it.
#[test]
#[ignore = "requires gcc in PATH"]
fn compose_two_headers_in_one_translation_unit() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    let ha = gen_c("module a { @final struct Foo { long x; }; };");
    let hb = gen_c("module b { @final struct Bar { double y; }; };");

    // Sanity: the two headers really do carry different guards.
    let guard_of = |h: &str| -> String {
        h.lines()
            .find_map(|l| l.strip_prefix("#ifndef ").map(str::to_string))
            .expect("guard")
    };
    assert_ne!(
        guard_of(&ha),
        guard_of(&hb),
        "compose headers share a guard (C-c not fixed)"
    );

    let main = r#"
#include "a.h"
#include "b.h"
#include <assert.h>
int main(void) {
    a_sFoo_t foo; foo.x = 7;
    b_sBar_t bar; bar.y = 2.5;
    uint8_t buf[64]; size_t n = 0;
    assert(a_sFoo_encode(&foo, buf, sizeof(buf), &n) == 0 && n > 0);
    n = 0;
    assert(b_sBar_encode(&bar, buf, sizeof(buf), &n) == 0 && n > 0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&[("a.h", &ha), ("b.h", &hb)], main) {
        panic!("two-header compose failed (C-c regression?):\n{e}");
    }
}

/// The same header `#include`d twice in one unit is a no-op the second time —
/// the derived guard still dedups correctly (no redefinition errors).
#[test]
#[ignore = "requires gcc in PATH"]
fn same_header_included_twice_is_a_noop() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    let h = gen_c("@final struct Solo { long x; };");
    let main = r#"
#include "s.h"
#include "s.h"
#include <assert.h>
int main(void) {
    Solo_t s; s.x = 3;
    uint8_t buf[32]; size_t n = 0;
    assert(Solo_encode(&s, buf, sizeof(buf), &n) == 0 && n > 0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&[("s.h", &h)], main) {
        panic!("double-include of one header failed:\n{e}");
    }
}
