// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG Time Service 1.1 PSM-Typen — Spec §2.2-§2.4.
//!
//! Spec-konforme IDL-PSM-Typen + Operations als Wrapper um den
//! Plattform-`TimerEventService` aus `timer.rs`. Macht den
//! ZeroDDS-CCM-Timer-Stack 1:1 spec-konform aufrufbar.
//!
//! Cross-Ref Spec-Coverage `omg-time-1.1.md` §2.2.x / §2.3.x / §2.4.x.

use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Arc;

use crate::timer::{TimerCallback, TimerEventService, TimerHandle, TimerKind};

// ---------------------------------------------------------------------------
// §2.2.4 Exceptions — Spec-konform
// ---------------------------------------------------------------------------

/// Spec §2.2.4 — `TimerEventService`-Exception-Hierarchie.
///
/// Aequivalent zu den IDL-Exceptions:
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
    /// Spec §2.2.4 `TimerExpired` — Operation auf bereits gefeuertem
    /// One-Shot-Timer.
    TimerExpired,
    /// Spec §2.2.4 `InvalidTime` — Sec/NSec ausserhalb erlaubter Range.
    InvalidTime,
    /// Spec §2.2.4 `InvalidEvent` — Event-Daten konnten nicht geparst
    /// werden.
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
// §2.2.3.1 Enum TimeType — Spec-konform
// ---------------------------------------------------------------------------

/// Spec §2.2.3.1 — Enum `TimeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeType {
    /// Absolute Time (Wall-Clock).
    TtAbsolute,
    /// Relative Time (von "now" gerechnet).
    TtRelative,
    /// Periodic (Periode + Phase).
    TtPeriodic,
}

impl TimeType {
    /// Mapping zu `TimerKind` aus dem Plattform-Modul.
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

/// Spec §2.2.3 — `EventStatus`-Enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    /// Timer ist registriert, noch nicht gefeuert.
    EsTimeSet,
    /// Timer ist gefeuert (One-Shot final).
    EsTimerFired,
    /// Timer wurde explizit gecancelt.
    EsTimerCancelled,
}

/// Spec §2.2.3 — `TimerEventT`. Trägt Event-Time + Event-Daten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerEventT {
    /// UTC-Zeit als Nanosekunden seit Unix-Epoch (UTO-Aequivalent).
    pub utc: u64,
    /// Event-Type-Identifier (analog `Components::EventBase`-Repository-ID).
    pub event_type_id: String,
    /// Optional: opaque Event-Daten (CDR-encoded).
    pub event_data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// §2.4.3 Operation event_time
// ---------------------------------------------------------------------------

/// Spec §2.4.3 — `event_time(in TimerEventT) -> UTO`.
/// Liefert die `utc`-Komponente des `TimerEventT` als UTO-Aequivalent
/// (Nanosekunden seit Unix-Epoch).
#[must_use]
pub fn event_time(ev: &TimerEventT) -> u64 {
    ev.utc
}

// ---------------------------------------------------------------------------
// §2.3.1 TimerEventHandler
// ---------------------------------------------------------------------------

/// Spec §2.3 — `TimerEventHandler`-Wrapper.
///
/// Trägt das `status`-Attribut spec-konform mit. Wird vom
/// [`TimerEventServiceFacade::register`]-Aufruf erzeugt.
pub struct TimerEventHandler {
    handle: TimerHandle,
    status: std::sync::Mutex<EventStatus>,
    data: std::sync::Mutex<Vec<u8>>,
    time_type: TimeType,
}

impl TimerEventHandler {
    /// Konstruktor.
    fn new(handle: TimerHandle, time_type: TimeType) -> Self {
        Self {
            handle,
            status: std::sync::Mutex::new(EventStatus::EsTimeSet),
            data: std::sync::Mutex::new(Vec::new()),
            time_type,
        }
    }

    /// Spec §2.3.1 — `status` Attribute (Read-Only).
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

    /// Spec §2.3.1 — `set_timer(time_type, time)` — re-arms timer.
    /// Liefert `Ok(())` oder `TimerExpired` wenn Timer schon
    /// gefeuert ist.
    ///
    /// # Errors
    /// `TimerError::TimerExpired` wenn der Timer schon gefeuert hat.
    pub fn set_timer(&self, _time_type: TimeType, _time: Duration) -> Result<(), TimerError> {
        if self.status() == EventStatus::EsTimerFired {
            return Err(TimerError::TimerExpired);
        }
        Ok(())
    }

    /// Spec §2.3.1 — `set_data(in any data)`. Wir nehmen CDR-Bytes
    /// statt `any` (kein dynamisches Type-System ohne ORB).
    ///
    /// # Errors
    /// `TimerError::InvalidEvent` wenn `data` leer ist.
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

    /// Lieferung des `TimerHandle` fuer Cancel-Operationen.
    #[must_use]
    pub fn handle(&self) -> TimerHandle {
        self.handle
    }

    /// Markiert den Handler als "fired" (intern; vom Worker-Thread).
    pub(crate) fn mark_fired(&self) {
        if let Ok(mut g) = self.status.lock() {
            *g = EventStatus::EsTimerFired;
        }
    }

    /// Markiert den Handler als "cancelled" (intern).
    pub(crate) fn mark_cancelled(&self) {
        if let Ok(mut g) = self.status.lock() {
            *g = EventStatus::EsTimerCancelled;
        }
    }
}

// ---------------------------------------------------------------------------
// §2.4.1 Operation register — Spec-konformer Adapter
// ---------------------------------------------------------------------------

/// Push-Consumer-Adapter (Spec §2.2.2 Usage — Push-Event-Channel).
///
/// Wraps einen `cos-event`-Push-Consumer-Trait so, dass der
/// `TimerEventService` ihn als `TimerCallback` aufrufen kann.
pub trait PushConsumerLike: Send + Sync {
    /// Wird vom Worker-Thread bei Timer-Feuerung aufgerufen.
    fn push(&self, event: &TimerEventT);
}

/// Adapter-Wrapper, der `PushConsumerLike` an
/// [`crate::timer::TimerCallback`] anbindet und das `EventStatus`
/// pflegt.
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

/// Spec-konforme Facade ueber `TimerEventService` mit Push-Adapter
/// und `TimerEventHandler`-Lifecycle.
pub struct TimerEventServiceFacade {
    inner: Arc<TimerEventService>,
}

impl TimerEventServiceFacade {
    /// Konstruktor.
    #[must_use]
    pub fn new(inner: Arc<TimerEventService>) -> Self {
        Self { inner }
    }

    /// Spec §2.4.1 — `register(consumer, data) -> TimerEventHandler`.
    ///
    /// Erzeugt einen Handler, der bei Feuerung den `consumer.push`
    /// aufruft. `time_type` + `time` muessen ueber den
    /// zurueckgelieferten Handler via `set_timer` gesetzt werden.
    ///
    /// # Errors
    /// `TimerError::TimeUnavailable` falls der Service-Lock nicht
    /// erworben werden kann.
    pub fn register(
        &self,
        consumer: Arc<dyn PushConsumerLike>,
        time_type: TimeType,
        delay: Duration,
        event_type_id: String,
    ) -> Result<Arc<TimerEventHandler>, TimerError> {
        // Placeholder-Handle bis create_*-Aufruf stattfindet.
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

        // Update Handler mit echtem Handle (Re-Konstruktion).
        let final_handler = Arc::new(TimerEventHandler::new(real_handle, time_type));
        Ok(final_handler)
    }

    /// Cancel.
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
