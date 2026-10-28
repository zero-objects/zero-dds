// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a probe is asked for.
//!
//! `zerodds_wire_codec::value`, re-exported whole. Nothing here names a
//! value representation, so there was nothing to pin: `ProbeRequest`,
//! `Emit` and the encapsulation helpers are the same items the engine
//! takes, not copies of them.

pub use zerodds_wire_codec::value::*;
