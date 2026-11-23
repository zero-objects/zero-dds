// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Timer Event Service — module `CosTimerEvent`, OMG Time Service 1.1
//! §2.2 to §2.4. Feature `timer-event` (default).
//!
//! ```idl
//! module CosTimerEvent {
//!   enum TimeType { TTAbsolute, TTRelative, TTPeriodic };
//!   enum EventStatus { ESTimeSet, ESTimeCleared, ESTriggered, ESFailedTrigger };
//!   struct TimerEventT { TimeBase::UtcT utc; any event_data; };
//!   interface TimerEventHandler {
//!     readonly attribute EventStatus status;
//!     boolean time_set(out CosTime::UTO uto);
//!     void set_timer(in TimeType time_type, in CosTime::UTO trigger_time);
//!     boolean cancel_timer();
//!     void set_data(in any event_data);
//!   };
//!   interface TimerEventService {
//!     TimerEventHandler register(in CosEventComm::PushConsumer event_interface, in any data);
//!     void unregister(in TimerEventHandler timer_event_handler);
//!     CosTime::UTO event_time(in TimerEventT timer_event);
//!   };
//! };
//! ```
//!
//! A [`TimerEventService`] keeps its handlers and one worker thread. A
//! handler is primed with [`TimerEventHandler::set_timer`]; when its time
//! comes, the worker pushes a [`TimerEventT`] to the handler's
//! `CosEventComm::PushConsumer` ([`zerodds_corba_cos_event::comm::PushConsumer`],
//! the push model, §2.4.1). The event travels as that crate's `any`
//! ([`AnyEvent`]) with the repository id [`TIMER_EVENT_T_TYPE_ID`]; see
//! [`TimerEventT::to_any`] for the encoding.
//!
//! The service works in the process: a `PushConsumer` is a Rust object.
//! The feature `serve` makes CosTime reachable over GIOP, not this
//! module; a remote `PushConsumer` reference and a `TimerEventT` as a
//! CORBA `any` with a TypeCode are not supported.
//!
//! # Expected trigger interval (§2.2.2)
//!
//! "The implementor of the service will document what the expected
//! interval is." The worker waits on a condition variable until the
//! earliest trigger time on the clock of the service's [`TimeService`]
//! and pushes as soon as it wakes. An event is therefore never pushed
//! before its requested time; it is pushed after it by the wake-up
//! latency of the operating system plus the time the pushes take that
//! are due before it, since one thread pushes them one after the other.
//! The worker re-reads the clock at least once a second, so a step of the
//! system clock delays a trigger by at most one second. Every event
//! carries its actual delay as the inaccuracy of `utc` (§2.2.3.3), so a
//! consumer sees the interval of each event. The crate's test
//! `time_2_2_2_trigger_delay_is_reported_and_bounded` measures it.
//!
//! # Errors
//!
//! The service has no exceptions of its own (§2.2.4); [`TimerEventError`]
//! names the standard CORBA exceptions the operations raise.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use zerodds_corba_cos_event::comm::{AnyEvent, PushConsumer};

use crate::service::TimeService;
use crate::time_base::{MAX_INACCURACY, TICKS_PER_SECOND, TimeT, UtcT};
use crate::uto::Uto;

/// Repository id of `CosTimerEvent::TimerEventT`, the `type_id` of the
/// [`AnyEvent`] a trigger pushes.
pub const TIMER_EVENT_T_TYPE_ID: &str = "IDL:omg.org/CosTimerEvent/TimerEventT:1.0";

/// How many handlers a service registers before `register` raises
/// `CORBA::NO_RESOURCES`, unless [`TimerEventService::with_capacity`]
/// says otherwise.
pub const DEFAULT_MAX_HANDLERS: usize = 65_536;

/// The longest the worker waits without reading the clock again.
const MAX_WAIT: Duration = Duration::from_secs(1);

/// `CosTimerEvent::TimeType` (§2.2.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeType {
    /// `TTAbsolute` — the time is an absolute time.
    Absolute,
    /// `TTRelative` — the time is relative to the current time.
    Relative,
    /// `TTPeriodic` — the time is a period between successive events.
    Periodic,
}

/// `CosTimerEvent::EventStatus` (§2.2.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    /// `ESTimeSet` — set with a time in the future, triggers when it
    /// comes. A periodic handler returns to it after each trigger.
    TimeSet,
    /// `ESTimeCleared` — not set to go off: the state after `register`
    /// and after a cancel before the trigger time.
    TimeCleared,
    /// `ESTriggered` — triggered and the data was sent to the channel.
    /// Never the state of a periodic handler.
    Triggered,
    /// `ESFailedTrigger` — triggered, but the data could not be
    /// delivered over the channel.
    FailedTrigger,
}

/// The standard CORBA exceptions of the Timer Event Service (§2.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEventError {
    /// `CORBA::BAD_PARAM` (§2.3.1.2): a relative or periodic time of zero
    /// with no relative time before it, or a trigger time beyond the
    /// range of `TimeT`.
    BadParam,
    /// `CORBA::NO_RESOURCES` (§2.4.1): the handler limit of the service
    /// is reached, or its worker thread could not be started.
    NoResources,
    /// `CORBA::INV_OBJREF` (§2.4.2): the handler was unregistered, belongs
    /// to another service, or its service is gone.
    InvObjref,
    /// The clock of the service gave no current time (the condition of
    /// [`crate::TimeUnavailable`]): a relative time cannot be placed and a
    /// new handler gets no initial time. A CORBA server reports it as a
    /// system exception.
    TimeUnavailable,
}

impl fmt::Display for TimerEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadParam => write!(f, "CORBA::BAD_PARAM: timer parameter out of range"),
            Self::NoResources => write!(f, "CORBA::NO_RESOURCES: timer event handler limit"),
            Self::InvObjref => write!(f, "CORBA::INV_OBJREF: timer event handler not registered"),
            Self::TimeUnavailable => write!(f, "underlying time service unavailable"),
        }
    }
}

impl std::error::Error for TimerEventError {}

/// `CORBA::MARSHAL` — an [`AnyEvent`] did not decode as a
/// [`TimerEventT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marshal;

impl fmt::Display for Marshal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CORBA::MARSHAL: not a CosTimerEvent::TimerEventT")
    }
}

impl std::error::Error for Marshal {}

/// `CosTimerEvent::TimerEventT` (§2.2.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerEventT {
    /// The actual time of the trigger in `time`; `inacclo`/`inacchi` hold
    /// the difference between the requested and the actual time.
    pub utc: UtcT,
    /// The data the requester asked to be sent ([`TimerEventHandler::set_data`]).
    pub event_data: AnyEvent,
}

impl TimerEventT {
    /// The pushed form: an [`AnyEvent`] with `type_id`
    /// [`TIMER_EVENT_T_TYPE_ID`] and as `data` a CDR encapsulation
    /// (CORBA 3.3 Part 2 §9.3.3) in the byte order of this host: the
    /// byte-order octet, `utc` as the struct `TimeBase::UtcT` (`time` at
    /// offset 8, `inacclo`, `inacchi`, `tdf`), then `event_data` the way
    /// `zerodds-corba-cos-event` carries every `any`, its repository id
    /// as a CDR `string` and its encapsulation as a `sequence<octet>`.
    #[must_use]
    pub fn to_any(&self) -> AnyEvent {
        let mut w = CdrWriter::new();
        w.u64(self.utc.time);
        w.u32(self.utc.inacclo);
        w.u16(self.utc.inacchi);
        w.u16(self.utc.tdf as u16);
        w.string(&self.event_data.type_id);
        w.octets(&self.event_data.data);
        AnyEvent::new(String::from(TIMER_EVENT_T_TYPE_ID), w.finish())
    }

    /// Decodes the form of [`Self::to_any`] in either byte order.
    ///
    /// # Errors
    /// [`Marshal`] if the repository id is not [`TIMER_EVENT_T_TYPE_ID`],
    /// the byte-order octet is neither 0 nor 1, a length runs past the
    /// end, the string is not NUL-terminated UTF-8, or bytes are left.
    pub fn from_any(any: &AnyEvent) -> Result<Self, Marshal> {
        if any.type_id != TIMER_EVENT_T_TYPE_ID {
            return Err(Marshal);
        }
        let mut r = CdrReader::new(&any.data)?;
        let time = r.u64()?;
        let inacclo = r.u32()?;
        let inacchi = r.u16()?;
        let tdf = r.u16()? as i16;
        let type_id = r.string()?;
        let data = r.octets()?;
        r.finish()?;
        Ok(Self {
            utc: UtcT {
                time,
                inacclo,
                inacchi,
                tdf,
            },
            event_data: AnyEvent::new(type_id, data),
        })
    }
}

/// CDR writer for one encapsulation in the native byte order; alignment
/// counts from the byte-order octet.
struct CdrWriter {
    buf: Vec<u8>,
}

impl CdrWriter {
    fn new() -> Self {
        Self {
            buf: alloc::vec![u8::from(cfg!(target_endian = "little"))],
        }
    }
    fn align(&mut self, n: usize) {
        while !self.buf.len().is_multiple_of(n) {
            self.buf.push(0);
        }
    }
    fn u16(&mut self, v: u16) {
        self.align(2);
        self.buf.extend_from_slice(&v.to_ne_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.align(4);
        self.buf.extend_from_slice(&v.to_ne_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.align(8);
        self.buf.extend_from_slice(&v.to_ne_bytes());
    }
    fn string(&mut self, s: &str) {
        self.u32(u32::try_from(s.len() + 1).unwrap_or(u32::MAX));
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
    fn octets(&mut self, b: &[u8]) {
        self.u32(u32::try_from(b.len()).unwrap_or(u32::MAX));
        self.buf.extend_from_slice(b);
    }
    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// CDR reader for one encapsulation; the first octet gives the byte
/// order.
struct CdrReader<'a> {
    buf: &'a [u8],
    pos: usize,
    little: bool,
}

impl<'a> CdrReader<'a> {
    fn new(buf: &'a [u8]) -> Result<Self, Marshal> {
        let little = match buf.first() {
            Some(0) => false,
            Some(1) => true,
            _ => return Err(Marshal),
        };
        Ok(Self {
            buf,
            pos: 1,
            little,
        })
    }
    fn take(&mut self, align: usize, n: usize) -> Result<&'a [u8], Marshal> {
        let start = self.pos.div_ceil(align) * align;
        let end = start.checked_add(n).ok_or(Marshal)?;
        let bytes = self.buf.get(start..end).ok_or(Marshal)?;
        self.pos = end;
        Ok(bytes)
    }
    fn u16(&mut self) -> Result<u16, Marshal> {
        let b: [u8; 2] = self.take(2, 2)?.try_into().map_err(|_| Marshal)?;
        Ok(if self.little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    fn u32(&mut self) -> Result<u32, Marshal> {
        let b: [u8; 4] = self.take(4, 4)?.try_into().map_err(|_| Marshal)?;
        Ok(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn u64(&mut self) -> Result<u64, Marshal> {
        let b: [u8; 8] = self.take(8, 8)?.try_into().map_err(|_| Marshal)?;
        Ok(if self.little {
            u64::from_le_bytes(b)
        } else {
            u64::from_be_bytes(b)
        })
    }
    fn string(&mut self) -> Result<String, Marshal> {
        let len = usize::try_from(self.u32()?).map_err(|_| Marshal)?;
        let bytes = self.take(1, len)?;
        let (last, text) = bytes.split_last().ok_or(Marshal)?;
        if *last != 0 {
            return Err(Marshal);
        }
        core::str::from_utf8(text)
            .map(String::from)
            .map_err(|_| Marshal)
    }
    fn octets(&mut self) -> Result<Vec<u8>, Marshal> {
        let len = usize::try_from(self.u32()?).map_err(|_| Marshal)?;
        Ok(self.take(1, len)?.to_vec())
    }
    fn finish(&self) -> Result<(), Marshal> {
        if self.pos == self.buf.len() {
            Ok(())
        } else {
            Err(Marshal)
        }
    }
}

/// State of one handler.
struct HandlerState {
    consumer: Arc<dyn PushConsumer>,
    data: AnyEvent,
    status: EventStatus,
    /// The current value of the timer (§2.3.1.1): the registration time,
    /// then the trigger time of the last `set_timer`, for a periodic
    /// handler the next trigger time.
    time: TimeT,
    /// The last non-zero relative or periodic time (§2.3.1.2).
    last_relative: Option<TimeT>,
    /// The period of a periodic handler.
    period: Option<TimeT>,
    /// Key of the pending trigger in [`State::schedule`].
    armed: Option<(TimeT, u64)>,
    /// Changes with every `set_timer`, `cancel_timer` and trigger, so the
    /// worker can tell whether a handler was reprogrammed during a push.
    generation: u64,
}

/// The shared state of a service.
struct State {
    next_id: u64,
    next_seq: u64,
    handlers: HashMap<u64, HandlerState>,
    /// Pending triggers by (time, sequence number) -> handler id.
    schedule: BTreeMap<(TimeT, u64), u64>,
    shutdown: bool,
}

struct Shared {
    clock: TimeService,
    max_handlers: usize,
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A poisoned lock means a push panicked in a consumer; the state
        // itself is consistent at every unlock, so keep serving.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn now(&self) -> Option<TimeT> {
        self.clock.universal_time().ok().map(Uto::time)
    }
}

impl State {
    fn disarm(&mut self, id: u64) {
        if let Some(key) = self.handlers.get_mut(&id).and_then(|h| h.armed.take()) {
            self.schedule.remove(&key);
        }
    }

    fn arm(&mut self, id: u64, at: TimeT) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        if let Some(h) = self.handlers.get_mut(&id) {
            h.armed = Some((at, seq));
            h.time = at;
            self.schedule.insert((at, seq), id);
        }
    }
}

/// `CosTimerEvent::TimerEventService` (§2.4).
pub struct TimerEventService {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl fmt::Debug for TimerEventService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let st = self.shared.lock();
        f.debug_struct("TimerEventService")
            .field("handlers", &st.handlers.len())
            .field("pending", &st.schedule.len())
            .field("max_handlers", &self.shared.max_handlers)
            .finish()
    }
}

impl TimerEventService {
    /// A service on the clock and time zone of `clock` (§2.5: the Timer
    /// Event Service builds on the Basic Time Service), with
    /// [`DEFAULT_MAX_HANDLERS`].
    ///
    /// # Errors
    /// [`TimerEventError::NoResources`] if the worker thread cannot be
    /// started.
    pub fn new(clock: TimeService) -> Result<Self, TimerEventError> {
        Self::with_capacity(clock, DEFAULT_MAX_HANDLERS)
    }

    /// As [`Self::new`], with at most `max_handlers` registered handlers.
    ///
    /// # Errors
    /// [`TimerEventError::NoResources`] if the worker thread cannot be
    /// started.
    pub fn with_capacity(clock: TimeService, max_handlers: usize) -> Result<Self, TimerEventError> {
        let shared = Arc::new(Shared {
            clock,
            max_handlers,
            state: Mutex::new(State {
                next_id: 1,
                next_seq: 0,
                handlers: HashMap::new(),
                schedule: BTreeMap::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
        });
        let for_worker = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name(String::from("zerodds-timer-event"))
            .spawn(move || run_worker(&for_worker))
            .map_err(|_| TimerEventError::NoResources)?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// §2.4.1 `register(event_interface, data)`: a handler whose events
    /// go to `event_interface` with `data`. Its time is the current time,
    /// its status [`EventStatus::TimeCleared`], and no event is scheduled
    /// until [`TimerEventHandler::set_timer`].
    ///
    /// # Errors
    /// [`TimerEventError::NoResources`] at the handler limit;
    /// [`TimerEventError::TimeUnavailable`] if the clock gives no time.
    pub fn register(
        &self,
        event_interface: Arc<dyn PushConsumer>,
        data: AnyEvent,
    ) -> Result<TimerEventHandler, TimerEventError> {
        let now = self.shared.now().ok_or(TimerEventError::TimeUnavailable)?;
        let mut st = self.shared.lock();
        if st.handlers.len() >= self.shared.max_handlers {
            return Err(TimerEventError::NoResources);
        }
        let id = st.next_id;
        st.next_id = st.next_id.wrapping_add(1);
        st.handlers.insert(
            id,
            HandlerState {
                consumer: event_interface,
                data,
                status: EventStatus::TimeCleared,
                time: now,
                last_relative: None,
                period: None,
                armed: None,
                generation: 0,
            },
        );
        Ok(TimerEventHandler {
            id,
            service: Arc::downgrade(&self.shared),
        })
    }

    /// §2.4.2 `unregister(timer_event_handler)`: releases the handler and
    /// cancels its pending trigger. Every later use of it raises
    /// `CORBA::INV_OBJREF`.
    ///
    /// # Errors
    /// [`TimerEventError::InvObjref`] if the handler is not registered
    /// with this service.
    pub fn unregister(
        &self,
        timer_event_handler: &TimerEventHandler,
    ) -> Result<(), TimerEventError> {
        if !Weak::ptr_eq(&timer_event_handler.service, &Arc::downgrade(&self.shared)) {
            return Err(TimerEventError::InvObjref);
        }
        let mut st = self.shared.lock();
        st.disarm(timer_event_handler.id);
        st.handlers
            .remove(&timer_event_handler.id)
            .map(|_| ())
            .ok_or(TimerEventError::InvObjref)
    }

    /// §2.4.3 `event_time(timer_event)`: a UTO with the time at which the
    /// event was triggered (and the inaccuracy of the trigger).
    #[must_use]
    pub fn event_time(&self, timer_event: &TimerEventT) -> Uto {
        Uto::from_utc(timer_event.utc)
    }

    /// The number of registered handlers.
    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.shared.lock().handlers.len()
    }
}

impl Drop for TimerEventService {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.wake.notify_all();
        if let Some(worker) = self.worker.take() {
            // Dropped from a push on the worker itself: it ends on its own
            // once the push returns and must not join itself.
            if worker.thread().id() != std::thread::current().id() {
                let _ = worker.join();
            }
        }
    }
}

/// `CosTimerEvent::TimerEventHandler` (§2.3): a reference to a handler of
/// a [`TimerEventService`]. Clones refer to the same handler.
#[derive(Clone)]
pub struct TimerEventHandler {
    id: u64,
    service: Weak<Shared>,
}

impl fmt::Debug for TimerEventHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimerEventHandler")
            .field("id", &self.id)
            .finish()
    }
}

impl TimerEventHandler {
    fn with_state<R>(
        &self,
        f: impl FnOnce(&Shared, &mut State, u64) -> Result<R, TimerEventError>,
    ) -> Result<R, TimerEventError> {
        let shared = self.service.upgrade().ok_or(TimerEventError::InvObjref)?;
        let mut st = shared.lock();
        if !st.handlers.contains_key(&self.id) {
            return Err(TimerEventError::InvObjref);
        }
        let r = f(&shared, &mut st, self.id);
        drop(st);
        shared.wake.notify_all();
        r
    }

    /// §2.3.1 attribute `status`.
    ///
    /// # Errors
    /// [`TimerEventError::InvObjref`] after `unregister`.
    pub fn status(&self) -> Result<EventStatus, TimerEventError> {
        self.with_state(|_, st, id| {
            st.handlers
                .get(&id)
                .map(|h| h.status)
                .ok_or(TimerEventError::InvObjref)
        })
    }

    /// §2.3.1.1 `time_set(out uto)`: `true` if a time is set for an event
    /// yet to be triggered, and always the current value of the timer as
    /// a UTO (absolute time, with the local `tdf` of §1.3.2.4).
    ///
    /// # Errors
    /// [`TimerEventError::InvObjref`] after `unregister`.
    pub fn time_set(&self) -> Result<(bool, Uto), TimerEventError> {
        self.with_state(|shared, st, id| {
            let h = st.handlers.get(&id).ok_or(TimerEventError::InvObjref)?;
            let set = h.status == EventStatus::TimeSet;
            let utc = UtcT {
                time: h.time,
                inacclo: 0,
                inacchi: 0,
                tdf: shared.clock.tdf_at(h.time),
            };
            Ok((set, Uto::from_utc(utc)))
        })
    }

    /// §2.3.1.2 `set_timer(time_type, trigger_time)`: cancels the previous
    /// trigger and sets a new one at `trigger_time.time()` if absolute,
    /// at the current time plus it if relative, and every period of it
    /// from the current time on if periodic. A relative or periodic time
    /// of zero reuses the last relative time that was given.
    ///
    /// # Errors
    /// [`TimerEventError::BadParam`] for a zero relative time with none
    /// before it, or a trigger time beyond `TimeT`;
    /// [`TimerEventError::TimeUnavailable`] if the clock gives no time for
    /// a relative or periodic time; [`TimerEventError::InvObjref`] after
    /// `unregister`.
    pub fn set_timer(&self, time_type: TimeType, trigger_time: Uto) -> Result<(), TimerEventError> {
        self.with_state(|shared, st, id| {
            let h = st.handlers.get(&id).ok_or(TimerEventError::InvObjref)?;
            let (at, period, last_relative) = match time_type {
                TimeType::Absolute => (trigger_time.time(), None, h.last_relative),
                TimeType::Relative | TimeType::Periodic => {
                    let relative = match trigger_time.time() {
                        0 => h.last_relative.ok_or(TimerEventError::BadParam)?,
                        t => t,
                    };
                    let now = shared.now().ok_or(TimerEventError::TimeUnavailable)?;
                    let at = now.checked_add(relative).ok_or(TimerEventError::BadParam)?;
                    let period = (time_type == TimeType::Periodic).then_some(relative);
                    (at, period, Some(relative))
                }
            };
            st.disarm(id);
            if let Some(h) = st.handlers.get_mut(&id) {
                h.period = period;
                h.last_relative = last_relative;
                h.status = EventStatus::TimeSet;
                h.generation = h.generation.wrapping_add(1);
            }
            st.arm(id, at);
            Ok(())
        })
    }

    /// §2.3.1.3 `cancel_timer()`: cancels the trigger if one was set and
    /// has not gone off yet (for a periodic handler: all further
    /// triggers). Returns `true` if an event was cancelled.
    ///
    /// # Errors
    /// [`TimerEventError::InvObjref`] after `unregister`.
    pub fn cancel_timer(&self) -> Result<bool, TimerEventError> {
        self.with_state(|_, st, id| {
            let h = st.handlers.get(&id).ok_or(TimerEventError::InvObjref)?;
            let pending =
                h.status == EventStatus::TimeSet && (h.armed.is_some() || h.period.is_some());
            if !pending {
                return Ok(false);
            }
            st.disarm(id);
            if let Some(h) = st.handlers.get_mut(&id) {
                h.status = EventStatus::TimeCleared;
                h.period = None;
                h.generation = h.generation.wrapping_add(1);
            }
            Ok(true)
        })
    }

    /// §2.3.1.4 `set_data(event_data)`: the data of every later trigger.
    ///
    /// # Errors
    /// [`TimerEventError::InvObjref`] after `unregister`.
    pub fn set_data(&self, event_data: AnyEvent) -> Result<(), TimerEventError> {
        self.with_state(|_, st, id| {
            let h = st.handlers.get_mut(&id).ok_or(TimerEventError::InvObjref)?;
            h.data = event_data;
            Ok(())
        })
    }
}

/// The ticks from `now` to `at` as a wait, at most [`MAX_WAIT`].
fn wait_for(now: TimeT, at: TimeT) -> Duration {
    let ticks = at.saturating_sub(now);
    let nanos = ticks.saturating_mul(1_000_000_000 / TICKS_PER_SECOND);
    Duration::from_nanos(nanos).min(MAX_WAIT)
}

/// The worker: pushes every trigger whose time has come, in time order.
fn run_worker(shared: &Shared) {
    let mut st = shared.lock();
    loop {
        if st.shutdown {
            return;
        }
        let Some((&(at, seq), &id)) = st.schedule.first_key_value() else {
            st = shared
                .wake
                .wait(st)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            continue;
        };
        let Some(now) = shared.now() else {
            // No clock reading: wait and read it again.
            st = shared
                .wake
                .wait_timeout(st, MAX_WAIT)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
            continue;
        };
        if at > now {
            st = shared
                .wake
                .wait_timeout(st, wait_for(now, at))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
            continue;
        }
        st.schedule.remove(&(at, seq));
        let Some(h) = st.handlers.get_mut(&id) else {
            continue;
        };
        h.armed = None;
        let generation = h.generation;
        let consumer = Arc::clone(&h.consumer);
        // §2.2.3.3: the actual time, and the delay after the requested
        // time as inaccuracy. A delay above the 48 bits of InaccuracyT
        // (about 325 days) is reported as the largest inaccuracy.
        let delay = (now - at).min(MAX_INACCURACY);
        let event = TimerEventT {
            utc: UtcT::new(now, delay, shared.clock.tdf_at(now)).unwrap_or_default(),
            event_data: h.data.clone(),
        };
        drop(st);
        let delivered = consumer.push(event.to_any()).is_ok();
        st = shared.lock();
        let Some(h) = st.handlers.get_mut(&id) else {
            continue;
        };
        if h.generation != generation {
            // Reprogrammed or cancelled during the push: the new setting
            // stands.
            continue;
        }
        match (delivered, h.period) {
            (true, Some(period)) => {
                // §2.2.3.2: a periodic event returns to ESTimeSet; the next
                // trigger is one period after the requested time, so the
                // period does not drift with the delay.
                h.status = EventStatus::TimeSet;
                h.generation = h.generation.wrapping_add(1);
                match at.checked_add(period) {
                    Some(next) => st.arm(id, next),
                    None => {
                        if let Some(h) = st.handlers.get_mut(&id) {
                            h.status = EventStatus::TimeCleared;
                            h.period = None;
                        }
                    }
                }
            }
            (true, None) => {
                h.status = EventStatus::Triggered;
                h.generation = h.generation.wrapping_add(1);
            }
            (false, _) => {
                // The consumer is disconnected (CosEventComm::Disconnected
                // is terminal), so a periodic handler stops as well.
                h.status = EventStatus::FailedTrigger;
                h.period = None;
                h.generation = h.generation.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use zerodds_corba_cos_event::comm::Disconnected;

    /// A push consumer that forwards every event to a channel, or refuses
    /// it once disconnected.
    struct ChannelConsumer {
        tx: Mutex<mpsc::Sender<(TimeT, AnyEvent)>>,
        connected: std::sync::atomic::AtomicBool,
    }

    impl PushConsumer for ChannelConsumer {
        fn push(&self, event: AnyEvent) -> Result<(), Disconnected> {
            if !self.connected.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(Disconnected);
            }
            let arrival = crate::time_base::current_time();
            let _ = self.tx.lock().unwrap().send((arrival, event));
            Ok(())
        }
        fn disconnect_push_consumer(&self) {
            self.connected
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn consumer() -> (Arc<ChannelConsumer>, mpsc::Receiver<(TimeT, AnyEvent)>) {
        let (tx, rx) = mpsc::channel();
        (
            Arc::new(ChannelConsumer {
                tx: Mutex::new(tx),
                connected: std::sync::atomic::AtomicBool::new(true),
            }),
            rx,
        )
    }

    fn data(tag: u8) -> AnyEvent {
        AnyEvent::new(String::from("IDL:demo/Tick:1.0"), alloc::vec![1, tag])
    }

    fn service() -> TimerEventService {
        TimerEventService::new(TimeService::default()).expect("worker")
    }

    /// Milliseconds as a relative UTO.
    fn ms(n: u64) -> Uto {
        Uto::new(n * TICKS_PER_SECOND / 1_000, 0, 0).expect("ok")
    }

    fn recv(rx: &mpsc::Receiver<(TimeT, AnyEvent)>) -> TimerEventT {
        let (_, any) = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("an event within 5 s");
        TimerEventT::from_any(&any).expect("TimerEventT")
    }

    fn wait_status(h: &TimerEventHandler, want: EventStatus) {
        for _ in 0..500 {
            if h.status().unwrap() == want {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("status stayed {:?}, wanted {want:?}", h.status().unwrap());
    }

    #[test]
    fn time_2_4_1_register_starts_cleared_and_schedules_nothing() {
        let svc = service();
        let (c, rx) = consumer();
        let before = crate::time_base::current_time();
        let h = svc.register(c, data(1)).unwrap();
        let after = crate::time_base::current_time();
        assert_eq!(h.status().unwrap(), EventStatus::TimeCleared);
        let (set, uto) = h.time_set().unwrap();
        assert!(!set);
        // "At initialization, the time in the handler is set to current time".
        assert!(uto.time() >= before && uto.time() <= after);
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        assert_eq!(svc.handler_count(), 1);
    }

    #[test]
    fn time_2_4_1_register_beyond_capacity_is_no_resources() {
        let svc = TimerEventService::with_capacity(TimeService::default(), 2).unwrap();
        let (c, _rx) = consumer();
        let a = svc.register(c.clone(), data(1)).unwrap();
        let _b = svc.register(c.clone(), data(2)).unwrap();
        assert_eq!(
            svc.register(c.clone(), data(3)).map(|_| ()),
            Err(TimerEventError::NoResources)
        );
        svc.unregister(&a).unwrap();
        assert!(svc.register(c, data(4)).is_ok());
    }

    #[test]
    fn time_2_2_3_2_status_triggered_after_one_shot() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Relative, ms(20)).unwrap();
        assert_eq!(h.status().unwrap(), EventStatus::TimeSet);
        let ev = recv(&rx);
        assert_eq!(ev.event_data, data(1));
        wait_status(&h, EventStatus::Triggered);
        assert!(!h.time_set().unwrap().0);
        // One-shot: no second event.
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn time_2_2_3_2_status_failed_trigger_when_push_fails() {
        let svc = service();
        let (c, rx) = consumer();
        c.disconnect_push_consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Relative, ms(10)).unwrap();
        wait_status(&h, EventStatus::FailedTrigger);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn time_2_2_3_2_periodic_never_triggered_stays_time_set() {
        // §2.2.3.2: "In case of TTPeriodic events, the status ESTriggered
        // never occurs. Upon successful triggering of a TTPeriodic event,
        // the status is set to ESTimeSet."
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(7)).unwrap();
        h.set_timer(TimeType::Periodic, ms(15)).unwrap();
        let mut requested = Vec::new();
        for _ in 0..4 {
            let ev = recv(&rx);
            requested.push(ev.utc.time - ev.utc.inaccuracy());
            assert_eq!(h.status().unwrap(), EventStatus::TimeSet);
        }
        // The requested times are one period apart.
        for pair in requested.windows(2) {
            assert_eq!(pair[1] - pair[0], 150_000);
        }
        assert!(h.cancel_timer().unwrap());
        assert_eq!(h.status().unwrap(), EventStatus::TimeCleared);
        // Drain an event that was in flight while cancelling, then none.
        let _ = rx.recv_timeout(Duration::from_millis(50));
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn time_2_3_1_1_time_set_returns_flag_and_uto() {
        let svc = TimerEventService::new(TimeService::default().with_tdf(60)).unwrap();
        let (c, _rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let at = crate::time_base::current_time() + 3_600 * TICKS_PER_SECOND;
        h.set_timer(TimeType::Absolute, Uto::new(at, 0, 0).unwrap())
            .unwrap();
        let (set, uto) = h.time_set().unwrap();
        assert!(set);
        assert_eq!(uto.time(), at);
        assert_eq!(uto.tdf(), 60);
        assert!(h.cancel_timer().unwrap());
        let (set, uto) = h.time_set().unwrap();
        assert!(!set);
        // "it always returns the current value of the timer".
        assert_eq!(uto.time(), at);
    }

    #[test]
    fn time_2_3_1_2_set_timer_relative_zero_reuses_last() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Relative, ms(10)).unwrap();
        let first = recv(&rx);
        let before = crate::time_base::current_time();
        h.set_timer(TimeType::Relative, Uto::new(0, 0, 0).unwrap())
            .unwrap();
        let (_, uto) = h.time_set().unwrap();
        // The zero reused the 10 ms of the first call.
        assert!(uto.time() >= before + 100_000);
        assert!(uto.time() <= crate::time_base::current_time() + 100_000);
        let second = recv(&rx);
        assert!(second.utc.time > first.utc.time);
    }

    #[test]
    fn time_2_3_1_2_set_timer_relative_zero_without_previous_is_bad_param() {
        let svc = service();
        let (c, _rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let zero = Uto::new(0, 0, 0).unwrap();
        assert_eq!(
            h.set_timer(TimeType::Relative, zero),
            Err(TimerEventError::BadParam)
        );
        assert_eq!(
            h.set_timer(TimeType::Periodic, zero),
            Err(TimerEventError::BadParam)
        );
        // An absolute time sets no relative time to reuse.
        h.set_timer(TimeType::Absolute, Uto::new(1, 0, 0).unwrap())
            .unwrap();
        assert_eq!(
            h.set_timer(TimeType::Relative, zero),
            Err(TimerEventError::BadParam)
        );
    }

    #[test]
    fn time_2_3_1_2_set_timer_overflow_is_bad_param() {
        let svc = service();
        let (c, _rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let huge = Uto::new(u64::MAX - 1, 0, 0).unwrap();
        assert_eq!(
            h.set_timer(TimeType::Relative, huge),
            Err(TimerEventError::BadParam)
        );
        assert_eq!(h.status().unwrap(), EventStatus::TimeCleared);
    }

    #[test]
    fn time_2_3_1_2_set_timer_absolute_fires_at_the_time() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let at = crate::time_base::current_time() + 50 * 10_000; // +50 ms
        h.set_timer(TimeType::Absolute, Uto::new(at, 0, 0).unwrap())
            .unwrap();
        let ev = recv(&rx);
        // Never before the requested time; the inaccuracy is the delay.
        assert!(ev.utc.time >= at);
        assert_eq!(ev.utc.time - ev.utc.inaccuracy(), at);
    }

    #[test]
    fn time_2_3_1_2_set_timer_cancels_the_previous_trigger() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Relative, ms(30)).unwrap();
        let far = crate::time_base::current_time() + 3_600 * TICKS_PER_SECOND;
        h.set_timer(TimeType::Absolute, Uto::new(far, 0, 0).unwrap())
            .unwrap();
        // The 30 ms trigger was replaced by the one in an hour.
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        assert_eq!(h.status().unwrap(), EventStatus::TimeSet);
    }

    #[test]
    fn time_2_3_1_3_cancel_timer_returns_true_only_for_a_pending_trigger() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        // Nothing set yet.
        assert!(!h.cancel_timer().unwrap());
        h.set_timer(TimeType::Relative, ms(200)).unwrap();
        assert!(h.cancel_timer().unwrap());
        assert_eq!(h.status().unwrap(), EventStatus::TimeCleared);
        assert!(!h.cancel_timer().unwrap());
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        // After the trigger went off there is nothing to cancel.
        h.set_timer(TimeType::Relative, ms(10)).unwrap();
        let _ = recv(&rx);
        wait_status(&h, EventStatus::Triggered);
        assert!(!h.cancel_timer().unwrap());
        assert_eq!(h.status().unwrap(), EventStatus::Triggered);
    }

    #[test]
    fn time_2_3_1_4_set_data_reaches_the_event() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Relative, ms(10)).unwrap();
        assert_eq!(recv(&rx).event_data, data(1));
        h.set_data(data(9)).unwrap();
        h.set_timer(TimeType::Relative, ms(10)).unwrap();
        assert_eq!(recv(&rx).event_data, data(9));
    }

    #[test]
    fn time_2_4_2_unregister_then_use_is_inv_objref() {
        let svc = service();
        let (c, _rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let copy = h.clone();
        svc.unregister(&h).unwrap();
        assert_eq!(copy.status(), Err(TimerEventError::InvObjref));
        assert_eq!(copy.time_set().map(|_| ()), Err(TimerEventError::InvObjref));
        assert_eq!(
            copy.set_timer(TimeType::Relative, ms(1)),
            Err(TimerEventError::InvObjref)
        );
        assert_eq!(copy.cancel_timer(), Err(TimerEventError::InvObjref));
        assert_eq!(copy.set_data(data(2)), Err(TimerEventError::InvObjref));
        assert_eq!(svc.unregister(&copy), Err(TimerEventError::InvObjref));
        assert_eq!(svc.handler_count(), 0);
    }

    #[test]
    fn time_2_4_2_unregister_cancels_the_pending_trigger() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Periodic, ms(20)).unwrap();
        svc.unregister(&h).unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn time_2_4_2_handler_of_another_service_is_inv_objref() {
        let a = service();
        let b = service();
        let (c, _rx) = consumer();
        let h = a.register(c, data(1)).unwrap();
        assert_eq!(b.unregister(&h), Err(TimerEventError::InvObjref));
        assert_eq!(h.status().unwrap(), EventStatus::TimeCleared);
        drop(a);
        // Its service is gone.
        assert_eq!(h.status(), Err(TimerEventError::InvObjref));
    }

    #[test]
    fn time_2_4_3_event_time_returns_uto() {
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        h.set_timer(TimeType::Relative, ms(10)).unwrap();
        let ev = recv(&rx);
        let uto = svc.event_time(&ev);
        assert_eq!(uto.time(), ev.utc.time);
        assert_eq!(uto.inaccuracy(), ev.utc.inaccuracy());
        assert_eq!(uto.utc_time(), ev.utc);
    }

    #[test]
    fn time_2_2_3_3_inaccuracy_is_requested_minus_actual() {
        // §2.2.3.3: `time` is the actual trigger time, the inaccuracy the
        // difference to the requested time.
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let before = crate::time_base::current_time();
        h.set_timer(TimeType::Relative, ms(30)).unwrap();
        let (_, armed) = h.time_set().unwrap();
        let requested = armed.time();
        assert!(requested >= before + 300_000);
        let (arrival, any) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let ev = TimerEventT::from_any(&any).unwrap();
        assert_eq!(ev.utc.time - requested, ev.utc.inaccuracy());
        assert!(ev.utc.time <= arrival);
    }

    #[test]
    #[allow(clippy::print_stdout)]
    fn time_2_2_2_trigger_delay_is_reported_and_bounded() {
        // §2.2.2: events are pushed "within a reasonable interval around
        // the requested event time". Measures the delay of 20 triggers of
        // 5 ms; on an idle host it is well below a millisecond, the bound
        // only fails a broken scheduler (a trigger a period late or more).
        let svc = service();
        let (c, rx) = consumer();
        let h = svc.register(c, data(1)).unwrap();
        let mut worst = 0;
        for _ in 0..20 {
            h.set_timer(TimeType::Relative, ms(5)).unwrap();
            let ev = recv(&rx);
            worst = worst.max(ev.utc.inaccuracy());
        }
        // 100 ms in 100 ns ticks.
        assert!(worst < 1_000_000, "worst trigger delay {worst} ticks");
        std::println!("time_2_2_2 worst trigger delay: {} us", worst / 10);
    }

    #[test]
    fn time_2_2_3_timer_event_t_any_roundtrip_both_byte_orders() {
        let ev = TimerEventT {
            utc: UtcT::new(0x0102_0304_0506_0708, 0x0A0B_0C0D_0E0F, -330).unwrap(),
            event_data: AnyEvent::new(String::from("IDL:demo/Tick:1.0"), alloc::vec![1, 2, 3]),
        };
        let any = ev.to_any();
        assert_eq!(any.type_id, TIMER_EVENT_T_TYPE_ID);
        assert_eq!(TimerEventT::from_any(&any), Ok(ev.clone()));
        // The same value written big-endian by hand: flag 0, padding to
        // offset 8, the UtcT, the string (17 characters + NUL), padding to
        // 4, the octet sequence.
        let mut be = alloc::vec![0u8; 8];
        be.extend_from_slice(&0x0102_0304_0506_0708u64.to_be_bytes());
        be.extend_from_slice(&ev.utc.inacclo.to_be_bytes());
        be.extend_from_slice(&ev.utc.inacchi.to_be_bytes());
        be.extend_from_slice(&(-330i16).to_be_bytes());
        be.extend_from_slice(&18u32.to_be_bytes());
        be.extend_from_slice(b"IDL:demo/Tick:1.0\0");
        while !be.len().is_multiple_of(4) {
            be.push(0);
        }
        be.extend_from_slice(&3u32.to_be_bytes());
        be.extend_from_slice(&[1, 2, 3]);
        let big = AnyEvent::new(String::from(TIMER_EVENT_T_TYPE_ID), be);
        assert_eq!(TimerEventT::from_any(&big), Ok(ev));
    }

    #[test]
    fn time_2_2_3_timer_event_t_malformed_any_is_marshal() {
        let good = TimerEventT {
            utc: UtcT::new(5, 1, 0).unwrap(),
            event_data: data(1),
        }
        .to_any();
        let wrong_id = AnyEvent::new(String::from("IDL:demo/Other:1.0"), good.data.clone());
        assert_eq!(TimerEventT::from_any(&wrong_id), Err(Marshal));
        let mut flag = good.clone();
        flag.data[0] = 7;
        assert_eq!(TimerEventT::from_any(&flag), Err(Marshal));
        let mut short = good.clone();
        short.data.truncate(short.data.len() - 1);
        assert_eq!(TimerEventT::from_any(&short), Err(Marshal));
        let mut long = good.clone();
        long.data.push(0);
        assert_eq!(TimerEventT::from_any(&long), Err(Marshal));
    }
}
