// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Changeset-driven indices. Spec §6.

use crate::id::NodeId;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Why a unique lookup did not yield exactly one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexLookup {
    /// No node carries the key.
    Missing,
    /// Several nodes carry the key.
    Ambiguous(Vec<NodeId>),
}

/// Multi-valued index from a key to the nodes carrying it.
///
/// Ordered map, not a hash map: `no_std` has no default hasher, and
/// deterministic key iteration keeps diagnostics and snapshots stable
/// across runs.
///
/// Candidate lists keep insertion order so that an ambiguity report
/// names the earlier declaration first. Spec §6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index<K: Ord + Clone> {
    forward: BTreeMap<K, Vec<NodeId>>,
    reverse: BTreeMap<NodeId, Vec<K>>,
}

impl<K: Ord + Clone> Default for Index<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone> Index<K> {
    /// Empty index.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            forward: BTreeMap::new(),
            reverse: BTreeMap::new(),
        }
    }

    /// Associate a node with a key. A repeated (key, node) pair is not
    /// duplicated.
    pub fn insert(&mut self, key: K, node: NodeId) {
        let nodes = self.forward.entry(key.clone()).or_default();
        if !nodes.contains(&node) {
            nodes.push(node);
        }
        let keys = self.reverse.entry(node).or_default();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    /// Drop one (key, node) association. Returns whether it existed.
    pub fn remove(&mut self, key: &K, node: NodeId) -> bool {
        let Some(nodes) = self.forward.get_mut(key) else {
            return false;
        };
        let before = nodes.len();
        nodes.retain(|candidate| *candidate != node);
        let removed = nodes.len() != before;
        if nodes.is_empty() {
            let _ = self.forward.remove(key);
        }
        if let Some(keys) = self.reverse.get_mut(&node) {
            keys.retain(|candidate| candidate != key);
            if keys.is_empty() {
                let _ = self.reverse.remove(&node);
            }
        }
        removed
    }

    /// Drop every association a node has.
    ///
    /// Called when a `Deleted` change arrives. The reverse map makes
    /// this cost the keys of that node rather than a full scan.
    pub fn remove_node(&mut self, node: NodeId) {
        let Some(keys) = self.reverse.remove(&node) else {
            return;
        };
        for key in keys {
            if let Some(nodes) = self.forward.get_mut(&key) {
                nodes.retain(|candidate| *candidate != node);
                if nodes.is_empty() {
                    let _ = self.forward.remove(&key);
                }
            }
        }
    }

    /// Drop a key and every association it had.
    ///
    /// Used when a pending reference is settled: every waiter on that
    /// key has been dealt with at once, so the key goes away as a unit.
    pub fn remove_key(&mut self, key: &K) {
        let Some(nodes) = self.forward.remove(key) else {
            return;
        };
        for node in nodes {
            if let Some(keys) = self.reverse.get_mut(&node) {
                keys.retain(|candidate| candidate != key);
                if keys.is_empty() {
                    let _ = self.reverse.remove(&node);
                }
            }
        }
    }

    /// Every node carrying the key, in insertion order.
    #[must_use]
    pub fn get(&self, key: &K) -> &[NodeId] {
        self.forward.get(key).map_or(&[], Vec::as_slice)
    }

    /// The single node carrying the key.
    ///
    /// # Errors
    /// [`IndexLookup::Missing`] when nothing carries it,
    /// [`IndexLookup::Ambiguous`] when several do. An ambiguity is
    /// reported with all candidates rather than silently resolved to the
    /// first — a wrong-but-plausible resolution is worse than a loud
    /// failure.
    pub fn unique(&self, key: &K) -> Result<NodeId, IndexLookup> {
        match self.get(key) {
            [] => Err(IndexLookup::Missing),
            [single] => Ok(*single),
            many => Err(IndexLookup::Ambiguous(many.to_vec())),
        }
    }

    /// The keys a node is registered under, in insertion order.
    #[must_use]
    pub fn keys_of(&self, node: NodeId) -> &[K] {
        self.reverse.get(&node).map_or(&[], Vec::as_slice)
    }

    /// Keys in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.forward.keys()
    }

    /// Every (key, nodes) pair in sorted key order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &[NodeId])> {
        self.forward.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// Number of distinct keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    /// Drop every entry.
    pub fn clear(&mut self) {
        self.forward.clear();
        self.reverse.clear();
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::id::IdSpace;
    use alloc::string::{String, ToString};
    use alloc::vec;

    fn n(counter: u64) -> NodeId {
        NodeId::from_parts(IdSpace::MODEL, counter)
    }

    #[test]
    fn a_unique_key_resolves_to_its_node() {
        let mut index: Index<String> = Index::new();
        index.insert("::Robot::Pose".to_string(), n(1));
        assert_eq!(index.unique(&"::Robot::Pose".to_string()), Ok(n(1)));
    }

    #[test]
    fn a_missing_key_reports_missing_not_an_empty_success() {
        let index: Index<String> = Index::new();
        assert_eq!(
            index.unique(&"::Nope".to_string()),
            Err(IndexLookup::Missing)
        );
    }

    #[test]
    fn a_duplicated_key_reports_every_candidate() {
        let mut index: Index<String> = Index::new();
        index.insert("::Dup".to_string(), n(1));
        index.insert("::Dup".to_string(), n(2));
        assert_eq!(
            index.unique(&"::Dup".to_string()),
            Err(IndexLookup::Ambiguous(vec![n(1), n(2)]))
        );
    }

    #[test]
    fn removing_the_last_node_for_a_key_drops_the_key() {
        let mut index: Index<String> = Index::new();
        index.insert("::Gone".to_string(), n(1));
        assert!(index.remove(&"::Gone".to_string(), n(1)));
        assert_eq!(index.len(), 0);
        assert!(index.get(&"::Gone".to_string()).is_empty());
        assert!(!index.remove(&"::Gone".to_string(), n(1)));
    }

    #[test]
    fn remove_node_purges_every_key_the_node_appeared_under() {
        let mut index: Index<String> = Index::new();
        index.insert("a".to_string(), n(1));
        index.insert("b".to_string(), n(1));
        index.insert("b".to_string(), n(2));
        index.remove_node(n(1));
        assert!(index.get(&"a".to_string()).is_empty());
        assert_eq!(index.get(&"b".to_string()), &[n(2)]);
        assert!(index.keys_of(n(1)).is_empty());
    }

    #[test]
    fn candidates_stay_in_insertion_order_for_deterministic_diagnostics() {
        let mut index: Index<String> = Index::new();
        index.insert("k".to_string(), n(3));
        index.insert("k".to_string(), n(1));
        index.insert("k".to_string(), n(2));
        assert_eq!(index.get(&"k".to_string()), &[n(3), n(1), n(2)]);
    }

    #[test]
    fn inserting_the_same_pair_twice_does_not_duplicate_it() {
        let mut index: Index<String> = Index::new();
        index.insert("k".to_string(), n(1));
        index.insert("k".to_string(), n(1));
        assert_eq!(index.get(&"k".to_string()), &[n(1)]);
        assert_eq!(index.keys_of(n(1)).len(), 1);
    }
}
