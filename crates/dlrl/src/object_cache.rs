// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Object cache with identity tracking — DDS 1.4 §B.2 + §B.6.
//!
//! Spec §B.2 — `Cache`: central repository for the DLRL objects of a
//! `DomainParticipant` instance. Unique identity via `ObjectId`
//! (topic DCPS key + type discriminator).
//!
//! Spec §B.6 — `ObjectRoot`: lifecycle state (`NEW`/`MODIFIED`/
//! `DELETED`/`COMMITTED`) + version counter for optimistic
//! concurrency.
//!
//! `WeakObjectRef` (Spec §B.6.4) — weak reference to an object;
//! becomes `None` when the object is removed from the cache.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// Unique object identity — topic + DCPS key.
///
/// Spec §B.6.1: every DLRL object instance has a topic name plus
/// a key (CDR-encoded BLOB).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    /// Topic name (`dds_topic`).
    pub topic: String,
    /// CDR-encoded DCPS key.
    pub key: Vec<u8>,
}

impl ObjectId {
    /// Constructor.
    #[must_use]
    pub fn new(topic: String, key: Vec<u8>) -> Self {
        Self { topic, key }
    }
}

/// Lifecycle state of a DLRL object. Spec §B.6.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectState {
    /// `NEW` — object created in the cache but not yet committed.
    New,
    /// `MODIFIED` — committed, then modified locally.
    Modified,
    /// `DELETED` — marked for deletion (cascade pending).
    Deleted,
    /// `COMMITTED` — last commit succeeded.
    Committed,
}

/// Object entry in the cache. Spec §B.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRef {
    /// Identity.
    pub id: ObjectId,
    /// CDR-encoded state bytes (caller serializes via XCDR2).
    pub state: Vec<u8>,
    /// Lifecycle state.
    pub lifecycle: ObjectState,
    /// Version counter — incremented on every modify.
    /// Optimistic-concurrency check (Spec §B.7.4).
    pub version: u64,
}

/// Weak reference to an object in the cache. Spec §B.6.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakObjectRef {
    /// Object identity.
    pub id: ObjectId,
    /// Expected version (caller can check whether the object has
    /// been changed in the meantime).
    pub expected_version: u64,
}

impl WeakObjectRef {
    /// Returns the ID.
    #[must_use]
    pub fn id(&self) -> &ObjectId {
        &self.id
    }

    /// Returns the expected version.
    #[must_use]
    pub fn expected_version(&self) -> u64 {
        self.expected_version
    }
}

/// Object cache — central identity-tracking container. Spec §B.2.
#[derive(Debug, Default)]
pub struct ObjectCache {
    objects: BTreeMap<ObjectId, ObjectRef>,
    seq: AtomicU64,
}

impl ObjectCache {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of held objects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Spec §B.6.3 `register_object` — inserts a new object into the
    /// cache. If the ID already exists, the state is overwritten and
    /// the version is incremented (`MODIFIED`).
    pub fn register(&mut self, id: ObjectId, state: Vec<u8>) -> ObjectRef {
        if let Some(existing) = self.objects.get_mut(&id) {
            existing.state = state;
            existing.version += 1;
            existing.lifecycle = ObjectState::Modified;
            existing.clone()
        } else {
            let v = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
            let entry = ObjectRef {
                id: id.clone(),
                state,
                lifecycle: ObjectState::New,
                version: v,
            };
            self.objects.insert(id, entry.clone());
            entry
        }
    }

    /// Lookup by ID.
    #[must_use]
    pub fn get(&self, id: &ObjectId) -> Option<&ObjectRef> {
        self.objects.get(id)
    }

    /// Returns a weak reference if the object exists.
    /// Spec §B.6.4.
    #[must_use]
    pub fn weak_ref(&self, id: &ObjectId) -> Option<WeakObjectRef> {
        self.objects.get(id).map(|o| WeakObjectRef {
            id: o.id.clone(),
            expected_version: o.version,
        })
    }

    /// Resolves a weak reference. Returns `None` if the object was
    /// removed or the version no longer matches
    /// (Spec §B.7.4 optimistic-concurrency check).
    #[must_use]
    pub fn resolve(&self, weak: &WeakObjectRef) -> Option<&ObjectRef> {
        self.objects
            .get(&weak.id)
            .filter(|o| o.version == weak.expected_version)
    }

    /// Spec §B.6.5 `mark_deleted` — marks an object for deletion.
    /// Actual removal happens on the next `commit_all`.
    pub fn mark_deleted(&mut self, id: &ObjectId) -> bool {
        if let Some(o) = self.objects.get_mut(id) {
            o.lifecycle = ObjectState::Deleted;
            o.version += 1;
            true
        } else {
            false
        }
    }

    /// Spec §B.7.4 `commit_all` — marks all `New`/`Modified` objects
    /// as `Committed` and removes `Deleted` objects.
    pub fn commit_all(&mut self) -> usize {
        let mut deleted = 0;
        let to_remove: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| matches!(o.lifecycle, ObjectState::Deleted))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &to_remove {
            self.objects.remove(id);
            deleted += 1;
        }
        for o in self.objects.values_mut() {
            o.lifecycle = ObjectState::Committed;
        }
        deleted
    }

    /// Spec §B.7.4 `rollback_all` — removes `New` objects, restores
    /// `Modified`/`Deleted` back to `Committed` (but not the state
    /// bytes — the caller must hold a snapshot before `register`).
    pub fn rollback_all(&mut self) -> usize {
        let mut affected = 0;
        let to_remove: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| matches!(o.lifecycle, ObjectState::New))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &to_remove {
            self.objects.remove(id);
            affected += 1;
        }
        for o in self.objects.values_mut() {
            if matches!(o.lifecycle, ObjectState::Modified | ObjectState::Deleted) {
                o.lifecycle = ObjectState::Committed;
                affected += 1;
            }
        }
        affected
    }

    /// List of all object IDs in stable order.
    #[must_use]
    pub fn ids(&self) -> Vec<ObjectId> {
        self.objects.keys().cloned().collect()
    }

    /// Iterates over all objects.
    pub fn iter(&self) -> impl Iterator<Item = &ObjectRef> {
        self.objects.values()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn id(topic: &str, key: &[u8]) -> ObjectId {
        ObjectId::new(topic.into(), key.to_vec())
    }

    #[test]
    fn register_then_get_round_trip() {
        let mut c = ObjectCache::new();
        let r = c.register(id("T", b"k1"), alloc::vec![1, 2, 3]);
        assert_eq!(r.lifecycle, ObjectState::New);
        assert_eq!(c.len(), 1);
        assert_eq!(c.get(&id("T", b"k1")).unwrap().state, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn re_register_increments_version_and_marks_modified() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"k"), alloc::vec![1]);
        let v0 = c.get(&id("T", b"k")).unwrap().version;
        let r = c.register(id("T", b"k"), alloc::vec![2]);
        assert_eq!(r.version, v0 + 1);
        assert_eq!(r.lifecycle, ObjectState::Modified);
    }

    #[test]
    fn weak_ref_resolves_at_same_version() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"k"), alloc::vec![1]);
        let w = c.weak_ref(&id("T", b"k")).unwrap();
        assert!(c.resolve(&w).is_some());
    }

    #[test]
    fn weak_ref_invalidated_on_modify() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"k"), alloc::vec![1]);
        let w = c.weak_ref(&id("T", b"k")).unwrap();
        c.register(id("T", b"k"), alloc::vec![2]);
        assert!(c.resolve(&w).is_none());
    }

    #[test]
    fn mark_deleted_then_commit_removes() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"k"), alloc::vec![1]);
        c.commit_all();
        assert!(c.mark_deleted(&id("T", b"k")));
        let removed = c.commit_all();
        assert_eq!(removed, 1);
        assert!(c.is_empty());
    }

    #[test]
    fn rollback_drops_new_objects() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"a"), alloc::vec![]);
        c.register(id("T", b"b"), alloc::vec![]);
        let n = c.rollback_all();
        assert_eq!(n, 2);
        assert!(c.is_empty());
    }

    #[test]
    fn rollback_after_commit_restores_modified_to_committed() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"k"), alloc::vec![1]);
        c.commit_all();
        c.register(id("T", b"k"), alloc::vec![2]); // Modified
        assert_eq!(
            c.get(&id("T", b"k")).unwrap().lifecycle,
            ObjectState::Modified
        );
        c.rollback_all();
        assert_eq!(
            c.get(&id("T", b"k")).unwrap().lifecycle,
            ObjectState::Committed
        );
    }

    #[test]
    fn mark_deleted_unknown_returns_false() {
        let mut c = ObjectCache::new();
        assert!(!c.mark_deleted(&id("T", b"x")));
    }

    #[test]
    fn ids_returns_stable_order() {
        let mut c = ObjectCache::new();
        c.register(id("T", b"b"), alloc::vec![]);
        c.register(id("T", b"a"), alloc::vec![]);
        let ids = c.ids();
        assert_eq!(ids[0].key, b"a");
        assert_eq!(ids[1].key, b"b");
    }
}
