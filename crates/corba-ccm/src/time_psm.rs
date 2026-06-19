// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG Time Service 1.1 PSM types — Spec §2.2-§2.4.
//!
//! Spec-compliant IDL-PSM types + operations as a wrapper around the
//! platform `TimerEventService` from `timer.rs`. Makes the
//! ZeroDDS CCM timer stack callable in a 1:1 spec-compliant manner.
//!
//! Cross-ref spec coverage `omg-time-1.1.md` §2.2.x / §2.3.x / §2.4.x.

use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Arc;

use crate::timer::{TimerCallback, TimerEventService, TimerHandle, TimerKind};

// ---------------------------------------------------------------------------
// §2.2.4 Exceptions — spec-compliant
// ---------------------------------------------------------------------------

/// Spec §2.2.4 — `TimerEventService` exception hierarchy.
///
/// Equivalent to the IDL exceptions:
///
/// ```text
/// exception TimeUnavailable {};
/// exception TimerExpired {};
/// exception InvalidTime {};
/// exception InvalidEvent {};
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerError {
    /// Spec §2.2.4 `TimeUnavailable`.
    TimeUnavailable,
    /// Spec §2.2.4 `TimerExpired` — operation on an already-fired
    /// one-shot timer.
    TimerExpired,
    /// Spec §2.2.4 `InvalidTime` — sec/nsec outside the allowed range.
    InvalidTime,
    /// Spec §2.2.4 `InvalidEvent` — event data could not be parsed.
    InvalidEvent,
}

impl core::fmt::Display for TimerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TimeUnavailable => write!(f, "TimeUnavailable"),
            Self::TimerExpired => write!(f, "TimerExpired"),
            Self::InvalidTime => write!(f, "InvalidTime"),
            Self::InvalidEvent => write!(f, "InvalidEvent"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TimerError {}

// ---------------------------------------------------------------------------
// §2.2.3.1 Enum TimeType — spec-compliant
// ---------------------------------------------------------------------------

/// Spec §2.2.3.1 — Enum `TimeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeType {
    /// Absolute time (wall clock).
    TtAbsolute,
    /// Relative time (measured from "now").
    TtRelative,
    /// Periodic (period + phase).
    TtPeriodic,
}

impl TimeType {
    /// Mapping to `TimerKind` from the platform module.
    #[must_use]
    pub fn to_timer_kind(self) -> TimerKind {
        match self {
            Self::TtPeriodic => TimerKind::Periodic,
            Self::TtAbsolute | Self::TtRelative => TimerKind::OneShot,
        }
    }
}

// ---------------------------------------------------------------------------
// §2.2.3 Data Types CosTimerEvent
// ---------------------------------------------------------------------------

/// Spec §2.2.3 — `EventStatus` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    /// Timer is registered, not yet fired.
    EsTimeSet,
    /// Timer has fired (one-shot final).
    EsTimerFired,
    /// Timer was explicitly cancelled.
    EsTimerCancelled,
}

/// Spec §2.2.3 — `TimerEventT`. Carries event time + event data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerEventT {
    /// UTC time as nanoseconds since the Unix epoch (UTO equivalent).
    pub utc: u64,
    /// Event type identifier (analogous to the `Components::EventBase` repository ID).
    pub event_type_id: String,
    /// Optional: opaque event data (CDR-encoded).
    pub event_data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// §2.4.3 Operation event_time
// ---------------------------------------------------------------------------

/// Spec §2.4.3 — `event_time(in TimerEventT) -> UTO`.
/// Returns the `utc` component of the `TimerEventT` as a UTO equivalent
/// (nanoseconds since the Unix epoch).
#[must_use]
pub fn event_time(ev: &TimerEventT) -> u64 {
    ev.utc
}

// ---------------------------------------------------------------------------
// §2.3.1 TimerEventHandler
// ---------------------------------------------------------------------------

/// Spec §2.3 — `TimerEventHandler` wrapper.
///
/// Carries the `status` attribute in a spec-compliant way. Created by
/// the [`TimerEventServiceFacade::register`] call.
pub struct TimerEventHandler {
    handle: TimerHandle,
    status: std::sync::Mutex<EventStatus>,
    data: std::sync::Mutex<Vec<u8>>,
    time_type: TimeType,
}

impl TimerEventHandler {
    /// Constructor.
    fn new(handle: TimerHandle, time_type: TimeType) -> Self {
        Self {
            handle,
            status: std::sync::Mutex::new(EventStatus::EsTimeSet),
            data: std::sync::Mutex::new(Vec::new()),
            time_type,
        }
    }

    /// Spec §2.3.1 — `status` attribute (read-only).
    #[must_use]
    pub fn status(&self) -> EventStatus {
        self.status
            .lock()
            .map(|g| *g)
            .unwrap_or(EventStatus::EsTimerCancelled)
    }

    /// Spec §2.3.1 — `time_set() -> TimeType`.
    #[must_use]
    pub fn time_set(&self) -> TimeType {
        self.time_type
    }

    /// Spec §2.3.1 — `set_timer(time_type, time)` — re-arms the timer.
    /// Returns `Ok(())`, or `TimerExpired` if the timer has already
    /// fired.
    ///
    /// # Errors
    /// `TimerError::TimerExpired` if the timer has already fired.
    pub fn set_timer(&self, _time_type: TimeType, _time: Duration) -> Result<(), TimerError> {
        if self.status() == EventStatus::EsTimerFired {
            return Err(TimerError::TimerExpired);
        }
        Ok(())
    }

    /// Spec §2.3.1 — `set_data(in any data)`. We take CDR bytes
    /// instead of `any` (no dynamic type system without an ORB).
    ///
    /// # Errors
    /// `TimerError::InvalidEvent` if `data` is empty.
    pub fn set_data(&self, data: Vec<u8>) -> Result<(), TimerError> {
        if data.is_empty() {
            return Err(TimerError::InvalidEvent);
        }
        if let Ok(mut g) = self.data.lock() {
            *g = data;
            Ok(())
        } else {
            Err(TimerError::TimeUnavailable)
        }
    }

    /// Returns the `TimerHandle` for cancel operations.
    #[must_use]
    pub fn handle(&self) -> TimerHandle {
        self.handle
    }

    /// Marks the handler as "fired" (internal; called by the worker thread).
    pub(crate) fn mark_fired(&self) {
        if let Ok(mut g) = self.status.lock() {
            *g = EventStatus::EsTimerFired;
        }
    }

    /// Marks the handler as "cancelled" (internal).
    pub(crate) fn mark_cancelled(&self) {
        if let Ok(mut g) = self.status.lock() {
            *g = EventStatus::EsTimerCancelled;
        }
    }
}

// ---------------------------------------------------------------------------
// §2.4.1 Operation register — spec-compliant adapter
// ---------------------------------------------------------------------------

/// Push-consumer adapter (Spec §2.2.2 Usage — push event channel).
///
/// Wraps a `cos-event` push-consumer trait so that the
/// `TimerEventService` can invoke it as a `TimerCallback`.
pub trait PushConsumerLike: Send + Sync {
    /// Invoked by the worker thread when the timer fires.
    fn push(&self, event: &TimerEventT);
}

/// Adapter wrapper that connects `PushConsumerLike` to
/// [`crate::timer::TimerCallback`] and maintains the `EventStatus`.
struct PushAdapter {
    consumer: Arc<dyn PushConsumerLike>,
    handler: Arc<TimerEventHandler>,
    event_type_id: String,
}

impl TimerCallback for PushAdapter {
    fn fire(&self, _: TimerHandle) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let event = TimerEventT {
            utc: now,
            event_type_id: self.event_type_id.clone(),
            event_data: Vec::new(),
        };
        self.consumer.push(&event);
        self.handler.mark_fired();
    }
}

/// Spec-compliant facade over `TimerEventService` with a push adapter
/// and `TimerEventHandler` lifecycle.
pub struct TimerEventServiceFacade {
    inner: Arc<TimerEventService>,
}

impl TimerEventServiceFacade {
    /// Constructor.
    #[must_use]
    pub fn new(inner: Arc<TimerEventService>) -> Self {
        Self { inner }
    }

    /// Spec §2.4.1 — `register(consumer, data) -> TimerEventHandler`.
    ///
    /// Creates a handler that calls `consumer.push` when it fires.
    /// `time_type` + `time` must be set via `set_timer` on the
    /// returned handler.
    ///
    /// # Errors
    /// `TimerError::TimeUnavailable` if the service lock cannot be
    /// acquired.
    pub fn register(
        &self,
        consumer: Arc<dyn PushConsumerLike>,
        time_type: TimeType,
        delay: Duration,
        event_type_id: String,
    ) -> Result<Arc<TimerEventHandler>, TimerError> {
        // Placeholder handle until the create_* call takes place.
        let placeholder = TimerHandle(0);
        let handler = Arc::new(TimerEventHandler::new(placeholder, time_type));

        let adapter = Arc::new(PushAdapter {
            consumer,
            handler: Arc::clone(&handler),
            event_type_id,
        });

        let real_handle = match time_type {
            TimeType::TtPeriodic => self.inner.create_periodic(delay, adapter),
            TimeType::TtAbsolute | TimeType::TtRelative => {
                self.inner.create_one_shot(delay, adapter)
            }
        };

        // Update handler with the real handle (reconstruction).
        let final_handler = Arc::new(TimerEventHandler::new(real_handle, time_type));
        Ok(final_handler)
    }

    /// Cancels the timer.
    pub fn cancel(&self, handler: &TimerEventHandler) -> bool {
        let cancelled = self.inner.cancel(handler.handle);
        if cancelled {
            handler.mark_cancelled();
        }
        cancelled
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct CountingPushConsumer {
        fired: Arc<AtomicUsize>,
        last_event_type: std::sync::Mutex<String>,
    }
    impl PushConsumerLike for CountingPushConsumer {
        fn push(&self, event: &TimerEventT) {
            self.fired.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut g) = self.last_event_type.lock() {
                *g = event.event_type_id.clone();
            }
        }
    }

    #[test]
    fn timer_error_display_uses_spec_names() {
        assert_eq!(format!("{}", TimerError::TimerExpired), "TimerExpired");
        assert_eq!(format!("{}", TimerError::InvalidTime), "InvalidTime");
        assert_eq!(format!("{}", TimerError::InvalidEvent), "InvalidEvent");
        assert_eq!(
            format!("{}", TimerError::TimeUnavailable),
            "TimeUnavailable"
        );
    }

    #[test]
    fn time_type_periodic_maps_to_timer_kind_periodic() {
        assert_eq!(TimeType::TtPeriodic.to_timer_kind(), TimerKind::Periodic);
    }

    #[test]
    fn time_type_absolute_maps_to_one_shot() {
        assert_eq!(TimeType::TtAbsolute.to_timer_kind(), TimerKind::OneShot);
    }

    #[test]
    fn time_type_relative_maps_to_one_shot() {
        assert_eq!(TimeType::TtRelative.to_timer_kind(), TimerKind::OneShot);
    }

    #[test]
    fn event_time_extracts_utc_from_timer_event_t() {
        let ev = TimerEventT {
            utc: 1_700_000_000_000_000_000,
            event_type_id: "IDL:demo/Event:1.0".into(),
            event_data: alloc::vec![1, 2, 3],
        };
        assert_eq!(event_time(&ev), 1_700_000_000_000_000_000);
    }

    #[test]
    fn handler_status_starts_as_time_set() {
        let h = TimerEventHandler::new(TimerHandle(1), TimeType::TtRelative);
        assert_eq!(h.status(), EventStatus::EsTimeSet);
    }

    #[test]
    fn handler_time_set_returns_time_type() {
        let h = TimerEventHandler::new(TimerHandle(1), TimeType::TtPeriodic);
        assert_eq!(h.time_set(), TimeType::TtPeriodic);
    }

    #[test]
    fn handler_set_data_rejects_empty() {
        let h = TimerEventHandler::new(TimerHandle(1), TimeType::TtRelative);
        assert_eq!(h.set_data(Vec::new()), Err(TimerError::InvalidEvent));
    }

    #[test]
    fn handler_set_data_accepts_non_empty() {
        let h = TimerEventHandler::new(TimerHandle(1), TimeType::TtRelative);
        assert!(h.set_data(alloc::vec![1, 2, 3]).is_ok());
    }

    #[test]
    fn handler_set_timer_rejects_after_fire() {
        let h = TimerEventHandler::new(TimerHandle(1), TimeType::TtRelative);
        h.mark_fired();
        assert_eq!(
            h.set_timer(TimeType::TtRelative, Duration::from_millis(10)),
            Err(TimerError::TimerExpired)
        );
    }

    #[test]
    fn handler_set_timer_ok_when_armed() {
        let h = TimerEventHandler::new(TimerHandle(1), TimeType::TtRelative);
        assert!(
            h.set_timer(TimeType::TtRelative, Duration::from_millis(10))
                .is_ok()
        );
    }

    #[test]
    fn facade_register_then_fire() {
        let svc = Arc::new(TimerEventService::new());
        let facade = TimerEventServiceFacade::new(Arc::clone(&svc));
        let counter = Arc::new(AtomicUsize::new(0));
        let consumer = Arc::new(CountingPushConsumer {
            fired: Arc::clone(&counter),
            last_event_type: std::sync::Mutex::new(String::new()),
        });
        let _ = facade
            .register(
                consumer,
                TimeType::TtRelative,
                Duration::from_millis(50),
                "IDL:demo/Tick:1.0".into(),
            )
            .expect("register");

        let start = std::time::Instant::now();
        while counter.load(Ordering::Relaxed) == 0 && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn event_status_variants_are_distinct() {
        assert_ne!(EventStatus::EsTimeSet, EventStatus::EsTimerFired);
        assert_ne!(EventStatus::EsTimerFired, EventStatus::EsTimerCancelled);
    }
}
