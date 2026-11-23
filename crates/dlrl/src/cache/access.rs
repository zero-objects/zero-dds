// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CacheAccess` and `Contract` (DDS 1.2 §8.1.6.3.3, §8.1.6.3.6,
//! §8.1.6.5).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::metamodel::DlrlException;
use crate::model::ObjectRef;
use crate::rows::ObjectData;
use crate::types::DlrlOid;

use super::home::{ObjectHome, RoundLists};
use super::object::{ObjectCell, ObjectRoot, ObjectScope, ObjectState, Owner};
use super::{Cache, CacheInner, CacheKind, CacheUsage};

pub use crate::types::UNLIMITED_RELATED_OBJECTS;

pub(crate) struct ContractState {
    pub(crate) object: ObjectRoot,
    pub(crate) scope: ObjectScope,
    pub(crate) depth: i32,
    /// Set by `delete_contract`, and by `purge` and the deletion of the
    /// access, which detach all contracts.
    pub(crate) deleted: bool,
}

/// `Contract` (§8.1.6.3.6): which objects of the Cache a refresh of the
/// CacheAccess clones: the contracted object, with its compositions
/// ([`ObjectScope::ContainedObjects`]), or also the objects navigable
/// from it up to `depth` relations ([`ObjectScope::RelatedObjects`]).
#[derive(Clone)]
pub struct Contract {
    pub(crate) inner: Arc<Mutex<ContractState>>,
}

impl core::fmt::Debug for Contract {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let c = self.lock();
        f.debug_struct("Contract")
            .field("object", &c.object)
            .field("scope", &c.scope)
            .field("depth", &c.depth)
            .finish()
    }
}

impl PartialEq for Contract {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Contract {
    fn lock(&self) -> MutexGuard<'_, ContractState> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `contracted_object`.
    #[must_use]
    pub fn contracted_object(&self) -> ObjectRoot {
        self.lock().object.clone()
    }

    /// `scope`.
    #[must_use]
    pub fn scope(&self) -> ObjectScope {
        self.lock().scope
    }

    /// `depth`.
    #[must_use]
    pub fn depth(&self) -> i32 {
        self.lock().depth
    }

    /// `set_depth`: taken into account at the next refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted contract (§8.1.6.2, §8.2.1.1).
    pub fn set_depth(&self, depth: i32) -> Result<(), DlrlException> {
        let mut c = self.lock();
        Self::alive(&c)?;
        c.depth = depth;
        Ok(())
    }

    /// `set_scope`: taken into account at the next refresh.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted contract (§8.1.6.2, §8.2.1.1).
    pub fn set_scope(&self, scope: ObjectScope) -> Result<(), DlrlException> {
        let mut c = self.lock();
        Self::alive(&c)?;
        c.scope = scope;
        Ok(())
    }

    /// TRUE once the contract is deleted: by `delete_contract`, or
    /// detached by `purge` or the deletion of its CacheAccess.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.lock().deleted
    }

    fn alive(c: &ContractState) -> Result<(), DlrlException> {
        if c.deleted {
            Err(DlrlException::already_deleted("the Contract is deleted"))
        } else {
            Ok(())
        }
    }

    fn mark_deleted(&self) {
        self.lock().deleted = true;
    }

    pub(crate) fn snapshot(&self) -> (ObjectRoot, ObjectScope, i32) {
        let c = self.lock();
        (c.object.clone(), c.scope, c.depth)
    }
}

pub(crate) struct AccessState {
    pub(crate) objects: BTreeMap<(String, DlrlOid), ObjectRoot>,
    pub(crate) unregistered: Vec<ObjectRoot>,
    pub(crate) contracts: Vec<Contract>,
    pub(crate) last: RoundLists,
}

pub(crate) struct AccessInner {
    pub(crate) cache: Weak<CacheInner>,
    pub(crate) usage: CacheUsage,
    pub(crate) state: Mutex<AccessState>,
    /// Set by `Cache::delete_access` and `CacheFactory::delete_cache`.
    pub(crate) deleted: AtomicBool,
}

/// `CacheAccess` (§8.1.6.3.3): a set of objects manipulated in
/// isolation: in read mode a consistent view of contracted objects that
/// incoming updates do not change until `refresh`; in write mode the
/// objects of one thread of modifications, published by `write`.
#[derive(Clone)]
pub struct CacheAccess {
    pub(crate) inner: Arc<AccessInner>,
}

impl core::fmt::Debug for CacheAccess {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CacheAccess")
            .field("cache_usage", &self.inner.usage)
            .finish_non_exhaustive()
    }
}

impl PartialEq for CacheAccess {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl CacheAccess {
    pub(crate) fn new(cache: &Arc<CacheInner>, usage: CacheUsage) -> Self {
        Self {
            inner: Arc::new(AccessInner {
                cache: Arc::downgrade(cache),
                usage,
                state: Mutex::new(AccessState {
                    objects: BTreeMap::new(),
                    unregistered: Vec::new(),
                    contracts: Vec::new(),
                    last: RoundLists::default(),
                }),
                deleted: AtomicBool::new(false),
            }),
        }
    }

    /// TRUE once the access is deleted, by `Cache::delete_access` or
    /// with its Cache by `CacheFactory::delete_cache`.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.inner.deleted.load(Ordering::Acquire)
    }

    /// `AlreadyDeleted` for a deleted access: every operation raises it
    /// (§8.1.6.2, §8.2.1.1). The attributes keep their last value.
    pub(crate) fn alive(&self) -> Result<(), DlrlException> {
        if self.is_deleted() {
            Err(DlrlException::already_deleted("the CacheAccess is deleted"))
        } else {
            Ok(())
        }
    }

    /// Purges the access and deletes it.
    pub(crate) fn delete(&self) {
        self.detach_all();
        self.inner.deleted.store(true, Ordering::Release);
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, AccessState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `owner`: the Cache that created the access.
    #[must_use]
    pub fn owner(&self) -> Option<Cache> {
        self.inner.cache.upgrade().map(|inner| Cache { inner })
    }

    /// `cache_usage`: the purpose given to `Cache::create_access`.
    #[must_use]
    pub fn cache_usage(&self) -> CacheUsage {
        self.inner.usage
    }

    /// `kind`: [`CacheKind::CacheAccess`].
    #[must_use]
    pub fn kind(&self) -> CacheKind {
        CacheKind::CacheAccess
    }

    /// `objects`: the objects of the access, registered and unregistered.
    #[must_use]
    pub fn objects(&self) -> Vec<ObjectRoot> {
        let st = self.lock();
        st.objects
            .values()
            .chain(st.unregistered.iter())
            .cloned()
            .collect()
    }

    /// `type_names`: the classes of which the access holds at least one
    /// object.
    #[must_use]
    pub fn type_names(&self) -> Vec<String> {
        let names: BTreeSet<String> = self.objects().iter().map(ObjectRoot::class_name).collect();
        names.into_iter().collect()
    }

    /// `contracts`.
    #[must_use]
    pub fn contracts(&self) -> Vec<Contract> {
        self.lock().contracts.clone()
    }

    /// `create_contract`: `object`, an object of the owning Cache, with
    /// the objects in `scope` up to `depth`, is cloned into the access
    /// at each refresh.
    ///
    /// # Errors
    /// `PreconditionNotMet` when `object` is not an object of the Cache
    /// that owns this access; `AlreadyDeleted` for a deleted access.
    pub fn create_contract(
        &self,
        object: &ObjectRoot,
        scope: ObjectScope,
        depth: i32,
    ) -> Result<Contract, DlrlException> {
        self.alive()?;
        let in_owner = match object.owner() {
            Some(super::CacheBase::Cache(c)) => Some(c) == self.owner(),
            _ => false,
        };
        if !in_owner {
            return Err(DlrlException::precondition(
                "the contracted object is not an object of the Cache that owns the CacheAccess",
            ));
        }
        let contract = Contract {
            inner: Arc::new(Mutex::new(ContractState {
                object: object.clone(),
                scope,
                depth,
                deleted: false,
            })),
        };
        self.lock().contracts.push(contract.clone());
        Ok(contract)
    }

    /// `delete_contract`: deletes the contract; the objects it covers
    /// leave the access at the next refresh, unless another contract
    /// covers them.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted access or a deleted contract;
    /// `PreconditionNotMet` when the contract is not attached to this
    /// access.
    pub fn delete_contract(&self, a_contract: &Contract) -> Result<(), DlrlException> {
        self.alive()?;
        Contract::alive(&a_contract.lock())?;
        let mut st = self.lock();
        let before = st.contracts.len();
        st.contracts.retain(|c| c != a_contract);
        if st.contracts.len() == before {
            return Err(DlrlException::precondition(
                "the contract is not attached to this CacheAccess",
            ));
        }
        drop(st);
        a_contract.mark_deleted();
        Ok(())
    }

    /// `purge`: detaches all contracts (they are deleted) and the objects
    /// from the access. The key of a `NoOid` object registered here and
    /// never published is free again (§8.1.4.5).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted access.
    pub fn purge(&self) -> Result<(), DlrlException> {
        self.alive()?;
        self.detach_all();
        Ok(())
    }

    fn detach_all(&self) {
        let cache = self.owner();
        let mut st = self.lock();
        for o in st.objects.values().chain(st.unregistered.iter()) {
            let mut c = o.lock();
            if self.inner.usage.reads() {
                c.read_state = ObjectState::Deleted;
            }
            if self.inner.usage.writes() {
                c.write_state = ObjectState::Deleted;
            }
            c.writable = false;
        }
        if let Some(cache) = &cache {
            for (class, oid) in st.objects.keys() {
                cache.release_key(class, *oid);
            }
        }
        st.objects.clear();
        st.unregistered.clear();
        let contracts = core::mem::take(&mut st.contracts);
        st.last = RoundLists::default();
        drop(st);
        for c in &contracts {
            c.mark_deleted();
        }
    }

    /// `refresh` (§8.1.6.3.2, Figure 8.6): the contracted objects are
    /// cloned from the Cache again. A clone that appears is NEW, one
    /// whose values changed MODIFIED (with its modified attributes), one
    /// that is no longer contracted leaves the access; uncommitted
    /// changes are rolled back, objects created and not written leave
    /// the access, and the key of a `NoOid` object among them is free
    /// again (§8.1.4.5).
    ///
    /// # Errors
    /// `PreconditionNotMet` when the owning Cache is gone;
    /// `AlreadyDeleted` for a deleted access.
    pub fn refresh(&self) -> Result<(), DlrlException> {
        self.alive()?;
        let cache = self
            .owner()
            .ok_or_else(|| DlrlException::precondition("the Cache of the CacheAccess is gone"))?;
        let contracts = self.contracts();
        let closure = cache.closure(&contracts);
        let reads = self.inner.usage.reads();
        let writes = self.inner.usage.writes();
        let mut st = self.lock();
        let mut old = core::mem::take(&mut st.objects);
        let mut last = RoundLists::default();
        for (primary, data) in closure {
            let (home, mapping) = {
                let c = primary.lock();
                (c.home.clone(), Arc::clone(&c.mapping))
            };
            let key = (data.class.clone(), data.oid);
            if let Some(clone) = old.remove(&key) {
                let changed = {
                    let mut c = clone.lock();
                    let changed = changed_members(&c.data, &data);
                    c.previous = changed
                        .iter()
                        .map(|name| (name.clone(), c.data.values.get(name).cloned()))
                        .collect();
                    c.data = data;
                    c.read_state = if !reads {
                        ObjectState::Void
                    } else if changed.is_empty() {
                        ObjectState::NotModified
                    } else {
                        ObjectState::Modified
                    };
                    c.write_state = if writes {
                        ObjectState::NotModified
                    } else {
                        ObjectState::Void
                    };
                    c.modified = changed.iter().cloned().collect();
                    !changed.is_empty()
                };
                if changed {
                    last.modified.push(clone.clone());
                }
                st.objects.insert(key, clone);
            } else {
                let clone = ObjectRoot::new(ObjectCell {
                    data,
                    read_state: if reads {
                        ObjectState::New
                    } else {
                        ObjectState::Void
                    },
                    write_state: if writes {
                        ObjectState::NotModified
                    } else {
                        ObjectState::Void
                    },
                    modified: BTreeSet::new(),
                    previous: BTreeMap::new(),
                    home,
                    owner: Owner::Access(Arc::downgrade(&self.inner)),
                    mapping,
                    registered: true,
                    writable: writes,
                    loaded: true,
                    owner_key: None,
                });
                last.created.push(clone.clone());
                st.objects.insert(key, clone);
            }
        }
        for (_, gone) in old {
            let created_here = gone.write_state() == ObjectState::New;
            {
                let mut c = gone.lock();
                if reads {
                    c.read_state = ObjectState::Deleted;
                }
                c.writable = false;
            }
            cache.release_key(&gone.class_name(), gone.oid());
            if !created_here {
                last.deleted.push(gone);
            }
        }
        st.unregistered.clear();
        st.last = last;
        Ok(())
    }

    /// `write`: publishes the objects created, modified and destroyed in
    /// the access, as one coherent set when the Publisher's PRESENTATION
    /// has `coherent_access`. Afterwards they are NOT_MODIFIED, and the
    /// destroyed ones leave the access.
    ///
    /// # Errors
    /// `ReadOnlyMode` when the access does not allow writing (the IDL of
    /// §8.2.1.2.1 raises it, the list of §8.1.6.2 does not name it);
    /// `PreconditionNotMet` when the Cache is not ENABLED, an object is not registered or lacks a value
    /// its rows need; `DCPSError` when DCPS refuses a sample;
    /// `AlreadyDeleted` for a deleted access.
    pub fn write(&self) -> Result<(), DlrlException> {
        self.alive()?;
        if !self.inner.usage.writes() {
            return Err(DlrlException::ReadOnlyMode {
                message: "the CacheAccess is READ_ONLY".into(),
            });
        }
        let cache = self
            .owner()
            .ok_or_else(|| DlrlException::precondition("the Cache of the CacheAccess is gone"))?;
        let mut st = self.lock();
        if !st.unregistered.is_empty() {
            return Err(DlrlException::precondition(
                "the CacheAccess holds an object of create_unregistered_object that is not registered",
            ));
        }
        let mut changes: Vec<(ObjectData, ObjectState)> = Vec::new();
        for o in st.objects.values() {
            let c = o.lock();
            if matches!(
                c.write_state,
                ObjectState::New | ObjectState::Modified | ObjectState::Deleted
            ) {
                changes.push((c.data.clone(), c.write_state));
            }
        }
        cache.publish(&changes)?;
        st.objects.retain(|_, o| {
            let mut c = o.lock();
            if c.write_state == ObjectState::Deleted {
                c.writable = false;
                false
            } else {
                c.write_state = ObjectState::NotModified;
                true
            }
        });
        Ok(())
    }

    pub(crate) fn insert_new(
        &self,
        home: &ObjectHome,
        oid: DlrlOid,
        registered: bool,
    ) -> ObjectRoot {
        let object = ObjectRoot::new(ObjectCell {
            data: ObjectData::new(home.class_name(), oid),
            read_state: ObjectState::Void,
            write_state: ObjectState::New,
            modified: BTreeSet::new(),
            previous: BTreeMap::new(),
            home: Arc::downgrade(&home.inner),
            owner: Owner::Access(Arc::downgrade(&self.inner)),
            mapping: Arc::clone(home.mapping()),
            registered,
            writable: true,
            loaded: true,
            owner_key: None,
        });
        // A collection starts empty.
        {
            let mut c = object.lock();
            let empties: Vec<(String, crate::model::AttributeValue)> = c
                .mapping
                .members_of_class(home.class_name())
                .into_iter()
                .filter_map(|m| {
                    let basis = match m {
                        crate::mapping::MemberMapping::MultiAttribute(a) => a.basis,
                        crate::mapping::MemberMapping::MultiRelation(r) => r.basis,
                        _ => return None,
                    };
                    Some((m.name().into(), crate::rows::empty_collection(basis)))
                })
                .collect();
            c.data.values.extend(empties);
        }
        let mut st = self.lock();
        if registered {
            st.objects
                .insert((home.class_name().into(), oid), object.clone());
        } else {
            st.unregistered.push(object.clone());
        }
        object
    }

    pub(crate) fn register(&self, object: &ObjectRoot, oid: DlrlOid) {
        let mut st = self.lock();
        st.unregistered.retain(|o| o != object);
        let class = {
            let mut c = object.lock();
            c.data.oid = oid;
            c.registered = true;
            c.data.class.clone()
        };
        st.objects.insert((class, oid), object.clone());
    }

    pub(crate) fn lookup(&self, target: &ObjectRef) -> Option<ObjectRoot> {
        self.lock()
            .objects
            .get(&(target.class.clone(), target.oid))
            .cloned()
    }

    /// The object `target` in this access, cloned from the Cache when it
    /// is only there; for the destruction of a constituent.
    pub(crate) fn adopt_for_destruction(&self, target: &ObjectRef) -> Option<ObjectRoot> {
        if let Some(o) = self.lookup(target) {
            return Some(o);
        }
        let cache = self.owner()?;
        let primary = cache.lookup(target)?;
        let data = primary.current_data();
        let (home, mapping) = {
            let c = primary.lock();
            (c.home.clone(), Arc::clone(&c.mapping))
        };
        let clone = ObjectRoot::new(ObjectCell {
            data,
            read_state: ObjectState::Void,
            write_state: ObjectState::NotModified,
            modified: BTreeSet::new(),
            previous: BTreeMap::new(),
            home,
            owner: Owner::Access(Arc::downgrade(&self.inner)),
            mapping,
            registered: true,
            writable: true,
            loaded: true,
            owner_key: None,
        });
        self.lock()
            .objects
            .insert((target.class.clone(), target.oid), clone.clone());
        Some(clone)
    }

    pub(crate) fn last_refresh(&self) -> RoundLists {
        self.lock().last.clone()
    }
}

/// The members whose values differ between two states of an object.
pub(crate) fn changed_members(old: &ObjectData, new: &ObjectData) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (k, v) in &new.values {
        if old.values.get(k) != Some(v) {
            out.push(k.clone());
        }
    }
    for k in old.values.keys() {
        if !new.values.contains_key(k) {
            out.push(k.clone());
        }
    }
    out
}
