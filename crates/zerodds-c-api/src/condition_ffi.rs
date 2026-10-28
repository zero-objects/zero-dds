// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Conditions + WaitSet C-FFI (Spec §2.2.2.1.2 + DDS-PSM-Cxx §7.5.10).
//!
//! Architecture:
//! - `ZeroDdsGuardCondition`: user-toggleable trigger flag.
//! - `ZeroDdsStatusCondition`: bound to an entity + status mask.
//! - `ZeroDdsReadCondition`: bound to a DataReader + sample/view/instance filter.
//! - `ZeroDdsQueryCondition`: ReadCondition + filter expression (RC1: passthrough).
//! - `ZeroDdsWaitSet`: container of conditions. `wait()` is event-driven: it
//!   blocks on a data-availability `Condvar` that is signalled by a
//!   `core::task::Waker` registered on the reader behind every attached
//!   Read/Query condition (forwarded from the DCPS `async_waker` hook). No
//!   fixed-interval poll loop on the data path. Guard/Status conditions, which
//!   have no waker, are observed within a short wakeup cap.
//!
//! The same signal is available as a descriptor through
//! `zerodds_data_notify_fd`, for a caller whose event loop already has a wait
//! point of its own and cannot block in `zerodds_waitset_wait`.
//!
//! Triggering:
//! - GuardCondition: `set_trigger_value`.
//! - StatusCondition: active if `enabled_statuses & current_status_mask != 0`.
//!   RC1 collects coarse trigger bits via reader/writer status polls (matched count
//!   not 0, sample_lost > 0, ...).
//! - ReadCondition: active if the DataReader has samples matching the
//!   (sample_states, view_states, instance_states) mask. The channel is drained
//!   into the read cache and the cache is evaluated; a sample arriving wakes any
//!   WaitSet blocked on the condition via the registered data waker.
//! - QueryCondition: like ReadCondition, plus the filter expression.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use core::task::{RawWaker, RawWakerVTable, Waker};
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::time::{Duration, Instant};

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDataReader, ZeroDdsDataWriter};
use crate::ffi_helpers::copy_list_out;

// ---------------------------------------------------------------------------
// Event-driven data-availability signal (no busy-poll)
// ---------------------------------------------------------------------------
//
// The DCPS layer already provides an event-driven hook: after every
// `sample_tx.send` on a user reader it calls `wake_async_waker(&waker)` on the
// reader's `async_waker` slot (see `crates/dcps/src/runtime.rs`,
// `recv_user_data_loop` Data/HeartBeat arms). Unlike the listener path, the
// waker fires *in addition* to the MPSC send — so the sample stays in the
// channel for `zerodds_dr_take`/`zerodds_dr_read` (lossless) while the waker
// signals "data arrived".
//
// We forward that signal into a process-global [`DataEvent`] (a `Condvar` plus
// a monotonic generation counter). When a ReadCondition / QueryCondition (or a
// reader-bound StatusCondition) is attached to a WaitSet, we register a
// `core::task::Waker` on the underlying reader via
// `DcpsRuntime::register_user_reader_waker`; waking it bumps the generation and
// notifies the condvar. `zerodds_waitset_wait` blocks on this condvar
// (`wait_timeout`, bounded by the caller deadline) and re-checks the condition
// trigger values on every wake — strictly event-driven, no fixed-interval poll
// loop.

/// Process-global data-availability event. A generation counter lets a waiter
/// detect a wake that raced ahead of its `wait_timeout` call (no lost wakeup).
/// The single source of truth is the mutex-protected `gen`; the waiter
/// snapshots it under the lock and only blocks if it has not moved since.
struct DataEvent {
    /// Lock anchor for the condvar; the protected value is the wake generation.
    wake_gen: Mutex<u64>,
    cvar: Condvar,
    /// The same signal as a readable descriptor, for callers with an event
    /// loop of their own.
    ///
    /// A `Condvar` can only be waited on by a thread that has nothing else to
    /// do. An `epoll`, `kqueue`, `select`, `asyncio`, `libuv` or .NET loop
    /// already has a wait point and cannot add one; without a descriptor such a
    /// caller has to spend a thread parked in `zerodds_waitset_wait` and hand
    /// the wake across — which is why the callback path in
    /// `docs/specs/zerodds-listener-callbacks-1.1.md` §4.1 settles for a poll
    /// the caller drives.
    #[cfg(unix)]
    pipe: Option<NotifyPipe>,
}

/// A self-pipe carrying the data-availability signal to a caller's event loop.
///
/// `UnixStream::pair` rather than `libc::pipe` or `eventfd`: it is in `std`, so
/// this crate takes no new dependency, and both ends are ordinary descriptors
/// that every readiness interface accepts. `eventfd` would be one descriptor
/// instead of two, but it is Linux only and this crate also builds for macOS
/// and the BSDs.
#[cfg(unix)]
struct NotifyPipe {
    /// The end the caller polls. Never written to here.
    read_end: std::os::unix::net::UnixStream,
    /// The end [`DataEvent::signal`] writes one byte to.
    write_end: std::os::unix::net::UnixStream,
    /// Whether an unread byte is already in the pipe.
    ///
    /// Coalescing, so a burst of samples does not fill the buffer and so a
    /// caller that reads one byte per wake does not fall behind by one wake per
    /// sample. The pipe carries "something arrived", never a count.
    armed: AtomicBool,
}

impl DataEvent {
    /// Signals that data became available on some reader. Bumps the generation
    /// and wakes every WaitSet blocked in `wait_timeout`.
    fn signal(&self) {
        if let Ok(mut g) = self.wake_gen.lock() {
            *g = g.wrapping_add(1);
        }
        self.cvar.notify_all();
        self.raise_fd();
    }

    /// Puts one byte in the pipe if it is not already armed.
    #[cfg(unix)]
    fn raise_fd(&self) {
        use std::io::Write as _;
        let Some(pipe) = self.pipe.as_ref() else {
            return;
        };
        // `swap` before the write: two signals racing here must produce at most
        // one byte, and the loser must not return before the winner's byte is
        // in the pipe — which it does not, because the loser sees `true` only
        // after the winner set it, and the winner writes before any reader can
        // disarm.
        if pipe.armed.swap(true, Ordering::AcqRel) {
            return;
        }
        // A full pipe already holds a wake the caller has not collected, so
        // `EAGAIN` here is the state we wanted anyway. Any other error means
        // the descriptor is unusable; disarm so a later signal tries again
        // rather than leaving the caller with a pipe that never fires.
        if (&pipe.write_end).write(&[1u8]).is_err() {
            pipe.armed.store(false, Ordering::Release);
        }
    }

    /// No descriptor off Unix; the condvar is the only wait point there.
    #[cfg(not(unix))]
    #[allow(clippy::unused_self)]
    fn raise_fd(&self) {}

    /// Empties the pipe and disarms it.
    ///
    /// Called by every `zerodds_waitset_wait` before it evaluates the trigger
    /// values, so the descriptor reflects "a signal has arrived since the last
    /// look" and not "a signal has ever arrived". Disarming *before* the read
    /// is deliberate: a signal that lands between the two re-arms and writes,
    /// and the caller wakes again — one spurious wake, never a missed one.
    #[cfg(unix)]
    fn drain_fd(&self) {
        use std::io::Read as _;
        let Some(pipe) = self.pipe.as_ref() else {
            return;
        };
        pipe.armed.store(false, Ordering::Release);
        let mut sink = [0u8; 64];
        while let Ok(n) = (&pipe.read_end).read(&mut sink) {
            if n < sink.len() {
                break;
            }
        }
    }

    /// No descriptor off Unix.
    #[cfg(not(unix))]
    #[allow(clippy::unused_self)]
    fn drain_fd(&self) {}
}

fn data_event() -> &'static Arc<DataEvent> {
    use std::sync::OnceLock;
    static EV: OnceLock<Arc<DataEvent>> = OnceLock::new();
    EV.get_or_init(|| {
        Arc::new(DataEvent {
            wake_gen: Mutex::new(0),
            cvar: Condvar::new(),
            #[cfg(unix)]
            pipe: new_notify_pipe(),
        })
    })
}

/// Builds the self-pipe, or `None` if the process cannot spare two descriptors.
///
/// A failure here costs the descriptor and nothing else: the condvar path is
/// untouched, `zerodds_waitset_wait` behaves as before, and
/// `zerodds_data_notify_fd` reports it rather than handing back a descriptor
/// that never fires.
#[cfg(unix)]
fn new_notify_pipe() -> Option<NotifyPipe> {
    let (read_end, write_end) = std::os::unix::net::UnixStream::pair().ok()?;
    // Non-blocking on both ends. The writer runs in the receive path and must
    // never block there; the reader is drained in a loop that stops on
    // `WouldBlock`.
    read_end.set_nonblocking(true).ok()?;
    write_end.set_nonblocking(true).ok()?;
    Some(NotifyPipe {
        read_end,
        write_end,
        armed: AtomicBool::new(false),
    })
}

// --- core::task::Waker built from an Arc<DataEvent> -------------------------
//
// We cannot use `std::task::Wake` (would need an unstable/`Arc<dyn Wake>`
// shape mismatch with the runtime API which wants a `core::task::Waker`), so a
// hand-rolled RawWaker vtable over `Arc<DataEvent>` is used. Each clone bumps
// the Arc strong count; wake/drop release it.

// SAFETY: RawWaker vtable clone — `p` is a live `Arc<DataEvent>` raw pointer; bumps the strong count without dropping the original.
unsafe fn de_clone(p: *const ()) -> RawWaker {
    // SAFETY: `p` is an `Arc<DataEvent>` raw pointer kept alive by the slot.
    let arc = unsafe { Arc::from_raw(p as *const DataEvent) };
    let cloned = Arc::clone(&arc);
    core::mem::forget(arc); // do not drop the original
    RawWaker::new(Arc::into_raw(cloned) as *const (), &DE_VTABLE)
}

// SAFETY: RawWaker vtable wake — `p` is a live `Arc<DataEvent>` raw pointer; consumes exactly one strong ref.
unsafe fn de_wake(p: *const ()) {
    // SAFETY: consumes one strong ref (by-value wake).
    let arc = unsafe { Arc::from_raw(p as *const DataEvent) };
    arc.signal();
    // arc dropped here → releases this ref.
}

// SAFETY: RawWaker vtable wake-by-ref — `p` is a live `Arc<DataEvent>` raw pointer; borrows it without consuming a ref.
unsafe fn de_wake_by_ref(p: *const ()) {
    // SAFETY: borrow without consuming the ref.
    let arc = unsafe { Arc::from_raw(p as *const DataEvent) };
    arc.signal();
    core::mem::forget(arc);
}

// SAFETY: RawWaker vtable drop — `p` is a live `Arc<DataEvent>` raw pointer; releases exactly one strong ref.
unsafe fn de_drop(p: *const ()) {
    // SAFETY: releases one strong ref.
    drop(unsafe { Arc::from_raw(p as *const DataEvent) });
}

static DE_VTABLE: RawWakerVTable = RawWakerVTable::new(de_clone, de_wake, de_wake_by_ref, de_drop);

/// Builds a `core::task::Waker` that, when woken, signals the global
/// [`DataEvent`]. The returned waker owns one strong ref to the event Arc.
fn make_data_waker() -> Waker {
    let arc = Arc::clone(data_event());
    let raw = RawWaker::new(Arc::into_raw(arc) as *const (), &DE_VTABLE);
    // SAFETY: the vtable upholds the Waker contract (clone/wake/drop manage the
    // Arc strong count correctly).
    unsafe { Waker::from_raw(raw) }
}

/// Registers the global data-availability waker on the reader behind a
/// Read/Query/Status condition, so that a sample arriving on that reader wakes
/// any WaitSet blocked on it. Idempotent and NULL-tolerant.
///
/// # Safety
/// `cond` is NULL or a valid box-allocated condition pointer.
unsafe fn register_condition_data_waker(cond: *mut core::ffi::c_void) {
    // SAFETY: condition_kind is NULL-tolerant; reader pointers come from
    // sub_create_datareader (Box::into_raw) and stay valid until destroy.
    unsafe {
        let reader = match condition_kind(cond) {
            Some(ConditionKind::Read) => (*(cond as *const ZeroDdsReadCondition)).reader,
            Some(ConditionKind::Query) => (*(cond as *const ZeroDdsQueryCondition)).reader,
            _ => return,
        };
        if reader.is_null() {
            return;
        }
        let drr = &*reader;
        // A reader not registered with the runtime yet takes the waker at
        // its registration (DDS 1.4 §2.2.2.1.1.7).
        drr.lifecycle.want_waker();
        if drr.lifecycle.is_registered() {
            register_reader_data_waker(drr);
        }
    }
}

/// Sets the global data-availability waker on the runtime slot of `drr`.
pub(crate) fn register_reader_data_waker(drr: &ZeroDdsDataReader) {
    drr.rt
        .register_user_reader_waker(drr.eid, Some(make_data_waker()));
}

// ---------------------------------------------------------------------------
// Condition types
// ---------------------------------------------------------------------------

/// Tag to know which condition variant is behind `*mut c_void`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionKind {
    /// GuardCondition.
    Guard = 1,
    /// StatusCondition.
    Status = 2,
    /// ReadCondition.
    Read = 3,
    /// QueryCondition.
    Query = 4,
}

/// Header in every condition boxed struct, so that the WaitSet can
/// recognize the type at runtime.
#[repr(C)]
struct ConditionHeader {
    kind: ConditionKind,
}

/// GuardCondition (Spec §2.2.2.1.2.1.6).
///
/// `#[repr(C)]` is MANDATORY: the generic `condition_kind()` dispatcher reads
/// the `ConditionKind` discriminant via a `*const ConditionHeader` cast,
/// i.e. the `header` field MUST be guaranteed to be at offset 0. Without `repr(C)`
/// `repr(Rust)` may reorder the fields → `header` lands elsewhere → the
/// dispatcher reads garbage as the kind, casts to the wrong type and
/// dereferences a junk pointer (Linux SIGSEGV; macOS happened to be right by chance).
///
/// On the C ABI this stays an **opaque** `void*`/pointer handle: excluded in
/// `cbindgen.toml` + forward-declared in the header preamble. `repr(C)`
/// is ONLY for the internal Rust layout — cbindgen must NOT emit the fields into
/// `zerodds.h` (otherwise the `String`/`Vec` follow-up fields of
/// `QueryCondition` become incomplete C types).
#[repr(C)]
pub struct ZeroDdsGuardCondition {
    /// Header. Layout-compatible with `ConditionHeader` for
    /// condition-kind discrimination via `condition_kind()`.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// User trigger flag.
    trigger: AtomicBool,
}

/// StatusCondition (Spec §2.2.2.1.2.1.4). In RC1 bound to `Entity = void*`
/// — no status-polling plumb (follow-up WP).
///
/// `#[repr(C)]` MANDATORY — `header` at offset 0 (see `ZeroDdsGuardCondition`); opaque on the C ABI (cbindgen.toml exclude + preamble forward decl).
#[repr(C)]
pub struct ZeroDdsStatusCondition {
    /// Header. Layout-compatible for condition-kind discrimination.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// `entity` pointer (DataReader / DataWriter / Topic / Participant).
    #[allow(dead_code)]
    entity: *mut core::ffi::c_void,
    /// Enabled status bits (bitmask).
    enabled_statuses: AtomicU32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsStatusCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsStatusCondition {}

/// ReadCondition (Spec §2.2.2.5.8).
///
/// `#[repr(C)]` MANDATORY — `header` at offset 0 (see `ZeroDdsGuardCondition`).
/// Without this `zerodds_condition_get_trigger_value` read the `reader` pointer from the
/// wrong offset → SIGSEGV (F-PSM-CXX-readcond-segv). C ABI: opaque (cbindgen.toml exclude + preamble forward decl).
#[repr(C)]
pub struct ZeroDdsReadCondition {
    /// Header. Layout-compatible for condition-kind discrimination.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// Bound DataReader.
    reader: *mut ZeroDdsDataReader,
    /// Sample-state mask.
    sample_states: u32,
    /// View-state mask.
    view_states: u32,
    /// Instance-state mask.
    instance_states: u32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsReadCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsReadCondition {}

/// QueryCondition (Spec §2.2.2.5.9).
///
/// `#[repr(C)]` MANDATORY — `header` at offset 0 (see `ZeroDdsGuardCondition`).
/// (`String`/`Vec` as follow-up fields are ok: accessed only via a Rust cast, never
/// by-value across the FFI boundary.) C ABI: opaque (cbindgen.toml exclude + preamble forward decl).
#[repr(C)]
pub struct ZeroDdsQueryCondition {
    /// Header. Layout-compatible for condition-kind discrimination.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// Bound DataReader.
    reader: *mut ZeroDdsDataReader,
    /// Sample/view/instance state.
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
    /// Filter expression. Behind a lock: `zerodds_querycondition_set_expression`
    /// changes it while a wait may be evaluating it.
    expression: std::sync::RwLock<alloc::string::String>,
    /// Filter parameters, behind a lock for `zerodds_querycondition_set_parameters`.
    parameters: std::sync::RwLock<Vec<alloc::string::String>>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsQueryCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsQueryCondition {}

// ---------------------------------------------------------------------------
// Condition trigger helpers — drain the channel into the cache, evaluate masks
// ---------------------------------------------------------------------------

fn drain_channel_into_cache(drr: &ZeroDdsDataReader) {
    // Same resolution the take/read path uses (`InstanceTracker`
    // view_state/instance_state/generation counts baked once at arrival),
    // not a second, unresolved trigger-only copy — see
    // `subscriber_ffi::resolve_instances_and_ownership`'s doc comment for
    // why re-baking an already-cached entry would corrupt that state.
    let _ = crate::subscriber_ffi::drain_fresh_into_read_cache(drr, usize::MAX);
}

fn cache_has_matching(
    drr: &ZeroDdsDataReader,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> bool {
    let cache = match drr.read_cache.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    cache.iter().any(|(_sample, st, info)| {
        crate::subscriber_ffi::entry_in_states(
            *st,
            info,
            (sample_states, view_states, instance_states),
        )
    })
}

/// The expression of a QueryCondition, parsed, with its parameters and the type it reads the
/// fields of a sample through (DDS 1.4 §2.2.2.5.9, Annex B).
pub(crate) struct Query {
    /// The condition; none when the expression only orders.
    expr: Option<zerodds_sql_filter::Expr>,
    /// The fields of `ORDER BY` (Annex B.2 `QueryExpression`).
    order_by: Vec<alloc::string::String>,
    values: Vec<zerodds_sql_filter::Value>,
    typed: Option<Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>>,
    /// The expression names a field and the reader has no type to find it in. No sample can
    /// be evaluated; a read that would have to evaluate one is refused (see `blocked`).
    blocked: bool,
}

impl Query {
    /// The query of `expression` over `drr`. `BadParameter` when the expression does not
    /// parse. An expression that names a field on a reader without a type
    /// (`zerodds_dr_set_type_objects`) is `blocked`: no sample passes it, and a read that
    /// finds a sample to evaluate returns `PreconditionNotMet` instead of passing it.
    pub(crate) fn prepare(
        drr: &ZeroDdsDataReader,
        expression: &str,
        params: &[alloc::string::String],
    ) -> Result<Self, ZeroDdsStatus> {
        let parsed = zerodds_sql_filter::parse_query_expression(expression)
            .map_err(|_| ZeroDdsStatus::BadParameter)?;
        let (expr, order_by) = (parsed.condition, parsed.order_by);
        let typed = drr
            .query_type
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .or_else(|| drr.cft_filter.as_ref().and_then(|f| f.typed.clone()));
        // A field the reader has no type for cannot be read, in the condition
        // or in `ORDER BY`.
        let blocked = typed.is_none()
            && (expr
                .as_ref()
                .is_some_and(zerodds_sql_filter::Expr::references_fields)
                || !order_by.is_empty());
        let values = params
            .iter()
            .map(|p| zerodds_sql_filter::Value::from_parameter(p))
            .collect();
        Ok(Self {
            expr,
            order_by,
            values,
            typed,
            blocked,
        })
    }

    /// Whether the expression orders (`ORDER BY`).
    pub(crate) fn orders(&self) -> bool {
        !self.order_by.is_empty()
    }

    /// The `ORDER BY` key of a sample: the value of each field, `null` for
    /// a notice without data or a field the sample lacks.
    pub(crate) fn order_key(
        &self,
        sample: &zerodds_dcps::runtime::UserSample,
    ) -> Vec<zerodds_sql_filter::Value> {
        use zerodds_dcps::runtime::UserSample;
        use zerodds_sql_filter::{RowAccess, Value};
        let row = match (sample, &self.typed) {
            (
                UserSample::Alive {
                    payload,
                    representation,
                    big_endian,
                    ..
                },
                Some(typed),
            ) => typed
                .read_body(
                    payload.as_ref(),
                    zerodds_dcps::serialized_type::read_body_representation(*representation),
                    *big_endian,
                )
                .ok(),
            _ => None,
        };
        self.order_by
            .iter()
            .map(|field| {
                row.as_ref()
                    .and_then(|row| crate::entities::TypedRow(row).get(field))
                    .unwrap_or(Value::Null)
            })
            .collect()
    }

    /// Whether the expression cannot be evaluated for want of the reader's type.
    pub(crate) fn blocked(&self) -> bool {
        self.blocked
    }

    /// Whether `sample` satisfies the expression. A body that does not decode, or a field the
    /// expression cannot resolve, does not.
    pub(crate) fn matches(&self, sample: &zerodds_dcps::runtime::UserSample) -> bool {
        use zerodds_dcps::runtime::UserSample;
        if self.blocked {
            return matches!(sample, UserSample::Lifecycle { .. });
        }
        match sample {
            UserSample::Alive {
                payload,
                representation,
                big_endian,
                ..
            } => match &self.typed {
                Some(typed) => {
                    let representation =
                        zerodds_dcps::serialized_type::read_body_representation(*representation);
                    match typed.read_body(payload.as_ref(), representation, *big_endian) {
                        Ok(row) => self.expr.as_ref().is_none_or(|expr| {
                            expr.evaluate(&crate::entities::TypedRow(&row), &self.values)
                                .unwrap_or(false)
                        }),
                        Err(_) => false,
                    }
                }
                None => {
                    struct EmptyRow;
                    impl zerodds_sql_filter::RowAccess for EmptyRow {
                        fn get(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
                            None
                        }
                    }
                    self.expr
                        .as_ref()
                        .is_none_or(|expr| expr.evaluate(&EmptyRow, &self.values).unwrap_or(false))
                }
            },
            UserSample::Lifecycle { .. } => true,
            // Not a sample — see `drain_channel_into_cache`.
            UserSample::CoherentSetEnd { .. } => false,
        }
    }
}

fn cache_has_matching_query(
    drr: &ZeroDdsDataReader,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
    expression: &str,
    params: &[alloc::string::String],
) -> bool {
    let Ok(query) = Query::prepare(drr, expression, params) else {
        return false;
    };
    let cache = match drr.read_cache.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    cache.iter().any(|(sample, st, info)| {
        crate::subscriber_ffi::entry_in_states(
            *st,
            info,
            (sample_states, view_states, instance_states),
        ) && query.matches(sample)
    })
}

/// The query of a QueryCondition, or `None` for a ReadCondition.
///
/// # Safety
/// `c` valid or NULL, as for [`condition_state_masks`].
pub(crate) unsafe fn condition_query(
    c: *const core::ffi::c_void,
) -> Result<Option<Query>, ZeroDdsStatus> {
    // SAFETY: see fn # Safety doc — the kind is read after the NULL check.
    unsafe {
        match condition_kind(c) {
            Some(ConditionKind::Query) => {
                let q = &*(c as *const ZeroDdsQueryCondition);
                if q.reader.is_null() {
                    return Err(ZeroDdsStatus::BadHandle);
                }
                let (expression, parameters) = query_snapshot(q);
                Query::prepare(&*q.reader, &expression, &parameters).map(Some)
            }
            Some(ConditionKind::Read) => Ok(None),
            _ => Err(ZeroDdsStatus::BadParameter),
        }
    }
}

// ---------------------------------------------------------------------------
// Condition-Helpers
// ---------------------------------------------------------------------------

/// Returns the state masks (sample_states, view_states, instance_states)
/// of a ReadCondition or QueryCondition. Other kinds → None.
///
/// # Safety
/// `c` valid or NULL.
pub unsafe fn condition_state_masks(c: *const core::ffi::c_void) -> Option<(u32, u32, u32)> {
    // SAFETY: see fn # Safety doc — c NULL-tolerant; condition_kind() checks NULL
    // and returns the ConditionKind; then a layout cast per kind.
    unsafe {
        let kind = condition_kind(c)?;
        match kind {
            ConditionKind::Read => {
                let r = &*(c as *const ZeroDdsReadCondition);
                Some((r.sample_states, r.view_states, r.instance_states))
            }
            ConditionKind::Query => {
                let q = &*(c as *const ZeroDdsQueryCondition);
                Some((q.sample_states, q.view_states, q.instance_states))
            }
            _ => None,
        }
    }
}

/// Reads the `ConditionKind` header. NULL-tolerant.
///
/// # Safety
/// `p` is NULL or points to a box-allocated condition whose
/// first field is a `ConditionHeader` (layout via `#[repr(C)]`).
unsafe fn condition_kind(p: *const core::ffi::c_void) -> Option<ConditionKind> {
    if p.is_null() {
        return None;
    }
    // SAFETY: NULL check above; caller pledge for a box-allocated condition.
    let hdr = unsafe { &*(p as *const ConditionHeader) };
    Some(hdr.kind)
}

/// Reads the current trigger status of a condition (Spec §2.2.2.1.2.1.1).
///
/// # Safety
/// `c` valid or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_condition_get_trigger_value(c: *const core::ffi::c_void) -> bool {
    // SAFETY: see fn # Safety doc — c NULL-tolerant; condition_kind checks NULL and
    // returns the ConditionKind; then a layout cast per kind. r.reader/q.reader
    // come from sub_create_datareader (Box::into_raw).
    unsafe {
        let kind = match condition_kind(c) {
            Some(k) => k,
            None => return false,
        };
        match kind {
            ConditionKind::Guard => (*(c as *const ZeroDdsGuardCondition))
                .trigger
                .load(Ordering::SeqCst),
            ConditionKind::Status => {
                // DDS 1.4 §2.2.2.1.6: the trigger is the entity's
                // StatusChangedFlags masked by the enabled statuses. It was
                // `enabled_statuses != 0`: a WaitSet holding a
                // StatusCondition returned at once, whatever had changed.
                let sc = &*(c as *const ZeroDdsStatusCondition);
                let enabled = sc.enabled_statuses.load(Ordering::SeqCst);
                enabled != 0 && status_changes_of(sc.entity) & enabled != 0
            }
            ConditionKind::Read => {
                let r = &*(c as *const ZeroDdsReadCondition);
                if r.reader.is_null() {
                    return false;
                }
                let drr = &*r.reader;
                // Spec §2.2.2.5.8: trigger_value true if the reader has samples that
                // match the (sample_states, view_states, instance_states) mask.
                //
                // The cache first, and the channel only if the cache does not
                // already answer. A drain takes two locks and, on a reader with
                // a content filter, evaluates the filter over every sample it
                // moves — work this question does not need when the answer is
                // already in hand. A WaitSet over N readers asks all N on every
                // wake, and the ones that are already ready are the common case.
                if cache_has_matching(drr, r.sample_states, r.view_states, r.instance_states) {
                    return true;
                }
                drain_channel_into_cache(drr);
                cache_has_matching(drr, r.sample_states, r.view_states, r.instance_states)
            }
            ConditionKind::Query => {
                let q = &*(c as *const ZeroDdsQueryCondition);
                if q.reader.is_null() {
                    return false;
                }
                let drr = &*q.reader;
                // Same order as the ReadCondition above, and it matters more
                // here: the filter expression is evaluated per sample on top of
                // the drain's own work.
                let (expression, parameters) = query_snapshot(q);
                if cache_has_matching_query(
                    drr,
                    q.sample_states,
                    q.view_states,
                    q.instance_states,
                    &expression,
                    &parameters,
                ) {
                    return true;
                }
                drain_channel_into_cache(drr);
                // Spec §2.2.2.5.9: additionally evaluate the filter expression.
                cache_has_matching_query(
                    drr,
                    q.sample_states,
                    q.view_states,
                    q.instance_states,
                    &expression,
                    &parameters,
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GuardCondition
// ---------------------------------------------------------------------------

/// Creates a GuardCondition.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_guardcondition_create() -> *mut ZeroDdsGuardCondition {
    Box::into_raw(Box::new(ZeroDdsGuardCondition {
        header: ConditionHeader {
            kind: ConditionKind::Guard,
        },
        trigger: AtomicBool::new(false),
    }))
}

/// Deletes a GuardCondition.
///
/// # Safety
/// `g` must come from `zerodds_guardcondition_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_guardcondition_destroy(g: *mut ZeroDdsGuardCondition) {
    if !g.is_null() {
        // SAFETY: see fn # Safety doc — g from zerodds_guardcondition_create.
        let _ = unsafe { Box::from_raw(g) };
    }
}

/// Sets the trigger value.
///
/// # Safety
/// `g` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_guardcondition_set_trigger_value(
    g: *mut ZeroDdsGuardCondition,
    v: bool,
) -> c_int {
    if g.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — g NULL-checked above.
    unsafe { (*g).trigger.store(v, Ordering::SeqCst) };
    // Wake any WaitSet blocked in `wait_timeout` so a guard flip (e.g. a
    // CancellationToken trip in the C# binding) is observed immediately,
    // event-driven, rather than at the next wakeup-cap tick.
    if v {
        data_event().signal();
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// StatusCondition
// ---------------------------------------------------------------------------

/// Creates a StatusCondition for an entity. RC1: allows any
/// `*mut c_void` as the entity slot — the match logic reads the status per
/// entity type via the listener FFI (follow-up WP).
///
/// # Safety
/// `entity` valid or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_entity_get_statuscondition(
    entity: *mut core::ffi::c_void,
) -> *mut ZeroDdsStatusCondition {
    Box::into_raw(Box::new(ZeroDdsStatusCondition {
        header: ConditionHeader {
            kind: ConditionKind::Status,
        },
        entity,
        enabled_statuses: AtomicU32::new(0xFFFF_FFFF),
    }))
}

/// Enables the status bits set in `mask`.
///
/// # Safety
/// `c` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_statuscondition_set_enabled_statuses(
    c: *mut ZeroDdsStatusCondition,
    mask: u32,
) -> c_int {
    if c.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above.
    unsafe { (*c).enabled_statuses.store(mask, Ordering::SeqCst) };
    ZeroDdsStatus::Ok as c_int
}

/// Reads the enabled status bits.
///
/// # Safety
/// `c` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_statuscondition_get_enabled_statuses(
    c: *mut ZeroDdsStatusCondition,
) -> u32 {
    if c.is_null() {
        return 0;
    }
    // SAFETY: see fn # Safety doc — c NULL-checked above.
    unsafe { (*c).enabled_statuses.load(Ordering::SeqCst) }
}

/// Deletes a StatusCondition.
///
/// # Safety
/// `c` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_statuscondition_destroy(c: *mut ZeroDdsStatusCondition) {
    if !c.is_null() {
        // SAFETY: see fn # Safety doc — c from zerodds_entity_get_statuscondition.
        let _ = unsafe { Box::from_raw(c) };
    }
}

// ---------------------------------------------------------------------------
// ReadCondition / QueryCondition
// ---------------------------------------------------------------------------

/// Creates a ReadCondition (Spec §2.2.2.5.2.4).
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_create_readcondition(
    dr: *mut ZeroDdsDataReader,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> *mut ZeroDdsReadCondition {
    if dr.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(ZeroDdsReadCondition {
        header: ConditionHeader {
            kind: ConditionKind::Read,
        },
        reader: dr,
        sample_states,
        view_states,
        instance_states,
    }))
}

/// Creates a QueryCondition (Spec §2.2.2.5.2.5).
///
/// # Safety
/// `dr`, `expr` valid; `params[0..param_count]` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_create_querycondition(
    dr: *mut ZeroDdsDataReader,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
    expr: *const core::ffi::c_char,
    params: *const *const core::ffi::c_char,
    param_count: usize,
) -> *mut ZeroDdsQueryCondition {
    if dr.is_null() || expr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dr+expr NULL-checked above; expr NUL-terminated
    // (caller pledge); params[0..param_count] valid if params != NULL.
    unsafe {
        let cs = std::ffi::CStr::from_ptr(expr);
        let expression = match cs.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let mut parameters: Vec<alloc::string::String> = Vec::with_capacity(param_count);
        if !params.is_null() && param_count > 0 {
            let slc = slice::from_raw_parts(params, param_count);
            for &p in slc {
                if p.is_null() {
                    continue;
                }
                let cs = std::ffi::CStr::from_ptr(p);
                if let Ok(s) = cs.to_str() {
                    parameters.push(s.to_string());
                }
            }
        }
        Box::into_raw(Box::new(ZeroDdsQueryCondition {
            header: ConditionHeader {
                kind: ConditionKind::Query,
            },
            reader: dr,
            sample_states,
            view_states,
            instance_states,
            expression: std::sync::RwLock::new(expression),
            parameters: std::sync::RwLock::new(parameters),
        }))
    }
}

/// The expression and parameters of a query condition as they are now.
fn query_snapshot(
    q: &ZeroDdsQueryCondition,
) -> (alloc::string::String, Vec<alloc::string::String>) {
    (
        q.expression.read().map(|g| g.clone()).unwrap_or_default(),
        q.parameters.read().map(|g| g.clone()).unwrap_or_default(),
    )
}

/// Replaces the filter expression of a QueryCondition (DDS-PSM-Cxx 1.0
/// `QueryCondition::expression(const std::string&)`).
///
/// Returns `BadHandle` for NULL or a condition that is not a QueryCondition,
/// `BadParameter` for a NULL or non-UTF-8 expression.
///
/// # Safety
/// `c` is NULL or a condition from `zerodds_dr_create_querycondition`;
/// `expr` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_querycondition_set_expression(
    c: *mut core::ffi::c_void,
    expr: *const core::ffi::c_char,
) -> c_int {
    // SAFETY: see fn # Safety doc — the kind is read from the header after the NULL check.
    unsafe {
        if !matches!(condition_kind(c), Some(ConditionKind::Query)) {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        if expr.is_null() {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        let Ok(text) = std::ffi::CStr::from_ptr(expr).to_str() else {
            return ZeroDdsStatus::BadParameter as c_int;
        };
        let q = &*(c as *const ZeroDdsQueryCondition);
        match q.expression.write() {
            Ok(mut g) => {
                *g = text.to_string();
                ZeroDdsStatus::Ok as c_int
            }
            Err(_) => ZeroDdsStatus::Error as c_int,
        }
    }
}

/// Replaces the parameters of a QueryCondition (DDS 1.4 §2.2.2.5.9
/// `set_query_parameters`).
///
/// Returns `BadHandle` for NULL or a condition that is not a QueryCondition.
/// A NULL entry or a non-UTF-8 entry is `BadParameter`, and then nothing changes.
///
/// # Safety
/// `c` is NULL or a condition from `zerodds_dr_create_querycondition`;
/// `params[0..count]` are NUL-terminated strings, or `params` is NULL with `count` 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_querycondition_set_parameters(
    c: *mut core::ffi::c_void,
    params: *const *const core::ffi::c_char,
    count: usize,
) -> c_int {
    // SAFETY: see fn # Safety doc.
    unsafe {
        if !matches!(condition_kind(c), Some(ConditionKind::Query)) {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let mut next: Vec<alloc::string::String> = Vec::with_capacity(count);
        if count > 0 {
            if params.is_null() {
                return ZeroDdsStatus::BadParameter as c_int;
            }
            for &p in slice::from_raw_parts(params, count) {
                if p.is_null() {
                    return ZeroDdsStatus::BadParameter as c_int;
                }
                let Ok(text) = std::ffi::CStr::from_ptr(p).to_str() else {
                    return ZeroDdsStatus::BadParameter as c_int;
                };
                next.push(text.to_string());
            }
        }
        let q = &*(c as *const ZeroDdsQueryCondition);
        match q.parameters.write() {
            Ok(mut g) => {
                *g = next;
                ZeroDdsStatus::Ok as c_int
            }
            Err(_) => ZeroDdsStatus::Error as c_int,
        }
    }
}

/// Deletes a ReadCondition or QueryCondition.
///
/// # Safety
/// `c` valid; must come from `dr_create_readcondition` or
/// `dr_create_querycondition`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_delete_readcondition(
    _dr: *mut ZeroDdsDataReader,
    c: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: see fn # Safety doc — c from dr_create_readcondition or
    // dr_create_querycondition; kind discrimination via ConditionHeader.
    unsafe {
        let kind = match condition_kind(c) {
            Some(k) => k,
            None => return ZeroDdsStatus::BadHandle as c_int,
        };
        match kind {
            ConditionKind::Read => {
                let _ = Box::from_raw(c as *mut ZeroDdsReadCondition);
            }
            ConditionKind::Query => {
                let _ = Box::from_raw(c as *mut ZeroDdsQueryCondition);
            }
            _ => return ZeroDdsStatus::PreconditionNotMet as c_int,
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// WaitSet
// ---------------------------------------------------------------------------

/// WaitSet (Spec §2.2.2.1.2.1.5).
pub struct ZeroDdsWaitSet {
    /// List of attached conditions (pointers to box-allocated
    /// condition structs).
    conditions: Mutex<Vec<*mut core::ffi::c_void>>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsWaitSet {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsWaitSet {}

/// Creates a WaitSet.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_waitset_create() -> *mut ZeroDdsWaitSet {
    Box::into_raw(Box::new(ZeroDdsWaitSet {
        conditions: Mutex::new(Vec::new()),
    }))
}

/// Deletes a WaitSet.
///
/// # Safety
/// `w` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_destroy(w: *mut ZeroDdsWaitSet) {
    if !w.is_null() {
        // SAFETY: see fn # Safety doc — w from zerodds_waitset_create.
        let _ = unsafe { Box::from_raw(w) };
    }
}

/// Attach Condition.
///
/// # Safety
/// `w`, `cond` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_attach_condition(
    w: *mut ZeroDdsWaitSet,
    cond: *mut core::ffi::c_void,
) -> c_int {
    if w.is_null() || cond.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — w+cond NULL-checked above.
    unsafe {
        if let Ok(mut g) = (*w).conditions.lock() {
            if !g.contains(&cond) {
                g.push(cond);
            }
        }
        // Wire the event-driven data-available signal: a sample arriving on the
        // reader behind a Read/Query condition now wakes this WaitSet's wait
        // (no busy-poll). No-op for Guard/Status conditions and reader-less ones.
        register_condition_data_waker(cond);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Detach Condition.
///
/// # Safety
/// `w`, `cond` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_detach_condition(
    w: *mut ZeroDdsWaitSet,
    cond: *mut core::ffi::c_void,
) -> c_int {
    if w.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — w NULL-checked above; cond tolerates NULL.
    unsafe {
        if let Ok(mut g) = (*w).conditions.lock() {
            let n = g.len();
            g.retain(|c| *c != cond);
            if g.len() == n {
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Waits until at least one condition triggers or a timeout.
///
/// Writes the active conditions into `out_active`: the first
/// `min(len, cap)` handles, `*out_count` set to the **total** number of active
/// conditions. The return value is `OutOfResources` when `len > cap` (so a
/// caller retries with a buffer of `*out_count`), `Ok` when the buffer held
/// every active condition, and `Timeout` when none triggered within the
/// deadline. A NULL `out_active` with `cap == 0` asks for the length alone
/// (`docs/specs/zerodds-c-api-1.0.md` §2.8).
///
/// # Safety
/// `w`, `out_count`, `timeout` valid; `out_active[0..cap]` writeable, or
/// `out_active` NULL exactly when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_wait(
    w: *mut ZeroDdsWaitSet,
    out_active: *mut *mut core::ffi::c_void,
    cap: usize,
    out_count: *mut usize,
    timeout_sec: i32,
    timeout_nanosec: u32,
) -> c_int {
    if w.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    if out_active.is_null() && cap != 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let timeout = if timeout_sec == i32::MAX && timeout_nanosec == u32::MAX {
        Duration::from_secs(60 * 60 * 24 * 365 * 100) // ~100 years = INFINITE
    } else {
        Duration::new(timeout_sec.max(0) as u64, timeout_nanosec)
    };
    let deadline = Instant::now() + timeout;
    // SAFETY: see fn # Safety doc — w+out_count NULL-checked above, and a NULL
    // `out_active` is admitted only for `cap == 0`; the conds items come from
    // attach_condition (the caller provided valid condition pointers).
    unsafe {
        let ws = &*w;
        let ev = data_event();
        loop {
            // Snapshot the generation BEFORE evaluating triggers, so a sample
            // that lands between the check and the block is not lost: if the
            // generation moved, we loop and re-check instead of blocking.
            let seen_gen = ev.wake_gen.lock().map(|g| *g).unwrap_or(0);
            // And empty the descriptor in the same breath, so a caller that
            // alternates `select` with a zero-timeout wait here sees one
            // readable event per arrival rather than a pipe that stays hot.
            ev.drain_fd();

            let conds: Vec<*mut core::ffi::c_void> =
                ws.conditions.lock().map(|g| g.clone()).unwrap_or_default();
            let mut active: Vec<*mut core::ffi::c_void> = Vec::new();
            for &c in conds.iter() {
                if zerodds_condition_get_trigger_value(c as *const _) {
                    active.push(c);
                }
            }
            if !active.is_empty() {
                return copy_list_out(out_active, cap, &active, out_count);
            }
            let now = Instant::now();
            if now >= deadline {
                *out_count = 0;
                return ZeroDdsStatus::Timeout as c_int;
            }

            // Event-driven block: wait on the data-available condvar until a
            // reader signals new data (waker → generation bump → notify) or the
            // remaining timeout elapses. GuardCondition / StatusCondition state
            // is user/poll-driven and has no waker; to keep those responsive
            // without busy-spinning, the per-iteration wait is additionally
            // capped so a manually-tripped guard is observed within that bound.
            // For data-availability the wakeup is immediate via the condvar.
            let remaining = deadline.saturating_duration_since(now);
            let only_data_conditions = conds.iter().all(|&c| {
                matches!(
                    condition_kind(c as *const _),
                    Some(ConditionKind::Read) | Some(ConditionKind::Query)
                )
            });
            // Data-only WaitSets block until the next data event (no poll cap).
            // Mixed/guard sets use a short cap so guard flips are still seen.
            let block = if only_data_conditions {
                remaining
            } else {
                remaining.min(Duration::from_millis(5))
            };
            if let Ok(guard) = ev.wake_gen.lock() {
                // If a signal arrived since `seen_gen`, skip the block entirely.
                if *guard == seen_gen {
                    let _ = ev.cvar.wait_timeout(guard, block);
                }
            }
        }
    }
}

/// Returns all attached conditions.
///
/// The first `min(len, cap)` handles are written into `out`, `*out_count` is
/// set to the **total** number of attached conditions, and the return value is
/// `OutOfResources` when `len > cap` (so a caller retries with a buffer of
/// `*out_count`), `Ok` otherwise. A NULL `out` with `cap == 0` asks for the
/// length alone (`docs/specs/zerodds-c-api-1.0.md` §2.8).
///
/// # Safety
/// `w`, `out_count` valid; `out[0..cap]` writeable, or `out` NULL exactly
/// when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_get_conditions(
    w: *mut ZeroDdsWaitSet,
    out: *mut *mut core::ffi::c_void,
    cap: usize,
    out_count: *mut usize,
) -> c_int {
    if w.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — w+out_count NULL-checked above; the
    // buffer contract is enforced by `copy_list_out`.
    unsafe {
        let conds: Vec<*mut core::ffi::c_void> = (*w)
            .conditions
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        copy_list_out(out, cap, &conds, out_count)
    }
}

/// A descriptor that becomes readable when a sample arrives on any reader.
///
/// For a caller that already has an event loop. Put the descriptor in
/// `select`, `poll`, `epoll`, `kqueue`, or whatever the language's loop
/// accepts; when it reads ready, call [`zerodds_waitset_wait`] with a zero
/// timeout to learn *which* conditions triggered. The descriptor says that
/// something arrived, never what or how much.
///
/// ```c
/// int fd = zerodds_data_notify_fd();
/// fd_set r; FD_ZERO(&r); FD_SET(fd, &r);
/// select(fd + 1, &r, NULL, NULL, NULL);          // sleeps, does not poll
/// zerodds_waitset_wait(ws, active, cap, &n, 0, 0);
/// ```
///
/// Without this, a caller with its own loop had two choices, and the callback
/// spec (`docs/specs/zerodds-listener-callbacks-1.1.md` §4.1) took the second:
/// park a thread in [`zerodds_waitset_wait`] and forward the wake, or drive
/// `zerodds_poll_listeners` on a timer. A poll's period is a floor under
/// latency in each direction, and it spends the CPU that the delivery threads
/// need — measured on an overbooked host, three poll variants delivered one
/// sample between two processes across 46 million iterations while the waker
/// path came through.
///
/// The descriptor is process-global and level-triggered until drained: it is
/// emptied by [`zerodds_waitset_wait`], so alternating the two is the intended
/// use. Reading it directly is allowed and drains one wake; the byte carries no
/// meaning.
///
/// Owned by the library — do not `close` it. It stays valid for the life of the
/// process.
///
/// # Returns
/// The descriptor, or `-1` where there is none: on Windows, which has no
/// descriptor `select` accepts for this, and in the (unexpected) case that the
/// process could not spare two descriptors at startup. `-1` is not an error to
/// recover from — it means this caller must use [`zerodds_waitset_wait`]
/// itself.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_data_notify_fd() -> c_int {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd as _;
        data_event()
            .pipe
            .as_ref()
            .map_or(-1, |p| p.read_end.as_raw_fd())
    }
    #[cfg(not(unix))]
    {
        -1
    }
}

/// Whether this reader is holding samples the application has not read.
///
/// The question a loop asks after [`zerodds_data_notify_fd`] wakes it: which
/// of my readers has something? It answers from what the reader already holds
/// and moves what the receive path has queued into that cache — it does not
/// decode a sample, does not consume one, and leaves every sample readable by
/// `zerodds_dr_take` / `zerodds_dr_read` afterwards.
///
/// Cheaper than a `ReadCondition` when the caller only wants a yes or no: no
/// condition object, no WaitSet, no state masks. On a reader with a content
/// filter it is cheaper again on the second and later calls, because the
/// filter runs once per sample as it is moved, not once per question.
///
/// # Safety
/// `dr` is NULL or a live reader handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_has_data(dr: *mut ZeroDdsDataReader) -> bool {
    if dr.is_null() {
        return false;
    }
    // The sample-state bit for NOT_READ (1 is READ, 2 is NOT_READ), and any
    // view or instance state. "Unread" is the question a wake-up asks; a
    // caller that wants a different mask builds a ReadCondition.
    const NOT_READ: u32 = 2;
    const ANY: u32 = 0;
    // SAFETY: see fn # Safety doc — NULL-checked above; the caller pledges a
    // live reader.
    unsafe {
        let drr = &*dr;
        // The cache first: a reader that already holds an unread sample needs
        // no channel work to answer.
        if cache_has_matching(drr, NOT_READ, ANY, ANY) {
            return true;
        }
        drain_channel_into_cache(drr);
        cache_has_matching(drr, NOT_READ, ANY, ANY)
    }
}

/// The StatusChangedFlags (DDS 1.4 §2.2.4.1) of `entity`, by what it was
/// created as: a writer's and a reader's from the runtime, DATA_AVAILABLE
/// while a reader holds an unread sample, DATA_ON_READERS while one of a
/// subscriber's readers does, INCONSISTENT_TOPIC while a topic's count
/// moved since it was read. 0 for a participant, a publisher, and anything
/// else.
///
/// # Safety
/// `entity` is a live entity handle or an address no entity has.
pub(crate) unsafe fn status_changes_of(entity: *mut core::ffi::c_void) -> u32 {
    use crate::entities::StatusEntityKind;
    use zerodds_dcps::psm_constants::status;
    const NOT_READ: u32 = 2;
    const ANY: u32 = 0;
    let unread = |drr: &ZeroDdsDataReader| {
        cache_has_matching(drr, NOT_READ, ANY, ANY) || {
            drain_channel_into_cache(drr);
            cache_has_matching(drr, NOT_READ, ANY, ANY)
        }
    };
    // SAFETY: the kind is what the live handle at this address was created
    // as (`note_status_entity`), so the cast is to its own type.
    unsafe {
        match crate::entities::status_entity_kind(entity) {
            Some(StatusEntityKind::DataWriter) => {
                let dw = &*(entity as *const ZeroDdsDataWriter);
                dw.rt.user_writer_status_changes(dw.eid)
            }
            Some(StatusEntityKind::DataReader) => {
                let dr = &*(entity as *const ZeroDdsDataReader);
                let mut changes = dr.rt.user_reader_status_changes(dr.eid);
                if unread(dr) {
                    changes |= status::DATA_AVAILABLE;
                }
                changes
            }
            Some(StatusEntityKind::Subscriber) => {
                let sub = &*(entity as *const crate::entities::ZeroDdsSubscriber);
                let readers = sub
                    .datareaders
                    .lock()
                    .map(|r| r.clone())
                    .unwrap_or_default();
                if readers.iter().any(|dr| !dr.is_null() && unread(&**dr)) {
                    status::DATA_ON_READERS
                } else {
                    0
                }
            }
            Some(StatusEntityKind::Topic) => {
                let topic = &*(entity as *const crate::entities::ZeroDdsTopic);
                if topic.participant.is_null() {
                    return 0;
                }
                let participant = &*topic.participant;
                let total = participant.rt.as_ref().map_or(0, |rt| {
                    i32::try_from(rt.inconsistent_topic_count_for(&topic.name)).unwrap_or(i32::MAX)
                });
                if total != topic.inconsistent_topic_read.load(Ordering::Acquire) {
                    status::INCONSISTENT_TOPIC
                } else {
                    0
                }
            }
            None => 0,
        }
    }
}

// suppress unused-import warning
#[allow(dead_code)]
fn _suppress(_: *mut ZeroDdsDataWriter) {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Regression guard for F-PSM-CXX-readcond-segv: the generic
    /// `condition_kind()` dispatcher reads the kind discriminant via a
    /// `*const ConditionHeader` cast, i.e. the `header` field MUST be at
    /// offset 0 in all condition structs. `repr(Rust)` could reorder —
    /// `#[repr(C)]` pins it. If this test breaks, a `#[repr(C)]` has
    /// been lost and the C++ path would segfault again.
    #[test]
    fn condition_header_at_offset_zero() {
        assert_eq!(core::mem::offset_of!(ZeroDdsGuardCondition, header), 0);
        assert_eq!(core::mem::offset_of!(ZeroDdsStatusCondition, header), 0);
        assert_eq!(core::mem::offset_of!(ZeroDdsReadCondition, header), 0);
        assert_eq!(core::mem::offset_of!(ZeroDdsQueryCondition, header), 0);
    }

    /// The descriptor exists and is not one of the standard streams.
    #[cfg(unix)]
    #[test]
    fn notify_fd_is_a_real_descriptor() {
        let fd = zerodds_data_notify_fd();
        assert!(fd > 2, "expected a descriptor of its own, got {fd}");
        assert_eq!(fd, zerodds_data_notify_fd(), "the descriptor is stable");
    }

    /// A signal makes the descriptor readable, and `select` returns without
    /// anyone polling.
    ///
    /// On an event of its own, not the process-global one: every
    /// `zerodds_waitset_wait` in this binary drains that descriptor, so a test
    /// asserting on its state would be racing every other test in the file.
    /// What is under test is the mechanism — `raise_fd`, `drain_fd`, and a
    /// `select` that returns because a byte was written — and that is the same
    /// code either way.
    ///
    /// The wait is the point: a `select` with a long timeout returns only
    /// because the signal wrote a byte. A poll loop would return early and fail
    /// the lower bound, so the timeout is there to fail the test rather than to
    /// drive it.
    #[cfg(unix)]
    #[test]
    fn a_signal_wakes_a_select() {
        use std::io::Read as _;
        use std::os::unix::io::AsRawFd as _;

        let ev = Arc::new(DataEvent {
            wake_gen: Mutex::new(0),
            pipe: new_notify_pipe(),
            cvar: Condvar::new(),
        });
        let fd = ev.pipe.as_ref().expect("pipe").read_end.as_raw_fd();
        assert!(fd > 2, "expected a descriptor of its own, got {fd}");

        // Signal from another thread once the reader is inside `select`, so
        // the wake is what returns the call and not a byte written earlier.
        let signaller = Arc::clone(&ev);
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            signaller.signal();
        });

        let started = Instant::now();
        let readable = wait_readable(fd, Duration::from_secs(5));
        let waited = started.elapsed();
        handle.join().expect("signalling thread");

        assert!(readable, "select did not report the descriptor readable");
        assert!(
            waited >= Duration::from_millis(40),
            "returned before the signal was sent ({waited:?}) — the descriptor \
             was already hot"
        );
        assert!(
            waited < Duration::from_secs(4),
            "select ran to its timeout ({waited:?}) — the signal never arrived"
        );

        // One byte, coalesced: three signals before a drain add nothing.
        ev.drain_fd();
        ev.signal();
        ev.signal();
        ev.signal();
        let mut buf = [0u8; 16];
        let pipe = ev.pipe.as_ref().expect("pipe");
        let n = (&pipe.read_end).read(&mut buf).expect("read");
        assert_eq!(n, 1, "expected the pipe to coalesce to a single byte");

        // And after a drain it is quiet until the next signal. Kept in this
        // test rather than its own: the event is process-global, so two tests
        // asserting on its state would signal into each other's wait.
        ev.drain_fd();
        assert!(
            !wait_readable(fd, Duration::from_millis(100)),
            "descriptor still readable after a drain"
        );
        ev.signal();
        assert!(
            wait_readable(fd, Duration::from_secs(2)),
            "descriptor did not come back after a fresh signal"
        );
        ev.drain_fd();
    }

    /// `select(2)` on one descriptor, through the raw syscall so the test
    /// depends on nothing this crate does not already depend on.
    #[cfg(unix)]
    fn wait_readable(fd: c_int, timeout: Duration) -> bool {
        // SAFETY: `fds` is a live, zeroed `fd_set` sized by the kernel's own
        // header; `fd` is a descriptor this process owns and is below
        // `FD_SETSIZE` (it is one of the first handful opened). `select` reads
        // and writes only the two structures passed by pointer.
        unsafe {
            let mut fds: FdSet = core::mem::zeroed();
            fd_set_insert(&mut fds, fd);
            let mut tv = TimeVal {
                tv_sec: timeout.as_secs() as isize,
                tv_usec: isize::try_from(timeout.subsec_micros()).unwrap_or(0),
            };
            select(
                fd + 1,
                &raw mut fds,
                ptr::null_mut(),
                ptr::null_mut(),
                &raw mut tv,
            ) > 0
        }
    }

    #[cfg(unix)]
    const FD_SETSIZE_WORDS: usize = 1024 / (8 * core::mem::size_of::<usize>());

    /// The kernel's `fd_set`: a bitmap of `FD_SETSIZE` bits.
    #[cfg(unix)]
    #[repr(C)]
    struct FdSet {
        bits: [usize; FD_SETSIZE_WORDS],
    }

    /// The kernel's `timeval`.
    #[cfg(unix)]
    #[repr(C)]
    struct TimeVal {
        tv_sec: isize,
        tv_usec: isize,
    }

    #[cfg(unix)]
    fn fd_set_insert(set: &mut FdSet, fd: c_int) {
        let bits = 8 * core::mem::size_of::<usize>();
        let index = (fd as usize) / bits;
        let bit = (fd as usize) % bits;
        set.bits[index] |= 1usize << bit;
    }

    #[cfg(unix)]
    unsafe extern "C" {
        fn select(
            nfds: c_int,
            readfds: *mut FdSet,
            writefds: *mut FdSet,
            exceptfds: *mut FdSet,
            timeout: *mut TimeVal,
        ) -> c_int;
    }

    #[test]
    fn guardcondition_lifecycle_and_trigger() {
        let g = zerodds_guardcondition_create();
        assert!(!g.is_null());
        // SAFETY: g from guardcondition_create.
        unsafe {
            assert!(!zerodds_condition_get_trigger_value(g as *const _));
            let _ = zerodds_guardcondition_set_trigger_value(g, true);
            assert!(zerodds_condition_get_trigger_value(g as *const _));
            zerodds_guardcondition_destroy(g);
        }
    }

    #[test]
    fn statuscondition_lifecycle_and_mask() {
        // SAFETY: NULL is an allowed entity slot for StatusCondition.
        unsafe {
            let sc = zerodds_entity_get_statuscondition(ptr::null_mut());
            assert!(!sc.is_null());
            let _ = zerodds_statuscondition_set_enabled_statuses(sc, 0x1234);
            assert_eq!(zerodds_statuscondition_get_enabled_statuses(sc), 0x1234);
            zerodds_statuscondition_destroy(sc);
        }
    }

    /// DDS 1.4 §2.2.2.1.6 through C: a writer's StatusCondition on
    /// PUBLICATION_MATCHED triggers once a reader matched and stops after
    /// the status was read; one on a participant, which has no status of
    /// its own, never triggers. It triggered whenever a mask was set.
    #[test]
    fn a_status_condition_triggers_on_the_entitys_flags() {
        use crate::participant_ffi::{
            zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
            zerodds_dp_delete_contained_entities,
        };
        use crate::publisher_ffi::{
            ZeroDdsPublicationMatchedStatus, zerodds_dw_get_publication_matched_status,
            zerodds_pub_create_datawriter,
        };
        use crate::subscriber_ffi::zerodds_sub_create_datareader;
        use zerodds_dcps::psm_constants::status::PUBLICATION_MATCHED;
        let f = crate::factory_ffi::zerodds_dpf_get_instance();
        // SAFETY: every handle comes from a create call; the conditions are
        // destroyed before the entities.
        unsafe {
            let p = crate::factory_ffi::zerodds_dpf_create_participant(f, 39, ptr::null());
            let t = zerodds_dp_create_topic(p, c"CondFlags".as_ptr(), c"T".as_ptr(), ptr::null());
            let dw = zerodds_pub_create_datawriter(
                zerodds_dp_create_publisher(p, ptr::null()),
                t,
                ptr::null(),
            );
            let sc = zerodds_entity_get_statuscondition(dw.cast());
            assert_eq!(
                zerodds_statuscondition_set_enabled_statuses(sc, PUBLICATION_MATCHED),
                0
            );
            let on_participant = zerodds_entity_get_statuscondition(p.cast());
            assert!(!zerodds_condition_get_trigger_value(sc.cast()));

            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let _dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert!(zerodds_condition_get_trigger_value(sc.cast()));
            assert!(!zerodds_condition_get_trigger_value(on_participant.cast()));

            let mut st = ZeroDdsPublicationMatchedStatus::default();
            assert_eq!(zerodds_dw_get_publication_matched_status(dw, &mut st), 0);
            assert!(!zerodds_condition_get_trigger_value(sc.cast()));

            zerodds_statuscondition_destroy(sc);
            zerodds_statuscondition_destroy(on_participant);
            zerodds_dp_delete_contained_entities(p);
            crate::factory_ffi::zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn waitset_attach_detach() {
        let ws = zerodds_waitset_create();
        let g = zerodds_guardcondition_create();
        let mut buf: [*mut core::ffi::c_void; 4] = [ptr::null_mut(); 4];
        let mut count: usize = 0;
        // SAFETY: ws + g from create fns; buf stack-local.
        unsafe {
            let rc = zerodds_waitset_attach_condition(ws, g as *mut core::ffi::c_void);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            let rc = zerodds_waitset_get_conditions(ws, buf.as_mut_ptr(), 4, &mut count);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 1);
            let rc = zerodds_waitset_detach_condition(ws, g as *mut core::ffi::c_void);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            zerodds_waitset_destroy(ws);
            zerodds_guardcondition_destroy(g);
        }
    }

    #[test]
    fn waitset_wait_returns_active_guard() {
        let ws = zerodds_waitset_create();
        let g = zerodds_guardcondition_create();
        let mut buf: [*mut core::ffi::c_void; 4] = [ptr::null_mut(); 4];
        let mut count: usize = 0;
        // SAFETY: ws + g from create fns; buf stack-local.
        unsafe {
            zerodds_guardcondition_set_trigger_value(g, true);
            zerodds_waitset_attach_condition(ws, g as *mut core::ffi::c_void);
            let rc = zerodds_waitset_wait(ws, buf.as_mut_ptr(), 4, &mut count, 1, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 1);
            assert_eq!(buf[0], g as *mut core::ffi::c_void);
            zerodds_waitset_destroy(ws);
            zerodds_guardcondition_destroy(g);
        }
    }

    #[test]
    fn waitset_wait_timeout_no_active() {
        let ws = zerodds_waitset_create();
        let g = zerodds_guardcondition_create();
        let mut buf: [*mut core::ffi::c_void; 4] = [ptr::null_mut(); 4];
        let mut count: usize = 0;
        // SAFETY: ws + g from create fns; buf stack-local.
        unsafe {
            zerodds_waitset_attach_condition(ws, g as *mut core::ffi::c_void);
            let rc = zerodds_waitset_wait(ws, buf.as_mut_ptr(), 4, &mut count, 0, 50_000_000);
            assert_eq!(rc, ZeroDdsStatus::Timeout as c_int);
            assert_eq!(count, 0);
            zerodds_waitset_destroy(ws);
            zerodds_guardcondition_destroy(g);
        }
    }

    /// `zerodds_waitset_get_conditions` writes `min(len, cap)` entries but
    /// reports the total and returns `OutOfResources` when the buffer is too
    /// small, so no condition is dropped silently
    /// (`docs/specs/zerodds-c-api-1.0.md` §2.8).
    #[test]
    fn waitset_get_conditions_reports_the_total() {
        let ws = zerodds_waitset_create();
        let gs: [*mut ZeroDdsGuardCondition; 3] = [
            zerodds_guardcondition_create(),
            zerodds_guardcondition_create(),
            zerodds_guardcondition_create(),
        ];
        let mut count: usize = 0;
        // SAFETY: ws + gs from create fns; every buffer is stack-local.
        unsafe {
            // Empty: nothing attached yet.
            let rc = zerodds_waitset_get_conditions(ws, ptr::null_mut(), 0, &mut count);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 0);

            for g in gs {
                assert_eq!(
                    zerodds_waitset_attach_condition(ws, g as *mut core::ffi::c_void),
                    ZeroDdsStatus::Ok as c_int
                );
            }

            // Too small: `cap` entries written, the total reported.
            let mut buf: [*mut core::ffi::c_void; 2] = [ptr::null_mut(); 2];
            let rc = zerodds_waitset_get_conditions(ws, buf.as_mut_ptr(), 2, &mut count);
            assert_eq!(rc, ZeroDdsStatus::OutOfResources as c_int);
            assert_eq!(count, 3);
            assert_eq!(buf[0], gs[0] as *mut core::ffi::c_void);
            assert_eq!(buf[1], gs[1] as *mut core::ffi::c_void);

            // `cap == 0` with NULL asks for the length alone.
            let rc = zerodds_waitset_get_conditions(ws, ptr::null_mut(), 0, &mut count);
            assert_eq!(rc, ZeroDdsStatus::OutOfResources as c_int);
            assert_eq!(count, 3);

            // Exactly full: `cap == len`.
            let mut exact: [*mut core::ffi::c_void; 3] = [ptr::null_mut(); 3];
            let rc = zerodds_waitset_get_conditions(ws, exact.as_mut_ptr(), 3, &mut count);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 3);
            assert_eq!(exact[0], gs[0] as *mut core::ffi::c_void);
            assert_eq!(exact[1], gs[1] as *mut core::ffi::c_void);
            assert_eq!(exact[2], gs[2] as *mut core::ffi::c_void);

            // Enough room: every entry, `Ok`.
            let mut all: [*mut core::ffi::c_void; 4] = [ptr::null_mut(); 4];
            let rc = zerodds_waitset_get_conditions(ws, all.as_mut_ptr(), 4, &mut count);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 3);
            assert_eq!(all[0], gs[0] as *mut core::ffi::c_void);
            assert_eq!(all[1], gs[1] as *mut core::ffi::c_void);
            assert_eq!(all[2], gs[2] as *mut core::ffi::c_void);

            // NULL with `cap > 0` is refused.
            let rc = zerodds_waitset_get_conditions(ws, ptr::null_mut(), 4, &mut count);
            assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);

            zerodds_waitset_destroy(ws);
            for g in gs {
                zerodds_guardcondition_destroy(g);
            }
        }
    }

    /// The active-conditions list of `zerodds_waitset_wait` follows the same
    /// total-length contract as `zerodds_waitset_get_conditions`
    /// (`docs/specs/zerodds-c-api-1.0.md` §2.8).
    #[test]
    fn waitset_wait_reports_the_total() {
        let ws = zerodds_waitset_create();
        let gs: [*mut ZeroDdsGuardCondition; 2] = [
            zerodds_guardcondition_create(),
            zerodds_guardcondition_create(),
        ];
        let mut count: usize = 0;
        // SAFETY: ws + gs from create fns; every buffer is stack-local.
        unsafe {
            // Empty: no condition attached, none active.
            let rc = zerodds_waitset_wait(ws, ptr::null_mut(), 0, &mut count, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Timeout as c_int);
            assert_eq!(count, 0);

            for g in gs {
                zerodds_guardcondition_set_trigger_value(g, true);
                assert_eq!(
                    zerodds_waitset_attach_condition(ws, g as *mut core::ffi::c_void),
                    ZeroDdsStatus::Ok as c_int
                );
            }

            // Too small: `cap` entries written, the total reported.
            let mut buf: [*mut core::ffi::c_void; 1] = [ptr::null_mut(); 1];
            let rc = zerodds_waitset_wait(ws, buf.as_mut_ptr(), 1, &mut count, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::OutOfResources as c_int);
            assert_eq!(count, 2);
            assert_eq!(buf[0], gs[0] as *mut core::ffi::c_void);

            // `cap == 0` with NULL asks for the length alone.
            let rc = zerodds_waitset_wait(ws, ptr::null_mut(), 0, &mut count, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::OutOfResources as c_int);
            assert_eq!(count, 2);

            // Exactly full: `cap == len`.
            let mut exact: [*mut core::ffi::c_void; 2] = [ptr::null_mut(); 2];
            let rc = zerodds_waitset_wait(ws, exact.as_mut_ptr(), 2, &mut count, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 2);
            assert_eq!(exact[0], gs[0] as *mut core::ffi::c_void);
            assert_eq!(exact[1], gs[1] as *mut core::ffi::c_void);

            // Enough room: every active entry, `Ok`.
            let mut all: [*mut core::ffi::c_void; 4] = [ptr::null_mut(); 4];
            let rc = zerodds_waitset_wait(ws, all.as_mut_ptr(), 4, &mut count, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 2);
            assert_eq!(all[0], gs[0] as *mut core::ffi::c_void);
            assert_eq!(all[1], gs[1] as *mut core::ffi::c_void);

            zerodds_waitset_destroy(ws);
            for g in gs {
                zerodds_guardcondition_destroy(g);
            }
        }
    }
}
