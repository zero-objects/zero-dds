// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The rows of the DCPS tables of a mapping (DDS 1.2 §8.1.4.2).
//!
//! §8.1.4.2 looks at DCPS data as tables: a topic is a table, an
//! instance is a row identified by its key value, a field is a column.
//! [`Mapping::rows_of`] writes the rows of one object; the rules are
//! those of §8.1.4.2:
//!
//! * the object has a row in the main topic of its class
//!   (§8.1.4.2.1); its key is the oid, or the full oid where the main
//!   topic needs a class part (§8.1.4.2.2);
//! * a mono-valued attribute or relation is one or several cells of one
//!   row keyed like the object: one cell for a basic value, one integer
//!   or string cell for an enumeration, one cell per leaf of a simple
//!   structure, the oid or full oid of a referenced object
//!   (§8.1.4.2.3); a reference to no object is an empty class part
//!   with [`DlrlOid::NIL`], and the nil oid is refused as the oid of an
//!   object, in a reference to an object and in the owner key of a row
//!   ([`RowError::NilOid`], [`TypeError::NilOid`]);
//! * a multi-valued attribute or relation is one row per item, keyed by
//!   the owner's key plus the index: the rank for a list, the key for a
//!   map. A set has no index; its item completes the key.
//!
//! [`Mapping::reconstruct`] reads rows back into objects. An object
//! exists if, and only if, it has a row in its main topic
//! (§8.1.4.2.1, §8.2.2.3.2.7); rows of other topics without that row
//! create nothing and are reported in [`Reconstruction::skipped`].
//!
//! A class mapped onto an existing DCPS model (§8.1.4.5, `NoOid` key)
//! has no oid on the wire: its key fields hold the values of its
//! identity attributes ([`crate::mapping::ClassMapping::identity`]), and
//! a reference to such an object holds those values in its target
//! fields. [`KeyAssociation`] keeps the association of key values and
//! oid: [`Mapping::rows_of_with`] looks up the key of a referenced
//! object there, [`Mapping::reconstruct_with`] gives each key value
//! met in a row its oid, the same one each time.
//!
//! The conversion of a row into a DCPS sample of the topic's type is in
//! `crate::dcps_binding`.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use portable_atomic::{AtomicU32, Ordering};

use crate::mapping::{
    CellType, KeyContent, KeyDescription, Mapping, MemberMapping, TopicDescription,
};
use crate::model::{
    AttributeType, AttributeValue, CollectionBasis, EnumCoding, Member, ObjectRef, SequenceValue,
    SimpleType, TypeError, Value,
};
use crate::types::DlrlOid;

/// The content of one DCPS cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// `long`.
    Long(i32),
    /// `short`.
    Short(i16),
    /// `char`.
    Char(char),
    /// `octet`.
    Octet(u8),
    /// `float`.
    Real(f32),
    /// `double`.
    Double(f64),
    /// `string`.
    String(String),
    /// `sequence<…>`.
    Sequence(SequenceValue),
    /// `DLRLOid`.
    Oid(DlrlOid),
}

impl Cell {
    /// True when the cell is of the field type `t`.
    #[must_use]
    pub fn fits(&self, t: CellType) -> bool {
        matches!(
            (self, t),
            (Self::Long(_), CellType::Long)
                | (Self::Short(_), CellType::Short)
                | (Self::Char(_), CellType::Char)
                | (Self::Octet(_), CellType::Octet)
                | (Self::Real(_), CellType::Real)
                | (Self::Double(_), CellType::Double)
                | (Self::String(_), CellType::String)
                | (Self::Oid(_), CellType::Oid)
        ) || matches!((self, t), (Self::Sequence(s), CellType::Sequence(i)) if s.item() == i)
    }
}

/// One row of a topic: the cells of one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Topic name.
    pub topic: String,
    /// The cells by field name; key fields first.
    pub cells: Vec<(String, Cell)>,
}

impl Row {
    /// The cell of a field.
    #[must_use]
    pub fn get(&self, field: &str) -> Option<&Cell> {
        self.cells.iter().find(|(n, _)| n == field).map(|(_, c)| c)
    }

    /// The key cells, in the order of the topic's key fields.
    #[must_use]
    pub fn key<'a>(&'a self, topic: &TopicDescription) -> Vec<Option<&'a Cell>> {
        topic.key_fields().iter().map(|k| self.get(k)).collect()
    }
}

/// The shared state of one object: its class, its oid and the values of
/// its shared members.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectData {
    /// The exact class.
    pub class: String,
    /// The oid.
    pub oid: DlrlOid,
    /// Values by member name.
    pub values: BTreeMap<String, AttributeValue>,
}

impl ObjectData {
    /// An object without values.
    #[must_use]
    pub fn new(class: &str, oid: DlrlOid) -> Self {
        Self {
            class: class.into(),
            oid,
            values: BTreeMap::new(),
        }
    }

    /// Sets the value of a member.
    #[must_use]
    pub fn with(mut self, member: &str, value: AttributeValue) -> Self {
        self.values.insert(member.into(), value);
        self
    }

    /// The reference to this object.
    #[must_use]
    pub fn reference(&self) -> ObjectRef {
        ObjectRef::new(&self.class, self.oid)
    }
}

/// The association of key values and oids for the classes with a
/// `NoOid` key (§8.1.4.5): "the DLRL implementation must indicate the
/// topic fields that are used to store the keys so that, behind the
/// scenes, it can manage the association keys to/from oid and perform
/// the needed indirection."
///
/// The oid of such an object stays local. One key value of one class
/// has one oid for as long as the association lives; an oid it
/// allocates is `[creator.0, creator.1, n]`, with `n` counting from 1
/// ([`KeyAssociation::with_creator`]) or taken from the counter of the
/// process ([`KeyAssociation::process_wide`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KeyAssociation {
    creator: (i32, i32),
    next: i32,
    /// `Some(seed)`: the third value of an oid is `seed` plus
    /// [`PROCESS_OIDS`].
    process: Option<u32>,
    by_key: BTreeMap<(String, String), DlrlOid>,
    by_oid: BTreeMap<(String, DlrlOid), Vec<Cell>>,
}

/// The counter that every association made with
/// [`KeyAssociation::process_wide`] in this process draws from.
static PROCESS_OIDS: AtomicU32 = AtomicU32::new(0);

impl KeyAssociation {
    /// An empty association whose oids carry `creator` in their first
    /// two values and its own count from 1 in the third.
    #[must_use]
    pub fn with_creator(a: i32, b: i32) -> Self {
        Self {
            creator: (a, b),
            ..Self::default()
        }
    }

    /// An empty association whose oids carry `creator` in their first
    /// two values and, in the third, `seed` plus one counter that all
    /// associations made with this call in the process share: two of
    /// them with one creator and one seed never hand out one oid (until
    /// the counter has gone round its 2^32 values).
    #[must_use]
    pub fn process_wide(a: i32, b: i32, seed: u32) -> Self {
        Self {
            creator: (a, b),
            process: Some(seed),
            ..Self::default()
        }
    }

    /// A fresh oid, never [`DlrlOid::NIL`], never handed out before by
    /// this association (nor by another process-wide one with the same
    /// creator and seed).
    pub fn allocate(&mut self) -> DlrlOid {
        let n = match self.process {
            Some(seed) => loop {
                let n = seed.wrapping_add(PROCESS_OIDS.fetch_add(1, Ordering::Relaxed));
                if n != 0 {
                    break i32::from_ne_bytes(n.to_ne_bytes());
                }
            },
            None => {
                self.next = self.next.wrapping_add(1);
                if self.next == 0 {
                    self.next = 1;
                }
                self.next
            }
        };
        DlrlOid::new(self.creator.0, self.creator.1, n)
    }

    fn canonical(key: &[Cell]) -> String {
        // Debug of the cells is exact for every cell kind (floats print
        // the shortest form that reads back to the same value).
        format!("{key:?}")
    }

    /// The oid of the object of `class` with the key values `key`; a
    /// fresh one, then associated, when the key is met the first time.
    pub fn oid_for(&mut self, class: &str, key: &[Cell]) -> DlrlOid {
        let k = (String::from(class), Self::canonical(key));
        if let Some(oid) = self.by_key.get(&k) {
            return *oid;
        }
        let oid = self.allocate();
        self.by_key.insert(k, oid);
        self.by_oid.insert((class.into(), oid), key.to_vec());
        oid
    }

    /// The oid associated with a key, if any.
    #[must_use]
    pub fn lookup(&self, class: &str, key: &[Cell]) -> Option<DlrlOid> {
        self.by_key
            .get(&(String::from(class), Self::canonical(key)))
            .copied()
    }

    /// Associates `oid` with the key values `key` of an object of
    /// `class`.
    ///
    /// # Errors
    /// The key is associated with another oid (the `AlreadyExisting` of
    /// §8.1.6.3.7 `register_object`), or the oid with another key; the
    /// error carries the oid the key or the oid is associated with.
    pub fn associate(&mut self, class: &str, oid: DlrlOid, key: &[Cell]) -> Result<(), DlrlOid> {
        let k = (String::from(class), Self::canonical(key));
        if let Some(existing) = self.by_key.get(&k) {
            return if *existing == oid {
                Ok(())
            } else {
                Err(*existing)
            };
        }
        if self.by_oid.contains_key(&(class.into(), oid)) {
            return Err(oid);
        }
        self.by_key.insert(k, oid);
        self.by_oid.insert((class.into(), oid), key.to_vec());
        Ok(())
    }

    /// The key values associated with an oid.
    #[must_use]
    pub fn key_of(&self, class: &str, oid: DlrlOid) -> Option<&[Cell]> {
        self.by_oid
            .get(&(String::from(class), oid))
            .map(Vec::as_slice)
    }

    /// Drops the association of an oid (the object is gone).
    pub fn forget(&mut self, class: &str, oid: DlrlOid) {
        if let Some(key) = self.by_oid.remove(&(String::from(class), oid)) {
            self.by_key
                .remove(&(String::from(class), Self::canonical(&key)));
        }
    }
}

/// Why a row did not contribute to an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The designated object has no row in its main topic, so it does
    /// not exist (§8.1.4.2.1).
    NoMainRow {
        /// The class of the designated object.
        class: String,
        /// Its oid.
        oid: DlrlOid,
    },
    /// The class part names a class the local model does not have
    /// (§8.1.4.5: the object model is not imposed on all participants).
    UnknownClass(String),
    /// The class part names a class of the model that has no rows in
    /// this topic.
    NotMappedHere(String),
}

/// A row that did not contribute to an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRow {
    /// The topic of the row.
    pub topic: String,
    /// Why.
    pub reason: SkipReason,
}

/// The objects reconstructed from a set of rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Reconstruction {
    /// The existing objects, ordered by class and oid.
    pub objects: Vec<ObjectData>,
    /// The rows that contributed to no object.
    pub skipped: Vec<SkippedRow>,
}

impl Reconstruction {
    /// An object by class and oid.
    #[must_use]
    pub fn object(&self, class: &str, oid: DlrlOid) -> Option<&ObjectData> {
        self.objects
            .iter()
            .find(|o| o.class == class && o.oid == oid)
    }
}

/// A row or an object that cannot be converted.
#[derive(Debug, Clone, PartialEq)]
pub enum RowError {
    /// The class of the object is not in the mapping.
    UnknownClass(String),
    /// The object carries a value for a member its class does not have.
    UnknownMember {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// The value of a shared mono-valued member is missing; every row of
    /// the object carries all its cells.
    MissingValue {
        /// The class.
        class: String,
        /// The member.
        member: String,
    },
    /// The value does not fit the member (§8.1.3.1).
    Type(TypeError),
    /// A reference to an object of a class with a `NoOid` key whose key
    /// values the [`KeyAssociation`] does not hold (§8.1.4.5).
    UnknownKey {
        /// The class of the referenced object.
        class: String,
        /// Its oid.
        oid: DlrlOid,
    },
    /// A to-1 relation to a class with a `NoOid` key that designates no
    /// object: the key fields of an existing DCPS model have no value
    /// for "no object" (§8.1.4.5).
    NoObjectInNoOidReference {
        /// The topic.
        topic: String,
        /// The relation.
        relation: String,
    },
    /// A row of a topic the mapping does not use.
    UnknownTopic(String),
    /// A field of the topic is missing in the row.
    MissingField {
        /// The topic.
        topic: String,
        /// The field.
        field: String,
    },
    /// A cell that does not fit its field or the value it stands for.
    BadCell {
        /// The topic.
        topic: String,
        /// The field.
        field: String,
        /// What is wrong.
        reason: String,
    },
    /// Two rows with the same key.
    DuplicateRow {
        /// The topic.
        topic: String,
    },
    /// [`DlrlOid::NIL`] as the oid of an object, or in the owner key of
    /// a row. The nil oid designates no object; it only stands for an
    /// unset to-1 relation.
    NilOid {
        /// The class of the object.
        class: String,
        /// The topic of the row.
        topic: String,
    },
    /// The ranks of a list do not run from 0 without a gap.
    ListGap {
        /// The class.
        class: String,
        /// The oid.
        oid: DlrlOid,
        /// The member.
        member: String,
    },
}

impl core::fmt::Display for RowError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownClass(c) => write!(f, "class {c} is not in the mapping"),
            Self::UnknownMember { class, member } => {
                write!(f, "class {class} has no shared member {member}")
            }
            Self::MissingValue { class, member } => {
                write!(f, "object of class {class} has no value for {member}")
            }
            Self::Type(e) => write!(f, "{e}"),
            Self::UnknownKey { class, oid } => write!(
                f,
                "object {oid} of class {class}: no key values are associated with this oid (DDS 1.2 §8.1.4.5)"
            ),
            Self::NoObjectInNoOidReference { topic, relation } => write!(
                f,
                "topic {topic}: relation {relation} designates no object, and the NoOid key of its target has no value for that"
            ),
            Self::UnknownTopic(t) => write!(f, "topic {t} is not used by the mapping"),
            Self::MissingField { topic, field } => {
                write!(f, "row of topic {topic} has no field {field}")
            }
            Self::BadCell {
                topic,
                field,
                reason,
            } => write!(f, "field {field} of topic {topic}: {reason}"),
            Self::DuplicateRow { topic } => write!(f, "two rows of topic {topic} with one key"),
            Self::NilOid { class, topic } => write!(
                f,
                "row of topic {topic}: the nil DLRLOid designates no object and cannot be the oid of an object of class {class}"
            ),
            Self::ListGap { class, oid, member } => write!(
                f,
                "list {class}::{member} of object {oid}: the ranks do not run from 0 without a gap"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RowError {}

impl From<TypeError> for RowError {
    fn from(e: TypeError) -> Self {
        Self::Type(e)
    }
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

impl Mapping {
    /// The rows of one object: the main row first, then one row per
    /// other topic that holds a mono-valued member, then one row per
    /// collection item.
    ///
    /// A row carries the cells of the object's members; fields of the
    /// topic that only members of other classes use are absent from it.
    ///
    /// # Errors
    /// [`RowError`] for an unknown class or member, an object whose oid
    /// is [`DlrlOid::NIL`], a missing or ill-typed value (a reference to
    /// the nil oid among them), or a reference to an object of a class
    /// with a `NoOid` key (it needs [`Mapping::rows_of_with`]).
    pub fn rows_of(&self, object: &ObjectData) -> Result<Vec<Row>, RowError> {
        self.rows_of_with(object, &KeyAssociation::default())
    }

    /// The identity of an object of a class with a `NoOid` key: the
    /// cells of its identity attributes, in key order (§8.1.4.5).
    ///
    /// # Errors
    /// [`RowError`] for an unknown class, a missing or ill-typed
    /// identity value.
    pub fn identity_key(&self, object: &ObjectData) -> Result<Vec<Cell>, RowError> {
        let cm = self
            .class(&object.class)
            .ok_or_else(|| RowError::UnknownClass(object.class.clone()))?;
        let mut cells = Vec::with_capacity(cm.identity.len());
        for a in &cm.identity {
            let Some(AttributeValue::Mono(v)) = object.values.get(a) else {
                return Err(RowError::MissingValue {
                    class: object.class.clone(),
                    member: a.clone(),
                });
            };
            self.model()
                .check_value(&object.class, a, &AttributeValue::Mono(v.clone()))?;
            simple_to_cells(
                self.simple_type(&object.class, a)?,
                v,
                &mut cells,
                &cm.main_topic.name,
            )?;
        }
        Ok(cells)
    }

    /// [`Mapping::rows_of`] with the key values of the referenced
    /// objects of `NoOid` classes taken from `keys`. The key of the
    /// object itself, for a `NoOid` class, is its identity
    /// ([`Mapping::identity_key`]).
    ///
    /// # Errors
    /// As [`Mapping::rows_of`]; [`RowError::UnknownKey`] for a reference
    /// whose key `keys` does not hold, and
    /// [`RowError::NoObjectInNoOidReference`] for a to-1 relation to a
    /// `NoOid` class that designates no object.
    pub fn rows_of_with(
        &self,
        object: &ObjectData,
        keys: &KeyAssociation,
    ) -> Result<Vec<Row>, RowError> {
        let cm = self
            .class(&object.class)
            .ok_or_else(|| RowError::UnknownClass(object.class.clone()))?;
        if object.oid.is_nil() {
            return Err(RowError::NilOid {
                class: object.class.clone(),
                topic: cm.main_topic.name.clone(),
            });
        }
        for (name, v) in &object.values {
            match self.model().member(&object.class, name) {
                Some((_, Member::Local)) | None => {
                    return Err(RowError::UnknownMember {
                        class: object.class.clone(),
                        member: name.clone(),
                    });
                }
                Some(_) => self.model().check_value(&object.class, name, v)?,
            }
        }
        let identity = if cm.main_topic.key.content == KeyContent::NoOid {
            Some(self.identity_key(object)?)
        } else {
            None
        };
        let owner_key = |key: &KeyDescription, topic: &str| match &identity {
            Some(cells) => Ok(key
                .fields
                .iter()
                .cloned()
                .zip(cells.iter().cloned())
                .collect()),
            None => key_cells(key, &object.class, object.oid, topic),
        };

        let mut rows: Vec<Row> = alloc::vec![Row {
            topic: cm.main_topic.name.clone(),
            cells: owner_key(&cm.main_topic.key, &cm.main_topic.name)?,
        }];
        let mut collections: Vec<Row> = Vec::new();
        for m in self.members_of_class(&object.class) {
            let value = object.values.get(m.name());
            let topic = m.topic();
            let missing = || RowError::MissingValue {
                class: object.class.clone(),
                member: m.name().into(),
            };
            match m {
                MemberMapping::MonoAttribute(a) => {
                    let Some(AttributeValue::Mono(v)) = value else {
                        return Err(missing());
                    };
                    let ty = self.simple_type(&object.class, &a.attribute)?;
                    let mut cells = Vec::new();
                    simple_to_cells(ty, v, &mut cells, &topic.name)?;
                    let row = row_for(&mut rows, &topic.name, || {
                        owner_key(&topic.key, &topic.name)
                    })?;
                    for (name, cell) in a.value_fields.iter().cloned().zip(cells) {
                        // An identity attribute of a NoOid class: its value
                        // field is a key field, already in the row.
                        if row.get(&name).is_none() {
                            row.cells.push((name, cell));
                        }
                    }
                }
                MemberMapping::MonoRelation(r) => {
                    let Some(AttributeValue::Mono(Value::Ref(target))) = value else {
                        return Err(missing());
                    };
                    let cells = reference_cells(
                        &r.target_key,
                        target.as_ref(),
                        keys,
                        &topic.name,
                        &r.relation,
                    )?;
                    let row = row_for(&mut rows, &topic.name, || {
                        owner_key(&topic.key, &topic.name)
                    })?;
                    row.cells.extend(cells);
                }
                MemberMapping::MultiAttribute(a) => {
                    let ty = self.simple_type(&object.class, &a.attribute)?;
                    for (index, item) in collection_items(value, &topic.name)? {
                        let mut cells = owner_key(&topic.key, &topic.name)?;
                        if let (Some(f), Some(i)) = (&a.index_field, index) {
                            cells.push((f.clone(), i));
                        }
                        let mut values = Vec::new();
                        simple_to_cells(ty, item, &mut values, &topic.name)?;
                        cells.extend(a.value_fields.iter().cloned().zip(values));
                        collections.push(Row {
                            topic: topic.name.clone(),
                            cells,
                        });
                    }
                }
                MemberMapping::MultiRelation(r) => {
                    for (index, item) in collection_items(value, &topic.name)? {
                        let mut cells = owner_key(&topic.key, &topic.name)?;
                        if let (Some(f), Some(i)) = (&r.index_field, index) {
                            cells.push((f.clone(), i));
                        }
                        let Value::Ref(Some(target)) = item else {
                            return Err(RowError::Type(TypeError::NilItem {
                                member: r.relation.clone(),
                            }));
                        };
                        cells.extend(reference_cells(
                            &r.target_key,
                            Some(target),
                            keys,
                            &topic.name,
                            &r.relation,
                        )?);
                        collections.push(Row {
                            topic: topic.name.clone(),
                            cells,
                        });
                    }
                }
            }
        }
        rows.extend(collections);
        Ok(rows)
    }

    fn simple_type(&self, class: &str, member: &str) -> Result<&SimpleType, RowError> {
        match self.model().member(class, member) {
            Some((_, Member::Attribute(a))) => match &a.ty {
                AttributeType::Mono(t) | AttributeType::Multi { item: t, .. } => Ok(t),
            },
            _ => Err(RowError::UnknownMember {
                class: class.into(),
                member: member.into(),
            }),
        }
    }
}

fn row_for<'r>(
    rows: &'r mut Vec<Row>,
    topic: &str,
    key: impl FnOnce() -> Result<Vec<(String, Cell)>, RowError>,
) -> Result<&'r mut Row, RowError> {
    let pos = match rows.iter().position(|r| r.topic == topic) {
        Some(p) => p,
        None => {
            rows.push(Row {
                topic: topic.into(),
                cells: key()?,
            });
            rows.len() - 1
        }
    };
    Ok(&mut rows[pos])
}

fn key_cells(
    key: &KeyDescription,
    class: &str,
    oid: DlrlOid,
    topic: &str,
) -> Result<Vec<(String, Cell)>, RowError> {
    match (key.content, key.fields.as_slice()) {
        (KeyContent::FullOid, [c, o]) => Ok(alloc::vec![
            (c.clone(), Cell::String(class.into())),
            (o.clone(), Cell::Oid(oid)),
        ]),
        (KeyContent::SimpleOid, [o]) => Ok(alloc::vec![(o.clone(), Cell::Oid(oid))]),
        // Mapping::new checks the number of key fields per content; a
        // NoOid key is filled from the identity, not here.
        (_, _) => Err(bad_cell(
            topic,
            "",
            "key description with a wrong number of fields",
        )),
    }
}

fn reference_cells(
    key: &KeyDescription,
    target: Option<&ObjectRef>,
    keys: &KeyAssociation,
    topic: &str,
    relation: &str,
) -> Result<Vec<(String, Cell)>, RowError> {
    match (key.content, target) {
        (KeyContent::NoOid, Some(t)) => {
            let cells = keys.key_of(&t.class, t.oid).ok_or(RowError::UnknownKey {
                class: t.class.clone(),
                oid: t.oid,
            })?;
            if cells.len() != key.fields.len() {
                return Err(bad_cell(
                    topic,
                    "",
                    "the associated key values do not match the target fields",
                ));
            }
            Ok(key
                .fields
                .iter()
                .cloned()
                .zip(cells.iter().cloned())
                .collect())
        }
        (KeyContent::NoOid, None) => Err(RowError::NoObjectInNoOidReference {
            topic: topic.into(),
            relation: relation.into(),
        }),
        (_, Some(t)) => key_cells(key, &t.class, t.oid, topic),
        // A reference to no object: empty class part, nil oid.
        (_, None) => key_cells(key, "", DlrlOid::NIL, topic),
    }
}

fn collection_items<'v>(
    value: Option<&'v AttributeValue>,
    topic: &str,
) -> Result<Vec<(Option<Cell>, &'v Value)>, RowError> {
    let mut out = Vec::new();
    match value {
        None | Some(AttributeValue::Mono(_)) => {}
        Some(AttributeValue::List(v)) => {
            for (i, x) in v.iter().enumerate() {
                let rank = i32::try_from(i)
                    .map_err(|_| bad_cell(topic, "", "a list rank beyond the range of a long"))?;
                out.push((Some(Cell::Long(rank)), x));
            }
        }
        Some(AttributeValue::Set(v)) => out.extend(v.iter().map(|x| (None, x))),
        Some(AttributeValue::StrMap(m)) => {
            out.extend(m.iter().map(|(k, x)| (Some(Cell::String(k.clone())), x)));
        }
        Some(AttributeValue::IntMap(m)) => {
            out.extend(m.iter().map(|(k, x)| (Some(Cell::Long(*k)), x)));
        }
    }
    Ok(out)
}

/// The cells of the value `v` of the model type `ty`, one per leaf.
///
/// zerodds-lint: recursion-depth 64 — one call per nested structure of
/// `ty`, a type of the mapping's model, which
/// [`crate::model::Model::new`] keeps within
/// [`crate::model::MAX_STRUCT_NESTING`].
fn simple_to_cells(
    ty: &SimpleType,
    v: &Value,
    out: &mut Vec<Cell>,
    topic: &str,
) -> Result<(), RowError> {
    let mismatch = || {
        bad_cell(
            topic,
            "",
            &format!(
                "a value of {} for a member of type {}",
                v.describe(),
                ty.describe()
            ),
        )
    };
    match (ty, v) {
        (SimpleType::Enumeration(e), Value::Enum { enumerator, .. }) => {
            let Some(pos) = e.enumerators.iter().position(|x| x == enumerator) else {
                return Err(mismatch());
            };
            out.push(match &e.coding {
                EnumCoding::Integer => Cell::Long(i32::try_from(pos).map_err(|_| mismatch())?),
                EnumCoding::Names(n) => Cell::String(n.get(pos).ok_or_else(mismatch)?.clone()),
            });
        }
        (SimpleType::Struct(s), Value::Struct { fields, .. }) => {
            if fields.len() != s.fields.len() {
                return Err(mismatch());
            }
            for (decl, (_, value)) in s.fields.iter().zip(fields) {
                simple_to_cells(&decl.ty, value, out, topic)?;
            }
        }
        (SimpleType::Basic(_), Value::Long(x)) => out.push(Cell::Long(*x)),
        (SimpleType::Basic(_), Value::Short(x)) => out.push(Cell::Short(*x)),
        (SimpleType::Basic(_), Value::Char(x)) => out.push(Cell::Char(*x)),
        (SimpleType::Basic(_), Value::Octet(x)) => out.push(Cell::Octet(*x)),
        (SimpleType::Basic(_), Value::Real(x)) => out.push(Cell::Real(*x)),
        (SimpleType::Basic(_), Value::Double(x)) => out.push(Cell::Double(*x)),
        (SimpleType::Basic(_), Value::String(x)) => out.push(Cell::String(x.clone())),
        (SimpleType::Basic(_), Value::Sequence(x)) => out.push(Cell::Sequence(x.clone())),
        (_, _) => return Err(mismatch()),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// The owner of a row as designated by its key.
enum Owner {
    Object(String, DlrlOid),
    Skip(SkipReason),
}

type ListItems = BTreeMap<(String, DlrlOid, String), BTreeMap<i32, Value>>;

impl Mapping {
    /// Reconstructs the objects from the rows of the mapping's topics.
    ///
    /// An object exists if, and only if, it has a row in the main topic
    /// of its class. A mono-valued member whose row is absent has no
    /// value; a collection without rows is empty.
    ///
    /// # Errors
    /// [`RowError`] for a row of an unknown topic, a missing field, a
    /// cell of the wrong type, two rows with one key, or a list with a
    /// gap in its ranks.
    pub fn reconstruct<'a>(
        &self,
        rows: impl IntoIterator<Item = &'a Row>,
    ) -> Result<Reconstruction, RowError> {
        self.reconstruct_with(rows, &mut KeyAssociation::default())
    }

    /// [`Mapping::reconstruct`] with the oids of the objects of `NoOid`
    /// classes, and of the references to them, taken from `keys`: a key
    /// value met the first time gets a fresh oid, which stays associated
    /// with it.
    ///
    /// # Errors
    /// As [`Mapping::reconstruct`].
    pub fn reconstruct_with<'a>(
        &self,
        rows: impl IntoIterator<Item = &'a Row>,
        keys: &mut KeyAssociation,
    ) -> Result<Reconstruction, RowError> {
        let rows: Vec<&Row> = rows.into_iter().collect();
        let mut objects: BTreeMap<(String, DlrlOid), ObjectData> = BTreeMap::new();
        let mut skipped = Vec::new();
        let mut owners: Vec<Owner> = Vec::with_capacity(rows.len());

        // Pass 1: the owner of each row, and the objects from the main
        // rows. A main topic is never the topic of a collection.
        for row in &rows {
            let topic = self
                .topic(&row.topic)
                .ok_or_else(|| RowError::UnknownTopic(row.topic.clone()))?;
            let owner = self.owner_of(topic, row, keys)?;
            if let Owner::Object(class, oid) = &owner {
                let is_main = self
                    .class(class)
                    .is_some_and(|cm| cm.main_topic.name == topic.name);
                if is_main
                    && objects
                        .insert((class.clone(), *oid), ObjectData::new(class, *oid))
                        .is_some()
                {
                    return Err(RowError::DuplicateRow {
                        topic: topic.name.clone(),
                    });
                }
            }
            owners.push(owner);
        }

        // Pass 2: the values.
        let mut seen_rows: Vec<(String, String, DlrlOid)> = Vec::new();
        let mut list_items: ListItems = BTreeMap::new();
        for (row, owner) in rows.iter().zip(owners) {
            let (class, oid) = match owner {
                Owner::Skip(reason) => {
                    skipped.push(SkippedRow {
                        topic: row.topic.clone(),
                        reason,
                    });
                    continue;
                }
                Owner::Object(c, o) => (c, o),
            };
            let Some(object) = objects.get_mut(&(class.clone(), oid)) else {
                skipped.push(SkippedRow {
                    topic: row.topic.clone(),
                    reason: SkipReason::NoMainRow { class, oid },
                });
                continue;
            };
            let members = self.members_of_class(&class);
            let here: Vec<&MemberMapping> = members
                .into_iter()
                .filter(|m| m.topic().name == row.topic)
                .collect();
            if here.iter().all(|m| !m.is_multi()) {
                let id = (row.topic.clone(), class.clone(), oid);
                if seen_rows.contains(&id) {
                    return Err(RowError::DuplicateRow {
                        topic: row.topic.clone(),
                    });
                }
                seen_rows.push(id);
            }
            for m in here {
                self.read_member(m, row, object, &mut list_items, keys)?;
            }
        }

        // Lists from their ranks; empty collections for the rest.
        for ((class, oid, member), items) in list_items {
            let mut out = Vec::with_capacity(items.len());
            for (expected, (rank, v)) in items.into_iter().enumerate() {
                if usize::try_from(rank).ok() != Some(expected) {
                    return Err(RowError::ListGap { class, oid, member });
                }
                out.push(v);
            }
            if let Some(o) = objects.get_mut(&(class, oid)) {
                o.values.insert(member, AttributeValue::List(out));
            }
        }
        for object in objects.values_mut() {
            for m in self.members_of_class(&object.class) {
                let basis = match m {
                    MemberMapping::MultiAttribute(a) => Some(a.basis),
                    MemberMapping::MultiRelation(r) => Some(r.basis),
                    MemberMapping::MonoAttribute(_) | MemberMapping::MonoRelation(_) => None,
                };
                if let Some(basis) = basis {
                    object
                        .values
                        .entry(m.name().into())
                        .or_insert_with(|| empty_collection(basis));
                }
            }
        }
        Ok(Reconstruction {
            objects: objects.into_values().collect(),
            skipped,
        })
    }

    fn owner_of(
        &self,
        topic: &TopicDescription,
        row: &Row,
        keys: &mut KeyAssociation,
    ) -> Result<Owner, RowError> {
        let key = topic.key_fields();
        match (topic.key_content, key.as_slice()) {
            // Mapping::new admits a NoOid key only for a topic with rows
            // of a single class; the owner key is the first fields of the
            // key, as many as the class has identity attributes.
            (KeyContent::NoOid, _) => {
                let [class] = topic.classes.as_slice() else {
                    return Err(bad_cell(&topic.name, "", "no single class for a NoOid key"));
                };
                let width = self.class(class).map_or(0, |cm| cm.identity.len());
                let cells = key_values(topic, row, &key[..width.min(key.len())])?;
                Ok(Owner::Object(class.clone(), keys.oid_for(class, &cells)))
            }
            (KeyContent::FullOid, [class_field, oid_field, ..]) => {
                let Cell::String(class) = field(row, class_field)? else {
                    return Err(bad_cell(&topic.name, class_field, "not a class name"));
                };
                let oid = owner_oid(row, oid_field, class, &topic.name)?;
                if self.model().class(class).is_none() {
                    return Ok(Owner::Skip(SkipReason::UnknownClass(class.clone())));
                }
                if !topic.classes.contains(class) {
                    return Ok(Owner::Skip(SkipReason::NotMappedHere(class.clone())));
                }
                Ok(Owner::Object(class.clone(), oid))
            }
            // Mapping::new admits a key without class part only for a
            // topic with rows of a single class.
            (KeyContent::SimpleOid, [oid_field, ..]) => match topic.classes.as_slice() {
                [class] => Ok(Owner::Object(
                    class.clone(),
                    owner_oid(row, oid_field, class, &topic.name)?,
                )),
                _ => Err(bad_cell(
                    &topic.name,
                    oid_field,
                    "no single class for a key without class part",
                )),
            },
            (_, _) => Err(bad_cell(
                &topic.name,
                "",
                "key description with a wrong number of fields",
            )),
        }
    }

    fn read_member(
        &self,
        m: &MemberMapping,
        row: &Row,
        object: &mut ObjectData,
        lists: &mut ListItems,
        keys: &mut KeyAssociation,
    ) -> Result<(), RowError> {
        let class_name = object.class.clone();
        let class = class_name.as_str();
        let oid = object.oid;
        let topic = row.topic.as_str();
        let duplicate = || RowError::DuplicateRow {
            topic: topic.into(),
        };
        match m {
            MemberMapping::MonoAttribute(a) => {
                let ty = self.simple_type(class, &a.attribute)?;
                let v = cells_to_simple(ty, &mut a.value_fields.iter(), row)?;
                object
                    .values
                    .insert(a.attribute.clone(), AttributeValue::Mono(v));
            }
            MemberMapping::MonoRelation(r) => {
                let target = self.read_reference(&r.relation, class, &r.target_key, row, keys)?;
                object
                    .values
                    .insert(r.relation.clone(), AttributeValue::Mono(Value::Ref(target)));
            }
            MemberMapping::MultiAttribute(a) => {
                let ty = self.simple_type(class, &a.attribute)?;
                let v = cells_to_simple(ty, &mut a.value_fields.iter(), row)?;
                let index = match &a.index_field {
                    Some(f) => Some(field(row, f)?),
                    None => None,
                };
                let at = (class, oid, a.attribute.as_str());
                insert_item(object, lists, at, a.basis, index, v).map_err(|()| duplicate())?;
            }
            MemberMapping::MultiRelation(r) => {
                let Some(target) =
                    self.read_reference(&r.relation, class, &r.target_key, row, keys)?
                else {
                    return Err(RowError::Type(TypeError::NilItem {
                        member: r.relation.clone(),
                    }));
                };
                let index = match &r.index_field {
                    Some(f) => Some(field(row, f)?),
                    None => None,
                };
                let at = (class, oid, r.relation.as_str());
                insert_item(object, lists, at, r.basis, index, Value::Ref(Some(target)))
                    .map_err(|()| duplicate())?;
            }
        }
        Ok(())
    }

    fn read_reference(
        &self,
        relation: &str,
        class: &str,
        key: &KeyDescription,
        row: &Row,
        keys: &mut KeyAssociation,
    ) -> Result<Option<ObjectRef>, RowError> {
        let Some((_, Member::Relation(rel))) = self.model().member(class, relation) else {
            return Err(RowError::UnknownMember {
                class: class.into(),
                member: relation.into(),
            });
        };
        let (target_class, oid, class_field) = match (key.content, key.fields.as_slice()) {
            (KeyContent::NoOid, fields) => {
                let topic = self
                    .topic(&row.topic)
                    .ok_or_else(|| RowError::UnknownTopic(row.topic.clone()))?;
                let names: Vec<&str> = fields.iter().map(String::as_str).collect();
                let cells = key_values(topic, row, &names)?;
                return Ok(Some(ObjectRef::new(
                    &rel.target,
                    keys.oid_for(&rel.target, &cells),
                )));
            }
            (KeyContent::FullOid, [c, o]) => {
                let Cell::String(name) = field(row, c)? else {
                    return Err(bad_cell(&row.topic, c, "not a class name"));
                };
                (name.clone(), oid_cell(row, o)?, Some(c))
            }
            (KeyContent::SimpleOid, [o]) => (rel.target.clone(), oid_cell(row, o)?, None),
            (_, _) => {
                return Err(bad_cell(
                    &row.topic,
                    "",
                    "key description with a wrong number of fields",
                ));
            }
        };
        if oid.is_nil() {
            // A reference to no object is written as an empty class part
            // with the nil oid; a class part beside the nil oid names an
            // object that the oid cannot designate.
            if let Some(c) = class_field
                && !target_class.is_empty()
            {
                return Err(bad_cell(
                    &row.topic,
                    c,
                    "a class part beside the nil DLRLOid, which designates no object",
                ));
            }
            return Ok(None);
        }
        if !self.model().is_kind_of(&target_class, &rel.target) {
            return Err(RowError::Type(TypeError::NotIncluded {
                member: relation.into(),
                target: rel.target.clone(),
                found: target_class,
            }));
        }
        Ok(Some(ObjectRef::new(&target_class, oid)))
    }
}

/// The cells of `names` in a row, each checked against its field type.
fn key_values(topic: &TopicDescription, row: &Row, names: &[&str]) -> Result<Vec<Cell>, RowError> {
    let mut out = Vec::with_capacity(names.len());
    for n in names {
        let cell = field(row, n)?;
        let fits = topic.field(n).is_some_and(|f| cell.fits(f.cell));
        if !fits {
            return Err(bad_cell(
                &topic.name,
                n,
                "not a cell of the key field's type",
            ));
        }
        out.push(cell.clone());
    }
    Ok(out)
}

fn field<'r>(row: &'r Row, name: &str) -> Result<&'r Cell, RowError> {
    row.get(name).ok_or_else(|| RowError::MissingField {
        topic: row.topic.clone(),
        field: name.into(),
    })
}

fn oid_cell(row: &Row, name: &str) -> Result<DlrlOid, RowError> {
    match field(row, name)? {
        Cell::Oid(o) => Ok(*o),
        _ => Err(bad_cell(&row.topic, name, "not a DLRLOid")),
    }
}

/// The oid of the owner key of a row; the nil oid designates no object.
fn owner_oid(row: &Row, name: &str, class: &str, topic: &str) -> Result<DlrlOid, RowError> {
    let oid = oid_cell(row, name)?;
    if oid.is_nil() {
        return Err(RowError::NilOid {
            class: class.into(),
            topic: topic.into(),
        });
    }
    Ok(oid)
}

fn insert_item(
    object: &mut ObjectData,
    lists: &mut ListItems,
    at: (&str, DlrlOid, &str),
    basis: CollectionBasis,
    index: Option<&Cell>,
    value: Value,
) -> Result<(), ()> {
    let (class, oid, member) = at;
    let fresh = match basis {
        CollectionBasis::List => {
            let Some(Cell::Long(i)) = index else {
                return Err(());
            };
            lists
                .entry((class.into(), oid, member.into()))
                .or_default()
                .insert(*i, value)
                .is_none()
        }
        CollectionBasis::IntMap => {
            let Some(Cell::Long(i)) = index else {
                return Err(());
            };
            match object
                .values
                .entry(member.into())
                .or_insert_with(|| AttributeValue::IntMap(BTreeMap::new()))
            {
                AttributeValue::IntMap(m) => m.insert(*i, value).is_none(),
                _ => false,
            }
        }
        CollectionBasis::StrMap => {
            let Some(Cell::String(k)) = index else {
                return Err(());
            };
            match object
                .values
                .entry(member.into())
                .or_insert_with(|| AttributeValue::StrMap(BTreeMap::new()))
            {
                AttributeValue::StrMap(m) => m.insert(k.clone(), value).is_none(),
                _ => false,
            }
        }
        CollectionBasis::Set => match object
            .values
            .entry(member.into())
            .or_insert_with(|| AttributeValue::Set(Vec::new()))
        {
            AttributeValue::Set(v) if !v.contains(&value) => {
                v.push(value);
                true
            }
            _ => false,
        },
    };
    if fresh { Ok(()) } else { Err(()) }
}

pub(crate) fn empty_collection(basis: CollectionBasis) -> AttributeValue {
    match basis {
        CollectionBasis::List => AttributeValue::List(Vec::new()),
        CollectionBasis::Set => AttributeValue::Set(Vec::new()),
        CollectionBasis::StrMap => AttributeValue::StrMap(BTreeMap::new()),
        CollectionBasis::IntMap => AttributeValue::IntMap(BTreeMap::new()),
    }
}

/// The value of the model type `ty` from the cells of `row` that `fields`
/// names, one field per leaf.
///
/// zerodds-lint: recursion-depth 64 — one call per nested structure of
/// `ty`, a type of the mapping's model, which
/// [`crate::model::Model::new`] keeps within
/// [`crate::model::MAX_STRUCT_NESTING`].
fn cells_to_simple<'f>(
    ty: &SimpleType,
    fields: &mut impl Iterator<Item = &'f String>,
    row: &Row,
) -> Result<Value, RowError> {
    if let SimpleType::Struct(s) = ty {
        let mut out = Vec::with_capacity(s.fields.len());
        for f in &s.fields {
            out.push((f.name.clone(), cells_to_simple(&f.ty, fields, row)?));
        }
        return Ok(Value::Struct {
            type_name: s.name.clone(),
            fields: out,
        });
    }
    let Some(name) = fields.next() else {
        return Err(bad_cell(
            &row.topic,
            "",
            "fewer value fields than the type has cells",
        ));
    };
    let cell = field(row, name)?;
    let bad = |reason: String| bad_cell(&row.topic, name, &reason);
    if let SimpleType::Enumeration(e) = ty {
        let pos = match (&e.coding, cell) {
            (EnumCoding::Integer, Cell::Long(i)) => usize::try_from(*i).ok(),
            (EnumCoding::Names(n), Cell::String(s)) => n.iter().position(|x| x == s),
            _ => return Err(bad(format!("not a cell of enumeration {}", e.name))),
        };
        let Some(enumerator) = pos.and_then(|p| e.enumerators.get(p)) else {
            return Err(bad(format!("no enumerator of {} for this cell", e.name)));
        };
        return Ok(Value::Enum {
            type_name: e.name.clone(),
            enumerator: enumerator.clone(),
        });
    }
    let mut cells = Vec::new();
    crate::mapping::simple_cells(ty, &mut cells);
    if !cells.first().is_some_and(|t| cell.fits(*t)) {
        return Err(bad(format!("not a cell of type {}", ty.describe())));
    }
    match cell.clone() {
        Cell::Long(x) => Ok(Value::Long(x)),
        Cell::Short(x) => Ok(Value::Short(x)),
        Cell::Char(x) => Ok(Value::Char(x)),
        Cell::Octet(x) => Ok(Value::Octet(x)),
        Cell::Real(x) => Ok(Value::Real(x)),
        Cell::Double(x) => Ok(Value::Double(x)),
        Cell::String(x) => Ok(Value::String(x)),
        Cell::Sequence(x) => Ok(Value::Sequence(x)),
        Cell::Oid(_) => Err(bad("a DLRLOid is not a basic value".into())),
    }
}

fn bad_cell(topic: &str, field: &str, reason: &str) -> RowError {
    RowError::BadCell {
        topic: topic.into(),
        field: field.into(),
        reason: reason.into(),
    }
}
