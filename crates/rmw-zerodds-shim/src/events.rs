// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! QoS event callbacks: `rmw_event_set_callback`.
//!
//! The runtime says that some communication status moved (its status hook,
//! registered in `rmw_zerodds_init`, wakes the context's [`WaitNotify`]); it
//! does so on the thread that made the change, possibly with that endpoint's
//! slot locked, so nothing there may read a status back. A dispatcher thread
//! per context waits on the same notify, reads the counters of every endpoint
//! with a registered callback outside any runtime lock, and calls
//! `callback(user_data, change)` for each counter that moved since it last
//! reported it.

use core::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::{RmwEventCallback, RmwZerodsPublisherStatus, RmwZerodsSubscriptionStatus, WaitNotify};

/// A status counter a callback follows. The numbering is the shim's own and
/// is shared with `rmw_zerodds.c`, which maps each `rmw_event_type_t` to one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EventKind {
    /// OFFERED_DEADLINE_MISSED.
    OfferedDeadlineMissed = 0,
    /// LIVELINESS_LOST.
    LivelinessLost = 1,
    /// OFFERED_INCOMPATIBLE_QOS.
    OfferedIncompatibleQos = 2,
    /// PUBLICATION_MATCHED.
    PublicationMatched = 3,
    /// PUBLISHER_INCOMPATIBLE_TYPE.
    PublisherIncompatibleType = 4,
    /// REQUESTED_DEADLINE_MISSED.
    RequestedDeadlineMissed = 10,
    /// LIVELINESS_CHANGED.
    LivelinessChanged = 11,
    /// REQUESTED_INCOMPATIBLE_QOS.
    RequestedIncompatibleQos = 12,
    /// MESSAGE_LOST.
    MessageLost = 13,
    /// SUBSCRIPTION_MATCHED.
    SubscriptionMatched = 14,
    /// SUBSCRIPTION_INCOMPATIBLE_TYPE.
    SubscriptionIncompatibleType = 15,
}

impl EventKind {
    /// The kind for the shim number `n`.
    #[must_use]
    pub fn from_u32(n: u32) -> Option<Self> {
        Some(match n {
            0 => Self::OfferedDeadlineMissed,
            1 => Self::LivelinessLost,
            2 => Self::OfferedIncompatibleQos,
            3 => Self::PublicationMatched,
            4 => Self::PublisherIncompatibleType,
            10 => Self::RequestedDeadlineMissed,
            11 => Self::LivelinessChanged,
            12 => Self::RequestedIncompatibleQos,
            13 => Self::MessageLost,
            14 => Self::SubscriptionMatched,
            15 => Self::SubscriptionIncompatibleType,
            _ => return None,
        })
    }

    /// Whether the kind belongs to a publisher.
    #[must_use]
    pub fn is_publisher(self) -> bool {
        (self as u32) < 10
    }
}

/// The value a kind's callback follows. A matched count moves both ways;
/// its change is the distance, so a match and an unmatch between two
/// passes cancel out, as they do in the status itself.
fn publisher_counter(kind: EventKind, st: &RmwZerodsPublisherStatus) -> u64 {
    match kind {
        EventKind::OfferedDeadlineMissed => st.offered_deadline_missed_total,
        EventKind::LivelinessLost => st.liveliness_lost_total,
        EventKind::OfferedIncompatibleQos => st.offered_incompatible_qos_total,
        EventKind::PublicationMatched => st.matched_current,
        EventKind::PublisherIncompatibleType => st.incompatible_type_total,
        _ => 0,
    }
}

fn subscription_counter(kind: EventKind, st: &RmwZerodsSubscriptionStatus) -> u64 {
    match kind {
        EventKind::RequestedDeadlineMissed => st.requested_deadline_missed_total,
        EventKind::LivelinessChanged => st.liveliness_alive + st.liveliness_not_alive,
        EventKind::RequestedIncompatibleQos => st.requested_incompatible_qos_total,
        EventKind::MessageLost => st.sample_lost_total,
        EventKind::SubscriptionMatched => st.matched_current,
        EventKind::SubscriptionIncompatibleType => st.incompatible_type_total,
        _ => 0,
    }
}

/// Where an endpoint's status is read from.
#[derive(Clone, Copy)]
pub(crate) enum Probe {
    Writer(*mut zerodds::ZeroDdsWriter),
    Reader(*mut zerodds::ZeroDdsReader),
}

#[derive(Clone, Copy)]
struct Registration {
    kind: EventKind,
    cb: RmwEventCallback,
    user: usize,
    reported: u64,
}

struct EventsState {
    /// `false` once the endpoint is being destroyed: the probe pointer is
    /// then no longer read.
    alive: bool,
    probe: Probe,
    registrations: Vec<Registration>,
}

/// One endpoint's registered event callbacks.
pub(crate) struct EndpointEvents {
    state: Mutex<EventsState>,
}

// SAFETY: the probe pointer is read only under `state`'s lock and only while
// `alive`; the endpoint clears `alive` under that lock before it destroys the
// writer or reader.
unsafe impl Send for EndpointEvents {}
// SAFETY: as above.
unsafe impl Sync for EndpointEvents {}

impl EndpointEvents {
    pub(crate) fn new(probe: Probe) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(EventsState {
                alive: true,
                probe,
                registrations: Vec::new(),
            }),
        })
    }

    /// The counter of `kind` now; `None` if the endpoint is gone or its
    /// status cannot be read.
    fn counter(state: &EventsState, kind: EventKind) -> Option<u64> {
        if !state.alive {
            return None;
        }
        match state.probe {
            Probe::Writer(w) => {
                let mut st = RmwZerodsPublisherStatus::default();
                let mut raw = zerodds::ZeroDdsWriterStatus::default();
                // SAFETY: `w` is alive (checked under the lock); `raw` a local.
                if unsafe { zerodds::zerodds_writer_get_status(w, &mut raw) } != 0 {
                    return None;
                }
                st.offered_deadline_missed_total = raw.offered_deadline_missed_total;
                st.liveliness_lost_total = raw.liveliness_lost_total;
                st.offered_incompatible_qos_total = raw.offered_incompatible_qos_total;
                st.matched_current = raw.matched_current;
                st.incompatible_type_total = raw.incompatible_type_total;
                Some(publisher_counter(kind, &st))
            }
            Probe::Reader(r) => {
                let mut st = RmwZerodsSubscriptionStatus::default();
                let mut raw = zerodds::ZeroDdsReaderStatus::default();
                // SAFETY: `r` is alive (checked under the lock); `raw` a local.
                if unsafe { zerodds::zerodds_reader_get_status(r, &mut raw) } != 0 {
                    return None;
                }
                st.requested_deadline_missed_total = raw.requested_deadline_missed_total;
                st.liveliness_alive = raw.liveliness_alive;
                st.liveliness_not_alive = raw.liveliness_not_alive;
                st.requested_incompatible_qos_total = raw.requested_incompatible_qos_total;
                st.sample_lost_total = raw.sample_lost_total;
                st.matched_current = raw.matched_current;
                st.incompatible_type_total = raw.incompatible_type_total;
                Some(subscription_counter(kind, &st))
            }
        }
    }

    /// Sets (`Some`) or clears (`None`) the callback of `kind`. A new
    /// callback starts from the current counter.
    pub(crate) fn set(&self, kind: EventKind, cb: Option<RmwEventCallback>, user: *const c_void) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.registrations.retain(|r| r.kind != kind);
        if let Some(cb) = cb {
            let reported = Self::counter(&state, kind).unwrap_or(0);
            state.registrations.push(Registration {
                kind,
                cb,
                user: user as usize,
                reported,
            });
        }
    }

    pub(crate) fn has_registrations(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.alive && !s.registrations.is_empty())
    }

    /// Stops reading the probe; called before the writer or reader goes.
    pub(crate) fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.alive = false;
            state.registrations.clear();
        }
    }

    /// Calls every callback whose counter moved since it last reported.
    fn dispatch(&self) {
        let calls: Vec<(RmwEventCallback, usize, usize)> = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let mut calls = Vec::new();
            for i in 0..state.registrations.len() {
                let reg = state.registrations[i];
                let Some(now) = Self::counter(&state, reg.kind) else {
                    continue;
                };
                if now != reg.reported {
                    let change = now.abs_diff(reg.reported);
                    state.registrations[i].reported = now;
                    calls.push((
                        reg.cb,
                        reg.user,
                        usize::try_from(change).unwrap_or(usize::MAX),
                    ));
                }
            }
            calls
        };
        // Outside the lock: a callback may set or clear callbacks.
        for (cb, user, change) in calls {
            cb(user as *const c_void, change);
        }
    }
}

/// A context's dispatcher: the endpoints with callbacks and the thread that
/// serves them, started with the first registration.
pub(crate) struct EventDispatcher {
    notify: Arc<WaitNotify>,
    endpoints: Mutex<Vec<Weak<EndpointEvents>>>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl EventDispatcher {
    pub(crate) fn new(notify: Arc<WaitNotify>) -> Arc<Self> {
        Arc::new(Self {
            notify,
            endpoints: Mutex::new(Vec::new()),
            stop: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        })
    }

    /// Follows `endpoint`, and starts the thread if it is not running.
    pub(crate) fn watch(self: &Arc<Self>, endpoint: &Arc<EndpointEvents>) {
        if let Ok(mut list) = self.endpoints.lock() {
            list.retain(|w| w.strong_count() > 0);
            if !list.iter().any(|w| w.as_ptr() == Arc::as_ptr(endpoint)) {
                list.push(Arc::downgrade(endpoint));
            }
        }
        let Ok(mut thread) = self.thread.lock() else {
            return;
        };
        if thread.is_none() {
            let me = Arc::downgrade(self);
            let stop = Arc::clone(&self.stop);
            let notify = Arc::clone(&self.notify);
            *thread = std::thread::Builder::new()
                .name("rmw-zdds-events".into())
                .spawn(move || run(&me, &stop, &notify))
                .ok();
        }
    }

    /// Stops and joins the thread.
    pub(crate) fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.notify.notify();
        let handle = self.thread.lock().ok().and_then(|mut t| t.take());
        if let Some(h) = handle {
            let _ = h.join();
        }
    }

    fn pass(&self) {
        let endpoints: Vec<Arc<EndpointEvents>> = self
            .endpoints
            .lock()
            .map(|list| list.iter().filter_map(Weak::upgrade).collect())
            .unwrap_or_default();
        for endpoint in endpoints {
            if endpoint.has_registrations() {
                endpoint.dispatch();
            }
        }
    }
}

fn run(me: &Weak<EventDispatcher>, stop: &AtomicBool, notify: &WaitNotify) {
    while !stop.load(Ordering::SeqCst) {
        let generation = notify.current();
        match me.upgrade() {
            Some(dispatcher) => dispatcher.pass(),
            None => return,
        }
        // Woken by the status hook (and by data arrivals, which cost one
        // pass over the endpoints with callbacks); the second is a backstop.
        notify.wait_until(generation, Instant::now() + Duration::from_secs(1));
    }
}
