// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The type of a topic for a client's code generator (Spec §5.2): the
//! path `/schema/<slug>` on the WebSocket listener, and the IDL it serves.
//!
//! `<slug>` is the slug of §5.1 of the topic's name, the default path
//! without `/topics/` (`Chat::Message` → `chat/message`), whatever
//! `ws_path` the topic has. The body is the IDL of the topic's `idl_file`
//! with each `#include` replaced by the text of the file it names, the
//! way the preprocessor reads it, so that one document compiles on its
//! own.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use zerodds_types::dynamic::{
    DynamicType, ExtensibilityKind, MemberDescriptor, TypeDescriptor, TypeKind,
};

/// The prefix of the schema paths.
pub const SCHEMA_PREFIX: &str = "/schema/";

/// How deep `#include`s nest at most; a cycle without include guards ends
/// here.
const MAX_INCLUDE_DEPTH: usize = 64;

/// How deep the types of a topic's type nest at most in the XML type
/// representation (a member's type, an element type, a base type), the
/// topic's type at depth 0; a deeper type is refused with an error.
const MAX_TYPE_NESTING: usize = 64;

/// The form `/schema/<slug>` serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaForm {
    /// `/schema/<slug>` and `/schema/<slug>.idl`: the IDL.
    Idl,
    /// `/schema/<slug>.xml`: the XML type representation (DDS-XTypes 1.3
    /// §7.3.2).
    Xml,
}

/// The slug of the schema path of `topic` (Spec §5.1, §5.2).
#[must_use]
pub fn slug(topic: &str) -> String {
    let path = super::config::default_ws_path(topic);
    path.strip_prefix("/topics/").unwrap_or(&path).to_string()
}

/// The schema URL of `topic` in the catalog (Spec §5.2).
#[must_use]
pub fn schema_url(topic: &str) -> String {
    format!("{SCHEMA_PREFIX}{}.idl", slug(topic))
}

/// The slug and the form a request path names; `None` for a path outside
/// `/schema/`.
#[must_use]
pub fn route(path: &str) -> Option<(&str, SchemaForm)> {
    let rest = path.strip_prefix(SCHEMA_PREFIX)?;
    Some(if let Some(slug) = rest.strip_suffix(".idl") {
        (slug, SchemaForm::Idl)
    } else if let Some(slug) = rest.strip_suffix(".xml") {
        (slug, SchemaForm::Xml)
    } else {
        (rest, SchemaForm::Idl)
    })
}

/// The text of the IDL file `path`, each `#include` line replaced by the
/// text of the file it names (recursively): `"file"` from the directory of
/// the including file, then from the directory of `path`; `<file>` from the
/// directory of `path`, the directory the daemon's IDL compiler searches.
/// An `#include` of a file that is not there stays as it is: the compiler
/// read the file without it, so it lies in a part the preprocessor skips.
/// A file with `#pragma once` is put in once, at its first `#include`, as
/// the daemon's preprocessor reads it once; the pragma line is left out,
/// inside one document it says nothing.
///
/// # Errors
/// `path` or an included file that exists does not read, or the includes
/// nest deeper than 64.
pub fn expand_includes(path: &Path) -> Result<String, String> {
    let root = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut out = String::new();
    let mut once = BTreeSet::new();
    expand_into(path, &root, 0, &mut once, &mut out)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (MAX_INCLUDE_DEPTH, refused deeper)
fn expand_into(
    file: &Path,
    root: &Path,
    depth: usize,
    once: &mut BTreeSet<PathBuf>,
    out: &mut String,
) -> Result<(), String> {
    // The identity of a file, as the preprocessor's file loader takes it:
    // two names of one file are one file.
    let identity = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    if once.contains(&identity) {
        return Ok(());
    }
    if depth > MAX_INCLUDE_DEPTH {
        return Err(format!(
            "the #include of {} nests deeper than {MAX_INCLUDE_DEPTH}",
            file.display()
        ));
    }
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let here = file.parent().map(Path::to_path_buf).unwrap_or_default();
    for line in text.split_inclusive('\n') {
        if is_pragma_once(line) {
            once.insert(identity.clone());
            continue;
        }
        match include_target(line).and_then(|(name, quoted)| resolve(name, quoted, &here, root)) {
            Some(target) => {
                expand_into(&target, root, depth + 1, once, out)?;
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            None => out.push_str(line),
        }
    }
    Ok(())
}

/// Whether `line` is `#pragma once` (with any blanks between the parts).
fn is_pragma_once(line: &str) -> bool {
    line.trim_start()
        .strip_prefix('#')
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix("pragma"))
        .is_some_and(|rest| rest.starts_with([' ', '\t']) && rest.trim() == "once")
}

/// The file an `#include` line names and whether it is in quotes.
fn include_target(line: &str) -> Option<(&str, bool)> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let rest = rest.strip_prefix("include")?.trim();
    if let Some(name) = rest.strip_prefix('"') {
        return Some((name.split('"').next()?, true));
    }
    let name = rest.strip_prefix('<')?;
    Some((name.split('>').next()?, false))
}

fn resolve(name: &str, quoted: bool, here: &Path, root: &Path) -> Option<PathBuf> {
    let candidates = if quoted {
        vec![here.join(name), root.join(name)]
    } else {
        vec![root.join(name)]
    };
    candidates.into_iter().find(|p| p.is_file())
}

/// The XML type representation (DDS-XTypes 1.3 §7.3.2, the schema of its
/// Annex A) of `ty` and of every type it uses, each before its first use:
/// `<types>` with the `<module>`s of each type's scope and a `<struct>`,
/// `<union>`, `<enum>`, `<bitmask>` or `<typedef>`.
///
/// # Errors
/// A type the representation cannot carry without a type of its own: a
/// bitset, a collection of a collection other than an array of sequences,
/// a map whose key or value is a collection; a model that does not
/// describe a type.
pub fn xml_type_representation(ty: &DynamicType) -> Result<String, String> {
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    collect(ty, 0, &mut seen, &mut order)?;
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<types>\n");
    for t in &order {
        write_type(t, &mut out)?;
    }
    out.push_str("</types>\n");
    Ok(out)
}

fn is_named(kind: TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Structure
            | TypeKind::Union
            | TypeKind::Enumeration
            | TypeKind::Bitmask
            | TypeKind::Alias
            | TypeKind::Bitset
    )
}

fn descriptor_of(ty: &DynamicType) -> Result<TypeDescriptor, String> {
    ty.descriptor()
        .map_err(|e| format!("{}: {e:?}", ty.qualified_name()))
}

fn member_descriptor(
    owner: &DynamicType,
    m: &zerodds_types::dynamic::DynamicTypeMember,
) -> Result<MemberDescriptor, String> {
    m.get_descriptor()
        .map_err(|e| format!("{}.{}: {e:?}", owner.qualified_name(), m.name()))
}

/// `ty` and the named types it uses, depth first, each once; `depth` is
/// how deep `ty` lies below the topic's type.
///
/// zerodds-lint: recursion-depth 64 (MAX_TYPE_NESTING, refused deeper)
fn collect(
    ty: &DynamicType,
    depth: usize,
    seen: &mut BTreeSet<String>,
    order: &mut Vec<DynamicType>,
) -> Result<(), String> {
    if depth > MAX_TYPE_NESTING {
        return Err(format!(
            "{}: the types nest deeper than {MAX_TYPE_NESTING}",
            ty.qualified_name()
        ));
    }
    let kind = ty.kind();
    if !is_named(kind) {
        let d = descriptor_of(ty)?;
        for t in [d.element_type, d.key_element_type].into_iter().flatten() {
            collect(&t, depth + 1, seen, order)?;
        }
        return Ok(());
    }
    if !seen.insert(ty.qualified_name()) {
        return Ok(());
    }
    let d = descriptor_of(ty)?;
    match kind {
        TypeKind::Bitset => {
            return Err(format!(
                "{}: a bitset has no form in this XML type representation",
                ty.qualified_name()
            ));
        }
        TypeKind::Structure | TypeKind::Union | TypeKind::Alias => {
            for t in [d.base_type, d.discriminator_type].into_iter().flatten() {
                collect(&t, depth + 1, seen, order)?;
            }
            for m in ty.members() {
                collect(&member_descriptor(ty, &m)?.type_, depth + 1, seen, order)?;
            }
        }
        _ => {}
    }
    order.push(ty.clone());
    Ok(())
}

fn primitive_name(kind: TypeKind) -> Option<&'static str> {
    Some(match kind {
        TypeKind::Boolean => "boolean",
        TypeKind::Byte => "byte",
        TypeKind::Int8 => "int8",
        TypeKind::UInt8 => "uint8",
        TypeKind::Int16 => "int16",
        TypeKind::UInt16 => "uint16",
        TypeKind::Int32 => "int32",
        TypeKind::UInt32 => "uint32",
        TypeKind::Int64 => "int64",
        TypeKind::UInt64 => "uint64",
        TypeKind::Float32 => "float32",
        TypeKind::Float64 => "float64",
        TypeKind::Float128 => "float128",
        TypeKind::Char8 => "char8",
        TypeKind::Char16 => "char16",
        _ => return None,
    })
}

/// The one bound of a sequence, a string or a map; `None` when unbounded.
fn bound_of(d: &TypeDescriptor) -> Option<u32> {
    d.bound
        .first()
        .copied()
        .filter(|b| *b != zerodds_types::dynamic::LENGTH_UNLIMITED)
}

/// `type` (and `nonBasicTypeName`, `stringMaxLength`) of a type that is no
/// collection.
fn scalar_attributes(ty: &DynamicType, out: &mut String) -> Result<(), String> {
    let kind = ty.kind();
    if let Some(name) = primitive_name(kind) {
        let _ = write!(out, " type=\"{name}\"");
        return Ok(());
    }
    match kind {
        TypeKind::String8 | TypeKind::String16 => {
            let name = if kind == TypeKind::String8 {
                "string"
            } else {
                "wstring"
            };
            let _ = write!(out, " type=\"{name}\"");
            if let Some(b) = bound_of(&descriptor_of(ty)?) {
                let _ = write!(out, " stringMaxLength=\"{b}\"");
            }
            Ok(())
        }
        k if is_named(k) => {
            let _ = write!(
                out,
                " type=\"nonBasic\" nonBasicTypeName=\"{}\"",
                ty.qualified_name()
            );
            Ok(())
        }
        _ => Err(format!(
            "{}: a collection inside a collection needs a typedef in the XML type representation",
            ty.qualified_name()
        )),
    }
}

/// The attributes of a member, a discriminator or a typedef of type `ty`
/// (Annex A `memberDecl`): an array of `arrayDimensions`, a sequence of
/// `sequenceMaxLength` (`-1` unbounded), a map of `mapMaxLength` and
/// `mapKeyType`, then the element type.
fn type_attributes(ty: &DynamicType, out: &mut String) -> Result<(), String> {
    let mut ty = ty.clone();
    if ty.kind() == TypeKind::Array {
        let d = descriptor_of(&ty)?;
        let dims: Vec<String> = d.bound.iter().map(u32::to_string).collect();
        let _ = write!(out, " arrayDimensions=\"{}\"", dims.join(","));
        ty = d.element_type.ok_or("an array without element type")?;
    }
    match ty.kind() {
        TypeKind::Sequence => {
            let d = descriptor_of(&ty)?;
            let _ = write!(
                out,
                " sequenceMaxLength=\"{}\"",
                bound_of(&d).map_or_else(|| "-1".to_string(), |b| b.to_string())
            );
            scalar_attributes(
                &d.element_type.ok_or("a sequence without element type")?,
                out,
            )
        }
        TypeKind::Map => {
            let d = descriptor_of(&ty)?;
            let key = d.key_element_type.clone().ok_or("a map without key type")?;
            let _ = write!(
                out,
                " mapMaxLength=\"{}\"",
                bound_of(&d).map_or_else(|| "-1".to_string(), |b| b.to_string())
            );
            match key.kind() {
                TypeKind::String8 | TypeKind::String16 => {
                    let name = if key.kind() == TypeKind::String8 {
                        "string"
                    } else {
                        "wstring"
                    };
                    let _ = write!(out, " mapKeyType=\"{name}\"");
                    if let Some(b) = bound_of(&descriptor_of(&key)?) {
                        let _ = write!(out, " mapKeyStringMaxLength=\"{b}\"");
                    }
                }
                k => match primitive_name(k) {
                    Some(name) => {
                        let _ = write!(out, " mapKeyType=\"{name}\"");
                    }
                    None => {
                        return Err(format!(
                            "the key {} of a map has no form in mapKeyType",
                            key.qualified_name()
                        ));
                    }
                },
            }
            scalar_attributes(&d.element_type.ok_or("a map without value type")?, out)
        }
        _ => scalar_attributes(&ty, out),
    }
}

fn extensibility_name(kind: Option<ExtensibilityKind>) -> &'static str {
    match kind {
        Some(ExtensibilityKind::Final) => "final",
        Some(ExtensibilityKind::Mutable) => "mutable",
        Some(ExtensibilityKind::Appendable) | None => "appendable",
    }
}

fn write_type(ty: &DynamicType, out: &mut String) -> Result<(), String> {
    let qualified = ty.qualified_name();
    let mut scope: Vec<&str> = qualified.split("::").collect();
    let name = scope.pop().unwrap_or_default();
    let mut indent = String::from("  ");
    for module in &scope {
        let _ = writeln!(out, "{indent}<module name=\"{module}\">");
        indent.push_str("  ");
    }
    let d = descriptor_of(ty)?;
    match ty.kind() {
        TypeKind::Structure => {
            let _ = write!(
                out,
                "{indent}<struct name=\"{name}\" extensibility=\"{}\"",
                extensibility_name(d.extensibility_kind)
            );
            if let Some(base) = &d.base_type {
                let _ = write!(out, " baseType=\"{}\"", base.qualified_name());
            }
            if d.is_nested {
                out.push_str(" nested=\"true\"");
            }
            out.push_str(">\n");
            let mutable = d.extensibility_kind == Some(ExtensibilityKind::Mutable);
            for m in ty.members() {
                let md = member_descriptor(ty, &m)?;
                let _ = write!(out, "{indent}  <member name=\"{}\"", md.name);
                type_attributes(&md.type_, out)?;
                if md.is_key {
                    out.push_str(" key=\"true\"");
                }
                if md.is_optional {
                    out.push_str(" optional=\"true\"");
                }
                if mutable {
                    let _ = write!(out, " id=\"{}\"", md.id);
                }
                out.push_str("/>\n");
            }
            let _ = writeln!(out, "{indent}</struct>");
        }
        TypeKind::Union => {
            let _ = writeln!(
                out,
                "{indent}<union name=\"{name}\" extensibility=\"{}\">",
                extensibility_name(d.extensibility_kind)
            );
            let disc = d
                .discriminator_type
                .ok_or("a union without discriminator")?;
            let _ = write!(out, "{indent}  <discriminator");
            type_attributes(&disc, out)?;
            out.push_str("/>\n");
            // An enumeration's labels by the names of its enumerators.
            let literals: Vec<(i64, String)> =
                if disc.resolve_alias().map(|t| t.kind()) == Ok(TypeKind::Enumeration) {
                    disc.resolve_alias()
                        .map(|e| e.members())
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|l| {
                            l.literal_value()
                                .map(|v| (i64::from(v), l.name().to_string()))
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            for m in ty.members() {
                let md = member_descriptor(ty, &m)?;
                let _ = writeln!(out, "{indent}  <case>");
                for label in &md.label {
                    let value = literals
                        .iter()
                        .find(|(v, _)| v == label)
                        .map_or_else(|| label.to_string(), |(_, n)| n.clone());
                    let _ = writeln!(out, "{indent}    <caseDiscriminator value=\"{value}\"/>");
                }
                if md.is_default_label {
                    let _ = writeln!(out, "{indent}    <caseDiscriminator value=\"default\"/>");
                }
                let _ = write!(out, "{indent}    <member name=\"{}\"", md.name);
                type_attributes(&md.type_, out)?;
                out.push_str("/>\n");
                let _ = writeln!(out, "{indent}  </case>");
            }
            let _ = writeln!(out, "{indent}</union>");
        }
        TypeKind::Enumeration => {
            let _ = write!(out, "{indent}<enum name=\"{name}\"");
            if let Some(b) = ty.bit_bound().filter(|b| *b != 32) {
                let _ = write!(out, " bitBound=\"{b}\"");
            }
            out.push_str(">\n");
            for l in ty.members() {
                let value = l
                    .literal_value()
                    .ok_or_else(|| format!("{qualified}::{}: no value", l.name()))?;
                let _ = writeln!(
                    out,
                    "{indent}  <enumerator name=\"{}\" value=\"{value}\"/>",
                    l.name()
                );
            }
            let _ = writeln!(out, "{indent}</enum>");
        }
        TypeKind::Bitmask => {
            let _ = write!(out, "{indent}<bitmask name=\"{name}\"");
            if let Some(b) = ty.bit_bound() {
                let _ = write!(out, " bitBound=\"{b}\"");
            }
            out.push_str(">\n");
            for f in ty.members() {
                let position = f
                    .literal_value()
                    .ok_or_else(|| format!("{qualified}::{}: no position", f.name()))?;
                let _ = writeln!(
                    out,
                    "{indent}  <flag name=\"{}\" position=\"{position}\"/>",
                    f.name()
                );
            }
            let _ = writeln!(out, "{indent}</bitmask>");
        }
        TypeKind::Alias => {
            let aliased = d.base_type.ok_or("a typedef without type")?;
            let _ = write!(out, "{indent}<typedef name=\"{name}\"");
            type_attributes(&aliased, out)?;
            out.push_str("/>\n");
        }
        other => return Err(format!("{qualified}: {other:?} is no named type")),
    }
    for _ in &scope {
        indent.truncate(indent.len() - 2);
        let _ = writeln!(out, "{indent}</module>");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zd-ws-schema-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Spec §5.2 with §5.1: the slug is the topic's default path without
    /// `/topics/`; the catalog's `schema_url` is `/schema/<slug>.idl`.
    #[test]
    fn wsbridge_5_2_the_schema_path_maps_a_slug_to_its_topic() {
        assert_eq!(slug("Chat::Message"), "chat/message");
        assert_eq!(slug("Sensor Reading"), "sensor_reading");
        assert_eq!(schema_url("Chat::Message"), "/schema/chat/message.idl");
        assert_eq!(
            route("/schema/chat/message.idl"),
            Some(("chat/message", SchemaForm::Idl))
        );
        assert_eq!(
            route("/schema/chat/message"),
            Some(("chat/message", SchemaForm::Idl))
        );
        assert_eq!(
            route("/schema/chat/message.xml"),
            Some(("chat/message", SchemaForm::Xml))
        );
        assert_eq!(route("/topics/chat/message"), None);
        assert_eq!(route("/schema"), None);
    }

    /// Spec §5.2: an `#include` line is replaced by the text of the file,
    /// nested includes too, from the including file's directory first.
    #[test]
    fn wsbridge_5_2_the_includes_are_in_place() {
        let d = dir("nested");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("base.idl"), "module Base { typedef long Id; };\n").unwrap();
        std::fs::write(
            d.join("sub/mid.idl"),
            "#include \"leaf.idl\"\nmodule Mid { struct M { Base::Id id; }; };",
        )
        .unwrap();
        std::fs::write(d.join("sub/leaf.idl"), "#include <base.idl>\n").unwrap();
        std::fs::write(
            d.join("top.idl"),
            "  #  include \"sub/mid.idl\"\nstruct Top { Mid::M m; };\n",
        )
        .unwrap();
        let text = expand_includes(&d.join("top.idl")).unwrap();
        assert_eq!(
            text,
            "module Base { typedef long Id; };\nmodule Mid { struct M { Base::Id id; }; };\nstruct Top { Mid::M m; };\n"
        );
    }

    /// Spec §5.2: an `#include` of a file that is not there stays; the
    /// guards of the preprocessor stay with the text.
    #[test]
    fn wsbridge_5_2_an_include_of_no_file_stays() {
        let d = dir("missing");
        let src = "#ifdef NEVER\n#include \"absent.idl\"\n#endif\nstruct T { long x; };\n";
        std::fs::write(d.join("t.idl"), src).unwrap();
        assert_eq!(expand_includes(&d.join("t.idl")).unwrap(), src);
    }

    /// An include cycle without guards ends with an error, not a loop.
    #[test]
    fn wsbridge_5_2_an_include_cycle_is_an_error() {
        let d = dir("cycle");
        std::fs::write(d.join("a.idl"), "#include \"b.idl\"\n").unwrap();
        std::fs::write(d.join("b.idl"), "#include \"a.idl\"\n").unwrap();
        let e = expand_includes(&d.join("a.idl")).unwrap_err();
        assert!(e.contains("deeper than 64"), "{e}");
    }

    /// Spec §5.2: a header with `#pragma once` that two files include is
    /// in the document once, at its first include, and without the pragma
    /// line; a header without it is in it each time, as the preprocessor
    /// reads it each time.
    #[test]
    fn wsbridge_5_2_a_header_with_pragma_once_is_in_place_once() {
        let d = dir("once");
        std::fs::create_dir_all(d.join("inc")).unwrap();
        std::fs::write(
            d.join("inc/common.idl"),
            "#pragma once\nmodule Common { typedef long Id; };\n",
        )
        .unwrap();
        std::fs::write(d.join("inc/plain.idl"), "const long P = 1;\n").unwrap();
        std::fs::write(
            d.join("a.idl"),
            "#include \"inc/common.idl\"\n#include \"inc/plain.idl\"\nstruct A { Common::Id id; };\n",
        )
        .unwrap();
        std::fs::write(
            d.join("top.idl"),
            "#include \"a.idl\"\n#  pragma   once\n#include \"inc/../inc/common.idl\"\n#include <inc/plain.idl>\nstruct Top { A a; Common::Id id; };\n",
        )
        .unwrap();
        let text = expand_includes(&d.join("top.idl")).unwrap();
        assert_eq!(
            text,
            "module Common { typedef long Id; };\nconst long P = 1;\nstruct A { Common::Id id; };\nconst long P = 1;\nstruct Top { A a; Common::Id id; };\n"
        );
        assert!(is_pragma_once("  #\tpragma once  \n"));
        assert!(!is_pragma_once("#pragma onces\n"));
        assert!(!is_pragma_once("#pragmaonce\n"));
        assert!(!is_pragma_once("#pragma keylist T id\n"));
    }

    /// An `#include` line is told from other directives and from text.
    #[test]
    fn wsbridge_5_2_an_include_line_is_recognised() {
        assert_eq!(
            include_target("#include \"a.idl\"\n"),
            Some(("a.idl", true))
        );
        assert_eq!(include_target("# include <b.idl>"), Some(("b.idl", false)));
        assert_eq!(include_target("#define X 1"), None);
        assert_eq!(include_target("struct include { long x; };"), None);
        assert_eq!(include_target("// #include \"c.idl\""), None);
    }

    /// The type of `name` read back from the XML type representation of the
    /// type `name` in `idl`, by the DDS-XML front end; it has to equal the
    /// type the IDL front end read.
    fn xml_round_trip(idl: &str, name: &str) -> String {
        let ty = super::super::json_frames::TopicType::from_idl(idl, name).unwrap();
        let xml = xml_type_representation(ty.dynamic_type()).unwrap();
        let composed = zerodds_xml_compose::compose_source("<schema>", &xml);
        assert!(
            composed.diagnostics.is_empty(),
            "{:?}\n{xml}",
            composed.diagnostics
        );
        let back = DynamicType::find(&std::sync::Arc::new(composed.ast), name)
            .unwrap_or_else(|e| panic!("{e:?}\n{xml}"));
        assert!(
            ty.dynamic_type().equals_ignoring_annotations(&back),
            "the XML type differs from the IDL type:\n{xml}"
        );
        xml
    }

    /// Spec §5.2 with DDS-XTypes 1.3 §7.3.2: the XML type representation of
    /// a struct with key, bounded and unbounded strings and sequences, an
    /// array, a map, an optional member, a typedef, an enumeration and a
    /// nested struct of another module, each type before its use; read
    /// back, it is the same type as the IDL.
    #[test]
    fn wsbridge_5_2_the_xml_form_is_the_type_of_the_idl() {
        let xml = xml_round_trip(
            "module Geo { enum Unit { METRE, FOOT }; @final struct Point { double x; double y; }; };\n\
             module Chat {\n\
               typedef string<16> Name;\n\
               @appendable struct Message {\n\
                 @key long id;\n\
                 Name author;\n\
                 string text;\n\
                 sequence<Geo::Point, 4> path;\n\
                 sequence<octet> blob;\n\
                 long grid[2][3];\n\
                 map<string, long> counts;\n\
                 @optional Geo::Unit unit;\n\
               };\n\
             };",
            "Chat::Message",
        );
        let pos = |what: &str| xml.find(what).unwrap_or_else(|| panic!("{what} in {xml}"));
        assert!(pos("<enum name=\"Unit\"") < pos("<struct name=\"Message\""));
        assert!(pos("<struct name=\"Point\"") < pos("<struct name=\"Message\""));
        assert!(xml.contains("<typedef name=\"Name\" type=\"string\" stringMaxLength=\"16\"/>"));
        assert!(xml.contains("<member name=\"id\" type=\"int32\" key=\"true\"/>"));
        assert!(xml.contains(
            "<member name=\"path\" sequenceMaxLength=\"4\" type=\"nonBasic\" nonBasicTypeName=\"Geo::Point\"/>"
        ));
        assert!(xml.contains("<member name=\"blob\" sequenceMaxLength=\"-1\" type=\"byte\"/>"));
        assert!(xml.contains("<member name=\"grid\" arrayDimensions=\"2,3\" type=\"int32\"/>"));
        assert!(xml.contains("optional=\"true\""));
    }

    /// Spec §5.2: a mutable struct carries the member ids, a union its
    /// cases with the enumerators of its discriminator and `default`, a
    /// bitmask its flags; read back, the same types.
    #[test]
    fn wsbridge_5_2_the_xml_form_of_a_union_a_bitmask_and_a_mutable_struct() {
        let xml = xml_round_trip(
            "enum Kind { A, B, C };\n\
             @bit_bound(8) bitmask Flags { F0, F1 };\n\
             union Choice switch (Kind) { case A: long a; case B: case C: string bc; };\n\
             union Num switch (short) { case 1: float f; default: double d; };\n\
             @mutable struct Envelope { @id(5) Choice pick; Num value; Flags bits; };",
            "Envelope",
        );
        assert!(xml.contains("<caseDiscriminator value=\"B\"/>"), "{xml}");
        assert!(
            xml.contains("<caseDiscriminator value=\"default\"/>"),
            "{xml}"
        );
        assert!(xml.contains("id=\"5\""), "{xml}");
        assert!(xml.contains("<flag name=\"F1\" position=\"1\"/>"), "{xml}");
    }

    /// A type whose members nest deeper than 64 has no XML form: an error,
    /// not a recursion without bound. `S63` holds `S62` and so on down to
    /// `S0` with its `long` 64 levels below `S63`: it has one; `S64` has
    /// the `long` 65 levels below it.
    #[test]
    fn wsbridge_5_2_types_nested_deeper_than_64_have_no_xml_form() {
        let idl = |levels: usize| {
            let mut idl = String::from("struct S0 { long x; };\n");
            for i in 1..=levels {
                let _ = writeln!(idl, "struct S{i} {{ S{} m; }};", i - 1);
            }
            idl
        };
        let ty = super::super::json_frames::TopicType::from_idl(&idl(63), "S63").unwrap();
        assert!(xml_type_representation(ty.dynamic_type()).is_ok());
        let ty = super::super::json_frames::TopicType::from_idl(&idl(64), "S64").unwrap();
        let e = xml_type_representation(ty.dynamic_type()).unwrap_err();
        assert!(e.contains("deeper than 64"), "{e}");
    }

    /// Spec §5.2: a sequence of sequences has no form without a typedef of
    /// its own; the XML form is refused with the reason, not written wrong.
    #[test]
    fn wsbridge_5_2_a_collection_of_collections_has_no_xml_form() {
        let ty = super::super::json_frames::TopicType::from_idl(
            "struct Deep { sequence<sequence<long> > rows; };",
            "Deep",
        )
        .unwrap();
        let e = xml_type_representation(ty.dynamic_type()).unwrap_err();
        assert!(e.contains("needs a typedef"), "{e}");
    }
}
