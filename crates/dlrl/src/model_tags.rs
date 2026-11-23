// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The model tags of the generation process (DDS 1.2 §8.2.2.3): an XML
//! document under the DTD of §8.2.2.3.1, read into [`ModelTags`].
//!
//! The reader checks the structure the DTD declares and refuses every
//! deviation with the path of the element ([`ModelTagsError`]):
//!
//! * the root `Dlrl` (attribute `name` implied) holds `enumDef`,
//!   `templateDef`, `associationDef`, `compoRelationDef` and
//!   `classMapping` in any order and number;
//! * `enumDef` (`name`) holds `value` elements; `templateDef` is empty
//!   with `name`, `pattern` (`List`, `StrMap`, `IntMap`, `Set`) and
//!   `itemType`; `associationDef` holds exactly two empty `relation`
//!   elements with `class` and `attribute`; `compoRelationDef` is empty
//!   with `class` and `attribute`;
//! * `classMapping` (`name`) holds `mainTopic?`, `extensionTopic?` in this
//!   order, then `monoAttribute`, `multiAttribute`, `monoRelation`,
//!   `multiRelation` and `local` in any order;
//! * `mainTopic`, `extensionTopic` and `placeTopic` (`name`, `typename`
//!   implied) and `multiPlaceTopic` (also `indexField` implied) hold one
//!   `keyDescription`; `monoAttribute` holds `placeTopic?, valueField+`,
//!   `multiAttribute` `multiPlaceTopic, valueField+`, `monoRelation`
//!   `placeTopic?, keyDescription`, `multiRelation`
//!   `multiPlaceTopic, keyDescription`; `local` is empty with `name`;
//! * `keyDescription` (`content` one of `FullOid`, `SimpleOid`, `NoOid`)
//!   holds `keyField` elements: two for `FullOid`, one for `SimpleOid`,
//!   one or more for `NoOid` (§8.2.2.3.2.8);
//! * `value`, `keyField` and `valueField` hold text only;
//! * an attribute the DTD does not declare for the element, a missing
//!   required attribute, an element or text where the DTD has none, all
//!   are errors.
//!
//! The document may declare the encoding ISO-8859-1 (the DTD and the
//! example of §8.2.3.3 do) or UTF-8, and may carry the DOCTYPE line of
//! §8.2.3.3; the external DTD it names is not fetched, the checks above
//! stand for it. What the tags mean for the model is applied in
//! [`crate::generation`].

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_xml::{XmlElement, parse_xml_tree_bytes};

use crate::mapping::{
    ClassMappingSpec, KeyContent, KeyDescription, MemberMappingSpec, MultiTopicSpec, TopicSpec,
};
use crate::model::CollectionBasis;

/// A violation of the DTD of §8.2.2.3.1, or XML that is not well-formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTagsError {
    /// The path of the element (`Dlrl[example]/classMapping[Track]/…`),
    /// empty for an error of the document as a whole.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl core::fmt::Display for ModelTagsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.path.is_empty() {
            write!(f, "model tags: {}", self.message)
        } else {
            write!(f, "model tags, {}: {}", self.path, self.message)
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ModelTagsError {}

/// `enumDef` (§8.2.2.3.2.2): the names of the items of an enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    /// Scoped name of the IDL enumeration.
    pub name: String,
    /// The names, in the order of the enumerators.
    pub values: Vec<String>,
}

/// `templateDef` (§8.2.2.3.2.3): a typed collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDef {
    /// Scoped name of the type (the forward valuetype of the model
    /// description).
    pub name: String,
    /// The pattern.
    pub pattern: CollectionBasis,
    /// The type of each element: a class, a basic type or a simple type
    /// of the model description.
    pub item_type: String,
}

/// One end of an `associationDef` or a `compoRelationDef`: a class and
/// the attribute that supports the relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRef {
    /// Scoped name of the class.
    pub class: String,
    /// The attribute.
    pub attribute: String,
}

/// `local` (§8.2.2.3.2.14) inside the `classMapping` of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDef {
    /// The class.
    pub class: String,
    /// The attribute the service ignores.
    pub attribute: String,
}

/// The content of a model tags document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelTags {
    /// The `name` attribute of `Dlrl`.
    pub name: Option<String>,
    /// `enumDef` elements.
    pub enums: Vec<EnumDef>,
    /// `templateDef` elements.
    pub templates: Vec<TemplateDef>,
    /// `associationDef` elements: the two ends.
    pub associations: Vec<[RelationRef; 2]>,
    /// `compoRelationDef` elements.
    pub compositions: Vec<RelationRef>,
    /// `classMapping` elements, without their `local` sub-tags.
    pub class_mappings: Vec<ClassMappingSpec>,
    /// The `local` sub-tags of all `classMapping` elements.
    pub locals: Vec<LocalDef>,
}

fn err(e: &XmlElement, message: impl Into<String>) -> ModelTagsError {
    ModelTagsError {
        path: e.path.clone(),
        message: message.into(),
    }
}

/// Reads a model tags document from its bytes.
///
/// # Errors
/// [`ModelTagsError`] for XML that is not well-formed, an encoding other
/// than UTF-8 or ISO-8859-1, and every deviation from the DTD of
/// §8.2.2.3.1.
pub fn parse_model_tags(bytes: &[u8]) -> Result<ModelTags, ModelTagsError> {
    let (doc, _) = parse_xml_tree_bytes(bytes).map_err(|e| ModelTagsError {
        path: String::new(),
        message: format!("{e}"),
    })?;
    let root = &doc.root;
    if root.name != "Dlrl" || root.namespace.is_some() {
        return Err(err(root, "the root element is not Dlrl"));
    }
    attributes(root, &[], &["name"])?;
    no_text(root)?;
    let mut tags = ModelTags {
        name: root.attribute("name").map(String::from),
        ..ModelTags::default()
    };
    for c in &root.children {
        match c.name.as_str() {
            "enumDef" => tags.enums.push(enum_def(c)?),
            "templateDef" => tags.templates.push(template_def(c)?),
            "associationDef" => {
                attributes(c, &[], &[])?;
                no_text(c)?;
                let ends: Vec<RelationRef> = c
                    .children
                    .iter()
                    .map(|r| {
                        if r.name != "relation" {
                            return Err(err(r, "associationDef holds relation elements only"));
                        }
                        relation_ref(r)
                    })
                    .collect::<Result<_, _>>()?;
                let [a, b]: [RelationRef; 2] = ends
                    .try_into()
                    .map_err(|_| err(c, "associationDef holds exactly two relation elements"))?;
                tags.associations.push([a, b]);
            }
            "compoRelationDef" => tags.compositions.push(relation_ref(c)?),
            "classMapping" => {
                let (spec, locals) = class_mapping(c)?;
                tags.class_mappings.push(spec);
                tags.locals.extend(locals);
            }
            other => {
                return Err(err(c, format!("<{other}> is not an element of Dlrl")));
            }
        }
    }
    Ok(tags)
}

/// Checks the attributes of `e`: each of `required` present, none
/// outside `required` and `implied`.
fn attributes(e: &XmlElement, required: &[&str], implied: &[&str]) -> Result<(), ModelTagsError> {
    for r in required {
        if e.attribute(r).is_none() {
            return Err(err(e, format!("the required attribute {r} is missing")));
        }
    }
    for a in e.attributes.keys() {
        if !required.contains(&a.as_str()) && !implied.contains(&a.as_str()) {
            return Err(err(e, format!("<{}> has no attribute {a}", e.name)));
        }
    }
    Ok(())
}

fn no_text(e: &XmlElement) -> Result<(), ModelTagsError> {
    if e.text.trim().is_empty() {
        Ok(())
    } else {
        Err(err(e, format!("<{}> holds no text", e.name)))
    }
}

fn empty(e: &XmlElement) -> Result<(), ModelTagsError> {
    no_text(e)?;
    match e.children.first() {
        None => Ok(()),
        Some(c) => Err(err(c, format!("<{}> is EMPTY", e.name))),
    }
}

/// The text of an element of content `#PCDATA`.
fn pcdata(e: &XmlElement) -> Result<String, ModelTagsError> {
    attributes(e, &[], &[])?;
    if let Some(c) = e.children.first() {
        return Err(err(c, format!("<{}> holds text only", e.name)));
    }
    let t = e.text.trim();
    if t.is_empty() {
        return Err(err(e, format!("<{}> is empty", e.name)));
    }
    Ok(t.into())
}

fn req<'a>(e: &'a XmlElement, name: &str) -> Result<&'a str, ModelTagsError> {
    e.attribute(name)
        .ok_or_else(|| err(e, format!("the required attribute {name} is missing")))
}

fn enum_def(e: &XmlElement) -> Result<EnumDef, ModelTagsError> {
    attributes(e, &["name"], &[])?;
    no_text(e)?;
    let values = e
        .children
        .iter()
        .map(|v| {
            if v.name != "value" {
                return Err(err(v, "enumDef holds value elements only"));
            }
            pcdata(v)
        })
        .collect::<Result<_, _>>()?;
    Ok(EnumDef {
        name: req(e, "name")?.into(),
        values,
    })
}

fn template_def(e: &XmlElement) -> Result<TemplateDef, ModelTagsError> {
    attributes(e, &["name", "pattern", "itemType"], &[])?;
    empty(e)?;
    let pattern = match req(e, "pattern")? {
        "List" => CollectionBasis::List,
        "StrMap" => CollectionBasis::StrMap,
        "IntMap" => CollectionBasis::IntMap,
        "Set" => CollectionBasis::Set,
        other => {
            return Err(err(
                e,
                format!("pattern {other} is not one of List, StrMap, IntMap, Set"),
            ));
        }
    };
    Ok(TemplateDef {
        name: req(e, "name")?.into(),
        pattern,
        item_type: req(e, "itemType")?.into(),
    })
}

fn relation_ref(e: &XmlElement) -> Result<RelationRef, ModelTagsError> {
    attributes(e, &["class", "attribute"], &[])?;
    empty(e)?;
    Ok(RelationRef {
        class: req(e, "class")?.into(),
        attribute: req(e, "attribute")?.into(),
    })
}

fn key_description(e: &XmlElement) -> Result<KeyDescription, ModelTagsError> {
    if e.name != "keyDescription" {
        return Err(err(
            e,
            format!("<{}> where keyDescription is required", e.name),
        ));
    }
    attributes(e, &["content"], &[])?;
    no_text(e)?;
    let content = match req(e, "content")? {
        "FullOid" => KeyContent::FullOid,
        "SimpleOid" => KeyContent::SimpleOid,
        "NoOid" => KeyContent::NoOid,
        other => {
            return Err(err(
                e,
                format!("content {other} is not one of FullOid, SimpleOid, NoOid"),
            ));
        }
    };
    let fields: Vec<String> = e
        .children
        .iter()
        .map(|k| {
            if k.name != "keyField" {
                return Err(err(k, "keyDescription holds keyField elements only"));
            }
            pcdata(k)
        })
        .collect::<Result<_, _>>()?;
    let fits = match content {
        KeyContent::FullOid => fields.len() == 2,
        KeyContent::SimpleOid => fields.len() == 1,
        KeyContent::NoOid => !fields.is_empty(),
    };
    if !fits {
        return Err(err(
            e,
            format!(
                "content {} needs {} keyField, {} given",
                content.name(),
                match content {
                    KeyContent::FullOid => "two",
                    KeyContent::SimpleOid => "one",
                    KeyContent::NoOid => "at least one",
                },
                fields.len()
            ),
        ));
    }
    Ok(KeyDescription { content, fields })
}

/// `mainTopic`, `extensionTopic`, `placeTopic`.
fn topic(e: &XmlElement) -> Result<TopicSpec, ModelTagsError> {
    attributes(e, &["name"], &["typename"])?;
    no_text(e)?;
    let key = single_key(e)?;
    Ok(TopicSpec {
        name: req(e, "name")?.into(),
        type_name: e.attribute("typename").map(String::from),
        key,
    })
}

fn multi_topic(e: &XmlElement) -> Result<MultiTopicSpec, ModelTagsError> {
    attributes(e, &["name"], &["typename", "indexField"])?;
    no_text(e)?;
    let key = single_key(e)?;
    Ok(MultiTopicSpec {
        name: req(e, "name")?.into(),
        type_name: e.attribute("typename").map(String::from),
        index_field: e.attribute("indexField").map(String::from),
        key,
    })
}

fn single_key(e: &XmlElement) -> Result<KeyDescription, ModelTagsError> {
    match e.children.as_slice() {
        [k] => key_description(k),
        [] => Err(err(e, format!("<{}> needs a keyDescription", e.name))),
        [_, extra, ..] => Err(err(extra, format!("<{}> holds one keyDescription", e.name))),
    }
}

/// `valueField+` at the end of `children`, from `start`.
fn value_fields(e: &XmlElement, rest: &[XmlElement]) -> Result<Vec<String>, ModelTagsError> {
    if rest.is_empty() {
        return Err(err(
            e,
            format!("<{}> needs at least one valueField", e.name),
        ));
    }
    rest.iter()
        .map(|v| {
            if v.name != "valueField" {
                return Err(err(v, format!("<{}> where valueField is required", v.name)));
            }
            pcdata(v)
        })
        .collect()
}

fn class_mapping(e: &XmlElement) -> Result<(ClassMappingSpec, Vec<LocalDef>), ModelTagsError> {
    attributes(e, &["name"], &[])?;
    no_text(e)?;
    let class = req(e, "name")?;
    let mut spec = ClassMappingSpec::new(class);
    let mut locals = Vec::new();
    let mut i = 0;
    let ch = &e.children;
    if ch.get(i).is_some_and(|c| c.name == "mainTopic") {
        spec.main_topic = Some(topic(&ch[i])?);
        i += 1;
    }
    if ch.get(i).is_some_and(|c| c.name == "extensionTopic") {
        spec.extension_topic = Some(topic(&ch[i])?);
        i += 1;
    }
    for c in &ch[i..] {
        match c.name.as_str() {
            "monoAttribute" | "multiAttribute" | "monoRelation" | "multiRelation" | "local" => {}
            "mainTopic" | "extensionTopic" => {
                return Err(err(
                    c,
                    format!(
                        "<{}> comes before the attributes, mainTopic before extensionTopic",
                        c.name
                    ),
                ));
            }
            other => {
                return Err(err(
                    c,
                    format!("<{other}> is not an element of classMapping"),
                ));
            }
        }
        attributes(c, &["name"], &[])?;
        no_text(c)?;
        let name: String = req(c, "name")?.into();
        let sub = c.children.as_slice();
        match c.name.as_str() {
            "monoAttribute" => {
                let (place_topic, rest) = match sub.first() {
                    Some(p) if p.name == "placeTopic" => (Some(topic(p)?), &sub[1..]),
                    _ => (None, sub),
                };
                spec.members.push(MemberMappingSpec::MonoAttribute {
                    name,
                    place_topic,
                    value_fields: value_fields(c, rest)?,
                });
            }
            "multiAttribute" => {
                let Some(p) = sub.first().filter(|p| p.name == "multiPlaceTopic") else {
                    return Err(err(c, "multiAttribute needs a multiPlaceTopic first"));
                };
                spec.members.push(MemberMappingSpec::MultiAttribute {
                    name,
                    topic: multi_topic(p)?,
                    value_fields: value_fields(c, &sub[1..])?,
                });
            }
            "monoRelation" => {
                let (place_topic, rest) = match sub.first() {
                    Some(p) if p.name == "placeTopic" => (Some(topic(p)?), &sub[1..]),
                    _ => (None, sub),
                };
                let target_key = match rest {
                    [k] => key_description(k)?,
                    [] => return Err(err(c, "monoRelation needs a keyDescription")),
                    [_, extra, ..] => {
                        return Err(err(extra, "monoRelation ends with one keyDescription"));
                    }
                };
                spec.members.push(MemberMappingSpec::MonoRelation {
                    name,
                    place_topic,
                    target_key,
                });
            }
            "multiRelation" => {
                let [p, k] = sub else {
                    return Err(err(
                        c,
                        "multiRelation holds a multiPlaceTopic and a keyDescription",
                    ));
                };
                if p.name != "multiPlaceTopic" {
                    return Err(err(p, "multiRelation needs a multiPlaceTopic first"));
                }
                spec.members.push(MemberMappingSpec::MultiRelation {
                    name,
                    topic: multi_topic(p)?,
                    target_key: key_description(k)?,
                });
            }
            // `local`, the one name left after the check above.
            _ => {
                empty(c)?;
                locals.push(LocalDef {
                    class: class.into(),
                    attribute: name,
                });
            }
        }
    }
    Ok((spec, locals))
}
