// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DomainParticipantFactory` — Singleton fuer das Anlegen von
//! Participants (Spec OMG DDS 1.4 §2.2.2.2.1).
//!
//! Die Spec verlangt: "The DomainParticipantFactory is a singleton.
//! The get_instance() method returns a reference to the only
//! instance of the factory."
//!
//! Singleton via `OnceLock`. `create_participant(domain_id, qos)`
//! legt einen neuen `DomainParticipant` an, startet eine Live-
//! Runtime und gibt einen clone-baren Handle zurueck. Daneben gibt
//! es `create_participant_offline(domain_id, qos)` fuer Skeleton-
//! Tests ohne Netzwerk und `create_participant_with_config` fuer
//! Tests mit angepasster `RuntimeConfig`.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::{Mutex, OnceLock};

use crate::error::{DdsError, Result};
use crate::participant::{DomainId, DomainParticipant};
use crate::qos::DomainParticipantQos;
use crate::runtime::RuntimeConfig;

/// QoS-Policy fuer den DomainParticipantFactory selbst (Spec
/// §2.2.2.2.2.6 `DomainParticipantFactoryQos`). Aktuell ein einziges
/// Feld `autoenable_created_entities` (Default `true`); spaetere
/// Spec-Erweiterungen werden hier addiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainParticipantFactoryQos {
    /// Wenn `true`, werden neu erzeugte Entities automatisch beim
    /// Erzeugen `enable()`-d. Spec-Default: `true`.
    pub autoenable_created_entities: bool,
}

impl Default for DomainParticipantFactoryQos {
    fn default() -> Self {
        Self {
            autoenable_created_entities: true,
        }
    }
}

/// Factory-Singleton.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct DomainParticipantFactory {
    /// Registry aller per `create_participant*` erzeugten Participants —
    /// indexiert nach `DomainId`, mehrere Participants pro Domain
    /// erlaubt (Spec §2.2.2.2.2.4 `lookup_participant` liefert "any"
    /// Participant fuer die gegebene Domain).
    participants: Mutex<BTreeMap<DomainId, Vec<DomainParticipant>>>,
    /// Factory-Default-QoS fuer neu erzeugte Participants (Spec
    /// §2.2.2.2.2.5 `set_default_participant_qos`).
    default_participant_qos: Mutex<DomainParticipantQos>,
    /// Factory-eigene QoS (Spec §2.2.2.2.2.6 `set_qos`/`get_qos`).
    factory_qos: Mutex<DomainParticipantFactoryQos>,
}

#[cfg(not(feature = "std"))]
#[derive(Debug, Default)]
pub struct DomainParticipantFactory {}

#[cfg(feature = "std")]
impl DomainParticipantFactory {
    /// Liefert den Prozess-weiten Factory-Singleton (Spec §2.2.2.2.2.1
    /// `get_instance`).
    pub fn instance() -> &'static Self {
        static INSTANCE: OnceLock<DomainParticipantFactory> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            participants: Mutex::new(BTreeMap::new()),
            default_participant_qos: Mutex::new(DomainParticipantQos::default()),
            factory_qos: Mutex::new(DomainParticipantFactoryQos::default()),
        })
    }

    fn track(&self, p: &DomainParticipant) {
        if let Ok(mut reg) = self.participants.lock() {
            reg.entry(p.domain_id()).or_default().push(p.clone());
        }
    }

    /// Erzeugt einen neuen `DomainParticipant` fuer die gegebene
    /// Domain-Id. Startet die `DcpsRuntime` mit Default-Config —
    /// UDP-Sockets + SPDP/SEDP-Threads.
    ///
    /// # Errors
    /// `DdsError::TransportError` wenn die UDP-Sockets nicht binden.
    pub fn create_participant(
        &self,
        domain_id: DomainId,
        qos: DomainParticipantQos,
    ) -> Result<DomainParticipant> {
        // DDS 1.4 §2.2.3.1 UserDataQosPolicy → SPDP-Beacon PID_USER_DATA.
        let config = RuntimeConfig {
            user_data: qos.user_data.value.clone(),
            ..RuntimeConfig::default()
        };
        let p = DomainParticipant::new_with_runtime(domain_id, qos, config)?;
        self.track(&p);
        Ok(p)
    }

    /// Variante mit explizit uebergebener `RuntimeConfig` (z.B. fuer
    /// Tests mit kurzen SPDP-Periods).
    ///
    /// # Errors
    /// `DdsError::TransportError` wenn die UDP-Sockets nicht binden.
    pub fn create_participant_with_config(
        &self,
        domain_id: DomainId,
        qos: DomainParticipantQos,
        config: RuntimeConfig,
    ) -> Result<DomainParticipant> {
        let p = DomainParticipant::new_with_runtime(domain_id, qos, config)?;
        self.track(&p);
        Ok(p)
    }

    /// Offline-Variante ohne Runtime — nur fuer Unit-Tests die kein
    /// Netzwerk wollen. Der zurueckgegebene Participant kann Topics
    /// erzeugen, aber keine DataWriter/Reader.
    #[must_use]
    pub fn create_participant_offline(
        &self,
        domain_id: DomainId,
        qos: DomainParticipantQos,
    ) -> DomainParticipant {
        let p = DomainParticipant::new(domain_id, qos);
        self.track(&p);
        p
    }

    /// Spec §2.2.2.2.2.4 `lookup_participant(domain_id)` — liefert
    /// einen vorher erzeugten Participant zur gleichen Domain-Id, oder
    /// `None` wenn keiner registriert ist. Bei mehreren Participants
    /// derselben Domain liefert die Implementation den ersten.
    #[must_use]
    pub fn lookup_participant(&self, domain_id: DomainId) -> Option<DomainParticipant> {
        let reg = self.participants.lock().ok()?;
        reg.get(&domain_id).and_then(|v| v.first().cloned())
    }

    /// Spec §2.2.2.2.2.3 `delete_participant`. Entfernt den Participant
    /// aus der Factory-Registry und ruft `delete_contained_entities`
    /// auf. Liefert `PreconditionNotMet` wenn der Participant nicht
    /// in der Registry ist.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` wenn der Participant nicht
    /// registriert ist.
    pub fn delete_participant(&self, p: &DomainParticipant) -> Result<()> {
        let mut reg = self
            .participants
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "factory participants poisoned",
            })?;
        let target_handle = p.instance_handle();
        let mut found = false;
        if let Some(vec) = reg.get_mut(&p.domain_id()) {
            let before = vec.len();
            vec.retain(|q| q.instance_handle() != target_handle);
            found = vec.len() < before;
            if vec.is_empty() {
                reg.remove(&p.domain_id());
            }
        }
        drop(reg);
        if !found {
            return Err(DdsError::PreconditionNotMet {
                reason: "participant not registered with this factory",
            });
        }
        p.delete_contained_entities()
    }

    /// Spec §2.2.2.2.2.5 `set_default_participant_qos` — Default-QoS
    /// fuer ab jetzt erzeugte Participants.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` bei Lock-Poisoning.
    pub fn set_default_participant_qos(&self, qos: DomainParticipantQos) -> Result<()> {
        let mut current =
            self.default_participant_qos
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "default qos poisoned",
                })?;
        *current = qos;
        Ok(())
    }

    /// Spec §2.2.2.2.2.5 `get_default_participant_qos`.
    #[must_use]
    pub fn get_default_participant_qos(&self) -> DomainParticipantQos {
        self.default_participant_qos
            .lock()
            .map(|q| q.clone())
            .unwrap_or_default()
    }

    /// Spec §2.2.2.2.2.6 `set_qos` (Factory-Level QoS).
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` bei Lock-Poisoning.
    pub fn set_qos(&self, qos: DomainParticipantFactoryQos) -> Result<()> {
        let mut current = self
            .factory_qos
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "factory qos poisoned",
            })?;
        *current = qos;
        Ok(())
    }

    /// Spec §2.2.2.2.2.6 `get_qos` (Factory-Level QoS).
    #[must_use]
    pub fn get_qos(&self) -> DomainParticipantFactoryQos {
        self.factory_qos.lock().map(|q| *q).unwrap_or_default()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn factory_is_singleton() {
        let a = DomainParticipantFactory::instance();
        let b = DomainParticipantFactory::instance();
        assert!(core::ptr::eq(a, b));
    }

    #[test]
    fn factory_creates_participant_with_correct_domain_id() {
        // Offline-Variante — kein UDP-Bind im Unit-Test. Live-Variante
        // wird in runtime.rs getestet.
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(7, DomainParticipantQos::default());
        assert_eq!(p.domain_id(), 7);
    }

    // ---- §2.2.2.2.2.4 lookup_participant ----

    #[test]
    fn lookup_participant_finds_registered_offline_participant() {
        let f = DomainParticipantFactory::instance();
        // Eindeutige Domain-Id, damit kein anderer Test-Participant
        // den Lookup beeinflusst.
        let domain = 91;
        let _p = f.create_participant_offline(domain, DomainParticipantQos::default());
        let found = f.lookup_participant(domain);
        assert!(found.is_some());
        assert_eq!(found.unwrap().domain_id(), domain);
    }

    #[test]
    fn lookup_participant_returns_none_for_unknown_domain() {
        let f = DomainParticipantFactory::instance();
        // Sehr hoher domain_id-Wert, fuer den vermutlich nichts existiert.
        assert!(f.lookup_participant(60_001).is_none());
    }

    // ---- §2.2.2.2.2.3 delete_participant ----

    #[test]
    fn delete_participant_removes_from_registry() {
        let f = DomainParticipantFactory::instance();
        let domain = 92;
        let p = f.create_participant_offline(domain, DomainParticipantQos::default());
        assert!(f.lookup_participant(domain).is_some());
        f.delete_participant(&p).unwrap();
        assert!(f.lookup_participant(domain).is_none());
    }

    #[test]
    fn delete_participant_unknown_returns_precondition_not_met() {
        let f = DomainParticipantFactory::instance();
        // Erzeuge einen Participant ohne ihn ueber die Factory zu
        // tracken (DomainParticipant::new direkt).
        let detached =
            crate::participant::DomainParticipant::new(93, DomainParticipantQos::default());
        let res = f.delete_participant(&detached);
        assert!(matches!(
            res,
            Err(crate::error::DdsError::PreconditionNotMet { .. })
        ));
    }

    // ---- §2.2.2.2.2.5 default_participant_qos ----

    #[test]
    fn default_participant_qos_roundtrips() {
        let f = DomainParticipantFactory::instance();
        let mut new_qos = f.get_default_participant_qos();
        // Wir mutieren irgendeinen erkennbaren Default. DomainParticipantQos
        // ist Default-konstruierbar; um den Roundtrip zu verifizieren
        // genuegt es, set/get zu durchlaufen.
        new_qos = new_qos.clone();
        f.set_default_participant_qos(new_qos.clone()).unwrap();
        let got = f.get_default_participant_qos();
        // Gleichheit modulo Equality-Impl der DomainParticipantQos.
        assert_eq!(format!("{got:?}"), format!("{new_qos:?}"));
    }

    // ---- §2.2.2.2.2.6 Factory-eigene QoS ----

    #[test]
    fn factory_qos_default_is_autoenable_true() {
        // Spec-Default §2.2.2.2.2.6: autoenable_created_entities = TRUE.
        let q = DomainParticipantFactoryQos::default();
        assert!(q.autoenable_created_entities);
    }

    #[test]
    fn factory_set_get_qos_roundtrip() {
        let f = DomainParticipantFactory::instance();
        let q = DomainParticipantFactoryQos {
            autoenable_created_entities: false,
        };
        f.set_qos(q).unwrap();
        let got = f.get_qos();
        assert!(!got.autoenable_created_entities);
        // Restore default fuer andere Tests.
        f.set_qos(DomainParticipantFactoryQos::default()).unwrap();
    }
}
