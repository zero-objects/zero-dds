// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncSubscriber — newtype.

use alloc::sync::Arc;

use zerodds_dcps::{
    ContentFilteredTopic, DataReaderQos, DdsType, DdsTypeRow, Result, Subscriber, Topic,
};

use crate::AsyncDataReader;

/// Async wrapper around `Subscriber`.
#[derive(Clone)]
pub struct AsyncSubscriber {
    inner: Arc<Subscriber>,
}

impl AsyncSubscriber {
    pub(crate) fn from_sync(inner: Subscriber) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Creates a DataReader.
    ///
    /// # Errors
    /// Same as `Subscriber::create_datareader`.
    pub fn create_datareader<T: DdsType + Send + Sync + 'static>(
        &self,
        topic: &Topic<T>,
        qos: DataReaderQos,
    ) -> Result<AsyncDataReader<T>> {
        let reader = self.inner.create_datareader::<T>(topic, qos)?;
        Ok(AsyncDataReader::from_sync(reader))
    }

    /// Creates a **content-filtered** DataReader: only samples for which
    /// `filter` returns `true` are delivered (via `take` / `take_stream` /
    /// `data_available_stream`). The async counterpart of
    /// [`zerodds_dcps::DataReader::with_filter`] (DDS 1.4 §2.2.2.5.4
    /// `ContentFilteredTopic`).
    ///
    /// # Errors
    /// Same as `Subscriber::create_datareader`.
    pub fn create_datareader_filtered<T, F>(
        &self,
        topic: &Topic<T>,
        qos: DataReaderQos,
        filter: F,
    ) -> Result<AsyncDataReader<T>>
    where
        T: DdsType + Send + Sync + 'static,
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let reader = self
            .inner
            .create_datareader::<T>(topic, qos)?
            .with_filter(filter);
        Ok(AsyncDataReader::from_sync(reader))
    }

    /// Creates a content-filtered DataReader from a [`ContentFilteredTopic`]
    /// (SQL subset). Only samples for which the filter expression evaluates true
    /// are delivered. The async counterpart of the sync CFT reader path.
    ///
    /// # Errors
    /// Same as `Subscriber::create_datareader`.
    pub fn create_datareader_cft<T: DdsType + Send + Sync + 'static>(
        &self,
        cft: &ContentFilteredTopic<T>,
        qos: DataReaderQos,
    ) -> Result<AsyncDataReader<T>> {
        let cft = cft.clone();
        let reader = self
            .inner
            .create_datareader::<T>(cft.get_related_topic(), qos)?
            .with_filter(move |s: &T| cft.evaluate(&DdsTypeRow::new(s)).unwrap_or(false));
        Ok(AsyncDataReader::from_sync(reader))
    }

    /// Returns the underlying sync variant.
    #[must_use]
    pub fn as_sync(&self) -> &Subscriber {
        &self.inner
    }
}
