// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.3 Building Block "Types" — XML→AST-Parser.
//!
//! Reads `<types>` top-level blocks and produces a
//! [`Vec<TypeLibrary>`]. The module hierarchy is kept as nested
//! [`TypeDef::Module`] entries.
//!
//! The grammar is the one of `dds-xml_type_definitions_nonamespace.xsd`
//! (XTypes 1.3 Annex A): `<annotate>` and `<verbatim>` children, the
//! attributes `nested`, `autoid`, `external`, `tryConstruct`,
//! `nonSerialized`, the `<annotation>` declarations and the `<const>`
//! children of a struct are read into the model. The attributes that bound
//! a collection, `arrayDimensions`, the `value` of a `<const>` and of an
//! `<enumerator>` and the `value` of a `<caseDiscriminator>` are constant
//! expressions (XTypes 1.3 §7.3.2.4), evaluated by `crate::xtypes_expr`.
//!
//! Spellings the schema does not have are vendor forms: the IDL names of the
//! primitives (`long`, `octet`, …), a type name in `type` without
//! `nonBasic`, the `discriminator="…"` attribute of `<union>`,
//! `<flag>` for `<flag>`, `base_type` for `baseType`, `must_understand`
//! for `mustUnderstand`, `forwardDcl` for `forward_dcl`, an upper-case or
//! absent `kind` of a forward declaration, `appendable` for `append`, and
//! the `<bitset>` element. They are refused with an error that names the
//! form and the schema's form, unless the reading has
//! [`crate::VendorForms::Accept`], which reads them and reports each.
//!
//! Spec source: OMG DDS-XML 1.0 §7.3.3 (Types Building Block).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::options::{Ctx, Parsed, ReadOptions};
use crate::parser::{XmlElement, parse_xml_tree};
use crate::types::parse_ulong;
use crate::xtypes_def::{
    Annotation, AnnotationMemberDecl, AnnotationTypeDef, AnnotationValue, AutoId, BitField,
    BitValue, BitmaskType, BitsetType, ConstEntry, EnumLiteral, EnumType, Extensibility,
    ForwardDeclEntry, IncludeEntry, ModuleEntry, PrimitiveType, StructMember, StructType,
    TryConstruct, TypeDef, TypeLibrary, TypeRef, TypedefType, UnionCase, UnionDiscriminator,
    UnionType, Verbatim,
};
use crate::xtypes_expr::ConstTable;

/// Parses all `<types>` top-level elements of a DDS-XML document.
///
/// The top-level wrapper `<dds>` is NOT required — `xml` may be directly
/// a `<types>` root element.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — not well-formed XML.
/// * [`XmlError::UnknownElement`] — unknown top-level element.
/// * [`XmlError::ValueOutOfRange`] — numeric attribute out of range.
pub fn parse_type_libraries(xml: &str) -> Result<Vec<TypeLibrary>, XmlError> {
    Ok(parse_type_libraries_with(xml, &ReadOptions::default())?.value)
}

/// As [`parse_type_libraries`], with reading options.
///
/// # Errors
/// As [`parse_type_libraries`].
pub fn parse_type_libraries_with(
    xml: &str,
    options: &ReadOptions,
) -> Result<Parsed<Vec<TypeLibrary>>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    let mut ctx = Ctx::new(*options);
    let mut out = Vec::new();
    match doc.root.name.as_str() {
        "dds" => {
            for child in &doc.root.children {
                if child.name == "types" {
                    out.push(parse_types_element_ctx(&mut ctx, child)?);
                }
            }
        }
        "types" => out.push(parse_types_element_ctx(&mut ctx, &doc.root)?),
        other => {
            return Err(XmlError::InvalidXml(format!(
                "expected <dds> or <types> root, got <{other}>"
            )));
        }
    }
    Ok(ctx.finish(out))
}

/// Parses a single `<types>` element into a `TypeLibrary`.
///
/// # Errors
/// As [`parse_type_libraries`].
pub fn parse_types_element(el: &XmlElement) -> Result<TypeLibrary, XmlError> {
    parse_types_element_ctx(&mut Ctx::new(ReadOptions::default()), el)
}

/// As [`parse_types_element`], with reading options.
///
/// # Errors
/// As [`parse_type_libraries`].
pub fn parse_types_element_with(
    el: &XmlElement,
    options: &ReadOptions,
) -> Result<Parsed<TypeLibrary>, XmlError> {
    let mut ctx = Ctx::new(*options);
    let library = parse_types_element_ctx(&mut ctx, el)?;
    Ok(ctx.finish(library))
}

pub(crate) fn parse_types_element_ctx(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<TypeLibrary, XmlError> {
    // `name` is not an attribute of the schema's `typeLibrary`; this crate
    // has always read it as the library's name.
    ctx.check_attributes(el, &["name"])?;
    let name = el.attribute("name").unwrap_or("").to_string();
    let mut consts = ConstTable::default();
    collect_consts(el, "", &mut consts);
    let mut types = Vec::new();
    for child in &el.children {
        if let Some(t) = parse_type_def(ctx, &consts, "", child)? {
            types.push(t);
        }
    }
    Ok(TypeLibrary { name, types })
}

/// Collects the `<const>` declarations of a `<types>` block with their
/// scope: those in a `<module>` are scoped to it, those in a `<struct>` to
/// the struct.
///
/// zerodds-lint: recursion-depth = element nesting (`<module>` in
/// `<module>`), capped by `parser::MAX_TREE_DEPTH` = 64 when the tree is
/// built.
fn collect_consts(el: &XmlElement, scope: &str, out: &mut ConstTable) {
    for child in &el.children {
        match child.name.as_str() {
            "const" => {
                if let (Some(n), Some(t), Some(v)) = (
                    child.attribute("name"),
                    child.attribute("type"),
                    child.attribute("value"),
                ) {
                    out.insert(scope, n, t, v);
                }
            }
            "module" | "struct" => {
                if let Some(n) = child.attribute("name") {
                    let inner = if scope.is_empty() {
                        n.to_string()
                    } else {
                        format!("{scope}::{n}")
                    };
                    collect_consts(child, &inner, out);
                }
            }
            _ => {}
        }
    }
}

fn join_scope(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{scope}::{name}")
    }
}

/// zerodds-lint: recursion-depth = module nesting depth (indirectly via
/// `parse_module`). The XML foundation's DoS cap limits the depth to ≤ 64.
fn parse_type_def(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<Option<TypeDef>, XmlError> {
    Ok(Some(match el.name.as_str() {
        "module" => TypeDef::Module(parse_module(ctx, consts, scope, el)?),
        "struct" => TypeDef::Struct(parse_struct(ctx, consts, scope, el)?),
        "enum" => TypeDef::Enum(parse_enum(ctx, consts, scope, el)?),
        "union" => TypeDef::Union(parse_union(ctx, consts, scope, el)?),
        "typedef" => TypeDef::Typedef(parse_typedef(ctx, consts, scope, el)?),
        "bitmask" => TypeDef::Bitmask(parse_bitmask(ctx, el)?),
        "bitset" => {
            ctx.vendor_form(
                &el.path,
                "<bitset>",
                "no element: the types schema declares no bit sets",
            )?;
            TypeDef::Bitset(parse_bitset(ctx, el)?)
        }
        "include" | "include2" => TypeDef::Include(parse_include(ctx, el)?),
        "forward_dcl" | "forwardDcl" => {
            if el.name == "forwardDcl" {
                ctx.vendor_form(&el.path, "<forwardDcl>", "<forward_dcl>")?;
            }
            TypeDef::ForwardDcl(parse_forward_dcl(ctx, el)?)
        }
        "const" => TypeDef::Const(parse_const(ctx, el)?),
        "annotation" => TypeDef::Annotation(parse_annotation_type(ctx, el)?),
        _ => {
            ctx.unknown_child(el)?;
            return Ok(None);
        }
    }))
}

/// A bound attribute (`stringMaxLength`, `sequenceMaxLength`,
/// `mapMaxLength`, `mapKeyStringMaxLength`): `None` when the attribute is
/// absent, `Some(None)` for `-1`, the unbounded collection (XTypes 1.3
/// §7.3.2.4: "The literal expression `-1` shall indicate an unbounded
/// collection; no other negative value is permitted"), else the value of
/// the constant expression.
fn bound_attr(
    el: &XmlElement,
    attr: &str,
    consts: &ConstTable,
    scope: &str,
) -> Result<Option<Option<u32>>, XmlError> {
    let Some(raw) = el.attribute(attr) else {
        return Ok(None);
    };
    if raw.trim() == "-1" {
        return Ok(Some(None));
    }
    let v = consts.eval(raw, scope).map_err(|e| context(el, attr, e))?;
    match u32::try_from(v) {
        Ok(n) => Ok(Some(Some(n))),
        Err(_) => Err(XmlError::ValueOutOfRange(format!(
            "{}@{attr}=\"{raw}\" is {v}: a bound is 0..=4294967295, and -1 for unbounded",
            el.path
        ))),
    }
}

/// Puts the element and attribute in front of an evaluation error.
fn context(el: &XmlElement, attr: &str, e: XmlError) -> XmlError {
    match e {
        XmlError::ValueOutOfRange(m) => {
            XmlError::ValueOutOfRange(format!("{}@{attr}: {m}", el.path))
        }
        XmlError::UnresolvedReference(m) => {
            XmlError::UnresolvedReference(format!("{}@{attr}: {m}", el.path))
        }
        other => other,
    }
}

/// `sequenceMaxLength`. XTypes 1.3 §7.3.2.4.2: the attribute marks the
/// member as a sequence and is required for every sequence; `-1` is the
/// unbounded sequence. `StructMember::sequence_max_length` is `None` for
/// "not a sequence", so the unbounded sequence is `Some(0)` — a bound of 0
/// as in `CompleteTypeObject` sequences. A written bound of 0 would read
/// as the same thing and is refused.
fn sequence_bound(
    el: &XmlElement,
    consts: &ConstTable,
    scope: &str,
) -> Result<Option<u32>, XmlError> {
    match bound_attr(el, "sequenceMaxLength", consts, scope)? {
        None => Ok(None),
        Some(None) => Ok(Some(0)),
        Some(Some(0)) => Err(XmlError::ValueOutOfRange(format!(
            "{}@sequenceMaxLength is 0; write -1 for an unbounded sequence",
            el.path
        ))),
        Some(Some(n)) => Ok(Some(n)),
    }
}

/// `arrayDimensions`: a comma-delimited list of constant expressions
/// (XTypes 1.3 §7.3.2.4.1, whitespace around each is not significant).
fn array_dimensions(
    el: &XmlElement,
    consts: &ConstTable,
    scope: &str,
) -> Result<Vec<u32>, XmlError> {
    let Some(raw) = el.attribute("arrayDimensions") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for piece in raw.split(',') {
        let n = consts
            .eval_u32(piece, scope)
            .map_err(|e| context(el, "arrayDimensions", e))?;
        if n == 0 {
            return Err(XmlError::ValueOutOfRange(format!(
                "{}@arrayDimensions=\"{raw}\": a dimension is at least 1",
                el.path
            )));
        }
        out.push(n);
    }
    Ok(out)
}

/// The map attributes of a member or typedef (XTypes 1.3 §7.3.2.4.3):
/// `mapKeyType` (with `mapKeyNonBasicTypeName`) identifies a map and is
/// required for every map, `mapMaxLength` bounds it and is required for
/// every map, and `mapKeyStringMaxLength` bounds a string key. The
/// bound `-1` is the unbounded map, read as `None`.
type MapAttributes = (Option<TypeRef>, Option<u32>, Option<u32>);

fn parse_map_attributes(
    ctx: &mut Ctx,
    el: &XmlElement,
    consts: &ConstTable,
    scope: &str,
) -> Result<MapAttributes, XmlError> {
    let name = el.attribute("name").unwrap_or_default();
    let map_key = match el.attribute("mapKeyType") {
        Some(_) => Some(parse_type_attribute(
            ctx,
            el,
            "mapKeyType",
            "mapKeyNonBasicTypeName",
        )?),
        None => None,
    };
    if map_key.is_some() {
        if el.attribute("mapMaxLength").is_none() {
            return Err(XmlError::MissingRequiredElement(format!(
                "`{name}`: mapKeyType requires mapMaxLength (XTypes 1.3 §7.3.2.4.3)"
            )));
        }
    } else if el.attribute("mapMaxLength").is_some() {
        return Err(XmlError::MissingRequiredElement(format!(
            "`{name}`: mapMaxLength requires mapKeyType (XTypes 1.3 §7.3.2.4.3)"
        )));
    }
    let map_max_length = bound_attr(el, "mapMaxLength", consts, scope)?.flatten();
    let map_key_string_max_length = match &map_key {
        Some(TypeRef::Primitive(PrimitiveType::String | PrimitiveType::WString)) => {
            bound_attr(el, "mapKeyStringMaxLength", consts, scope)?.flatten()
        }
        _ => {
            // "If the 'key' type is not a string type, this attribute is
            // not allowed."
            if el.attribute("mapKeyStringMaxLength").is_some() {
                return Err(XmlError::InvalidXml(format!(
                    "`{name}`: mapKeyStringMaxLength needs a string mapKeyType \
                     (XTypes 1.3 §7.3.2.4.3)"
                )));
            }
            None
        }
    };
    Ok((map_key, map_max_length, map_key_string_max_length))
}

fn parse_include(ctx: &mut Ctx, el: &XmlElement) -> Result<IncludeEntry, XmlError> {
    ctx.check_attributes(el, &["file"])?;
    no_children(ctx, el)?;
    let file = require_attr(el, "file")?.to_string();
    Ok(IncludeEntry { file })
}

fn parse_forward_dcl(ctx: &mut Ctx, el: &XmlElement) -> Result<ForwardDeclEntry, XmlError> {
    ctx.check_attributes(el, &["name", "kind"])?;
    no_children(ctx, el)?;
    let name = identifier_attr(el, "name")?.to_string();
    // `forwardDecl/@kind` is required in the schema and takes the lower-case
    // values; an absent kind and the upper-case spellings are vendor forms.
    let kind = match el.attribute("kind") {
        Some(k @ ("enum" | "struct" | "union" | "bitmask")) => k,
        Some(k @ ("STRUCT" | "UNION")) => {
            ctx.vendor_form(
                &format!("{}@kind", el.path),
                &format!("kind=\"{k}\""),
                &format!("kind=\"{}\"", k.to_lowercase()),
            )?;
            k
        }
        None => {
            ctx.vendor_form(
                &el.path,
                "a <forward_dcl> without kind",
                "the required kind attribute",
            )?;
            "STRUCT"
        }
        Some(other) => {
            return Err(XmlError::BadEnum(format!("{}@kind: `{other}`", el.path)));
        }
    };
    Ok(ForwardDeclEntry {
        name,
        kind: kind.to_string(),
    })
}

fn parse_const(ctx: &mut Ctx, el: &XmlElement) -> Result<ConstEntry, XmlError> {
    ctx.check_attributes(
        el,
        &[
            "name",
            "type",
            "nonBasicTypeName",
            "value",
            "stringMaxLength",
        ],
    )?;
    no_children(ctx, el)?;
    let name = identifier_attr(el, "name")?.to_string();
    let type_name = require_attr(el, "type")?.to_string();
    let value = require_attr(el, "value")?.to_string();
    Ok(ConstEntry {
        name,
        type_name,
        value,
        string_max_length: el.attribute("stringMaxLength").map(ToString::to_string),
    })
}

fn no_children(ctx: &mut Ctx, el: &XmlElement) -> Result<(), XmlError> {
    for c in &el.children {
        ctx.unknown_child(c)?;
    }
    Ok(())
}

/// zerodds-lint: recursion-depth = module nesting depth (indirectly via
/// `parse_type_def`), capped by `parser::MAX_TREE_DEPTH` = 64.
fn parse_module(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<ModuleEntry, XmlError> {
    ctx.check_attributes(el, &["name", "autoid"])?;
    let name = identifier_attr(el, "name")?.to_string();
    let inner = join_scope(scope, &name);
    let auto_id = el.attribute("autoid").map(parse_auto_id).transpose()?;
    let mut types = Vec::new();
    for child in &el.children {
        if let Some(t) = parse_type_def(ctx, consts, &inner, child)? {
            types.push(t);
        }
    }
    Ok(ModuleEntry {
        name,
        types,
        auto_id,
    })
}

fn parse_auto_id(s: &str) -> Result<AutoId, XmlError> {
    match s {
        "hash" => Ok(AutoId::Hash),
        "sequencial" | "sequential" => Ok(AutoId::Sequential),
        other => Err(XmlError::BadEnum(other.to_string())),
    }
}

/// `<annotate name="…"><member name="…" value="…"/>…</annotate>`.
fn parse_annotate(ctx: &mut Ctx, el: &XmlElement) -> Result<Annotation, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = identifier_attr(el, "name")?.to_string();
    let mut members = Vec::new();
    for child in &el.children {
        if child.name == "member" {
            ctx.check_attributes(child, &["name", "value"])?;
            no_children(ctx, child)?;
            members.push(AnnotationValue {
                name: identifier_attr(child, "name")?.to_string(),
                value: child.attribute("value").map(ToString::to_string),
            });
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(Annotation { name, members })
}

/// `<verbatim language=… placement=…><text>…</text></verbatim>`.
fn parse_verbatim(ctx: &mut Ctx, el: &XmlElement) -> Result<Verbatim, XmlError> {
    ctx.check_attributes(el, &["language", "placement"])?;
    let mut text = None;
    for child in &el.children {
        if child.name == "text" {
            if text.is_some() {
                return Err(XmlError::InvalidXml(format!(
                    "<{}> carries <text> more than once",
                    el.path
                )));
            }
            ctx.check_attributes(child, &[])?;
            text = Some(child.text.clone());
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(Verbatim {
        language: el.attribute("language").unwrap_or("*").to_string(),
        placement: el
            .attribute("placement")
            .unwrap_or("before-declaration")
            .to_string(),
        text: text.ok_or_else(|| XmlError::MissingRequiredElement(format!("{}/text", el.path)))?,
    })
}

fn parse_annotation_type(ctx: &mut Ctx, el: &XmlElement) -> Result<AnnotationTypeDef, XmlError> {
    ctx.check_attributes(el, &["name", "baseType"])?;
    let name = identifier_attr(el, "name")?.to_string();
    let mut members = Vec::new();
    for child in &el.children {
        if child.name == "member" {
            ctx.check_attributes(child, &["name", "type", "nonBasicTypeName"])?;
            no_children(ctx, child)?;
            members.push(AnnotationMemberDecl {
                name: identifier_attr(child, "name")?.to_string(),
                type_ref: parse_type_attribute(ctx, child, "type", "nonBasicTypeName")?,
                default_value: None,
            });
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(AnnotationTypeDef {
        name,
        base_type: el.attribute("baseType").map(ToString::to_string),
        members,
    })
}

const STRUCT_ATTRIBUTES: &[&str] = &[
    "name",
    "nested",
    "extensibility",
    "autoid",
    "baseType",
    "base_type",
];

fn parse_struct(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<StructType, XmlError> {
    ctx.check_attributes(el, STRUCT_ATTRIBUTES)?;
    let name = identifier_attr(el, "name")?.to_string();
    let inner = join_scope(scope, &name);
    if el.attribute("base_type").is_some() {
        ctx.vendor_form(
            &format!("{}@base_type", el.path),
            "the base_type attribute",
            "baseType",
        )?;
    }
    if let Some(written) = el.attribute("baseType") {
        check_identifier(el, "baseType", written)?;
    }
    let base_type = el
        .attribute("baseType")
        .or_else(|| el.attribute("base_type"))
        .map(ToString::to_string);
    let mut st = StructType {
        name,
        extensibility: extensibility_attr(ctx, el)?,
        base_type,
        nested: opt_bool(el, "nested")?,
        auto_id: el.attribute("autoid").map(parse_auto_id).transpose()?,
        ..StructType::default()
    };
    for child in &el.children {
        match child.name.as_str() {
            "member" => st
                .members
                .push(parse_member(ctx, consts, &inner, child, true)?),
            "const" => st.constants.push(parse_const(ctx, child)?),
            "annotate" => st.annotations.push(parse_annotate(ctx, child)?),
            "verbatim" => st.verbatims.push(parse_verbatim(ctx, child)?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(st)
}

/// The values of `extensibilityKind` are `final`, `append` and `mutable` in
/// the schema of DDS-XML 1.0 (namespace `http://www.omg.org/spec/DDS-XML`,
/// or none), and `final`, `appendable` and `mutable` in the schema of
/// XTypes 1.3 Annex A (namespace `http://www.omg.org/dds`). The two
/// specifications spell the middle value differently; each element is read
/// by the schema of the namespace it is in, and the other specification's
/// spelling is a vendor form.
fn extensibility_attr(ctx: &mut Ctx, el: &XmlElement) -> Result<Option<Extensibility>, XmlError> {
    let Some(s) = el.attribute("extensibility") else {
        return Ok(None);
    };
    let annex_a = el.namespace.as_deref() == Some(crate::schemas::XTYPES_TYPES_NAMESPACE);
    Ok(Some(match (s, annex_a) {
        ("final", _) => Extensibility::Final,
        ("mutable", _) => Extensibility::Mutable,
        ("append", false) | ("appendable", true) => Extensibility::Appendable,
        ("appendable", false) => {
            ctx.vendor_form(
                &format!("{}@extensibility", el.path),
                "extensibility=\"appendable\"",
                "extensibility=\"append\"",
            )?;
            Extensibility::Appendable
        }
        ("append", true) => {
            return Err(XmlError::BadEnum(format!(
                "{}@extensibility=\"append\" is the spelling of DDS-XML 1.0; the schema of \
                 XTypes 1.3 Annex A, whose namespace this element is in, writes \"appendable\"",
                el.path
            )));
        }
        (other, _) => return Err(XmlError::BadEnum(other.to_string())),
    }))
}

const MEMBER_ATTRIBUTES: &[&str] = &[
    "name",
    "type",
    "nonBasicTypeName",
    "external",
    "tryConstruct",
    "mapKeyType",
    "mapKeyNonBasicTypeName",
    "stringMaxLength",
    "mapKeyStringMaxLength",
    "sequenceMaxLength",
    "mapMaxLength",
    "arrayDimensions",
];

/// The attributes of a struct member (`structMemberDecl`): those of
/// `memberDecl` and the ones only a struct member has.
const STRUCT_MEMBER_ALL: &[&str] = &[
    "name",
    "type",
    "nonBasicTypeName",
    "external",
    "tryConstruct",
    "mapKeyType",
    "mapKeyNonBasicTypeName",
    "stringMaxLength",
    "mapKeyStringMaxLength",
    "sequenceMaxLength",
    "mapMaxLength",
    "arrayDimensions",
    "id",
    "optional",
    "mustUnderstand",
    "must_understand",
    "nonSerialized",
    "key",
];

fn opt_bool(el: &XmlElement, key: &str) -> Result<Option<bool>, XmlError> {
    el.attribute(key)
        .map(crate::qos_parser::parse_bool_strict)
        .transpose()
}

fn parse_member(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
    struct_member: bool,
) -> Result<StructMember, XmlError> {
    if struct_member {
        ctx.check_attributes(el, STRUCT_MEMBER_ALL)?;
    } else {
        ctx.check_attributes(el, MEMBER_ATTRIBUTES)?;
    }
    let name = identifier_attr(el, "name")?.to_string();
    let type_ref = parse_type_attribute(ctx, el, "type", "nonBasicTypeName")?;
    let string_max_length = bound_attr(el, "stringMaxLength", consts, scope)?.flatten();
    let sequence_max_length = sequence_bound(el, consts, scope)?;
    let (map_key, map_max_length, map_key_string_max_length) =
        parse_map_attributes(ctx, el, consts, scope)?;
    let mut m = StructMember {
        name,
        type_ref,
        string_max_length,
        sequence_max_length,
        map_max_length,
        map_key,
        map_key_string_max_length,
        array_dimensions: array_dimensions(el, consts, scope)?,
        external: opt_bool(el, "external")?,
        try_construct: el
            .attribute("tryConstruct")
            .map(parse_try_construct)
            .transpose()?,
        ..StructMember::default()
    };
    if struct_member {
        m.key = opt_bool(el, "key")?.unwrap_or(false);
        m.optional = opt_bool(el, "optional")?.unwrap_or(false);
        if el.attribute("must_understand").is_some() {
            ctx.vendor_form(
                &format!("{}@must_understand", el.path),
                "the must_understand attribute",
                "mustUnderstand",
            )?;
        }
        m.must_understand = opt_bool(el, "mustUnderstand")?.unwrap_or(false)
            || opt_bool(el, "must_understand")?.unwrap_or(false);
        m.id = ranged(el, "id", 0, 0x0FFF_FFFF, "memberId")?;
        m.non_serialized = opt_bool(el, "nonSerialized")?;
    }
    for child in &el.children {
        if child.name == "annotate" {
            m.annotations.push(parse_annotate(ctx, child)?);
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(m)
}

fn parse_try_construct(s: &str) -> Result<TryConstruct, XmlError> {
    match s {
        "discard" => Ok(TryConstruct::Discard),
        "use_default" => Ok(TryConstruct::UseDefault),
        "trim" => Ok(TryConstruct::Trim),
        other => Err(XmlError::BadEnum(other.to_string())),
    }
}

/// Reads a `type`-style attribute (XTypes 1.3 §7.3.2.4: "The element type
/// identified by the type and nonBasicTypeName attributes"). `nonBasic`
/// names the type through the parallel attribute; every other spelling is
/// a primitive symbol or a type name.
fn parse_type_attribute(
    ctx: &mut Ctx,
    el: &XmlElement,
    type_attr: &str,
    nonbasic_attr: &str,
) -> Result<TypeRef, XmlError> {
    let written = require_attr(el, type_attr)?;
    if written == "nonBasic" {
        let name = identifier_attr(el, nonbasic_attr)?;
        return Ok(TypeRef::Named(name.to_string()));
    }
    if written == "nonbasic" {
        ctx.vendor_form(
            &format!("{}@{type_attr}", el.path),
            "type=\"nonbasic\"",
            "type=\"nonBasic\"",
        )?;
        let name = identifier_attr(el, nonbasic_attr)?;
        return Ok(TypeRef::Named(name.to_string()));
    }
    // XTypes 1.3 §7.3.2.2: "It is an error to include this attribute if the
    // type attribute does not indicate a non-basic type." For a map key
    // §7.3.2.4.3 says it again: "If the 'key' type is basic, the
    // mapKeyNonBasicTypeName attribute is not allowed."
    if el.attribute(nonbasic_attr).is_some() {
        return Err(XmlError::InvalidXml(format!(
            "{}: {nonbasic_attr} on an element whose {type_attr} is `{written}`, which is not \
             nonBasic",
            el.path
        )));
    }
    parse_type_ref(ctx, &format!("{}@{type_attr}", el.path), written)
}

/// The spelling of a primitive in the schema's `type` attribute
/// (`primitiveTypeKind` and XTypes 1.3 Annex A).
fn schema_spelling(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "boolean",
        PrimitiveType::Octet => "byte",
        PrimitiveType::Int8 => "int8",
        PrimitiveType::UInt8 => "uint8",
        PrimitiveType::Char => "char8",
        PrimitiveType::WChar => "char16",
        PrimitiveType::Short => "int16",
        PrimitiveType::UShort => "uint16",
        PrimitiveType::Long => "int32",
        PrimitiveType::ULong => "uint32",
        PrimitiveType::LongLong => "int64",
        PrimitiveType::ULongLong => "uint64",
        PrimitiveType::Float => "float32",
        PrimitiveType::Double => "float64",
        PrimitiveType::LongDouble => "float128",
        PrimitiveType::String => "string",
        PrimitiveType::WString => "wstring",
    }
}

fn parse_type_ref(ctx: &mut Ctx, path: &str, s: &str) -> Result<TypeRef, XmlError> {
    if let Some(p) = PrimitiveType::from_xml(s) {
        let spec = schema_spelling(p);
        if s != spec {
            ctx.vendor_form(
                path,
                &format!("the IDL name `{s}` of a primitive"),
                &format!("`{spec}`"),
            )?;
        }
        Ok(TypeRef::Primitive(p))
    } else {
        ctx.vendor_form(
            path,
            &format!("the type name `{s}` written as the value of `type`"),
            &format!("type=\"nonBasic\" with nonBasicTypeName=\"{s}\""),
        )?;
        Ok(TypeRef::Named(s.to_string()))
    }
}

/// The value of a constant-expression attribute as a decimal string, or
/// the text as written when it is not an integer expression but a name that
/// is no `<const>` (an enumerator, `default`, …).
fn case_label(raw: &str, consts: &ConstTable, scope: &str) -> Result<String, XmlError> {
    match consts.eval(raw, scope) {
        Ok(v) => Ok(v.to_string()),
        Err(XmlError::UnresolvedReference(_)) => Ok(raw.to_string()),
        Err(e) => Err(e),
    }
}

fn parse_enum(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<EnumType, XmlError> {
    ctx.check_attributes(el, &["name", "bitBound", "extensibility"])?;
    let name = identifier_attr(el, "name")?.to_string();
    let inner = join_scope(scope, &name);
    let mut e = EnumType {
        name,
        bit_bound: ranged(el, "bitBound", 1, 32, "enumBitBound")?,
        extensibility: extensibility_attr(ctx, el)?,
        ..EnumType::default()
    };
    for child in &el.children {
        match child.name.as_str() {
            "enumerator" => {
                ctx.check_attributes(child, &["name", "value", "defaultLiteral"])?;
                let n = identifier_attr(child, "name")?.to_string();
                let value = match child.attribute("value") {
                    Some(raw) => {
                        let v = consts
                            .eval(raw, &inner)
                            .map_err(|e| context(child, "value", e))?;
                        // A value above `i32::MAX` up to `0xFFFFFFFF` is
                        // the bit pattern of a negative `long` (§7.1.4
                        // Tab.7.1 writes a `long` as `0x80000000` to
                        // `0x7fffffff`).
                        Some(
                            i32::try_from(v)
                                .or_else(|_| u32::try_from(v).map(|u| u as i32))
                                .map_err(|_| {
                                    XmlError::ValueOutOfRange(format!(
                                        "{}@value=\"{raw}\" is {v}, outside the 32-bit range",
                                        child.path
                                    ))
                                })?,
                        )
                    }
                    None => None,
                };
                let mut lit = EnumLiteral {
                    name: n,
                    value,
                    default_literal: opt_bool(child, "defaultLiteral")?,
                    annotations: Vec::new(),
                };
                for c in &child.children {
                    if c.name == "annotate" {
                        lit.annotations.push(parse_annotate(ctx, c)?);
                    } else {
                        ctx.unknown_child(c)?;
                    }
                }
                e.enumerators.push(lit);
            }
            "annotate" => e.annotations.push(parse_annotate(ctx, child)?),
            "verbatim" => e.verbatims.push(parse_verbatim(ctx, child)?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(e)
}

fn parse_union(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<UnionType, XmlError> {
    ctx.check_attributes(
        el,
        &["name", "nested", "extensibility", "autoid", "discriminator"],
    )?;
    let name = identifier_attr(el, "name")?.to_string();
    let inner = join_scope(scope, &name);
    // XTypes 1.3 §7.3.2.5.2 / Annex A `unionDecl`: the discriminator is a
    // `<discriminator type=".." nonBasicTypeName=".."/>` child. The
    // `discriminator="…"` attribute is this crate's older spelling and is
    // still read; naming the discriminator twice is ambiguous.
    let disc_el = el.child("discriminator");
    let mut u = UnionType {
        name: name.clone(),
        extensibility: extensibility_attr(ctx, el)?,
        nested: opt_bool(el, "nested")?,
        auto_id: el.attribute("autoid").map(parse_auto_id).transpose()?,
        ..UnionType::default()
    };
    u.discriminator = match (disc_el, el.attribute("discriminator")) {
        (Some(_), Some(_)) => {
            return Err(XmlError::InvalidXml(format!(
                "union `{name}` names its discriminator as an element and as an attribute"
            )));
        }
        (Some(d), None) => {
            ctx.check_attributes(d, &["type", "nonBasicTypeName", "key"])?;
            u.discriminator_key = opt_bool(d, "key")?.unwrap_or(false);
            for c in &d.children {
                if c.name == "annotate" {
                    u.discriminator_annotations.push(parse_annotate(ctx, c)?);
                } else {
                    ctx.unknown_child(c)?;
                }
            }
            parse_type_attribute(ctx, d, "type", "nonBasicTypeName")?
        }
        (None, Some(attr)) => {
            let path = format!("{}@discriminator", el.path);
            ctx.vendor_form(
                &path,
                "the discriminator=\"…\" attribute of <union>",
                "a <discriminator type=\"…\"/> child",
            )?;
            parse_type_ref(ctx, &path, attr)?
        }
        (None, None) => {
            return Err(XmlError::MissingRequiredElement(format!(
                "union `{name}`: discriminator"
            )));
        }
    };
    for child in &el.children {
        match child.name.as_str() {
            "discriminator" => {}
            "case" => u.cases.push(parse_union_case(ctx, consts, &inner, child)?),
            "annotate" => u.annotations.push(parse_annotate(ctx, child)?),
            "verbatim" => u.verbatims.push(parse_verbatim(ctx, child)?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(u)
}

fn parse_union_case(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<UnionCase, XmlError> {
    ctx.check_attributes(el, &[])?;
    let mut discriminators = Vec::new();
    let mut member: Option<StructMember> = None;
    for child in &el.children {
        match child.name.as_str() {
            "caseDiscriminator" => {
                ctx.check_attributes(child, &["value"])?;
                no_children(ctx, child)?;
                let v = require_attr(child, "value")?;
                if v == "default" {
                    discriminators.push(UnionDiscriminator::Default);
                } else {
                    discriminators.push(UnionDiscriminator::Value(
                        case_label(v, consts, scope).map_err(|e| context(child, "value", e))?,
                    ));
                }
            }
            "member" => {
                if member.is_some() {
                    return Err(XmlError::InvalidXml(format!(
                        "<{}> carries <member> more than once",
                        el.path
                    )));
                }
                member = Some(parse_member(ctx, consts, scope, child, false)?);
            }
            _ => ctx.unknown_child(child)?,
        }
    }
    let member = member.ok_or_else(|| XmlError::MissingRequiredElement("member".to_string()))?;
    Ok(UnionCase {
        discriminators,
        member,
    })
}

const TYPEDEF_ATTRIBUTES: &[&str] = &[
    "name",
    "type",
    "nonBasicTypeName",
    "mapKeyType",
    "mapKeyNonBasicTypeName",
    "stringMaxLength",
    "mapKeyStringMaxLength",
    "sequenceMaxLength",
    "mapMaxLength",
    "arrayDimensions",
    "external",
];

fn parse_typedef(
    ctx: &mut Ctx,
    consts: &ConstTable,
    scope: &str,
    el: &XmlElement,
) -> Result<TypedefType, XmlError> {
    ctx.check_attributes(el, TYPEDEF_ATTRIBUTES)?;
    no_children(ctx, el)?;
    let name = identifier_attr(el, "name")?.to_string();
    let type_ref = parse_type_attribute(ctx, el, "type", "nonBasicTypeName")?;
    let (map_key, map_max_length, map_key_string_max_length) =
        parse_map_attributes(ctx, el, consts, scope)?;
    Ok(TypedefType {
        name,
        type_ref,
        array_dimensions: array_dimensions(el, consts, scope)?,
        sequence_max_length: sequence_bound(el, consts, scope)?,
        string_max_length: bound_attr(el, "stringMaxLength", consts, scope)?.flatten(),
        map_max_length,
        map_key,
        map_key_string_max_length,
        external: opt_bool(el, "external")?,
    })
}

fn parse_bitmask(ctx: &mut Ctx, el: &XmlElement) -> Result<BitmaskType, XmlError> {
    ctx.check_attributes(el, &["name", "bitBound"])?;
    let name = identifier_attr(el, "name")?.to_string();
    let mut b = BitmaskType {
        name,
        bit_bound: ranged(el, "bitBound", 1, 64, "bitmaskBitBound")?,
        ..BitmaskType::default()
    };
    for child in &el.children {
        match child.name.as_str() {
            // XTypes 1.3 §7.3.2.7.2 / Annex A `bitmaskDecl`: `<flag name
            // position/>`. `<flag>` is this crate's older spelling.
            "flag" | "bit_value" => {
                if child.name == "bit_value" {
                    ctx.vendor_form(&child.path, "<flag>", "<flag>")?;
                }
                ctx.check_attributes(child, &["name", "position"])?;
                let n = identifier_attr(child, "name")?.to_string();
                // `flagDecl/@position` is `use="required"` (Annex A, S. 271).
                require_attr(child, "position")?;
                let position = ranged(child, "position", 0, 63, "flagIndex")?;
                let mut v = BitValue {
                    name: n,
                    position,
                    annotations: Vec::new(),
                };
                for c in &child.children {
                    if c.name == "annotate" {
                        v.annotations.push(parse_annotate(ctx, c)?);
                    } else {
                        ctx.unknown_child(c)?;
                    }
                }
                b.bit_values.push(v);
            }
            "annotate" => b.annotations.push(parse_annotate(ctx, child)?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(b)
}

fn parse_bitset(ctx: &mut Ctx, el: &XmlElement) -> Result<BitsetType, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = identifier_attr(el, "name")?.to_string();
    let mut bit_fields = Vec::new();
    for child in &el.children {
        if child.name == "bitfield" {
            ctx.check_attributes(child, &["name", "type", "mask"])?;
            no_children(ctx, child)?;
            let n = identifier_attr(child, "name")?.to_string();
            let type_str = require_attr(child, "type")?;
            let mask = require_attr(child, "mask")?.to_string();
            bit_fields.push(BitField {
                name: n,
                type_ref: parse_type_ref(ctx, &format!("{}@type", child.path), type_str)?,
                mask,
            });
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(BitsetType { name, bit_fields })
}

/// An integer attribute within the range of its Annex A simple type
/// (`memberId` 0..=0x0FFFFFFF, `enumBitBound` 1..=32, `bitmaskBitBound`
/// 1..=64, the position of a flag 0..=63).
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] naming the attribute and the range.
fn ranged(
    el: &XmlElement,
    key: &str,
    min: u32,
    max: u32,
    schema_type: &str,
) -> Result<Option<u32>, XmlError> {
    let Some(written) = el.attribute(key) else {
        return Ok(None);
    };
    let value = parse_ulong(written)?;
    if (min..=max).contains(&value) {
        Ok(Some(value))
    } else {
        Err(XmlError::ValueOutOfRange(format!(
            "{}@{key}=\"{written}\" is outside {min}..={max}, the range of {schema_type} \
             (XTypes 1.3 Annex A)",
            el.path
        )))
    }
}

/// Whether `s` matches the `identifierName` of Annex A (S. 243),
/// `([a-zA-Z]|::)([a-zA-Z_0-9]|::)*`, which an XSD pattern anchors at both
/// ends: a letter or `::` first, then letters, digits, `_` and `::`.
#[must_use]
pub fn is_identifier_name(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let mut first = true;
    while i < b.len() {
        if b[i] == b':' {
            if b.get(i + 1) != Some(&b':') {
                return false;
            }
            i += 2;
        } else if (first && b[i].is_ascii_alphabetic())
            || (!first && (b[i].is_ascii_alphanumeric() || b[i] == b'_'))
        {
            i += 1;
        } else {
            return false;
        }
        first = false;
    }
    !first
}

/// A required attribute whose Annex A type is `identifierName`.
///
/// # Errors
/// [`XmlError::MissingRequiredElement`] when absent,
/// [`XmlError::InvalidXml`] when it does not match the pattern.
fn identifier_attr<'a>(el: &'a XmlElement, key: &str) -> Result<&'a str, XmlError> {
    let value = require_attr(el, key)?;
    check_identifier(el, key, value)?;
    Ok(value)
}

/// An `identifierName` attribute value, checked against the pattern.
fn check_identifier(el: &XmlElement, key: &str, value: &str) -> Result<(), XmlError> {
    if is_identifier_name(value) {
        Ok(())
    } else {
        Err(XmlError::InvalidXml(format!(
            "{}@{key}=\"{value}\" is no identifierName ([a-zA-Z] or :: first, then letters, \
             digits, _ and ::; XTypes 1.3 Annex A)",
            el.path
        )))
    }
}

fn require_attr<'a>(el: &'a XmlElement, key: &str) -> Result<&'a str, XmlError> {
    el.attribute(key)
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("@{key} on <{}>", el.name)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_struct() {
        let xml = r#"<types>
            <struct name="State">
                <member name="id" type="int32" key="true"/>
            </struct>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs.len(), 1);
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        assert_eq!(s.name, "State");
        assert_eq!(s.members.len(), 1);
        assert!(s.members[0].key);
    }

    #[test]
    fn parse_module_nested() {
        let xml = r#"<types>
            <module name="Outer">
                <module name="Inner">
                    <struct name="X"/>
                </module>
            </module>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Module(m) = &libs[0].types[0] else {
            panic!()
        };
        assert_eq!(m.name, "Outer");
        let TypeDef::Module(inner) = &m.types[0] else {
            panic!()
        };
        assert_eq!(inner.name, "Inner");
    }

    #[test]
    fn dds_root_with_multiple_types_blocks() {
        let xml = r#"<dds>
            <types><struct name="A"/></types>
            <types><struct name="B"/></types>
        </dds>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs.len(), 2);
    }

    #[test]
    fn unknown_root_rejected() {
        let xml = r#"<other/>"#;
        let err = parse_type_libraries(xml).expect_err("err");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    // ---- §7.3.2 XML type repr extended constructs ----

    #[test]
    fn parse_include_element() {
        let xml = r#"<types>
            <include file="shared/common.xml"/>
            <struct name="Local"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs[0].types.len(), 2);
        let TypeDef::Include(inc) = &libs[0].types[0] else {
            panic!("expected Include");
        };
        assert_eq!(inc.file, "shared/common.xml");
    }

    #[test]
    fn parse_forward_dcl_element() {
        let xml = r#"<types>
            <forward_dcl name="Node" kind="struct"/>
            <struct name="Node">
                <member name="payload" type="int32"/>
            </struct>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::ForwardDcl(fwd) = &libs[0].types[0] else {
            panic!("expected ForwardDcl");
        };
        assert_eq!(fwd.name, "Node");
        assert_eq!(fwd.kind, "struct");
    }

    #[test]
    fn parse_forward_dcl_default_kind_struct() {
        let xml = r#"<types>
            <forward_dcl name="X"/>
        </types>"#;
        // The schema requires `kind`; an absent one is a vendor form.
        assert!(parse_type_libraries(xml).is_err());
        let libs = parse_type_libraries_with(xml, &ReadOptions::accept_vendor_forms())
            .expect("parse")
            .value;
        let TypeDef::ForwardDcl(fwd) = &libs[0].types[0] else {
            panic!("expected ForwardDcl");
        };
        assert_eq!(fwd.kind, "STRUCT");
    }

    #[test]
    fn parse_const_element() {
        let xml = r#"<types>
            <const name="MaxSize" type="long" value="1024"/>
            <const name="Greeting" type="string" value="hello"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs[0].types.len(), 2);
        let TypeDef::Const(c1) = &libs[0].types[0] else {
            panic!("expected Const");
        };
        assert_eq!(c1.name, "MaxSize");
        assert_eq!(c1.type_name, "long");
        assert_eq!(c1.value, "1024");
        let TypeDef::Const(c2) = &libs[0].types[1] else {
            panic!("expected Const");
        };
        assert_eq!(c2.value, "hello");
    }

    #[test]
    fn parse_const_missing_value_attribute_rejected() {
        let xml = r#"<types>
            <const name="X" type="long"/>
        </types>"#;
        let res = parse_type_libraries(xml);
        assert!(res.is_err());
    }

    #[test]
    fn parse_include_missing_file_attribute_rejected() {
        let xml = r#"<types>
            <include/>
        </types>"#;
        let res = parse_type_libraries(xml);
        assert!(res.is_err());
    }

    /// Reads `<member key="...">` through the real attribute path.
    fn member_key_attribute(value: &str) -> Result<bool, XmlError> {
        let xml = format!(
            r#"<types><struct name="S"><member name="m" type="int32" key="{value}"/></struct></types>"#
        );
        let libs = parse_type_libraries(&xml)?;
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        Ok(s.members[0].key)
    }

    /// DDS-XML 1.0 §7.1.5 Tab.7.2 — an attribute of type `boolean` takes
    /// `1`/`true` for yes and `0`/`false` for no, and the values are case
    /// sensitive. The check runs through the attribute path itself
    /// (`<member key="…">`), not through the value parser in isolation.
    #[test]
    fn spec_7_1_5_1_boolean_attribute_is_case_sensitive() {
        // The four legal spellings.
        assert!(member_key_attribute("true").expect("true"));
        assert!(member_key_attribute("1").expect("1"));
        assert!(!member_key_attribute("false").expect("false"));
        assert!(!member_key_attribute("0").expect("0"));

        // Case variants are not legal values and must be rejected rather
        // than silently read as the lowercase spelling.
        for bad in ["TRUE", "True", "FALSE", "False", "Yes", "yes"] {
            let res = member_key_attribute(bad);
            assert!(
                matches!(res, Err(XmlError::ValueOutOfRange(_))),
                "attribute value `{bad}` must be rejected, got {res:?}"
            );
        }
    }

    /// XTypes 1.3 §7.3.2.4 / DDS-XML 1.0 §7.3.3.1: `type="nonBasic"`
    /// names the type through `nonBasicTypeName`, not the literal
    /// `nonBasic`.
    #[test]
    fn spec_nonbasic_type_is_named_by_nonbasic_type_name() {
        let xml = r#"<types>
            <struct name="S">
                <member name="m" type="nonBasic" nonBasicTypeName="MyOtherStructure"/>
            </struct>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Named("MyOtherStructure".into())
        );
    }

    #[test]
    fn spec_nonbasic_without_name_rejected() {
        let xml = r#"<types><struct name="S">
            <member name="m" type="nonBasic"/>
        </struct></types>"#;
        let err = parse_type_libraries(xml).expect_err("no name");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    /// XTypes 1.3 §7.3.2.4.3: `mapKeyType` identifies a map, `mapMaxLength`
    /// bounds it, `mapKeyStringMaxLength` bounds a string key.
    #[test]
    fn spec_map_members_are_read() {
        let xml = r#"<types>
            <struct name="S">
                <member name="m" type="float32" mapKeyType="int32" mapMaxLength="-1"/>
                <member name="n" type="nonBasic" nonBasicTypeName="MyOtherStructure"
                        mapKeyType="string" mapKeyStringMaxLength="128"
                        mapMaxLength="6"/>
            </struct>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        let m = &s.members[0];
        assert!(m.is_map());
        assert_eq!(m.map_key, Some(TypeRef::Primitive(PrimitiveType::Long)));
        // mapMaxLength="-1" is the unbounded map.
        assert_eq!(m.map_max_length, None);

        let n = &s.members[1];
        assert!(n.is_map());
        assert_eq!(n.map_key, Some(TypeRef::Primitive(PrimitiveType::String)));
        assert_eq!(n.map_key_string_max_length, Some(128));
        assert_eq!(n.map_max_length, Some(6));
        assert_eq!(n.type_ref, TypeRef::Named("MyOtherStructure".into()));
    }

    #[test]
    fn spec_map_requires_key_and_bound() {
        let no_key = r#"<types><struct name="S">
            <member name="m" type="int32" mapMaxLength="4"/>
        </struct></types>"#;
        assert!(matches!(
            parse_type_libraries(no_key),
            Err(XmlError::MissingRequiredElement(_))
        ));
        let no_bound = r#"<types><struct name="S">
            <member name="m" type="int32" mapKeyType="int32"/>
        </struct></types>"#;
        assert!(matches!(
            parse_type_libraries(no_bound),
            Err(XmlError::MissingRequiredElement(_))
        ));
    }

    /// DDS-XML 1.0 §7.1.5 Tab.7.2 — an attribute of type `long` takes a
    /// 32-bit signed decimal, the `0x…` hex spelling of the same bit
    /// pattern, or the symbol `LENGTH_UNLIMITED` (= -1). The check runs
    /// through the attribute path (`<enumerator value="…">`).
    #[test]
    fn spec_7_1_5_3_long_attribute_takes_decimal_hex_and_length_unlimited() {
        let xml = r#"<types>
            <enum name="E">
                <enumerator name="MIN" value="-2147483648"/>
                <enumerator name="MAX" value="2147483647"/>
                <enumerator name="HEX" value="0x7fffffff"/>
                <enumerator name="NEG_HEX" value="0x80000000"/>
                <enumerator name="UNLIMITED" value="LENGTH_UNLIMITED"/>
            </enum>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Enum(e) = &libs[0].types[0] else {
            panic!("expected enum");
        };
        let values: Vec<Option<i32>> = e.enumerators.iter().map(|l| l.value).collect();
        assert_eq!(
            values,
            alloc::vec![
                Some(i32::MIN),
                Some(i32::MAX),
                Some(0x7fff_ffff),
                Some(i32::MIN),
                Some(crate::types::LENGTH_UNLIMITED),
            ]
        );
        assert_eq!(crate::types::LENGTH_UNLIMITED, -1);

        // Something that is neither a number nor the symbol is an error,
        // not a dropped value.
        let bad = r#"<types><enum name="E"><enumerator name="X" value="huge"/></enum></types>"#;
        assert!(matches!(
            parse_type_libraries(bad),
            Err(XmlError::UnresolvedReference(_))
        ));
    }

    /// XTypes 1.3 §7.3.2.4.2: `sequenceMaxLength="-1"` is the unbounded
    /// sequence. The member stays a sequence (`Some(0)`); `None` would read
    /// as "no sequence".
    #[test]
    fn spec_unbounded_sequence_stays_a_sequence() {
        let xml = r#"<types>
            <struct name="S">
                <member name="a" type="int32" sequenceMaxLength="-1"/>
                <member name="b" type="int32" sequenceMaxLength="18"/>
            </struct>
            <typedef name="T" type="int32" sequenceMaxLength="-1"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        assert_eq!(s.members[0].sequence_max_length, Some(0));
        assert_eq!(s.members[1].sequence_max_length, Some(18));
        let TypeDef::Typedef(t) = &libs[0].types[1] else {
            panic!("expected typedef");
        };
        assert_eq!(t.sequence_max_length, Some(0));
    }

    /// XTypes 1.3 §7.3.2.4.3: "If the 'key' type is basic, the
    /// mapKeyNonBasicTypeName attribute is not allowed", and
    /// mapKeyStringMaxLength is allowed for a string key only.
    #[test]
    fn spec_map_key_attributes_are_checked() {
        let basic_with_name = r#"<types><struct name="S">
            <member name="m" type="int32" mapKeyType="int32"
                    mapKeyNonBasicTypeName="K" mapMaxLength="4"/>
        </struct></types>"#;
        assert!(matches!(
            parse_type_libraries(basic_with_name),
            Err(XmlError::InvalidXml(_))
        ));
        let bound_on_int_key = r#"<types><struct name="S">
            <member name="m" type="int32" mapKeyType="int32"
                    mapKeyStringMaxLength="4" mapMaxLength="4"/>
        </struct></types>"#;
        assert!(matches!(
            parse_type_libraries(bound_on_int_key),
            Err(XmlError::InvalidXml(_))
        ));
        let nonbasic_key = r#"<types><struct name="S">
            <member name="m" type="int32" mapKeyType="nonBasic"
                    mapKeyNonBasicTypeName="K" mapMaxLength="4"/>
        </struct></types>"#;
        let libs = parse_type_libraries(nonbasic_key).expect("parse");
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        assert_eq!(s.members[0].map_key, Some(TypeRef::Named("K".into())));
    }

    /// A typedef can be a map as well (XTypes 1.3 §7.3.2.4.3 describes the
    /// attributes for the type of a member or alias alike); they are read,
    /// not dropped.
    #[test]
    fn spec_map_typedef_is_read() {
        let xml = r#"<types>
            <typedef name="Lookup" type="float64" mapKeyType="string"
                     mapKeyStringMaxLength="16" mapMaxLength="-1"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Typedef(t) = &libs[0].types[0] else {
            panic!("expected typedef");
        };
        assert!(t.is_map());
        assert_eq!(t.map_key, Some(TypeRef::Primitive(PrimitiveType::String)));
        assert_eq!(t.map_key_string_max_length, Some(16));
        assert_eq!(t.map_max_length, None);
    }

    /// XTypes 1.3 §7.3.2.5.2: the example union writes its discriminator as
    /// a child element, and §7.3.2.7.2 writes bitmask flags as `<flag>`.
    #[test]
    fn spec_union_discriminator_element_and_bitmask_flag() {
        let xml = r#"<types>
            <union name="MyUnion">
                <discriminator type="int32"/>
                <case>
                    <caseDiscriminator value="1"/>
                    <caseDiscriminator value="2"/>
                    <member name="small_value" type="float32"/>
                </case>
                <case>
                    <caseDiscriminator value="default"/>
                    <member name="large_value" type="float64"/>
                </case>
            </union>
            <union name="Named">
                <discriminator type="nonBasic" nonBasicTypeName="MyEnum"/>
                <case>
                    <caseDiscriminator value="A"/>
                    <member name="v" type="int32"/>
                </case>
            </union>
            <bitmask name="MyBitmask" bitBound="64">
                <flag name="FIRST_BIT" position="0"/>
                <flag name="SECOND_BIT" position="1"/>
            </bitmask>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Union(u) = &libs[0].types[0] else {
            panic!("expected union");
        };
        assert_eq!(u.discriminator, TypeRef::Primitive(PrimitiveType::Long));
        assert_eq!(u.cases.len(), 2);
        assert_eq!(u.cases[0].discriminators.len(), 2);
        let TypeDef::Union(named) = &libs[0].types[1] else {
            panic!("expected union");
        };
        assert_eq!(named.discriminator, TypeRef::Named("MyEnum".into()));
        let TypeDef::Bitmask(b) = &libs[0].types[2] else {
            panic!("expected bitmask");
        };
        assert_eq!(b.bit_bound, Some(64));
        assert_eq!(b.bit_values.len(), 2);
        assert_eq!(b.bit_values[1].name, "SECOND_BIT");
        assert_eq!(b.bit_values[1].position, Some(1));
    }

    #[test]
    fn union_discriminator_given_twice_or_not_at_all_is_refused() {
        let both = r#"<types><union name="U" discriminator="long">
            <discriminator type="int32"/>
            <case><caseDiscriminator value="0"/><member name="a" type="int32"/></case>
        </union></types>"#;
        assert!(matches!(
            parse_type_libraries(both),
            Err(XmlError::InvalidXml(_))
        ));
        let none = r#"<types><union name="U">
            <case><caseDiscriminator value="0"/><member name="a" type="int32"/></case>
        </union></types>"#;
        assert!(matches!(
            parse_type_libraries(none),
            Err(XmlError::MissingRequiredElement(_))
        ));
    }

    /// XTypes 1.3 Annex A `allTypeKind`: `char8`, `char16`, `int8` and
    /// `uint8` are primitive symbols, not type names.
    #[test]
    fn spec_all_type_kind_symbols_are_primitives() {
        let xml = r#"<types><struct name="S">
            <member name="a" type="char8"/>
            <member name="b" type="char16"/>
            <member name="c" type="int8"/>
            <member name="d" type="uint8"/>
            <member name="e" type="byte"/>
            <member name="f" type="float128"/>
        </struct></types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        let kinds: Vec<_> = s.members.iter().map(|m| m.type_ref.clone()).collect();
        assert_eq!(
            kinds,
            [
                TypeRef::Primitive(PrimitiveType::Char),
                TypeRef::Primitive(PrimitiveType::WChar),
                TypeRef::Primitive(PrimitiveType::Int8),
                TypeRef::Primitive(PrimitiveType::UInt8),
                TypeRef::Primitive(PrimitiveType::Octet),
                TypeRef::Primitive(PrimitiveType::LongDouble),
            ]
        );
    }
}
