// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DLRL generator of DDS 1.2 §8.2.2.1 (Figure 8.7): the model
//! description (IDL valuetypes, §8.2.2.2) and the model tags (XML,
//! §8.2.2.3) give the model and its mapping.
//!
//! [`ModelDescription`] is the content of the IDL valuetypes in a form
//! that does not depend on an IDL front end. The caller fills it; no
//! front end does yet (`zerodds-idlc` neither builds a description from
//! the valuetypes of an IDL file nor calls [`generate`]). [`generate`]
//! applies the tags to it:
//!
//! * each valuetype of the description is a class; its members are the
//!   class's attributes: a basic type, an enumeration or a simple
//!   structure is a mono-valued attribute, a valuetype of the
//!   description a to-1 relation, a forward valuetype that a
//!   `templateDef` defines a collection (§8.2.2.2: "a void value
//!   declaration, which will be transformed in the generation process"):
//!   of objects a to-many relation, of values a multi-valued attribute;
//! * `local` makes a member a local attribute (§8.2.2.3.2.14);
//!   `compoRelationDef` makes a relation a composition, `associationDef`
//!   makes two relations inverse to each other; `enumDef` codes an
//!   enumeration by the strings it gives (§8.1.4.2.3);
//! * the `classMapping` elements are the mapping information; every part
//!   they do not give takes the default mapping of §8.1.4.3 (through
//!   [`Mapping::new`]).
//!
//! A `templateDef` or `enumDef` name is matched exactly; failing that,
//! a single name equal but for case is taken, as IDL identifiers that
//! differ only in case denote one type (IDL 4.2 §7.2.3) and the example
//! of §8.2.3 writes `stringStrMap` in the IDL and `StringStrMap` in the
//! tags. A name that resolves to nothing is an error, as are a tag that
//! names a class or attribute the description does not have, an
//! `enumDef` whose number of values differs from the enumerators, and a
//! `templateDef` or `enumDef` that no member of the description uses
//! (the forward valuetype a `templateDef` defines is then not in the
//! description). The `itemType` of an unused `templateDef` is resolved
//! first, so that one naming nothing is reported as such.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::mapping::{Mapping, MappingError};
use crate::model::{
    BasicType, ClassModel, EnumerationType, Model, ModelError, Relation, SimpleStructType,
    SimpleType, StructField,
};
use crate::model_tags::{EnumDef, ModelTags, RelationRef, TemplateDef};

/// The type of a member of a valuetype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    /// A basic type.
    Basic(BasicType),
    /// An enumeration.
    Enumeration(EnumerationType),
    /// A simple structure.
    Struct(SimpleStructType),
    /// A valuetype, by its scoped name: a class of the description, or a
    /// forward valuetype a `templateDef` defines.
    Valuetype(String),
}

/// A public state member of a valuetype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDecl {
    /// The member name.
    pub name: String,
    /// Its type.
    pub ty: TypeRef,
}

/// A valuetype of the model description with its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuetypeDecl {
    /// Scoped name.
    pub name: String,
    /// The inherited valuetypes: `ObjectRoot` (any spelling of
    /// [`crate::model::OBJECT_ROOT_NAMES`]) or a class of the
    /// description, plus native bases.
    pub bases: Vec<String>,
    /// The public state members, in declaration order.
    pub members: Vec<MemberDecl>,
}

/// The model description (§8.2.2.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelDescription {
    /// The valuetypes with a body, in declaration order.
    pub valuetypes: Vec<ValuetypeDecl>,
    /// The enumerations and simple structures of the description, by
    /// which a `templateDef` may name its `itemType`.
    pub types: Vec<SimpleType>,
}

/// Why the generator refuses its input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationError {
    /// A construct of the description or the tags that does not resolve.
    Unresolved(String),
    /// The resulting model breaks a rule of §8.1.3.
    Model(ModelError),
    /// The mapping breaks a rule of §8.1.4.
    Mapping(MappingError),
}

impl core::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unresolved(m) => write!(f, "DLRL generation: {m}"),
            Self::Model(e) => write!(f, "DLRL generation: {e}"),
            Self::Mapping(e) => write!(f, "DLRL generation: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GenerationError {}

fn unresolved(m: impl Into<String>) -> GenerationError {
    GenerationError::Unresolved(m.into())
}

/// Finds by exact name, else by the one name equal but for case.
fn find_by_name<'a, T>(items: &'a [T], name: &str, key: impl Fn(&T) -> &str) -> Option<&'a T> {
    if let Some(t) = items.iter().find(|t| key(t) == name) {
        return Some(t);
    }
    let mut folded = items.iter().filter(|t| key(t).eq_ignore_ascii_case(name));
    match (folded.next(), folded.next()) {
        (Some(t), None) => Some(t),
        _ => None,
    }
}

/// The basic types by their IDL names (`real` is the name of §8.1.3.3).
fn basic_by_name(name: &str) -> Option<BasicType> {
    Some(match name {
        "long" => BasicType::Long,
        "short" => BasicType::Short,
        "char" => BasicType::Char,
        "octet" => BasicType::Octet,
        "float" | "real" => BasicType::Real,
        "double" => BasicType::Double,
        "string" => BasicType::String,
        _ => return None,
    })
}

/// Codes `ty` and the enumerations inside it by their `enumDef`.
///
/// zerodds-lint: recursion-depth 64 (structure nesting, checked on entry)
fn coded(
    ty: &SimpleType,
    enums: &[EnumDef],
    used: &mut Vec<String>,
    depth: usize,
) -> Result<SimpleType, GenerationError> {
    if depth > 64 {
        return Err(unresolved("structures nested deeper than 64"));
    }
    Ok(match ty {
        SimpleType::Basic(b) => SimpleType::Basic(*b),
        SimpleType::Enumeration(e) => match find_by_name(enums, &e.name, |d| d.name.as_str()) {
            None => SimpleType::Enumeration(e.clone()),
            Some(d) => {
                if d.values.len() != e.enumerators.len() {
                    return Err(unresolved(format!(
                        "enumDef {} gives {} values, the enumeration has {} enumerators",
                        d.name,
                        d.values.len(),
                        e.enumerators.len()
                    )));
                }
                used.push(d.name.clone());
                let names: Vec<&str> = d.values.iter().map(String::as_str).collect();
                SimpleType::Enumeration(e.clone().coded_as(&names))
            }
        },
        SimpleType::Struct(s) => SimpleType::Struct(SimpleStructType {
            name: s.name.clone(),
            fields: s
                .fields
                .iter()
                .map(|f| {
                    Ok(StructField {
                        name: f.name.clone(),
                        ty: coded(&f.ty, enums, used, depth + 1)?,
                    })
                })
                .collect::<Result<_, GenerationError>>()?,
        }),
    })
}

/// What the `itemType` of a `templateDef` designates.
enum Item {
    /// A class of the description: the collection is a to-many relation.
    Class(String),
    /// A basic type, an enumeration or a structure of the description:
    /// the collection is a multi-valued attribute.
    Value(SimpleType),
}

/// The `itemType` of `t` resolved against the description: a class, a
/// basic type, or an enumeration or structure of the description.
fn item_of(
    t: &TemplateDef,
    class_names: &[&str],
    desc: &ModelDescription,
    enums: &[EnumDef],
    used_enums: &mut Vec<String>,
) -> Result<Item, GenerationError> {
    let item = t.item_type.as_str();
    if class_names.contains(&item) {
        Ok(Item::Class(item.into()))
    } else if let Some(b) = basic_by_name(item) {
        Ok(Item::Value(SimpleType::Basic(b)))
    } else if let Some(ty) = find_by_name(&desc.types, item, |ty| match ty {
        SimpleType::Enumeration(e) => e.name.as_str(),
        SimpleType::Struct(s) => s.name.as_str(),
        SimpleType::Basic(_) => "",
    }) {
        Ok(Item::Value(coded(ty, enums, used_enums, 0)?))
    } else {
        Err(unresolved(format!(
            "templateDef {}: itemType {item} is no class, basic type, enumeration or structure of the description",
            t.name
        )))
    }
}

fn find_relation<'a>(
    classes: &'a mut [ClassModel],
    r: &RelationRef,
    tag: &str,
) -> Result<&'a mut Relation, GenerationError> {
    let class = classes
        .iter_mut()
        .find(|c| c.name == r.class)
        .ok_or_else(|| unresolved(format!("{tag}: no class {}", r.class)))?;
    class
        .relations
        .iter_mut()
        .find(|x| x.name == r.attribute)
        .ok_or_else(|| {
            unresolved(format!(
                "{tag}: {} is no relation of class {}",
                r.attribute, r.class
            ))
        })
}

/// The model and its mapping from the model description and the model
/// tags (§8.2.2.1).
///
/// # Errors
/// [`GenerationError::Unresolved`] for a member type, `templateDef`,
/// `itemType`, `enumDef`, `local`, `compoRelationDef` or `associationDef`
/// that does not resolve against the description;
/// [`GenerationError::Model`] and [`GenerationError::Mapping`] when the
/// result breaks a rule of §8.1.3 or §8.1.4.
pub fn generate(desc: &ModelDescription, tags: &ModelTags) -> Result<Mapping, GenerationError> {
    let class_names: Vec<&str> = desc.valuetypes.iter().map(|v| v.name.as_str()).collect();
    for t in &tags.templates {
        if class_names.contains(&t.name.as_str()) {
            return Err(unresolved(format!(
                "templateDef {} names a valuetype with a body; it defines a forward valuetype",
                t.name
            )));
        }
    }
    let mut used_enums: Vec<String> = Vec::new();
    let mut used_templates: Vec<String> = Vec::new();
    let mut used_locals = alloc::vec![false; tags.locals.len()];
    let mut classes: Vec<ClassModel> = Vec::new();
    for v in &desc.valuetypes {
        let mut c = ClassModel::new(&v.name);
        for b in &v.bases {
            c = c.base(b);
        }
        for m in &v.members {
            if let Some(i) = tags
                .locals
                .iter()
                .position(|l| l.class == v.name && l.attribute == m.name)
            {
                used_locals[i] = true;
                c = c.local(&m.name);
                continue;
            }
            c = match &m.ty {
                TypeRef::Basic(b) => c.mono(&m.name, SimpleType::Basic(*b)),
                TypeRef::Enumeration(e) => c.mono(
                    &m.name,
                    coded(
                        &SimpleType::Enumeration(e.clone()),
                        &tags.enums,
                        &mut used_enums,
                        0,
                    )?,
                ),
                TypeRef::Struct(s) => c.mono(
                    &m.name,
                    coded(
                        &SimpleType::Struct(s.clone()),
                        &tags.enums,
                        &mut used_enums,
                        0,
                    )?,
                ),
                TypeRef::Valuetype(n) if class_names.contains(&n.as_str()) => {
                    c.relation(Relation::to_one(&m.name, n))
                }
                TypeRef::Valuetype(n) => {
                    let t: &TemplateDef = find_by_name(&tags.templates, n, |t| t.name.as_str())
                        .ok_or_else(|| {
                            unresolved(format!(
                                "{}::{}: {n} is neither a valuetype of the description nor defined by a templateDef",
                                v.name, m.name
                            ))
                        })?;
                    used_templates.push(t.name.clone());
                    match item_of(t, &class_names, desc, &tags.enums, &mut used_enums)? {
                        Item::Class(target) => {
                            c.relation(Relation::to_many(&m.name, &target, t.pattern))
                        }
                        Item::Value(ty) => c.multi(&m.name, t.pattern, ty),
                    }
                }
            };
        }
        classes.push(c);
    }
    if let Some(i) = used_locals.iter().position(|u| !u) {
        let l = &tags.locals[i];
        return Err(unresolved(format!(
            "local {} of classMapping {}: the description has no such member",
            l.attribute, l.class
        )));
    }
    for t in &tags.templates {
        if !used_templates.contains(&t.name) {
            // The itemType first, so that an unresolvable one is named.
            item_of(t, &class_names, desc, &tags.enums, &mut used_enums)?;
            return Err(unresolved(format!(
                "templateDef {}: no member of the description has that type",
                t.name
            )));
        }
    }
    for d in &tags.enums {
        if !used_enums.contains(&d.name) {
            return Err(unresolved(format!(
                "enumDef {}: no enumeration of that name is used by the description",
                d.name
            )));
        }
    }
    for r in &tags.compositions {
        find_relation(&mut classes, r, "compoRelationDef")?.is_composition = true;
    }
    for [a, b] in &tags.associations {
        find_relation(&mut classes, a, "associationDef")?.inverse = Some(b.attribute.clone());
        find_relation(&mut classes, b, "associationDef")?.inverse = Some(a.attribute.clone());
    }
    let model = Model::new(classes).map_err(GenerationError::Model)?;
    Mapping::new(model, &tags.class_mappings).map_err(GenerationError::Mapping)
}
