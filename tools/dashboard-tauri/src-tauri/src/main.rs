// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Tauri 2.0 Native-App-Wrapper für das ZeroDDS-Dashboard.
//!
//! Embedded den HTTP-Server aus dem `zerodds-dashboard`-Crate
//! in-process; das Frontend ist die gleiche SPA wie im Standalone-
//! Mode. Zusaetzlich exposed das Native-App eine Tauri-Command-API
//! ueber die das Frontend / ein Plugin via `__TAURI__.invoke()`
//! State-Updates pushen kann — schneller als der Round-Trip ueber
//! HTTP-POST und ohne CORS.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::thread;

use zerodds_dashboard::DashboardState;
use tauri::{Manager, State};

/// Globaler State-Container für Tauri.
struct DashboardCtx {
    state: Arc<DashboardState>,
}

fn main() {
    let state = Arc::new(DashboardState::new());

    // HTTP-Server im Hintergrund: Default 127.0.0.1:8089.
    let bind = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8089));
    {
        let s = Arc::clone(&state);
        thread::spawn(move || {
            let _ = zerodds_dashboard::run_blocking(bind, s);
        });
    }

    let ctx = DashboardCtx {
        state: Arc::clone(&state),
    };

    tauri::Builder::default()
        .manage(ctx)
        .invoke_handler(tauri::generate_handler![
            inject_topics,
            inject_participants,
            inject_histograms,
            get_topics,
            get_participants,
            get_histograms,
            get_graph,
            get_recording,
        ])
        .setup(|app| {
            // Bei DevTools-Builds Auto-Open auf macOS/Linux.
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---- Inject-Commands (Frontend → Backend) ----

/// `__TAURI__.invoke('inject_topics', { json: '...' })`
/// Gleiche Semantik wie POST /api/inject/topics.
#[tauri::command]
fn inject_topics(json: String, ctx: State<DashboardCtx>) -> Result<usize, String> {
    ctx.state.inject_topics_json(&json)
}

#[tauri::command]
fn inject_participants(json: String, ctx: State<DashboardCtx>) -> Result<usize, String> {
    ctx.state.inject_participants_json(&json)
}

#[tauri::command]
fn inject_histograms(json: String, ctx: State<DashboardCtx>) -> Result<usize, String> {
    ctx.state.inject_histograms_json(&json)
}

// ---- Read-Only-Commands (Backend → Frontend) ----
// Wer das HTTP-API umgehen will (z.B. weil CSP fetch()-Block hat),
// bekommt hier den gleichen JSON-Body als Tauri-Command-Result.

#[tauri::command]
fn get_topics(ctx: State<DashboardCtx>) -> String {
    ctx.state.topics_json()
}

#[tauri::command]
fn get_participants(ctx: State<DashboardCtx>) -> String {
    ctx.state.participants_json()
}

#[tauri::command]
fn get_histograms(ctx: State<DashboardCtx>) -> String {
    ctx.state.histograms_json()
}

#[tauri::command]
fn get_graph(ctx: State<DashboardCtx>) -> String {
    ctx.state.graph_json()
}

#[tauri::command]
fn get_recording(ctx: State<DashboardCtx>) -> String {
    ctx.state.recording_json()
}
