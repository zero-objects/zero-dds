//! TS-3 — Codegen-Compile-Tests fuer Java.
//!
//! Generiert `.java`-Files aus IDL und ruft `javac -nowarn`. Faengt
//! Generated-Code-Bugs (vergessene Imports, falsche Generics-Form,
//! Bean-Pattern-Verstoesse) ab — das Snapshot-Test allein wuerde
//! kompilations-relevante Bugs nicht aufdecken.
//!
//! **Voraussetzung:** `javac` im `PATH`. Tests werden geskippt wenn
//! nicht verfuegbar.

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

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Schreibt einen minimalen Java-Runtime-Stub fuer die Symbole, die
/// der Codegen referenziert (DDS-Java-PSM `org.omg.dds.topic.TopicType`,
/// ZeroDDS-spezifische Annotations unter `org.zerodds.types.*`).
/// Reine Compile-Time-Stubs — keine Runtime-Semantik.
fn write_runtime_stubs(root: &std::path::Path) -> Result<(), String> {
    use std::io::Write;
    // org/omg/dds/topic/TopicType.java
    let topic_dir = root.join("org/omg/dds/topic");
    std::fs::create_dir_all(&topic_dir).map_err(|e| e.to_string())?;
    let mut f =
        std::fs::File::create(topic_dir.join("TopicType.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.omg.dds.topic; public interface TopicType<T> {{}}"
    )
    .map_err(|e| e.to_string())?;

    // org/omg/dds/topic/TopicTypeSupport.java — OMG-Marker.
    let mut f = std::fs::File::create(topic_dir.join("TopicTypeSupport.java"))
        .map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.omg.dds.topic; \
         import java.nio.ByteBuffer; \
         public interface TopicTypeSupport<T> {{ \
             String getTypeName(); \
             void serialize(T value, ByteBuffer buf); \
             T deserialize(ByteBuffer buf); \
         }}"
    )
    .map_err(|e| e.to_string())?;

    // org/zerodds/cdr/* — XCDR2-Helper-Stubs (compile-only).
    let cdr_dir = root.join("org/zerodds/cdr");
    std::fs::create_dir_all(&cdr_dir).map_err(|e| e.to_string())?;

    let mut f =
        std::fs::File::create(cdr_dir.join("EndianMode.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; public enum EndianMode {{ LITTLE_ENDIAN, BIG_ENDIAN }}"
    )
    .map_err(|e| e.to_string())?;

    let mut f =
        std::fs::File::create(cdr_dir.join("ExtensibilityKind.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; public enum ExtensibilityKind {{ FINAL, APPENDABLE, MUTABLE }}"
    )
    .map_err(|e| e.to_string())?;

    let mut f =
        std::fs::File::create(cdr_dir.join("XcdrException.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; \
         public final class XcdrException extends RuntimeException {{ \
             public XcdrException(String m) {{ super(m); }} \
         }}"
    )
    .map_err(|e| e.to_string())?;

    let mut f = std::fs::File::create(cdr_dir.join("Md5.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; \
         public final class Md5 {{ \
             public static byte[] hash(byte[] b) {{ return new byte[16]; }} \
         }}"
    )
    .map_err(|e| e.to_string())?;

    let mut f =
        std::fs::File::create(cdr_dir.join("Xcdr2Writer.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; \
         public final class Xcdr2Writer {{ \
             public static final int LC_BYTE = 0; \
             public static final int LC_SHORT = 1; \
             public static final int LC_INT32 = 2; \
             public static final int LC_INT64 = 3; \
             public static final int LC_NEXTINT = 4; \
             public Xcdr2Writer() {{}} \
             public Xcdr2Writer(EndianMode e) {{}} \
             public byte[] toByteArray() {{ return new byte[0]; }} \
             public int beginAppendable() {{ return 0; }} \
             public int beginMutable() {{ return 0; }} \
             public void endDelimited(int p) {{}} \
             public void writeBoolean(boolean v) {{}} \
             public void writeOctet(byte v) {{}} \
             public void writeUInt8(int v) {{}} \
             public void writeChar(char v) {{}} \
             public void writeWChar(char v) {{}} \
             public void writeInt16(short v) {{}} \
             public void writeUInt16(int v) {{}} \
             public void writeInt32(int v) {{}} \
             public void writeUInt32(long v) {{}} \
             public void writeInt64(long v) {{}} \
             public void writeUInt64(long v) {{}} \
             public void writeFloat32(float v) {{}} \
             public void writeFloat64(double v) {{}} \
             public void writeString(String s) {{}} \
             public void writeWString(String s) {{}} \
             public void writeBytes(byte[] b) {{}} \
             public void writeSequenceCount(int n) {{}} \
             public void writeEmHeader(int id, int lc, boolean mu) {{}} \
             public void writeNextInt(int n) {{}} \
             public void writePresenceFlag(boolean p) {{}} \
             public void align(int b) {{}} \
         }}"
    )
    .map_err(|e| e.to_string())?;

    let mut f =
        std::fs::File::create(cdr_dir.join("Xcdr2Reader.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; \
         public final class Xcdr2Reader {{ \
             public static final class EmHeader {{ \
                 public int memberId; public int lc; public boolean mustUnderstand; \
             }} \
             public Xcdr2Reader(byte[] b) {{}} \
             public Xcdr2Reader(byte[] b, int o, int l, EndianMode e) {{}} \
             public int position() {{ return 0; }} \
             public int remaining() {{ return 0; }} \
             public int readDHeader() {{ return 0; }} \
             public EmHeader readEmHeader() {{ return new EmHeader(); }} \
             public int readNextInt() {{ return 0; }} \
             public boolean readPresenceFlag() {{ return false; }} \
             public boolean readBoolean() {{ return false; }} \
             public byte readOctet() {{ return 0; }} \
             public int readUInt8() {{ return 0; }} \
             public char readChar() {{ return 0; }} \
             public char readWChar() {{ return 0; }} \
             public short readInt16() {{ return 0; }} \
             public int readUInt16() {{ return 0; }} \
             public int readInt32() {{ return 0; }} \
             public long readUInt32() {{ return 0; }} \
             public long readInt64() {{ return 0; }} \
             public long readUInt64() {{ return 0; }} \
             public float readFloat32() {{ return 0; }} \
             public double readFloat64() {{ return 0; }} \
             public String readString() {{ return \"\"; }} \
             public String readWString() {{ return \"\"; }} \
             public int readSequenceCount() {{ return 0; }} \
             public byte[] readBytes(int n) {{ return new byte[n]; }} \
             public void skip(int n) {{}} \
             public void align(int b) {{}} \
         }}"
    )
    .map_err(|e| e.to_string())?;

    let mut f =
        std::fs::File::create(cdr_dir.join("TopicTypeSupport.java")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "package org.zerodds.cdr; \
         import java.nio.ByteBuffer; \
         public interface TopicTypeSupport<T> extends org.omg.dds.topic.TopicTypeSupport<T> {{ \
             String getTypeName(); \
             boolean isKeyed(); \
             ExtensibilityKind getExtensibility(); \
             byte[] encode(T sample); \
             byte[] encode(T sample, EndianMode endian); \
             T decode(byte[] bytes); \
             T decode(byte[] bytes, int offset, int length); \
             byte[] keyHash(T sample); \
             default void serialize(T value, ByteBuffer buf) {{ buf.put(encode(value)); }} \
             default T deserialize(ByteBuffer buf) {{ \
                 int len = buf.remaining(); byte[] tmp = new byte[len]; buf.get(tmp); \
                 return decode(tmp); \
             }} \
         }}"
    )
    .map_err(|e| e.to_string())?;

    // org/zerodds/types/*.java — Annotations.
    let zd_dir = root.join("org/zerodds/types");
    std::fs::create_dir_all(&zd_dir).map_err(|e| e.to_string())?;
    let annotations = &[
        "Key",
        "Id",
        "Optional",
        "MustUnderstand",
        "External",
        "Nested",
        "Final",
        "Appendable",
        "Mutable",
    ];
    for ann in annotations {
        let mut f =
            std::fs::File::create(zd_dir.join(format!("{ann}.java"))).map_err(|e| e.to_string())?;
        writeln!(
            f,
            "package org.zerodds.types; \
             import java.lang.annotation.*; \
             @Retention(RetentionPolicy.RUNTIME) \
             @Target({{ElementType.TYPE, ElementType.FIELD, ElementType.METHOD}}) \
             public @interface {ann} {{ \
                 int value() default 0; \
                 String name() default \"\"; \
             }}"
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Schreibt alle generierten Java-Files in ein Temp-Dir und ruft
/// `javac` auf der Sammlung. Stub-Runtime wird vorab im selben Dir
/// abgelegt. `Ok(())` bei Erfolg, sonst stderr.
fn check_compiles(src: &str) -> Result<(), String> {
    if !javac_available() {
        eprintln!("WARNING: skipping Java compile-check, no javac in PATH");
        return Ok(());
    }

    let ast =
        zerodds_idl::parse(src, &ParserConfig::default()).map_err(|e| format!("parse: {e:?}"))?;
    let files =
        generate_java_files(&ast, &JavaGenOptions::default()).map_err(|e| format!("gen: {e:?}"))?;
    if files.is_empty() {
        return Ok(());
    }

    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    write_runtime_stubs(tmp.path())?;

    let mut paths = Vec::new();
    for f in &files {
        let dir = if f.package_path.is_empty() {
            tmp.path().to_path_buf()
        } else {
            tmp.path().join(f.package_path.replace('.', "/"))
        };
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.java", f.class_name));
        std::fs::write(&path, &f.source).map_err(|e| e.to_string())?;
        paths.push(path);
    }

    let output = Command::new("javac")
        .arg("-Xlint:none")
        .arg("-d")
        .arg(tmp.path())
        .arg("-sourcepath")
        .arg(tmp.path())
        .args(&paths)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("javac FAILED:\n--- stderr ---\n{stderr}"))
    }
}

#[test]
fn compiles_simple_struct() {
    check_compiles("struct Point { long x; long y; };").expect("simple struct must compile");
}

#[test]
fn compiles_struct_with_string_sequence() {
    check_compiles("struct Bag { string name; sequence<long> ids; };")
        .expect("string+seq must compile");
}

#[test]
fn compiles_module_nesting() {
    check_compiles("module Outer { module Inner { struct S { long x; }; }; };")
        .expect("nested modules must compile");
}

#[test]
fn compiles_enum() {
    check_compiles("enum Color { RED, GREEN, BLUE };").expect("enum must compile");
}

#[test]
fn compiles_inheritance() {
    check_compiles("struct Base { long base_field; }; struct Child : Base { long child_field; };")
        .expect("inheritance must compile");
}

#[test]
fn compiles_keyed_struct() {
    check_compiles("struct Sensor { @key long id; double value; };")
        .expect("keyed struct must compile");
}

#[test]
fn compiles_optional_member() {
    check_compiles("struct S { @optional long maybe; };").expect("optional must compile");
}

#[test]
fn compiles_union() {
    check_compiles(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    )
    .expect("union must compile");
}

#[test]
fn compiles_typedef() {
    check_compiles("typedef long Counter;").expect("typedef must compile");
}

#[test]
fn compiles_exception() {
    check_compiles("exception NotFound { string what_; };").expect("exception must compile");
}

#[test]
fn compiles_unsigned_workaround() {
    check_compiles("struct U { unsigned short us; unsigned long ul; unsigned long long ull; };")
        .expect("unsigned must compile");
}

#[test]
fn compiles_array_member() {
    check_compiles("struct M { long matrix[3][4]; };").expect("array must compile");
}
