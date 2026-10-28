// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The layout cache. Spec §7.
//!
//! Layout is computed from the model, never stored on its nodes, and
//! cached in a side table keyed by `(type, layout key)`. Each entry
//! records which types its computation consulted, so a change evicts
//! that type and its dependents and nothing else.

use crate::key::LayoutKey;
use crate::plan::{LayoutError, TypeLayout};
use crate::xcdr::{DepCollector, plan};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Change, Changeset, Id, NodeId};

/// One cached plan together with what it depended on.
#[derive(Debug, Clone)]
struct Entry {
    layout: TypeLayout,
    deps: Vec<NodeId>,
}

/// Cached layout plans.
///
/// Offsets are invariant under endianness, so an entry serves both byte
/// orders; it is keyed by everything else that moves one, which is
/// [`LayoutKey::offset_key`]. Spec §7.
#[derive(Debug, Clone, Default)]
pub struct LayoutCache {
    entries: BTreeMap<(NodeId, crate::OffsetKey), Entry>,
    hits: u64,
    misses: u64,
}

impl LayoutCache {
    /// Empty cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// The plan for a type, computing and caching it on first use.
    ///
    /// # Errors
    /// Whatever [`plan`] returns — an unresolved reference, an
    /// unsupported construct, a little-endian key request.
    pub fn layout(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        key: LayoutKey,
    ) -> Result<&TypeLayout, LayoutError> {
        let node_id = ast.type_(type_)?.node_id;
        let slot = (node_id, key.offset_key());

        if let alloc::collections::btree_map::Entry::Vacant(vacant) = self.entries.entry(slot) {
            let mut deps = DepCollector::new();
            let layout = plan(ast, type_, key, &mut deps)?;
            let _ = vacant.insert(Entry {
                layout,
                deps: deps.into_deps(),
            });
            self.misses = self.misses.saturating_add(1);
        } else {
            self.hits = self.hits.saturating_add(1);
        }

        self.entries
            .get(&slot)
            .map(|entry| &entry.layout)
            .ok_or(LayoutError::Unresolved { node: node_id })
    }

    /// Evict everything whose plan consulted any node the changeset
    /// touched.
    ///
    /// Precise rather than wholesale: a change to one type leaves plans
    /// for unrelated types in place, and evicts the ones that contain it
    /// however deeply. Spec §7.
    pub fn invalidate(&mut self, changeset: &Changeset) {
        let touched: Vec<NodeId> = changeset.nodes_touched().collect();
        if touched.is_empty() {
            return;
        }
        self.entries.retain(|(node, _), entry| {
            !touched.contains(node) && !entry.deps.iter().any(|dep| touched.contains(dep))
        });
    }

    /// Evict every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached plans.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many lookups were served from the cache.
    #[must_use]
    pub const fn hits(&self) -> u64 {
        self.hits
    }

    /// How many lookups had to compute a plan.
    #[must_use]
    pub const fn misses(&self) -> u64 {
        self.misses
    }

    /// Whether a plan for this type and key is cached.
    #[must_use]
    pub fn contains(&self, type_: NodeId, key: LayoutKey) -> bool {
        self.entries.contains_key(&(type_, key.offset_key()))
    }
}

/// The cache behind a lock, so it can register as a change observer.
///
/// Registers like any other consumer; it holds no privileged path into
/// the model. Spec §5.3, §7.
#[cfg(feature = "std")]
pub struct SharedLayoutCache {
    inner: std::sync::Mutex<LayoutCache>,
}

#[cfg(feature = "std")]
impl SharedLayoutCache {
    /// An empty shared cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(LayoutCache::new()),
        }
    }

    /// Run a closure against the cache.
    ///
    /// Returns `None` if the lock was poisoned by a panic elsewhere; the
    /// caller decides whether that is fatal rather than this type
    /// deciding for it.
    pub fn with<R>(&self, f: impl FnOnce(&mut LayoutCache) -> R) -> Option<R> {
        self.inner.lock().ok().map(|mut guard| f(&mut guard))
    }

    /// Number of cached plans, or zero if the lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.with(|cache| cache.len()).unwrap_or(0)
    }

    /// Whether nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(feature = "std")]
impl Default for SharedLayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl core::fmt::Debug for SharedLayoutCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SharedLayoutCache")
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(feature = "std")]
impl zerodds_ast_core::AstObserver for SharedLayoutCache {
    fn on_commit(&self, changeset: &Changeset) {
        let _ = self.with(|cache| cache.invalidate(changeset));
    }
}

/// Whether a changeset can affect any layout at all.
///
/// A commit that only created nodes nothing points at cannot change an
/// existing plan; this lets a consumer skip the retain sweep.
#[must_use]
pub fn touches_layout(changeset: &Changeset) -> bool {
    changeset.iter().any(|change| {
        matches!(
            change,
            Change::Updated { .. } | Change::Deleted { .. } | Change::Created { .. }
        )
    })
}
