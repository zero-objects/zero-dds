// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CLIENT_Representation` / `AGENT_Representation` codec — moved to
//! [`zerodds_xrce::handshake`] so `zerodds-xrce-client` can share it for
//! building its own `CREATE_CLIENT` (previously duplicated per-crate; see
//! that module's doc for the wire format, the byte-oracle evidence behind
//! [`DEFAULT_MTU`]/`requested_mtu`, and the full test suite). Re-exported
//! here so every existing `crate::daemon::handshake::*` reference in this
//! crate keeps working unchanged.

pub use zerodds_xrce::handshake::*;
