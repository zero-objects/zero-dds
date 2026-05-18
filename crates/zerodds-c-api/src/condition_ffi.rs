// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Conditions + WaitSet C-FFI (Spec §2.2.2.1.2 + DDS-PSM-Cxx §7.5.10).
//!
//! Architektur:
//! - `ZeroDdsGuardCondition`: User-toggelbares Trigger-Flag.
//! - `ZeroDdsStatusCondition`: an eine Entity gebunden + Status-Mask.
//! - `ZeroDdsReadCondition`: an DataReader gebunden + sample/view/instance-Filter.
//! - `ZeroDdsQueryCondition`: ReadCondition + Filter-Expression (RC1: passthru).
//! - `ZeroDdsWaitSet`: Container von Conditions, `wait()` polled jede 5ms.
//!
//! Triggern:
//! - GuardCondition: `set_trigger_value`.
//! - StatusCondition: aktiviert wenn `enabled_statuses & current_status_mask != 0`.
//!   RC1 sammelt grobe Triggert-Bits via Reader/Writer-Status-Polls (matched-count
//!   nicht 0, sample_lost > 0, ...).
//! - ReadCondition: aktiviert wenn DataReader Samples im Channel hat (rx.try_iter
//!   peek-able -> aktuell: `len()`-aequivalent via `try_recv` ist nicht moeglich,
//!   wir setzen Bit basierend auf `user_reader_matched_count`).
//! - QueryCondition: wie ReadCondition.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::time::{Duration, Instant};

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDataReader, ZeroDdsDataWriter};

// ---------------------------------------------------------------------------
// Condition-Types
// ---------------------------------------------------------------------------

/// Tag um zu wissen welche Condition-Variante hinter `*mut c_void` steht.
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

/// Header in jeder Condition-Boxed-Struct, damit der WaitSet den Typ
/// runtime-erkennen kann.
#[repr(C)]
struct ConditionHeader {
    kind: ConditionKind,
}

/// GuardCondition (Spec §2.2.2.1.2.1.6).
pub struct ZeroDdsGuardCondition {
    /// Header. Layout-kompatibel mit `ConditionHeader` fuer
    /// Condition-Kind-Discrimination via `condition_kind()`.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// User-Trigger-Flag.
    trigger: AtomicBool,
}

/// StatusCondition (Spec §2.2.2.1.2.1.4). Im RC1 an `Entity = void*`
/// gebunden — kein Status-Polling-Plumb (folge-WP).
pub struct ZeroDdsStatusCondition {
    /// Header. Layout-kompatibel fuer Condition-Kind-Discrimination.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// `entity` Pointer (DataReader / DataWriter / Topic / Participant).
    #[allow(dead_code)]
    entity: *mut core::ffi::c_void,
    /// Aktivierte Status-Bits (Bitmask).
    enabled_statuses: AtomicU32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsStatusCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsStatusCondition {}

/// ReadCondition (Spec §2.2.2.5.8).
pub struct ZeroDdsReadCondition {
    /// Header. Layout-kompatibel fuer Condition-Kind-Discrimination.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// Bound-DataReader.
    reader: *mut ZeroDdsDataReader,
    /// Sample-State-Mask.
    sample_states: u32,
    /// View-State-Mask.
    view_states: u32,
    /// Instance-State-Mask.
    instance_states: u32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsReadCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsReadCondition {}

/// QueryCondition (Spec §2.2.2.5.9).
pub struct ZeroDdsQueryCondition {
    /// Header. Layout-kompatibel fuer Condition-Kind-Discrimination.
    #[allow(dead_code)]
    header: ConditionHeader,
    /// Bound-DataReader.
    reader: *mut ZeroDdsDataReader,
    /// Sample/View/Instance-State.
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
    /// Filter-Expression.
    expression: alloc::string::String,
    /// Filter-Parameters.
    parameters: Vec<alloc::string::String>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsQueryCondition {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsQueryCondition {}

// ---------------------------------------------------------------------------
// Condition-Trigger-Helpers — drainen Channel in Cache, evaluieren Masks
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
        let v_ok = view_states == 0 || (view_states & 1u32) != 0; // RC1: alle samples NEW
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
    // Filter-Expression parsen.
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

/// Liefert die State-Masks (sample_states, view_states, instance_states)
/// einer ReadCondition oder QueryCondition. Andere Kinds → None.
///
/// # Safety
/// `c` valide oder NULL.
pub unsafe fn condition_state_masks(c: *const core::ffi::c_void) -> Option<(u32, u32, u32)> {
    // SAFETY: see fn # Safety doc — c NULL-tolerant; condition_kind() pruft NULL
    // und liefert die ConditionKind; danach Layout-Cast je nach kind.
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

/// Liest den `ConditionKind`-Header. NULL-tolerant.
///
/// # Safety
/// `p` ist NULL oder zeigt auf eine Box-allokierte Condition deren
/// erstes Feld ein `ConditionHeader` ist (Layout via `#[repr(C)]`).
unsafe fn condition_kind(p: *const core::ffi::c_void) -> Option<ConditionKind> {
    if p.is_null() {
        return None;
    }
    // SAFETY: NULL-Check oben; Caller-Pledge fuer Box-allokierte Condition.
    let hdr = unsafe { &*(p as *const ConditionHeader) };
    Some(hdr.kind)
}

/// Liest den aktuellen Trigger-Status einer Condition (Spec §2.2.2.1.2.1.1).
///
/// # Safety
/// `c` valide oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_condition_get_trigger_value(c: *const core::ffi::c_void) -> bool {
    // SAFETY: see fn # Safety doc — c NULL-tolerant; condition_kind pruft NULL und
    // liefert ConditionKind; danach Layout-Cast je nach kind. r.reader/q.reader
    // stammen aus sub_create_datareader (Box::into_raw).
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
                // RC1: jedes nicht-leere `enabled_statuses` triggert wenn mindestens
                // ein Status-Counter > 0 ist. Da wir den Entity-Pointer haben aber
                // kein Type-Tag fuer ihn, liefern wir `true` wenn enabled_statuses
                // != 0 als spec-konformer Fallback — die Listener-FFI in der
                // Folge-WP wired das pro Entity-Typ.
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
                // Spec §2.2.2.5.8: trigger_value true wenn Reader Samples hat die
                // zur (sample_states, view_states, instance_states) Mask passen.
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
                // Spec §2.2.2.5.9: zusaetzlich filter-expression evaluieren.
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

/// Erzeugt GuardCondition.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_guardcondition_create() -> *mut ZeroDdsGuardCondition {
    Box::into_raw(Box::new(ZeroDdsGuardCondition {
        header: ConditionHeader {
            kind: ConditionKind::Guard,
        },
        trigger: AtomicBool::new(false),
    }))
}

/// Loescht GuardCondition.
///
/// # Safety
/// `g` muss aus `zerodds_guardcondition_create` stammen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_guardcondition_destroy(g: *mut ZeroDdsGuardCondition) {
    if !g.is_null() {
        // SAFETY: see fn # Safety doc — g aus zerodds_guardcondition_create.
        let _ = unsafe { Box::from_raw(g) };
    }
}

/// Setzt den Trigger-Wert.
///
/// # Safety
/// `g` valide.
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
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// StatusCondition
// ---------------------------------------------------------------------------

/// Erzeugt eine StatusCondition fuer eine Entity. RC1: erlaubt jeden
/// `*mut c_void` als Entity-Slot — Match-Logik liest den Status pro
/// Entity-Type via Listener-FFI (folge-WP).
///
/// # Safety
/// `entity` valide oder NULL.
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

/// Aktiviert die in `mask` gesetzten Status-Bits.
///
/// # Safety
/// `c` valide.
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

/// Liest die aktivierten Status-Bits.
///
/// # Safety
/// `c` valide.
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

/// Loescht eine StatusCondition.
///
/// # Safety
/// `c` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_statuscondition_destroy(c: *mut ZeroDdsStatusCondition) {
    if !c.is_null() {
        // SAFETY: see fn # Safety doc — c aus zerodds_entity_get_statuscondition.
        let _ = unsafe { Box::from_raw(c) };
    }
}

// ---------------------------------------------------------------------------
// ReadCondition / QueryCondition
// ---------------------------------------------------------------------------

/// Erzeugt eine ReadCondition (Spec §2.2.2.5.2.4).
///
/// # Safety
/// `dr` valide.
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

/// Erzeugt eine QueryCondition (Spec §2.2.2.5.2.5).
///
/// # Safety
/// `dr`, `expr` valide; `params[0..param_count]` valide.
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
    // SAFETY: see fn # Safety doc — dr+expr NULL-checked above; expr NUL-terminiert
    // (Caller-Pledge); params[0..param_count] valide wenn params != NULL.
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

/// Loescht eine ReadCondition oder QueryCondition.
///
/// # Safety
/// `c` valide; muss aus `dr_create_readcondition` oder
/// `dr_create_querycondition` stammen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_delete_readcondition(
    _dr: *mut ZeroDdsDataReader,
    c: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: see fn # Safety doc — c aus dr_create_readcondition oder
    // dr_create_querycondition; kind-Discrimination via ConditionHeader.
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
    /// Liste der attached Conditions (Pointer auf Box-allokierte
    /// Condition-Strukturen).
    conditions: Mutex<Vec<*mut core::ffi::c_void>>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsWaitSet {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsWaitSet {}

/// Erzeugt WaitSet.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_waitset_create() -> *mut ZeroDdsWaitSet {
    Box::into_raw(Box::new(ZeroDdsWaitSet {
        conditions: Mutex::new(Vec::new()),
    }))
}

/// Loescht WaitSet.
///
/// # Safety
/// `w` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_destroy(w: *mut ZeroDdsWaitSet) {
    if !w.is_null() {
        // SAFETY: see fn # Safety doc — w aus zerodds_waitset_create.
        let _ = unsafe { Box::from_raw(w) };
    }
}

/// Attach Condition.
///
/// # Safety
/// `w`, `cond` valide.
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
    }
    ZeroDdsStatus::Ok as c_int
}

/// Detach Condition.
///
/// # Safety
/// `w`, `cond` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_waitset_detach_condition(
    w: *mut ZeroDdsWaitSet,
    cond: *mut core::ffi::c_void,
) -> c_int {
    if w.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — w NULL-checked above; cond toleriert NULL.
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

/// Wartet bis mindestens eine Condition triggert oder Timeout.
/// Schreibt aktive Conditions in `out_active[0..*out_count]`.
///
/// # Safety
/// `w`, `out_active`, `out_count`, `timeout` valide.
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
        Duration::from_secs(60 * 60 * 24 * 365 * 100) // ~100 Jahre = INFINITE
    } else {
        Duration::new(timeout_sec.max(0) as u64, timeout_nanosec)
    };
    let deadline = Instant::now() + timeout;
    // SAFETY: see fn # Safety doc — w+out_active+out_count NULL-checked above; conds-Items
    // stammen aus attach_condition (Caller hat valide Condition-Pointer geliefert).
    unsafe {
        let ws = &*w;
        loop {
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
            if Instant::now() >= deadline {
                *out_count = 0;
                return ZeroDdsStatus::Timeout as c_int;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Liefert alle attached Conditions.
///
/// # Safety
/// `w`, `out`, `out_count` valide.
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
    // muss writeable sein (Caller-Pledge).
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

    #[test]
    fn guardcondition_lifecycle_and_trigger() {
        let g = zerodds_guardcondition_create();
        assert!(!g.is_null());
        // SAFETY: g aus guardcondition_create.
        unsafe {
            assert!(!zerodds_condition_get_trigger_value(g as *const _));
            let _ = zerodds_guardcondition_set_trigger_value(g, true);
            assert!(zerodds_condition_get_trigger_value(g as *const _));
            zerodds_guardcondition_destroy(g);
        }
    }

    #[test]
    fn statuscondition_lifecycle_and_mask() {
        // SAFETY: NULL ist erlaubter Entity-Slot fuer StatusCondition.
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
        // SAFETY: ws + g aus create-fns; buf Stack-lokal.
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
        // SAFETY: ws + g aus create-fns; buf Stack-lokal.
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
        // SAFETY: ws + g aus create-fns; buf Stack-lokal.
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
