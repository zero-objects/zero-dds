// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bridge zwischen `TimerEventService` und `CosEventService::EventChannel`.
//!
//! Spec-Anker: OMG Time Service 1.1 §2.2 — TimerEventHandler ist
//! ein `CosEventComm::PushConsumer`. Wenn ein periodischer Timer feuert,
//! wird das vor-konfigurierte `AnyEvent` an den verbundenen Channel
//! gepusht.
//!
//! Feature-Gate `cos-event` zieht `zerodds-corba-cos-event` als
//! optionale Dep hinzu; ohne Feature bleibt `corba-ccm` von dem Stack
//! unabhaengig.

use alloc::sync::Arc;

use zerodds_corba_cos_event::comm::{AnyEvent, PushConsumer};

use crate::timer::{TimerCallback, TimerHandle};

/// Adapter: bei jedem Timer-Fire wird `event` an `consumer` gepusht.
///
/// Spec OMG Time Service 1.1 §2.2.4 — der TimerEventHandler ist
/// strukturell ein `CosEventComm::PushConsumer`; bei Disconnect des
/// Channels schluckt der Adapter den Fehler still (Timer laeuft
/// weiter, kann via `TimerEventService::cancel` abgesetzt werden).
pub struct EventChannelTimerCallback {
    consumer: Arc<dyn PushConsumer>,
    event: AnyEvent,
}

impl EventChannelTimerCallback {
    /// Konstruktor.
    #[must_use]
    pub fn new(consumer: Arc<dyn PushConsumer>, event: AnyEvent) -> Self {
        Self { consumer, event }
    }
}

impl core::fmt::Debug for EventChannelTimerCallback {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EventChannelTimerCallback")
            .field("event", &self.event)
            .finish()
    }
}

impl TimerCallback for EventChannelTimerCallback {
    fn fire(&self, _handle: TimerHandle) {
        let _ = self.consumer.push(self.event.clone());
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::timer::TimerEventService;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::thread::sleep;
    use std::time::Duration;
    use zerodds_corba_cos_event::channel::EventChannel;
    use zerodds_corba_cos_event::comm::{Disconnected, PushConsumer};

    /// Minimaler Test-Konsument der Pushes zaehlt + letzten AnyEvent festhaelt.
    struct CountingConsumer {
        count: AtomicUsize,
        last: Mutex<Option<AnyEvent>>,
    }

    impl CountingConsumer {
        fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
                last: Mutex::new(None),
            }
        }
    }

    impl PushConsumer for CountingConsumer {
        fn push(&self, event: AnyEvent) -> Result<(), Disconnected> {
            self.count.fetch_add(1, Ordering::Relaxed);
            *self.last.lock().unwrap() = Some(event);
            Ok(())
        }
        fn disconnect_push_consumer(&self) {}
    }

    #[test]
    fn one_shot_timer_pushes_event_to_channel() {
        let svc = TimerEventService::default();
        let channel = EventChannel::new();
        // Konsument an EventChannel registrieren.
        let counting = Arc::new(CountingConsumer::new());
        let supplier_proxy = channel.for_consumers().obtain_push_supplier();
        supplier_proxy
            .connect_push_consumer(counting.clone() as Arc<dyn PushConsumer>)
            .expect("connect consumer");
        // Supplier-Seite: Push-Consumer-Proxy bei Channel beziehen +
        // verbinden.
        let push_consumer = channel.for_suppliers().obtain_push_consumer();
        push_consumer
            .connect_push_supplier()
            .expect("connect supplier");
        // TimerEventService → cos-event Adapter.
        let event = AnyEvent::new("IDL:Tick:1.0".into(), alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let cb = Arc::new(EventChannelTimerCallback::new(
            push_consumer as Arc<dyn PushConsumer>,
            event.clone(),
        ));
        let _h = svc.create_one_shot(Duration::from_millis(10), cb);
        // Worker laeuft mit 20ms-Tick; warten bis gefeuert.
        for _ in 0..20 {
            if counting.count.load(Ordering::Relaxed) > 0 {
                break;
            }
            sleep(Duration::from_millis(10));
        }
        assert_eq!(counting.count.load(Ordering::Relaxed), 1);
        assert_eq!(counting.last.lock().unwrap().clone(), Some(event));
    }
}
