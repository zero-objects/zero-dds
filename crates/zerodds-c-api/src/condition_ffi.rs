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
}

impl DataEvent {
    /// Signals that data became available on some reader. Bumps the generation
    /// and wakes every WaitSet blocked in `wait_timeout`.
    fn signal(&self) {
        if let Ok(mut g) = self.wake_gen.lock() {
            *g = g.wrapping_add(1);
        }
        self.cvar.notify_all();
    }
}

fn data_event() -> &'static Arc<DataEvent> {
    use std::sync::OnceLock;
    static EV: OnceLock<Arc<DataEvent>> = OnceLock::new();
    EV.get_or_init(|| {
        Arc::new(DataEvent {
            wake_gen: Mutex::new(0),
            cvar: Condvar::new(),
        })
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
        drr.rt
            .register_user_reader_waker(drr.eid, Some(make_data_waker()));
    }
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
    /// Filter expression.
    expression: alloc::string::String,
    /// Filter parameters.
    parameters: Vec<alloc::string::String>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsQueryCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsQueryCondition {}

// ---------------------------------------------------------------------------
// Condition trigger helpers — drain the channel into the cache, evaluate masks
// ---------------------------------------------------------------------------

fn drain_channel_into_cache(drr: &ZeroDdsDataReader) {
    use zerodds_dcps::runtime::UserSample;
    if let (Ok(rx), Ok(mut cache)) = (drr.rx.lock(), drr.read_cache.lock()) {
        while let Ok(s) = rx.try_recv() {
            let pass = if let Some(filter) = &drr.cft_filter {
                match &s {
                    UserSample::Alive { payload, .. } => filter.evaluate(payload),
                    UserSample::Lifecycle { .. } => true,
                }
            } else {
                true
            };
            if pass {
                cache.push((s, crate::entities::ReadSampleState::NotRead));
            }
        }
    }
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
    cache.iter().any(|(_sample, st)| {
        let s_bit = match st {
            crate::entities::ReadSampleState::Read => 1u32,
            crate::entities::ReadSampleState::NotRead => 2u32,
        };
        let s_ok = sample_states == 0 || (sample_states & s_bit) != 0;
        let v_ok = view_states == 0 || (view_states & 1u32) != 0; // RC1: all samples NEW
        let i_ok = instance_states == 0 || (instance_states & 1u32) != 0; // RC1: ALIVE
        s_ok && v_ok && i_ok
    })
}

fn cache_has_matching_query(
    drr: &ZeroDdsDataReader,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
    expression: &str,
    params: &[alloc::string::String],
) -> bool {
    use zerodds_dcps::runtime::UserSample;
    if !cache_has_matching(drr, sample_states, view_states, instance_states) {
        return false;
    }
    // Parse the filter expression.
    let expr = match zerodds_sql_filter::parse(expression) {
        Ok(e) => e,
        Err(_) => return false,
    };
    let values: Vec<zerodds_sql_filter::Value> = params
        .iter()
        .map(|p| zerodds_sql_filter::Value::String(p.clone()))
        .collect();
    let cache = match drr.read_cache.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    struct EmptyRow;
    impl zerodds_sql_filter::RowAccess for EmptyRow {
        fn get(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
            None
        }
    }
    cache.iter().any(|(s, _)| match s {
        UserSample::Alive { .. } => expr.evaluate(&EmptyRow, &values).unwrap_or(true),
        UserSample::Lifecycle { .. } => true,
    })
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
                // RC1: any non-empty `enabled_statuses` triggers if at least
                // one status counter is > 0. Since we have the entity pointer but
                // no type tag for it, we return `true` if enabled_statuses
                // != 0 as a spec-conformant fallback — the listener FFI in the
                // follow-up WP wires this per entity type.
                (*(c as *const ZeroDdsStatusCondition))
                    .enabled_statuses
                    .load(Ordering::SeqCst)
                    != 0
            }
            ConditionKind::Read => {
                let r = &*(c as *const ZeroDdsReadCondition);
                if r.reader.is_null() {
                    return false;
                }
                let drr = &*r.reader;
                // Spec §2.2.2.5.8: trigger_value true if the reader has samples that
                // match the (sample_states, view_states, instance_states) mask.
                drain_channel_into_cache(drr);
                cache_has_matching(drr, r.sample_states, r.view_states, r.instance_states)
            }
            ConditionKind::Query => {
                let q = &*(c as *const ZeroDdsQueryCondition);
                if q.reader.is_null() {
                    return false;
                }
                let drr = &*q.reader;
                drain_channel_into_cache(drr);
                // Spec §2.2.2.5.9: additionally evaluate the filter expression.
                cache_has_matching_query(
                    drr,
                    q.sample_states,
                    q.view_states,
                    q.instance_states,
                    &q.expression,
                    &q.parameters,
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
            expression,
            parameters,
        }))
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
/// Writes active conditions into `out_active[0..*out_count]`.
///
/// # Safety
/// `w`, `out_active`, `out_count`, `timeout` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_wait(
    w: *mut ZeroDdsWaitSet,
    out_active: *mut *mut core::ffi::c_void,
    cap: usize,
    out_count: *mut usize,
    timeout_sec: i32,
    timeout_nanosec: u32,
) -> c_int {
    if w.is_null() || out_active.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let timeout = if timeout_sec == i32::MAX && timeout_nanosec == u32::MAX {
        Duration::from_secs(60 * 60 * 24 * 365 * 100) // ~100 years = INFINITE
    } else {
        Duration::new(timeout_sec.max(0) as u64, timeout_nanosec)
    };
    let deadline = Instant::now() + timeout;
    // SAFETY: see fn # Safety doc — w+out_active+out_count NULL-checked above; the conds items
    // come from attach_condition (the caller provided valid condition pointers).
    unsafe {
        let ws = &*w;
        let ev = data_event();
        loop {
            // Snapshot the generation BEFORE evaluating triggers, so a sample
            // that lands between the check and the block is not lost: if the
            // generation moved, we loop and re-check instead of blocking.
            let seen_gen = ev.wake_gen.lock().map(|g| *g).unwrap_or(0);

            let conds: Vec<*mut core::ffi::c_void> =
                ws.conditions.lock().map(|g| g.clone()).unwrap_or_default();
            let mut active: Vec<*mut core::ffi::c_void> = Vec::new();
            for &c in conds.iter() {
                if zerodds_condition_get_trigger_value(c as *const _) {
                    active.push(c);
                }
            }
            if !active.is_empty() {
                let n = active.len().min(cap);
                let dst = slice::from_raw_parts_mut(out_active, n);
                dst.copy_from_slice(&active[..n]);
                *out_count = n;
                return ZeroDdsStatus::Ok as c_int;
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
/// # Safety
/// `w`, `out`, `out_count` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_get_conditions(
    w: *mut ZeroDdsWaitSet,
    out: *mut *mut core::ffi::c_void,
    cap: usize,
    out_count: *mut usize,
) -> c_int {
    if w.is_null() || out.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — w+out+out_count NULL-checked above; out[0..cap]
    // must be writeable (caller pledge).
    unsafe {
        let conds: Vec<*mut core::ffi::c_void> = (*w)
            .conditions
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let n = conds.len().min(cap);
        let dst = slice::from_raw_parts_mut(out, n);
        dst.copy_from_slice(&conds[..n]);
        *out_count = n;
    }
    ZeroDdsStatus::Ok as c_int
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
}
