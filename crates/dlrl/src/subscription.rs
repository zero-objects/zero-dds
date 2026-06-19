// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Subscription hierarchy — DDS 1.4 §B.3 + §B.6.
//!
//! Spec §B.3: `HomeFactory` holds `HomeListener` instances per topic;
//! `ObjectListener` reacts to object lifecycle events within a home.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::object_cache::{ObjectId, ObjectRef};

/// Object change kind. Spec §B.6.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectChangeKind {
    /// New object created.
    Created,
    /// Object modified.
    Modified,
    /// Object deleted.
    Deleted,
}

/// HomeListener — invoked when an object appears in / disappears from a
/// home (topic). Spec §B.3.4.
pub trait HomeListener: Send + Sync {
    /// An object was created in the home.
    fn on_object_created(&mut self, obj: &ObjectRef);
    /// An object was modified in the home.
    fn on_object_modified(&mut self, obj: &ObjectRef);
    /// An object was deleted.
    fn on_object_deleted(&mut self, id: &ObjectId);
}

/// ObjectListener — per-object listener. Spec §B.6.7.
pub trait ObjectListener: Send + Sync {
    /// The object's own state has changed.
    fn on_state_changed(&mut self, obj: &ObjectRef, kind: ObjectChangeKind);
}

/// HomeFactory — Spec §B.3.1.
#[derive(Default)]
pub struct HomeFactory {
    homes: BTreeMap<String, Vec<Box<dyn HomeListener>>>,
}

impl core::fmt::Debug for HomeFactory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HomeFactory")
            .field("home_count", &self.homes.len())
            .finish_non_exhaustive()
    }
}

impl HomeFactory {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a HomeListener for a topic.
    pub fn register_home_listener(&mut self, topic: String, listener: Box<dyn HomeListener>) {
        self.homes.entry(topic).or_default().push(listener);
    }

    /// Number of listeners for a topic.
    #[must_use]
    pub fn listener_count(&self, topic: &str) -> usize {
        self.homes.get(topic).map(Vec::len).unwrap_or(0)
    }

    /// List of all registered topics.
    #[must_use]
    pub fn topics(&self) -> Vec<String> {
        self.homes.keys().cloned().collect()
    }

    /// Fan-out of an object event to all listeners of the topic.
    pub fn fanout(&mut self, obj: &ObjectRef, kind: ObjectChangeKind) {
        if let Some(listeners) = self.homes.get_mut(&obj.id.topic) {
            for l in listeners {
                match kind {
                    ObjectChangeKind::Created => l.on_object_created(obj),
                    ObjectChangeKind::Modified => l.on_object_modified(obj),
                    ObjectChangeKind::Deleted => l.on_object_deleted(&obj.id),
                }
            }
        }
    }
}

/// SubscriptionRegistry — combines HomeFactory with per-object
/// listeners.
#[derive(Default)]
pub struct SubscriptionRegistry {
    home: HomeFactory,
    object_listeners: BTreeMap<ObjectId, Vec<Box<dyn ObjectListener>>>,
}

impl core::fmt::Debug for SubscriptionRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SubscriptionRegistry")
            .field("home", &self.home)
            .field("object_listener_count", &self.object_listeners.len())
            .finish_non_exhaustive()
    }
}

impl SubscriptionRegistry {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mutable reference to the HomeFactory.
    pub fn home_mut(&mut self) -> &mut HomeFactory {
        &mut self.home
    }

    /// Registers a per-object listener.
    pub fn register_object_listener(&mut self, id: ObjectId, listener: Box<dyn ObjectListener>) {
        self.object_listeners.entry(id).or_default().push(listener);
    }

    /// Notify all relevant listeners (home and object level).
    pub fn notify(&mut self, obj: &ObjectRef, kind: ObjectChangeKind) {
        self.home.fanout(obj, kind);
        if let Some(listeners) = self.object_listeners.get_mut(&obj.id) {
            for l in listeners {
                l.on_state_changed(obj, kind);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::object_cache::ObjectState;
    use alloc::string::ToString;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHome {
        created: alloc::sync::Arc<AtomicUsize>,
        modified: alloc::sync::Arc<AtomicUsize>,
        deleted: alloc::sync::Arc<AtomicUsize>,
    }
    impl HomeListener for CountingHome {
        fn on_object_created(&mut self, _: &ObjectRef) {
            self.created.fetch_add(1, Ordering::Relaxed);
        }
        fn on_object_modified(&mut self, _: &ObjectRef) {
            self.modified.fetch_add(1, Ordering::Relaxed);
        }
        fn on_object_deleted(&mut self, _: &ObjectId) {
            self.deleted.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingObject {
        n: alloc::sync::Arc<AtomicUsize>,
    }
    impl ObjectListener for CountingObject {
        fn on_state_changed(&mut self, _: &ObjectRef, _: ObjectChangeKind) {
            self.n.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn obj(topic: &str, key: &[u8]) -> ObjectRef {
        ObjectRef {
            id: ObjectId::new(topic.into(), key.to_vec()),
            state: alloc::vec![],
            lifecycle: ObjectState::New,
            version: 1,
        }
    }

    #[test]
    fn home_listener_registered_per_topic() {
        let mut h = HomeFactory::new();
        let c = alloc::sync::Arc::new(AtomicUsize::new(0));
        h.register_home_listener(
            "T".into(),
            Box::new(CountingHome {
                created: c.clone(),
                modified: alloc::sync::Arc::new(AtomicUsize::new(0)),
                deleted: alloc::sync::Arc::new(AtomicUsize::new(0)),
            }),
        );
        assert_eq!(h.listener_count("T"), 1);
        assert_eq!(h.topics(), alloc::vec!["T".to_string()]);
    }

    #[test]
    fn fanout_invokes_listeners_for_matching_topic() {
        let mut h = HomeFactory::new();
        let created = alloc::sync::Arc::new(AtomicUsize::new(0));
        let modified = alloc::sync::Arc::new(AtomicUsize::new(0));
        let deleted = alloc::sync::Arc::new(AtomicUsize::new(0));
        h.register_home_listener(
            "T".into(),
            Box::new(CountingHome {
                created: created.clone(),
                modified: modified.clone(),
                deleted: deleted.clone(),
            }),
        );
        h.fanout(&obj("T", b"k"), ObjectChangeKind::Created);
        h.fanout(&obj("T", b"k"), ObjectChangeKind::Modified);
        h.fanout(&obj("T", b"k"), ObjectChangeKind::Deleted);
        assert_eq!(created.load(Ordering::Relaxed), 1);
        assert_eq!(modified.load(Ordering::Relaxed), 1);
        assert_eq!(deleted.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn fanout_ignores_other_topics() {
        let mut h = HomeFactory::new();
        let n = alloc::sync::Arc::new(AtomicUsize::new(0));
        h.register_home_listener(
            "T".into(),
            Box::new(CountingHome {
                created: n.clone(),
                modified: alloc::sync::Arc::new(AtomicUsize::new(0)),
                deleted: alloc::sync::Arc::new(AtomicUsize::new(0)),
            }),
        );
        h.fanout(&obj("OTHER", b"k"), ObjectChangeKind::Created);
        assert_eq!(n.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn registry_notifies_both_home_and_object_listeners() {
        let mut r = SubscriptionRegistry::new();
        let home_n = alloc::sync::Arc::new(AtomicUsize::new(0));
        let obj_n = alloc::sync::Arc::new(AtomicUsize::new(0));
        r.home_mut().register_home_listener(
            "T".into(),
            Box::new(CountingHome {
                created: home_n.clone(),
                modified: alloc::sync::Arc::new(AtomicUsize::new(0)),
                deleted: alloc::sync::Arc::new(AtomicUsize::new(0)),
            }),
        );
        r.register_object_listener(
            ObjectId::new("T".into(), b"k".to_vec()),
            Box::new(CountingObject { n: obj_n.clone() }),
        );
        r.notify(&obj("T", b"k"), ObjectChangeKind::Created);
        assert_eq!(home_n.load(Ordering::Relaxed), 1);
        assert_eq!(obj_n.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn object_listener_only_fires_for_its_own_id() {
        let mut r = SubscriptionRegistry::new();
        let n = alloc::sync::Arc::new(AtomicUsize::new(0));
        r.register_object_listener(
            ObjectId::new("T".into(), b"a".to_vec()),
            Box::new(CountingObject { n: n.clone() }),
        );
        r.notify(&obj("T", b"b"), ObjectChangeKind::Modified);
        assert_eq!(n.load(Ordering::Relaxed), 0);
    }
}
