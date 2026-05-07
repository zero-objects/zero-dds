// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stable C-ABI, basis for non-Rust bindings
//!
//! Crate `zerodds-sys`.
//!
//! Safety classification: **SAFE (Kern) / BINDING (FFI-Modul)**.
//! Siehe `docs/architecture/02_architecture.md §3`, §4.4.3, §4.4.4 und
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! Der `lib.rs`-Kern ist Safe/no_std und `#![forbid(unsafe_code)]`. Die
//! tatsaechliche C-ABI-Oberflaeche (`extern "C"` Exports, `#[no_mangle]`
//! Symbole) wird in einem separaten `mod ffi;` angelegt, das per
//! `#![allow(unsafe_code)]` die Ausnahme lokal traegt. Safe-Audits des
//! Kerns umfassen nicht das FFI-Modul.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

// ZeroDDS-Sys-Crate ist die historische C-ABI-Surface. Mit der
// Veroeffentlichung von `zerodds-c-api` (Layer 6, RC1) ist die
// vollstaendige spec-konforme C-FFI-Schnittstelle dort gebuendelt
// (~115 Funktionen, ~4100 LOC, Spec-konform DDS 1.4 §2.2.2 +
// DDS-PSM-Cxx 1.0 §7.5).
//
// Diese Crate bleibt als Workspace-Member bestehen, exportiert aber
// keine Symbole — Konsumenten verlinken stattdessen gegen
// `zerodds-c-api` (cdylib `libzerodds.dylib` / `.so` / `.dll`).
//
// Siehe `crates/zerodds-c-api/include/zerodds.h` und
// `docs/specs/zerodds-c-api-1.0.md` fuer die vollstaendige API.

/// Marker-Konstante: weist auf die voll spec-konforme C-FFI in
/// `zerodds-c-api` hin.
pub const REFERENCE_C_API_CRATE: &str = "zerodds-c-api";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_c_api_crate() {
        assert_eq!(REFERENCE_C_API_CRATE, "zerodds-c-api");
    }
}
