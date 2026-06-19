//! Fixture tests for the IDL→C# codegen.
//!
//! Per fixture: parse IDL → generate C# → assert a list of marker
//! substrings that must appear in the output.
//!
//! Marker-based snapshots are more robust than byte-equal snapshots —
//! whitespace drift in the emitter (phase 3 polishing) does not break
//! the tests as long as the semantic anchors (type names, using
//! imports, inheritance clauses) are preserved.

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
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

const FIXTURES: &[(&str, &str, &[&str])] = &[
    (
        "prim_struct",
        include_str!("fixtures/prim_struct.idl"),
        &[
            "#nullable enable",
            "using System;",
            "public partial record class Primitives",
            "public bool ABool { get; init; }",
            "public byte AOctet { get; init; }",
            "public char AChar { get; init; }",
            "public short AShort { get; init; }",
            "public ushort AUshort { get; init; }",
            "public int ALong { get; init; }",
            "public uint AUlong { get; init; }",
            "public long ALlong { get; init; }",
            "public ulong AUllong { get; init; }",
            "public float AFloat { get; init; }",
            "public double ADouble { get; init; }",
        ],
    ),
    (
        "nested_modules",
        include_str!("fixtures/nested_modules.idl"),
        &[
            "namespace Outer",
            "namespace Middle",
            "namespace Inner",
            "public partial record class Leaf",
            "} // namespace Inner",
            "} // namespace Middle",
            "} // namespace Outer",
        ],
    ),
    (
        "simple_enum",
        include_str!("fixtures/simple_enum.idl"),
        &["public enum Color : int", "RED,", "GREEN,", "BLUE,"],
    ),
    (
        "union_with_default",
        include_str!("fixtures/union_with_default.idl"),
        &[
            "public partial record class Choice",
            "public int Discriminator",
            "public object? Value",
            "// case default",
        ],
    ),
    (
        "string_member",
        include_str!("fixtures/string_member.idl"),
        &[
            "public string Name { get; init; }",
            "public string Locale { get; init; }",
        ],
    ),
    (
        "sequence_of_long",
        include_str!("fixtures/sequence_of_long.idl"),
        &[
            "using System.Collections.Generic;",
            "using Omg.Types;",
            "public ISequence<int> Items { get; init; }",
        ],
    ),
    (
        "array_2d",
        include_str!("fixtures/array_2d.idl"),
        &["int[][] Cells"],
    ),
    (
        "inherited_struct",
        include_str!("fixtures/inherited_struct.idl"),
        &[
            "public partial record class Parent",
            "public partial record class Child : Parent",
        ],
    ),
    (
        "typedef_alias",
        include_str!("fixtures/typedef_alias.idl"),
        &[
            "public sealed record class Counter(int Value);",
            "public sealed record class Bag(ISequence<int> Value);",
        ],
    ),
    (
        "keyed_struct",
        include_str!("fixtures/keyed_struct.idl"),
        &[
            "public partial record class Sensor",
            "[Key]",
            "public int SensorId { get; init; }",
            "public double Reading { get; init; }",
        ],
    ),
    (
        "optional_member",
        include_str!("fixtures/optional_member.idl"),
        &["public string? Nickname { get; init; }"],
    ),
    (
        "multiple_types",
        include_str!("fixtures/multiple_types.idl"),
        &[
            "public enum Status : int",
            "public sealed record class EntityId(int Value);",
            "public partial record class Entity",
            "public sealed class EntityNotFound : Exception",
        ],
    ),
];

#[test]
fn all_fixtures_generate_required_markers() {
    let opts = CsGenOptions::default();
    for (name, src, markers) in FIXTURES {
        let ast = zerodds_idl::parse(src, &ParserConfig::default())
            .unwrap_or_else(|e| panic!("fixture '{name}': parse failed: {e}"));
        let cs = generate_csharp(&ast, &opts)
            .unwrap_or_else(|e| panic!("fixture '{name}': gen failed: {e}"));
        for marker in *markers {
            assert!(
                cs.contains(marker),
                "fixture '{name}': missing marker '{marker}'\n--- generated ---\n{cs}\n--- end ---",
            );
        }
    }
}

// One `#[test]` per fixture so that failure localization is precise.

macro_rules! fixture_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            let src = include_str!(concat!("fixtures/", stringify!($name), ".idl"));
            let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
            let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");
            // Minimum anchor: every fixture must contain `#nullable enable`
            // and `using System;` (preamble invariant).
            assert!(
                cs.contains("#nullable enable"),
                "missing #nullable enable in {}",
                stringify!($name)
            );
            assert!(
                cs.contains("using System;"),
                "missing using System; in {}",
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
