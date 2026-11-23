// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The shared part of a DLRL object model (DDS 1.2 §8.1.3).
//!
//! §8.1.3.3 gives the metamodel of Figure 8.1 "for explanation purpose"
//! and does not require it to be implemented as such. This module is
//! the form in which a model is described to the rest of the crate:
//!
//! | Figure 8.1                       | Here                                        |
//! |----------------------------------|---------------------------------------------|
//! | `Class` (`final`, `parent`, `children`) | [`ClassModel`], [`Model::parent`], [`Model::children`] |
//! | `Attribute`, `MonoAttribute`, `MultiAttribute` | [`Attribute`], [`AttributeType::Mono`], [`AttributeType::Multi`] |
//! | `Relation` (`is_composition`, `inverse`), `MonoRelation`, `MultiRelation` | [`Relation`], [`Multiplicity`] |
//! | `SimpleType`: `BasicType`, `EnumerationType`, `SimpleStructType` | [`SimpleType`], [`BasicType`], [`EnumerationType`], [`SimpleStructType`] |
//! | `MultiSimpleType`, `MultiRefType` | [`AttributeType::Multi`], [`Multiplicity::ToMany`] |
//! | `CollectionBase`: `ListBase`, `MapBase`, `SetBase` | [`CollectionBasis`] (the map basis in its two PSM forms `StrMap` and `IntMap`) |
//! | `ObjectRoot`                     | [`OBJECT_ROOT`], the base every class descends from |
//!
//! A [`Model`] is only built through [`Model::new`], which checks the
//! rules of §8.1.3: at least one shared attribute per class, single
//! inheritance from `ObjectRoot` or another DLRL class, no extension of
//! a `final` class, and the two rules for inverse relations. Values are
//! checked against it with [`Model::check_value`] (the type enforcement
//! of §8.1.3.1).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::types::DlrlOid;

/// The name of the ultimate root of all DLRL classes (§8.1.3.2.1).
pub const OBJECT_ROOT: &str = "ObjectRoot";

/// The spellings of [`OBJECT_ROOT`] accepted as a base: the plain name,
/// the scoped name of the normative IDL (module `DDS`, §8.2.1.2.1) and
/// the scoped name of the example (`DLRL::ObjectRoot`, §8.2.3.2).
pub const OBJECT_ROOT_NAMES: [&str; 3] = ["ObjectRoot", "DDS::ObjectRoot", "DLRL::ObjectRoot"];

fn is_object_root(name: &str) -> bool {
    OBJECT_ROOT_NAMES.contains(&name)
}

// ---------------------------------------------------------------------------
// Types (Figure 8.1, lower half)
// ---------------------------------------------------------------------------

/// An item type of a `sequence` basic type (§8.1.3.3: "sequence of any
/// of the above").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicItem {
    /// `long`.
    Long,
    /// `short`.
    Short,
    /// `char`.
    Char,
    /// `octet`.
    Octet,
    /// `real` (IDL `float`).
    Real,
    /// `double`.
    Double,
    /// `string`.
    String,
}

impl BasicItem {
    /// The name used in §8.1.3.3.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
            Self::Char => "char",
            Self::Octet => "octet",
            Self::Real => "real",
            Self::Double => "double",
            Self::String => "string",
        }
    }
}

/// §8.1.3.3 `BasicType`: long, short, char, octet, real, double,
/// string, and a sequence of any of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicType {
    /// `long`.
    Long,
    /// `short`.
    Short,
    /// `char`.
    Char,
    /// `octet`.
    Octet,
    /// `real` (IDL `float`).
    Real,
    /// `double`.
    Double,
    /// `string`.
    String,
    /// `sequence<item>`.
    Sequence(BasicItem),
}

/// How an enumeration is held in a DCPS cell (§8.1.4.2.3: "one cell of
/// type integer (default behavior) or string"; §8.2.2.3.2.2 `enumDef`
/// gives the strings).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum EnumCoding {
    /// The position of the enumerator, as an integer.
    #[default]
    Integer,
    /// One string per enumerator, in the order of the enumerators.
    Names(Vec<String>),
}

/// §8.1.3.3 `EnumerationType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumerationType {
    /// Scoped name of the enumeration.
    pub name: String,
    /// The enumerators in declaration order.
    pub enumerators: Vec<String>,
    /// The representation in a DCPS cell.
    pub coding: EnumCoding,
}

impl EnumerationType {
    /// An enumeration coded as integer.
    #[must_use]
    pub fn new(name: &str, enumerators: &[&str]) -> Self {
        Self {
            name: name.into(),
            enumerators: enumerators.iter().map(|e| String::from(*e)).collect(),
            coding: EnumCoding::Integer,
        }
    }

    /// The same enumeration coded by the given strings (`enumDef`).
    #[must_use]
    pub fn coded_as(mut self, names: &[&str]) -> Self {
        self.coding = EnumCoding::Names(names.iter().map(|e| String::from(*e)).collect());
        self
    }
}

/// One member of a [`SimpleStructType`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    /// Member name.
    pub name: String,
    /// Member type.
    pub ty: SimpleType,
}

/// §8.1.3.3 `SimpleStructType`: "all the structures that can be mapped
/// in a single DCPS data". Its members are simple types, so the
/// structure maps to one cell per leaf member (§8.1.4.2.3); a member
/// that is a collection or a reference cannot be written down here.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimpleStructType {
    /// Scoped name of the structure.
    pub name: String,
    /// Members in declaration order.
    pub fields: Vec<StructField>,
}

impl SimpleStructType {
    /// A structure from its members.
    #[must_use]
    pub fn new(name: &str, fields: &[(&str, SimpleType)]) -> Self {
        Self {
            name: name.into(),
            fields: fields
                .iter()
                .map(|(n, t)| StructField {
                    name: String::from(*n),
                    ty: t.clone(),
                })
                .collect(),
        }
    }
}

/// How deep the structures of a [`SimpleType`] nest: at most this many
/// structures inside one another, the type of an attribute the first.
/// [`Model::new`] refuses a deeper type with [`ModelError::BadStruct`]; the
/// functions that walk the types of a model go down one call per
/// structure. A model description read by the generator (`generation`)
/// holds no deeper structures either.
pub const MAX_STRUCT_NESTING: usize = 64;

/// §8.1.3.3 `SimpleType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleType {
    /// A basic type.
    Basic(BasicType),
    /// An enumeration.
    Enumeration(EnumerationType),
    /// A simple structure.
    Struct(SimpleStructType),
}

impl SimpleType {
    /// A short description for diagnostics.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Basic(BasicType::Sequence(i)) => format!("sequence<{}>", i.name()),
            Self::Basic(b) => String::from(basic_name(*b)),
            Self::Enumeration(e) => format!("enum {}", e.name),
            Self::Struct(s) => format!("struct {}", s.name),
        }
    }
}

fn basic_name(b: BasicType) -> &'static str {
    match b {
        BasicType::Long => "long",
        BasicType::Short => "short",
        BasicType::Char => "char",
        BasicType::Octet => "octet",
        BasicType::Real => "real",
        BasicType::Double => "double",
        BasicType::String => "string",
        BasicType::Sequence(_) => "sequence",
    }
}

/// §8.1.3.1 collection basis of a multi-valued attribute or relation:
/// a list (ordered with index), a map (access by key) in its string-
/// and integer-keyed forms (§8.1.4.2.3, §8.2.2.3.2.3), a set (not
/// ordered).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionBasis {
    /// `List`.
    List,
    /// `StrMap`.
    StrMap,
    /// `IntMap`.
    IntMap,
    /// `Set`.
    Set,
}

impl CollectionBasis {
    /// The pattern name of §8.2.2.3.1 (`templateDef pattern`).
    #[must_use]
    pub const fn pattern(self) -> &'static str {
        match self {
            Self::List => "List",
            Self::StrMap => "StrMap",
            Self::IntMap => "IntMap",
            Self::Set => "Set",
        }
    }
}

// ---------------------------------------------------------------------------
// Attributes and relations (Figure 8.1, upper half)
// ---------------------------------------------------------------------------

/// The type of a shared attribute that is not a relation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AttributeType {
    /// `MonoAttribute` with its `SimpleType`.
    Mono(SimpleType),
    /// `MultiAttribute` with its `MultiSimpleType` (basis and item type).
    Multi {
        /// The collection basis.
        basis: CollectionBasis,
        /// The item type.
        item: SimpleType,
    },
}

/// A shared attribute whose value is of a simple type or a collection
/// of a simple type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attribute {
    /// Attribute name.
    pub name: String,
    /// Attribute type.
    pub ty: AttributeType,
}

/// The two kinds of association ends (§8.1.3.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Multiplicity {
    /// A to-1 relation (`MonoRelation`): a reference to the target.
    ToOne,
    /// A to-many relation (`MultiRelation`): a collection of references.
    ToMany(CollectionBasis),
}

/// §8.1.3.2.2 a relation: an attribute whose value designates objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Relation {
    /// Attribute name.
    pub name: String,
    /// Name of the target class (`target_type` / `item_type`).
    pub target: String,
    /// To-1 or to-many.
    pub multiplicity: Multiplicity,
    /// A composition: the constituents' life cycle follows the compound
    /// (`compoRelationDef`, §8.2.2.3.2.5).
    pub is_composition: bool,
    /// The relation of the target class this one is the inverse of
    /// (`associationDef`, §8.2.2.3.2.4).
    pub inverse: Option<String>,
}

impl Relation {
    /// A plain to-1 relation.
    #[must_use]
    pub fn to_one(name: &str, target: &str) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            multiplicity: Multiplicity::ToOne,
            is_composition: false,
            inverse: None,
        }
    }

    /// A plain to-many relation.
    #[must_use]
    pub fn to_many(name: &str, target: &str, basis: CollectionBasis) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            multiplicity: Multiplicity::ToMany(basis),
            is_composition: false,
            inverse: None,
        }
    }

    /// Marks the relation as a composition.
    #[must_use]
    pub fn composition(mut self) -> Self {
        self.is_composition = true;
        self
    }

    /// Names the inverse relation in the target class.
    #[must_use]
    pub fn inverse_of(mut self, inverse: &str) -> Self {
        self.inverse = Some(inverse.into());
        self
    }
}

/// §8.1.3.3 `Class`: the description of one DLRL class.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassModel {
    /// Scoped class name.
    pub name: String,
    /// `final`: the class cannot be extended.
    pub is_final: bool,
    /// The declared bases: exactly one DLRL base (`ObjectRoot` or a
    /// class of the model) plus any number of native bases
    /// (§8.1.3.2.1: "DLRL objects can, in addition, inherit from any
    /// number of native language objects").
    pub bases: Vec<String>,
    /// Shared attributes that are not relations, in declaration order.
    pub attributes: Vec<Attribute>,
    /// Relations, in declaration order.
    pub relations: Vec<Relation>,
    /// Local attributes (§8.1.3.1: "do not participate in the data
    /// distribution"); they have no mapping.
    pub locals: Vec<String>,
}

impl ClassModel {
    /// A class without bases, attributes or relations.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            is_final: false,
            bases: Vec::new(),
            attributes: Vec::new(),
            relations: Vec::new(),
            locals: Vec::new(),
        }
    }

    /// Adds a declared base.
    #[must_use]
    pub fn base(mut self, base: &str) -> Self {
        self.bases.push(base.into());
        self
    }

    /// Marks the class `final`.
    #[must_use]
    pub fn final_class(mut self) -> Self {
        self.is_final = true;
        self
    }

    /// Adds a mono-valued attribute.
    #[must_use]
    pub fn mono(mut self, name: &str, ty: SimpleType) -> Self {
        self.attributes.push(Attribute {
            name: name.into(),
            ty: AttributeType::Mono(ty),
        });
        self
    }

    /// Adds a multi-valued attribute.
    #[must_use]
    pub fn multi(mut self, name: &str, basis: CollectionBasis, item: SimpleType) -> Self {
        self.attributes.push(Attribute {
            name: name.into(),
            ty: AttributeType::Multi { basis, item },
        });
        self
    }

    /// Adds a relation.
    #[must_use]
    pub fn relation(mut self, relation: Relation) -> Self {
        self.relations.push(relation);
        self
    }

    /// Adds a local attribute.
    #[must_use]
    pub fn local(mut self, name: &str) -> Self {
        self.locals.push(name.into());
        self
    }
}

/// A member of a class as seen through [`Model::member`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Member<'a> {
    /// A shared attribute that is not a relation.
    Attribute(&'a Attribute),
    /// A relation.
    Relation(&'a Relation),
    /// A local attribute.
    Local,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A model that breaks a rule of §8.1.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A class, member, enumerator or structure member without a name.
    EmptyName {
        /// Where the name is missing.
        context: String,
    },
    /// Two classes with the same name.
    DuplicateClass(String),
    /// A class without `ObjectRoot` or a DLRL class among its bases.
    NoDlrlBase(String),
    /// §8.1.3.2.1 "Single inheritance is allowed between DLRL objects".
    SeveralDlrlBases {
        /// The class.
        class: String,
        /// Its DLRL bases.
        bases: Vec<String>,
    },
    /// The DLRL base is `final` (§8.1.4.4.1).
    FinalParent {
        /// The class.
        class: String,
        /// Its final parent.
        parent: String,
    },
    /// The class is its own ancestor.
    InheritanceCycle(String),
    /// §8.1.3.1 "A DLRL object has at least one shared attribute".
    NoSharedAttribute(String),
    /// A member name used twice in a class or its ancestors.
    DuplicateMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// A relation whose target is not a class of the model.
    UnknownTarget {
        /// The owner class.
        class: String,
        /// The relation.
        relation: String,
        /// The missing target.
        target: String,
    },
    /// The inverse named by a relation does not exist in the target or
    /// does not name the relation back.
    InverseNotMutual {
        /// The owner class.
        class: String,
        /// The relation.
        relation: String,
        /// The named inverse.
        inverse: String,
    },
    /// §8.1.3.2.2 "providing that the types match": the inverse does
    /// not target the owner of the relation.
    InverseTypeMismatch {
        /// The owner class.
        class: String,
        /// The relation.
        relation: String,
    },
    /// §8.1.3.2.2 "One composition relation can only inverse a to-1
    /// relation to the compound object".
    CompositionInverse {
        /// The class that owns the composition.
        class: String,
        /// The composition.
        relation: String,
    },
    /// An enumeration without enumerators, with a repeated enumerator,
    /// or with a coding whose length differs from the enumerators.
    BadEnumeration {
        /// The enumeration.
        name: String,
        /// What is wrong.
        reason: String,
    },
    /// A structure without members or with a repeated member.
    BadStruct {
        /// The structure.
        name: String,
        /// What is wrong.
        reason: String,
    },
}

impl core::fmt::Display for ModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyName { context } => write!(f, "empty name in {context}"),
            Self::DuplicateClass(c) => write!(f, "class {c} is declared twice"),
            Self::NoDlrlBase(c) => write!(
                f,
                "class {c} has neither ObjectRoot nor a DLRL class among its bases (DDS 1.2 §8.1.3.2.1)"
            ),
            Self::SeveralDlrlBases { class, bases } => write!(
                f,
                "class {class} has several DLRL bases {bases:?}; only single inheritance is allowed between DLRL objects (DDS 1.2 §8.1.3.2.1)"
            ),
            Self::FinalParent { class, parent } => {
                write!(
                    f,
                    "class {class} extends the final class {parent} (DDS 1.2 §8.1.4.4.1)"
                )
            }
            Self::InheritanceCycle(c) => write!(f, "class {c} is its own ancestor"),
            Self::NoSharedAttribute(c) => write!(
                f,
                "class {c} has no shared attribute; a DLRL object has at least one (DDS 1.2 §8.1.3.1)"
            ),
            Self::DuplicateMember { class, member } => {
                write!(
                    f,
                    "member {member} is declared twice in class {class} or its ancestors"
                )
            }
            Self::UnknownTarget {
                class,
                relation,
                target,
            } => write!(
                f,
                "relation {class}::{relation} targets {target}, which is not a DLRL class of the model"
            ),
            Self::InverseNotMutual {
                class,
                relation,
                inverse,
            } => write!(
                f,
                "relation {class}::{relation} names {inverse} as inverse, which does not exist in the target or does not name it back"
            ),
            Self::InverseTypeMismatch { class, relation } => write!(
                f,
                "relation {class}::{relation} and its inverse do not target each other's owner (DDS 1.2 §8.1.3.2.2)"
            ),
            Self::CompositionInverse { class, relation } => write!(
                f,
                "composition {class}::{relation} can only inverse a plain to-1 relation to the compound object (DDS 1.2 §8.1.3.2.2)"
            ),
            Self::BadEnumeration { name, reason } => write!(f, "enumeration {name}: {reason}"),
            Self::BadStruct { name, reason } => write!(f, "structure {name}: {reason}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ModelError {}

/// A value that does not fit the type of the member it is meant for
/// (the type enforcement of §8.1.3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    /// The class is not in the model.
    UnknownClass(String),
    /// The class (with its ancestors) has no such member.
    UnknownMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// The member is local and holds no shared value.
    LocalMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// A collection value for a mono-valued member, a mono value for a
    /// collection, or a collection of another basis.
    WrongShape {
        /// The member.
        member: String,
        /// What the member expects.
        expected: String,
    },
    /// §8.1.3.1 "Strict type equality for simple types".
    NotEqual {
        /// The member.
        member: String,
        /// The type of the member.
        expected: String,
        /// The type of the value.
        found: String,
    },
    /// §8.1.3.1 "Equality based on inclusion for reference to a DLRL
    /// object": the referenced class is neither the target nor derived
    /// from it.
    NotIncluded {
        /// The member.
        member: String,
        /// The target class of the member.
        target: String,
        /// The class of the referenced object.
        found: String,
    },
    /// An enumerator that the enumeration does not have.
    UnknownEnumerator {
        /// The member.
        member: String,
        /// The enumerator.
        enumerator: String,
    },
    /// A reference to no object inside a collection.
    NilItem {
        /// The member.
        member: String,
    },
    /// The same item twice in a set.
    DuplicateSetItem {
        /// The member.
        member: String,
    },
    /// A reference that names an object by [`DlrlOid::NIL`]; the nil
    /// oid designates no object.
    NilOid {
        /// The member.
        member: String,
    },
}

impl core::fmt::Display for TypeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownClass(c) => write!(f, "class {c} is not in the model"),
            Self::UnknownMember { class, member } => {
                write!(f, "class {class} has no member {member}")
            }
            Self::LocalMember { class, member } => {
                write!(
                    f,
                    "member {class}::{member} is local and has no shared value"
                )
            }
            Self::WrongShape { member, expected } => {
                write!(f, "member {member} expects {expected}")
            }
            Self::NotEqual {
                member,
                expected,
                found,
            } => write!(
                f,
                "member {member} is of type {expected}, the value is {found} (strict type equality, DDS 1.2 §8.1.3.1)"
            ),
            Self::NotIncluded {
                member,
                target,
                found,
            } => write!(
                f,
                "member {member} references {target}; an object of class {found} is not one (DDS 1.2 §8.1.3.1)"
            ),
            Self::UnknownEnumerator { member, enumerator } => {
                write!(f, "member {member}: unknown enumerator {enumerator}")
            }
            Self::NilItem { member } => {
                write!(
                    f,
                    "member {member}: a collection cannot hold a reference to no object"
                )
            }
            Self::DuplicateSetItem { member } => {
                write!(f, "member {member}: a set holds each item once")
            }
            Self::NilOid { member } => write!(
                f,
                "member {member}: a reference names an object by the nil DLRLOid, which designates no object"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TypeError {}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

/// The value of a `sequence` basic type.
#[derive(Debug, Clone, PartialEq)]
pub enum SequenceValue {
    /// `sequence<long>`.
    Long(Vec<i32>),
    /// `sequence<short>`.
    Short(Vec<i16>),
    /// `sequence<char>`.
    Char(Vec<char>),
    /// `sequence<octet>`.
    Octet(Vec<u8>),
    /// `sequence<real>`.
    Real(Vec<f32>),
    /// `sequence<double>`.
    Double(Vec<f64>),
    /// `sequence<string>`.
    String(Vec<String>),
}

impl SequenceValue {
    /// The item type.
    #[must_use]
    pub const fn item(&self) -> BasicItem {
        match self {
            Self::Long(_) => BasicItem::Long,
            Self::Short(_) => BasicItem::Short,
            Self::Char(_) => BasicItem::Char,
            Self::Octet(_) => BasicItem::Octet,
            Self::Real(_) => BasicItem::Real,
            Self::Double(_) => BasicItem::Double,
            Self::String(_) => BasicItem::String,
        }
    }
}

/// A reference to a DLRL object: its class and its oid (the full oid
/// of §8.1.4.2.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    /// The exact class of the object.
    pub class: String,
    /// The oid of the object.
    pub oid: DlrlOid,
}

impl ObjectRef {
    /// A reference from class and oid.
    #[must_use]
    pub fn new(class: &str, oid: DlrlOid) -> Self {
        Self {
            class: class.into(),
            oid,
        }
    }
}

/// The value of a mono-valued member or of one collection item.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `long`.
    Long(i32),
    /// `short`.
    Short(i16),
    /// `char`.
    Char(char),
    /// `octet`.
    Octet(u8),
    /// `real`.
    Real(f32),
    /// `double`.
    Double(f64),
    /// `string`.
    String(String),
    /// A sequence of a basic item type.
    Sequence(SequenceValue),
    /// An enumerator of an enumeration.
    Enum {
        /// Scoped name of the enumeration.
        type_name: String,
        /// The enumerator.
        enumerator: String,
    },
    /// A simple structure: its members in declaration order.
    Struct {
        /// Scoped name of the structure.
        type_name: String,
        /// The members.
        fields: Vec<(String, Value)>,
    },
    /// A reference, or `None` for a to-1 relation that designates no
    /// object.
    Ref(Option<ObjectRef>),
}

impl Value {
    /// A short description of the value's type for diagnostics.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Long(_) => "long".into(),
            Self::Short(_) => "short".into(),
            Self::Char(_) => "char".into(),
            Self::Octet(_) => "octet".into(),
            Self::Real(_) => "real".into(),
            Self::Double(_) => "double".into(),
            Self::String(_) => "string".into(),
            Self::Sequence(s) => format!("sequence<{}>", s.item().name()),
            Self::Enum { type_name, .. } => format!("enum {type_name}"),
            Self::Struct { type_name, .. } => format!("struct {type_name}"),
            Self::Ref(Some(r)) => format!("reference to {}", r.class),
            Self::Ref(None) => "reference to no object".into(),
        }
    }
}

/// The value of a shared member: one value, or a collection by its
/// basis.
///
/// Two sets are equal when they hold the same items in any order.
#[derive(Debug, Clone)]
pub enum AttributeValue {
    /// A mono-valued attribute or a to-1 relation.
    Mono(Value),
    /// A `List`: items by rank.
    List(Vec<Value>),
    /// A `Set`: items without order, each once.
    Set(Vec<Value>),
    /// A `StrMap`: items by string key.
    StrMap(BTreeMap<String, Value>),
    /// An `IntMap`: items by integer key.
    IntMap(BTreeMap<i32, Value>),
}

impl PartialEq for AttributeValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Mono(a), Self::Mono(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Set(a), Self::Set(b)) => {
                a.len() == b.len()
                    && a.iter().all(|x| b.contains(x))
                    && b.iter().all(|x| a.contains(x))
            }
            (Self::StrMap(a), Self::StrMap(b)) => a == b,
            (Self::IntMap(a), Self::IntMap(b)) => a == b,
            (
                Self::Mono(_) | Self::List(_) | Self::Set(_) | Self::StrMap(_) | Self::IntMap(_),
                _,
            ) => false,
        }
    }
}

impl AttributeValue {
    fn shape(&self) -> Option<CollectionBasis> {
        match self {
            Self::Mono(_) => None,
            Self::List(_) => Some(CollectionBasis::List),
            Self::Set(_) => Some(CollectionBasis::Set),
            Self::StrMap(_) => Some(CollectionBasis::StrMap),
            Self::IntMap(_) => Some(CollectionBasis::IntMap),
        }
    }

    fn items(&self) -> Vec<&Value> {
        match self {
            Self::Mono(v) => alloc::vec![v],
            Self::List(v) | Self::Set(v) => v.iter().collect(),
            Self::StrMap(m) => m.values().collect(),
            Self::IntMap(m) => m.values().collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// A checked DLRL model: the classes with their resolved inheritance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    classes: Vec<ClassModel>,
    index: BTreeMap<String, usize>,
    parents: Vec<Option<usize>>,
}

impl Model {
    /// Checks the classes against the rules of §8.1.3 and builds the
    /// model.
    ///
    /// # Errors
    /// The first rule a class breaks, as [`ModelError`]; a simple type
    /// with structures nested deeper than [`MAX_STRUCT_NESTING`] as
    /// [`ModelError::BadStruct`].
    pub fn new(classes: Vec<ClassModel>) -> Result<Self, ModelError> {
        let mut index = BTreeMap::new();
        for (i, c) in classes.iter().enumerate() {
            if c.name.is_empty() {
                return Err(ModelError::EmptyName {
                    context: "a class".into(),
                });
            }
            if index.insert(c.name.clone(), i).is_some() {
                return Err(ModelError::DuplicateClass(c.name.clone()));
            }
        }
        let mut parents = Vec::with_capacity(classes.len());
        for c in &classes {
            let dlrl: Vec<&String> = c
                .bases
                .iter()
                .filter(|b| is_object_root(b) || index.contains_key(b.as_str()))
                .collect();
            match dlrl.as_slice() {
                [] => return Err(ModelError::NoDlrlBase(c.name.clone())),
                [one] => {
                    if is_object_root(one) {
                        parents.push(None);
                    } else {
                        let p = index[one.as_str()];
                        if classes[p].is_final {
                            return Err(ModelError::FinalParent {
                                class: c.name.clone(),
                                parent: classes[p].name.clone(),
                            });
                        }
                        parents.push(Some(p));
                    }
                }
                more => {
                    return Err(ModelError::SeveralDlrlBases {
                        class: c.name.clone(),
                        bases: more.iter().map(|b| (*b).clone()).collect(),
                    });
                }
            }
        }
        let model = Self {
            classes,
            index,
            parents,
        };
        model.check_acyclic()?;
        for c in &model.classes {
            model.check_class(c)?;
        }
        for c in &model.classes {
            for r in &c.relations {
                model.check_inverse(c, r)?;
            }
        }
        Ok(model)
    }

    fn check_acyclic(&self) -> Result<(), ModelError> {
        for (i, c) in self.classes.iter().enumerate() {
            let mut cur = self.parents[i];
            let mut steps = 0usize;
            while let Some(p) = cur {
                if p == i || steps > self.classes.len() {
                    return Err(ModelError::InheritanceCycle(c.name.clone()));
                }
                steps += 1;
                cur = self.parents[p];
            }
        }
        Ok(())
    }

    fn check_class(&self, c: &ClassModel) -> Result<(), ModelError> {
        let mut seen: Vec<&str> = Vec::new();
        let mut shared = 0usize;
        for owner in self.lineage(&c.name).into_iter().rev() {
            for a in &owner.attributes {
                check_name(&a.name, "an attribute")?;
                push_unique(&mut seen, &a.name, &c.name)?;
                match &a.ty {
                    AttributeType::Mono(t) | AttributeType::Multi { item: t, .. } => {
                        check_simple_type(t, 1)?;
                    }
                }
                shared += 1;
            }
            for r in &owner.relations {
                check_name(&r.name, "a relation")?;
                push_unique(&mut seen, &r.name, &c.name)?;
                if !self.index.contains_key(&r.target) {
                    return Err(ModelError::UnknownTarget {
                        class: owner.name.clone(),
                        relation: r.name.clone(),
                        target: r.target.clone(),
                    });
                }
                shared += 1;
            }
            for l in &owner.locals {
                check_name(l, "a local attribute")?;
                push_unique(&mut seen, l, &c.name)?;
            }
        }
        if shared == 0 {
            return Err(ModelError::NoSharedAttribute(c.name.clone()));
        }
        Ok(())
    }

    fn check_inverse(&self, owner: &ClassModel, r: &Relation) -> Result<(), ModelError> {
        let Some(inv_name) = &r.inverse else {
            return Ok(());
        };
        let not_mutual = || ModelError::InverseNotMutual {
            class: owner.name.clone(),
            relation: r.name.clone(),
            inverse: inv_name.clone(),
        };
        let target = &self.classes[self.index[&r.target]];
        let Some(inv) = target.relations.iter().find(|x| &x.name == inv_name) else {
            return Err(not_mutual());
        };
        if inv.inverse.as_deref() != Some(r.name.as_str()) {
            return Err(not_mutual());
        }
        if inv.target != owner.name {
            return Err(ModelError::InverseTypeMismatch {
                class: owner.name.clone(),
                relation: r.name.clone(),
            });
        }
        if r.is_composition && (inv.is_composition || inv.multiplicity != Multiplicity::ToOne) {
            return Err(ModelError::CompositionInverse {
                class: owner.name.clone(),
                relation: r.name.clone(),
            });
        }
        Ok(())
    }

    /// The classes in the order they were given.
    #[must_use]
    pub fn classes(&self) -> &[ClassModel] {
        &self.classes
    }

    /// A class by name.
    #[must_use]
    pub fn class(&self, name: &str) -> Option<&ClassModel> {
        self.index.get(name).map(|i| &self.classes[*i])
    }

    /// The DLRL parent of a class, `None` for a class directly under
    /// `ObjectRoot` or an unknown class.
    #[must_use]
    pub fn parent(&self, name: &str) -> Option<&ClassModel> {
        let i = *self.index.get(name)?;
        self.parents[i].map(|p| &self.classes[p])
    }

    /// The direct DLRL children of a class (Figure 8.1 `children`).
    #[must_use]
    pub fn children(&self, name: &str) -> Vec<&ClassModel> {
        let Some(&i) = self.index.get(name) else {
            return Vec::new();
        };
        self.classes
            .iter()
            .enumerate()
            .filter(|(j, _)| self.parents[*j] == Some(i))
            .map(|(_, c)| c)
            .collect()
    }

    /// True when at least one class derives from `name`.
    #[must_use]
    pub fn has_derived(&self, name: &str) -> bool {
        !self.children(name).is_empty()
    }

    /// The class and its DLRL ancestors, the class first; the chain ends
    /// below `ObjectRoot`. Empty for an unknown class.
    #[must_use]
    pub fn lineage(&self, name: &str) -> Vec<&ClassModel> {
        let mut out = Vec::new();
        let mut cur = self.index.get(name).copied();
        while let Some(i) = cur {
            out.push(&self.classes[i]);
            cur = self.parents[i];
        }
        out
    }

    /// The native bases of a class: its declared bases that are neither
    /// `ObjectRoot` nor a DLRL class.
    #[must_use]
    pub fn native_bases(&self, name: &str) -> Vec<&str> {
        self.class(name)
            .map(|c| {
                c.bases
                    .iter()
                    .filter(|b| !is_object_root(b) && !self.index.contains_key(b.as_str()))
                    .map(String::as_str)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// §8.1.3.1 inclusion: `class` is `base` or derives from it.
    #[must_use]
    pub fn is_kind_of(&self, class: &str, base: &str) -> bool {
        self.lineage(class).iter().any(|c| c.name == base)
    }

    /// A member of a class or of one of its ancestors, with the class
    /// that declares it.
    #[must_use]
    pub fn member(&self, class: &str, member: &str) -> Option<(&ClassModel, Member<'_>)> {
        for owner in self.lineage(class) {
            if let Some(a) = owner.attributes.iter().find(|a| a.name == member) {
                return Some((owner, Member::Attribute(a)));
            }
            if let Some(r) = owner.relations.iter().find(|r| r.name == member) {
                return Some((owner, Member::Relation(r)));
            }
            if owner.locals.iter().any(|l| l == member) {
                return Some((owner, Member::Local));
            }
        }
        None
    }

    /// Checks a value for a shared member of a class (§8.1.3.1 type
    /// enforcement): strict type equality for simple types, inclusion
    /// for references, for a mono value and for each collection item.
    ///
    /// # Errors
    /// [`TypeError`] naming the first mismatch.
    pub fn check_value(
        &self,
        class: &str,
        member: &str,
        value: &AttributeValue,
    ) -> Result<(), TypeError> {
        if self.class(class).is_none() {
            return Err(TypeError::UnknownClass(class.into()));
        }
        let Some((_, m)) = self.member(class, member) else {
            return Err(TypeError::UnknownMember {
                class: class.into(),
                member: member.into(),
            });
        };
        let (expected_shape, is_mono) = match m {
            Member::Local => {
                return Err(TypeError::LocalMember {
                    class: class.into(),
                    member: member.into(),
                });
            }
            Member::Attribute(a) => match &a.ty {
                AttributeType::Mono(_) => (None, true),
                AttributeType::Multi { basis, .. } => (Some(*basis), false),
            },
            Member::Relation(r) => match r.multiplicity {
                Multiplicity::ToOne => (None, true),
                Multiplicity::ToMany(b) => (Some(b), false),
            },
        };
        if value.shape() != expected_shape {
            return Err(TypeError::WrongShape {
                member: member.into(),
                expected: match expected_shape {
                    None => "a single value".into(),
                    Some(b) => format!("a {} collection", b.pattern()),
                },
            });
        }
        let items = value.items();
        for item in &items {
            match m {
                Member::Attribute(a) => {
                    let t = match &a.ty {
                        AttributeType::Mono(t) | AttributeType::Multi { item: t, .. } => t,
                    };
                    check_simple(member, t, item)?;
                }
                Member::Relation(r) => self.check_reference(member, &r.target, item, is_mono)?,
                Member::Local => {}
            }
        }
        if let AttributeValue::Set(v) = value {
            for (i, a) in v.iter().enumerate() {
                if v[i + 1..].contains(a) {
                    return Err(TypeError::DuplicateSetItem {
                        member: member.into(),
                    });
                }
            }
        }
        Ok(())
    }

    fn check_reference(
        &self,
        member: &str,
        target: &str,
        value: &Value,
        nil_allowed: bool,
    ) -> Result<(), TypeError> {
        match value {
            Value::Ref(Some(r)) if r.oid.is_nil() => Err(TypeError::NilOid {
                member: member.into(),
            }),
            Value::Ref(Some(r)) => {
                if self.is_kind_of(&r.class, target) {
                    Ok(())
                } else {
                    Err(TypeError::NotIncluded {
                        member: member.into(),
                        target: target.into(),
                        found: r.class.clone(),
                    })
                }
            }
            Value::Ref(None) if nil_allowed => Ok(()),
            Value::Ref(None) => Err(TypeError::NilItem {
                member: member.into(),
            }),
            other => Err(TypeError::NotEqual {
                member: member.into(),
                expected: format!("reference to {target}"),
                found: other.describe(),
            }),
        }
    }
}

fn check_name(name: &str, context: &str) -> Result<(), ModelError> {
    if name.is_empty() {
        return Err(ModelError::EmptyName {
            context: context.into(),
        });
    }
    Ok(())
}

fn push_unique<'a>(seen: &mut Vec<&'a str>, name: &'a str, class: &str) -> Result<(), ModelError> {
    if seen.contains(&name) {
        return Err(ModelError::DuplicateMember {
            class: class.into(),
            member: name.into(),
        });
    }
    seen.push(name);
    Ok(())
}

/// Checks `t`, which sits inside `level - 1` structures of the attribute
/// type it belongs to.
///
/// zerodds-lint: recursion-depth 64 — one call per nested structure; a
/// structure at a `level` above [`MAX_STRUCT_NESTING`] is refused before
/// its members are checked.
fn check_simple_type(t: &SimpleType, level: usize) -> Result<(), ModelError> {
    match t {
        SimpleType::Basic(_) => Ok(()),
        SimpleType::Enumeration(e) => {
            let bad = |reason: &str| ModelError::BadEnumeration {
                name: e.name.clone(),
                reason: reason.into(),
            };
            if e.name.is_empty() {
                return Err(ModelError::EmptyName {
                    context: "an enumeration".into(),
                });
            }
            if e.enumerators.is_empty() {
                return Err(bad("no enumerator"));
            }
            if has_duplicate(&e.enumerators) || e.enumerators.iter().any(String::is_empty) {
                return Err(bad("an enumerator is empty or repeated"));
            }
            if let EnumCoding::Names(n) = &e.coding {
                if n.len() != e.enumerators.len() {
                    return Err(bad(
                        "the coding names a different number of values than the enumerators",
                    ));
                }
                if has_duplicate(n) {
                    return Err(bad("the coding repeats a value"));
                }
            }
            Ok(())
        }
        SimpleType::Struct(s) => {
            if s.name.is_empty() {
                return Err(ModelError::EmptyName {
                    context: "a structure".into(),
                });
            }
            if level > MAX_STRUCT_NESTING {
                return Err(ModelError::BadStruct {
                    name: s.name.clone(),
                    reason: format!("structures nested deeper than {MAX_STRUCT_NESTING}"),
                });
            }
            if s.fields.is_empty() {
                return Err(ModelError::BadStruct {
                    name: s.name.clone(),
                    reason: "no member".into(),
                });
            }
            let names: Vec<String> = s.fields.iter().map(|f| f.name.clone()).collect();
            if has_duplicate(&names) || names.iter().any(String::is_empty) {
                return Err(ModelError::BadStruct {
                    name: s.name.clone(),
                    reason: "a member name is empty or repeated".into(),
                });
            }
            for f in &s.fields {
                check_simple_type(&f.ty, level + 1)?;
            }
            Ok(())
        }
    }
}

fn has_duplicate(v: &[String]) -> bool {
    v.iter().enumerate().any(|(i, a)| v[i + 1..].contains(a))
}

/// zerodds-lint: recursion-depth 64 — one call per nested structure of
/// `t`, a type of the model: [`Model::new`] refuses structures nested
/// deeper than [`MAX_STRUCT_NESTING`], and a value goes down only where
/// its type is a structure.
fn check_simple(member: &str, t: &SimpleType, v: &Value) -> Result<(), TypeError> {
    let mismatch = || TypeError::NotEqual {
        member: member.into(),
        expected: t.describe(),
        found: v.describe(),
    };
    match (t, v) {
        (SimpleType::Basic(BasicType::Long), Value::Long(_))
        | (SimpleType::Basic(BasicType::Short), Value::Short(_))
        | (SimpleType::Basic(BasicType::Char), Value::Char(_))
        | (SimpleType::Basic(BasicType::Octet), Value::Octet(_))
        | (SimpleType::Basic(BasicType::Real), Value::Real(_))
        | (SimpleType::Basic(BasicType::Double), Value::Double(_))
        | (SimpleType::Basic(BasicType::String), Value::String(_)) => Ok(()),
        (SimpleType::Basic(BasicType::Sequence(item)), Value::Sequence(s)) if s.item() == *item => {
            Ok(())
        }
        (
            SimpleType::Enumeration(e),
            Value::Enum {
                type_name,
                enumerator,
            },
        ) if *type_name == e.name => {
            if e.enumerators.contains(enumerator) {
                Ok(())
            } else {
                Err(TypeError::UnknownEnumerator {
                    member: member.into(),
                    enumerator: enumerator.clone(),
                })
            }
        }
        (SimpleType::Struct(s), Value::Struct { type_name, fields }) if *type_name == s.name => {
            if fields.len() != s.fields.len() {
                return Err(mismatch());
            }
            for (decl, (name, value)) in s.fields.iter().zip(fields) {
                if decl.name != *name {
                    return Err(mismatch());
                }
                check_simple(&format!("{member}.{name}"), &decl.ty, value)?;
            }
            Ok(())
        }
        _ => Err(mismatch()),
    }
}
