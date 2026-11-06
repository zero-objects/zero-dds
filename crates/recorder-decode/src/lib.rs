// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Decode DDS samples to typed outputs for `zerodds-spy` / `zerodds-record`.
//!
//! Crate `zerodds-recorder-decode`. Safety classification: **COMFORT**
//! (tooling — no runtime hot path).
//!
//! Four pieces sit on top of `zerodds_wire_codec`, which walks the model in
//! both directions:
//!
//! - [`type_source`] — load an out-of-band IDL file and resolve a named type to
//!   a [`zerodds_types::dynamic::type_::DynamicType`] (the `--type-file` path);
//! - [`wire`] — which encoding a received sample is in, from the type and the
//!   XCDR version the reader was told;
//! - `json_sink` — `DynamicData` → JSON (NDJSON);
//! - `sqlite_sink` — per-topic relational SQLite with a record marker.
//!
//! Both sinks also retain the raw CDR bytes per sample so `zerodds-replay` can
//! re-publish byte-exact from any format.

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod json_sink;
pub mod replay_source;
pub mod sqlite_sink;
pub mod type_source;
pub mod wire;

/// The serialized body of a sample and the representation identifier of the
/// encapsulation header it came with (DDSI-RTPS 2.5 §10.5): both sinks keep
/// the two together, so a replay sends the body under the header that
/// describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSample<'a> {
    /// The body, without the four octets of the encapsulation header.
    pub body: &'a [u8],
    /// The first two octets of that header; `None` where the capture did not
    /// keep them (`.zddsrec` before version 3).
    pub representation_identifier: Option<[u8; 2]>,
}
