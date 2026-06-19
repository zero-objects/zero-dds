//! Fixture tests for the IDL→Java codegen.
//!
//! Per fixture: parse IDL → generate Java files → assert a list
//! of marker substrings that must necessarily appear in the output.
//!
//! Marker-based snapshots are more robust than byte-equal snapshots —
//! whitespace drift in the emitter does not break the tests as long as the
//! semantic anchors (type names, imports, inheritance clauses)
//! are preserved.

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
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

/// Helper function: concatenates all files into a searchable blob.
fn join_sources(files: &[zerodds_idl_java::JavaFile]) -> String {
    files
        .iter()
        .map(|f| f.source.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

const FIXTURES: &[(&str, &str, &[&str])] = &[
    (
        "prim_struct",
        include_str!("fixtures/prim_struct.idl"),
        &[
            "public class Primitives",
            "private boolean a_bool;",
            "private byte a_octet;",
            "private char a_char;",
            "private char a_wchar;",
            "private short a_short;",
            "private int a_ushort;",
            "private int a_long;",
            "private long a_ulong;",
            "private long a_llong;",
            "private long a_ullong;",
            "private float a_float;",
            "private double a_double;",
            // Bean-Pattern Getter:
            "public int getA_long()",
            "public void setA_long(int a_long)",
            // Unsigned-Doc-Comment:
            "unsigned IDL value",
        ],
    ),
    (
        "nested_modules",
        include_str!("fixtures/nested_modules.idl"),
        &[
            "package outer.middle.inner;",
            "public class Leaf",
            "private int value;",
        ],
    ),
    (
        "simple_enum",
        include_str!("fixtures/simple_enum.idl"),
        &[
            "public enum Color {",
            "RED(0),",
            "GREEN(1),",
            "BLUE(2);",
            "public int value()",
        ],
    ),
    (
        "union_with_default",
        include_str!("fixtures/union_with_default.idl"),
        &[
            "public sealed interface Choice",
            // TS-3-Finding 5 (fixed 2026-05-01): nested records need
            // qualified names in the `permits` clause.
            "permits Choice.Int_branch, Choice.Double_branch, Choice.Default_branch",
            "record Int_branch(int int_branch) implements Choice",
            "record Double_branch(double double_branch) implements Choice",
            "// case default",
        ],
    ),
    (
        "sequence_of_long",
        include_str!("fixtures/sequence_of_long.idl"),
        &["public class Bag", "private java.util.List<Integer> items;"],
    ),
    (
        "array_2d",
        include_str!("fixtures/array_2d.idl"),
        &["public class Matrix", "private int[][] cells;"],
    ),
    (
        "inherited_struct",
        include_str!("fixtures/inherited_struct.idl"),
        &[
            "public class Parent",
            "public class Child extends Parent",
            "private String name;",
        ],
    ),
    (
        "typedef_alias",
        include_str!("fixtures/typedef_alias.idl"),
        &[
            "public final class Counter",
            "private int value;",
            "public final class Bag",
            "private java.util.List<Integer> value;",
        ],
    ),
    (
        "keyed_struct",
        include_str!("fixtures/keyed_struct.idl"),
        &[
            "public class Sensor",
            "@org.zerodds.types.Key",
            "private int sensor_id;",
            "private double reading;",
        ],
    ),
    (
        "optional_member",
        include_str!("fixtures/optional_member.idl"),
        &[
            "public class Profile",
            "java.util.Optional<String> nickname",
        ],
    ),
    (
        "multiple_types",
        include_str!("fixtures/multiple_types.idl"),
        &[
            "public enum Status",
            "public final class EntityId",
            "public class Entity",
            "public class EntityNotFound extends RuntimeException",
        ],
    ),
];

#[test]
fn all_fixtures_generate_required_markers() {
    let opts = JavaGenOptions::default();
    for (name, src, markers) in FIXTURES {
        let ast = zerodds_idl::parse(src, &ParserConfig::default())
            .unwrap_or_else(|e| panic!("fixture '{name}': parse failed: {e}"));
        let files = generate_java_files(&ast, &opts)
            .unwrap_or_else(|e| panic!("fixture '{name}': gen failed: {e}"));
        let blob = join_sources(&files);
        for marker in *markers {
            assert!(
                blob.contains(marker),
                "fixture '{name}': missing marker '{marker}'\n--- generated ---\n{blob}\n--- end ---",
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
            let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
            // Minimum anchor: every generated file begins with the
            // generator marker.
            for f in &files {
                assert!(
                    f.source.starts_with("// Generated by zerodds idl-java."),
                    "missing generator marker in {} fixture for class {}",
                    stringify!($name),
                    f.class_name
                );
            }
            // Safety: at least 1 file (the fixtures are all
            // non-empty).
            assert!(
                !files.is_empty(),
                "fixture {} produced 0 files",
                stringify!($name)
            );
        }
    };
}

fixture_test!(prim_struct);
fixture_test!(nested_modules);
fixture_test!(simple_enum);
fixture_test!(union_with_default);
fixture_test!(sequence_of_long);
fixture_test!(array_2d);
fixture_test!(inherited_struct);
fixture_test!(typedef_alias);
fixture_test!(keyed_struct);
fixture_test!(optional_member);
fixture_test!(multiple_types);

#[test]
fn multi_file_output_one_class_per_top_level_type() {
    // 3 top-level structs → 3 POJOs + 3 TypeSupports (xcdr2-java-1.0 §4).
    let src = "struct A { long x; }; struct B { long y; }; struct C { long z; };";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    assert_eq!(files.len(), 6);
    let pojos: Vec<&str> = files
        .iter()
        .map(|f| f.class_name.as_str())
        .filter(|n| !n.ends_with("TypeSupport"))
        .collect();
    assert!(pojos.contains(&"A"));
    assert!(pojos.contains(&"B"));
    assert!(pojos.contains(&"C"));
    let supports: Vec<&str> = files
        .iter()
        .map(|f| f.class_name.as_str())
        .filter(|n| n.ends_with("TypeSupport"))
        .collect();
    assert!(supports.contains(&"ATypeSupport"));
    assert!(supports.contains(&"BTypeSupport"));
    assert!(supports.contains(&"CTypeSupport"));
    // Each POJO file's class name must equal its top-level public class.
    for f in &files {
        if f.class_name.ends_with("TypeSupport") {
            let needle = format!("public final class {}", f.class_name);
            assert!(
                f.source.contains(&needle),
                "class {} missing in its own source",
                f.class_name
            );
        } else {
            let needle = format!("public class {}", f.class_name);
            assert!(
                f.source.contains(&needle),
                "class {} missing in its own source",
                f.class_name
            );
        }
    }
}

#[test]
fn typedef_with_two_aliases_emits_two_files() {
    let src = "typedef long A; typedef long B;";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    assert_eq!(files.len(), 2);
}
