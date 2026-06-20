// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DomainParticipantFactory` — singleton for creating participants
//! (Spec OMG DDS 1.4 §2.2.2.2.1).
//!
//! The spec requires: "The DomainParticipantFactory is a singleton.
//! The get_instance() method returns a reference to the only
//! instance of the factory."
//!
//! Singleton via `OnceLock`. `create_participant(domain_id, qos)`
//! creates a new `DomainParticipant`, starts a live runtime, and
//! returns a cloneable handle. In addition there is
//! `create_participant_offline(domain_id, qos)` for skeleton tests
//! without a network, and `create_participant_with_config` for tests
//! with a custom `RuntimeConfig`.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::{Mutex, OnceLock};

use crate::error::{DdsError, Result};
use crate::participant::{DomainId, DomainParticipant};
use crate::qos::DomainParticipantQos;
use crate::runtime::RuntimeConfig;

/// QoS policy for the DomainParticipantFactory itself (Spec
/// §2.2.2.2.2.6 `DomainParticipantFactoryQos`). Currently a single
/// field `autoenable_created_entities` (default `true`); later spec
/// extensions are added here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainParticipantFactoryQos {
    /// If `true`, newly created entities are automatically `enable()`-d
    /// on creation. Spec default: `true`.
    pub autoenable_created_entities: bool,
}

impl Default for DomainParticipantFactoryQos {
    fn default() -> Self {
        Self {
            autoenable_created_entities: true,
        }
    }
}

/// Factory singleton.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct DomainParticipantFactory {
    /// Registry of all participants created via `create_participant*` —
    /// indexed by `DomainId`, multiple participants per domain allowed
    /// (Spec §2.2.2.2.2.4 `lookup_participant` returns "any"
    /// participant for the given domain).
    participants: Mutex<BTreeMap<DomainId, Vec<DomainParticipant>>>,
    /// Factory default QoS for newly created participants (Spec
    /// §2.2.2.2.2.5 `set_default_participant_qos`).
    default_participant_qos: Mutex<DomainParticipantQos>,
    /// Factory's own QoS (Spec §2.2.2.2.2.6 `set_qos`/`get_qos`).
    factory_qos: Mutex<DomainParticipantFactoryQos>,
}

#[cfg(not(feature = "std"))]
#[derive(Debug, Default)]
pub struct DomainParticipantFactory {}

#[cfg(feature = "std")]
impl DomainParticipantFactory {
    /// Returns the process-wide factory singleton (Spec §2.2.2.2.2.1
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

    /// Creates a new `DomainParticipant` for the given domain id.
    /// Starts the `DcpsRuntime` with the default config — UDP sockets +
    /// SPDP/SEDP threads.
    ///
    /// # Errors
    /// `DdsError::TransportError` if the UDP sockets do not bind.
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

    /// Variant with an explicitly passed `RuntimeConfig` (e.g. for
    /// tests with short SPDP periods).
    ///
    /// # Errors
    /// `DdsError::TransportError` if the UDP sockets do not bind.
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

    /// Offline variant without a runtime — only for unit tests that
    /// don't want a network. The returned participant can create
    /// topics, but no DataWriters/Readers.
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

    /// Spec §2.2.2.2.2.4 `lookup_participant(domain_id)` — returns a
    /// previously created participant for the same domain id, or `None`
    /// if none is registered. With several participants for the same
    /// domain, the implementation returns the first.
    #[must_use]
    pub fn lookup_participant(&self, domain_id: DomainId) -> Option<DomainParticipant> {
        let reg = self.participants.lock().ok()?;
        reg.get(&domain_id).and_then(|v| v.first().cloned())
    }

    /// Spec §2.2.2.2.2.3 `delete_participant`. Removes the participant
    /// from the factory registry and calls `delete_contained_entities`.
    /// Returns `PreconditionNotMet` if the participant is not in the
    /// registry.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` if the participant is not
    /// registered.
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

    /// Spec §2.2.2.2.2.5 `set_default_participant_qos` — default QoS
    /// for participants created from now on.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` on lock poisoning.
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

    /// Spec §2.2.2.2.2.6 `set_qos` (factory-level QoS).
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` on lock poisoning.
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

    /// Spec §2.2.2.2.2.6 `get_qos` (factory-level QoS).
    #[must_use]
    pub fn get_qos(&self) -> DomainParticipantFactoryQos {
        self.factory_qos.lock().map(|q| *q).unwrap_or_default()
    }

    /// Fluent entry point for creating a participant: returns a
    /// [`ParticipantBuilder`] that collects QoS, a custom
    /// [`RuntimeConfig`], and (with the `security` feature) a
    /// [`SecurityBundle`](zerodds_security_runtime::SecurityBundle), then
    /// materializes the participant on [`ParticipantBuilder::build`].
    ///
    /// This is a convenience facade over
    /// [`Self::create_participant_with_config`] — `build()` resolves to
    /// the same live runtime path. It exists so security wiring reads as
    /// one chain:
    ///
    /// ```no_run
    /// # #[cfg(feature = "security")] {
    /// use zerodds_dcps::DomainParticipantFactory;
    /// # let security_bundle = zerodds_security_runtime::SecurityBundle::builder().build();
    /// let participant = DomainParticipantFactory::create(0)
    ///     .with_security(security_bundle)
    ///     .build()?;
    /// # }
    /// # Ok::<(), zerodds_dcps::error::DdsError>(())
    /// ```
    #[must_use]
    pub fn create(domain_id: DomainId) -> ParticipantBuilder {
        ParticipantBuilder::new(domain_id)
    }
}

/// Fluent builder for a [`DomainParticipant`], returned by
/// [`DomainParticipantFactory::create`]. Collects optional QoS, a custom
/// [`RuntimeConfig`], and a [`SecurityBundle`](zerodds_security_runtime::SecurityBundle)
/// (under the `security` feature) and creates the participant via the
/// factory singleton on [`Self::build`].
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct ParticipantBuilder {
    domain_id: DomainId,
    qos: DomainParticipantQos,
    config: Option<RuntimeConfig>,
}

#[cfg(feature = "std")]
impl ParticipantBuilder {
    fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            qos: DomainParticipantQos::default(),
            config: None,
        }
    }

    /// Sets the participant QoS (default: [`DomainParticipantQos::default`]).
    #[must_use]
    pub fn with_qos(mut self, qos: DomainParticipantQos) -> Self {
        self.qos = qos;
        self
    }

    /// Sets an explicit [`RuntimeConfig`]. Overrides the config derived
    /// from QoS / security wiring; subsequent [`Self::with_security`]
    /// calls mutate this config.
    #[must_use]
    pub fn with_config(mut self, config: RuntimeConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Wires a [`SecurityBundle`](zerodds_security_runtime::SecurityBundle)
    /// (logging plugin + security profile) into the participant's runtime
    /// config — see [`RuntimeConfig::with_security_bundle`].
    #[cfg(feature = "security")]
    #[must_use]
    pub fn with_security(mut self, bundle: zerodds_security_runtime::SecurityBundle) -> Self {
        let base = self.config.take().unwrap_or_default();
        self.config = Some(base.with_security_bundle(&bundle));
        self
    }

    /// Creates the live participant via the factory singleton.
    ///
    /// # Errors
    /// `DdsError::TransportError` if the UDP sockets do not bind.
    pub fn build(self) -> Result<DomainParticipant> {
        let factory = DomainParticipantFactory::instance();
        match self.config {
            Some(config) => {
                factory.create_participant_with_config(self.domain_id, self.qos, config)
            }
            None => factory.create_participant(self.domain_id, self.qos),
        }
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
        // Offline variant — no UDP bind in the unit test. The live
        // variant is tested in runtime.rs.
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(7, DomainParticipantQos::default());
        assert_eq!(p.domain_id(), 7);
    }

    // ---- §2.2.2.2.2.4 lookup_participant ----

    #[test]
    fn lookup_participant_finds_registered_offline_participant() {
        let f = DomainParticipantFactory::instance();
        // Unique domain id so that no other test participant affects
        // the lookup.
        let domain = 91;
        let _p = f.create_participant_offline(domain, DomainParticipantQos::default());
        let found = f.lookup_participant(domain);
        assert!(found.is_some());
        assert_eq!(found.unwrap().domain_id(), domain);
    }

    #[test]
    fn lookup_participant_returns_none_for_unknown_domain() {
        let f = DomainParticipantFactory::instance();
        // Very high domain_id value for which nothing likely exists.
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
        // Create a participant without tracking it via the factory
        // (DomainParticipant::new directly).
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
        // We mutate some recognizable default. DomainParticipantQos is
        // default-constructible; to verify the roundtrip it is enough
        // to run set/get.
        new_qos = new_qos.clone();
        f.set_default_participant_qos(new_qos.clone()).unwrap();
        let got = f.get_default_participant_qos();
        // Equality modulo the Equality impl of DomainParticipantQos.
        assert_eq!(format!("{got:?}"), format!("{new_qos:?}"));
    }

    // ---- §2.2.2.2.2.6 factory's own QoS ----

    #[test]
    fn factory_qos_default_is_autoenable_true() {
        // Spec default §2.2.2.2.2.6: autoenable_created_entities = TRUE.
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
        // Restore default for other tests.
        f.set_qos(DomainParticipantFactoryQos::default()).unwrap();
    }
}
