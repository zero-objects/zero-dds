//! ZeroDDS Inspect-Endpoint.
//!
//! Crate `zerodds-inspect-endpoint`. Safety classification: **STANDARD**.
//! Debug-tool crate, fully `#[cfg(feature = "inspect")]`-gated and
//! `#![forbid(unsafe_code)]`. In the release build (feature off) the
//! entire tap mechanism is dropped (R-099, C-021).
//!
//! A hidden god-mode debug path (PDE spec R-097, C-020) for the
//! external Reality Inspector (zerodds-pde). When the Cargo feature
//! `inspect` is **off**, this crate exports **no** functions
//! that touch the DDS hot path — the entire tap mechanism
//! is `#[cfg(feature = "inspect")]`-gated and is dropped in the release build
//! (R-099, C-021).
//!
//! ## Architecture
//!
//! - [`tap`] — trait + registry for tap hooks on DCPS/RTPS/transport.
//! - [`frame`] — wire frame for the side channel.
//! - [`auth`] — `cert.d` loader for X.509 PEM certs (R-100..R-104).
//!
//! ## Security invariants
//!
//! - **Ghost inject** (R-110): inject functions are separate API
//!   paths; they publish directly into the DDS production data path,
//!   **without** going through the tap hooks. As a result
//!   production taps do NOT see the inject — old DDS stacks without security
//!   would otherwise chicken out.
//! - **Idle-branch elision**: tap-hook calls in dcps/rtps/transport
//!   are hidden behind `#[cfg(feature = "inspect")]`. Without the
//!   feature there is no branch in the hot path.

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
