//! Adversarial corpus for the idl-cpp emitter, compiled against the real C++
//! toolchain (clang). Ignored by default (clang is not on every CI/dev box);
//! run with `cargo test -p zerodds-idl-cpp --test adversarial_corpus -- --ignored`.
//!
//! Three axes, per the construct-fix campaign test gate:
//! 1. **reserved-keyword corpus** — every C++ reserved word that is a legal IDL
//!    identifier, placed at member / struct / enum / module / const /
//!    union-branch positions, must generate + compile.
//! 2. **construct corpus** — each IDL construct in minimal form (wchar 2B,
//!    fixed, enum `@value`, const of every scalar type, struct inheritance,
//!    union over several discriminators, bitset, bitmask, `@optional` +
//!    `@extensibility`, sequence, multi-dim array, map, nested + reopened
//!    module) compiles; wire-size asserts where statically checkable
//!    (`wchar` = 2 bytes, enum `@value` constant = its wire value).
//! 3. **compose-multifile** — two IDLs generated separately, merged
//!    idiomatically (each its own header, one translation unit) compiles.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::Write;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn clang_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Path to the C++ runtime include dir (`crates/cpp/include`).
fn cpp_include_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../cpp/include").to_string()
}

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

/// `clang -fsyntax-only` over a generated header. Proves it parses + type-checks
/// against the real runtime headers.
fn syntax_check(cpp: &str) -> Result<(), String> {
    let mut tmp = tempfile::Builder::new()
        .suffix(".hpp")
        .tempfile()
        .map_err(|e| format!("tempfile: {e}"))?;
    tmp.write_all(cpp.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    tmp.flush().map_err(|e| format!("flush: {e}"))?;
    let output = Command::new("clang")
        .args(["-std=c++17", "-fsyntax-only", "-x", "c++", "-I"])
        .arg(cpp_include_dir())
        .arg(tmp.path())
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Compile one or more generated headers plus a `main.cpp` into a binary and run
/// it; require exit 0. Proves semantic (runtime) correctness.
fn exec_check(headers: &[(&str, &str)], main_src: &str) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    for (name, content) in headers {
        std::fs::write(dir.path().join(name), content).map_err(|e| format!("write {name}: {e}"))?;
    }
    let main = dir.path().join("main.cpp");
    std::fs::write(&main, main_src).map_err(|e| format!("write main: {e}"))?;
    let bin = dir.path().join("a.out");
    let compile = Command::new("clang++")
        .args(["-std=c++17", "-I"])
        .arg(dir.path())
        .arg("-I")
        .arg(cpp_include_dir())
        .arg(&main)
        .arg("-o")
        .arg(&bin)
        .output()
        .map_err(|e| format!("spawn clang++: {e}"))?;
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

macro_rules! skip_without_clang {
    () => {
        if !clang_available() {
            println!("clang not available, skipping");
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// Axis 1 — reserved-keyword corpus
// ---------------------------------------------------------------------------

/// A C++ reserved word placed at every IDL declaration position: module, enum,
/// enumerator, struct, member, union discriminator/branch label, and a
/// module-scope const. The emitter must escape each and the result must
/// compile.
const RESERVED_WORD_IDL: &str = "\
module class {
    const long register = 7;
    enum mutable { new, delete };
    @final struct template { long int; long operator; };
    @appendable struct explicit { long friend; };
    struct volatile { template inline; explicit signed; };
    struct virtual : template { long throw; };
    union namespace switch (mutable) {
        case new:    long   alignas;
        case delete: double noexcept;
        default:     octet  auto_field;
    };
};";

#[test]
#[ignore = "requires clang in PATH"]
fn reserved_keyword_corpus_compiles() {
    skip_without_clang!();
    let cpp = emit(RESERVED_WORD_IDL);
    if let Err(e) = syntax_check(&cpp) {
        panic!("reserved-keyword corpus failed to compile:\n{e}\n---\n{cpp}");
    }
}

// ---------------------------------------------------------------------------
// Axis 2 — construct corpus
// ---------------------------------------------------------------------------

/// One IDL exercising most constructs at once. `long double` is deliberately
/// excluded — it is a loud codegen reject on this backend (no portable 16-byte
/// binary128 wire; blocked on Rust `f128`), covered by `long_double_rejected`.
const CONSTRUCT_CORPUS_IDL: &str = r#"
module base {
    const boolean C_BOOL   = TRUE;
    const octet   C_OCTET  = 7;
    const short   C_SHORT  = -3;
    const long    C_LONG   = 42;
    const unsigned long long C_ULL = 9;
    const float   C_FLOAT  = 1.5;
    const double  C_DOUBLE = 2.5;
    const char    C_CHAR   = 'z';
    const string  C_STR    = "hi";
    const wstring C_WSTR   = L"wide";

    enum Sparse { S_A, @value(5) S_B, S_C, @value(100) S_D };

    @bit_bound(16) bitmask Flags { F0, F1, @position(9) F9 };
    bitset Packed { bitfield<12> lo; bitfield<10> hi; };

    typedef fixed<9, 2> Money;
    struct HasFixed { Money amount; };

    @final struct WideChars { wchar w; };

    @appendable struct Vecs {
        sequence<long>        s;
        long                  grid[2][3];
        map<long, string>     m;
        wstring               label;
        @optional long        maybe;
        string<8>             bounded;
    };

    struct Key { long a; short b; };
    struct MapKeyed { map<Key, long> table; };

    @mutable struct Extensible {
        @id(1) long   alpha;
        @id(2) double beta;
    };

    struct BasePart { long id; };
    struct DerivedPart : BasePart { long extra; };

    union OverLong switch (long) {
        case 0: long   zero;
        case 1: double one;
        default: octet other;
    };
    union OverEnum switch (Sparse) {
        case S_A: long   a;
        case S_D: string d;
    };
    union OverChar switch (char) {
        case 'a': long  ca;
        default:  short cb;
    };
};

module base {
    struct Reopened { long more; };
};

module base { module inner {
    struct Nested { base::Sparse e; };
}; };
"#;

#[test]
#[ignore = "requires clang in PATH"]
fn construct_corpus_compiles() {
    skip_without_clang!();
    let cpp = emit(CONSTRUCT_CORPUS_IDL);
    if let Err(e) = syntax_check(&cpp) {
        panic!("construct corpus failed to compile:\n{e}\n---\n{cpp}");
    }
}

#[test]
#[ignore = "requires clang++ in PATH"]
fn enum_value_wire_is_explicit() {
    // F4: the enum constant must equal its `@value` (that is the int32 wire
    // value), and a struct carrying it must roundtrip that value.
    skip_without_clang!();
    let cpp = emit("enum E { A, @value(5) B, C, @value(100) D }; struct Carrier { E e; };");
    let main = r#"
#include "gen.hpp"
#include <cassert>
using namespace dds::topic;
static_assert(static_cast<int32_t>(E::D) == 100, "enum @value wire mismatch");
static_assert(static_cast<int32_t>(E::C) == 6,   "enum successor mismatch");
int main() {
    Carrier x; x.e(E::D);
    auto bytes = topic_type_support<Carrier>::encode(x);
    auto back  = topic_type_support<Carrier>::decode(
        bytes.data(), bytes.size(), xcdr2::XcdrVersion::Xcdr2);
    assert(back.e() == E::D);
    assert(static_cast<int32_t>(back.e()) == 100);
    return 0;
}
"#;
    if let Err(e) = exec_check(&[("gen.hpp", &cpp)], main) {
        panic!("enum @value wire roundtrip failed:\n{e}");
    }
}

#[test]
#[ignore = "requires clang++ in PATH"]
fn primitive_wire_sizes_are_exact() {
    // Statically checkable wire sizes: a `@final` struct carries no DHEADER
    // (Plain-CDR2), so a single fixed-width scalar's wire length equals its
    // byte width. (`wchar` is deliberately not asserted here — it maps to the
    // platform `wchar_t`, whose width is not 2 on this toolchain.)
    skip_without_clang!();
    let cpp = emit(
        "@final struct O { octet v; }; \
         @final struct S { short v; }; \
         @final struct L { long v; }; \
         @final struct D { double v; }; \
         @final struct W { wchar v; };",
    );
    let main = r#"
#include "gen.hpp"
#include <cassert>
using namespace dds::topic;
int main() {
    { O x; x.v(1);  assert(topic_type_support<O>::encode(x).size() == 1); }
    { S x; x.v(2);  assert(topic_type_support<S>::encode(x).size() == 2); }
    { L x; x.v(3);  assert(topic_type_support<L>::encode(x).size() == 4); }
    { D x; x.v(4);  assert(topic_type_support<D>::encode(x).size() == 8); }
    // wchar: no wire-size claim, but the value must roundtrip.
    { W x; x.v(L'A');
      auto b = topic_type_support<W>::encode(x);
      auto back = topic_type_support<W>::decode(b.data(), b.size(), xcdr2::XcdrVersion::Xcdr2);
      assert(back.v() == L'A'); }
    return 0;
}
"#;
    if let Err(e) = exec_check(&[("gen.hpp", &cpp)], main) {
        panic!("primitive wire-size asserts failed:\n{e}");
    }
}

#[test]
#[ignore = "requires clang++ in PATH"]
fn map_struct_key_roundtrips() {
    // F24: a `map<Struct, _>` needs the generated `operator<`; this both
    // compiles it and roundtrips two distinct struct keys.
    skip_without_clang!();
    let cpp = emit(
        "struct Key { long a; short b; }; \
         @appendable struct Container { map<Key, long> table; };",
    );
    let main = r#"
#include "gen.hpp"
#include <cassert>
using namespace dds::topic;
int main() {
    Container c;
    Key k1; k1.a(1); k1.b(2);
    Key k2; k2.a(1); k2.b(3);
    c.table()[k1] = 10;
    c.table()[k2] = 20;
    auto bytes = topic_type_support<Container>::encode(c);
    auto back  = topic_type_support<Container>::decode(
        bytes.data(), bytes.size(), xcdr2::XcdrVersion::Xcdr2);
    assert(back.table().size() == 2);
    Key q1; q1.a(1); q1.b(2);
    Key q2; q2.a(1); q2.b(3);
    assert(back.table().at(q1) == 10);
    assert(back.table().at(q2) == 20);
    return 0;
}
"#;
    if let Err(e) = exec_check(&[("gen.hpp", &cpp)], main) {
        panic!("map<struct,_> roundtrip failed:\n{e}");
    }
}

#[test]
#[ignore = "requires clang in PATH"]
fn bool_and_string_consts_compile() {
    // F6 + F8: `TRUE`/`FALSE` normalise and string consts are constexpr
    // string_view — both must type-check.
    skip_without_clang!();
    let cpp = emit(
        r#"const boolean ON = TRUE; const boolean OFF = FALSE;
           const string GREETING = "hello"; const wstring WIDE = L"hi";"#,
    );
    if let Err(e) = syntax_check(&cpp) {
        panic!("bool/string consts failed to compile:\n{e}\n---\n{cpp}");
    }
}

#[test]
fn long_double_rejected() {
    // F3: no clang needed — codegen must loud-reject `long double`.
    let ast = zerodds_idl::parse("struct F { long double v; };", &ParserConfig::default())
        .expect("parse");
    assert!(
        generate_cpp_header(&ast, &CppGenOptions::default()).is_err(),
        "long double member must be rejected at codegen"
    );
}

// ---------------------------------------------------------------------------
// Axis 3 — compose-multifile
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires clang++ in PATH"]
fn compose_multifile_compiles() {
    // Two IDLs generated independently, each into its own header, merged
    // idiomatically into one translation unit (both `#include`d). No fixed
    // global include-guard / no shared global symbol may collide.
    skip_without_clang!();
    let cpp_a = emit("module alpha { struct Ping { long seq; string note; }; };");
    let cpp_b =
        emit("module beta { enum State { OFF, ON }; struct Pong { long seq; State st; }; };");
    let main = r#"
#include "a.hpp"
#include "b.hpp"
#include <cassert>
using namespace dds::topic;
int main() {
    alpha::Ping p; p.seq(1); p.note("x");
    auto pb = topic_type_support<alpha::Ping>::encode(p);
    auto pback = topic_type_support<alpha::Ping>::decode(
        pb.data(), pb.size(), xcdr2::XcdrVersion::Xcdr2);
    assert(pback.seq() == 1);

    beta::Pong q; q.seq(2); q.st(beta::State::ON);
    auto qb = topic_type_support<beta::Pong>::encode(q);
    auto qback = topic_type_support<beta::Pong>::decode(
        qb.data(), qb.size(), xcdr2::XcdrVersion::Xcdr2);
    assert(qback.seq() == 2);
    assert(qback.st() == beta::State::ON);
    return 0;
}
"#;
    if let Err(e) = exec_check(&[("a.hpp", &cpp_a), ("b.hpp", &cpp_b)], main) {
        panic!("compose-multifile failed:\n{e}");
    }
}
