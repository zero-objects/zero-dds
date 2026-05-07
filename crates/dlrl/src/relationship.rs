// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Relationship-Resolver — DDS 1.4 §B.5.
//!
//! Voll implementiert.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::object_cache::ObjectId;

/// Relationship-Kind. Spec §B.5.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationshipKind {
    /// `Reference` — referentielle Bindung (Source kann frei
    /// erstellt werden).
    Reference,
    /// `Composition` — kompositionelle Bindung (Lifecycle gekoppelt).
    Composition,
}

/// Direction. Spec §B.5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// `Mono` — einseitig.
    Mono,
    /// `Bi` — beidseitig.
    Bi,
}

/// Cascade-Mode bei Update/Delete. Spec §B.5.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CascadeMode {
    /// `None` — keine Kaskade.
    None,
    /// `Update` — Updates kaskadieren.
    Update,
    /// `Delete` — Deletes kaskadieren.
    Delete,
    /// `All` — alle Kaskaden.
    All,
}

/// Eine konkrete Relationship-Definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    /// Frei waehlbarer Name (z.B. `"orders"`).
    pub name: String,
    /// Quelle.
    pub source: ObjectId,
    /// Ziel.
    pub target: ObjectId,
    /// Kind.
    pub kind: RelationshipKind,
    /// Direction.
    pub direction: Direction,
    /// Cascade-Mode.
    pub cascade: CascadeMode,
}

/// Resolver — Index ueber Relationships pro Source-Object.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RelationshipResolver {
    by_source: BTreeMap<ObjectId, Vec<Relationship>>,
}

impl RelationshipResolver {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuege eine Relationship hinzu.
    pub fn add(&mut self, rel: Relationship) {
        let source = rel.source.clone();
        self.by_source.entry(source).or_default().push(rel.clone());
        if rel.direction == Direction::Bi {
            let mut reverse = rel;
            core::mem::swap(&mut reverse.source, &mut reverse.target);
            self.by_source
                .entry(reverse.source.clone())
                .or_default()
                .push(reverse);
        }
    }

    /// Liste der Relationships, die von einer Source ausgehen.
    #[must_use]
    pub fn outgoing(&self, source: &ObjectId) -> Vec<&Relationship> {
        self.by_source
            .get(source)
            .map_or_else(Vec::new, |v| v.iter().collect())
    }

    /// Anzahl gespeicherter Relationships (inkl. inverser bei `Bi`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_source.values().map(Vec::len).sum()
    }

    /// `true` wenn keine Relationships.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_source.is_empty()
    }

    /// Spec §B.5.4 — kaskadierende Targets fuer eine Source-ID, wenn
    /// das Source-Object geloescht wird.
    #[must_use]
    pub fn cascade_targets_for_delete(&self, source: &ObjectId) -> Vec<ObjectId> {
        self.by_source.get(source).map_or_else(Vec::new, |rels| {
            rels.iter()
                .filter(|r| matches!(r.cascade, CascadeMode::Delete | CascadeMode::All))
                .map(|r| r.target.clone())
                .collect()
        })
    }

    /// Spec §B.5.4 — kaskadierende Targets fuer ein Update.
    #[must_use]
    pub fn cascade_targets_for_update(&self, source: &ObjectId) -> Vec<ObjectId> {
        self.by_source.get(source).map_or_else(Vec::new, |rels| {
            rels.iter()
                .filter(|r| matches!(r.cascade, CascadeMode::Update | CascadeMode::All))
                .map(|r| r.target.clone())
                .collect()
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn oid(t: &str, k: &[u8]) -> ObjectId {
        ObjectId::new(t.into(), k.to_vec())
    }

    fn rel(name: &str, src: ObjectId, tgt: ObjectId) -> Relationship {
        Relationship {
            name: name.into(),
            source: src,
            target: tgt,
            kind: RelationshipKind::Reference,
            direction: Direction::Mono,
            cascade: CascadeMode::None,
        }
    }

    #[test]
    fn mono_adds_one_entry() {
        let mut r = RelationshipResolver::new();
        r.add(rel("a", oid("T", b"1"), oid("T", b"2")));
        assert_eq!(r.len(), 1);
        assert_eq!(r.outgoing(&oid("T", b"1")).len(), 1);
        assert_eq!(r.outgoing(&oid("T", b"2")).len(), 0);
    }

    #[test]
    fn bi_adds_inverse() {
        let mut r = RelationshipResolver::new();
        let mut x = rel("a", oid("T", b"1"), oid("T", b"2"));
        x.direction = Direction::Bi;
        r.add(x);
        assert_eq!(r.len(), 2);
        assert_eq!(r.outgoing(&oid("T", b"1")).len(), 1);
        assert_eq!(r.outgoing(&oid("T", b"2")).len(), 1);
    }

    #[test]
    fn cascade_delete_targets_only_marked_relations() {
        let mut r = RelationshipResolver::new();
        let mut a = rel("a", oid("T", b"1"), oid("T", b"2"));
        a.cascade = CascadeMode::Delete;
        let b = rel("b", oid("T", b"1"), oid("T", b"3"));
        r.add(a);
        r.add(b);
        let targets = r.cascade_targets_for_delete(&oid("T", b"1"));
        assert_eq!(targets, alloc::vec![oid("T", b"2")]);
    }

    #[test]
    fn cascade_update_targets_only_marked_relations() {
        let mut r = RelationshipResolver::new();
        let mut a = rel("a", oid("T", b"1"), oid("T", b"2"));
        a.cascade = CascadeMode::Update;
        r.add(a);
        let mut b = rel("b", oid("T", b"1"), oid("T", b"3"));
        b.cascade = CascadeMode::All;
        r.add(b);
        let targets = r.cascade_targets_for_update(&oid("T", b"1"));
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn relationship_kind_distinct() {
        assert_ne!(RelationshipKind::Reference, RelationshipKind::Composition);
    }

    #[test]
    fn cascade_modes_distinct() {
        assert_ne!(CascadeMode::None, CascadeMode::All);
        assert_ne!(CascadeMode::Update, CascadeMode::Delete);
    }

    #[test]
    fn empty_resolver_returns_empty_lists() {
        let r = RelationshipResolver::new();
        assert!(r.is_empty());
        assert!(r.cascade_targets_for_delete(&oid("T", b"x")).is_empty());
        assert!(r.cascade_targets_for_update(&oid("T", b"x")).is_empty());
    }
}
