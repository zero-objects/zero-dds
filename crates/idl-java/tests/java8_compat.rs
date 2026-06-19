//! Java-8-Compat-Mode (`JavaGenOptions.java8_compat`).
//!
//! The standard emit (default) uses a `sealed interface` with
//! `record` case types (Java 17) for unions. The opt-in compat mode
//! instead emits an `abstract class` with a private constructor
//! (pseudo-sealing) and `static final` subclasses — all Java-8-capable.
//! Structs are bean classes anyway and identical in both modes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

const UNION_IDL: &str = r#"
    union U switch (long) {
        case 1: long a;
        case 2: string b;
    };
"#;

fn gen_java(src: &str, java8: bool) -> String {
    let opts = JavaGenOptions {
        emit_typesupport: false,
        java8_compat: java8,
        ..Default::default()
    };
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &opts).expect("gen");
    files
        .into_iter()
        .map(|f| f.source)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn standard_mode_uses_sealed_interface_and_records() {
    let java = gen_java(UNION_IDL, false);
    assert!(
        java.contains("sealed interface U"),
        "standard mode should emit a sealed interface:\n{java}"
    );
    assert!(
        java.contains("record A") && java.contains("record B"),
        "standard mode should emit case records:\n{java}"
    );
}

#[test]
fn java8_mode_uses_abstract_class_and_static_subclasses() {
    let java = gen_java(UNION_IDL, true);
    // No Java-9+ construct.
    assert!(
        !java.contains("sealed interface"),
        "Java-8 mode must not emit a 'sealed interface':\n{java}"
    );
    assert!(
        !java.contains("record A") && !java.contains("record B"),
        "Java-8 mode must not emit case records:\n{java}"
    );
    // Java-8-Form.
    assert!(
        java.contains("public abstract class U"),
        "Java-8 mode should emit 'abstract class U':\n{java}"
    );
    assert!(
        java.contains("private U()"),
        "Java-8 mode should emit a private pseudo-seal constructor:\n{java}"
    );
    assert!(
        java.contains("public static final class A extends U")
            && java.contains("public static final class B extends U"),
        "Java-8 mode should emit static-final subclasses:\n{java}"
    );
    // Record equivalent: a final field + accessor of the same name.
    assert!(
        java.contains("private final") && java.contains("public int a()"),
        "Java 8 subclass should have final field + accessor:\n{java}"
    );
}
