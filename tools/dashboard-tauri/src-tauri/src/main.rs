// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Tauri 2.0 native app wrapper for the ZeroDDS dashboard.
//!
//! Embeds the HTTP server from the `zerodds-dashboard` crate
//! in-process; the frontend is the same SPA as in standalone
//! mode. Additionally the native app exposes a Tauri command API
//! through which the frontend / a plugin via `__TAURI__.invoke()`
//! can push state updates — faster than the round-trip over
//! HTTP POST and without CORS.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::thread;

use zerodds_dashboard::DashboardState;
use tauri::{Manager, State};

/// Global state container for Tauri.
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
            // On DevTools builds auto-open on macOS/Linux.
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
/// Same semantics as POST /api/inject/topics.
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
// Whoever wants to bypass the HTTP API (e.g. because CSP has a fetch() block),
// gets here the same JSON body as a Tauri command result.

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
