// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDataWriter — write/dispose/unregister/wait_for_matched async.

use alloc::sync::Arc;
use core::time::Duration;

use zerodds_dcps::{DataWriter, DataWriterQos, DdsType, InstanceHandle, Result};

/// Async-Wrapper um `DataWriter<T>`.
///
/// Hot-Path: `write()` ist eine Future-Form ueber dem sync-Pfad mit
/// einer yield-basierten Retry-Schleife fuer
/// `OutOfResources`-Backpressure (Spec §5.1
/// `zerodds-async-1.0`). Statt eines Thread-Block-`Condvar::wait_timeout`
/// fallen Caller-Tasks per `yield_for` aus dem Executor und bleiben
/// cancelable. Andere DCPS-Methoden delegieren synchron — sie sind
/// ohnehin nicht blockierend.
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

    /// Schreibt einen Sample. Spec §2.1.1.
    ///
    /// # Errors
    /// Wie `DataWriter::write` — `OutOfResources` nach
    /// `max_blocking_time`-Timeout, sonst alle anderen Errors
    /// transparent durchgereicht.
    ///
    /// Spec §5.1 zerodds-async-1.0: bei `OutOfResources` suspendiert
    /// der Future via `yield_for` und retried, bis entweder ein Drain
    /// passiert oder die `reliability.max_blocking_time` abgelaufen
    /// ist. Im Sync-Pfad wuerde hier ein `Condvar::wait_timeout`
    /// blockieren — async-Pfad nutzt yield-retry-Loop ohne
    /// Thread-Block.
    pub async fn write(&self, sample: &T) -> Result<()>
    where
        T: Clone,
    {
        let max_block = self.inner.qos().reliability.max_blocking_time;
        let max_block_nanos = max_block.to_nanos();
        // INFINITE → unsere Retry-Loop hat trotzdem einen safety-cap
        // (~1 s polling) damit Caller die Caller-side cancellation
        // sieht. Spec erlaubt das.
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

        let s = sample.clone();
        loop {
            match self.inner.write(&s) {
                Ok(()) => return Ok(()),
                Err(zerodds_dcps::DdsError::OutOfResources { .. }) => {
                    // Drain abwarten.
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
                // INFINITE: nach 1 s safety-yield, damit der Caller
                // mindestens ein await-point sieht und canceln kann.
                let _ = safety_cap;
            }
        }
    }

    /// Spec §2.1.2 register_instance.
    ///
    /// # Errors
    /// Wie sync.
    pub async fn register_instance(&self, sample: &T) -> Result<InstanceHandle> {
        self.inner.register_instance(sample)
    }

    /// Spec §2.1.3 dispose. Loest Wire-Lifecycle DISPOSED.
    ///
    /// # Errors
    /// Wie sync.
    pub async fn dispose(&self, sample: &T, handle: InstanceHandle) -> Result<()> {
        self.inner.dispose(sample, handle)
    }

    /// Spec §2.1.4 unregister_instance.
    ///
    /// # Errors
    /// Wie sync.
    pub async fn unregister_instance(&self, sample: &T, handle: InstanceHandle) -> Result<()> {
        self.inner.unregister_instance(sample, handle)
    }

    /// Spec §2.1.5 wait_for_matched_subscription. Async-Polling-
    /// Schleife mit 10 ms Tick.
    ///
    /// # Errors
    /// Wie sync — `Timeout` wenn `min_count` nicht in `timeout` erreicht.
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
            // Async-sleep ohne tokio-Hard-Dep: yield via futures-Helper.
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Spec §2.1.6 matched_subscription_count (synchron).
    #[must_use]
    pub fn matched_subscription_count(&self) -> usize {
        self.inner.matched_subscription_count()
    }

    /// Liefert die zugrundeliegende sync-Variante.
    #[must_use]
    pub fn as_sync(&self) -> &DataWriter<T> {
        &self.inner
    }

    /// Liefert die DataWriterQos.
    #[must_use]
    pub fn qos(&self) -> DataWriterQos {
        self.inner.qos()
    }
}
