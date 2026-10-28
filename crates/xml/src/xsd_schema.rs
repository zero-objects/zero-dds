// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! W3C XML Schema (XSD) → DDS-XTypes bridge (XTypes 1.3 §7.3.3).
//!
//! Spec OMG XTypes 1.3 §7.3.3 allows XSD as an alternative type
//! representation alongside IDL/XML/TypeObject: "An XSD Representation of a
//! given type shall be as if the OMG-standard IDL mapping to XSD were
//! applied to the IDL Representation of the type". This module maps an XSD
//! document onto the internal [`TypeLibrary`] data model from
//! [`crate::xtypes_def`]; via [`crate::typeobject_bridge`] a
//! [`zerodds_types::TypeObject`] is computed from it. The opposite
//! direction, a library to a schema, is [`crate::xsd_writer`].
//!
//! # Mapping (XTypes 1.3 §7.3.3)
//!
//! ```text
//! XSD                                       | DDS type
//! ----------------------------------------- | -------------------------
//! <xsd:schema>                              | TypeLibrary (top-level)
//! name="M.S" (§7.3.3.3: "MyModule.MyStructure")
//!                                           | type S in module M
//! <xsd:complexType name=X>                  | <struct name=X>
//!   <xsd:sequence>                          |
//!     <xsd:element name=f type=t/>          | <member name=f type=t/>
//! minOccurs=0 (element), §7.3.3.2.2         | optional=true
//! <xsd:complexContent><xsd:extension        |
//!   base=P>, §7.3.3.2.1                     | baseType=P
//! <sequence><discriminator/><choice>        | <union>
//! <xsd:simpleType name=E>                   | <enum name=E>
//!   <xsd:restriction base="xsd:string">     |
//!     <xsd:enumeration value=A/>            | <enumerator name=A/>
//! <xsd:simpleType><xsd:list>                | <bitmask>
//! <xsd:complexType> with <sequence> of one  |
//!   element named `item` (4.1.7.5, 4.1.7.6;  |
//!   `item1`, ... for further dimensions; the  |
//!   `element` of DDS-XML 1.0 is read too)     | sequence or array
//! <xsd:complexContent><xsd:restriction      |
//!   base=P> restating the content, 4.1.7.3    | alias of P
//! <xsd:sequence maxOccurs=N> of `key` and   |
//!   `value`, §7.3.3.4                       | map<key, value, N>
//! top-level <xsd:element name=T type=T/>    | T is not nested, §7.3.3.3
//! ```
//!
//! # Annotations (§7.3.3.1)
//!
//! A `complexType` with an `xsd:annotation` whose `xsd:appinfo` has the
//! `source` `http://www.omg.org/Type/Annotation/Definition` is an annotation
//! type (§7.3.3.1.1): each `xsd:attribute` is a member, with its type and
//! `default`. An `xsd:appinfo` with the `source`
//! `http://www.omg.org/Type/Annotation/Usage` in a definition's element
//! applies annotations (§7.3.3.1.2): one `annotate` element per annotation,
//! its `type` attribute naming the annotation and the other attributes the
//! values of its members. What the IDL Type Representation writes with a
//! built-in annotation (§7.3.3.1.3) is read from the annotation of the same
//! name: `key`, `id`, `optional`, `must_understand`, `non_serialized`,
//! `external`, `try_construct`, `nested`, `autoid`, `final`, `appendable`,
//! `mutable`, `extensibility`, `bit_bound`, `value`, `default_literal`,
//! `position`, `verbatim`. `minOccurs="0"` is `@optional` as well
//! (§7.3.3.2.2).
//!
//! # Built-In Type Mapping
//!
//! ```text
//! xsd:boolean | xsd:byte | xsd:short | xsd:int     | xsd:long
//! -> boolean    int8       short       long          longlong
//!
//! xsd:unsignedByte | xsd:unsignedShort | xsd:unsignedInt | xsd:unsignedLong
//! -> octet           ushort              ulong             ulonglong
//!
//! xsd:float | xsd:double  | xsd:string | xsd:decimal
//! -> float    double        string       longdouble
//! ```
//!
//! `uint8`, `wchar`, `wstring` and a one-character `string` have the same
//! XSD type as another DDS type; the annotation `uint8`, `wchar`, `wstring`
//! or `char` on the element says which.
//!
//! # Deliberately not in the crate
//!
//! - A full XSD-1.1 validator (key/keyref, assertions).
//! - XSD `xsd:include`/`xsd:import` (multi-file schemas).
//! - Mixed content / attribute-as-member.

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};
use crate::xtypes_def::{
    Annotation, AnnotationMemberDecl, AnnotationTypeDef, AnnotationValue, AutoId, BitValue,
    BitmaskType, EnumLiteral, EnumType, Extensibility, ModuleEntry, PrimitiveType, StructMember,
    StructType, TryConstruct, TypeDef, TypeLibrary, TypeRef, TypedefType, UnionCase,
    UnionDiscriminator, UnionType, Verbatim,
};

/// `source` of an `xsd:appinfo` that defines an OMG annotation type
/// (§7.3.3.1.1).
pub const APPINFO_ANNOTATION_DEFINITION: &str = "http://www.omg.org/Type/Annotation/Definition";

/// `source` of an `xsd:appinfo` that applies OMG annotations (§7.3.3.1.2).
pub const APPINFO_ANNOTATION_USAGE: &str = "http://www.omg.org/Type/Annotation/Usage";

/// The namespace of the `annotate` element (§7.3.3.1.2).
pub const OMG_TYPE_NAMESPACE: &str = "http://www.omg.org/Type";

/// The namespace the schema of the IDL-to-XSD mapping puts the types in, as
/// the example of §7.3.3.1.2 writes it.
pub const IDL_MAPPED_NAMESPACE: &str = "http://www.omg.org/IDL-Mapped/";

/// The names of the built-in annotations this representation reads as
/// properties of the element they are applied to.
const BUILTIN_ANNOTATIONS: [&str; 20] = [
    "key",
    "id",
    "optional",
    "must_understand",
    "non_serialized",
    "external",
    "try_construct",
    "nested",
    "autoid",
    "final",
    "appendable",
    "mutable",
    "extensibility",
    "bit_bound",
    "value",
    "default_literal",
    "position",
    "verbatim",
    // Not annotations of the IDL representation: the labels of a union
    // branch, and the mark of an alias of a complex type.
    "case",
    "typedef",
];

/// The annotations that say which of two DDS types an XSD type stands for.
const REFINEMENTS: [&str; 5] = ["uint8", "wchar", "wstring", "char", "float128"];

/// Maps a W3C-XSD built-in type name (with or without the `xsd:`
/// prefix) onto a DDS `PrimitiveType` or, if it is a
/// user type, `None`.
fn map_builtin(local: &str) -> Option<PrimitiveType> {
    let trimmed = match local.split_once(':') {
        Some((_prefix, suffix)) => suffix,
        None => local,
    };
    Some(match trimmed {
        "boolean" => PrimitiveType::Boolean,
        "byte" => PrimitiveType::Int8,
        "unsignedByte" => PrimitiveType::Octet,
        "short" => PrimitiveType::Short,
        "unsignedShort" => PrimitiveType::UShort,
        "int" | "integer" => PrimitiveType::Long,
        "unsignedInt" => PrimitiveType::ULong,
        "long" => PrimitiveType::LongLong,
        "unsignedLong" => PrimitiveType::ULongLong,
        "float" => PrimitiveType::Float,
        "double" => PrimitiveType::Double,
        "decimal" => PrimitiveType::LongDouble,
        "string" | "normalizedString" | "token" => PrimitiveType::String,
        _ => return None,
    })
}

/// A type name as the model writes it: the prefix dropped, the `.` of
/// §7.3.3.3 (`MyModule.MyStructure`) the `::` of a scoped name.
fn model_name(raw: &str) -> String {
    let local = match raw.split_once(':') {
        Some((_p, s)) => s,
        None => raw,
    };
    local.replace('.', "::")
}

/// Resolves a `type=...` attribute into a `TypeRef`. Built-in
/// XSD types are mapped onto [`PrimitiveType`], all others
/// (user types, other namespaces) are passed through as [`TypeRef::Named`].
fn resolve_type_ref(raw: &str) -> TypeRef {
    if let Some(prim) = map_builtin(raw) {
        TypeRef::Primitive(prim)
    } else {
        TypeRef::Named(model_name(raw))
    }
}

// ---------------------------------------------------------------------
// Annotations (§7.3.3.1)
// ---------------------------------------------------------------------

/// One `annotate` of an `appinfo` with the usage `source`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Applied {
    /// The `type` attribute: the name of the OMG annotation.
    name: String,
    /// The other attributes, by name.
    values: Vec<(String, String)>,
}

impl Applied {
    fn value(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn to_annotation(&self) -> Annotation {
        Annotation {
            name: self.name.clone(),
            members: self
                .values
                .iter()
                .map(|(k, v)| AnnotationValue {
                    name: k.clone(),
                    value: Some(v.clone()),
                })
                .collect(),
        }
    }
}

/// The `appinfo` elements of the `xsd:annotation` children of `el` with the
/// given `source`.
fn appinfos<'a>(el: &'a XmlElement, source: &'a str) -> impl Iterator<Item = &'a XmlElement> + 'a {
    el.children_named("annotation")
        .flat_map(|annotation| {
            // The element of XML Schema is `appinfo`; §7.3.3.1 writes
            // `appInfo`, and both are read.
            annotation
                .children
                .iter()
                .filter(|c| c.name == "appinfo" || c.name == "appInfo")
        })
        .filter(move |info| info.attribute("source") == Some(source))
}

/// The annotations applied to the definition `el` (§7.3.3.1.2): the
/// `annotate` elements of its usage `appinfo`.
fn applied_to(el: &XmlElement) -> Result<Vec<Applied>, XmlError> {
    let mut out = Vec::new();
    for info in appinfos(el, APPINFO_ANNOTATION_USAGE) {
        for note in info.children_named("annotate") {
            let type_key = format!("{{{OMG_TYPE_NAMESPACE}}}type");
            let name = note
                .attribute(&type_key)
                .or_else(|| note.attribute("type"))
                .ok_or_else(|| {
                    XmlError::MissingRequiredElement(format!(
                        "{}: an annotate without its required `type` attribute",
                        note.path
                    ))
                })?;
            let values = note
                .attributes
                .iter()
                .filter(|(k, _)| **k != type_key && k.as_str() != "type")
                .map(|(k, v)| {
                    let local = match k.strip_prefix('{').and_then(|s| s.split_once('}')) {
                        Some((_namespace, local)) => local,
                        None => k.as_str(),
                    };
                    (local.to_string(), v.clone())
                })
                .collect();
            out.push(Applied {
                name: name.to_string(),
                values,
            });
        }
    }
    Ok(out)
}

fn is_builtin(applied: &Applied) -> bool {
    BUILTIN_ANNOTATIONS.contains(&applied.name.as_str())
        || REFINEMENTS.contains(&applied.name.as_str())
}

fn custom_annotations(applied: &[Applied]) -> Vec<Annotation> {
    applied
        .iter()
        .filter(|a| !is_builtin(a))
        .map(Applied::to_annotation)
        .collect()
}

fn flag(applied: &[Applied], name: &str) -> Result<Option<bool>, XmlError> {
    let Some(a) = applied.iter().find(|a| a.name == name) else {
        return Ok(None);
    };
    match a.value("value") {
        None | Some("TRUE" | "true") => Ok(Some(true)),
        Some("FALSE" | "false") => Ok(Some(false)),
        Some(other) => Err(XmlError::BadEnum(format!(
            "@{name}: `{other}` is no boolean"
        ))),
    }
}

fn number(applied: &[Applied], name: &str) -> Result<Option<u32>, XmlError> {
    let Some(a) = applied.iter().find(|a| a.name == name) else {
        return Ok(None);
    };
    let value = a
        .value("value")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("@{name} without its `value`")))?;
    crate::types::parse_ulong(value).map(Some)
}

/// A signed 32-bit annotation value (`@value` of an enumerator); a value
/// between 2^31 and 2^32 is the bit pattern of a negative `long`.
fn signed(applied: &[Applied], name: &str) -> Result<Option<i32>, XmlError> {
    let Some(a) = applied.iter().find(|a| a.name == name) else {
        return Ok(None);
    };
    let value = a
        .value("value")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("@{name} without its `value`")))?;
    if let Ok(v) = value.trim().parse::<i32>() {
        return Ok(Some(v));
    }
    crate::types::parse_ulong(value).map(|u| Some(u as i32))
}

fn extensibility_of(applied: &[Applied]) -> Result<Option<Extensibility>, XmlError> {
    let mut found = None;
    for a in applied {
        let kind = match a.name.as_str() {
            "final" => Extensibility::Final,
            "appendable" => Extensibility::Appendable,
            "mutable" => Extensibility::Mutable,
            "extensibility" => match a.value("value") {
                Some("FINAL") => Extensibility::Final,
                Some("APPENDABLE") => Extensibility::Appendable,
                Some("MUTABLE") => Extensibility::Mutable,
                other => {
                    return Err(XmlError::BadEnum(format!(
                        "@extensibility: {other:?} is none of FINAL, APPENDABLE, MUTABLE"
                    )));
                }
            },
            _ => continue,
        };
        if found.replace(kind).is_some() {
            return Err(XmlError::InvalidXml(String::from(
                "more than one extensibility annotation on one definition",
            )));
        }
    }
    Ok(found)
}

fn autoid_of(applied: &[Applied]) -> Result<Option<AutoId>, XmlError> {
    let Some(a) = applied.iter().find(|a| a.name == "autoid") else {
        return Ok(None);
    };
    match a.value("value") {
        Some("HASH") => Ok(Some(AutoId::Hash)),
        Some("SEQUENTIAL") => Ok(Some(AutoId::Sequential)),
        other => Err(XmlError::BadEnum(format!(
            "@autoid: {other:?} is none of HASH, SEQUENTIAL"
        ))),
    }
}

fn verbatims_of(applied: &[Applied]) -> Result<Vec<Verbatim>, XmlError> {
    applied
        .iter()
        .filter(|a| a.name == "verbatim")
        .map(|a| {
            Ok(Verbatim {
                language: a.value("language").unwrap_or("*").to_string(),
                placement: a
                    .value("placement")
                    .unwrap_or("before-declaration")
                    .to_string(),
                text: a
                    .value("text")
                    .ok_or_else(|| {
                        XmlError::MissingRequiredElement(String::from("@verbatim without `text`"))
                    })?
                    .to_string(),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------
// Shapes of members
// ---------------------------------------------------------------------

/// What an element declares, with the collections around it (§7.3.2.4.4:
/// the sequence binds tightest, then the array, then the map).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shape {
    /// A basic or named type; `string_max` bounds a string.
    Plain {
        type_ref: TypeRef,
        string_max: Option<u32>,
    },
    /// `sequence<T, bound>`; a bound of 0 is unbounded.
    Sequence(Box<Shape>, u32),
    /// One dimension of an array.
    Array(Box<Shape>, u32),
    /// `map<key, value, bound>`; `None` is unbounded.
    Map {
        key: Box<Shape>,
        value: Box<Shape>,
        bound: Option<u32>,
    },
}

fn occurs(el: &XmlElement) -> Result<(u32, Option<u32>), XmlError> {
    let min = match el.attribute("minOccurs") {
        None => 1,
        Some(v) => v
            .trim()
            .parse::<u32>()
            .map_err(|_| XmlError::ValueOutOfRange(format!("{}@minOccurs=\"{v}\"", el.path)))?,
    };
    let max =
        match el.attribute("maxOccurs") {
            None => Some(1),
            Some("unbounded") => None,
            Some(v) => Some(v.trim().parse::<u32>().map_err(|_| {
                XmlError::ValueOutOfRange(format!("{}@maxOccurs=\"{v}\"", el.path))
            })?),
        };
    Ok((min, max))
}

/// The collection layer an occurrence range stands for: an exact count is
/// an array, anything else a sequence bounded by `maxOccurs`.
fn layer(inner: Shape, min: u32, max: Option<u32>) -> Shape {
    match max {
        Some(n) if n == min && n >= 1 => Shape::Array(Box::new(inner), n),
        Some(n) => Shape::Sequence(Box::new(inner), n),
        None => Shape::Sequence(Box::new(inner), 0),
    }
}

/// The restriction of an inline `xsd:simpleType`: its base and `maxLength`.
fn inline_simple(el: &XmlElement) -> Result<Option<Shape>, XmlError> {
    let Some(simple) = el.child("simpleType") else {
        return Ok(None);
    };
    let Some(restriction) = simple.child("restriction") else {
        return Err(XmlError::InvalidXml(format!(
            "{}: an inline simpleType without a restriction",
            simple.path
        )));
    };
    let base = restriction.attribute("base").unwrap_or("xsd:string");
    let max_length = match restriction.child("maxLength") {
        Some(m) => Some(
            m.attribute("value")
                .and_then(|v| v.trim().parse::<u32>().ok())
                .ok_or_else(|| {
                    XmlError::ValueOutOfRange(format!("{}: maxLength needs a number", m.path))
                })?,
        ),
        None => None,
    };
    Ok(Some(Shape::Plain {
        type_ref: resolve_type_ref(base),
        string_max: max_length,
    }))
}

/// Reads what the element `el` declares. The occurrences of `el` itself are
/// not part of the result (an optional member is `minOccurs="0"`), except in
/// the flattened form of an element that carries its own `maxOccurs`.
///
/// zerodds-lint: recursion-depth = nesting of inline complexTypes, capped by
/// `parser::MAX_TREE_DEPTH` = 64 when the tree is built.
fn shape_of(el: &XmlElement) -> Result<Shape, XmlError> {
    let (min, max) = occurs(el)?;
    let own = if let Some(raw) = el.attribute("type") {
        Shape::Plain {
            type_ref: resolve_type_ref(raw),
            string_max: None,
        }
    } else if let Some(simple) = inline_simple(el)? {
        simple
    } else if let Some(complex) = el.child("complexType") {
        wrapper_shape(complex)?
    } else {
        // An element without a type is `xsd:anyType`; the earlier reader
        // read it as a string.
        Shape::Plain {
            type_ref: TypeRef::Primitive(PrimitiveType::String),
            string_max: None,
        }
    };
    // The flattened form: the element is repeated within its sequence.
    // Inside a wrapper the occurrences of the child are the layer; here, on
    // a member, `maxOccurs` above 1 (or unbounded) is the layer.
    match max {
        Some(1) => Ok(own),
        _ => Ok(layer(own, min, max)),
    }
}

/// zerodds-lint: recursion-depth 64
///
/// An inline or named `complexType` that is not a structure: a sequence or
/// array wrapper (one child `element`), or a map (§7.3.3.4). It recurses
/// through [`shape_of_without_own_layer`] once per nested collection, each
/// nesting being an open element of the document, so the depth is at most
/// `MAX_TREE_DEPTH` of the parser.
fn wrapper_shape(complex: &XmlElement) -> Result<Shape, XmlError> {
    let Some(sequence) = complex.child("sequence") else {
        return Err(XmlError::InvalidXml(format!(
            "{}: a complexType without a sequence is no collection or map",
            complex.path
        )));
    };
    let (_, sequence_max) = occurs(sequence)?;
    let elements: Vec<&XmlElement> = sequence.children_named("element").collect();
    let names: Vec<&str> = elements
        .iter()
        .map(|e| e.attribute("name").unwrap_or(""))
        .collect();
    if names == ["key", "value"] {
        return Ok(Shape::Map {
            key: Box::new(shape_of(elements[0])?),
            value: Box::new(shape_of(elements[1])?),
            bound: sequence_max,
        });
    }
    if let [only] = elements.as_slice() {
        if only.attribute("name").is_some_and(is_item_name) {
            // The layer is the occurrence of the child.
            let (min, max) = occurs(only)?;
            let inner = shape_of_without_own_layer(only)?;
            return Ok(layer(inner, min, max));
        }
    }
    Err(XmlError::InvalidXml(format!(
        "{}: a complexType that is neither a collection (one `element`) nor a map \
         (`key` and `value`) has no place here",
        complex.path
    )))
}

/// zerodds-lint: recursion-depth 64
///
/// As [`shape_of`], leaving the occurrences of `el` to the caller. The depth
/// is bounded as that of [`wrapper_shape`].
fn shape_of_without_own_layer(el: &XmlElement) -> Result<Shape, XmlError> {
    if let Some(raw) = el.attribute("type") {
        return Ok(Shape::Plain {
            type_ref: resolve_type_ref(raw),
            string_max: None,
        });
    }
    if let Some(simple) = inline_simple(el)? {
        return Ok(simple);
    }
    if let Some(complex) = el.child("complexType") {
        return wrapper_shape(complex);
    }
    Ok(Shape::Plain {
        type_ref: TypeRef::Primitive(PrimitiveType::String),
        string_max: None,
    })
}

/// The properties of a [`StructMember`] a [`Shape`] sets.
struct Flattened {
    type_ref: TypeRef,
    string_max_length: Option<u32>,
    sequence_max_length: Option<u32>,
    array_dimensions: Vec<u32>,
    map_key: Option<TypeRef>,
    map_max_length: Option<u32>,
    map_key_string_max_length: Option<u32>,
}

/// Folds a shape into the attributes of a member: the sequence binds
/// tightest, then the array dimensions, then the map (§7.3.2.4.4); a shape
/// in another order needs an alias between its collections.
fn flatten(shape: Shape, at: &str) -> Result<Flattened, XmlError> {
    let mut out = Flattened {
        type_ref: TypeRef::Primitive(PrimitiveType::Long),
        string_max_length: None,
        sequence_max_length: None,
        array_dimensions: Vec::new(),
        map_key: None,
        map_max_length: None,
        map_key_string_max_length: None,
    };
    let mut current = shape;
    if let Shape::Map { key, value, bound } = current {
        let Shape::Plain {
            type_ref,
            string_max,
        } = *key
        else {
            return Err(XmlError::InvalidXml(format!(
                "{at}: the key of a map is a basic or a named type"
            )));
        };
        out.map_key = Some(type_ref);
        out.map_key_string_max_length = string_max;
        out.map_max_length = bound;
        current = *value;
    }
    while let Shape::Array(inner, n) = current {
        out.array_dimensions.push(n);
        current = *inner;
    }
    // The mapping nests the dimensions the other way round: the first
    // dimension of the model is the innermost type (IDL-to-XSD mapping
    // 4.1.7.6, `long matrix[5][3]`).
    out.array_dimensions.reverse();
    if let Shape::Sequence(inner, bound) = current {
        out.sequence_max_length = Some(bound);
        current = *inner;
    }
    match current {
        Shape::Plain {
            type_ref,
            string_max,
        } => {
            out.type_ref = type_ref;
            out.string_max_length = string_max;
            Ok(out)
        }
        _ => Err(XmlError::InvalidXml(format!(
            "{at}: the collections nest in an order the type system binds only through an \
             alias (sequence inside array inside map, §7.3.2.4.4)"
        ))),
    }
}

/// `string`, `wstring`, `char`, `wchar`, `uint8` and `float128` share an
/// XSD type with another DDS type; the annotation of the same name says
/// which one is meant.
fn refine(type_ref: TypeRef, applied: &[Applied]) -> TypeRef {
    let TypeRef::Primitive(primitive) = type_ref else {
        return type_ref;
    };
    let has = |name: &str| applied.iter().any(|a| a.name == name);
    TypeRef::Primitive(match primitive {
        PrimitiveType::Octet if has("uint8") => PrimitiveType::UInt8,
        PrimitiveType::String if has("wstring") => PrimitiveType::WString,
        PrimitiveType::String if has("wchar") => PrimitiveType::WChar,
        PrimitiveType::String if has("char") => PrimitiveType::Char,
        other => other,
    })
}

// ---------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------

/// A type found in the schema, with the module path its dotted name puts it
/// in.
struct Found {
    path: Vec<String>,
    def: TypeDef,
}

fn split_name(raw: &str) -> (Vec<String>, String) {
    let mut parts: Vec<String> = raw.split('.').map(ToString::to_string).collect();
    let name = parts.pop().unwrap_or_default();
    (parts, name)
}

/// Parses an XSD schema document and returns a [`TypeLibrary`].
///
/// # Errors
/// * [`XmlError::InvalidXml`] — not well-formed XML, missing
///   `<xsd:schema>` root, or syntactic errors in the XSD structure.
pub fn parse_xsd_schema(xml: &str) -> Result<TypeLibrary, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "schema" {
        return Err(XmlError::InvalidXml(format!(
            "expected <xsd:schema> root, got <{}>",
            doc.root.name
        )));
    }

    let mut found: Vec<Found> = Vec::new();
    // §7.3.3.3: the names of the root elements, that is, of the types that
    // are not nested.
    let mut roots: BTreeSet<String> = BTreeSet::new();
    for child in &doc.root.children {
        match child.name.as_str() {
            "complexType" => {
                let name = child.attribute("name").unwrap_or_default();
                // The SOAP-encoding twin of a collection or of a type that
                // has one (IDL-to-XSD mapping 4.1.7.5): the same type in
                // the form of `soapenc:Array`, which this reader leaves to
                // the WS-I form beside it.
                if is_soap_encoding_variant(child, name) {
                    continue;
                }
                let (path, simple_name) = split_name(name);
                let def = complex_type(child, &simple_name)?;
                found.push(Found { path, def });
            }
            "simpleType" => {
                let name = child.attribute("name").unwrap_or_default();
                let (path, simple_name) = split_name(name);
                if let Some(def) = simple_type(child, &simple_name)? {
                    found.push(Found { path, def });
                }
            }
            "element" => {
                if let Some(raw) = child.attribute("type") {
                    // §7.3.3.3: "an XML element of that type suitable for
                    // use as a document root. The name of this element shall
                    // be the fully qualified name of T."
                    roots.insert(model_name(raw));
                } else if let Some(ct) = child.child("complexType") {
                    // A top-level element with an anonymous complexType is
                    // a structure of the element's name.
                    let mut s = complex_type(ct, "")?;
                    if let TypeDef::Struct(st) = &mut s {
                        if st.name.is_empty() {
                            st.name = child.attribute("name").unwrap_or_default().to_string();
                        }
                        if !st.name.is_empty() {
                            found.push(Found {
                                path: Vec::new(),
                                def: s,
                            });
                        }
                    }
                }
            }
            "include" | "import" | "annotation" | "notation" | "group" | "attributeGroup" => {
                // Not in scope; silently ignore (XSD includes are
                // purely documentary annotations).
            }
            _ => {
                // Unknown top-level element — strictly rejecting would be
                // too harsh, so lax.
            }
        }
    }
    inline_array_types(&mut found);
    let mut lib = TypeLibrary::default();
    for Found { path, def } in found {
        let fq = if path.is_empty() {
            def.name().to_string()
        } else {
            format!("{}::{}", path.join("::"), def.name())
        };
        let mut def = def;
        if roots.contains(&fq) {
            match &mut def {
                TypeDef::Struct(s) => {
                    if s.nested == Some(true) {
                        return Err(XmlError::InvalidXml(format!(
                            "`{fq}` is annotated nested and has a root element (§7.3.3.3)"
                        )));
                    }
                }
                TypeDef::Union(u) => {
                    if u.nested == Some(true) {
                        return Err(XmlError::InvalidXml(format!(
                            "`{fq}` is annotated nested and has a root element (§7.3.3.3)"
                        )));
                    }
                }
                _ => {}
            }
        }
        place(&mut lib.types, &path, def);
    }
    Ok(lib)
}

/// Puts `def` into the module `path` of `types`, making the modules.
///
/// zerodds-lint: recursion-depth = length of the dotted name, which is
/// bounded by the name's length in the document.
fn place(types: &mut Vec<TypeDef>, path: &[String], def: TypeDef) {
    let Some((first, rest)) = path.split_first() else {
        types.push(def);
        return;
    };
    let existing = types
        .iter()
        .position(|t| matches!(t, TypeDef::Module(m) if &m.name == first));
    let index = existing.unwrap_or_else(|| {
        types.push(TypeDef::Module(ModuleEntry {
            name: first.clone(),
            types: Vec::new(),
            auto_id: None,
        }));
        types.len() - 1
    });
    if let TypeDef::Module(module) = &mut types[index] {
        place(&mut module.types, rest, def);
    }
}

/// Whether a `complexType` is the SOAP-encoding twin of another type: a
/// restriction of `soapenc:Array`, or a type with the prefix `_SE_` the
/// mapping gives to those (4.1.7.5).
fn is_soap_encoding_variant(node: &XmlElement, name: &str) -> bool {
    if name.starts_with("_SE_") || name.contains("._SE_") {
        return true;
    }
    node.child("complexContent")
        .and_then(|cc| cc.child("restriction"))
        .and_then(|r| r.attribute("base"))
        .is_some_and(|base| base.ends_with(":Array") && base.contains("soapenc"))
}

/// The types the mapping writes for the arrays of a declaration and that are
/// no declarations of their own: the type of an array declared as a field,
/// `<enclosing>.<field>_ArrayOf<element>`, and the intermediate types of an
/// array of more than one dimension, `ArrayOf<element>` (4.1.7.6). Each is
/// folded into what refers to it, which gets the dimensions and the
/// element type of the array type; the type itself goes unless something
/// else refers to it.
fn inline_array_types(found: &mut Vec<Found>) {
    let fq_of = |f: &Found| -> String {
        if f.path.is_empty() {
            f.def.name().to_string()
        } else {
            format!("{}::{}", f.path.join("::"), f.def.name())
        }
    };
    let synthetic = |f: &Found| -> bool {
        let local = f.def.name();
        match &f.def {
            TypeDef::Typedef(t) => {
                t.map_key.is_none()
                    && !t.array_dimensions.is_empty()
                    && ((f.path.is_empty() && local.starts_with("ArrayOf"))
                        || (!f.path.is_empty() && local.contains("_ArrayOf")))
            }
            _ => false,
        }
    };
    let mut inlined: BTreeSet<String> = BTreeSet::new();
    // Each pass folds the array types into their users; a type used by
    // another array type is folded in the next pass.
    for _ in 0..crate::parser::MAX_TREE_DEPTH {
        let candidates: alloc::collections::BTreeMap<String, TypedefType> = found
            .iter()
            .filter(|f| synthetic(f))
            .filter_map(|f| match &f.def {
                TypeDef::Typedef(t) => Some((fq_of(f), t.clone())),
                _ => None,
            })
            .collect();
        if candidates.is_empty() {
            break;
        }
        let mut changed = false;
        let owners: Vec<String> = found.iter().map(&fq_of).collect();
        for (index, f) in found.iter_mut().enumerate() {
            let owner = owners[index].clone();
            match &mut f.def {
                TypeDef::Struct(st) => {
                    for m in &mut st.members {
                        let implicit = format!("{owner}::{}_ArrayOf", m.name);
                        changed |= fold_member(m, &candidates, &mut inlined, Some(&implicit));
                    }
                }
                TypeDef::Union(un) => {
                    for case in &mut un.cases {
                        let implicit = format!("{owner}::{}_ArrayOf", case.member.name);
                        changed |= fold_member(
                            &mut case.member,
                            &candidates,
                            &mut inlined,
                            Some(&implicit),
                        );
                    }
                }
                TypeDef::Typedef(t) => {
                    let TypeRef::Named(n) = &t.type_ref else {
                        continue;
                    };
                    let Some(target) = candidates.get(n) else {
                        continue;
                    };
                    // Only an array level refers to an intermediate type.
                    if t.array_dimensions.is_empty() || !is_intermediate(n) {
                        continue;
                    }
                    inlined.insert(n.clone());
                    t.type_ref = target.type_ref.clone();
                    t.string_max_length = target.string_max_length;
                    if target.sequence_max_length.is_some() {
                        t.sequence_max_length = target.sequence_max_length;
                    }
                    let mut dims = target.array_dimensions.clone();
                    dims.extend(t.array_dimensions.iter().copied());
                    t.array_dimensions = dims;
                    changed = true;
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
    // What was folded in goes, unless something still refers to it.
    let still_used: BTreeSet<String> = found.iter().flat_map(named_references).collect();
    found.retain(|f| {
        let fq = fq_of(f);
        !(inlined.contains(&fq) && synthetic(f) && !still_used.contains(&fq))
    });
}

/// An intermediate array type: `ArrayOf<element>` without a module.
fn is_intermediate(fq: &str) -> bool {
    !fq.contains("::") && fq.starts_with("ArrayOf")
}

/// Folds the array type a member names into the member, when it is the type
/// written for that very member (`implicit` is the start of its name) or an
/// intermediate type under an array level.
fn fold_member(
    m: &mut StructMember,
    candidates: &alloc::collections::BTreeMap<String, TypedefType>,
    inlined: &mut BTreeSet<String>,
    implicit: Option<&str>,
) -> bool {
    let TypeRef::Named(n) = &m.type_ref else {
        return false;
    };
    let Some(target) = candidates.get(n) else {
        return false;
    };
    let is_implicit = implicit.is_some_and(|prefix| n.starts_with(prefix));
    let is_level = !m.array_dimensions.is_empty() && is_intermediate(n);
    if !(is_implicit || is_level) {
        return false;
    }
    inlined.insert(n.clone());
    m.type_ref = target.type_ref.clone();
    m.string_max_length = target.string_max_length;
    if target.sequence_max_length.is_some() {
        m.sequence_max_length = target.sequence_max_length;
    }
    let mut dims = target.array_dimensions.clone();
    dims.extend(m.array_dimensions.iter().copied());
    m.array_dimensions = dims;
    true
}

/// The names of the types a declaration refers to.
fn named_references(f: &Found) -> Vec<String> {
    let named = |t: &TypeRef| match t {
        TypeRef::Named(n) => Some(n.clone()),
        TypeRef::Primitive(_) => None,
    };
    match &f.def {
        TypeDef::Struct(st) => st
            .members
            .iter()
            .filter_map(|m| named(&m.type_ref))
            .chain(st.base_type.clone())
            .collect(),
        TypeDef::Union(un) => un
            .cases
            .iter()
            .filter_map(|c| named(&c.member.type_ref))
            .collect(),
        TypeDef::Typedef(t) => named(&t.type_ref).into_iter().collect(),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------
// complexType
// ---------------------------------------------------------------------

fn complex_type(node: &XmlElement, name: &str) -> Result<TypeDef, XmlError> {
    let applied = applied_to(node)?;
    // §7.3.3.1.1: an OMG annotation type.
    if appinfos(node, APPINFO_ANNOTATION_DEFINITION)
        .next()
        .is_some()
    {
        return annotation_definition(node, name).map(TypeDef::Annotation);
    }
    // A collection or a map: an alias.
    if node.child("complexContent").is_none() && is_collection_wrapper(node) {
        return typedef_of_wrapper(node, name).map(TypeDef::Typedef);
    }
    if is_union(node) {
        return union_type(node, name, &applied).map(TypeDef::Union);
    }
    // An alias of a complex type: a restriction of it that states its
    // content again (IDL-to-XSD mapping 4.1.7.3).
    if let Some(restriction) = node
        .child("complexContent")
        .and_then(|cc| cc.child("restriction"))
    {
        let base = restriction.attribute("base").ok_or_else(|| {
            XmlError::MissingRequiredElement(format!(
                "{}: a restriction without its base",
                restriction.path
            ))
        })?;
        return Ok(TypeDef::Typedef(TypedefType {
            name: name.to_string(),
            type_ref: resolve_type_ref(base),
            ..TypedefType::default()
        }));
    }
    // An alias of a complex type, marked by `typedef` (the form an earlier
    // version wrote, read as before).
    if let Some(ext) = node
        .child("complexContent")
        .and_then(|cc| cc.child("extension"))
    {
        if applied.iter().any(|a| a.name == "typedef") {
            let base = ext.attribute("base").unwrap_or_default();
            return Ok(TypeDef::Typedef(TypedefType {
                name: name.to_string(),
                type_ref: resolve_type_ref(base),
                ..TypedefType::default()
            }));
        }
    }
    struct_type(node, name, &applied).map(TypeDef::Struct)
}

/// A named complexType whose `sequence` holds the one element `element`, or
/// the `key` and `value` of a map (§7.3.3.4).
fn is_collection_wrapper(node: &XmlElement) -> bool {
    let Some(sequence) = node.child("sequence") else {
        return false;
    };
    let names: Vec<&str> = sequence
        .children_named("element")
        .map(|e| e.attribute("name").unwrap_or(""))
        .collect();
    names == ["key", "value"] || (names.len() == 1 && is_item_name(names[0]))
}

/// Whether `name` is the name of the one element of a collection type: the
/// `item` of the IDL-to-XSD mapping (4.1.7.5), `item1`, `item2`, ... of the
/// further dimensions of an array (4.1.7.6), or the `element` DDS-XML 1.0
/// wrote.
pub(crate) fn is_item_name(name: &str) -> bool {
    name == "element"
        || name
            .strip_prefix("item")
            .is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_digit()))
}

fn is_union(node: &XmlElement) -> bool {
    let Some(sequence) = node.child("sequence") else {
        return false;
    };
    sequence.children.first().is_some_and(|first| {
        first.name == "element" && first.attribute("name") == Some("discriminator")
    }) && sequence.child("choice").is_some()
}

fn typedef_of_wrapper(node: &XmlElement, name: &str) -> Result<TypedefType, XmlError> {
    let shape = wrapper_shape(node)?;
    let f = flatten(shape, &node.path)?;
    Ok(TypedefType {
        name: name.to_string(),
        type_ref: f.type_ref,
        array_dimensions: f.array_dimensions,
        sequence_max_length: f.sequence_max_length,
        string_max_length: f.string_max_length,
        map_max_length: f.map_max_length,
        map_key: f.map_key,
        map_key_string_max_length: f.map_key_string_max_length,
        external: None,
    })
}

fn annotation_definition(node: &XmlElement, name: &str) -> Result<AnnotationTypeDef, XmlError> {
    let mut members = Vec::new();
    for attribute in node.children_named("attribute") {
        let member_name = attribute.attribute("name").ok_or_else(|| {
            XmlError::MissingRequiredElement(format!(
                "{}: an attribute without a name",
                attribute.path
            ))
        })?;
        let raw = attribute.attribute("type").unwrap_or("xsd:string");
        members.push(AnnotationMemberDecl {
            name: member_name.to_string(),
            type_ref: resolve_type_ref(raw),
            default_value: attribute.attribute("default").map(ToString::to_string),
        });
    }
    Ok(AnnotationTypeDef {
        name: name.to_string(),
        base_type: None,
        members,
    })
}

fn struct_type(node: &XmlElement, name: &str, applied: &[Applied]) -> Result<StructType, XmlError> {
    let mut members = Vec::new();
    // `<xsd:sequence>` is the normalized member order. The spec
    // also allows `<xsd:all>` (unordered) — we treat it identically.
    if let Some(seq) = node.child("sequence").or_else(|| node.child("all")) {
        for el in seq.children_named("element") {
            members.push(member_of(el)?);
        }
    }
    // `<xsd:complexContent><xsd:extension base="..."><xsd:sequence>...`
    // -> Inheritance (§7.3.3.2.1).
    let mut base_type: Option<String> = None;
    if let Some(cc) = node.child("complexContent") {
        if let Some(ext) = cc.child("extension") {
            if let Some(b) = ext.attribute("base") {
                base_type = Some(model_name(b));
            }
            if let Some(seq) = ext.child("sequence").or_else(|| ext.child("all")) {
                for el in seq.children_named("element") {
                    members.push(member_of(el)?);
                }
            }
        }
    }
    Ok(StructType {
        name: name.to_string(),
        extensibility: extensibility_of(applied)?,
        base_type,
        members,
        nested: flag(applied, "nested")?,
        auto_id: autoid_of(applied)?,
        annotations: custom_annotations(applied),
        verbatims: verbatims_of(applied)?,
        constants: Vec::new(),
    })
}

/// Parses an `<xsd:element name="..." type="..." minOccurs="..."
/// maxOccurs="...">` as a struct member.
fn member_of(node: &XmlElement) -> Result<StructMember, XmlError> {
    let name = node.attribute("name").unwrap_or_default().to_string();
    let applied = applied_to(node)?;
    let (min_occurs, max_occurs) = occurs(node)?;
    let flat = flatten(shape_of(node)?, &node.path)?;
    let optional =
        (min_occurs == 0 && max_occurs == Some(1)) || flag(&applied, "optional")?.unwrap_or(false);
    let try_construct = match applied.iter().find(|a| a.name == "try_construct") {
        None => None,
        Some(a) => Some(match a.value("value") {
            Some("DISCARD") => TryConstruct::Discard,
            Some("USE_DEFAULT") => TryConstruct::UseDefault,
            Some("TRIM") => TryConstruct::Trim,
            other => {
                return Err(XmlError::BadEnum(format!(
                    "@try_construct: {other:?} is none of DISCARD, USE_DEFAULT, TRIM"
                )));
            }
        }),
    };
    Ok(StructMember {
        name,
        type_ref: refine(flat.type_ref, &applied),
        key: flag(&applied, "key")?.unwrap_or(false),
        optional,
        must_understand: flag(&applied, "must_understand")?.unwrap_or(false),
        id: number(&applied, "id")?,
        string_max_length: flat.string_max_length,
        sequence_max_length: flat.sequence_max_length,
        map_max_length: flat.map_max_length,
        map_key: flat.map_key,
        map_key_string_max_length: flat.map_key_string_max_length,
        array_dimensions: flat.array_dimensions,
        external: flag(&applied, "external")?,
        try_construct,
        non_serialized: flag(&applied, "non_serialized")?,
        annotations: custom_annotations(&applied),
    })
}

/// A union: `<sequence><element name="discriminator"/><choice>...</choice>`.
/// Each member of the choice is a branch; its `case` annotation lists the
/// labels (`default` for the default branch).
fn union_type(node: &XmlElement, name: &str, applied: &[Applied]) -> Result<UnionType, XmlError> {
    let sequence = node.child("sequence").ok_or_else(|| {
        XmlError::InvalidXml(format!("{}: a union without a sequence", node.path))
    })?;
    let disc_el = sequence
        .children_named("element")
        .next()
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("{}: discriminator", node.path)))?;
    let disc_applied = applied_to(disc_el)?;
    let flat = flatten(shape_of(disc_el)?, &disc_el.path)?;
    let choice = sequence.child("choice").ok_or_else(|| {
        XmlError::MissingRequiredElement(format!("{}: the choice of the branches", node.path))
    })?;
    let mut cases = Vec::new();
    for branch in choice.children_named("element") {
        let branch_applied = applied_to(branch)?;
        let labels = branch_applied
            .iter()
            .find(|a| a.name == "case")
            .and_then(|a| a.value("value"))
            .ok_or_else(|| {
                XmlError::MissingRequiredElement(format!(
                    "{}: a union branch without its `case` annotation",
                    branch.path
                ))
            })?;
        let discriminators = labels
            .split(',')
            .map(|label| {
                let label = label.trim();
                if label == "default" {
                    UnionDiscriminator::Default
                } else {
                    UnionDiscriminator::Value(label.to_string())
                }
            })
            .collect();
        cases.push(UnionCase {
            discriminators,
            member: member_of(branch)?,
        });
    }
    Ok(UnionType {
        name: name.to_string(),
        discriminator: refine(flat.type_ref, &disc_applied),
        cases,
        extensibility: extensibility_of(applied)?,
        nested: flag(applied, "nested")?,
        auto_id: autoid_of(applied)?,
        discriminator_key: flag(&disc_applied, "key")?.unwrap_or(false),
        discriminator_annotations: custom_annotations(&disc_applied),
        annotations: custom_annotations(applied),
        verbatims: verbatims_of(applied)?,
    })
}

// ---------------------------------------------------------------------
// simpleType
// ---------------------------------------------------------------------

/// Parses `<xsd:simpleType name="X">`: a restriction of `xsd:string` with
/// enumerations is an enumeration; a list of such a restriction is a
/// bitmask; any other restriction is an alias of its base. A simpleType
/// without a restriction or list is left out.
fn simple_type(node: &XmlElement, name: &str) -> Result<Option<TypeDef>, XmlError> {
    let applied = applied_to(node)?;
    if let Some(list) = node.child("list") {
        return bitmask(node, list, name, &applied).map(|b| Some(TypeDef::Bitmask(b)));
    }
    let Some(restriction) = node.child("restriction") else {
        return Ok(None);
    };
    let enumerations: Vec<&XmlElement> = restriction.children_named("enumeration").collect();
    if !enumerations.is_empty() {
        return enumeration(&enumerations, name, &applied).map(|e| Some(TypeDef::Enum(e)));
    }
    // An alias of a simple type: `<restriction base="xsd:int"/>`,
    // optionally with a `maxLength`.
    let base = restriction.attribute("base").unwrap_or("xsd:string");
    let max_length = restriction
        .child("maxLength")
        .and_then(|m| m.attribute("value"))
        .and_then(|v| v.trim().parse::<u32>().ok());
    Ok(Some(TypeDef::Typedef(TypedefType {
        name: name.to_string(),
        type_ref: refine(resolve_type_ref(base), &applied),
        string_max_length: max_length,
        ..TypedefType::default()
    })))
}

fn enumeration(
    enumerations: &[&XmlElement],
    name: &str,
    applied: &[Applied],
) -> Result<EnumType, XmlError> {
    let mut enumerators: Vec<EnumLiteral> = Vec::new();
    for en in enumerations {
        if let Some(value) = en.attribute("value") {
            let literal_applied = applied_to(en)?;
            enumerators.push(EnumLiteral {
                name: value.to_string(),
                value: signed(&literal_applied, "value")?,
                default_literal: flag(&literal_applied, "default_literal")?,
                annotations: custom_annotations(&literal_applied),
            });
        }
    }
    Ok(EnumType {
        name: name.to_string(),
        bit_bound: number(applied, "bit_bound")?,
        extensibility: extensibility_of(applied)?,
        enumerators,
        annotations: custom_annotations(applied),
        verbatims: verbatims_of(applied)?,
    })
}

/// A bitmask is a list of flag names (the sample of a bitmask is the names
/// of the set flags, separated by blanks, §7.4.4).
fn bitmask(
    node: &XmlElement,
    list: &XmlElement,
    name: &str,
    applied: &[Applied],
) -> Result<BitmaskType, XmlError> {
    let restriction = list
        .child("simpleType")
        .and_then(|s| s.child("restriction"))
        .ok_or_else(|| {
            XmlError::InvalidXml(format!(
                "{}: a bitmask is a list of a restriction with the flags as enumerations",
                node.path
            ))
        })?;
    let mut bit_values = Vec::new();
    for en in restriction.children_named("enumeration") {
        let flag_name = en.attribute("value").ok_or_else(|| {
            XmlError::MissingRequiredElement(format!("{}: an enumeration without a value", en.path))
        })?;
        let flag_applied = applied_to(en)?;
        bit_values.push(BitValue {
            name: flag_name.to_string(),
            position: number(&flag_applied, "position")?,
            annotations: custom_annotations(&flag_applied),
        });
    }
    Ok(BitmaskType {
        name: name.to_string(),
        bit_bound: number(applied, "bit_bound")?,
        bit_values,
        annotations: custom_annotations(applied),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn lib_of(xml: &str) -> TypeLibrary {
        parse_xsd_schema(xml).expect("parse")
    }

    #[test]
    fn empty_schema_yields_empty_library() {
        let xml = r#"<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema"/>"#;
        let lib = lib_of(xml);
        assert!(lib.types.is_empty());
    }

    #[test]
    fn complex_type_maps_to_struct_with_primitive_members() {
        // Spec §7.3.3 — XSD complexType + xsd:sequence → DDS struct.
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Point">
                    <xsd:sequence>
                        <xsd:element name="x" type="xsd:int"/>
                        <xsd:element name="y" type="xsd:int"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        assert_eq!(lib.types.len(), 1);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!("expected struct, got {:?}", lib.types[0]);
        };
        assert_eq!(s.name, "Point");
        assert_eq!(s.members.len(), 2);
        assert_eq!(s.members[0].name, "x");
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::Long)
        );
        assert_eq!(s.members[1].name, "y");
    }

    #[test]
    fn xsd_long_maps_to_dds_longlong() {
        // Spec §7.3.3 — xsd:long is 64-bit → DDS longlong (§7.4.13).
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Big">
                    <xsd:sequence>
                        <xsd:element name="v" type="xsd:long"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::LongLong)
        );
    }

    #[test]
    fn unsigned_xsd_types_map_to_unsigned_dds_primitives() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="U">
                    <xsd:sequence>
                        <xsd:element name="a" type="xsd:unsignedByte"/>
                        <xsd:element name="b" type="xsd:unsignedShort"/>
                        <xsd:element name="c" type="xsd:unsignedInt"/>
                        <xsd:element name="d" type="xsd:unsignedLong"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::Octet)
        );
        assert_eq!(
            s.members[1].type_ref,
            TypeRef::Primitive(PrimitiveType::UShort)
        );
        assert_eq!(
            s.members[2].type_ref,
            TypeRef::Primitive(PrimitiveType::ULong)
        );
        assert_eq!(
            s.members[3].type_ref,
            TypeRef::Primitive(PrimitiveType::ULongLong)
        );
    }

    #[test]
    fn min_occurs_zero_yields_optional_member() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Opt">
                    <xsd:sequence>
                        <xsd:element name="maybe" type="xsd:int" minOccurs="0"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert!(s.members[0].optional);
    }

    #[test]
    fn max_occurs_unbounded_yields_sequence() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="WithSeq">
                    <xsd:sequence>
                        <xsd:element name="items" type="xsd:int" maxOccurs="unbounded"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert!(s.members[0].sequence_max_length.is_some());
    }

    #[test]
    fn max_occurs_bounded_yields_bounded_sequence() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="WithBoundedSeq">
                    <xsd:sequence>
                        <xsd:element name="items" type="xsd:int" maxOccurs="5"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert_eq!(s.members[0].sequence_max_length, Some(5));
    }

    #[test]
    fn simple_type_with_enumeration_maps_to_dds_enum() {
        // Spec §7.3.3 — XSD restriction with enumeration → DDS enum.
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:simpleType name="Color">
                    <xsd:restriction base="xsd:string">
                        <xsd:enumeration value="RED"/>
                        <xsd:enumeration value="GREEN"/>
                        <xsd:enumeration value="BLUE"/>
                    </xsd:restriction>
                </xsd:simpleType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Enum(e) = &lib.types[0] else {
            panic!("expected enum, got {:?}", lib.types[0]);
        };
        assert_eq!(e.name, "Color");
        assert_eq!(e.enumerators.len(), 3);
        assert_eq!(e.enumerators[0].name, "RED");
        assert_eq!(e.enumerators[1].name, "GREEN");
        assert_eq!(e.enumerators[2].name, "BLUE");
    }

    #[test]
    fn user_type_reference_yields_named_typeref() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Origin">
                    <xsd:sequence>
                        <xsd:element name="lat" type="xsd:double"/>
                    </xsd:sequence>
                </xsd:complexType>
                <xsd:complexType name="Trip">
                    <xsd:sequence>
                        <xsd:element name="from" type="Origin"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        assert_eq!(lib.types.len(), 2);
        let TypeDef::Struct(trip) = &lib.types[1] else {
            panic!()
        };
        assert_eq!(
            trip.members[0].type_ref,
            TypeRef::Named(String::from("Origin"))
        );
    }

    #[test]
    fn complex_content_extension_yields_inheritance() {
        // Spec — `<xsd:complexContent><xsd:extension base="P">` is
        // XSD inheritance, which is mapped onto DDS struct inheritance.
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Parent">
                    <xsd:sequence>
                        <xsd:element name="a" type="xsd:int"/>
                    </xsd:sequence>
                </xsd:complexType>
                <xsd:complexType name="Child">
                    <xsd:complexContent>
                        <xsd:extension base="Parent">
                            <xsd:sequence>
                                <xsd:element name="b" type="xsd:int"/>
                            </xsd:sequence>
                        </xsd:extension>
                    </xsd:complexContent>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(child) = &lib.types[1] else {
            panic!()
        };
        assert_eq!(child.name, "Child");
        assert_eq!(child.base_type.as_deref(), Some("Parent"));
        assert_eq!(child.members.len(), 1);
        assert_eq!(child.members[0].name, "b");
    }

    #[test]
    fn non_schema_root_is_rejected() {
        let xml = r#"<dds><types/></dds>"#;
        let r = parse_xsd_schema(xml);
        assert!(r.is_err(), "non-<xsd:schema> root must be rejected");
    }

    #[test]
    fn xsd_to_typeobject_via_bridge_produces_minimal_struct() {
        // End-to-End: XSD-Source → TypeLibrary →
        // typeobject_bridge::bridge_library → MinimalTypeObject.
        // This fully covers the spec path XTypes 1.3 §7.3.3
        // (XSD → TypeObject in wire form).
        use crate::typeobject_bridge::bridge_library;

        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Sensor">
                    <xsd:sequence>
                        <xsd:element name="id" type="xsd:int"/>
                        <xsd:element name="name" type="xsd:string"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = parse_xsd_schema(xml).expect("xsd-parse");
        let map = bridge_library(&lib).expect("bridge");
        assert!(
            map.contains_key("Sensor"),
            "TypeObject for Sensor missing: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }
}
