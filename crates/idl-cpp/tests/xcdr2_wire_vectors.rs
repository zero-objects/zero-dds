//! Wire-vector conformance tests for the XCDR2 C++ codegen.
//!
//! Mandatory corpus: V-1 .. V-12 from
//! `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6.
//!
//! Per vector:
//! 1. Parse IDL + generate the C++ header.
//! 2. Build a C++ test program that constructs the sample values,
//!    calls `topic_type_support<T>::encode(sample)` and prints the raw
//!    bytes as hex to stdout.
//! 3. Compare the byte stream against the expected sequence.
//!
//! Ground-Truth:
//! - Bytes of scalar primitives are mathematically unambiguous via
//!   `struct.pack('<...>')` / IEEE-754 — we use them as the truth.
//! - Vectors V-3, V-8, V-10, V-11 demonstrably contain typos in the
//!   printed bytes in the spec (see test comments); we
//!   therefore check against the OMG XTypes 1.3 §7.4-conformant values.
//! - All other vectors match the spec byte-exactly.

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
// Spec §6 V-3 contains typos in the hexadecimal bytes for ul and
// ll (see the test comment). We check against the OMG XTypes 1.3 §7.4
// conformant bytes (Python `struct.pack('<...>')`-verified).
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
        // XCDR2 §7.4.1.1.1: no 8-byte pad — double @ offset 36 (4-aligned).
        0x6E, 0x86, 0x1B, 0xF0, 0xF9, 0x21, 0x09, 0x40, // d = 3.14159 @36
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
// XCDR2 §7.4.3.5: sequence<string> has NON-primitive elements →
// DHEADER (uint32 = byte length of [count + elements]) in front.
// Layout: DHEADER(4)=19 + count(4) + str0("a\0", pad zu 4) + str1("bc\0").
// V-6 Body: `02 00 00 00 02 00 00 00 61 00 00 00 03 00 00 00 62 63 00` (19 B),
// with a DHEADER in front: `13 00 00 00 …`. Cyclone-DDS-verified (V-5
// seq<long> primitive → no DHEADER; V-6 seq<string> → DHEADER).
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
            // XCDR2 §7.4.3.5: seq<string> (non-primitive) → DHEADER in front.
            0x13, 0x00, 0x00, 0x00, // DHEADER = 19 (count + elements)
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
// Spec §6 V-8 shows a supposedly expected hash that does not
// MD5(00 00 00 2A) matches (spec typo). We verify against the
// real RFC-1321 MD5 value: a5 15 85 57 99 dd bd a0 8b c9 9f c2 ce 87 fa 79.
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
            // XCDR2 §7.4.1.1.1: no 8-byte pad — double @ offset 4 (4-aligned).
            0x1F, 0x85, 0xEB, 0x51, 0xB8, 0x1E, 0x09, 0x40, // value = 3.14 @4
        ],
    );

    // Key-Hash test
    let body_hash = r#"    ::Sensor s; s.id(42); s.value(3.14);
    auto __h = ::dds::topic::topic_type_support<::Sensor>::key_hash(s);
    __buf.assign(__h.begin(), __h.end());
"#;
    let bytes_h = run_encode(idl, body_hash).expect("hash run");
    // XTypes 1.3 §7.6.8.4: the holder (4 bytes BE-int32 = 00 00 00 2A) is
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
// Spec §6 V-10 states DHEADER=20; in fact the shown
// Body 23 Bytes (4 EMHEADER1 + 4 long + 4 EMHEADER2 + 4 NEXTINT + 7 string).
// Per OMG XTypes 1.3 §7.4.4.4, DHEADER = body-size = 23. We test
// against the OMG-conformant sequence.
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
    // Body-Layout (origin = byte after DHEADER). Bug XV-mut: @mutable members use
    // COMPACT length codes (XTypes 1.3 §7.4.3.4.2), cross-vendor-validated against
    // CycloneDDS/RTI/FastDDS and the Rust reference (`LengthCode`):
    //   EMHEADER1 (LC=2, id=1) = u32 0x20000001 LE = 01 00 00 20 (4-byte body, no NEXTINT)
    //   a = 42 (LE int32) = 2A 00 00 00
    //   EMHEADER2 (LC=5, id=2) = u32 0x50000002 LE = 02 00 00 50 (string reuses its
    //     own uint32 len-prefix as NEXTINT — no separate NEXTINT)
    //   string "hi\0" = 03 00 00 00 68 69 00
    // Body = 8 (a) + 11 (b) = 19 bytes. DHEADER = 19 (0x13).
    let expected: Vec<u8> = vec![
        0x13, 0x00, 0x00, 0x00, // DHEADER = 19
        0x01, 0x00, 0x00, 0x20, // EMHEADER1 LE: M=0 LC=2 id=1
        0x2A, 0x00, 0x00, 0x00, // a = 42
        0x02, 0x00, 0x00, 0x50, // EMHEADER2 LE: M=0 LC=5 id=2 (string len-prefix = NEXTINT)
        0x03, 0x00, 0x00, 0x00, b'h', b'i', 0x00, // string "hi\0"
    ];
    assert_bytes("V-10", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-11 Optional Member (Mutable)
//
// Sample-A (Some(7)): OMG-konformer DHEADER=8 (4 EMHEADER + 4 long).
// Sample-B (None):    DHEADER=0 (member omitted).
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
    // Bug XV-mut: @mutable 4-byte primitive = compact LC=2 (no NEXTINT).
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
// Spec §6.V-12: mutable streams MUST NOT emit an explicit sentinel
// — the DHEADER size bounds the reading. We verify
// that the encoder output does not append a PID_LIST_END (`0x3F02 0x00 0x00`).
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
    // Bug XV-mut: @mutable 4-byte primitive = compact LC=2 (no NEXTINT).
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
// Roundtrip sanity over a few vectors (decode(encode(v)) == v).
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
    auto __q = ::dds::topic::topic_type_support<::Point>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr1);
    if (__q.x() != 7 || __q.y() != -3) { std::fprintf(stderr, "v2 roundtrip fail\n"); return 1; }
    __buf.push_back(0xAA);
"#;
    let bytes = run_encode(idl, body).expect("v2 rt");
    assert_eq!(bytes, vec![0xAA]);

    // V-9 appendable — XCDR1 round-trip (encode + decode with the SAME repr).
    // An @appendable struct carries a DHEADER under XCDR2 but NONE under XCDR1,
    // so the previous `encode(v)` [XCDR2] + `decode(.., Xcdr1)` was a repr
    // mismatch that only round-tripped while XCDR1 was (wrongly) XCDR2-framed.
    let idl9 = "@appendable struct V { long a; long b; };";
    let body9 = r#"    ::V v; v.a(11); v.b(22);
    auto __b = ::dds::topic::topic_type_support<::V>::encode(v, ::dds::topic::xcdr2::XcdrVersion::Xcdr1);
    auto __q = ::dds::topic::topic_type_support<::V>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr1);
    if (__q.a() != 11 || __q.b() != 22) { std::fprintf(stderr, "v9 roundtrip fail\n"); return 1; }
    __buf.push_back(0xBB);
"#;
    let bytes9 = run_encode(idl9, body9).expect("v9 rt");
    assert_eq!(bytes9, vec![0xBB]);
}

// ---------------------------------------------------------------------------
// Header content probe: extensibility() correct for all 3 modes.
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

// ---------------------------------------------------------------------------
// V-XCDR2-ALIGN: @final with an 8-byte field after a 4-byte field. The DEFINITIVE
// Difference XCDR1 vs XCDR2:
//   XCDR1: double aligned to 8 -> 4 bytes padding -> 16 bytes total.
//   XCDR2 (PLAIN_CDR2, @final): double aligned to min(8,4)=4 ->
//          no padding -> 12 bytes total.
// Proves that `encode(s, Xcdr2)` applies the XTypes-1.3-§7.4.3.4.2 alignment
// rule applies and `encode(s)` (default) stays XCDR1.
// ---------------------------------------------------------------------------

#[test]
fn vxcdr2_final_alignment_differs_from_xcdr1() {
    let idl = "@final struct M { long a; double d; };";

    // XCDR2: no padding between a and d.
    let body_x2 = r#"    ::M m;
    m.a(7); m.d(1.0);
    __buf = ::dds::topic::topic_type_support<::M>::encode(m, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
"#;
    let Some(x2) = run_encode(idl, body_x2) else {
        eprintln!("WARNING: skipping VXCDR2-ALIGN, no C++ compiler");
        return;
    };
    assert_bytes(
        "VXCDR2-ALIGN/Xcdr2",
        &x2,
        &[
            0x07, 0x00, 0x00, 0x00, // a=7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0x3F, // d=1.0 @ offset 4 (no pad)
        ],
    );

    // XCDR1 (explicit): 4 bytes padding -> d @ offset 8. (The default is
    // XCDR2 since the XCDR2 default flip, hence explicit Xcdr1 here.)
    let body_x1 = r#"    ::M m;
    m.a(7); m.d(1.0);
    __buf = ::dds::topic::topic_type_support<::M>::encode(m, ::dds::topic::xcdr2::XcdrVersion::Xcdr1);
"#;
    let x1 = run_encode(idl, body_x1).expect("compiler war oben da");
    assert_bytes(
        "VXCDR2-ALIGN/Xcdr1",
        &x1,
        &[
            0x07, 0x00, 0x00, 0x00, // a=7
            0x00, 0x00, 0x00, 0x00, // padding auf 8-align
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0x3F, // d=1.0 @ offset 8
        ],
    );

    assert_eq!(x1.len(), 16, "XCDR1 with padding");
    assert_eq!(x2.len(), 12, "XCDR2 without padding");
}

// ---------------------------------------------------------------------------
// V-wstring  wstring (conformance §9.1: UTF-16 wire, byte-identical to the
// cross-vendor cdr-core XTypes reference): uint32 octets = (#units * 2), then
// the raw UTF-16-LE code units — NO byte-order mark, NO terminator. (The
// CORBA-GIOP `WString` form prepends a BOM; the XTypes/DDS golden does not.)
// ---------------------------------------------------------------------------

#[test]
fn v_wstring_bmp_roundtrips_byte_exact() {
    let idl = "@final struct W { wstring label; };";
    let body = "    ::W w;\n    w.label(L\"Hi\");\n    __buf = ::dds::topic::topic_type_support<::W>::encode(w);\n";
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-wstring, no C++ compiler");
        return;
    };
    // octets=4 (2 units * 2) ; no BOM ; 'H'=0x48 'i'=0x69 little-endian.
    assert_bytes(
        "V-wstring",
        &bytes,
        &[0x04, 0x00, 0x00, 0x00, 0x48, 0x00, 0x69, 0x00],
    );
}

#[test]
fn v_wstring_empty_is_zero_length() {
    let idl = "@final struct W { wstring label; };";
    let body = "    ::W w;\n    w.label(L\"\");\n    __buf = ::dds::topic::topic_type_support<::W>::encode(w);\n";
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-wstring-empty, no C++ compiler");
        return;
    };
    // Empty wstring = uint32 length 0, no BOM (Rust + GIOP convention).
    assert_bytes("V-wstring-empty", &bytes, &[0x00, 0x00, 0x00, 0x00]);
}

// ---------------------------------------------------------------------------
// V-array  1-D fixed array of primitive: N contiguous elements, no length
// prefix, no DHEADER (XTypes §7.4.3). `long vals[3]` = 3 × int32-LE.
// ---------------------------------------------------------------------------

#[test]
fn v_array_long3_roundtrips_byte_exact() {
    let idl = "@final struct A { long vals[3]; };";
    let body = "    ::A a;\n    auto __t = a.vals(); __t[0]=1; __t[1]=2; __t[2]=3; a.vals(__t);\n    __buf = ::dds::topic::topic_type_support<::A>::encode(a);\n";
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-array, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-array",
        &bytes,
        &[0x01, 0, 0, 0, 0x02, 0, 0, 0, 0x03, 0, 0, 0],
    );
}

// ---------------------------------------------------------------------------
// V-enum  enum member encoded as its int32 underlying type (Spec §7.4.1.4.2).
// ---------------------------------------------------------------------------

#[test]
fn v_enum_member_is_int32() {
    let idl = "enum Color { RED, GREEN, BLUE }; @final struct S { Color c; };";
    let body = "    ::S s;\n    s.c(::Color::GREEN);\n    __buf = ::dds::topic::topic_type_support<::S>::encode(s);\n";
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-enum, no C++ compiler");
        return;
    };
    // GREEN = 1 → int32-LE.
    assert_bytes("V-enum", &bytes, &[0x01, 0x00, 0x00, 0x00]);
}

// ---------------------------------------------------------------------------
// V-nested  nested @final struct: members encoded inline, no DHEADER.
// ---------------------------------------------------------------------------

#[test]
fn v_nested_final_struct_inline() {
    let idl =
        "@final struct Inner { long a; long b; }; @final struct Outer { Inner inner; long tail; };";
    let body = "    ::Outer o;\n    auto __i = o.inner(); __i.a(10); __i.b(20); o.inner(__i);\n    o.tail(30);\n    __buf = ::dds::topic::topic_type_support<::Outer>::encode(o);\n";
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-nested, no C++ compiler");
        return;
    };
    // inner.a=10, inner.b=20, tail=30 — three int32-LE, no DHEADER (@final).
    assert_bytes(
        "V-nested",
        &bytes,
        &[0x0A, 0, 0, 0, 0x14, 0, 0, 0, 0x1E, 0, 0, 0],
    );
}

// ---------------------------------------------------------------------------
// V-seqstruct  sequence<@final struct>: DHEADER (non-primitive element,
// IS_PRIMITIVE=false in the canonical Rust encoder) + count + each element
// inline. Byte-exact encode AND decode roundtrip in one TU.
// ---------------------------------------------------------------------------

#[test]
fn v_sequence_of_struct() {
    let idl = "@final struct Pt { long x; long y; }; @final struct Path { sequence<Pt> pts; };";
    let body = r#"    ::Path p;
    std::vector<::Pt> __v;
    ::Pt __e0; __e0.x(1); __e0.y(2); __v.push_back(__e0);
    ::Pt __e1; __e1.x(3); __e1.y(4); __v.push_back(__e1);
    p.pts(__v);
    __buf = ::dds::topic::topic_type_support<::Path>::encode(p);
    auto __q = ::dds::topic::topic_type_support<::Path>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.pts().size() != 2 || __q.pts()[0].x() != 1 || __q.pts()[0].y() != 2
        || __q.pts()[1].x() != 3 || __q.pts()[1].y() != 4) {
        std::fprintf(stderr, "seq<struct> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seqstruct, no C++ compiler");
        return;
    };
    // DHEADER=0x14 (count4 + 2*8) + count=2 + (1,2) + (3,4), all int32-LE.
    assert_bytes(
        "V-seqstruct",
        &bytes,
        &[
            0x14, 0, 0, 0, // DHEADER = 20
            0x02, 0, 0, 0, // count = 2
            0x01, 0, 0, 0, 0x02, 0, 0, 0, // {1,2}
            0x03, 0, 0, 0, 0x04, 0, 0, 0, // {3,4}
        ],
    );
}

#[test]
fn v_sequence_of_appendable_struct() {
    // sequence<@appendable struct>: each element carries its own DHEADER and
    // is 4-aligned before it. The earlier idl-cpp gap (seq elements gated to
    // @final) — now closed via the per-element pad-to-4 + splice/sub-decode.
    let idl =
        "@appendable struct Pt { long x; long y; }; @final struct Path { sequence<Pt> pts; };";
    let body = r#"    ::Path p;
    std::vector<::Pt> __v;
    ::Pt __e0; __e0.x(1); __e0.y(2); __v.push_back(__e0);
    ::Pt __e1; __e1.x(3); __e1.y(4); __v.push_back(__e1);
    p.pts(__v);
    __buf = ::dds::topic::topic_type_support<::Path>::encode(p);
    auto __q = ::dds::topic::topic_type_support<::Path>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.pts().size() != 2 || __q.pts()[0].x() != 1 || __q.pts()[0].y() != 2
        || __q.pts()[1].x() != 3 || __q.pts()[1].y() != 4) {
        std::fprintf(stderr, "seq<@appendable struct> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seq-appendable-struct, no C++ compiler");
        return;
    };
    // Path @final → no outer DHEADER. seq DHEADER = count(4) + 2*[Pt-DHEADER(4)
    // + x(4) + y(4)] = 4 + 24 = 28 = 0x1C. Each @appendable Pt element:
    // DHEADER=8 (body x+y) then x, y; element starts 4-aligned.
    assert_bytes(
        "V-seq-appendable-struct",
        &bytes,
        &[
            0x1C, 0, 0, 0, // seq DHEADER = 28
            0x02, 0, 0, 0, // count = 2
            0x08, 0, 0, 0, 0x01, 0, 0, 0, 0x02, 0, 0, 0, // elem0: Pt-DHEADER=8, {1,2}
            0x08, 0, 0, 0, 0x03, 0, 0, 0, 0x04, 0, 0, 0, // elem1: Pt-DHEADER=8, {3,4}
        ],
    );
}

#[test]
fn v_mutable_member_seq_of_appendable_struct() {
    // sequence<@appendable struct> as a member of a @mutable struct (the
    // EMHEADER body-origin path). Roundtrip-only: the @mutable EMHEADER framing
    // is exercised by encode→decode; the C++ harness returns 1 on mismatch
    // (run_encode panics on a non-zero exit), so a green run proves correctness.
    let idl = "@appendable struct Pt { long x; long y; }; @mutable struct M { sequence<Pt> pts; };";
    let body = r#"    ::M m;
    std::vector<::Pt> __v;
    ::Pt __e0; __e0.x(10); __e0.y(20); __v.push_back(__e0);
    ::Pt __e1; __e1.x(30); __e1.y(40); __v.push_back(__e1);
    m.pts(__v);
    __buf = ::dds::topic::topic_type_support<::M>::encode(m);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.pts().size() != 2 || __q.pts()[0].x() != 10 || __q.pts()[0].y() != 20
        || __q.pts()[1].x() != 30 || __q.pts()[1].y() != 40) {
        std::fprintf(stderr, "mutable-member seq<@appendable struct> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mutable-seq-appendable-struct, no C++ compiler");
        return;
    };
    assert!(
        !bytes.is_empty(),
        "encode produced no bytes (roundtrip enforced C++-side)"
    );
}

// ---------------------------------------------------------------------------
// V-seqenum  sequence<enum>: enum is non-primitive (IS_PRIMITIVE=false, per
// XTypes §7.4.3.5 + canonical Rust `CdrEncode` default) -> DHEADER + count +
// each element as int32-LE. Byte-exact encode AND decode roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_sequence_of_enum() {
    let idl = "enum Color { RED, GREEN, BLUE }; @final struct S { sequence<Color> cs; };";
    let body = r#"    ::S s;
    std::vector<::Color> __v;
    __v.push_back(::Color::GREEN); __v.push_back(::Color::BLUE); __v.push_back(::Color::RED);
    s.cs(__v);
    __buf = ::dds::topic::topic_type_support<::S>::encode(s);
    auto __q = ::dds::topic::topic_type_support<::S>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.cs().size() != 3 || __q.cs()[0] != ::Color::GREEN
        || __q.cs()[1] != ::Color::BLUE || __q.cs()[2] != ::Color::RED) {
        std::fprintf(stderr, "seq<enum> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seqenum, no C++ compiler");
        return;
    };
    // DHEADER=0x10 (count4 + 3*4) + count=3 + GREEN(1),BLUE(2),RED(0).
    assert_bytes(
        "V-seqenum",
        &bytes,
        &[
            0x10, 0, 0, 0, // DHEADER = 16
            0x03, 0, 0, 0, // count = 3
            0x01, 0, 0, 0, // GREEN
            0x02, 0, 0, 0, // BLUE
            0x00, 0, 0, 0, // RED
        ],
    );
}

// ---------------------------------------------------------------------------
// V-seqwstring  sequence<wstring>: non-primitive element -> DHEADER + count +
// each wstring (uint32 octets + BOM + UTF-16-LE units, per Finding 1). Byte-
// exact encode AND decode roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_sequence_of_wstring() {
    let idl = "@final struct W { sequence<wstring> ws; };";
    let body = r#"    ::W w;
    std::vector<std::wstring> __v;
    __v.push_back(L"Hi");
    w.ws(__v);
    __buf = ::dds::topic::topic_type_support<::W>::encode(w);
    auto __q = ::dds::topic::topic_type_support<::W>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.ws().size() != 1 || __q.ws()[0] != L"Hi") {
        std::fprintf(stderr, "seq<wstring> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seqwstring, no C++ compiler");
        return;
    };
    // DHEADER=0x0C (count4 + 8-byte wstring) + count=1 + "Hi" wstring (no BOM).
    assert_bytes(
        "V-seqwstring",
        &bytes,
        &[
            0x0C, 0, 0, 0, // DHEADER = 12
            0x01, 0, 0, 0, // count = 1
            0x04, 0, 0, 0, 0x48, 0x00, 0x69, 0x00, // "Hi" (octets=4, no BOM)
        ],
    );
}

// ---------------------------------------------------------------------------
// V-fwdskip  Forward-compat: a reader whose @mutable schema lacks members the
// writer sent MUST skip them by LengthCode. Exercises the two non-trivial skip
// arms: a variable member (string, LC=4 -> skip NEXTINT bytes) and an 8-byte
// primitive (double, LC=3 -> skip exactly 8 bytes, NO NEXTINT). With the prior
// LC=3-for-variable bug, the string skip read a phantom NEXTINT and desynced.
// ---------------------------------------------------------------------------

#[test]
fn v_mutable_forward_compat_skip() {
    let idl = r#"
        @mutable struct Wide { @id(1) long a; @id(2) string s; @id(3) double d; @id(4) long c; };
        @mutable struct Narrow { @id(1) long a; @id(4) long c; };
    "#;
    let body = r#"    ::Wide __w;
    __w.a(7); __w.s("hello"); __w.d(2.5); __w.c(99);
    auto __b = ::dds::topic::topic_type_support<::Wide>::encode(__w);
    auto __n = ::dds::topic::topic_type_support<::Narrow>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__n.a() != 7 || __n.c() != 99) {
        std::fprintf(stderr, "fwd-compat skip fail: a=%ld c=%ld\n", (long)__n.a(), (long)__n.c());
        return 1;
    }
    __buf.push_back(0xCC);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-fwdskip, no C++ compiler");
        return;
    };
    // Runtime asserted the skip already; the sentinel confirms we reached the end.
    assert_bytes("V-fwdskip", &bytes, &[0xCC]);
}

// ---------------------------------------------------------------------------
// V-mutscoped  @mutable struct carrying an enum member (compact LC=2), a nested
// @final struct member (LC=4 NEXTINT frame), and a sequence<enum> member.
// Encode + decode roundtrip proves the mutable-path enum/struct/seq arms are
// wired symmetrically (previously these members were silently dropped).
// ---------------------------------------------------------------------------

#[test]
fn v_mutable_enum_struct_seq_members() {
    let idl = r#"
        enum Color { RED, GREEN, BLUE };
        @final struct Pt { long x; long y; };
        @mutable struct MM { @id(1) Color c; @id(2) Pt p; @id(3) sequence<Color> cs; };
    "#;
    let body = r#"    ::MM __m;
    __m.c(::Color::BLUE);
    ::Pt __p; __p.x(5); __p.y(6); __m.p(__p);
    std::vector<::Color> __cs; __cs.push_back(::Color::RED); __cs.push_back(::Color::GREEN); __m.cs(__cs);
    auto __b = ::dds::topic::topic_type_support<::MM>::encode(__m);
    auto __q = ::dds::topic::topic_type_support<::MM>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.c() != ::Color::BLUE || __q.p().x() != 5 || __q.p().y() != 6
        || __q.cs().size() != 2 || __q.cs()[0] != ::Color::RED || __q.cs()[1] != ::Color::GREEN) {
        std::fprintf(stderr, "mutable enum/struct/seq roundtrip fail\n"); return 1;
    }
    __buf.push_back(0xDD);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mutscoped, no C++ compiler");
        return;
    };
    assert_bytes("V-mutscoped", &bytes, &[0xDD]);
}

// ---------------------------------------------------------------------------
// V-nested-app  A nested @appendable struct MEMBER of a @final struct. The
// nested struct contributes its OWN DHEADER (Plain-CDR2 §7.4.3.4.2) — unlike a
// nested @final member which is inlined. Spliced from the nested type's own
// encode; byte-exact for one inner long (no padding) + decode roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_nested_appendable_struct_byte_exact() {
    let idl = r#"
        @appendable struct A { long x; };
        @final struct Outer { A a; };
    "#;
    let body = r#"    ::Outer __o;
    ::A __a; __a.x(7); __o.a(__a);
    __buf = ::dds::topic::topic_type_support<::Outer>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::Outer>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.a().x() != 7) { std::fprintf(stderr, "nested appendable roundtrip fail\n"); return 1; }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-nested-app, no C++ compiler");
        return;
    };
    // Outer @final -> no DHEADER. Member A @appendable -> its own DHEADER = 4
    // (one long body), then x=7 little-endian.
    assert_bytes(
        "V-nested-app",
        &bytes,
        &[
            0x04, 0, 0, 0, // A's DHEADER = 4
            0x07, 0, 0, 0, // A.x = 7
        ],
    );
}

// ---------------------------------------------------------------------------
// V-nested-mut  A nested @mutable struct MEMBER of a @final struct. Mutable
// wire (DHEADER + EMHEADER stream) makes a hand-vector brittle, so roundtrip
// only — proves the splice encode/decode is symmetric.
// ---------------------------------------------------------------------------

#[test]
fn v_nested_mutable_struct_roundtrips() {
    let idl = r#"
        @mutable struct M { @id(1) long v; @id(2) string s; };
        @final struct OuterM { M m; };
    "#;
    let body = r#"    ::OuterM __o;
    ::M __m; __m.v(99); __m.s("hi"); __o.m(__m);
    __buf = ::dds::topic::topic_type_support<::OuterM>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::OuterM>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.m().v() != 99 || __q.m().s() != "hi") { std::fprintf(stderr, "nested mutable roundtrip fail\n"); return 1; }
    __buf.clear(); __buf.push_back(0xEE);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-nested-mut, no C++ compiler");
        return;
    };
    assert_bytes("V-nested-mut", &bytes, &[0xEE]);
}

// ---------------------------------------------------------------------------
// V-nested-app-in-mut  A nested @appendable struct as a member of a @MUTABLE
// struct: the member sits in an EMHEADER LC=4 NEXTINT frame, and the nested
// struct's own DHEADER lives inside that frame. Roundtrip proves the mutable
// splice arm (encode + decode) is symmetric.
// ---------------------------------------------------------------------------

#[test]
fn v_nested_appendable_in_mutable_member_roundtrips() {
    let idl = r#"
        @appendable struct A2 { long x; long y; };
        @mutable struct Holder { @id(1) A2 a; @id(2) long tag; };
    "#;
    let body = r#"    ::Holder __h;
    ::A2 __a; __a.x(3); __a.y(4); __h.a(__a); __h.tag(123);
    __buf = ::dds::topic::topic_type_support<::Holder>::encode(__h);
    auto __q = ::dds::topic::topic_type_support<::Holder>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.a().x() != 3 || __q.a().y() != 4 || __q.tag() != 123) {
        std::fprintf(stderr, "nested appendable-in-mutable roundtrip fail\n"); return 1;
    }
    __buf.clear(); __buf.push_back(0xFF);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-nested-app-in-mut, no C++ compiler");
        return;
    };
    assert_bytes("V-nested-app-in-mut", &bytes, &[0xFF]);
}

// ---------------------------------------------------------------------------
// V-map  map<K,V> (XTypes §7.4.4.6): non-primitive collection -> DHEADER +
// count + interleaved key/value in ascending key order (std::map == BTreeMap).
// Byte-exact for map<long,long> (all int32, no padding) + decode roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_map_long_long() {
    let idl = "@final struct M { map<long, long> m; };";
    let body = r#"    ::M __o;
    std::map<int32_t, int32_t> __mp;
    __mp[1] = 10; __mp[2] = 20;
    __o.m(__mp);
    __buf = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.m().size() != 2 || __q.m().at(1) != 10 || __q.m().at(2) != 20) {
        std::fprintf(stderr, "map<long,long> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-map, no C++ compiler");
        return;
    };
    // XCDR2 §7.4.3.5 (MapPrim fix d41cfb33): a map with a PRIMITIVE key AND
    // primitive value carries NO DHEADER — same rule as sequence<primitive>.
    // count=2 + (1,10),(2,20) int32-LE, no leading length word. Byte-anchored
    // against Fast DDS + OpenDDS (proofs/structural/mapbare).
    assert_bytes(
        "V-map",
        &bytes,
        &[
            0x02, 0, 0, 0, // count = 2
            0x01, 0, 0, 0, 0x0A, 0, 0, 0, // 1 -> 10
            0x02, 0, 0, 0, 0x14, 0, 0, 0, // 2 -> 20
        ],
    );
}

// map<long,string>: string values exercise per-entry variable-length encoding
// + intra-body alignment. Roundtrip only (padding makes a hand-vector brittle).
#[test]
fn v_map_long_string_roundtrips() {
    let idl = "@final struct M { map<long, string> m; };";
    let body = r#"    ::M __o;
    std::map<int32_t, std::string> __mp;
    __mp[1] = "a"; __mp[2] = "bb"; __mp[3] = "ccc";
    __o.m(__mp);
    auto __b = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.m().size() != 3 || __q.m().at(1) != "a" || __q.m().at(2) != "bb" || __q.m().at(3) != "ccc") {
        std::fprintf(stderr, "map<long,string> roundtrip fail\n"); return 1;
    }
    __buf.push_back(0xEE);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-map-str, no C++ compiler");
        return;
    };
    assert_bytes("V-map-str", &bytes, &[0xEE]);
}

// @mutable struct with a map member: cpp<->cpp roundtrip (cross-vendor inner-
// DHEADER question shared with mutable seq — Finding 6).
#[test]
fn v_mutable_map_roundtrips() {
    let idl = "@mutable struct MM { @id(1) map<long, long> m; @id(2) long tail; };";
    let body = r#"    ::MM __o;
    std::map<int32_t, int32_t> __mp;
    __mp[7] = 70; __mp[8] = 80;
    __o.m(__mp);
    __o.tail(42);
    auto __b = ::dds::topic::topic_type_support<::MM>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::MM>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.m().size() != 2 || __q.m().at(7) != 70 || __q.m().at(8) != 80 || __q.tail() != 42) {
        std::fprintf(stderr, "mutable map roundtrip fail\n"); return 1;
    }
    __buf.push_back(0xFE);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mutmap, no C++ compiler");
        return;
    };
    assert_bytes("V-mutmap", &bytes, &[0xFE]);
}

// sequence<map<K,V>> — a sequence whose element is a map. Each map carries its
// own DHEADER inside the outer sequence's DHEADER. cpp<->cpp roundtrip.
#[test]
fn v_sequence_of_map_roundtrips() {
    let idl = "@final struct M { sequence<map<long, long>> sm; };";
    let body = r#"    ::M __o;
    std::vector<std::map<int32_t, int32_t>> __v;
    std::map<int32_t, int32_t> __m0; __m0[1] = 10; __v.push_back(__m0);
    std::map<int32_t, int32_t> __m1; __m1[2] = 20; __m1[3] = 30; __v.push_back(__m1);
    __o.sm(__v);
    auto __b = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.sm().size() != 2 || __q.sm()[0].at(1) != 10 || __q.sm()[1].at(2) != 20 || __q.sm()[1].at(3) != 30) {
        std::fprintf(stderr, "seq<map> roundtrip fail\n"); return 1;
    }
    __buf.push_back(0xEC);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seqmap, no C++ compiler");
        return;
    };
    assert_bytes("V-seqmap", &bytes, &[0xEC]);
}

// ---------------------------------------------------------------------------
// V-seqseq  nested sequence<sequence<long>>: outer is non-primitive (inner
// seq) -> outer DHEADER; the inner sequence<long> has primitive elements -> NO
// inner DHEADER. Byte-exact + decode roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_sequence_of_sequence() {
    let idl = "@final struct M { sequence<sequence<long>> m; };";
    let body = r#"    ::M __o;
    std::vector<std::vector<int32_t>> __mm;
    __mm.push_back(std::vector<int32_t>{1, 2});
    __mm.push_back(std::vector<int32_t>{3});
    __o.m(__mm);
    __buf = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.m().size() != 2 || __q.m()[0].size() != 2 || __q.m()[0][0] != 1
        || __q.m()[0][1] != 2 || __q.m()[1].size() != 1 || __q.m()[1][0] != 3) {
        std::fprintf(stderr, "seq<seq<long>> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seqseq, no C++ compiler");
        return;
    };
    // outer DHEADER=0x18 (count4 + inner0[12] + inner1[8]); inner seqs carry NO
    // DHEADER (long is primitive).
    assert_bytes(
        "V-seqseq",
        &bytes,
        &[
            0x18, 0, 0, 0, // outer DHEADER = 24
            0x02, 0, 0, 0, // outer count = 2
            0x02, 0, 0, 0, 0x01, 0, 0, 0, 0x02, 0, 0, 0, // inner0 = [1,2]
            0x01, 0, 0, 0, 0x03, 0, 0, 0, // inner1 = [3]
        ],
    );
}

// @mutable struct with a nested-sequence member: cpp<->cpp roundtrip (the
// inner-DHEADER cross-vendor question is Finding 6, shared with mutable seq/map).
#[test]
fn v_mutable_sequence_of_sequence_roundtrips() {
    let idl = "@mutable struct MM { @id(1) sequence<sequence<long>> m; @id(2) long tail; };";
    let body = r#"    ::MM __o;
    std::vector<std::vector<int32_t>> __mm;
    __mm.push_back(std::vector<int32_t>{5, 6, 7});
    __mm.push_back(std::vector<int32_t>{8});
    __o.m(__mm);
    __o.tail(99);
    auto __b = ::dds::topic::topic_type_support<::MM>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::MM>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.m().size() != 2 || __q.m()[0].size() != 3 || __q.m()[0][2] != 7
        || __q.m()[1].size() != 1 || __q.m()[1][0] != 8 || __q.tail() != 99) {
        std::fprintf(stderr, "mutable seq<seq<long>> roundtrip fail\n"); return 1;
    }
    __buf.push_back(0xFD);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mutseqseq, no C++ compiler");
        return;
    };
    assert_bytes("V-mutseqseq", &bytes, &[0xFD]);
}

// ---------------------------------------------------------------------------
// V-arr2d  multi-dimensional array long[2][3]: row-major, fixed size, NO
// DHEADER (primitive elements; XTypes §7.4.3). Byte-exact + roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_array_2d_long() {
    let idl = "@final struct M { long grid[2][3]; };";
    let body = r#"    ::M __o;
    auto __g = __o.grid();
    __g[0][0]=1; __g[0][1]=2; __g[0][2]=3;
    __g[1][0]=4; __g[1][1]=5; __g[1][2]=6;
    __o.grid(__g);
    __buf = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.grid()[0][0] != 1 || __q.grid()[0][2] != 3 || __q.grid()[1][0] != 4 || __q.grid()[1][2] != 6) {
        std::fprintf(stderr, "array[2][3] roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-arr2d, no C++ compiler");
        return;
    };
    // Bug XV-arr: a multi-dim array of PRIMITIVE elements is a PARRAY (XTypes 1.3
    // §7.4.3.5 rule 8) — PLAIN-collection regardless of dimensionality, so it
    // carries NO collection DHEADER. The 6 row-major int32-LE values are written
    // back-to-back (cross-vendor-validated against the Rust golden).
    assert_bytes(
        "V-arr2d",
        &bytes,
        &[
            0x01, 0, 0, 0, 0x02, 0, 0, 0, 0x03, 0, 0, 0, // row 0
            0x04, 0, 0, 0, 0x05, 0, 0, 0, 0x06, 0, 0, 0, // row 1
        ],
    );
}

// ---------------------------------------------------------------------------
// V-arrstruct  1-D array of @final struct: non-primitive elements -> a DHEADER
// (XTypes §7.4.3.5), then N structs inline, NO count (fixed size). Byte-exact
// + roundtrip.
// ---------------------------------------------------------------------------

#[test]
fn v_array_of_struct() {
    let idl = "@final struct Pt { long x; long y; }; @final struct M { Pt path[2]; };";
    let body = r#"    ::M __o;
    auto __p = __o.path();
    ::Pt __e0; __e0.x(1); __e0.y(2); __p[0] = __e0;
    ::Pt __e1; __e1.x(3); __e1.y(4); __p[1] = __e1;
    __o.path(__p);
    __buf = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.path()[0].x() != 1 || __q.path()[0].y() != 2 || __q.path()[1].x() != 3 || __q.path()[1].y() != 4) {
        std::fprintf(stderr, "array-of-struct roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-arrstruct, no C++ compiler");
        return;
    };
    // DHEADER=0x10 (2 structs * 8 bytes) + {1,2} + {3,4}, no count.
    assert_bytes(
        "V-arrstruct",
        &bytes,
        &[
            0x10, 0, 0, 0, // DHEADER = 16
            0x01, 0, 0, 0, 0x02, 0, 0, 0, // {1,2}
            0x03, 0, 0, 0, 0x04, 0, 0, 0, // {3,4}
        ],
    );
}

// multi-dim array of @final struct: Pt grid[2][2] — non-primitive -> one DHEADER
// wrapping 4 inline structs (row-major, no count). Byte-exact + roundtrip.
#[test]
fn v_array_2d_of_struct() {
    let idl = "@final struct Pt { long x; long y; }; @final struct M { Pt grid[2][2]; };";
    let body = r#"    ::M __o;
    auto __g = __o.grid();
    __g[0][0].x(1); __g[0][0].y(2); __g[0][1].x(3); __g[0][1].y(4);
    __g[1][0].x(5); __g[1][0].y(6); __g[1][1].x(7); __g[1][1].y(8);
    __o.grid(__g);
    __buf = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.grid()[0][0].x() != 1 || __q.grid()[0][1].y() != 4 || __q.grid()[1][1].x() != 7 || __q.grid()[1][1].y() != 8) {
        std::fprintf(stderr, "array[2][2]-of-struct roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-arr2dstruct, no C++ compiler");
        return;
    };
    // DHEADER=0x20 (4 structs * 8 bytes) + {1,2}{3,4}{5,6}{7,8}.
    assert_bytes(
        "V-arr2dstruct",
        &bytes,
        &[
            0x20, 0, 0, 0, // DHEADER = 32
            0x01, 0, 0, 0, 0x02, 0, 0, 0, 0x03, 0, 0, 0, 0x04, 0, 0, 0, // row 0
            0x05, 0, 0, 0, 0x06, 0, 0, 0, 0x07, 0, 0, 0, 0x08, 0, 0, 0, // row 1
        ],
    );
}

// multi-dim array of string: string grid[2][2] — non-primitive -> one DHEADER
// wrapping 4 inline strings. Roundtrip (string padding makes a hand-vector brittle).
#[test]
fn v_array_2d_of_string_roundtrips() {
    let idl = "@final struct M { string grid[2][2]; };";
    let body = r#"    ::M __o;
    auto __g = __o.grid();
    __g[0][0] = "a"; __g[0][1] = "bb"; __g[1][0] = "ccc"; __g[1][1] = "dddd";
    __o.grid(__g);
    auto __b = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__b.data(), __b.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.grid()[0][0] != "a" || __q.grid()[0][1] != "bb" || __q.grid()[1][0] != "ccc" || __q.grid()[1][1] != "dddd") {
        std::fprintf(stderr, "string grid[2][2] roundtrip fail\n"); return 1;
    }
    __buf.push_back(0xEB);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-arr2dstr, no C++ compiler");
        return;
    };
    assert_bytes("V-arr2dstr", &bytes, &[0xEB]);
}

// ---------------------------------------------------------------------------
// V-mutseqstr-xvendor  @mutable { @id(1) sequence<string> } — BYTE-EXACT against
// the Rust reference encoder (zerodds-cdr, Cyclone-interop-verified). FINDING
// T1b: a non-primitive-element sequence's body BEGINS with its own DHEADER (a
// 4-byte length word), so the EMHEADER uses LengthCode-5 which REUSES that
// DHEADER as the NEXTINT — there is NO separate NEXTINT. The Rust reference
// (`member_body_has_leading_dheader` → `LengthCode::Lc5`) generates
// `encode_member_lc(1, false, Lc5, …)` for this member. Ground-truth bytes,
// encoding vec!["hi"] for member id 1 (LE, max_alignment=4):
//   13 00 00 00            outer DHEADER = 19
//   01 00 00 50            EMHEADER LC=5 id=1
//   0B 00 00 00            inner DHEADER = 11 (= count 4 + string 7) = the NEXTINT
//   01 00 00 00            count = 1
//   03 00 00 00 68 69 00   "hi\0"
// ---------------------------------------------------------------------------

#[test]
fn v_mutable_seq_string_matches_rust_wire() {
    let idl = "@mutable struct M { @id(1) sequence<string> s; };";
    let body = r#"    ::M __o;
    std::vector<std::string> __s; __s.push_back("hi");
    __o.s(__s);
    __buf = ::dds::topic::topic_type_support<::M>::encode(__o);
    auto __q = ::dds::topic::topic_type_support<::M>::decode(__buf.data(), __buf.size(), ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
    if (__q.s().size() != 1 || __q.s()[0] != "hi") {
        std::fprintf(stderr, "mutable seq<string> roundtrip fail\n"); return 1;
    }
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mutseqstr-xvendor, no C++ compiler");
        return;
    };
    assert_bytes(
        "V-mutseqstr-xvendor",
        &bytes,
        &[
            0x13, 0, 0, 0, // outer DHEADER = 19
            0x01, 0, 0, 0x50, // EMHEADER LC=5 id=1
            0x0B, 0, 0, 0, // inner DHEADER = 11 (= count 4 + string 7) = the NEXTINT
            0x01, 0, 0, 0, // count = 1
            0x03, 0, 0, 0, b'h', b'i', 0x00, // "hi\0"
        ],
    );
}

// ---------------------------------------------------------------------------
// V-autoid-mut — @mutable members WITHOUT explicit @id must take SEQUENTIAL
// 0-based member ids (XTypes 1.3 §7.3.4.3: @autoid defaults to SEQUENTIAL).
// REGRESSION GATE: the C++ backend previously assigned FNV name-hash ids here,
// diverging from rust/python/ts/csharp + Cyclone on the wire. Bytes are
// vendor-anchored to CycloneDDS (`idlc -l c` @mutable Pt + dds_stream_writeLE).
// ---------------------------------------------------------------------------
#[test]
fn v_autoid_mutable_sequential() {
    let idl = r#"@mutable struct AutoId { long a; long b; };"#;
    let body = r#"    ::AutoId m;
    m.a(42);
    m.b(99);
    __buf = ::dds::topic::topic_type_support<::AutoId>::encode(m);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-autoid-mut, no C++ compiler");
        return;
    };
    // a has NO @id -> auto-id 0; b -> auto-id 1. Both 4-byte primitives -> LC=2,
    // no NEXTINT. Body = 8 + 8 = 16 (0x10).
    let expected: Vec<u8> = vec![
        0x10, 0x00, 0x00, 0x00, // DHEADER = 16
        0x00, 0x00, 0x00, 0x20, // EMHEADER1 LE: LC=2 id=0  (NOT a name-hash!)
        0x2A, 0x00, 0x00, 0x00, // a = 42
        0x01, 0x00, 0x00, 0x20, // EMHEADER2 LE: LC=2 id=1
        0x63, 0x00, 0x00, 0x00, // b = 99
    ];
    assert_bytes("V-autoid-mut", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-arr-append-struct — a fixed array of an @appendable struct element. The C++
// backend used to SILENTLY DROP this member (gate only handled @final/enum
// elements) = data loss. REGRESSION GATE: the array carries a collection DHEADER
// (XTypes §7.4.3.5, non-primitive element) and each element carries its own
// @appendable DHEADER. Vendor-anchored to FastDDS (= ZeroDDS default).
// ---------------------------------------------------------------------------
#[test]
fn v_array_of_appendable_struct() {
    let idl = r#"@appendable struct Pt { long x; long y; };
@appendable struct Arr { Pt shape[2]; };"#;
    let body = r#"    ::Arr a;
    a.shape()[0].x(1); a.shape()[0].y(2);
    a.shape()[1].x(3); a.shape()[1].y(4);
    __buf = ::dds::topic::topic_type_support<::Arr>::encode(a);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-arr-append-struct, no C++ compiler");
        return;
    };
    let expected: Vec<u8> = vec![
        0x1C, 0x00, 0x00, 0x00, // Arr DHEADER = 28
        0x18, 0x00, 0x00, 0x00, // shape[] collection DHEADER = 24 (non-primitive elem)
        0x08, 0x00, 0x00, 0x00, // Pt[0] @appendable DHEADER = 8
        0x01, 0x00, 0x00, 0x00, // x = 1
        0x02, 0x00, 0x00, 0x00, // y = 2
        0x08, 0x00, 0x00, 0x00, // Pt[1] @appendable DHEADER = 8
        0x03, 0x00, 0x00, 0x00, // x = 3
        0x04, 0x00, 0x00, 0x00, // y = 4
    ];
    assert_bytes("V-arr-append-struct", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-bitbound-enum — a `@bit_bound(16)` enum is serialized at its declared holder
// width (int16, 2 bytes), NOT the default 32-bit int. REGRESSION GATE for T2
// (narrow-encode, spec §7.4.5.1, vendor-confirmed vs CycloneDDS which honours
// @bit_bound). Without the fix this member would be 4 bytes.
// ---------------------------------------------------------------------------
#[test]
fn v_bit_bound_enum_narrow_width() {
    let idl = r#"@bit_bound(16) enum E16 { A, B, C };
@final struct S { E16 e; };"#;
    let body = r#"    ::S s;
    s.e(::E16::B);
    __buf = ::dds::topic::topic_type_support<::S>::encode(s);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-bitbound-enum, no C++ compiler");
        return;
    };
    // @final struct -> no DHEADER; e = B = 1 as int16 LE = 2 bytes.
    assert_bytes("V-bitbound-enum", &bytes, &[0x01, 0x00]);
}

// ---------------------------------------------------------------------------
// V-union-final — a @final union is `discriminator + selected branch`, NO
// DHEADER (XTypes §7.4.4.5 / rule 26). REGRESSION GATE for union wire framing
// (the corpus only exercised union inside combo::Telemetry — no standalone gate).
// Vendor-anchored: union byte-identical to Cyclone/RTI/FastDDS (oracle W2-A).
// ---------------------------------------------------------------------------
#[test]
fn v_union_final_selected_branch() {
    let idl =
        r#"@final union U switch(long) { case 1: long a; case 2: double b; default: octet c; };"#;
    let body = r#"    ::U u;
    u._d(1);
    u.value() = static_cast<int32_t>(42);
    __buf = ::dds::topic::topic_type_support<::U>::encode(u);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-union-final, no C++ compiler");
        return;
    };
    // disc (long) = 1, then the selected branch `a` (long) = 42. No DHEADER (@final).
    let expected: Vec<u8> = vec![
        0x01, 0x00, 0x00, 0x00, // discriminator = 1
        0x2A, 0x00, 0x00, 0x00, // a = 42
    ];
    assert_bytes("V-union-final", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-union-append — an @appendable union (the SX2 default for an unannotated
// union) prepends a DHEADER over [discriminator + branch]. GATE so the SX2
// extensibility default can't silently change union framing.
// ---------------------------------------------------------------------------
#[test]
fn v_union_appendable_has_dheader() {
    let idl = r#"@appendable union U2 switch(long) { case 1: long a; default: octet c; };"#;
    let body = r#"    ::U2 u;
    u._d(1);
    u.value() = static_cast<int32_t>(7);
    __buf = ::dds::topic::topic_type_support<::U2>::encode(u);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-union-append, no C++ compiler");
        return;
    };
    // DHEADER = body length = 8 (disc 4 + a 4), then disc=1, a=7.
    let expected: Vec<u8> = vec![
        0x08, 0x00, 0x00, 0x00, // DHEADER = 8
        0x01, 0x00, 0x00, 0x00, // discriminator = 1
        0x07, 0x00, 0x00, 0x00, // a = 7
    ];
    assert_bytes("V-union-append", &bytes, &expected);
}

// ---------------------------------------------------------------------------
// V-union-of-struct + V-seq-of-union — compound union framing, both verified
// byte-identical to CycloneDDS (@appendable). GATEs so the union DHEADER fix +
// the nested @appendable-struct/sequence splices can't regress together.
// ---------------------------------------------------------------------------
#[test]
fn v_union_of_appendable_struct() {
    let idl = r#"@appendable struct Pt2 { long x; long y; };
@appendable union U3 switch(long) { case 1: Pt2 p; default: octet z; };"#;
    let body = r#"    ::U3 u; u._d(1);
    ::Pt2 p; p.x(5); p.y(6); u.value() = p;
    __buf = ::dds::topic::topic_type_support<::U3>::encode(u);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-union-of-struct, no C++ compiler");
        return;
    };
    // U3 DHEADER(16) + disc(1) + Pt2 DHEADER(8) + x(5) + y(6) = 20 B (== Cyclone).
    assert_bytes(
        "V-union-of-struct",
        &bytes,
        &[
            0x10, 0, 0, 0, 0x01, 0, 0, 0, 0x08, 0, 0, 0, 0x05, 0, 0, 0, 0x06, 0, 0, 0,
        ],
    );
}

#[test]
fn v_sequence_of_union() {
    let idl = r#"@appendable union U2 switch(long) { case 1: long a; default: octet z; };
@appendable struct S2 { sequence<U2> s; };"#;
    let body = r#"    ::S2 s2; ::U2 u; u._d(1); u.value() = static_cast<int32_t>(7);
    s2.s().push_back(u);
    __buf = ::dds::topic::topic_type_support<::S2>::encode(s2);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-seq-of-union, no C++ compiler");
        return;
    };
    // S2 DHEADER(20) + seq DHEADER(16) + count(1) + U2 DHEADER(8) + disc(1) + a(7) = 24 B (== Cyclone).
    assert_bytes(
        "V-seq-of-union",
        &bytes,
        &[
            0x14, 0, 0, 0, 0x10, 0, 0, 0, 0x01, 0, 0, 0, 0x08, 0, 0, 0, 0x01, 0, 0, 0, 0x07, 0, 0,
            0,
        ],
    );
}

// ---------------------------------------------------------------------------
// Cross-extensibility nesting + mixed @id/auto-id — all verified byte-identical
// to CycloneDDS. GATEs lock the splice paths + the auto-id @id-reset together.
// ---------------------------------------------------------------------------
#[test]
fn v_mutable_in_appendable() {
    let idl = r#"@mutable struct Inner { long a; }; @appendable struct Outer { Inner i; };"#;
    let body = r#"    ::Outer o; ::Inner in_; in_.a(42); o.i(in_);
    __buf = ::dds::topic::topic_type_support<::Outer>::encode(o);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mut-in-append");
        return;
    };
    // Outer DHEADER(12) + Inner @mutable DHEADER(8) + EMHEADER(id0,LC2) + a(42) = 16 B (== Cyclone).
    assert_bytes(
        "V-mut-in-append",
        &bytes,
        &[
            0x0c, 0, 0, 0, 0x08, 0, 0, 0, 0x00, 0, 0, 0x20, 0x2a, 0, 0, 0,
        ],
    );
}

#[test]
fn v_appendable_in_final() {
    let idl = r#"@appendable struct Inner2 { long a; }; @final struct Outer2 { Inner2 i; };"#;
    let body = r#"    ::Outer2 o; ::Inner2 in_; in_.a(42); o.i(in_);
    __buf = ::dds::topic::topic_type_support<::Outer2>::encode(o);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-append-in-final");
        return;
    };
    // Outer2 @final -> NO DHEADER; Inner2 @appendable DHEADER(4) + a(42) = 8 B (== Cyclone).
    assert_bytes("V-append-in-final", &bytes, &[0x04, 0, 0, 0, 0x2a, 0, 0, 0]);
}

#[test]
fn v_mutable_mixed_explicit_and_auto_id() {
    // @id(5) sets the id and RESETS the sequential counter to 6, so the next
    // auto-id member is 6 (not 1, not a name-hash). Vendor-confirmed vs Cyclone.
    let idl = r#"@mutable struct Mx { @id(5) long a; long b; };"#;
    let body = r#"    ::Mx m; m.a(1); m.b(2);
    __buf = ::dds::topic::topic_type_support<::Mx>::encode(m);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-mixed-id");
        return;
    };
    // DHEADER(16) + EMHEADER(id5,LC2)+a(1) + EMHEADER(id6,LC2)+b(2) = 20 B.
    assert_bytes(
        "V-mixed-id",
        &bytes,
        &[
            0x10, 0, 0, 0, 0x05, 0, 0, 0x20, 0x01, 0, 0, 0, 0x06, 0, 0, 0x20, 0x02, 0, 0, 0,
        ],
    );
}

// ---------------------------------------------------------------------------
// V-map-after-enum — a map<K,V> DHEADER must be 4-aligned even when preceded by
// a sub-4-byte member (@bit_bound(16) enum). REGRESSION GATE for the map/seq
// DHEADER alignment bug surfaced by the MapEnum cross-PSM probe.
// ---------------------------------------------------------------------------
#[test]
fn v_map_dheader_aligned_after_bitbound_enum() {
    let idl = r#"@bit_bound(16) enum E16 { A, B, C };
@appendable struct EM { E16 e; map<long,long> m; };"#;
    let body = r#"    ::EM x; x.e(::E16::B);
    std::map<int32_t,int32_t> mm; mm[5] = 7; x.m(mm);
    __buf = ::dds::topic::topic_type_support<::EM>::encode(x);
"#;
    let Some(bytes) = run_encode(idl, body) else {
        eprintln!("WARNING: skipping V-map-after-enum, no C++ compiler");
        return;
    };
    // MapPrim fix (d41cfb33): map<long,long> carries NO inner DHEADER. So the EM
    // DHEADER(16) + e(0x0001 int16) + 2-byte PAD (4-align the map count) + count(1)
    // + key(5) + val(7). The pad still lands the primitive map count on a 4-byte
    // boundary — the alignment this test guards — just without the spurious DHEADER.
    assert_bytes(
        "V-map-after-enum",
        &bytes,
        &[
            0x10, 0, 0, 0, 0x01, 0x00, 0x00, 0x00, 0x01, 0, 0, 0, 0x05, 0, 0, 0, 0x07, 0, 0, 0,
        ],
    );
}
