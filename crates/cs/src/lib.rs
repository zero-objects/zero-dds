// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-cs`. Safety classification: **STANDARD**.
//!
//! C# P/Invoke + NativeAOT-Bindings ueber `zerodds-c-api`. Die
//! eigentliche C#-Source lebt unter `csharp/ZeroDDS/src/` (Maven-
//! style Modul); der Rust-Lib-Kern ist nur Cargo-Container.
//!
//! Spec: OMG DDS-PSM-Cxx 1.0 (formal/2013-11-01) — adaptiert auf
//! C#-Idiome (IDisposable, record struct).
//!
//! ## Schichten-Position
//!
//! Layer 6 — PSMs / Bindings.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! Keine Rust-Public-API. Caller-API ist `ZeroDDS.Domain.*`,
//! `ZeroDDS.Pub.*`, `ZeroDDS.Sub.*`, `ZeroDDS.Topic.*`,
//! `ZeroDDS.Cond.*`, `ZeroDDS.Listener.*`, `ZeroDDS.Qos.*`.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke-Test: Crate kompiliert und Testharness laeuft.
    }
}
