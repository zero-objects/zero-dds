// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeObject lowering + per-language rendering, extracted from
//! `zerodds-idlc` so non-CLI build-time consumers (`zerodds-build`, CMake)
//! get the identical XTypes-1.3 TypeObject block the CLI emits.
//!
//! RTI, Fast DDS and Cyclone DDS all emit the XTypes-1.3 TypeObject-/
//! TypeInformation description of a type default-on in generated code — it
//! is the basis for XTypes discovery, TypeLookup and assignability checks.
//!
//! This module lowers every named type in a `Specification` to its
//! XCDR2-LE-serialised minimal `TypeObject` and renders it as a
//! language-idiomatic constant block. Unlike the 17 `zerodds-idl-*`
//! emitter crates (which stay untouched), the emission happens as a
//! post-pass: callers append the rendered block to whatever the language
//! emitter produced.

/// A serialised minimal `TypeObject`: fully-qualified name + XCDR2-LE bytes.
pub struct TypeObjectBlob {
    /// Fully-qualified IDL name (`Robot::Pose`).
    pub fqn: String,
    /// XCDR2-LE-serialised minimal `TypeObject`.
    pub bytes: Vec<u8>,
}

/// Lowers every named type in a spec and serialises its minimal
/// `TypeObject`. Order is topological (dependencies first).
///
/// # Errors
/// A plain-text message when the TypeObject mapping fails (e.g. a
/// recursive type without SCC support, a not-yet-mappable construct).
pub fn type_object_blobs(
    spec: &zerodds_idl::ast::Specification,
) -> Result<Vec<TypeObjectBlob>, String> {
    let lowered =
        zerodds_idl::semantics::build_type_registry(spec).map_err(|e| format!("{e:?}"))?;
    let mut blobs = Vec::new();
    for (fqn, obj) in lowered.iter() {
        let bytes = obj
            .to_bytes_le()
            .map_err(|e| format!("serialising TypeObject for {fqn}: {e:?}"))?;
        blobs.push(TypeObjectBlob {
            fqn: fqn.to_string(),
            bytes,
        });
    }
    Ok(blobs)
}

/// Constant identifier from a FQN: `Robot::Pose` -> `Robot_Pose`.
fn ident(fqn: &str) -> String {
    fqn.replace("::", "_")
}

/// Byte-slice as `0xAB, 0xCD, ...`, 12 bytes per line, each line prefixed
/// with `prefix` (indentation).
fn hex_rows(bytes: &[u8], prefix: &str) -> String {
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i % 12 == 0 {
            if i != 0 {
                out.push('\n');
            }
            out.push_str(prefix);
        } else {
            out.push(' ');
        }
        out.push_str(&format!("0x{b:02x},"));
    }
    out
}

/// Java variant of [`hex_rows`]: every element is emitted as `(byte) 0xNN,`.
/// In Java `0xNN` literals are `int`; the implicit narrowing conversion to
/// `byte` is only legal (JLS SS5.2) for values in -128..127, so `0x80..0xFF`
/// without an explicit `(byte)` cast fails to compile
/// (`incompatible types: possible lossy conversion`).
fn hex_rows_java(bytes: &[u8], prefix: &str) -> String {
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i % 8 == 0 {
            if i != 0 {
                out.push('\n');
            }
            out.push_str(prefix);
        } else {
            out.push(' ');
        }
        out.push_str(&format!("(byte) 0x{b:02x},"));
    }
    out
}

const BANNER: &str = "XTypes 1.3 Minimal TypeObjects (XCDR2-LE serialised)";

/// Renders the Rust TypeObject constant block. Empty string if `blobs` is
/// empty.
#[must_use]
pub fn render_rust(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n// {BANNER}\n#[allow(dead_code)]\npub mod type_objects {{\n");
    for b in blobs {
        out.push_str(&format!(
            "    /// Minimal TypeObject for `{}`.\n    pub const {}: &[u8] = &[\n{}\n    ];\n",
            b.fqn,
            ident(&b.fqn).to_uppercase(),
            hex_rows(&b.bytes, "        ")
        ));
    }
    out.push_str("}\n");
    out
}

/// Renders the C TypeObject constant block. Empty string if `blobs` is
/// empty.
#[must_use]
pub fn render_c(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n/* {BANNER} */\n");
    for b in blobs {
        out.push_str(&format!(
            "static const unsigned char {}_type_object[] = {{\n{}\n}};\n",
            ident(&b.fqn),
            hex_rows(&b.bytes, "    ")
        ));
    }
    out
}

/// Renders the C++ TypeObject constant block. Empty string if `blobs` is
/// empty.
#[must_use]
pub fn render_cpp(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n// {BANNER}\n");
    for b in blobs {
        out.push_str(&format!(
            "inline constexpr unsigned char {}_type_object[] = {{\n{}\n}};\n",
            ident(&b.fqn),
            hex_rows(&b.bytes, "    ")
        ));
    }
    out
}

/// Renders the C# TypeObject constant block. Empty string if `blobs` is
/// empty.
#[must_use]
pub fn render_csharp(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n// {BANNER}\npublic static class TypeObjects\n{{\n");
    for b in blobs {
        out.push_str(&format!(
            "    /// <summary>Minimal TypeObject for {}.</summary>\n    \
             public static readonly byte[] {} = {{\n{}\n    }};\n",
            b.fqn,
            ident(&b.fqn),
            hex_rows(&b.bytes, "        ")
        ));
    }
    out.push_str("}\n");
    out
}

/// Renders the Java TypeObject compilation unit (`TypeObjects.java`).
/// Empty string if `blobs` is empty.
#[must_use]
pub fn render_java(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("// {BANNER}\npublic final class TypeObjects {{\n");
    out.push_str("    private TypeObjects() {}\n");
    for b in blobs {
        out.push_str(&format!(
            "    /** Minimal TypeObject for {}. */\n    \
             public static final byte[] {} = {{\n{}\n    }};\n",
            b.fqn,
            ident(&b.fqn).to_uppercase(),
            hex_rows_java(&b.bytes, "        ")
        ));
    }
    out.push_str("}\n");
    out
}

/// Renders the Python TypeObject constant block. Empty string if `blobs`
/// is empty.
#[must_use]
pub fn render_python(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n# {BANNER}\n");
    for b in blobs {
        out.push_str(&format!(
            "# Minimal TypeObject for {}.\n{} = bytes([\n{}\n])\n",
            b.fqn,
            ident(&b.fqn).to_uppercase(),
            hex_rows(&b.bytes, "    ")
        ));
    }
    out
}

/// Renders the TypeScript TypeObject constant block. Empty string if
/// `blobs` is empty.
#[must_use]
pub fn render_ts(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n// {BANNER}\n");
    for b in blobs {
        out.push_str(&format!(
            "/** Minimal TypeObject for {}. */\nexport const {} = new Uint8Array([\n{}\n]);\n",
            b.fqn,
            ident(&b.fqn),
            hex_rows(&b.bytes, "    ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn sample() -> Vec<TypeObjectBlob> {
        vec![TypeObjectBlob {
            fqn: "Robot::Pose".to_string(),
            bytes: vec![0x01, 0x02, 0xff],
        }]
    }

    #[test]
    fn ident_flattens_scope() {
        assert_eq!(ident("Robot::Pose"), "Robot_Pose");
        assert_eq!(ident("Flat"), "Flat");
    }

    #[test]
    fn render_empty_is_empty() {
        assert!(render_rust(&[]).is_empty());
    }

    #[test]
    fn rust_block_has_module_and_const() {
        let out = render_rust(&sample());
        assert!(out.contains("pub mod type_objects"));
        assert!(out.contains("pub const ROBOT_POSE: &[u8]"));
        assert!(out.contains("0xff,"));
    }

    #[test]
    fn c_block_is_static_array() {
        let out = render_c(&sample());
        assert!(out.contains("static const unsigned char Robot_Pose_type_object[]"));
    }

    #[test]
    fn java_block_is_a_class() {
        let out = render_java(&sample());
        assert!(out.contains("public final class TypeObjects"));
        assert!(out.contains("public static final byte[] ROBOT_POSE"));
    }

    #[test]
    fn java_byte_literals_are_cast_for_high_bytes() {
        // Bare `0xff,` is an int literal in Java; assigning it to a byte[]
        // element fails to compile for values > 0x7F. Each element must be
        // emitted as `(byte) 0xNN,`. 0xff is > 0x7F and must carry the cast.
        let out = render_java(&sample());
        assert!(
            out.contains("(byte) 0xff,"),
            "high byte 0xff must be cast: {out}"
        );
        assert!(
            out.contains("(byte) 0x01,"),
            "low bytes also carry the cast for consistency: {out}"
        );
        assert!(
            !out.contains(" 0xff,") || out.contains("(byte) 0xff,"),
            "no bare 0xff int-literal: {out}"
        );
    }

    #[test]
    fn python_uses_bytes_literal() {
        let out = render_python(&sample());
        assert!(out.contains("ROBOT_POSE = bytes(["));
    }

    #[test]
    fn ts_uses_uint8array() {
        let out = render_ts(&sample());
        assert!(out.contains("export const Robot_Pose = new Uint8Array(["));
    }

    #[test]
    fn csharp_block_is_a_class() {
        let out = render_csharp(&sample());
        assert!(out.contains("public static class TypeObjects"));
        assert!(out.contains("public static readonly byte[] Robot_Pose"));
    }
}
