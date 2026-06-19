// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-cs`. Safety classification: **STANDARD**.
//!
//! C# P/Invoke + NativeAOT bindings over `zerodds-c-api`. The
//! actual C# source lives under `csharp/ZeroDDS/src/` (Maven-
//! style module); the Rust lib core is only a Cargo container.
//!
//! Spec: OMG DDS-PSM-Cxx 1.0 (formal/2013-11-01) — adapted to
//! C# idioms (IDisposable, record struct).
//!
//! ## Schichten-Position
//!
//! Layer 6 — PSMs / Bindings.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! No Rust public API. The caller API is `ZeroDDS.Domain.*`,
//! `ZeroDDS.Pub.*`, `ZeroDDS.Sub.*`, `ZeroDDS.Topic.*`,
//! `ZeroDDS.Cond.*`, `ZeroDDS.Listener.*`, `ZeroDDS.Qos.*`.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke test: the crate compiles and the test harness runs.
    }
}
