// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `rmw_zerodds` — ROS 2 RMW shim ueber ZeroDDS.
//!
//! Crate `rmw-zerodds-shim`. Safety classification: **STANDARD** (FFI-Boundary).
//!
//! # Architektur
//!
//! ```text
//!   rclcpp/rclpy
//!       │  rmw API (REP-2007)
//!       ▼
//!   librmw_zerodds.so   ◀── diese Crate
//!       │  ZeroDDS C-API (zerodds.h)
//!       ▼
//!   libzerodds.so       (crates/zerodds-c-api)
//! ```
//!
//! REP-2007-Mapping kommt aus `zerodds-ros2-rmw` (Topic-Mangling, QoS-
//! Profile, Identifier-Constraints). Der untergelegte Wire-Pfad ist
//! die `zerodds-c-api`-Runtime — `zerodds_runtime_create`,
//! `zerodds_writer_create`, `zerodds_reader_take`.
//!
//! # aktuelle Distros-Scope
//!
//! Diese Phase liefert das Crate-Skeleton mit den kritischsten
//! Entry-Points implementiert + Stubs fuer den Rest. Stubs returnen
//! `RMW_RET_UNSUPPORTED` (=3) — die Library laedt damit als RMW-
//! Plugin (rclcpp-Discovery), aber Funktionen wie Services,
//! Actions, Events sind noch nicht aktiv.
//!
//! Implementiert (volle Pub-Sub-Pipeline):
//! * `rmw_zerodds_get_implementation_identifier`
//! * `rmw_zerodds_create_init_options` / `_fini`
//! * `rmw_zerodds_init` / `_shutdown`
//! * `rmw_zerodds_create_node` / `_destroy_node`
//! * `rmw_zerodds_create_publisher` / `_destroy_publisher` / `_publish`
//! * `rmw_zerodds_create_subscription` / `_destroy_subscription` / `_take`
//!
//! Stubs (RMW_RET_UNSUPPORTED in aktuelle Distros):
//! * Services (`rmw_zerodds_create_client/service`)
//! * Events / Wait-Sets / Guard-Conditions
//! * Type-Support-Hashing (REP-2009)
//! * Loaning (`rmw_zerodds_borrow_loaned_message`)

#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]

extern crate alloc;

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::CStr;
use std::sync::Mutex;
use std::time::Duration;

use zerodds_ros2_rmw::ffi_api::RMW_IMPLEMENTATION_IDENTIFIER;
// `zerodds`-Crate (= `zerodds-c-api` per `[lib] name = "zerodds"`)
// wird unten via `zerodds::ZeroDdsRuntime` / `zerodds::zerodds_*` direkt
// genutzt. Der explizite `use zerodds as _;`-Marker entfaellt — die
// echten Pfade sind selbsterklaerend.

/// `rmw_ret_t`-Aliase als plain int. Spec REP-2007 §4 Codes.
#[allow(non_upper_case_globals)]
pub const RMW_RET_OK: i32 = 0;
#[allow(non_upper_case_globals)]
/// `RMW_RET_ERROR`.
pub const RMW_RET_ERROR: i32 = 1;
#[allow(non_upper_case_globals)]
/// `RMW_RET_TIMEOUT`.
pub const RMW_RET_TIMEOUT: i32 = 2;
#[allow(non_upper_case_globals)]
/// `RMW_RET_UNSUPPORTED`.
pub const RMW_RET_UNSUPPORTED: i32 = 3;
#[allow(non_upper_case_globals)]
/// `RMW_RET_BAD_ALLOC`.
pub const RMW_RET_BAD_ALLOC: i32 = 10;
#[allow(non_upper_case_globals)]
/// `RMW_RET_INVALID_ARGUMENT`.
pub const RMW_RET_INVALID_ARGUMENT: i32 = 11;
#[allow(non_upper_case_globals)]
/// `RMW_RET_INCORRECT_RMW_IMPLEMENTATION`.
pub const RMW_RET_INCORRECT_RMW_IMPLEMENTATION: i32 = 12;

// ============================================================================
// Implementation-Identifier
// ============================================================================

/// `rmw_get_implementation_identifier()` — REP-2007 §3.
/// Returnt einen statischen NUL-terminierten String "rmw_zerodds_cpp".
///
/// # Safety
/// Pointer ist `'static` und darf nicht freigegeben werden.
#[unsafe(no_mangle)]
pub extern "C" fn rmw_zerodds_get_implementation_identifier() -> *const c_char {
    static IDENT: &[u8] = b"rmw_zerodds_cpp\0";
    IDENT.as_ptr().cast()
}

/// `rmw_get_serialization_format()` — fixed `"cdr"` (XCDR1).
///
/// # Safety
/// Pointer ist `'static`.
#[unsafe(no_mangle)]
pub extern "C" fn rmw_zerodds_get_serialization_format() -> *const c_char {
    static FMT: &[u8] = b"cdr\0";
    FMT.as_ptr().cast()
}

// ============================================================================
// Init / Shutdown
// ============================================================================

/// Opaque-Handle: rmw_zerodds Context (1:1 zu einem Domain-Participant-
/// Init). Hardcoded Domain 0 in aktuelle Distros; spaeter aus init_options.
pub struct RmwZerodsContext {
    domain_id: u32,
    runtime: *mut c_void, // ZeroDdsRuntime aus zerodds.h
}

impl Drop for RmwZerodsContext {
    fn drop(&mut self) {
        if !self.runtime.is_null() {
            // SAFETY: runtime kommt aus zerodds_runtime_create.
            unsafe {
                zerodds::zerodds_runtime_destroy(self.runtime as *mut zerodds::ZeroDdsRuntime);
            }
            self.runtime = ptr::null_mut();
        }
    }
}

/// `rmw_init(domain_id) -> *mut RmwZerodsContext`.
/// In aktuelle Distros nimmt sie einen einzelnen Domain-Id-Parameter.
///
/// # Safety
/// Caller muss `rmw_zerodds_shutdown` mit dem zurueckgegebenen Handle
/// aufrufen, sonst Leak.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_init(domain_id: u32) -> *mut RmwZerodsContext {
    // SAFETY: zerodds_runtime_create ist NULL-tolerant + heap-allokiert.
    let rt = unsafe { zerodds::zerodds_runtime_create(domain_id) };
    if rt.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(RmwZerodsContext {
        domain_id,
        runtime: rt as *mut c_void,
    }))
}

/// `rmw_shutdown(*mut Context)` — gibt die Runtime frei. NULL-safe.
///
/// # Safety
/// `ctx` muss aus `rmw_zerodds_init` stammen oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_shutdown(ctx: *mut RmwZerodsContext) -> i32 {
    if ctx.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Rueckgabe + Drop kuemmert sich um die Runtime.
    let _ = unsafe { Box::from_raw(ctx) };
    RMW_RET_OK
}

// ============================================================================
// Node
// ============================================================================

/// Opaque Node-Handle.
pub struct RmwZerodsNode {
    /// Owning context — wir borrowen Runtime durch ihn.
    ctx: *mut RmwZerodsContext,
    /// Logische Node-Identitaet (Name + Namespace + Domain).
    pub identity: zerodds_ros2_rmw::ffi_api::RmwNode,
}

/// `rmw_create_node(ctx, name, namespace_)` — REP-2007 §5.1.
///
/// # Safety
/// `ctx` muss live; `name`/`namespace_` NUL-terminiert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_node(
    ctx: *mut RmwZerodsContext,
    name: *const c_char,
    namespace_: *const c_char,
) -> *mut RmwZerodsNode {
    if ctx.is_null() || name.is_null() || namespace_.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: ctx NULL-checked.
    let cref = unsafe { &*ctx };
    // SAFETY: NUL-terminierte C-Strings vom Caller-Kontrakt verlangt.
    let n = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NUL-terminierter C-String, NULL-checked oben.
    let ns = match unsafe { CStr::from_ptr(namespace_) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(RmwZerodsNode {
        ctx,
        identity: zerodds_ros2_rmw::ffi_api::RmwNode {
            implementation_identifier: RMW_IMPLEMENTATION_IDENTIFIER.into(),
            name: n,
            namespace: ns,
            domain_id: cref.domain_id,
        },
    }))
}

/// `rmw_destroy_node(*mut Node)`. NULL-safe.
///
/// # Safety
/// `node` muss aus `rmw_zerodds_create_node` stammen oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_node(node: *mut RmwZerodsNode) -> i32 {
    if node.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box from_raw mit eigenem Pointer.
    let _ = unsafe { Box::from_raw(node) };
    RMW_RET_OK
}

// ============================================================================
// Publisher / Subscription
// ============================================================================

/// Opaque Publisher-Handle. Wrapt einen ZeroDDS-Writer.
pub struct RmwZerodsPublisher {
    inner: Mutex<*mut zerodds::ZeroDdsWriter>,
    /// Ros-logischer Topic-Name (vor Mangling).
    pub ros_topic: alloc::string::String,
    /// DDS-Topic-Name (nach `rt/`-Prefix).
    pub dds_topic: alloc::string::String,
    /// Type-Name aus der TypeSupport-Schicht.
    pub type_name: alloc::string::String,
}

/// Opaque Subscription-Handle. Wrapt einen ZeroDDS-Reader.
pub struct RmwZerodsSubscription {
    inner: Mutex<*mut zerodds::ZeroDdsReader>,
    /// Ros-logischer Topic-Name (vor Mangling).
    pub ros_topic: alloc::string::String,
    /// DDS-Topic-Name (nach `rt/`-Prefix).
    pub dds_topic: alloc::string::String,
    /// Type-Name aus der TypeSupport-Schicht.
    pub type_name: alloc::string::String,
}

/// `rmw_create_publisher(node, type_name, topic_name, reliable)`.
///
/// # Safety
/// Pointer-Validitaet wie immer; Strings NUL-terminiert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_publisher(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    reliable: c_int,
) -> *mut RmwZerodsPublisher {
    if node.is_null() || type_name.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked oben.
    let n = unsafe { &*node };
    // SAFETY: n.ctx muss durch rmw_zerodds_init initialisiert sein
    // (caller-Kontrakt der pub unsafe fn).
    let ctx = unsafe { &*n.ctx };
    // SAFETY: topic_name NULL-checked oben; caller-Kontrakt verlangt
    // NUL-terminierten C-String.
    let topic_ros = match unsafe { CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: type_name NULL-checked oben; caller-Kontrakt: NUL-
    // terminierter C-String.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // ROS-2-Topic-Mangling: rt/<topic> fuer reguläre Topics.
    let topic_dds = zerodds_ros2_rmw::topic_mangling::mangle_topic_name(
        &topic_ros,
        zerodds_ros2_rmw::topic_mangling::RosKind::Topic,
    )
    .unwrap_or_else(|_| topic_ros.clone());
    let dds_topic_c = match std::ffi::CString::new(topic_dds.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: writer_create NULL-tolerant.
    let writer = unsafe {
        zerodds::zerodds_writer_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            dds_topic_c.as_ptr(),
            typ_c.as_ptr(),
            reliable,
        )
    };
    if writer.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(RmwZerodsPublisher {
        inner: Mutex::new(writer),
        ros_topic: topic_ros,
        dds_topic: topic_dds,
        type_name: typ,
    }))
}

/// `rmw_destroy_publisher(*mut Publisher)`.
///
/// # Safety
/// `pub_` muss aus `rmw_zerodds_create_publisher` oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_publisher(pub_: *mut RmwZerodsPublisher) -> i32 {
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let p = unsafe { Box::from_raw(pub_) };
    if let Ok(g) = p.inner.lock() {
        // SAFETY: writer kommt aus zerodds_writer_create + Box-owns.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_publish(pub, payload, len)` — schreibt CDR-encoded bytes.
///
/// # Safety
/// `pub_` valid; `payload` mit `len` byte lebt waehrend des Calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publish(
    pub_: *mut RmwZerodsPublisher,
    payload: *const u8,
    len: usize,
) -> i32 {
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: payload + len kontract via FFI; writer aus create.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, payload, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// `rmw_create_subscription(node, type, topic, reliable)`.
///
/// # Safety
/// Wie create_publisher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_subscription(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    reliable: c_int,
) -> *mut RmwZerodsSubscription {
    if node.is_null() || type_name.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked oben.
    let n = unsafe { &*node };
    // SAFETY: n.ctx muss durch rmw_zerodds_init initialisiert sein.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: topic_name NULL-checked oben; caller-Kontrakt: NUL-
    // terminierter C-String.
    let topic_ros = match unsafe { CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: type_name NULL-checked oben; caller-Kontrakt: NUL-
    // terminierter C-String.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    let topic_dds = zerodds_ros2_rmw::topic_mangling::mangle_topic_name(
        &topic_ros,
        zerodds_ros2_rmw::topic_mangling::RosKind::Topic,
    )
    .unwrap_or_else(|_| topic_ros.clone());
    let dds_topic_c = match std::ffi::CString::new(topic_dds.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: reader_create NULL-tolerant.
    let reader = unsafe {
        zerodds::zerodds_reader_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            dds_topic_c.as_ptr(),
            typ_c.as_ptr(),
            reliable,
        )
    };
    if reader.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(RmwZerodsSubscription {
        inner: Mutex::new(reader),
        ros_topic: topic_ros,
        dds_topic: topic_dds,
        type_name: typ,
    }))
}

/// `rmw_destroy_subscription(*mut Subscription)`.
///
/// # Safety
/// `sub` muss aus `rmw_zerodds_create_subscription` oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_subscription(sub: *mut RmwZerodsSubscription) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let s = unsafe { Box::from_raw(sub) };
    if let Ok(g) = s.inner.lock() {
        // SAFETY: reader kommt aus zerodds_reader_create.
        unsafe { zerodds::zerodds_reader_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_take(sub, *mut buf, *mut len)` — versucht ein Sample zu lesen.
/// Caller MUSS `rmw_zerodds_buffer_free(buf, len)` aufrufen.
///
/// # Safety
/// Pointer-Validitaet; out_buf/out_len NULL-checked unten.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take(
    sub: *mut RmwZerodsSubscription,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if sub.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*sub };
    let reader = match s.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: out_buf/out_len NULL-checked; reader aus create.
    let rc = unsafe { zerodds::zerodds_reader_take(reader, out_buf, out_len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// `rmw_zerodds_buffer_free` — dual zu zerodds_buffer_free, fuer
/// CDR-Bytes aus rmw_zerodds_take.
///
/// # Safety
/// Wie zerodds_buffer_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_buffer_free(buf: *mut u8, len: usize) {
    // SAFETY: Pass-through.
    unsafe { zerodds::zerodds_buffer_free(buf, len) }
}

// ============================================================================
// Phase-B: Service-Layer + Wait-Sets + Loaning + REP-2009 Type-Hash
// ============================================================================

/// Opaque-Handle: rmw_zerodds Client (request-Pub + reply-Sub auf
/// `<service>_Request` / `<service>_Reply`).
pub struct RmwZerodsClient {
    /// Underlying writer auf `<service>_Request`.
    request_writer: Mutex<*mut zerodds::ZeroDdsWriter>,
    /// Underlying reader auf `<service>_Reply`.
    reply_reader: Mutex<*mut zerodds::ZeroDdsReader>,
    /// Service-Name (vor Topic-Mangling).
    pub service_name: alloc::string::String,
}

/// Opaque-Handle: rmw_zerodds Service (request-Sub + reply-Pub).
pub struct RmwZerodsService {
    /// Underlying reader auf `<service>_Request`.
    request_reader: Mutex<*mut zerodds::ZeroDdsReader>,
    /// Underlying writer auf `<service>_Reply`.
    reply_writer: Mutex<*mut zerodds::ZeroDdsWriter>,
    /// Service-Name (vor Topic-Mangling).
    pub service_name: alloc::string::String,
}

/// `rmw_create_client(node, service, type_name)`.
///
/// # Safety
/// Pointer-Validitaet wie Publisher/Subscription-create.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_client(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    type_name: *const c_char,
) -> *mut RmwZerodsClient {
    if node.is_null() || service_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked.
    let n = unsafe { &*node };
    // SAFETY: ctx-Pointer aus RmwZerodsNode-Construct + lebt
    // solange node lebt.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: NUL-terminierter C-String, NULL-checked oben.
    let service = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NUL-terminierter C-String, NULL-checked oben.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    if zerodds_rpc::topic_naming::validate_service_name(&service).is_err() {
        return ptr::null_mut();
    }
    let req_topic = match zerodds_rpc::topic_naming::request_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic = match zerodds_rpc::topic_naming::reply_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let req_topic_c = match std::ffi::CString::new(req_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic_c = match std::ffi::CString::new(rep_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NULL-tolerante FFI-Calls; Cleanup im Fehler-Pfad.
    let writer = unsafe {
        zerodds::zerodds_writer_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            req_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1, // services sind reliable
        )
    };
    // SAFETY: NULL-tolerante FFI; Cleanup-Pfad below.
    let reader = unsafe {
        zerodds::zerodds_reader_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            rep_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1,
        )
    };
    if writer.is_null() || reader.is_null() {
        if !writer.is_null() {
            // SAFETY: writer aus writer_create.
            unsafe { zerodds::zerodds_writer_destroy(writer) };
        }
        if !reader.is_null() {
            // SAFETY: reader aus reader_create.
            unsafe { zerodds::zerodds_reader_destroy(reader) };
        }
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(RmwZerodsClient {
        request_writer: Mutex::new(writer),
        reply_reader: Mutex::new(reader),
        service_name: service,
    }))
}

/// `rmw_destroy_client(*mut Client)`.
///
/// # Safety
/// `client` aus `rmw_zerodds_create_client` oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_client(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer aus oben.
    let c = unsafe { Box::from_raw(client) };
    if let Ok(g) = c.request_writer.lock() {
        // SAFETY: writer aus create_client.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    if let Ok(g) = c.reply_reader.lock() {
        // SAFETY: reader aus create_client.
        unsafe { zerodds::zerodds_reader_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_send_request(client, payload, len)`.
///
/// # Safety
/// `client` valid; payload + len lebt waehrend des Calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_send_request(
    client: *mut RmwZerodsClient,
    payload: *const u8,
    len: usize,
) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*client };
    let writer = match c.request_writer.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer aus create_client.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, payload, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// `rmw_take_response(client, *mut buf, *mut len)`.
///
/// # Safety
/// Wie rmw_zerodds_take.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_response(
    client: *mut RmwZerodsClient,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if client.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*client };
    let reader = match c.reply_reader.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: reader aus create_client.
    let rc = unsafe { zerodds::zerodds_reader_take(reader, out_buf, out_len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// `rmw_create_service(node, service, type_name)` — Server-side.
///
/// # Safety
/// Wie rmw_zerodds_create_client.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_service(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    type_name: *const c_char,
) -> *mut RmwZerodsService {
    if node.is_null() || service_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked.
    let n = unsafe { &*node };
    // SAFETY: ctx-Pointer aus RmwZerodsNode-Construct.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: NUL-terminierter C-String.
    let service = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NUL-terminierter C-String.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    if zerodds_rpc::topic_naming::validate_service_name(&service).is_err() {
        return ptr::null_mut();
    }
    let req_topic = match zerodds_rpc::topic_naming::request_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic = match zerodds_rpc::topic_naming::reply_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let req_topic_c = match std::ffi::CString::new(req_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic_c = match std::ffi::CString::new(rep_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NULL-tolerante FFI-Calls.
    let reader = unsafe {
        zerodds::zerodds_reader_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            req_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1,
        )
    };
    // SAFETY: NULL-tolerante FFI-Calls.
    let writer = unsafe {
        zerodds::zerodds_writer_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            rep_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1,
        )
    };
    if writer.is_null() || reader.is_null() {
        if !writer.is_null() {
            // SAFETY: writer aus writer_create.
            unsafe { zerodds::zerodds_writer_destroy(writer) };
        }
        if !reader.is_null() {
            // SAFETY: reader aus reader_create.
            unsafe { zerodds::zerodds_reader_destroy(reader) };
        }
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(RmwZerodsService {
        request_reader: Mutex::new(reader),
        reply_writer: Mutex::new(writer),
        service_name: service,
    }))
}

/// `rmw_destroy_service(*mut Service)`.
///
/// # Safety
/// `service` aus `rmw_zerodds_create_service` oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_service(service: *mut RmwZerodsService) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let s = unsafe { Box::from_raw(service) };
    if let Ok(g) = s.request_reader.lock() {
        // SAFETY: reader aus create_service.
        unsafe { zerodds::zerodds_reader_destroy(*g) };
    }
    if let Ok(g) = s.reply_writer.lock() {
        // SAFETY: writer aus create_service.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_take_request(service, *mut buf, *mut len)`.
///
/// # Safety
/// Pointer-Validitaet wie rmw_zerodds_take.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_request(
    service: *mut RmwZerodsService,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if service.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*service };
    let reader = match s.request_reader.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: reader aus create_service.
    let rc = unsafe { zerodds::zerodds_reader_take(reader, out_buf, out_len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// `rmw_send_response(service, payload, len)`.
///
/// # Safety
/// Wie rmw_zerodds_publish.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_send_response(
    service: *mut RmwZerodsService,
    payload: *const u8,
    len: usize,
) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*service };
    let writer = match s.reply_writer.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer aus create_service.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, payload, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

// ----- Wait-Set (Phase-B): poll-based ---------------------------------------

/// Wait-Set-Handle. Phase-B implementiert: Poll-basiert, nicht
/// edge-triggered. Caller fuegt Subscriptions hinzu, ruft `wait`
/// und bekommt zurueck welche Indices Daten bereit haben.
pub struct RmwZerodsWaitSet {
    /// Pointer auf Subscriptions die wir pollen.
    subscriptions: Mutex<Vec<*mut RmwZerodsSubscription>>,
}

/// `rmw_create_wait_set()`.
///
/// # Safety
/// Result-Pointer ist heap-allokiert; Caller muss
/// `rmw_zerodds_destroy_wait_set` aufrufen.
#[unsafe(no_mangle)]
pub extern "C" fn rmw_zerodds_create_wait_set() -> *mut RmwZerodsWaitSet {
    Box::into_raw(Box::new(RmwZerodsWaitSet {
        subscriptions: Mutex::new(Vec::new()),
    }))
}

/// `rmw_destroy_wait_set(*mut WaitSet)`.
///
/// # Safety
/// `ws` aus `rmw_zerodds_create_wait_set` oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_wait_set(ws: *mut RmwZerodsWaitSet) -> i32 {
    if ws.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let _ = unsafe { Box::from_raw(ws) };
    RMW_RET_OK
}

/// Fuegt eine Subscription zum Wait-Set hinzu.
///
/// # Safety
/// `ws` + `sub` muessen valid sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_wait_set_add_subscription(
    ws: *mut RmwZerodsWaitSet,
    sub: *mut RmwZerodsSubscription,
) -> i32 {
    if ws.is_null() || sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let w = unsafe { &*ws };
    if let Ok(mut g) = w.subscriptions.lock() {
        g.push(sub);
        RMW_RET_OK
    } else {
        RMW_RET_ERROR
    }
}

/// Helper-Wrapper, damit der `unsafe`-Block einen sauber adjacenten
/// `// SAFETY:`-Kommentar hat (zerodds-lint-Anforderung).
fn call_wait_matched(reader: *mut zerodds::ZeroDdsReader) -> i32 {
    // SAFETY: reader aus zerodds_reader_create.
    unsafe { zerodds::zerodds_reader_wait_for_matched(reader, 1, 0) }
}

/// Phase-C Wait: pollt alle Subscriptions im Wait-Set mit adaptivem
/// Backoff. Edge-Trigger-Emulation:
/// * Erste 100 µs spin_loop_hint (sub-µs Latenz)
/// * 100 µs..10 ms: 10 µs Sleep
/// * 10..100 ms: 100 µs Sleep
/// * Danach: 1 ms Sleep
///
/// Echter Edge-Trigger ueber Condvar/Channel braucht eine Notify-
/// Bruecke aus dem Reader-Receive-Thread (Phase-D).
///
/// # Safety
/// `ws` muss valid sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_wait(ws: *mut RmwZerodsWaitSet, timeout_ms: u64) -> i32 {
    if ws.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let w = unsafe { &*ws };
    let start = std::time::Instant::now();
    let deadline = start + Duration::from_millis(timeout_ms);
    loop {
        let ready = {
            let Ok(g) = w.subscriptions.lock() else {
                return RMW_RET_ERROR;
            };
            g.iter()
                .filter(|s| {
                    if s.is_null() {
                        return false;
                    }
                    // SAFETY: NULL-checked oben.
                    let sref: &RmwZerodsSubscription = unsafe { &***s };
                    if let Ok(reader_g) = sref.inner.lock() {
                        // SAFETY: reader aus create_subscription.
                        let rc = call_wait_matched(*reader_g);
                        rc == 0
                    } else {
                        false
                    }
                })
                .count()
        };
        if ready > 0 {
            return RMW_RET_OK;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return RMW_RET_TIMEOUT;
        }
        let elapsed = now - start;
        if elapsed < Duration::from_micros(100) {
            for _ in 0..16 {
                std::hint::spin_loop();
            }
        } else if elapsed < Duration::from_millis(10) {
            std::thread::sleep(Duration::from_micros(10));
        } else if elapsed < Duration::from_millis(100) {
            std::thread::sleep(Duration::from_micros(100));
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

// ----- Loaning (Phase-C: malloc-backed; Phase-D: SHM-backed) ---------------

/// `rmw_borrow_loaned_message(publisher, len, *out_ptr, *out_len)` —
/// reserviert einen Buffer beim Writer fuer Zero-Copy-Publish.
/// Phase-C: malloc-backed (kein echter Zero-Copy, aber Code-Pfad
/// stabil). Phase-D wechselt auf SHM-Buffer-Pool wenn der
/// Writer auf einem SHM-Transport sitzt.
///
/// # Safety
/// `pub_` muss aus rmw_zerodds_create_publisher stammen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_borrow_loaned_message(
    pub_: *mut RmwZerodsPublisher,
    len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if pub_.is_null() || out_ptr.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer aus create_publisher.
    let rc = unsafe { zerodds::zerodds_writer_loan_message(writer, len, out_ptr, out_len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Commit-Pfad fuer einen Loan — schreibt den Buffer als Sample und
/// gibt ihn frei.
///
/// # Safety
/// `pub_` valid; `ptr`/`len` aus rmw_zerodds_borrow_loaned_message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publish_loaned_message(
    pub_: *mut RmwZerodsPublisher,
    ptr: *mut u8,
    len: usize,
) -> i32 {
    if pub_.is_null() || ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer + ptr aus borrow.
    let rc = unsafe { zerodds::zerodds_writer_commit_loan(writer, ptr, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Verwirft einen Loan ohne ihn zu publishen.
///
/// # Safety
/// Wie rmw_zerodds_publish_loaned_message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_return_loaned_message(
    pub_: *mut RmwZerodsPublisher,
    ptr: *mut u8,
    len: usize,
) -> i32 {
    if pub_.is_null() || ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer + ptr aus borrow.
    let rc = unsafe { zerodds::zerodds_writer_discard_loan(writer, ptr, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

// ----- REP-2009 Type-Hash -------------------------------------------------

/// REP-2009 Type-Hash: SHA-256 ueber den IDL-Type-String.
/// Liefert exakt 32 byte in `out_hash` (muss 32-byte-Buffer sein).
///
/// # Safety
/// `type_str` NUL-terminierter C-String; `out_hash` zeigt auf einen
/// 32-byte-Buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_compute_type_hash(
    type_str: *const c_char,
    out_hash: *mut u8,
) -> i32 {
    if type_str.is_null() || out_hash.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NUL-terminierter C-String.
    let s = match unsafe { CStr::from_ptr(type_str) }.to_bytes_with_nul() {
        b if b.len() > 1 => &b[..b.len() - 1],
        _ => return RMW_RET_INVALID_ARGUMENT,
    };
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s);
    let result = h.finalize();
    // SAFETY: out_hash NULL-checked; Caller-Kontrakt: 32-byte-Buffer.
    unsafe {
        core::ptr::copy_nonoverlapping(result.as_ptr(), out_hash, 32);
    }
    RMW_RET_OK
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;

    #[test]
    fn rmw_codes_match_rep_2007() {
        assert_eq!(RMW_RET_OK, 0);
        assert_eq!(RMW_RET_ERROR, 1);
        assert_eq!(RMW_RET_TIMEOUT, 2);
        assert_eq!(RMW_RET_UNSUPPORTED, 3);
        assert_eq!(RMW_RET_BAD_ALLOC, 10);
        assert_eq!(RMW_RET_INVALID_ARGUMENT, 11);
        assert_eq!(RMW_RET_INCORRECT_RMW_IMPLEMENTATION, 12);
    }

    #[test]
    fn impl_identifier_is_static_cstring() {
        let p = rmw_zerodds_get_implementation_identifier();
        assert!(!p.is_null());
        // SAFETY: static byte-array, CStr-checked.
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
        assert_eq!(s, "rmw_zerodds_cpp");
    }

    #[test]
    fn serialization_format_is_cdr() {
        let p = rmw_zerodds_get_serialization_format();
        // SAFETY: static array.
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
        assert_eq!(s, "cdr");
    }

    #[test]
    fn shutdown_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerantes Verhalten ist Teil des Kontrakts.
        let r = unsafe { rmw_zerodds_shutdown(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_node_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_destroy_node(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn loaning_null_args_rejected() {
        let mut p: *mut u8 = ptr::null_mut();
        let mut l: usize = 0;
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_borrow_loaned_message(ptr::null_mut(), 64, &mut p, &mut l) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn return_loan_null_args_rejected() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_return_loaned_message(ptr::null_mut(), ptr::null_mut(), 0) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_client_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_destroy_client(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_service_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_destroy_service(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn wait_set_create_destroy_smoke() {
        let ws = rmw_zerodds_create_wait_set();
        assert!(!ws.is_null());
        // SAFETY: ws aus create_wait_set.
        let r = unsafe { rmw_zerodds_destroy_wait_set(ws) };
        assert_eq!(r, RMW_RET_OK);
    }

    #[test]
    fn wait_returns_invalid_for_null() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_wait(ptr::null_mut(), 100) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn type_hash_sha256_is_deterministic() {
        let s = std::ffi::CString::new("std_msgs/msg/String").unwrap();
        let mut h1 = [0u8; 32];
        let mut h2 = [0u8; 32];
        // SAFETY: NUL-Strings + 32-byte buffers.
        let rc1 = unsafe { rmw_zerodds_compute_type_hash(s.as_ptr(), h1.as_mut_ptr()) };
        // SAFETY: NUL-Strings + 32-byte buffers (gleicher Kontrakt).
        let rc2 = unsafe { rmw_zerodds_compute_type_hash(s.as_ptr(), h2.as_mut_ptr()) };
        assert_eq!(rc1, RMW_RET_OK);
        assert_eq!(rc2, RMW_RET_OK);
        assert_eq!(h1, h2);
        // Erste Bytes != 0 (Hash kein-leer).
        assert_ne!(h1, [0u8; 32]);
    }

    #[test]
    fn type_hash_differs_for_different_types() {
        let a = std::ffi::CString::new("std_msgs/msg/String").unwrap();
        let b = std::ffi::CString::new("std_msgs/msg/Int32").unwrap();
        let mut ha = [0u8; 32];
        let mut hb = [0u8; 32];
        // SAFETY: NUL-Strings + Buffer.
        let _ = unsafe { rmw_zerodds_compute_type_hash(a.as_ptr(), ha.as_mut_ptr()) };
        // SAFETY: NUL-Strings + Buffer (gleicher Kontrakt).
        let _ = unsafe { rmw_zerodds_compute_type_hash(b.as_ptr(), hb.as_mut_ptr()) };
        assert_ne!(ha, hb);
    }

    #[test]
    fn type_hash_null_args_rejected() {
        let mut h = [0u8; 32];
        // SAFETY: ein Null-Arg pruefen.
        let r1 = unsafe { rmw_zerodds_compute_type_hash(ptr::null(), h.as_mut_ptr()) };
        assert_eq!(r1, RMW_RET_INVALID_ARGUMENT);
        let s = std::ffi::CString::new("x").unwrap();
        // SAFETY: out_hash NULL.
        let r2 = unsafe { rmw_zerodds_compute_type_hash(s.as_ptr(), ptr::null_mut()) };
        assert_eq!(r2, RMW_RET_INVALID_ARGUMENT);
    }
}
