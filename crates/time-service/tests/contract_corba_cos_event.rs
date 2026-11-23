// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-time-service` with `zerodds-corba-cos-event`
//! (internal/release/rc8-test-suite.md, crate tier, point 2): the part of
//! `CosEventComm` the Timer Event Service uses. `register` takes an
//! `Arc<dyn PushConsumer>`; a trigger calls `push(AnyEvent)` with the
//! `TimerEventT` as `any`; `Err(Disconnected)` turns the handler into
//! `ESFailedTrigger` (OMG Time Service 1.1 §2.2.3.2, §2.4.1).

#![cfg(feature = "timer-event")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use zerodds_corba_cos_event::channel::EventChannel;
use zerodds_corba_cos_event::comm::{AnyEvent, Disconnected, PushConsumer};
use zerodds_time_service::TimeService;
use zerodds_time_service::Uto;
use zerodds_time_service::timer_event::{
    EventStatus, TIMER_EVENT_T_TYPE_ID, TimeType, TimerEventService, TimerEventT,
};

struct Consumer {
    tx: Mutex<mpsc::Sender<AnyEvent>>,
    connected: AtomicBool,
}

impl PushConsumer for Consumer {
    fn push(&self, event: AnyEvent) -> Result<(), Disconnected> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(Disconnected);
        }
        self.tx.lock().unwrap().send(event).unwrap();
        Ok(())
    }
    fn disconnect_push_consumer(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}

fn consumer() -> (Arc<Consumer>, mpsc::Receiver<AnyEvent>) {
    let (tx, rx) = mpsc::channel();
    (
        Arc::new(Consumer {
            tx: Mutex::new(tx),
            connected: AtomicBool::new(true),
        }),
        rx,
    )
}

#[test]
fn contract_push_consumer_receives_the_timer_event_as_any() {
    let svc = TimerEventService::new(TimeService::default()).unwrap();
    let (c, rx) = consumer();
    let data = AnyEvent::new("IDL:demo/Alarm:1.0".into(), vec![0, 1, 2]);
    let h = svc.register(c, data.clone()).unwrap();
    h.set_timer(TimeType::Relative, Uto::new(100_000, 0, 0).unwrap())
        .unwrap();
    let any = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(any.type_id, TIMER_EVENT_T_TYPE_ID);
    let ev = TimerEventT::from_any(&any).unwrap();
    assert_eq!(ev.event_data, data);
}

#[test]
fn contract_disconnected_is_a_failed_trigger() {
    let svc = TimerEventService::new(TimeService::default()).unwrap();
    let (c, rx) = consumer();
    let h = svc
        .register(c.clone(), AnyEvent::new(String::new(), Vec::new()))
        .unwrap();
    c.disconnect_push_consumer();
    h.set_timer(TimeType::Relative, Uto::new(10_000, 0, 0).unwrap())
        .unwrap();
    for _ in 0..500 {
        if h.status().unwrap() == EventStatus::FailedTrigger {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(h.status().unwrap(), EventStatus::FailedTrigger);
    assert!(rx.try_recv().is_err());
}

#[test]
fn time_2_2_2_timer_events_flow_through_a_push_event_channel() {
    // §2.2.2: create a push event channel, register it as the sink of a
    // timer event handler, set the timer; the event reaches the consumers
    // of the channel.
    let channel = EventChannel::new();
    let sink = channel.for_suppliers().obtain_push_consumer();
    sink.connect_push_supplier().unwrap();
    let (c, rx) = consumer();
    channel
        .for_consumers()
        .obtain_push_supplier()
        .connect_push_consumer(c)
        .unwrap();
    let svc = TimerEventService::new(TimeService::default()).unwrap();
    let data = AnyEvent::new("IDL:demo/Tick:1.0".into(), vec![42]);
    let h = svc.register(sink, data.clone()).unwrap();
    h.set_timer(TimeType::Periodic, Uto::new(100_000, 0, 0).unwrap())
        .unwrap();
    for _ in 0..3 {
        let any = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(TimerEventT::from_any(&any).unwrap().event_data, data);
    }
    assert!(h.cancel_timer().unwrap());
    assert_eq!(h.status().unwrap(), EventStatus::TimeCleared);
}
