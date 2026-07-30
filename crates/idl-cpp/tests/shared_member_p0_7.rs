//! Broad-audit P0-7 — `@shared` members must reach the wire BY VALUE.
//!
//! Before the fix the C++ emitter held a `@shared` member in memory as a
//! `std::shared_ptr<T>` but SKIPPED it on every wire path (`emit_plain_*`,
//! `emit_mutable_*`, `emit_pl_cdr1_*`): encode wrote nothing, decode read
//! nothing, so the referenced value was silently dropped (data loss, no error).
//!
//! `@shared` (XTypes 1.3 §7.3.1.2.1.9 / annotation `@shared`) governs ONLY the
//! in-memory representation (a shared reference). On the wire the referenced
//! value is serialized fully by value — byte-identical to the same member
//! WITHOUT `@shared`. This test proves — through the REAL emit path AND a real
//! C++ compile+run — that a `@shared` primitive, string and nested-struct member
//! now round-trips under BOTH XCDR2 and XCDR1, LE and BE, for @final /
//! @appendable / @mutable, and that its wire bytes are byte-identical to the same
//! member declared without `@shared`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

/// First available C++17 compiler (`clang++` or `g++`), or `None`.
fn cpp_compiler() -> Option<&'static str> {
    ["clang++", "g++"].into_iter().find(|cc| {
        Command::new(cc)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Path to the C++ runtime include dir (`crates/cpp/include`).
fn cpp_include_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../cpp/include").to_string()
}

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("cpp gen")
}

/// Compile `cpp` (a generated header) + `main_src` into a binary, run it, and
/// require exit 0 — a real encode/decode roundtrip, not just a syntax check.
fn compile_and_run(cc: &str, cpp: &str, main_src: &str) {
    let dir = tempfile::tempdir().expect("tempdir");
    let hdr = dir.path().join("gen.hpp");
    std::fs::write(&hdr, cpp).expect("write hdr");
    let main = dir.path().join("main.cpp");
    std::fs::write(&main, main_src).expect("write main");
    let bin = dir.path().join("a.out");
    let compile = Command::new(cc)
        .args(["-std=c++17", "-I"])
        .arg(dir.path())
        .arg("-I")
        .arg(cpp_include_dir())
        .arg(&main)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn compiler");
    assert!(
        compile.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = Command::new(&bin).output().expect("spawn bin");
    assert!(
        run.status.success(),
        "roundtrip binary failed (exit {:?}):\nstdout:\n{}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
}

/// The IDL corpus: for each extensibility a `Shared*` struct with `@shared`
/// primitive/string/nested-struct members and a `Plain*` twin with the SAME
/// members without `@shared` (identical member positions → identical member ids
/// → the wire bytes must match).
const CORPUS: &str = "\
module p0_7 {
  struct Child { long x; string label; };

  @final struct SharedFin {
    long          lead;
    @shared Child c;
    @shared long  v;
    @shared string note;
    long          tail;
  };
  @final struct PlainFin {
    long   lead;
    Child  c;
    long   v;
    string note;
    long   tail;
  };

  @appendable struct SharedApp {
    long          lead;
    @shared Child c;
    @shared long  v;
    @shared string note;
    long          tail;
  };
  @appendable struct PlainApp {
    long   lead;
    Child  c;
    long   v;
    string note;
    long   tail;
  };

  @mutable struct SharedMut {
    long          lead;
    @shared Child c;
    @shared long  v;
    @shared string note;
    long          tail;
  };
  @mutable struct PlainMut {
    long   lead;
    Child  c;
    long   v;
    string note;
    long   tail;
  };

  // @shared @optional (std::optional<std::shared_ptr<T>>) and its plain
  // @optional twin — the wire bytes must match on both the present and the
  // absent path.
  @final struct SharedOpt {
    long                    lead;
    @shared @optional Child c;
    @shared @optional long  v;
    long                    tail;
  };
  @final struct PlainOpt {
    long            lead;
    @optional Child c;
    @optional long  v;
    long            tail;
  };
  @mutable struct SharedOptMut {
    long                    lead;
    @shared @optional Child c;
    @shared @optional long  v;
    long                    tail;
  };
  @mutable struct PlainOptMut {
    long            lead;
    @optional Child c;
    @optional long  v;
    long            tail;
  };
};";

/// Source-level guard (always runs, no compiler needed): the `@shared` members
/// must NOT be emitted as a "not supported (skip)" comment, the encode path must
/// deref the shared_ptr (`zd_shref`), and the decode path must wrap the decoded
/// value back into a fresh `shared_ptr` (`make_shared`).
#[test]
fn shared_member_is_wired_not_skipped() {
    let cpp = gen_cpp(CORPUS);
    assert!(
        !cpp.contains("@shared member"),
        "a @shared member is still emitted as a silent skip:\n{cpp}"
    );
    assert!(
        !cpp.contains("not supported (skip)"),
        "a member is still silently skipped:\n{cpp}"
    );
    // Encode side: the pointee is dereferenced and serialized by value.
    assert!(
        cpp.contains("zd_shref"),
        "@shared encode path does not deref the shared_ptr (member dropped)"
    );
    // Decode side: the decoded value is wrapped back into a shared_ptr.
    assert!(
        cpp.contains("std::make_shared<"),
        "@shared decode path does not reconstruct the shared_ptr"
    );
}

/// A `@shared` ARRAY member is the one shape this backend does not serialize:
/// it must fail LOUDLY (a hard codegen error), never a silent skip that would
/// drop the value from the wire.
#[test]
fn shared_array_member_hard_error() {
    let arr = zerodds_idl::parse(
        "@final struct S { @shared long v[2]; };",
        &ParserConfig::default(),
    )
    .expect("parse");
    assert!(
        generate_cpp_header(&arr, &CppGenOptions::default()).is_err(),
        "@shared array member must be a hard error, not a silent skip"
    );
}

/// Full encode→decode roundtrip of `@shared` primitive/string/nested-struct
/// members under XCDR2 AND XCDR1, LE AND BE, for @final / @appendable /
/// @mutable — plus the byte-identity proof (`@shared` wire == non-`@shared`
/// wire) on every one of those paths.
#[test]
fn shared_members_roundtrip_and_byte_identical() {
    let Some(cc) = cpp_compiler() else {
        println!("no C++ compiler available, skipping compile+run");
        return;
    };
    let cpp = gen_cpp(CORPUS);
    let main = r#"
#include "gen.hpp"
#include <cassert>
using namespace dds::topic;

template <class S>
static void verify(const S& back) {
    assert(back.lead() == 7);
    assert(back.c());                        // shared_ptr<Child> non-null
    assert(back.c()->x() == 42);             // nested @shared value survived
    assert(back.c()->label() == "hi");
    assert(back.v());                        // shared_ptr<long> non-null
    assert(*back.v() == 99);                 // primitive @shared value survived
    assert(back.note());                     // shared_ptr<string> non-null
    assert(*back.note() == "deep");          // string @shared value survived
    assert(back.tail() == 123);              // member after the @shared ones intact
}

template <class S, class P>
static void check(xcdr2::XcdrVersion repr) {
    p0_7::Child child; child.x(42); child.label("hi");
    S s; s.lead(7); s.c(child); s.v(99); s.note(std::string("deep")); s.tail(123);
    P p; p.lead(7); p.c(child); p.v(99); p.note(std::string("deep")); p.tail(123);

    // --- LE (encode/decode) ---
    {
        auto sb = topic_type_support<S>::encode(s, repr);
        auto pb = topic_type_support<P>::encode(p, repr);
        assert(sb == pb);   // @shared wire bytes IDENTICAL to non-@shared (LE)
        auto back = topic_type_support<S>::decode(sb.data(), sb.size(), repr);
        verify(back);
    }
    // --- BE (encode_be/decode) ---
    {
        auto sb = topic_type_support<S>::encode_be(s, repr);
        auto pb = topic_type_support<P>::encode_be(p, repr);
        assert(sb == pb);   // @shared wire bytes IDENTICAL to non-@shared (BE)
        auto back = topic_type_support<S>::decode(sb.data(), sb.size(), repr, true);
        verify(back);
    }
}

// @shared @optional: present path AND absent path, byte-identical to the plain
// @optional twin, on both XCDR2 and XCDR1.
template <class S, class P>
static void check_opt(xcdr2::XcdrVersion repr) {
    p0_7::Child child; child.x(42); child.label("hi");

    // present
    {
        S s; s.lead(7); s.c(child); s.v(99); s.tail(123);
        P p; p.lead(7); p.c(child); p.v(99); p.tail(123);
        auto sb = topic_type_support<S>::encode(s, repr);
        auto pb = topic_type_support<P>::encode(p, repr);
        assert(sb == pb);   // @shared @optional wire == plain @optional (present)
        auto back = topic_type_support<S>::decode(sb.data(), sb.size(), repr);
        assert(back.lead() == 7 && back.tail() == 123);
        assert(back.c().has_value() && (*back.c()));      // optional engaged, ptr set
        assert((*back.c())->x() == 42 && (*back.c())->label() == "hi");
        assert(back.v().has_value() && (*back.v()));
        assert(*(*back.v()) == 99);
        // BE
        auto sbe = topic_type_support<S>::encode_be(s, repr);
        auto pbe = topic_type_support<P>::encode_be(p, repr);
        assert(sbe == pbe);
    }
    // absent (both optionals unset)
    {
        S s; s.lead(7); s.tail(123);
        P p; p.lead(7); p.tail(123);
        auto sb = topic_type_support<S>::encode(s, repr);
        auto pb = topic_type_support<P>::encode(p, repr);
        assert(sb == pb);   // absent path also byte-identical
        auto back = topic_type_support<S>::decode(sb.data(), sb.size(), repr);
        assert(!back.c().has_value() && !back.v().has_value());
        assert(back.lead() == 7 && back.tail() == 123);
    }
}

int main() {
    // @final: plain-CDR (no DHEADER)
    check<p0_7::SharedFin, p0_7::PlainFin>(xcdr2::XcdrVersion::Xcdr2);
    check<p0_7::SharedFin, p0_7::PlainFin>(xcdr2::XcdrVersion::Xcdr1);
    // @appendable: XCDR2 DHEADER / XCDR1 plain
    check<p0_7::SharedApp, p0_7::PlainApp>(xcdr2::XcdrVersion::Xcdr2);
    check<p0_7::SharedApp, p0_7::PlainApp>(xcdr2::XcdrVersion::Xcdr1);
    // @mutable: XCDR2 PL_CDR2 (EMHEADER) / XCDR1 PL_CDR1 (PID)
    check<p0_7::SharedMut, p0_7::PlainMut>(xcdr2::XcdrVersion::Xcdr2);
    check<p0_7::SharedMut, p0_7::PlainMut>(xcdr2::XcdrVersion::Xcdr1);
    // @shared @optional (present + absent), @final and @mutable
    check_opt<p0_7::SharedOpt, p0_7::PlainOpt>(xcdr2::XcdrVersion::Xcdr2);
    check_opt<p0_7::SharedOpt, p0_7::PlainOpt>(xcdr2::XcdrVersion::Xcdr1);
    check_opt<p0_7::SharedOptMut, p0_7::PlainOptMut>(xcdr2::XcdrVersion::Xcdr2);
    check_opt<p0_7::SharedOptMut, p0_7::PlainOptMut>(xcdr2::XcdrVersion::Xcdr1);
    return 0;
}
"#;
    compile_and_run(cc, &cpp, main);
}
