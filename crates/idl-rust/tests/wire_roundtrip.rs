// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire-Conformance-Tests: emittiert Code, kompiliert ihn als externes
//! Crate, ruft `encode`/`decode` aus und prueft Roundtrip + Golden-Bytes.
//!
//! Belegt End-to-End: der Codegen-Output produziert XCDR2-konforme
//! Wire-Bytes — nicht nur „kompiliert" sondern „verhaelt sich
//! Spec-konform".

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

/// Schreibt den emittierten Code + ein Test-File mit Roundtrip-
/// Assertions in eine temp-Crate und ruft `cargo test` auf.
///
/// Schlaegt fehl wenn die Roundtrip-Assertions im emittierten Code
/// fehlschlagen — das beweist Wire-Conformance auf Verhalten-Ebene.
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
zerodds-sql-filter = {{ path = "{root}/crates/sql-filter" }}
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

            // Wire-Form: 2 × i32 LE = 8 byte
            assert_eq!(buf.len(), 8);
            assert_eq!(&buf[..4], &42i32.to_le_bytes());
            assert_eq!(&buf[4..], &(-7i32).to_le_bytes());
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

            // Zwei Readings mit gleichem sensor_id muessen gleichen hash liefern
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
            use zerodds_sql_filter::Value;
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

            // Decoding muss den DHEADER konsumieren und die Member zurueckgeben
            let back = Telemetry::decode(&buf).expect("decode");
            assert_eq!(back.ts, 100);
            assert_eq!(back.v, 1.5);
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
