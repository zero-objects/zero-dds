// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AsyncDataReader + SampleStream.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use futures_core::Stream;
use zerodds_dcps::{DataReader, DataReaderQos, DdsType, Result};

/// Async-Wrapper um `DataReader<T>`.
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
    /// Live-Mode (Reader hat `runtime_handle()`): der Stream
    /// registriert sich mit `register_user_reader_waker` an der
    /// Runtime; `cx.waker()` wird beim naechsten Sample-Zufluss
    /// gefeuert (kein Polling). Offline-Mode: detached-Thread-Sleep
    /// als Polling-Fallback (Spec §3.3).
    #[must_use]
    pub fn take_stream(&self) -> SampleStream<T> {
        SampleStream {
            reader: Arc::clone(&self.inner),
            buffered: Vec::new(),
            poll_interval: Duration::from_millis(5),
            sleep_until: None,
        }
    }

    /// Spec §2.2.2 take(timeout). Resolves Ok mit `Vec<T>` wenn Samples
    /// verfuegbar sind oder Timeout abgelaufen ist (dann leerer Vec
    /// statt Err — analog `take()` sync).
    ///
    /// # Errors
    /// Wire-/Decode-Fehler wie sync.
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

    /// Spec §2.2.3 wait_for_matched_publication.
    ///
    /// # Errors
    /// `Timeout` wenn `min_count` nicht erreicht.
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

    /// Spec §2.2.4 matched_publication_count (synchron).
    #[must_use]
    pub fn matched_publication_count(&self) -> usize {
        self.inner.matched_publication_count()
    }

    /// Liefert die zugrundeliegende sync-Variante.
    #[must_use]
    pub fn as_sync(&self) -> &DataReader<T> {
        &self.inner
    }

    /// Liefert die DataReaderQos.
    #[must_use]
    pub fn qos(&self) -> DataReaderQos {
        self.inner.qos()
    }

    /// Spec §6.1 `data_available_stream` — `Stream<Item = ()>`.
    /// Emittiert pro Sample-Zufluss ein `()`-Event; Caller ruft danach
    /// `take()` / iteriert ueber `take_stream()` um die Samples zu
    /// holen. Konsumiert keine Samples (im Gegensatz zu
    /// [`Self::take_stream`]).
    ///
    /// Live-Mode: registriert sich am Reader-Slot der Runtime
    /// (`register_user_reader_waker`) — Wake erfolgt beim
    /// `sample_tx.send` durch die Runtime, kein Polling. Offline-
    /// Mode: detached-Thread-Sleep als Polling-Fallback.
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
    /// `Stream<Item = SubscriptionMatchedStatus>`. Emittiert den
    /// vollen Reader-side Match-Status (DDS 1.4 §2.2.4.1
    /// SUBSCRIPTION_MATCHED) jedes Mal wenn sich der Count an
    /// matched Publications (Writers) aendert. Felder:
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

/// Stream uber Reader-Samples. Endet wenn der Reader gedroppt wird.
pub struct SampleStream<T: DdsType + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    buffered: Vec<T>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for SampleStream<T> {}

/// Stream der "data available"-Events. Yieldet `()` jedes Mal wenn
/// neue Samples im Reader sind. Konsumiert keine Samples — Caller
/// muss `take()` oder `take_stream` separat aufrufen.
pub struct DataAvailableStream<T: DdsType + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    poll_interval: Duration,
    sleep_until: Option<std::time::Instant>,
    /// Sample-Anzahl bei der letzten Emission. Steigender Wert =
    /// neue Samples → emit `()`. Lese-Quelle ist der nicht-
    /// konsumierende `read()`-Pfad.
    last_seen_count: usize,
}

impl<T: DdsType + Send + Sync + 'static> Unpin for DataAvailableStream<T> {}

impl<T: DdsType + Send + Sync + 'static> Stream for DataAvailableStream<T> {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
        let this = self.get_mut();

        // Pruefen ob neue Samples eingetroffen sind — `read()` ist
        // non-consuming (DDS 1.4 §2.2.2.5.3.5), die Samples bleiben
        // im Reader-Cache und koennen vom Caller danach via `take()`
        // konsumiert werden.
        let cur_count = match this.reader.read() {
            Ok(samples) => samples.len(),
            Err(_) => {
                // Reader-State korrupt → Stream-Ende.
                return Poll::Ready(None);
            }
        };
        if cur_count > this.last_seen_count {
            this.last_seen_count = cur_count;
            return Poll::Ready(Some(()));
        }

        // Kein neuer Sample. Live-Mode: nativer Reader-Slot-Waker;
        // bei jedem `sample_tx.send` weckt die Runtime uns auf.
        if let Some((rt, eid)) = this.reader.runtime_handle() {
            rt.register_user_reader_waker(eid, Some(cx.waker().clone()));
            return Poll::Pending;
        }

        // Offline-Mode: Polling-Fallback ueber detached-Thread-Sleep.
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

/// Stream der publication-matched-Counts. Yieldet jeden neuen Count.
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
            // Delta-Berechnung: bei initialem Aufruf
            // (last_count == usize::MAX) ist der Delta == now_count.
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

        // Pufferierte Samples zuerst.
        if !this.buffered.is_empty() {
            return Poll::Ready(Some(this.buffered.remove(0)));
        }

        // Sleep-Pfad: wenn wir mid-pause sind, warten.
        if let Some(deadline) = this.sleep_until {
            if std::time::Instant::now() < deadline {
                // Re-schedule — Caller-Runtime weckt den Future.
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

        // Take aus dem Reader.
        match this.reader.take() {
            Ok(mut samples) if !samples.is_empty() => {
                let first = samples.remove(0);
                this.buffered = samples;
                Poll::Ready(Some(first))
            }
            Ok(_) => {
                // Kein Sample — Spec §3.3: Waker beim Reader-Slot
                // registrieren. Bei `sample_tx.send` weckt die
                // Runtime uns nativ. Live-Mode-Pfad.
                if let Some((rt, eid)) = this.reader.runtime_handle() {
                    rt.register_user_reader_waker(eid, Some(cx.waker().clone()));
                    return Poll::Pending;
                }
                // Offline-Mode: detached-thread-Sleep als Polling-
                // Fallback (kein Reader-Slot, kein Sample-Zufluss).
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
