// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Changes and changesets. Spec §5.2.

use crate::id::NodeId;
use alloc::string::String;
use alloc::vec::Vec;

/// Discriminator for a node kind.
///
/// Open-coded so that `zerodds-ast` owns its own kind space; this crate
/// only compares and logs them. The numbers are part of the snapshot
/// format — append only, never renumber. Spec §9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeKindTag(pub u16);

/// Discriminator for a field within a node kind.
///
/// Typed, never a string: a misspelled field must be a compile error,
/// not a silently missed branch in an observer's match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldTag(pub u16);

/// State of a reference, for changesets that record a resolution
/// transition. Spec §2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefState {
    /// Points at the given node.
    Resolved(NodeId),
    /// Still awaiting resolution.
    Pending,
    /// Resolution failed.
    Broken,
}

/// A field value as carried in an `Updated` change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldValue {
    /// The field had no value.
    Absent,
    /// Boolean field.
    Bool(bool),
    /// Unsigned integral field.
    U64(u64),
    /// Signed integral field.
    I64(i64),
    /// Textual field.
    Text(String),
    /// Field holding another node's identity.
    Node(NodeId),
    /// Field holding a reference.
    Reference(RefState),
    /// Opaque field, for facet payloads with no simpler shape.
    Bytes(Vec<u8>),
}

/// One committed model change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A node came into existence.
    Created {
        /// The new node.
        node: NodeId,
        /// Its kind.
        kind: NodeKindTag,
    },
    /// A field of an existing node changed.
    Updated {
        /// The node whose field changed.
        node: NodeId,
        /// Which field.
        field: FieldTag,
        /// Value before the change.
        before: FieldValue,
        /// Value after the change.
        after: FieldValue,
    },
    /// A node ceased to exist.
    Deleted {
        /// The removed node.
        node: NodeId,
        /// Its kind.
        kind: NodeKindTag,
    },
}

impl Change {
    /// The node this change concerns.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        match self {
            Self::Created { node, .. }
            | Self::Updated { node, .. }
            | Self::Deleted { node, .. } => *node,
        }
    }

    /// Whether this change created its node.
    #[must_use]
    pub const fn is_created(&self) -> bool {
        matches!(self, Self::Created { .. })
    }

    /// Whether this change removed its node.
    #[must_use]
    pub const fn is_deleted(&self) -> bool {
        matches!(self, Self::Deleted { .. })
    }
}

/// The changes of one commit, in the order they were applied.
///
/// Replaying a changeset onto the model that preceded the commit
/// reproduces the committed model exactly; that property is what lets
/// observers such as the layout cache invalidate precisely instead of
/// wholesale. Spec §5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changeset {
    changes: Vec<Change>,
    sequence: u64,
}

impl Changeset {
    /// Empty changeset carrying the given commit sequence number.
    #[must_use]
    pub const fn new(sequence: u64) -> Self {
        Self {
            changes: Vec::new(),
            sequence,
        }
    }

    /// Monotonic commit sequence number.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Append a change.
    pub fn push(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// Whether the commit changed nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of changes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// The changes in application order.
    pub fn iter(&self) -> impl Iterator<Item = &Change> {
        self.changes.iter()
    }

    /// Every node touched, in change order, once per change.
    pub fn nodes_touched(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.changes.iter().map(Change::node)
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::id::IdSpace;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    fn node(counter: u64) -> NodeId {
        NodeId::from_parts(IdSpace::MODEL, counter)
    }

    #[test]
    fn changeset_preserves_insertion_order() {
        let mut cs = Changeset::new(1);
        cs.push(Change::Created {
            node: node(1),
            kind: NodeKindTag(7),
        });
        cs.push(Change::Updated {
            node: node(1),
            field: FieldTag(2),
            before: FieldValue::Absent,
            after: FieldValue::Text("Pose".to_string()),
        });
        cs.push(Change::Deleted {
            node: node(2),
            kind: NodeKindTag(7),
        });

        let kinds: Vec<&'static str> = cs
            .iter()
            .map(|c| match c {
                Change::Created { .. } => "created",
                Change::Updated { .. } => "updated",
                Change::Deleted { .. } => "deleted",
            })
            .collect();
        assert_eq!(kinds, ["created", "updated", "deleted"]);
        assert_eq!(cs.len(), 3);
        assert_eq!(cs.sequence(), 1);
    }

    #[test]
    fn nodes_touched_reports_every_affected_node_once_per_change() {
        let mut cs = Changeset::new(4);
        cs.push(Change::Created {
            node: node(1),
            kind: NodeKindTag(1),
        });
        cs.push(Change::Updated {
            node: node(1),
            field: FieldTag(0),
            before: FieldValue::Bool(false),
            after: FieldValue::Bool(true),
        });
        cs.push(Change::Deleted {
            node: node(9),
            kind: NodeKindTag(1),
        });

        let touched: Vec<NodeId> = cs.nodes_touched().collect();
        assert_eq!(touched, vec![node(1), node(1), node(9)]);
    }

    #[test]
    fn empty_changeset_is_reported_as_empty() {
        let cs = Changeset::new(0);
        assert!(cs.is_empty());
        assert_eq!(cs.len(), 0);
    }

    #[test]
    fn reference_transitions_are_representable_as_field_values() {
        let before = FieldValue::Reference(RefState::Pending);
        let after = FieldValue::Reference(RefState::Resolved(node(3)));
        assert_ne!(before, after);
    }
}
