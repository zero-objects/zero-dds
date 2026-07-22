// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Zero-copy same-host SHM loan FFI (feature `flatdata-loan`).
//!
//! Turns the writer loan API into a *real* zero-copy path: after enabling the
//! SHM loan, `loan_message` hands the caller a pointer **into a POSIX
//! shared-memory slot** (via the flatdata `PosixSlotAllocator` + the in-place
//! `slot_data_ptr`/`commit_in_place` primitives), and `commit_loan` finalizes
//! the slot in place — no staging copy. A same-host reader maps the same
//! segment and takes the slot zero-copy (a pointer into the writer's segment).
//!
//! **Runtime-eid based, shared across both FFI surfaces.** The loan state lives
//! in a process-local side registry keyed by `(runtime, entity-id)` — *not* by
//! the opaque FFI pointer — so the same writer reached through either FFI
//! surface shares one backend:
//!   - the DCPS path (`ZeroDdsDataWriter` / `ZeroDdsDataReader`): the
//!     `zerodds_dw_enable_shm_loan` + `zerodds_dr_enable_shm`/`_take_shm`/
//!     `_release_shm` entry points in this module, plus the transparent
//!     `loan_message`/`commit_loan`/`discard_loan` hooks in `extra_ffi`;
//!   - the runtime path (`ZeroDdsWriter` / `ZeroDdsReader`): the
//!     `zerodds_writer_enable_shm_loan` + `zerodds_reader_enable_shm`/… entry
//!     points in `lib.rs`, which the ROS-2 RMW bridge uses.
//!
//! The `(runtime, eid)` key is collision-safe across multiple runtimes in one
//! process (an `EntityId` is only unique within its runtime).
//!
//! Cross-host / non-SHM readers keep working transparently: commit also
//! publishes the sample over RTPS via `write_user_sample_borrowed`, so a
//! cross-host reader receives it on the wire while a same-host SHM reader reads
//! it from shared memory (different readers, different take APIs — no dedup).
//!
//! Discovery: on enable, the writer's `PID_SHM_LOCATOR` (ADR-0006) is set on
//! the runtime so the SEDP `PublicationData` advertises the segment.
//!
//! The module is gated `#[cfg(feature = "flatdata-loan")]` at its declaration
//! in `lib.rs`.

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int};
use std::sync::{Arc, Mutex, OnceLock};

use zerodds_dcps::runtime::DcpsRuntime;
use zerodds_flatdata::{PosixSlotAllocator, ShmLocator, SlotBackend, SlotHandle};
use zerodds_rtps::wire_types::EntityId;

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDataReader, ZeroDdsDataWriter};

/// Registry key: a runtime instance plus an entity-id. The runtime pointer
/// disambiguates equal `EntityId`s living in different runtimes in one process.
type Key = (usize, EntityId);

fn key(rt: &Arc<DcpsRuntime>, eid: EntityId) -> Key {
    (Arc::as_ptr(rt) as usize, eid)
}

/// Per-writer SHM loan state.
struct WriterShm {
    backend: Arc<PosixSlotAllocator>,
    capacity: usize,
    /// raw slot-data pointer (as usize) → reserved slot handle.
    loans: HashMap<usize, SlotHandle>,
}

/// Per-reader SHM state.
struct ReaderShm {
    backend: Arc<PosixSlotAllocator>,
    reader_index: u8,
}

fn writers() -> &'static Mutex<HashMap<Key, WriterShm>> {
    static W: OnceLock<Mutex<HashMap<Key, WriterShm>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(HashMap::new()))
}

fn readers() -> &'static Mutex<HashMap<Key, ReaderShm>> {
    static R: OnceLock<Mutex<HashMap<Key, ReaderShm>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// Delivery mode (`zerodds-delivery-modes-1.0` §3/§4)
// ---------------------------------------------------------------------------

/// Portable serialized form; delivered over RTPS (cross-host + cross-vendor).
pub(crate) const MODE_PORTABLE: u8 = 0;
/// Raw in-memory form in ZeroDDS's own SHM slot; same-host only, no wire.
pub(crate) const MODE_RAW_SAME_HOST: u8 = 1;
/// iceoryx2-delivered raw form; same-host, cross-stack. Not wired on this path
/// yet (`zerodds-delivery-modes-1.0` §11).
pub(crate) const MODE_ICEORYX: u8 = 2;

/// Per-writer configured delivery mode, keyed by `(runtime, eid)`. Set via the
/// FFI setter; consulted at commit. Absent → the env/`Portable` default.
fn modes() -> &'static Mutex<HashMap<Key, u8>> {
    static M: OnceLock<Mutex<HashMap<Key, u8>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The participant-wide default from `ZERODDS_DELIVERY_MODE`, parsed once.
/// Unknown / `iceoryx` (not yet wired) fall back to the interop-safe
/// `Portable`, so no deployment silently loses delivery.
fn env_default_mode() -> u8 {
    static D: OnceLock<u8> = OnceLock::new();
    *D.get_or_init(
        || match std::env::var("ZERODDS_DELIVERY_MODE").ok().as_deref() {
            Some("raw-same-host") => MODE_RAW_SAME_HOST,
            _ => MODE_PORTABLE,
        },
    )
}

/// Whether a sample in this mode is published onto the RTPS wire. Only
/// `Portable` reaches the wire; the raw modes are same-host only and would put
/// non-portable bytes on the wire if published — so they do not. Pure +
/// `pub(crate)` for direct unit testing.
pub(crate) fn publishes_to_wire(mode: u8) -> bool {
    mode == MODE_PORTABLE
}

fn configured_mode(rt: &Arc<DcpsRuntime>, eid: EntityId) -> u8 {
    modes()
        .lock()
        .ok()
        .and_then(|m| m.get(&key(rt, eid)).copied())
        .unwrap_or_else(env_default_mode)
}

/// Sets the delivery mode for this `(runtime, eid)` writer. `Iceoryx` is not
/// yet wired on this path → `Unsupported`. Invalid values → `BadParameter`.
pub(crate) fn set_delivery_mode(rt: &Arc<DcpsRuntime>, eid: EntityId, mode: u8) -> c_int {
    match mode {
        MODE_PORTABLE | MODE_RAW_SAME_HOST => {}
        MODE_ICEORYX => {
            // Accepted only when the iceoryx delivery backend is compiled in.
            #[cfg(not(feature = "delivery-iceoryx"))]
            return ZeroDdsStatus::Unsupported as c_int;
        }
        _ => return ZeroDdsStatus::BadParameter as c_int,
    }
    let Ok(mut m) = modes().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    m.insert(key(rt, eid), mode);
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// Iceoryx delivery (`zerodds-delivery-modes-1.0` §3.3, feature delivery-iceoryx)
// ---------------------------------------------------------------------------
//
// Routes a writer's samples over iceoryx2 so iceoryx-based peers on the same
// host can read them. The byte FFI has no Rust type, so it uses the
// byte-oriented `RawIceoryx2*` ports (thread-safe `ipc_threadsafe` service,
// hence storable in the global `(runtime, eid)` registry). The loan buffer is
// a heap buffer the caller fills; `commit` copies it into an iceoryx slot and
// sends (one copy at the boundary — end-to-end zero-copy into the iceoryx slot
// is a refinement, spec §11). No RTPS publish (same-host only).

#[cfg(feature = "delivery-iceoryx")]
use zerodds_flatdata::{RawIceoryx2Publisher, RawIceoryx2Subscriber};

#[cfg(feature = "delivery-iceoryx")]
struct IceoryxWriter {
    publisher: RawIceoryx2Publisher,
    /// heap loan buffers tracked by data pointer; sent into iceoryx at commit.
    loans: HashMap<usize, Vec<u8>>,
}

#[cfg(feature = "delivery-iceoryx")]
struct IceoryxReader {
    // `Arc` so a blocking wait (`try_raw_wait`) can clone the handle and release
    // the registry lock before parking on the listener.
    subscriber: Arc<RawIceoryx2Subscriber>,
    /// received buffers held until release, keyed by a synthetic slot id.
    pending: HashMap<u32, Vec<u8>>,
    next_slot: u32,
}

#[cfg(feature = "delivery-iceoryx")]
fn ice_writers() -> &'static Mutex<HashMap<Key, IceoryxWriter>> {
    static W: OnceLock<Mutex<HashMap<Key, IceoryxWriter>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(feature = "delivery-iceoryx")]
fn ice_readers() -> &'static Mutex<HashMap<Key, IceoryxReader>> {
    static R: OnceLock<Mutex<HashMap<Key, IceoryxReader>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Creates an iceoryx2 publisher for this `(runtime, eid)` writer on
/// `service_name`, with a max payload of `max_len` bytes.
#[cfg(feature = "delivery-iceoryx")]
pub(crate) fn enable_iceoryx_writer(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    service_name: String,
    max_len: usize,
) -> c_int {
    if service_name.is_empty() || max_len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let publisher = match RawIceoryx2Publisher::create(&service_name, max_len) {
        Ok(p) => p,
        Err(_) => return ZeroDdsStatus::OutOfResources as c_int,
    };
    let Ok(mut w) = ice_writers().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    w.insert(
        key(rt, eid),
        IceoryxWriter {
            publisher,
            loans: HashMap::new(),
        },
    );
    ZeroDdsStatus::Ok as c_int
}

/// Creates an iceoryx2 subscriber for this `(runtime, eid)` reader on
/// `service_name`.
#[cfg(feature = "delivery-iceoryx")]
pub(crate) fn enable_iceoryx_reader(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    service_name: String,
) -> c_int {
    if service_name.is_empty() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let subscriber = match RawIceoryx2Subscriber::create(&service_name) {
        Ok(s) => s,
        Err(_) => return ZeroDdsStatus::PreconditionNotMet as c_int,
    };
    let Ok(mut r) = ice_readers().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    r.insert(
        key(rt, eid),
        IceoryxReader {
            subscriber: Arc::new(subscriber),
            pending: HashMap::new(),
            next_slot: 0,
        },
    );
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// Writer core — shared by both FFI surfaces
// ---------------------------------------------------------------------------

/// Creates the POSIX shm segment for this `(runtime, eid)` writer and
/// advertises it via `PID_SHM_LOCATOR`. Returns a `ZeroDdsStatus` as `c_int`.
pub(crate) fn enable_writer(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    path: String,
    slot_count: usize,
    slot_capacity: usize,
) -> c_int {
    if slot_count == 0 || slot_capacity == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let backend = match PosixSlotAllocator::create(path.clone(), slot_count, slot_capacity) {
        Ok(b) => Arc::new(b),
        Err(_) => return ZeroDdsStatus::OutOfResources as c_int,
    };
    let locator = ShmLocator {
        hostname_hash: zerodds_flatdata::fnv1a_32(host_id().as_bytes()),
        uid: process_uid(),
        slot_count: u32::try_from(slot_count).unwrap_or(u32::MAX),
        slot_size: u32::try_from(backend.slot_total_size()).unwrap_or(u32::MAX),
        segment_path: path,
    };
    if let Ok(bytes) = locator.to_bytes_le() {
        rt.set_shm_locator(eid, bytes);
    }
    let Ok(mut w) = writers().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    w.insert(
        key(rt, eid),
        WriterShm {
            backend,
            capacity: slot_capacity,
            loans: HashMap::new(),
        },
    );
    ZeroDdsStatus::Ok as c_int
}

/// Transparent loan: `Some(status)` (and writes `out_ptr`/`out_len`) when this
/// `(runtime, eid)` writer has SHM loan enabled; `None` to fall through to the
/// heap path.
///
/// # Safety
/// `out_ptr`/`out_len` valid (checked by the caller).
pub(crate) unsafe fn try_loan(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> Option<c_int> {
    // Iceoryx writer: hand out a heap buffer the caller fills; commit copies it
    // into an iceoryx slot and sends.
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut iw) = ice_writers().lock() {
        if let Some(entry) = iw.get_mut(&key(rt, eid)) {
            let mut buf = vec![0u8; len];
            let ptr = buf.as_mut_ptr();
            entry.loans.insert(ptr as usize, buf);
            // SAFETY: out pointers NULL-checked by the caller.
            unsafe {
                *out_ptr = ptr;
                *out_len = len;
            }
            return Some(ZeroDdsStatus::Ok as c_int);
        }
    }
    let mut w = writers().lock().ok()?;
    let entry = w.get_mut(&key(rt, eid))?;
    if len > entry.capacity {
        return Some(ZeroDdsStatus::OutOfResources as c_int);
    }
    // mask 0 → reuse any non-loaned slot (keep-last-N ring); a same-host reader
    // reading promptly keeps unread slots from being recycled mid-flight.
    let handle = match entry.backend.reserve_slot(0) {
        Ok(h) => h,
        Err(_) => return Some(ZeroDdsStatus::OutOfResources as c_int),
    };
    let (ptr, _cap) = match entry.backend.slot_data_ptr(handle) {
        Ok(p) => p,
        Err(_) => {
            let _ = entry.backend.discard_slot(handle);
            return Some(ZeroDdsStatus::Error as c_int);
        }
    };
    entry.loans.insert(ptr as usize, handle);
    // SAFETY: out pointers NULL-checked by the caller.
    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }
    Some(ZeroDdsStatus::Ok as c_int)
}

/// Transparent commit: `Some(status)` when `ptr` is an active SHM loan of this
/// `(runtime, eid)` writer; `None` to fall through to the heap path.
///
/// # Safety
/// `ptr` valid (checked by the caller).
pub(crate) unsafe fn try_commit(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    ptr: *mut u8,
    len: usize,
) -> Option<c_int> {
    // Iceoryx writer: send the filled heap buffer over iceoryx2 (no RTPS).
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut iw) = ice_writers().lock() {
        if let Some(entry) = iw.get_mut(&key(rt, eid)) {
            let Some(buf) = entry.loans.remove(&(ptr as usize)) else {
                return Some(ZeroDdsStatus::BadParameter as c_int);
            };
            let n = len.min(buf.len());
            return Some(match entry.publisher.send(&buf[..n]) {
                Ok(()) => ZeroDdsStatus::Ok as c_int,
                Err(_) => ZeroDdsStatus::Error as c_int,
            });
        }
    }
    let (backend, handle) = {
        let mut w = writers().lock().ok()?;
        let entry = w.get_mut(&key(rt, eid))?;
        let handle = entry.loans.remove(&(ptr as usize))?;
        (Arc::clone(&entry.backend), handle)
    };
    // Finalize the slot in place (no copy) — same-host SHM readers see it.
    if backend.commit_in_place(handle, len).is_err() {
        return Some(ZeroDdsStatus::Error as c_int);
    }
    // Portable mode also publishes over RTPS so cross-host / non-SHM readers
    // receive the sample (`zerodds-delivery-modes-1.0` §3.1/§5). The raw modes
    // are same-host only and would put non-portable bytes on the wire — they
    // skip the publish entirely (no wire, no double delivery).
    if publishes_to_wire(configured_mode(rt, eid)) {
        if let Ok((rptr, n)) = backend.slot_read_ptr(handle) {
            // SAFETY: rptr is the slot data area, valid for n bytes (= sample_size).
            let bytes = unsafe { core::slice::from_raw_parts(rptr, n) };
            return Some(match rt.write_user_sample_borrowed(eid, bytes) {
                Ok(()) => ZeroDdsStatus::Ok as c_int,
                Err(_) => ZeroDdsStatus::Error as c_int,
            });
        }
    }
    Some(ZeroDdsStatus::Ok as c_int)
}

/// Transparent discard: `Some(status)` when `ptr` is an active SHM loan of this
/// `(runtime, eid)` writer; `None` to fall through to the heap path.
pub(crate) fn try_discard(rt: &Arc<DcpsRuntime>, eid: EntityId, ptr: *mut u8) -> Option<c_int> {
    // Iceoryx writer: drop the heap loan buffer (nothing sent).
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut iw) = ice_writers().lock() {
        if let Some(entry) = iw.get_mut(&key(rt, eid)) {
            return Some(if entry.loans.remove(&(ptr as usize)).is_some() {
                ZeroDdsStatus::Ok as c_int
            } else {
                ZeroDdsStatus::BadParameter as c_int
            });
        }
    }
    let mut w = writers().lock().ok()?;
    let entry = w.get_mut(&key(rt, eid))?;
    let handle = entry.loans.remove(&(ptr as usize))?;
    Some(match entry.backend.discard_slot(handle) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    })
}

/// Removes a writer's SHM state (called on writer destroy). Idempotent.
pub(crate) fn forget_writer(rt: &Arc<DcpsRuntime>, eid: EntityId) {
    if let Ok(mut w) = writers().lock() {
        w.remove(&key(rt, eid));
    }
    if let Ok(mut m) = modes().lock() {
        m.remove(&key(rt, eid));
    }
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut iw) = ice_writers().lock() {
        iw.remove(&key(rt, eid));
    }
}

// ---------------------------------------------------------------------------
// Reader core — shared by both FFI surfaces
// ---------------------------------------------------------------------------

/// Maps the writer's shm segment at flink path `path` for zero-copy reads.
pub(crate) fn enable_reader(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    path: String,
    reader_index: u8,
) -> c_int {
    if reader_index >= 32 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let backend = match PosixSlotAllocator::attach(path) {
        Ok(b) => Arc::new(b),
        Err(_) => return ZeroDdsStatus::PreconditionNotMet as c_int,
    };
    let Ok(mut r) = readers().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    r.insert(
        key(rt, eid),
        ReaderShm {
            backend,
            reader_index,
        },
    );
    ZeroDdsStatus::Ok as c_int
}

/// Zero-copy take for this `(runtime, eid)` reader.
///
/// # Safety
/// `out_ptr`/`out_len`/`out_slot` valid (checked by the caller).
pub(crate) unsafe fn try_take(
    rt: &Arc<DcpsRuntime>,
    eid: EntityId,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    out_slot: *mut u32,
) -> c_int {
    // Iceoryx reader: receive the next sample (copied into an owned buffer held
    // until release).
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut ir) = ice_readers().lock() {
        if let Some(entry) = ir.get_mut(&key(rt, eid)) {
            return match entry.subscriber.receive() {
                Ok(Some(buf)) => {
                    let slot = entry.next_slot;
                    entry.next_slot = entry.next_slot.wrapping_add(1);
                    let n = buf.len();
                    let p = buf.as_ptr();
                    entry.pending.insert(slot, buf);
                    // SAFETY: out pointers NULL-checked by the caller.
                    unsafe {
                        *out_ptr = p;
                        *out_len = n;
                        *out_slot = slot;
                    }
                    ZeroDdsStatus::Ok as c_int
                }
                Ok(None) => ZeroDdsStatus::NoData as c_int,
                Err(_) => ZeroDdsStatus::Error as c_int,
            };
        }
    }
    let Ok(r) = readers().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    let Some(entry) = r.get(&key(rt, eid)) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    let handle = match entry.backend.next_unread_slot(entry.reader_index) {
        Ok(Some(h)) => h,
        Ok(None) => return ZeroDdsStatus::NoData as c_int,
        Err(_) => return ZeroDdsStatus::Error as c_int,
    };
    match entry.backend.slot_read_ptr(handle) {
        Ok((ptr, n)) => {
            // SAFETY: out pointers NULL-checked by the caller.
            unsafe {
                *out_ptr = ptr;
                *out_len = n;
                *out_slot = handle.slot_index;
            }
            ZeroDdsStatus::Ok as c_int
        }
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Releases a slot previously returned by `try_take`.
pub(crate) fn try_release(rt: &Arc<DcpsRuntime>, eid: EntityId, slot_index: u32) -> c_int {
    // Iceoryx reader: drop the held received buffer for this slot id.
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut ir) = ice_readers().lock() {
        if let Some(entry) = ir.get_mut(&key(rt, eid)) {
            return if entry.pending.remove(&slot_index).is_some() {
                ZeroDdsStatus::Ok as c_int
            } else {
                ZeroDdsStatus::PreconditionNotMet as c_int
            };
        }
    }
    let Ok(r) = readers().lock() else {
        return ZeroDdsStatus::Error as c_int;
    };
    let Some(entry) = r.get(&key(rt, eid)) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    let handle = SlotHandle {
        segment_id: 0,
        slot_index,
    };
    match entry.backend.mark_read(handle, entry.reader_index) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Removes a reader's SHM state (called on reader destroy). Idempotent.
pub(crate) fn forget_reader(rt: &Arc<DcpsRuntime>, eid: EntityId) {
    if let Ok(mut r) = readers().lock() {
        r.remove(&key(rt, eid));
    }
    #[cfg(feature = "delivery-iceoryx")]
    if let Ok(mut ir) = ice_readers().lock() {
        ir.remove(&key(rt, eid));
    }
}

/// Blocks until this `(runtime, eid)` reader's raw source signals a new sample
/// or `timeout_ms` elapses — event-driven, no busy-poll. SHM waits on the
/// flatdata change-generation futex; iceoryx on the listener. The registry lock
/// is released before parking (a clone of the backend/subscriber handle is held
/// across the wait), so concurrent takes are not blocked.
///
/// Returns `Ok` when woken by an actual signal (a sample is likely available),
/// `NoData` on a plain timeout (so an idle doorbell does not wake the consumer),
/// `PreconditionNotMet` if the reader has no raw source. A spurious `Ok` is
/// harmless: the caller re-checks via `try_take`.
pub(crate) fn try_raw_wait(rt: &Arc<DcpsRuntime>, eid: EntityId, timeout_ms: u64) -> c_int {
    let dur = core::time::Duration::from_millis(timeout_ms);
    // Iceoryx reader: park on the listener (clone the Arc, drop the lock).
    #[cfg(feature = "delivery-iceoryx")]
    {
        let sub = {
            let Ok(r) = ice_readers().lock() else {
                return ZeroDdsStatus::Error as c_int;
            };
            r.get(&key(rt, eid)).map(|e| Arc::clone(&e.subscriber))
        };
        if let Some(sub) = sub {
            return if sub.wait(dur) {
                ZeroDdsStatus::Ok as c_int
            } else {
                ZeroDdsStatus::NoData as c_int
            };
        }
    }
    // SHM reader: park on the change-generation futex (clone the backend Arc,
    // capture the generation, drop the lock).
    let backend_gen = {
        let Ok(r) = readers().lock() else {
            return ZeroDdsStatus::Error as c_int;
        };
        r.get(&key(rt, eid))
            .map(|e| (Arc::clone(&e.backend), e.backend.notify_generation()))
    };
    let Some((backend, generation)) = backend_gen else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    backend.wait_for_change(generation, dur);
    // Woken by a commit iff the change-generation advanced (else it timed out).
    if backend.notify_generation() != generation {
        ZeroDdsStatus::Ok as c_int
    } else {
        ZeroDdsStatus::NoData as c_int
    }
}

// ---------------------------------------------------------------------------
// DCPS FFI surface (ZeroDdsDataWriter / ZeroDdsDataReader)
// ---------------------------------------------------------------------------

/// Enables zero-copy SHM loan on a DCPS DataWriter. Creates a POSIX
/// shared-memory segment of `slot_count` slots × `slot_capacity` bytes at the
/// flink path `name`. After this, `zerodds_dw_loan_message` returns a pointer
/// into a SHM slot and `zerodds_dw_commit_loan` finalizes it in place.
///
/// # Safety
/// `dw` is a valid registered DataWriter; `name` is a NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_enable_shm_loan(
    dw: *mut ZeroDdsDataWriter,
    name: *const c_char,
    slot_count: usize,
    slot_capacity: usize,
) -> c_int {
    if dw.is_null() || name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let path = match unsafe { c_str(name) } {
        Ok(s) => s,
        Err(rc) => return rc,
    };
    // SAFETY: dw valid per the contract.
    let dwr = unsafe { &*dw };
    enable_writer(&dwr.rt, dwr.eid, path, slot_count, slot_capacity)
}

/// Sets the delivery mode of a DCPS DataWriter (`zerodds-delivery-modes-1.0`
/// §3/§4): `0`=Portable (default, interop-safe), `1`=RawSameHost (same-host,
/// no wire). `2`=Iceoryx routes over the iceoryx2 bridge and is opt-in: it
/// requires building with the `delivery-iceoryx` feature and returns
/// `Unsupported` when that feature is not compiled in. Other values →
/// `BadParameter`.
///
/// # Safety
/// `dw` is a valid registered DataWriter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_set_delivery_mode(
    dw: *mut ZeroDdsDataWriter,
    mode: c_int,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let Ok(mode) = u8::try_from(mode) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: dw valid per the contract.
    let dwr = unsafe { &*dw };
    set_delivery_mode(&dwr.rt, dwr.eid, mode)
}

/// Maps the writer's SHM segment at flink path `name` on a DCPS DataReader for
/// zero-copy reads. `reader_index` is the reader's bit (0..32) in the slot mask.
///
/// # Safety
/// `dr` valid; `name` is a NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_enable_shm(
    dr: *mut ZeroDdsDataReader,
    name: *const c_char,
    reader_index: u8,
) -> c_int {
    if dr.is_null() || name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let path = match unsafe { c_str(name) } {
        Ok(s) => s,
        Err(rc) => return rc,
    };
    // SAFETY: dr valid per the contract.
    let drr = unsafe { &*dr };
    enable_reader(&drr.rt, drr.eid, path, reader_index)
}

/// Zero-copy take on a DCPS DataReader: returns a read-only pointer into the
/// writer's SHM slot, its length and the slot index (for
/// `zerodds_dr_release_shm`). `NoData` when nothing is pending.
///
/// # Safety
/// `dr`/`out_ptr`/`out_len`/`out_slot` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_shm(
    dr: *mut ZeroDdsDataReader,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    out_slot: *mut u32,
) -> c_int {
    if dr.is_null() || out_ptr.is_null() || out_len.is_null() || out_slot.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: dr valid; out pointers NULL-checked above.
    let drr = unsafe { &*dr };
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    unsafe { try_take(&drr.rt, drr.eid, out_ptr, out_len, out_slot) }
}

/// Releases a slot previously returned by `zerodds_dr_take_shm`.
///
/// # Safety
/// `dr` valid; `slot_index` from a prior `zerodds_dr_take_shm`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_release_shm(
    dr: *mut ZeroDdsDataReader,
    slot_index: u32,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: dr valid per the contract.
    let drr = unsafe { &*dr };
    try_release(&drr.rt, drr.eid, slot_index)
}

/// Enables `Iceoryx` delivery on a DCPS DataWriter (feature `delivery-iceoryx`):
/// publishes the writer's samples over the iceoryx2 service `service_name`, with
/// a max payload of `max_len` bytes. The loan API then routes through iceoryx2
/// and commit does not publish over RTPS.
///
/// # Safety
/// `dw` is a valid registered DataWriter; `service_name` is a NUL-terminated C
/// string.
#[cfg(feature = "delivery-iceoryx")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_enable_iceoryx(
    dw: *mut ZeroDdsDataWriter,
    service_name: *const c_char,
    max_len: usize,
) -> c_int {
    if dw.is_null() || service_name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let service = match unsafe { c_str(service_name) } {
        Ok(s) => s,
        Err(rc) => return rc,
    };
    // SAFETY: dw valid per the contract.
    let dwr = unsafe { &*dw };
    enable_iceoryx_writer(&dwr.rt, dwr.eid, service, max_len)
}

/// Enables `Iceoryx` delivery on a DCPS DataReader: receives samples from the
/// iceoryx2 service `service_name` via `zerodds_dr_take_shm` /
/// `zerodds_dr_release_shm`.
///
/// # Safety
/// `dr` valid; `service_name` is a NUL-terminated C string.
#[cfg(feature = "delivery-iceoryx")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_enable_iceoryx(
    dr: *mut ZeroDdsDataReader,
    service_name: *const c_char,
) -> c_int {
    if dr.is_null() || service_name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let service = match unsafe { c_str(service_name) } {
        Ok(s) => s,
        Err(rc) => return rc,
    };
    // SAFETY: dr valid per the contract.
    let drr = unsafe { &*dr };
    enable_iceoryx_reader(&drr.rt, drr.eid, service)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reads a NUL-terminated C string as an owned UTF-8 `String`, mapping a
/// decode failure to `InvalidUtf8`.
///
/// # Safety
/// `name` is a valid NUL-terminated C string.
unsafe fn c_str(name: *const c_char) -> Result<String, c_int> {
    // SAFETY: name is a NUL-terminated C string per the contract.
    match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Err(ZeroDdsStatus::InvalidUtf8 as c_int),
    }
}

fn host_id() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string())
}

fn process_uid() -> u32 {
    // Best-effort: the locator's uid only needs to be stable per host/user for
    // the same-host check; 0 is a safe default where the uid is unavailable.
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_portable_reaches_the_wire() {
        // Portable is published over RTPS; the raw modes are same-host only and
        // must never put non-portable bytes on the wire.
        assert!(publishes_to_wire(MODE_PORTABLE));
        assert!(!publishes_to_wire(MODE_RAW_SAME_HOST));
        assert!(!publishes_to_wire(MODE_ICEORYX));
    }
}
