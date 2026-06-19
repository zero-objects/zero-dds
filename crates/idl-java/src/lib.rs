// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 → Java 17 source codegen (OMG IDL4-Java mapping v1.0).
//!
//! Crate `zerodds-idl-java` — Java language bindings, cluster C5.4-a (foundation)
//! plus C5.4-b (bitset/bitmask, multi-inheritance, annotation bridge,
//! TopicType marker).
//!
//! Safety classification: **SAFE (std-only)**. A pure build-time tool —
//! `forbid(unsafe_code)`, no no_std use case.
//!
//! # Scope (C5.4-a)
//! - Block A: header layout (`package`, class modifiers, FQN imports).
//! - Block B: primitive mapping (boolean → boolean, octet → byte, ...,
//!   incl. unsigned workaround per spec §6).
//! - Block C: struct → public class (bean pattern), enum, union →
//!   sealed interface + case records, typedef → wrapper class,
//!   sequence → `java.util.List<T>`, array → Java array, single
//!   inheritance → `extends`.
//! - Block D: Exception → `class X extends RuntimeException`.
//!
//! # Java version targets
//! The standard emit targets **Java 17**: unions use a `sealed
//! interface` with `record` case types. Structs/enums/typedefs are bean
//! classes and thus version-neutral.
//!
//! The opt-in **Java-8 compat mode** ([`JavaGenOptions::java8_compat`])
//! avoids all Java-9+ constructs: unions are instead emitted as an
//! `abstract class` with a private constructor (pseudo-sealing) +
//! `static final` subclasses (final field + constructor + same-named
//! accessor). Everything else is identical in both modes.
//!
//! # Scope (C5.4-b — Cluster E)
//! - Bitmask → wrapper class with an inner enum `Flag` and
//!   `EnumSet<Flag> bits` (spec idl4-java-1.0 §6.3).
//! - Bitset (≤ 64 bits cumulative) → wrapper class with `long bits` and
//!   mask/shift accessors per bitfield. > 64 bits → hard error
//!   [`error::JavaGenError::UnsupportedConstruct`].
//! - Multi-inheritance via an interface pattern: every struct that is itself
//!   the base of another struct gets a
//!   `<Name>Interface.java` companion. Sub-sub-classes use
//!   `extends DirectBase implements GrandparentInterface, ...`.
//! - `@value(N)` on enum members → an explicit `int` constructor value
//!   instead of the auto ordinal.
//! - Annotation bridge: `@key`, `@id(N)`, `@optional`,
//!   `@must_understand`, `@external`, `@nested`, `@extensibility(...)`
//!   → Java annotations under `org.zerodds.types.*` (see
//!   `runtime/`).
//! - DDS Java PSM stub: every top-level `struct` without `@nested`
//!   implements `org.omg.dds.topic.TopicType<SelfType>`.
//!
//! # Deliberately not in the crate
//! - Clusters F-H: ServiceEnvironment SPI, Time/Duration/Status/QoS/
//!   listener codegen (C5.5).
//! - Reflection-based TypeRep (java-psm §8) — a stretch goal.
//! - `interface`, `valuetype`, `fixed`, `any`, `map<K,V>` → come with
//!   `zerodds-rpc-java`.
//!
//! # Multi-file output
//! Java requires one `.java` file per top-level public class.
//! Therefore [`generate_java_files`] returns a [`Vec<JavaFile>`]; each
//! `JavaFile` has a package path + class name + source.
//!
//! # Example
//!
//! ```
//! use zerodds_idl::config::ParserConfig;
//! use zerodds_idl_java::{generate_java_files, JavaGenOptions};
//!
//! let ast = zerodds_idl::parse(
//!     "module M { struct S { long x; }; };",
//!     &ParserConfig::default(),
//! )
//! .expect("parse");
//! let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
//! // POJO + TypeSupport (zerodds-xcdr2-java-1.0 §4).
//! assert_eq!(files.len(), 2);
//! let pojo = files.iter().find(|f| f.class_name == "S").expect("POJO");
//! assert!(pojo.source.contains("package m;"));
//! assert!(pojo.source.contains("public class S"));
//! assert!(files.iter().any(|f| f.class_name == "STypeSupport"));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub(crate) mod amqp;
pub(crate) mod annotations;
pub(crate) mod bitset;
pub(crate) mod corba_traits;
pub mod emitter;
pub mod error;
pub mod keywords;
pub mod rpc;
pub mod type_map;
pub(crate) mod typesupport;
pub(crate) mod verbatim;

pub use emitter::JavaFile;
pub use error::JavaGenError;

use zerodds_idl::ast::Specification;

/// Configuration of the Java code generator.
#[derive(Debug, Clone)]
pub struct JavaGenOptions {
    /// Java root package that all generated classes belong to
    /// (e.g. `"org.example.types"`). Empty string = default package.
    pub root_package: String,
    /// Indent width in spaces. Default 4.
    pub indent_width: usize,
    /// If `true`, flat aggregate types are emitted as Java `record`
    /// (Java 14+). Default `false` — the spec requires the bean pattern.
    pub use_records: bool,
    /// Spec §7.2.3 / §8.1.2 / §8.1.3 — opt-in: emits per top-level
    /// struct/union an additional `<TypeName>AmqpCodec.java` file with
    /// static `toAmqpValue` / `toJsonString` helpers. Default `false`,
    /// because the emitted calls require an `org.zerodds.amqp` runtime
    /// library.
    pub emit_amqp_helpers: bool,
    /// Annex A.1 (idl4-java-1.0) — opt-in: emits per top-level type an
    /// additional `<TypeName>CorbaTraits.java` file with per-type
    /// constants (`FULL_NAME`, `IS_VARIABLE_SIZE`, `IS_LOCAL`).
    /// Default `false`.
    pub emit_corba_traits: bool,
    /// Spec zerodds-xcdr2-java-1.0 §4 — opt-in: emits per top-level
    /// struct an additional `<TypeName>TypeSupport.java` file with an
    /// `org.zerodds.cdr.TopicTypeSupport<T>` implementation
    /// (encode/decode/keyHash + INSTANCE). Default `true` from v1.0
    /// (spec-mandatory).
    pub emit_typesupport: bool,
    /// Java-8 compat mode — opt-in. If `true`, the emitter avoids all
    /// Java-9+ constructs: unions are emitted as an `abstract class` with
    /// a private constructor (pseudo-sealing) + `static final` subclasses
    /// instead of as a `sealed interface` + `record` (Java 17).
    /// Structs are bean classes anyway (`use_records=false`) and thus
    /// already Java-8-capable. Default `false` (standard = Java 17).
    pub java8_compat: bool,
}

impl Default for JavaGenOptions {
    fn default() -> Self {
        Self {
            root_package: String::new(),
            indent_width: 4,
            use_records: false,
            emit_amqp_helpers: false,
            emit_corba_traits: false,
            emit_typesupport: true,
            java8_compat: false,
        }
    }
}

/// Produces a list of Java source files from an IDL specification.
///
/// # Errors
/// - [`JavaGenError::UnsupportedConstruct`]: IDL construct outside the current scope
///   (e.g. `interface`, `valuetype`, `fixed`, `any`) or a C5.4-b
///   constraint violated (e.g. bitset sum > 64 bit, bitmask
///   `bit_bound > 64`).
/// - [`JavaGenError::InvalidName`]: an identifier is empty or still
///   collides with a Java keyword after sanitization.
/// - [`JavaGenError::InheritanceCycle`]: direct or indirect
///   self-inheritance in the struct graph.
pub fn generate_java_files(
    ast: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let mut files = emitter::emit_files(ast, opts)?;
    if opts.emit_amqp_helpers {
        files.extend(amqp::emit_amqp_codec_files(ast, opts)?);
    }
    if opts.emit_corba_traits {
        files.extend(corba_traits::emit_corba_traits_files(ast, opts)?);
    }
    if opts.emit_typesupport {
        files.extend(typesupport::emit_typesupport_files(ast, opts)?);
    }
    Ok(files)
}

/// Convenience variant with the `emit_corba_traits` flag enabled.
///
/// Cross-Ref: `idl4-java-1.0` Annex A.1.
///
/// # Errors
/// Wie [`generate_java_files`].
pub fn generate_java_files_with_corba_traits(
    ast: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let opts = JavaGenOptions {
        emit_corba_traits: true,
        ..opts.clone()
    };
    generate_java_files(ast, &opts)
}

/// Convenience variant with the `emit_amqp_helpers` flag enabled.
///
/// Identical to [`generate_java_files`], but forces
/// `opts.emit_amqp_helpers = true`.
///
/// # Errors
/// Wie [`generate_java_files`].
pub fn generate_java_files_with_amqp(
    ast: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let opts = JavaGenOptions {
        emit_amqp_helpers: true,
        ..opts.clone()
    };
    generate_java_files(ast, &opts)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_idl::config::ParserConfig;

    fn gen_java(src: &str) -> Vec<JavaFile> {
        // Inline tests check the POJO emitter behavior — TypeSupport
        // has its own tests in the `typesupport` module + snapshots.
        let opts = JavaGenOptions {
            emit_typesupport: false,
            ..Default::default()
        };
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed");
        generate_java_files(&ast, &opts).expect("gen must succeed")
    }

    fn gen_with(src: &str, opts: &JavaGenOptions) -> Vec<JavaFile> {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed");
        generate_java_files(&ast, opts).expect("gen must succeed")
    }

    #[test]
    fn empty_source_emits_no_files() {
        assert!(gen_java("").is_empty());
    }

    #[test]
    fn empty_module_emits_no_files() {
        // A module without type defs produces no Java file (Java has no
        // package-marker file).
        assert!(gen_java("module M {};").is_empty());
    }

    #[test]
    fn struct_emits_one_file_per_type() {
        let files = gen_java("struct A { long x; }; struct B { long y; }; struct C { long z; };");
        assert_eq!(files.len(), 3);
        let names: Vec<&str> = files.iter().map(|f| f.class_name.as_str()).collect();
        assert!(names.contains(&"A"));
        assert!(names.contains(&"B"));
        assert!(names.contains(&"C"));
    }

    #[test]
    fn module_becomes_lowercase_package() {
        let files = gen_java("module Foo { struct S { long x; }; };");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].package_path, "foo");
        assert!(files[0].source.contains("package foo;"));
    }

    #[test]
    fn three_level_modules_become_three_packages() {
        let files = gen_java("module A { module B { module C { struct S { long x; }; }; }; };");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].package_path, "a.b.c");
        assert!(files[0].source.contains("package a.b.c;"));
    }

    #[test]
    fn primitive_struct_uses_correct_java_types() {
        let files = gen_java(
            "struct S { boolean b; octet o; short s; long l; long long ll; \
             unsigned short us; unsigned long ul; unsigned long long ull; \
             float f; double d; char c; wchar wc; string str; };",
        );
        let src = &files[0].source;
        assert!(src.contains("private boolean b;"));
        assert!(src.contains("private byte o;"));
        assert!(src.contains("private short s;"));
        assert!(src.contains("private int l;"));
        assert!(src.contains("private long ll;"));
        // Unsigned workaround:
        assert!(src.contains("private int us;"));
        assert!(src.contains("private long ul;"));
        assert!(src.contains("private long ull;"));
        assert!(src.contains("private float f;"));
        assert!(src.contains("private double d;"));
        assert!(src.contains("private char c;"));
        assert!(src.contains("private char wc;"));
        assert!(src.contains("private String str;"));
    }

    #[test]
    fn unsigned_member_gets_doc_comment() {
        let files = gen_java("struct S { unsigned long u; };");
        assert!(files[0].source.contains("unsigned IDL value"));
    }

    #[test]
    fn enum_emits_explicit_values() {
        let files = gen_java("enum Color { RED, GREEN, BLUE };");
        let src = &files[0].source;
        assert!(src.contains("public enum Color {"));
        assert!(src.contains("RED(0),"));
        assert!(src.contains("GREEN(1),"));
        assert!(src.contains("BLUE(2);"));
        assert!(src.contains("public int value()"));
    }

    #[test]
    fn union_emits_sealed_interface() {
        let files = gen_java(
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        );
        let src = &files[0].source;
        assert!(src.contains("public sealed interface U"));
        assert!(src.contains("permits"));
        assert!(src.contains("record A(int a) implements U"));
        assert!(src.contains("record B(double b) implements U"));
        assert!(src.contains("// case default"));
    }

    #[test]
    fn typedef_emits_wrapper_class() {
        let files = gen_java("typedef long Counter;");
        assert_eq!(files.len(), 1);
        let src = &files[0].source;
        assert!(src.contains("public final class Counter"));
        assert!(src.contains("private int value;"));
    }

    #[test]
    fn sequence_uses_list() {
        let files = gen_java("struct Bag { sequence<long> items; };");
        let src = &files[0].source;
        assert!(src.contains("private java.util.List<Integer> items;"));
    }

    #[test]
    fn array_uses_java_array_syntax() {
        let files = gen_java("struct M { long cells[3][4]; };");
        let src = &files[0].source;
        assert!(src.contains("private int[][] cells;"));
    }

    #[test]
    fn inherited_struct_uses_extends() {
        let files = gen_java("struct Parent { long x; }; struct Child : Parent { long y; };");
        let child = files
            .iter()
            .find(|f| f.class_name == "Child")
            .expect("Child file");
        assert!(child.source.contains("public class Child extends Parent"));
    }

    #[test]
    fn keyed_member_emits_key_annotation() {
        let files = gen_java("struct S { @key long id; long val; };");
        assert!(files[0].source.contains("@org.zerodds.types.Key"));
    }

    #[test]
    fn optional_member_uses_optional() {
        let files = gen_java("struct S { @optional long maybe; };");
        let src = &files[0].source;
        assert!(src.contains("java.util.Optional<Integer> maybe"));
    }

    #[test]
    fn exception_extends_runtime_exception() {
        let files = gen_java("exception NotFound { string what_; };");
        let src = &files[0].source;
        assert!(src.contains("public class NotFound extends RuntimeException"));
        assert!(src.contains("public NotFound(String message)"));
    }

    #[test]
    fn reserved_member_name_gets_underscore_suffix() {
        let files = gen_java("struct S { long class; };");
        let src = &files[0].source;
        assert!(src.contains("class_"));
        assert!(src.contains("getClass_"));
    }

    #[test]
    fn non_service_interface_emits_java_interface() {
        let opts = JavaGenOptions {
            emit_typesupport: false,
            ..Default::default()
        };
        let ast = zerodds_idl::parse("interface I { void op(); };", &ParserConfig::default())
            .expect("parse");
        let files = generate_java_files(&ast, &opts).expect("ok");
        let combined: String = files.iter().map(|f| f.source.clone()).collect();
        assert!(combined.contains("public interface I"));
    }

    #[test]
    fn any_member_emits_object() {
        // `any` members are outside the TypeSupport scope (v1.0);
        // the POJO still emits `Object`, TypeSupport is omitted.
        let ast = zerodds_idl::parse("struct S { any value; };", &ParserConfig::default())
            .expect("parse");
        let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("ok");
        let combined: String = files.iter().map(|f| f.source.clone()).collect();
        assert!(combined.contains("Object"));
        // No TypeSupport generation for `any`.
        assert!(!combined.contains("STypeSupport"));
    }

    #[test]
    fn root_package_prepends_to_modules() {
        let opts = JavaGenOptions {
            root_package: "org.example".into(),
            ..Default::default()
        };
        let files = gen_with("module Inner { struct S { long x; }; };", &opts);
        assert_eq!(files[0].package_path, "org.example.inner");
    }

    #[test]
    fn relative_path_uses_package_directory() {
        let files = gen_java("module M { struct S { long x; }; };");
        assert_eq!(files[0].relative_path(), "m/S.java");
    }

    #[test]
    fn relative_path_default_package() {
        let files = gen_java("struct S { long x; };");
        assert_eq!(files[0].relative_path(), "S.java");
    }

    #[test]
    fn inheritance_cycle_is_rejected() {
        let ast = zerodds_idl::parse(
            "struct A : B { long a; };\n\
             struct B : A { long b; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        let res = generate_java_files(&ast, &JavaGenOptions::default());
        assert!(matches!(res, Err(JavaGenError::InheritanceCycle { .. })));
    }

    #[test]
    fn options_have_sensible_defaults() {
        let o = JavaGenOptions::default();
        assert_eq!(o.indent_width, 4);
        assert!(o.root_package.is_empty());
        assert!(!o.use_records);
    }

    #[test]
    fn options_clone_works() {
        let o = JavaGenOptions {
            root_package: "foo.bar".into(),
            indent_width: 2,
            use_records: true,
            emit_amqp_helpers: false,
            emit_corba_traits: false,
            emit_typesupport: true,
            java8_compat: false,
        };
        let cloned = o.clone();
        assert_eq!(cloned.indent_width, 2);
        assert_eq!(cloned.root_package, "foo.bar");
        assert!(cloned.use_records);
    }

    #[test]
    fn java_file_struct_field_access() {
        let files = gen_java("struct S { long x; };");
        assert_eq!(files[0].class_name, "S");
        assert_eq!(files[0].package_path, "");
        assert!(files[0].source.contains("public class S"));
    }

    #[test]
    fn const_decl_emits_holder_class() {
        let files = gen_java("const long MAX = 100;");
        let src = &files[0].source;
        assert!(src.contains("public final class MAXConstant"));
        assert!(src.contains("public static final int MAX = 100;"));
    }

    #[test]
    fn each_file_starts_with_generated_marker() {
        for f in gen_java("struct S { long x; };") {
            assert!(f.source.starts_with("// Generated by zerodds idl-java."));
        }
    }
}
