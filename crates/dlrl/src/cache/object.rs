// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectRoot` (DDS 1.2 §8.1.6.3.14): one DLRL object, primary in a
//! [`Cache`] or a clone in a [`CacheAccess`].

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use std::sync::{Mutex, MutexGuard};

use crate::mapping::Mapping;
use crate::metamodel::DlrlException;
use crate::model::{AttributeValue, Member, Multiplicity, ObjectRef, Value};
use crate::rows::ObjectData;
use crate::types::{DlrlOid, RelationDescription};

use super::access::{AccessInner, CacheAccess};
use super::collection::{IntMap, List, Set, StrMap};
use super::home::{HomeInner, ObjectHome};
use super::{Cache, CacheBase, CacheInner, CacheUsage};

/// PSM `ObjectState` (§8.2.1): the `read_state` and `write_state` of an
/// object (Figures 8.5 and 8.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectState {
    /// `OBJECT_VOID`: the state does not apply.
    Void,
    /// `OBJECT_NEW`.
    New,
    /// `OBJECT_NOT_MODIFIED`.
    NotModified,
    /// `OBJECT_MODIFIED`.
    Modified,
    /// `OBJECT_DELETED`.
    Deleted,
}

/// PSM `ObjectScope` (§8.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectScope {
    /// `SIMPLE_OBJECT_SCOPE`: only the object.
    SimpleObject,
    /// `CONTAINED_OBJECTS_SCOPE`: the object and its (nested)
    /// compositions.
    ContainedObjects,
    /// `RELATED_OBJECTS_SCOPE`: the object, its compositions and the
    /// objects navigable from it.
    RelatedObjects,
}

/// Where an object lives.
#[derive(Clone)]
pub(crate) enum Owner {
    Cache(Weak<CacheInner>),
    Access(Weak<AccessInner>),
}

pub(crate) struct ObjectCell {
    pub(crate) data: ObjectData,
    pub(crate) read_state: ObjectState,
    pub(crate) write_state: ObjectState,
    /// The attributes changed by the last update round (primary) or the
    /// last refresh (clone).
    pub(crate) modified: BTreeSet<String>,
    /// The values the members of `modified` had before that round or
    /// refresh (absent: the member had no value), for the
    /// `added_elements`, `removed_elements` and `modified_elements` of a
    /// collection (§8.1.6.3.16 to §8.1.6.3.19).
    pub(crate) previous: BTreeMap<String, Option<AttributeValue>>,
    pub(crate) home: Weak<HomeInner>,
    pub(crate) owner: Owner,
    pub(crate) mapping: Arc<Mapping>,
    /// False for an object of `create_unregistered_object` until
    /// `register_object`.
    pub(crate) registered: bool,
    /// Writable: a clone in a CacheAccess that allows writing.
    pub(crate) writable: bool,
    /// The state (the values of `data`) is loaded into the object. False
    /// only for an object of a Cache whose home has `auto_deref` FALSE
    /// (§8.1.6.3.7) and that was not accessed since it arrived or since
    /// `underef_all`; `data` then holds class and oid only, the state is
    /// in the rows of the Cache's tables.
    pub(crate) loaded: bool,
    /// The owner key of the object's rows in the Cache's tables; `None`
    /// for an object of a CacheAccess.
    pub(crate) owner_key: Option<super::OwnerId>,
}

/// `ObjectRoot` (§8.1.6.3.14): the handle of one DLRL object. Clones of
/// the handle designate the same object; a [`Cache`] and a
/// [`CacheAccess`] hold one object per identity.
///
/// The generated `get_<attribute>`, `set_<attribute>` and
/// `is_<attribute>_modified` of §8.1.6.3.14 are [`ObjectRoot::get`],
/// [`ObjectRoot::set`] and [`ObjectRoot::is_attribute_modified`] with
/// the attribute's name; navigation through a relation is
/// [`ObjectRoot::get_reference`] and [`ObjectRoot::get_references`].
#[derive(Clone)]
pub struct ObjectRoot {
    pub(crate) cell: Arc<Mutex<ObjectCell>>,
}

impl core::fmt::Debug for ObjectRoot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let c = self.lock();
        f.debug_struct("ObjectRoot")
            .field("class", &c.data.class)
            .field("oid", &c.data.oid)
            .field("read_state", &c.read_state)
            .field("write_state", &c.write_state)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ObjectRoot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cell, &other.cell)
    }
}

impl Eq for ObjectRoot {}

impl ObjectRoot {
    pub(crate) fn new(cell: ObjectCell) -> Self {
        Self {
            cell: Arc::new(Mutex::new(cell)),
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, ObjectCell> {
        self.cell
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `AlreadyDeleted` for a destroyed object: write_state DELETED, by
    /// `destroy` (also after the `write` that published the destruction)
    /// or by the `purge` or deletion of a CacheAccess that allows writing
    /// (§8.1.6.2, §8.2.1.1). Every operation of a destroyed object raises
    /// it; its attributes (`oid`, `read_state`, `write_state`,
    /// `object_home`, `class_name`, `owner`) keep their last value. An
    /// object with read_state DELETED (deleted by another participant,
    /// or no longer contracted at a `refresh`) is not destroyed: the
    /// listeners of a deletion and `get_deleted_objects` give it to the
    /// application, which reads its last state (§8.1.6.4).
    pub(crate) fn alive_cell(c: &ObjectCell) -> Result<(), DlrlException> {
        if c.write_state == ObjectState::Deleted {
            Err(DlrlException::AlreadyDeleted {
                message: alloc::format!("object {} is destroyed", c.data.oid),
            })
        } else {
            Ok(())
        }
    }

    /// [`ObjectRoot::alive_cell`] of this object.
    pub(crate) fn alive(&self) -> Result<(), DlrlException> {
        Self::alive_cell(&self.lock())
    }

    /// The checks of every modification, in this order: `AlreadyDeleted`
    /// for a destroyed object, `PreconditionNotMet` for an object that is
    /// not in a CacheAccess that allows writing (§8.1.6.3.4: the objects
    /// of the Cache are not modifiable).
    pub(crate) fn writable_cell(c: &ObjectCell) -> Result<(), DlrlException> {
        Self::alive_cell(c)?;
        if c.writable {
            Ok(())
        } else {
            Err(DlrlException::precondition(
                "the object is not in a CacheAccess that allows writing",
            ))
        }
    }

    /// [`ObjectRoot::writable_cell`] of this object.
    pub(crate) fn check_writable(&self) -> Result<(), DlrlException> {
        Self::writable_cell(&self.lock())
    }

    /// `oid`: the identity. [`DlrlOid::NIL`] for an object of
    /// `create_unregistered_object` until `register_object`.
    #[must_use]
    pub fn oid(&self) -> DlrlOid {
        self.lock().data.oid
    }

    /// The exact class of the object.
    #[must_use]
    pub fn class_name(&self) -> String {
        self.lock().data.class.clone()
    }

    /// The reference to the object (class and oid).
    #[must_use]
    pub fn reference(&self) -> ObjectRef {
        self.lock().data.reference()
    }

    /// `read_state` (Figures 8.5, 8.6).
    #[must_use]
    pub fn read_state(&self) -> ObjectState {
        self.lock().read_state
    }

    /// `write_state` (Figure 8.6); [`ObjectState::Void`] for an object of
    /// the Cache.
    #[must_use]
    pub fn write_state(&self) -> ObjectState {
        self.lock().write_state
    }

    /// `object_home`: the home of the object's class.
    #[must_use]
    pub fn object_home(&self) -> Option<ObjectHome> {
        self.lock().home.upgrade().map(|inner| ObjectHome { inner })
    }

    /// `owner`: the Cache or the CacheAccess the object belongs to.
    #[must_use]
    pub fn owner(&self) -> Option<CacheBase> {
        match self.lock().owner.clone() {
            Owner::Cache(w) => w.upgrade().map(|inner| CacheBase::Cache(Cache { inner })),
            Owner::Access(w) => w
                .upgrade()
                .map(|inner| CacheBase::Access(CacheAccess { inner })),
        }
    }

    /// The shared state: class, oid and the values of the shared members.
    /// Loads the state of an object whose home has `auto_deref` FALSE
    /// (§8.1.6.3.7).
    #[must_use]
    pub fn data(&self) -> ObjectData {
        self.ensure_loaded();
        self.lock().data.clone()
    }

    /// Whether the state of the object is loaded into it (§8.1.6.3.7,
    /// `auto_deref`): always TRUE for an object of a CacheAccess and for
    /// an object of a Cache whose home has `auto_deref` TRUE; for a home
    /// with `auto_deref` FALSE, TRUE once the application accessed the
    /// state (`get`, `data`, navigation, a collection,
    /// `Cache::deref_reference`) or called `deref_all`, until
    /// `underef_all`. Not an attribute of the PIM; it makes the effect of
    /// `auto_deref` visible.
    #[must_use]
    pub fn is_dereferenced(&self) -> bool {
        self.lock().loaded
    }

    /// Loads the state of an object of a Cache that is not loaded: the
    /// object is rebuilt from the rows of the Cache's tables.
    pub(crate) fn ensure_loaded(&self) {
        let (owner, key) = {
            let c = self.lock();
            if c.loaded {
                return;
            }
            (c.owner.clone(), c.owner_key.clone())
        };
        let (Owner::Cache(w), Some(key)) = (owner, key) else {
            return;
        };
        if let Some(inner) = w.upgrade() {
            Cache { inner }.load_state(self, &key);
        }
    }

    /// The current state of the object without loading it: the loaded
    /// one, or for an object that is not loaded the state rebuilt from
    /// the rows of its Cache. For the work of the service itself
    /// (`is_modified`, a `QueryCriterion`, a Selection's comparison,
    /// cloning into a CacheAccess), which is no access of the
    /// application.
    pub(crate) fn current_data(&self) -> ObjectData {
        let (owner, key, stripped) = {
            let c = self.lock();
            if c.loaded {
                return c.data.clone();
            }
            (c.owner.clone(), c.owner_key.clone(), c.data.clone())
        };
        let (Owner::Cache(w), Some(key)) = (owner, key) else {
            return stripped;
        };
        w.upgrade()
            .and_then(|inner| Cache { inner }.state_of(&key, &stripped))
            .unwrap_or(stripped)
    }

    /// `get_<attribute>`: the value of a shared attribute or relation (a
    /// relation as references, see [`ObjectRoot::get_reference`] to
    /// navigate).
    ///
    /// # Errors
    /// `PreconditionNotMet` for a name that is no shared member of the
    /// class; `NoSuchElement` for a mono-valued member that has no value
    /// yet (an object created and not yet given one); `AlreadyDeleted`
    /// for a destroyed object.
    pub fn get(&self, member: &str) -> Result<AttributeValue, DlrlException> {
        self.alive()?;
        self.ensure_loaded();
        let c = self.lock();
        match c.mapping.model().member(&c.data.class, member) {
            Some((_, Member::Attribute(_) | Member::Relation(_))) => {}
            _ => {
                return Err(DlrlException::precondition(alloc::format!(
                    "class {} has no shared member {member}",
                    c.data.class
                )));
            }
        }
        c.data
            .values
            .get(member)
            .cloned()
            .ok_or_else(|| DlrlException::NoSuchElement {
                message: alloc::format!("{member} of {} has no value", c.data.oid),
            })
    }

    /// `set_<attribute>`: a new value of a shared attribute or relation,
    /// noted for the next `CacheAccess::write`.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the object is not in a CacheAccess that
    /// allows writing (§8.1.6.3.4: the objects of the Cache are not
    /// modifiable), when the value does not fit the member (§8.1.3.1),
    /// and for a key field of a registered object of a `NoOid` class
    /// (§8.1.6.3.14); `AlreadyDeleted` for a destroyed object, also
    /// after the `write` that published its destruction, and for an
    /// object of a CacheAccess that allows writing after its `purge`.
    pub fn set(&self, member: &str, value: AttributeValue) -> Result<(), DlrlException> {
        let mut c = self.lock();
        Self::writable_cell(&c)?;
        c.mapping
            .model()
            .check_value(&c.data.class, member, &value)
            .map_err(|e| DlrlException::precondition(alloc::format!("{e}")))?;
        let identity = c
            .mapping
            .class(&c.data.class)
            .is_some_and(|cm| cm.identity.iter().any(|a| a == member));
        if identity && c.registered {
            return Err(DlrlException::precondition(alloc::format!(
                "{member} is a key field of the NoOid class {}; the identity of a registered object cannot change",
                c.data.class
            )));
        }
        c.data.values.insert(member.into(), value);
        if c.write_state == ObjectState::NotModified {
            c.write_state = ObjectState::Modified;
        }
        Ok(())
    }

    /// `is_<attribute>_modified`: the attribute was changed by the
    /// incoming modifications of the last update round (an object of the
    /// Cache) or by the last refresh (a clone).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn is_attribute_modified(&self, member: &str) -> Result<bool, DlrlException> {
        let c = self.lock();
        Self::alive_cell(&c)?;
        Ok(c.modified.contains(member))
    }

    /// `is_modified`: the object was modified by incoming modifications
    /// (an object of the Cache) or by the last refresh (a clone); with
    /// [`ObjectScope::ContainedObjects`] also one of its (nested)
    /// compositions, with [`ObjectScope::RelatedObjects`] also one of
    /// the objects it designates. FALSE for a newly created object.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn is_modified(&self, scope: ObjectScope) -> Result<bool, DlrlException> {
        self.alive()?;
        Ok(self.modified(scope))
    }

    /// `is_modified` without the check of a destroyed object, for the
    /// service's own work.
    fn modified(&self, scope: ObjectScope) -> bool {
        let mut seen = BTreeSet::new();
        self.modified_in(scope, &mut seen)
    }

    fn modified_in(&self, scope: ObjectScope, seen: &mut BTreeSet<ObjectRef>) -> bool {
        let (own, mapping) = {
            let c = self.lock();
            if !seen.insert(c.data.reference()) {
                return false;
            }
            (
                c.read_state == ObjectState::Modified,
                Arc::clone(&c.mapping),
            )
        };
        if own {
            return true;
        }
        let mut refs = Vec::new();
        if scope != ObjectScope::SimpleObject {
            let data = self.current_data();
            for (name, v) in &data.values {
                let Some((_, Member::Relation(r))) = mapping.model().member(&data.class, name)
                else {
                    continue;
                };
                if scope == ObjectScope::ContainedObjects && !r.is_composition {
                    continue;
                }
                refs.extend(references_in(v).into_iter().map(|t| (t, r.is_composition)));
            }
        }
        let Some(base) = self.owner() else {
            return false;
        };
        refs.into_iter().any(|(target, composition)| {
            base.lookup(&target).is_some_and(|o| {
                if composition {
                    o.modified_in(scope, seen)
                } else {
                    o.modified_in(ObjectScope::SimpleObject, seen)
                }
            })
        })
    }

    /// `which_contained_modified` (§8.1.6.3.14): the descriptions of the
    /// composition relations of this object that designate a modified
    /// contained object, one per designated object: the name of the
    /// relation, and for a `List` the index, for an `IntMap` or a
    /// `StrMap` the key of that object. A contained object is modified
    /// as `is_modified(CONTAINED_OBJECTS_SCOPE)` says: its contents
    /// changed by the incoming modifications (the last update round for
    /// an object of a Cache, the last `refresh` for a clone), or those of
    /// an object it contains. A newly created contained object is not
    /// modified (§8.1.6.3.14). The order is that of the relations' names,
    /// then that of the collection.
    ///
    /// The PSM's `RelationKind` has no value for a `Set`, and a set has
    /// no index or key: a `Set` relation with at least one modified
    /// contained object is described once, by [`RelationDescription::Set`],
    /// whose kind is the extension
    /// [`RelationKind::SetRelation`](crate::types::RelationKind::SetRelation).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a destroyed object.
    pub fn which_contained_modified(&self) -> Result<Vec<RelationDescription>, DlrlException> {
        let mapping = {
            let c = self.lock();
            Self::alive_cell(&c)?;
            Arc::clone(&c.mapping)
        };
        let data = self.current_data();
        let Some(base) = self.owner() else {
            return Ok(Vec::new());
        };
        let modified = |item: &Value| -> bool {
            let Value::Ref(Some(r)) = item else {
                return false;
            };
            base.lookup(r)
                .is_some_and(|o| o.modified(ObjectScope::ContainedObjects))
        };
        let mut out = Vec::new();
        for (name, v) in &data.values {
            let Some((_, Member::Relation(r))) = mapping.model().member(&data.class, name) else {
                continue;
            };
            if !r.is_composition {
                continue;
            }
            match v {
                AttributeValue::Mono(x) => {
                    if modified(x) {
                        out.push(RelationDescription::Ref { name: name.clone() });
                    }
                }
                AttributeValue::List(xs) => {
                    for (i, x) in xs.iter().enumerate() {
                        if modified(x) {
                            out.push(RelationDescription::List {
                                name: name.clone(),
                                index: i32::try_from(i).unwrap_or(i32::MAX),
                            });
                        }
                    }
                }
                AttributeValue::Set(xs) => {
                    // A set has no index or key that tells its objects
                    // apart: one description for the relation.
                    if xs.iter().any(&modified) {
                        out.push(RelationDescription::Set { name: name.clone() });
                    }
                }
                AttributeValue::StrMap(m) => {
                    for (k, x) in m {
                        if modified(x) {
                            out.push(RelationDescription::StrMap {
                                name: name.clone(),
                                key: k.clone(),
                            });
                        }
                    }
                }
                AttributeValue::IntMap(m) => {
                    for (k, x) in m {
                        if modified(x) {
                            out.push(RelationDescription::IntMap {
                                name: name.clone(),
                                key: *k,
                            });
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// `destroy`: marks the object for destruction at the next
    /// `CacheAccess::write`. The objects it contains through a
    /// composition (§8.1.3.2.2) are destroyed with it: those in the same
    /// CacheAccess, and those of the Cache, which are cloned into the
    /// CacheAccess for that.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the object is not in a CacheAccess that
    /// allows writing; `AlreadyDeleted` when it is already destroyed (as
    /// for [`ObjectRoot::set`]).
    pub fn destroy(&self) -> Result<(), DlrlException> {
        let constituents = {
            let mut c = self.lock();
            Self::writable_cell(&c)?;
            c.write_state = ObjectState::Deleted;
            let mut out = Vec::new();
            for (name, v) in &c.data.values {
                if let Some((_, Member::Relation(r))) =
                    c.mapping.model().member(&c.data.class, name)
                    && r.is_composition
                {
                    out.extend(references_in(v));
                }
            }
            out
        };
        let Some(CacheBase::Access(access)) = self.owner() else {
            return Ok(());
        };
        for target in constituents {
            if let Some(o) = access.adopt_for_destruction(&target) {
                match o.destroy() {
                    Ok(()) | Err(DlrlException::AlreadyDeleted { .. }) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    /// Navigates a to-1 relation: the object it designates, `None` when
    /// it designates no object.
    ///
    /// # Errors
    /// `NotFound` when the target's class has no subscription in the
    /// Cache (§8.1.6.1.2.1: navigating to an object for which no
    /// subscription exists raises NotFound), or the object has not (yet)
    /// been received, or is not in this CacheAccess; `PreconditionNotMet`
    /// for a name that is no to-1 relation; `AlreadyDeleted` for a
    /// destroyed object.
    pub fn get_reference(&self, relation: &str) -> Result<Option<ObjectRoot>, DlrlException> {
        self.alive()?;
        self.ensure_loaded();
        let target = {
            let c = self.lock();
            match c.mapping.model().member(&c.data.class, relation) {
                Some((_, Member::Relation(r))) if r.multiplicity == Multiplicity::ToOne => {}
                _ => {
                    return Err(DlrlException::precondition(alloc::format!(
                        "{relation} is no to-1 relation of {}",
                        c.data.class
                    )));
                }
            }
            match c.data.values.get(relation) {
                Some(AttributeValue::Mono(Value::Ref(r))) => r.clone(),
                _ => None,
            }
        };
        match target {
            None => Ok(None),
            Some(t) => self.resolve(&t).map(Some),
        }
    }

    /// Navigates a to-many relation: the objects it designates, in the
    /// order of the collection (rank, key, or the order of the set).
    ///
    /// # Errors
    /// As [`ObjectRoot::get_reference`], for each designated object.
    pub fn get_references(&self, relation: &str) -> Result<Vec<ObjectRoot>, DlrlException> {
        self.alive()?;
        self.ensure_loaded();
        let targets = {
            let c = self.lock();
            match c.mapping.model().member(&c.data.class, relation) {
                Some((_, Member::Relation(r))) if r.multiplicity != Multiplicity::ToOne => {}
                _ => {
                    return Err(DlrlException::precondition(alloc::format!(
                        "{relation} is no to-many relation of {}",
                        c.data.class
                    )));
                }
            }
            c.data
                .values
                .get(relation)
                .map(references_in)
                .unwrap_or_default()
        };
        targets.iter().map(|t| self.resolve(t)).collect()
    }

    /// The objects this object contains through compositions
    /// (§8.1.3.2.2), nested, that its Cache or CacheAccess holds.
    pub(crate) fn contained_objects(&self) -> Vec<ObjectRoot> {
        let Some(base) = self.owner() else {
            return Vec::new();
        };
        let mut out: Vec<ObjectRoot> = Vec::new();
        let mut seen: BTreeSet<ObjectRef> = BTreeSet::new();
        seen.insert(self.reference());
        let mut stack: Vec<ObjectRoot> = alloc::vec![self.clone()];
        while let Some(o) = stack.pop() {
            let targets: Vec<ObjectRef> = {
                let mapping = Arc::clone(&o.lock().mapping);
                let data = o.current_data();
                data.values
                    .iter()
                    .filter(|(name, _)| {
                        matches!(
                            mapping.model().member(&data.class, name),
                            Some((_, Member::Relation(r))) if r.is_composition
                        )
                    })
                    .flat_map(|(_, v)| references_in(v))
                    .collect()
            };
            for t in targets {
                if !seen.insert(t.clone()) {
                    continue;
                }
                if let Some(c) = base.lookup(&t) {
                    out.push(c.clone());
                    stack.push(c);
                }
            }
        }
        out
    }

    /// `get_<attribute>` of a member whose basis is `List`: the
    /// collection (§8.1.6.3.16).
    ///
    /// # Errors
    /// `PreconditionNotMet` for a name that is no `List` attribute or
    /// relation of the class; `AlreadyDeleted` for a destroyed object.
    pub fn get_list(&self, member: &str) -> Result<List, DlrlException> {
        List::new(self, member)
    }

    /// `get_<attribute>` of a member whose basis is `Set` (§8.1.6.3.17).
    ///
    /// # Errors
    /// As [`ObjectRoot::get_list`].
    pub fn get_set(&self, member: &str) -> Result<Set, DlrlException> {
        Set::new(self, member)
    }

    /// `get_<attribute>` of a member whose basis is `StrMap`
    /// (§8.1.6.3.18).
    ///
    /// # Errors
    /// As [`ObjectRoot::get_list`].
    pub fn get_str_map(&self, member: &str) -> Result<StrMap, DlrlException> {
        StrMap::new(self, member)
    }

    /// `get_<attribute>` of a member whose basis is `IntMap`
    /// (§8.1.6.3.19).
    ///
    /// # Errors
    /// As [`ObjectRoot::get_list`].
    pub fn get_int_map(&self, member: &str) -> Result<IntMap, DlrlException> {
        IntMap::new(self, member)
    }

    /// `set_<relation>` of a to-1 relation with the object it designates,
    /// `None` for no object.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a name that is no to-1 relation, a target
    /// of another Cache (§8.1.6.2: "Only DLRL objects belonging to one
    /// Cache can be put in relation") or without oid; the errors of
    /// [`ObjectRoot::set`].
    pub fn set_reference(
        &self,
        relation: &str,
        target: Option<&ObjectRoot>,
    ) -> Result<(), DlrlException> {
        {
            let c = self.lock();
            Self::writable_cell(&c)?;
            match c.mapping.model().member(&c.data.class, relation) {
                Some((_, Member::Relation(r))) if r.multiplicity == Multiplicity::ToOne => {}
                _ => {
                    return Err(DlrlException::precondition(alloc::format!(
                        "{relation} is no to-1 relation of {}",
                        c.data.class
                    )));
                }
            }
        }
        let value = match target {
            Some(t) => self.reference_value(t)?,
            None => Value::Ref(None),
        };
        self.set(relation, AttributeValue::Mono(value))
    }

    /// The Cache an object belongs to: its own, or the owner of its
    /// CacheAccess.
    fn home_cache(&self) -> Option<Cache> {
        match self.owner()? {
            CacheBase::Cache(c) => Some(c),
            CacheBase::Access(a) => a.owner(),
        }
    }

    /// The reference to `target` as an item of a relation of this
    /// object.
    pub(crate) fn reference_value(&self, target: &ObjectRoot) -> Result<Value, DlrlException> {
        let mine = self.home_cache();
        if mine.is_none() || mine != target.home_cache() {
            return Err(DlrlException::precondition(
                "only DLRL objects belonging to one Cache can be put in relation",
            ));
        }
        let r = target.reference();
        if r.oid.is_nil() {
            return Err(DlrlException::precondition(alloc::format!(
                "the {} object has no oid yet (register_object)",
                r.class
            )));
        }
        Ok(Value::Ref(Some(r)))
    }

    pub(crate) fn resolve(&self, target: &ObjectRef) -> Result<ObjectRoot, DlrlException> {
        let base = self
            .owner()
            .ok_or_else(|| DlrlException::not_found("the object's cache is gone"))?;
        let cache = match &base {
            CacheBase::Cache(c) => c.clone(),
            CacheBase::Access(a) => a
                .owner()
                .ok_or_else(|| DlrlException::not_found("the cache of the CacheAccess is gone"))?,
        };
        let subscribed = cache.cache_usage() != CacheUsage::WriteOnly
            && cache.home_by_name(&target.class).is_some();
        if !subscribed && matches!(base, CacheBase::Cache(_)) {
            return Err(DlrlException::not_found(alloc::format!(
                "class {} has no subscription in this cache",
                target.class
            )));
        }
        base.lookup(target).ok_or_else(|| {
            DlrlException::not_found(alloc::format!(
                "object {} of class {} is not there",
                target.oid,
                target.class
            ))
        })
    }
}

/// The references a member value holds, in collection order.
pub(crate) fn references_in(v: &AttributeValue) -> Vec<ObjectRef> {
    let mut out = Vec::new();
    let mut push = |x: &Value| {
        if let Value::Ref(Some(r)) = x {
            out.push(r.clone());
        }
    };
    match v {
        AttributeValue::Mono(x) => push(x),
        AttributeValue::List(xs) | AttributeValue::Set(xs) => xs.iter().for_each(&mut push),
        AttributeValue::StrMap(m) => m.values().for_each(&mut push),
        AttributeValue::IntMap(m) => m.values().for_each(&mut push),
    }
    out
}
