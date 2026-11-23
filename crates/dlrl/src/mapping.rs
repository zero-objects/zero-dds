// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Structural mapping of a DLRL model onto DCPS topics (DDS 1.2 §8.1.4).
//!
//! The input is a checked [`Model`] and, per class, the mapping
//! information the application developer gives ([`ClassMappingSpec`],
//! the content of a `classMapping` element of §8.2.2.3). Every part that
//! is not given takes the default mapping of §8.1.4.3. The result,
//! [`Mapping`], holds per class the information of Figure 8.2
//! ([`ClassMapping`] with `main_topic`, `oid_field`, `class_field`,
//! `full_oid_required`, and per member `topic`, `key_fields`,
//! `target_field(s)`, `index_field`, `full_oid_required`,
//! `is_composition`) and the description of every DCPS topic the
//! mapping uses ([`TopicDescription`], the "DCPS description" of
//! §8.1.4.6).
//!
//! [`Mapping::new`] refuses a mapping that breaks a rule of §8.1.4:
//! key fields per key content (§8.2.2.3.2.8), a multi-valued member in
//! the owner's main topic (§8.1.4.4.3, §8.1.4.4.5), a class part missing
//! where several classes put rows into one topic or a relation targets a
//! class with derived classes (§8.1.4.2.2), value fields that do not
//! match the attribute type (§8.1.4.2.3), and two uses of one topic that
//! give it different keys, field types or type names.
//!
//! The rows of the tables (§8.1.4.2) are written and read in
//! [`crate::rows`].

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::model::{
    Attribute, AttributeType, BasicItem, BasicType, ClassModel, CollectionBasis, EnumCoding,
    Member, Model, Multiplicity, SimpleType,
};

/// §8.2.2.3.2.8 the content of a key description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyContent {
    /// Two key fields: the class name, then the oid (the full oid).
    FullOid,
    /// One key field: the oid.
    SimpleOid,
    /// Any number of key fields of an existing DCPS model; the
    /// association with the oid is kept by the implementation
    /// (§8.1.4.5, [`crate::rows::KeyAssociation`]). Each key field of the
    /// main topic is the value field of a mono-valued attribute of the
    /// class (its identity attributes, [`ClassMapping::identity`]); the
    /// key fields of the class's other topics and the target fields of a
    /// relation to the class hold the same values, in the same order.
    NoOid,
}

impl KeyContent {
    /// The name used in the DTD of §8.2.2.3.1.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FullOid => "FullOid",
            Self::SimpleOid => "SimpleOid",
            Self::NoOid => "NoOid",
        }
    }
}

/// §8.2.2.3.2.8 `keyDescription`: what the key fields hold, and their
/// names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyDescription {
    /// The content.
    pub content: KeyContent,
    /// The key field names, in order.
    pub fields: Vec<String>,
}

impl KeyDescription {
    /// A full oid key: class field, then oid field.
    #[must_use]
    pub fn full_oid(class_field: &str, oid_field: &str) -> Self {
        Self {
            content: KeyContent::FullOid,
            fields: alloc::vec![class_field.into(), oid_field.into()],
        }
    }

    /// A simple oid key.
    #[must_use]
    pub fn simple_oid(oid_field: &str) -> Self {
        Self {
            content: KeyContent::SimpleOid,
            fields: alloc::vec![oid_field.into()],
        }
    }

    /// A key of an existing DCPS model.
    #[must_use]
    pub fn no_oid(fields: &[&str]) -> Self {
        Self {
            content: KeyContent::NoOid,
            fields: fields.iter().map(|f| String::from(*f)).collect(),
        }
    }

    fn check(&self, context: &str) -> Result<(), MappingError> {
        let ok = match self.content {
            KeyContent::FullOid => self.fields.len() == 2,
            KeyContent::SimpleOid => self.fields.len() == 1,
            KeyContent::NoOid => !self.fields.is_empty(),
        };
        if !ok || self.fields.iter().any(String::is_empty) || has_duplicate(&self.fields) {
            return Err(MappingError::KeyFields {
                context: context.into(),
                content: self.content,
                fields: self.fields.clone(),
            });
        }
        Ok(())
    }

    /// The cell types of the key fields; for `NoOid` the cells of the
    /// identity attributes of the class the key designates.
    fn cells(&self, identity: &[CellType]) -> Vec<CellType> {
        match self.content {
            KeyContent::FullOid => alloc::vec![CellType::String, CellType::Oid],
            KeyContent::SimpleOid => alloc::vec![CellType::Oid],
            KeyContent::NoOid => identity.to_vec(),
        }
    }
}

/// `mainTopic`, `extensionTopic` or `placeTopic` as given.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicSpec {
    /// Topic name.
    pub name: String,
    /// Type name; absent means equal to the topic name (§8.2.2.3.2.7).
    pub type_name: Option<String>,
    /// The key description.
    pub key: KeyDescription,
}

impl TopicSpec {
    /// A topic whose type name is its name.
    #[must_use]
    pub fn new(name: &str, key: KeyDescription) -> Self {
        Self {
            name: name.into(),
            type_name: None,
            key,
        }
    }
}

/// `multiPlaceTopic` as given.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MultiTopicSpec {
    /// Topic name.
    pub name: String,
    /// Type name; absent means equal to the topic name.
    pub type_name: Option<String>,
    /// The field that holds the index; absent means `index` for a list
    /// or a map. A set has none (its items are designated by their
    /// value).
    pub index_field: Option<String>,
    /// The key description of the owner part.
    pub key: KeyDescription,
}

impl MultiTopicSpec {
    /// A collection topic.
    #[must_use]
    pub fn new(name: &str, index_field: Option<&str>, key: KeyDescription) -> Self {
        Self {
            name: name.into(),
            type_name: None,
            index_field: index_field.map(String::from),
            key,
        }
    }
}

/// The mapping of one member as given (§8.2.2.3.2.10 to .13).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MemberMappingSpec {
    /// `monoAttribute`.
    MonoAttribute {
        /// Attribute name.
        name: String,
        /// `placeTopic`; absent means the extension topic, or the main
        /// topic without one.
        place_topic: Option<TopicSpec>,
        /// `valueField+`.
        value_fields: Vec<String>,
    },
    /// `multiAttribute`.
    MultiAttribute {
        /// Attribute name.
        name: String,
        /// `multiPlaceTopic`.
        topic: MultiTopicSpec,
        /// `valueField+`.
        value_fields: Vec<String>,
    },
    /// `monoRelation`.
    MonoRelation {
        /// Relation name.
        name: String,
        /// `placeTopic`, as for a mono attribute.
        place_topic: Option<TopicSpec>,
        /// The fields that designate the target.
        target_key: KeyDescription,
    },
    /// `multiRelation`.
    MultiRelation {
        /// Relation name.
        name: String,
        /// `multiPlaceTopic`.
        topic: MultiTopicSpec,
        /// The fields that designate each target.
        target_key: KeyDescription,
    },
}

impl MemberMappingSpec {
    fn name(&self) -> &str {
        match self {
            Self::MonoAttribute { name, .. }
            | Self::MultiAttribute { name, .. }
            | Self::MonoRelation { name, .. }
            | Self::MultiRelation { name, .. } => name,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::MonoAttribute { .. } => "monoAttribute",
            Self::MultiAttribute { .. } => "multiAttribute",
            Self::MonoRelation { .. } => "monoRelation",
            Self::MultiRelation { .. } => "multiRelation",
        }
    }
}

/// `classMapping` as given: every part is optional and defaults to
/// §8.1.4.3.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClassMappingSpec {
    /// Class name.
    pub class: String,
    /// `mainTopic`.
    pub main_topic: Option<TopicSpec>,
    /// `extensionTopic`.
    pub extension_topic: Option<TopicSpec>,
    /// Mappings of members declared by this class.
    pub members: Vec<MemberMappingSpec>,
}

impl ClassMappingSpec {
    /// An empty mapping for a class.
    #[must_use]
    pub fn new(class: &str) -> Self {
        Self {
            class: class.into(),
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Resolved mapping (Figure 8.2)
// ---------------------------------------------------------------------------

/// A topic as used by a class or member.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Topic {
    /// Topic name.
    pub name: String,
    /// Type name.
    pub type_name: String,
    /// The key description.
    pub key: KeyDescription,
}

impl Topic {
    fn from_spec(s: &TopicSpec) -> Self {
        Self {
            name: s.name.clone(),
            type_name: s.type_name.clone().unwrap_or_else(|| s.name.clone()),
            key: s.key.clone(),
        }
    }

    fn named(name: String, key: KeyDescription) -> Self {
        Self {
            type_name: name.clone(),
            name,
            key,
        }
    }
}

/// §8.1.4.4.2 `MonoAttribute` with mapping information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoAttributeMapping {
    /// Attribute name.
    pub attribute: String,
    /// `topic` with its key.
    pub topic: Topic,
    /// `target_field`: one field, or one per leaf of a simple structure.
    pub value_fields: Vec<String>,
}

/// §8.1.4.4.3 `MultiAttribute` with mapping information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MultiAttributeMapping {
    /// Attribute name.
    pub attribute: String,
    /// Collection basis.
    pub basis: CollectionBasis,
    /// `topic` with the owner part of its key.
    pub topic: Topic,
    /// `index_field`; `None` for a set.
    pub index_field: Option<String>,
    /// `target_field`(s) of one item.
    pub value_fields: Vec<String>,
}

/// §8.1.4.4.4 `MonoRelation` with mapping information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoRelationMapping {
    /// Relation name.
    pub relation: String,
    /// `topic` with its key.
    pub topic: Topic,
    /// `target_fields` and their content.
    pub target_key: KeyDescription,
    /// `is_composition`.
    pub is_composition: bool,
}

/// §8.1.4.4.5 `MultiRelation` with mapping information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MultiRelationMapping {
    /// Relation name.
    pub relation: String,
    /// Collection basis.
    pub basis: CollectionBasis,
    /// `topic` with the owner part of its key.
    pub topic: Topic,
    /// `index_field`; `None` for a set.
    pub index_field: Option<String>,
    /// `target_fields` of one item and their content.
    pub target_key: KeyDescription,
    /// `is_composition`.
    pub is_composition: bool,
}

/// The mapping of one member.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MemberMapping {
    /// A mono-valued attribute.
    MonoAttribute(MonoAttributeMapping),
    /// A multi-valued attribute.
    MultiAttribute(MultiAttributeMapping),
    /// A to-1 relation.
    MonoRelation(MonoRelationMapping),
    /// A to-many relation.
    MultiRelation(MultiRelationMapping),
}

impl MemberMapping {
    /// The member name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::MonoAttribute(m) => &m.attribute,
            Self::MultiAttribute(m) => &m.attribute,
            Self::MonoRelation(m) => &m.relation,
            Self::MultiRelation(m) => &m.relation,
        }
    }

    /// The topic the member is placed in.
    #[must_use]
    pub fn topic(&self) -> &Topic {
        match self {
            Self::MonoAttribute(m) => &m.topic,
            Self::MultiAttribute(m) => &m.topic,
            Self::MonoRelation(m) => &m.topic,
            Self::MultiRelation(m) => &m.topic,
        }
    }

    /// `key_fields`: the owner part of the key in the member's topic.
    #[must_use]
    pub fn key_fields(&self) -> &[String] {
        &self.topic().key.fields
    }

    /// True for a multi-valued member (one row per item).
    #[must_use]
    pub fn is_multi(&self) -> bool {
        matches!(self, Self::MultiAttribute(_) | Self::MultiRelation(_))
    }

    /// `index_field` of a multi-valued member.
    #[must_use]
    pub fn index_field(&self) -> Option<&str> {
        match self {
            Self::MultiAttribute(m) => m.index_field.as_deref(),
            Self::MultiRelation(m) => m.index_field.as_deref(),
            _ => None,
        }
    }

    /// `target_field(s)`: the fields of the value, or of the reference
    /// to the target.
    #[must_use]
    pub fn target_fields(&self) -> &[String] {
        match self {
            Self::MonoAttribute(m) => &m.value_fields,
            Self::MultiAttribute(m) => &m.value_fields,
            Self::MonoRelation(m) => &m.target_key.fields,
            Self::MultiRelation(m) => &m.target_key.fields,
        }
    }

    /// `full_oid_required` of a relation.
    #[must_use]
    pub fn full_oid_required(&self) -> bool {
        match self {
            Self::MonoRelation(m) => m.target_key.content == KeyContent::FullOid,
            Self::MultiRelation(m) => m.target_key.content == KeyContent::FullOid,
            _ => false,
        }
    }
}

/// §8.1.4.4.1 `Class` with mapping information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassMapping {
    /// Class name.
    pub class: String,
    /// `main_topic`: a row in it makes the object exist.
    pub main_topic: Topic,
    /// The extension topic, if any.
    pub extension_topic: Option<Topic>,
    /// The members declared by this class (inherited members are mapped
    /// by the class that declares them).
    pub members: Vec<MemberMapping>,
    /// For a `NoOid` main topic (§8.1.4.5): per key field, in order, the
    /// mono-valued attribute whose value field it is. The values of these
    /// attributes are the identity of an object; empty for a key that
    /// stores the oid.
    pub identity: Vec<String>,
}

impl ClassMapping {
    /// `full_oid_required`: the key is `(class_field, oid_field)`.
    #[must_use]
    pub fn full_oid_required(&self) -> bool {
        self.main_topic.key.content == KeyContent::FullOid
    }

    /// `oid_field`; `None` for a `NoOid` key.
    #[must_use]
    pub fn oid_field(&self) -> Option<&str> {
        let k = &self.main_topic.key;
        match k.content {
            KeyContent::FullOid => k.fields.get(1).map(String::as_str),
            KeyContent::SimpleOid => k.fields.first().map(String::as_str),
            KeyContent::NoOid => None,
        }
    }

    /// `class_field`; `None` unless the key is a full oid.
    #[must_use]
    pub fn class_field(&self) -> Option<&str> {
        let k = &self.main_topic.key;
        match k.content {
            KeyContent::FullOid => k.fields.first().map(String::as_str),
            _ => None,
        }
    }

    /// A member by name.
    #[must_use]
    pub fn member(&self, name: &str) -> Option<&MemberMapping> {
        self.members.iter().find(|m| m.name() == name)
    }
}

/// The DCPS type of one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellType {
    /// `long`; also an enumeration coded as integer and a list or
    /// `IntMap` index.
    Long,
    /// `short`.
    Short,
    /// `char`.
    Char,
    /// `octet`.
    Octet,
    /// `float`.
    Real,
    /// `double`.
    Double,
    /// `string`; also a class name, an enumeration coded as string and
    /// a `StrMap` index.
    String,
    /// `sequence<item>`.
    Sequence(BasicItem),
    /// `DLRLOid`.
    Oid,
}

/// One field of a topic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldDescription {
    /// Field name.
    pub name: String,
    /// Field type.
    pub cell: CellType,
    /// Part of the key.
    pub key: bool,
}

/// One DCPS topic as the mapping uses it: the DCPS description of
/// §8.1.4.6.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicDescription {
    /// Topic name.
    pub name: String,
    /// Type name.
    pub type_name: String,
    /// The content of the owner part of the key.
    pub key_content: KeyContent,
    /// Key fields first, then the other fields.
    pub fields: Vec<FieldDescription>,
    /// The classes whose objects have rows in this topic.
    pub classes: Vec<String>,
}

impl TopicDescription {
    /// A field by name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&FieldDescription> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The names of the key fields.
    #[must_use]
    pub fn key_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.key)
            .map(|f| f.name.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A mapping that breaks a rule of §8.1.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    /// A `classMapping` for a class that is not in the model.
    UnknownClass(String),
    /// Two `classMapping` elements for one class.
    DuplicateClassMapping(String),
    /// A member mapping for a member the class does not declare.
    UnknownMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// A member mapping for an inherited member; the declaring class
    /// maps it.
    InheritedMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// A member mapping for a local attribute (§8.2.2.3.2.14).
    LocalMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// The element does not fit the member (e.g. `monoAttribute` for a
    /// relation).
    MemberKind {
        /// The class.
        class: String,
        /// The member.
        member: String,
        /// The element given.
        given: &'static str,
        /// The element the member needs.
        expected: &'static str,
    },
    /// Two mappings for one member.
    DuplicateMemberMapping {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// §8.2.2.3.2.8: FullOid needs two key fields, SimpleOid one, NoOid
    /// at least one; names are not empty and not repeated.
    KeyFields {
        /// Where.
        context: String,
        /// The content.
        content: KeyContent,
        /// The fields given.
        fields: Vec<String>,
    },
    /// §8.1.4.4.2 to .5: the key of a place, extension or collection
    /// topic designates the owner as the main topic does (same content,
    /// same number of fields).
    OwnerKeyMismatch {
        /// The class.
        class: String,
        /// The topic.
        topic: String,
    },
    /// The extension topic is the main topic.
    ExtensionIsMainTopic {
        /// The class.
        class: String,
    },
    /// §8.1.4.4.3, §8.1.4.4.5: the topic of a multi-valued member
    /// "cannot be the same as the owner Class::topic"; it holds the rows
    /// of that member only.
    CollectionTopic {
        /// The class.
        class: String,
        /// The member.
        member: String,
        /// The topic.
        topic: String,
    },
    /// A set has no index field; a list or a map has one.
    IndexField {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// §8.1.4.2.3: the value fields do not match the type (one cell per
    /// basic or enumeration value, one per leaf of a simple structure).
    ValueFields {
        /// The class.
        class: String,
        /// The member.
        member: String,
        /// Number of cells the type needs.
        expected: usize,
        /// Number of fields given.
        given: usize,
    },
    /// §8.1.4.2.3: one value field named twice; each cell of the type
    /// has a field of its own.
    RepeatedValueField {
        /// The class.
        class: String,
        /// The member.
        member: String,
        /// The repeated field.
        field: String,
    },
    /// §8.1.4.2.2: the target fields cannot designate the target (a
    /// class part is needed, or the target uses a `NoOid` key and the
    /// relation does not, or the other way round).
    TargetKey {
        /// The class.
        class: String,
        /// The relation.
        relation: String,
        /// What is wrong.
        reason: String,
    },
    /// §8.1.4.2.2 "A class name is needed when several classes share
    /// the same main table": objects of several classes have rows in
    /// this topic and its key has no class part.
    ClassPartRequired {
        /// The topic.
        topic: String,
        /// The classes with rows in it.
        classes: Vec<String>,
    },
    /// Two uses of one topic give it different keys.
    KeyConflict {
        /// The topic.
        topic: String,
    },
    /// Two uses of one topic give it different type names.
    TypeNameConflict {
        /// The topic.
        topic: String,
    },
    /// A field of one topic with two types, a key field used as value
    /// field, or an index or set item field that repeats a key field.
    FieldConflict {
        /// The topic.
        topic: String,
        /// The field.
        field: String,
    },
    /// Two members write the same field of one topic.
    FieldUsedTwice {
        /// The topic.
        topic: String,
        /// The field.
        field: String,
    },
    /// A topic, type or field without a name.
    EmptyName {
        /// Where.
        context: String,
    },
    /// A key field of a `NoOid` main topic that is not the one value
    /// field of a mono-valued attribute of the class mapped into that
    /// topic, or whose attribute is of a structure or sequence type
    /// (§8.1.4.5: the key fields are what identifies the objects).
    NoOidKeyField {
        /// The class.
        class: String,
        /// The key field.
        field: String,
    },
}

impl core::fmt::Display for MappingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownClass(c) => write!(f, "classMapping {c}: no such class in the model"),
            Self::DuplicateClassMapping(c) => write!(f, "class {c} is mapped twice"),
            Self::UnknownMember { class, member } => {
                write!(f, "class {class} declares no member {member}")
            }
            Self::InheritedMember { class, member } => write!(
                f,
                "member {member} is inherited by {class}; the class that declares it maps it"
            ),
            Self::LocalMember { class, member } => {
                write!(f, "member {class}::{member} is local and has no mapping")
            }
            Self::MemberKind {
                class,
                member,
                given,
                expected,
            } => write!(
                f,
                "member {class}::{member} is mapped as {given}, it needs {expected}"
            ),
            Self::DuplicateMemberMapping { class, member } => {
                write!(f, "member {class}::{member} is mapped twice")
            }
            Self::KeyFields {
                context,
                content,
                fields,
            } => write!(
                f,
                "{context}: a {} key description takes {} key fields with distinct names, given {fields:?} (DDS 1.2 §8.2.2.3.2.8)",
                content.name(),
                match content {
                    KeyContent::FullOid => "two",
                    KeyContent::SimpleOid => "one",
                    KeyContent::NoOid => "at least one",
                }
            ),
            Self::OwnerKeyMismatch { class, topic } => write!(
                f,
                "topic {topic} of class {class} designates the owner with another key content or number of fields than the main topic (DDS 1.2 §8.1.4.4)"
            ),
            Self::ExtensionIsMainTopic { class } => {
                write!(f, "class {class}: the extension topic is the main topic")
            }
            Self::CollectionTopic {
                class,
                member,
                topic,
            } => write!(
                f,
                "collection {class}::{member} is placed in {topic}, which is a topic of the owner class or of another member (DDS 1.2 §8.1.4.4.3)"
            ),
            Self::IndexField { class, member } => write!(
                f,
                "collection {class}::{member}: a list or a map has an index field, a set has none"
            ),
            Self::ValueFields {
                class,
                member,
                expected,
                given,
            } => write!(
                f,
                "member {class}::{member} needs {expected} value fields, given {given} (DDS 1.2 §8.1.4.2.3)"
            ),
            Self::RepeatedValueField {
                class,
                member,
                field,
            } => write!(
                f,
                "member {class}::{member} names the value field {field} twice; each cell has a field of its own (DDS 1.2 §8.1.4.2.3)"
            ),
            Self::TargetKey {
                class,
                relation,
                reason,
            } => write!(
                f,
                "relation {class}::{relation}: {reason} (DDS 1.2 §8.1.4.2.2)"
            ),
            Self::ClassPartRequired { topic, classes } => write!(
                f,
                "topic {topic} holds rows of the classes {classes:?} and its key has no class part (DDS 1.2 §8.1.4.2.2)"
            ),
            Self::KeyConflict { topic } => write!(f, "topic {topic} is given two different keys"),
            Self::TypeNameConflict { topic } => {
                write!(f, "topic {topic} is given two different type names")
            }
            Self::FieldConflict { topic, field } => write!(
                f,
                "field {field} of topic {topic} is given two types, is a key field and a value field, or is a key field twice (owner key, index, set item)"
            ),
            Self::FieldUsedTwice { topic, field } => {
                write!(
                    f,
                    "field {field} of topic {topic} is written by two members"
                )
            }
            Self::EmptyName { context } => write!(f, "empty name in {context}"),
            Self::NoOidKeyField { class, field } => write!(
                f,
                "key field {field} of the NoOid main topic of class {class} is not the value field of a mono-valued attribute of a basic or enumeration type mapped into that topic"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MappingError {}

// ---------------------------------------------------------------------------
// Default mapping (§8.1.4.3)
// ---------------------------------------------------------------------------

/// The names of the default mapping of §8.1.4.3.
pub mod default_mapping {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;

    use crate::mapping::KeyContent;
    use crate::model::SimpleType;

    /// Default name of the class field.
    pub const CLASS_FIELD: &str = "class";
    /// Default name of the oid field.
    pub const OID_FIELD: &str = "oid";
    /// Default name of the index field of a collection.
    pub const INDEX_FIELD: &str = "index";

    /// "Name of the DCPS Topic is the DLRL class name." Also the name of
    /// the extension topic of a derived class.
    #[must_use]
    pub fn main_topic(class: &str) -> String {
        class.into()
    }

    /// "name of the DCPS Topic is the DLRL class name concatenated with
    /// the DLRL attribute name, with '.' as separator."
    #[must_use]
    pub fn multi_topic(class: &str, attribute: &str) -> String {
        format!("{class}.{attribute}")
    }

    /// "name of the DLRL attribute, if only one field is required; name
    /// of the DLRL attribute, concatenated with the name of each
    /// sub-field, with '.' as separator, otherwise."
    #[must_use]
    pub fn value_fields(attribute: &str, ty: &SimpleType) -> Vec<String> {
        let mut leaves = Vec::new();
        leaf_paths(ty, &mut String::new(), &mut leaves);
        if leaves.len() == 1 {
            return alloc::vec![attribute.into()];
        }
        leaves
            .into_iter()
            .map(|p| format!("{attribute}{p}"))
            .collect()
    }

    /// The fields of a reference to a target whose key is
    /// `target_key_fields`: one field named after the attribute, or the
    /// attribute name joined with each sub-field (`class`, `oid` for a
    /// full oid; the target's key fields for `NoOid`).
    #[must_use]
    pub fn reference_fields(
        attribute: &str,
        content: KeyContent,
        target_key_fields: &[String],
    ) -> Vec<String> {
        match content {
            KeyContent::SimpleOid => alloc::vec![attribute.into()],
            KeyContent::FullOid => alloc::vec![
                format!("{attribute}.{CLASS_FIELD}"),
                format!("{attribute}.{OID_FIELD}"),
            ],
            KeyContent::NoOid if target_key_fields.len() == 1 => alloc::vec![attribute.into()],
            KeyContent::NoOid => target_key_fields
                .iter()
                .map(|f| format!("{attribute}.{f}"))
                .collect(),
        }
    }

    /// zerodds-lint: recursion-depth 64 — one call per nested structure.
    /// The mapping passes the types of its model, whose structures
    /// [`crate::model::Model::new`] keeps within
    /// [`crate::model::MAX_STRUCT_NESTING`]; a type given to
    /// [`value_fields`] directly nests as deep as its caller built it.
    fn leaf_paths(ty: &SimpleType, prefix: &mut String, out: &mut Vec<String>) {
        match ty {
            SimpleType::Struct(s) => {
                for f in &s.fields {
                    let len = prefix.len();
                    prefix.push('.');
                    prefix.push_str(&f.name);
                    leaf_paths(&f.ty, prefix, out);
                    prefix.truncate(len);
                }
            }
            _ => out.push(prefix.clone()),
        }
    }
}

/// The cell types of a simple type, one per leaf.
///
/// zerodds-lint: recursion-depth 64 — one call per nested structure of a
/// type of the model, which [`crate::model::Model::new`] keeps within
/// [`crate::model::MAX_STRUCT_NESTING`].
pub(crate) fn simple_cells(ty: &SimpleType, out: &mut Vec<CellType>) {
    match ty {
        SimpleType::Basic(b) => out.push(match b {
            BasicType::Long => CellType::Long,
            BasicType::Short => CellType::Short,
            BasicType::Char => CellType::Char,
            BasicType::Octet => CellType::Octet,
            BasicType::Real => CellType::Real,
            BasicType::Double => CellType::Double,
            BasicType::String => CellType::String,
            BasicType::Sequence(i) => CellType::Sequence(*i),
        }),
        SimpleType::Enumeration(e) => out.push(match e.coding {
            EnumCoding::Integer => CellType::Long,
            EnumCoding::Names(_) => CellType::String,
        }),
        SimpleType::Struct(s) => {
            for f in &s.fields {
                simple_cells(&f.ty, out);
            }
        }
    }
}

fn index_cell(basis: CollectionBasis) -> CellType {
    match basis {
        CollectionBasis::StrMap => CellType::String,
        _ => CellType::Long,
    }
}

fn has_duplicate(v: &[String]) -> bool {
    v.iter().enumerate().any(|(i, a)| v[i + 1..].contains(a))
}

// ---------------------------------------------------------------------------
// Mapping
// ---------------------------------------------------------------------------

/// A model with its checked structural mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    model: Model,
    classes: Vec<ClassMapping>,
    topics: Vec<TopicDescription>,
}

impl Mapping {
    /// The default mapping of §8.1.4.3 for every class.
    ///
    /// # Errors
    /// See [`Mapping::new`].
    pub fn default_for(model: Model) -> Result<Self, MappingError> {
        Self::new(model, &[])
    }

    /// Applies the given class mappings, completes them with the
    /// defaults of §8.1.4.3 and checks the result against §8.1.4.
    ///
    /// # Errors
    /// The first rule the mapping breaks, as [`MappingError`].
    pub fn new(model: Model, specs: &[ClassMappingSpec]) -> Result<Self, MappingError> {
        let mut by_class: BTreeMap<&str, &ClassMappingSpec> = BTreeMap::new();
        for s in specs {
            if model.class(&s.class).is_none() {
                return Err(MappingError::UnknownClass(s.class.clone()));
            }
            if by_class.insert(s.class.as_str(), s).is_some() {
                return Err(MappingError::DuplicateClassMapping(s.class.clone()));
            }
        }

        // Parents before children.
        let order = inheritance_order(&model);

        // Main topic names first: the default key needs to know whether
        // several classes share a main topic.
        let mut main_names: BTreeMap<String, String> = BTreeMap::new();
        for c in &order {
            let name = match by_class
                .get(c.name.as_str())
                .and_then(|s| s.main_topic.as_ref())
            {
                Some(t) => t.name.clone(),
                None => match model.parent(&c.name) {
                    Some(p) => main_names
                        .get(&p.name)
                        .cloned()
                        .unwrap_or_else(|| default_mapping::main_topic(&c.name)),
                    None => default_mapping::main_topic(&c.name),
                },
            };
            main_names.insert(c.name.clone(), name);
        }
        let sharing = |topic: &str| main_names.values().filter(|n| *n == topic).count();

        let mut mains: BTreeMap<String, (Topic, bool)> = BTreeMap::new();
        for c in &order {
            let spec = by_class.get(c.name.as_str());
            let (main, defaulted) = match spec.and_then(|s| s.main_topic.as_ref()) {
                Some(t) => (Topic::from_spec(t), false),
                None => match model.parent(&c.name).and_then(|p| mains.get(&p.name)) {
                    Some((t, _)) => (t.clone(), true),
                    None => {
                        let name = default_mapping::main_topic(&c.name);
                        let key = if sharing(&name) > 1 {
                            KeyDescription::full_oid(
                                default_mapping::CLASS_FIELD,
                                default_mapping::OID_FIELD,
                            )
                        } else {
                            KeyDescription::simple_oid(default_mapping::OID_FIELD)
                        };
                        (Topic::named(name, key), true)
                    }
                },
            };
            mains.insert(c.name.clone(), (main, defaulted));
        }

        let mut classes = Vec::with_capacity(order.len());
        for c in &order {
            let spec = by_class.get(c.name.as_str()).copied();
            let (main, defaulted) = mains[&c.name].clone();
            check_topic_names(&main, &c.name)?;
            main.key
                .check(&format!("main topic {} of class {}", main.name, c.name))?;
            let extension = match spec.and_then(|s| s.extension_topic.as_ref()) {
                Some(t) => Some(Topic::from_spec(t)),
                None => {
                    let shares_parent = model
                        .parent(&c.name)
                        .is_some_and(|p| mains[&p.name].0.name == main.name);
                    let has_mono = c
                        .attributes
                        .iter()
                        .any(|a| matches!(a.ty, AttributeType::Mono(_)))
                        || c.relations
                            .iter()
                            .any(|r| r.multiplicity == Multiplicity::ToOne);
                    if defaulted && shares_parent && has_mono {
                        Some(Topic::named(
                            default_mapping::main_topic(&c.name),
                            main.key.clone(),
                        ))
                    } else {
                        None
                    }
                }
            };
            if let Some(e) = &extension {
                check_topic_names(e, &c.name)?;
                if e.name == main.name {
                    return Err(MappingError::ExtensionIsMainTopic {
                        class: c.name.clone(),
                    });
                }
                check_owner_key(&main, e, &c.name)?;
            }
            let members = resolve_members(&model, c, spec, &main, extension.as_ref(), &mains)?;
            classes.push(ClassMapping {
                class: c.name.clone(),
                main_topic: main,
                extension_topic: extension,
                members,
                identity: Vec::new(),
            });
        }

        let mut mapping = Self {
            model,
            classes,
            topics: Vec::new(),
        };
        mapping.check_collection_topics()?;
        for i in 0..mapping.classes.len() {
            let identity = mapping.identity_of(&mapping.classes[i])?;
            mapping.classes[i].identity = identity;
        }
        mapping.topics = mapping.describe_topics()?;
        Ok(mapping)
    }

    /// The model.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The class mappings, parents before children.
    #[must_use]
    pub fn classes(&self) -> &[ClassMapping] {
        &self.classes
    }

    /// The mapping of a class.
    #[must_use]
    pub fn class(&self, name: &str) -> Option<&ClassMapping> {
        self.classes.iter().find(|c| c.class == name)
    }

    /// The mapping of a member of a class or of one of its ancestors,
    /// with the class that declares it.
    #[must_use]
    pub fn member(&self, class: &str, member: &str) -> Option<(&ClassMapping, &MemberMapping)> {
        self.model.lineage(class).into_iter().find_map(|c| {
            let cm = self.class(&c.name)?;
            cm.member(member).map(|m| (cm, m))
        })
    }

    /// All member mappings of a class, inherited ones first.
    #[must_use]
    pub fn members_of_class(&self, class: &str) -> Vec<&MemberMapping> {
        let mut out = Vec::new();
        for c in self.model.lineage(class).into_iter().rev() {
            if let Some(cm) = self.class(&c.name) {
                out.extend(cm.members.iter());
            }
        }
        out
    }

    /// Every topic of the mapping (the DCPS description), in the order
    /// of first use.
    #[must_use]
    pub fn topics(&self) -> &[TopicDescription] {
        &self.topics
    }

    /// A topic by name.
    #[must_use]
    pub fn topic(&self, name: &str) -> Option<&TopicDescription> {
        self.topics.iter().find(|t| t.name == name)
    }

    /// The names of the topics in which objects of `class` have rows.
    #[must_use]
    pub fn topics_of_class(&self, class: &str) -> Vec<String> {
        self.topics_of_class_unchecked(class)
    }

    /// §8.1.4.5: the attributes behind the key fields of a `NoOid` main
    /// topic, in key order.
    fn identity_of(&self, cm: &ClassMapping) -> Result<Vec<String>, MappingError> {
        if cm.main_topic.key.content != KeyContent::NoOid {
            return Ok(Vec::new());
        }
        let members = self.members_of_class(&cm.class);
        let mut out = Vec::with_capacity(cm.main_topic.key.fields.len());
        for f in &cm.main_topic.key.fields {
            let found = members.iter().find_map(|m| match m {
                MemberMapping::MonoAttribute(a)
                    if a.topic.name == cm.main_topic.name && a.value_fields == [f.clone()] =>
                {
                    Some(a.attribute.clone())
                }
                _ => None,
            });
            let scalar = found.as_ref().is_some_and(|a| {
                matches!(
                    self.model.member(&cm.class, a),
                    Some((
                        _,
                        Member::Attribute(Attribute {
                            ty: AttributeType::Mono(
                                SimpleType::Enumeration(_)
                                    | SimpleType::Basic(
                                        BasicType::Long
                                            | BasicType::Short
                                            | BasicType::Char
                                            | BasicType::Octet
                                            | BasicType::Real
                                            | BasicType::Double
                                            | BasicType::String
                                    )
                            ),
                            ..
                        })
                    ))
                )
            });
            match found {
                Some(a) if scalar => out.push(a),
                _ => {
                    return Err(MappingError::NoOidKeyField {
                        class: cm.class.clone(),
                        field: f.clone(),
                    });
                }
            }
        }
        Ok(out)
    }

    /// The cells of the identity attributes of a class (empty unless its
    /// main topic has a `NoOid` key).
    #[must_use]
    pub fn identity_cells(&self, class: &str) -> Vec<CellType> {
        let mut cells = Vec::new();
        if let Some(cm) = self.class(class) {
            for a in &cm.identity {
                if let Some((
                    _,
                    Member::Attribute(Attribute {
                        ty: AttributeType::Mono(t),
                        ..
                    }),
                )) = self.model.member(class, a)
                {
                    simple_cells(t, &mut cells);
                }
            }
        }
        cells
    }

    fn check_collection_topics(&self) -> Result<(), MappingError> {
        // The topics of the classes, and the members by topic.
        let mut class_topics: Vec<&str> = Vec::new();
        for cm in &self.classes {
            class_topics.push(&cm.main_topic.name);
            if let Some(e) = &cm.extension_topic {
                class_topics.push(&e.name);
            }
        }
        for cm in &self.classes {
            for m in &cm.members {
                let t = m.topic().name.as_str();
                let used_elsewhere = self.classes.iter().any(|other| {
                    other.members.iter().any(|o| {
                        o.topic().name == t && !(other.class == cm.class && o.name() == m.name())
                    })
                });
                if m.is_multi() && (class_topics.contains(&t) || used_elsewhere) {
                    return Err(MappingError::CollectionTopic {
                        class: cm.class.clone(),
                        member: m.name().into(),
                        topic: t.into(),
                    });
                }
            }
        }
        Ok(())
    }

    fn describe_topics(&self) -> Result<Vec<TopicDescription>, MappingError> {
        // (topic, type name, key layout, value fields with writer)
        struct Acc {
            desc: TopicDescription,
            writers: BTreeMap<String, (String, String)>,
        }
        let mut accs: Vec<Acc> = Vec::new();

        let mut add = |topic: &Topic,
                       owner: &str,
                       extra_keys: Vec<FieldDescription>,
                       values: Vec<FieldDescription>,
                       writer: Option<(&str, &str)>|
         -> Result<(), MappingError> {
            let identity = self.identity_cells(owner);
            if topic.key.content == KeyContent::NoOid && identity.len() != topic.key.fields.len() {
                return Err(MappingError::KeyConflict {
                    topic: topic.name.clone(),
                });
            }
            let mut keys: Vec<FieldDescription> = topic
                .key
                .fields
                .iter()
                .zip(topic.key.cells(&identity))
                .map(|(n, c)| FieldDescription {
                    name: n.clone(),
                    cell: c,
                    key: true,
                })
                .collect();
            // The index of a list or a map, or the item fields of a set,
            // complete the owner key; none of them repeats a key field.
            for (i, k) in extra_keys.iter().enumerate() {
                if keys.iter().any(|o| o.name == k.name)
                    || extra_keys[i + 1..].iter().any(|o| o.name == k.name)
                {
                    return Err(MappingError::FieldConflict {
                        topic: topic.name.clone(),
                        field: k.name.clone(),
                    });
                }
            }
            keys.extend(extra_keys);
            let pos = accs.iter().position(|a| a.desc.name == topic.name);
            let acc = match pos {
                Some(p) => {
                    let a = &mut accs[p];
                    if a.desc.type_name != topic.type_name {
                        return Err(MappingError::TypeNameConflict {
                            topic: topic.name.clone(),
                        });
                    }
                    let existing: Vec<&FieldDescription> =
                        a.desc.fields.iter().filter(|f| f.key).collect();
                    if existing.len() != keys.len()
                        || existing.iter().zip(&keys).any(|(x, y)| *x != y)
                    {
                        return Err(MappingError::KeyConflict {
                            topic: topic.name.clone(),
                        });
                    }
                    a
                }
                None => {
                    accs.push(Acc {
                        desc: TopicDescription {
                            name: topic.name.clone(),
                            type_name: topic.type_name.clone(),
                            key_content: topic.key.content,
                            fields: keys,
                            classes: Vec::new(),
                        },
                        writers: BTreeMap::new(),
                    });
                    let last = accs.len() - 1;
                    &mut accs[last]
                }
            };
            for v in values {
                if let Some((class, member)) = writer {
                    let w = (String::from(class), String::from(member));
                    match acc.writers.get(&v.name) {
                        Some(prev) if *prev != w => {
                            return Err(MappingError::FieldUsedTwice {
                                topic: topic.name.clone(),
                                field: v.name.clone(),
                            });
                        }
                        Some(_) => {}
                        None => {
                            acc.writers.insert(v.name.clone(), w);
                        }
                    }
                }
                match acc.desc.field(&v.name) {
                    // §8.1.4.5: an identity attribute's value field is a
                    // key field of the NoOid main topic.
                    Some(f)
                        if f.key
                            && acc.desc.key_content == KeyContent::NoOid
                            && f.cell == v.cell => {}
                    Some(f) if *f != v => {
                        return Err(MappingError::FieldConflict {
                            topic: topic.name.clone(),
                            field: v.name.clone(),
                        });
                    }
                    Some(_) => {}
                    None => acc.desc.fields.push(v),
                }
            }
            Ok(())
        };

        for cm in &self.classes {
            add(&cm.main_topic, &cm.class, Vec::new(), Vec::new(), None)?;
            if let Some(e) = &cm.extension_topic {
                add(e, &cm.class, Vec::new(), Vec::new(), None)?;
            }
            for m in &cm.members {
                let (extra, values) = self.member_fields(&cm.class, m)?;
                add(
                    m.topic(),
                    &cm.class,
                    extra,
                    values,
                    Some((&cm.class, m.name())),
                )?;
            }
        }

        let mut out: Vec<TopicDescription> = accs.into_iter().map(|a| a.desc).collect();
        // The classes with rows per topic, and the class part where
        // there are several.
        for c in self.model.classes() {
            for t in self.topics_of_class_unchecked(&c.name) {
                if let Some(d) = out.iter_mut().find(|d| d.name == t)
                    && !d.classes.contains(&c.name)
                {
                    d.classes.push(c.name.clone());
                }
            }
        }
        for d in &out {
            if d.classes.len() > 1 && d.key_content != KeyContent::FullOid {
                return Err(MappingError::ClassPartRequired {
                    topic: d.name.clone(),
                    classes: d.classes.clone(),
                });
            }
        }
        // Order: key fields first, others in order of first use.
        for d in &mut out {
            let (k, v): (Vec<FieldDescription>, Vec<FieldDescription>) =
                d.fields.drain(..).partition(|f| f.key);
            d.fields = k.into_iter().chain(v).collect();
        }
        Ok(out)
    }

    fn topics_of_class_unchecked(&self, class: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |n: &str| {
            if !out.iter().any(|x| x == n) {
                out.push(n.into());
            }
        };
        if let Some(cm) = self.class(class) {
            push(&cm.main_topic.name);
            if let Some(e) = &cm.extension_topic {
                push(&e.name);
            }
        }
        for m in self.members_of_class(class) {
            push(&m.topic().name);
        }
        out
    }

    /// The extra key fields and the value fields a member puts into its
    /// topic.
    fn member_fields(
        &self,
        class: &str,
        m: &MemberMapping,
    ) -> Result<(Vec<FieldDescription>, Vec<FieldDescription>), MappingError> {
        let field = |name: &String, cell: CellType, key: bool| FieldDescription {
            name: name.clone(),
            cell,
            key,
        };
        let value_cells = |name: &str| -> Vec<CellType> {
            let mut cells = Vec::new();
            if let Some((_, Member::Attribute(a))) = self.model.member(class, name) {
                match &a.ty {
                    AttributeType::Mono(t) | AttributeType::Multi { item: t, .. } => {
                        simple_cells(t, &mut cells);
                    }
                }
            }
            cells
        };
        let target_identity = |relation: &str| -> Vec<CellType> {
            match self.model.member(class, relation) {
                Some((_, Member::Relation(r))) => self.identity_cells(&r.target),
                _ => Vec::new(),
            }
        };
        let fields_of = |names: &[String], cells: Vec<CellType>, key: bool| {
            names
                .iter()
                .zip(cells)
                .map(|(n, c)| field(n, c, key))
                .collect::<Vec<_>>()
        };
        Ok(match m {
            MemberMapping::MonoAttribute(a) => (
                Vec::new(),
                fields_of(&a.value_fields, value_cells(&a.attribute), false),
            ),
            MemberMapping::MonoRelation(r) => (
                Vec::new(),
                fields_of(
                    &r.target_key.fields,
                    r.target_key.cells(&target_identity(&r.relation)),
                    false,
                ),
            ),
            MemberMapping::MultiAttribute(a) => {
                let cells = value_cells(&a.attribute);
                collection_fields(
                    a.basis,
                    a.index_field.as_ref(),
                    fields_of(&a.value_fields, cells, a.basis == CollectionBasis::Set),
                )
            }
            MemberMapping::MultiRelation(r) => collection_fields(
                r.basis,
                r.index_field.as_ref(),
                fields_of(
                    &r.target_key.fields,
                    r.target_key.cells(&target_identity(&r.relation)),
                    r.basis == CollectionBasis::Set,
                ),
            ),
        })
    }
}

fn collection_fields(
    basis: CollectionBasis,
    index: Option<&String>,
    values: Vec<FieldDescription>,
) -> (Vec<FieldDescription>, Vec<FieldDescription>) {
    match (basis, index) {
        // A set: the item itself completes the key.
        (CollectionBasis::Set, _) | (_, None) => (values, Vec::new()),
        (b, Some(i)) => (
            alloc::vec![FieldDescription {
                name: i.clone(),
                cell: index_cell(b),
                key: true,
            }],
            values,
        ),
    }
}

fn inheritance_order(model: &Model) -> Vec<&ClassModel> {
    let mut order: Vec<&ClassModel> = Vec::new();
    let mut remaining: Vec<&ClassModel> = model.classes().iter().collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        remaining.retain(|c| {
            let ready = model
                .parent(&c.name)
                .is_none_or(|p| order.iter().any(|o| o.name == p.name));
            if ready {
                order.push(c);
            }
            !ready
        });
        if remaining.len() == before {
            // Model::new refuses cycles; keep the rest in given order.
            order.append(&mut remaining);
        }
    }
    order
}

fn check_topic_names(t: &Topic, class: &str) -> Result<(), MappingError> {
    if t.name.is_empty() || t.type_name.is_empty() {
        return Err(MappingError::EmptyName {
            context: format!("a topic of class {class}"),
        });
    }
    Ok(())
}

fn check_owner_key(main: &Topic, other: &Topic, class: &str) -> Result<(), MappingError> {
    other
        .key
        .check(&format!("topic {} of class {class}", other.name))?;
    if other.key.content != main.key.content || other.key.fields.len() != main.key.fields.len() {
        return Err(MappingError::OwnerKeyMismatch {
            class: class.into(),
            topic: other.name.clone(),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn resolve_members(
    model: &Model,
    c: &ClassModel,
    spec: Option<&ClassMappingSpec>,
    main: &Topic,
    extension: Option<&Topic>,
    mains: &BTreeMap<String, (Topic, bool)>,
) -> Result<Vec<MemberMapping>, MappingError> {
    let mut given: BTreeMap<&str, &MemberMappingSpec> = BTreeMap::new();
    if let Some(s) = spec {
        for m in &s.members {
            let name = m.name();
            match model.member(&c.name, name) {
                None => {
                    return Err(MappingError::UnknownMember {
                        class: c.name.clone(),
                        member: name.into(),
                    });
                }
                Some((owner, _)) if owner.name != c.name => {
                    return Err(MappingError::InheritedMember {
                        class: c.name.clone(),
                        member: name.into(),
                    });
                }
                Some((_, Member::Local)) => {
                    return Err(MappingError::LocalMember {
                        class: c.name.clone(),
                        member: name.into(),
                    });
                }
                Some(_) => {}
            }
            if given.insert(name, m).is_some() {
                return Err(MappingError::DuplicateMemberMapping {
                    class: c.name.clone(),
                    member: name.into(),
                });
            }
        }
    }
    let place_default = extension.unwrap_or(main);
    let kind_err =
        |name: &str, given: &'static str, expected: &'static str| MappingError::MemberKind {
            class: c.name.clone(),
            member: name.into(),
            given,
            expected,
        };
    let value_count = |ty: &SimpleType| {
        let mut cells = Vec::new();
        simple_cells(ty, &mut cells);
        cells.len()
    };
    let check_values = |name: &str, fields: &[String], ty: &SimpleType| {
        let expected = value_count(ty);
        if fields.len() != expected || fields.iter().any(String::is_empty) {
            return Err(MappingError::ValueFields {
                class: c.name.clone(),
                member: name.into(),
                expected,
                given: fields.len(),
            });
        }
        if let Some(f) = fields
            .iter()
            .enumerate()
            .find_map(|(i, f)| fields[i + 1..].contains(f).then_some(f))
        {
            return Err(MappingError::RepeatedValueField {
                class: c.name.clone(),
                member: name.into(),
                field: f.clone(),
            });
        }
        Ok(())
    };
    let multi_topic = |name: &str,
                       basis: CollectionBasis,
                       given: Option<&MultiTopicSpec>|
     -> Result<(Topic, Option<String>), MappingError> {
        let (topic, index) = match given {
            Some(t) => (
                Topic {
                    name: t.name.clone(),
                    type_name: t.type_name.clone().unwrap_or_else(|| t.name.clone()),
                    key: t.key.clone(),
                },
                t.index_field.clone(),
            ),
            None => (
                Topic::named(
                    default_mapping::multi_topic(&c.name, name),
                    main.key.clone(),
                ),
                None,
            ),
        };
        check_topic_names(&topic, &c.name)?;
        check_owner_key(main, &topic, &c.name)?;
        let index = match (basis, index) {
            (CollectionBasis::Set, None) => None,
            (CollectionBasis::Set, Some(_)) => {
                return Err(MappingError::IndexField {
                    class: c.name.clone(),
                    member: name.into(),
                });
            }
            (_, Some(i)) if i.is_empty() => {
                return Err(MappingError::IndexField {
                    class: c.name.clone(),
                    member: name.into(),
                });
            }
            (_, Some(i)) => Some(i),
            (_, None) => Some(default_mapping::INDEX_FIELD.into()),
        };
        Ok((topic, index))
    };
    let place = |given: Option<&TopicSpec>| -> Result<Topic, MappingError> {
        match given {
            Some(t) => {
                let t = Topic::from_spec(t);
                check_topic_names(&t, &c.name)?;
                check_owner_key(main, &t, &c.name)?;
                Ok(t)
            }
            None => Ok(place_default.clone()),
        }
    };

    let mut out = Vec::new();
    for a in &c.attributes {
        let g = given.get(a.name.as_str()).copied();
        match &a.ty {
            AttributeType::Mono(ty) => {
                let (topic, fields) = match g {
                    None => (
                        place_default.clone(),
                        default_mapping::value_fields(&a.name, ty),
                    ),
                    Some(MemberMappingSpec::MonoAttribute {
                        place_topic,
                        value_fields,
                        ..
                    }) => (place(place_topic.as_ref())?, value_fields.clone()),
                    Some(other) => return Err(kind_err(&a.name, other.kind(), "monoAttribute")),
                };
                check_values(&a.name, &fields, ty)?;
                out.push(MemberMapping::MonoAttribute(MonoAttributeMapping {
                    attribute: a.name.clone(),
                    topic,
                    value_fields: fields,
                }));
            }
            AttributeType::Multi { basis, item } => {
                let (topic, index, fields) = match g {
                    None => {
                        let (t, i) = multi_topic(&a.name, *basis, None)?;
                        (t, i, default_mapping::value_fields(&a.name, item))
                    }
                    Some(MemberMappingSpec::MultiAttribute {
                        topic,
                        value_fields,
                        ..
                    }) => {
                        let (t, i) = multi_topic(&a.name, *basis, Some(topic))?;
                        (t, i, value_fields.clone())
                    }
                    Some(other) => return Err(kind_err(&a.name, other.kind(), "multiAttribute")),
                };
                check_values(&a.name, &fields, item)?;
                out.push(MemberMapping::MultiAttribute(MultiAttributeMapping {
                    attribute: a.name.clone(),
                    basis: *basis,
                    topic,
                    index_field: index,
                    value_fields: fields,
                }));
            }
        }
    }
    for r in &c.relations {
        let g = given.get(r.name.as_str()).copied();
        let target_main = &mains[&r.target].0;
        let class_needed = model.has_derived(&r.target)
            || mains
                .values()
                .filter(|(t, _)| t.name == target_main.name)
                .count()
                > 1;
        let default_key = || {
            let content = match target_main.key.content {
                KeyContent::NoOid => KeyContent::NoOid,
                _ if class_needed => KeyContent::FullOid,
                _ => KeyContent::SimpleOid,
            };
            KeyDescription {
                content,
                fields: default_mapping::reference_fields(
                    &r.name,
                    content,
                    &target_main.key.fields,
                ),
            }
        };
        let check_target = |key: &KeyDescription| -> Result<(), MappingError> {
            key.check(&format!("target key of relation {}::{}", c.name, r.name))?;
            let err = |reason: &str| MappingError::TargetKey {
                class: c.name.clone(),
                relation: r.name.clone(),
                reason: reason.into(),
            };
            match (target_main.key.content, key.content) {
                (KeyContent::NoOid, KeyContent::NoOid) => {
                    if key.fields.len() == target_main.key.fields.len() {
                        Ok(())
                    } else {
                        Err(err(
                            "the target fields do not match the key fields of the target's main topic",
                        ))
                    }
                }
                (KeyContent::NoOid, _) => Err(err(
                    "the target's main topic has a NoOid key, the relation has to designate it by those fields",
                )),
                (_, KeyContent::NoOid) => Err(err(
                    "the target's main topic stores the oid, the relation cannot designate it by a NoOid key",
                )),
                (_, KeyContent::SimpleOid) if class_needed => Err(err(
                    "the target class has derived classes or shares its main topic, so the class part is needed (FullOid)",
                )),
                _ => Ok(()),
            }
        };
        match r.multiplicity {
            Multiplicity::ToOne => {
                let (topic, key) = match g {
                    None => (place_default.clone(), default_key()),
                    Some(MemberMappingSpec::MonoRelation {
                        place_topic,
                        target_key,
                        ..
                    }) => (place(place_topic.as_ref())?, target_key.clone()),
                    Some(other) => return Err(kind_err(&r.name, other.kind(), "monoRelation")),
                };
                check_target(&key)?;
                out.push(MemberMapping::MonoRelation(MonoRelationMapping {
                    relation: r.name.clone(),
                    topic,
                    target_key: key,
                    is_composition: r.is_composition,
                }));
            }
            Multiplicity::ToMany(basis) => {
                let (topic, index, key) = match g {
                    None => {
                        let (t, i) = multi_topic(&r.name, basis, None)?;
                        (t, i, default_key())
                    }
                    Some(MemberMappingSpec::MultiRelation {
                        topic, target_key, ..
                    }) => {
                        let (t, i) = multi_topic(&r.name, basis, Some(topic))?;
                        (t, i, target_key.clone())
                    }
                    Some(other) => return Err(kind_err(&r.name, other.kind(), "multiRelation")),
                };
                check_target(&key)?;
                out.push(MemberMapping::MultiRelation(MultiRelationMapping {
                    relation: r.name.clone(),
                    basis,
                    topic,
                    index_field: index,
                    target_key: key,
                    is_composition: r.is_composition,
                }));
            }
        }
    }
    Ok(out)
}
