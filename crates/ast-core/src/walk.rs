// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Cycle-safe traversal. Spec §6.

use crate::id::NodeId;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

/// Records which nodes a walk has already reached.
///
/// Backed by an ordered set rather than a hash set: `no_std` has no
/// default hasher, and deterministic iteration keeps walk output stable
/// across runs, which snapshot determinism depends on. Spec §9.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VisitSet {
    seen: BTreeSet<NodeId>,
}

impl VisitSet {
    /// Empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
        }
    }

    /// Mark a node visited. Returns `true` if this was the first visit.
    pub fn mark(&mut self, node: NodeId) -> bool {
        self.seen.insert(node)
    }

    /// Whether the node has been visited.
    #[must_use]
    pub fn contains(&self, node: NodeId) -> bool {
        self.seen.contains(&node)
    }

    /// Forget every visit.
    pub fn clear(&mut self) {
        self.seen.clear();
    }

    /// Number of visited nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether nothing has been visited.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Visited nodes in identity order.
    pub fn iter(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.seen.iter().copied()
    }
}

/// When a node is emitted relative to its successors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOrder {
    /// Emit a node before its successors.
    PreOrder,
    /// Emit a node after its successors.
    PostOrder,
}

/// Walk a possibly cyclic graph, visiting every reachable node once.
///
/// Iterative rather than recursive: a deeply nested type graph must not
/// depend on stack depth, and this crate forbids panics, which a stack
/// overflow would not respect. Recursive types — a struct containing
/// `sequence<Self>`, mutual recursion through aliases — are legal in the
/// model, so termination is a requirement, not a nicety. Spec §6.
pub fn walk_graph<F>(roots: &[NodeId], successors: F, order: WalkOrder) -> Vec<NodeId>
where
    F: Fn(NodeId) -> Vec<NodeId>,
{
    enum Step {
        Enter(NodeId),
        Emit(NodeId),
    }

    let mut seen = VisitSet::new();
    let mut out = Vec::new();
    let mut stack: Vec<Step> = Vec::new();

    for root in roots.iter().rev() {
        stack.push(Step::Enter(*root));
    }

    while let Some(step) = stack.pop() {
        match step {
            Step::Emit(node) => out.push(node),
            Step::Enter(node) => {
                if !seen.mark(node) {
                    continue;
                }
                match order {
                    WalkOrder::PreOrder => out.push(node),
                    WalkOrder::PostOrder => stack.push(Step::Emit(node)),
                }
                for successor in successors(node).into_iter().rev() {
                    stack.push(Step::Enter(successor));
                }
            }
        }
    }

    out
}

/// Every node reachable from the roots, in identity order.
///
/// A convenience over [`walk_graph`] for callers that need the set
/// rather than an ordering — transitive dependents for cache
/// invalidation, for instance.
pub fn reachable<F>(roots: &[NodeId], successors: F) -> VisitSet
where
    F: Fn(NodeId) -> Vec<NodeId>,
{
    let mut set = VisitSet::new();
    for node in walk_graph(roots, successors, WalkOrder::PreOrder) {
        let _ = set.mark(node);
    }
    set
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::id::IdSpace;
    use alloc::vec;
    use alloc::vec::Vec;

    fn n(counter: u64) -> NodeId {
        NodeId::from_parts(IdSpace::MODEL, counter)
    }

    #[test]
    fn mark_reports_first_visit_only() {
        let mut seen = VisitSet::new();
        assert!(seen.mark(n(1)));
        assert!(!seen.mark(n(1)));
        assert!(seen.contains(n(1)));
        assert_eq!(seen.len(), 1);
        seen.clear();
        assert!(seen.is_empty());
    }

    #[test]
    fn a_self_referential_node_terminates() {
        // 1 -> 1
        let successors = |id: NodeId| {
            if id == n(1) { vec![n(1)] } else { Vec::new() }
        };
        let visited = walk_graph(&[n(1)], successors, WalkOrder::PreOrder);
        assert_eq!(visited, vec![n(1)]);
    }

    #[test]
    fn a_mutual_cycle_terminates_and_visits_each_node_once() {
        // 1 -> 2 -> 3 -> 1
        let successors = |id: NodeId| match id.counter() {
            1 => vec![n(2)],
            2 => vec![n(3)],
            3 => vec![n(1)],
            _ => Vec::new(),
        };
        let visited = walk_graph(&[n(1)], successors, WalkOrder::PreOrder);
        assert_eq!(visited, vec![n(1), n(2), n(3)]);
    }

    #[test]
    fn post_order_emits_children_before_parents() {
        // 1 -> 2, 1 -> 3
        let successors = |id: NodeId| {
            if id == n(1) {
                vec![n(2), n(3)]
            } else {
                Vec::new()
            }
        };
        let visited = walk_graph(&[n(1)], successors, WalkOrder::PostOrder);
        assert_eq!(visited, vec![n(2), n(3), n(1)]);
    }

    #[test]
    fn multiple_roots_are_all_walked_without_duplicates() {
        // 1 -> 3, 2 -> 3
        let successors = |id: NodeId| match id.counter() {
            1 | 2 => vec![n(3)],
            _ => Vec::new(),
        };
        let visited = walk_graph(&[n(1), n(2)], successors, WalkOrder::PreOrder);
        assert_eq!(visited, vec![n(1), n(3), n(2)]);
    }

    #[test]
    fn reachable_collects_the_transitive_closure() {
        // 1 -> 2 -> 3, and 3 -> 1 closing the cycle
        let successors = |id: NodeId| match id.counter() {
            1 => vec![n(2)],
            2 => vec![n(3)],
            3 => vec![n(1)],
            _ => Vec::new(),
        };
        let set = reachable(&[n(1)], successors);
        let all: Vec<NodeId> = set.iter().collect();
        assert_eq!(all, vec![n(1), n(2), n(3)]);
    }

    #[test]
    fn a_deep_chain_does_not_depend_on_stack_depth() {
        // A chain of 100_000 nodes would overflow a recursive walk.
        let successors = |id: NodeId| {
            let next = id.counter() + 1;
            if next <= 100_000 {
                vec![n(next)]
            } else {
                Vec::new()
            }
        };
        let visited = walk_graph(&[n(1)], successors, WalkOrder::PreOrder);
        assert_eq!(visited.len(), 100_000);
    }
}
