//! ZeroDDS Inspect-Endpoint.
//!
//! Crate `zerodds-inspect-endpoint`. Safety classification: **STANDARD**.
//! Debug-Tool-Crate, voll `#[cfg(feature = "inspect")]`-gated und
//! `#![forbid(unsafe_code)]`. Im Release-Build (Feature aus) faellt
//! der gesamte Tap-Mechanismus weg (R-099, C-021).
//!
//! Hidden God-Mode-Debug-Pfad (PDE-Spec R-097, C-020) fuer den
//! externen Reality Inspector (zerodds-pde). Wenn das Cargo-Feature
//! `inspect` **aus** ist, exportiert dieses Crate **keine** Funktionen
//! die in den DDS-Hot-Path eingreifen — der gesamte Tap-Mechanismus
//! ist `#[cfg(feature = "inspect")]`-gated und faellt im Release-Build
//! weg (R-099, C-021).
//!
//! ## Architektur
//!
//! - [`tap`] — Trait + Registry fuer Tap-Hooks an DCPS/RTPS/Transport.
//! - [`frame`] — Wire-Frame fuer den Side-Channel.
//! - [`auth`] — `cert.d`-Loader fuer X.509-PEM-Certs (R-100..R-104).
//!
//! ## Sicherheits-Invarianten
//!
//! - **Ghost-Inject** (R-110): Inject-Funktionen sind separate API-
//!   Pfade, sie publishen direkt in den DDS-Production-Datenpfad,
//!   **ohne** durch die Tap-Hooks zu laufen. Dadurch sehen
//!   Production-Taps den Inject NICHT — alte DDS-Stacks ohne Security
//!   wuerden sonst chicken-out.
//! - **Idle-Branch-Elision**: Tap-Hook-Aufrufe in dcps/rtps/transport
//!   sind hinter `#[cfg(feature = "inspect")]` versteckt. Ohne
//!   Feature kein Branch im Hot-Path.

#![cfg_attr(not(feature = "inspect"), allow(dead_code))]
#![forbid(unsafe_code)]

pub mod auth;
pub mod frame;
#[cfg(feature = "inspect")]
pub mod server;
pub mod tap;

#[cfg(feature = "inspect")]
pub use server::{BroadcastHook, InspectServer, ServerConfig};
#[cfg(feature = "inspect")]
pub use tap::{TapHook, register_dcps_tap, register_rtps_tap, register_transport_tap};

pub use frame::{Frame, FrameKind};
