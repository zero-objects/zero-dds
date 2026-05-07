// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Active-Object-Map (AOM) — Spec §11.3.5.
//!
//! Bei RETAIN-POAs haelt der POA eine bidirektionale Map zwischen
//! ObjectId und Servant. UNIQUE_ID erlaubt nur 1:1-Mappings;
//! MULTIPLE_ID erlaubt einem Servant mehrere ObjectIds.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::error::{PoaError, PoaResult};
use crate::object_id::ObjectId;
use crate::policies::IdUniquenessPolicy;
use crate::servant::Servant;

/// Eindeutige interne ID pro registriertem Servant — wir brauchen
/// das, weil `Box<dyn Servant>` keine eindeutige Identitaet hat
/// (zwei Boxes auf dasselbe Type sind verschieden).
pub type ServantId = u64;

/// Active-Object-Map.
#[derive(Default)]
pub struct ActiveObjectMap {
    next_servant_id: ServantId,
    by_oid: BTreeMap<ObjectId, ServantId>,
    by_servant: BTreeMap<ServantId, Vec<ObjectId>>,
    servants: BTreeMap<ServantId, Box<dyn Servant>>,
    uniqueness: IdUniquenessPolicy,
}

impl core::fmt::Debug for ActiveObjectMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ActiveObjectMap")
            .field("entries", &self.by_oid.len())
            .field("servants", &self.servants.len())
            .field("uniqueness", &self.uniqueness)
            .finish()
    }
}

impl ActiveObjectMap {
    /// Konstruktor.
    #[must_use]
    pub const fn new(uniqueness: IdUniquenessPolicy) -> Self {
        Self {
            next_servant_id: 1,
            by_oid: BTreeMap::new(),
            by_servant: BTreeMap::new(),
            servants: BTreeMap::new(),
            uniqueness,
        }
    }

    /// Registriert ein neues Servant unter der gegebenen ObjectId.
    ///
    /// # Errors
    /// * `ObjectAlreadyActive` wenn `oid` schon mappt.
    /// * `ServantAlreadyActive` (UNIQUE_ID) wenn der Servant schon
    ///   eine andere ObjectId hat.
    pub fn activate(&mut self, oid: ObjectId, servant: Box<dyn Servant>) -> PoaResult<ServantId> {
        if self.by_oid.contains_key(&oid) {
            return Err(PoaError::ObjectAlreadyActive);
        }
        let sid = self.next_servant_id;
        self.next_servant_id += 1;
        self.by_oid.insert(oid.clone(), sid);
        self.by_servant.insert(sid, alloc::vec![oid]);
        self.servants.insert(sid, servant);
        Ok(sid)
    }

    /// Registriert ein bestehendes Servant unter einer zusaetzlichen
    /// ObjectId (nur unter MULTIPLE_ID erlaubt).
    ///
    /// # Errors
    /// * `ServantNotActive` wenn `sid` unbekannt.
    /// * `ObjectAlreadyActive` wenn `oid` schon mappt.
    /// * `ServantAlreadyActive` wenn UNIQUE_ID.
    pub fn add_alias(&mut self, sid: ServantId, oid: ObjectId) -> PoaResult<()> {
        if !self.servants.contains_key(&sid) {
            return Err(PoaError::ServantNotActive);
        }
        if self.uniqueness == IdUniquenessPolicy::Unique {
            return Err(PoaError::ServantAlreadyActive);
        }
        if self.by_oid.contains_key(&oid) {
            return Err(PoaError::ObjectAlreadyActive);
        }
        self.by_oid.insert(oid.clone(), sid);
        self.by_servant.entry(sid).or_default().push(oid);
        Ok(())
    }

    /// Liefert den Servant fuer eine ObjectId.
    #[must_use]
    pub fn get(&self, oid: &ObjectId) -> Option<&dyn Servant> {
        let sid = *self.by_oid.get(oid)?;
        self.servants.get(&sid).map(|s| s.as_ref())
    }

    /// Liefert die `ServantId` fuer eine ObjectId.
    #[must_use]
    pub fn servant_id(&self, oid: &ObjectId) -> Option<ServantId> {
        self.by_oid.get(oid).copied()
    }

    /// Liefert die Object-Ids unter denen ein Servant registriert ist.
    #[must_use]
    pub fn ids_for_servant(&self, sid: ServantId) -> &[ObjectId] {
        self.by_servant.get(&sid).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Deaktiviert eine ObjectId. Wenn der Servant keine weiteren
    /// Aliasses hat, wird er auch entfernt.
    ///
    /// # Errors
    /// `ObjectNotActive` wenn `oid` nicht mappt.
    pub fn deactivate(&mut self, oid: &ObjectId) -> PoaResult<Box<dyn Servant>> {
        let sid = self.by_oid.remove(oid).ok_or(PoaError::ObjectNotActive)?;
        if let Some(list) = self.by_servant.get_mut(&sid) {
            list.retain(|i| i != oid);
            if list.is_empty() {
                self.by_servant.remove(&sid);
                let s = self.servants.remove(&sid);
                return s.ok_or(PoaError::ServantNotActive);
            }
        }
        // Servant haengt noch unter anderen IDs — kein Servant
        // zurueckgeben (Box noch in Map).
        Err(PoaError::BadInvocationOrder(
            "servant still has other aliases".into(),
        ))
    }

    /// Anzahl Eintraege.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_oid.len()
    }

    /// `true` wenn leer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_oid.is_empty()
    }

    /// Setzt die Uniqueness-Policy (intern, fuer POA-Konstruktion).
    pub(crate) fn set_uniqueness(&mut self, u: IdUniquenessPolicy) {
        self.uniqueness = u;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::servant::EchoServant;

    fn echo() -> Box<dyn Servant> {
        Box::new(EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        })
    }

    #[test]
    fn activate_and_get_round_trip() {
        let mut m = ActiveObjectMap::new(IdUniquenessPolicy::Unique);
        let oid = ObjectId::system_id(1);
        let sid = m.activate(oid.clone(), echo()).unwrap();
        assert!(m.get(&oid).is_some());
        assert_eq!(m.servant_id(&oid), Some(sid));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn activate_twice_with_same_oid_is_rejected() {
        let mut m = ActiveObjectMap::new(IdUniquenessPolicy::Unique);
        let oid = ObjectId::system_id(1);
        m.activate(oid.clone(), echo()).unwrap();
        let err = m.activate(oid, echo()).unwrap_err();
        assert_eq!(err, PoaError::ObjectAlreadyActive);
    }

    #[test]
    fn unique_id_rejects_alias() {
        let mut m = ActiveObjectMap::new(IdUniquenessPolicy::Unique);
        let sid = m.activate(ObjectId::system_id(1), echo()).unwrap();
        let err = m.add_alias(sid, ObjectId::system_id(2)).unwrap_err();
        assert_eq!(err, PoaError::ServantAlreadyActive);
    }

    #[test]
    fn multiple_id_allows_aliases() {
        let mut m = ActiveObjectMap::new(IdUniquenessPolicy::Multiple);
        let sid = m.activate(ObjectId::system_id(1), echo()).unwrap();
        m.add_alias(sid, ObjectId::system_id(2)).unwrap();
        m.add_alias(sid, ObjectId::system_id(3)).unwrap();
        assert_eq!(m.ids_for_servant(sid).len(), 3);
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn deactivate_removes_only_when_last_alias_gone() {
        let mut m = ActiveObjectMap::new(IdUniquenessPolicy::Multiple);
        let sid = m.activate(ObjectId::system_id(1), echo()).unwrap();
        m.add_alias(sid, ObjectId::system_id(2)).unwrap();

        // Erste Deaktivierung — Servant haengt noch unter ID 2.
        let err = m.deactivate(&ObjectId::system_id(1)).unwrap_err();
        assert!(matches!(err, PoaError::BadInvocationOrder(_)));
        assert_eq!(m.len(), 1);

        // Zweite Deaktivierung entfernt den Servant.
        let _s = m.deactivate(&ObjectId::system_id(2)).unwrap();
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn deactivate_unknown_oid_is_diagnostic() {
        let mut m = ActiveObjectMap::new(IdUniquenessPolicy::Unique);
        let err = m.deactivate(&ObjectId::system_id(99)).unwrap_err();
        assert_eq!(err, PoaError::ObjectNotActive);
    }
}
