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

use std::collections::HashSet;

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

/// Constant identifier from a FQN via the shared injective encoding:
/// `Robot::Pose` -> `Robot_sPose`, `Robot::My_Type` -> `Robot_sMy_uType`.
/// Injective and case-preserving (see [`zerodds_idl::naming`]); `A::B_C` and
/// `A_B::C` no longer collapse to the same identifier.
fn ident(fqn: &str) -> String {
    zerodds_idl::naming::encode_ident(fqn)
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
/// `byte` is only legal (JLS §5.2) for values in -128..127, so `0x80..0xFF`
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

/// A valid identifier fragment from an IDL basename stem, via the shared
/// injective encoding ([`zerodds_idl::naming::encode_ident`]): `my-idl` ->
/// `my_hidl`, `v2.types` -> `v2_dtypes`, `9lives` -> `_n9lives`. Unlike the
/// old lossy sanitiser (`-`/`_` both `_`, case folded) this never conflates
/// two distinct stems, so `a-b.idl` and `a_b.idl` yield distinct holders.
/// An empty stem falls back to `TypeObjects`.
fn sanitize_base(base: &str) -> String {
    if base.is_empty() {
        return "TypeObjects".to_string();
    }
    zerodds_idl::naming::encode_ident(base)
}

/// Collects the raw names of every outermost (top-level) definition in `spec`.
/// Used by the synthetic-holder preflight so the generated `TypeObjects` /
/// `type_objects` holder never silently shadows a user symbol that a backend
/// emits at the same crate-root / namespace / package level.
fn top_level_names(spec: &zerodds_idl::ast::Specification) -> HashSet<String> {
    use zerodds_idl::ast::types::{
        ConstrTypeDecl, Definition, InterfaceDcl, StructDcl, TypeDecl, UnionDcl,
    };
    let mut set = HashSet::new();
    for def in &spec.definitions {
        match def {
            Definition::Module(m) => {
                set.insert(m.name.text.clone());
            }
            Definition::Const(c) => {
                set.insert(c.name.text.clone());
            }
            Definition::Except(e) => {
                set.insert(e.name.text.clone());
            }
            Definition::Interface(InterfaceDcl::Def(i)) => {
                set.insert(i.name.text.clone());
            }
            Definition::Interface(InterfaceDcl::Forward(i)) => {
                set.insert(i.name.text.clone());
            }
            Definition::Type(TypeDecl::Constr(c)) => {
                let name = match c {
                    ConstrTypeDecl::Struct(StructDcl::Def(s)) => &s.name.text,
                    ConstrTypeDecl::Struct(StructDcl::Forward(s)) => &s.name.text,
                    ConstrTypeDecl::Union(UnionDcl::Def(u)) => &u.name.text,
                    ConstrTypeDecl::Union(UnionDcl::Forward(u)) => &u.name.text,
                    ConstrTypeDecl::Enum(e) => &e.name.text,
                    ConstrTypeDecl::Bitset(b) => &b.name.text,
                    ConstrTypeDecl::Bitmask(b) => &b.name.text,
                };
                set.insert(name.clone());
            }
            Definition::Type(TypeDecl::Typedef(t)) => {
                for d in &t.declarators {
                    set.insert(d.name().text.clone());
                }
            }
            Definition::Type(TypeDecl::Native(n)) => {
                set.insert(n.name.text.clone());
            }
            _ => {}
        }
    }
    set
}

/// Disambiguates a synthetic holder `candidate` against the set of `taken`
/// user symbols: if it collides, append `_x` (a reserved escape fragment that
/// cannot appear in an encoded user name mid-token) until the result is free.
/// Deterministic and terminating.
fn disambiguate(mut candidate: String, taken: &HashSet<String>) -> String {
    while taken.contains(&candidate) {
        candidate.push_str("_x");
    }
    candidate
}

/// Name of the per-file `TypeObjects` holder class emitted for C# and Java
/// (`TypeObjects_common`, `TypeObjects_metrics`). Derived from the base IDL
/// name via the shared injective encoding so the holder stays globally unique
/// when several generated files are composed into one namespace / package —
/// and disambiguated against `spec`'s top-level user symbols so it never
/// silently shadows a user type named `TypeObjects_<base>` (C# CS0101 /
/// Java duplicate-class, or worse, an overwrite). For Java the file name must
/// equal the public class name, so `zerodds-idlc` uses this same helper for
/// the `<name>.java` file it writes.
#[must_use]
pub fn type_objects_class_name(spec: &zerodds_idl::ast::Specification, base: &str) -> String {
    let candidate = format!("TypeObjects_{}", sanitize_base(base));
    disambiguate(candidate, &top_level_names(spec))
}

/// Name of the per-file Rust `type_objects` module (`type_objects_common`,
/// `type_objects_metrics`). Derived from the base IDL name via the shared
/// injective encoding — case is **preserved** (not lowercased) so the module
/// name stays injective (`My_Type` and `my_type` must not collapse); the
/// rendered `pub mod` therefore carries `#[allow(non_snake_case)]`. Keeps the
/// module globally unique when several generated files are composed via
/// `include!` into one parent module (E0428, GH #25), and is disambiguated
/// against `spec`'s top-level user symbols so it never shadows a user
/// `module type_objects_<base>`.
#[must_use]
pub fn type_objects_mod_name(spec: &zerodds_idl::ast::Specification, base: &str) -> String {
    let candidate = format!("type_objects_{}", sanitize_base(base));
    disambiguate(candidate, &top_level_names(spec))
}

/// Renders the Rust TypeObject constant block. Empty string if `blobs` is
/// empty.
#[must_use]
pub fn render_rust(
    blobs: &[TypeObjectBlob],
    spec: &zerodds_idl::ast::Specification,
    base: &str,
) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mod_name = type_objects_mod_name(spec, base);
    // `non_snake_case`: the injective module name may keep upper-case bytes.
    // `non_upper_case_globals`: the injective const names keep their source
    // case and escape fragments (`Robot_sPose`), never SCREAMING_CASE.
    let mut out = format!(
        "\n// {BANNER}\n#[allow(dead_code, non_snake_case)]\npub mod {mod_name} {{\n    \
         #![allow(non_upper_case_globals)]\n"
    );
    for b in blobs {
        out.push_str(&format!(
            "    /// Minimal TypeObject for `{}`.\n    pub const {}: &[u8] = &[\n{}\n    ];\n",
            b.fqn,
            ident(&b.fqn),
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
/// empty. `spec`/`base` derive and disambiguate the holder class
/// ([`type_objects_class_name`]) so multi-file compose into one namespace
/// does not collide (CS0101) and no user symbol is shadowed.
#[must_use]
pub fn render_csharp(
    blobs: &[TypeObjectBlob],
    spec: &zerodds_idl::ast::Specification,
    base: &str,
) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let class = type_objects_class_name(spec, base);
    let mut out = format!("\n// {BANNER}\npublic static class {class}\n{{\n");
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

/// Renders the Java TypeObject compilation unit (`TypeObjects_<base>.java`).
/// Empty string if `blobs` is empty. `spec`/`base` derive and disambiguate
/// the public class name ([`type_objects_class_name`]) so composing several
/// generated files into one package yields no duplicate class and no user
/// symbol is shadowed — and Java's file name must equal that public class
/// name.
#[must_use]
pub fn render_java(
    blobs: &[TypeObjectBlob],
    spec: &zerodds_idl::ast::Specification,
    base: &str,
) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let class = type_objects_class_name(spec, base);
    let mut out = format!("// {BANNER}\npublic final class {class} {{\n");
    out.push_str(&format!("    private {class}() {{}}\n"));
    for b in blobs {
        out.push_str(&format!(
            "    /** Minimal TypeObject for {}. */\n    \
             public static final byte[] {} = {{\n{}\n    }};\n",
            b.fqn,
            ident(&b.fqn),
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
            ident(&b.fqn),
            hex_rows(&b.bytes, "    ")
        ));
    }
    out
}

/// Renders the TypeScript TypeObject constant block. Empty string if
/// `blobs` is empty.
///
/// The constant carries a `_TYPE_OBJECT` suffix (`NarrowEnum_TYPE_OBJECT`).
/// Unlike Rust (`pub mod`), C#/Java (holder class) or C/C++ (`_type_object`
/// suffix), the TS post-pass emits `export const` at module scope — the same
/// value namespace the enum- and bitmask-emitters use for their
/// `export const <Name> = { ... }` object. A bare `ident(fqn)` here therefore
/// re-declares that block-scoped variable and the generated file fails to
/// compile (`tsc` TS2451 "Cannot redeclare block-scoped variable"). The
/// suffix keeps the TypeObject byte-array in a collision-free identifier that
/// no backend emitter produces, so enum/bitmask value exports and the
/// TypeObject export coexist. (struct/union/bitset declare their type name in
/// the *type* namespace — `interface`/`type` — so a same-named `const` would
/// merge legally there; the suffix stays uniform across all kinds anyway.)
#[must_use]
pub fn render_ts(blobs: &[TypeObjectBlob]) -> String {
    if blobs.is_empty() {
        return String::new();
    }
    let mut out = format!("\n// {BANNER}\n");
    for b in blobs {
        out.push_str(&format!(
            "/** Minimal TypeObject for {}. */\nexport const {}_TYPE_OBJECT = new Uint8Array([\n{}\n]);\n",
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

    fn parse(idl: &str) -> zerodds_idl::ast::Specification {
        zerodds_idl::parser::parse(idl, &zerodds_idl::config::ParserConfig::default())
            .expect("parse")
    }

    fn empty_spec() -> zerodds_idl::ast::Specification {
        parse("")
    }

    fn sample() -> Vec<TypeObjectBlob> {
        vec![TypeObjectBlob {
            fqn: "Robot::Pose".to_string(),
            bytes: vec![0x01, 0x02, 0xff],
        }]
    }

    #[test]
    fn ident_encodes_scope_injectively() {
        assert_eq!(ident("Robot::Pose"), "Robot_sPose");
        assert_eq!(ident("Flat"), "Flat");
        // The core bug: `A::B_C` and `A_B::C` must not collapse.
        assert_ne!(ident("A::B_C"), ident("A_B::C"));
    }

    #[test]
    fn render_empty_is_empty() {
        assert!(render_rust(&[], &empty_spec(), "any").is_empty());
    }

    #[test]
    fn rust_block_has_module_and_const() {
        let out = render_rust(&sample(), &empty_spec(), "robot");
        assert!(out.contains("pub mod type_objects_robot"));
        assert!(out.contains("pub const Robot_sPose: &[u8]"));
        assert!(out.contains("#![allow(non_upper_case_globals)]"));
        assert!(out.contains("0xff,"));
    }

    #[test]
    fn rust_mod_name_is_per_base_unique() {
        // Multi-file include! into one parent mod must not collide (GH #25).
        let e = empty_spec();
        assert_eq!(type_objects_mod_name(&e, "common"), "type_objects_common");
        assert_eq!(type_objects_mod_name(&e, "metrics"), "type_objects_metrics");
        assert_ne!(
            type_objects_mod_name(&e, "common"),
            type_objects_mod_name(&e, "metrics")
        );
        // `-` and `_` no longer conflate (was the `a-b`/`a_b` E0428 bug).
        assert_ne!(
            type_objects_mod_name(&e, "a-b"),
            type_objects_mod_name(&e, "a_b")
        );
        // Case is preserved (no lowercasing) so injectivity holds.
        assert_eq!(
            type_objects_mod_name(&e, "V2.Types"),
            "type_objects_V2_dTypes"
        );
    }

    #[test]
    fn holder_preflight_avoids_user_symbol() {
        // A user `module type_objects_input` must not be shadowed by the
        // synthetic Rust holder for base `input`.
        let spec = parse("module type_objects_input { };");
        let name = type_objects_mod_name(&spec, "input");
        assert_ne!(name, "type_objects_input");
        assert!(name.starts_with("type_objects_input_x"));

        // Same for a C#/Java user `struct TypeObjects_input`.
        let spec2 = parse("struct TypeObjects_input { long x; };");
        let cls = type_objects_class_name(&spec2, "input");
        assert_ne!(cls, "TypeObjects_input");
        assert!(cls.starts_with("TypeObjects_input_x"));
    }

    #[test]
    fn c_block_is_static_array() {
        let out = render_c(&sample());
        assert!(out.contains("static const unsigned char Robot_sPose_type_object[]"));
    }

    #[test]
    fn java_block_is_a_class() {
        let out = render_java(&sample(), &empty_spec(), "chat");
        assert!(out.contains("public final class TypeObjects_chat"));
        assert!(out.contains("private TypeObjects_chat()"));
        assert!(out.contains("public static final byte[] Robot_sPose"));
    }

    #[test]
    fn java_byte_literals_are_cast_for_high_bytes() {
        // Bare `0xff,` is an int literal in Java; assigning it to a byte[]
        // element fails to compile for values > 0x7F. Each element must be
        // emitted as `(byte) 0xNN,`. 0xff is > 0x7F and must carry the cast.
        let out = render_java(&sample(), &empty_spec(), "chat");
        assert!(
            out.contains("(byte) 0xff,"),
            "high byte 0xff must be cast: {out}"
        );
        assert!(
            out.contains("(byte) 0x01,"),
            "low bytes also carry the cast for consistency: {out}"
        );
    }

    #[test]
    fn python_uses_bytes_literal() {
        let out = render_python(&sample());
        assert!(out.contains("Robot_sPose = bytes(["));
    }

    #[test]
    fn ts_uses_uint8array() {
        let out = render_ts(&sample());
        assert!(out.contains("export const Robot_sPose_TYPE_OBJECT = new Uint8Array(["));
    }

    #[test]
    fn ts_typeobject_symbol_is_suffixed_to_avoid_enum_bitmask_redeclaration() {
        // P0-8: the enum- and bitmask-emitters emit `export const <Name> = {…}`
        // in the value namespace at module scope. A bare TypeObject
        // `export const <Name>` would re-declare that block-scoped variable
        // (tsc TS2451). The `_TYPE_OBJECT` suffix keeps the two collision-free.
        let blobs = vec![
            TypeObjectBlob {
                fqn: "NarrowEnum".to_string(),
                bytes: vec![0x01, 0x02],
            },
            TypeObjectBlob {
                fqn: "Flags".to_string(),
                bytes: vec![0x03, 0x04],
            },
        ];
        let out = render_ts(&blobs);
        assert!(out.contains("export const NarrowEnum_TYPE_OBJECT = new Uint8Array(["));
        assert!(out.contains("export const Flags_TYPE_OBJECT = new Uint8Array(["));
        // The bare value-namespace names the emitters own must NOT reappear.
        assert!(!out.contains("export const NarrowEnum = "));
        assert!(!out.contains("export const Flags = "));
    }

    #[test]
    fn csharp_block_is_a_class() {
        let out = render_csharp(&sample(), &empty_spec(), "chat");
        assert!(out.contains("public static class TypeObjects_chat"));
        assert!(out.contains("public static readonly byte[] Robot_sPose"));
    }

    #[test]
    fn class_name_is_per_base_unique() {
        let e = empty_spec();
        assert_eq!(type_objects_class_name(&e, "common"), "TypeObjects_common");
        assert_eq!(
            type_objects_class_name(&e, "metrics"),
            "TypeObjects_metrics"
        );
        assert_ne!(
            type_objects_class_name(&e, "common"),
            type_objects_class_name(&e, "metrics")
        );
    }

    #[test]
    fn class_name_encodes_invalid_identifier_chars_injectively() {
        // Filename stems can carry `-`/`.`/digits illegal in a C#/Java
        // identifier; they are encoded injectively, not lossily folded.
        let e = empty_spec();
        assert_eq!(type_objects_class_name(&e, "my-idl"), "TypeObjects_my_hidl");
        assert_eq!(
            type_objects_class_name(&e, "v2.types"),
            "TypeObjects_v2_dtypes"
        );
        assert_eq!(
            type_objects_class_name(&e, "9lives"),
            "TypeObjects__n9lives"
        );
        assert_eq!(type_objects_class_name(&e, ""), "TypeObjects_TypeObjects");
        // `-` and `_` stay distinct.
        assert_ne!(
            type_objects_class_name(&e, "a-b"),
            type_objects_class_name(&e, "a_b")
        );
    }
}
