// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Safe-Boundary fuer die FFI-Schicht.
//!
//! Jeder Newtype kapselt eine Rohzeiger-Operation, die heute inline
//! in den `pub unsafe extern "C" fn` Bodies stand. Die `from_raw`
//! Konstruktoren bleiben `unsafe fn`, weil der Lifetime-Pledge des
//! C-Callers nicht statisch beweisbar ist — die einzige unsafe-Stelle
//! pro extern fn ist genau dort, alles dahinter ist safe Rust.
//!
//! ## Klassen
//!
//! | Newtype         | Klasse                  | Caller-Pledge |
//! |-----------------|-------------------------|---------------|
//! | `Borrowed<T>`   | Read-Borrow             | `*const T` valide oder NULL, lebt fuer fn-Dauer, nicht aliased mutiert |
//! | `OutPtr<T>`     | Write-Only Out-Pointer  | `*mut T` valide + aligned + beschreibbar, exklusiv |
//! | `Owned<T>`      | Lifecycle-Owned Box     | `from_raw_drop`: Pointer stammt aus `into_raw` derselben Instanz |
//! | `BytesIn`       | Byte-Buffer-Eingang     | `ptr..ptr+len` valides initialisiertes Memory; NULL OK falls len==0 |
//! | `BytesOut`      | Byte-Buffer-Ausgang     | `ptr..ptr+len` beschreibbar, aligned, exklusiv |
//! | `CStrIn`        | C-String-Eingang        | NUL-terminierter, valide UTF-8 String |
//!
//! Hinweis: in Phase 1 ist noch keine Call-Site migriert; die Newtypes
//! sind temporaer als `dead_code` markiert. Phase 2+ entfernt das Allow.
//!
//! ## Pattern in Call-Sites
//!
//! ```ignore
//! #[unsafe(no_mangle)]
//! pub unsafe extern "C" fn zerodds_xyz(p: *mut Foo, out: *mut Bar) -> c_int {
//!     // SAFETY: see fn # Safety doc — caller pledges validity.
//!     status(unsafe {
//!         (|| -> Result<(), ZeroDdsStatus> {
//!             let pp = Borrowed::from_raw(p)?;
//!             let out = OutPtr::from_raw(out)?;
//!             out.write(pp.some_safe_method());
//!             Ok(())
//!         })()
//!     })
//! }
//! ```

// Phase-1-Scaffolding: Newtypes existieren, sind aber bis Phase 2 ungenutzt.
#![allow(dead_code)]

use core::ffi::c_int;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::ZeroDdsStatus;

// ============================================================================
// Klasse A — Borrowed<T>: read-only Borrow eines C-Rohzeigers
// ============================================================================

/// Read-Borrow eines C-Rohzeigers.
///
/// Wraps `&T` mit FFI-Pointer-Herkunft. NULL-tolerant: gibt
/// `Err(BadHandle)` bei NULL-Pointern.
pub(crate) struct Borrowed<'a, T: 'a>(&'a T);

impl<'a, T> Borrowed<'a, T> {
    /// # Safety
    /// Caller garantiert: `ptr` ist NULL oder zeigt auf eine valide
    /// `T`-Instanz, die fuer mindestens `'a` lebt und in dem Zeitraum
    /// nicht aliased mutiert wird.
    #[inline]
    pub unsafe fn from_raw(ptr: *const T) -> Result<Self, ZeroDdsStatus> {
        // SAFETY: NULL-Check via `as_ref`; ansonsten Caller-Pledge.
        match unsafe { ptr.as_ref() } {
            Some(r) => Ok(Borrowed(r)),
            None => Err(ZeroDdsStatus::BadHandle),
        }
    }
}

impl<T> Deref for Borrowed<'_, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.0
    }
}

// ============================================================================
// Klasse B — OutPtr<T>: konsumierender Write-Only Out-Pointer
// ============================================================================

/// Write-Only Out-Pointer fuer extern fn-Resultate.
///
/// Konsumierend → garantiert genau einen Write am Typsystem.
pub(crate) struct OutPtr<T>(NonNull<T>);

impl<T> OutPtr<T> {
    /// # Safety
    /// Caller garantiert: `ptr` ist NULL oder zeigt auf einen
    /// beschreibbaren, korrekt aligned `T`-Slot.
    #[inline]
    pub unsafe fn from_raw(ptr: *mut T) -> Result<Self, ZeroDdsStatus> {
        NonNull::new(ptr)
            .map(OutPtr)
            .ok_or(ZeroDdsStatus::BadParameter)
    }

    /// Schreibt `value` in den Out-Slot. Konsumiert `self`.
    #[inline]
    pub fn write(self, value: T) {
        // SAFETY: NonNull garantiert non-NULL; Caller-Pledge aus from_raw
        // garantiert valider beschreibbarer aligned Slot.
        unsafe { self.0.as_ptr().write(value) }
    }
}

// ============================================================================
// Klasse C — Owned<T>: Lifecycle-Owned Box-Wrapper
// ============================================================================

/// Owned Box-Wrapper fuer FFI-Lifecycle-Pairs.
///
/// `into_raw` und `from_raw_drop` sind das spec-geschützte Paar
/// — wer `into_raw` ruft, muss spaeter `from_raw_drop` rufen.
pub(crate) struct Owned<T>(Box<T>);

impl<T> Owned<T> {
    /// Allokiert auf Heap und gibt einen Box-Wrapper zurueck.
    #[inline]
    pub fn new(value: T) -> Self {
        Owned(Box::new(value))
    }

    /// Konvertiert in Raw-Pointer fuer FFI-Rueckgabe. Caller besitzt.
    #[inline]
    pub fn into_raw(self) -> *mut T {
        Box::into_raw(self.0)
    }

    /// # Safety
    /// `ptr` stammt aus `Owned::into_raw` derselben `T`-Instanz und
    /// wurde noch nicht ge-drop'pt. NULL ist erlaubt (no-op).
    #[inline]
    pub unsafe fn from_raw_drop(ptr: *mut T) {
        if !ptr.is_null() {
            // SAFETY: Caller-Pledge: stammt aus into_raw, nicht doppelt-droppen.
            let _ = unsafe { Box::from_raw(ptr) };
        }
    }
}

// ============================================================================
// Klasse D — BytesIn / BytesOut: Byte-Buffer
// ============================================================================

/// Read-Borrow eines `(*const u8, usize)` Byte-Buffers.
pub(crate) struct BytesIn<'a>(&'a [u8]);

impl<'a> BytesIn<'a> {
    /// # Safety
    /// `ptr..ptr+len` ist valides initialisiertes Memory fuer `'a`,
    /// nicht aliased mutiert. NULL ist erlaubt nur falls `len == 0`.
    #[inline]
    pub unsafe fn from_raw(ptr: *const u8, len: usize) -> Result<Self, ZeroDdsStatus> {
        if ptr.is_null() {
            if len == 0 {
                return Ok(BytesIn(&[]));
            }
            return Err(ZeroDdsStatus::BadParameter);
        }
        // SAFETY: NULL-Check + len-Pledge.
        Ok(BytesIn(unsafe { core::slice::from_raw_parts(ptr, len) }))
    }
}

impl Deref for BytesIn<'_> {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        self.0
    }
}

/// Write-Borrow eines `(*mut u8, usize)` Byte-Buffers.
pub(crate) struct BytesOut<'a>(&'a mut [u8]);

impl<'a> BytesOut<'a> {
    /// # Safety
    /// `ptr..ptr+len` ist beschreibbar, korrekt aligned, exklusiv
    /// fuer `'a`. NULL ist erlaubt nur falls `len == 0`.
    #[inline]
    pub unsafe fn from_raw(ptr: *mut u8, len: usize) -> Result<Self, ZeroDdsStatus> {
        if ptr.is_null() {
            if len == 0 {
                return Ok(BytesOut(&mut []));
            }
            return Err(ZeroDdsStatus::BadParameter);
        }
        // SAFETY: NULL-Check + len-Pledge.
        Ok(BytesOut(unsafe {
            core::slice::from_raw_parts_mut(ptr, len)
        }))
    }
}

impl Deref for BytesOut<'_> {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        self.0
    }
}

impl DerefMut for BytesOut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.0
    }
}

// ============================================================================
// Klasse E — CStrIn: NUL-terminierter UTF-8 Eingang
// ============================================================================

/// Read-Borrow eines NUL-terminierten `*const c_char` als UTF-8 `&str`.
pub(crate) struct CStrIn<'a>(&'a str);

impl<'a> CStrIn<'a> {
    /// # Safety
    /// `ptr` ist NULL oder zeigt auf eine NUL-terminierte Byte-Sequenz
    /// die fuer `'a` lebt. Inhalt muss valide UTF-8 sein, sonst
    /// `Err(InvalidUtf8)`.
    #[inline]
    pub unsafe fn from_raw(ptr: *const core::ffi::c_char) -> Result<Self, ZeroDdsStatus> {
        if ptr.is_null() {
            return Err(ZeroDdsStatus::BadParameter);
        }
        // SAFETY: NULL-Check + Caller-Pledge fuer NUL-Terminierung.
        let cs = unsafe { core::ffi::CStr::from_ptr(ptr) };
        cs.to_str()
            .map(CStrIn)
            .map_err(|_| ZeroDdsStatus::InvalidUtf8)
    }
}

impl Deref for CStrIn<'_> {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.0
    }
}

// ============================================================================
// Status-Adapter
// ============================================================================

/// Konvertiert `Result<T, ZeroDdsStatus>` zu `c_int` fuer extern fn return.
#[inline]
pub(crate) fn status<T>(r: Result<T, ZeroDdsStatus>) -> c_int {
    match r {
        Ok(_) => ZeroDdsStatus::Ok as c_int,
        Err(s) => s as c_int,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr;

    #[test]
    fn borrowed_null_returns_bad_handle() {
        // SAFETY: NULL-Pointer ist erlaubter Input fuer from_raw.
        let r: Result<Borrowed<'_, u32>, _> = unsafe { Borrowed::from_raw(ptr::null()) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadHandle)));
    }

    #[test]
    fn borrowed_valid_derefs() {
        let x: u32 = 42;
        // SAFETY: x lebt fuer den Test, &x ist valider Pointer.
        let b = unsafe { Borrowed::from_raw(&x as *const u32) }.unwrap();
        assert_eq!(*b, 42);
    }

    #[test]
    fn outptr_null_returns_bad_parameter() {
        // SAFETY: NULL ist erlaubter Input fuer from_raw.
        let r: Result<OutPtr<u32>, _> = unsafe { OutPtr::from_raw(ptr::null_mut()) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadParameter)));
    }

    #[test]
    fn outptr_writes_exactly_once() {
        let mut x: u32 = 0;
        // SAFETY: x lebt fuer den Test als beschreibbarer Slot.
        let out = unsafe { OutPtr::from_raw(&mut x as *mut u32) }.unwrap();
        out.write(99);
        assert_eq!(x, 99);
    }

    #[test]
    fn owned_roundtrip_drops_on_from_raw() {
        let o = Owned::new(String::from("alive"));
        let raw = o.into_raw();
        // SAFETY: raw stammt aus into_raw, wird nicht doppelt gedroppt.
        unsafe { Owned::from_raw_drop(raw) };
    }

    #[test]
    fn owned_from_raw_drop_null_is_noop() {
        // SAFETY: NULL ist explizit erlaubt (no-op).
        unsafe { Owned::<u32>::from_raw_drop(ptr::null_mut()) };
    }

    #[test]
    fn bytesin_null_with_len_zero_ok() {
        // SAFETY: NULL+len=0 ist explizit erlaubter Input.
        let b = unsafe { BytesIn::from_raw(ptr::null(), 0) }.unwrap();
        assert!(b.is_empty());
    }

    #[test]
    fn bytesin_null_with_len_nonzero_errors() {
        // SAFETY: NULL+len>0 muss BadParameter zurueckliefern (Test).
        let r = unsafe { BytesIn::from_raw(ptr::null(), 4) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadParameter)));
    }

    #[test]
    fn bytesin_valid_derefs() {
        let data = [1u8, 2, 3, 4];
        // SAFETY: data lebt fuer den Test als valides 4-Byte-Buffer.
        let b = unsafe { BytesIn::from_raw(data.as_ptr(), data.len()) }.unwrap();
        assert_eq!(&*b, &data[..]);
    }

    #[test]
    fn bytesout_null_with_len_zero_ok() {
        // SAFETY: NULL+len=0 ist explizit erlaubter Input.
        let b = unsafe { BytesOut::from_raw(ptr::null_mut(), 0) }.unwrap();
        assert!(b.is_empty());
    }

    #[test]
    fn bytesout_valid_writes() {
        let mut buf = [0u8; 4];
        {
            // SAFETY: buf lebt fuer den Block als beschreibbarer 4-Byte-Slot.
            let mut b = unsafe { BytesOut::from_raw(buf.as_mut_ptr(), buf.len()) }.unwrap();
            b[0] = 0xAA;
            b[3] = 0xBB;
        }
        assert_eq!(buf, [0xAA, 0, 0, 0xBB]);
    }

    #[test]
    fn cstrin_null_errors() {
        // SAFETY: NULL ist explizit verbotener Input — muss BadParameter sein.
        let r = unsafe { CStrIn::from_raw(ptr::null()) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadParameter)));
    }

    #[test]
    fn cstrin_valid_utf8_derefs() {
        let s = c"hello";
        // SAFETY: c"hello" ist NUL-terminierter UTF-8.
        let c = unsafe { CStrIn::from_raw(s.as_ptr()) }.unwrap();
        assert_eq!(&*c, "hello");
    }

    #[test]
    fn cstrin_invalid_utf8_errors() {
        // Bytes 0xFF, 0x00 — invalid UTF-8 + NUL.
        let bad: [core::ffi::c_char; 2] = [-1, 0];
        // SAFETY: bad ist NUL-terminiert, aber invalid UTF-8 — Test der Validierung.
        let r = unsafe { CStrIn::from_raw(bad.as_ptr()) };
        assert!(matches!(r, Err(ZeroDdsStatus::InvalidUtf8)));
    }

    #[test]
    fn status_ok_returns_zero() {
        let r: Result<(), ZeroDdsStatus> = Ok(());
        assert_eq!(status(r), 0);
    }

    #[test]
    fn status_err_returns_negative() {
        let r: Result<(), ZeroDdsStatus> = Err(ZeroDdsStatus::BadHandle);
        assert_eq!(status(r), ZeroDdsStatus::BadHandle as c_int);
    }
}
