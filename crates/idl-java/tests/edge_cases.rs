//! Edge-case integration tests for the IDL→Java codegen.
//!
//! - Empty IDL → empty file list
//! - Reserved Java keyword as an IDL field name → underscore suffix
//! - Inheritance cycle → error
//! - Java construct negatives: interface, valuetype, fixed, any, bitset,
//!   bitmask
//! - Multi-file layout: 3 structs → 3 .java files
//! - Indent-width option
//! - root_package option

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
use zerodds_idl_java::{JavaGenError, JavaGenOptions, generate_java_files};

fn parse(src: &str) -> zerodds_idl::ast::Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed")
}

#[test]
fn empty_ast_produces_no_files() {
    let files = generate_java_files(&parse(""), &JavaGenOptions::default()).expect("gen");
    assert!(files.is_empty());
}

#[test]
fn struct_with_reserved_field_name_uses_underscore_suffix() {
    // `class` is reserved in Java. Java does not allow an `@` escape — we
    // expect the identifier with a `_` suffix.
    let ast = parse("struct Foo { long class; };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    // 1 POJO + 1 TypeSupport per struct (zerodds-xcdr2-java-1.0 §4).
    assert_eq!(files.len(), 2);
    let pojo = files
        .iter()
        .find(|f| f.class_name == "Foo")
        .expect("POJO file");
    let src = &pojo.source;
    assert!(src.contains("private int class_;"));
    // Bean pattern: the getter name is `getClass_` (NOT `getClass`, which
    // would collide with `Object.getClass()`).
    assert!(src.contains("public int getClass_()"));
}

#[test]
fn struct_with_reserved_int_field_name_gets_underscore() {
    let ast = parse("struct Foo { long int; };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    // POJO + TypeSupport.
    let pojo = files
        .iter()
        .find(|f| f.class_name == "Foo")
        .expect("POJO file");
    assert!(pojo.source.contains("int_"));
}

#[test]
fn inheritance_self_loop_is_rejected() {
    let ast = parse(
        "struct A : B { long a; };\n\
         struct B : A { long b; };",
    );
    let res = generate_java_files(&ast, &JavaGenOptions::default());
    assert!(
        matches!(res, Err(JavaGenError::InheritanceCycle { .. })),
        "expected InheritanceCycle, got {res:?}",
    );
}

#[test]
fn three_top_level_structs_produce_three_files() {
    let ast = parse("struct A { long x; }; struct B { long y; }; struct C { long z; };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    // 3 POJOs + 3 TypeSupports.
    assert_eq!(files.len(), 6);
    let pojos: Vec<_> = files
        .iter()
        .filter(|f| !f.class_name.ends_with("TypeSupport"))
        .collect();
    assert_eq!(pojos.len(), 3);
    let supports: Vec<_> = files
        .iter()
        .filter(|f| f.class_name.ends_with("TypeSupport"))
        .collect();
    assert_eq!(supports.len(), 3);
}

#[test]
fn nested_three_modules_become_three_packages() {
    let ast = parse("module A { module B { module C { struct S { long x; }; }; }; };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    // POJO + TypeSupport in the same package.
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.package_path == "a.b.c"));
}

#[test]
fn interface_emits_java_interface() {
    let ast = parse("interface I { void op(); };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("ok");
    let combined: String = files.iter().map(|f| f.source.clone()).collect();
    assert!(combined.contains("public interface I"));
}

#[test]
fn any_type_emits_object() {
    let ast = parse("struct S { any value; };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("ok");
    let combined: String = files.iter().map(|f| f.source.clone()).collect();
    assert!(combined.contains("Object"));
}

#[test]
fn fixed_type_emits_bigdecimal() {
    let ast = parse("typedef fixed<5, 2> Money;");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("ok");
    let combined: String = files.iter().map(|f| f.source.clone()).collect();
    assert!(combined.contains("java.math.BigDecimal"));
}

#[test]
fn bitset_is_now_supported_in_cluster_e() {
    // C5.4-b provides bitset mapping → wrapper class with `long bits`.
    let ast = parse("bitset Flags { bitfield<3> a; };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    assert_eq!(files.len(), 1);
    assert!(files[0].source.contains("public final class Flags"));
    assert!(files[0].source.contains("private long bits;"));
}

#[test]
fn bitmask_is_now_supported_in_cluster_e() {
    // C5.4-b provides bitmask mapping → inner enum + EnumSet wrapper.
    let ast = parse("bitmask Flags { F0, F1 };");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    assert_eq!(files.len(), 1);
    assert!(files[0].source.contains("public enum Flag"));
    assert!(files[0].source.contains("java.util.EnumSet<Flag>"));
}

#[test]
fn bitset_too_wide_is_unsupported() {
    // Sum > 64 → hard error.
    let ast = parse("bitset Big { bitfield<40> a; bitfield<30> b; };");
    match generate_java_files(&ast, &JavaGenOptions::default()) {
        Err(JavaGenError::UnsupportedConstruct { construct, .. }) => {
            assert!(construct.contains("64"));
        }
        other => panic!("expected UnsupportedConstruct, got {other:?}"),
    }
}

#[test]
fn const_decl_is_emitted_as_holder_class() {
    let files = generate_java_files(
        &parse("const long MAX_SIZE = 1024;"),
        &JavaGenOptions::default(),
    )
    .expect("gen");
    let src = &files[0].source;
    assert!(src.contains("public final class MAX_SIZEConstant"));
    assert!(src.contains("public static final int MAX_SIZE = 1024;"));
}

#[test]
fn root_package_option_is_prepended_to_modules() {
    let opts = JavaGenOptions {
        root_package: "org.example".into(),
        ..Default::default()
    };
    let files =
        generate_java_files(&parse("module Inner { struct S { long x; }; };"), &opts).expect("gen");
    assert_eq!(files[0].package_path, "org.example.inner");
    assert!(files[0].source.contains("package org.example.inner;"));
}

#[test]
fn root_package_alone_without_modules() {
    let opts = JavaGenOptions {
        root_package: "com.acme".into(),
        ..Default::default()
    };
    let files = generate_java_files(&parse("struct S { long x; };"), &opts).expect("gen");
    assert_eq!(files[0].package_path, "com.acme");
    assert!(files[0].source.contains("package com.acme;"));
}

#[test]
fn forward_struct_does_not_emit_file() {
    // Forward-decl + def → 1 POJO + 1 TypeSupport (a forward-decl
    // produces no files itself).
    let files = generate_java_files(
        &parse("struct Forward; struct Forward { long x; };"),
        &JavaGenOptions::default(),
    )
    .expect("gen");
    assert_eq!(files.len(), 2);
    let pojo = files
        .iter()
        .find(|f| f.class_name == "Forward")
        .expect("POJO file");
    assert!(pojo.source.contains("public class Forward"));
    assert!(files.iter().any(|f| f.class_name == "ForwardTypeSupport"));
}

#[test]
fn empty_module_emits_zero_files() {
    let files =
        generate_java_files(&parse("module M {};"), &JavaGenOptions::default()).expect("gen");
    assert!(files.is_empty());
}

#[test]
fn relative_path_uses_directory_separator() {
    let files = generate_java_files(
        &parse("module Outer { module Inner { struct S { long x; }; }; };"),
        &JavaGenOptions::default(),
    )
    .expect("gen");
    assert_eq!(files[0].relative_path(), "outer/inner/S.java");
}

#[test]
fn unsigned_workaround_widens_correctly() {
    let files = generate_java_files(
        &parse("struct S { unsigned short us; unsigned long ul; unsigned long long ull; };"),
        &JavaGenOptions::default(),
    )
    .expect("gen");
    let src = &files[0].source;
    assert!(src.contains("private int us;"));
    assert!(src.contains("private long ul;"));
    assert!(src.contains("private long ull;"));
}

#[test]
fn module_name_lowercased_in_package() {
    // The IDL convention allows `module FooBar`, the Java convention is
    // package = lowercase.
    let files = generate_java_files(
        &parse("module FooBar { struct S { long x; }; };"),
        &JavaGenOptions::default(),
    )
    .expect("gen");
    assert_eq!(files[0].package_path, "foobar");
}
