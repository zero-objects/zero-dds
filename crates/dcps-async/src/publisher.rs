// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncPublisher — newtype.

use alloc::sync::Arc;

use zerodds_dcps::{DataWriterQos, DdsType, Publisher, Result, Topic};

use crate::AsyncDataWriter;

/// Async wrapper around `Publisher`.
#[derive(Clone)]
pub struct AsyncPublisher {
    inner: Arc<Publisher>,
}

impl AsyncPublisher {
    pub(crate) fn from_sync(inner: Publisher) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Creates a DataWriter.
    ///
    /// # Errors
    /// As `Publisher::create_datawriter`.
    pub fn create_datawriter<T: DdsType + Send + Sync + 'static>(
        &self,
        topic: &Topic<T>,
        qos: DataWriterQos,
    ) -> Result<AsyncDataWriter<T>> {
        let writer = self.inner.create_datawriter::<T>(topic, qos)?;
        Ok(AsyncDataWriter::from_sync(writer))
    }

    /// Returns the underlying sync variant.
    #[must_use]
    pub fn as_sync(&self) -> &Publisher {
        &self.inner
    }
}
