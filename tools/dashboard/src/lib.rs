// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-dashboard` Bibliothek.
//!
//! Crate `zerodds-dashboard`. Safety classification: **COMFORT**.
//!
//! # Architektur
//!
//! Die `zerodds-dashboard`-Binary startet einen Mini-HTTP-Server, der:
//! 1. Ein eingebettetes Single-Page-HTML-Dashboard ausliefert.
//! 2. JSON-Endpoints fuer Live-Daten anbietet (Topics, Participants,
//!    Histograms, Discovery-Graph).
//! 3. Steuer-Endpoints fuer Recording-Start/Stop hat.
//!
//! Datenmodell ([`state::DashboardState`]) ist **schreib-/lesbar von
//! externer App** — Konsumenten füllen den State aus DcpsRuntime
//! (Built-in-Topics-Reader) und der Dashboard-Server serialisiert ihn
//! als JSON. Damit ist der Server testbar ohne DDS-Stack.
//!
//! # Tauri 2.0 als Wrapper
//!
//! Der ursprueliche Plan F.4 sieht eine Tauri-2.0-Native-App vor.
//! Diese Phase-A-Implementierung liefert das **Web-Backend** in
//! reinem Rust ohne externe Crate-Deps (kein hyper, kein axum, kein
//! Node.js). Eine spaetere Phase-B kann das gleiche Backend in eine
//! Tauri-2.0-App einbetten — der HTML-Frontend ist bereits
//! Tauri-kompatibel (verwendet Standard-Web-APIs).

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
