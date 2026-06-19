// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bridge between `TimerEventService` and `CosEventService::EventChannel`.
//!
//! Spec anchor: OMG Time Service 1.1 §2.2 — a TimerEventHandler is
//! a `CosEventComm::PushConsumer`. When a periodic timer fires,
//! the pre-configured `AnyEvent` is pushed to the connected channel.
//!
//! The `cos-event` feature gate pulls in `zerodds-corba-cos-event` as
//! an optional dependency; without the feature, `corba-ccm` stays
//! independent of that stack.

use alloc::sync::Arc;

use zerodds_corba_cos_event::comm::{AnyEvent, PushConsumer};

use crate::timer::{TimerCallback, TimerHandle};

/// Adapter: on each timer fire, `event` is pushed to `consumer`.
///
/// Spec OMG Time Service 1.1 §2.2.4 — the TimerEventHandler is
/// structurally a `CosEventComm::PushConsumer`; if the channel
/// disconnects, the adapter swallows the error silently (the timer
/// keeps running and can be cancelled via `TimerEventService::cancel`).
pub struct EventChannelTimerCallback {
    consumer: Arc<dyn PushConsumer>,
    event: AnyEvent,
}

impl EventChannelTimerCallback {
    /// Constructor.
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

    /// Minimal test consumer that counts pushes and records the last AnyEvent.
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
        // Register the consumer with the EventChannel.
        let counting = Arc::new(CountingConsumer::new());
        let supplier_proxy = channel.for_consumers().obtain_push_supplier();
        supplier_proxy
            .connect_push_consumer(counting.clone() as Arc<dyn PushConsumer>)
            .expect("connect consumer");
        // Supplier side: obtain the push-consumer proxy from the channel +
        // connect.
        let push_consumer = channel.for_suppliers().obtain_push_consumer();
        push_consumer
            .connect_push_supplier()
            .expect("connect supplier");
        // TimerEventService → cos-event adapter.
        let event = AnyEvent::new("IDL:Tick:1.0".into(), alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let cb = Arc::new(EventChannelTimerCallback::new(
            push_consumer as Arc<dyn PushConsumer>,
            event.clone(),
        ));
        let _h = svc.create_one_shot(Duration::from_millis(10), cb);
        // The worker runs with a 20ms tick; wait until it fires.
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
