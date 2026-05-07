// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDomainParticipant — newtype um Sync-Variante.

use zerodds_dcps::{
    DdsType, DomainParticipant, PublisherQos, Result, SubscriberQos, Topic, TopicQos,
};

use crate::{AsyncPublisher, AsyncSubscriber};

/// Async-Wrapper um `DomainParticipant`. Topics, Pubs und Subs werden
/// ueber die Sync-API erzeugt; nur der Newtype wechselt das Async-API
/// frei.
#[derive(Clone)]
pub struct AsyncDomainParticipant {
    inner: DomainParticipant,
}

impl AsyncDomainParticipant {
    pub(crate) fn from_sync(inner: DomainParticipant) -> Self {
        Self { inner }
    }

    /// Domain-ID.
    #[must_use]
    pub fn domain_id(&self) -> i32 {
        self.inner.domain_id()
    }

    /// Erstellt ein Topic (ist data-only — keine async-Operation).
    ///
    /// # Errors
    /// Wie `DomainParticipant::create_topic`.
    pub fn create_topic<T: DdsType>(&self, name: &str, qos: TopicQos) -> Result<Topic<T>> {
        self.inner.create_topic::<T>(name, qos)
    }

    /// Erstellt einen Publisher.
    #[must_use]
    pub fn create_publisher(&self, qos: PublisherQos) -> AsyncPublisher {
        AsyncPublisher::from_sync(self.inner.create_publisher(qos))
    }

    /// Erstellt einen Subscriber.
    #[must_use]
    pub fn create_subscriber(&self, qos: SubscriberQos) -> AsyncSubscriber {
        AsyncSubscriber::from_sync(self.inner.create_subscriber(qos))
    }

    /// Liefert die zugrundeliegende sync-Variante.
    #[must_use]
    pub fn as_sync(&self) -> &DomainParticipant {
        &self.inner
    }
}
