// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectHome` and `ObjectListener` (DDS 1.2 §8.1.6.3.7, §8.1.6.3.8).

use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use std::sync::{Mutex, MutexGuard};

use crate::criterion::QueryExpression;
use crate::mapping::Mapping;
use crate::metamodel::DlrlException;
use crate::model::OBJECT_ROOT_NAMES;
use crate::rows::ObjectData;
use crate::types::DlrlOid;

use super::access::CacheAccess;
use super::object::{ObjectRoot, ObjectState};
use super::selection::{Selection, SelectionCriterion};
use super::{Cache, CacheBase, CacheInner, DcpsState};

/// `ObjectListener` (§8.1.6.3.8): the application is made aware of the
/// incoming changes on the objects of one home. Each operation returns
/// TRUE when the event is fully taken into account and need not reach
/// the listeners of the parent classes' homes (§8.1.6.4.2).
pub trait ObjectListener: Send + Sync {
    /// A new object appeared in the Cache.
    fn on_object_created(&self, the_object: &ObjectRoot) -> bool;
    /// The contents of an object changed.
    fn on_object_modified(&self, the_object: &ObjectRoot) -> bool;
    /// An object was deleted by another participant.
    fn on_object_deleted(&self, the_object: &ObjectRoot) -> bool;
}

/// The objects of the last update round of a home, by kind.
#[derive(Default, Clone)]
pub(crate) struct RoundLists {
    pub(crate) created: Vec<ObjectRoot>,
    pub(crate) modified: Vec<ObjectRoot>,
    pub(crate) deleted: Vec<ObjectRoot>,
}

pub(crate) struct HomeState {
    pub(crate) cache: Option<Weak<CacheInner>>,
    pub(crate) index: Option<u32>,
    pub(crate) listeners: Vec<(Arc<dyn ObjectListener>, bool)>,
    pub(crate) last: RoundLists,
    /// `content_filter` (§8.1.6.3.7), `None` by default.
    pub(crate) content_filter: Option<Arc<QueryExpression>>,
    /// `selections` (§8.1.6.3.7), in creation order.
    pub(crate) selections: Vec<Selection>,
    /// `auto_deref` (§8.1.6.3.7), TRUE by default.
    pub(crate) auto_deref: bool,
}

pub(crate) struct HomeInner {
    pub(crate) class: String,
    pub(crate) mapping: Arc<Mapping>,
    pub(crate) state: Mutex<HomeState>,
}

/// `ObjectHome` (§8.1.6.3.7): the representative of all local instances
/// of one application-defined class. The home of class `Foo` is what the
/// generated `FooHome` specialises; it is created directly
/// ([`ObjectHome::new`]) and registered with one [`Cache`].
#[derive(Clone)]
pub struct ObjectHome {
    pub(crate) inner: Arc<HomeInner>,
}

impl core::fmt::Debug for ObjectHome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ObjectHome")
            .field("class", &self.inner.class)
            .field("registration_index", &self.registration_index())
            .finish_non_exhaustive()
    }
}

impl PartialEq for ObjectHome {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl ObjectHome {
    /// The home of `class` of `mapping`. All homes registered with one
    /// Cache share one mapping.
    ///
    /// # Errors
    /// `BadHomeDefinition` when the mapping has no class `class`.
    pub fn new(mapping: Arc<Mapping>, class: &str) -> Result<Self, DlrlException> {
        if mapping.class(class).is_none() {
            return Err(DlrlException::BadHomeDefinition {
                message: alloc::format!("the mapping has no class {class}"),
            });
        }
        Ok(Self {
            inner: Arc::new(HomeInner {
                class: class.into(),
                mapping,
                state: Mutex::new(HomeState {
                    cache: None,
                    index: None,
                    listeners: Vec::new(),
                    last: RoundLists::default(),
                    content_filter: None,
                    selections: Vec::new(),
                    auto_deref: true,
                }),
            }),
        })
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, HomeState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `name` / `class_name`: the class of the home.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.inner.class
    }

    /// The mapping the home belongs to.
    #[must_use]
    pub fn mapping(&self) -> &Arc<Mapping> {
        &self.inner.mapping
    }

    /// `registration_index`: the index `Cache::register_home` returned;
    /// `None` before.
    #[must_use]
    pub fn registration_index(&self) -> Option<u32> {
        self.lock().index
    }

    pub(crate) fn cache(&self) -> Option<Cache> {
        self.lock()
            .cache
            .as_ref()
            .and_then(Weak::upgrade)
            .map(|inner| Cache { inner })
    }

    /// `parent`: the home of the parent class, registered with the same
    /// Cache.
    #[must_use]
    pub fn parent(&self) -> Option<ObjectHome> {
        let parent = self.inner.mapping.model().parent(&self.inner.class)?;
        if OBJECT_ROOT_NAMES.contains(&parent.name.as_str()) {
            return None;
        }
        self.cache()?.home_by_name(&parent.name)
    }

    /// `children`: the homes of the classes derived directly from this
    /// one, registered with the same Cache.
    #[must_use]
    pub fn children(&self) -> Vec<ObjectHome> {
        let Some(cache) = self.cache() else {
            return Vec::new();
        };
        self.inner
            .mapping
            .model()
            .children(&self.inner.class)
            .into_iter()
            .filter_map(|c| cache.home_by_name(&c.name))
            .collect()
    }

    fn require_registered(&self) -> Result<Cache, DlrlException> {
        let cache = self.cache().ok_or_else(|| {
            DlrlException::precondition("the home is not registered with a Cache")
        })?;
        if cache.pubsub_state() == DcpsState::Initial {
            return Err(DlrlException::precondition(
                "the DCPS state of the Cache is INITIAL",
            ));
        }
        Ok(cache)
    }

    /// `get_topic_name`: the topic that holds the value of an attribute.
    ///
    /// # Errors
    /// `PreconditionNotMet` while the DCPS state of the Cache is INITIAL,
    /// or for a name that is no shared member of the class.
    pub fn get_topic_name(&self, attribute_name: &str) -> Result<String, DlrlException> {
        self.require_registered()?;
        self.inner
            .mapping
            .member(&self.inner.class, attribute_name)
            .map(|(_, m)| m.topic().name.clone())
            .ok_or_else(|| {
                DlrlException::precondition(alloc::format!(
                    "class {} has no shared member {attribute_name}",
                    self.inner.class
                ))
            })
    }

    /// `get_all_topic_names`: the topics that hold the values of all
    /// attributes of the class, the main topic first.
    ///
    /// # Errors
    /// `PreconditionNotMet` while the DCPS state of the Cache is INITIAL.
    pub fn get_all_topic_names(&self) -> Result<Vec<String>, DlrlException> {
        self.require_registered()?;
        Ok(self.inner.mapping.topics_of_class(&self.inner.class))
    }

    /// `content_filter`: the expression set by
    /// [`ObjectHome::set_content_filter`], `None` (NULL) by default.
    #[must_use]
    pub fn content_filter(&self) -> Option<String> {
        self.lock()
            .content_filter
            .as_ref()
            .map(|q| q.expression().into())
    }

    /// `set_content_filter`: the filter of the incoming objects of this
    /// home, in the syntax of Annex B. Only the incoming objects that
    /// pass it are created in the Cache (§8.1.6.3.7); an incoming update
    /// of an object of the Cache that does not pass it is not applied,
    /// as a content-filtered DataReader does not deliver it. The filter
    /// is applied in the update round of the Cache, to the object built
    /// from all its rows; it is not a ContentFilteredTopic of DCPS,
    /// because one object's attributes may lie in several topics.
    ///
    /// A FIELDNAME of the filter does not go across a relation. The
    /// filter "is intended to be mapped on the underlying DCPS
    /// infrastructure to provide content-based subscription"
    /// (§8.1.6.3.7): it decides on the data of the incoming object, as a
    /// DCPS content filter decides on the incoming sample. A field of
    /// another object would make the decision depend on whether and when
    /// that object arrives, which the filter of an incoming object does
    /// not see again.
    ///
    /// # Errors
    /// `PreconditionNotMet` once the home is registered with a Cache;
    /// `SQLError` for an expression that is not valid for the class
    /// (bad syntax, a field the class does not have, a constant that
    /// does not fit its field, or a `%n`, since a content filter has no
    /// parameters) and for a FIELDNAME that goes across a relation.
    pub fn set_content_filter(&self, expression: &str) -> Result<(), DlrlException> {
        let mut st = self.lock();
        if st.cache.is_some() {
            return Err(DlrlException::precondition(
                "the content filter can be set only before the home is registered",
            ));
        }
        let q = QueryExpression::new(
            self.inner.mapping.model(),
            &self.inner.class,
            expression,
            &[],
        )?;
        let across = q.fields_across_relations();
        if !across.is_empty() {
            return Err(DlrlException::sql(alloc::format!(
                "{expression}: a content filter decides on the data of the incoming object; {} goes across a relation",
                across.join(", ")
            )));
        }
        st.content_filter = Some(Arc::new(q));
        Ok(())
    }

    /// `auto_deref` (§8.1.6.3.7): TRUE (the default) when the state of an
    /// object of this home is always loaded into the object; FALSE when
    /// it is loaded only once the application accesses it.
    #[must_use]
    pub fn auto_deref(&self) -> bool {
        self.lock().auto_deref
    }

    /// `set_auto_deref` (§8.1.6.3.7). With FALSE, an object of this home
    /// that arrives in the Cache holds only its oid; its state stays in
    /// the rows of the Cache's DCPS tables and is loaded into the object
    /// when the application accesses it (`ObjectRoot::get`, `data`,
    /// `get_reference`, `get_references`, a collection's contents,
    /// `Cache::deref_reference`). Read states, the modified attributes
    /// (`is_<attribute>_modified`), the listeners, the Selections and the
    /// clones of a CacheAccess work as with TRUE; a CacheAccess's clones
    /// always hold their state. An object loaded once stays loaded, with
    /// the state of each round, until `underef_all`. With TRUE, every
    /// object of this home in the Cache is loaded at once.
    ///
    /// The homes of derived classes have their own `auto_deref`: this one
    /// concerns the objects whose class is the home's class.
    pub fn set_auto_deref(&self, value: bool) {
        self.lock().auto_deref = value;
        if value {
            self.deref_all();
        }
    }

    /// `deref_all` (§8.1.6.3.7): loads the most recent state into every
    /// object of this home in the Cache.
    pub fn deref_all(&self) {
        let Some(cache) = self.cache() else {
            return;
        };
        for o in cache.objects_of_home(self) {
            o.ensure_loaded();
        }
    }

    /// `underef_all` (§8.1.6.3.7): unloads the state of every object of
    /// this home in the Cache; it is loaded again at the next access. An
    /// object in the middle of an update round (NEW, MODIFIED, DELETED)
    /// keeps the modification lists of the round; a DELETED object keeps
    /// its last state, since its rows are gone.
    pub fn underef_all(&self) {
        let Some(cache) = self.cache() else {
            return;
        };
        // Not in the middle of another thread's update round, which
        // reads whether an object is loaded before and after it changes
        // the tables.
        let _round = cache.enter_round();
        for o in cache.objects_of_home(self) {
            let mut c = o.lock();
            if c.read_state == ObjectState::Deleted || c.owner_key.is_none() {
                continue;
            }
            let (class, oid) = (c.data.class.clone(), c.data.oid);
            c.data = ObjectData::new(&class, oid);
            c.loaded = false;
        }
    }

    /// `attach_listener`: `concerns_contained_objects` TRUE makes the
    /// listener also hear of the modification of the objects an object
    /// of this home contains (compositions), as a modification of that
    /// object.
    pub fn attach_listener(
        &self,
        listener: Arc<dyn ObjectListener>,
        concerns_contained_objects: bool,
    ) {
        let mut st = self.lock();
        if !st.listeners.iter().any(|(l, _)| Arc::ptr_eq(l, &listener)) {
            st.listeners.push((listener, concerns_contained_objects));
        }
    }

    /// `detach_listener`.
    pub fn detach_listener(&self, listener: &Arc<dyn ObjectListener>) {
        self.lock()
            .listeners
            .retain(|(l, _)| !Arc::ptr_eq(l, listener));
    }

    /// `listeners`.
    #[must_use]
    pub fn listeners(&self) -> Vec<Arc<dyn ObjectListener>> {
        self.lock()
            .listeners
            .iter()
            .map(|(l, _)| Arc::clone(l))
            .collect()
    }

    /// `selections`: the Selections created by this home and not
    /// deleted, in creation order.
    #[must_use]
    pub fn selections(&self) -> Vec<Selection> {
        self.lock().selections.clone()
    }

    /// `create_selection` (§8.1.6.3.7): a Selection of the objects of
    /// this class and its derived classes that pass `criterion`, which
    /// then belongs to the Selection. `auto_refresh` TRUE: the Selection
    /// is examined at each update round; FALSE: at each
    /// `Selection::refresh`. `concerns_contained_objects` TRUE: a member
    /// is also modified when an object it contains is. The objects of
    /// the Cache are examined once here, before a listener can be set.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the home is not registered or the DCPS
    /// state of the Cache is INITIAL; `BadParameter` (the exception of
    /// `FooHome::create_selection` in §8.2.1.2.2) for a `QueryCriterion`
    /// of a class this home's class does not derive from, one that
    /// belongs to another Selection, or one deleted by `delete_selection`.
    pub fn create_selection(
        &self,
        criterion: SelectionCriterion,
        auto_refresh: bool,
        concerns_contained_objects: bool,
    ) -> Result<Selection, DlrlException> {
        self.require_registered()?;
        if let SelectionCriterion::Query(q) = &criterion
            && !self
                .inner
                .mapping
                .model()
                .is_kind_of(&self.inner.class, q.class_name())
        {
            return Err(DlrlException::BadParameter {
                message: alloc::format!(
                    "the QueryCriterion is for class {}, the home of {}",
                    q.class_name(),
                    self.inner.class
                ),
            });
        }
        let selection = Selection::new(self, criterion, auto_refresh, concerns_contained_objects);
        selection.attach_query()?;
        self.lock().selections.push(selection.clone());
        if let Err(e) = selection.refresh() {
            self.lock().selections.retain(|s| s != &selection);
            selection.mark_deleted(false);
            return Err(e);
        }
        Ok(selection)
    }

    /// `delete_selection`: deletes the Selection and its criterion
    /// (§8.1.6.3.7); the Selection has no members and no listener any
    /// more, and its operations raise `AlreadyDeleted` (§8.1.6.2,
    /// §8.2.1.1). A `QueryCriterion` of it is deleted: it belongs to no
    /// Selection, `set_query` and `set_parameters` raise `AlreadyDeleted`,
    /// and `create_selection` refuses it. A `FilterCriterion` is an
    /// object of the application: the Selection releases its reference
    /// to it.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a Selection deleted already;
    /// `PreconditionNotMet` when this home did not create it.
    pub fn delete_selection(&self, a_selection: &Selection) -> Result<(), DlrlException> {
        a_selection.alive()?;
        {
            let mut st = self.lock();
            let before = st.selections.len();
            st.selections.retain(|s| s != a_selection);
            if st.selections.len() == before {
                return Err(DlrlException::precondition(
                    "the Selection was not created by this ObjectHome",
                ));
            }
        }
        a_selection.mark_deleted(true);
        Ok(())
    }

    fn writable_access(&self, access: &CacheAccess) -> Result<Cache, DlrlException> {
        access.alive()?;
        let cache = self.cache().ok_or_else(|| {
            DlrlException::precondition("the home is not registered with a Cache")
        })?;
        if cache.pubsub_state() != DcpsState::Enabled {
            return Err(DlrlException::precondition(
                "the DCPS state of the Cache is not ENABLED",
            ));
        }
        if !access.cache_usage().writes() {
            return Err(DlrlException::precondition(
                "the CacheAccess does not allow writing",
            ));
        }
        if access.owner().as_ref() != Some(&cache) {
            return Err(DlrlException::precondition(
                "the CacheAccess belongs to another Cache",
            ));
        }
        Ok(cache)
    }

    /// `create_object`: a new object in `access`, with a fresh oid; it is
    /// published by `access.write()`.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted access; `PreconditionNotMet` when
    /// the Cache is not ENABLED, the access does not allow writing or
    /// belongs to another Cache, or the class has a `NoOid` key: its identity is its key fields, so it is created by
    /// `create_unregistered_object` and `register_object`.
    pub fn create_object(&self, access: &CacheAccess) -> Result<ObjectRoot, DlrlException> {
        let cache = self.writable_access(access)?;
        if !self.identity().is_empty() {
            return Err(DlrlException::precondition(alloc::format!(
                "class {} has a NoOid key: create_unregistered_object, set the key fields, register_object",
                self.inner.class
            )));
        }
        let oid = cache.allocate_oid();
        Ok(access.insert_new(self, oid, true))
    }

    /// `create_unregistered_object`: a new object in `access` without oid
    /// ([`DlrlOid::NIL`]), whose content is filled before
    /// `register_object` derives its identity.
    ///
    /// # Errors
    /// As [`ObjectHome::create_object`], without the `NoOid` restriction.
    pub fn create_unregistered_object(
        &self,
        access: &CacheAccess,
    ) -> Result<ObjectRoot, DlrlException> {
        self.writable_access(access)?;
        Ok(access.insert_new(self, DlrlOid::NIL, false))
    }

    /// `register_object`: gives an object of `create_unregistered_object`
    /// its oid: for a `NoOid` class the oid associated with its key
    /// values (§8.1.4.5), for another class a fresh one. Afterwards the
    /// key fields of a `NoOid` object cannot change.
    ///
    /// # Errors
    /// `PreconditionNotMet` for an object not of `create_unregistered_object`
    /// or of another class, or a `NoOid` object without its key values;
    /// `AlreadyExisting` when the key values are those of an existing
    /// object.
    pub fn register_object(&self, unregistered_object: &ObjectRoot) -> Result<(), DlrlException> {
        let (data, registered) = {
            let c = unregistered_object.lock();
            (c.data.clone(), c.registered)
        };
        if registered {
            return Err(DlrlException::precondition(
                "only an object of create_unregistered_object can be registered",
            ));
        }
        if data.class != self.inner.class {
            return Err(DlrlException::precondition(alloc::format!(
                "the object is of class {}, the home of {}",
                data.class,
                self.inner.class
            )));
        }
        let Some(CacheBase::Access(access)) = unregistered_object.owner() else {
            return Err(DlrlException::precondition(
                "the object is in no CacheAccess",
            ));
        };
        let cache = self.writable_access(&access)?;
        let oid = if self.identity().is_empty() {
            cache.allocate_oid()
        } else {
            let key = self
                .inner
                .mapping
                .identity_key(&data)
                .map_err(|e| DlrlException::precondition(alloc::format!("{e}")))?;
            cache.associate_new_key(&self.inner.class, &key)?
        };
        access.register(unregistered_object, oid);
        Ok(())
    }

    fn identity(&self) -> Vec<String> {
        self.inner
            .mapping
            .class(&self.inner.class)
            .map(|cm| cm.identity.clone())
            .unwrap_or_default()
    }

    fn matches(&self, o: &ObjectRoot) -> bool {
        let class = o.class_name();
        self.inner
            .mapping
            .model()
            .is_kind_of(&class, &self.inner.class)
    }

    /// `find_object`: the object of this class (or a derived one) with
    /// `oid` in `source`.
    ///
    /// # Errors
    /// `NotFound` when `source` holds no such object; `AlreadyDeleted`
    /// for a deleted Cache or CacheAccess.
    pub fn find_object(
        &self,
        oid: DlrlOid,
        source: &CacheBase,
    ) -> Result<ObjectRoot, DlrlException> {
        source.alive()?;
        source
            .objects()
            .into_iter()
            .find(|o| o.oid() == oid && self.matches(o))
            .ok_or_else(|| {
                DlrlException::not_found(alloc::format!(
                    "no object {oid} of class {} there",
                    self.inner.class
                ))
            })
    }

    /// `get_objects`: the objects of this class and its derived classes
    /// in `source`.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache or CacheAccess.
    pub fn get_objects(&self, source: &CacheBase) -> Result<Vec<ObjectRoot>, DlrlException> {
        source.alive()?;
        Ok(source
            .objects()
            .into_iter()
            .filter(|o| self.matches(o))
            .collect())
    }

    fn last_of(
        &self,
        source: &CacheBase,
        pick: fn(&RoundLists) -> &Vec<ObjectRoot>,
    ) -> Result<Vec<ObjectRoot>, DlrlException> {
        source.alive()?;
        let lists = match source {
            CacheBase::Cache(_) => self.lock().last.clone(),
            CacheBase::Access(a) => a.last_refresh(),
        };
        Ok(pick(&lists)
            .iter()
            .filter(|o| self.matches(o))
            .cloned()
            .collect())
    }

    /// `get_created_objects`: the objects of this class created by the
    /// last update round (Cache) or refresh (CacheAccess).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache or CacheAccess.
    pub fn get_created_objects(
        &self,
        source: &CacheBase,
    ) -> Result<Vec<ObjectRoot>, DlrlException> {
        self.last_of(source, |l| &l.created)
    }

    /// `get_modified_objects`: as [`ObjectHome::get_created_objects`],
    /// for modified objects.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache or CacheAccess.
    pub fn get_modified_objects(
        &self,
        source: &CacheBase,
    ) -> Result<Vec<ObjectRoot>, DlrlException> {
        self.last_of(source, |l| &l.modified)
    }

    /// `get_deleted_objects`: as [`ObjectHome::get_created_objects`], for
    /// deleted objects.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache or CacheAccess.
    pub fn get_deleted_objects(
        &self,
        source: &CacheBase,
    ) -> Result<Vec<ObjectRoot>, DlrlException> {
        self.last_of(source, |l| &l.deleted)
    }
}
