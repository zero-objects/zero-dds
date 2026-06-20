// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ZeroDDS C-FFI — Cross-Language-Hub.
//!
//! Crate `zerodds-c-api`. Safety classification: **STANDARD**.
//!
//! This crate exports an `extern "C"` layer over the ZeroDDS
//! DCPS runtime, so that non-Rust languages (C++, C#, TypeScript, C)
//! can access ZeroDDS through a uniform binary interface.
//!
//! The generated `include/zerodds.h` (via the cbindgen build script) is
//! the contract document for all consumers.
//!
//! # Type model — deliberately untyped
//!
//! The C-FFI is **byte-oriented**: a `Topic` carries a
//! `topic_name` + `type_name` string, a `write` takes a
//! `(*const u8, len)` buffer with the already-CDR-encoded sample bytes,
//! a `take` returns the raw wire bytes. The CDR encode/decode logic
//! lives in the language bindings (idl-cpp emits a C++ encoder, idl-csharp
//! a C# encoder, etc.) — the C-FFI is neutral.
//!
//! Advantages:
//! * No generic-type acrobatics through FFI.
//! * Wire-drift tests are transparent: every byte buffer passes 1:1.
//! * The Apex.AI plugin and ROS-2 RMW can keep their own marshaling paths.
//!
//! # Handle model
//!
//! All objects are exposed as opaque pointers. Callers must
//! pair `*_destroy()`. Memory ownership is explicit:
//! * `zerodds_runtime_create()` -> the caller owns it; `zerodds_runtime_destroy()`.
//! * `zerodds_writer_create()` -> the caller owns it; must be before runtime destroy.
//! * `zerodds_reader_take()` -> the `out_buf` bytes must be freed with
//!   `zerodds_buffer_free()`.
//!
//! # What does NOT go through the C-FFI
//!
//! * Java + Python — their own bridges (jni-rs / pyo3). A direct
//!   Rust↔language path without a C intermediate layer.
//! * QoS builders with complex default logic — simplified knobs
//!   in the FFI; full QoS control only via the Rust API.

#![warn(missing_docs)]
// The FFI module needs `unsafe` code, hence no `#![deny(unsafe_code)]`.
// Instead one SAFETY comment per `unsafe` block.
#![allow(clippy::missing_safety_doc)]
// FFI boundary: pointer args are by design the caller's duty; internal
// helper functions taking `*mut` args are designed as `pub fn`
// (not `unsafe fn`) so that their callers from FFI functions can use them
// without re-wrapping in `unsafe` blocks — the `unsafe` block
// sits at the FFI-extern function level.
#![allow(clippy::not_unsafe_ptr_arg_deref)]
// The listener callback path uses `cb.unwrap()` after a `cb.is_some()` check.
#![allow(clippy::unwrap_used)]
// Some QoS field assignments follow the builder pattern instead of struct init.
#![allow(clippy::field_reassign_with_default)]
// Lifetime elision in `unsafe fn cstr_to_str<'a>` is necessary at the FFI edge
// for the borrow lifetimes of the caller string.
#![allow(clippy::needless_lifetimes)]
// Pub fields in opaque FFI wrapper structs are documented at the
// struct level; per-field docs are redundant.
#![allow(missing_docs)]
// QoS policies like DeadlineQosPolicy implement Copy, but in
// generic `Clone`-based foreach patterns they read more consistently with
// `.clone()`.
#![allow(clippy::clone_on_copy)]
// `arr.lock().map(|g| g.clone()).unwrap_or_default()` is clearer than
// `.map_or_else(Default::default, |g| g.clone())`.
#![allow(clippy::map_unwrap_or)]
// FFI fatal path: `zerodds_runtime_create`/`_write` report errors that
// the C caller otherwise does not see via `eprintln!` on stderr — there is
// no other diagnostic channel at the C boundary.
#![allow(clippy::print_stderr)]

extern crate alloc;

pub mod builtin_ffi;
pub mod condition_ffi;
pub mod entities;
pub mod extra_ffi;
pub mod factory_ffi;
pub(crate) mod ffi_helpers;
pub mod listener_ffi;
pub mod participant_ffi;
pub mod publisher_ffi;
pub mod qos_ffi;
#[cfg(feature = "security")]
pub mod security_ffi;
#[cfg(feature = "flatdata-loan")]
pub mod shm_loan_ffi;
pub mod subscriber_ffi;
pub mod topic_ffi;

/// XCDR2 TypeSupport FFI — implements the `zerodds-xcdr2-c-1.0` vendor spec.
pub mod xcdr2;

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::slice;
use std::ffi::CStr;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy,
    OwnershipKind,
};
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

pub(crate) fn random_guid_prefix() -> GuidPrefix {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let pid = std::process::id();
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = [0u8; 12];
    // Bytes 0-3: deterministic host ID — identical for all processes
    // on the same host, so that `GuidPrefix::is_same_host` takes effect (same-host
    // SHM and fragmentation optimizations). Bytes 4-7: PID, bytes 8-11:
    // time+counter → globally unique. Identical layout to
    // `zerodds_dcps::participant::random_guid_prefix`.
    bytes[0..4].copy_from_slice(&zerodds_dcps::participant::host_id_bytes());
    bytes[4..8].copy_from_slice(&pid.to_le_bytes());
    bytes[8..12].copy_from_slice(&(t as u32).wrapping_add(c).to_le_bytes());
    GuidPrefix::from_bytes(bytes)
}

// ============================================================================
// Status-Codes
// ============================================================================

/// FFI status codes. 0 = OK, negative values = error.
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsStatus {
    /// Operation successful.
    Ok = 0,
    /// Generic error (details in `zerodds_last_error()`).
    Error = -1,
    /// NULL pointer where not allowed.
    BadHandle = -2,
    /// Invalid UTF-8 string.
    InvalidUtf8 = -3,
    /// Operation blocked and the timeout elapsed.
    Timeout = -4,
    /// Precondition not met (e.g. reader/writer already destroyed).
    PreconditionNotMet = -5,
    /// Invalid parameter value (e.g. len=0, negative size).
    BadParameter = -6,
    /// Operation returned no data (e.g. take() on an empty reader).
    NoData = -7,
    /// Resource limit reached.
    OutOfResources = -8,
    /// Entity not enabled.
    NotEnabled = -9,
    /// QoS policy is immutable.
    ImmutablePolicy = -10,
    /// QoS policies inconsistent.
    InconsistentPolicy = -11,
    /// Entity already deleted.
    AlreadyDeleted = -12,
    /// Operation not supported (profiles/stretch goals).
    Unsupported = -13,
    /// Call in an incompatible context.
    IllegalOperation = -14,
}

// ============================================================================
// Opaque handles
// ============================================================================

/// Opaque runtime handle. Holds the DcpsRuntime + spawned threads.
pub struct ZeroDdsRuntime {
    rt: Arc<DcpsRuntime>,
    /// Track spawned worker thread(s) for clean shutdown.
    _shutdown: (),
}

/// Opaque writer handle.
pub struct ZeroDdsWriter {
    rt: Arc<DcpsRuntime>,
    eid: EntityId,
}

/// Opaque reader handle.
pub struct ZeroDdsReader {
    rt: Arc<DcpsRuntime>,
    eid: EntityId,
    rx: Mutex<mpsc::Receiver<zerodds_dcps::runtime::UserSample>>,
}

// ============================================================================
// Lifecycle — Runtime
// ============================================================================

/// Creates a new ZeroDDS runtime on the given domain ID.
///
/// # Safety
/// The return value is a heap-allocated pointer; the caller must
/// free it with `zerodds_runtime_destroy`.
///
/// Returns `NULL` on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create(domain_id: u32) -> *mut ZeroDdsRuntime {
    let cfg = RuntimeConfig::default();
    let rt = match DcpsRuntime::start(domain_id as i32, random_guid_prefix(), cfg) {
        Ok(r) => r,
        Err(e) => {
            // A silent NULL is a grenade without a safety pin —
            // see the A2 bench bug (domain > 232 → port overflow →
            // start() failed → NULL → the caller only crashed at
            // wait_for_peers with a cryptic BadHandle).
            eprintln!("zerodds_runtime_create(domain_id={domain_id}) failed: {e:?}");
            return ptr::null_mut();
        }
    };
    // `DcpsRuntime::start` already returns an `Arc<DcpsRuntime>`.
    let handle = Box::new(ZeroDdsRuntime { rt, _shutdown: () });
    Box::into_raw(handle)
}

/// Destroys a runtime. NULL-safe.
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_destroy(runtime: *mut ZeroDdsRuntime) {
    if runtime.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — runtime from zerodds_runtime_create (Box::into_raw).
    let _ = unsafe { Box::from_raw(runtime) };
}

/// Prints phase timing counters to stderr (active only if the
/// runtime code was started with `ZERODDS_PHASE_TIMING=1`).
///
/// This is called by the bench app code before `runtime_destroy`, because
/// the runtime drop-path threads hold the Arc refcount and the
/// real drop only fires on process exit.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_phase_dump() {
    use zerodds_dcps::runtime::{
        PHASE_HANDLE_SUB_NS, PHASE_HANDLE_USER_CALLS, PHASE_HANDLE_USER_NS, PHASE_WRITE_SUB_NS,
        PHASE_WRITE_USER_CALLS, PHASE_WRITE_USER_NS,
    };
    let hu_ns = PHASE_HANDLE_USER_NS.load(core::sync::atomic::Ordering::Relaxed);
    let hu_n = PHASE_HANDLE_USER_CALLS.load(core::sync::atomic::Ordering::Relaxed);
    let wu_ns = PHASE_WRITE_USER_NS.load(core::sync::atomic::Ordering::Relaxed);
    let wu_n = PHASE_WRITE_USER_CALLS.load(core::sync::atomic::Ordering::Relaxed);
    let hu_us = if hu_n > 0 {
        hu_ns as f64 / hu_n as f64 / 1000.0
    } else {
        0.0
    };
    let wu_us = if wu_n > 0 {
        wu_ns as f64 / wu_n as f64 / 1000.0
    } else {
        0.0
    };
    eprintln!(
        "[ZERODDS_PHASE] handle_user_datagram:  N={}  avg={:.3}us  total={:.1}ms",
        hu_n,
        hu_us,
        hu_ns as f64 / 1_000_000.0
    );
    eprintln!(
        "[ZERODDS_PHASE] write_user_sample:      N={}  avg={:.3}us  total={:.1}ms",
        wu_n,
        wu_us,
        wu_ns as f64 / 1_000_000.0
    );
    if wu_n > 0 {
        let names = [
            "write[lookup]",
            "write[lock]  ",
            "write[wwh]   ",
            "write[send]  ",
        ];
        for (i, name) in names.iter().enumerate() {
            let ns = PHASE_WRITE_SUB_NS[i].load(core::sync::atomic::Ordering::Relaxed);
            eprintln!(
                "[ZERODDS_PHASE]   sub[{}]: avg={:.3}us  total={:.1}ms",
                name,
                ns as f64 / wu_n as f64 / 1000.0,
                ns as f64 / 1_000_000.0
            );
        }
    }
    if hu_n > 0 {
        let names = [
            "handle[decode]   ",
            "handle[slot-look]",
            "handle[reader+lk]",
            "handle[reserved] ",
            "handle[dispatch] ",
        ];
        for (i, name) in names.iter().enumerate() {
            let ns = PHASE_HANDLE_SUB_NS[i].load(core::sync::atomic::Ordering::Relaxed);
            if ns > 0 {
                eprintln!(
                    "[ZERODDS_PHASE]   sub[{}]: avg={:.3}us  total={:.1}ms",
                    name,
                    ns as f64 / hu_n as f64 / 1000.0,
                    ns as f64 / 1_000_000.0
                );
            }
        }
    }
}

/// Waits until SPDP has discovered at least `min_count` remote
/// participants. Returns 0 (Ok) on success, -4 (Timeout) if the number is
/// not reached within `timeout_ms`.
///
/// **Optional, not mandatory.** SEDP is reliable + keeps history,
/// so an endpoint registered too early will self-heal after 600-720 ms
/// (empirically llvm-Linux) as soon as SPDP sees the peer
/// and the heartbeat replay runs through. This helper is useful
/// when you want a deterministic test setup or want to avoid a long
/// publish loop.
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_wait_for_peers(
    runtime: *mut ZeroDdsRuntime,
    min_count: c_int,
    timeout_ms: u64,
) -> c_int {
    if runtime.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — runtime NULL-checked above.
    let rt_clone = unsafe { (*runtime).rt.clone() };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = rt_clone.discovered_participants().len();
        if (n as c_int) >= min_count {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Callback for runtime topic enumeration: `(user_data, topic_name, type_name)`
/// where the strings are NUL-terminated and valid only for the call.
pub type ZeroDdsTopicCallback =
    extern "C" fn(*mut core::ffi::c_void, *const core::ffi::c_char, *const core::ffi::c_char);

/// Invokes `callback` once per discovered remote **publication**, passing the
/// raw DDS topic + type name. For ROS-2 graph introspection
/// (`rmw_get_topic_names_and_types`, `rmw_count_publishers`).
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_for_each_publication(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<ZeroDdsTopicCallback>,
    user_data: *mut core::ffi::c_void,
) -> c_int {
    zerodds_runtime_for_each_topic(runtime, callback, user_data, true)
}

/// Invokes `callback` once per discovered remote **subscription**. Counterpart
/// to [`zerodds_runtime_for_each_publication`].
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_for_each_subscription(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<ZeroDdsTopicCallback>,
    user_data: *mut core::ffi::c_void,
) -> c_int {
    zerodds_runtime_for_each_topic(runtime, callback, user_data, false)
}

fn zerodds_runtime_for_each_topic(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<ZeroDdsTopicCallback>,
    user_data: *mut core::ffi::c_void,
    publications: bool,
) -> c_int {
    if runtime.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let Some(cb) = callback else {
        return ZeroDdsStatus::BadHandle as c_int;
    };
    // SAFETY: runtime NULL-checked above; comes from zerodds_runtime_create.
    let rt = unsafe { (*runtime).rt.clone() };
    let topics = if publications {
        rt.discovered_publication_topics()
    } else {
        rt.discovered_subscription_topics()
    };
    for (topic, typ) in topics {
        if let (Ok(t), Ok(y)) = (std::ffi::CString::new(topic), std::ffi::CString::new(typ)) {
            cb(user_data, t.as_ptr(), y.as_ptr());
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes this runtime's 16-byte participant GUID (12-byte RTPS GUID prefix +
/// 4-byte `ENTITYID_PARTICIPANT` = `00 00 01 c1`) into `out_guid`. ROS 2 uses
/// this as the participant gid on `ros_discovery_info`; the first 12 bytes match
/// every endpoint GUID owned by this participant, so endpoint-info lookups can
/// resolve a node name from an endpoint's GUID prefix.
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL; `out_guid` must
/// point to at least 16 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_participant_guid(
    runtime: *mut ZeroDdsRuntime,
    out_guid: *mut u8,
) -> c_int {
    if runtime.is_null() || out_guid.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: runtime NULL-checked; comes from zerodds_runtime_create.
    let rt = unsafe { (*runtime).rt.clone() };
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&rt.guid_prefix.to_bytes());
    // ENTITYID_PARTICIPANT = { entity_key: [0,0,1], entity_kind: 0xc1 } (RTPS §8.2.4.2).
    g[12..16].copy_from_slice(&[0x00, 0x00, 0x01, 0xc1]);
    // SAFETY: out_guid NULL-checked; caller guarantees >= 16 writable bytes.
    unsafe { core::ptr::copy_nonoverlapping(g.as_ptr(), out_guid, 16) };
    ZeroDdsStatus::Ok as c_int
}

/// Writes a writer's 16-byte endpoint GUID (participant prefix + entity id) into
/// `out_guid`. The first 12 bytes equal the participant GUID, so ROS 2 can map
/// the endpoint to its owning node.
///
/// # Safety
/// `writer` must come from `zerodds_writer_create` or be NULL; `out_guid` must
/// point to at least 16 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_guid(
    writer: *mut ZeroDdsWriter,
    out_guid: *mut u8,
) -> c_int {
    if writer.is_null() || out_guid.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: writer NULL-checked; comes from zerodds_writer_create.
    let w = unsafe { &*writer };
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&w.rt.guid_prefix.to_bytes());
    g[12..16].copy_from_slice(&w.eid.to_bytes());
    // SAFETY: out_guid NULL-checked; caller guarantees >= 16 writable bytes.
    unsafe { core::ptr::copy_nonoverlapping(g.as_ptr(), out_guid, 16) };
    ZeroDdsStatus::Ok as c_int
}

/// Writes a reader's 16-byte endpoint GUID into `out_guid`. See
/// [`zerodds_writer_guid`].
///
/// # Safety
/// `reader` must come from `zerodds_reader_create` or be NULL; `out_guid` must
/// point to at least 16 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_guid(
    reader: *mut ZeroDdsReader,
    out_guid: *mut u8,
) -> c_int {
    if reader.is_null() || out_guid.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: reader NULL-checked; comes from zerodds_reader_create.
    let r = unsafe { &*reader };
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&r.rt.guid_prefix.to_bytes());
    g[12..16].copy_from_slice(&r.eid.to_bytes());
    // SAFETY: out_guid NULL-checked; caller guarantees >= 16 writable bytes.
    unsafe { core::ptr::copy_nonoverlapping(g.as_ptr(), out_guid, 16) };
    ZeroDdsStatus::Ok as c_int
}

/// Per-endpoint discovery info passed to a [`ZeroDdsEndpointCallback`]. All
/// pointers are valid only for the duration of the one callback invocation.
/// Powers ROS-2 `rmw_get_publishers_info_by_topic` / `..subscriptions..`
/// (`ros2 topic info -v`). QoS is best-effort from what discovery carries.
#[repr(C)]
pub struct ZeroDdsEndpointInfo {
    /// NUL-terminated DDS topic name (raw, un-demangled).
    pub topic_name: *const core::ffi::c_char,
    /// NUL-terminated IDL type name (raw).
    pub type_name: *const core::ffi::c_char,
    /// Pointer to the 16-byte endpoint GUID (12-byte participant prefix +
    /// 4-byte entity id). Bytes 0..12 identify the owning participant.
    pub endpoint_guid: *const u8,
    /// 1 = RELIABLE, 0 = BEST_EFFORT.
    pub reliable: u8,
    /// 1 = TRANSIENT_LOCAL or stronger, 0 = VOLATILE.
    pub transient_local: u8,
    /// Deadline period in whole seconds (0 = INFINITE).
    pub deadline_seconds: i32,
    /// Lifespan in whole seconds (0 = INFINITE).
    pub lifespan_seconds: i32,
    /// Liveliness lease in whole seconds (0 = INFINITE).
    pub liveliness_lease_seconds: i32,
}

/// Callback for runtime endpoint enumeration: `(user_data, &info)`. The
/// `info` pointer and everything it references are valid only for the call.
pub type ZeroDdsEndpointCallback =
    extern "C" fn(*mut core::ffi::c_void, *const ZeroDdsEndpointInfo);

/// Invokes `callback` once per **publication** endpoint on this domain (local
/// user writers + remote SEDP), with full per-endpoint info (GUID + QoS).
/// Backs `rmw_get_publishers_info_by_topic`.
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_for_each_publication_endpoint(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<ZeroDdsEndpointCallback>,
    user_data: *mut core::ffi::c_void,
) -> c_int {
    zerodds_runtime_for_each_endpoint(runtime, callback, user_data, true)
}

/// Counterpart to [`zerodds_runtime_for_each_publication_endpoint`] for
/// **subscription** endpoints. Backs `rmw_get_subscriptions_info_by_topic`.
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_for_each_subscription_endpoint(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<ZeroDdsEndpointCallback>,
    user_data: *mut core::ffi::c_void,
) -> c_int {
    zerodds_runtime_for_each_endpoint(runtime, callback, user_data, false)
}

fn zerodds_runtime_for_each_endpoint(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<ZeroDdsEndpointCallback>,
    user_data: *mut core::ffi::c_void,
    publications: bool,
) -> c_int {
    if runtime.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let Some(cb) = callback else {
        return ZeroDdsStatus::BadHandle as c_int;
    };
    // SAFETY: runtime NULL-checked above; comes from zerodds_runtime_create.
    let rt = unsafe { (*runtime).rt.clone() };
    let eps = if publications {
        rt.discovered_publication_endpoints()
    } else {
        rt.discovered_subscription_endpoints()
    };
    for e in eps {
        let (Ok(t), Ok(y)) = (
            std::ffi::CString::new(e.topic_name),
            std::ffi::CString::new(e.type_name),
        ) else {
            continue;
        };
        let info = ZeroDdsEndpointInfo {
            topic_name: t.as_ptr(),
            type_name: y.as_ptr(),
            endpoint_guid: e.endpoint_guid.as_ptr(),
            reliable: u8::from(e.reliable),
            transient_local: u8::from(e.transient_local),
            deadline_seconds: e.deadline_seconds,
            lifespan_seconds: e.lifespan_seconds,
            liveliness_lease_seconds: e.liveliness_lease_seconds,
        };
        cb(user_data, &info);
    }
    ZeroDdsStatus::Ok as c_int
}

// ============================================================================
// Writer
// ============================================================================

/// Creates a DataWriter on a topic. Topic + type are identified by name
/// (DDS spec §2.2.2).
///
/// # Safety
/// `runtime` must be valid. `topic_name` and `type_name` must
/// be NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_create(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    reliable: c_int,
) -> *mut ZeroDdsWriter {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — runtime+topic_name+type_name NULL-checked above;
    // both strings NUL-terminated (caller pledge).
    let (rt_clone, topic, typ) = unsafe {
        let topic = match CStr::from_ptr(topic_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let typ = match CStr::from_ptr(type_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        ((*runtime).rt.clone(), topic, typ)
    };

    let cfg = UserWriterConfig {
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        // F-TYPES-3: the C-FFI is byte-oriented (no typed topic type),
        // so no TypeIdentifier is available.
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    };
    let eid = match rt_clone.register_user_writer(cfg) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(ZeroDdsWriter { rt: rt_clone, eid }))
}

/// Creates a DataWriter with an explicit keyed/no-keyed flag.
///
/// Cross-vendor interop: if the IDL type has no `@key`, `is_keyed=0`
/// MUST be set. Otherwise ZeroDDS publishes as a
/// WithKey writer (entityKind 0x02) and a remote reader (e.g. Cyclone)
/// rejects the DATA submessage due to a mismatch with its NoKey reader
/// (entityKind 0x04). Spec §9.3.1.2 Table 9.1.
///
/// # Safety
/// Same as [`zerodds_writer_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_create_kind(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    reliable: c_int,
    is_keyed: c_int,
) -> *mut ZeroDdsWriter {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: runtime/topic_name/type_name are NULL-checked above; the
    // C strings are NUL-terminated and `runtime` comes from
    // `zerodds_runtime_create` (caller duty per the # Safety doc).
    let (rt_clone, topic, typ) = unsafe {
        let topic = match CStr::from_ptr(topic_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let typ = match CStr::from_ptr(type_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        ((*runtime).rt.clone(), topic, typ)
    };
    let cfg = UserWriterConfig {
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    };
    let eid = match rt_clone.register_user_writer_kind(cfg, is_keyed != 0) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "zerodds_writer_create_kind(topic={topic_name:?}, is_keyed={is_keyed}) failed: {e:?}"
            );
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(ZeroDdsWriter { rt: rt_clone, eid }))
}

/// Writes a sample. `payload` points to already-CDR-encoded bytes.
///
/// # Safety
/// `writer` and `payload` must be valid, `len` <= buffer size.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_write(
    writer: *mut ZeroDdsWriter,
    payload: *const u8,
    len: usize,
) -> c_int {
    if writer.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: writer+payload NULL-checked above; payload[0..len] valid
    // if len > 0 (caller pledge). The slice is borrowed only for the duration
    // of this call — `write_user_sample_borrowed` copies internally
    // exactly ONCE into the retransmit cache `Arc` (copy 1 eliminated: no
    // more `to_vec` at the FFI boundary).
    let (rt, eid) = unsafe {
        let w = &*writer;
        (w.rt.clone(), w.eid)
    };
    let bytes: &[u8] = if len == 0 {
        &[]
    } else {
        // SAFETY: payload is non-NULL (checked above) and `len` bytes
        // valid (caller duty per the # Safety doc); borrowed only for the
        // duration of this call.
        unsafe { slice::from_raw_parts(payload, len) }
    };
    match rt.write_user_sample_borrowed(eid, bytes) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Sets a writer's type extensibility, so that the
/// encapsulation header of the payload is declared correctly:
/// `0` = FINAL (PLAIN_CDR/PLAIN_CDR2), `1` = APPENDABLE (DELIMITED_CDR2),
/// `2` = MUTABLE (PL_CDR/PL_CDR2). The default without a call is FINAL.
///
/// Only relevant for XCDR2 wire (offer XCDR2-first) with non-@final
/// types — otherwise the header is `CDR_LE` in both cases. The codegen
/// calls this after `zerodds_writer_create*` with the compile-time
/// extensibility of the type.
///
/// # Safety
/// `writer` must come from `zerodds_writer_create*` and be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_set_wire_extensibility(
    writer: *mut ZeroDdsWriter,
    extensibility: c_int,
) -> c_int {
    if writer.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    use zerodds_types::qos::ExtensibilityForRepr;
    let ext = match extensibility {
        0 => ExtensibilityForRepr::Final,
        1 => ExtensibilityForRepr::Appendable,
        2 => ExtensibilityForRepr::Mutable,
        _ => return ZeroDdsStatus::BadParameter as c_int,
    };
    // SAFETY: writer NULL-checked above; comes from zerodds_writer_create*
    // (Box::into_raw), lives for the caller lifetime.
    let (rt, eid) = unsafe {
        let w = &*writer;
        (w.rt.clone(), w.eid)
    };
    match rt.set_user_writer_wire_extensibility(eid, ext) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Waits until at least `min_count` subscriptions have matched or
/// the timeout elapses. `timeout_ms` = 0 -> non-blocking poll.
///
/// # Safety
/// `writer` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_wait_for_matched(
    writer: *mut ZeroDdsWriter,
    min_count: c_int,
    timeout_ms: u64,
) -> c_int {
    if writer.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer NULL-checked above.
    let (rt, eid) = unsafe { ((*writer).rt.clone(), (*writer).eid) };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = rt.user_writer_matched_count(eid);
        if (n as c_int) >= min_count {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Sends a lifecycle marker (Spec §9.6.3.9 PID_STATUS_INFO):
/// `dispose` sets the DISPOSED bit, so that remote readers classify the
/// instance as NotAliveDisposed. The caller must pass the 16-byte
/// key hash of the instance (PLAIN_CDR2-BE encoding with zero
/// padding, or MD5 if > 16 bytes).
///
/// # Safety
/// `writer` and `key_hash` must be valid; `key_hash` must point to
/// exactly 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_dispose(
    writer: *mut ZeroDdsWriter,
    key_hash: *const u8,
) -> c_int {
    if writer.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+key_hash NULL-checked above; key_hash[0..16]
    // valid (caller pledge).
    let (rt, eid, kh) = unsafe {
        let w = &*writer;
        let mut kh = [0u8; 16];
        std::ptr::copy_nonoverlapping(key_hash, kh.as_mut_ptr(), 16);
        (w.rt.clone(), w.eid, kh)
    };
    match rt.write_user_lifecycle(eid, kh, zerodds_rtps::inline_qos::status_info::DISPOSED) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Sends an UNREGISTER marker (Spec §2.2.2.4.2.7). Sets only the
/// UNREGISTERED bit (no autodispose). A caller wanting spec-default
/// behavior (autodispose=true) should instead use
/// `zerodds_writer_unregister_with_dispose`.
///
/// # Safety
/// Like [`zerodds_writer_dispose`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unregister(
    writer: *mut ZeroDdsWriter,
    key_hash: *const u8,
) -> c_int {
    if writer.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+key_hash NULL-checked above; key_hash[0..16]
    // valid (caller pledge).
    let (rt, eid, kh) = unsafe {
        let w = &*writer;
        let mut kh = [0u8; 16];
        std::ptr::copy_nonoverlapping(key_hash, kh.as_mut_ptr(), 16);
        (w.rt.clone(), w.eid, kh)
    };
    match rt.write_user_lifecycle(eid, kh, zerodds_rtps::inline_qos::status_info::UNREGISTERED) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Sends a combined DISPOSE+UNREGISTER marker (Spec §2.2.3.21 with
/// `autodispose_unregistered_instances=true`). The reader sees both
/// NotAliveDisposed and NotAliveNoWriters.
///
/// # Safety
/// Like [`zerodds_writer_dispose`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unregister_with_dispose(
    writer: *mut ZeroDdsWriter,
    key_hash: *const u8,
) -> c_int {
    if writer.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+key_hash NULL-checked above; key_hash[0..16]
    // valid (caller pledge).
    let (rt, eid, kh) = unsafe {
        let w = &*writer;
        let mut kh = [0u8; 16];
        std::ptr::copy_nonoverlapping(key_hash, kh.as_mut_ptr(), 16);
        (w.rt.clone(), w.eid, kh)
    };
    let bits = zerodds_rtps::inline_qos::status_info::DISPOSED
        | zerodds_rtps::inline_qos::status_info::UNREGISTERED;
    match rt.write_user_lifecycle(eid, kh, bits) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Destroys a writer. NULL-safe.
///
/// # Safety
/// `writer` must come from `zerodds_writer_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_destroy(writer: *mut ZeroDdsWriter) {
    if writer.is_null() {
        return;
    }
    // Drop any zero-copy SHM loan state keyed by this writer's (runtime, eid).
    #[cfg(feature = "flatdata-loan")]
    // SAFETY: writer NULL-checked above.
    unsafe {
        let w = &*writer;
        crate::shm_loan_ffi::forget_writer(&w.rt, w.eid);
    }
    // SAFETY: see fn # Safety doc — writer from zerodds_writer_create (Box::into_raw).
    let _ = unsafe { Box::from_raw(writer) };
}

// ============================================================================
// Reader
// ============================================================================

/// Creates a DataReader on a topic.
///
/// # Safety
/// Like `zerodds_writer_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_create(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    reliable: c_int,
) -> *mut ZeroDdsReader {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — runtime+topic_name+type_name NULL-checked above;
    // both strings NUL-terminated (caller pledge).
    let (rt_clone, topic, typ) = unsafe {
        let topic = match CStr::from_ptr(topic_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let typ = match CStr::from_ptr(type_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        ((*runtime).rt.clone(), topic, typ)
    };

    let cfg = UserReaderConfig {
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        // F-TYPES-3: the C-FFI is byte-oriented.
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    };
    let (eid, rx) = match rt_clone.register_user_reader(cfg) {
        Ok(p) => p,
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(ZeroDdsReader {
        rt: rt_clone,
        eid,
        rx: Mutex::new(rx),
    }))
}

/// Like [`zerodds_reader_create`] but with an explicit keyed/no-keyed
/// flag. Cross-vendor interop: the reader kind must match the writer kind
/// (Spec §9.3.1.2 Table 9.1).
///
/// # Safety
/// Same as [`zerodds_reader_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_create_kind(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    reliable: c_int,
    is_keyed: c_int,
) -> *mut ZeroDdsReader {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: runtime/topic_name/type_name are NULL-checked above; the
    // C strings are NUL-terminated and `runtime` comes from
    // `zerodds_runtime_create` (caller duty per the # Safety doc).
    let (rt_clone, topic, typ) = unsafe {
        let topic = match CStr::from_ptr(topic_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let typ = match CStr::from_ptr(type_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        ((*runtime).rt.clone(), topic, typ)
    };
    let cfg = UserReaderConfig {
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    };
    let (eid, rx) = match rt_clone.register_user_reader_kind(cfg, is_keyed != 0) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "zerodds_reader_create_kind(topic={topic_name:?}, is_keyed={is_keyed}) failed: {e:?}"
            );
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(ZeroDdsReader {
        rt: rt_clone,
        eid,
        rx: Mutex::new(rx),
    }))
}

/// Tries to read a sample.
/// * On success: writes an allocated buffer into `*out_buf`, its
///   length into `*out_len`. The caller MUST call `zerodds_buffer_free(*out_buf)`.
/// * On no sample: `*out_buf = NULL`, `*out_len = 0`, returns Ok.
/// * On error: a negative status code.
///
/// `out_repr` (nullable): receives the XCDR version of the sample —
/// `0` = XCDR1, `1` = XCDR2 — from the encapsulation header of the
/// wire sample. The typed consumer needs this to decode the body
/// with the correct alignment rule. NULL = ignore.
///
/// # Safety
/// Pointers must be valid. `out_repr` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take(
    reader: *mut ZeroDdsReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_repr: *mut u8,
) -> c_int {
    if reader.is_null() || out_buf.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — reader+out_buf+out_len NULL-checked above.
    // The C-API delivers only alive samples; lifecycle markers are deliberately discarded,
    // so that the C-FFI consumer sees unchanged bytes.
    unsafe {
        let r = &*reader;
        let bytes = match r.rx.lock() {
            Ok(rx) => loop {
                match rx.try_recv().ok() {
                    Some(zerodds_dcps::runtime::UserSample::Alive {
                        payload: b,
                        representation,
                        ..
                    }) => {
                        break Some((b, representation));
                    }
                    Some(zerodds_dcps::runtime::UserSample::Lifecycle { .. }) => continue,
                    None => break None,
                }
            },
            Err(_) => {
                *out_buf = ptr::null_mut();
                *out_len = 0;
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
        };
        match bytes {
            Some((bs, repr)) => {
                // Hand over a heap buffer — the caller frees via zerodds_buffer_free.
                // SampleBytes -> Vec materialization at the C-FFI boundary.
                let mut boxed = bs.to_vec().into_boxed_slice();
                *out_buf = boxed.as_mut_ptr();
                *out_len = boxed.len();
                // Leak — the caller now has ownership.
                let _ = Box::into_raw(boxed);
                if !out_repr.is_null() {
                    *out_repr = repr;
                }
            }
            None => {
                *out_buf = ptr::null_mut();
                *out_len = 0;
                if !out_repr.is_null() {
                    *out_repr = 0;
                }
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Fixed-buffer `take` convenience: copies one sample's payload into a
/// caller-supplied buffer and returns the byte count — no heap
/// allocation, no `zerodds_buffer_free`.
///
/// This is the ergonomic shape for plain-C / FFI consumers that prefer a
/// stack buffer + return-length idiom (Go/Zig/Ada/MATLAB cgo bindings,
/// the C quickstart) over the allocate-and-out-param
/// [`zerodds_reader_take`].
///
/// Returns:
/// * `> 0`: number of payload bytes written into `buf` (one sample).
/// * `0`: no sample available right now.
/// * `< 0`: a negative status code (bad handle, or — if the sample is
///   larger than `cap` — `ZeroDdsStatus::OutOfResources`, in which case
///   the sample is consumed; use [`zerodds_reader_take`] for unbounded
///   samples).
///
/// # Safety
/// `reader` must come from `zerodds_reader_create*` or be NULL. `buf`
/// must point to at least `cap` writable bytes (or be NULL iff
/// `cap == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take_into(
    reader: *mut ZeroDdsReader,
    buf: *mut u8,
    cap: usize,
) -> isize {
    if reader.is_null() || (buf.is_null() && cap != 0) {
        return ZeroDdsStatus::BadHandle as isize;
    }
    let mut raw: *mut u8 = ptr::null_mut();
    let mut len: usize = 0;
    // SAFETY: reader NULL-checked above; raw/len are valid local out-params;
    // out_repr is NULL (representation not needed for raw byte copy).
    let rc = unsafe { zerodds_reader_take(reader, &mut raw, &mut len, ptr::null_mut()) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc as isize;
    }
    if raw.is_null() || len == 0 {
        return 0;
    }
    if len > cap {
        // Sample too large for the caller buffer — it has already been
        // consumed from the queue, so free it and signal the overflow.
        // SAFETY: `raw`/`len` are the buffer the reader take just handed us; freeing it once is the documented ownership handoff.
        unsafe { zerodds_buffer_free(raw, len) };
        return ZeroDdsStatus::OutOfResources as isize;
    }
    // SAFETY: raw points to `len` readable bytes (from zerodds_reader_take),
    // buf points to >= cap >= len writable bytes (checked above).
    unsafe {
        ptr::copy_nonoverlapping(raw, buf, len);
        zerodds_buffer_free(raw, len);
    }
    len as isize
}

/// Waits until at least `min_count` publications have matched.
///
/// # Safety
/// `reader` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_wait_for_matched(
    reader: *mut ZeroDdsReader,
    min_count: c_int,
    timeout_ms: u64,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — reader NULL-checked above.
    let (rt, eid) = unsafe { ((*reader).rt.clone(), (*reader).eid) };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let matched = rt.user_reader_matched_count(eid) as c_int;
        if matched >= min_count {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Returns the number of samples dropped because of an unknown writer (no
/// matching writer_proxy). Diagnosis for interop bugs:
/// if the writer_proxy was not created from SEDP PublicationData
/// (e.g. because of an entity-id mismatch between the SEDP pub and the DATA submessage),
/// this counter increments for every incoming DATA sample.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create*` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_unknown_src_count(reader: *mut ZeroDdsReader) -> u64 {
    if reader.is_null() {
        return 0;
    }
    // SAFETY: reader NULL-checked above; comes from zerodds_reader_create*
    // (caller duty per the # Safety doc).
    let r = unsafe { &*reader };
    r.rt.user_reader_unknown_src_count(r.eid)
}

/// Destroys a reader handle and releases its resources.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create*` or be NULL and
/// must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_destroy(reader: *mut ZeroDdsReader) {
    if reader.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — reader from zerodds_reader_create (Box::into_raw).
    // Before destroy, clear any registered data callback, otherwise the
    // recv thread fires with a dangling listener until the reader slot is out of the runtime index.
    unsafe {
        let r = &*reader;
        r.rt.set_user_reader_listener(r.eid, None);
        // Drop any zero-copy SHM map state keyed by this reader's (runtime, eid).
        #[cfg(feature = "flatdata-loan")]
        crate::shm_loan_ffi::forget_reader(&r.rt, r.eid);
        let _ = Box::from_raw(reader);
    }
}

/// Data-available callback for alive samples (latency optimization).
///
/// Registers a synchronous callback invoked by the runtime's recv thread
/// directly after sample arrival. Eliminates
/// the polling latency of `zerodds_reader_take()` (~50-100 µs removed).
///
/// `callback = NULL` clears an existing listener.
///
/// **Contract**:
/// * The callback runs in the recv thread, NOT in the user thread.
/// * Short and non-blocking.
/// * No ZeroDDS API calls inside (recursion risk).
/// * The callback MUST be `noexcept` — a C++ exception that escapes into
///   the Rust recv thread leads to `abort()` ("Rust cannot
///   catch foreign exceptions"). C++ consumers catch internally (`try`/
///   `catch (...)`).
/// * `payload` points to the CDR payload (without the encapsulation header).
///   Lifetime only for the duration of the callback; copy if needed beyond
///   the call.
/// * `representation` is the XCDR version of the sample: `0` = XCDR1,
///   `1` = XCDR2. The typed consumer needs it to decode the body
///   with the correct alignment rule.
/// * Disposed/unregistered lifecycle events do NOT fire the callback.
///
/// # Safety
/// `reader` must be a valid pointer from `zerodds_reader_create`.
/// `user_data` is opaque; the user must keep it alive themselves
/// until the listener is cleared with NULL.
pub type ZeroDdsDataCallback = extern "C" fn(
    user_data: *mut core::ffi::c_void,
    payload: *const u8,
    payload_len: usize,
    representation: u8,
);

/// Sets a data-available callback (or clears it with NULL).
/// See the `ZeroDdsDataCallback` docs for the full contract.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_data_callback(
    reader: *mut ZeroDdsReader,
    callback: Option<ZeroDdsDataCallback>,
    user_data: *mut core::ffi::c_void,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — reader NULL-checked above.
    let (rt, eid) = unsafe { ((*reader).rt.clone(), (*reader).eid) };
    let listener: Option<zerodds_dcps::runtime::UserReaderListener> = match callback {
        Some(cb) => {
            // Store user_data as a usize, because *mut c_void is not Send;
            // in the closure we cast it back. Per the contract the caller must keep
            // user_data alive until the listener is cleared with NULL.
            let ud_addr = user_data as usize;
            Some(Box::new(move |bytes: &[u8], repr: u8| {
                cb(
                    ud_addr as *mut core::ffi::c_void,
                    bytes.as_ptr(),
                    bytes.len(),
                    repr,
                );
            }))
        }
        None => None,
    };
    if rt.set_user_reader_listener(eid, listener) {
        ZeroDdsStatus::Ok as c_int
    } else {
        ZeroDdsStatus::BadHandle as c_int
    }
}

// ============================================================================
// Buffer free (for from-take)
// ============================================================================

/// Frees a buffer that a previous `zerodds_reader_take`
/// allocated. NULL-safe.
///
/// # Safety
/// `buf` must come from `zerodds_reader_take` or be NULL.
/// `len` must be exactly the value belonging to that buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_buffer_free(buf: *mut u8, len: usize) {
    if buf.is_null() || len == 0 {
        return;
    }
    // SAFETY: see fn # Safety doc — buf+len from zerodds_reader_take Box::into_raw.
    let _ = unsafe { Box::from_raw(slice::from_raw_parts_mut(buf, len)) };
}

// ============================================================================
// Read-Loan (Opt-1, Zero-Copy-Roadmap §6 R6)
// ============================================================================
//
// `zerodds_reader_loan/_return_loan` is a zero-copy alternative to
// `zerodds_reader_take`. Instead of copying the payload via `to_vec().into_boxed_slice()`
// into an owned C heap buffer, the loan holds an
// `Arc<[u8]>` refcount on the internal `SampleBytes` and hands out a
// raw pointer to the bytes. The caller must return the buffer with
// `zerodds_reader_return_loan(loan_handle)` as soon as it
// no longer needs the bytes.
//
// Contract:
// - `*out_buf` is valid only while `loan_handle` has not been returned.
// - `loan_handle` is opaque (`*mut c_void`) — the caller must not dereference the pointer
//   or pass it beyond the call.
// - `zerodds_reader_return_loan(NULL)` is a no-op.

/// Opaque loan handle — wraps a `SampleBytes` box so that the
/// Arc refcount stays up until `return_loan`.
type ZeroDdsReadLoanHandle = zerodds_dcps::sample_bytes::SampleBytes;

/// Loan-based `take`: returns a live pointer into an
/// internal `Arc<[u8]>` without a copy.
///
/// On success:
/// * `*out_buf` points to the payload (read-only),
/// * `*out_len` is the length,
/// * `*out_loan_handle` is an opaque pointer that must later be passed to
///   [`zerodds_reader_return_loan`].
///
/// On no sample: `*out_buf = NULL`, `*out_len = 0`,
/// `*out_loan_handle = NULL`, returns Ok.
///
/// # Safety
/// All pointers must be valid. The returned `*out_buf` is
/// valid only while `*out_loan_handle` lives; after
/// `zerodds_reader_return_loan` the read lifetime has ended.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_loan(
    reader: *mut ZeroDdsReader,
    out_buf: *mut *const u8,
    out_len: *mut usize,
    out_loan_handle: *mut *mut c_void,
) -> c_int {
    if reader.is_null() || out_buf.is_null() || out_len.is_null() || out_loan_handle.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — all pointers NULL-checked.
    unsafe {
        let r = &*reader;
        let bytes = match r.rx.lock() {
            Ok(rx) => loop {
                match rx.try_recv().ok() {
                    Some(zerodds_dcps::runtime::UserSample::Alive { payload: b, .. }) => {
                        break Some(b);
                    }
                    Some(zerodds_dcps::runtime::UserSample::Lifecycle { .. }) => continue,
                    None => break None,
                }
            },
            Err(_) => {
                *out_buf = ptr::null();
                *out_len = 0;
                *out_loan_handle = ptr::null_mut();
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
        };
        match bytes {
            Some(bs) => {
                let len = bs.as_slice().len();
                // Box<SampleBytes> → leak ptr; the caller returns it via
                // return_loan. Important: as_slice().as_ptr()
                // only AFTER boxing, because bs is moved and the
                // heap box holds the Arc refcount anchor.
                let boxed: Box<ZeroDdsReadLoanHandle> = Box::new(bs);
                let buf_ptr = boxed.as_slice().as_ptr();
                let handle = Box::into_raw(boxed);
                *out_buf = buf_ptr;
                *out_len = len;
                *out_loan_handle = handle.cast::<c_void>();
            }
            None => {
                *out_buf = ptr::null();
                *out_len = 0;
                *out_loan_handle = ptr::null_mut();
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns a loan that a previous [`zerodds_reader_loan`]
/// created. After this call the associated `*out_buf`
/// pointer is invalid (the Arc refcount may go to 0 and the bytes
/// are freed).
///
/// NULL-safe — a `loan_handle = NULL` is a no-op.
///
/// # Safety
/// `loan_handle` must come from [`zerodds_reader_loan`] or be NULL.
/// Do not return it twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_return_loan(loan_handle: *mut c_void) {
    if loan_handle.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — handle from zerodds_reader_loan Box::into_raw.
    let _ = unsafe { Box::from_raw(loan_handle.cast::<ZeroDdsReadLoanHandle>()) };
}

// ============================================================================
// Loaning
// ============================================================================
//
// Heap-backed loans as the standard path. With the SHM transport enabled
// (see the `zerodds-flatdata-1.0` vendor spec) the memory path
// is internally transparently replaced by an SHM buffer-pool lookup — the
// FFI signatures stay stable.

/// Reserves an output buffer at the writer for zero-copy publish.
/// The caller writes the sample into the returned pointer and
/// then commits it via [`zerodds_writer_commit_loan`].
///
/// Returns 0 (Ok) on success + fills `*out_ptr` and `*out_len`.
/// On today's malloc-backed path `*out_len = len`; for
/// SHM-backed loans `*out_len > len` is possible (slot boundary).
///
/// # Safety
/// `writer` valid; `out_ptr`/`out_len` non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_loan_message(
    writer: *mut ZeroDdsWriter,
    len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> c_int {
    if writer.is_null() || out_ptr.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Zero-copy SHM path when the writer has `zerodds_writer_enable_shm_loan`
    // active; otherwise fall through to the heap-box variant below.
    #[cfg(feature = "flatdata-loan")]
    // SAFETY: writer NULL-checked above; out_ptr/out_len NULL-checked above.
    if let Some(rc) = unsafe {
        let w = &*writer;
        crate::shm_loan_ffi::try_loan(&w.rt, w.eid, len, out_ptr, out_len)
    } {
        return rc;
    }
    // Heap-allocated buffer (default path).
    let mut v = alloc::vec![0u8; len].into_boxed_slice();
    let ptr = v.as_mut_ptr();
    // Leak — the caller now owns the buffer until commit/discard.
    let _ = Box::into_raw(v);
    // SAFETY: see fn # Safety doc — out_ptr+out_len NULL-checked above.
    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Commit path: writes the loaned buffer as a sample and frees
/// it. The caller must not read the pointer afterwards.
///
/// # Safety
/// `writer` from `zerodds_writer_create`; `ptr` from
/// `zerodds_writer_loan_message`; `len` the same value as returned in
/// `out_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_commit_loan(
    writer: *mut ZeroDdsWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if writer.is_null() || ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Zero-copy SHM commit when `ptr` is an active SHM loan; else heap path.
    #[cfg(feature = "flatdata-loan")]
    // SAFETY: writer NULL-checked above; ptr valid per the contract.
    if let Some(rc) = unsafe {
        let w = &*writer;
        crate::shm_loan_ffi::try_commit(&w.rt, w.eid, ptr, len)
    } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — writer+ptr NULL-checked above; ptr+len come
    // from loan_message (Box::into_raw).
    //
    // Zero-copy path (spec zerodds-zero-copy-1.0 §6 wave 1): use the borrowed variant
    // of write_user_sample instead of zerodds_writer_write (which Vec-allocates).
    // Saves a Vec roundtrip + heap alloc per commit_loan.
    let (rt, eid, payload) = unsafe {
        let w = &*writer;
        let payload = slice::from_raw_parts(ptr, len);
        (w.rt.clone(), w.eid, payload)
    };
    let rc = match rt.write_user_sample_borrowed(eid, payload) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    };
    // Drop the buffer after write (the borrow lifetime is held up to here).
    // SAFETY: ptr+len from loan_message Box::into_raw.
    unsafe {
        let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, len));
    }
    rc
}

/// Discards a loan without publishing it. The buffer is freed.
///
/// # Safety
/// Like `zerodds_writer_commit_loan`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_discard_loan(
    _writer: *mut ZeroDdsWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Zero-copy SHM discard when `ptr` is an active SHM loan; else heap path.
    // `_writer` is not NULL-checked by this fn's contract, so guard it here.
    #[cfg(feature = "flatdata-loan")]
    if !_writer.is_null() {
        // SAFETY: _writer checked non-null just above; ptr valid per the contract.
        if let Some(rc) = unsafe {
            let w = &*_writer;
            crate::shm_loan_ffi::try_discard(&w.rt, w.eid, ptr)
        } {
            return rc;
        }
    }
    // SAFETY: see fn # Safety doc — ptr+len from loan_message (Box::into_raw).
    unsafe {
        let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, len));
    }
    ZeroDdsStatus::Ok as c_int
}

// ============================================================================
// Zero-copy SHM loan — runtime path (ZeroDdsWriter / ZeroDdsReader)
// ============================================================================

/// Sample delivery mode (`zerodds-delivery-modes-1.0` §3). Selects the form a
/// writer's samples take. The default is `Portable` so no deployment loses
/// interoperability without an explicit opt-in.
#[cfg(feature = "flatdata-loan")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroDdsDeliveryMode {
    /// Serialized portable form over RTPS — cross-host + cross-vendor (default).
    Portable = 0,
    /// In-memory form in ZeroDDS's own SHM slot — same-host, ZeroDDS-only,
    /// no wire (no serialization).
    RawSameHost = 1,
    /// In-memory form via the iceoryx2 bridge — same-host, cross-stack. Not yet
    /// wired on this path (`zerodds-delivery-modes-1.0` §11).
    Iceoryx = 2,
}

/// Sets the delivery mode of a runtime-path DataWriter (the handle the ROS-2
/// RMW bridge uses). `0`=Portable (default, interop-safe), `1`=RawSameHost
/// (same-host, no wire). `2`=Iceoryx is not yet wired → `Unsupported`. Other
/// values → `BadParameter`.
///
/// # Safety
/// `writer` from `zerodds_writer_create`.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_set_delivery_mode(
    writer: *mut ZeroDdsWriter,
    mode: c_int,
) -> c_int {
    if writer.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let Ok(mode) = u8::try_from(mode) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: writer NULL-checked above.
    let w = unsafe { &*writer };
    crate::shm_loan_ffi::set_delivery_mode(&w.rt, w.eid, mode)
}

/// Enables zero-copy SHM loan on a runtime-path DataWriter (the handle the
/// ROS-2 RMW bridge uses). Creates a POSIX shared-memory segment of
/// `slot_count` slots × `slot_capacity` bytes at the flink path `name`. After
/// this, `zerodds_writer_loan_message` returns a pointer into a SHM slot and
/// `zerodds_writer_commit_loan` finalizes it in place — no staging copy.
///
/// The loan state is keyed by `(runtime, entity-id)`, so a writer reached
/// through the DCPS FFI (`zerodds_dw_*`) and this runtime FFI shares the same
/// backend. Without this feature/call the loan path stays on the heap box.
///
/// # Safety
/// `writer` from `zerodds_writer_create`; `name` is a NUL-terminated C string.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_enable_shm_loan(
    writer: *mut ZeroDdsWriter,
    name: *const c_char,
    slot_count: usize,
    slot_capacity: usize,
) -> c_int {
    if writer.is_null() || name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: name is a NUL-terminated C string per the contract.
    let path = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    // SAFETY: writer NULL-checked above.
    let w = unsafe { &*writer };
    crate::shm_loan_ffi::enable_writer(&w.rt, w.eid, path, slot_count, slot_capacity)
}

/// Maps the writer's SHM segment at flink path `name` on a runtime-path
/// DataReader for zero-copy reads. `reader_index` is the reader's bit (0..32)
/// in the slot reader-mask.
///
/// # Safety
/// `reader` from `zerodds_reader_create`; `name` is a NUL-terminated C string.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_enable_shm(
    reader: *mut ZeroDdsReader,
    name: *const c_char,
    reader_index: u8,
) -> c_int {
    if reader.is_null() || name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: name is a NUL-terminated C string per the contract.
    let path = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    // SAFETY: reader NULL-checked above.
    let r = unsafe { &*reader };
    crate::shm_loan_ffi::enable_reader(&r.rt, r.eid, path, reader_index)
}

/// Zero-copy take on a runtime-path DataReader: returns a read-only pointer
/// into the writer's SHM slot, its length and the slot index (for
/// `zerodds_reader_release_shm`). Returns `NoData` when nothing is pending.
/// The pointer stays valid until `zerodds_reader_release_shm`.
///
/// # Safety
/// `reader`/`out_ptr`/`out_len`/`out_slot` valid.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take_shm(
    reader: *mut ZeroDdsReader,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    out_slot: *mut u32,
) -> c_int {
    if reader.is_null() || out_ptr.is_null() || out_len.is_null() || out_slot.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: reader NULL-checked above; out pointers NULL-checked above.
    let r = unsafe { &*reader };
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    unsafe { crate::shm_loan_ffi::try_take(&r.rt, r.eid, out_ptr, out_len, out_slot) }
}

/// Releases a slot previously returned by `zerodds_reader_take_shm`.
///
/// # Safety
/// `reader` valid; `slot_index` from a prior `zerodds_reader_take_shm`.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_release_shm(
    reader: *mut ZeroDdsReader,
    slot_index: u32,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: reader NULL-checked above.
    let r = unsafe { &*reader };
    crate::shm_loan_ffi::try_release(&r.rt, r.eid, slot_index)
}

/// Blocks until this reader's raw source (RawSameHost SHM segment or Iceoryx
/// service) signals a new sample, or `timeout_ms` elapses — event-driven (SHM
/// change-generation futex / iceoryx2 listener), no busy-poll. Lets a consumer
/// (e.g. the ROS-2 RMW `rmw_wait` doorbell) park on raw-data arrival instead of
/// polling. A spurious wake is harmless — re-check with `zerodds_reader_take_shm`.
/// Returns `Ok` when woken or timed out, `PreconditionNotMet` if no raw source.
///
/// # Safety
/// `reader` from `zerodds_reader_create` or NULL.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_raw_wait(
    reader: *mut ZeroDdsReader,
    timeout_ms: u64,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: reader NULL-checked above.
    let r = unsafe { &*reader };
    crate::shm_loan_ffi::try_raw_wait(&r.rt, r.eid, timeout_ms)
}

/// Enables `Iceoryx` delivery on a runtime-path DataWriter (feature
/// `delivery-iceoryx`): publishes its samples over the iceoryx2 service
/// `service_name` (max `max_len` bytes/sample). The loan API then routes through
/// iceoryx2; commit does not publish over RTPS.
///
/// # Safety
/// `writer` from `zerodds_writer_create`; `service_name` is a NUL-terminated C
/// string.
#[cfg(feature = "delivery-iceoryx")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_enable_iceoryx(
    writer: *mut ZeroDdsWriter,
    service_name: *const c_char,
    max_len: usize,
) -> c_int {
    if writer.is_null() || service_name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: service_name is a NUL-terminated C string per the contract.
    let service = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    // SAFETY: writer NULL-checked above.
    let w = unsafe { &*writer };
    crate::shm_loan_ffi::enable_iceoryx_writer(&w.rt, w.eid, service, max_len)
}

/// Enables `Iceoryx` delivery on a runtime-path DataReader: receives from the
/// iceoryx2 service `service_name` via `zerodds_reader_take_shm` /
/// `zerodds_reader_release_shm`.
///
/// # Safety
/// `reader` from `zerodds_reader_create`; `service_name` is a NUL-terminated C
/// string.
#[cfg(feature = "delivery-iceoryx")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_enable_iceoryx(
    reader: *mut ZeroDdsReader,
    service_name: *const c_char,
) -> c_int {
    if reader.is_null() || service_name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: service_name is a NUL-terminated C string per the contract.
    let service = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    // SAFETY: reader NULL-checked above.
    let r = unsafe { &*reader };
    crate::shm_loan_ffi::enable_iceoryx_reader(&r.rt, r.eid, service)
}

// ============================================================================
// Version info
// ============================================================================

/// Version string of the C-FFI. Static, not to be freed.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}
