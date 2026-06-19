// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectStore` — per-client object lifecycle (Spec §7.5).
//!
//! Per `ProxyClient` the agent keeps a `BTreeMap<ObjectId, ObjectInstance>`
//! that contains all objects currently created in the client (participant,
//! topic, publisher, subscriber, DataWriter, DataReader, ...).
//!
//! ## CREATE semantics (Spec §7.5.1)
//!
//! On receiving a `CREATE` submessage, the combination of the
//! `creation_mode` flags `reuse` and `replace` decides the behavior
//! for an already existing ID:
//!
//! | reuse | replace | for an existing object            |
//! |-------|---------|-----------------------------------|
//! | false | false   | error (`STATUS_ERR_ALREADY_EXISTS`) |
//! | true  | false   | reuse (`STATUS_OK_MATCHED`)        |
//! | false | true    | delete+new (`STATUS_OK`)           |
//! | true  | true    | reuse (variant 2 dominates)        |
//!
//! `STATUS_*` are reported back by the agent caller side — the
//! store returns a `CreateOutcome` from which the caller can derive
//! the status.

extern crate alloc;
use alloc::collections::BTreeMap;

use crate::error::XrceError;
use crate::object_id::ObjectId;
use crate::object_kind::ObjectKind;
use crate::object_repr::ObjectVariant;

/// Creation mode flags (Spec §7.5.1, encoded as submessage flags bits 1+2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CreationMode {
    /// Reuse an existing object.
    pub reuse: bool,
    /// Delete an existing object and create it anew.
    pub replace: bool,
}

impl CreationMode {
    /// Strict: neither reuse nor replace.
    pub const STRICT: Self = Self {
        reuse: false,
        replace: false,
    };
    /// Reuse: reuse if it exists, otherwise create anew.
    pub const REUSE: Self = Self {
        reuse: true,
        replace: false,
    };
    /// Replace: delete+new if it exists.
    pub const REPLACE: Self = Self {
        reuse: false,
        replace: true,
    };
}

/// An object instance registered in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInstance {
    /// Object kind (4-bit, mirrored from the ObjectId).
    pub kind: ObjectKind,
    /// Current wire representation of the object data.
    pub variant: ObjectVariant,
    /// Version counter — incremented on replace.
    pub version: u32,
}

impl ObjectInstance {
    /// Constructor.
    #[must_use]
    pub fn new(kind: ObjectKind, variant: ObjectVariant) -> Self {
        Self {
            kind,
            variant,
            version: 0,
        }
    }
}

/// Result of a `create` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    /// Object was freshly created.
    Created,
    /// Object already existed, reused with `reuse=true`
    /// (content is equal → `STATUS_OK_MATCHED`, or content differs
    /// → `STATUS_ERR_MISMATCH` — the caller handles the distinction).
    Reused {
        /// `true` if the content matches the existing one.
        equal: bool,
    },
    /// Object already existed, replaced with `replace=true`.
    Replaced,
    /// Object already existed, no reuse/replace → conflict.
    Conflict,
}

/// Pro-Client Object-Store.
#[derive(Debug, Clone, Default)]
pub struct ObjectStore {
    objects: BTreeMap<ObjectId, ObjectInstance>,
}

impl ObjectStore {
    /// Leerer Store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of held objects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Leer?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Existiert dieses Object?
    #[must_use]
    pub fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains_key(&id)
    }

    /// Lookup.
    #[must_use]
    pub fn get(&self, id: ObjectId) -> Option<&ObjectInstance> {
        self.objects.get(&id)
    }

    /// Iterates all objects sorted by `ObjectId`.
    pub fn iter(&self) -> impl Iterator<Item = (ObjectId, &ObjectInstance)> {
        self.objects.iter().map(|(&id, inst)| (id, inst))
    }

    /// CREATE operation.
    ///
    /// Validates that the `kind` of the `ObjectId` matches the argument
    /// (Spec §7.2.1: the lower-4-bits kind must match the variant).
    ///
    /// # Errors
    /// - `ValueOutOfRange` if `id.kind() != kind` or `id` is invalid.
    pub fn create(
        &mut self,
        id: ObjectId,
        kind: ObjectKind,
        variant: ObjectVariant,
        mode: CreationMode,
    ) -> Result<CreateOutcome, XrceError> {
        if id.is_invalid() {
            return Err(XrceError::ValueOutOfRange {
                message: "cannot create OBJECTID_INVALID",
            });
        }
        let id_kind = id.kind()?;
        if id_kind != kind {
            return Err(XrceError::ValueOutOfRange {
                message: "ObjectId.kind() does not match argument kind",
            });
        }

        let existing = self.objects.get(&id);
        match (existing, mode.reuse, mode.replace) {
            (None, _, _) => {
                self.objects.insert(id, ObjectInstance::new(kind, variant));
                Ok(CreateOutcome::Created)
            }
            (Some(prev), true, _) => {
                let equal = prev.kind == kind && prev.variant == variant;
                Ok(CreateOutcome::Reused { equal })
            }
            (Some(prev), false, true) => {
                let next_version = prev.version.wrapping_add(1);
                self.objects.insert(
                    id,
                    ObjectInstance {
                        kind,
                        variant,
                        version: next_version,
                    },
                );
                Ok(CreateOutcome::Replaced)
            }
            (Some(_), false, false) => Ok(CreateOutcome::Conflict),
        }
    }

    /// DELETE operation. Returns `true` if the ID existed.
    pub fn delete(&mut self, id: ObjectId) -> bool {
        self.objects.remove(&id).is_some()
    }

    /// Deletes all objects (e.g. when the ProxyClient terminates).
    pub fn clear(&mut self) {
        self.objects.clear();
    }

    /// Filters by kind.
    pub fn iter_by_kind(
        &self,
        kind: ObjectKind,
    ) -> impl Iterator<Item = (ObjectId, &ObjectInstance)> {
        self.iter().filter(move |(_, inst)| inst.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use alloc::string::String;

    fn participant_xml(name: &str) -> ObjectVariant {
        ObjectVariant::ByXmlString(alloc::format!(
            "<dds><domain_participant><name>{name}</name></domain_participant></dds>"
        ))
    }

    #[test]
    fn create_inserts_new_object() {
        let mut store = ObjectStore::new();
        let id = ObjectId::new(0x010, ObjectKind::Participant).unwrap();
        let r = store
            .create(
                id,
                ObjectKind::Participant,
                participant_xml("p1"),
                CreationMode::STRICT,
            )
            .unwrap();
        assert_eq!(r, CreateOutcome::Created);
        assert!(store.contains(id));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn create_strict_on_existing_returns_conflict() {
        let mut store = ObjectStore::new();
        let id = ObjectId::new(0x010, ObjectKind::Topic).unwrap();
        store
            .create(
                id,
                ObjectKind::Topic,
                ObjectVariant::ByReference("foo".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        let r = store
            .create(
                id,
                ObjectKind::Topic,
                ObjectVariant::ByReference("bar".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        assert_eq!(r, CreateOutcome::Conflict);
    }

    #[test]
    fn create_reuse_returns_equal_marker() {
        let mut store = ObjectStore::new();
        let id = ObjectId::new(0x011, ObjectKind::Topic).unwrap();
        let first = ObjectVariant::ByReference("the_topic".into());
        store
            .create(id, ObjectKind::Topic, first.clone(), CreationMode::STRICT)
            .unwrap();
        // 1) Reuse with the same content → equal=true
        let r1 = store
            .create(id, ObjectKind::Topic, first.clone(), CreationMode::REUSE)
            .unwrap();
        assert_eq!(r1, CreateOutcome::Reused { equal: true });
        // 2) Reuse with different content → equal=false
        let r2 = store
            .create(
                id,
                ObjectKind::Topic,
                ObjectVariant::ByReference("other".into()),
                CreationMode::REUSE,
            )
            .unwrap();
        assert_eq!(r2, CreateOutcome::Reused { equal: false });
        // Content was NOT replaced
        assert_eq!(store.get(id).unwrap().variant, first);
        assert_eq!(store.get(id).unwrap().version, 0);
    }

    #[test]
    fn create_replace_increments_version() {
        let mut store = ObjectStore::new();
        let id = ObjectId::new(0x012, ObjectKind::DataWriter).unwrap();
        store
            .create(
                id,
                ObjectKind::DataWriter,
                ObjectVariant::ByReference("wr1".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        let r = store
            .create(
                id,
                ObjectKind::DataWriter,
                ObjectVariant::ByReference("wr2".into()),
                CreationMode::REPLACE,
            )
            .unwrap();
        assert_eq!(r, CreateOutcome::Replaced);
        let inst = store.get(id).unwrap();
        assert_eq!(inst.version, 1);
        assert_eq!(inst.variant, ObjectVariant::ByReference("wr2".into()));
    }

    #[test]
    fn delete_removes_object() {
        let mut store = ObjectStore::new();
        let id = ObjectId::new(0x020, ObjectKind::Subscriber).unwrap();
        store
            .create(
                id,
                ObjectKind::Subscriber,
                ObjectVariant::ByReference("s".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        assert!(store.delete(id));
        assert!(!store.contains(id));
        assert!(!store.delete(id));
    }

    #[test]
    fn create_kind_mismatch_with_id_rejected() {
        let mut store = ObjectStore::new();
        // ObjectId with kind=Topic, but we pass DataWriter
        let id = ObjectId::new(0x030, ObjectKind::Topic).unwrap();
        let r = store.create(
            id,
            ObjectKind::DataWriter,
            ObjectVariant::ByReference("x".into()),
            CreationMode::STRICT,
        );
        assert!(r.is_err());
    }

    #[test]
    fn create_invalid_object_id_rejected() {
        let mut store = ObjectStore::new();
        let r = store.create(
            crate::object_id::OBJECTID_INVALID,
            ObjectKind::Topic,
            ObjectVariant::ByReference("x".into()),
            CreationMode::STRICT,
        );
        assert!(r.is_err());
    }

    #[test]
    fn iter_by_kind_filters() {
        let mut store = ObjectStore::new();
        let topic = ObjectId::new(0x100, ObjectKind::Topic).unwrap();
        let dw = ObjectId::new(0x101, ObjectKind::DataWriter).unwrap();
        let dr = ObjectId::new(0x102, ObjectKind::DataReader).unwrap();
        store
            .create(
                topic,
                ObjectKind::Topic,
                ObjectVariant::ByReference("t".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        store
            .create(
                dw,
                ObjectKind::DataWriter,
                ObjectVariant::ByReference("w".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        store
            .create(
                dr,
                ObjectKind::DataReader,
                ObjectVariant::ByReference("r".into()),
                CreationMode::STRICT,
            )
            .unwrap();
        let topics: alloc::vec::Vec<_> = store.iter_by_kind(ObjectKind::Topic).collect();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].0, topic);
    }

    #[test]
    fn clear_drops_everything() {
        let mut store = ObjectStore::new();
        for i in 0..10 {
            let id = ObjectId::new(i, ObjectKind::Topic).unwrap();
            store
                .create(
                    id,
                    ObjectKind::Topic,
                    ObjectVariant::ByReference(String::from("x")),
                    CreationMode::STRICT,
                )
                .unwrap();
        }
        assert_eq!(store.len(), 10);
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn replace_then_reuse_is_consistent() {
        let mut store = ObjectStore::new();
        let id = ObjectId::new(0x040, ObjectKind::Topic).unwrap();
        let v1 = ObjectVariant::ByReference("A".into());
        let v2 = ObjectVariant::ByReference("B".into());
        store
            .create(id, ObjectKind::Topic, v1.clone(), CreationMode::STRICT)
            .unwrap();
        store
            .create(id, ObjectKind::Topic, v2.clone(), CreationMode::REPLACE)
            .unwrap();
        let r = store
            .create(id, ObjectKind::Topic, v2.clone(), CreationMode::REUSE)
            .unwrap();
        assert_eq!(r, CreateOutcome::Reused { equal: true });
        assert_eq!(store.get(id).unwrap().version, 1);
    }

    #[test]
    fn iter_yields_sorted_ids() {
        let mut store = ObjectStore::new();
        for raw in [0x300u16, 0x100, 0x200] {
            let id = ObjectId::new(raw, ObjectKind::Topic).unwrap();
            store
                .create(
                    id,
                    ObjectKind::Topic,
                    ObjectVariant::ByReference(String::from("x")),
                    CreationMode::STRICT,
                )
                .unwrap();
        }
        let ids: alloc::vec::Vec<ObjectId> = store.iter().map(|(id, _)| id).collect();
        // BTreeMap sorted by ObjectId raw
        assert!(ids.windows(2).all(|w| w[0] <= w[1]));
    }
}
