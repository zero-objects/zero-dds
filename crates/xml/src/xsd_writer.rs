// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A [`TypeLibrary`] as an XSD document (XTypes 1.3 §7.3.3): the inverse of
//! [`crate::xsd_schema::parse_xsd_schema`].
//!
//! §7.3.3: "An XSD Representation of a given type shall be as if the
//! OMG-standard IDL mapping to XSD were applied to the IDL Representation of
//! the type", augmented by §7.3.3.1 to §7.3.3.4:
//!
//! - **Names** are the fully qualified names with `.` for `::`
//!   (`MyModule.MyStructure`, §7.3.3.3); the schema has the target namespace
//!   [`IDL_MAPPED_NAMESPACE`] and references types with the prefix `tns`.
//! - **A structure** is a `complexType` with a `sequence` of its members; the
//!   subtype extends its base with `complexContent` (§7.3.3.2.1); an
//!   optional member has `minOccurs="0"` (§7.3.3.2.2); a member of a string
//!   type is `nillable` (IDL-to-XSD mapping 4.1.7.2).
//! - **A union** is a `complexType` with the `discriminator` and a `choice`
//!   of the branches (4.1.7.4), each branch `minOccurs="0"` and carrying the
//!   annotation `case` with its labels.
//! - **An alias of a complex type** is a `complexType` whose
//!   `complexContent` restricts the aliased type and states its content again
//!   (4.1.7.3); an alias of a simple type is a `simpleType` restriction.
//! - **An enumeration** is a `simpleType` restricting `xs:string` to the
//!   literals; **a bitmask** a `simpleType` that is a list of such a
//!   restriction (the sample of a bitmask is the names of the set flags). A
//!   bounded string has a `maxLength` that is `fixed` (4.1.6).
//! - **A sequence** is a `complexType` whose `sequence` has the one element
//!   `item`, `minOccurs="0"` and `maxOccurs` the bound (4.1.7.5).
//! - **An array** is the same with `minOccurs` and `maxOccurs` the dimension
//!   (4.1.7.6). A field of an array type has the type `<enclosing>.<field>_ArrayOf<element>`
//!   and a name of its own. An array of more than one dimension is the
//!   nesting the mapping shows for `long matrix[5][3]`: the intermediate type
//!   `ArrayOf<element>` has the element `item` five times, and `matrix` has
//!   the element `item1` of that type three times, so that the first
//!   dimension of the model is the innermost one; an intermediate type of
//!   the same name and other dimensions gets the suffix `_1`, `_2`, ... .
//!   The mapping offers a SOAP-encoding form beside this WS-I form (a
//!   restriction of `soapenc:Array` under the prefix `_SE_`); the WS-I form
//!   is what is written, the other is skipped on reading.
//! - **A map** is a `complexType` whose `sequence` has `maxOccurs` the bound
//!   and the elements `key` and `value`, each once (§7.3.3.4).
//! - **A root element** is declared for each type that is not nested, named
//!   by its fully qualified name (§7.3.3.3).
//! - **Annotations** (§7.3.3.1): an OMG annotation type is a `complexType`
//!   with an `appinfo` of the source `.../Annotation/Definition` whose
//!   attributes are its members; an application is an `appinfo` of the
//!   source `.../Annotation/Usage` with an `annotate` element per
//!   annotation; the built-in annotations of the IDL Type Representation
//!   are written under their IDL names (`key`, `id`, `must_understand`,
//!   `non_serialized`, `external`, `try_construct`, `nested`, `autoid`,
//!   `final`, `appendable`, `mutable`, `bit_bound`, `value`,
//!   `default_literal`, `position`, `verbatim`).
//!
//! What has no XSD form is refused by name: a `bitset` (neither Annex A nor
//! the IDL-to-XSD mapping has one), and a structure that has the single
//! member `element` (the reader would take it for a collection). Constants,
//! includes and forward declarations declare no XSD component; the values of
//! constants are already in the bounds that name them.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt::Write as _;

use crate::errors::XmlError;
use crate::xsd_schema::{
    APPINFO_ANNOTATION_DEFINITION, APPINFO_ANNOTATION_USAGE, IDL_MAPPED_NAMESPACE,
    OMG_TYPE_NAMESPACE, is_item_name,
};
use crate::xtypes_def::{
    Annotation, AnnotationTypeDef, AutoId, BitmaskType, EnumType, Extensibility, PrimitiveType,
    StructMember, StructType, TryConstruct, TypeDef, TypeLibrary, TypeRef, TypedefType,
    UnionDiscriminator, UnionType, Verbatim,
};

/// The XSD type of a basic or named type: the `type` attribute, the inline
/// restriction instead of it, and the annotation that tells a DDS type from
/// another one of the same XSD type.
type SimpleType = (Option<String>, Option<String>, Option<Note>);

/// How many aliases and base types a restated content follows.
const MAX_RESTATE_DEPTH: usize = 64;

/// What kind of XSD component a declared type is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A `simpleType`: an enumeration, a bitmask, an alias of a simple type.
    Simple,
    /// A `complexType`.
    Complex,
}

/// The XSD of the library.
///
/// # Errors
/// * [`XmlError::UnresolvedReference`] — a type name the library does not
///   declare.
/// * [`XmlError::InvalidXml`] — a construct with no XSD form (see the module
///   documentation).
pub fn type_library_to_xsd(lib: &TypeLibrary) -> Result<String, XmlError> {
    let mut declared: Vec<(Vec<String>, &TypeDef)> = Vec::new();
    collect(&lib.types, &mut Vec::new(), &mut declared)?;
    let mut writer = Writer::new(&declared)?;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<xs:schema xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"\n           \
         xmlns:omg=\"{OMG_TYPE_NAMESPACE}\"\n           \
         xmlns:tns=\"{IDL_MAPPED_NAMESPACE}\"\n           \
         targetNamespace=\"{IDL_MAPPED_NAMESPACE}\"\n           \
         elementFormDefault=\"qualified\">"
    );
    for (path, def) in &declared {
        writer.scope = path.clone();
        match def {
            TypeDef::Struct(s) => writer.structure(s, path, &mut out)?,
            TypeDef::Union(u) => writer.union(u, path, &mut out)?,
            TypeDef::Enum(e) => writer.enumeration(e, path, &mut out)?,
            TypeDef::Bitmask(b) => writer.bitmask(b, path, &mut out)?,
            TypeDef::Typedef(t) => writer.typedef(t, path, &mut out)?,
            TypeDef::Annotation(a) => writer.annotation_type(a, path, &mut out)?,
            _ => {}
        }
        // The named types the declaration needed for its arrays
        // (IDL-to-XSD mapping 4.1.7.6).
        out.push_str(&writer.take_extra());
    }
    // §7.3.3.3: a root element for each type that is not nested.
    for (path, def) in &declared {
        let nested = match def {
            TypeDef::Struct(s) => s.nested,
            TypeDef::Union(u) => u.nested,
            _ => continue,
        };
        if nested != Some(true) {
            let name = xsd_name(path, def.name());
            let _ = writeln!(out, "  <xs:element name=\"{name}\" type=\"tns:{name}\"/>");
        }
    }
    out.push_str("</xs:schema>\n");
    Ok(out)
}

/// The declared types of the library, with the module path of each.
///
/// zerodds-lint: recursion-depth = module nesting depth, capped by
/// `parser::MAX_TREE_DEPTH` = 64 when the library is read from a document.
fn collect<'a>(
    types: &'a [TypeDef],
    path: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, &'a TypeDef)>,
) -> Result<(), XmlError> {
    for def in types {
        match def {
            TypeDef::Module(m) => {
                path.push(m.name.clone());
                collect(&m.types, path, out)?;
                path.pop();
            }
            TypeDef::Const(_) | TypeDef::Include(_) | TypeDef::ForwardDcl(_) => {}
            TypeDef::Bitset(b) => {
                return Err(XmlError::InvalidXml(format!(
                    "bitset `{}` has no XSD form: neither Annex A nor the IDL-to-XSD mapping \
                     (XTypes 1.3 §7.3.3) has one",
                    b.name
                )));
            }
            other => out.push((path.clone(), other)),
        }
    }
    Ok(())
}

fn xsd_name(path: &[String], name: &str) -> String {
    let mut out = String::new();
    for part in path {
        out.push_str(part);
        out.push('.');
    }
    out.push_str(name);
    out
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            other => out.push(other),
        }
    }
    out
}

/// One `annotate`: the name of the annotation and its attributes.
struct Note {
    name: String,
    values: Vec<(String, String)>,
}

impl Note {
    fn flag(name: &str) -> Self {
        Self {
            name: name.to_string(),
            values: Vec::new(),
        }
    }

    fn valued(name: &str, value: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            values: alloc::vec![(String::from("value"), value.into())],
        }
    }

    fn custom(annotation: &Annotation) -> Self {
        Self {
            name: annotation.name.clone(),
            values: annotation
                .members
                .iter()
                .filter_map(|m| m.value.clone().map(|v| (m.name.clone(), v)))
                .collect(),
        }
    }
}

/// The `xsd:annotation` of a definition: an `appinfo` with one `annotate`
/// per note, or nothing.
fn usage(notes: &[Note], indent: usize) -> String {
    if notes.is_empty() {
        return String::new();
    }
    let pad = " ".repeat(indent);
    let mut out = String::new();
    let _ = writeln!(out, "{pad}<xs:annotation>");
    let _ = writeln!(
        out,
        "{pad}  <xs:appinfo source=\"{APPINFO_ANNOTATION_USAGE}\">"
    );
    for note in notes {
        let _ = write!(
            out,
            "{pad}    <omg:annotate omg:type=\"{}\"",
            escape(&note.name)
        );
        for (key, value) in &note.values {
            let _ = write!(out, " {}=\"{}\"", key, escape(value));
        }
        out.push_str("/>\n");
    }
    let _ = writeln!(out, "{pad}  </xs:appinfo>");
    let _ = writeln!(out, "{pad}</xs:annotation>");
    out
}

fn verbatim_note(v: &Verbatim) -> Note {
    Note {
        name: String::from("verbatim"),
        values: alloc::vec![
            (String::from("language"), v.language.clone()),
            (String::from("placement"), v.placement.clone()),
            (String::from("text"), v.text.clone()),
        ],
    }
}

fn extensibility_note(e: Extensibility) -> Note {
    Note::flag(match e {
        Extensibility::Final => "final",
        Extensibility::Appendable => "appendable",
        Extensibility::Mutable => "mutable",
    })
}

fn autoid_note(a: AutoId) -> Note {
    Note::valued(
        "autoid",
        match a {
            AutoId::Hash => "HASH",
            AutoId::Sequential => "SEQUENTIAL",
        },
    )
}

fn bool_note(name: &str, value: bool) -> Note {
    if value {
        Note::flag(name)
    } else {
        Note::valued(name, "FALSE")
    }
}

/// What the declarations of the library are, for the names in the types.
struct Writer {
    /// The fully qualified names (`A::B`) of the declared types, by kind.
    kinds: BTreeMap<String, Kind>,
    /// The annotation types, for `baseType`.
    annotation_types: BTreeMap<String, AnnotationTypeDef>,
    /// The module path of the declaration being written.
    scope: Vec<String>,
    /// The aggregates and aliases by fully qualified name, to restate their
    /// content in an alias of a complex type.
    defs: BTreeMap<String, Def>,
    /// The named types an array needs, written after the declaration that
    /// needs them.
    extra: RefCell<Extra>,
}

/// A declaration an alias of a complex type restates the content of.
#[derive(Clone)]
enum Def {
    Struct(StructType),
    Union(UnionType),
    Typedef(TypedefType),
}

/// The named array types of the declaration being written
/// (IDL-to-XSD mapping 4.1.7.6) and the names given to the intermediate ones.
#[derive(Default)]
struct Extra {
    /// The text of the types to write after the declaration.
    text: String,
    /// The intermediate array types so far: the shape they stand for and
    /// the name they were given.
    intermediates: Vec<(String, String)>,
}

impl Writer {
    fn new(declared: &[(Vec<String>, &TypeDef)]) -> Result<Self, XmlError> {
        let mut kinds = BTreeMap::new();
        let mut annotation_types = BTreeMap::new();
        let mut defs: BTreeMap<String, Def> = BTreeMap::new();
        let fq = |path: &[String], name: &str| {
            let mut s = path.join("::");
            if !s.is_empty() {
                s.push_str("::");
            }
            s.push_str(name);
            s
        };
        // Aliases whose kind depends on their target are settled after the
        // others; each pass settles those whose target is settled.
        let mut pending: Vec<(String, Vec<String>, &TypedefType)> = Vec::new();
        for (path, def) in declared {
            let name = fq(path, def.name());
            match def {
                TypeDef::Struct(st) => {
                    defs.insert(name.clone(), Def::Struct(st.clone()));
                    kinds.insert(name, Kind::Complex);
                }
                TypeDef::Union(un) => {
                    defs.insert(name.clone(), Def::Union(un.clone()));
                    kinds.insert(name, Kind::Complex);
                }
                TypeDef::Enum(_) | TypeDef::Bitmask(_) => {
                    kinds.insert(name, Kind::Simple);
                }
                TypeDef::Typedef(t) => {
                    defs.insert(name.clone(), Def::Typedef(t.clone()));
                    pending.push((name, path.clone(), t));
                }
                TypeDef::Annotation(a) => {
                    annotation_types.insert(name, a.clone());
                }
                _ => {}
            }
        }
        let mut progress = true;
        while progress && !pending.is_empty() {
            progress = false;
            let mut still = Vec::new();
            for (name, path, t) in pending {
                if is_collection(t) {
                    kinds.insert(name, Kind::Complex);
                    progress = true;
                    continue;
                }
                match &t.type_ref {
                    TypeRef::Primitive(_) => {
                        kinds.insert(name, Kind::Simple);
                        progress = true;
                    }
                    TypeRef::Named(target) => match lookup(&kinds, &path, target) {
                        Some((_, kind)) => {
                            kinds.insert(name, kind);
                            progress = true;
                        }
                        None => still.push((name, path, t)),
                    },
                }
            }
            pending = still;
        }
        if let Some((name, _, t)) = pending.first() {
            return Err(XmlError::UnresolvedReference(format!(
                "typedef `{name}` names `{}`, which the library does not declare",
                match &t.type_ref {
                    TypeRef::Named(n) => n.as_str(),
                    TypeRef::Primitive(_) => "",
                }
            )));
        }
        Ok(Self {
            kinds,
            annotation_types,
            scope: Vec::new(),
            defs,
            extra: RefCell::new(Extra::default()),
        })
    }

    /// The text of the named types written since the last call.
    fn take_extra(&self) -> String {
        core::mem::take(&mut self.extra.borrow_mut().text)
    }

    /// The fully qualified XSD name of the type `written` names from the
    /// scope of the declaration being written.
    fn reference(&self, written: &str) -> Result<(String, Kind), XmlError> {
        lookup(&self.kinds, &self.scope, written)
            .map(|(fq, kind)| (fq.replace("::", "."), kind))
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!(
                    "`{written}`, used in module `{}`, is not declared in the library",
                    self.scope.join("::")
                ))
            })
    }

    // -- structures and unions -------------------------------------------

    fn structure(
        &mut self,
        s: &StructType,
        path: &[String],
        out: &mut String,
    ) -> Result<(), XmlError> {
        let name = xsd_name(path, &s.name);
        if s.members.len() == 1 && s.base_type.is_none() && is_item_name(&s.members[0].name) {
            return Err(XmlError::InvalidXml(format!(
                "structure `{name}` has the single member `{}`, which the reader would \
                 take for a sequence or an array",
                s.members[0].name
            )));
        }
        let mut notes = Vec::new();
        if let Some(e) = s.extensibility {
            notes.push(extensibility_note(e));
        }
        if let Some(a) = s.auto_id {
            notes.push(autoid_note(a));
        }
        if let Some(n) = s.nested {
            notes.push(bool_note("nested", n));
        }
        notes.extend(s.verbatims.iter().map(verbatim_note));
        notes.extend(s.annotations.iter().map(Note::custom));
        let _ = writeln!(out, "  <xs:complexType name=\"{name}\">");
        out.push_str(&usage(&notes, 4));
        match &s.base_type {
            Some(base) => {
                let (reference, _) = self.reference(base)?;
                let _ = writeln!(out, "    <xs:complexContent>");
                if s.members.is_empty() {
                    let _ = writeln!(out, "      <xs:extension base=\"tns:{reference}\"/>");
                } else {
                    let _ = writeln!(out, "      <xs:extension base=\"tns:{reference}\">");
                    self.members_sequence(&s.members, 8, &name, out)?;
                    let _ = writeln!(out, "      </xs:extension>");
                }
                let _ = writeln!(out, "    </xs:complexContent>");
            }
            None if s.members.is_empty() => {
                let _ = writeln!(out, "    <xs:sequence/>");
            }
            None => self.members_sequence(&s.members, 4, &name, out)?,
        }
        let _ = writeln!(out, "  </xs:complexType>");
        Ok(())
    }

    /// `<xs:sequence>` of the members at `indent`.
    fn members_sequence(
        &self,
        members: &[StructMember],
        indent: usize,
        owner: &str,
        out: &mut String,
    ) -> Result<(), XmlError> {
        let pad = " ".repeat(indent);
        let _ = writeln!(out, "{pad}<xs:sequence>");
        for m in members {
            self.member(m, indent + 2, owner, out)?;
        }
        let _ = writeln!(out, "{pad}</xs:sequence>");
        Ok(())
    }

    fn union(&mut self, u: &UnionType, path: &[String], out: &mut String) -> Result<(), XmlError> {
        let name = xsd_name(path, &u.name);
        let mut notes = Vec::new();
        if let Some(e) = u.extensibility {
            notes.push(extensibility_note(e));
        }
        if let Some(a) = u.auto_id {
            notes.push(autoid_note(a));
        }
        if let Some(n) = u.nested {
            notes.push(bool_note("nested", n));
        }
        notes.extend(u.verbatims.iter().map(verbatim_note));
        notes.extend(u.annotations.iter().map(Note::custom));
        let _ = writeln!(out, "  <xs:complexType name=\"{name}\">");
        out.push_str(&usage(&notes, 4));
        self.union_body(u, &name, 4, out)?;
        let _ = writeln!(out, "  </xs:complexType>");
        Ok(())
    }

    /// The `sequence` of a union: the discriminator and the choice of its
    /// branches (IDL-to-XSD mapping 4.1.7.4), each branch optional within the
    /// choice.
    fn union_body(
        &self,
        u: &UnionType,
        owner: &str,
        indent: usize,
        out: &mut String,
    ) -> Result<(), XmlError> {
        let pad = " ".repeat(indent);
        let _ = writeln!(out, "{pad}<xs:sequence>");
        let mut disc_notes = Vec::new();
        if u.discriminator_key {
            disc_notes.push(Note::flag("key"));
        }
        disc_notes.extend(u.discriminator_annotations.iter().map(Note::custom));
        let discriminator = Shape::Plain {
            type_ref: u.discriminator.clone(),
            string_max: None,
        };
        let own = format!("{owner}.discriminator");
        self.element(
            &discriminator,
            "discriminator",
            (1, Some(1)),
            &disc_notes,
            indent + 2,
            &own,
            true,
            out,
        )?;
        let _ = writeln!(out, "{pad}  <xs:choice>");
        for case in &u.cases {
            let labels: Vec<String> = case
                .discriminators
                .iter()
                .map(|d| match d {
                    UnionDiscriminator::Value(v) => v.clone(),
                    UnionDiscriminator::Default => String::from("default"),
                })
                .collect();
            let mut member_notes = alloc::vec![Note::valued("case", labels.join(","))];
            let shape = member_shape(&case.member)?;
            member_notes.extend(member_notes_of(&case.member, false));
            let own = format!("{owner}.{}", case.member.name);
            self.element(
                &shape,
                &case.member.name,
                (0, Some(1)),
                &member_notes,
                indent + 4,
                &own,
                true,
                out,
            )?;
        }
        let _ = writeln!(out, "{pad}  </xs:choice>");
        let _ = writeln!(out, "{pad}</xs:sequence>");
        Ok(())
    }

    fn member(
        &self,
        m: &StructMember,
        indent: usize,
        owner: &str,
        out: &mut String,
    ) -> Result<(), XmlError> {
        let shape = member_shape(m)?;
        let notes = member_notes_of(m, m.optional && !matches!(shape, Shape::Plain { .. }));
        let occurs = if m.optional {
            (0, Some(1))
        } else {
            (1, Some(1))
        };
        let own = format!("{owner}.{}", m.name);
        self.element(&shape, &m.name, occurs, &notes, indent, &own, true, out)
    }

    // -- elements ---------------------------------------------------------

    /// The element `name` of shape `shape` with the occurrences `occurs`
    /// (and the notes of the member), at `indent`.
    ///
    /// `owner` is the dotted name the element is below (`M.S.field`): an
    /// array is a named type, whose name starts with it (IDL-to-XSD mapping
    /// 4.1.7.6: "the name of the enclosing type and the name of the field").
    /// `is_member` says the element is a member of an aggregate and not an
    /// item of a collection: only a member of a string type is `nillable`
    /// (4.1.7.2).
    ///
    /// zerodds-lint: recursion-depth = nesting of the collections of one
    /// member, three at most (sequence, array dimensions, map) plus the
    /// dimensions.
    #[allow(clippy::too_many_arguments)]
    fn element(
        &self,
        shape: &Shape,
        name: &str,
        occurs: (u32, Option<u32>),
        notes: &[Note],
        indent: usize,
        owner: &str,
        is_member: bool,
        out: &mut String,
    ) -> Result<(), XmlError> {
        let pad = " ".repeat(indent);
        let mut occurs_attrs = occurs_attributes(occurs);
        match shape {
            Shape::Plain {
                type_ref,
                string_max,
            } => {
                let (xs_type, facets, refinement) = self.simple_type_of(type_ref, *string_max)?;
                if is_member
                    && matches!(
                        type_ref,
                        TypeRef::Primitive(PrimitiveType::String | PrimitiveType::WString)
                    )
                {
                    occurs_attrs.push_str(" nillable=\"true\"");
                }
                let mut all_notes: Vec<Note> = Vec::new();
                all_notes.extend(refinement);
                for n in notes {
                    all_notes.push(Note {
                        name: n.name.clone(),
                        values: n.values.clone(),
                    });
                }
                let annotation = usage(&all_notes, indent + 2);
                match (&xs_type, facets) {
                    (Some(t), None) if annotation.is_empty() => {
                        let _ = writeln!(
                            out,
                            "{pad}<xs:element name=\"{}\" type=\"{t}\"{occurs_attrs}/>",
                            escape(name)
                        );
                    }
                    (Some(t), None) => {
                        let _ = writeln!(
                            out,
                            "{pad}<xs:element name=\"{}\" type=\"{t}\"{occurs_attrs}>",
                            escape(name)
                        );
                        out.push_str(&annotation);
                        let _ = writeln!(out, "{pad}</xs:element>");
                    }
                    (_, Some(restriction)) => {
                        let _ = writeln!(
                            out,
                            "{pad}<xs:element name=\"{}\"{occurs_attrs}>",
                            escape(name)
                        );
                        out.push_str(&annotation);
                        let _ = writeln!(out, "{pad}  <xs:simpleType>");
                        let _ = writeln!(out, "{pad}    {restriction}");
                        let _ = writeln!(out, "{pad}  </xs:simpleType>");
                        let _ = writeln!(out, "{pad}</xs:element>");
                    }
                    (None, None) => {}
                }
                Ok(())
            }
            Shape::Array(..) => {
                // A named type, as the mapping has it for an array declared
                // as a field: `<enclosing>.<field>_ArrayOf<element>`.
                let (_, base) = array_chain(shape);
                let implicit = format!("{owner}_ArrayOf{}", self.label_of(base)?);
                let mut body = String::new();
                self.collection_body(shape, 4, owner, &mut body)?;
                {
                    let mut extra = self.extra.borrow_mut();
                    let _ = writeln!(extra.text, "  <xs:complexType name=\"{implicit}\">");
                    extra.text.push_str(&body);
                    let _ = writeln!(extra.text, "  </xs:complexType>");
                }
                if is_member {
                    occurs_attrs.push_str(" nillable=\"true\"");
                }
                let annotation = usage(notes, indent + 2);
                if annotation.is_empty() {
                    let _ = writeln!(
                        out,
                        "{pad}<xs:element name=\"{}\" type=\"tns:{implicit}\"{occurs_attrs}/>",
                        escape(name)
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "{pad}<xs:element name=\"{}\" type=\"tns:{implicit}\"{occurs_attrs}>",
                        escape(name)
                    );
                    out.push_str(&annotation);
                    let _ = writeln!(out, "{pad}</xs:element>");
                }
                Ok(())
            }
            Shape::Sequence(..) | Shape::Map { .. } => {
                let _ = writeln!(
                    out,
                    "{pad}<xs:element name=\"{}\"{occurs_attrs}>",
                    escape(name)
                );
                out.push_str(&usage(notes, indent + 2));
                let _ = writeln!(out, "{pad}  <xs:complexType>");
                self.collection_body(shape, indent + 4, owner, out)?;
                let _ = writeln!(out, "{pad}  </xs:complexType>");
                let _ = writeln!(out, "{pad}</xs:element>");
                Ok(())
            }
        }
    }

    /// The `sequence` of a collection or map type: the one element `item`
    /// of a sequence or an array (IDL-to-XSD mapping 4.1.7.5, 4.1.7.6), or
    /// the `key` and the `value` of a map under a `maxOccurs` of the bound
    /// (§7.3.3.4).
    ///
    /// An array of more than one dimension is the nesting the mapping
    /// writes: the first dimension is the innermost type, one element
    /// `item` of that many occurrences, and each further dimension is the
    /// element `item1`, `item2`, ... of one more named type
    /// (`ArrayOf<element>`), the last dimension in the type that is written.
    fn collection_body(
        &self,
        shape: &Shape,
        indent: usize,
        owner: &str,
        out: &mut String,
    ) -> Result<(), XmlError> {
        let pad = " ".repeat(indent);
        match shape {
            Shape::Sequence(inner, bound) => {
                let max = if *bound == 0 { None } else { Some(*bound) };
                let _ = writeln!(out, "{pad}<xs:sequence>");
                let own = format!("{owner}_item");
                self.element(inner, "item", (0, max), &[], indent + 2, &own, false, out)?;
                let _ = writeln!(out, "{pad}</xs:sequence>");
            }
            Shape::Array(..) => {
                let (dims, base) = array_chain(shape);
                let last = dims.len() - 1;
                let _ = writeln!(out, "{pad}<xs:sequence>");
                if last == 0 {
                    let own = format!("{owner}_item");
                    self.element(
                        base,
                        "item",
                        (dims[0], Some(dims[0])),
                        &[],
                        indent + 2,
                        &own,
                        false,
                        out,
                    )?;
                } else {
                    let inner = self.array_level(base, &dims[..last], owner)?;
                    let n = dims[last];
                    let _ = writeln!(
                        out,
                        "{pad}  <xs:element name=\"item{last}\" type=\"tns:{inner}\" \
                         minOccurs=\"{n}\" maxOccurs=\"{n}\"/>"
                    );
                }
                let _ = writeln!(out, "{pad}</xs:sequence>");
            }
            Shape::Map { key, value, bound } => {
                let max = match bound {
                    Some(n) => n.to_string(),
                    None => String::from("unbounded"),
                };
                let _ = writeln!(out, "{pad}<xs:sequence maxOccurs=\"{max}\">");
                let key_owner = format!("{owner}_key");
                self.element(
                    key,
                    "key",
                    (1, Some(1)),
                    &[],
                    indent + 2,
                    &key_owner,
                    false,
                    out,
                )?;
                let value_owner = format!("{owner}_value");
                self.element(
                    value,
                    "value",
                    (1, Some(1)),
                    &[],
                    indent + 2,
                    &value_owner,
                    false,
                    out,
                )?;
                let _ = writeln!(out, "{pad}</xs:sequence>");
            }
            Shape::Plain { .. } => {
                return Err(XmlError::InvalidXml(String::from(
                    "a basic or named type has no collection body",
                )));
            }
        }
        Ok(())
    }

    /// The name of the intermediate type for the first `dims.len()`
    /// dimensions of an array of `base`, written if it is new
    /// (`ArrayOf<element>`; where the name is taken by a type with other
    /// dimensions, the suffix `_n` of the n-th collision, 4.1.7.6).
    ///
    /// zerodds-lint: recursion-depth = the number of dimensions of one array
    fn array_level(&self, base: &Shape, dims: &[u32], owner: &str) -> Result<String, XmlError> {
        let key = format!("{base:?}/{dims:?}");
        if let Some((_, name)) = self
            .extra
            .borrow()
            .intermediates
            .iter()
            .find(|(k, _)| *k == key)
        {
            return Ok(name.clone());
        }
        let label = format!("ArrayOf{}", self.label_of(base)?);
        let name = {
            let extra = self.extra.borrow();
            let taken = |candidate: &str| extra.intermediates.iter().any(|(_, n)| n == candidate);
            let mut candidate = label.clone();
            let mut n = 0;
            while taken(&candidate) {
                n += 1;
                candidate = format!("{label}_{n}");
            }
            candidate
        };
        // Registered before the body is written, so that the body finds the
        // name of a type it needs below it.
        self.extra
            .borrow_mut()
            .intermediates
            .push((key, name.clone()));
        let last = dims.len() - 1;
        let mut body = String::new();
        let _ = writeln!(body, "    <xs:sequence>");
        if last == 0 {
            let own = format!("{owner}_item");
            self.element(
                base,
                "item",
                (dims[0], Some(dims[0])),
                &[],
                6,
                &own,
                false,
                &mut body,
            )?;
        } else {
            let inner = self.array_level(base, &dims[..last], owner)?;
            let n = dims[last];
            let _ = writeln!(
                body,
                "      <xs:element name=\"item{last}\" type=\"tns:{inner}\" \
                 minOccurs=\"{n}\" maxOccurs=\"{n}\"/>"
            );
        }
        let _ = writeln!(body, "    </xs:sequence>");
        let mut extra = self.extra.borrow_mut();
        let _ = writeln!(extra.text, "  <xs:complexType name=\"{name}\">");
        extra.text.push_str(&body);
        let _ = writeln!(extra.text, "  </xs:complexType>");
        Ok(name)
    }

    /// The part of the name of an array type that names its element:
    /// `int` for `xs:int`, `M_Inner` for `M::Inner`.
    fn label_of(&self, shape: &Shape) -> Result<String, XmlError> {
        Ok(match shape {
            Shape::Plain { type_ref, .. } => match type_ref {
                TypeRef::Named(written) => self.reference(written)?.0.replace('.', "_"),
                TypeRef::Primitive(p) => String::from(match p {
                    PrimitiveType::Char => "char",
                    PrimitiveType::WChar => "wchar",
                    PrimitiveType::String => "string",
                    PrimitiveType::WString => "wstring",
                    PrimitiveType::UInt8 => "uint8",
                    other => xs_primitive(*other),
                }),
            },
            Shape::Sequence(..) => String::from("sequence"),
            Shape::Array(..) => String::from("array"),
            Shape::Map { .. } => String::from("map"),
        })
    }

    /// The XSD type of a basic or named type: the `type` attribute, or an
    /// inline restriction (`Some`), and the annotation that tells a DDS type
    /// from another one of the same XSD type.
    fn simple_type_of(
        &self,
        type_ref: &TypeRef,
        string_max: Option<u32>,
    ) -> Result<SimpleType, XmlError> {
        let restriction = |max: Option<u32>, length: Option<u32>| {
            let mut s = String::from("<xs:restriction base=\"xs:string\">");
            if let Some(n) = max {
                let _ = write!(s, "<xs:maxLength value=\"{n}\" fixed=\"true\"/>");
            }
            if let Some(n) = length {
                let _ = write!(s, "<xs:length value=\"{n}\"/>");
            }
            s.push_str("</xs:restriction>");
            s
        };
        Ok(match type_ref {
            TypeRef::Named(written) => {
                let (reference, _) = self.reference(written)?;
                (Some(format!("tns:{reference}")), None, None)
            }
            TypeRef::Primitive(p) => match p {
                PrimitiveType::String => match string_max {
                    Some(n) => (None, Some(restriction(Some(n), None)), None),
                    None => (Some(String::from("xs:string")), None, None),
                },
                PrimitiveType::WString => (
                    None,
                    Some(restriction(string_max, None)),
                    Some(Note::flag("wstring")),
                ),
                PrimitiveType::Char => (
                    None,
                    Some(restriction(None, Some(1))),
                    Some(Note::flag("char")),
                ),
                PrimitiveType::WChar => (
                    None,
                    Some(restriction(None, Some(1))),
                    Some(Note::flag("wchar")),
                ),
                PrimitiveType::UInt8 => (
                    Some(String::from("xs:unsignedByte")),
                    None,
                    Some(Note::flag("uint8")),
                ),
                other => (Some(format!("xs:{}", xs_primitive(*other))), None, None),
            },
        })
    }

    // -- enumerations, bitmasks -------------------------------------------

    fn enumeration(
        &mut self,
        e: &EnumType,
        path: &[String],
        out: &mut String,
    ) -> Result<(), XmlError> {
        let name = xsd_name(path, &e.name);
        let mut notes = Vec::new();
        if let Some(x) = e.extensibility {
            notes.push(extensibility_note(x));
        }
        if let Some(b) = e.bit_bound {
            notes.push(Note::valued("bit_bound", b.to_string()));
        }
        notes.extend(e.verbatims.iter().map(verbatim_note));
        notes.extend(e.annotations.iter().map(Note::custom));
        let _ = writeln!(out, "  <xs:simpleType name=\"{name}\">");
        out.push_str(&usage(&notes, 4));
        let _ = writeln!(out, "    <xs:restriction base=\"xs:string\">");
        for literal in &e.enumerators {
            let mut literal_notes = Vec::new();
            if let Some(v) = literal.value {
                literal_notes.push(Note::valued("value", v.to_string()));
            }
            if let Some(d) = literal.default_literal {
                literal_notes.push(bool_note("default_literal", d));
            }
            literal_notes.extend(literal.annotations.iter().map(Note::custom));
            enumeration_value(&literal.name, &literal_notes, 6, out);
        }
        let _ = writeln!(out, "    </xs:restriction>");
        let _ = writeln!(out, "  </xs:simpleType>");
        Ok(())
    }

    fn bitmask(
        &mut self,
        b: &BitmaskType,
        path: &[String],
        out: &mut String,
    ) -> Result<(), XmlError> {
        let name = xsd_name(path, &b.name);
        let mut notes = Vec::new();
        if let Some(bound) = b.bit_bound {
            notes.push(Note::valued("bit_bound", bound.to_string()));
        }
        notes.extend(b.annotations.iter().map(Note::custom));
        let _ = writeln!(out, "  <xs:simpleType name=\"{name}\">");
        out.push_str(&usage(&notes, 4));
        let _ = writeln!(out, "    <xs:list>");
        let _ = writeln!(out, "      <xs:simpleType>");
        let _ = writeln!(out, "        <xs:restriction base=\"xs:string\">");
        for flag in &b.bit_values {
            let mut flag_notes = Vec::new();
            if let Some(p) = flag.position {
                flag_notes.push(Note::valued("position", p.to_string()));
            }
            flag_notes.extend(flag.annotations.iter().map(Note::custom));
            enumeration_value(&flag.name, &flag_notes, 10, out);
        }
        let _ = writeln!(out, "        </xs:restriction>");
        let _ = writeln!(out, "      </xs:simpleType>");
        let _ = writeln!(out, "    </xs:list>");
        let _ = writeln!(out, "  </xs:simpleType>");
        Ok(())
    }

    // -- aliases ----------------------------------------------------------

    fn typedef(
        &mut self,
        t: &TypedefType,
        path: &[String],
        out: &mut String,
    ) -> Result<(), XmlError> {
        let name = xsd_name(path, &t.name);
        let mut notes = Vec::new();
        if let Some(e) = t.external {
            notes.push(bool_note("external", e));
        }
        if is_collection(t) {
            let shape = typedef_shape(t)?;
            let _ = writeln!(out, "  <xs:complexType name=\"{name}\">");
            out.push_str(&usage(&notes, 4));
            self.collection_body(&shape, 4, &name, out)?;
            let _ = writeln!(out, "  </xs:complexType>");
            return Ok(());
        }
        match &t.type_ref {
            TypeRef::Named(written) => {
                let (reference, kind) = self.reference(written)?;
                match kind {
                    Kind::Simple => {
                        let _ = writeln!(out, "  <xs:simpleType name=\"{name}\">");
                        out.push_str(&usage(&notes, 4));
                        let _ = writeln!(out, "    <xs:restriction base=\"tns:{reference}\"/>");
                        let _ = writeln!(out, "  </xs:simpleType>");
                    }
                    Kind::Complex => {
                        // IDL-to-XSD mapping 4.1.7.3: a restriction of the
                        // type that states its content again.
                        let target = reference.replace('.', "::");
                        let _ = writeln!(out, "  <xs:complexType name=\"{name}\">");
                        out.push_str(&usage(&notes, 4));
                        let _ = writeln!(out, "    <xs:complexContent>");
                        let _ = writeln!(out, "      <xs:restriction base=\"tns:{reference}\">");
                        self.restate(&target, &name, 8, out)?;
                        let _ = writeln!(out, "      </xs:restriction>");
                        let _ = writeln!(out, "    </xs:complexContent>");
                        let _ = writeln!(out, "  </xs:complexType>");
                    }
                }
            }
            TypeRef::Primitive(p) => {
                let (xs_type, facets, refinement) =
                    self.simple_type_of(&TypeRef::Primitive(*p), t.string_max_length)?;
                notes.extend(refinement);
                let _ = writeln!(out, "  <xs:simpleType name=\"{name}\">");
                out.push_str(&usage(&notes, 4));
                match (xs_type, facets) {
                    (Some(base), None) => {
                        let _ = writeln!(out, "    <xs:restriction base=\"{base}\"/>");
                    }
                    (_, Some(restriction)) => {
                        let _ = writeln!(out, "    {restriction}");
                    }
                    (None, None) => {}
                }
                let _ = writeln!(out, "  </xs:simpleType>");
            }
        }
        Ok(())
    }

    /// The content of the declaration `target` (a fully qualified name, with
    /// `::`), as the restriction of an alias states it again: the members of
    /// a structure with those of its bases first, the `sequence` of a union,
    /// the body of a collection; an alias of an alias is resolved first.
    ///
    /// zerodds-lint: recursion-depth 64 (the chain of aliases and of base
    /// types, bounded by `MAX_RESTATE_DEPTH` below)
    fn restate(
        &mut self,
        target: &str,
        owner: &str,
        indent: usize,
        out: &mut String,
    ) -> Result<(), XmlError> {
        self.restate_at(target, owner, indent, out, 0)
    }

    /// zerodds-lint: recursion-depth 64
    fn restate_at(
        &mut self,
        target: &str,
        owner: &str,
        indent: usize,
        out: &mut String,
        depth: usize,
    ) -> Result<(), XmlError> {
        if depth > MAX_RESTATE_DEPTH {
            return Err(XmlError::CircularInheritance(format!(
                "the aliases and base types of `{target}`"
            )));
        }
        let saved = core::mem::take(&mut self.scope);
        let outcome = self.restate_in(target, owner, indent, out, depth);
        self.scope = saved;
        outcome
    }

    /// zerodds-lint: recursion-depth 64
    fn restate_in(
        &mut self,
        target: &str,
        owner: &str,
        indent: usize,
        out: &mut String,
        depth: usize,
    ) -> Result<(), XmlError> {
        let def = self.defs.get(target).cloned().ok_or_else(|| {
            XmlError::UnresolvedReference(format!("`{target}` is not declared in the library"))
        })?;
        let module: Vec<String> = match target.rsplit_once("::") {
            Some((m, _)) => m.split("::").map(ToString::to_string).collect(),
            None => Vec::new(),
        };
        self.scope = module.clone();
        match def {
            Def::Struct(_) => {
                let mut members: Vec<StructMember> = Vec::new();
                self.collect_members(target, &mut members, 0)?;
                self.scope = module;
                self.members_sequence(&members, indent, owner, out)
            }
            Def::Union(un) => self.union_body(&un, owner, indent, out),
            Def::Typedef(t) => {
                if is_collection(&t) {
                    let shape = typedef_shape(&t)?;
                    self.collection_body(&shape, indent, owner, out)
                } else if let TypeRef::Named(written) = &t.type_ref {
                    let (reference, _) = self.reference(written)?;
                    let next = reference.replace('.', "::");
                    self.restate_at(&next, owner, indent, out, depth + 1)
                } else {
                    Err(XmlError::InvalidXml(format!(
                        "`{target}` is an alias of a basic type and has no content to state"
                    )))
                }
            }
        }
    }

    /// The members of the structure `fq` and of its bases, the bases' first.
    ///
    /// zerodds-lint: recursion-depth 64 (the chain of base types, bounded
    /// by `MAX_RESTATE_DEPTH`)
    fn collect_members(
        &self,
        fq: &str,
        out: &mut Vec<StructMember>,
        depth: usize,
    ) -> Result<(), XmlError> {
        if depth > MAX_RESTATE_DEPTH {
            return Err(XmlError::CircularInheritance(format!(
                "the base types of `{fq}`"
            )));
        }
        let Some(Def::Struct(st)) = self.defs.get(fq) else {
            return Err(XmlError::UnresolvedReference(format!(
                "`{fq}` is no structure of the library"
            )));
        };
        if let Some(base) = &st.base_type {
            let module: Vec<String> = match fq.rsplit_once("::") {
                Some((m, _)) => m.split("::").map(ToString::to_string).collect(),
                None => Vec::new(),
            };
            let (base_fq, _) = lookup(&self.kinds, &module, base).ok_or_else(|| {
                XmlError::UnresolvedReference(format!(
                    "the base type `{base}` of `{fq}` is not declared in the library"
                ))
            })?;
            self.collect_members(&base_fq, out, depth + 1)?;
        }
        out.extend(st.members.iter().cloned());
        Ok(())
    }

    // -- annotation types -------------------------------------------------

    fn annotation_type(
        &mut self,
        a: &AnnotationTypeDef,
        path: &[String],
        out: &mut String,
    ) -> Result<(), XmlError> {
        let name = xsd_name(path, &a.name);
        let _ = writeln!(out, "  <xs:complexType name=\"{name}\">");
        let _ = writeln!(out, "    <xs:annotation>");
        let _ = writeln!(
            out,
            "      <xs:appinfo source=\"{APPINFO_ANNOTATION_DEFINITION}\"/>"
        );
        let _ = writeln!(out, "    </xs:annotation>");
        let mut members = Vec::new();
        self.annotation_members(a, path, &mut members, 0)?;
        for member in members {
            let (xs_type, _, _) = self.simple_type_of(&member.type_ref, None)?;
            let Some(xs_type) = xs_type else {
                return Err(XmlError::InvalidXml(format!(
                    "member `{}` of annotation `{name}` has a type with no XSD attribute type",
                    member.name
                )));
            };
            let _ = write!(
                out,
                "    <xs:attribute name=\"{}\" type=\"{xs_type}\"",
                escape(&member.name)
            );
            if let Some(default) = &member.default_value {
                let _ = write!(out, " default=\"{}\"", escape(default));
            }
            out.push_str("/>\n");
        }
        let _ = writeln!(out, "  </xs:complexType>");
        Ok(())
    }

    /// The members of an annotation type, those of its base first.
    ///
    /// zerodds-lint: recursion-depth = the chain of `baseType`, capped at
    /// 64 below.
    fn annotation_members(
        &self,
        a: &AnnotationTypeDef,
        path: &[String],
        out: &mut Vec<crate::xtypes_def::AnnotationMemberDecl>,
        depth: usize,
    ) -> Result<(), XmlError> {
        if depth > 64 {
            return Err(XmlError::CircularInheritance(format!(
                "the baseType chain of annotation `{}`",
                a.name
            )));
        }
        if let Some(base) = &a.base_type {
            let scope: Vec<String> = path.to_vec();
            let found = candidates(&scope, base)
                .into_iter()
                .find_map(|c| self.annotation_types.get(&c).map(|d| (c, d)));
            let Some((fq, base_def)) = found else {
                return Err(XmlError::UnresolvedReference(format!(
                    "annotation `{}` has the baseType `{base}`, which the library does not declare",
                    a.name
                )));
            };
            let base_path: Vec<String> = fq
                .rsplit_once("::")
                .map(|(p, _)| p.split("::").map(ToString::to_string).collect())
                .unwrap_or_default();
            self.annotation_members(base_def, &base_path, out, depth + 1)?;
        }
        out.extend(a.members.iter().cloned());
        Ok(())
    }
}

fn enumeration_value(name: &str, notes: &[Note], indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    if notes.is_empty() {
        let _ = writeln!(out, "{pad}<xs:enumeration value=\"{}\"/>", escape(name));
    } else {
        let _ = writeln!(out, "{pad}<xs:enumeration value=\"{}\">", escape(name));
        out.push_str(&usage(notes, indent + 2));
        let _ = writeln!(out, "{pad}</xs:enumeration>");
    }
}

/// The attributes `minOccurs` and `maxOccurs` where they are not the default.
fn occurs_attributes(occurs: (u32, Option<u32>)) -> String {
    let mut s = String::new();
    if occurs.0 != 1 {
        let _ = write!(s, " minOccurs=\"{}\"", occurs.0);
    }
    match occurs.1 {
        Some(1) => {}
        Some(n) => {
            let _ = write!(s, " maxOccurs=\"{n}\"");
        }
        None => s.push_str(" maxOccurs=\"unbounded\""),
    }
    s
}

fn xs_primitive(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "boolean",
        PrimitiveType::Octet | PrimitiveType::UInt8 => "unsignedByte",
        PrimitiveType::Int8 => "byte",
        PrimitiveType::Short => "short",
        PrimitiveType::UShort => "unsignedShort",
        PrimitiveType::Long => "int",
        PrimitiveType::ULong => "unsignedInt",
        PrimitiveType::LongLong => "long",
        PrimitiveType::ULongLong => "unsignedLong",
        PrimitiveType::Float => "float",
        PrimitiveType::Double => "double",
        PrimitiveType::LongDouble => "decimal",
        PrimitiveType::Char
        | PrimitiveType::WChar
        | PrimitiveType::String
        | PrimitiveType::WString => "string",
    }
}

// -- shapes ---------------------------------------------------------------

/// What a member declares: the sequence binds tightest, then the array
/// dimensions, then the map (§7.3.2.4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shape {
    Plain {
        type_ref: TypeRef,
        string_max: Option<u32>,
    },
    Sequence(alloc::boxed::Box<Shape>, u32),
    Array(alloc::boxed::Box<Shape>, u32),
    Map {
        key: alloc::boxed::Box<Shape>,
        value: alloc::boxed::Box<Shape>,
        bound: Option<u32>,
    },
}

#[allow(clippy::too_many_arguments)]
fn build_shape(
    type_ref: &TypeRef,
    string_max: Option<u32>,
    sequence: Option<u32>,
    dims: &[u32],
    map_key: Option<&TypeRef>,
    map_max: Option<u32>,
    map_key_string_max: Option<u32>,
) -> Shape {
    use alloc::boxed::Box;
    let mut shape = Shape::Plain {
        type_ref: type_ref.clone(),
        string_max,
    };
    if let Some(bound) = sequence {
        shape = Shape::Sequence(Box::new(shape), bound);
    }
    for dimension in dims.iter().rev() {
        shape = Shape::Array(Box::new(shape), *dimension);
    }
    if let Some(key) = map_key {
        shape = Shape::Map {
            key: Box::new(Shape::Plain {
                type_ref: key.clone(),
                string_max: map_key_string_max,
            }),
            value: Box::new(shape),
            bound: map_max,
        };
    }
    shape
}

fn member_shape(m: &StructMember) -> Result<Shape, XmlError> {
    Ok(build_shape(
        &m.type_ref,
        m.string_max_length,
        m.sequence_max_length,
        &m.array_dimensions,
        m.map_key.as_ref(),
        m.map_max_length,
        m.map_key_string_max_length,
    ))
}

fn typedef_shape(t: &TypedefType) -> Result<Shape, XmlError> {
    Ok(build_shape(
        &t.type_ref,
        t.string_max_length,
        t.sequence_max_length,
        &t.array_dimensions,
        t.map_key.as_ref(),
        t.map_max_length,
        t.map_key_string_max_length,
    ))
}

/// The dimensions of an array shape in the order of the model (the first is
/// the outermost of the shape) and the shape of its element.
fn array_chain(shape: &Shape) -> (Vec<u32>, &Shape) {
    let mut dims = Vec::new();
    let mut current = shape;
    while let Shape::Array(inner, n) = current {
        dims.push(*n);
        current = inner;
    }
    (dims, current)
}

fn is_collection(t: &TypedefType) -> bool {
    t.sequence_max_length.is_some() || !t.array_dimensions.is_empty() || t.map_key.is_some()
}

/// The annotations of a member (§7.3.3.1.3): `optional` only where
/// `minOccurs="0"` would not say it, that is, on a collection.
fn member_notes_of(m: &StructMember, mark_optional: bool) -> Vec<Note> {
    let mut notes = Vec::new();
    if m.key {
        notes.push(Note::flag("key"));
    }
    if let Some(id) = m.id {
        notes.push(Note::valued("id", id.to_string()));
    }
    if mark_optional {
        notes.push(Note::flag("optional"));
    }
    if m.must_understand {
        notes.push(Note::flag("must_understand"));
    }
    if let Some(n) = m.non_serialized {
        notes.push(bool_note("non_serialized", n));
    }
    if let Some(e) = m.external {
        notes.push(bool_note("external", e));
    }
    if let Some(t) = m.try_construct {
        notes.push(Note::valued(
            "try_construct",
            match t {
                TryConstruct::Discard => "DISCARD",
                TryConstruct::UseDefault => "USE_DEFAULT",
                TryConstruct::Trim => "TRIM",
            },
        ));
    }
    notes.extend(m.annotations.iter().map(Note::custom));
    notes
}

// -- names ----------------------------------------------------------------

/// The scopes a name written in module `scope` may name, innermost first.
fn candidates(scope: &[String], written: &str) -> Vec<String> {
    if let Some(absolute) = written.strip_prefix("::") {
        return alloc::vec![absolute.to_string()];
    }
    (0..=scope.len())
        .rev()
        .map(|depth| {
            let mut candidate = scope[..depth].join("::");
            if !candidate.is_empty() {
                candidate.push_str("::");
            }
            candidate.push_str(written);
            candidate
        })
        .collect()
}

fn lookup(
    kinds: &BTreeMap<String, Kind>,
    scope: &[String],
    written: &str,
) -> Option<(String, Kind)> {
    candidates(scope, written)
        .into_iter()
        .find_map(|c| kinds.get(&c).map(|k| (c, *k)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::xsd_schema::parse_xsd_schema;
    use crate::xtypes_parser::parse_type_libraries;

    fn library(body: &str) -> TypeLibrary {
        parse_type_libraries(&format!("<types>{body}</types>"))
            .expect("the library parses")
            .remove(0)
    }

    #[test]
    fn a_structure_is_a_complex_type_with_a_root_element() {
        let xsd = type_library_to_xsd(&library(
            r#"<module name="M"><struct name="S">
                 <member name="a" type="int32"/>
                 <member name="b" type="string" optional="true"/>
               </struct></module>"#,
        ))
        .expect("writes");
        assert!(xsd.contains("<xs:complexType name=\"M.S\">"), "{xsd}");
        assert!(
            xsd.contains("<xs:element name=\"M.S\" type=\"tns:M.S\"/>"),
            "{xsd}"
        );
        assert!(
            xsd.contains("name=\"b\" type=\"xs:string\" minOccurs=\"0\""),
            "{xsd}"
        );
    }

    #[test]
    fn a_nested_type_has_no_root_element() {
        let xsd = type_library_to_xsd(&library(
            r#"<struct name="N" nested="true"><member name="a" type="int32"/></struct>"#,
        ))
        .expect("writes");
        assert!(!xsd.contains("<xs:element name=\"N\" type"), "{xsd}");
        assert!(xsd.contains("omg:type=\"nested\""), "{xsd}");
    }

    #[test]
    fn what_is_written_is_read_back() {
        let lib = library(
            r#"<module name="M">
                 <enum name="Color" bitBound="8">
                   <enumerator name="RED" value="1" defaultLiteral="true"/><enumerator name="GREEN"/>
                 </enum>
                 <bitmask name="Flags" bitBound="16"><flag name="A" position="0"/>
                   <flag name="B" position="3"/></bitmask>
                 <struct name="S" extensibility="mutable">
                   <member name="id" type="int32" key="true" id="7"/>
                   <member name="c" type="nonBasic" nonBasicTypeName="M::Color"/>
                   <member name="text" type="string" stringMaxLength="12" optional="true"/>
                   <member name="q" type="float64" sequenceMaxLength="-1"/>
                   <member name="grid" type="int16" arrayDimensions="2,3"/>
                   <member name="m" type="float64" mapKeyType="int32" mapMaxLength="32"/>
                   <member name="u" type="uint8"/>
                   <member name="w" type="wstring" stringMaxLength="4"/>
                 </struct>
               </module>"#,
        );
        let xsd = type_library_to_xsd(&lib).expect("writes");
        let back = parse_xsd_schema(&xsd).expect("reads");
        let again = type_library_to_xsd(&back).expect("writes again");
        assert_eq!(xsd, again, "writing what was read gives the same schema");
    }
}
