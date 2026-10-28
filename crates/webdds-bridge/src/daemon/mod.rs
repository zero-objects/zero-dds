// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-webdds-bridged` daemon implementation.
//!
//! Same shape as every other `zerodds-*-bridged` daemon
//! (`crates/coap-bridge`, `crates/websocket-bridge`): [`config`] (the
//! layered YAML schema), [`cli`] (convenience flags as `--set`
//! entries), [`security`] (TLS + auth + the bridge-security ACL wired
//! to this daemon's config), [`server`] (the accept loop, graceful
//! shutdown, the admin endpoint).

pub mod cli;
pub mod config;
pub mod runtime_common;
pub mod security;
pub mod server;
