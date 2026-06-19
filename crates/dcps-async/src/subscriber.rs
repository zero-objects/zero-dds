// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncSubscriber — newtype.

use alloc::sync::Arc;

use zerodds_dcps::{DataReaderQos, DdsType, Result, Subscriber, Topic};

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

    /// Returns the underlying sync variant.
    #[must_use]
    pub fn as_sync(&self) -> &Subscriber {
        &self.inner
    }
}
