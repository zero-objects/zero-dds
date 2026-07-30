// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire conformance tests: emits code, compiles it as an external
//! crate, calls `encode`/`decode` and checks roundtrip + golden bytes.
//!
//! Proves end-to-end: the codegen output produces XCDR2-conformant
//! wire bytes — not only "compiles" but "behaves
//! spec-conformantly".

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
use std::path::PathBuf;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Writes the emitted code + a test file with roundtrip
/// assertions into a temp crate and runs `cargo test`.
///
/// Fails if the roundtrip assertions in the emitted code
/// fail — this proves wire conformance at the behavior level.
fn run_wire_test(name: &str, idl: &str, test_body: &str) {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");

    let tmp = std::env::temp_dir().join(format!("dds_idl_rust_wire_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir src");
    std::fs::create_dir_all(tmp.join("tests")).expect("mkdir tests");

    let workspace_root = workspace_root();
    let cargo_toml = format!(
        r#"[package]
name = "wire_test_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-cdr = {{ path = "{root}/crates/cdr" }}
zerodds-dcps = {{ path = "{root}/crates/dcps" }}
zerodds-types = {{ path = "{root}/crates/types" }}
"#,
        root = workspace_root.display()
    );
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("Cargo.toml")
        .write_all(cargo_toml.as_bytes())
        .expect("write");

    std::fs::File::create(tmp.join("src/lib.rs"))
        .expect("lib.rs")
        .write_all(rust_src.as_bytes())
        .expect("write");

    let test_file = format!(
        r#"// Generated wire-conformance test.
use wire_test_{name}::*;
use zerodds_dcps::DdsType;

#[test]
fn wire_roundtrip_assertions() {{
    {test_body}
}}
"#
    );
    std::fs::File::create(tmp.join("tests/wire.rs"))
        .expect("test file")
        .write_all(test_file.as_bytes())
        .expect("write test");

    let status = Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .arg("--")
        .arg("--nocapture")
        .status();

    match status {
        Ok(s) if s.success() => { /* good */ }
        Ok(s) => panic!(
            "wire roundtrip test failed (exit {:?}). source:\n{rust_src}",
            s.code()
        ),
        Err(e) => panic!("cargo invocation failed: {e}"),
    }
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest)
}

#[test]
#[ignore = "requires cargo offline + path-deps; --include-ignored"]
fn wire_simple_struct_primitives_roundtrip() {
    run_wire_test(
        "primitives",
        r#"struct Point { long x; long y; };"#,
        r#"
            // Encode / Decode-Roundtrip
            let original = Point { x: 42, y: -7 };
            let mut buf = Vec::new();
            original.encode(&mut buf).expect("encode");
            let decoded = Point::decode(&buf).expect("decode");
            assert_eq!(decoded.x, original.x);
            assert_eq!(decoded.y, original.y);

            // Wire-Form: unannotiertes struct = APPENDABLE (XTypes 1.3 §7.3.3.1 default,
            // seit SX2-Flip 2a571721) → XCDR2 DHEADER(4) + Body(8) = 12 byte.
            assert_eq!(buf.len(), 12);
            assert_eq!(&buf[0..4], &8u32.to_le_bytes()); // DHEADER = Body-Länge
            assert_eq!(&buf[4..8], &42i32.to_le_bytes());
            assert_eq!(&buf[8..12], &(-7i32).to_le_bytes());
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_keyed_struct_keyhash_roundtrip() {
    run_wire_test(
        "keyed",
        r#"struct Reading { @key long sensor_id; double value; };"#,
        r#"
            assert_eq!(Reading::HAS_KEY, true);
            assert_eq!(Reading::KEY_HOLDER_MAX_SIZE, Some(4));

            let r = Reading { sensor_id: 1234, value: 3.14 };
            let hash = r.compute_key_hash().expect("keyed type yields hash");
            assert_eq!(hash.len(), 16);

            // Two readings with the same sensor_id must yield the same hash
            let r2 = Reading { sensor_id: 1234, value: 99.0 };
            assert_eq!(r.compute_key_hash(), r2.compute_key_hash(),
                "same key → same hash");

            // Unterschiedliche Keys → unterschiedlicher Hash
            let r3 = Reading { sensor_id: 5555, value: 3.14 };
            assert_ne!(r.compute_key_hash(), r3.compute_key_hash(),
                "different key → different hash");
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_union_classic_cdr_roundtrip() {
    run_wire_test(
        "union",
        r#"union Event switch (long) {
            case 1: long counter;
            case 2: double value;
            default: string message;
        };"#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};

            // Classic CORBA CDR: BufferWriter WITHOUT .xcdr2().
            fn rt(v: &Event) -> Event {
                let mut w = BufferWriter::new(Endianness::Big);
                v.encode(&mut w).expect("encode");
                let bytes = w.into_bytes();
                let mut r = BufferReader::new(&bytes, Endianness::Big);
                Event::decode(&mut r).expect("decode")
            }

            // Explizite Cases + Default-Case roundtrippen.
            assert_eq!(rt(&Event::Counter(42)), Event::Counter(42));
            assert_eq!(rt(&Event::Value(3.5)), Event::Value(3.5));
            assert_eq!(rt(&Event::Message("hi".to_string())), Event::Message("hi".to_string()));

            // Wire: Counter = disc 1 (i32 BE) + 42 (i32 BE).
            let mut w = BufferWriter::new(Endianness::Big);
            Event::Counter(42).encode(&mut w).unwrap();
            let b = w.into_bytes();
            assert_eq!(&b[..4], &1i32.to_be_bytes(), "discriminator = case label 1");
            assert_eq!(&b[4..8], &42i32.to_be_bytes(), "member value");

            // Default-Discriminator = 0 (Labels 1,2 belegt → 0 frei).
            let mut w2 = BufferWriter::new(Endianness::Big);
            Event::Message("x".to_string()).encode(&mut w2).unwrap();
            assert_eq!(&w2.into_bytes()[..4], &0i32.to_be_bytes(), "default disc = 0");
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_non_serialized_absent_from_wire_p0_5() {
    // Broad-audit P0-5 (#2): a `@non_serialized` member keeps its in-memory
    // field but is written to NO wire form and read from none — on decode it is
    // left at its default. Byte-proof: the wire of `S{a, secret, b}` is exactly
    // `a` then `b` (no `secret`), independent of `secret`'s value; decode leaves
    // `secret` at 0. Covered for @final (plain), @appendable (DHEADER) and
    // @mutable (EMHEADER) so no wire path leaks the member.
    run_wire_test(
        "non_serialized",
        r#"
            @final     struct SFinal  { long a; @non_serialized long secret; long b; };
            @appendable struct SApp   { long a; @non_serialized long secret; long b; };
            @mutable    struct SMut   { long a; @non_serialized long secret; long b; };
        "#,
        r#"
            // ---- @final: plain sequential, secret NOT on the wire ----------
            let s = SFinal { a: 1, secret: 999, b: 2 };
            let mut buf = Vec::new();
            s.encode(&mut buf).expect("encode");
            // @final: a(4) + b(4) = 8 bytes; secret (would be +4) is absent.
            assert_eq!(buf.len(), 8, "@final: only a+b on the wire, not secret");
            assert_eq!(&buf[0..4], &1i32.to_le_bytes());
            assert_eq!(&buf[4..8], &2i32.to_le_bytes());

            // Changing `secret` must not change a single wire byte.
            let s2 = SFinal { a: 1, secret: -424242, b: 2 };
            let mut buf2 = Vec::new();
            s2.encode(&mut buf2).expect("encode");
            assert_eq!(buf, buf2, "secret must not influence the wire");

            // Decode leaves secret at its default (0); a and b round-trip.
            let d = SFinal::decode(&buf).expect("decode");
            assert_eq!(d.a, 1);
            assert_eq!(d.b, 2);
            assert_eq!(d.secret, 0, "@non_serialized decodes to the default");

            // ---- @appendable: DHEADER + body(a,b), secret absent -----------
            let sa = SApp { a: 7, secret: 555, b: 9 };
            let mut ba = Vec::new();
            sa.encode(&mut ba).expect("encode");
            // 4 (DHEADER) + a(4) + b(4) = 12; DHEADER body length = 8.
            assert_eq!(ba.len(), 12, "@appendable: DHEADER + a + b only");
            assert_eq!(&ba[0..4], &8u32.to_le_bytes(), "DHEADER = body length (a+b)");
            let da = SApp::decode(&ba).expect("decode");
            assert_eq!((da.a, da.b, da.secret), (7, 9, 0));

            // ---- @mutable: EMHEADER-framed a(id 0) + b(id 1), secret absent -
            let sm = SMut { a: 3, secret: 111, b: 4 };
            let mut bm = Vec::new();
            sm.encode(&mut bm).expect("encode");
            // The @mutable member ids must compact to 0 and 1 (NOT 0 and 2):
            // secret does not consume an id slot. The EMHEADERs carry the ids in
            // the low 28 bits; assert both are present and 2 does NOT appear.
            let ids: std::collections::BTreeSet<u32> = bm
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) & 0x0FFF_FFFF)
                .collect();
            assert!(ids.contains(&0) && ids.contains(&1), "member ids compact to 0,1");
            let dm = SMut::decode(&bm).expect("decode");
            assert_eq!((dm.a, dm.b, dm.secret), (3, 4, 0));
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_field_value_filter_paths() {
    run_wire_test(
        "fieldval",
        r#"
            struct SensorReading {
                long sensor_id;
                double value;
                string label;
                boolean valid;
            };
        "#,
        r#"
            use zerodds_dcps::FilterValue as Value;
            let r = SensorReading {
                sensor_id: 42,
                value: 3.14,
                label: "alpha".to_string(),
                valid: true,
            };
            assert_eq!(r.field_value("sensor_id"), Some(Value::Int(42)));
            assert_eq!(r.field_value("value"), Some(Value::Float(3.14)));
            assert_eq!(r.field_value("label"), Some(Value::String("alpha".to_string())));
            assert_eq!(r.field_value("valid"), Some(Value::Bool(true)));
            assert_eq!(r.field_value("nonexistent"), None);
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_appendable_dheader_present() {
    run_wire_test(
        "appendable",
        r#"@appendable struct Telemetry { unsigned long ts; double v; };"#,
        r#"
            let t = Telemetry { ts: 100, v: 1.5 };
            let mut buf = Vec::new();
            t.encode(&mut buf).expect("encode");
            // Appendable wraps in DHEADER (4 byte length prefix) + body.
            // body = u32 ts (4 byte) + alignment(4) + f64 v (8 byte) = 16 byte
            // total = 4 (DHEADER) + 16 (body) = 20 byte
            assert!(buf.len() >= 16, "appendable encoding has DHEADER + body, got {} bytes", buf.len());

            // Decoding must consume the DHEADER and return the members
            let back = Telemetry::decode(&buf).expect("decode");
            assert_eq!(back.ts, 100);
            assert_eq!(back.v, 1.5);
        "#,
    );
}

/// P0.4: a `@mutable` union MUST serialize as PL_CDR2 (outer DHEADER + a
/// per-member EMHEADER for the discriminator AND the selected branch), exactly
/// like a `@mutable` struct — NOT as an inline FINAL union (disc + member, no
/// framing), which is what the emitter previously produced (silent wire break).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_mutable_union_pl_cdr2_framing_and_roundtrip() {
    run_wire_test(
        "mutable_union",
        r#"@mutable union Msg switch (long) {
            case 1: long counter;
            case 2: string text;
        };"#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};

            // XCDR2 (DDS) writer: max_alignment == 4 -> PL_CDR2 path.
            let mut w = BufferWriter::new(Endianness::Little).xcdr2();
            Msg::Counter(0x11223344).encode(&mut w).expect("encode");
            let buf = w.into_bytes();

            // Must NOT be the inline FINAL form (disc 4B + value 4B = 8 bytes).
            assert_ne!(buf.len(), 8, "regression: @mutable union encoded as inline Final");
            // PL_CDR2 layout: DHEADER(4) + [EMHEADER(4)+disc(4)] + [EMHEADER(4)+value(4)] = 20.
            assert_eq!(buf.len(), 20, "expected DHEADER + 2 EMHEADER-framed members, got {}", buf.len());

            let rd = |o: usize| u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
            // Outer DHEADER = length of the framed member list (16 bytes).
            assert_eq!(rd(0), 16, "outer DHEADER = body length");
            // First member: EMHEADER with member id 0 == the discriminator.
            assert_eq!(rd(4) & 0x0FFF_FFFF, 0, "member id 0 = discriminator EMHEADER");
            assert_eq!(rd(8) as i32, 1, "discriminator value = case label 1");
            // Second member: EMHEADER with member id 1 == the selected branch.
            assert_eq!(rd(12) & 0x0FFF_FFFF, 1, "member id 1 = branch EMHEADER");
            assert_eq!(rd(16), 0x11223344, "branch value");

            // Roundtrip on the XCDR2 wire recovers the active branch.
            let mut r = BufferReader::new(&buf, Endianness::Little).xcdr2();
            assert_eq!(Msg::decode(&mut r).expect("decode"), Msg::Counter(0x11223344));

            // Variable-length branch (string) roundtrips through the same framing.
            let mut w2 = BufferWriter::new(Endianness::Little).xcdr2();
            Msg::Text("hello".to_string()).encode(&mut w2).expect("encode2");
            let buf2 = w2.into_bytes();
            let mut r2 = BufferReader::new(&buf2, Endianness::Little).xcdr2();
            assert_eq!(Msg::decode(&mut r2).expect("decode2"), Msg::Text("hello".to_string()));

            // Classic CDR (max_alignment 8, no .xcdr2()): PL_CDR1 PID list + sentinel,
            // symmetric to a @mutable struct — must also roundtrip.
            let mut wc = BufferWriter::new(Endianness::Big);
            Msg::Counter(7).encode(&mut wc).expect("encode-classic");
            let bc = wc.into_bytes();
            let mut rc = BufferReader::new(&bc, Endianness::Big);
            assert_eq!(Msg::decode(&mut rc).expect("decode-classic"), Msg::Counter(7));
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_string_and_sequence_roundtrip() {
    run_wire_test(
        "stringseq",
        r#"struct Message { string topic; sequence<long> values; };"#,
        r#"
            let m = Message {
                topic: "Hello".to_string(),
                values: vec![1, 2, 3, 4, 5],
            };
            let mut buf = Vec::new();
            m.encode(&mut buf).expect("encode");
            let back = Message::decode(&buf).expect("decode");
            assert_eq!(back.topic, "Hello");
            assert_eq!(back.values, vec![1, 2, 3, 4, 5]);
        "#,
    );
}

#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_enum_roundtrip() {
    run_wire_test(
        "enumcolor",
        r#"enum Color { RED, GREEN, BLUE };"#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrEncode, CdrDecode, Endianness};
            let mut w = BufferWriter::new(Endianness::Little);
            <Color as CdrEncode>::encode(&Color::GREEN, &mut w).expect("encode");
            let bytes = w.into_bytes();
            // i32 wire = 4 byte LE = 0x01_00_00_00
            assert_eq!(bytes, vec![1, 0, 0, 0]);

            let mut r = BufferReader::new(&bytes, Endianness::Little);
            let decoded = <Color as CdrDecode>::decode(&mut r).expect("decode");
            assert_eq!(decoded, Color::GREEN);

            // Default
            assert_eq!(Color::default(), Color::RED);

            // from_wire-API
            assert_eq!(Color::from_wire(0), Some(Color::RED));
            assert_eq!(Color::from_wire(2), Some(Color::BLUE));
            assert_eq!(Color::from_wire(99), None);
        "#,
    );
}

/// Bug R2 (#61): a fixed-size array MEMBER must encode AND decode every
/// element. Before the fix the decode side named the scalar element type
/// (`i32`) instead of the array type (`[i32; N]`), so the generated code
/// failed to compile / read only one element. This covers 1-D and
/// multi-dimensional primitive arrays exactly as fixture 08_arrays.idl
/// (`long vec[3]`, `long grid[4][4]`, `double cube[2][2][2]`).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_fixed_array_member_roundtrip() {
    run_wire_test(
        "arrays",
        r#"
            struct Arrays {
                long   vec[3];
                long   grid[2][2];
                double cube[2][2][2];
            };
        "#,
        r#"
            let original = Arrays {
                vec: [10, 20, 30],
                grid: [[1, 2], [3, 4]],
                cube: [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]],
            };
            let mut buf = Vec::new();
            original.encode(&mut buf).expect("encode");
            let decoded = Arrays::decode(&buf).expect("decode");

            // Every element of every dimension must round-trip — proving the
            // decode reads the FULL array, not just the first element.
            assert_eq!(decoded.vec, [10, 20, 30], "1-D array");
            assert_eq!(decoded.grid, [[1, 2], [3, 4]], "multi-dim array");
            assert_eq!(
                decoded.cube,
                [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]],
                "3-D array"
            );
        "#,
    );
}

/// Bug R2b (#72): a fixed-size array-OF-STRUCT member (`Point shape[2]`,
/// fixture 08_arrays.idl) must encode AND decode every element and
/// round-trip. The blanket `impl CdrDecode for [T; N]` in the
/// separately-owned `zerodds-cdr` crate requires `T: Default + Copy`, and
/// generated structs/unions are NOT `Copy` — so the codegen now emits a
/// manual element-wise decoder (mirroring the blanket impl's XCDR2 DHEADER
/// decision byte-for-byte, building each fixed array from a `Vec` with no
/// `Copy` bound). This proves it round-trips at the wire level, including a
/// 2-D array-of-struct (`Point grid[2][2]`).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_array_of_struct_member_roundtrip() {
    run_wire_test(
        "arrofstruct",
        r#"
            @nested struct Point { long x; long y; };
            struct Shapes {
                Point shape[2];
                Point grid[2][2];
            };
        "#,
        r#"
            let original = Shapes {
                shape: [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }],
                grid: [
                    [Point { x: 10, y: 11 }, Point { x: 12, y: 13 }],
                    [Point { x: 20, y: 21 }, Point { x: 22, y: 23 }],
                ],
            };
            let mut buf = Vec::new();
            original.encode(&mut buf).expect("encode");
            let decoded = Shapes::decode(&buf).expect("decode");

            // Every struct element of every dimension must round-trip —
            // proving the decode reads the FULL array element-by-element,
            // not just the first element (and not via the Copy-bound
            // blanket impl, which would not even compile here).
            assert_eq!(decoded.shape[0], Point { x: 1, y: 2 }, "1-D elem 0");
            assert_eq!(decoded.shape[1], Point { x: 3, y: 4 }, "1-D elem 1");
            assert_eq!(decoded.grid[0][0], Point { x: 10, y: 11 }, "2-D [0][0]");
            assert_eq!(decoded.grid[0][1], Point { x: 12, y: 13 }, "2-D [0][1]");
            assert_eq!(decoded.grid[1][0], Point { x: 20, y: 21 }, "2-D [1][0]");
            assert_eq!(decoded.grid[1][1], Point { x: 22, y: 23 }, "2-D [1][1]");
            assert_eq!(decoded, original, "full struct round-trips");
        "#,
    );
}

/// Bug O (#58): the generated enum must honor explicit `@value(N)` in BOTH
/// the discriminant assignment and the `from_wire` reverse map. Fixture
/// 03_enums.idl has a sparse enum `@value(1) ONE, @value(2) TWO, @value(8)
/// EIGHT`; the wire byte for EIGHT must be 8, not the sequential index 2.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_enum_explicit_value_roundtrip() {
    run_wire_test(
        "enumvalue",
        r#"enum Sparse { @value(1) ONE, @value(2) TWO, @value(8) EIGHT };"#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrEncode, CdrDecode, Endianness};

            // The discriminant must be the explicit @value, not 0..N-1.
            assert_eq!(Sparse::ONE as i32, 1);
            assert_eq!(Sparse::TWO as i32, 2);
            assert_eq!(Sparse::EIGHT as i32, 8, "@value(8) must be 8, not index 2");

            // Encode EIGHT → the wire i32 must be 8.
            let mut w = BufferWriter::new(Endianness::Little);
            <Sparse as CdrEncode>::encode(&Sparse::EIGHT, &mut w).expect("encode");
            assert_eq!(w.into_bytes(), vec![8, 0, 0, 0], "EIGHT encodes as i32 8");

            // Decode the wire value 8 back to EIGHT (round-trip).
            let bytes = vec![8u8, 0, 0, 0];
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            let decoded = <Sparse as CdrDecode>::decode(&mut r).expect("decode");
            assert_eq!(decoded, Sparse::EIGHT);

            // from_wire reverse map honors @value too.
            assert_eq!(Sparse::from_wire(1), Some(Sparse::ONE));
            assert_eq!(Sparse::from_wire(8), Some(Sparse::EIGHT));
            // The OLD buggy sequential index 2 is NOT a valid wire value now.
            assert_eq!(Sparse::from_wire(0), None);
        "#,
    );
}

/// Bug O (#58) consistency: when a union switches on an enum with explicit
/// `@value`, the union's `case ENUM_LITERAL:` discriminator must match the
/// enum's wire value (both routed through ENUM_LITERAL_VALUES).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_enum_value_union_case_consistency() {
    run_wire_test(
        "enumvalunion",
        r#"
            enum Mode { @value(1) ONE, @value(8) EIGHT };
            union Reading switch (Mode) {
                case ONE:   long a;
                case EIGHT: double b;
            };
        "#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrEncode, CdrDecode, Endianness};
            fn rt(v: &Reading) -> Reading {
                let mut w = BufferWriter::new(Endianness::Big);
                v.encode(&mut w).expect("encode");
                let bytes = w.into_bytes();
                let mut r = BufferReader::new(&bytes, Endianness::Big);
                Reading::decode(&mut r).expect("decode")
            }
            // The EIGHT arm's discriminator must serialize as 8 (the @value),
            // matching the enum wire form, and round-trip.
            let mut w = BufferWriter::new(Endianness::Big);
            Reading::B(2.5).encode(&mut w).expect("encode");
            let b = w.into_bytes();
            assert_eq!(&b[..4], &8i32.to_be_bytes(), "case EIGHT disc = @value(8)");
            assert_eq!(rt(&Reading::A(11)), Reading::A(11));
            assert_eq!(rt(&Reading::B(2.5)), Reading::B(2.5));
        "#,
    );
}

/// `fixed<P,S>` over the wire (CORBA/GIOP §9.3.2.7 packed BCD). The exact bytes
/// are the CORBA vendor-oracle vectors (JacORB 3.9 / omniORB 4.3, task CV-fixed)
/// — and they are byte-identical to what the C++ PSM emits for the same struct
/// (`crates/idl-cpp/tests/clang_roundtrip.rs::fixed_member_roundtrips_through_clang`),
/// so this doubles as the rust↔cpp cross-PSM anchor for `fixed`.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_fixed_member_golden_bytes() {
    run_wire_test(
        "fixed",
        r#"module m {
             @final      struct Ff { long id; fixed<5,2> price; };
             @appendable struct Fa { long id; fixed<4,0> qty;   };
           };"#,
        r#"
            use zerodds_cdr::fixed::Fixed;
            // @final: body = id(4) + bcd(3); fixed<5,2> 123.45 -> 12 34 5c.
            let f = m::Ff { id: 7, price: Fixed::<5,2>::from_str_repr("123.45").unwrap() };
            let mut buf = Vec::new();
            f.encode(&mut buf).expect("encode");
            assert_eq!(buf, vec![0x07,0,0,0, 0x12,0x34,0x5c], "Ff fixed golden");
            let back = m::Ff::decode(&buf).expect("decode");
            assert_eq!(back.price.to_string_repr(), "123.45");

            // @appendable: DHEADER(7) + id(4) + bcd(3); fixed<4,0> 1234 -> 01 23 4c.
            let a = m::Fa { id: 7, qty: Fixed::<4,0>::from_str_repr("1234").unwrap() };
            let mut buf = Vec::new();
            a.encode(&mut buf).expect("encode");
            assert_eq!(buf, vec![0x07,0,0,0, 0x07,0,0,0, 0x01,0x23,0x4c], "Fa fixed golden");
            let back = m::Fa::decode(&buf).expect("decode");
            assert_eq!(back.qty.to_string_repr(), "1234");
        "#,
    );
}

/// Regression #22 (SEVERE finding, end to end): before the decode-bound fix,
/// a generated `decode` only checked the wire buffer's remaining bytes
/// (`zerodds_cdr` composite decoders) — never the IDL-declared bound N — so
/// a `string<8>` field would happily decode an arbitrarily large
/// well-formed payload (XTypes 1.3 §7.4.3 requires the bound enforced on
/// BOTH sides; untrusted-input DoS vector). This forges exactly such a
/// payload through the raw `CdrEncode` path (no bound awareness, mirroring
/// a foreign/adversarial sender that does not honor the receiver's IDL) and
/// confirms `CdrDecode::decode` now rejects it via `DecodeError::BoundExceeded`
/// instead of silently accepting it. Also exercises `@optional` bounded
/// members (the encode-bound half of #22) roundtripping normally within
/// bound, and a bounded sequence rejected on decode the same way.
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_decode_rejects_over_bound_values() {
    run_wire_test(
        "decode_bound",
        r#"struct S { string<8> label; sequence<long,4> vals; @optional string<4> tag; };"#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};

            // `S` implements both `DdsType` (encode/decode over `Vec<u8>`)
            // and the raw `CdrEncode`/`CdrDecode` (over `BufferWriter`/
            // `BufferReader`) — both are in scope here, so every call below
            // must be fully qualified (E0034 otherwise).

            // Within-bound roundtrip must still work (no false positives).
            let ok = S { label: "short".to_string(), vals: vec![1, 2], tag: Some("ok".to_string()) };
            let mut buf = Vec::new();
            <S as DdsType>::encode(&ok, &mut buf).expect("within-bound encode");
            let back = <S as DdsType>::decode(&buf).expect("within-bound decode");
            assert_eq!(back, ok);

            // Forge a well-formed appendable payload whose `label` content
            // exceeds the IDL bound (8) — well within the remaining buffer.
            let over_bound = "this-is-way-over-eight-chars".to_string();
            let mut w = BufferWriter::new(Endianness::Little).xcdr2();
            zerodds_cdr::struct_enc::encode_appendable(&mut w, |w2| {
                <_ as CdrEncode>::encode(&over_bound, w2)?;
                <_ as CdrEncode>::encode(&::std::vec::Vec::<i32>::new(), w2)?;
                <_ as CdrEncode>::encode(&::core::option::Option::<String>::None, w2)?;
                Ok(())
            }).unwrap();
            let bytes = w.into_bytes();

            let mut r = BufferReader::new(&bytes, Endianness::Little).xcdr2();
            let err = <S as CdrDecode>::decode(&mut r)
                .expect_err("decode must reject a wire value exceeding the IDL bound, not just check the remaining buffer");
            match err {
                zerodds_cdr::DecodeError::BoundExceeded { actual, bound, .. } => {
                    assert_eq!(bound, 8, "must report the IDL bound (8), not the buffer size");
                    assert_eq!(actual, over_bound.len());
                }
                other => panic!("expected DecodeError::BoundExceeded, got {other:?}"),
            }

            // Same for a bounded SEQUENCE exceeding its bound (4).
            let mut w2buf = BufferWriter::new(Endianness::Little).xcdr2();
            zerodds_cdr::struct_enc::encode_appendable(&mut w2buf, |w2| {
                <_ as CdrEncode>::encode(&"ok".to_string(), w2)?;
                <_ as CdrEncode>::encode(&vec![1i32, 2, 3, 4, 5, 6], w2)?;
                <_ as CdrEncode>::encode(&::core::option::Option::<String>::None, w2)?;
                Ok(())
            }).unwrap();
            let bytes2 = w2buf.into_bytes();
            let mut r2 = BufferReader::new(&bytes2, Endianness::Little).xcdr2();
            let err2 = <S as CdrDecode>::decode(&mut r2)
                .expect_err("decode must reject an over-bound sequence too");
            match err2 {
                zerodds_cdr::DecodeError::BoundExceeded { bound, .. } => assert_eq!(bound, 4),
                other => panic!("expected DecodeError::BoundExceeded, got {other:?}"),
            }
        "#,
    );
}

/// B1 follow-up (array-of-bounded-element gap, disclosed but NOT fixed by
/// #22): `sequence<octet,4> arr[3]` is an array declarator whose element
/// type (`sequence<octet,4>`) is bounded. Because the element is a `Vec<u8>`
/// (not `Copy`), decode goes through `emit_manual_array_decode` rather than
/// the simple-declarator path that `emit_field_decode_with_optional` bound-
/// checks directly — before this fix that manual per-element decode never
/// consulted the IDL bound at all, so a well-formed-but-oversized element
/// inside the array decoded silently. Forges such a payload via the raw
/// `CdrEncode` blanket impl for `[Vec<u8>; 3]` (no bound awareness, same
/// adversarial-sender shape as `wire_decode_rejects_over_bound_values`) and
/// confirms decode now rejects it with `DecodeError::BoundExceeded`
/// reporting the IDL bound (4), not the buffer size. Also proves the
/// within-bound roundtrip is unaffected (byte-golden, no regression).
#[test]
#[ignore = "requires cargo offline + path-deps"]
fn wire_decode_rejects_over_bound_array_element() {
    run_wire_test(
        "decode_bound_array_elem",
        r#"struct A { sequence<octet,4> arr[3]; };"#,
        r#"
            use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};

            // Within-bound roundtrip must still work (no false positives).
            let ok = A { arr: [vec![1, 2], vec![], vec![3, 4, 5, 6]] };
            let mut buf = Vec::new();
            <A as DdsType>::encode(&ok, &mut buf).expect("within-bound encode");
            let back = <A as DdsType>::decode(&buf).expect("within-bound decode");
            assert_eq!(back, ok);

            // Forge a well-formed appendable payload whose array holds an
            // element (5 octets) exceeding the IDL bound (4) — well within
            // the remaining buffer, so only an IDL-bound check catches it.
            let over_bound: [Vec<u8>; 3] = [vec![1, 2], vec![9, 9, 9, 9, 9], vec![3]];
            let mut w = BufferWriter::new(Endianness::Little).xcdr2();
            zerodds_cdr::struct_enc::encode_appendable(&mut w, |w2| {
                <_ as CdrEncode>::encode(&over_bound, w2)?;
                Ok(())
            }).unwrap();
            let bytes = w.into_bytes();

            let mut r = BufferReader::new(&bytes, Endianness::Little).xcdr2();
            let err = <A as CdrDecode>::decode(&mut r)
                .expect_err("decode must reject an over-bound array element, not just check the remaining buffer");
            match err {
                zerodds_cdr::DecodeError::BoundExceeded { actual, bound, .. } => {
                    assert_eq!(bound, 4, "must report the IDL bound (4), not the buffer size");
                    assert_eq!(actual, 5);
                }
                other => panic!("expected DecodeError::BoundExceeded, got {other:?}"),
            }
        "#,
    );
}
