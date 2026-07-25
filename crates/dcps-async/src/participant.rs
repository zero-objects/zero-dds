// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDomainParticipant — newtype around the sync variant.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_dcps::{
    ContentFilteredTopic, DdsType, DomainParticipant, PublisherQos, Result, SubscriberQos, Topic,
    TopicQos,
};

use crate::{AsyncPublisher, AsyncSubscriber};

/// Async wrapper around `DomainParticipant`. Topics, pubs and subs are
/// created via the sync API; only the newtype freely switches to the
/// async API.
#[derive(Clone)]
pub struct AsyncDomainParticipant {
    inner: DomainParticipant,
}

impl AsyncDomainParticipant {
    pub(crate) fn from_sync(inner: DomainParticipant) -> Self {
        Self { inner }
    }

    /// Domain ID.
    #[must_use]
    pub fn domain_id(&self) -> i32 {
        self.inner.domain_id()
    }

    /// Creates a topic (data-only — no async operation).
    ///
    /// # Errors
    /// As `DomainParticipant::create_topic`.
    pub fn create_topic<T: DdsType>(&self, name: &str, qos: TopicQos) -> Result<Topic<T>> {
        self.inner.create_topic::<T>(name, qos)
    }

    /// Creates a content-filtered topic (SQL subset, DDS 1.4 §2.2.2.2.1.13). Use
    /// it with [`crate::AsyncSubscriber::create_datareader_cft`] for a filtered
    /// async reader. `filter_parameters` replace `%0`, `%1`, ... in the
    /// expression.
    ///
    /// # Errors
    /// As `DomainParticipant::create_contentfilteredtopic` (empty name /
    /// expression, parse error, missing `%N` parameter).
    pub fn create_contentfilteredtopic<T: DdsType>(
        &self,
        name: &str,
        related_topic: &Topic<T>,
        filter_expression: &str,
        filter_parameters: Vec<String>,
    ) -> Result<ContentFilteredTopic<T>> {
        self.inner.create_contentfilteredtopic::<T>(
            name,
            related_topic,
            filter_expression,
            filter_parameters,
        )
    }

    /// Creates a publisher.
    #[must_use]
    pub fn create_publisher(&self, qos: PublisherQos) -> AsyncPublisher {
        AsyncPublisher::from_sync(self.inner.create_publisher(qos))
    }

    /// Creates a subscriber.
    #[must_use]
    pub fn create_subscriber(&self, qos: SubscriberQos) -> AsyncSubscriber {
        AsyncSubscriber::from_sync(self.inner.create_subscriber(qos))
    }

    /// Returns the underlying sync variant.
    #[must_use]
    pub fn as_sync(&self) -> &DomainParticipant {
        &self.inner
    }
}
