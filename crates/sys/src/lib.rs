// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Marker crate; the C ABI of ZeroDDS is `zerodds-c-api`.
//!
//! Crate `zerodds-sys`. Safety classification: **SAFE**.
//!
//! It exports no `extern "C"` symbols and holds only
//! [`REFERENCE_C_API_CRATE`]. C and other non-Rust consumers link
//! `zerodds-c-api` (`libzerodds.so` / `.dylib` / `.dll`, header
//! `crates/zerodds-c-api/include/zerodds.h`, spec
//! `docs/specs/zerodds-c-api-1.0.md`). `#![forbid(unsafe_code)]`, `no_std`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

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
