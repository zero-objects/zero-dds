// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Undo-logged transactions. Spec §5.1.

use crate::change::{Change, Changeset, FieldTag, FieldValue};
use crate::id::NodeId;
use alloc::vec::Vec;

/// What has to happen to reverse one applied mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoPayload {
    /// Reinsert a removed node from the implementor's stash.
    ///
    /// The body is not carried here: serialising every removed node to
    /// bytes just to roll a transaction back would be work done on the
    /// success path for the sake of the failure path. The implementor
    /// keeps the node itself and this record names its slot in that
    /// stash.
    Recreate {
        /// Index into the implementor's stash of removed nodes.
        stash: u32,
    },
    /// Put a field back to its previous value.
    RestoreField {
        /// Which field.
        field: FieldTag,
        /// Value to restore.
        value: FieldValue,
    },
    /// Put a reference back to pending under the key it waited on.
    ///
    /// `RestoreField` cannot carry this: a pending reference is only
    /// identified by its key, and [`FieldValue::Reference`] records the
    /// state without it. Reversing a resolution without the key would
    /// leave the reference pending on nothing.
    RestorePending {
        /// The key the reference was waiting on before it resolved.
        key: crate::reference::PendingRef,
        /// Which reference of the node: its position in the node's
        /// reference list. A member has exactly one, a map has two.
        slot: usize,
    },
    /// Remove a node that this transaction created.
    Remove,
}

/// One reversal step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoRecord {
    /// Node the reversal applies to.
    pub node: NodeId,
    /// How to reverse.
    pub payload: UndoPayload,
}

/// Something that knows how to apply an [`UndoRecord`] to itself.
///
/// The model implements this. `ast-core` drives rollback without knowing
/// what a struct member or a QoS profile is. Spec §5.1.
pub trait Undoable {
    /// Failure type of the reversal.
    type Error;

    /// Reverse one recorded mutation.
    ///
    /// # Errors
    /// Implementation-defined. A failure here means the model could not
    /// be restored, which the caller must treat as fatal rather than
    /// continuing on a half-reverted model.
    fn undo(&mut self, record: &UndoRecord) -> Result<(), Self::Error>;
}

/// Paired undo records and changes accumulated during a transaction.
///
/// Both sequences grow in application order. Rollback walks the undo
/// records backwards; a successful commit hands the changes to observers
/// forwards. Spec §5.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxnLog {
    undo: Vec<UndoRecord>,
    changes: Vec<Change>,
    sequence: u64,
}

impl TxnLog {
    /// Empty log for the given commit sequence number.
    #[must_use]
    pub const fn new(sequence: u64) -> Self {
        Self {
            undo: Vec::new(),
            changes: Vec::new(),
            sequence,
        }
    }

    /// The commit sequence number this log will stamp its changeset with.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Record one applied mutation and how to reverse it.
    pub fn record(&mut self, undo: UndoRecord, change: Change) {
        self.undo.push(undo);
        self.changes.push(change);
    }

    /// Whether the transaction has applied nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of applied mutations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Reversal steps in application order; call `.rev()` to roll back.
    pub fn undo_records(&self) -> impl DoubleEndedIterator<Item = &UndoRecord> {
        self.undo.iter()
    }

    /// The changes in application order, without consuming the log.
    pub fn changes(&self) -> impl Iterator<Item = &Change> {
        self.changes.iter()
    }

    /// Every node this transaction touched, in change order.
    pub fn nodes_touched(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.changes.iter().map(Change::node)
    }

    /// Consume the log into the changeset for observers.
    #[must_use]
    pub fn into_changeset(self) -> Changeset {
        let mut cs = Changeset::new(self.sequence);
        for change in self.changes {
            cs.push(change);
        }
        cs
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::change::NodeKindTag;
    use crate::id::IdSpace;
    use alloc::vec;
    use alloc::vec::Vec;

    fn node(counter: u64) -> NodeId {
        NodeId::from_parts(IdSpace::MODEL, counter)
    }

    #[test]
    fn undo_records_come_back_in_reverse_application_order() {
        let mut log = TxnLog::new(1);
        log.record(
            UndoRecord {
                node: node(1),
                payload: UndoPayload::Remove,
            },
            Change::Created {
                node: node(1),
                kind: NodeKindTag(1),
            },
        );
        log.record(
            UndoRecord {
                node: node(1),
                payload: UndoPayload::RestoreField {
                    field: FieldTag(0),
                    value: FieldValue::Absent,
                },
            },
            Change::Updated {
                node: node(1),
                field: FieldTag(0),
                before: FieldValue::Absent,
                after: FieldValue::Bool(true),
            },
        );

        let order: Vec<&UndoRecord> = log.undo_records().rev().collect();
        assert!(matches!(order[0].payload, UndoPayload::RestoreField { .. }));
        assert!(matches!(order[1].payload, UndoPayload::Remove));
    }

    #[test]
    fn the_log_yields_a_changeset_in_application_order() {
        let mut log = TxnLog::new(9);
        log.record(
            UndoRecord {
                node: node(1),
                payload: UndoPayload::Remove,
            },
            Change::Created {
                node: node(1),
                kind: NodeKindTag(1),
            },
        );
        log.record(
            UndoRecord {
                node: node(2),
                payload: UndoPayload::Remove,
            },
            Change::Created {
                node: node(2),
                kind: NodeKindTag(1),
            },
        );

        assert_eq!(log.len(), 2);
        let cs = log.into_changeset();
        assert_eq!(cs.sequence(), 9);
        assert_eq!(cs.len(), 2);
        let nodes: Vec<NodeId> = cs.nodes_touched().collect();
        assert_eq!(nodes, vec![node(1), node(2)]);
    }

    #[test]
    fn an_empty_log_is_reported_as_empty() {
        let log = TxnLog::new(0);
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }
}
