// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ZeroDDS C-FFI — Cross-Language-Hub.
//!
//! Crate `zerodds-c-api`. Safety classification: **STANDARD**.
//!
//! Diese Crate exportiert eine `extern "C"` Schicht ueber die ZeroDDS-
//! DCPS-Runtime, sodass Nicht-Rust-Sprachen (C++, C#, TypeScript, C)
//! ueber einen einheitlichen Binary-Interface auf ZeroDDS zugreifen.
//!
//! Die generierte `include/zerodds.h` (via cbindgen build-script) ist
//! das Vertragsdokument fuer alle Konsumenten.
//!
//! # Type-Modell — bewusst untyped
//!
//! Das C-FFI ist **byte-orientiert**: ein `Topic` traegt einen
//! `topic_name` + `type_name`-String, ein `write` nimmt einen
//! `(*const u8, len)`-Buffer mit der bereits CDR-encodeten Sample-Bytes,
//! ein `take` liefert die rohen Wire-Bytes. Die CDR-Encode/Decode-Logik
//! lebt in den Sprach-Bindings (idl-cpp emittiert C++-Encoder, idl-csharp
//! C#-Encoder, etc.) — das C-FFI ist neutral.
//!
//! Vorteile:
//! * Keine Generic-Type-Akrobatik durch FFI.
//! * Wire-Drift-Tests sind transparent: jeder Bytes-Buffer geht 1:1.
//! * Apex.AI-Plugin und ROS-2-RMW koennen ihre eigenen Marshaling-Pfade
//!   beibehalten.
//!
//! # Handle-Modell
//!
//! Alle Objekte sind als opaque-Pointer exponiert. Caller muessen
//! `*_destroy()` paaren. Memory-Ownership ist explizit:
//! * `zerodds_runtime_create()` -> Caller besitzt; `zerodds_runtime_destroy()`.
//! * `zerodds_writer_create()` -> Caller besitzt; muss vor Runtime-destroy.
//! * `zerodds_reader_take()` -> die `out_buf`-Bytes muessen mit
//!   `zerodds_buffer_free()` freigegeben werden.
//!
//! # Was NICHT durch das C-FFI geht
//!
//! * Java + Python — eigene Bruecken (jni-rs / pyo3). Direkter
//!   Rust↔Sprache-Pfad ohne C-Zwischenschicht.
//! * QoS-Builder mit komplexen Default-Logiken — vereinfachte Knobs
//!   im FFI; vollstaendige QoS-Kontrolle nur ueber Rust-API.

#![warn(missing_docs)]
// FFI-Modul braucht `unsafe`-Code, daher kein `#![deny(unsafe_code)]`.
// Stattdessen pro `unsafe`-Block ein SAFETY-Kommentar.
#![allow(clippy::missing_safety_doc)]
// FFI-Boundary: Pointer-Args sind by design Caller-Pflicht; interne
// Helper-Funktionen die `*mut`-Args nehmen sind als `pub fn` ausgelegt
// (nicht `unsafe fn`) damit ihre Aufrufer aus FFI-Funktionen sie ohne
// Re-Wrapping in `unsafe`-Bloecke nutzen koennen — der `unsafe`-Block
// liegt am FFI-extern-Funktions-Niveau.
#![allow(clippy::not_unsafe_ptr_arg_deref)]
// Listener-Callback-Pfad nutzt `cb.unwrap()` nach `cb.is_some()`-Check.
#![allow(clippy::unwrap_used)]
// Manche QoS-Field-Assignments folgen Builder-Pattern statt struct-init.
#![allow(clippy::field_reassign_with_default)]
// Lifetime-elision in `unsafe fn cstr_to_str<'a>` ist an der FFI-Kante
// notwendig fuer borrow-Lifetimes des Caller-Strings.
#![allow(clippy::needless_lifetimes)]
// Pub-Fields in opaque-FFI-Wrapper-Strukturen sind dokumentiert auf
// Struktur-Ebene; pro-Field-Doc ist redundant.
#![allow(missing_docs)]
// QoS-Policies wie DeadlineQosPolicy implementieren Copy, sind aber in
// generischen `Clone`-basierten Foreach-Patterns konsistenter mit
// `.clone()` zu lesen.
#![allow(clippy::clone_on_copy)]
// `arr.lock().map(|g| g.clone()).unwrap_or_default()` ist klarer als
// `.map_or_else(Default::default, |g| g.clone())`.
#![allow(clippy::map_unwrap_or)]

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
pub mod subscriber_ffi;
pub mod topic_ffi;

/// XCDR2 TypeSupport-FFI — implementiert `zerodds-xcdr2-c-1.0` Vendor-Spec.
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
    bytes[0..4].copy_from_slice(&pid.to_le_bytes());
    bytes[4..12].copy_from_slice(&(t.wrapping_add(u64::from(c))).to_le_bytes());
    GuidPrefix::from_bytes(bytes)
}

// ============================================================================
// Status-Codes
// ============================================================================

/// FFI-Statuscodes. 0 = OK, negative Werte = Fehler.
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsStatus {
    /// Operation erfolgreich.
    Ok = 0,
    /// Generischer Fehler (Details in `zerodds_last_error()`).
    Error = -1,
    /// NULL-Pointer wo nicht erlaubt.
    BadHandle = -2,
    /// Ungültiger UTF-8-String.
    InvalidUtf8 = -3,
    /// Operation blockierte und Timeout lief ab.
    Timeout = -4,
    /// Pre-Condition nicht erfüllt (z.B. Reader/Writer schon zerstört).
    PreconditionNotMet = -5,
    /// Ungueltiger Parameter-Wert (z.B. len=0, negative size).
    BadParameter = -6,
    /// Operation lieferte keine Daten (z.B. take() auf leerem Reader).
    NoData = -7,
    /// Resource-Limit erreicht.
    OutOfResources = -8,
    /// Entity nicht enabled.
    NotEnabled = -9,
    /// QoS-Policy ist immutable.
    ImmutablePolicy = -10,
    /// QoS-Policies inkonsistent.
    InconsistentPolicy = -11,
    /// Entity bereits geloescht.
    AlreadyDeleted = -12,
    /// Operation nicht unterstuetzt (Profile/Stretch-Goals).
    Unsupported = -13,
    /// Aufruf in inkompatiblem Kontext.
    IllegalOperation = -14,
}

// ============================================================================
// Opaque handles
// ============================================================================

/// Opaque Runtime-Handle. Hält die DcpsRuntime + spawned threads.
pub struct ZeroDdsRuntime {
    rt: Arc<DcpsRuntime>,
    /// Track Spawned Worker-Thread(s) für sauberes Shutdown.
    _shutdown: (),
}

/// Opaque Writer-Handle.
pub struct ZeroDdsWriter {
    rt: Arc<DcpsRuntime>,
    eid: EntityId,
}

/// Opaque Reader-Handle.
pub struct ZeroDdsReader {
    rt: Arc<DcpsRuntime>,
    eid: EntityId,
    rx: Mutex<mpsc::Receiver<zerodds_dcps::runtime::UserSample>>,
}

// ============================================================================
// Lifecycle — Runtime
// ============================================================================

/// Erzeugt eine neue ZeroDDS-Runtime auf der gegebenen Domain-ID.
///
/// # Safety
/// Der Rückgabewert ist ein Heap-allokierter Pointer; der Aufrufer muss
/// ihn mit `zerodds_runtime_destroy` freigeben.
///
/// Liefert `NULL` bei Fehler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create(domain_id: u32) -> *mut ZeroDdsRuntime {
    let cfg = RuntimeConfig::default();
    let rt = match DcpsRuntime::start(domain_id as i32, random_guid_prefix(), cfg) {
        Ok(r) => r,
        Err(_) => return ptr::null_mut(),
    };
    // `DcpsRuntime::start` liefert bereits ein `Arc<DcpsRuntime>` zurueck.
    let handle = Box::new(ZeroDdsRuntime { rt, _shutdown: () });
    Box::into_raw(handle)
}

/// Zerstört eine Runtime. NULL-safe.
///
/// # Safety
/// `runtime` muss aus `zerodds_runtime_create` stammen oder NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_destroy(runtime: *mut ZeroDdsRuntime) {
    if runtime.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — runtime aus zerodds_runtime_create (Box::into_raw).
    let _ = unsafe { Box::from_raw(runtime) };
}

/// Wartet bis SPDP mindestens `min_count` Remote-Participants entdeckt
/// hat. Returnt 0 (Ok) bei Erfolg, -4 (Timeout) wenn die Zahl in
/// `timeout_ms` nicht erreicht wird.
///
/// **Optional, nicht zwingend.** SEDP ist reliable + behaelt History,
/// also wird ein zu frueh registrierter Endpoint sich nach 600-720 ms
/// (empirisch llvm-Linux) selbst heilen sobald SPDP den Peer sieht
/// und der Heartbeat-Replay durchlaeuft. Dieser Helper ist nuetzlich
/// wenn man deterministisches Test-Setup will oder einen langen
/// Publish-Loop vermeiden moechte.
///
/// # Safety
/// `runtime` muss aus `zerodds_runtime_create` stammen oder NULL sein.
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

// ============================================================================
// Writer
// ============================================================================

/// Erzeugt einen DataWriter auf einem Topic. Topic + Type werden by-name
/// identifiziert (DDS-Spec §2.2.2).
///
/// # Safety
/// `runtime` muss valide sein. `topic_name` und `type_name` müssen
/// NUL-terminierte UTF-8-Strings sein.
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
    // beide Strings NUL-terminiert (Caller-Pledge).
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
        // F-TYPES-3: C-FFI ist byte-orientiert (kein typed Topic-Type),
        // also kein TypeIdentifier verfügbar.
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    };
    let eid = match rt_clone.register_user_writer(cfg) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(ZeroDdsWriter { rt: rt_clone, eid }))
}

/// Schreibt einen Sample. `payload` zeigt auf bereits-CDR-encodete Bytes.
///
/// # Safety
/// `writer` und `payload` muessen valide sein, `len` <= Buffergröße.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_write(
    writer: *mut ZeroDdsWriter,
    payload: *const u8,
    len: usize,
) -> c_int {
    if writer.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+payload NULL-checked above; payload[0..len]
    // valide wenn len > 0 (Caller-Pledge).
    let (rt, eid, bytes) = unsafe {
        let w = &*writer;
        let bytes = if len == 0 {
            Vec::new()
        } else {
            slice::from_raw_parts(payload, len).to_vec()
        };
        (w.rt.clone(), w.eid, bytes)
    };
    match rt.write_user_sample(eid, bytes) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Wartet bis mindestens `min_count` Subscriptions gematcht haben oder
/// Timeout abläuft. `timeout_ms` = 0 -> non-blocking poll.
///
/// # Safety
/// `writer` muss valide sein.
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

/// Sendet einen Lifecycle-Marker (Spec §9.6.3.9 PID_STATUS_INFO):
/// `dispose` setzt das DISPOSED-Bit, sodass Remote-Reader die Instanz
/// als NotAliveDisposed klassifizieren. Der Caller muss den 16-byte
/// Key-Hash der Instanz uebergeben (PLAIN_CDR2-BE-Encoding mit Zero-
/// Padding bzw. MD5 falls > 16 byte).
///
/// # Safety
/// `writer` und `key_hash` muessen valide sein; `key_hash` muss auf
/// genau 16 byte zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_dispose(
    writer: *mut ZeroDdsWriter,
    key_hash: *const u8,
) -> c_int {
    if writer.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+key_hash NULL-checked above; key_hash[0..16]
    // valide (Caller-Pledge).
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

/// Sendet einen UNREGISTER-Marker (Spec §2.2.2.4.2.7). Setzt nur das
/// UNREGISTERED-Bit (kein autodispose). Caller, der Spec-Default-
/// Verhalten will (autodispose=true), soll stattdessen
/// `zerodds_writer_unregister_with_dispose` nutzen.
///
/// # Safety
/// Wie [`zerodds_writer_dispose`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unregister(
    writer: *mut ZeroDdsWriter,
    key_hash: *const u8,
) -> c_int {
    if writer.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+key_hash NULL-checked above; key_hash[0..16]
    // valide (Caller-Pledge).
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

/// Sendet kombinierten DISPOSE+UNREGISTER-Marker (Spec §2.2.3.21 mit
/// `autodispose_unregistered_instances=true`). Reader sieht sowohl
/// NotAliveDisposed als auch NotAliveNoWriters.
///
/// # Safety
/// Wie [`zerodds_writer_dispose`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_unregister_with_dispose(
    writer: *mut ZeroDdsWriter,
    key_hash: *const u8,
) -> c_int {
    if writer.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+key_hash NULL-checked above; key_hash[0..16]
    // valide (Caller-Pledge).
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

/// Zerstört einen Writer. NULL-safe.
///
/// # Safety
/// `writer` muss aus `zerodds_writer_create` stammen oder NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_destroy(writer: *mut ZeroDdsWriter) {
    if writer.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — writer aus zerodds_writer_create (Box::into_raw).
    let _ = unsafe { Box::from_raw(writer) };
}

// ============================================================================
// Reader
// ============================================================================

/// Erzeugt einen DataReader auf einem Topic.
///
/// # Safety
/// Wie `zerodds_writer_create`.
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
    // beide Strings NUL-terminiert (Caller-Pledge).
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
        // F-TYPES-3: C-FFI ist byte-orientiert.
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

/// Versucht einen Sample zu lesen.
/// * Bei Erfolg: schreibt allocierten Buffer in `*out_buf`, dessen
///   Länge in `*out_len`. Caller MUSS `zerodds_buffer_free(*out_buf)`.
/// * Bei keinem Sample: `*out_buf = NULL`, `*out_len = 0`, return Ok.
/// * Bei Fehler: negativer Statuscode.
///
/// # Safety
/// Pointers müssen valide sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take(
    reader: *mut ZeroDdsReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
) -> c_int {
    if reader.is_null() || out_buf.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — reader+out_buf+out_len NULL-checked above.
    // C-API liefert nur Alive-Samples; Lifecycle-Marker werden bewusst verworfen,
    // damit der C-FFI-Konsument unveraenderte Bytes sieht.
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
                *out_buf = ptr::null_mut();
                *out_len = 0;
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
        };
        match bytes {
            Some(bs) => {
                // Heap-Buffer uebergeben — Caller free't via zerodds_buffer_free.
                // SampleBytes -> Vec materialization an der C-FFI-Boundary.
                let mut boxed = bs.to_vec().into_boxed_slice();
                *out_buf = boxed.as_mut_ptr();
                *out_len = boxed.len();
                // Leak — Caller hat jetzt Ownership.
                let _ = Box::into_raw(boxed);
            }
            None => {
                *out_buf = ptr::null_mut();
                *out_len = 0;
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Wartet bis mindestens `min_count` Publications gematcht haben.
///
/// # Safety
/// `reader` muss valide sein.
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

/// Zerstört einen Reader. NULL-safe.
///
/// # Safety
/// Wie `zerodds_writer_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_destroy(reader: *mut ZeroDdsReader) {
    if reader.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — reader aus zerodds_reader_create (Box::into_raw).
    // Vor Destroy einen ev. registrierten Data-Callback loeschen, sonst feuert der
    // Recv-Thread mit Dangling-Listener bis der Reader-Slot aus dem Runtime-Index raus ist.
    unsafe {
        let r = &*reader;
        r.rt.set_user_reader_listener(r.eid, None);
        let _ = Box::from_raw(reader);
    }
}

/// Data-Available-Callback fuer Alive-Samples (Latenz-Optimierung).
///
/// Registriert einen synchronen Callback, der vom Recv-Thread des
/// Runtimes direkt nach Sample-Arrival aufgerufen wird. Eliminiert
/// die Polling-Latenz von `zerodds_reader_take()` (~50-100 µs raus).
///
/// `callback = NULL` loescht einen vorhandenen Listener.
///
/// **Vertrag**:
/// * Callback laeuft im Recv-Thread, NICHT im User-Thread.
/// * Kurz und nicht-blockierend.
/// * Keine ZeroDDS-API-Aufrufe rein (Recursion-Risiko).
/// * `payload` zeigt auf den CDR-Payload (ohne Encapsulation-Header).
///   Lifetime nur fuer die Dauer des Callbacks; kopieren wenn ueber
///   den Call hinaus benoetigt.
/// * Disposed-/Unregistered-Lifecycle-Events feuern den Callback
///   NICHT.
///
/// # Safety
/// `reader` muss valider Pointer aus `zerodds_reader_create` sein.
/// `user_data` ist opaque; muss durch User selbst sicher gehalten
/// werden bis der Listener mit NULL geloescht wird.
pub type ZeroDdsDataCallback =
    extern "C" fn(user_data: *mut core::ffi::c_void, payload: *const u8, payload_len: usize);

/// Setzt einen Data-Available-Callback (oder loescht ihn mit NULL).
/// Siehe `ZeroDdsDataCallback` Doc fuer den vollen Vertrag.
///
/// # Safety
/// `reader` muss aus `zerodds_reader_create` stammen.
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
            // user_data als usize speichern, weil *mut c_void nicht Send ist;
            // im Closure casten wir zurueck. Caller muss laut Contract user_data
            // lebendig halten bis Listener mit NULL geloescht wird.
            let ud_addr = user_data as usize;
            Some(Box::new(move |bytes: &[u8]| {
                cb(
                    ud_addr as *mut core::ffi::c_void,
                    bytes.as_ptr(),
                    bytes.len(),
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
// Buffer-Free (für from-take)
// ============================================================================

/// Gibt einen Buffer frei, den ein vorheriges `zerodds_reader_take`
/// alloziert hat. NULL-safe.
///
/// # Safety
/// `buf` muss aus `zerodds_reader_take` stammen oder NULL sein.
/// `len` muss exakt der zu dem Buffer gehörige Wert sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_buffer_free(buf: *mut u8, len: usize) {
    if buf.is_null() || len == 0 {
        return;
    }
    // SAFETY: see fn # Safety doc — buf+len aus zerodds_reader_take Box::into_raw.
    let _ = unsafe { Box::from_raw(slice::from_raw_parts_mut(buf, len)) };
}

// ============================================================================
// Read-Loan (Opt-1, Zero-Copy-Roadmap §6 R6)
// ============================================================================
//
// `zerodds_reader_loan/_return_loan` ist eine zero-copy Alternative zu
// `zerodds_reader_take`. Statt den Payload via `to_vec().into_boxed_slice()`
// in einen owned C-Heap-Buffer zu kopieren, hält der Loan einen
// `Arc<[u8]>`-Refcount auf den internen `SampleBytes` und gibt einen
// rohen Pointer auf die Bytes aus. Caller muss den Buffer mit
// `zerodds_reader_return_loan(loan_handle)` zurueckgeben, sobald er
// die Bytes nicht mehr braucht.
//
// Vertrag:
// - `*out_buf` ist gueltig nur solange `loan_handle` nicht returned wurde.
// - `loan_handle` ist opake (`*mut c_void`) — Caller darf den Pointer
//   nicht dereferenzieren oder ueber den Aufruf hinaus weitergeben.
// - `zerodds_reader_return_loan(NULL)` ist no-op.

/// Opaker Loan-Handle — wrappt eine `SampleBytes`-Box damit der
/// Arc-Refcount bis zum `return_loan` aufrecht bleibt.
type ZeroDdsReadLoanHandle = zerodds_dcps::sample_bytes::SampleBytes;

/// Loan-basierter `take`: liefert einen lebendigen Pointer in einen
/// internen `Arc<[u8]>` ohne Copy.
///
/// Bei Erfolg:
/// * `*out_buf` zeigt auf den Payload (read-only),
/// * `*out_len` ist die Laenge,
/// * `*out_loan_handle` ist ein opaker Pointer, der spaeter an
///   [`zerodds_reader_return_loan`] uebergeben werden muss.
///
/// Bei keinem Sample: `*out_buf = NULL`, `*out_len = 0`,
/// `*out_loan_handle = NULL`, return Ok.
///
/// # Safety
/// Alle Pointer muessen valide sein. Der returnierte `*out_buf` ist
/// nur gueltig solange `*out_loan_handle` lebt; nach
/// `zerodds_reader_return_loan` ist die Lese-Lifetime beendet.
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
    // SAFETY: see fn # Safety doc — alle Pointer NULL-checked.
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
                // Box<SampleBytes> → leak ptr; Caller gibt es via
                // return_loan zurueck. Wichtig: as_slice().as_ptr()
                // erst NACH dem Boxing, weil bs gemoved wird und das
                // Heap-Box den Arc-Refcount-Anker haelt.
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

/// Gibt einen Loan zurueck, den ein vorheriges [`zerodds_reader_loan`]
/// erzeugt hat. Nach diesem Aufruf ist der zugehoerige `*out_buf`-
/// Pointer ungueltig (Arc-Refcount geht eventuell auf 0 und die Bytes
/// werden freigegeben).
///
/// NULL-safe — ein `loan_handle = NULL` ist no-op.
///
/// # Safety
/// `loan_handle` muss aus [`zerodds_reader_loan`] stammen oder NULL sein.
/// Nicht doppelt-zurueckgeben.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_return_loan(loan_handle: *mut c_void) {
    if loan_handle.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — handle aus zerodds_reader_loan Box::into_raw.
    let _ = unsafe { Box::from_raw(loan_handle.cast::<ZeroDdsReadLoanHandle>()) };
}

// ============================================================================
// Loaning
// ============================================================================
//
// Heap-Backed Loans als Standard-Pfad. Bei aktiviertem SHM-Transport
// (siehe `zerodds-flatdata-1.0` Vendor-Spec) wird der Speicher-Pfad
// intern transparent durch SHM-Buffer-Pool-Lookup ersetzt — die
// FFI-Signaturen bleiben stabil.

/// Reserviert einen Output-Buffer beim Writer fuer Zero-Copy-Publish.
/// Caller schreibt den Sample in den zurueckgegebenen Pointer und
/// commit'd ihn dann via [`zerodds_writer_commit_loan`].
///
/// Returnt 0 (Ok) bei Erfolg + befuellt `*out_ptr` und `*out_len`.
/// Beim heutigen malloc-backed Pfad ist `*out_len = len`; bei
/// SHM-backed Loans kann `*out_len > len` sein (Slot-Boundary).
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
    // Phase-C: heap-allokierter Buffer. Phase-D: SHM-Slot-Lookup.
    let mut v = alloc::vec![0u8; len].into_boxed_slice();
    let ptr = v.as_mut_ptr();
    // Leak — Caller besitzt jetzt das Buffer-Eigentum bis commit/discard.
    let _ = Box::into_raw(v);
    // SAFETY: see fn # Safety doc — out_ptr+out_len NULL-checked above.
    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Commit-Pfad: schreibt den geliehenen Buffer als Sample und gibt
/// ihn frei. Caller darf den Pointer danach nicht mehr lesen.
///
/// # Safety
/// `writer` aus `zerodds_writer_create`; `ptr` aus
/// `zerodds_writer_loan_message`; `len` der gleiche Wert wie in
/// `out_len` zurueckgegeben.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_commit_loan(
    writer: *mut ZeroDdsWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if writer.is_null() || ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — writer+ptr NULL-checked above; ptr+len stammen
    // aus loan_message (Box::into_raw).
    //
    // Zero-Copy-Pfad (spec zerodds-zero-copy-1.0 §6 Welle 1): borrowed-Variante
    // von write_user_sample nutzen statt zerodds_writer_write (das Vec-allokiert).
    // Spart einen Vec-Roundtrip + Heap-Alloc pro commit_loan.
    let (rt, eid, payload) = unsafe {
        let w = &*writer;
        let payload = slice::from_raw_parts(ptr, len);
        (w.rt.clone(), w.eid, payload)
    };
    let rc = match rt.write_user_sample_borrowed(eid, payload) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    };
    // Buffer-Drop nach Write (Borrow-Lifetime ist bis hierhin gehalten).
    // SAFETY: ptr+len aus loan_message Box::into_raw.
    unsafe {
        let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, len));
    }
    rc
}

/// Verwirft einen Loan ohne ihn zu publishen. Buffer wird freigegeben.
///
/// # Safety
/// Wie `zerodds_writer_commit_loan`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_discard_loan(
    _writer: *mut ZeroDdsWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — ptr+len aus loan_message (Box::into_raw).
    unsafe {
        let _ = Box::from_raw(slice::from_raw_parts_mut(ptr, len));
    }
    ZeroDdsStatus::Ok as c_int
}

// ============================================================================
// Version-Info
// ============================================================================

/// Version-String des C-FFI. Statisch, nicht freizugeben.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}
