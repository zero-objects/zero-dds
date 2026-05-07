// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDomainParticipantFactory — Singleton-Wrapper.

use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos, Result};

use crate::AsyncDomainParticipant;

/// Async-Wrapper um den Sync-Singleton. `instance()` liefert immer den
/// gleichen Singleton; create_participant-Methoden teilen den State
/// mit der sync-API.
pub struct AsyncDomainParticipantFactory {
    inner: &'static DomainParticipantFactory,
}

impl AsyncDomainParticipantFactory {
    /// Singleton-Zugriff.
    #[must_use]
    pub fn instance() -> Self {
        Self {
            inner: DomainParticipantFactory::instance(),
        }
    }

    /// Offline-Participant (kein UDP-Bind). Spec §1.1.
    #[must_use]
    pub fn create_participant_offline(&self, domain_id: i32) -> AsyncDomainParticipant {
        let p = self
            .inner
            .create_participant_offline(domain_id, DomainParticipantQos::default());
        AsyncDomainParticipant::from_sync(p)
    }

    /// Live-Participant (UDP + SPDP/SEDP).
    ///
    /// # Errors
    /// Wie `DomainParticipantFactory::create_participant`.
    pub fn create_participant(&self, domain_id: i32) -> Result<AsyncDomainParticipant> {
        let p = self
            .inner
            .create_participant(domain_id, DomainParticipantQos::default())?;
        Ok(AsyncDomainParticipant::from_sync(p))
    }

    /// Like `create_participant` aber mit eigener QoS.
    ///
    /// # Errors
    /// Wie `DomainParticipantFactory::create_participant`.
    pub fn create_participant_with_qos(
        &self,
        domain_id: i32,
        qos: DomainParticipantQos,
    ) -> Result<AsyncDomainParticipant> {
        let p = self.inner.create_participant(domain_id, qos)?;
        Ok(AsyncDomainParticipant::from_sync(p))
    }
}

impl Default for AsyncDomainParticipantFactory {
    fn default() -> Self {
        Self::instance()
    }
}
