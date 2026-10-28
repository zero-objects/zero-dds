// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Change observers. Spec §5.3.
//!
//! zerodds-lint: allow no_dyn_in_safe — this file is the plugin
//! boundary `docs/architecture/04_safety_by_architecture.md` §2.1
//! carves out for `dyn`: an observer is by definition a caller-supplied
//! implementation behind a stable trait, and a generic parameter here
//! would monomorphise the registry per observer set and leak every
//! observer type into `Ast`'s own signature.

use crate::change::Changeset;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Receives the changeset of every successful commit.
///
/// Invoked after the commit, never during it, so an observer always sees
/// a model that satisfies the structural invariants. Spec §5.3, §8.1.
pub trait AstObserver: Send + Sync {
    /// React to a committed changeset.
    fn on_commit(&self, changeset: &Changeset);
}

/// Handle for removing a registered observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObserverToken(u64);

/// The set of registered observers.
///
/// The layout cache and the model indices are ordinary members of this
/// set; they hold no privileged path into the model.
#[derive(Clone, Default)]
pub struct ObserverRegistry {
    entries: Vec<(ObserverToken, Arc<dyn AstObserver>)>,
    next_token: u64,
}

impl ObserverRegistry {
    /// Empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_token: 0,
        }
    }

    /// Register an observer, returning the token that removes it again.
    pub fn register(&mut self, observer: Arc<dyn AstObserver>) -> ObserverToken {
        let token = ObserverToken(self.next_token);
        self.next_token = self.next_token.wrapping_add(1);
        self.entries.push((token, observer));
        token
    }

    /// Remove an observer. Returns whether it was registered.
    pub fn unregister(&mut self, token: ObserverToken) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(candidate, _)| *candidate != token);
        self.entries.len() != before
    }

    /// Number of registered observers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no observer is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Notify every observer, in registration order.
    ///
    /// Called only after a successful commit. A commit that changed
    /// nothing still notifies with an empty changeset, so a consumer
    /// tracking sequence numbers sees no gaps.
    pub fn notify(&self, changeset: &Changeset) {
        for (_, observer) in &self.entries {
            observer.on_commit(changeset);
        }
    }
}

impl core::fmt::Debug for ObserverRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ObserverRegistry")
            .field("len", &self.entries.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::change::{Change, NodeKindTag};
    use crate::id::{IdSpace, NodeId};
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct Counter {
        changes: AtomicUsize,
        calls: AtomicUsize,
    }

    impl Counter {
        fn new() -> Self {
            Self {
                changes: AtomicUsize::new(0),
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl AstObserver for Counter {
        fn on_commit(&self, changeset: &Changeset) {
            self.changes.fetch_add(changeset.len(), Ordering::SeqCst);
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn one_change_set() -> Changeset {
        let mut cs = Changeset::new(1);
        cs.push(Change::Created {
            node: NodeId::from_parts(IdSpace::MODEL, 1),
            kind: NodeKindTag(1),
        });
        cs
    }

    #[test]
    fn every_registered_observer_is_notified() {
        let mut registry = ObserverRegistry::new();
        let a = Arc::new(Counter::new());
        let b = Arc::new(Counter::new());
        let _ = registry.register(a.clone());
        let _ = registry.register(b.clone());

        registry.notify(&one_change_set());

        assert_eq!(a.changes.load(Ordering::SeqCst), 1);
        assert_eq!(b.changes.load(Ordering::SeqCst), 1);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn an_unregistered_observer_stops_receiving() {
        let mut registry = ObserverRegistry::new();
        let observer = Arc::new(Counter::new());
        let token = registry.register(observer.clone());

        registry.notify(&one_change_set());
        assert!(registry.unregister(token));
        registry.notify(&one_change_set());

        assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn unregistering_twice_reports_the_second_call_as_a_no_op() {
        let mut registry = ObserverRegistry::new();
        let token = registry.register(Arc::new(Counter::new()));
        assert!(registry.unregister(token));
        assert!(!registry.unregister(token));
    }
}
