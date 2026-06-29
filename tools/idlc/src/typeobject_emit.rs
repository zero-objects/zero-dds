// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeObject-Emission in den generierten Code.
//!
//! RTI, Fast DDS und Cyclone DDS emittieren alle drei die XTypes-1.3-
//! TypeObject-/TypeInformation-Beschreibung eines Typs default-on in
//! den generierten Code — sie ist Basis fuer XTypes-Discovery,
//! TypeLookup und Assignability-Checks.
//!
//! `zerodds-idlc` zieht nach: pro benanntem Typ wird das XCDR2-LE-
//! serialisierte Minimal-`TypeObject` (aus
//! `zerodds_idl::semantics::build_type_registry`) als Byte-Konstante in
//! den generierten Code geschrieben. Die Emission ist default-on und
//! ueber `--no-typeobject` abschaltbar.
//!
//! Die Emission passiert idlc-seitig — die sieben `zerodds-idl-*`-
//! Emitter-Crates bleiben unangetastet; pro Backend wird lediglich ein
//! sprach-idiomatischer Konstanten-Block an die Ausgabe angehaengt.

use crate::Backend;

/// Ein serialisiertes Minimal-`TypeObject`: vollqualifizierter Name +
/// XCDR2-LE-Bytes.
pub struct TypeObjectBlob {
    /// Vollqualifizierter IDL-Name (`Robot::Pose`).
    pub fqn: String,
    /// XCDR2-LE-serialisiertes Minimal-`TypeObject`.
    pub bytes: Vec<u8>,
}

/// Lowert alle benannten Typen einer Spec und serialisiert ihr
/// Minimal-`TypeObject`. Reihenfolge ist topologisch (Abhaengigkeiten
/// zuerst).
///
/// # Errors
/// Eine Klartext-Meldung, wenn das TypeObject-Mapping scheitert (z.B.
/// rekursiver Typ ohne SCC-Support, noch nicht abbildbares Konstrukt).
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

/// Konstanten-Bezeichner aus einem FQN: `Robot::Pose` → `Robot_Pose`.
fn ident(fqn: &str) -> String {
    fqn.replace("::", "_")
}

/// Byte-Slice als `0xAB, 0xCD, …`, je 12 Bytes eine Zeile, mit
/// `prefix` (Einrueckung) je Zeile.
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

/// Java-Variante von [`hex_rows`]: jedes Element wird als `(byte) 0xNN,`
/// emittiert. In Java sind `0xNN`-Literale `int`; die implizite Narrowing-
/// Konvertierung auf `byte` ist laut JLS §5.2 nur fuer Werte in -128..127
/// erlaubt, sodass `0x80..0xFF` ohne expliziten `(byte)`-Cast einen
/// Compile-Fehler `incompatible types: possible lossy conversion` ausloest.
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

/// Rendert den sprach-idiomatischen TypeObject-Konstanten-Block fuer
/// ein Backend. Leerer String, wenn es keine Typen gibt.
#[must_use]
pub fn render(backend: Backend, blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    match backend {
        Backend::Rust => render_rust(blobs),
        Backend::C => render_c(blobs),
        Backend::Cpp => render_cpp(blobs),
        Backend::CSharp => render_csharp(blobs),
        Backend::Java => render_java(blobs),
        Backend::Python => render_python(blobs),
        Backend::Ts => render_ts(blobs),
    }
}

const BANNER: &str = "XTypes 1.3 Minimal TypeObjects (XCDR2-LE serialised)";

fn render_rust(blobs: &[TypeObjectBlob]) -> String {
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

fn render_c(blobs: &[TypeObjectBlob]) -> String {
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

fn render_cpp(blobs: &[TypeObjectBlob]) -> String {
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

fn render_csharp(blobs: &[TypeObjectBlob]) -> String {
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

/// Java: eigenstaendige Compilation-Unit (idlc schreibt sie als
/// `TypeObjects.java`).
fn render_java(blobs: &[TypeObjectBlob]) -> String {
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

fn render_python(blobs: &[TypeObjectBlob]) -> String {
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

fn render_ts(blobs: &[TypeObjectBlob]) -> String {
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
        assert!(render(Backend::Rust, &[]).is_empty());
    }

    #[test]
    fn rust_block_has_module_and_const() {
        let out = render(Backend::Rust, &sample());
        assert!(out.contains("pub mod type_objects"));
        assert!(out.contains("pub const ROBOT_POSE: &[u8]"));
        assert!(out.contains("0xff,"));
    }

    #[test]
    fn c_block_is_static_array() {
        let out = render(Backend::C, &sample());
        assert!(out.contains("static const unsigned char Robot_Pose_type_object[]"));
    }

    #[test]
    fn java_block_is_a_class() {
        let out = render(Backend::Java, &sample());
        assert!(out.contains("public final class TypeObjects"));
        assert!(out.contains("public static final byte[] ROBOT_POSE"));
    }

    #[test]
    fn java_byte_literals_are_cast_for_high_bytes() {
        // Bug J #65(8): bare `0xff,` is an int literal in Java; assigning it to a
        // byte[] element fails to compile for values > 0x7F. Each element must be
        // emitted as `(byte) 0xNN,`. 0xff is > 0x7F and must carry the cast.
        let out = render(Backend::Java, &sample());
        assert!(
            out.contains("(byte) 0xff,"),
            "high byte 0xff must be cast: {out}"
        );
        assert!(
            out.contains("(byte) 0x01,"),
            "low bytes also carry the cast for consistency: {out}"
        );
        // The un-cast form must NOT appear anywhere in the Java emission.
        assert!(
            !out.contains(" 0xff,") || out.contains("(byte) 0xff,"),
            "no bare 0xff int-literal: {out}"
        );
    }

    #[test]
    fn python_uses_bytes_literal() {
        let out = render(Backend::Python, &sample());
        assert!(out.contains("ROBOT_POSE = bytes(["));
    }

    #[test]
    fn ts_uses_uint8array() {
        let out = render(Backend::Ts, &sample());
        assert!(out.contains("export const Robot_Pose = new Uint8Array(["));
    }

    #[test]
    fn csharp_block_is_a_class() {
        let out = render(Backend::CSharp, &sample());
        assert!(out.contains("public static class TypeObjects"));
        assert!(out.contains("public static readonly byte[] Robot_Pose"));
    }
}
