// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDomainParticipantFactory — singleton wrapper.

use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos, Result};

use crate::AsyncDomainParticipant;

/// Async wrapper around the sync singleton. `instance()` always returns
/// the same singleton; the create_participant methods share state
/// with the sync API.
pub struct AsyncDomainParticipantFactory {
    inner: &'static DomainParticipantFactory,
}

impl AsyncDomainParticipantFactory {
    /// Singleton access.
    #[must_use]
    pub fn instance() -> Self {
        Self {
            inner: DomainParticipantFactory::instance(),
        }
    }

    /// Offline participant (no UDP bind). Spec §1.1.
    #[must_use]
    pub fn create_participant_offline(&self, domain_id: i32) -> AsyncDomainParticipant {
        let p = self
            .inner
            .create_participant_offline(domain_id, DomainParticipantQos::default());
        AsyncDomainParticipant::from_sync(p)
    }

    /// Live participant (UDP + SPDP/SEDP).
    ///
    /// # Errors
    /// As `DomainParticipantFactory::create_participant`.
    pub fn create_participant(&self, domain_id: i32) -> Result<AsyncDomainParticipant> {
        let p = self
            .inner
            .create_participant(domain_id, DomainParticipantQos::default())?;
        Ok(AsyncDomainParticipant::from_sync(p))
    }

    /// Like `create_participant` but with custom QoS.
    ///
    /// # Errors
    /// As `DomainParticipantFactory::create_participant`.
    pub fn create_participant_with_qos(
        &self,
        domain_id: i32,
        qos: DomainParticipantQos,
    ) -> Result<AsyncDomainParticipant> {
        let p = self.inner.create_participant(domain_id, qos)?;
        Ok(AsyncDomainParticipant::from_sync(p))
    }
}

/// zerodds-async-1.0 §4 — Tokio-Glue. With `--features tokio-glue` the factory
/// gains `spawn_in_tokio`, which runs a participant's periodic DDS tick loop on
/// a tokio runtime instead of a dedicated `std::thread`.
#[cfg(feature = "tokio-glue")]
impl AsyncDomainParticipantFactory {
    /// Creates a live participant whose periodic tick loop (SPDP announce,
    /// SEDP/WLP, deadline/lifespan/liveliness) runs as a task on the given
    /// tokio runtime instead of the dedicated `zdds-tick` `std::thread` —
    /// saving one thread per participant. With many participants on one tokio
    /// runtime their tick loops multiplex onto the tokio worker pool. The recv
    /// worker threads are unaffected: they block on socket recv and stay.
    ///
    /// The spawned task observes shutdown via the runtime stop flag and returns
    /// when the participant is dropped — no explicit join needed.
    ///
    /// # Errors
    /// As [`zerodds_dcps::DomainParticipantFactory::create_participant_with_config`].
    pub fn spawn_in_tokio(
        &self,
        domain_id: i32,
        handle: &tokio::runtime::Handle,
    ) -> Result<AsyncDomainParticipant> {
        self.spawn_in_tokio_with_qos(domain_id, DomainParticipantQos::default(), handle)
    }

    /// Like [`Self::spawn_in_tokio`] but with explicit participant QoS.
    ///
    /// # Errors
    /// As [`zerodds_dcps::DomainParticipantFactory::create_participant_with_config`].
    pub fn spawn_in_tokio_with_qos(
        &self,
        domain_id: i32,
        qos: DomainParticipantQos,
        handle: &tokio::runtime::Handle,
    ) -> Result<AsyncDomainParticipant> {
        let config = zerodds_dcps::runtime::RuntimeConfig {
            user_data: qos.user_data.value.clone(),
            external_tick: true,
            ..zerodds_dcps::runtime::RuntimeConfig::default()
        };
        let p = self
            .inner
            .create_participant_with_config(domain_id, qos, config)?;
        // Drive the suppressed tick loop on the tokio runtime. `tick_period`
        // mirrors the internal thread's cadence; the loop self-terminates once
        // the runtime is shutting down.
        if let Some(rt) = p.runtime() {
            let mut driver = rt.tick_driver();
            let period = driver.tick_period();
            handle.spawn(async move {
                while !driver.is_stopped() {
                    driver.tick();
                    tokio::time::sleep(period).await;
                }
            });
        }
        Ok(AsyncDomainParticipant::from_sync(p))
    }
}

impl Default for AsyncDomainParticipantFactory {
    fn default() -> Self {
        Self::instance()
    }
}
