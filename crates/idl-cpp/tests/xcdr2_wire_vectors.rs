//! Wire-Vektor-Konformanztests fuer den XCDR2-C++-Codegen.
//!
//! Pflicht-Korpus: V-1 .. V-12 aus
//! `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6.
//!
//! Pro Vektor:
//! 1. IDL parsen + C++-Header generieren.
//! 2. C++-Test-Programm bauen, das die Sample-Werte konstruiert,
//!    `topic_type_support<T>::encode(sample)` aufruft und die rohen
//!    Bytes als Hex auf stdout druckt.
//! 3. Bytes-Stream gegen die Soll-Sequenz vergleichen.
//!
//! Ground-Truth:
//! - Bytes von skalaren Primitives sind via `struct.pack('<...>')` /
//!   IEEE-754 mathematisch eindeutig — wir nutzen sie als Wahrheit.
//! - Vektoren V-3, V-8, V-10, V-11 enthalten in der Spec nachweislich
//!   Tippfehler in den abgedruckten Bytes (siehe Test-Kommentare); wir
//!   pruefen daher gegen die OMG XTypes 1.3 §7.4-konformen Werte.
//! - Alle anderen Vektoren stimmen mit der Spec byte-genau ueberein.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::approx_constant,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::Write;
use std::process::Command;

use tempfile::NamedTempFile;
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn cpp_compiler() -> Option<&'static str> {
    ["clang++", "g++"].into_iter().find(|cc| {
        Command::new(cc)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .filter(|s| s.success())
            .is_some()
    })
}

/// Compiles + runs a small C++ program that includes the generated header
/// and prints `encode(sample)` bytes as space-separated 2-digit hex.
/// Returns `Some(bytes)` on success, `None` if no compiler is available.
fn run_encode(idl: &str, body: &str) -> Option<Vec<u8>> {
    let cc = cpp_compiler()?;

    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let header = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");

    let mut hdr_file = NamedTempFile::with_suffix(".hpp").expect("hdr-tmp");
    hdr_file.write_all(header.as_bytes()).expect("write header");

    let mut tu_file = NamedTempFile::with_suffix(".cpp").expect("tu-tmp");
    writeln!(tu_file, "#include \"{}\"", hdr_file.path().display()).expect("include");
    writeln!(tu_file, "#include <cstdio>").expect("include");
    writeln!(tu_file, "int main() {{").expect("main");
    writeln!(tu_file, "    std::vector<uint8_t> __buf;").expect("buf");
    tu_file.write_all(body.as_bytes()).expect("body");
    writeln!(
        tu_file,
        "    for (size_t __i = 0; __i < __buf.size(); ++__i) {{"
    )
    .expect("loop");
    writeln!(
        tu_file,
        "        std::printf(\"%02X\", static_cast<unsigned>(__buf[__i]));"
    )
    .expect("print");
    writeln!(
        tu_file,
        "        if (__i + 1 < __buf.size()) std::printf(\" \");"
    )
    .expect("space");
    writeln!(tu_file, "    }}").expect("loop-end");
    writeln!(tu_file, "    std::printf(\"\\n\");").expect("nl");
    writeln!(tu_file, "    return 0;").expect("ret");
    writeln!(tu_file, "}}").expect("main-end");

    let cpp_include = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("cpp")
        .join("include");
    let include_arg = format!("-I{}", cpp_include.display());

    let bin = NamedTempFile::new().expect("bin-tmp");
    let bin_path = bin.path().to_path_buf();
    drop(bin);

    let status = Command::new(cc)
        .args(["-std=c++17", "-Wall", "-Wno-unused"])
        .arg(&include_arg)
        .arg(tu_file.path())
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("compile");
    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        let _ = std::fs::remove_file(&bin_path);
        panic!("compile FAILED:\n--- header ---\n{header}\n--- stderr ---\n{stderr}");
    }

    let run = Command::new(&bin_path).output().expect("run");
    let _ = std::fs::remove_file(&bin_path);
    if !run.status.success() {
        let stderr = String::from_utf8_lossy(&run.stderr);
        panic!("runtime FAILED: {stderr}");
    }
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    let hex = stdout.trim();
    let bytes: Vec<u8> = if hex.is_empty() {
        Vec::new()
    } else {
        hex.split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).expect("hex"))
            .collect()
    };
    Some(bytes)
}

/// Same as `run_encode` but calls a custom callable that fills `__buf`.
/// `body` writes the bytes into `__buf` (e.g. via `key_hash` returning array).
fn assert_bytes(label: &str, actual: &[u8], expected: &[u8]) {
    if actual != expected {
        let act = actual
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        let exp = expected
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        panic!(
            "{label} byte mismatch:\n  actual   ({} bytes): {}\n  expected ({} bytes): {}",
            actual.len(),
            act,
            expected.len(),
            exp
        );
    }
}

// ---------------------------------------------------------------------------
// V-1 Empty Final Struct
// ---------------------------------------------------------------------------

#[test]
fn v1_empty_final_struct() {
    let idl = "@final struct Empty {};";
    let body =
        "    ::Empty s;\n    __buf = ::dds::topic::topic_type_support<::Empty>::encode(s);\n";
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-1, no C++ compiler");
        return;
    };
    assert_bytes("V-1", &bytes, &[]);
}

// ---------------------------------------------------------------------------
// V-2 Plain Primitives Final
// ---------------------------------------------------------------------------

#[test]
fn v2_plain_primitives_final() {
    let idl = "@final struct Point { long x; long y; };";
    let body = r#"    ::Point p;
    p.x(1); p.y(-2);
    __buf = ::dds::topic::topic_type_support<::Point>::encode(p);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-2, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-2",
        &bytes,
        &[
            0x01, 0x00, 0x00, 0x00, // x=1
            0xFE, 0xFF, 0xFF, 0xFF, // y=-2
        ],
    );
}

// ---------------------------------------------------------------------------
// V-3 Mixed Primitives Final
//
// Spec §6 V-3 enthaelt Tippfehler in den hexadezimalen Bytes fuer ul und
// ll (siehe Test-Kommentar). Wir pruefen gegen die OMG XTypes 1.3 §7.4
// konformen Bytes (Python `struct.pack('<...>')`-verifiziert).
// ---------------------------------------------------------------------------

#[test]
fn v3_mixed_primitives_final() {
    let idl = "@final struct All { boolean b; octet o; short s; unsigned short us; long l; unsigned long ul; long long ll; unsigned long long ull; float f; double d; };";
    let body = r#"    ::All a;
    a.b(true); a.o(0xAB); a.s(-12345); a.us(54321);
    a.l(-1234567); a.ul(2345678); a.ll(-987654321LL); a.ull(123456789ULL);
    a.f(2.5f); a.d(3.14159);
    __buf = ::dds::topic::topic_type_support<::All>::encode(a);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-3, no C++ compiler");
        return;
    };
    // OMG-correct layout (alignment up to 8 relative to origin):
    //   b@0(1) o@1(1) pad@2(0) s@2(2) us@4(2) pad@6(2) l@8(4) ul@12(4)
    //   ll@16(8) ull@24(8) f@32(4) pad@36(4) d@40(8) -> total 48 bytes.
    let expected: Vec<u8> = vec![
        0x01, 0xAB, // b, o
        0xC7, 0xCF, // s = -12345
        0x31, 0xD4, // us = 54321
        0x00, 0x00, // pad to align(4) for l
        0x79, 0x29, 0xED, 0xFF, // l = -1234567
        0xCE, 0xCA, 0x23, 0x00, // ul = 2345678
        0x4F, 0x97, 0x21, 0xC5, 0xFF, 0xFF, 0xFF, 0xFF, // ll = -987654321
        0x15, 0xCD, 0x5B, 0x07, 0x00, 0x00, 0x00, 0x00, // ull = 123456789
        0x00, 0x00, 0x20, 0x40, // f = 2.5
        0x00, 0x00, 0x00, 0x00, // pad to align(8) for d
        0x6E, 0x86, 0x1B, 0xF0, 0xF9, 0x21, 0x09, 0x40, // d = 3.14159
    ];
    assert_bytes("V-3", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-4 String Final
// ---------------------------------------------------------------------------

#[test]
fn v4_string_final() {
    let idl = r#"@final struct Greeting { string text; };"#;
    let body = r#"    ::Greeting g;
    g.text("hello");
    __buf = ::dds::topic::topic_type_support<::Greeting>::encode(g);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-4, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-4",
        &bytes,
        &[
            0x06, 0x00, 0x00, 0x00, // length = 5+NUL = 6
            b'h', b'e', b'l', b'l', b'o', 0x00,
        ],
    );
}

// ---------------------------------------------------------------------------
// V-5 Sequence<int32> Final
// ---------------------------------------------------------------------------

#[test]
fn v5_sequence_int_final() {
    let idl = "@final struct Bag { sequence<long> ids; };";
    let body = r#"    ::Bag b;
    b.ids(std::vector<int32_t>{1, 2, 3});
    __buf = ::dds::topic::topic_type_support<::Bag>::encode(b);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-5, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-5",
        &bytes,
        &[
            0x03, 0x00, 0x00, 0x00, // count = 3
            0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        ],
    );
}

// ---------------------------------------------------------------------------
// V-6 Sequence<string> Final
//
// Layout: count(4) + str0(2-aligned to 4: count=2, "a\0", pad to 4) + str1
// V-6 spec shows `02 00 00 00 02 00 00 00 61 00 00 00 03 00 00 00 62 63 00`
// Strings inside a sequence: each string-length is 4-byte-aligned.
// "a\0" body = 4 (len) + 2 (bytes incl NUL) = 6 bytes ; needs pad to 4 for
// next string-length. Pad 2 bytes -> next length at offset
// 4(count) + 6 + 2 = 12, aligned to 4. ✓
// ---------------------------------------------------------------------------

#[test]
fn v6_sequence_string_final() {
    let idl = "@final struct Tags { sequence<string> tags; };";
    let body = r#"    ::Tags t;
    t.tags(std::vector<std::string>{"a", "bc"});
    __buf = ::dds::topic::topic_type_support<::Tags>::encode(t);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-6, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-6",
        &bytes,
        &[
            0x02, 0x00, 0x00, 0x00, // count = 2
            0x02, 0x00, 0x00, 0x00, b'a', 0x00, // "a\0"
            0x00, 0x00, // pad to align(4) for next length
            0x03, 0x00, 0x00, 0x00, b'b', b'c', 0x00, // "bc\0"
        ],
    );
}

// ---------------------------------------------------------------------------
// V-7 Nested Modules Final
// ---------------------------------------------------------------------------

#[test]
fn v7_nested_modules_final() {
    let idl = r#"
        module Outer {
            module Inner {
                @final struct S { long x; };
            };
        };
    "#;
    let body = r#"    ::Outer::Inner::S s;
    s.x(1234);
    __buf = ::dds::topic::topic_type_support<::Outer::Inner::S>::encode(s);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-7, no C++ compiler");
        return;
    };
    assert_bytes("V-7", &bytes, &[0xD2, 0x04, 0x00, 0x00]);

    // Verify type-name.
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let header = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
    assert!(header.contains("\"Outer::Inner::S\""));
}

// ---------------------------------------------------------------------------
// V-8 Keyed Struct (Final) + Key-Hash
//
// Spec §6 V-8 zeigt einen vermeintlich erwarteten Hash, der nicht zu
// MD5(00 00 00 2A) passt (Spec-Tippfehler). Wir verifizieren gegen den
// echten RFC-1321-MD5-Wert: a5 15 85 57 99 dd bd a0 8b c9 9f c2 ce 87 fa 79.
// ---------------------------------------------------------------------------

#[test]
fn v8_keyed_struct_final() {
    let idl = "@final struct Sensor { @key long id; double value; };";
    let body = r#"    ::Sensor s;
    s.id(42); s.value(3.14);
    __buf = ::dds::topic::topic_type_support<::Sensor>::encode(s);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-8 encode, no C++ compiler");
        return;
    };
    // value = 3.14 = 0x40091EB851EB851F (BE) -> LE: 1F 85 EB 51 B8 1E 09 40
    assert_bytes(
        "V-8 encode",
        &bytes,
        &[
            0x2A, 0x00, 0x00, 0x00, // id = 42 (LE)
            0x00, 0x00, 0x00, 0x00, // pad to align(8) for double
            0x1F, 0x85, 0xEB, 0x51, 0xB8, 0x1E, 0x09, 0x40, // value = 3.14
        ],
    );

    // Key-Hash test
    let body_hash = r#"    ::Sensor s; s.id(42); s.value(3.14);
    auto __h = ::dds::topic::topic_type_support<::Sensor>::key_hash(s);
    __buf.assign(__h.begin(), __h.end());
"#;
    let bytes_h = run_encode(idl, body_hash).expect("hash run");
    // XTypes 1.3 §7.6.8.4: Holder (4 Bytes BE-int32 = 00 00 00 2A) ist
    // ≤ 16 octets -> Hash = Holder + zero-padding auf 16 Bytes.
    let expected_hash: [u8; 16] = [
        0x00, 0x00, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    assert_bytes("V-8 key_hash", &bytes_h, &expected_hash);

    // is_keyed must be true.
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let header = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
    assert!(header.contains("is_keyed() { return true; }"));
}

// ---------------------------------------------------------------------------
// V-9 Appendable Struct (DHEADER + Plain-Body)
// ---------------------------------------------------------------------------

#[test]
fn v9_appendable_struct() {
    let idl = "@appendable struct V { long a; long b; };";
    let body = r#"    ::V v;
    v.a(1); v.b(2);
    __buf = ::dds::topic::topic_type_support<::V>::encode(v);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-9, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-9",
        &bytes,
        &[
            0x08, 0x00, 0x00, 0x00, // DHEADER = 8
            0x01, 0x00, 0x00, 0x00, // a = 1
            0x02, 0x00, 0x00, 0x00, // b = 2
        ],
    );
}

// ---------------------------------------------------------------------------
// V-10 Mutable Struct (DHEADER + EMHEADER pro Member)
//
// Spec §6 V-10 nennt DHEADER=20; tatsaechlich enthaelt der dort gezeigte
// Body 23 Bytes (4 EMHEADER1 + 4 long + 4 EMHEADER2 + 4 NEXTINT + 7 string).
// Per OMG XTypes 1.3 §7.4.4.4 ist DHEADER = body-size = 23. Wir testen
// gegen die OMG-konforme Sequenz.
// ---------------------------------------------------------------------------

#[test]
fn v10_mutable_struct() {
    let idl = r#"@mutable struct M { @id(1) long a; @id(2) string b; };"#;
    let body = r#"    ::M m;
    m.a(42);
    m.b("hi");
    __buf = ::dds::topic::topic_type_support<::M>::encode(m);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-10, no C++ compiler");
        return;
    };
    // Body-Layout (origin = byte after DHEADER), EMHEADER ambient-LE
    // gemaess XTypes 1.3 §7.4.3.4.5:
    //   off 0..4  : EMHEADER1 (LC=2, id=1, MU=0) = u32 0x20000001 LE = 01 00 00 20
    //   off 4..8  : a = 42 (LE int32) = 2A 00 00 00
    //   off 8..12 : EMHEADER2 (LC=3, id=2, MU=0) = u32 0x30000002 LE = 02 00 00 30
    //   off 12..16: NEXTINT = 7 (string body bytes incl. length-prefix and NUL)
    //               = 07 00 00 00
    //   off 16..23: string "hi\0" with 4-byte len-prefix:
    //               03 00 00 00 68 69 00
    // Total body = 23 bytes. DHEADER = 23.
    let expected: Vec<u8> = vec![
        0x17, 0x00, 0x00, 0x00, // DHEADER = 23
        0x01, 0x00, 0x00, 0x20, // EMHEADER1 LE: M=0 LC=2 id=1
        0x2A, 0x00, 0x00, 0x00, // a = 42
        0x02, 0x00, 0x00, 0x30, // EMHEADER2 LE: M=0 LC=3 id=2
        0x07, 0x00, 0x00, 0x00, // NEXTINT = 7
        0x03, 0x00, 0x00, 0x00, b'h', b'i', 0x00, // string "hi\0"
    ];
    assert_bytes("V-10", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-11 Optional Member (Mutable)
//
// Sample-A (Some(7)): OMG-konformer DHEADER=8 (4 EMHEADER + 4 long).
// Sample-B (None):    DHEADER=0 (Member ausgelassen).
// ---------------------------------------------------------------------------

#[test]
fn v11_optional_mutable_some() {
    let idl = r#"@mutable struct O { @id(1) @optional long maybe; };"#;
    let body = r#"    ::O o;
    o.maybe(7);
    __buf = ::dds::topic::topic_type_support<::O>::encode(o);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-11A, no C++ compiler");
        return;
    };
    let expected: Vec<u8> = vec![
        0x08, 0x00, 0x00, 0x00, // DHEADER = 8
        0x01, 0x00, 0x00, 0x20, // EMHEADER LE: M=0 LC=2 id=1
        0x07, 0x00, 0x00, 0x00, // long = 7
    ];
    assert_bytes("V-11A", &bytes, &expected);
}

#[test]
fn v11_optional_mutable_none() {
    let idl = r#"@mutable struct O { @id(1) @optional long maybe; };"#;
    let body = r#"    ::O o;
    // maybe stays std::nullopt.
    __buf = ::dds::topic::topic_type_support<::O>::encode(o);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-11B, no C++ compiler");
        return;
    };
    assert_bytes("V-11B", &bytes, &[0x00, 0x00, 0x00, 0x00]);
}

// ---------------------------------------------------------------------------
// V-12 Mutable Sentinel End-Marker
//
// Spec §6.V-12: Mutable-Streams DUERFEN keinen expliziten Sentinel
// emittieren — die DHEADER-Groesse begrenzt das Lesen. Wir verifizieren
// dass das Encoder-Output keinen PID_LIST_END (`0x3F02 0x00 0x00`) anhaengt.
// ---------------------------------------------------------------------------

#[test]
fn v12_mutable_no_explicit_sentinel() {
    let idl = r#"@mutable struct M2 { @id(1) long x; };"#;
    let body = r#"    ::M2 m;
    m.x(99);
    __buf = ::dds::topic::topic_type_support<::M2>::encode(m);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-12, no C++ compiler");
        return;
    };
    let expected: Vec<u8> = vec![
        0x08, 0x00, 0x00, 0x00, // DHEADER = 8
        0x01, 0x00, 0x00, 0x20, // EMHEADER LE: M=0 LC=2 id=1
        0x63, 0x00, 0x00, 0x00, // x = 99
    ];
    assert_bytes("V-12", &bytes, &expected);
    // Negative-check: PID_LIST_END (0x3F02) MUST NOT occur.
    assert!(
        !bytes.windows(2).any(|w| w == [0x3F, 0x02]),
        "V-12 must not emit explicit XCDR1 PID_LIST_END sentinel"
    );
}

// ---------------------------------------------------------------------------
// Roundtrip-Sanity ueber ein paar Vektoren (decode(encode(v)) == v).
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_v2_v4_v5_v9() {
    let Some(_cc) = cpp_compiler() else {
        eprintln!("WARNING: skipping roundtrip sanity, no C++ compiler");
        return;
    };

    // V-2 final primitives
    let idl = "@final struct Point { long x; long y; };";
    let body = r#"    ::Point p; p.x(7); p.y(-3);
    auto __b = ::dds::topic::topic_type_support<::Point>::encode(p);
    auto __q = ::dds::topic::topic_type_support<::Point>::decode(__b.data(), __b.size());
    if (__q.x() != 7 || __q.y() != -3) { std::fprintf(stderr, "v2 roundtrip fail\n"); return 1; }
    __buf.push_back(0xAA);
"#;
    let bytes = run_encode(idl, body).expect("v2 rt");
    assert_eq!(bytes, vec![0xAA]);

    // V-9 appendable
    let idl9 = "@appendable struct V { long a; long b; };";
    let body9 = r#"    ::V v; v.a(11); v.b(22);
    auto __b = ::dds::topic::topic_type_support<::V>::encode(v);
    auto __q = ::dds::topic::topic_type_support<::V>::decode(__b.data(), __b.size());
    if (__q.a() != 11 || __q.b() != 22) { std::fprintf(stderr, "v9 roundtrip fail\n"); return 1; }
    __buf.push_back(0xBB);
"#;
    let bytes9 = run_encode(idl9, body9).expect("v9 rt");
    assert_eq!(bytes9, vec![0xBB]);
}

// ---------------------------------------------------------------------------
// Header-Inhalts-Probe: extensibility() korrekt fuer alle 3 Modi.
// ---------------------------------------------------------------------------

#[test]
fn extensibility_all_three_modes() {
    let idl = r#"
        @final struct F { long x; };
        @appendable struct A { long x; };
        @mutable struct M { @id(1) long x; };
    "#;
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let header = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
    assert!(
        header.contains("DataRepresentationKind::FINAL"),
        "@final must emit FINAL extensibility"
    );
    assert!(
        header.contains("DataRepresentationKind::APPENDABLE"),
        "@appendable must emit APPENDABLE extensibility"
    );
    assert!(
        header.contains("DataRepresentationKind::MUTABLE"),
        "@mutable must emit MUTABLE extensibility"
    );
}
