// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-grpc-bridged` — DDS↔gRPC bridge daemon.
//
// Spec: `docs/specs/zerodds-grpc-bridge-1.0.md`. The daemon itself is
// `zerodds_grpc_bridge::bridge_daemon`.

//! `zerodds-grpc-bridged`: the DDS↔gRPC bridge daemon.
#![allow(clippy::print_stderr)]

use std::process::ExitCode;

/// On Windows, started by the service control manager, the daemon runs as
/// the service `ZeroDDSGrpcBridge` (Spec §11); elsewhere and from a
/// console, as it is.
fn main() -> ExitCode {
    zerodds_dcps::cli::run_as_service(
        zerodds_grpc_bridge::bridge_daemon::WINDOWS_SERVICE,
        daemon_main,
    )
}

fn daemon_main() -> ExitCode {
    match zerodds_grpc_bridge::bridge_daemon::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
