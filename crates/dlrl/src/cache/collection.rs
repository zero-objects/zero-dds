// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The collections of a DLRL object (DDS 1.2 §8.1.6.3.15 to §8.1.6.3.19):
//! [`Collection`], [`List`], [`Set`], [`StrMap`], [`IntMap`].
//!
//! A collection is the value of one multi-valued member of one object, a
//! `MultiAttribute` (items of a simple type) or a `MultiRelation` (items
//! that designate objects). [`ObjectRoot::get_list`],
//! [`ObjectRoot::get_set`], [`ObjectRoot::get_str_map`] and
//! [`ObjectRoot::get_int_map`] give it, as the generated `get_<attribute>`
//! of §8.1.6.3.14 gives the typed `FooList`, `FooSet`, `FooStrMap` and
//! `FooIntMap`. The handle reads and writes that member of that object:
//!
//! * Reading works on every object that is not destroyed. A member
//!   without value is an empty collection.
//! * Writing (`add`, `put`, `remove`) changes the member as
//!   `ObjectRoot::set` does, with the same checks, made before anything
//!   else (also before a `remove` finds nothing to remove or a `Set::add`
//!   finds the element there): the object must not be destroyed
//!   (`AlreadyDeleted`) and must be in a CacheAccess that allows writing
//!   (`PreconditionNotMet`), and the new value must fit the member
//!   (§8.1.3.1); the object becomes MODIFIED and the change is published
//!   by `CacheAccess::write`.
//! * Every operation of the collection of a destroyed object raises
//!   `AlreadyDeleted` (§8.1.6.2, §8.2.1.1), as the operations of the
//!   object do; the attributes `length`, `values` and `keys` keep their
//!   last value.
//! * `added_elements`, `removed_elements` and `modified_elements` give
//!   the changes of the last update round (an object of the Cache) or of
//!   the last refresh (a clone in a CacheAccess), as
//!   `ObjectRoot::is_modified` does; for a NEW object every element is
//!   added. The modification states are cleaned at the end of an update
//!   round (§8.1.6.4.1).
//! * An index or a key that is not in the collection raises
//!   `NoSuchElement` (§8.1.6.2: "an attempt is made to retrieve a
//!   non-existing element from a Collection"), for `get` and for
//!   `List::put` beyond the end. Removing what is not there is a warning
//!   that does not affect the behaviour, given as a return value
//!   (§8.2.1.1): every `remove` returns TRUE when it removed an element
//!   and FALSE otherwise (`List::remove` on an empty list, the `remove`
//!   of an absent key or element). `Set::add` of an element already
//!   there is ignored, as §8.1.6.3.17 says.
//!
//! The items of a relation collection are references; the `*_object`
//! operations take and give the [`ObjectRoot`]s. An object of another
//! Cache cannot be put in relation (§8.1.6.2: "Only DLRL objects
//! belonging to one Cache can be put in relation").

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::metamodel::DlrlException;
use crate::model::{AttributeType, AttributeValue, CollectionBasis, Member, Multiplicity, Value};
use crate::rows::empty_collection;

use super::object::{ObjectRoot, ObjectState};

/// `Collection` (§8.1.6.3.15): the abstract root of the collections.
pub trait Collection {
    /// `length`: the number of elements.
    fn length(&self) -> usize;
    /// `values`: all elements, in the order of the collection (rank, key
    /// order, the order of the set).
    fn values(&self) -> Vec<Value>;
}

/// The member of one object a collection handle stands for.
#[derive(Clone, Debug, PartialEq)]
struct Binding {
    object: ObjectRoot,
    member: String,
    basis: CollectionBasis,
    relation: bool,
}

impl Binding {
    fn new(
        object: &ObjectRoot,
        member: &str,
        basis: CollectionBasis,
    ) -> Result<Self, DlrlException> {
        let (class, found) = {
            let c = object.lock();
            ObjectRoot::alive_cell(&c)?;
            let found = match c.mapping.model().member(&c.data.class, member) {
                Some((_, Member::Attribute(a))) => match &a.ty {
                    AttributeType::Multi { basis: b, .. } => Some((*b, false)),
                    AttributeType::Mono(_) => None,
                },
                Some((_, Member::Relation(r))) => match r.multiplicity {
                    Multiplicity::ToMany(b) => Some((b, true)),
                    Multiplicity::ToOne => None,
                },
                _ => None,
            };
            (c.data.class.clone(), found)
        };
        match found {
            Some((b, relation)) if b == basis => Ok(Self {
                object: object.clone(),
                member: member.into(),
                basis,
                relation,
            }),
            _ => Err(DlrlException::precondition(alloc::format!(
                "{member} is no {} member of class {class}",
                basis.pattern()
            ))),
        }
    }

    fn current(&self) -> AttributeValue {
        self.object.ensure_loaded();
        self.object
            .lock()
            .data
            .values
            .get(&self.member)
            .cloned()
            .unwrap_or_else(|| empty_collection(self.basis))
    }

    /// The value before the last update round or refresh.
    fn before(&self) -> AttributeValue {
        self.object.ensure_loaded();
        let c = self.object.lock();
        if c.read_state == ObjectState::New {
            return empty_collection(self.basis);
        }
        if !c.modified.contains(&self.member) {
            return c
                .data
                .values
                .get(&self.member)
                .cloned()
                .unwrap_or_else(|| empty_collection(self.basis));
        }
        c.previous
            .get(&self.member)
            .cloned()
            .flatten()
            .unwrap_or_else(|| empty_collection(self.basis))
    }

    fn write(&self, value: AttributeValue) -> Result<(), DlrlException> {
        self.object.set(&self.member, value)
    }

    /// `AlreadyDeleted` for a destroyed object: the check of every
    /// operation that reads.
    fn readable(&self) -> Result<(), DlrlException> {
        self.object.alive()
    }

    /// The checks of `ObjectRoot::set`, made by every operation that
    /// writes before it looks at the elements.
    fn writable(&self) -> Result<(), DlrlException> {
        self.object.check_writable()
    }

    fn item_of(&self, target: &ObjectRoot) -> Result<Value, DlrlException> {
        if !self.relation {
            return Err(DlrlException::precondition(alloc::format!(
                "{} holds values, not objects",
                self.member
            )));
        }
        self.object.reference_value(target)
    }

    fn object_of(&self, item: &Value) -> Result<ObjectRoot, DlrlException> {
        if !self.relation {
            return Err(DlrlException::precondition(alloc::format!(
                "{} holds values, not objects",
                self.member
            )));
        }
        match item {
            Value::Ref(Some(r)) => self.object.resolve(r),
            other => Err(DlrlException::not_found(alloc::format!(
                "the item {} of {} designates no object",
                other.describe(),
                self.member
            ))),
        }
    }

    fn no_such(&self, what: impl core::fmt::Display) -> DlrlException {
        DlrlException::NoSuchElement {
            message: alloc::format!("{} has no element {what}", self.member),
        }
    }
}

fn as_list(v: AttributeValue) -> Vec<Value> {
    match v {
        AttributeValue::List(xs) | AttributeValue::Set(xs) => xs,
        _ => Vec::new(),
    }
}

fn as_str_map(v: AttributeValue) -> BTreeMap<String, Value> {
    match v {
        AttributeValue::StrMap(m) => m,
        _ => BTreeMap::new(),
    }
}

fn as_int_map(v: AttributeValue) -> BTreeMap<i32, Value> {
    match v {
        AttributeValue::IntMap(m) => m,
        _ => BTreeMap::new(),
    }
}

fn index_of(i: usize) -> i32 {
    i32::try_from(i).unwrap_or(i32::MAX)
}

/// The changed keys between two maps: added, removed, modified.
fn map_diff<K: Ord + Clone>(
    before: &BTreeMap<K, Value>,
    now: &BTreeMap<K, Value>,
) -> (Vec<K>, Vec<K>, Vec<K>) {
    let added = now
        .keys()
        .filter(|k| !before.contains_key(*k))
        .cloned()
        .collect();
    let removed = before
        .keys()
        .filter(|k| !now.contains_key(*k))
        .cloned()
        .collect();
    let modified = now
        .iter()
        .filter(|(k, v)| before.get(*k).is_some_and(|b| b != *v))
        .map(|(k, _)| k.clone())
        .collect();
    (added, removed, modified)
}

macro_rules! collection_common {
    ($t:ident) => {
        impl $t {
            /// The object the collection belongs to.
            #[must_use]
            pub fn object(&self) -> &ObjectRoot {
                &self.b.object
            }

            /// The member of the object the collection is the value of.
            #[must_use]
            pub fn member(&self) -> &str {
                &self.b.member
            }
        }
    };
}

/// `List` (§8.1.6.3.16): the elements by rank, from index 0.
#[derive(Clone, Debug, PartialEq)]
pub struct List {
    b: Binding,
}

collection_common!(List);

impl Collection for List {
    fn length(&self) -> usize {
        as_list(self.b.current()).len()
    }

    fn values(&self) -> Vec<Value> {
        as_list(self.b.current())
    }
}

impl List {
    pub(crate) fn new(object: &ObjectRoot, member: &str) -> Result<Self, DlrlException> {
        Binding::new(object, member, CollectionBasis::List).map(|b| Self { b })
    }

    /// `remove`: removes the element with the highest index. TRUE when
    /// an element was removed, FALSE for an empty list: removing what is
    /// not there is a warning, given as a return value (§8.2.1.1).
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`, also for an empty list.
    pub fn remove(&self) -> Result<bool, DlrlException> {
        self.b.writable()?;
        let mut xs = as_list(self.b.current());
        if xs.pop().is_none() {
            return Ok(false);
        }
        self.b.write(AttributeValue::List(xs)).map(|()| true)
    }

    /// `add`: appends `value`.
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`.
    pub fn add(&self, value: Value) -> Result<(), DlrlException> {
        self.b.writable()?;
        let mut xs = as_list(self.b.current());
        xs.push(value);
        self.b.write(AttributeValue::List(xs))
    }

    /// `put`: `value` at `index`; an index equal to the length appends.
    ///
    /// # Errors
    /// `NoSuchElement` for a negative index or one beyond the length; the
    /// errors of `ObjectRoot::set`.
    pub fn put(&self, index: i32, value: Value) -> Result<(), DlrlException> {
        self.b.writable()?;
        let mut xs = as_list(self.b.current());
        match usize::try_from(index) {
            Ok(i) if i < xs.len() => xs[i] = value,
            Ok(i) if i == xs.len() => xs.push(value),
            _ => return Err(self.b.no_such(index)),
        }
        self.b.write(AttributeValue::List(xs))
    }

    /// `get`: the element at `index`.
    ///
    /// # Errors
    /// `NoSuchElement` when there is no element at `index`;
    /// `AlreadyDeleted` for a destroyed object.
    pub fn get(&self, index: i32) -> Result<Value, DlrlException> {
        self.b.readable()?;
        let xs = as_list(self.b.current());
        usize::try_from(index)
            .ok()
            .and_then(|i| xs.get(i).cloned())
            .ok_or_else(|| self.b.no_such(index))
    }

    /// `add` of a relation list: appends a reference to `target`.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a list of values, a target of another
    /// Cache or without oid; the errors of [`List::add`].
    pub fn add_object(&self, target: &ObjectRoot) -> Result<(), DlrlException> {
        self.b.writable()?;
        let item = self.b.item_of(target)?;
        self.add(item)
    }

    /// `put` of a relation list.
    ///
    /// # Errors
    /// As [`List::add_object`] and [`List::put`].
    pub fn put_object(&self, index: i32, target: &ObjectRoot) -> Result<(), DlrlException> {
        self.b.writable()?;
        let item = self.b.item_of(target)?;
        self.put(index, item)
    }

    /// `get` of a relation list: the object at `index`.
    ///
    /// # Errors
    /// `NoSuchElement` when there is no element at `index`; `NotFound`
    /// when the object is not available (§8.1.6.1.2.1);
    /// `PreconditionNotMet` for a list of values.
    pub fn get_object(&self, index: i32) -> Result<ObjectRoot, DlrlException> {
        let item = self.get(index)?;
        self.b.object_of(&item)
    }

    /// `added_elements`: the indexes added by the last update round or
    /// refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn added_elements(&self) -> Result<Vec<i32>, DlrlException> {
        self.b.readable()?;
        let (before, now) = (as_list(self.b.before()), as_list(self.b.current()));
        Ok((before.len()..now.len()).map(index_of).collect())
    }

    /// `removed_elements`: the indexes removed by the last update round
    /// or refresh (those beyond the new length).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn removed_elements(&self) -> Result<Vec<i32>, DlrlException> {
        self.b.readable()?;
        let (before, now) = (as_list(self.b.before()), as_list(self.b.current()));
        Ok((now.len()..before.len()).map(index_of).collect())
    }

    /// `modified_elements`: the indexes present before and after whose
    /// element the last update round or refresh changed.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn modified_elements(&self) -> Result<Vec<i32>, DlrlException> {
        self.b.readable()?;
        let (before, now) = (as_list(self.b.before()), as_list(self.b.current()));
        Ok(before
            .iter()
            .zip(now.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| index_of(i))
            .collect())
    }
}

/// `Set` (§8.1.6.3.17): elements without order, each once.
#[derive(Clone, Debug, PartialEq)]
pub struct Set {
    b: Binding,
}

collection_common!(Set);

impl Collection for Set {
    fn length(&self) -> usize {
        as_list(self.b.current()).len()
    }

    fn values(&self) -> Vec<Value> {
        as_list(self.b.current())
    }
}

impl Set {
    pub(crate) fn new(object: &ObjectRoot, member: &str) -> Result<Self, DlrlException> {
        Binding::new(object, member, CollectionBasis::Set).map(|b| Self { b })
    }

    /// `contains`.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn contains(&self, value: &Value) -> Result<bool, DlrlException> {
        self.b.readable()?;
        Ok(as_list(self.b.current()).contains(value))
    }

    /// `add`: an element already in the set is ignored (§8.1.6.3.17).
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`, also for an element already in
    /// the set.
    pub fn add(&self, value: Value) -> Result<(), DlrlException> {
        self.b.writable()?;
        let mut xs = as_list(self.b.current());
        if xs.contains(&value) {
            return Ok(());
        }
        xs.push(value);
        self.b.write(AttributeValue::Set(xs))
    }

    /// `remove`: TRUE when `value` was removed; an element not in the
    /// set is ignored (§8.1.6.3.17) and gives FALSE, the warning as a
    /// return value (§8.2.1.1).
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`, also for an element not in the
    /// set.
    pub fn remove(&self, value: &Value) -> Result<bool, DlrlException> {
        self.b.writable()?;
        let mut xs = as_list(self.b.current());
        let before = xs.len();
        xs.retain(|x| x != value);
        if xs.len() == before {
            return Ok(false);
        }
        self.b.write(AttributeValue::Set(xs)).map(|()| true)
    }

    /// `contains` of a relation set.
    ///
    /// # Errors
    /// As [`Set::add_object`].
    pub fn contains_object(&self, target: &ObjectRoot) -> Result<bool, DlrlException> {
        self.b.readable()?;
        let item = self.b.item_of(target)?;
        self.contains(&item)
    }

    /// `add` of a relation set.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a set of values, a target of another
    /// Cache or without oid; the errors of [`Set::add`].
    pub fn add_object(&self, target: &ObjectRoot) -> Result<(), DlrlException> {
        self.b.writable()?;
        let item = self.b.item_of(target)?;
        self.add(item)
    }

    /// `remove` of a relation set.
    ///
    /// # Errors
    /// As [`Set::add_object`].
    pub fn remove_object(&self, target: &ObjectRoot) -> Result<bool, DlrlException> {
        self.b.writable()?;
        let item = self.b.item_of(target)?;
        self.remove(&item)
    }

    /// The objects of a relation set.
    ///
    /// # Errors
    /// `NotFound` when an object is not available (§8.1.6.1.2.1);
    /// `PreconditionNotMet` for a set of values; `AlreadyDeleted` for a
    /// destroyed object.
    pub fn objects(&self) -> Result<Vec<ObjectRoot>, DlrlException> {
        self.b.readable()?;
        self.values().iter().map(|v| self.b.object_of(v)).collect()
    }

    /// `added_elements`: the elements added by the last update round or
    /// refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn added_elements(&self) -> Result<Vec<Value>, DlrlException> {
        self.b.readable()?;
        let (before, now) = (as_list(self.b.before()), as_list(self.b.current()));
        Ok(now.into_iter().filter(|x| !before.contains(x)).collect())
    }

    /// `removed_elements`: the elements removed by the last update round
    /// or refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn removed_elements(&self) -> Result<Vec<Value>, DlrlException> {
        self.b.readable()?;
        let (before, now) = (as_list(self.b.before()), as_list(self.b.current()));
        Ok(before.into_iter().filter(|x| !now.contains(x)).collect())
    }
}

/// `StrMap` (§8.1.6.3.18): elements by string key.
#[derive(Clone, Debug, PartialEq)]
pub struct StrMap {
    b: Binding,
}

collection_common!(StrMap);

impl Collection for StrMap {
    fn length(&self) -> usize {
        as_str_map(self.b.current()).len()
    }

    fn values(&self) -> Vec<Value> {
        as_str_map(self.b.current()).into_values().collect()
    }
}

impl StrMap {
    pub(crate) fn new(object: &ObjectRoot, member: &str) -> Result<Self, DlrlException> {
        Binding::new(object, member, CollectionBasis::StrMap).map(|b| Self { b })
    }

    /// `keys`.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        as_str_map(self.b.current()).into_keys().collect()
    }

    /// `remove`: TRUE when the element under `key` was removed, FALSE
    /// when `key` is not in the map (a warning, given as a return value,
    /// §8.2.1.1).
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`, also for a key not in the map.
    pub fn remove(&self, key: &str) -> Result<bool, DlrlException> {
        self.b.writable()?;
        let mut m = as_str_map(self.b.current());
        if m.remove(key).is_none() {
            return Ok(false);
        }
        self.b.write(AttributeValue::StrMap(m)).map(|()| true)
    }

    /// `put`: `value` under `key`, replacing an element there.
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`.
    pub fn put(&self, key: &str, value: Value) -> Result<(), DlrlException> {
        self.b.writable()?;
        let mut m = as_str_map(self.b.current());
        m.insert(key.into(), value);
        self.b.write(AttributeValue::StrMap(m))
    }

    /// `get`.
    ///
    /// # Errors
    /// `NoSuchElement` when `key` is not in the map; `AlreadyDeleted` for
    /// a destroyed object.
    pub fn get(&self, key: &str) -> Result<Value, DlrlException> {
        self.b.readable()?;
        as_str_map(self.b.current())
            .remove(key)
            .ok_or_else(|| self.b.no_such(alloc::format!("'{key}'")))
    }

    /// `put` of a relation map.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a map of values, a target of another
    /// Cache or without oid; the errors of [`StrMap::put`].
    pub fn put_object(&self, key: &str, target: &ObjectRoot) -> Result<(), DlrlException> {
        self.b.writable()?;
        let item = self.b.item_of(target)?;
        self.put(key, item)
    }

    /// `get` of a relation map.
    ///
    /// # Errors
    /// `NoSuchElement` when `key` is not in the map; `NotFound` when the
    /// object is not available; `PreconditionNotMet` for a map of values.
    pub fn get_object(&self, key: &str) -> Result<ObjectRoot, DlrlException> {
        let item = self.get(key)?;
        self.b.object_of(&item)
    }

    /// `added_elements`: the keys added by the last update round or
    /// refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn added_elements(&self) -> Result<Vec<String>, DlrlException> {
        self.b.readable()?;
        Ok(map_diff(&as_str_map(self.b.before()), &as_str_map(self.b.current())).0)
    }

    /// `removed_elements`: the keys removed by the last update round or
    /// refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn removed_elements(&self) -> Result<Vec<String>, DlrlException> {
        self.b.readable()?;
        Ok(map_diff(&as_str_map(self.b.before()), &as_str_map(self.b.current())).1)
    }

    /// `modified_elements`: the keys whose element the last update round
    /// or refresh changed.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn modified_elements(&self) -> Result<Vec<String>, DlrlException> {
        self.b.readable()?;
        Ok(map_diff(&as_str_map(self.b.before()), &as_str_map(self.b.current())).2)
    }
}

/// `IntMap` (§8.1.6.3.19): elements by integer key. Its `keys` are a
/// `LongSeq` as in the IDL of §8.2.1.2.1 (the table of §8.1.6.3.19 says
/// `string[]`).
#[derive(Clone, Debug, PartialEq)]
pub struct IntMap {
    b: Binding,
}

collection_common!(IntMap);

impl Collection for IntMap {
    fn length(&self) -> usize {
        as_int_map(self.b.current()).len()
    }

    fn values(&self) -> Vec<Value> {
        as_int_map(self.b.current()).into_values().collect()
    }
}

impl IntMap {
    pub(crate) fn new(object: &ObjectRoot, member: &str) -> Result<Self, DlrlException> {
        Binding::new(object, member, CollectionBasis::IntMap).map(|b| Self { b })
    }

    /// `keys`.
    #[must_use]
    pub fn keys(&self) -> Vec<i32> {
        as_int_map(self.b.current()).into_keys().collect()
    }

    /// `remove`: TRUE when the element under `key` was removed, FALSE
    /// when `key` is not in the map (a warning, given as a return value,
    /// §8.2.1.1).
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`, also for a key not in the map.
    pub fn remove(&self, key: i32) -> Result<bool, DlrlException> {
        self.b.writable()?;
        let mut m = as_int_map(self.b.current());
        if m.remove(&key).is_none() {
            return Ok(false);
        }
        self.b.write(AttributeValue::IntMap(m)).map(|()| true)
    }

    /// `put`: `value` under `key`, replacing an element there.
    ///
    /// # Errors
    /// The errors of `ObjectRoot::set`.
    pub fn put(&self, key: i32, value: Value) -> Result<(), DlrlException> {
        self.b.writable()?;
        let mut m = as_int_map(self.b.current());
        m.insert(key, value);
        self.b.write(AttributeValue::IntMap(m))
    }

    /// `get`.
    ///
    /// # Errors
    /// `NoSuchElement` when `key` is not in the map; `AlreadyDeleted` for
    /// a destroyed object.
    pub fn get(&self, key: i32) -> Result<Value, DlrlException> {
        self.b.readable()?;
        as_int_map(self.b.current())
            .remove(&key)
            .ok_or_else(|| self.b.no_such(key))
    }

    /// `put` of a relation map.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a map of values, a target of another
    /// Cache or without oid; the errors of [`IntMap::put`].
    pub fn put_object(&self, key: i32, target: &ObjectRoot) -> Result<(), DlrlException> {
        self.b.writable()?;
        let item = self.b.item_of(target)?;
        self.put(key, item)
    }

    /// `get` of a relation map.
    ///
    /// # Errors
    /// `NoSuchElement` when `key` is not in the map; `NotFound` when the
    /// object is not available; `PreconditionNotMet` for a map of values.
    pub fn get_object(&self, key: i32) -> Result<ObjectRoot, DlrlException> {
        let item = self.get(key)?;
        self.b.object_of(&item)
    }

    /// `added_elements`: the keys added by the last update round or
    /// refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn added_elements(&self) -> Result<Vec<i32>, DlrlException> {
        self.b.readable()?;
        Ok(map_diff(&as_int_map(self.b.before()), &as_int_map(self.b.current())).0)
    }

    /// `removed_elements`: the keys removed by the last update round or
    /// refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn removed_elements(&self) -> Result<Vec<i32>, DlrlException> {
        self.b.readable()?;
        Ok(map_diff(&as_int_map(self.b.before()), &as_int_map(self.b.current())).1)
    }

    /// `modified_elements`: the keys whose element the last update round
    /// or refresh changed.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn modified_elements(&self) -> Result<Vec<i32>, DlrlException> {
        self.b.readable()?;
        Ok(map_diff(&as_int_map(self.b.before()), &as_int_map(self.b.current())).2)
    }
}
