// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectStore` — pro-Client Object-Lifecycle (Spec §7.5).
//!
//! Pro `ProxyClient` haelt der Agent eine `BTreeMap<ObjectId, ObjectInstance>`,
//! die alle aktuell im Client erzeugten Objekte (Participant, Topic,
//! Publisher, Subscriber, DataWriter, DataReader, ...) enthaelt.
//!
//! ## CREATE-Semantik (Spec §7.5.1)
//!
//! Beim Empfang einer `CREATE`-Submessage entscheidet die Kombination
//! der `creation_mode`-Flags `reuse` und `replace` ueber das Verhalten
//! bei einer bereits existierenden ID:
//!
//! | reuse | replace | bei existierendem Objekt          |
//! |-------|---------|-----------------------------------|
//! | false | false   | Fehler (`STATUS_ERR_ALREADY_EXISTS`)|
//! | true  | false   | Wiederverwenden (`STATUS_OK_MATCHED`)|
//! | false | true    | Loeschen+Neu (`STATUS_OK`)         |
//! | true  | true    | Wiederverwenden (Variante 2 dominiert)|
//!
//! `STATUS_*` werden von der Agent-Aufrufseite zurueckgemeldet — das
//! Store liefert eine `CreateOutcome`, aus der der Caller den Status
//! ableiten kann.

extern crate alloc;
use alloc::collections::BTreeMap;

use crate::error::XrceError;
use crate::object_id::ObjectId;
use crate::object_kind::ObjectKind;
use crate::object_repr::ObjectVariant;

/// Creation-Mode-Flags (Spec §7.5.1, codiert als Submessage-Flags Bits 1+2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CreationMode {
    /// Existierendes Objekt wiederverwenden.
    pub reuse: bool,
    /// Existierendes Objekt loeschen und neu anlegen.
    pub replace: bool,
}

impl CreationMode {
    /// Strict: weder reuse noch replace.
    pub const STRICT: Self = Self {
        reuse: false,
        replace: false,
    };
    /// Reuse: bei Existenz wiederverwenden, sonst neu.
    pub const REUSE: Self = Self {
        reuse: true,
        replace: false,
    };
    /// Replace: bei Existenz loeschen+neu.
    pub const REPLACE: Self = Self {
        reuse: false,
        replace: true,
    };
}

/// Eine im Store registrierte Object-Instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInstance {
    /// Object-Kind (4-bit, gespiegelt aus der ObjectId).
    pub kind: ObjectKind,
    /// Aktuelle Wire-Repraesentation der Object-Daten.
    pub variant: ObjectVariant,
    /// Versionszaehler — wird bei Replace inkrementiert.
    pub version: u32,
}

impl ObjectInstance {
    /// Konstruktor.
    #[must_use]
    pub fn new(kind: ObjectKind, variant: ObjectVariant) -> Self {
        Self {
            kind,
            variant,
            version: 0,
        }
    }
}

/// Resultat einer `create`-Operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    /// Object wurde frisch angelegt.
    Created,
    /// Objekt existierte bereits, mit `reuse=true` wiederverwendet
    /// (Inhalt ist gleich → `STATUS_OK_MATCHED`, oder Inhalt unterscheidet
    /// sich → `STATUS_ERR_MISMATCH` — die Unterscheidung uebernimmt der Caller).
    Reused {
        /// `true`, falls der Inhalt mit dem bestehenden uebereinstimmt.
        equal: bool,
    },
    /// Objekt existierte bereits, mit `replace=true` ersetzt.
    Replaced,
    /// Objekt existierte bereits, kein reuse/replace → Konflikt.
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

    /// Anzahl gehaltener Objekte.
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

    /// Iteriert alle Objekte sortiert nach `ObjectId`.
    pub fn iter(&self) -> impl Iterator<Item = (ObjectId, &ObjectInstance)> {
        self.objects.iter().map(|(&id, inst)| (id, inst))
    }

    /// CREATE-Operation.
    ///
    /// Validiert dass der `kind` der `ObjectId` mit dem Argument
    /// uebereinstimmt (Spec §7.2.1: Lower-4-Bits-Kind muss zur Variante
    /// passen).
    ///
    /// # Errors
    /// - `ValueOutOfRange`, wenn `id.kind() != kind` oder `id` invalid ist.
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

    /// DELETE-Operation. Liefert `true`, wenn die ID existiert hat.
    pub fn delete(&mut self, id: ObjectId) -> bool {
        self.objects.remove(&id).is_some()
    }

    /// Loescht alle Objekte (z.B. wenn der ProxyClient terminated).
    pub fn clear(&mut self) {
        self.objects.clear();
    }

    /// Filtert nach Kind.
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
        // 1) Reuse mit gleichem Inhalt → equal=true
        let r1 = store
            .create(id, ObjectKind::Topic, first.clone(), CreationMode::REUSE)
            .unwrap();
        assert_eq!(r1, CreateOutcome::Reused { equal: true });
        // 2) Reuse mit anderem Inhalt → equal=false
        let r2 = store
            .create(
                id,
                ObjectKind::Topic,
                ObjectVariant::ByReference("other".into()),
                CreationMode::REUSE,
            )
            .unwrap();
        assert_eq!(r2, CreateOutcome::Reused { equal: false });
        // Inhalt wurde NICHT ersetzt
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
        // ObjectId mit Kind=Topic, aber wir geben DataWriter
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
        // BTreeMap sortiert nach ObjectId-raw
        assert!(ids.windows(2).all(|w| w[0] <= w[1]));
    }
}
