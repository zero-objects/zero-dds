// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc dlrl` — OMG DDS 1.2 §8's Data Local Reconstruction Layer, as
//! object and home classes for one of the four languages
//! `zerodds-dlrl-codegen` emits text for: C++, C#, Java, TypeScript.
//!
//! ## Why this is not `generate` with a fifth model
//!
//! DLRL is not on the wire. §8.1.1 describes an object-oriented layer an
//! application programs against, above DCPS — nothing here is bytes a
//! reader decodes, so the class this module writes has nothing in common
//! with `generate`'s declarations-plus-codec walk beyond both starting
//! from an `.idl` file. `zerodds-dlrl-codegen`'s own doc comment gives the
//! fuller version of this: no reader ever parses a DLRL object, so there
//! is nothing here for a `zerodds-lang-cst` tree to keep consistent
//! across backends that emitting formatted text does not already do.
//!
//! ## The pragma-name collision
//!
//! §8.2.2's Generation Process reads three pragmas out of the IDL text:
//!
//! ```text
//! #pragma DCPS_DATA_TYPE     "<scoped-name>"
//! #pragma DCPS_DATA_KEY      "<scoped-name> <field>"
//! #pragma DCPS_DLRL_RELATION "<scoped-name> <relation> <target>"
//! ```
//!
//! The first two spellings are not DLRL's alone: OpenDDS's devguide
//! reuses the identical two pragma names for an unrelated purpose — a
//! plain topic-type/key marker with no object layer above it — and
//! `idl-lower` already reads them that way for every other subcommand
//! (`crates/idl-lower/src/lower.rs`, and its diagnostic on each now names
//! this collision by name). This module does not extend `idl-lower`'s
//! AST facets to add a second, DLRL meaning for the same two pragma
//! spellings — that would make the two dialects mean whichever the
//! *last* thing to read the pragma decided, silently, for every other
//! subcommand that walks the same AST. Instead it reads the raw IDL text
//! on its own, through [`zerodds_dlrl::pragma::parse_pragma`] — the
//! crate's own already-tested line parser — independently of `idl-lower`
//! entirely. Running `idlc check`/`generate` and `idlc dlrl` over the
//! same file therefore each apply exactly one reading to
//! `DCPS_DATA_TYPE`/`DCPS_DATA_KEY`, and neither silently eats the
//! other's: `check`/`generate` name the OpenDDS reading and say where the
//! other one lives (`idl-lower`'s diagnostic), and `dlrl` reads all
//! three pragmas as DDS 1.2 §8's, regardless of what a peer file's
//! `#pragma DCPS_DATA_TYPE` might otherwise mean to OpenDDS.
//!
//! [`scan`] is deliberately not "every `#pragma` line through
//! `parse_pragma`": that function parses a quoted argument unconditionally
//! once a line has a tag it recognises, and would report a line like
//! `#pragma keylist Sample id` or `#pragma version Sample 1.2` —
//! perfectly legal under their own, unrelated pragmas — as a malformed
//! DLRL one. [`dlrl_tag`] filters to the three tags this module owns
//! before `parse_pragma` ever sees the line, so a genuinely unrelated
//! pragma is silent here and a line that names one of the three tags and
//! still fails to parse is the diagnostic §1's requirement asks for: "a
//! pragma the lowering does not understand is a diagnostic, not ignored".
//!
//! ## No ZeroDDS DLRL runtime
//!
//! The four backends emit classes that derive from PSM base types this
//! tree does not implement: C++'s from `DLRL::ObjectRoot`/`ObjectHome`/
//! `RefList`/`List`/`StrMap`/`IntMap` and `DDS::Long` (DDS 1.2 §B.6/§B.7,
//! the specification's own C++ PSM), Java's from
//! `org.omg.dds.dlrl.ObjectRoot`/`ObjectHome` (§B.8), C#'s from an
//! `ObjectRoot`/`ObjectHome` pair and `[DlrlKey]`/`[DlrlMonoAttribute]`/
//! `[DlrlMultiAttribute]`/`[DlrlRelation]` attributes this codebase
//! invented for a target OMG never gave a PSM (there is no vendor mapping
//! to follow either). `grep`-ing this tree for `DLRL::ObjectRoot`,
//! `org.omg.dds.dlrl` and `DlrlObject]` outside `crates/dlrl` and
//! `crates/dlrl-codegen` finds nothing: ZeroDDS ships a DLRL *runtime*
//! for exactly one language, Rust (`crates/dlrl`), and `dlrl-codegen`
//! does not target Rust — the four languages it does target have no
//! runtime here for the generated class to compile against. [`header`]
//! says so in the file it writes, per language, rather than generating
//! an `#include`/`import` for a header or a package that does not exist.
//! TypeScript is the exception: its two generated forms — a plain
//! `interface` and the class implementing it — name no DLRL base type,
//! so nothing is missing for that column.
//!
//! An attribute or key of an IDL enumeration or structure is a different
//! case: its type is the one `idlc generate` writes for the same file,
//! which does exist. The C++ header includes `<stem>.hpp` and the
//! TypeScript file imports from `./<stem>.js` when a class names one; the C#
//! and Java classes name them fully qualified.

use std::collections::BTreeMap;

use zerodds_dlrl::pragma::{DlrlPragma, ParsePragmaError, parse_pragma};
use zerodds_dlrl_codegen::{
    DlrlTypeInfo, generate_cpp_home, generate_cpp_object, generate_csharp_home,
    generate_csharp_object, generate_java_home, generate_java_object, generate_ts_class,
    generate_ts_import, generate_ts_interface,
};
use zerodds_idl_cst::Span;
use zerodds_idl_parse::{Diagnostic, DiagnosticCode};
use zerodds_lang_cst::Language;

use crate::generate::Generated;

/// The four languages `dlrl --lang` accepts — every one
/// `zerodds-dlrl-codegen` has a backend for, and no other: `--lang rust`
/// is a `generate` question, since `crates/dlrl` is read from Rust
/// directly and `dlrl-codegen` was never asked to emit a fifth,
/// redundant column for it.
pub const LANGUAGES: [Language; 4] = [
    Language::Cpp,
    Language::CSharp,
    Language::Java,
    Language::TypeScript,
];

/// Every name `--lang` accepts under `dlrl`, comma-separated, for the
/// usage text and the error an unknown one gets.
#[must_use]
pub fn language_names() -> String {
    LANGUAGES
        .into_iter()
        .map(Language::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `--lang` for `dlrl`: the same spellings and aliases
/// [`crate::generate::parse_language`] takes — `cpp`/`cxx`, `csharp`/`cs`,
/// `ts` included — narrowed to the four [`LANGUAGES`]. A build system that
/// passes `--lang rust` to `dlrl` by copying its `generate` line gets the
/// same "not a language this subcommand knows" answer a typo would, with
/// the four valid names named back.
#[must_use]
pub fn parse_language(token: &str) -> Option<Language> {
    let language = crate::generate::parse_language(token)?;
    LANGUAGES.contains(&language).then_some(language)
}

/// One file's three DLRL pragmas, and the diagnostics for a line that
/// names one of them and is otherwise malformed.
///
/// Independent of the AST entirely — see the module doc on why — so this
/// takes the file's raw text and nothing else.
#[must_use]
pub fn scan(text: &str) -> (Vec<DlrlPragma>, Vec<Diagnostic>) {
    let mut pragmas = Vec::new();
    let mut diagnostics = Vec::new();
    let mut offset: u32 = 0;
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset = offset.saturating_add(u32::try_from(line.len()).unwrap_or(u32::MAX));
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let Some(tag) = dlrl_tag(trimmed) else {
            continue;
        };
        match parse_pragma(trimmed) {
            Ok(pragma) => pragmas.push(pragma),
            Err(error) => {
                let end = line_start.saturating_add(u32::try_from(trimmed.len()).unwrap_or(0));
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::Unsupported,
                    Span::new(line_start, end),
                    dlrl_parse_error(tag, error),
                    "OMG DDS 1.2 §8.2.2",
                ));
            }
        }
    }
    (pragmas, diagnostics)
}

/// The message for a line that names one of the three DLRL tags and does
/// not parse under [`parse_pragma`]'s grammar.
fn dlrl_parse_error(tag: &str, error: ParsePragmaError) -> String {
    format!("`#pragma {tag}`: {error}")
}

/// The tag word of a `#pragma <TAG> ...` line, if `<TAG>` is one of the
/// three DLRL pragmas — `DCPS_DATA_TYPE`, `DCPS_DATA_KEY`,
/// `DCPS_DLRL_RELATION`.
///
/// Mirrors [`parse_pragma`]'s own tag extraction, ahead of the
/// quoted-argument parsing that function does unconditionally once a tag
/// is present at all — see the module doc's note on why `scan` filters
/// before parsing rather than parsing every pragma line and reporting
/// what fails.
fn dlrl_tag(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    let after_hash = trimmed.strip_prefix('#')?.trim_start();
    let after_pragma = after_hash.strip_prefix("pragma")?.trim_start();
    let tag_end = after_pragma
        .find(char::is_whitespace)
        .unwrap_or(after_pragma.len());
    match &after_pragma[..tag_end] {
        "DCPS_DATA_TYPE" => Some("DCPS_DATA_TYPE"),
        "DCPS_DATA_KEY" => Some("DCPS_DATA_KEY"),
        "DCPS_DLRL_RELATION" => Some("DCPS_DLRL_RELATION"),
        _ => None,
    }
}

/// `infos`, as `language`'s files.
///
/// One bundle per run for C++, C# and TypeScript — nothing in DDS 1.2 §8
/// or in any of those three languages limits how many top-level types one
/// compilation unit may declare. Java is the exception:
/// `generate_java_object`/`generate_java_home` each emit a `public`
/// class, and JLS §7.6 allows one `public` top-level type per compilation
/// unit — the same rule `crate::generate::files`'s own doc on
/// `ZdTypes.java`/`ZdCodec.java` cites — so each type's object and home
/// class get their own file, named after the class the way `javac`
/// requires.
#[must_use]
pub fn files(
    infos: &BTreeMap<String, DlrlTypeInfo>,
    language: Language,
    path: &str,
    stem: &str,
) -> Vec<Generated> {
    if language == Language::Java {
        let mut out = Vec::new();
        for info in infos.values() {
            let class = info.simple_name();
            out.push(Generated {
                name: format!("{class}.java"),
                text: format!("{}{}", header(path, language), generate_java_object(info)),
            });
            out.push(Generated {
                name: format!("{class}Home.java"),
                text: format!("{}{}", header(path, language), generate_java_home(info)),
            });
        }
        return out;
    }
    // The object/home pair for `language` — or, for TypeScript, the
    // interface an application programs against and the class the
    // runtime instantiates, which `zerodds-dlrl-codegen` shapes the same
    // way. `None` rather than a wildcard `unreachable!`: `LANGUAGES` is
    // the only source `--lang` builds a value from and Java returned
    // above, but this `pub fn` takes a plain `Language` and a caller
    // that reaches it directly with one outside those four gets no files
    // rather than a panic.
    type Backend = fn(&DlrlTypeInfo) -> String;
    let backends: Option<(Backend, Backend)> = match language {
        Language::Cpp => Some((generate_cpp_object, generate_cpp_home)),
        Language::CSharp => Some((generate_csharp_object, generate_csharp_home)),
        Language::TypeScript => Some((generate_ts_interface, generate_ts_class)),
        _ => None,
    };
    let Some((object, home)) = backends else {
        return Vec::new();
    };
    let mut body = String::new();
    for info in infos.values() {
        body.push_str(&object(info));
        body.push('\n');
        body.push_str(&home(info));
        body.push('\n');
    }
    // A class that names an IDL enumeration or structure needs the data
    // types `idlc generate --lang <language>` writes for the same file:
    // `<stem>.hpp` for C++, `<stem>.ts` for TypeScript. C# and Java need
    // no line: the generated C# names them `global::`-qualified and the
    // Java ones sit in `ZdTypes`, both compiled beside these classes.
    let names_idl_types = infos.values().any(|info| !info.declared_types().is_empty());
    let guard = if language == Language::Cpp {
        "#pragma once\n"
    } else {
        ""
    };
    let imports = match language {
        Language::Cpp if names_idl_types => format!("#include \"{stem}.hpp\"\n\n"),
        Language::TypeScript => generate_ts_import(infos.values(), &format!("./{stem}.js"))
            .map(|line| format!("{line}\n"))
            .unwrap_or_default(),
        _ => String::new(),
    };
    vec![Generated {
        name: format!("{stem}_dlrl.{}", language.extension()),
        text: format!("{guard}{}{imports}{body}", header(path, language)),
    }]
}

/// Fills every type's `key_types` and `attributes` from the type model.
/// The DLRL pragmas name a key; the IDL declares its type, and every other
/// member that is not a relation is an attribute (DDS 1.2 §8.1.4.3):
/// mono-valued for a basic type, an enumeration or a simple structure,
/// multi-valued for a `sequence`, a one-dimensional array or a `map`
/// (§8.1.2.2). Each problem is returned as a message: a DLRL type that is
/// not a structure of this file, a key the structure has no member for,
/// and a key or attribute whose type has no DLRL mapping (a collection as
/// a key; a union, a nested collection, a structure with a collection in
/// it, a wide string, …). Every key used to be generated as a 32-bit
/// integer whatever the IDL declared, and no attribute was generated at
/// all.
pub fn resolve_member_types(
    ast: &zerodds_ast::Ast,
    infos: &mut BTreeMap<String, DlrlTypeInfo>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for info in infos.values_mut() {
        let Some(structure) = find_structure(ast, &info.name) else {
            errors.push(format!(
                "DLRL type `{}` is not a structure this file declares",
                info.name
            ));
            continue;
        };
        let members: Vec<_> = ast
            .all_members_of(structure)
            .into_iter()
            .filter_map(|id| ast.member(id).ok())
            .collect();
        for key in info.keys.clone() {
            let Some(member) = members.iter().find(|m| m.name == key) else {
                errors.push(format!(
                    "DLRL key `{key}` of `{}`: the structure has no member of that name",
                    info.name
                ));
                continue;
            };
            match member_type(member).and_then(|ty| mono_type(ast, ty, "key")) {
                Ok(ty) => {
                    info.key_types.insert(key, ty);
                }
                Err(why) => errors.push(format!("DLRL key `{key}` of `{}`: {why}", info.name)),
            }
        }
        for member in &members {
            let name = &member.name;
            if info.keys.contains(name) || info.relations.iter().any(|(r, _)| r == name) {
                continue;
            }
            match member_type(member).and_then(|ty| attribute_type(ast, ty)) {
                Ok(ty) => info.attributes.push((name.clone(), ty)),
                Err(why) => {
                    errors.push(format!("DLRL attribute `{name}` of `{}`: {why}", info.name))
                }
            }
        }
    }
    errors
}

/// A bound on typedef chains and on structures nested in structures.
const MAX_DEPTH: usize = 64;

type TypeId = zerodds_ast_core::Id<zerodds_ast::TypeNode>;

fn member_type(member: &zerodds_ast::MemberNode) -> Result<TypeId, String> {
    member
        .type_
        .resolved()
        .ok_or_else(|| "its type is not resolved".to_string())
}

/// `ty` through its typedefs, a bounded number of them.
fn unaliased(ast: &zerodds_ast::Ast, mut ty: TypeId) -> Result<TypeId, String> {
    for _ in 0..MAX_DEPTH {
        let node = ast.type_(ty).map_err(|e| e.to_string())?;
        match &node.kind {
            zerodds_ast::TypeKind::Alias { aliased } => {
                ty = aliased
                    .resolved()
                    .ok_or_else(|| "its typedef is not resolved".to_string())?;
            }
            _ => return Ok(ty),
        }
    }
    Err(format!("a typedef chain longer than {MAX_DEPTH}"))
}

/// The scoped name of a declared type, without the leading `::`.
fn scoped_name(ast: &zerodds_ast::Ast, ty: TypeId) -> Result<String, String> {
    let node = ast.type_(ty).map_err(|e| e.to_string())?;
    Ok(ast
        .scoped_name_of(zerodds_ast::AnyId::Type(ty, node.node_id))
        .map(|s| s.to_string().trim_start_matches("::").to_string())
        .unwrap_or_else(|| node.name.clone()))
}

/// The type of an attribute: a collection for a `sequence`, a
/// one-dimensional array or a `map`, else one value.
fn attribute_type(
    ast: &zerodds_ast::Ast,
    ty: TypeId,
) -> Result<zerodds_dlrl_codegen::DlrlAttributeType, String> {
    use zerodds_ast::TypeKind;
    use zerodds_dlrl_codegen::{DlrlAttributeType as A, DlrlCollection as C, DlrlMonoType};
    let ty = unaliased(ast, ty)?;
    let item = |element: &zerodds_ast_core::Ref<zerodds_ast::TypeNode>| {
        let element = element
            .resolved()
            .ok_or_else(|| "its element type is not resolved".to_string())?;
        mono_type(ast, element, "collection item")
    };
    match &ast.type_(ty).map_err(|e| e.to_string())?.kind {
        TypeKind::Sequence { element, .. } => Ok(A::Multi {
            basis: C::List,
            item: item(element)?,
        }),
        TypeKind::Array { element, dims } if dims.len() == 1 => Ok(A::Multi {
            basis: C::List,
            item: item(element)?,
        }),
        TypeKind::Array { dims, .. } => Err(format!(
            "an array of {} dimensions has no DLRL mapping; a list has one index (DDS 1.2 §8.1.4.2.3)",
            dims.len()
        )),
        TypeKind::Map { key, value, .. } => {
            let key = key
                .resolved()
                .ok_or_else(|| "its key type is not resolved".to_string())?;
            let basis = match mono_type(ast, key, "map key") {
                Ok(DlrlMonoType::Basic(zerodds_dlrl_codegen::DlrlFieldType::String)) => C::StrMap,
                Ok(DlrlMonoType::Basic(integer)) if is_integer(integer) => C::IntMap(integer),
                _ => {
                    return Err(
                        "a map's key is a `string` or an integer type (StrMap, IntMap: DDS 1.2 §8.1.6.3.18–19)"
                            .to_string(),
                    );
                }
            };
            Ok(A::Multi {
                basis,
                item: item(value)?,
            })
        }
        _ => Ok(A::Mono(mono_type(ast, ty, "attribute")?)),
    }
}

fn is_integer(ty: zerodds_dlrl_codegen::DlrlFieldType) -> bool {
    use zerodds_dlrl_codegen::DlrlFieldType as K;
    matches!(
        ty,
        K::Octet
            | K::Int8
            | K::UInt8
            | K::Short
            | K::UShort
            | K::Long
            | K::ULong
            | K::LongLong
            | K::ULongLong
    )
}

/// The mono-valued DLRL type of `ty`, a `role` (key, attribute, map key,
/// collection item) of its structure: a basic type, an enumeration or a
/// simple structure (DDS 1.2 §8.1.2.2).
fn mono_type(
    ast: &zerodds_ast::Ast,
    ty: TypeId,
    role: &str,
) -> Result<zerodds_dlrl_codegen::DlrlMonoType, String> {
    use zerodds_ast::TypeKind;
    use zerodds_dlrl_codegen::DlrlMonoType;
    let ty = unaliased(ast, ty)?;
    match &ast.type_(ty).map_err(|e| e.to_string())?.kind {
        TypeKind::Enumeration { .. } => Ok(DlrlMonoType::Enum {
            name: scoped_name(ast, ty)?,
            default: zerodds_lang_rules::emit::default_enumerator_name(ast, ty)
                .ok_or_else(|| "its enumeration declares no enumerator".to_string())?,
        }),
        TypeKind::Structure { .. } => {
            let name = scoped_name(ast, ty)?;
            simple_structure(ast, ty, 0).map_err(|why| {
                format!(
                    "`{name}` is not a simple structure (DDS 1.2 §8.1.2.2: one that maps inside one DCPS data): {why}"
                )
            })?;
            Ok(DlrlMonoType::Struct { name })
        }
        _ => basic_type(ast, ty, role).map(DlrlMonoType::Basic),
    }
}

/// Whether every member of the structure `ty` — its base's included — is a
/// basic type, an enumeration, a simple structure or a fixed array of
/// those: a fixed set of DCPS cells (§8.1.4.2.3, "as many cells as needed
/// to hold the structure"). A sequence, a map or a union has no fixed
/// set, and a sequence of a structure is a multi-valued attribute of its
/// own rather than a field of one.
///
/// zerodds-lint: recursion-depth 64 (nested structures; bounded by `MAX_DEPTH`)
fn simple_structure(ast: &zerodds_ast::Ast, ty: TypeId, depth: usize) -> Result<(), String> {
    use zerodds_ast::TypeKind;
    if depth > MAX_DEPTH {
        return Err(format!("structures nested deeper than {MAX_DEPTH}"));
    }
    for member in ast.all_members_of(ty) {
        let member = ast.member(member).map_err(|e| e.to_string())?;
        let mut field = unaliased(ast, member_type(member)?)?;
        if let TypeKind::Array { element, .. } = &ast.type_(field).map_err(|e| e.to_string())?.kind
        {
            field = unaliased(
                ast,
                element.resolved().ok_or_else(|| {
                    format!("member `{}`: its element type is not resolved", member.name)
                })?,
            )?;
        }
        let why = |e: String| format!("member `{}`: {e}", member.name);
        match &ast.type_(field).map_err(|e| e.to_string())?.kind {
            TypeKind::Enumeration { .. } => {}
            TypeKind::Structure { .. } => simple_structure(ast, field, depth + 1).map_err(why)?,
            _ => {
                basic_type(ast, field, "structure field").map_err(why)?;
            }
        }
    }
    Ok(())
}

/// The basic DLRL type of `ty`, already through its typedefs.
fn basic_type(
    ast: &zerodds_ast::Ast,
    ty: TypeId,
    role: &str,
) -> Result<zerodds_dlrl_codegen::DlrlFieldType, String> {
    use zerodds_ast::{CharWidth, PrimitiveKind as P, TypeKind};
    use zerodds_dlrl_codegen::DlrlFieldType as K;
    match &ast.type_(ty).map_err(|e| e.to_string())?.kind {
        TypeKind::Primitive(p) => match p {
            P::Boolean => Ok(K::Boolean),
            P::Char => Ok(K::Char),
            P::Octet => Ok(K::Octet),
            P::Int8 => Ok(K::Int8),
            P::UInt8 => Ok(K::UInt8),
            P::Int16 => Ok(K::Short),
            P::UInt16 => Ok(K::UShort),
            P::Int32 => Ok(K::Long),
            P::UInt32 => Ok(K::ULong),
            P::Int64 => Ok(K::LongLong),
            P::UInt64 => Ok(K::ULongLong),
            P::Float32 => Ok(K::Float),
            P::Float64 => Ok(K::Double),
            other => Err(format!("{other:?} has no DLRL {role} mapping")),
        },
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => Ok(K::String),
        TypeKind::Sequence { .. } | TypeKind::Array { .. } | TypeKind::Map { .. } => Err(format!(
            "a collection has no DLRL {role} mapping; a {role} is a basic type, an enumeration or a simple structure"
        )),
        _ => Err(format!(
            "its type has no DLRL {role} mapping; a {role} is a basic type, an enumeration or a simple structure"
        )),
    }
}

/// The structure `name` names: by its scoped name, or by its simple name
/// when exactly one structure carries it.
fn find_structure(
    ast: &zerodds_ast::Ast,
    name: &str,
) -> Option<zerodds_ast_core::Id<zerodds_ast::TypeNode>> {
    let wanted = name.trim_start_matches("::");
    let structures: Vec<_> = ast
        .types()
        .filter(|(_, node)| matches!(node.kind, zerodds_ast::TypeKind::Structure { .. }))
        .map(|(id, node)| {
            let scoped = ast
                .scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
                .map(|s| s.to_string().trim_start_matches("::").to_string())
                .unwrap_or_else(|| node.name.clone());
            (id, node.name.clone(), scoped)
        })
        .collect();
    if let Some((id, _, _)) = structures.iter().find(|(_, _, scoped)| scoped == wanted) {
        return Some(*id);
    }
    let mut by_simple = structures.iter().filter(|(_, simple, _)| simple == wanted);
    match (by_simple.next(), by_simple.next()) {
        (Some((id, _, _)), None) => Some(*id),
        _ => None,
    }
}

/// The header comment every `dlrl`-generated file opens on: what it was
/// generated from, and — since no ZeroDDS runtime backs three of the four
/// columns — what it needs to actually compile.
///
/// The module doc's "No ZeroDDS DLRL runtime" section is the fuller
/// version of the same finding; this is what a person reading the
/// generated file sees without having to find that doc comment first.
fn header(path: &str, language: Language) -> String {
    let runtime_note = match language {
        Language::Cpp => {
            "// These classes derive from the DLRL C++ PSM's own base types \u{2014}\n\
             // `DLRL::ObjectRoot`, `DLRL::ObjectHome`, `DLRL::RefList`, `DLRL::List`,\n\
             // `DLRL::StrMap`, `DLRL::IntMap` \u{2014} and use `DDS::Long`.\n\
             // ZeroDDS ships no C++ DLRL runtime that defines them; link this header\n\
             // against a DLRL-conformant vendor runtime, or your own.\n"
        }
        Language::Java => {
            "// These classes derive from `org.omg.dds.dlrl.ObjectRoot`/`ObjectHome`.\n\
             // ZeroDDS ships no Java DLRL runtime that defines them; compile against a\n\
             // DLRL-conformant vendor runtime, or your own.\n"
        }
        Language::CSharp => {
            "// These classes derive from `ObjectRoot`/`ObjectHome` and carry\n\
             // `[DlrlObject]`/`[DlrlKey]`/`[DlrlMonoAttribute]`/`[DlrlMultiAttribute]`/\n\
             // `[DlrlRelation]` attributes \u{2014} a mapping this codebase chose, since\n\
             // DDS 1.2 Annex B has no C# PSM (see the crate's own `csharp.rs` doc).\n\
             // ZeroDDS ships none of those seven types; supply them yourself, or a\n\
             // vendor's, before this compiles.\n"
        }
        // TypeScript's two forms name no DLRL base type.
        _ => "",
    };
    format!(
        "// Generated by `idlc dlrl` from {path}, {} (OMG DDS 1.2 §8.2.2). Do not edit.\n{}",
        language.as_str(),
        runtime_note
    )
}
