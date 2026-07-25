// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDataReader + SampleStream.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use futures_core::Stream;
use zerodds_dcps::{DataReader, DataReaderQos, DdsType, Result, Sample};

/// Async wrapper around `DataReader<T>`.
pub struct AsyncDataReader<T: DdsType + Send + Sync + 'static> {
    inner: Arc<DataReader<T>>,
}

impl<T: DdsType + Send + Sync + 'static> Clone for AsyncDataReader<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: DdsType + Send + Sync + 'static> AsyncDataReader<T> {
    pub(crate) fn from_sync(inner: DataReader<T>) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Spec §2.2.1 `take_stream` — `Stream<Item = T>`.
    ///
    /// Live mode (the reader has a `runtime_handle()`): the stream
    /// registers itself with `register_user_reader_waker` on the
    /// runtime; `cx.waker()` is fired on the next sample arrival
    /// (no polling). Offline mode: detached-thread sleep as a
    /// polling fallback (Spec §3.3).
    #[must_use]
    pub fn take_stream(&self) -> SampleStream<T> {
        SampleStream {
            reader: Arc::clone(&self.inner),
            buffered: Vec::new(),
            poll_interval: Duration::from_millis(5),
            sleep_until: None,
        }
    }

    /// Spec §2.2.2 take(timeout). Resolves to Ok with a `Vec<T>` when
    /// samples are available or the timeout has elapsed (in which case
    /// an empty Vec rather than Err — same as sync `take()`).
    ///
    /// # Errors
    /// Wire/decode errors, same as sync.
    pub async fn take(&self, timeout: Duration) -> Result<Vec<T>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let samples = self.inner.take()?;
            if !samples.is_empty() {
                return Ok(samples);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(Vec::new());
            }
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Non-consuming counterpart of [`Self::take`]: resolves with the samples
    /// currently in the reader (marking them `READ`, spec §2.2.2.5.3.4) without
    /// removing them, or an empty `Vec` on timeout.
    ///
    /// # Errors
    /// Wire/decode errors, same as sync `read`.
    pub async fn read(&self, timeout: Duration) -> Result<Vec<T>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let samples = self.inner.read()?;
            if !samples.is_empty() {
                return Ok(samples);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(Vec::new());
            }
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Like [`Self::take`] but returns each sample with its [`SampleInfo`]
    /// (`Sample<T>`, spec §2.2 — source timestamp, instance/view/sample state,
    /// `valid_data`, generation counts).
    ///
    /// [`SampleInfo`]: zerodds_dcps::SampleInfo
    ///
    /// # Errors
    /// Wire/decode errors, same as sync `take_with_info`.
    pub async fn take_with_info(&self, timeout: Duration) -> Result<Vec<Sample<T>>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let samples = self.inner.take_with_info()?;
            if !samples.is_empty() {
                return Ok(samples);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(Vec::new());
            }
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Non-consuming [`Self::take_with_info`]: samples with full `SampleInfo`,
    /// left in the reader (marked `READ`).
    ///
    /// # Errors
    /// Wire/decode errors, same as sync `read_with_info`.
    pub async fn read_with_info(&self, timeout: Duration) -> Result<Vec<Sample<T>>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let samples = self.inner.read_with_info()?;
            if !samples.is_empty() {
                return Ok(samples);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(Vec::new());
            }
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Like [`Self::take_stream`] but yields `Sample<T>` (data + `SampleInfo`) —
    /// the `Stream<Item = Sample<T>>` of spec §2.2.
    #[must_use]
    pub fn take_stream_with_info(&self) -> SampleInfoStream<T> {
        SampleInfoStream {
            reader: Arc::clone(&self.inner),
            buffered: Vec::new(),
            poll_interval: Duration::from_millis(5),
            sleep_until: None,
        }
    }

    /// Spec §2.2.3 wait_for_matched_publication.
    ///
    /// # Errors
    /// `Timeout` if `min_count` is not reached.
    pub async fn wait_for_matched_publication(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.inner.matched_publication_count() >= min_count {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(zerodds_dcps::DdsError::Timeout);
            }
            crate::yield_for(Duration::from_millis(10)).await;
        }
    }

    /// Spec §2.2.4 matched_publication_count (synchronous).
    #[must_use]
    pub fn matched_publication_count(&self) -> usize {
        self.inner.matched_publication_count()
    }

    /// Returns the underlying sync variant.
    #[must_use]
    pub fn as_sync(&self) -> &DataReader<T> {
        &self.inner
    }

    /// Returns the DataReaderQos.
    #[must_use]
    pub fn qos(&self) -> DataReaderQos {
        self.inner.qos()
    }

    /// Spec §6.1 `data_available_stream` — `Stream<Item = ()>`.
    /// Emits a `()` event per sample arrival; the caller then calls
    /// `take()` / iterates over `take_stream()` to fetch the samples.
    /// Does not consume samples (in contrast to
    /// [`Self::take_stream`]).
    ///
    /// Live mode: registers itself on the runtime's reader slot
    /// (`register_user_reader_waker`) — the wake happens on
    /// `sample_tx.send` by the runtime, no polling. Offline
    /// mode: detached-thread sleep as a polling fallback.
    #[must_use]
    pub fn data_available_stream(&self) -> DataAvailableStream<T> {
        DataAvailableStream {
            reader: Arc::clone(&self.inner),
            poll_interval: Duration::from_millis(10),
            sleep_until: None,
            last_seen_count: 0,
        }
    }

    /// Spec §6.2 `publication_matched_stream` —
    /// `Stream<Item = SubscriptionMatchedStatus>`. Emits the
    /// full reader-side match status (DDS 1.4 §2.2.4.1
    /// SUBSCRIPTION_MATCHED) every time the count of
    /// matched publications (writers) changes. Fields:
    /// `total_count` (cumulative), `total_count_change` (delta),
    /// `current_count` (currently matched), `current_count_change`
    /// (signed delta), `last_publication_handle`.
    #[must_use]
    pub fn publication_matched_stream(&self) -> PublicationMatchedStream<T> {
        PublicationMatchedStream {
            reader: Arc::clone(&self.inner),
            poll_interval: Duration::from_millis(20),
            sleep_until: None,
            last_count: usize::MAX,
        }
    }
}

/// Stream over reader samples. Ends when the reader is dropped.
pub struct SampleStream<T: DdsType + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    buffered: Vec<T>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for SampleStream<T> {}

/// Stream of "data available" events. Yields `()` every time there
/// are new samples in the reader. Does not consume samples — the
/// caller must call `take()` or `take_stream` separately.
pub struct DataAvailableStream<T: DdsType + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
    /// Sample count at the last emission. A rising value =
    /// new samples → emit `()`. The read source is the non-
    /// consuming `read()` path.
    last_seen_count: usize,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for DataAvailableStream<T> {}

impl<T: DdsType + Send + Sync + 'static> Stream for DataAvailableStream<T> {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
        let this = self.get_mut();

        // Check whether new samples have arrived — `read()` is
        // non-consuming (DDS 1.4 §2.2.2.5.3.5); the samples stay
        // in the reader cache and can subsequently be consumed by
        // the caller via `take()`.
        let cur_count = match this.reader.read() {
            Ok(samples) => samples.len(),
            Err(_) => {
                // Reader state corrupt → end of stream.
                return Poll::Ready(None);
            }
        };
        if cur_count > this.last_seen_count {
            this.last_seen_count = cur_count;
            return Poll::Ready(Some(()));
        }

        // No new sample. Live mode: native reader-slot waker;
        // on every `sample_tx.send` the runtime wakes us up.
        if let Some((rt, eid)) = this.reader.runtime_handle() {
            rt.register_user_reader_waker(eid, Some(cx.waker().clone()));
            return Poll::Pending;
        }

        // Offline mode: polling fallback via detached-thread sleep.
        if let Some(deadline) = this.sleep_until {
            if std::time::Instant::now() < deadline {
                schedule_wake(cx, deadline);
                return Poll::Pending;
            }
            this.sleep_until = None;
        }
        this.sleep_until = Some(std::time::Instant::now() + this.poll_interval);
        schedule_wake_in(cx, this.poll_interval);
        Poll::Pending
    }
}

/// Stream of publication-matched counts. Yields every new count.
pub struct PublicationMatchedStream<T: DdsType + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
    last_count: usize,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for PublicationMatchedStream<T> {}

impl<T: DdsType + Send + Sync + 'static> Stream for PublicationMatchedStream<T> {
    type Item = zerodds_dcps::status::SubscriptionMatchedStatus;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(deadline) = this.sleep_until {
            if std::time::Instant::now() < deadline {
                schedule_wake(cx, deadline);
                return Poll::Pending;
            }
            this.sleep_until = None;
        }

        let now_count = this.reader.matched_publication_count();
        if now_count != this.last_count {
            // Delta computation: on the initial call
            // (last_count == usize::MAX) the delta == now_count.
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
            let status = zerodds_dcps::status::SubscriptionMatchedStatus {
                total_count: now_i.max(prev_i),
                total_count_change: delta.max(0),
                current_count: now_i,
                current_count_change: delta,
                last_publication_handle: zerodds_dcps::HANDLE_NIL,
            };
            Poll::Ready(Some(status))
        } else {
            this.sleep_until = Some(std::time::Instant::now() + this.poll_interval);
            schedule_wake_in(cx, this.poll_interval);
            Poll::Pending
        }
    }
}

fn schedule_wake(cx: &mut Context<'_>, deadline: std::time::Instant) {
    let waker = cx.waker().clone();
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    std::thread::spawn(move || {
        std::thread::sleep(remaining);
        waker.wake();
    });
}

fn schedule_wake_in(cx: &mut Context<'_>, interval: Duration) {
    let waker = cx.waker().clone();
    std::thread::spawn(move || {
        std::thread::sleep(interval);
        waker.wake();
    });
}

impl<T: DdsType + Send + Sync + 'static> Stream for SampleStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let this = self.get_mut();

        // Buffered samples first.
        if !this.buffered.is_empty() {
            return Poll::Ready(Some(this.buffered.remove(0)));
        }

        // Sleep path: if we are mid-pause, wait.
        if let Some(deadline) = this.sleep_until {
            if std::time::Instant::now() < deadline {
                // Re-schedule — the caller runtime wakes the future.
                let waker = cx.waker().clone();
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                std::thread::spawn(move || {
                    std::thread::sleep(remaining);
                    waker.wake();
                });
                return Poll::Pending;
            }
            this.sleep_until = None;
        }

        // Take from the reader.
        match this.reader.take() {
            Ok(mut samples) if !samples.is_empty() => {
                let first = samples.remove(0);
                this.buffered = samples;
                Poll::Ready(Some(first))
            }
            Ok(_) => {
                // No sample — Spec §3.3: register the waker on the
                // reader slot. On `sample_tx.send` the runtime wakes
                // us natively. Live-mode path.
                if let Some((rt, eid)) = this.reader.runtime_handle() {
                    rt.register_user_reader_waker(eid, Some(cx.waker().clone()));
                    return Poll::Pending;
                }
                // Offline mode: detached-thread sleep as a polling
                // fallback (no reader slot, no sample arrival).
                this.sleep_until = Some(std::time::Instant::now() + this.poll_interval);
                let waker = cx.waker().clone();
                let interval = this.poll_interval;
                std::thread::spawn(move || {
                    std::thread::sleep(interval);
                    waker.wake();
                });
                Poll::Pending
            }
            Err(_) => Poll::Ready(None),
        }
    }
}

/// Stream over reader samples **with** their [`SampleInfo`] (`Sample<T>`). Like
/// [`SampleStream`] but preserves the per-sample metadata. Ends when the reader
/// is dropped.
///
/// [`SampleInfo`]: zerodds_dcps::SampleInfo
pub struct SampleInfoStream<T: DdsType + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    buffered: Vec<Sample<T>>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for SampleInfoStream<T> {}

impl<T: DdsType + Send + Sync + 'static> Stream for SampleInfoStream<T> {
    type Item = Sample<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Sample<T>>> {
        let this = self.get_mut();

        if !this.buffered.is_empty() {
            return Poll::Ready(Some(this.buffered.remove(0)));
        }

        if let Some(deadline) = this.sleep_until {
            if std::time::Instant::now() < deadline {
                schedule_wake(cx, deadline);
                return Poll::Pending;
            }
            this.sleep_until = None;
        }

        match this.reader.take_with_info() {
            Ok(mut samples) if !samples.is_empty() => {
                let first = samples.remove(0);
                this.buffered = samples;
                Poll::Ready(Some(first))
            }
            Ok(_) => {
                if let Some((rt, eid)) = this.reader.runtime_handle() {
                    rt.register_user_reader_waker(eid, Some(cx.waker().clone()));
                    return Poll::Pending;
                }
                this.sleep_until = Some(std::time::Instant::now() + this.poll_interval);
                schedule_wake_in(cx, this.poll_interval);
                Poll::Pending
            }
            Err(_) => Poll::Ready(None),
        }
    }
}
