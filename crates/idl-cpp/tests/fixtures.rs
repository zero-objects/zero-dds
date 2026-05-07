//! Fixture-Tests fuer den IDL→C++-Codegen.
//!
//! Pro Fixture: parse IDL → generate C++ → asserte eine Liste von
//! Marker-Substrings, die zwingend im Output erscheinen muessen.
//!
//! Marker-basierte Snapshots sind robuster als Byte-equal-Snapshots —
//! Whitespace-Drift im Emitter (Block-A-Polishing) bricht die Tests
//! nicht, solange die semantischen Anker (Type-Namen, Includes,
//! Inheritance-Klauseln) erhalten bleiben.
//!
//! Die Fixture-Liste deckt alle Block-A-bis-E-Use-Cases ab.

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
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

const FIXTURES: &[(&str, &str, &[&str])] = &[
    (
        "prim_struct",
        include_str!("fixtures/prim_struct.idl"),
        &[
            "#pragma once",
            "#include <cstdint>",
            "class Primitives",
            "bool a_bool_;",
            "uint8_t a_octet_;",
            "char a_char_;",
            "int16_t a_short_;",
            "uint16_t a_ushort_;",
            "int32_t a_long_;",
            "uint32_t a_ulong_;",
            "int64_t a_llong_;",
            "uint64_t a_ullong_;",
            "float a_float_;",
            "double a_double_;",
            // Reference-Pattern Getter:
            "int32_t& a_long()",
            "const int32_t& a_long() const",
        ],
    ),
    (
        "nested_modules",
        include_str!("fixtures/nested_modules.idl"),
        &[
            "namespace Outer {",
            "namespace Middle {",
            "namespace Inner {",
            "class Leaf",
            "} // namespace Inner",
            "} // namespace Middle",
            "} // namespace Outer",
        ],
    ),
    (
        "simple_enum",
        include_str!("fixtures/simple_enum.idl"),
        &["enum class Color : int32_t", "RED,", "GREEN,", "BLUE,"],
    ),
    (
        "union_with_default",
        include_str!("fixtures/union_with_default.idl"),
        &[
            "#include <variant>",
            "class Choice",
            "std::variant<",
            "// case default",
            "value_type",
        ],
    ),
    (
        "string_member",
        include_str!("fixtures/string_member.idl"),
        &[
            "#include <string>",
            "std::string name_;",
            "std::wstring locale_;",
        ],
    ),
    (
        "sequence_of_long",
        include_str!("fixtures/sequence_of_long.idl"),
        &["#include <vector>", "std::vector<int32_t> items_;"],
    ),
    (
        "array_2d",
        include_str!("fixtures/array_2d.idl"),
        &["#include <array>", "std::array<std::array<int32_t, 3>, 4>"],
    ),
    (
        "inherited_struct",
        include_str!("fixtures/inherited_struct.idl"),
        &["class Parent", "class Child : public Parent"],
    ),
    (
        "typedef_alias",
        include_str!("fixtures/typedef_alias.idl"),
        &[
            "using Counter = int32_t;",
            "using Bag = std::vector<int32_t>;",
        ],
    ),
    (
        "keyed_struct",
        include_str!("fixtures/keyed_struct.idl"),
        &[
            "class Sensor",
            "int32_t sensor_id_;",
            "// @key",
            "double reading_;",
        ],
    ),
    (
        "optional_member",
        include_str!("fixtures/optional_member.idl"),
        &["#include <optional>", "std::optional<std::string>"],
    ),
    (
        "multiple_types",
        include_str!("fixtures/multiple_types.idl"),
        &[
            "enum class Status : int32_t",
            "using EntityId = int32_t;",
            "class Entity",
            "class EntityNotFound : public std::exception",
        ],
    ),
];

#[test]
fn all_fixtures_generate_required_markers() {
    let opts = CppGenOptions::default();
    for (name, src, markers) in FIXTURES {
        let ast = zerodds_idl::parse(src, &ParserConfig::default())
            .unwrap_or_else(|e| panic!("fixture '{name}': parse failed: {e}"));
        let cpp = generate_cpp_header(&ast, &opts)
            .unwrap_or_else(|e| panic!("fixture '{name}': gen failed: {e}"));
        for marker in *markers {
            assert!(
                cpp.contains(marker),
                "fixture '{name}': missing marker '{marker}'\n--- generated ---\n{cpp}\n--- end ---",
            );
        }
    }
}

// Pro Fixture ein eigener `#[test]`, damit Failure-Lokalisierung praezise ist.

macro_rules! fixture_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            let src = include_str!(concat!("fixtures/", stringify!($name), ".idl"));
            let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
            let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
            // Mindest-Anker: jedes Fixture muss `#pragma once` und
            // `#include <cstdint>` enthalten (Praeambel-Invariante).
            assert!(
                cpp.contains("#pragma once"),
                "missing pragma in {}",
                stringify!($name)
            );
            assert!(
                cpp.contains("#include <cstdint>"),
                "missing cstdint include in {}",
                stringify!($name)
            );
        }
    };
}

fixture_test!(prim_struct);
fixture_test!(nested_modules);
fixture_test!(simple_enum);
fixture_test!(union_with_default);
fixture_test!(string_member);
fixture_test!(sequence_of_long);
fixture_test!(array_2d);
fixture_test!(inherited_struct);
fixture_test!(typedef_alias);
fixture_test!(keyed_struct);
fixture_test!(optional_member);
fixture_test!(multiple_types);
