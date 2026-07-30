// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Adversarial edge-hardening roundtrips for the Rust IDL backend.
//!
//! Each test emits real codegen, compiles it as an external crate, and runs a
//! REAL `encode` → `decode` roundtrip (not a string match on the source).
//! These are the cases that break adapters the moment they leave hello-world:
//! empty collections, bound enforcement, deep nesting, optional aggregates,
//! every union branch, multi-byte Unicode, arrays with distinct elements,
//! extreme primitive values, and keyed samples.
//!
//! All categories are driven through a single generated module + a single
//! `cargo test` invocation to keep the (slow) temp-crate compile to one pass.

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

/// Emits `idl`, drops `test_body` into `tests/wire.rs` of a throwaway crate,
/// and runs `cargo test`. The roundtrip `assert!`s live in the *compiled*
/// crate, so a green run proves wire-level behavior.
fn run_wire_test(name: &str, idl: &str, test_body: &str) {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");

    let tmp = std::env::temp_dir().join(format!("dds_idl_rust_edge_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir src");
    std::fs::create_dir_all(tmp.join("tests")).expect("mkdir tests");

    let workspace_root = workspace_root();
    let cargo_toml = format!(
        r#"[package]
name = "edge_test_{name}"
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
        r#"// Generated edge-conformance test.
#![allow(unused_imports)]
use edge_test_{name}::*;
use zerodds_dcps::DdsType;
use zerodds_cdr::{{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness}};

#[test]
fn edge_roundtrip_assertions() {{
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
            "edge roundtrip test failed (exit {:?}). source:\n{rust_src}",
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

/// One IDL covering every aggregate the edge checklist touches. Kept in one
/// module so the whole checklist compiles + runs in a single temp crate.
const EDGE_IDL: &str = r#"
    // --- deep nesting ---
    @final struct L3 { long deep; };
    @final struct L2 { L3 inner; long mid; };
    @final struct L1 { L2 inner; long top; };

    @final struct WithSeq { sequence<long> items; long id; };

    // --- a @nested struct used as array / sequence element ---
    @nested @final struct Point { long x; long y; };

    // --- union: explicit cases + default + as seq element + as map value ---
    union Reading switch (long) {
        case 1: long counter;
        case 2: double value;
        default: string message;
    };

    // --- empty collections + optional aggregates (present and absent) ---
    @final struct Collections {
        sequence<long>        any_seq;
        sequence<long, 4>     bnd_seq;
        string                str;
        wstring               wstr;
        map<long, long>       m;
        @optional sequence<long> opt_seq;
        @optional L3             opt_struct;
        @optional string         opt_str;
        @optional map<long,long> opt_map;
    };

    // --- arrays: distinct elements so a mis-stride is caught ---
    @final struct Arrays {
        long   vec[3];
        long   grid[2][2];
        double cube[2][2][2];
        Point  shape[2];
        string names[3];
    };

    // --- deep nesting via collections ---
    @final struct SeqSeq { sequence<sequence<Point>> grid; };
    @final struct MapStruct { map<string, WithSeq> m; };

    // --- union containers ---
    @final struct UnionSeq { sequence<Reading> us; };
    @final struct UnionMap { map<long, Reading> um; };

    // --- extreme primitives ---
    @final struct Extremes {
        octet           o;
        short           i16;
        unsigned short  u16;
        long            i32;
        unsigned long   u32;
        long long       i64;
        unsigned long long u64;
        float           f32;
        double          f64;
    };

    // --- unicode ---
    @final struct Text { string s; wstring w; };

    // --- keyed ---
    @final struct Keyed { @key long id; double payload; };
"#;

/// Edges 1, 3, 4, 5, 6, 7, 8, 9 in one compiled crate (one cargo build).
#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn edge_full_checklist_roundtrip() {
    run_wire_test(
        "checklist",
        EDGE_IDL,
        r#"
        // Helper: XCDR2 default roundtrip via the DdsType API. Fully
        // qualified because generated types implement BOTH DdsType::encode
        // and CdrEncode::encode (ambiguous otherwise).
        fn rt<T: DdsType>(v: &T) -> T {
            let mut buf = Vec::new();
            <T as DdsType>::encode(v, &mut buf).expect("encode");
            <T as DdsType>::decode(&buf).expect("decode")
        }
        // Helper: classic-CDR roundtrip for unions (and any CdrEncode type).
        fn cdr_rt<T: CdrEncode + CdrDecode>(v: &T) -> T {
            let mut w = BufferWriter::new(Endianness::Little);
            v.encode(&mut w).expect("cdr encode");
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            T::decode(&mut r).expect("cdr decode")
        }

        // ===== Edge 1: EMPTY collections (count=0 must round-trip) =====
        // ===== Edge 4: @optional aggregates ABSENT (stay absent) =====
        let empty = Collections {
            any_seq: vec![],
            bnd_seq: vec![],
            str: String::new(),
            wstr: zerodds_cdr::WString::from(""),
            m: ::std::collections::BTreeMap::new(),
            opt_seq: None,
            opt_struct: None,
            opt_str: None,
            opt_map: None,
        };
        let back = rt(&empty);
        assert!(back.any_seq.is_empty(), "empty unbounded seq");
        assert!(back.bnd_seq.is_empty(), "empty bounded seq");
        assert_eq!(back.str, "", "empty string");
        assert_eq!(back.wstr.as_str(), "", "empty wstring");
        assert!(back.m.is_empty(), "empty map");
        assert_eq!(back.opt_seq, None, "absent optional seq stays None");
        assert_eq!(back.opt_struct, None, "absent optional struct stays None");
        assert_eq!(back.opt_str, None, "absent optional string stays None");
        assert_eq!(back.opt_map, None, "absent optional map stays None");

        // ===== Edge 4: @optional aggregates PRESENT (round-trip, not zero) =====
        let mut full_map = ::std::collections::BTreeMap::new();
        full_map.insert(7i32, 8i32);
        let present = Collections {
            any_seq: vec![1, 2, 3],
            bnd_seq: vec![9, 8, 7, 6], // exactly N=4 (Edge 2 OK case)
            str: "hi".to_string(),
            wstr: zerodds_cdr::WString::from("yo"),
            m: { let mut x = ::std::collections::BTreeMap::new(); x.insert(1, 100); x },
            opt_seq: Some(vec![5, 6]),
            opt_struct: Some(L3 { deep: 42 }),
            opt_str: Some("present".to_string()),
            opt_map: Some(full_map.clone()),
        };
        let back = rt(&present);
        assert_eq!(back.any_seq, vec![1, 2, 3]);
        assert_eq!(back.bnd_seq, vec![9, 8, 7, 6], "bounded seq exactly at N round-trips");
        assert_eq!(back.opt_seq, Some(vec![5, 6]), "present optional seq");
        assert_eq!(back.opt_struct, Some(L3 { deep: 42 }), "present optional struct");
        assert_eq!(back.opt_str, Some("present".to_string()), "present optional string");
        assert_eq!(back.opt_map, Some(full_map), "present optional map");

        // ===== Edge 2: BOUND enforcement — over N must ERROR, not corrupt =====
        let over = Collections {
            any_seq: vec![],
            bnd_seq: vec![1, 2, 3, 4, 5], // N=4, this is 5 → must reject
            str: String::new(),
            wstr: zerodds_cdr::WString::from(""),
            m: ::std::collections::BTreeMap::new(),
            opt_seq: None,
            opt_struct: None,
            opt_str: None,
            opt_map: None,
        };
        let mut obuf = Vec::new();
        let res = <Collections as DdsType>::encode(&over, &mut obuf);
        assert!(res.is_err(), "bounded seq over its bound (5 > 4) MUST error, not silently corrupt");

        // ===== Edge 3: DEEP nesting struct→struct→struct (3 levels) =====
        let deep = L1 { inner: L2 { inner: L3 { deep: 99 }, mid: 50 }, top: 1 };
        let back = rt(&deep);
        assert_eq!(back.inner.inner.deep, 99, "3-level nested leaf");
        assert_eq!(back.inner.mid, 50);
        assert_eq!(back.top, 1);

        // sequence<sequence<struct>>
        let ss = SeqSeq {
            grid: vec![
                vec![Point { x: 1, y: 2 }, Point { x: 3, y: 4 }],
                vec![],
                vec![Point { x: 5, y: 6 }],
            ],
        };
        let back = rt(&ss);
        assert_eq!(back.grid.len(), 3);
        assert_eq!(back.grid[0][1], Point { x: 3, y: 4 });
        assert!(back.grid[1].is_empty(), "inner empty seq survives");
        assert_eq!(back.grid[2][0], Point { x: 5, y: 6 });

        // map<string, struct-with-a-sequence>
        let mut ms = ::std::collections::BTreeMap::new();
        ms.insert("a".to_string(), WithSeq { items: vec![1, 2], id: 10 });
        ms.insert("b".to_string(), WithSeq { items: vec![], id: 20 });
        let map_struct = MapStruct { m: ms.clone() };
        let back = rt(&map_struct);
        assert_eq!(back.m, ms, "map<string, struct-with-seq> round-trips");

        // ===== Edge 5: UNION every branch + default + in seq + in map =====
        assert_eq!(cdr_rt(&Reading::Counter(7)), Reading::Counter(7), "union case 1");
        assert_eq!(cdr_rt(&Reading::Value(2.5)), Reading::Value(2.5), "union case 2");
        assert_eq!(
            cdr_rt(&Reading::Message("def".to_string())),
            Reading::Message("def".to_string()),
            "union default branch"
        );
        // union as sequence element
        let useq = UnionSeq {
            us: vec![Reading::Counter(1), Reading::Value(9.0), Reading::Message("z".to_string())],
        };
        let back = rt(&useq);
        assert_eq!(back.us.len(), 3);
        assert_eq!(back.us[0], Reading::Counter(1));
        assert_eq!(back.us[1], Reading::Value(9.0));
        assert_eq!(back.us[2], Reading::Message("z".to_string()));
        // union as map value
        let mut um = ::std::collections::BTreeMap::new();
        um.insert(1i32, Reading::Counter(11));
        um.insert(2i32, Reading::Message("m".to_string()));
        let umap = UnionMap { um: um.clone() };
        let back = rt(&umap);
        assert_eq!(back.um, um, "union as map value round-trips");

        // ===== Edge 6: UNICODE — CJK + emoji must survive exactly =====
        let txt = Text {
            s: "héllo 世界 🚀".to_string(),
            w: zerodds_cdr::WString::from("café 漢字 😀"),
        };
        let back = rt(&txt);
        assert_eq!(back.s, "héllo 世界 🚀", "UTF-8 string code points survive");
        assert_eq!(back.w.as_str(), "café 漢字 😀", "wstring code points survive");
        assert_eq!(back.s.chars().count(), "héllo 世界 🚀".chars().count());

        // ===== Edge 7: ARRAYS — distinct elements catch a mis-stride =====
        let arr = Arrays {
            vec: [10, 20, 30],
            grid: [[1, 2], [3, 4]],
            cube: [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]],
            shape: [Point { x: 100, y: 101 }, Point { x: 102, y: 103 }],
            names: ["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        };
        let back = rt(&arr);
        assert_eq!(back.vec, [10, 20, 30]);
        assert_eq!(back.grid, [[1, 2], [3, 4]]);
        assert_eq!(back.cube, [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]]);
        assert_eq!(back.shape[0], Point { x: 100, y: 101 });
        assert_eq!(back.shape[1], Point { x: 102, y: 103 });
        assert_eq!(back.names, ["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            "array-of-bounded-string, every distinct element survives");

        // ===== Edge 8: EXTREME primitives — min/max/0/-1 =====
        let ex = Extremes {
            o: 255,
            i16: i16::MIN,
            u16: u16::MAX,
            i32: i32::MIN,
            u32: u32::MAX,
            i64: i64::MIN,
            u64: u64::MAX,
            f32: -1.0,
            f64: 3.141592653589793,
        };
        let back = rt(&ex);
        assert_eq!(back.o, 255);
        assert_eq!(back.i16, i16::MIN);
        assert_eq!(back.u16, u16::MAX);
        assert_eq!(back.i32, i32::MIN);
        assert_eq!(back.u32, u32::MAX);
        assert_eq!(back.i64, i64::MIN);
        assert_eq!(back.u64, u64::MAX);
        assert_eq!(back.f32, -1.0);
        assert_eq!(back.f64, 3.141592653589793);
        // 0 and -1 too
        let ex0 = Extremes {
            o: 0, i16: 0, u16: 0, i32: -1, u32: 0, i64: -1, u64: 0, f32: 0.0, f64: 0.0,
        };
        let back = rt(&ex0);
        assert_eq!(back.i32, -1);
        assert_eq!(back.i64, -1);
        assert_eq!(back.o, 0);

        // ===== Edge 9: KEYED — same key, different payload =====
        assert_eq!(Keyed::HAS_KEY, true);
        let a = Keyed { id: 7, payload: 1.0 };
        let b = Keyed { id: 7, payload: 999.0 };
        let ha = a.compute_key_hash().expect("keyed type yields hash");
        let hb = b.compute_key_hash().expect("keyed type yields hash");
        assert_eq!(ha.len(), 16);
        assert_eq!(ha, hb, "same @key id → same key hash regardless of payload");
        let c = Keyed { id: 8, payload: 1.0 };
        assert_ne!(ha, c.compute_key_hash().unwrap(), "different key → different hash");
        // the key field itself round-trips identically for both payloads
        assert_eq!(rt(&a).id, 7);
        assert_eq!(rt(&b).id, 7);
        assert_eq!(rt(&a).payload, 1.0);
        assert_eq!(rt(&b).payload, 999.0);

        println!("ALL EDGE CATEGORIES PASSED");
        "#,
    );
}

/// Big-endian decode roundtrip for the two constructs whose generated decoders
/// historically hardcoded little-endian: an `@mutable` struct (EMHEADER member
/// framing, incl. an LC5 string whose reused length word is multi-byte) and a
/// `wstring` (UTF-16 code units). A big-endian XCDR2 stream carries big-endian
/// length prefixes and UTF-16 units; decoding them as LE silently corrupts
/// every multi-byte field (regression: an LC5 string length `00 00 00 03` was
/// read as `0x03000000`). Exercised through the `CdrEncode`/`CdrDecode` trait
/// pair with a big-endian writer/reader, in BOTH byte orders for symmetry.
#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn mutable_and_wstring_big_endian_roundtrip() {
    run_wire_test(
        "be_decode",
        r#"
        module m {
          @mutable struct Mut {
            @id(10) long      a;
            @id(20) double    b;
            @id(30) string<8> c;
          };
          @appendable struct W {
            wstring<16> label;
            wstring     text;
          };
        };
        "#,
        r#"
        // Roundtrip a CdrEncode/CdrDecode type through ONE byte order.
        fn rt_endian<T: CdrEncode + CdrDecode + PartialEq + std::fmt::Debug>(v: &T, e: Endianness) {
            let mut w = BufferWriter::new(e).xcdr2();
            v.encode(&mut w).expect("encode");
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, e).xcdr2();
            let back = T::decode(&mut r).expect("decode");
            assert_eq!(&back, v, "roundtrip mismatch in {:?}", e);
        }

        let mut_v = m::Mut { a: 1_000_000, b: 2.5, c: "ok".to_string() };
        let w_v = m::W {
            label: zerodds_cdr::WString::from("café"),
            text: zerodds_cdr::WString::from("日本語"),
        };
        for e in [Endianness::Little, Endianness::Big] {
            rt_endian(&mut_v, e);
            rt_endian(&w_v, e);
        }
        println!("BE/LE @mutable + wstring decode roundtrip PASSED");
        "#,
    );
}
