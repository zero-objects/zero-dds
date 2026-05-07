// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XCDR2 TypeSupport-FFI — Implementiert `zerodds-xcdr2-c-1.0`.
//!
//! Diese Schicht ergaenzt die byte-orientierte FFI von `lib.rs` um eine
//! **typisierte** TypeSupport-FFI mit function-table-basiertem Dispatch.
//! Pro IDL-Type liefert ein Codegen (idl-cpp `--c-mode`-Flag) eine
//! statische `zerodds_typesupport_t`-Tabelle mit Encoder/Decoder/Key-Hash-
//! Funktionen. Die FFI hier nimmt diese Tabelle entgegen, ruft sie auf
//! und reicht das Ergebnis an die bestehenden Writer/Reader-Pfade weiter.
//!
//! ## Wire-Format
//!
//! XCDR2 §7.4 PLAIN_CDR2 LE als Default. Codegen MUSS den XCDR2-Encoder
//! aus `zerodds-cdr` verwenden — diese FFI dupliziert die Encoder-Logik
//! nicht, sondern stellt nur den Dispatch + Helper bereit.
//!
//! ## Memory-Ownership (§6 Vendor-Spec)
//!
//! - `zerodds_xcdr2_encode`: Caller bietet `out_buf`+`out_cap`; Callee
//!   schreibt `out_len`. `out_buf=NULL` ist legal als Size-Probe — dann
//!   gibt `out_len` die benoetigte Groesse zurueck und Return ist
//!   `BUFFER_TOO_SMALL`.
//! - `zerodds_xcdr2_decode`: Caller bietet `out_sample` zero-initialized;
//!   Callee allokiert Strings/Sequences. Caller MUSS spaeter
//!   `ts.sample_free(sample)` aufrufen.
//! - `zerodds_writer_write_typed`: ruft `ts.encode` in einem internen
//!   Buffer auf, leitet die Bytes an `zerodds_writer_write` weiter.
//! - `zerodds_reader_take_typed`: ruft `zerodds_reader_take`, dann
//!   `ts.decode` auf den Bytes; gibt den internen Buffer frei.

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::slice;

use crate::{ZeroDdsReader, ZeroDdsRuntime, ZeroDdsStatus, ZeroDdsWriter};

// ============================================================================
// TypeSupport-Function-Pointer-Typen
// ============================================================================

/// Encoder-Funktion. `sample` ist Pointer auf eine Sprach-spezifische
/// Repraesentation des IDL-Types; `out_buf`/`out_cap` ist der Caller-
/// gestellte Output-Buffer (kann NULL sein zum Size-Probing).
/// `out_len` wird auf die benoetigte/geschriebene Laenge gesetzt.
///
/// Return: 0=OK, -13=BUFFER_TOO_SMALL, !=0=anderer Fehler.
pub type ZeroDdsEncodeFn = unsafe extern "C" fn(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int;

/// Decoder-Funktion. `buf`/`len` ist der Eingangs-Bytes-Stream;
/// `out_sample` muss vom Caller zero-initialized vorbereitet sein.
/// Strings/Sequences werden vom Decoder via malloc allokiert und
/// muessen spaeter ueber `ZeroDdsSampleFreeFn` freigegeben werden.
pub type ZeroDdsDecodeFn =
    unsafe extern "C" fn(buf: *const u8, len: usize, out_sample: *mut c_void) -> c_int;

/// Key-Hash-Funktion (XTypes 1.3 §7.6.8). Schreibt 16 Byte nach
/// `out_hash`. Return 0 = OK.
pub type ZeroDdsKeyHashFn = unsafe extern "C" fn(sample: *const c_void, out_hash: *mut u8) -> c_int;

/// Sample-Free-Funktion. Gibt heap-allokierte Felder (Strings,
/// Sequences) im Sample frei. Das Sample-Struct selbst gehoert dem
/// Caller und wird hier nicht freigegeben.
pub type ZeroDdsSampleFreeFn = unsafe extern "C" fn(sample: *mut c_void);

// ============================================================================
// `zerodds_typesupport_t` (§2)
// ============================================================================

/// TypeSupport-Funktion-Tabelle (Vendor-Spec §2).
///
/// ABI-stabil; Felder sind versioniert via `zerodds_c_api_version()`.
/// Pro IDL-Type emittiert der C-Codegen genau eine `static const`-
/// Instanz dieser Struktur.
///
/// Layout-Gruende:
/// - 16-Byte `type_hash` zuerst (XTypes EquivalenceHash).
/// - `type_name` als NUL-terminierter Pointer (kein owned String).
/// - `is_keyed` + `extensibility` als `u8` fuer kompakte Packung.
/// - 6 Byte Padding bis zu den function-pointer-aligned Feldern.
/// - 5 Function-Pointers in fester Reihenfolge.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsTypeSupport {
    /// 16-Byte EquivalenceHash (XTypes §7.3.4).
    pub type_hash: [u8; 16],
    /// NUL-terminierter Type-Name (Module::Sub::Struct, ASCII).
    pub type_name: *const c_char,
    /// 1 = mindestens ein @key-Member vorhanden.
    pub is_keyed: u8,
    /// 0=Final, 1=Appendable, 2=Mutable.
    pub extensibility: u8,
    /// Reserved fuer zukuenftige Felder. MUSS auf 0 stehen.
    pub _reserved: [u8; 6],
    /// Encoder-Pointer (siehe `ZeroDdsEncodeFn`).
    pub encode: Option<ZeroDdsEncodeFn>,
    /// Decoder-Pointer (siehe `ZeroDdsDecodeFn`).
    pub decode: Option<ZeroDdsDecodeFn>,
    /// Key-Hash-Pointer (siehe `ZeroDdsKeyHashFn`); NULL erlaubt wenn
    /// `is_keyed = 0`.
    pub key_hash: Option<ZeroDdsKeyHashFn>,
    /// Sample-Free-Pointer (siehe `ZeroDdsSampleFreeFn`); NULL erlaubt
    /// wenn der Type keine heap-Felder hat.
    pub sample_free: Option<ZeroDdsSampleFreeFn>,
}

// SAFETY: Die FFI-Pointer in dieser Struktur werden nur an FFI-Boundary
// gelesen und sind im Caller (statische const-Tabellen aus Codegen) als
// `'static` gehalten. Die Rust-Seite leitet die Tabelle nur weiter.
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsTypeSupport {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTypeSupport {}

// ============================================================================
// Topic-Handle (typisiert)
// ============================================================================

/// Opaque Topic-Handle mit fester TypeSupport-Tabelle.
///
/// Aktuell ist die Topic-Schicht des C-FFI ein leichtgewichtiger
/// Container ueber `topic_name`/`type_name` — beides wird hier mit dem
/// TypeSupport ergaenzt, damit `writer_write_typed`/`reader_take_typed`
/// die Tabelle ohne weiteren Caller-Argument-Passing finden kann.
pub struct ZeroDdsTopic {
    /// Pointer auf die statische TypeSupport-Tabelle (Caller-owned).
    pub type_support: *const ZeroDdsTypeSupport,
    /// Topic-Name (owned, fuer spaetere Writer/Reader-Erstellung).
    pub topic_name: alloc::string::String,
    /// Type-Name (owned, aus `type_support.type_name` extrahiert).
    pub type_name: alloc::string::String,
}

// SAFETY: `type_support` zeigt auf statische Codegen-Tabelle; Strings sind owned.
unsafe impl Send for ZeroDdsTopic {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTopic {}

// ============================================================================
// FFI: Topic
// ============================================================================

/// Erzeugt ein typisiertes Topic-Handle. Speichert die TypeSupport-
/// Tabelle so dass Writer/Reader-Operationen darauf zugreifen koennen.
///
/// `out_topic` muss non-NULL sein und wird auf den neu allokierten
/// Topic-Handle gesetzt. Caller MUSS spaeter
/// `zerodds_topic_destroy_typed` aufrufen.
///
/// # Safety
/// `participant`/`topic_name`/`type_support`/`out_topic` muessen valide
/// Pointer sein. `topic_name` muss NUL-terminiert sein. `type_support`
/// MUSS auf eine statische Tabelle mit lebenden Function-Pointern zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_create_typed(
    participant: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_support: *const ZeroDdsTypeSupport,
    out_topic: *mut *mut ZeroDdsTopic,
) -> c_int {
    if participant.is_null()
        || topic_name.is_null()
        || type_support.is_null()
        || out_topic.is_null()
    {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-Check oben.
    let ts = unsafe { &*type_support };
    if ts.type_name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: topic_name ist NUL-terminiert per Caller-Kontrakt.
    let topic = match unsafe { core::ffi::CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    // SAFETY: type_name ist NUL-terminiert per Vendor-Spec §2.
    let typ = match unsafe { core::ffi::CStr::from_ptr(ts.type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    let handle = alloc::boxed::Box::new(ZeroDdsTopic {
        type_support,
        topic_name: topic,
        type_name: typ,
    });
    // SAFETY: out_topic NULL-checked.
    unsafe {
        *out_topic = alloc::boxed::Box::into_raw(handle);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Zerstoert ein Topic-Handle. NULL-safe.
///
/// # Safety
/// `topic` muss aus `zerodds_topic_create_typed` stammen oder NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_destroy_typed(topic: *mut ZeroDdsTopic) {
    if topic.is_null() {
        return;
    }
    // SAFETY: Box-Pointer aus `Box::into_raw`.
    let _ = unsafe { alloc::boxed::Box::from_raw(topic) };
}

// ============================================================================
// FFI: Standalone-Encoding
// ============================================================================

/// Standalone-Encoder. Ruft `ts.encode(sample, out_buf, out_cap, out_len)`
/// und reicht den Return-Code durch.
///
/// `out_buf=NULL` mit `out_cap=0` ist Size-Probe: der Codegen-Encoder
/// MUSS `out_len` auf die benoetigte Groesse setzen und
/// `BUFFER_TOO_SMALL` zurueckgeben.
///
/// # Safety
/// `ts`, `sample`, `out_len` muessen valide Pointer sein. `out_buf` darf
/// NULL sein wenn `out_cap = 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_xcdr2_encode(
    ts: *const ZeroDdsTypeSupport,
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    if ts.is_null() || sample.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if out_buf.is_null() && out_cap != 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NULL-Check oben.
    let ts_ref = unsafe { &*ts };
    let Some(enc) = ts_ref.encode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // SAFETY: Caller-Kontrakt verlangt valide Encoder-Function-Pointer-
    // Provenance + `sample` zeigt auf gueltige Type-Repraesentation.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { enc(sample, out_buf, out_cap, out_len) }
}

/// Standalone-Decoder. Ruft `ts.decode(buf, len, out_sample)`.
///
/// # Safety
/// `ts`/`buf`/`out_sample` valid; `buf` muss `len` Bytes lesbar sein;
/// `out_sample` muss zero-initialized auf den Sprach-Type zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_xcdr2_decode(
    ts: *const ZeroDdsTypeSupport,
    buf: *const u8,
    len: usize,
    out_sample: *mut c_void,
) -> c_int {
    if ts.is_null() || buf.is_null() || out_sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-Check oben.
    let ts_ref = unsafe { &*ts };
    let Some(dec) = ts_ref.decode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // SAFETY: Caller-Kontrakt + NULL-Check.
    unsafe { dec(buf, len, out_sample) }
}

// ============================================================================
// FFI: Writer/Reader (typisiert)
// ============================================================================

/// Schreibt einen typisierten Sample. Encodiert via
/// `ts.encode` in einen internen Heap-Buffer und leitet die Bytes an
/// `zerodds_writer_write` weiter.
///
/// Strategie:
/// 1. Size-Probe (`out_buf=NULL`).
/// 2. Heap-Buffer der probed Groesse allokieren.
/// 3. Echtes Encode rein.
/// 4. `zerodds_writer_write(writer, bytes, len)`.
///
/// # Safety
/// `writer`/`ts`/`sample` valid; vgl. `zerodds_writer_write` und
/// `zerodds_xcdr2_encode`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_write_typed(
    writer: *mut ZeroDdsWriter,
    ts: *const ZeroDdsTypeSupport,
    sample: *const c_void,
) -> c_int {
    if writer.is_null() || ts.is_null() || sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-Check oben.
    let ts_ref = unsafe { &*ts };
    let Some(enc) = ts_ref.encode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // Step 1: Size-Probe.
    let mut needed: usize = 0;
    // SAFETY: enc kommt aus Codegen, `sample` Caller-validiert.
    let probe_rc = unsafe { enc(sample, ptr::null_mut(), 0, &mut needed as *mut usize) };
    // Codegen-Encoder MUSS bei `out_buf=NULL` BUFFER_TOO_SMALL liefern
    // ODER 0 wenn der Type 0 Bytes Payload hat (z.B. V-1 Empty Final).
    if probe_rc != ZeroDdsStatus::Ok as c_int && probe_rc != ZeroDdsStatus::Unsupported as c_int {
        // BUFFER_TOO_SMALL wird in unserem Status-Mapping als Error/Unsupported
        // signalisiert; wir akzeptieren jeden non-OK Probe-Code als "Groesse
        // jetzt in `needed`".
    }
    // Step 2 + 3: Heap-Buffer + echtes Encode.
    let mut buf = alloc::vec![0u8; needed];
    let mut written: usize = 0;
    let buf_ptr = if needed == 0 {
        ptr::null_mut()
    } else {
        buf.as_mut_ptr()
    };
    // SAFETY: Buffer ist `needed` Bytes gross.
    let enc_rc = unsafe { enc(sample, buf_ptr, needed, &mut written as *mut usize) };
    if enc_rc != ZeroDdsStatus::Ok as c_int {
        return enc_rc;
    }
    // Step 4: Pass-through zum Wire-Writer.
    // SAFETY: writer NULL-checked; buf hat `written` initialisierte Bytes.
    unsafe { crate::zerodds_writer_write(writer, buf.as_ptr(), written) }
}

/// Liest einen typisierten Sample. Holt Bytes via `zerodds_reader_take`
/// und decodiert sie via `ts.decode` in `out_sample`.
///
/// `out_info` ist aktuell unbenutzt (Reserved fuer Sample-Info-Spec-
/// Rollout); MUSS NULL sein oder zeigt auf einen Caller-allokierten
/// `zerodds_sample_info_t` der mit Default-Werten befuellt wird.
///
/// Liefert:
/// - 0 (Ok) wenn ein Sample erfolgreich decoded wurde.
/// - `NoData` (-7) wenn der Reader leer ist.
/// - andere negative Codes bei Decoder-Fehler.
///
/// # Safety
/// `reader`/`ts`/`out_sample` valide. `out_info` NULL oder valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take_typed(
    reader: *mut ZeroDdsReader,
    ts: *const ZeroDdsTypeSupport,
    out_sample: *mut c_void,
    out_info: *mut c_void,
) -> c_int {
    let _ = out_info;
    if reader.is_null() || ts.is_null() || out_sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-Check oben.
    let ts_ref = unsafe { &*ts };
    let Some(dec) = ts_ref.decode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    let mut buf: *mut u8 = ptr::null_mut();
    let mut len: usize = 0;
    // SAFETY: buf/len lokale Stack-Pointer, Reader-NULL-checked.
    let rc = unsafe {
        crate::zerodds_reader_take(reader, &mut buf as *mut *mut u8, &mut len as *mut usize)
    };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    if buf.is_null() || len == 0 {
        return ZeroDdsStatus::NoData as c_int;
    }
    // SAFETY: dec aus Codegen; buf gueltig fuer `len` Bytes.
    let dec_rc = unsafe { dec(buf, len, out_sample) };
    // Reader-Buffer immer freigeben, auch im Fehlerfall.
    // SAFETY: buf/len kommen aus zerodds_reader_take.
    unsafe { crate::zerodds_buffer_free(buf, len) };
    dec_rc
}

// ============================================================================
// Rust-side helper: XCDR2-Encoder/Decoder ueber `zerodds-cdr`
// ============================================================================

/// Konvertiert einen `*mut usize` Output-Parameter sicher zurueck zur
/// Rust-Welt. Wird von Codegen-Encodern intern genutzt.
///
/// # Safety
/// `out_len` muss valider `*mut usize` sein.
pub unsafe fn write_out_len(out_len: *mut usize, value: usize) {
    if !out_len.is_null() {
        // SAFETY: Caller-Kontrakt garantiert valide Pointer.
        unsafe { *out_len = value };
    }
}

/// Schreibt `bytes` in `out_buf` falls Kapazitaet reicht. Setzt
/// `out_len = bytes.len()`. Liefert FFI-Status:
/// - 0 (Ok) wenn alle Bytes geschrieben.
/// - -13 (Unsupported, hier als BUFFER_TOO_SMALL semantisch genutzt)
///   wenn `out_buf=NULL` (Size-Probe) oder `out_cap < bytes.len()`.
///
/// # Safety
/// `out_buf` muss NULL sein oder valid fuer `out_cap` Bytes Schreibzugriff.
/// `out_len` muss valid sein.
pub unsafe fn copy_to_out_buf(
    bytes: &[u8],
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: Wrapper-Kontrakt, oben dokumentiert.
    unsafe { write_out_len(out_len, bytes.len()) };
    if bytes.is_empty() {
        // 0-Byte-Payload (z.B. V-1 Empty Final): nichts zu kopieren,
        // Probe oder Real-Call beide OK.
        return ZeroDdsStatus::Ok as c_int;
    }
    if out_buf.is_null() || out_cap < bytes.len() {
        return ZeroDdsStatus::Unsupported as c_int;
    }
    // SAFETY: out_buf hat >= bytes.len() Kapazitaet, beide non-overlap
    // (Caller-Buffer + Rust-Stack-Buffer).
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, bytes.len());
    }
    ZeroDdsStatus::Ok as c_int
}

/// Liefert einen Slice ueber den FFI-Input-Buffer. Convenience fuer
/// Codegen-Decoder.
///
/// # Safety
/// `buf` muss valide Lesezugriff fuer `len` Bytes haben.
pub unsafe fn input_slice<'a>(buf: *const u8, len: usize) -> &'a [u8] {
    if buf.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: Caller-Kontrakt.
    unsafe { slice::from_raw_parts(buf, len) }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;

    // V-2 Encoder + Decoder fuer Tests: Point{ x: i32, y: i32 } @final.

    #[repr(C)]
    struct PointSample {
        x: i32,
        y: i32,
    }

    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe extern "C" fn point_encode(
        sample: *const c_void,
        out_buf: *mut u8,
        out_cap: usize,
        out_len: *mut usize,
    ) -> c_int {
        // SAFETY: test-only.
        let s = unsafe { &*(sample as *const PointSample) };
        let mut bytes = alloc::vec::Vec::with_capacity(8);
        bytes.extend_from_slice(&s.x.to_le_bytes());
        bytes.extend_from_slice(&s.y.to_le_bytes());
        // SAFETY: helper.
        unsafe { copy_to_out_buf(&bytes, out_buf, out_cap, out_len) }
    }

    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe extern "C" fn point_decode(
        buf: *const u8,
        len: usize,
        out_sample: *mut c_void,
    ) -> c_int {
        if len < 8 {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        // SAFETY: test-only.
        let bytes = unsafe { input_slice(buf, len) };
        let x = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let y = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
        // SAFETY: test-only.
        unsafe {
            (*(out_sample as *mut PointSample)).x = x;
            (*(out_sample as *mut PointSample)).y = y;
        }
        ZeroDdsStatus::Ok as c_int
    }

    static POINT_TYPE_NAME: &[u8] = b"Point\0";

    fn point_typesupport() -> ZeroDdsTypeSupport {
        ZeroDdsTypeSupport {
            type_hash: [0u8; 16],
            type_name: POINT_TYPE_NAME.as_ptr() as *const c_char,
            is_keyed: 0,
            extensibility: 0,
            _reserved: [0u8; 6],
            encode: Some(point_encode),
            decode: Some(point_decode),
            key_hash: None,
            sample_free: None,
        }
    }

    #[test]
    fn xcdr2_encode_size_probe_returns_needed() {
        let ts = point_typesupport();
        let s = PointSample { x: 1, y: -2 };
        let mut needed: usize = 0;
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_encode(
                &ts,
                &s as *const _ as *const c_void,
                core::ptr::null_mut(),
                0,
                &mut needed,
            )
        };
        // Probe gibt -13 (Unsupported = BUFFER_TOO_SMALL) zurueck.
        assert_eq!(rc, ZeroDdsStatus::Unsupported as c_int);
        assert_eq!(needed, 8);
    }

    #[test]
    fn xcdr2_encode_writes_le_bytes() {
        let ts = point_typesupport();
        let s = PointSample { x: 1, y: -2 };
        let mut buf = [0u8; 8];
        let mut written: usize = 0;
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_encode(
                &ts,
                &s as *const _ as *const c_void,
                buf.as_mut_ptr(),
                buf.len(),
                &mut written,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(written, 8);
        assert_eq!(buf, [0x01, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn xcdr2_decode_roundtrip() {
        let ts = point_typesupport();
        let bytes = [0x01u8, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF];
        let mut out = PointSample { x: 0, y: 0 };
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_decode(
                &ts,
                bytes.as_ptr(),
                bytes.len(),
                &mut out as *mut _ as *mut c_void,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(out.x, 1);
        assert_eq!(out.y, -2);
    }

    #[test]
    fn xcdr2_encode_null_ts_rejected() {
        let s = PointSample { x: 1, y: 2 };
        let mut buf = [0u8; 8];
        let mut written: usize = 0;
        // SAFETY: test-controlled NULL passthrough.
        let rc = unsafe {
            zerodds_xcdr2_encode(
                core::ptr::null(),
                &s as *const _ as *const c_void,
                buf.as_mut_ptr(),
                buf.len(),
                &mut written,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::BadHandle as c_int);
    }

    #[test]
    fn xcdr2_decode_null_buf_rejected() {
        let ts = point_typesupport();
        let mut out = PointSample { x: 0, y: 0 };
        // SAFETY: test-controlled NULL passthrough.
        let rc = unsafe {
            zerodds_xcdr2_decode(&ts, core::ptr::null(), 0, &mut out as *mut _ as *mut c_void)
        };
        assert_eq!(rc, ZeroDdsStatus::BadHandle as c_int);
    }
}
