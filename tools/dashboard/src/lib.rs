// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-dashboard` library.
//!
//! Crate `zerodds-dashboard`. Safety classification: **COMFORT**.
//!
//! # Architecture
//!
//! The `zerodds-dashboard` binary starts a mini HTTP server that:
//! 1. Serves an embedded single-page HTML dashboard.
//! 2. Offers JSON endpoints for live data (topics, participants,
//!    Histograms, Discovery-Graph).
//! 3. Has control endpoints for recording start/stop.
//!
//! data model ([`state::DashboardState`]) is **writable/readable by
//! external app** — consumers fill the state from DcpsRuntime
//! (built-in topics reader) and the dashboard server serializes it
//! as JSON. This makes the server testable without a DDS stack.
//!
//! # Tauri 2.0 as a wrapper
//!
//! The original plan F.4 envisions a Tauri 2.0 native app.
//! This phase-A implementation provides the **web backend** in
//! pure Rust with no external crate deps (no hyper, no axum, no
//! Node.js). A later Phase-B can put the same backend into a
//! embed a Tauri 2.0 app — the HTML frontend is already
//! Tauri-compatible (uses standard Web APIs).

#![warn(missing_docs)]
#![allow(clippy::print_stdout, clippy::print_stderr)]

pub mod server;
pub mod state;
pub mod web;

pub use server::{ServerError, run_blocking};
pub use state::{
    DashboardState, DiscoveryEdge, DiscoveryNode, HistogramSnapshot, ParticipantInfo,
    RecordingStatus, TopicInfo,
};
