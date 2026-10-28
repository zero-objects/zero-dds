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
//! # Type model — byte-oriented, with an optional typed-create path
//!
//! The C-FFI is **byte-oriented** for samples: a `Topic` carries a
//! `topic_name` + `type_name` string, a `write` takes a
//! `(*const u8, len)` buffer with the already-CDR-encoded sample bytes,
//! a `take` returns the raw wire bytes. The CDR encode/decode logic
//! lives in the language bindings (idl-cpp emits a C++ encoder, idl-csharp
//! a C# encoder, etc.) — the C-FFI is neutral for sample payloads.
//!
//! Type identity can still cross the FFI: the typed-create variants
//! (`zerodds_writer_create_typed` / `zerodds_reader_create_typed`, and the
//! topic-based PSM-Cxx creates) take a serialized COMPLETE `TypeObject`,
//! register it in the runtime's TypeLookup registry and advertise the
//! derived strongly-hashed `TypeIdentifier` (F-TYPES-3 / #24). The plain
//! `zerodds_writer_create` / `zerodds_reader_create` take no `TypeObject`
//! and advertise `TypeIdentifier::None`.
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
//! * Java — `crates/java-omgdds`, a pure-Java stack with its own RTPS
//!   implementation; Python — `crates/py`, pyo3 over the Rust API.
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

pub mod arrival_ffi;
pub mod builtin_ffi;
pub mod builtin_reader_ffi;
pub mod builtin_type_ffi;
pub mod condition_ffi;
pub mod dyntype_ffi;
pub mod entities;
pub mod entity_ffi;
pub mod extra_ffi;
pub mod factory_ffi;
pub(crate) mod ffi_helpers;
pub mod instance_ffi;
pub mod listener_ffi;
pub mod multitopic_ffi;
pub mod participant_ffi;
#[cfg(feature = "security")]
pub mod plugin_ffi;
pub mod publisher_ffi;
pub mod qos_ffi;
pub mod qos_provider_ffi;
#[cfg(feature = "rpc")]
pub mod rpc_discovery_ffi;
#[cfg(feature = "rpc")]
pub mod rpc_profile_ffi;
pub mod rpc_qos_ffi;
#[cfg(feature = "security")]
pub mod security_ffi;
#[cfg(feature = "flatdata-loan")]
pub mod shm_loan_ffi;
pub mod status_ffi;
pub mod subscriber_ffi;
pub mod topic_ffi;
pub mod types_ffi;

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
    // The vendor id, the deterministic host ID — identical for all
    // processes on the same host, so that `GuidPrefix::is_same_host` takes
    // effect (same-host SHM and fragmentation optimizations) — and the
    // process id and a counter. The layout
    // `zerodds_dcps::participant::random_guid_prefix` builds, from the same
    // counter.
    GuidPrefix::zerodds(
        zerodds_dcps::participant::host_id_bytes(),
        zerodds_dcps::participant::next_process_unique(),
    )
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
    /// HISTORY KEEP_LAST depth applied at `take`/`loan` (DDS 1.4
    /// §2.2.3.18): pending samples past it are dropped, oldest first.
    /// `None` = every sample is kept until taken (KEEP_ALL, and the
    /// constructors that take no QoS).
    keep_last: Option<usize>,
    /// Alive samples drained from `rx` under a KEEP_LAST depth, oldest first.
    pending: Mutex<alloc::collections::VecDeque<zerodds_dcps::runtime::UserSample>>,
}

/// One alive sample as `take`/`loan` hand it out: payload, representation,
/// big-endian flag.
type AliveSample = (zerodds_dcps::sample_bytes::SampleBytes, u8, bool);

impl ZeroDdsReader {
    /// The HISTORY depth the reader was created with: `None` is KEEP_ALL.
    fn history_depth(&self) -> Option<usize> {
        self.keep_last
    }

    fn new(
        rt: Arc<DcpsRuntime>,
        eid: EntityId,
        rx: mpsc::Receiver<zerodds_dcps::runtime::UserSample>,
        keep_last: Option<usize>,
    ) -> Self {
        Self {
            rt,
            eid,
            rx: Mutex::new(rx),
            keep_last,
            pending: Mutex::new(alloc::collections::VecDeque::new()),
        }
    }

    /// The next alive sample. Lifecycle and end-of-coherent-set markers carry
    /// no user data and are skipped. Under KEEP_LAST(n) everything pending is
    /// drained first and only the newest n are kept.
    fn next_alive(&self) -> Result<Option<AliveSample>, ()> {
        let rx = self.rx.lock().map_err(|_| ())?;
        let Some(depth) = self.keep_last else {
            loop {
                match rx.try_recv() {
                    Ok(sample) => {
                        if let Some(a) = alive(&sample) {
                            return Ok(Some(a));
                        }
                    }
                    Err(_) => return Ok(None),
                }
            }
        };
        let mut pending = self.pending.lock().map_err(|_| ())?;
        while let Ok(sample) = rx.try_recv() {
            if matches!(sample, zerodds_dcps::runtime::UserSample::Alive { .. }) {
                pending.push_back(sample);
                while pending.len() > depth.max(1) {
                    pending.pop_front();
                }
            }
        }
        Ok(pending.pop_front().and_then(|sample| alive(&sample)))
    }

    /// Every alive sample the reader holds that no `take` handed out, oldest
    /// first and trimmed to the KEEP_LAST depth as `take` trims it: the
    /// backlog a data callback receives when it is registered
    /// ([`DcpsRuntime::set_user_reader_listener_with_backlog`]). Locks `rx`
    /// before `pending`, as [`Self::next_alive`] does.
    fn drain_backlog(&self) -> Vec<zerodds_dcps::runtime::UserSample> {
        let mut out: alloc::collections::VecDeque<zerodds_dcps::runtime::UserSample> =
            alloc::collections::VecDeque::new();
        if let Ok(rx) = self.rx.lock() {
            if let Ok(mut pending) = self.pending.lock() {
                out.extend(pending.drain(..));
            }
            while let Ok(sample) = rx.try_recv() {
                if matches!(sample, zerodds_dcps::runtime::UserSample::Alive { .. }) {
                    out.push_back(sample);
                }
            }
        }
        if let Some(depth) = self.keep_last {
            while out.len() > depth.max(1) {
                out.pop_front();
            }
        }
        out.into()
    }
}

/// The payload, representation and big-endian flag of an alive sample.
/// Lifecycle and end-of-coherent-set markers carry no user data: `None`.
fn alive(sample: &zerodds_dcps::runtime::UserSample) -> Option<AliveSample> {
    match sample {
        zerodds_dcps::runtime::UserSample::Alive {
            payload,
            representation,
            big_endian,
            writer_guid,
            source_sequence_number,
            related_sample_identity,
            group,
            ..
        } => Some((
            payload
                .clone()
                .with_wire(zerodds_dcps::sample_bytes::SampleWire {
                    writer_guid: *writer_guid,
                    sequence_number: *source_sequence_number,
                    related_sample_identity: *related_sample_identity,
                    group: group.map(|g| g.info),
                }),
            *representation,
            *big_endian,
        )),
        zerodds_dcps::runtime::UserSample::Lifecycle { .. }
        | zerodds_dcps::runtime::UserSample::CoherentSetEnd { .. } => None,
    }
}

/// The QoS of a DataWriter or DataReader created with
/// [`zerodds_writer_create_qos`] / [`zerodds_reader_create_qos`]. Obtain the
/// defaults from [`zerodds_endpoint_qos_default`] and change what differs.
///
/// Times are nanoseconds; `0` means infinite (DDS 1.4 `DURATION_INFINITE`),
/// except `max_blocking_time_ns`, where not waiting is a value of its own.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroDdsEndpointQos {
    /// `1` RELIABLE, `0` BEST_EFFORT.
    pub reliable: c_int,
    /// `1` TRANSIENT_LOCAL, `0` VOLATILE.
    pub transient_local: c_int,
    /// `1` KEEP_ALL, `0` KEEP_LAST(`history_depth`).
    pub history_keep_all: c_int,
    /// KEEP_LAST depth, at least 1.
    pub history_depth: u32,
    /// DEADLINE period.
    pub deadline_ns: u64,
    /// LIFESPAN duration (writers only).
    pub lifespan_ns: u64,
    /// LIVELINESS kind: `0` AUTOMATIC, `1` MANUAL_BY_PARTICIPANT,
    /// `2` MANUAL_BY_TOPIC.
    pub liveliness_kind: c_int,
    /// LIVELINESS lease duration.
    pub liveliness_lease_ns: u64,
    /// RESOURCE_LIMITS `max_samples`; `-1` is LENGTH_UNLIMITED.
    pub max_samples: i32,
    /// RESOURCE_LIMITS `max_instances`; `-1` is LENGTH_UNLIMITED.
    pub max_instances: i32,
    /// RESOURCE_LIMITS `max_samples_per_instance`; `-1` is LENGTH_UNLIMITED.
    pub max_samples_per_instance: i32,
    /// RELIABILITY `max_blocking_time` of a writer: how long `write`,
    /// `dispose` and `unregister` wait for room under RESOURCE_LIMITS before
    /// they return `ZERODDS_TIMEOUT` (DDS 1.4 §2.2.2.4.2.11). `0` does not
    /// wait, `u64::MAX` waits without end.
    pub max_blocking_time_ns: u64,
}

/// `max_blocking_time_ns` that waits without end.
pub const ZERODDS_BLOCKING_INFINITE: u64 = u64::MAX;

/// The status a failed runtime write, dispose or unregister returns: one
/// that waited out `max_blocking_time` is `Timeout`, one the writer's
/// RESOURCE_LIMITS refused is `OutOfResources` (DDS 1.4 §2.2.2.4.2.11),
/// anything else `Error`.
pub(crate) fn write_status(e: &zerodds_dcps::DdsError) -> c_int {
    match e {
        zerodds_dcps::DdsError::Timeout => ZeroDdsStatus::Timeout as c_int,
        zerodds_dcps::DdsError::OutOfResources { .. } => ZeroDdsStatus::OutOfResources as c_int,
        _ => ZeroDdsStatus::Error as c_int,
    }
}

/// The DDS 1.4 defaults: RELIABLE, VOLATILE, KEEP_LAST(1), infinite
/// deadline, lifespan and lease, AUTOMATIC liveliness, no RESOURCE_LIMITS,
/// and a writer's `max_blocking_time` of 100 ms.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_endpoint_qos_default() -> ZeroDdsEndpointQos {
    ZeroDdsEndpointQos {
        reliable: 1,
        transient_local: 0,
        history_keep_all: 0,
        history_depth: 1,
        deadline_ns: 0,
        lifespan_ns: 0,
        liveliness_kind: 0,
        liveliness_lease_ns: 0,
        max_samples: zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED,
        max_instances: zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED,
        max_samples_per_instance: zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED,
        max_blocking_time_ns: 100_000_000,
    }
}

/// A DDS duration from nanoseconds, `0` = infinite.
fn qos_duration(ns: u64) -> zerodds_qos::Duration {
    if ns == 0 {
        return zerodds_qos::Duration::INFINITE;
    }
    let secs = ns / 1_000_000_000;
    if secs >= i32::MAX as u64 {
        return zerodds_qos::Duration::INFINITE;
    }
    // 2^-32 s fraction, rounded to the nearest.
    zerodds_qos::Duration::from_sec_nanosec(secs as i32, (ns % 1_000_000_000) as u32)
}

/// The checked parts of a [`ZeroDdsEndpointQos`].
struct EndpointQos {
    reliable: bool,
    durability: DurabilityKind,
    keep_last: Option<usize>,
    deadline: DeadlineQosPolicy,
    lifespan: LifespanQosPolicy,
    liveliness: LivelinessQosPolicy,
    resource_limits: zerodds_qos::ResourceLimitsQosPolicy,
    max_blocking_time: zerodds_qos::Duration,
}

fn endpoint_qos(q: &ZeroDdsEndpointQos) -> Result<EndpointQos, String> {
    let flag = |v: c_int, name: &str| match v {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(format!("{name} = {other}, expected 0 or 1")),
    };
    let kind = match q.liveliness_kind {
        0 => LivelinessKind::Automatic,
        1 => LivelinessKind::ManualByParticipant,
        2 => LivelinessKind::ManualByTopic,
        other => return Err(format!("liveliness_kind = {other}, expected 0, 1 or 2")),
    };
    let keep_all = flag(q.history_keep_all, "history_keep_all")?;
    if !keep_all && q.history_depth == 0 {
        return Err(String::from(
            "history_depth = 0 under KEEP_LAST, expected >= 1",
        ));
    }
    // DDS 1.4 §2.2.3.19: each limit is positive or LENGTH_UNLIMITED, one
    // instance cannot hold more than all, and KEEP_LAST cannot keep more
    // per instance than the limit allows.
    for (name, v) in [
        ("max_samples", q.max_samples),
        ("max_instances", q.max_instances),
        ("max_samples_per_instance", q.max_samples_per_instance),
    ] {
        if v != zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED && v < 1 {
            return Err(format!(
                "{name} = {v}, expected >= 1 or -1 (LENGTH_UNLIMITED)"
            ));
        }
    }
    let resource_limits = zerodds_qos::ResourceLimitsQosPolicy {
        max_samples: q.max_samples,
        max_instances: q.max_instances,
        max_samples_per_instance: q.max_samples_per_instance,
    };
    if !resource_limits.is_consistent() {
        return Err(format!(
            "max_samples_per_instance = {} exceeds max_samples = {}",
            q.max_samples_per_instance, q.max_samples
        ));
    }
    if !keep_all
        && q.max_samples_per_instance != zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED
        && i64::from(q.history_depth) > i64::from(q.max_samples_per_instance)
    {
        return Err(format!(
            "history_depth = {} exceeds max_samples_per_instance = {}",
            q.history_depth, q.max_samples_per_instance
        ));
    }
    let max_blocking_time = if q.max_blocking_time_ns == ZERODDS_BLOCKING_INFINITE {
        zerodds_qos::Duration::INFINITE
    } else if q.max_blocking_time_ns == 0 {
        zerodds_qos::Duration::ZERO
    } else {
        qos_duration(q.max_blocking_time_ns)
    };
    Ok(EndpointQos {
        resource_limits,
        max_blocking_time,
        reliable: flag(q.reliable, "reliable")?,
        durability: if flag(q.transient_local, "transient_local")? {
            DurabilityKind::TransientLocal
        } else {
            DurabilityKind::Volatile
        },
        keep_last: (!keep_all).then_some(q.history_depth as usize),
        deadline: DeadlineQosPolicy {
            period: qos_duration(q.deadline_ns),
        },
        lifespan: LifespanQosPolicy {
            duration: qos_duration(q.lifespan_ns),
        },
        liveliness: LivelinessQosPolicy {
            kind,
            lease_duration: qos_duration(q.liveliness_lease_ns),
        },
    })
}

/// The writer configuration [`zerodds_writer_create_qos`] registers, with
/// the endpoint's `USER_DATA`. After the registration,
/// [`apply_user_writer_qos`] sets what the configuration does not carry. A
/// Rust helper for the crates built on this API, not a C ABI.
///
/// # Errors
/// An invalid QoS value, named.
pub fn user_writer_config(
    topic: &str,
    type_name: &str,
    qos: &ZeroDdsEndpointQos,
    user_data: Vec<u8>,
) -> Result<UserWriterConfig, String> {
    let q = endpoint_qos(qos)?;
    let cfg = UserWriterConfig {
        // These entry points take no PropertyQosPolicy (§7.3.21): the defaults.
        property: Default::default(),
        data_tags: Default::default(),
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        reliable: q.reliable,
        durability: q.durability,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        deadline: q.deadline,
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: q.lifespan,
        liveliness: q.liveliness,
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        transport_priority: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data,
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
        group_entity_id: None,
        data_max_size_serialized: None,
        entity_name: None,
    };
    Ok(cfg)
}

/// Applies the parts of `qos` a registered writer takes after its
/// registration: the history depth (KEEP_ALL retains without a
/// per-instance bound), RESOURCE_LIMITS and `max_blocking_time`. A Rust
/// helper for the crates built on this API, not a C ABI.
///
/// # Errors
/// An invalid QoS value, named, or the runtime's refusal.
pub fn apply_user_writer_qos(
    rt: &zerodds_dcps::runtime::DcpsRuntime,
    eid: zerodds_rtps::wire_types::EntityId,
    qos: &ZeroDdsEndpointQos,
) -> Result<(), String> {
    let q = endpoint_qos(qos)?;
    rt.set_user_writer_history_depth(eid, q.keep_last.unwrap_or(usize::MAX))
        .map_err(|e| format!("history depth: {e:?}"))?;
    rt.set_user_writer_resource_limits(eid, q.resource_limits, q.max_blocking_time)
        .map_err(|e| format!("resource limits: {e:?}"))
}

/// The reader configuration [`zerodds_reader_create_qos`] registers, with
/// the endpoint's `USER_DATA`, and its KEEP_LAST depth (`None` = KEEP_ALL).
/// A Rust helper for the crates built on this API, not a C ABI.
///
/// # Errors
/// An invalid QoS value, named.
pub fn user_reader_config(
    topic: &str,
    type_name: &str,
    qos: &ZeroDdsEndpointQos,
    user_data: Vec<u8>,
) -> Result<(UserReaderConfig, Option<usize>), String> {
    let q = endpoint_qos(qos)?;
    let cfg = UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        reliable: q.reliable,
        durability: q.durability,
        deadline: q.deadline,
        liveliness: q.liveliness,
        resource_limits: q.resource_limits,
        user_data,
        ..Default::default()
    };
    Ok((cfg, q.keep_last))
}

/// Creates a DataWriter with the full [`ZeroDdsEndpointQos`]: reliability,
/// durability, KEEP_LAST depth or KEEP_ALL, deadline, lifespan,
/// liveliness, RESOURCE_LIMITS and `max_blocking_time`. `is_keyed` as in
/// [`zerodds_writer_create_kind`].
///
/// Returns `NULL` for a NULL argument, an invalid QoS value (named on
/// stderr) or a registration failure.
///
/// # Safety
/// `runtime` from `zerodds_runtime_create*`; strings NUL-terminated; `qos`
/// points at a [`ZeroDdsEndpointQos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_create_qos(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    is_keyed: c_int,
    qos: *const ZeroDdsEndpointQos,
) -> *mut ZeroDdsWriter {
    // SAFETY: the caller's contract, passed on; no USER_DATA.
    unsafe {
        zerodds_writer_create_qos_user_data(
            runtime,
            topic_name,
            type_name,
            is_keyed,
            qos,
            ptr::null(),
            0,
        )
    }
}

/// The octets a caller passed as USER_DATA: `len` at `data`, none for NULL
/// with `0`, and `None` for NULL with a length, which names no buffer.
///
/// # Safety
/// A non-NULL `data` is readable for `len` octets.
unsafe fn user_data_octets(data: *const u8, len: usize) -> Option<Vec<u8>> {
    if data.is_null() {
        return (len == 0).then(Vec::new);
    }
    // SAFETY: non-NULL and readable for `len` octets by the caller's contract.
    Some(unsafe { core::slice::from_raw_parts(data, len) }.to_vec())
}

/// [`zerodds_writer_create_qos`] with USER_DATA (DDS 1.4 §2.2.3.1):
/// `user_data_len` octets at `user_data`, announced through SEDP's
/// `PID_USER_DATA`. NULL with `0` is none; NULL with a length is refused.
/// ROS 2 RMWs from Iron on put `typehash=RIHS01_…;` there.
///
/// # Safety
/// As [`zerodds_writer_create_qos`]; a non-NULL `user_data` is readable
/// for `user_data_len` octets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_create_qos_user_data(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    is_keyed: c_int,
    qos: *const ZeroDdsEndpointQos,
    user_data: *const u8,
    user_data_len: usize,
) -> *mut ZeroDdsWriter {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() || qos.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller's contract.
    let Some(user_data) = (unsafe { user_data_octets(user_data, user_data_len) }) else {
        eprintln!("zerodds_writer_create_qos_user_data: user_data is NULL with a length");
        return ptr::null_mut();
    };
    // SAFETY: non-null, NUL-terminated and live by the caller's contract.
    let (rt, topic, typ, q) = unsafe {
        let (Ok(topic), Ok(typ)) = (
            CStr::from_ptr(topic_name).to_str(),
            CStr::from_ptr(type_name).to_str(),
        ) else {
            return ptr::null_mut();
        };
        (
            (*runtime).rt.clone(),
            topic.to_string(),
            typ.to_string(),
            *qos,
        )
    };
    let cfg = match user_writer_config(&topic, &typ, &q, user_data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zerodds_writer_create_qos(topic={topic}): {e}");
            return ptr::null_mut();
        }
    };
    let eid = match rt.register_user_writer_kind(cfg, is_keyed != 0) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("zerodds_writer_create_qos(topic={topic}) failed: {e:?}");
            return ptr::null_mut();
        }
    };
    if let Err(e) = apply_user_writer_qos(&rt, eid, &q) {
        eprintln!("zerodds_writer_create_qos(topic={topic}): {e}");
        rt.remove_user_writer(eid);
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(ZeroDdsWriter { rt, eid }))
}

/// Creates a DataReader with the full [`ZeroDdsEndpointQos`] (lifespan is
/// a writer policy and ignored here). The KEEP_LAST depth bounds what
/// `zerodds_reader_take`/`zerodds_reader_loan` keep between calls; a data
/// callback receives every sample as it arrives.
///
/// # Safety
/// As [`zerodds_writer_create_qos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_create_qos(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    is_keyed: c_int,
    qos: *const ZeroDdsEndpointQos,
) -> *mut ZeroDdsReader {
    // SAFETY: the caller's contract, passed on; no USER_DATA.
    unsafe {
        zerodds_reader_create_qos_user_data(
            runtime,
            topic_name,
            type_name,
            is_keyed,
            qos,
            ptr::null(),
            0,
        )
    }
}

/// [`zerodds_reader_create_qos`] with USER_DATA, as
/// [`zerodds_writer_create_qos_user_data`].
///
/// # Safety
/// As [`zerodds_writer_create_qos_user_data`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_create_qos_user_data(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    is_keyed: c_int,
    qos: *const ZeroDdsEndpointQos,
    user_data: *const u8,
    user_data_len: usize,
) -> *mut ZeroDdsReader {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() || qos.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller's contract.
    let Some(user_data) = (unsafe { user_data_octets(user_data, user_data_len) }) else {
        eprintln!("zerodds_reader_create_qos_user_data: user_data is NULL with a length");
        return ptr::null_mut();
    };
    // SAFETY: non-null, NUL-terminated and live by the caller's contract.
    let (rt, topic, typ, q) = unsafe {
        let (Ok(topic), Ok(typ)) = (
            CStr::from_ptr(topic_name).to_str(),
            CStr::from_ptr(type_name).to_str(),
        ) else {
            return ptr::null_mut();
        };
        (
            (*runtime).rt.clone(),
            topic.to_string(),
            typ.to_string(),
            *qos,
        )
    };
    let (cfg, keep_last) = match user_reader_config(&topic, &typ, &q, user_data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zerodds_reader_create_qos(topic={topic}): {e}");
            return ptr::null_mut();
        }
    };
    let (eid, rx) = match rt.register_user_reader_kind(cfg, is_keyed != 0) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("zerodds_reader_create_qos(topic={topic}) failed: {e:?}");
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(ZeroDdsReader::new(rt, eid, rx, keep_last)))
}

/// Registers the `dependency_count` dependency objects and then the root
/// object, all in the `TypeObject::to_bytes_le` form, and returns the
/// identifier of the root. `None` for a NULL or empty root, a NULL array
/// with a count, or an object that does not decode or register.
///
/// # Safety
/// `root` is readable for `root_len` octets; `dependencies` and
/// `dependency_lens` hold `dependency_count` entries, each pointer readable
/// for its length.
unsafe fn register_typed_objects(
    rt: &DcpsRuntime,
    root: *const u8,
    root_len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    dependency_count: usize,
) -> Option<zerodds_types::TypeIdentifier> {
    if root.is_null()
        || root_len == 0
        || (dependency_count > 0 && (dependencies.is_null() || dependency_lens.is_null()))
    {
        return None;
    }
    for at in 0..dependency_count {
        // SAFETY: `at < dependency_count` entries are readable (caller duty).
        let (bytes, len) = unsafe { (*dependencies.add(at), *dependency_lens.add(at)) };
        if bytes.is_null() || len == 0 {
            return None;
        }
        // SAFETY: readable for `len` octets (caller duty).
        register_type_object_from_bytes(rt, unsafe { core::slice::from_raw_parts(bytes, len) })?;
    }
    // SAFETY: readable for `root_len` octets (caller duty).
    register_type_object_from_bytes(rt, unsafe { core::slice::from_raw_parts(root, root_len) })
}

/// [`zerodds_writer_create_qos`] for a topic whose type is described by a
/// serialized COMPLETE `TypeObject` and the objects it names by hash
/// (DDS-RPC 1.0 §7.5.1.2, XTypes 1.3 §7.6.3): the dependencies and the root
/// are registered, and the endpoint announces the root's `TypeIdentifier`
/// from its first publication. `dependencies[i]` is readable for
/// `dependency_lens[i]` octets.
///
/// # Safety
/// As [`zerodds_writer_create_qos`]; the object buffers as described above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_create_qos_typed(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    is_keyed: c_int,
    qos: *const ZeroDdsEndpointQos,
    type_object: *const u8,
    type_object_len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    dependency_count: usize,
) -> *mut ZeroDdsWriter {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() || qos.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: non-null, NUL-terminated and live by the caller's contract.
    let (rt, topic, typ, q) = unsafe {
        let (Ok(topic), Ok(typ)) = (
            CStr::from_ptr(topic_name).to_str(),
            CStr::from_ptr(type_name).to_str(),
        ) else {
            return ptr::null_mut();
        };
        (
            (*runtime).rt.clone(),
            topic.to_string(),
            typ.to_string(),
            *qos,
        )
    };
    // SAFETY: the caller's contract for the object buffers.
    let Some(identifier) = (unsafe {
        register_typed_objects(
            &rt,
            type_object,
            type_object_len,
            dependencies,
            dependency_lens,
            dependency_count,
        )
    }) else {
        eprintln!("zerodds_writer_create_qos_typed(topic={topic}): malformed type objects");
        return ptr::null_mut();
    };
    let mut cfg = match user_writer_config(&topic, &typ, &q, Vec::new()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zerodds_writer_create_qos_typed(topic={topic}): {e}");
            return ptr::null_mut();
        }
    };
    cfg.type_identifier = identifier;
    let eid = match rt.register_user_writer_kind(cfg, is_keyed != 0) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("zerodds_writer_create_qos_typed(topic={topic}) failed: {e:?}");
            return ptr::null_mut();
        }
    };
    if let Err(e) = apply_user_writer_qos(&rt, eid, &q) {
        eprintln!("zerodds_writer_create_qos_typed(topic={topic}): {e}");
        rt.remove_user_writer(eid);
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(ZeroDdsWriter { rt, eid }))
}

/// The reader-side mirror of [`zerodds_writer_create_qos_typed`].
///
/// # Safety
/// As [`zerodds_writer_create_qos_typed`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_create_qos_typed(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    is_keyed: c_int,
    qos: *const ZeroDdsEndpointQos,
    type_object: *const u8,
    type_object_len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    dependency_count: usize,
) -> *mut ZeroDdsReader {
    if runtime.is_null() || topic_name.is_null() || type_name.is_null() || qos.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: non-null, NUL-terminated and live by the caller's contract.
    let (rt, topic, typ, q) = unsafe {
        let (Ok(topic), Ok(typ)) = (
            CStr::from_ptr(topic_name).to_str(),
            CStr::from_ptr(type_name).to_str(),
        ) else {
            return ptr::null_mut();
        };
        (
            (*runtime).rt.clone(),
            topic.to_string(),
            typ.to_string(),
            *qos,
        )
    };
    // SAFETY: the caller's contract for the object buffers.
    let Some(identifier) = (unsafe {
        register_typed_objects(
            &rt,
            type_object,
            type_object_len,
            dependencies,
            dependency_lens,
            dependency_count,
        )
    }) else {
        eprintln!("zerodds_reader_create_qos_typed(topic={topic}): malformed type objects");
        return ptr::null_mut();
    };
    let (mut cfg, keep_last) = match user_reader_config(&topic, &typ, &q, Vec::new()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zerodds_reader_create_qos_typed(topic={topic}): {e}");
            return ptr::null_mut();
        }
    };
    cfg.type_identifier = identifier;
    let (eid, rx) = match rt.register_user_reader_kind(cfg, is_keyed != 0) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("zerodds_reader_create_qos_typed(topic={topic}) failed: {e:?}");
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(ZeroDdsReader::new(rt, eid, rx, keep_last)))
}

/// MANUAL_BY_TOPIC `assert_liveliness` of a DataWriter (DDS 1.4
/// §2.2.3.11). Returns `0`, or `-1` for a NULL writer.
///
/// # Safety
/// `writer` from `zerodds_writer_create*` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_assert_liveliness(writer: *mut ZeroDdsWriter) -> c_int {
    if writer.is_null() {
        return -1;
    }
    // SAFETY: non-null and live by the caller's contract.
    let w = unsafe { &*writer };
    w.rt.assert_writer_liveliness_eid(w.eid);
    0
}

/// Blocks until every matched reader has acknowledged every sample written
/// so far, or `timeout_ns` elapses (DDS 1.4 §2.2.2.4.2.22
/// `wait_for_acknowledgments`). Returns `0` when acknowledged, `1` on
/// timeout, `-1` for a NULL writer.
///
/// # Safety
/// `writer` from `zerodds_writer_create*` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_wait_for_acknowledgments(
    writer: *mut ZeroDdsWriter,
    timeout_ns: u64,
) -> c_int {
    if writer.is_null() {
        return -1;
    }
    // SAFETY: non-null and live by the caller's contract.
    let w = unsafe { &*writer };
    // A timeout past what an Instant holds waits without bound.
    let deadline =
        std::time::Instant::now().checked_add(std::time::Duration::from_nanos(timeout_ns));
    loop {
        if w.rt.user_writer_all_acknowledged(w.eid) {
            return 0;
        }
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            return 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

// ============================================================================
// Lifecycle — Runtime
// ============================================================================

/// The data-representation offer every runtime created through this FFI
/// uses as its compiled default: `[XCDR2, XCDR1]`. An explicit validated
/// `wire.data_representation_offer` file setting is applied at runtime creation.
///
/// **XCDR2 is first, and the order is the load-bearing half.** The
/// byte-oriented writer here takes the encapsulation header it stamps
/// from the offer's first element, and the callers of this FFI — the
/// roundtrip bench, the C and C++ codegen backends, direct FFI apps —
/// hand it XCDR2 bodies. An XCDR1-first offer stamps `0x0001` on an
/// XCDR2 body, and every reader that believes the header, ZeroDDS's own
/// included, then decodes the sample at the wrong offsets.
///
/// **XCDR1 stays in the set**, so a reader created here still accepts a
/// foreign XCDR1 writer (Cyclone, `rmw_*`) without configuration.
///
/// This is stated here rather than taken from `RuntimeConfig::default()`
/// because that default is not this FFI's to pick: it is
/// `DEFAULT_OFFER = [XCDR1, XCDR2]`, the list a peer requests when it
/// leaves `DataRepresentation` at the DDS default, and it moved to
/// XCDR1-first for peers that do exactly that. The FFI does not — it
/// serialises XCDR2 — so it names its own order. The secured path
/// (`security_ffi`) reads the offer from here for the same reason and
/// through the same function, so the two cannot drift apart again.
///
/// The byte-oriented caller remains responsible for encoding according to an
/// explicitly configured offer. No process environment override is read here.
#[must_use]
pub(crate) fn ffi_data_representation_offer() -> alloc::vec::Vec<i16> {
    use zerodds_rtps::publication_data::data_representation as dr;
    alloc::vec![dr::XCDR2, dr::XCDR]
}

/// The `rmw_zerodds` runtime (ROS 2 profile: the reader offers XCDR1 and
/// XCDR2) from layers the caller loaded. A Rust helper, not a C ABI: the RMW
/// reads `--config`/`--set` from its process's command line, which
/// [`zerodds_runtime_create_ros_defaults`] (environment and system directory
/// only) cannot see.
///
/// # Errors
/// An invalid configuration or a failed start.
pub fn runtime_create_ros_from_source(
    domain_id: u32,
    source: &zerodds_dcps::runtime::ConfigSource,
) -> Result<*mut ZeroDdsRuntime, String> {
    let config = ffi_config_from_source(
        source,
        RuntimeConfig::ros_defaults().data_representation_offer,
    )?;
    runtime_create_with_config(domain_id, config)
}

/// Apply the caller profile only where the validated file layer is unspecified.
pub(crate) fn ffi_config_from_source(
    source: &zerodds_dcps::runtime::ConfigSource,
    default_offer: alloc::vec::Vec<i16>,
) -> Result<RuntimeConfig, String> {
    let mut config = RuntimeConfig::from_source(source).map_err(|e| e.to_string())?;
    if source.var("wire.data_representation_offer").is_none() {
        config.data_representation_offer = default_offer;
    }
    Ok(config)
}

/// The configuration a runtime was created with. A Rust helper for the
/// crates built on this API (the ROS 2 shim), not a C ABI.
///
/// # Safety
/// `rt` must be NULL or come from a `zerodds_runtime_create*` call and not
/// be destroyed while the returned reference is in use.
#[must_use]
pub unsafe fn runtime_config<'a>(rt: *const ZeroDdsRuntime) -> Option<&'a RuntimeConfig> {
    if rt.is_null() {
        return None;
    }
    // SAFETY: non-null and live by the caller's contract.
    let runtime: &'a ZeroDdsRuntime = unsafe { &*rt };
    Some(&runtime.rt.config)
}

/// The DCPS runtime behind a C runtime handle. A Rust helper for the crates
/// built on this API (the ROS 2 shim builds its service endpoints on it,
/// because they need the sample identity a C take does not carry), not a C
/// ABI.
///
/// # Safety
/// `rt` must be NULL or come from a `zerodds_runtime_create*` call that is
/// not destroyed during this call.
#[must_use]
pub unsafe fn runtime_arc(rt: *const ZeroDdsRuntime) -> Option<Arc<DcpsRuntime>> {
    if rt.is_null() {
        return None;
    }
    // SAFETY: non-null and live by the caller's contract.
    let runtime: &ZeroDdsRuntime = unsafe { &*rt };
    Some(Arc::clone(&runtime.rt))
}

/// Load validated standard files with the caller's byte-encoding profile as
/// the fallback offer. Explicit file representation remains authoritative.
/// # Errors
/// Invalid or unreadable configuration, with its source diagnostic.
pub fn ffi_runtime_config(default_offer: alloc::vec::Vec<i16>) -> Result<RuntimeConfig, String> {
    let source =
        zerodds_dcps::runtime::ConfigSource::from_standard_paths().map_err(|e| e.to_string())?;
    ffi_config_from_source(&source, default_offer)
}

/// Load an explicit runtime base and its sibling drop-ins through the same
/// typed loader as standard-file startup. This is a Rust helper, not a C ABI.
/// # Errors
/// Missing, invalid or unreadable explicit configuration retains its diagnostic.
pub fn ffi_runtime_config_from_path(
    path: impl AsRef<std::path::Path>,
    default_offer: alloc::vec::Vec<i16>,
) -> Result<RuntimeConfig, String> {
    let source = zerodds_dcps::runtime::ConfigSource::from_path(path).map_err(|e| e.to_string())?;
    ffi_config_from_source(&source, default_offer)
}

/// A plain constructor with `security.enclave_dir` set would start a
/// participant without the security the configuration asks for; it used to
/// do so without a word. The secured entry point is
/// `zerodds_runtime_create_secure_from_env` (feature `security`).
fn refuse_unloaded_enclave(config: &RuntimeConfig) -> Result<(), String> {
    #[cfg(feature = "security")]
    let secured = config.security.is_some();
    #[cfg(not(feature = "security"))]
    let secured = false;
    match (&config.security_enclave_dir, secured) {
        (Some(dir), false) => Err(format!(
            "security.enclave_dir {} is set; this constructor starts an unsecured participant — \
             use zerodds_runtime_create_secure_from_env (feature `security`)",
            dir.display()
        )),
        _ => Ok(()),
    }
}

/// Start the C-API runtime hub from an explicit Rust configuration.
///
/// Rust launchers can supply ephemeral CLI choices without process environment
/// overrides. The returned handle must be released with `zerodds_runtime_destroy`.
/// This is a Rust entrypoint, not a new exported C ABI symbol.
/// # Errors
/// Invalid domain or a runtime startup failure.
pub fn runtime_create_with_config(
    domain_id: u32,
    config: RuntimeConfig,
) -> Result<*mut ZeroDdsRuntime, String> {
    let domain = i32::try_from(domain_id).map_err(|_| String::from("domain exceeds i32"))?;
    refuse_unloaded_enclave(&config)?;
    let rt = DcpsRuntime::start(domain, random_guid_prefix(), config)
        .map_err(|error| format!("runtime startup failed: {error:?}"))?;
    Ok(Box::into_raw(Box::new(ZeroDdsRuntime {
        rt,
        _shutdown: (),
    })))
}

/// Creates a new ZeroDDS runtime on the given domain ID.
///
/// The runtime's data-representation offer is
/// `ffi_data_representation_offer` — XCDR2 first, XCDR1 also offered.
///
/// # Safety
/// The return value is a heap-allocated pointer; the caller must
/// free it with `zerodds_runtime_destroy`.
///
/// Returns `NULL` on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create(domain_id: u32) -> *mut ZeroDdsRuntime {
    let cfg = match ffi_runtime_config(ffi_data_representation_offer()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("zerodds_runtime_create: invalid configuration: {error}");
            return ptr::null_mut();
        }
    };
    match runtime_create_with_config(domain_id, cfg) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("zerodds_runtime_create(domain_id={domain_id}) failed: {error}");
            ptr::null_mut()
        }
    }
}

/// Creates a runtime from an explicitly selected configuration, as a
/// program's `--config PATH` and `--set PATH=VALUE` flags would select it:
/// the environment beneath, then `config_path` with its sibling `<stem>.d`
/// (or, for `NULL`, the standard system layout), then `sets[0..n_sets]` in
/// order, each `"path=value"` (`"+path=value"` appends to a list).
///
/// Returns `NULL` and names the refusal on stderr for an unreadable or
/// invalid configuration, an unknown setting path, a malformed assignment or
/// a startup failure.
///
/// # Safety
/// `config_path` is `NULL` or a NUL-terminated UTF-8 string; `sets` is
/// `NULL` (with `n_sets == 0`) or points at `n_sets` NUL-terminated UTF-8
/// strings. The returned pointer is freed with `zerodds_runtime_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create_from_config(
    domain_id: u32,
    config_path: *const c_char,
    sets: *const *const c_char,
    n_sets: usize,
) -> *mut ZeroDdsRuntime {
    // SAFETY: the pointer contract is the caller's, per the `# Safety` above.
    let command_line = match unsafe { command_line_from_c(config_path, sets, n_sets) } {
        Ok(cl) => cl,
        Err(error) => {
            eprintln!("zerodds_runtime_create_from_config: {error}");
            return ptr::null_mut();
        }
    };
    let config = zerodds_dcps::runtime::ConfigSource::from_command_line(&command_line)
        .map_err(|e| e.to_string())
        .and_then(|source| ffi_config_from_source(&source, ffi_data_representation_offer()));
    let config = match config {
        Ok(config) => config,
        Err(error) => {
            eprintln!("zerodds_runtime_create_from_config: invalid configuration: {error}");
            return ptr::null_mut();
        }
    };
    match runtime_create_with_config(domain_id, config) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("zerodds_runtime_create_from_config(domain_id={domain_id}) failed: {error}");
            ptr::null_mut()
        }
    }
}

/// The `--config`/`--set` selection a C caller passed as strings.
///
/// # Safety
/// As [`zerodds_runtime_create_from_config`].
unsafe fn command_line_from_c(
    config_path: *const c_char,
    sets: *const *const c_char,
    n_sets: usize,
) -> Result<zerodds_config::CommandLine, String> {
    let text = |p: *const c_char, what: &str| -> Result<String, String> {
        // SAFETY: non-null, NUL-terminated by the caller's contract.
        unsafe { core::ffi::CStr::from_ptr(p) }
            .to_str()
            .map(str::to_string)
            .map_err(|_| format!("{what} is not UTF-8"))
    };
    let mut command_line = zerodds_config::CommandLine::default();
    if !config_path.is_null() {
        command_line = command_line.with_config(text(config_path, "config_path")?);
    }
    if n_sets > 0 {
        if sets.is_null() {
            return Err(String::from("sets is NULL with n_sets > 0"));
        }
        for i in 0..n_sets {
            // SAFETY: `sets` holds `n_sets` pointers by the caller's contract.
            let entry = unsafe { *sets.add(i) };
            if entry.is_null() {
                return Err(format!("sets[{i}] is NULL"));
            }
            command_line
                .push_set(&text(entry, "a set")?)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(command_line)
}

/// Like [`zerodds_runtime_create`] but with the **ROS-2 out-of-the-box profile**
/// ([`RuntimeConfig::ros_defaults`]): the reader announces both XCDR1 and XCDR2
/// data representations, so a ZeroDDS node matches `rmw_cyclonedds`/
/// `rmw_fastrtps` writers that emit XCDR1 for final/simple types
/// (e.g. `std_msgs/String`) without any `ZERODDS_DATA_REPR_OFFER` env tuning.
/// This is what `rmw_zerodds` uses, so ROS-2 interop works config-free.
///
/// # Safety
/// Same as [`zerodds_runtime_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create_ros_defaults(
    domain_id: u32,
) -> *mut ZeroDdsRuntime {
    let cfg = match ffi_runtime_config(RuntimeConfig::ros_defaults().data_representation_offer) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("zerodds_runtime_create_ros_defaults: invalid configuration: {error}");
            return ptr::null_mut();
        }
    };
    if let Err(error) = refuse_unloaded_enclave(&cfg) {
        eprintln!("zerodds_runtime_create_ros_defaults: {error}");
        return ptr::null_mut();
    }
    let rt = match DcpsRuntime::start(domain_id as i32, random_guid_prefix(), cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("zerodds_runtime_create_ros_defaults(domain_id={domain_id}) failed: {e:?}");
            return ptr::null_mut();
        }
    };
    let handle = Box::new(ZeroDdsRuntime { rt, _shutdown: () });
    Box::into_raw(handle)
}

/// Destroys a runtime: stops its threads, announces the participant's
/// departure, closes its same-host channels and sockets. NULL-safe.
///
/// The worker threads hold the runtime, so freeing the handle alone
/// stopped nothing: the participant stayed on the wire until the process
/// ended, and its peers kept it, and sent to it, until its lease ran out.
///
/// # Safety
/// `runtime` must come from `zerodds_runtime_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_destroy(runtime: *mut ZeroDdsRuntime) {
    if runtime.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — runtime from zerodds_runtime_create (Box::into_raw).
    let runtime = unsafe { Box::from_raw(runtime) };
    runtime.rt.shutdown();
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

/// Writes the RTPS sequence number the next sample written through `writer`
/// will carry into `out_sn` (DDSI-RTPS 2.5 §8.3.5.4: one more than the last
/// sample the writer wrote). A DDS-RPC request header holds the identity of
/// the sample it is part of (DDS-RPC 1.0 §7.5.1.1.1 `requestId`), and the
/// header is filled before the write, so a binding asks the writer for the
/// number instead of counting on its own.
///
/// # Safety
/// `writer` must come from `zerodds_writer_create` or be NULL; `out_sn` must
/// point to one writable `int64_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_next_sequence_number(
    writer: *mut ZeroDdsWriter,
    out_sn: *mut i64,
) -> c_int {
    if writer.is_null() || out_sn.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: writer NULL-checked; comes from zerodds_writer_create.
    let w = unsafe { &*writer };
    match w.rt.user_writer_next_sequence_number(w.eid) {
        Some(sn) => {
            // SAFETY: out_sn NULL-checked; valid per the # Safety doc.
            unsafe { *out_sn = sn };
            ZeroDdsStatus::Ok as c_int
        }
        // The writer is not registered with the runtime any more, or its
        // sequence numbers are used up.
        None => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
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
    /// Deadline period in nanoseconds (0 = INFINITE).
    pub deadline_ns: u64,
    /// Lifespan in nanoseconds (0 = INFINITE; a subscription has none).
    pub lifespan_ns: u64,
    /// Liveliness lease in nanoseconds (0 = INFINITE).
    pub liveliness_lease_ns: u64,
    /// USER_DATA the endpoint announced (DDS 1.4 §2.2.3.1),
    /// `user_data_len` octets; NULL when it announced none. ROS 2 RMWs
    /// from Iron on put `typehash=RIHS01_…;` there.
    pub user_data: *const u8,
    /// Octets at `user_data`.
    pub user_data_len: usize,
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
            deadline_ns: e.deadline_nanos,
            lifespan_ns: e.lifespan_nanos,
            liveliness_lease_ns: e.liveliness_lease_nanos,
            user_data: if e.user_data.is_empty() {
                ptr::null()
            } else {
                e.user_data.as_ptr()
            },
            user_data_len: e.user_data.len(),
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
        // These entry points take no PropertyQosPolicy (§7.3.21): the defaults.
        property: Default::default(),
        data_tags: Default::default(),
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        transport_priority: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        // F-TYPES-3: this untyped create takes no TypeObject, so it
        // advertises None. The typed variant (`zerodds_writer_create_typed`)
        // carries a TypeObject and derives a TypeIdentifier.
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
        group_entity_id: None,
        data_max_size_serialized: None,
        entity_name: None,
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
        // These entry points take no PropertyQosPolicy (§7.3.21): the defaults.
        property: Default::default(),
        data_tags: Default::default(),
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        transport_priority: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
        group_entity_id: None,
        data_max_size_serialized: None,
        entity_name: None,
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

/// F-TYPES-3 / #24: deserializes a serialized `TypeObject` (the XCDR-LE
/// `zerodds_types::TypeObject::to_bytes_le` form the IDL codegen emits),
/// registers it in the runtime's TypeLookup registry (so peers can resolve
/// it via `getTypes` and the runtime can match it structurally — XTypes 1.3
/// §7.6.3) and returns the derived strongly-hashed `TypeIdentifier`.
///
/// The returned identifier is byte-identical to
/// `zerodds_types::to_hashed_type_identifier(&obj)` — the SAME value Path A's
/// `T::TYPE_IDENTIFIER` carries for the same TypeObject — because
/// `register_type_object` hashes with the identical `compute_hash` routine.
/// Returns `None` if the bytes are not a valid `TypeObject` or registration
/// fails (lock poisoning / hash error).
///
/// `pub(crate)` so the topic-based PSM-Cxx create paths
/// (`publisher_ffi::zerodds_pub_create_datawriter_typed` /
/// `subscriber_ffi::zerodds_sub_create_datareader_typed`, used by the C#/Java
/// bindings) share the identical register-and-derive routine as the flat
/// writer/reader typed creates (used by cpp/python).
pub(crate) fn register_type_object_from_bytes(
    rt: &DcpsRuntime,
    bytes: &[u8],
) -> Option<zerodds_types::TypeIdentifier> {
    let obj = zerodds_types::type_object::TypeObject::from_bytes_le(bytes).ok()?;
    let is_complete = matches!(obj, zerodds_types::type_object::TypeObject::Complete(_));
    let hash = rt.register_type_object(obj).ok()?;
    Some(if is_complete {
        zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash)
    } else {
        zerodds_types::TypeIdentifier::EquivalenceHashMinimal(hash)
    })
}

/// Whether the type that the serialized `TypeObject` describes has a key
/// (XTypes 1.3 §7.2.2.4.4.4.8: a member with `IS_KEY`, its own or one
/// inherited from a base type registered with `rt`). The entityKind of an
/// endpoint follows it (DDSI-RTPS 2.5 Table 9.1): a reader of a topic without
/// a key is a NoKey reader and its topic has one instance (DDS 1.4
/// §2.2.1.2.2).
pub(crate) fn type_object_keyed_from_bytes(rt: &DcpsRuntime, bytes: &[u8]) -> bool {
    use zerodds_types::type_object::complete::CompleteStructType;
    use zerodds_types::type_object::flags::StructMemberFlag;
    use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};
    /// zerodds-lint: recursion-depth 16
    fn complete_keyed(rt: &DcpsRuntime, st: &CompleteStructType, depth: u32) -> bool {
        if st
            .member_seq
            .iter()
            .any(|m| m.common.member_flags.0 & StructMemberFlag::IS_KEY != 0)
        {
            return true;
        }
        let zerodds_types::TypeIdentifier::EquivalenceHashComplete(base) =
            st.header.base_type.clone()
        else {
            return false;
        };
        if depth == 0 {
            return false;
        }
        let Ok(server) = rt.type_lookup_server.lock() else {
            return false;
        };
        match server.registry.get_complete(&base) {
            Some(CompleteTypeObject::Struct(base)) => {
                let base = base.clone();
                drop(server);
                complete_keyed(rt, &base, depth - 1)
            }
            _ => false,
        }
    }
    match TypeObject::from_bytes_le(bytes) {
        Ok(TypeObject::Complete(CompleteTypeObject::Struct(st))) => complete_keyed(rt, &st, 16),
        Ok(TypeObject::Minimal(MinimalTypeObject::Struct(st))) => st
            .member_seq
            .iter()
            .any(|m| m.common.member_flags.0 & StructMemberFlag::IS_KEY != 0),
        _ => false,
    }
}

/// Creates a DataWriter that advertises its type via a serialized COMPLETE
/// `TypeObject` (F-TYPES-3 / #24).
///
/// Unlike [`zerodds_writer_create`]/[`zerodds_writer_create_kind`] — which are
/// byte-oriented and advertise `TypeIdentifier::None` — this typed variant
/// deserializes `type_object_bytes`, derives the strongly-hashed
/// `TypeIdentifier`, registers the object into the shared TypeLookup registry
/// and sets the writer slot's `type_identifier` from it **before the endpoint
/// is published** (via `register_user_writer_kind`). There is therefore no
/// discovery window in which the endpoint is matchable while still carrying
/// `None` — the reason for a typed create instead of a post-create setter,
/// which would race SEDP.
///
/// `type_object_bytes` is the `zerodds_types::TypeObject::to_bytes_le` form the
/// IDL codegen emits (see `idlc cpp`'s generated `type_object()`). `is_keyed`
/// mirrors [`zerodds_writer_create_kind`] and must match the type's `@key`
/// presence (Spec §9.3.1.2 Table 9.1). Returns NULL on invalid arguments, a
/// malformed TypeObject, or registration failure.
///
/// # Safety
/// Like [`zerodds_writer_create_kind`], plus: `type_object_bytes` must point to
/// `len` readable bytes (NULL / `len == 0` is rejected with NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_create_typed(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    reliable: c_int,
    is_keyed: c_int,
    type_object_bytes: *const u8,
    len: usize,
) -> *mut ZeroDdsWriter {
    if runtime.is_null()
        || topic_name.is_null()
        || type_name.is_null()
        || type_object_bytes.is_null()
        || len == 0
    {
        return ptr::null_mut();
    }
    // SAFETY: runtime/topic_name/type_name/type_object_bytes NULL-checked above;
    // both C strings are NUL-terminated and `type_object_bytes` points to `len`
    // readable bytes (caller pledge per the # Safety doc). `runtime` comes from
    // `zerodds_runtime_create`.
    let (rt_clone, topic, typ, obj_id) = unsafe {
        let topic = match CStr::from_ptr(topic_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let typ = match CStr::from_ptr(type_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let rt = (*runtime).rt.clone();
        let bytes = core::slice::from_raw_parts(type_object_bytes, len);
        // Register + derive BEFORE publish — no None-window.
        let obj_id = match register_type_object_from_bytes(&rt, bytes) {
            Some(id) => id,
            None => return ptr::null_mut(),
        };
        (rt, topic, typ, obj_id)
    };

    let cfg = UserWriterConfig {
        // These entry points take no PropertyQosPolicy (§7.3.21): the defaults.
        property: Default::default(),
        data_tags: Default::default(),
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        transport_priority: 0,
        presentation: Default::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        // F-TYPES-3 / #24: derived from the registered TypeObject (not None).
        type_identifier: obj_id,
        data_representation_offer: None,
        group_entity_id: None,
        data_max_size_serialized: None,
        entity_name: None,
    };
    let eid = match rt_clone.register_user_writer_kind(cfg, is_keyed != 0) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "zerodds_writer_create_typed(topic={topic_name:?}, is_keyed={is_keyed}) failed: {e:?}"
            );
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(ZeroDdsWriter { rt: rt_clone, eid }))
}

/// Writes a sample. `payload` points to already-CDR-encoded bytes.
///
/// Carries no key hash on the wire (the `zerodds_writer_*` family has no
/// keyed-write entry point at all — see [`zerodds_writer_dispose`] for what
/// that means for a later dispose/unregister of this writer's instances).
/// `zerodds_dw_write_keyed` (`crates/zerodds-c-api/src/publisher_ffi.rs`,
/// the `zerodds_dw_*`/`zerodds_pub_*` family) is the one C-FFI write
/// function that does.
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
        Err(e) => write_status(&e),
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
/// This entry point never has the instance's key value, only its hash —
/// unlike [`crate::publisher_ffi::zerodds_dw_dispose`] (the `zerodds_dw_*`/
/// `zerodds_pub_*` family), the `zerodds_writer_*` family has no
/// keyed-write entry point at all (`zerodds_writer_write` carries no key
/// hash either), so the writer never has a chance to remember a canonical
/// key holder for `key_hash` to fall back on here — the DATA this sends
/// always carries `PID_KEY_HASH` alone, with no `serializedKey` and the K
/// flag clear. RTI Connext and CoreDX accept that form; Eclipse Cyclone,
/// Fast DDS and OpenDDS do not treat it as a dispose
/// (`Test_FinalInstanceState_1`, measured 0/4 on all three). A typed
/// `DataWriter<T>::dispose`/`unregister_instance`, or the `zerodds_dw_*`
/// family via [`crate::publisher_ffi::zerodds_dw_write_keyed`] +
/// [`crate::publisher_ffi::zerodds_dw_dispose`], has (or can recover) the
/// instance value and sends the full form all five vendors accept; prefer
/// one of those when a real key is available. A writer created via
/// [`zerodds_writer_create_typed`] and a sample buffer (not just its key
/// hash) also has that value — see [`zerodds_writer_dispose_sample`].
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
    match rt.write_user_lifecycle(
        eid,
        kh,
        None,
        zerodds_rtps::inline_qos::status_info::DISPOSED,
        None,
    ) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(e) => write_status(&e),
    }
}

/// Sends an UNREGISTER marker (Spec §2.2.2.4.2.7). Sets only the
/// UNREGISTERED bit (no autodispose). A caller wanting spec-default
/// behavior (autodispose=true) should instead use
/// `zerodds_writer_unregister_with_dispose`.
///
/// Same `PID_KEY_HASH`-only marker caveat as [`zerodds_writer_dispose`]'s
/// own doc comment — this family never carries a key hash on a write
/// either, so there is nothing for this call to fall back on. See
/// [`zerodds_writer_unregister_sample`] for the sample-derived form.
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
    match rt.write_user_lifecycle(
        eid,
        kh,
        None,
        zerodds_rtps::inline_qos::status_info::UNREGISTERED,
        None,
    ) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(e) => write_status(&e),
    }
}

/// Sends a combined DISPOSE+UNREGISTER marker (Spec §2.2.3.21 with
/// `autodispose_unregistered_instances=true`). The reader sees both
/// NotAliveDisposed and NotAliveNoWriters.
///
/// Same `PID_KEY_HASH`-only marker caveat as [`zerodds_writer_dispose`]'s
/// own doc comment. See [`zerodds_writer_unregister_with_dispose_sample`]
/// for the sample-derived form.
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
    match rt.write_user_lifecycle(eid, kh, None, bits, None) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(e) => write_status(&e),
    }
}

/// Shared tail of [`zerodds_writer_dispose_sample`]/
/// [`zerodds_writer_unregister_sample`]/
/// [`zerodds_writer_unregister_with_dispose_sample`]: derives the real
/// KeyHash + canonical key holder from `bytes` through
/// [`DcpsRuntime::writer_sample_key_material`] and sends the lifecycle
/// marker with the full `serializedKey` form. Returns
/// [`ZeroDdsStatus::PreconditionNotMet`] instead of falling back to a
/// hash-only marker when this writer's type does not resolve as a keyed
/// `CompleteTypeObject` (untyped `zerodds_writer_create*`). A writer of a
/// topic without a key sends the marker for the topic's one instance.
fn writer_lifecycle_from_sample(
    rt: &DcpsRuntime,
    eid: EntityId,
    bytes: &[u8],
    status_bits: u32,
) -> c_int {
    // DDS 1.4 §2.2.1.2.2: a topic without a key has one instance. Its
    // writer names it by the status bits alone: there is no key to derive
    // from `bytes`.
    if eid.entity_kind == zerodds_rtps::wire_types::EntityKind::UserWriterNoKey {
        return match rt.write_user_lifecycle(eid, [0u8; 16], None, status_bits, None) {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(e) => write_status(&e),
        };
    }
    let (key_hash, holder) = match rt.writer_sample_key_material(eid, bytes) {
        Ok(pair) => pair,
        Err(_) => return ZeroDdsStatus::PreconditionNotMet as c_int,
    };
    match rt.write_user_lifecycle(eid, key_hash, Some(&holder), status_bits, None) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(e) => write_status(&e),
    }
}

/// Sends a DISPOSED lifecycle marker (Spec §9.6.3.9 PID_STATUS_INFO)
/// derived from the sample itself rather than a caller-supplied key hash.
/// `sample` is this writer's own encoding of the instance — the same bytes
/// a [`zerodds_writer_write`] payload carries (representation/byte order
/// as the writer's own encoding declares, see
/// [`zerodds_writer_set_wire_extensibility`]).
///
/// The runtime decodes `sample` against this writer's registered
/// `TypeObject` ([`zerodds_writer_create_typed`]), derives the instance's
/// real 16-byte KeyHash and its canonical key holder
/// (`DcpsRuntime::writer_sample_key_material`, the same
/// `SerializedTypeSupport::read_body` + `key_material` pair
/// `zerodds_dw_write_keyed` uses), and sends both — `PID_KEY_HASH` plus
/// `serializedKey` with the K flag set (RTPS 2.5 §9.4.2.11). That is the
/// dispose form Eclipse Cyclone 11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35
/// were all measured to act on (`Test_FinalInstanceState_1`, 4/4), unlike
/// [`zerodds_writer_dispose`]'s `PID_KEY_HASH`-only marker (0/4 on those
/// three — see its own doc comment).
///
/// Returns [`ZeroDdsStatus::PreconditionNotMet`] — never a silent
/// hash-only fallback — when this writer was not created via
/// [`zerodds_writer_create_typed`] (no registered `TypeObject`) or `sample`
/// does not decode as that type. A typed writer whose type has no `@key`
/// member writes a topic of one instance (DDS 1.4 §2.2.1.2.2); the marker
/// names it.
///
/// # Safety
/// `writer` must come from `zerodds_writer_create_typed` and be valid;
/// `sample[0..len]` must be `len` readable bytes with `len > 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_dispose_sample(
    writer: *mut ZeroDdsWriter,
    sample: *const u8,
    len: usize,
) -> c_int {
    if writer.is_null() || (sample.is_null() && len > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: writer NULL-checked above; comes from zerodds_writer_create*
    // (Box::into_raw), lives for the caller lifetime. sample NULL-checked
    // for len>0 above; sample[0..len] valid (caller pledge).
    let (rt, eid, bytes) = unsafe {
        let w = &*writer;
        (
            w.rt.clone(),
            w.eid,
            slice::from_raw_parts(sample, len).to_vec(),
        )
    };
    writer_lifecycle_from_sample(
        &rt,
        eid,
        &bytes,
        zerodds_rtps::inline_qos::status_info::DISPOSED,
    )
}

/// Sends an UNREGISTER marker (Spec §2.2.2.4.2.7) derived from the sample
/// itself — sets only the UNREGISTERED bit (no autodispose). Same
/// sample-decoding/derivation and error behavior as
/// [`zerodds_writer_dispose_sample`]'s own doc comment; a caller wanting
/// spec-default behavior (autodispose=true) should instead use
/// [`zerodds_writer_unregister_with_dispose_sample`].
///
/// # Safety
/// Like [`zerodds_writer_dispose_sample`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unregister_sample(
    writer: *mut ZeroDdsWriter,
    sample: *const u8,
    len: usize,
) -> c_int {
    if writer.is_null() || (sample.is_null() && len > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see zerodds_writer_dispose_sample's own SAFETY comment.
    let (rt, eid, bytes) = unsafe {
        let w = &*writer;
        (
            w.rt.clone(),
            w.eid,
            slice::from_raw_parts(sample, len).to_vec(),
        )
    };
    writer_lifecycle_from_sample(
        &rt,
        eid,
        &bytes,
        zerodds_rtps::inline_qos::status_info::UNREGISTERED,
    )
}

/// Sends a combined DISPOSE+UNREGISTER marker (Spec §2.2.3.21 with
/// `autodispose_unregistered_instances=true`) derived from the sample
/// itself. Same sample-decoding/derivation and error behavior as
/// [`zerodds_writer_dispose_sample`]'s own doc comment.
///
/// # Safety
/// Like [`zerodds_writer_dispose_sample`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unregister_with_dispose_sample(
    writer: *mut ZeroDdsWriter,
    sample: *const u8,
    len: usize,
) -> c_int {
    if writer.is_null() || (sample.is_null() && len > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see zerodds_writer_dispose_sample's own SAFETY comment.
    let (rt, eid, bytes) = unsafe {
        let w = &*writer;
        (
            w.rt.clone(),
            w.eid,
            slice::from_raw_parts(sample, len).to_vec(),
        )
    };
    let bits = zerodds_rtps::inline_qos::status_info::DISPOSED
        | zerodds_rtps::inline_qos::status_info::UNREGISTERED;
    writer_lifecycle_from_sample(&rt, eid, &bytes, bits)
}

/// Number of remote DataReaders currently matched to this writer (DDS 1.4
/// §2.2.4.2 PublicationMatchedStatus.current_count). Powers
/// `rmw_publisher_count_matched_subscriptions` → `Publisher.get_subscription_count()`
/// in rclpy/rclcpp. Returns `0` on a NULL handle.
///
/// # Safety
/// `writer` must come from `zerodds_writer_create*` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_matched_count(writer: *mut ZeroDdsWriter) -> usize {
    if writer.is_null() {
        return 0;
    }
    // SAFETY: writer NULL-checked; caller pledge it came from writer_create*.
    let w = unsafe { &*writer };
    w.rt.user_writer_matched_count(w.eid)
}

/// Number of remote DataWriters currently matched to this reader (DDS 1.4
/// §2.2.4.2 SubscriptionMatchedStatus.current_count). Powers
/// `rmw_subscription_count_matched_publishers` → `Subscription.get_publisher_count()`.
/// Returns `0` on a NULL handle.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create*` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_matched_count(reader: *mut ZeroDdsReader) -> usize {
    if reader.is_null() {
        return 0;
    }
    // SAFETY: reader NULL-checked; caller pledge it came from reader_create*.
    let r = unsafe { &*reader };
    r.rt.user_reader_matched_count(r.eid)
}

/// Communication status of a runtime-path writer (DDS 1.4 §2.2.4.1),
/// cumulative since the writer was created; a caller that reports changes
/// keeps the previous values.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsWriterStatus {
    /// OFFERED_DEADLINE_MISSED `total_count`.
    pub offered_deadline_missed_total: u64,
    /// LIVELINESS_LOST `total_count`.
    pub liveliness_lost_total: u64,
    /// OFFERED_INCOMPATIBLE_QOS `total_count`.
    pub offered_incompatible_qos_total: u64,
    /// OFFERED_INCOMPATIBLE_QOS `last_policy_id` (DDS QosPolicyId, 0 = none).
    pub offered_incompatible_qos_last_policy_id: u32,
    /// PUBLICATION_MATCHED `current_count`.
    pub matched_current: u64,
    /// Remote readers of an incompatible type refused on this writer's
    /// topic (XTypes 1.3 §7.6.3.4.2 INCONSISTENT_TOPIC).
    pub incompatible_type_total: u64,
}

/// Communication status of a runtime-path reader (DDS 1.4 §2.2.4.1),
/// cumulative since the reader was created.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsReaderStatus {
    /// REQUESTED_DEADLINE_MISSED `total_count`.
    pub requested_deadline_missed_total: u64,
    /// LIVELINESS_CHANGED: writers seen becoming alive, as the DCPS
    /// `DataReader` counts them.
    pub liveliness_alive: u64,
    /// LIVELINESS_CHANGED: writers seen becoming not alive.
    pub liveliness_not_alive: u64,
    /// REQUESTED_INCOMPATIBLE_QOS `total_count`.
    pub requested_incompatible_qos_total: u64,
    /// REQUESTED_INCOMPATIBLE_QOS `last_policy_id` (0 = none).
    pub requested_incompatible_qos_last_policy_id: u32,
    /// SAMPLE_LOST `total_count`.
    pub sample_lost_total: u64,
    /// SUBSCRIPTION_MATCHED `current_count`.
    pub matched_current: u64,
    /// Remote writers of an incompatible type refused on this reader's
    /// topic.
    pub incompatible_type_total: u64,
}

/// Fills `out` with the writer's communication status.
///
/// # Safety
/// `writer` from `zerodds_writer_create*` or NULL; `out` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_get_status(
    writer: *mut ZeroDdsWriter,
    out: *mut ZeroDdsWriterStatus,
) -> c_int {
    if writer.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: both NULL-checked; caller pledge on their origin.
    let (w, out) = unsafe { (&*writer, &mut *out) };
    let incompatible = w.rt.user_writer_offered_incompatible_qos(w.eid);
    *out = ZeroDdsWriterStatus {
        offered_deadline_missed_total: w.rt.user_writer_offered_deadline_missed(w.eid),
        liveliness_lost_total: w.rt.user_writer_liveliness_lost(w.eid),
        offered_incompatible_qos_total: u64::try_from(incompatible.total_count).unwrap_or(0),
        offered_incompatible_qos_last_policy_id: incompatible.last_policy_id,
        matched_current: w.rt.user_writer_matched_count(w.eid) as u64,
        incompatible_type_total: w.rt.user_endpoint_inconsistent_topic(w.eid, true),
    };
    ZeroDdsStatus::Ok as c_int
}

/// Fills `out` with the reader's communication status.
///
/// # Safety
/// `reader` from `zerodds_reader_create*` or NULL; `out` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_get_status(
    reader: *mut ZeroDdsReader,
    out: *mut ZeroDdsReaderStatus,
) -> c_int {
    if reader.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: both NULL-checked; caller pledge on their origin.
    let (r, out) = unsafe { (&*reader, &mut *out) };
    let incompatible = r.rt.user_reader_requested_incompatible_qos(r.eid);
    let (_, alive, not_alive) = r.rt.user_reader_liveliness_status(r.eid);
    *out = ZeroDdsReaderStatus {
        requested_deadline_missed_total: r.rt.user_reader_requested_deadline_missed(r.eid),
        liveliness_alive: alive,
        liveliness_not_alive: not_alive,
        requested_incompatible_qos_total: u64::try_from(incompatible.total_count).unwrap_or(0),
        requested_incompatible_qos_last_policy_id: incompatible.last_policy_id,
        sample_lost_total: r.rt.user_reader_sample_lost(r.eid),
        matched_current: r.rt.user_reader_matched_count(r.eid) as u64,
        incompatible_type_total: r.rt.user_endpoint_inconsistent_topic(r.eid, false),
    };
    ZeroDdsStatus::Ok as c_int
}

/// A unicast locator of a runtime-path endpoint (DDSI-RTPS 2.5 §8.3.5.7):
/// `kind` 1 UDPv4, 2 UDPv6, 4 TCPv4, 8 TCPv6 (vendor kinds such as
/// same-host SHM are negative); `address` the 16 address bytes, an IPv4
/// address in the last four.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsLocator {
    /// Locator kind.
    pub kind: i32,
    /// Port.
    pub port: u32,
    /// Address bytes.
    pub address: [u8; 16],
}

/// Copies up to `capacity` of the unicast locators user traffic reaches
/// this runtime on (the ones its endpoints announce over SEDP) into `out`
/// and returns how many there are, which may exceed `capacity`.
fn copy_unicast_locators(rt: &DcpsRuntime, out: *mut ZeroDdsLocator, capacity: usize) -> usize {
    let locators = &rt.user_announce_locators;
    if !out.is_null() {
        for (i, l) in locators.iter().take(capacity).enumerate() {
            // SAFETY: `out` holds `capacity` elements per the caller; i < capacity.
            unsafe {
                *out.add(i) = ZeroDdsLocator {
                    kind: l.kind.as_i32(),
                    port: l.port,
                    address: l.address,
                };
            }
        }
    }
    locators.len()
}

/// The unicast locators a writer's readers reach it on; see
/// [`ZeroDdsLocator`]. Returns the count (0 for NULL); copies at most
/// `capacity` into `out` (which may be NULL to ask for the count).
///
/// # Safety
/// `writer` from `zerodds_writer_create*` or NULL; `out` writable for
/// `capacity` elements or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unicast_locators(
    writer: *mut ZeroDdsWriter,
    out: *mut ZeroDdsLocator,
    capacity: usize,
) -> usize {
    if writer.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    copy_unicast_locators(unsafe { &(*writer).rt }, out, capacity)
}

/// [`zerodds_writer_unicast_locators`] for a reader.
///
/// # Safety
/// As [`zerodds_writer_unicast_locators`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_unicast_locators(
    reader: *mut ZeroDdsReader,
    out: *mut ZeroDdsLocator,
    capacity: usize,
) -> usize {
    if reader.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    copy_unicast_locators(unsafe { &(*reader).rt }, out, capacity)
}

/// Announces (or with a NULL `expression` withdraws) the content filter a
/// runtime-path reader applies, in its SEDP subscription as
/// `PID_CONTENT_FILTER_PROPERTY` (DDSI-RTPS 2.5 §9.6.3.4, class `DDSSQL`),
/// so a writer may filter on its side. The reader's owner still filters
/// what arrives; this only says what it wants. `related_topic` is the topic
/// the reader reads.
///
/// # Safety
/// `reader` from `zerodds_reader_create*` or NULL; the strings
/// NUL-terminated; `parameters[0..count]` valid or `count == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_content_filter(
    reader: *mut ZeroDdsReader,
    filtered_topic: *const c_char,
    related_topic: *const c_char,
    expression: *const c_char,
    parameters: *const *const c_char,
    count: usize,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let r = unsafe { &*reader };
    if expression.is_null() {
        return match r.rt.update_user_reader_content_filter(r.eid, None) {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(_) => ZeroDdsStatus::Error as c_int,
        };
    }
    let text = |p: *const c_char| -> Option<String> {
        if p.is_null() {
            return None;
        }
        // SAFETY: NUL-terminated per the caller.
        unsafe { CStr::from_ptr(p) }.to_str().ok().map(String::from)
    };
    let (Some(name), Some(related), Some(expr)) =
        (text(filtered_topic), text(related_topic), text(expression))
    else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    let mut params = Vec::with_capacity(count);
    for i in 0..count {
        // SAFETY: `parameters[0..count]` readable per the caller.
        let Some(p) = text(unsafe { *parameters.add(i) }) else {
            return ZeroDdsStatus::BadParameter as c_int;
        };
        params.push(p);
    }
    let property = zerodds_rtps::subscription_data::ContentFilterProperty {
        content_filtered_topic_name: name,
        related_topic_name: related,
        filter_class_name: zerodds_rtps::subscription_data::filter_class::DDSSQL.into(),
        filter_expression: expr,
        expression_parameters: params,
    };
    match r
        .rt
        .update_user_reader_content_filter(r.eid, Some(property))
    {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Registers `callback(user_data)`, called after a communication status of
/// any writer or reader of this runtime changed — for a waiter to re-read
/// the status with `zerodds_writer_get_status` / `zerodds_reader_get_status`.
/// It runs on a runtime thread and must not call back into ZeroDDS. There
/// is no unregistration; `user_data` must outlive the runtime.
///
/// # Safety
/// `runtime` from `zerodds_runtime_create*` or NULL; `callback` callable
/// with `user_data` from any thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_add_status_callback(
    runtime: *mut ZeroDdsRuntime,
    callback: Option<extern "C" fn(*mut c_void)>,
    user_data: *mut c_void,
) -> c_int {
    let Some(callback) = callback else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    if runtime.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Carried as an address so the hook is Send + Sync; the caller pledged
    // that `user_data` may be used from any thread.
    let user = user_data as usize;
    // SAFETY: runtime NULL-checked; caller pledge on its origin.
    let rt = unsafe { &(*runtime).rt };
    rt.add_status_hook(Arc::new(move || callback(user as *mut c_void)));
    ZeroDdsStatus::Ok as c_int
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
    // SAFETY: writer NULL-checked above.
    unsafe {
        let w = &*writer;
        // Drop any zero-copy SHM loan state keyed by this writer's (runtime, eid).
        #[cfg(feature = "flatdata-loan")]
        crate::shm_loan_ffi::forget_writer(&w.rt, w.eid);
        // Take the writer out of the runtime's local endpoint registry and send
        // the SEDP dispose. Without this the writer stays in the graph forever:
        // `rmw_count_publishers` and `rmw_get_publishers_info_by_topic` keep
        // reporting a publisher that no longer exists, and a remote peer waits
        // for a liveliness timeout instead of unmatching at once.
        w.rt.remove_user_writer(w.eid);
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
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        // F-TYPES-3: this untyped create takes no TypeObject, so it
        // advertises None. The typed variant (`zerodds_reader_create_typed`)
        // carries a TypeObject and derives a TypeIdentifier.
        ..Default::default()
    };
    let (eid, rx) = match rt_clone.register_user_reader(cfg) {
        Ok(p) => p,
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(ZeroDdsReader::new(rt_clone, eid, rx, None)))
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
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ..Default::default()
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
    Box::into_raw(Box::new(ZeroDdsReader::new(rt_clone, eid, rx, None)))
}

/// Creates a DataReader that advertises its type via a serialized COMPLETE
/// `TypeObject` (F-TYPES-3 / #24). Reader-side mirror of
/// [`zerodds_writer_create_typed`]: deserializes `type_object_bytes`, registers
/// the object into the shared TypeLookup registry, derives the strongly-hashed
/// `TypeIdentifier` and sets the reader slot's `type_identifier` from it before
/// the endpoint is published — no `None`-window against discovery.
///
/// # Safety
/// Like [`zerodds_reader_create_kind`], plus: `type_object_bytes` must point to
/// `len` readable bytes (NULL / `len == 0` is rejected with NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_create_typed(
    runtime: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_name: *const c_char,
    reliable: c_int,
    is_keyed: c_int,
    type_object_bytes: *const u8,
    len: usize,
) -> *mut ZeroDdsReader {
    if runtime.is_null()
        || topic_name.is_null()
        || type_name.is_null()
        || type_object_bytes.is_null()
        || len == 0
    {
        return ptr::null_mut();
    }
    // SAFETY: runtime/topic_name/type_name/type_object_bytes NULL-checked above;
    // both C strings are NUL-terminated and `type_object_bytes` points to `len`
    // readable bytes (caller pledge). `runtime` comes from
    // `zerodds_runtime_create`.
    let (rt_clone, topic, typ, obj_id) = unsafe {
        let topic = match CStr::from_ptr(topic_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let typ = match CStr::from_ptr(type_name).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let rt = (*runtime).rt.clone();
        let bytes = core::slice::from_raw_parts(type_object_bytes, len);
        let obj_id = match register_type_object_from_bytes(&rt, bytes) {
            Some(id) => id,
            None => return ptr::null_mut(),
        };
        (rt, topic, typ, obj_id)
    };
    let cfg = UserReaderConfig {
        topic_name: topic,
        type_name: typ,
        reliable: reliable != 0,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        // F-TYPES-3 / #24: derived from the registered TypeObject (not None).
        type_identifier: obj_id,
        ..Default::default()
    };
    let (eid, rx) = match rt_clone.register_user_reader_kind(cfg, is_keyed != 0) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "zerodds_reader_create_typed(topic={topic_name:?}, is_keyed={is_keyed}) failed: {e:?}"
            );
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(ZeroDdsReader::new(rt_clone, eid, rx, None)))
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
/// This little-endian-default form does not surface the wire byte order;
/// a consumer that must decode a big-endian peer uses
/// [`zerodds_reader_take_endian`].
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
    // SAFETY: delegates to the endian-aware form with a NULL `out_be`.
    unsafe { zerodds_reader_take_endian(reader, out_buf, out_len, out_repr, ptr::null_mut()) }
}

/// Like [`zerodds_reader_take`], plus `out_be` (nullable): receives the wire
/// byte order — `0` = little-endian (the canonical XCDR2 wire), `1` =
/// big-endian — derived from the encapsulation representation identifier
/// (RTPS 2.5 §10.5). A typed FFI consumer dispatches its little-endian vs
/// big-endian decoder on this, so a big-endian peer's sample decodes
/// correctly. NULL = ignore (assume little-endian).
///
/// # Safety
/// Pointers must be valid. `out_repr` and `out_be` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take_endian(
    reader: *mut ZeroDdsReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_repr: *mut u8,
    out_be: *mut u8,
) -> c_int {
    if reader.is_null() || out_buf.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — reader+out_buf+out_len NULL-checked above.
    // The C-API delivers only alive samples; lifecycle markers are deliberately discarded,
    // so that the C-FFI consumer sees unchanged bytes.
    unsafe {
        let r = &*reader;
        let bytes = match r.next_alive() {
            Ok(sample) => sample,
            Err(()) => {
                *out_buf = ptr::null_mut();
                *out_len = 0;
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
        };
        match bytes {
            Some((bs, repr, be)) => {
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
                if !out_be.is_null() {
                    *out_be = u8::from(be);
                }
            }
            None => {
                *out_buf = ptr::null_mut();
                *out_len = 0;
                if !out_repr.is_null() {
                    *out_repr = 0;
                }
                if !out_be.is_null() {
                    *out_be = 0;
                }
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes a sample with the sample it answers in the DATA's inline QoS:
/// `PID_RELATED_SAMPLE_IDENTITY` (0x0083, DDS-RPC 1.0 §7.8.2) and
/// eProsima's 0x800f. `payload` points to already-encoded bytes.
///
/// `related_guid` points to the 16 octets of the writer GUID of the
/// related sample and `related_sn` is its RTPS sequence number. `out_sn`
/// (nullable) receives the RTPS sequence number this sample went out with.
///
/// # Safety
/// `writer`, `payload` (if `len > 0`) and `related_guid` must be valid;
/// `out_sn` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_write_related(
    writer: *mut ZeroDdsWriter,
    payload: *const u8,
    len: usize,
    related_guid: *const u8,
    related_sn: u64,
    out_sn: *mut i64,
) -> c_int {
    if writer.is_null() || related_guid.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: writer NULL-checked; `related_guid` points to 16 octets
    // (caller duty per the # Safety doc).
    let (rt, eid, guid) = unsafe {
        let w = &*writer;
        let mut guid = [0u8; 16];
        guid.copy_from_slice(slice::from_raw_parts(related_guid, 16));
        (w.rt.clone(), w.eid, guid)
    };
    let bytes: &[u8] = if len == 0 {
        &[]
    } else {
        // SAFETY: payload is non-NULL and `len` bytes valid (caller duty).
        unsafe { slice::from_raw_parts(payload, len) }
    };
    let related = zerodds_rtps::inline_qos::SampleIdentityBytes::new(guid, related_sn);
    match rt.write_user_sample_related(eid, bytes, related) {
        Ok(sn) => {
            if !out_sn.is_null() {
                // SAFETY: non-NULL and valid per the # Safety doc.
                unsafe { *out_sn = sn };
            }
            ZeroDdsStatus::Ok as c_int
        }
        Err(e) => write_status(&e),
    }
}

/// Writes a sample for some of the matched readers only (DDS-RPC 1.0
/// §7.11.1.4.3: a request through a requester bound to a service instance
/// "shall be sent to the bound service instance only"). `payload` points to
/// already-encoded bytes. `reader_guids` points to `reader_count` GUIDs of
/// 16 octets each, the endpoint GUIDs of the readers the sample is for (a
/// reader's `zerodds_reader_guid`, or the key of a matched reader that
/// announced the DDS-RPC discovery extension). `out_sn` (nullable) receives
/// the RTPS sequence number the sample went out with.
///
/// Every other matched reader is told the sample is not for it (a GAP for
/// the number, DDSI-RTPS 2.5 §8.3.7.4) and does not get it, and neither does
/// a reader of the same runtime. A GUID that is no matched reader names
/// nobody; with `reader_count` 0 the sample is for none.
///
/// # Safety
/// `writer` and `payload` (if `len > 0`) must be valid; `reader_guids` must
/// point to `reader_count * 16` readable bytes when `reader_count > 0`;
/// `out_sn` is NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_write_to_readers(
    writer: *mut ZeroDdsWriter,
    payload: *const u8,
    len: usize,
    reader_guids: *const u8,
    reader_count: usize,
    out_sn: *mut i64,
) -> c_int {
    if writer.is_null()
        || (payload.is_null() && len > 0)
        || (reader_guids.is_null() && reader_count > 0)
    {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: writer NULL-checked; `reader_guids` points to
    // `reader_count * 16` octets (caller duty per the # Safety doc).
    let (rt, eid, readers) = unsafe {
        let w = &*writer;
        let mut readers = Vec::with_capacity(reader_count);
        for at in 0..reader_count {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(slice::from_raw_parts(reader_guids.add(at * 16), 16));
            readers.push(zerodds_rtps::wire_types::Guid::from_bytes(bytes));
        }
        (w.rt.clone(), w.eid, readers)
    };
    let bytes: &[u8] = if len == 0 {
        &[]
    } else {
        // SAFETY: payload is non-NULL and `len` bytes valid (caller duty).
        unsafe { slice::from_raw_parts(payload, len) }
    };
    match rt.write_user_sample_to_readers(eid, bytes, &readers) {
        Ok(sn) => {
            if !out_sn.is_null() {
                // SAFETY: non-NULL and valid per the # Safety doc.
                unsafe { *out_sn = sn };
            }
            ZeroDdsStatus::Ok as c_int
        }
        Err(e) => write_status(&e),
    }
}

/// [`zerodds_reader_take_endian`], plus where the sample came from, from
/// its DATA submessage: the writer GUID (16 octets), the RTPS sequence
/// number, and `PID_RELATED_SAMPLE_IDENTITY` (DDS-RPC 1.0 §7.8.2). Every
/// out parameter after `out_be` is nullable. `out_has_related` receives
/// `1` when the DATA carried a related sample identity — `out_related_guid`
/// (16 octets) and `out_related_sn` are then set — and `0` otherwise.
///
/// # Safety
/// As [`zerodds_reader_take_endian`]; the GUID pointers point to 16
/// writable octets when not NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take_wire(
    reader: *mut ZeroDdsReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_repr: *mut u8,
    out_be: *mut u8,
    out_writer_guid: *mut u8,
    out_sn: *mut i64,
    out_has_related: *mut u8,
    out_related_guid: *mut u8,
    out_related_sn: *mut u64,
) -> c_int {
    if reader.is_null() || out_buf.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: reader NULL-checked above; the out pointers are valid or
    // NULL per the # Safety doc.
    unsafe {
        let r = &*reader;
        let next = match r.next_alive() {
            Ok(next) => next,
            Err(()) => {
                *out_buf = ptr::null_mut();
                *out_len = 0;
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
        };
        let Some((bs, repr, be)) = next else {
            *out_buf = ptr::null_mut();
            *out_len = 0;
            if !out_has_related.is_null() {
                *out_has_related = 0;
            }
            return ZeroDdsStatus::Ok as c_int;
        };
        let wire = bs.wire().copied();
        let mut boxed = bs.to_vec().into_boxed_slice();
        *out_buf = boxed.as_mut_ptr();
        *out_len = boxed.len();
        let _ = Box::into_raw(boxed);
        if !out_repr.is_null() {
            *out_repr = repr;
        }
        if !out_be.is_null() {
            *out_be = u8::from(be);
        }
        if !out_writer_guid.is_null() {
            let guid = wire.map_or([0u8; 16], |w| w.writer_guid);
            ptr::copy_nonoverlapping(guid.as_ptr(), out_writer_guid, 16);
        }
        if !out_sn.is_null() {
            *out_sn = wire.map_or(-1, |w| w.sequence_number);
        }
        let related = wire.and_then(|w| w.related_sample_identity);
        if !out_has_related.is_null() {
            *out_has_related = u8::from(related.is_some());
        }
        if let Some(related) = related {
            if !out_related_guid.is_null() {
                ptr::copy_nonoverlapping(related.writer_guid.as_ptr(), out_related_guid, 16);
            }
            if !out_related_sn.is_null() {
                *out_related_sn = related.sequence_number;
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

/// A2 — arm TIME_BASED_FILTER (DDS 1.4 §2.2.3.12) on a reader: it then delivers
/// at most one sample per instance per `min_separation_nanos`; closer-spaced
/// samples are dropped before `zerodds_reader_take`/the data callback sees them.
/// `0` disables the filter. ROS 2 exposes no TIME_BASED_FILTER field in
/// `rmw_qos_profile_t`, so `rmw_zerodds` calls this with the runtime's
/// `ros2.time_based_filter` (env `ZERODDS_TIME_BASED_FILTER`). Returns `0` on
/// success, `-1` on a NULL/unknown reader.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create*` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_time_based_filter_ns(
    reader: *mut ZeroDdsReader,
    min_separation_nanos: u64,
) -> c_int {
    if reader.is_null() {
        return -1;
    }
    // SAFETY: reader NULL-checked; caller pledge it came from reader_create*.
    let r = unsafe { &*reader };
    if r.rt
        .set_user_reader_time_based_filter(r.eid, u128::from(min_separation_nanos))
    {
        0
    } else {
        -1
    }
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
        // Symmetric to the writer: leave the local endpoint registry and dispose
        // the SEDP subscription, so the reader disappears from the graph.
        r.rt.remove_user_reader(r.eid);
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
/// * Samples the reader received before the callback was set (a
///   TRANSIENT_LOCAL writer's retained samples, anything no `take` handed
///   out) reach it during the `set` call, on the calling thread, oldest
///   first and before any later sample.
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
/// * `big_endian` is the wire byte order from the encapsulation header
///   (RTPS 2.5 §10.5): `0` = little-endian, `1` = big-endian. The typed
///   consumer dispatches the little-endian vs big-endian decoder on it.
///   Intra-runtime / durability-replay samples are always `0`.
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
    big_endian: u8,
);

/// Registers `listener` on `r` and hands it the samples the reader already
/// holds ([`ZeroDdsReader::drain_backlog`]), or clears the listener with
/// `None`. `false` if the runtime no longer knows the reader.
///
/// Without the backlog, a callback set right after `zerodds_reader_create*`
/// missed what arrived before it: the retained samples a TRANSIENT_LOCAL
/// writer of the same runtime replays while the reader is created, and a
/// remote writer's answer to the reader's SEDP announcement.
fn set_reader_listener(
    r: &ZeroDdsReader,
    listener: Option<zerodds_dcps::runtime::UserReaderListener>,
) -> bool {
    match listener {
        Some(l) => {
            r.rt.set_user_reader_listener_with_backlog(r.eid, l, || r.drain_backlog())
        }
        None => r.rt.set_user_reader_listener(r.eid, None),
    }
}

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
    let r = unsafe { &*reader };
    let listener: Option<zerodds_dcps::runtime::UserReaderListener> = match callback {
        Some(cb) => {
            // Store user_data as a usize, because *mut c_void is not Send;
            // in the closure we cast it back. Per the contract the caller must keep
            // user_data alive until the listener is cleared with NULL.
            let ud_addr = user_data as usize;
            Some(Box::new(
                move |bytes: &[u8],
                      repr: u8,
                      big_endian: u8,
                      _origin: &zerodds_dcps::runtime::SampleOrigin| {
                    cb(
                        ud_addr as *mut core::ffi::c_void,
                        bytes.as_ptr(),
                        bytes.len(),
                        repr,
                        big_endian,
                    );
                },
            ))
        }
        None => None,
    };
    if set_reader_listener(r, listener) {
        ZeroDdsStatus::Ok as c_int
    } else {
        ZeroDdsStatus::BadHandle as c_int
    }
}

/// Where a sample handed to a [`ZeroDdsDataCallbackEx`] came from.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleOrigin {
    /// The writer's GUID.
    pub writer_guid: [u8; 16],
    /// The writer's source timestamp in nanoseconds since the Unix epoch,
    /// `-1` when the sample came without one (same-runtime delivery, a
    /// durability replay).
    pub source_timestamp_ns: i64,
    /// The writer's RTPS sequence number of the sample, `-1` when unknown.
    pub sequence_number: i64,
}

/// [`ZeroDdsDataCallback`] with the sample's origin; `origin` is valid for
/// the call.
pub type ZeroDdsDataCallbackEx = extern "C" fn(
    user_data: *mut core::ffi::c_void,
    payload: *const u8,
    payload_len: usize,
    representation: u8,
    big_endian: u8,
    origin: *const ZeroDdsSampleOrigin,
);

/// [`zerodds_reader_set_data_callback`] for a [`ZeroDdsDataCallbackEx`],
/// which also receives the writer's GUID, source timestamp and sequence
/// number of each sample. Same contract.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_data_callback_ex(
    reader: *mut ZeroDdsReader,
    callback: Option<
        extern "C" fn(
            user_data: *mut core::ffi::c_void,
            payload: *const u8,
            payload_len: usize,
            representation: u8,
            big_endian: u8,
            origin: *const ZeroDdsSampleOrigin,
        ),
    >,
    user_data: *mut core::ffi::c_void,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: reader NULL-checked above.
    let r = unsafe { &*reader };
    let listener: Option<zerodds_dcps::runtime::UserReaderListener> = callback.map(|cb| {
        let ud_addr = user_data as usize;
        let listener: zerodds_dcps::runtime::UserReaderListener = Box::new(
            move |bytes: &[u8],
                  repr: u8,
                  big_endian: u8,
                  origin: &zerodds_dcps::runtime::SampleOrigin| {
                let origin = ZeroDdsSampleOrigin {
                    writer_guid: origin.writer_guid,
                    source_timestamp_ns: origin.source_timestamp.map_or(-1, |t| {
                        let time = zerodds_dcps::time::he_timestamp_to_time(t);
                        i64::from(time.sec) * 1_000_000_000 + i64::from(time.nanosec)
                    }),
                    sequence_number: origin.sequence_number,
                };
                cb(
                    ud_addr as *mut core::ffi::c_void,
                    bytes.as_ptr(),
                    bytes.len(),
                    repr,
                    big_endian,
                    &origin,
                );
            },
        );
        listener
    });
    if set_reader_listener(r, listener) {
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
        let bytes = match r.next_alive() {
            Ok(sample) => sample.map(|(b, _, _)| b),
            Err(()) => {
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
        Err(e) => write_status(&e),
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
    /// In-memory form via the iceoryx2 bridge — same-host, cross-stack. Wired
    /// behind the `delivery-iceoryx` build feature; the commit-time copy into
    /// the iceoryx slot is the remaining zero-copy refinement
    /// (`zerodds-delivery-modes-1.0` §11).
    Iceoryx = 2,
}

/// Sets the delivery mode of a runtime-path DataWriter (the handle the ROS-2
/// RMW bridge uses). `0`=Portable (default, interop-safe), `1`=RawSameHost
/// (same-host, no wire). `2`=Iceoryx routes over the iceoryx2 bridge and is
/// opt-in: it requires building with the `delivery-iceoryx` feature and returns
/// `Unsupported` when that feature is not compiled in. Other values →
/// `BadParameter`.
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

/// ABI revision of this library (`zerodds-ffi-loader-1.0` §7).
///
/// The revision is bumped only on a major version, when the binary
/// interface breaks. A patch release changes nothing about the ABI and a
/// minor release only adds symbols, so both keep the revision they had.
///
/// A loader is expected to compare this against the revision it was built
/// for and to fail fast on a mismatch, rather than calling into a library
/// whose symbols mean something else:
///
/// ```c
/// if (zerodds_abi_revision() != ZERODDS_ABI_REVISION) {
///     fprintf(stderr, "libzerodds ABI %u, expected %u\n",
///             zerodds_abi_revision(), ZERODDS_ABI_REVISION);
///     return 1;
/// }
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_abi_revision() -> u32 {
    ZERODDS_ABI_REVISION
}

/// The ABI revision the current major version carries
/// (`zerodds-ffi-loader-1.0` §7). Consumers compile against this constant
/// and check it against [`zerodds_abi_revision`] at load time.
pub const ZERODDS_ABI_REVISION: u32 = 1;

// ============================================================================
// Tests — the offer the plain runtime path starts with
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod runtime_destroy_tests {
    use super::*;

    /// Destroying a runtime stops it: its worker threads let go of it, so
    /// the clone held here is the last one within moments. Before, the
    /// threads kept running until the process ended.
    #[test]
    fn destroy_stops_the_runtime_threads() {
        let domain = u32::try_from(zerodds_test_domains::unique_domain()).unwrap();
        // SAFETY: a fresh runtime, destroyed once below.
        let handle = unsafe { zerodds_runtime_create(domain) };
        assert!(!handle.is_null());
        // SAFETY: `handle` is valid until the destroy.
        let rt = Arc::clone(unsafe { &(*handle).rt });
        assert!(Arc::strong_count(&rt) > 2, "the worker threads hold it");
        // SAFETY: from zerodds_runtime_create, not used after.
        unsafe { zerodds_runtime_destroy(handle) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while Arc::strong_count(&rt) > 1 {
            assert!(
                std::time::Instant::now() < deadline,
                "{} holders left after destroy",
                Arc::strong_count(&rt)
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod endpoint_qos_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn durations_convert_and_zero_is_infinite() {
        assert!(qos_duration(0).is_infinite());
        let d = qos_duration(1_500_000_000);
        assert_eq!(d.seconds, 1);
        assert_eq!(d.fraction, 1 << 31, "half a second is 2^31 / 2^32");
        assert!(qos_duration(u64::MAX).is_infinite());
    }

    #[test]
    fn an_invalid_value_is_refused_by_name() {
        let base = zerodds_endpoint_qos_default();
        for (q, needle) in [
            (
                ZeroDdsEndpointQos {
                    reliable: 2,
                    ..base
                },
                "reliable",
            ),
            (
                ZeroDdsEndpointQos {
                    liveliness_kind: 7,
                    ..base
                },
                "liveliness_kind",
            ),
            (
                ZeroDdsEndpointQos {
                    history_depth: 0,
                    ..base
                },
                "history_depth",
            ),
        ] {
            let err = endpoint_qos(&q).err().expect("refused");
            assert!(err.contains(needle), "{err}");
        }
        let keep_all = ZeroDdsEndpointQos {
            history_keep_all: 1,
            history_depth: 0,
            ..base
        };
        assert_eq!(endpoint_qos(&keep_all).unwrap().keep_last, None);
    }

    /// DDS 1.4 §2.2.3.19: a limit is positive or LENGTH_UNLIMITED, one
    /// instance holds no more than all, KEEP_LAST keeps no more than the
    /// per-instance limit.
    #[test]
    fn resource_limits_are_checked_by_name() {
        let base = zerodds_endpoint_qos_default();
        for (q, needle) in [
            (
                ZeroDdsEndpointQos {
                    max_samples: 0,
                    ..base
                },
                "max_samples = 0",
            ),
            (
                ZeroDdsEndpointQos {
                    max_instances: -2,
                    ..base
                },
                "max_instances = -2",
            ),
            (
                ZeroDdsEndpointQos {
                    max_samples: 2,
                    max_samples_per_instance: 5,
                    ..base
                },
                "exceeds max_samples",
            ),
            (
                ZeroDdsEndpointQos {
                    history_depth: 3,
                    max_samples_per_instance: 2,
                    ..base
                },
                "history_depth = 3",
            ),
        ] {
            let err = endpoint_qos(&q).err().expect("refused");
            assert!(err.contains(needle), "{err}");
        }
        let q = endpoint_qos(&base).unwrap();
        assert_eq!(
            q.resource_limits,
            zerodds_qos::ResourceLimitsQosPolicy::default()
        );
        assert_eq!(q.max_blocking_time, qos_duration(100_000_000));
        let no_wait = ZeroDdsEndpointQos {
            max_blocking_time_ns: 0,
            ..base
        };
        assert_eq!(
            endpoint_qos(&no_wait).unwrap().max_blocking_time,
            zerodds_qos::Duration::ZERO
        );
        let forever = ZeroDdsEndpointQos {
            max_blocking_time_ns: ZERODDS_BLOCKING_INFINITE,
            ..base
        };
        assert!(
            endpoint_qos(&forever)
                .unwrap()
                .max_blocking_time
                .is_infinite()
        );
    }

    /// TRANSIENT_LOCAL, KEEP_ALL, `max_samples = 2`: the third write finds
    /// no room that waiting could make and returns OUT_OF_RESOURCES (DDS 1.4
    /// §2.2.3.19). Before, the flat QoS had no RESOURCE_LIMITS, and the flat
    /// write answered every failure with ERROR.
    #[test]
    fn a_flat_writer_applies_its_resource_limits() {
        let domain = u32::try_from(zerodds_test_domains::unique_domain()).unwrap();
        // SAFETY: valid domain.
        let rt = unsafe { zerodds_runtime_create(domain) };
        assert!(!rt.is_null());
        let topic = CString::new("QosLimits").unwrap();
        let typ = CString::new("Bytes").unwrap();
        let qos = ZeroDdsEndpointQos {
            transient_local: 1,
            history_keep_all: 1,
            max_samples: 2,
            ..zerodds_endpoint_qos_default()
        };
        let writer =
            // SAFETY: valid runtime, strings and QoS.
            unsafe { zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos) };
        assert!(!writer.is_null());
        let codes: Vec<c_int> = (1u8..=3)
            .map(|n| {
                let body = [0u8, 7, 0, 0, n];
                // SAFETY: valid writer, body pointer and length.
                unsafe { zerodds_writer_write(writer, body.as_ptr(), body.len()) }
            })
            .collect();
        assert_eq!(
            codes,
            vec![0, 0, ZeroDdsStatus::OutOfResources as c_int],
            "the third write is over max_samples"
        );
        // SAFETY: handles from their creates, destroyed once.
        unsafe {
            zerodds_writer_destroy(writer);
            zerodds_runtime_destroy(rt);
        }
    }

    #[test]
    fn a_keep_last_reader_takes_only_the_newest_samples() {
        let domain = u32::try_from(zerodds_test_domains::unique_domain()).unwrap();
        // SAFETY: valid domain.
        let rt = unsafe { zerodds_runtime_create(domain) };
        assert!(!rt.is_null());
        let topic = CString::new("QosKeepLast").unwrap();
        let typ = CString::new("Bytes").unwrap();
        let qos = ZeroDdsEndpointQos {
            history_keep_all: 0,
            history_depth: 2,
            ..zerodds_endpoint_qos_default()
        };
        let reader =
            // SAFETY: valid runtime, strings and QoS.
            unsafe { zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos) };
        let writer =
            // SAFETY: as above.
            unsafe { zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos) };
        assert!(!reader.is_null() && !writer.is_null());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        // Same runtime: intra-process delivery once the route exists.
        while std::time::Instant::now() < deadline {
            // SAFETY: valid writer.
            if unsafe { (*writer).rt.user_writer_matched_count((*writer).eid) } > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        for n in 1u8..=5 {
            let body = [0u8, 7, 0, 0, n];
            // SAFETY: valid writer, body pointer and length.
            let rc = unsafe { zerodds_writer_write(writer, body.as_ptr(), body.len()) };
            assert_eq!(rc, 0);
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut got = Vec::new();
        loop {
            let mut buf = ptr::null_mut();
            let mut len = 0usize;
            // SAFETY: valid reader and out-pointers.
            let rc = unsafe { zerodds_reader_take(reader, &mut buf, &mut len, ptr::null_mut()) };
            assert_eq!(rc, 0);
            if buf.is_null() {
                break;
            }
            // SAFETY: buf/len from take.
            got.push(unsafe { *buf.add(len - 1) });
            // SAFETY: buffer from take, freed once.
            unsafe { zerodds_buffer_free(buf, len) };
        }
        assert_eq!(got, vec![4, 5], "KEEP_LAST(2): the newest two");
        // SAFETY: handles from their creates, destroyed once.
        unsafe {
            zerodds_writer_destroy(writer);
            zerodds_reader_destroy(reader);
            zerodds_runtime_destroy(rt);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod config_selection_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn a_c_caller_selects_a_file_and_sets_values_above_it() {
        let dir = std::env::temp_dir().join(format!("zerodds-ffi-select-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.join("app.yaml");
        std::fs::write(&base, "transport:\n  recv_batch: 8\ntsn:\n  vlan: 3\n").unwrap();
        let path = CString::new(base.to_str().unwrap()).unwrap();
        let set = CString::new("transport.recv_batch=64").unwrap();
        let sets = [set.as_ptr()];
        // SAFETY: valid C strings, n_sets matches the array.
        let cl = unsafe { command_line_from_c(path.as_ptr(), sets.as_ptr(), 1) }.unwrap();
        let source = zerodds_dcps::runtime::ConfigSource::from_command_line(&cl).unwrap();
        let config = ffi_config_from_source(&source, ffi_data_representation_offer()).unwrap();
        assert_eq!(config.recv_batch_size, 64, "the set wins over the file");
        assert_eq!(config.tsn_vlan, 3, "the file wins over the default");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_setting_or_a_missing_file_yields_null() {
        let set = CString::new("transport.recv_bacth=8").unwrap();
        let sets = [set.as_ptr()];
        // SAFETY: valid C strings; the configuration is refused before any
        // socket exists.
        let rt = unsafe { zerodds_runtime_create_from_config(0, ptr::null(), sets.as_ptr(), 1) };
        assert!(rt.is_null());
        let missing = CString::new("/nonexistent/zerodds-ffi.yaml").unwrap();
        // SAFETY: as above.
        let rt = unsafe { zerodds_runtime_create_from_config(0, missing.as_ptr(), ptr::null(), 0) };
        assert!(rt.is_null());
        // SAFETY: as above; a count without an array is refused, not read.
        let rt = unsafe { zerodds_runtime_create_from_config(0, ptr::null(), ptr::null(), 2) };
        assert!(rt.is_null());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod offer_tests {
    use super::*;

    #[test]
    fn file_runtime_settings_reach_the_ffi_without_losing_its_default_offer() {
        use zerodds_dcps::runtime::ConfigSource;
        let dir = std::env::temp_dir().join(format!("zerodds-ffi-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zerodds.yaml");
        std::fs::write(&path, "transport:\n  recv_batch: 7\n").unwrap();
        let source = ConfigSource::from_path(&path).unwrap();
        let config = ffi_config_from_source(&source, ffi_data_representation_offer()).unwrap();
        assert_eq!(config.recv_batch_size, 7);
        assert_eq!(config.data_representation_offer, vec![2, 0]);

        std::fs::write(
            &path,
            "wire:\n  data_representation_offer:\n    - XCDR1\n    - XCDR2\n",
        )
        .unwrap();
        let source = ConfigSource::from_path(&path).unwrap();
        let config = ffi_config_from_source(&source, ffi_data_representation_offer()).unwrap();
        assert_eq!(config.data_representation_offer, vec![0, 2]);

        std::fs::write(&path, "wire:\n  data_representation_offer:\n    - 99\n").unwrap();
        let source = ConfigSource::from_path(&path).unwrap();
        assert!(ffi_config_from_source(&source, ffi_data_representation_offer()).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn plain_runtime_offers_xcdr2_first() {
        // Regression: `zerodds_runtime_create` took
        // `RuntimeConfig::default()`, whose offer is `DEFAULT_OFFER =
        // [XCDR1, XCDR2]`. The byte-oriented FFI writer derives the
        // encapsulation header it stamps from the offer's FIRST element,
        // so it stamped `0x0001` on the XCDR2 bodies its callers hand
        // it; readers then reported representation = XCDR1 and decoded
        // at XCDR1 offsets. Plain zerodds -> zerodds delivered zero
        // samples in the cross-vendor security matrix while the secured
        // path, which pinned XCDR2-first, measured 13/13.
        use zerodds_rtps::publication_data::data_representation as dr;
        let offer = ffi_data_representation_offer();
        assert_eq!(
            offer.first().copied(),
            Some(dr::XCDR2),
            "the FFI writer must stamp an XCDR2 encapsulation on the XCDR2 \
             bodies its callers serialise"
        );
        assert!(
            offer.contains(&dr::XCDR),
            "the offer SET must still advertise XCDR1, so a reader created \
             here accepts a Cyclone / rmw_* XCDR1 writer without configuration"
        );
    }
}

// ============================================================================
// Tests — F-TYPES-3 / #24 typed-create TypeObject path
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod typed_create_tests {
    use super::*;
    use std::ffi::CString;

    use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
    use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
    use zerodds_types::{PrimitiveKind, TypeIdentifier, compute_complete_hash};

    /// A known struct's COMPLETE `TypeObject`, built via the `StructBuilder`
    /// "Path A" uses. Returns `(TypeObject, expected_identifier)`.
    fn sample_typeobject() -> (TypeObject, TypeIdentifier) {
        let cs = TypeObjectBuilder::struct_type("Sensor")
            .extensibility(Extensibility::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member(
                "value",
                TypeIdentifier::Primitive(PrimitiveKind::Float64),
                |m| m,
            )
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(cs));
        let expected = zerodds_types::to_hashed_type_identifier(&to).unwrap();
        (to, expected)
    }

    /// Core assertion: the identifier the typed-create path derives from the
    /// serialized TypeObject bytes is BYTE-IDENTICAL to the one Path A's
    /// `StructBuilder::build_complete()` produces — and the object lands in the
    /// runtime's TypeLookup registry.
    #[test]
    fn typed_derive_is_byte_identical_to_path_a_and_registers() {
        let (to, expected) = sample_typeobject();
        let bytes = to.to_bytes_le().unwrap();
        let TypeObject::Complete(cto) = &to else {
            panic!("sample_typeobject builds a Complete TypeObject");
        };
        let expected_hash = compute_complete_hash(cto).unwrap();

        // A fixed number, not `base + pid % N`: the domain is the UDP
        // port (RTPS 9.6.1.4.1, `7400 + 250*domain`), and spreading it by
        // PID walked it into the kernel's ephemeral port range
        // (`net.ipv4.ip_local_port_range`, `32768 60999` on the bench
        // host), where `zerodds_runtime_create` fails with
        // `TransportError { label: "spdp multicast bind" }` and returns
        // NULL. Measured: 11 of 90 parallel runs of this binary, on
        // domains 124, 128 and 166. Every domain in this workspace is
        // listed in `crates/dcps/tests/common/mod.rs::RESERVED_DOMAINS`
        // so that the domain lease there never hands out the same one.
        let domain: u32 = 23;
        // SAFETY: valid domain; handle deref within the crate.
        let handle = unsafe { zerodds_runtime_create(domain) };
        assert!(!handle.is_null());
        // SAFETY: non-null handle from create; deref within the crate.
        let rt = unsafe { (*handle).rt.clone() };

        let derived =
            register_type_object_from_bytes(&rt, &bytes).expect("deserialize+register+derive");

        // BYTE-IDENTICAL to Path A (compare the serialized identifier bytes,
        // not only structural equality).
        assert_eq!(derived, expected, "derived identifier != Path A identifier");
        assert_eq!(
            derived.to_bytes_le().unwrap(),
            expected.to_bytes_le().unwrap(),
            "derived identifier bytes != Path A identifier bytes"
        );

        // The COMPLETE TypeObject is now in the runtime's registry.
        {
            let server = rt.type_lookup_server.lock().unwrap();
            assert!(
                server.registry.get_complete(&expected_hash).is_some(),
                "TypeObject not registered under its complete hash"
            );
        }

        // SAFETY: handle from create, not yet destroyed; consumed once here.
        unsafe { zerodds_runtime_destroy(handle) };
    }

    /// Full FFI: `zerodds_writer_create_typed` succeeds and has registered the
    /// TypeObject before returning (i.e. before the slot is published).
    #[test]
    fn writer_and_reader_create_typed_register_before_publish() {
        let (to, _expected) = sample_typeobject();
        let bytes = to.to_bytes_le().unwrap();
        let TypeObject::Complete(cto) = &to else {
            panic!("sample_typeobject builds a Complete TypeObject");
        };
        let expected_hash = compute_complete_hash(cto).unwrap();

        // A fixed number, not `base + pid % N`: the domain is the UDP
        // port (RTPS 9.6.1.4.1, `7400 + 250*domain`), and spreading it by
        // PID walked it into the kernel's ephemeral port range
        // (`net.ipv4.ip_local_port_range`, `32768 60999` on the bench
        // host), where `zerodds_runtime_create` fails with
        // `TransportError { label: "spdp multicast bind" }` and returns
        // NULL. Measured: 11 of 90 parallel runs of this binary, on
        // domains 124, 128 and 166. Every domain in this workspace is
        // listed in `crates/dcps/tests/common/mod.rs::RESERVED_DOMAINS`
        // so that the domain lease there never hands out the same one.
        let domain: u32 = 24;
        let topic = CString::new("TypedTopic").unwrap();
        let typ = CString::new("Sensor").unwrap();

        // SAFETY: valid C strings + valid byte slice for the typed create.
        unsafe {
            let handle = zerodds_runtime_create(domain);
            assert!(!handle.is_null());

            let w = zerodds_writer_create_typed(
                handle,
                topic.as_ptr(),
                typ.as_ptr(),
                1,
                1,
                bytes.as_ptr(),
                bytes.len(),
            );
            assert!(!w.is_null(), "writer_create_typed returned NULL");

            let r = zerodds_reader_create_typed(
                handle,
                topic.as_ptr(),
                typ.as_ptr(),
                1,
                1,
                bytes.as_ptr(),
                bytes.len(),
            );
            assert!(!r.is_null(), "reader_create_typed returned NULL");

            // Registered by both create paths.
            {
                let rt = (*handle).rt.clone();
                let server = rt.type_lookup_server.lock().unwrap();
                assert!(server.registry.get_complete(&expected_hash).is_some());
            }

            zerodds_writer_destroy(w);
            zerodds_reader_destroy(r);
            zerodds_runtime_destroy(handle);
        }
    }

    /// A NULL / zero-length / malformed TypeObject is rejected with NULL — the
    /// typed path never falls back to a `None`-identifier endpoint.
    #[test]
    fn typed_create_rejects_invalid_type_object() {
        // A fixed number, not `base + pid % N`: the domain is the UDP
        // port (RTPS 9.6.1.4.1, `7400 + 250*domain`), and spreading it by
        // PID walked it into the kernel's ephemeral port range
        // (`net.ipv4.ip_local_port_range`, `32768 60999` on the bench
        // host), where `zerodds_runtime_create` fails with
        // `TransportError { label: "spdp multicast bind" }` and returns
        // NULL. Measured: 11 of 90 parallel runs of this binary, on
        // domains 124, 128 and 166. Every domain in this workspace is
        // listed in `crates/dcps/tests/common/mod.rs::RESERVED_DOMAINS`
        // so that the domain lease there never hands out the same one.
        let domain: u32 = 25;
        let topic = CString::new("TypedBad").unwrap();
        let typ = CString::new("Sensor").unwrap();
        let garbage: [u8; 3] = [0x00, 0x01, 0x02];

        // SAFETY: valid strings; deliberately NULL / bad byte pointers.
        unsafe {
            let handle = zerodds_runtime_create(domain);
            assert!(!handle.is_null());

            // NULL bytes.
            assert!(
                zerodds_writer_create_typed(
                    handle,
                    topic.as_ptr(),
                    typ.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                    0,
                )
                .is_null()
            );
            // Malformed (non-empty but not a valid TypeObject) bytes.
            assert!(
                zerodds_reader_create_typed(
                    handle,
                    topic.as_ptr(),
                    typ.as_ptr(),
                    1,
                    1,
                    garbage.as_ptr(),
                    garbage.len(),
                )
                .is_null()
            );

            zerodds_runtime_destroy(handle);
        }
    }
}
