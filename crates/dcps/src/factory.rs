// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DomainParticipantFactory` — singleton for creating participants
//! (Spec OMG DDS 1.4 §2.2.2.2.1).
//!
//! The spec requires: "The DomainParticipantFactory is a singleton.
//! The get_instance() method returns a reference to the only
//! instance of the factory."
//!
//! Singleton via `LazyLock`. `create_participant(domain_id, qos)`
//! creates a new `DomainParticipant`, starts a live runtime, and
//! returns a cloneable handle. In addition there is
//! `create_participant_offline(domain_id, qos)` for skeleton tests
//! without a network, and `create_participant_with_config` for tests
//! with a custom `RuntimeConfig`.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::{LazyLock, Mutex, Weak};

use crate::entity::Entity;
use crate::error::{DdsError, Result};
use crate::participant::{DomainId, DomainParticipant, ParticipantInner};
use crate::qos::{DomainParticipantQos, QosArg};
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
    /// (Spec §2.2.2.2.2.4 `lookup_participant` returns "any" participant for
    /// the given domain).
    ///
    /// Stored as **weak** references so the factory never keeps a participant
    /// alive past the user's last strong handle: dropping the user's
    /// `DomainParticipant` releases its runtime (threads + sockets) via RAII.
    /// `lookup_participant` upgrades on demand and garbage-collects dead weaks;
    /// `delete_participant` stays available for explicit, spec-symmetric cleanup
    /// but is a no-op once the participant has already dropped.
    participants: Mutex<BTreeMap<DomainId, Vec<Weak<ParticipantInner>>>>,
    /// Factory default QoS for newly created participants (Spec
    /// §2.2.2.2.2.5 `set_default_participant_qos`).
    default_participant_qos: Mutex<DomainParticipantQos>,
    /// Factory's own QoS (Spec §2.2.2.2.2.6 `set_qos`/`get_qos`).
    factory_qos: Mutex<DomainParticipantFactoryQos>,
}

#[cfg(not(feature = "std"))]
#[derive(Debug, Default)]
pub struct DomainParticipantFactory {}

/// DDS 1.4 §2.2.2.2.2.3: "The pre-defined value TheParticipantFactory can
/// also be used as an alias for the singleton factory returned by the
/// operation get_instance."
#[cfg(feature = "std")]
pub static THE_PARTICIPANT_FACTORY: LazyLock<DomainParticipantFactory> =
    LazyLock::new(|| DomainParticipantFactory {
        participants: Mutex::new(BTreeMap::new()),
        default_participant_qos: Mutex::new(DomainParticipantQos::default()),
        factory_qos: Mutex::new(DomainParticipantFactoryQos::default()),
    });

#[cfg(feature = "std")]
impl DomainParticipantFactory {
    /// Returns the process-wide factory singleton (Spec §2.2.2.2.2.1
    /// `get_instance`), the one [`THE_PARTICIPANT_FACTORY`] names.
    pub fn instance() -> &'static Self {
        &THE_PARTICIPANT_FACTORY
    }

    fn track(&self, p: &DomainParticipant) {
        if let Ok(mut reg) = self.participants.lock() {
            reg.entry(p.domain_id()).or_default().push(p.downgrade());
        }
        self.autoenable(p);
    }

    /// Spec §2.2.3.20: "The setting of `autoenable_created_entities` to
    /// TRUE indicates that the factory `create_<entity>` operation will
    /// automatically enable the created entity." The factory is the
    /// participant's factory, so its own [`DomainParticipantFactoryQos`]
    /// decides.
    fn autoenable(&self, p: &DomainParticipant) {
        if self.get_qos().autoenable_created_entities {
            p.entity_state().enable();
        }
    }

    /// Creates a new `DomainParticipant` for the given domain id.
    /// Starts the `DcpsRuntime` with the default config — UDP sockets +
    /// SPDP/SEDP threads. With [`PARTICIPANT_QOS_DEFAULT`] the participant
    /// gets [`Self::get_default_participant_qos`] (DDS 1.4 §2.2.2.2.2.1).
    ///
    /// [`PARTICIPANT_QOS_DEFAULT`]: crate::psm_constants::PARTICIPANT_QOS_DEFAULT
    ///
    /// # Errors
    /// `DdsError::TransportError` if the UDP sockets do not bind.
    pub fn create_participant(
        &self,
        domain_id: DomainId,
        qos: impl Into<QosArg<DomainParticipantQos>>,
    ) -> Result<DomainParticipant> {
        let qos = self.resolve(qos);
        // DDS 1.4 §2.2.3.1 UserDataQosPolicy → SPDP-Beacon PID_USER_DATA.
        // A participant this factory does not enable on creation does not
        // announce itself before its `enable` (§2.2.2.1.1.7).
        let config = RuntimeConfig {
            user_data: qos.user_data.value.clone(),
            discoverable: self.get_qos().autoenable_created_entities,
            ..RuntimeConfig::from_standard_paths().map_err(|error| {
                crate::error::DdsError::Configuration {
                    message: error.to_string(),
                }
            })?
        };
        let p = DomainParticipant::new_with_runtime(domain_id, qos, config)?;
        self.track(&p);
        Ok(p)
    }

    /// DDS 1.4 §2.2.2.2.2.1 `create_participant(domain_id, qos, a_listener,
    /// mask)`: the participant with its listener. `None` with a non-empty
    /// `mask` is the 'nil' listener (§2.2.2.1.1.3).
    ///
    /// # Errors
    /// As [`Self::create_participant`].
    pub fn create_participant_with_listener(
        &self,
        domain_id: DomainId,
        qos: impl Into<QosArg<DomainParticipantQos>>,
        listener: Option<crate::listener::ArcDomainParticipantListener>,
        mask: crate::entity::StatusMask,
    ) -> Result<DomainParticipant> {
        let participant = self.create_participant(domain_id, qos)?;
        participant.set_listener(listener, mask);
        Ok(participant)
    }

    /// Variant with an explicitly passed `RuntimeConfig` (e.g. for
    /// tests with short SPDP periods).
    ///
    /// # Errors
    /// `DdsError::TransportError` if the UDP sockets do not bind.
    pub fn create_participant_with_config(
        &self,
        domain_id: DomainId,
        qos: impl Into<QosArg<DomainParticipantQos>>,
        mut config: RuntimeConfig,
    ) -> Result<DomainParticipant> {
        let qos = self.resolve(qos);
        // DDS 1.4 §2.2.3.1: the QoS is the authority for PID_USER_DATA. A
        // config built from files has none, and dropping the QoS value here
        // would announce an empty one.
        if !qos.user_data.value.is_empty() {
            config.user_data = qos.user_data.value.clone();
        }
        // See `create_participant`.
        config.discoverable &= self.get_qos().autoenable_created_entities;
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
        qos: impl Into<QosArg<DomainParticipantQos>>,
    ) -> DomainParticipant {
        let p = DomainParticipant::new(domain_id, self.resolve(qos));
        self.track(&p);
        p
    }

    /// Spec §2.2.2.2.2.4 `lookup_participant(domain_id)` — returns a
    /// previously created participant for the same domain id, or `None`
    /// if none is registered. With several participants for the same
    /// domain, the implementation returns the first.
    #[must_use]
    pub fn lookup_participant(&self, domain_id: DomainId) -> Option<DomainParticipant> {
        let mut reg = self.participants.lock().ok()?;
        let mut found = None;
        let mut now_empty = false;
        if let Some(vec) = reg.get_mut(&domain_id) {
            // Upgrade weaks; return the first still-live participant and garbage
            // -collect any that the user has already dropped.
            vec.retain(|w| match w.upgrade() {
                Some(inner) => {
                    if found.is_none() {
                        found = Some(DomainParticipant::from_inner(inner));
                    }
                    true
                }
                None => false,
            });
            now_empty = vec.is_empty();
        }
        if now_empty {
            reg.remove(&domain_id);
        }
        found
    }

    /// DDS 1.4 §2.2.2.2.2.2 `delete_participant`: "This operation can only
    /// be invoked if all domain entities belonging to the participant have
    /// already been deleted. Otherwise the error PRECONDITION_NOT_MET is
    /// returned." The participant is deleted then: its operations return
    /// ALREADY_DELETED (§2.2.1.1), and its runtime announces its departure
    /// (DDSI-RTPS 2.5 §8.5.3.3) and stops.
    ///
    /// It deleted the contained entities itself, and the participant went
    /// on working. `DomainParticipant::delete_contained_entities` first
    /// does what it did.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` if the participant still has
    /// publishers, subscribers or topics, or is not registered;
    /// `AlreadyDeleted` if it was deleted.
    pub fn delete_participant(&self, p: &DomainParticipant) -> Result<()> {
        p.inner().entity_state.check_not_deleted()?;
        if p.has_contained_entities() {
            return Err(DdsError::PreconditionNotMet {
                reason: "the participant still has publishers, subscribers or topics",
            });
        }
        let mut reg = self
            .participants
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "factory participants poisoned",
            })?;
        let target_handle = p.instance_handle();
        let did = p.domain_id();
        let mut found = false;
        let mut now_empty = false;
        if let Some(vec) = reg.get_mut(&did) {
            // Remove the target (matched by handle through an upgrade) and GC any
            // already-dropped weaks while we hold the lock.
            vec.retain(|w| match w.upgrade() {
                Some(inner) => {
                    let is_target =
                        DomainParticipant::from_inner(inner).instance_handle() == target_handle;
                    if is_target {
                        found = true;
                    }
                    !is_target
                }
                None => false,
            });
            now_empty = vec.is_empty();
        }
        if now_empty {
            reg.remove(&did);
        }
        drop(reg);
        if !found {
            return Err(DdsError::PreconditionNotMet {
                reason: "participant not registered with this factory",
            });
        }
        p.inner().entity_state.mark_deleted();
        #[cfg(feature = "std")]
        if let Some(rt) = p.runtime() {
            rt.shutdown();
        }
        Ok(())
    }

    /// Spec §2.2.2.2.2.5 `set_default_participant_qos` — default QoS
    /// for participants created from now on with
    /// [`PARTICIPANT_QOS_DEFAULT`]. That value itself resets the default to
    /// "the values that would be used if the set_default_participant_qos
    /// operation had never been called".
    ///
    /// [`PARTICIPANT_QOS_DEFAULT`]: crate::psm_constants::PARTICIPANT_QOS_DEFAULT
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` on lock poisoning.
    pub fn set_default_participant_qos(
        &self,
        qos: impl Into<QosArg<DomainParticipantQos>>,
    ) -> Result<()> {
        let mut current =
            self.default_participant_qos
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "default qos poisoned",
                })?;
        *current = qos.into().resolve(DomainParticipantQos::default);
        Ok(())
    }

    /// The QoS a `create_participant*` call creates with.
    fn resolve(&self, qos: impl Into<QosArg<DomainParticipantQos>>) -> DomainParticipantQos {
        qos.into().resolve(|| self.get_default_participant_qos())
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
    /// `zerodds_security_runtime::SecurityBundle`, then
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
/// [`RuntimeConfig`], and a `zerodds_security_runtime::SecurityBundle`
/// (under the `security` feature) and creates the participant via the
/// factory singleton on [`Self::build`].
#[cfg(feature = "std")]
pub struct ParticipantBuilder {
    domain_id: DomainId,
    qos: QosArg<DomainParticipantQos>,
    config: Option<RuntimeConfig>,
    #[cfg(feature = "security")]
    security: Option<zerodds_security_runtime::SecurityBundle>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for ParticipantBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("ParticipantBuilder");
        d.field("domain_id", &self.domain_id)
            .field("qos", &self.qos)
            .field("config", &self.config);
        #[cfg(feature = "security")]
        d.field("security", &self.security.as_ref().map(|_| "<bundle>"));
        d.finish()
    }
}

#[cfg(feature = "std")]
impl ParticipantBuilder {
    fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            qos: QosArg::Default,
            config: None,
            #[cfg(feature = "security")]
            security: None,
        }
    }

    /// Sets the participant QoS (default: the factory's
    /// [`DomainParticipantFactory::get_default_participant_qos`] at
    /// [`Self::build`]).
    #[must_use]
    pub fn with_qos(mut self, qos: impl Into<QosArg<DomainParticipantQos>>) -> Self {
        self.qos = qos.into();
        self
    }

    /// Sets an explicit [`RuntimeConfig`] in place of the one the standard
    /// layers (environment, system files) would give. A bundle from
    /// `Self::with_security` is applied on top of it, in either order.
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
        self.security = Some(bundle);
        self
    }

    /// Creates the live participant via the factory singleton.
    ///
    /// Without [`Self::with_config`] the configuration is the standard
    /// layers, as [`DomainParticipantFactory::create_participant`] reads
    /// them — also when a security bundle is set.
    ///
    /// # Errors
    /// `DdsError::Configuration` for an invalid configuration,
    /// `DdsError::TransportError` if the UDP sockets do not bind.
    pub fn build(self) -> Result<DomainParticipant> {
        let factory = DomainParticipantFactory::instance();
        let config = match self.config {
            Some(config) => config,
            None => RuntimeConfig::from_standard_paths().map_err(|error| {
                crate::error::DdsError::Configuration {
                    message: error.to_string(),
                }
            })?,
        };
        #[cfg(feature = "security")]
        let config = match &self.security {
            Some(bundle) => config.with_security_bundle(bundle),
            None => config,
        };
        factory.create_participant_with_config(self.domain_id, self.qos, config)
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
    fn dropping_participant_releases_it_from_the_factory() {
        // The factory holds only weak references: once the user drops their last
        // strong handle, the participant is gone (RAII) and `lookup_participant`
        // returns `None`, garbage-collecting the dead weak. (Pre-weak-refs the
        // factory kept a strong clone and this would still return `Some`.)
        let f = DomainParticipantFactory::instance();
        let domain = 9182; // isolated from other tests
        {
            let _p = f.create_participant_offline(domain, DomainParticipantQos::default());
            assert!(
                f.lookup_participant(domain).is_some(),
                "tracked while a strong handle is alive"
            );
        } // _p dropped here
        assert!(
            f.lookup_participant(domain).is_none(),
            "factory must not keep the participant alive after the user drops it"
        );
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

    /// DDS 1.4 §2.2.2.2.2.2: "This operation can only be invoked if all
    /// domain entities belonging to the participant have already been
    /// deleted. Otherwise the error PRECONDITION_NOT_MET is returned." A
    /// participant with a publisher is refused and stays; once the
    /// publisher is gone it is deleted, and its operations return
    /// ALREADY_DELETED. It deleted the publisher itself and went on.
    #[test]
    fn delete_participant_wants_its_entities_deleted_first() {
        let f = DomainParticipantFactory::instance();
        let domain = 94;
        let p = f.create_participant_offline(domain, DomainParticipantQos::default());
        let publisher = p.create_publisher(crate::qos::PublisherQos::default());
        assert!(matches!(
            f.delete_participant(&p),
            Err(DdsError::PreconditionNotMet { .. })
        ));
        assert!(
            f.lookup_participant(domain).is_some(),
            "refused, so it stays"
        );
        p.delete_publisher(&publisher).unwrap();
        f.delete_participant(&p).unwrap();
        assert!(f.lookup_participant(domain).is_none());
        assert!(matches!(
            p.create_topic::<crate::RawBytes>("Gone", crate::qos::TopicQos::default()),
            Err(DdsError::AlreadyDeleted)
        ));
        assert!(matches!(
            f.delete_participant(&p),
            Err(DdsError::AlreadyDeleted)
        ));
    }

    /// DDS 1.4 §2.2.1.1: an operation on a deleted entity returns
    /// ALREADY_DELETED, `create_publisher` and `create_subscriber` among
    /// them.
    #[test]
    fn a_deleted_participant_creates_no_publisher_or_subscriber() {
        let f = DomainParticipantFactory::instance();
        let p = f.create_participant_offline(95, DomainParticipantQos::default());
        f.delete_participant(&p).unwrap();
        assert!(matches!(
            p.try_create_publisher(crate::qos::PublisherQos::default()),
            Err(DdsError::AlreadyDeleted)
        ));
        assert!(matches!(
            p.try_create_subscriber(crate::qos::SubscriberQos::default()),
            Err(DdsError::AlreadyDeleted)
        ));
        // The infallible forms hand back an entity that is deleted.
        let publisher = p.create_publisher(crate::qos::PublisherQos::default());
        let subscriber = p.create_subscriber(crate::qos::SubscriberQos::default());
        let topic_less = p.create_topic::<crate::RawBytes>("Gone", crate::qos::TopicQos::default());
        assert!(matches!(topic_less, Err(DdsError::AlreadyDeleted)));
        assert!(crate::entity::Entity::entity_state(&publisher).is_deleted());
        assert!(crate::entity::Entity::entity_state(&subscriber).is_deleted());
        // An operation on them is ALREADY_DELETED, not a success.
        let other = f.create_participant_offline(96, DomainParticipantQos::default());
        let topic = other
            .create_topic::<crate::RawBytes>("Live", crate::qos::TopicQos::default())
            .unwrap();
        assert!(matches!(
            publisher.create_datawriter(&topic, crate::qos::DataWriterQos::default()),
            Err(DdsError::AlreadyDeleted)
        ));
        assert!(matches!(
            subscriber.create_datareader(&topic, crate::qos::DataReaderQos::default()),
            Err(DdsError::AlreadyDeleted)
        ));
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
