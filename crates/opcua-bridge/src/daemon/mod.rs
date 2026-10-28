// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `zerodds-opcua-bridged` process: CLI, YAML config, SIGHUP/SIGTERM,
//! systemd `Type=notify`, observability.

pub mod cli;
pub mod config;
pub mod security;
pub mod server;
