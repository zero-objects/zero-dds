// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 → Java-17-Source-Codegen (OMG IDL4-Java-Mapping v1.0).
//!
//! Crate `zerodds-idl-java` — Java-Sprach-Bindings, Cluster C5.4-a (Foundation)
//! plus C5.4-b (Bitset/Bitmask, Multi-Inheritance, Annotation-Bridge,
//! TopicType-Marker).
//!
//! Safety classification: **SAFE (std-only)**. Reines Build-Zeit-Tool —
//! `forbid(unsafe_code)`, kein no_std-Use-Case.
//!
//! # Scope (C5.4-a)
//! - Block A: Header-Layout (`package`, Class-Modifiers, FQN-Imports).
//! - Block B: Primitive-Mapping (boolean → boolean, octet → byte, ...,
//!   inkl. unsigned-Workaround per Spec §6).
//! - Block C: struct → public class (Bean-Pattern), enum, union →
//!   sealed interface + case-records, typedef → Wrapper-Class,
//!   sequence → `java.util.List<T>`, array → Java-Array, single
//!   inheritance → `extends`.
//! - Block D: Exception → `class X extends RuntimeException`.
//!
//! # Scope (C5.4-b — Cluster E)
//! - Bitmask → Wrapper-Class mit Inner-Enum `Flag` und
//!   `EnumSet<Flag> bits` (Spec idl4-java-1.0 §6.3).
//! - Bitset (≤ 64 Bit kumulativ) → Wrapper-Class mit `long bits` und
//!   Mask/Shift-Accessors pro Bitfield. > 64 Bit → harter Fehler
//!   [`error::JavaGenError::UnsupportedConstruct`].
//! - Multi-Inheritance via Interface-Pattern: jeder Struct, der selbst
//!   Basis eines anderen Structs ist, bekommt ein
//!   `<Name>Interface.java`-Companion. Sub-Sub-Klassen verwenden
//!   `extends DirectBase implements GrandparentInterface, ...`.
//! - `@value(N)` auf Enum-Members → expliziter `int`-Konstruktor-Wert
//!   statt Auto-Ordinal.
//! - Annotation-Bridge: `@key`, `@id(N)`, `@optional`,
//!   `@must_understand`, `@external`, `@nested`, `@extensibility(...)`
//!   → Java-Annotations unter `org.zerodds.types.*` (siehe
//!   `runtime/`).
//! - DDS-Java-PSM-Stub: jeder Top-Level-`struct` ohne `@nested`
//!   implementiert `org.omg.dds.topic.TopicType<SelfType>`.
//!
//! # Bewusst nicht im Crate
//! - Cluster F-H: ServiceEnvironment-SPI, Time/Duration/Status/QoS/
//!   Listener-Codegen (C5.5).
//! - Reflection-basierte TypeRep (java-psm §8) — Stretch-Goal.
//! - JNI-Bridge zu Rust-Core (C5.5).
//! - `interface`, `valuetype`, `fixed`, `any`, `map<K,V>` → kommen
//!   mit `zerodds-rpc-java`.
//!
//! # Multi-File-Output
//! Java erfordert eine `.java`-Datei pro top-level public class.
//! Daher gibt [`generate_java_files`] eine [`Vec<JavaFile>`] zurueck;
//! jede `JavaFile` hat package-path + class-name + source.
//!
//! # Beispiel
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

/// Konfiguration des Java-Code-Generators.
#[derive(Debug, Clone)]
pub struct JavaGenOptions {
    /// Java-Root-Package, in das alle generierten Klassen gehoeren
    /// (z.B. `"org.example.types"`). Leer-String = Default-Package.
    pub root_package: String,
    /// Indent-Breite in Leerzeichen. Default 4.
    pub indent_width: usize,
    /// Wenn `true`, werden flache Aggregat-Types als Java `record`
    /// emittiert (Java 14+). Default `false` — Spec verlangt
    /// Bean-Pattern.
    pub use_records: bool,
    /// Spec §7.2.3 / §8.1.2 / §8.1.3 — opt-in: emittiert pro
    /// Top-Level-Struct/Union eine zusätzliche `<TypeName>AmqpCodec.java`-
    /// Datei mit statischen `toAmqpValue` / `toJsonString`-Helpern.
    /// Default `false`, weil die emittierten Calls eine
    /// `org.zerodds.amqp`-Runtime-Library voraussetzen.
    pub emit_amqp_helpers: bool,
    /// Annex A.1 (idl4-java-1.0) — opt-in: emittiert pro
    /// Top-Level-Type eine zusaetzliche `<TypeName>CorbaTraits.java`-
    /// Datei mit per-Type-Konstanten (`FULL_NAME`, `IS_VARIABLE_SIZE`,
    /// `IS_LOCAL`). Default `false`.
    pub emit_corba_traits: bool,
    /// Spec zerodds-xcdr2-java-1.0 §4 — opt-in: emittiert pro
    /// Top-Level-Struct eine zusaetzliche `<TypeName>TypeSupport.java`-
    /// Datei mit `org.zerodds.cdr.TopicTypeSupport<T>`-
    /// Implementierung (encode/decode/keyHash + INSTANCE).
    /// Default `true` ab v1.0 (Spec-Pflicht).
    pub emit_typesupport: bool,
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
        }
    }
}

/// Erzeugt eine Liste von Java-Source-Files aus einer IDL-Specification.
///
/// # Errors
/// - [`JavaGenError::UnsupportedConstruct`]: IDL-Konstrukt außerhalb des aktuellen Scopes
///   (z.B. `interface`, `valuetype`, `fixed`, `any`) oder C5.4-b-
///   Constraint verletzt (z.B. Bitset-Summe > 64 Bit, Bitmask-
///   `bit_bound > 64`).
/// - [`JavaGenError::InvalidName`]: Ein Identifier ist leer oder
///   kollidiert nach Sanitisierung weiterhin mit einem Java-Keyword.
/// - [`JavaGenError::InheritanceCycle`]: Direkte oder indirekte
///   Self-Inheritance im Struct-Graphen.
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

/// Convenience-Variante mit aktiviertem `emit_corba_traits`-Flag.
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

/// Convenience-Variante mit aktiviertem `emit_amqp_helpers`-Flag.
///
/// Identisch zu [`generate_java_files`], aber zwingt
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
        // Inline-Tests pruefen das POJO-Emitter-Verhalten — TypeSupport
        // hat eigene Tests in `typesupport`-Modul + Snapshots.
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
        // Module ohne Type-Defs erzeugt keine Java-File (Java hat keinen
        // package-marker-File).
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
        // Unsigned-Workaround:
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
        // `any`-Member ist ausserhalb des TypeSupport-Scopes (v1.0);
        // die POJO emittiert weiterhin `Object`, TypeSupport entfaellt.
        let ast = zerodds_idl::parse("struct S { any value; };", &ParserConfig::default())
            .expect("parse");
        let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("ok");
        let combined: String = files.iter().map(|f| f.source.clone()).collect();
        assert!(combined.contains("Object"));
        // Keine TypeSupport-Generation fuer `any`.
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
