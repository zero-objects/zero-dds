// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Decode DDS samples to typed outputs for `zerodds-spy` / `zerodds-record`.
//!
//! Crate `zerodds-recorder-decode`. Safety classification: **COMFORT**
//! (tooling — no runtime hot path).
//!
//! Three pieces sit on top of the reflective codec
//! ([`zerodds_types::dynamic::codec`]):
//!
//! - [`type_source`] — load an out-of-band IDL file and resolve a named type to
//!   a [`zerodds_types::dynamic::type_::DynamicType`] (the `--type-file` path);
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
