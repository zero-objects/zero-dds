// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stable C-ABI, basis for non-Rust bindings
//!
//! Crate `zerodds-sys`.
//!
//! Safety classification: **SAFE (core) / BINDING (FFI module)**.
//! See `docs/architecture/02_architecture.md §3`, §4.4.3, §4.4.4 and
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! The `lib.rs` core is safe/no_std and `#![forbid(unsafe_code)]`. The
//! actual C-ABI surface (`extern "C"` exports, `#[no_mangle]`
//! symbols) is placed in a separate `mod ffi;`, which carries the
//! exception locally via `#![allow(unsafe_code)]`. Safe audits of the
//! core do not cover the FFI module.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

// The zerodds-sys crate is the historical C-ABI surface. With the
// release of `zerodds-c-api` (Layer 6, RC1), the complete
// spec-compliant C-FFI interface is bundled there
// (~115 functions, ~4100 LOC, spec-compliant DDS 1.4 §2.2.2 +
// DDS-PSM-Cxx 1.0 §7.5).
//
// This crate remains a workspace member but exports no
// symbols — consumers link against
// `zerodds-c-api` instead (cdylib `libzerodds.dylib` / `.so` / `.dll`).
//
// See `crates/zerodds-c-api/include/zerodds.h` and
// `docs/specs/zerodds-c-api-1.0.md` for the complete API.

/// Marker constant: points to the fully spec-compliant C-FFI in
/// `zerodds-c-api`.
pub const REFERENCE_C_API_CRATE: &str = "zerodds-c-api";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_c_api_crate() {
        assert_eq!(REFERENCE_C_API_CRATE, "zerodds-c-api");
    }
}
