//! #24 / F-TYPES-3 golden + cross-binding parity for the emitted TypeObject.
//!
//! Proves three things about `idlc cpp`'s new `topic_type_support<T>::type_object()`
//! byte constant:
//!  1. It is byte-identical to the SHARED golden
//!     `zerodds_idl::semantics::complete_struct_type_object_bytes` — the single
//!     source both bindings compute from.
//!  2. The MD5-14 hash of those bytes equals the `TYPE_IDENTIFIER` hash that
//!     `idl-rust` emits for the SAME IDL — i.e. a cpp writer and a rust writer
//!     advertise the IDENTICAL `TypeIdentifier` (the whole point of #24).
//!  3. The bytes start with `EK_COMPLETE` (0xF2) — a COMPLETE TypeObject.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_idl::ast::types::{Specification, StructDef};
use zerodds_idl::ast::{ConstrTypeDecl, Definition, StructDcl, TypeDecl};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::semantics::{build_type_registry, complete_struct_type_object_bytes};
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Recursively finds the first struct named `name` and returns it with its
/// module scope (outermost-first).
fn find_struct(spec: &Specification, name: &str) -> (StructDef, Vec<String>) {
    fn walk(
        defs: &[Definition],
        scope: &mut Vec<String>,
        name: &str,
    ) -> Option<(StructDef, Vec<String>)> {
        for d in defs {
            match d {
                Definition::Module(m) => {
                    scope.push(m.name.text.clone());
                    if let Some(hit) = walk(&m.definitions, scope, name) {
                        return Some(hit);
                    }
                    scope.pop();
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))))
                    if s.name.text == name =>
                {
                    return Some((s.clone(), scope.clone()));
                }
                _ => {}
            }
        }
        None
    }
    let mut scope = Vec::new();
    walk(&spec.definitions, &mut scope, name).expect("struct not found")
}

/// Extracts the `0xNN` bytes of the `ZD_TYPE_OBJECT[] = { ... }` array of the
/// first `type_object()` body in a generated cpp header.
fn extract_cpp_type_object(cpp: &str) -> Vec<u8> {
    let start = cpp
        .find("ZD_TYPE_OBJECT[] = {")
        .expect("no ZD_TYPE_OBJECT array in cpp output");
    let rest = &cpp[start..];
    let close = rest.find("};").expect("unterminated ZD_TYPE_OBJECT array");
    parse_hex_bytes(&rest[..close])
}

/// Extracts the 14 hash bytes from an `idl-rust` `TYPE_IDENTIFIER` const of the
/// form `...EquivalenceHashComplete(zerodds_types::EquivalenceHash([0x..,..]))`.
fn extract_rust_identifier_hash(rust: &str) -> Vec<u8> {
    let anchor = rust
        .find("EquivalenceHashComplete")
        .expect("no EquivalenceHashComplete TYPE_IDENTIFIER in rust output");
    let rest = &rust[anchor..];
    let open = rest.find('[').expect("no [ in identifier");
    let close = rest.find(']').expect("no ] in identifier");
    parse_hex_bytes(&rest[open..close])
}

/// Parses every `0x..` token in a text fragment into a byte vector. Tolerates
/// tokens glued to opening brackets (e.g. `[0xff`, `{0xf2`).
fn parse_hex_bytes(fragment: &str) -> Vec<u8> {
    fragment
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|tok| tok.trim().trim_start_matches(['[', '{', '(']))
        .filter_map(|tok| tok.strip_prefix("0x"))
        .map(|h| u8::from_str_radix(h, 16).expect("bad hex byte"))
        .collect()
}

fn parse(src: &str) -> Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse")
}

/// The shared golden bytes for `struct_name` in `src`.
fn golden_bytes(src: &str, struct_name: &str) -> Vec<u8> {
    let spec = parse(src);
    let (s, scope) = find_struct(&spec, struct_name);
    let names = build_type_registry(&spec)
        .map(|l| l.names)
        .unwrap_or_default();
    complete_struct_type_object_bytes(&s, &scope, &names).expect("golden TypeObject bytes")
}

/// Runs the full parity check for a single struct in `src`.
fn assert_parity(src: &str, struct_name: &str) {
    let spec = parse(src);

    // 1. cpp-emitted bytes == shared golden.
    let cpp = generate_cpp_header(&spec, &CppGenOptions::default()).expect("cpp gen");
    let cpp_bytes = extract_cpp_type_object(&cpp);
    let golden = golden_bytes(src, struct_name);
    assert_eq!(
        cpp_bytes, golden,
        "cpp type_object() bytes != shared golden for {}",
        struct_name
    );

    // 3. COMPLETE TypeObject discriminator.
    assert_eq!(cpp_bytes[0], 0xF2, "emitted TypeObject is not EK_COMPLETE");

    // 2. MD5-14(cpp bytes) == idl-rust's advertised TYPE_IDENTIFIER hash.
    let rust = generate_rust_module(&spec, &RustGenOptions::default()).expect("rust gen");
    let rust_hash = extract_rust_identifier_hash(&rust);
    let md5 = zerodds_foundation::md5(&cpp_bytes);
    assert_eq!(
        &md5[..14],
        rust_hash.as_slice(),
        "MD5-14 of cpp TypeObject bytes != idl-rust TYPE_IDENTIFIER hash for {} (cross-binding identifier mismatch)",
        struct_name
    );
}

#[test]
fn parity_simple_struct() {
    assert_parity("struct Point { long x; long y; };", "Point");
}

#[test]
fn parity_keyed_struct() {
    assert_parity("struct Sensor { @key long id; double value; };", "Sensor");
}

#[test]
fn parity_string_and_sequence() {
    assert_parity(
        "struct Bag { string name; sequence<long> ids; sequence<string, 16> tags; };",
        "Bag",
    );
}

#[test]
fn parity_module_nested_struct() {
    // Exercises the module-scope threading (fqn_module_scope) — the nested
    // struct's member resolution must use the same scope idl-rust uses.
    assert_parity(
        "module Outer { module Inner { struct Pose { long a; double b; }; }; };",
        "Pose",
    );
}

#[test]
fn parity_array_member() {
    assert_parity("struct M { long grid[4][4]; };", "M");
}
