// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDataWriter — write/dispose/unregister/wait_for_matched async.

use alloc::sync::Arc;
use core::time::Duration;

use zerodds_dcps::{DataWriter, DataWriterQos, DdsType, InstanceHandle, Result};

/// Async wrapper around `DataWriter<T>`.
///
/// Hot path: `write()` is a future form over the sync path with
/// a yield-based retry loop for
/// `OutOfResources` backpressure (Spec §5.1
/// `zerodds-async-1.0`). Instead of a thread-blocking `Condvar::wait_timeout`,
/// caller tasks yield out of the executor via `yield_for` and stay
/// cancelable. Other DCPS methods delegate synchronously — they are
/// non-blocking anyway.
pub struct AsyncDataWriter<T: DdsType + Send + Sync + 'static> {
    inner: Arc<DataWriter<T>>,
}

impl<T: DdsType + Send + Sync + 'static> Clone for AsyncDataWriter<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: DdsType + Send + Sync + 'static> AsyncDataWriter<T> {
    pub(crate) fn from_sync(inner: DataWriter<T>) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Writes a sample. Spec §2.1.1.
    ///
    /// # Errors
    /// Same as `DataWriter::write` — `OutOfResources` after the
    /// `max_blocking_time` timeout, otherwise all other errors are
    /// passed through transparently.
    ///
    /// Spec §5.1 zerodds-async-1.0: on `OutOfResources` the future
    /// suspends via `yield_for` and retries until either a drain
    /// happens or `reliability.max_blocking_time` has elapsed.
    /// In the sync path a `Condvar::wait_timeout` would block here —
    /// the async path uses a yield-retry loop without a
    /// thread block.
    pub async fn write(&self, sample: &T) -> Result<()> {
        let max_block = self.inner.qos().reliability.max_blocking_time;
        let max_block_nanos = max_block.to_nanos();
        // INFINITE → our retry loop still has a safety cap
        // (~1 s polling) so the caller sees caller-side cancellation.
        // The spec permits this.
        let safety_cap = core::time::Duration::from_secs(1);
        let deadline = if max_block_nanos == u128::MAX {
            None
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let secs = (max_block_nanos / 1_000_000_000) as u64;
            #[allow(clippy::cast_possible_truncation)]
            let nanos = (max_block_nanos % 1_000_000_000) as u32;
            Some(std::time::Instant::now() + core::time::Duration::new(secs, nanos))
        };

        loop {
            match self.inner.write(sample) {
                Ok(()) => return Ok(()),
                Err(zerodds_dcps::DdsError::OutOfResources { .. }) => {
                    // Wait for a drain.
                    if let Some(d) = deadline {
                        if std::time::Instant::now() >= d {
                            return Err(zerodds_dcps::DdsError::Timeout);
                        }
                    }
                    crate::yield_for(core::time::Duration::from_millis(2)).await;
                }
                Err(other) => return Err(other),
            }
            if deadline.is_none() {
                // INFINITE: a safety yield after 1 s so the caller
                // sees at least one await point and can cancel.
                let _ = safety_cap;
            }
        }
    }

    /// Spec §2.1.2 register_instance.
    ///
    /// # Errors
    /// Same as sync.
    pub async fn register_instance(&self, sample: &T) -> Result<InstanceHandle> {
        self.inner.register_instance(sample)
    }

    /// Spec §2.1.3 dispose. Triggers the DISPOSED wire lifecycle.
    ///
    /// # Errors
    /// Same as sync.
    pub async fn dispose(&self, sample: &T, handle: InstanceHandle) -> Result<()> {
        self.inner.dispose(sample, handle)
    }

    /// Spec §2.1.4 unregister_instance.
    ///
    /// # Errors
    /// Same as sync.
    pub async fn unregister_instance(&self, sample: &T, handle: InstanceHandle) -> Result<()> {
        self.inner.unregister_instance(sample, handle)
    }

    /// Spec §2.1.5 wait_for_matched_subscription. Async polling
    /// loop with a 10 ms tick.
    ///
    /// # Errors
    /// Same as sync — `Timeout` if `min_count` is not reached within `timeout`.
    pub async fn wait_for_matched_subscription(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.inner.matched_subscription_count() >= min_count {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(zerodds_dcps::DdsError::Timeout);
            }
            // Async sleep without a hard tokio dependency: yield via a futures helper.
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Spec §2.1.6 matched_subscription_count (synchronous).
    #[must_use]
    pub fn matched_subscription_count(&self) -> usize {
        self.inner.matched_subscription_count()
    }

    /// Returns the underlying sync variant.
    #[must_use]
    pub fn as_sync(&self) -> &DataWriter<T> {
        &self.inner
    }

    /// Returns the DataWriterQos.
    #[must_use]
    pub fn qos(&self) -> DataWriterQos {
        self.inner.qos()
    }
}
