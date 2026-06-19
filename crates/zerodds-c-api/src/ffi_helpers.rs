// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Safe boundary for the FFI layer.
//!
//! Each newtype encapsulates a raw-pointer operation that today stood inline
//! in the `pub unsafe extern "C" fn` bodies. The `from_raw`
//! constructors stay `unsafe fn`, because the lifetime pledge of the
//! C caller is not statically provable — the only unsafe site
//! per extern fn is exactly there, everything behind it is safe Rust.
//!
//! ## Classes
//!
//! | Newtype         | Class                   | Caller pledge |
//! |-----------------|-------------------------|---------------|
//! | `Borrowed<T>`   | Read borrow             | `*const T` valid or NULL, lives for the fn duration, not aliased-mutated |
//! | `OutPtr<T>`     | Write-only out-pointer  | `*mut T` valid + aligned + writeable, exclusive |
//! | `Owned<T>`      | Lifecycle-owned box     | `from_raw_drop`: pointer comes from `into_raw` of the same instance |
//! | `BytesIn`       | Byte-buffer input       | `ptr..ptr+len` valid initialized memory; NULL OK if len==0 |
//! | `BytesOut`      | Byte-buffer output      | `ptr..ptr+len` writeable, aligned, exclusive |
//! | `CStrIn`        | C-string input          | NUL-terminated, valid UTF-8 string |
//!
//! Note: in phase 1 no call site is migrated yet; the newtypes
//! are temporarily marked `dead_code`. Phase 2+ removes the allow.
//!
//! ## Pattern in call sites
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

// Phase-1 scaffolding: the newtypes exist but are unused until phase 2.
#![allow(dead_code)]

use core::ffi::c_int;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::ZeroDdsStatus;

// ============================================================================
// Class A — Borrowed<T>: read-only borrow of a C raw pointer
// ============================================================================

/// Read borrow of a C raw pointer.
///
/// Wraps `&T` with FFI-pointer provenance. NULL-tolerant: returns
/// `Err(BadHandle)` for NULL pointers.
pub(crate) struct Borrowed<'a, T: 'a>(&'a T);

impl<'a, T> Borrowed<'a, T> {
    /// # Safety
    /// The caller guarantees: `ptr` is NULL or points to a valid
    /// `T` instance that lives for at least `'a` and is not aliased-mutated
    /// during that time.
    #[inline]
    pub unsafe fn from_raw(ptr: *const T) -> Result<Self, ZeroDdsStatus> {
        // SAFETY: NULL check via `as_ref`; otherwise caller pledge.
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
// Class B — OutPtr<T>: consuming write-only out-pointer
// ============================================================================

/// Write-only out-pointer for extern fn results.
///
/// Consuming → guarantees exactly one write at the type-system level.
pub(crate) struct OutPtr<T>(NonNull<T>);

impl<T> OutPtr<T> {
    /// # Safety
    /// The caller guarantees: `ptr` is NULL or points to a
    /// writeable, correctly aligned `T` slot.
    #[inline]
    pub unsafe fn from_raw(ptr: *mut T) -> Result<Self, ZeroDdsStatus> {
        NonNull::new(ptr)
            .map(OutPtr)
            .ok_or(ZeroDdsStatus::BadParameter)
    }

    /// Writes `value` into the out slot. Consumes `self`.
    #[inline]
    pub fn write(self, value: T) {
        // SAFETY: NonNull guarantees non-NULL; the caller pledge from from_raw
        // guarantees a valid writeable aligned slot.
        unsafe { self.0.as_ptr().write(value) }
    }
}

// ============================================================================
// Class C — Owned<T>: lifecycle-owned box wrapper
// ============================================================================

/// Owned box wrapper for FFI lifecycle pairs.
///
/// `into_raw` and `from_raw_drop` are the spec-protected pair
/// — whoever calls `into_raw` must later call `from_raw_drop`.
pub(crate) struct Owned<T>(Box<T>);

impl<T> Owned<T> {
    /// Allocates on the heap and returns a box wrapper.
    #[inline]
    pub fn new(value: T) -> Self {
        Owned(Box::new(value))
    }

    /// Converts into a raw pointer for the FFI return. The caller owns it.
    #[inline]
    pub fn into_raw(self) -> *mut T {
        Box::into_raw(self.0)
    }

    /// # Safety
    /// `ptr` comes from `Owned::into_raw` of the same `T` instance and
    /// has not yet been dropped. NULL is allowed (no-op).
    #[inline]
    pub unsafe fn from_raw_drop(ptr: *mut T) {
        if !ptr.is_null() {
            // SAFETY: caller pledge: comes from into_raw, do not double-drop.
            let _ = unsafe { Box::from_raw(ptr) };
        }
    }
}

// ============================================================================
// Class D — BytesIn / BytesOut: byte buffer
// ============================================================================

/// Read borrow of a `(*const u8, usize)` byte buffer.
pub(crate) struct BytesIn<'a>(&'a [u8]);

impl<'a> BytesIn<'a> {
    /// # Safety
    /// `ptr..ptr+len` is valid initialized memory for `'a`,
    /// not aliased-mutated. NULL is allowed only if `len == 0`.
    #[inline]
    pub unsafe fn from_raw(ptr: *const u8, len: usize) -> Result<Self, ZeroDdsStatus> {
        if ptr.is_null() {
            if len == 0 {
                return Ok(BytesIn(&[]));
            }
            return Err(ZeroDdsStatus::BadParameter);
        }
        // SAFETY: NULL check + len pledge.
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

/// Write borrow of a `(*mut u8, usize)` byte buffer.
pub(crate) struct BytesOut<'a>(&'a mut [u8]);

impl<'a> BytesOut<'a> {
    /// # Safety
    /// `ptr..ptr+len` is writeable, correctly aligned, exclusive
    /// for `'a`. NULL is allowed only if `len == 0`.
    #[inline]
    pub unsafe fn from_raw(ptr: *mut u8, len: usize) -> Result<Self, ZeroDdsStatus> {
        if ptr.is_null() {
            if len == 0 {
                return Ok(BytesOut(&mut []));
            }
            return Err(ZeroDdsStatus::BadParameter);
        }
        // SAFETY: NULL check + len pledge.
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
// Class E — CStrIn: NUL-terminated UTF-8 input
// ============================================================================

/// Read borrow of a NUL-terminated `*const c_char` as a UTF-8 `&str`.
pub(crate) struct CStrIn<'a>(&'a str);

impl<'a> CStrIn<'a> {
    /// # Safety
    /// `ptr` is NULL or points to a NUL-terminated byte sequence
    /// that lives for `'a`. The content must be valid UTF-8, otherwise
    /// `Err(InvalidUtf8)`.
    #[inline]
    pub unsafe fn from_raw(ptr: *const core::ffi::c_char) -> Result<Self, ZeroDdsStatus> {
        if ptr.is_null() {
            return Err(ZeroDdsStatus::BadParameter);
        }
        // SAFETY: NULL check + caller pledge for NUL termination.
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
// Status adapter
// ============================================================================

/// Converts `Result<T, ZeroDdsStatus>` to `c_int` for the extern fn return.
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
        // SAFETY: a NULL pointer is allowed input for from_raw.
        let r: Result<Borrowed<'_, u32>, _> = unsafe { Borrowed::from_raw(ptr::null()) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadHandle)));
    }

    #[test]
    fn borrowed_valid_derefs() {
        let x: u32 = 42;
        // SAFETY: x lives for the test, &x is a valid pointer.
        let b = unsafe { Borrowed::from_raw(&x as *const u32) }.unwrap();
        assert_eq!(*b, 42);
    }

    #[test]
    fn outptr_null_returns_bad_parameter() {
        // SAFETY: NULL is allowed input for from_raw.
        let r: Result<OutPtr<u32>, _> = unsafe { OutPtr::from_raw(ptr::null_mut()) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadParameter)));
    }

    #[test]
    fn outptr_writes_exactly_once() {
        let mut x: u32 = 0;
        // SAFETY: x lives for the test as a writeable slot.
        let out = unsafe { OutPtr::from_raw(&mut x as *mut u32) }.unwrap();
        out.write(99);
        assert_eq!(x, 99);
    }

    #[test]
    fn owned_roundtrip_drops_on_from_raw() {
        let o = Owned::new(String::from("alive"));
        let raw = o.into_raw();
        // SAFETY: raw comes from into_raw, is not double-dropped.
        unsafe { Owned::from_raw_drop(raw) };
    }

    #[test]
    fn owned_from_raw_drop_null_is_noop() {
        // SAFETY: NULL is explicitly allowed (no-op).
        unsafe { Owned::<u32>::from_raw_drop(ptr::null_mut()) };
    }

    #[test]
    fn bytesin_null_with_len_zero_ok() {
        // SAFETY: NULL+len=0 is explicitly allowed input.
        let b = unsafe { BytesIn::from_raw(ptr::null(), 0) }.unwrap();
        assert!(b.is_empty());
    }

    #[test]
    fn bytesin_null_with_len_nonzero_errors() {
        // SAFETY: NULL+len>0 must return BadParameter (test).
        let r = unsafe { BytesIn::from_raw(ptr::null(), 4) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadParameter)));
    }

    #[test]
    fn bytesin_valid_derefs() {
        let data = [1u8, 2, 3, 4];
        // SAFETY: data lives for the test as a valid 4-byte buffer.
        let b = unsafe { BytesIn::from_raw(data.as_ptr(), data.len()) }.unwrap();
        assert_eq!(&*b, &data[..]);
    }

    #[test]
    fn bytesout_null_with_len_zero_ok() {
        // SAFETY: NULL+len=0 is explicitly allowed input.
        let b = unsafe { BytesOut::from_raw(ptr::null_mut(), 0) }.unwrap();
        assert!(b.is_empty());
    }

    #[test]
    fn bytesout_valid_writes() {
        let mut buf = [0u8; 4];
        {
            // SAFETY: buf lives for the block as a writeable 4-byte slot.
            let mut b = unsafe { BytesOut::from_raw(buf.as_mut_ptr(), buf.len()) }.unwrap();
            b[0] = 0xAA;
            b[3] = 0xBB;
        }
        assert_eq!(buf, [0xAA, 0, 0, 0xBB]);
    }

    #[test]
    fn cstrin_null_errors() {
        // SAFETY: NULL is explicitly forbidden input — must be BadParameter.
        let r = unsafe { CStrIn::from_raw(ptr::null()) };
        assert!(matches!(r, Err(ZeroDdsStatus::BadParameter)));
    }

    #[test]
    fn cstrin_valid_utf8_derefs() {
        let s = c"hello";
        // SAFETY: c"hello" is NUL-terminated UTF-8.
        let c = unsafe { CStrIn::from_raw(s.as_ptr()) }.unwrap();
        assert_eq!(&*c, "hello");
    }

    #[test]
    fn cstrin_invalid_utf8_errors() {
        // Bytes 0xFF, 0x00 — invalid UTF-8 + NUL.
        let bad: [core::ffi::c_char; 2] = [-1, 0];
        // SAFETY: bad is NUL-terminated, but invalid UTF-8 — tests the validation.
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
