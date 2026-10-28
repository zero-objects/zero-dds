// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDataWriter — write/dispose/unregister/wait_for_matched async.

use alloc::sync::Arc;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use futures_core::Stream;
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
        // Uncontended path — spec §9.1 budgets no more than 5 % over the
        // sync write, so nothing may happen here beyond the sync call
        // itself. In particular the reliability budget is *not* read: on
        // this path it is never used, and `DataWriter::qos()` clones the
        // whole QoS set, which alone cost about 13 % per write. The retry
        // loop is a future of its own, entered only on `OutOfResources`:
        // inline, its state made every write's future larger, 10 ns per
        // write against 3.5 ns with it apart (an offline write on the Linux
        // test host, release build).
        match self.inner.write(sample) {
            Err(zerodds_dcps::DdsError::OutOfResources { .. }) => {
                self.write_under_backpressure(sample).await
            }
            done => done,
        }
    }

    /// Spec §5.1 zerodds-async-1.0: the retry loop of [`Self::write`]
    /// after a write met `OutOfResources`.
    async fn write_under_backpressure(&self, sample: &T) -> Result<()> {
        // Now the blocking budget matters, so it is read once, here.
        let max_block_nanos = self.inner.qos().reliability.max_blocking_time.to_nanos();
        // INFINITE → no deadline; the future then retries until the
        // caller drops it.
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
            if let Some(d) = deadline {
                if std::time::Instant::now() >= d {
                    return Err(zerodds_dcps::DdsError::Timeout);
                }
            }
            // Suspend rather than block the caller thread, then retry.
            crate::yield_for(core::time::Duration::from_millis(2)).await;
            match self.inner.write(sample) {
                Ok(()) => return Ok(()),
                Err(zerodds_dcps::DdsError::OutOfResources { .. }) => {}
                Err(other) => return Err(other),
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

    /// Spec §6.2 `publication_matched_stream` —
    /// `Stream<Item = PublicationMatchedStatus>`. The writer-side
    /// counterpart of `DataWriterListener::on_publication_matched`
    /// (DDS 1.4 §2.2.4.1 PUBLICATION_MATCHED): one item every time the
    /// number of matched subscriptions changes, carrying `total_count`,
    /// `total_count_change`, `current_count` and `current_count_change`.
    #[must_use]
    pub fn publication_matched_stream(&self) -> PublicationMatchedStream<T> {
        PublicationMatchedStream {
            writer: Arc::clone(&self.inner),
            poll_interval: Duration::from_millis(20),
            sleep_until: None,
            last_count: usize::MAX,
        }
    }
}

/// Stream of [`PublicationMatchedStatus`] events on a writer. Yields one
/// item per change of the matched-subscription count. Spec §6.2.
///
/// [`PublicationMatchedStatus`]: zerodds_dcps::status::PublicationMatchedStatus
pub struct PublicationMatchedStream<T: DdsType + Send + Sync + 'static> {
    writer: Arc<DataWriter<T>>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
    last_count: usize,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for PublicationMatchedStream<T> {}

impl<T: DdsType + Send + Sync + 'static> Stream for PublicationMatchedStream<T> {
    type Item = zerodds_dcps::status::PublicationMatchedStatus;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(deadline) = this.sleep_until {
            if std::time::Instant::now() < deadline {
                crate::reader::schedule_wake(cx, deadline);
                return Poll::Pending;
            }
            this.sleep_until = None;
        }

        let now_count = this.writer.matched_subscription_count();
        if now_count == this.last_count {
            this.sleep_until = Some(std::time::Instant::now() + this.poll_interval);
            crate::reader::schedule_wake_in(cx, this.poll_interval);
            return Poll::Pending;
        }

        // On the first poll (`last_count == usize::MAX`) the delta is the
        // current count itself — the same convention the reader-side
        // stream uses.
        let prev_known = if this.last_count == usize::MAX {
            0
        } else {
            this.last_count
        };
        this.last_count = now_count;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let now_i = now_count as i32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let prev_i = prev_known as i32;
        let delta = now_i - prev_i;
        Poll::Ready(Some(zerodds_dcps::status::PublicationMatchedStatus {
            total_count: now_i.max(prev_i),
            total_count_change: delta.max(0),
            current_count: now_i,
            current_count_change: delta,
            last_subscription_handle: zerodds_dcps::HANDLE_NIL,
        }))
    }
}
